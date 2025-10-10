// 新しいレイアウトシステム - 完全に再設計
// 子要素から親要素への計算（Bottom-Up）を基本とし、
// width/height の優先度を明確化した汎用レイアウトエンジン

use crate::parser::ast::{Style, Edges, DimensionValue, RelativeEdges, Unit};
use crate::parser::ast::{ViewNode, WithSpan, Expr, App};
use crate::engine::state::format_text;
use crate::stencil::stencil::Stencil as DrawStencil;

// テキスト測定: Native環境とWASM環境で異なる実装を使用
#[cfg(any(feature = "glyphon", target_arch = "wasm32"))]
use crate::ui::text_measurement::{TextMeasurement, get_text_measurement_system};

use std::collections::HashMap;

/// 2つのスタイルをマージ（second が first を上書き）
fn merge_styles(first: Option<&Style>, second: Option<&Style>) -> Style {
    match (first, second) {
        (None, None) => Style::default(),
        (Some(f), None) => f.clone(),
        (None, Some(s)) => s.clone(),
        (Some(f), Some(s)) => {
            let mut merged = f.clone();
            // second のスタイルで first を上書き（None でない場合のみ）
            if s.color.is_some() { merged.color = s.color.clone(); }
            if s.background.is_some() { merged.background = s.background.clone(); }
            if s.border_color.is_some() { merged.border_color = s.border_color.clone(); }
            if s.font_size.is_some() { merged.font_size = s.font_size; }
            if s.relative_font_size.is_some() { merged.relative_font_size = s.relative_font_size.clone(); }
            if s.width.is_some() { merged.width = s.width; }
            if s.height.is_some() { merged.height = s.height; }
            if s.relative_width.is_some() { merged.relative_width = s.relative_width.clone(); }
            if s.relative_height.is_some() { merged.relative_height = s.relative_height.clone(); }
            if s.rounded.is_some() { merged.rounded = s.rounded; }
            // 他の必要なフィールドも追加...
            merged
        }
    }
}

/// レイアウト結果（ノード＋座標・サイズ）
#[derive(Debug, Clone)]
pub struct LayoutedNode<'a> {
    pub node: &'a WithSpan<ViewNode>,
    pub position: [f32; 2],
    pub size: [f32; 2],
}

/// レイアウトの初期パラメータ（後方互換性のため維持）
#[derive(Debug, Clone)]
pub struct LayoutParams {
    pub start: [f32; 2],
    pub spacing: f32,
    /// ウィンドウサイズ（相対単位計算用）
    pub window_size: [f32; 2],
    /// 親要素サイズ（%計算用）
    pub parent_size: [f32; 2],
    /// ルートフォントサイズ（rem計算用）
    pub root_font_size: f32,
    /// 現在のフォントサイズ（em計算用）
    pub font_size: f32,
    /// デフォルトフォント名
    pub default_font: String,
}

impl Default for LayoutParams {
    fn default() -> Self {
        // デフォルト値をハードコードではなく、よりリアルな初期値で設定
        // 実際の使用時は make_layout_params や with_window_size を使用することを推奨
        Self {
            start: [0.0, 0.0],
            spacing: 12.0,
            window_size: [800.0, 600.0], // より一般的なデフォルトサイズ
            parent_size: [800.0, 600.0],
            root_font_size: 16.0,
            font_size: 16.0,
            default_font: "system-ui".to_string(), // システムフォントを優先
        }
    }
}

impl LayoutParams {
    /// ウィンドウサイズから適切なLayoutParamsを作成
    pub fn with_window_size(window_size: [f32; 2]) -> Self {
        Self {
            start: [0.0, 0.0],
            spacing: 12.0,
            window_size,
            parent_size: window_size,
            root_font_size: 16.0,
            font_size: 16.0,
            default_font: "system-ui".to_string(),
        }
    }

    /// システム設定から動的に取得したLayoutParamsを作成
    pub fn from_system_defaults() -> Self {
        // 将来的にはシステムからフォントサイズやDPIなどを取得
        Self::with_window_size([800.0, 600.0])
    }
}

/// レイアウトコンテキスト
#[derive(Debug, Clone)]
pub struct LayoutContext {
    /// ウィンドウサイズ（相対単位計算用）
    pub window_size: [f32; 2],
    /// 親要素サイズ（%計算用）
    pub parent_size: [f32; 2],
    /// ルートフォントサイズ（rem計算用）
    pub root_font_size: f32,
    /// 現在のフォントサイズ（em計算用）
    pub font_size: f32,
    /// デフォルトフォント名
    pub default_font: String,
}

impl From<&LayoutParams> for LayoutContext {
    fn from(params: &LayoutParams) -> Self {
        Self {
            window_size: params.window_size,
            parent_size: params.parent_size,
            root_font_size: params.root_font_size,
            font_size: params.font_size,
            default_font: params.default_font.clone(),
        }
    }
}

/// 計算されたサイズ情報
#[derive(Debug, Clone, Default)]
pub struct ComputedSize {
    pub width: f32,
    pub height: f32,
    /// 内在的サイズ（コンテンツが要求するサイズ）
    pub intrinsic_width: f32,
    pub intrinsic_height: f32,
    /// 明示的に指定されたかどうか
    pub has_explicit_width: bool,
    pub has_explicit_height: bool,
}

/// 新しいレイアウトエンジン
pub struct LayoutEngine {
    /// コンポーネントのキャッシュ
    component_cache: HashMap<String, ComputedSize>,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            component_cache: HashMap::new(),
        }
    }

    /// ノードのサイズを計算（メイン関数）
    pub fn compute_node_size<F, G>(
        &mut self,
        node: &WithSpan<ViewNode>,
        context: &LayoutContext,
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        // ComponentCallの場合は特別処理
        if let ViewNode::ComponentCall { name, args: _ } = &node.node {
            return self.compute_component_size_with_style(node, name, context, eval, get_image_size, app);
        }
        
        self.compute_node_size_internal(node, context, eval, get_image_size, app)
    }
    
    /// ノードのサイズを計算（内部関数、ComponentCallチェックなし）
    fn compute_node_size_internal<F, G>(
        &mut self,
        node: &WithSpan<ViewNode>,
        context: &LayoutContext,
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        // 1. スタイルから明示的なサイズを取得（最優先）
        let mut computed = self.get_explicit_size_from_style(node.style.as_ref(), context);
        
        // 2. 明示的な幅がある場合は子要素のコンテキストに適用（VStack/HStackのみでなく全ノード対象）
        let child_context = if computed.has_explicit_width {
            let mut new_context = context.clone();
            new_context.parent_size[0] = computed.width;
            new_context
        } else {
            context.clone()
        };
        
        // 3. 内在的サイズを計算（子要素から計算）
        let intrinsic = self.compute_intrinsic_size(node, &child_context, eval, get_image_size, app);
        
        // 4. 明示的でない部分は内在的サイズを使用
        if !computed.has_explicit_width {
            computed.width = intrinsic.width;
            computed.intrinsic_width = intrinsic.width;
        }
        if !computed.has_explicit_height {
            computed.height = intrinsic.height;
            computed.intrinsic_height = intrinsic.height;
        }
        
        // 5. min/max制約を適用
        self.apply_size_constraints(&mut computed, node.style.as_ref(), context);
        
        computed
    }

    /// スタイルから明示的なサイズを取得
    fn get_explicit_size_from_style(&self, style: Option<&Style>, context: &LayoutContext) -> ComputedSize {
        let mut computed = ComputedSize::default();
        
        if let Some(style) = style {
            // width の優先順位: width > relative_width > width_expr
            if let Some(width) = style.width {
                computed.width = width;
                computed.has_explicit_width = true;
            } else if let Some(ref relative_width) = style.relative_width {
                computed.width = self.resolve_dimension_value(relative_width, context, true);
                computed.has_explicit_width = true;
            } else if let Some(ref width_expr) = style.width_expr {
                // 計算式を評価
                if let Some(resolved_width) = self.eval_dimension_expr(width_expr, context, true) {
                    computed.width = resolved_width;
                    computed.has_explicit_width = true;
                }
            }
            
            // height の優先順位: height > relative_height > height_expr
            if let Some(height) = style.height {
                computed.height = height;
                computed.has_explicit_height = true;
            } else if let Some(ref relative_height) = style.relative_height {
                computed.height = self.resolve_dimension_value(relative_height, context, false);
                computed.has_explicit_height = true;
            } else if let Some(ref height_expr) = style.height_expr {
                // 計算式を評価
                if let Some(resolved_height) = self.eval_dimension_expr(height_expr, context, false) {
                    computed.height = resolved_height;
                    computed.has_explicit_height = true;
                }
            }
        }
        
        computed
    }

    /// 内在的サイズを計算（子要素から）
    fn compute_intrinsic_size<F, G>(
        &mut self,
        node: &WithSpan<ViewNode>,
        context: &LayoutContext,
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        match &node.node {
            ViewNode::Text { format, args } => {
                self.compute_text_size(format, args, node.style.as_ref(), context, eval)
            }
            ViewNode::Button { label, .. } => {
                self.compute_button_size(label, node.style.as_ref(), context)
            }
            ViewNode::Image { path } => {
                self.compute_image_size(path, get_image_size)
            }
            ViewNode::VStack(children) => {
                self.compute_vstack_size(children, node.style.as_ref(), context, eval, get_image_size, app)
            }
            ViewNode::HStack(children) => {
                self.compute_hstack_size(children, node.style.as_ref(), context, eval, get_image_size, app)
            }
            ViewNode::ComponentCall { name, args } => {
                self.compute_component_size_with_style(node, name, context, eval, get_image_size, app)
            }
            ViewNode::Spacing(dimension_value) => {
                let pixel_size = self.resolve_dimension_value(dimension_value, context, true);
                ComputedSize {
                    width: pixel_size,
                    height: pixel_size,
                    intrinsic_width: pixel_size,
                    intrinsic_height: pixel_size,
                    has_explicit_width: true,
                    has_explicit_height: true,
                }
            }
            ViewNode::Stencil(st) => {
                let size = self.compute_stencil_size(st);
                ComputedSize {
                    width: size[0],
                    height: size[1],
                    intrinsic_width: size[0],
                    intrinsic_height: size[1],
                    has_explicit_width: false,
                    has_explicit_height: false,
                }
            }
            ViewNode::ForEach { var, iterable, body } => {
                self.compute_foreach_size(var, iterable, body, context, eval, get_image_size, app)
            }
            ViewNode::DynamicSection { name: _, body } => {
                // DynamicSectionの内容のサイズを計算
                self.compute_vstack_size(body, None, context, eval, get_image_size, app)
            }
            // 状態操作ノード（Set, RustCallなど）はUIに干渉しない
            ViewNode::Set { .. } | ViewNode::RustCall { .. } => {
                ComputedSize {
                    width: 0.0,
                    height: 0.0,
                    intrinsic_width: 0.0,
                    intrinsic_height: 0.0,
                    has_explicit_width: true,
                    has_explicit_height: true,
                }
            }
            _ => {
                // その他のノードはデフォルトサイズ
                ComputedSize {
                    width: 100.0,
                    height: 30.0,
                    intrinsic_width: 100.0,
                    intrinsic_height: 30.0,
                    has_explicit_width: false,
                    has_explicit_height: false,
                }
            }
        }
    }

    /// テキストサイズを計算
    fn compute_text_size<F>(
        &self,
        format: &str,
        args: &[Expr],
        style: Option<&Style>,
        context: &LayoutContext,
        eval: &F,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
    {
        let values: Vec<String> = args.iter().map(|e| eval(e)).collect();
        let text = format_text(format, &values);
        
        // フォントサイズを取得（relative_font_sizeも考慮）
        let font_size = if let Some(style) = style {
            if let Some(ref rel_size) = style.relative_font_size {
                // relative_font_sizeがある場合は解決する
                self.resolve_dimension_value(rel_size, context, false)
            } else {
                style.font_size.unwrap_or(context.font_size)
            }
        } else {
            context.font_size
        };
        
        // フォントファミリーを取得
        let font_family = if let Some(style) = style {
            style.font_family.as_deref().unwrap_or(&context.default_font)
        } else {
            &context.default_font
        };
        
        // パディングを計算
        let padding = self.get_padding_from_style(style, context);
        
        // max_widthを考慮（パディングを差し引く）
        // 注意: ウィンドウサイズは使用せず、常に親要素のサイズを基準とする
        // デフォルトはauto（親要素の幅を利用）
        let max_width = if let Some(style) = style {
            if let Some(ref max_w) = style.max_width {
                if max_w.unit == Unit::Auto {
                    // 親要素の幅を常に利用可能幅として使用（>0でなければ0を許容）
                    let available_width = (context.parent_size[0] - padding.left - padding.right).max(0.0);
                    Some(available_width)
                } else {
                    let calculated_width = self.resolve_dimension_value(max_w, context, true);
                    // 親要素のサイズも考慮して制限
                    let available_width = if context.parent_size[0] > 0.0 {
                        calculated_width.min(context.parent_size[0] - padding.left - padding.right)
                    } else { calculated_width };
                    Some(available_width.max(0.0))
                }
            } else {
                // max_widthが指定されていない場合、デフォルトでautoとして扱う
                if context.parent_size[0] > 0.0 {
                    let available_width = (context.parent_size[0] - padding.left - padding.right).max(0.0);
                    Some(available_width)
                } else {
                    None
                }
            }
        } else {
            // スタイルが指定されていない場合もデフォルトでautoとして扱う
            if context.parent_size[0] > 0.0 {
                let available_width = (context.parent_size[0] - padding.left - padding.right).max(0.0);
                Some(available_width)
            } else {
                None
            }
        };
        
        // テキスト測定
        let measurement = self.measure_text(&text, font_size, font_family, max_width);
        
        ComputedSize {
            width: measurement.width + padding.left + padding.right,
            height: measurement.height + padding.top + padding.bottom,
            intrinsic_width: measurement.width + padding.left + padding.right,
            intrinsic_height: measurement.height + padding.top + padding.bottom,
            has_explicit_width: false,
            has_explicit_height: false,
        }
    }

    /// ボタンサイズを計算
    fn compute_button_size(&self, label: &str, style: Option<&Style>, context: &LayoutContext) -> ComputedSize {
        // フォントサイズを取得
        let font_size = if let Some(style) = style {
            style.font_size.unwrap_or(context.font_size)
        } else {
            context.font_size
        };
        
        // フォントファミリーを取得
        let font_family = if let Some(style) = style {
            style.font_family.as_deref().unwrap_or(&context.default_font)
        } else {
            &context.default_font
        };
        
        // テキスト測定
        let measurement = self.measure_text(label, font_size, font_family, None);
        
        // ボタンのパディング
        let button_padding = 20.0;
        let min_button_width = 120.0;
        let min_button_height = 48.0;
        
        ComputedSize {
            width: (measurement.width + button_padding * 2.0).max(min_button_width),
            height: (measurement.height + button_padding).max(min_button_height),
            intrinsic_width: (measurement.width + button_padding * 2.0).max(min_button_width),
            intrinsic_height: (measurement.height + button_padding).max(min_button_height),
            has_explicit_width: false,
            has_explicit_height: false,
        }
    }

    /// 画像サイズを計算
    fn compute_image_size<G>(&self, path: &str, get_image_size: &G) -> ComputedSize
    where
        G: Fn(&str) -> (u32, u32),
    {
        let (img_w, img_h) = get_image_size(path);
        
        ComputedSize {
            width: img_w as f32,
            height: img_h as f32,
            intrinsic_width: img_w as f32,
            intrinsic_height: img_h as f32,
            has_explicit_width: false,
            has_explicit_height: false,
        }
    }

    /// VStackサイズを計算（子要素から積み上げ）
    fn compute_vstack_size<F, G>(
        &mut self,
        children: &[WithSpan<ViewNode>],
        parent_style: Option<&Style>,
        context: &LayoutContext,
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        // VStackの最終的な幅を事前に決定
        // 親が利用可能幅を提示している場合（>0）は、それを優先して使用
        let has_parent_width = context.parent_size[0] > 0.0;
        
        // VStackの幅を決定（親幅またはスタイルから）
        let vstack_width = if has_parent_width {
            context.parent_size[0]
        } else {
            // 親幅が不明な場合、スタイルから幅を取得（または0）
            if let Some(style) = parent_style {
                if let Some(w) = style.width {
                    w
                } else if let Some(ref rw) = style.relative_width {
                    self.resolve_dimension_value(rw, context, true)
                } else {
                    0.0
                }
            } else {
                0.0
            }
        };
        
        // Paddingを考慮して子要素に渡す利用可能幅を計算
        let padding = if let Some(style) = parent_style {
            style.padding.unwrap_or(Edges { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 })
        } else {
            Edges { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 }
        };
        let available_width_for_children = (vstack_width - padding.left - padding.right).max(0.0);
        
        // パス1: 子要素のサイズを計算（VStackの利用可能幅を使用）
        let mut max_width: f32 = 0.0;
        let mut child_sizes = Vec::new();
        
        let mut child_context = context.clone();
        child_context.parent_size = [available_width_for_children, context.parent_size[1]];
        
        for child in children.iter() {
            let child_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
            child_sizes.push(child_size.clone());
            max_width = max_width.max(child_size.width);
        }
        
        // VStackの最終的な幅を決定
        let final_width = if vstack_width > 0.0 {
            vstack_width
        } else {
            // 幅が確定していない場合、子要素の最大幅 + padding
            max_width + padding.left + padding.right
        };
        
        // 高さの合計を計算
        let mut total_height: f32 = 0.0;
        
        for (i, child_size) in child_sizes.iter().enumerate() {
            total_height += child_size.height;
            
            // スペーシングを追加（最後の要素以外、親のスタイルから取得）
            if i < children.len() - 1 {
                total_height += self.get_spacing_from_style(parent_style, context);
            }
        }
        
        // Paddingを高さに追加
        total_height += padding.top + padding.bottom;
        
        ComputedSize {
            width: final_width,
            height: total_height,
            intrinsic_width: max_width + padding.left + padding.right,
            intrinsic_height: total_height,
            // 親が幅を与えている（>0）なら、明示的幅として扱う（トップレベル=ウィンドウ幅も含む）
            has_explicit_width: context.parent_size[0] > 0.0,
            has_explicit_height: false,
        }
    }

    /// HStackサイズを計算（子要素から横に並べる）
    fn compute_hstack_size<F, G>(
        &mut self,
        children: &[WithSpan<ViewNode>],
        parent_style: Option<&Style>,
        context: &LayoutContext,
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        let mut total_width: f32 = 0.0;
        let mut max_height: f32 = 0.0;
        
        // HStackの最終的な幅を事前に決定
        // 親が利用可能幅を提示している場合（>0）は常にそれを使用する
        let has_parent_width = context.parent_size[0] > 0.0;
        let final_width_early = if has_parent_width {
            context.parent_size[0]
        } else {
            0.0 // パス1で計算する
        };
        
        // パス1: 子要素のサイズを計算してHStackのサイズを決定
        let mut child_sizes = Vec::new();
        
        for (i, child) in children.iter().enumerate() {
            // 親幅がある場合、相対幅を持つ子要素にはそれを伝える
            let child_context = if has_parent_width {
                let mut new_context = context.clone();
                new_context.parent_size = [final_width_early, context.parent_size[1]];
                new_context
            } else {
                context.clone()
            };
            
            let child_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
            child_sizes.push(child_size.clone());
            total_width += child_size.width;
            if i < children.len() - 1 {
                // レイアウト時と同じスペーシングの取り扱いに合わせる（親スタイルから）
                total_width += self.get_spacing_from_style(parent_style, context);
            }
            max_height = max_height.max(child_size.height);
        }
        
        // HStackの最終的なサイズを決定
        let final_height = if context.parent_size[1] > 0.0 && context.parent_size[1] != context.window_size[1] {
            // 親から明示的な高さが指定されている場合
            context.parent_size[1]
        } else {
            max_height
        };
        
        // HStackの最終的な幅を確定
        let final_width = if has_parent_width {
            final_width_early
        } else {
            total_width
        };
        
        // パス2: 親幅がなく、最終幅が確定した場合のみ再計算
        let needs_recompute = !has_parent_width && final_width != total_width;
        
        if needs_recompute {
            // 最終幅が確定したので再計算
            total_width = 0.0;
            max_height = 0.0;
            
            for (i, child) in children.iter().enumerate() {
                let mut child_context = context.clone();
                
                // 子要素が相対幅を持つ場合、HStack全体の幅を親幅として使用
                let child_has_relative_width = if let Some(style) = &child.style {
                    style.relative_width.is_some()
                } else {
                    false
                };
                
                let child_parent_width = if child_has_relative_width {
                    final_width
                } else {
                    child_sizes[i].width
                };
                
                child_context.parent_size = [child_parent_width, final_height];
                
                let child_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
                total_width += child_size.width;
                if i < children.len() - 1 {
                    total_width += self.get_spacing_from_style(parent_style, context);
                }
                max_height = max_height.max(child_size.height);
            }
        }
        
        ComputedSize {
            width: final_width,
            height: max_height,
            intrinsic_width: if needs_recompute { total_width } else { total_width },
            intrinsic_height: max_height,
            has_explicit_width: false,
            has_explicit_height: false,
        }
    }

    /// コンポーネントサイズを計算（コンポーネント定義のスタイルを考慮）
    fn compute_component_size<F>(
        &mut self,
        name: &str,
        _args: &[Expr],
        context: &LayoutContext,
        eval: &F,
        app: &App,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
    {
        // コンポーネント定義を探す
        if let Some(component) = app.components.iter().find(|c| c.name == name) {
            // コンポーネントの本体の最初のノード（通常はVStack）のスタイルを考慮
            if let Some(first_node) = component.body.first() {
                let computed_size = self.compute_node_size(first_node, context, eval, &|_| (100, 100), app);
                computed_size
            } else {
                // 空のコンポーネントの場合
                ComputedSize {
                    width: 0.0,
                    height: 0.0,
                    intrinsic_width: 0.0,
                    intrinsic_height: 0.0,
                    has_explicit_width: false,
                    has_explicit_height: false,
                }
            }
        } else {
            // コンポーネントが見つからない場合のデフォルト
            ComputedSize {
                width: 200.0,
                height: 100.0,
                intrinsic_width: 200.0,
                intrinsic_height: 100.0,
                has_explicit_width: false,
                has_explicit_height: false,
            }
        }
    }

    /// ForEach文のサイズを計算（各アイテムの展開を事前計算）
    fn compute_foreach_size<F, G>(
        &mut self,
        var: &str,
        iterable: &Expr,
        body: &[WithSpan<ViewNode>],
        context: &LayoutContext,
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        // 反復対象の評価
        let iterable_value = eval(iterable);
        let items: Vec<String> = if iterable_value.starts_with('[') && iterable_value.ends_with(']') {
            // JSON配列として解析を試行
            serde_json::from_str::<Vec<serde_json::Value>>(&iterable_value)
                .map(|vs| vs.into_iter().map(|v| match v {
                    serde_json::Value::String(s) => s,
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => v.to_string().trim_matches('"').to_string(),
                }).collect())
                .unwrap_or_else(|_| vec![iterable_value])
        } else {
            vec![iterable_value]
        };

        let mut total_height: f32 = 0.0;
        let mut max_width: f32 = 0.0;

        // 各アイテムに対してボディの各ノードのサイズを計算
        for (index, item) in items.iter().enumerate() {
            // bodyの各ノードのサイズを直接計算（再帰を避けるため compute_node_size_safe を使用）
            for child in body {
                // ForEach内の子要素は簡略計算を使用して再帰を制限
                let child_size = self.compute_node_size_safe(child, context, eval, get_image_size, app);
                
                total_height += child_size.height;
                if child_size.width > max_width {
                    max_width = child_size.width;
                }
            }
            
            // アイテム間のスペーシングを追加（最後のアイテム以外）
            if index < items.len() - 1 {
                total_height += context.root_font_size * 0.5; // スペーシング
            }
        }

        ComputedSize {
            width: max_width,
            height: total_height,
            intrinsic_width: max_width,
            intrinsic_height: total_height,
            has_explicit_width: false,
            has_explicit_height: false,
        }
    }

    /// ForEach文内での再帰を避けるためのサイズ計算（安全版）
    fn compute_node_size_safe<F, G>(
        &mut self,
        node: &WithSpan<ViewNode>,
        context: &LayoutContext,
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        // ForEachノードの場合は固定サイズを返して再帰を避ける
        match &node.node {
            ViewNode::ForEach { .. } => {
                ComputedSize {
                    width: 200.0,
                    height: 100.0,
                    intrinsic_width: 200.0,
                    intrinsic_height: 100.0,
                    has_explicit_width: false,
                    has_explicit_height: false,
                }
            }
            ViewNode::Text { format, args } => {
                self.compute_text_size(format, args, node.style.as_ref(), context, eval)
            }
            ViewNode::Button { label, .. } => {
                self.compute_button_size(label, node.style.as_ref(), context)
            }
            ViewNode::Image { path } => {
                self.compute_image_size(path, get_image_size)
            }
            ViewNode::ComponentCall { name, .. } => {
                self.compute_component_size_with_style(node, name, context, eval, get_image_size, app)
            }
            _ => {
                // その他のノードはデフォルトサイズ
                ComputedSize {
                    width: 100.0,
                    height: 30.0,
                    intrinsic_width: 100.0,
                    intrinsic_height: 30.0,
                    has_explicit_width: false,
                    has_explicit_height: false,
                }
            }
        }
    }
    
    /// ComponentCallのスタイル優先度システム付きでコンポーネントサイズを計算
    fn compute_component_size_with_priority<F, G>(
        &mut self,
        name: &str,
        _args: &[Expr], 
        context: &LayoutContext,
        eval: &F,
        get_image_size: &G,
        app: &App,
        override_width: Option<bool>,
        override_height: Option<bool>,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        // コンポーネント定義を探す
        if let Some(component) = app.components.iter().find(|c| c.name == name) {
            if let Some(first_node) = component.body.first() {
                
                // コンポーネントのデフォルトスタイルを基準として開始
                let mut merged_style = component.default_style.clone();
                
                // コンポーネント本体のスタイルとマージ（本体が優先）
                if let Some(body_style) = &first_node.style {
                    merged_style = Some(merge_styles(merged_style.as_ref(), Some(body_style)));
                }
                
                if let Some(ref mut style) = merged_style {
                    // ComponentCallでwidth/heightが指定されている場合、本体の同じ属性を無効化
                    if override_width == Some(true) {
                        style.width = None;
                        style.relative_width = None;
                    }
                    if override_height == Some(true) {
                        style.height = None;
                        style.relative_height = None;
                    }
                }
                
                // 修正されたスタイルで新しいノードを作成
                let modified_node = WithSpan {
                    node: first_node.node.clone(),
                    line: first_node.line,
                    column: first_node.column,
                    style: merged_style.clone(),
                };
                
                self.compute_node_size_internal(&modified_node, context, eval, get_image_size, app)
            } else {
                ComputedSize {
                    width: 0.0,
                    height: 0.0,
                    intrinsic_width: 0.0,
                    intrinsic_height: 0.0,
                    has_explicit_width: false,
                    has_explicit_height: false,
                }
            }
        } else {
            ComputedSize {
                width: 200.0,
                height: 100.0,
                intrinsic_width: 200.0,
                intrinsic_height: 100.0,
                has_explicit_width: false,
                has_explicit_height: false,
            }
        }
    }

    /// ComponentCallのスタイルを考慮したサイズ計算
    fn compute_component_size_with_style<F, G>(
        &mut self,
        node: &WithSpan<ViewNode>,
        name: &str,
        context: &LayoutContext,
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        // 1. ComponentCallノード自体のスタイルから明示的なサイズを取得
        let explicit = self.get_explicit_size_from_style(node.style.as_ref(), context);
        
        // 2. 常に優先度システムを使用してComponentCallのスタイルを優先する
        
        // ComponentCallの明示的なスタイルを優先度システムに渡す
        let override_width = explicit.has_explicit_width;
        let override_height = explicit.has_explicit_height;
        
        let intrinsic = self.compute_component_size_with_priority(
            name, 
            &[], 
            context, 
            eval, 
            get_image_size,
            app,
            Some(override_width),
            Some(override_height),
        );
        
        // ComponentCallのスタイルがある場合はそれを優先
        let mut computed = if override_width || override_height {
            ComputedSize {
                width: if override_width { explicit.width } else { intrinsic.width },
                height: if override_height { explicit.height } else { intrinsic.height },
                intrinsic_width: intrinsic.intrinsic_width,
                intrinsic_height: intrinsic.intrinsic_height,
                has_explicit_width: override_width || intrinsic.has_explicit_width,
                has_explicit_height: override_height || intrinsic.has_explicit_height,
            }
        } else {
            intrinsic
        };
        
        // min/max制約を適用
        self.apply_size_constraints(&mut computed, node.style.as_ref(), context);
        computed
    }

    /// Stencilサイズを計算
    fn compute_stencil_size(&self, st: &DrawStencil) -> [f32; 2] {
        match st {
            DrawStencil::Rect { width, height, .. } => [*width, *height],
            DrawStencil::RoundedRect { width, height, .. } => [*width, *height],
            DrawStencil::Circle { radius, .. } => [radius * 2.0, radius * 2.0],
            DrawStencil::Triangle { p1, p2, p3, .. } => {
                let min_x = p1[0].min(p2[0].min(p3[0]));
                let max_x = p1[0].max(p2[0].max(p3[0]));
                let min_y = p1[1].min(p2[1].min(p3[1]));
                let max_y = p1[1].max(p2[1].max(p3[1]));
                [max_x - min_x, max_y - min_y]
            }
            DrawStencil::Text { content, size, .. } => {
                let w = (content.chars().count() as f32) * size * 0.6;
                [w, size * 1.2]
            }
            DrawStencil::Image { width, height, .. } => [*width, *height],
            _ => [0.0, 0.0],
        }
    }

    /// サイズ制約を適用（min/max）
    fn apply_size_constraints(&self, computed: &mut ComputedSize, style: Option<&Style>, context: &LayoutContext) {
        if let Some(style) = style {
            // min_width制約
            if let Some(ref min_w) = style.min_width {
                let min_width = self.resolve_dimension_value(min_w, context, true);
                computed.width = computed.width.max(min_width);
            }
            
            // max_width制約
            if let Some(ref max_w) = style.max_width {
                if max_w.unit != Unit::Auto {
                    let max_width = self.resolve_dimension_value(max_w, context, true);
                    computed.width = computed.width.min(max_width);
                }
            }
            
            // min_height制約
            if let Some(ref min_h) = style.min_height {
                let min_height = self.resolve_dimension_value(min_h, context, false);
                computed.height = computed.height.max(min_height);
            }
        }
    }

    /// 相対単位を絶対値に変換
    fn resolve_dimension_value(&self, dim: &DimensionValue, context: &LayoutContext, is_width: bool) -> f32 {
        let result = match dim.unit {
            Unit::Px => dim.value,
            Unit::Percent => {
                if is_width {
                    dim.value * context.parent_size[0] / 100.0
                } else {
                    dim.value * context.parent_size[1] / 100.0
                }
            }
            Unit::Vw => dim.value * context.window_size[0] / 100.0,
            Unit::Vh => dim.value * context.window_size[1] / 100.0,
            Unit::Ww => dim.value * context.window_size[0] / 100.0,
            Unit::Wh => dim.value * context.window_size[1] / 100.0,
            Unit::Em => dim.value * context.font_size,
            Unit::Rem => dim.value * context.root_font_size,
            Unit::Auto => {
                // Autoの場合は親サイズを使用（ウィンドウサイズではなく）
                if is_width {
                    context.parent_size[0]
                } else {
                    context.parent_size[1]
                }
            }
            Unit::PercentHeight => dim.value * context.parent_size[1] / 100.0,
        };
        result
    }

    /// 計算式（Expr）を評価してDimensionValueに変換し、さらにf32に解決
    fn eval_dimension_expr(&self, expr: &crate::parser::ast::Expr, context: &LayoutContext, is_width: bool) -> Option<f32> {
        use crate::parser::ast::{Expr, BinaryOperator};
        
        match expr {
            Expr::CalcExpr(inner) => self.eval_dimension_expr(inner, context, is_width),
            Expr::Dimension(d) => Some(self.resolve_dimension_value(d, context, is_width)),
            Expr::Number(n) => Some(*n),
            Expr::BinaryOp { left, op, right } => {
                let left_val = self.eval_dimension_expr(left, context, is_width)?;
                let right_val = self.eval_dimension_expr(right, context, is_width)?;
                
                let result = match op {
                    BinaryOperator::Add => left_val + right_val,
                    BinaryOperator::Sub => left_val - right_val,
                    BinaryOperator::Mul => left_val * right_val,
                    BinaryOperator::Div => {
                        if right_val != 0.0 {
                            left_val / right_val
                        } else {
                            0.0
                        }
                    }
                    _ => return None,
                };
                
                Some(result)
            }
            _ => None,
        }
    }

    /// スタイルからパディングを取得
    fn get_padding_from_style(&self, style: Option<&Style>, context: &LayoutContext) -> Edges {
        if let Some(style) = style {
            if let Some(ref rel_padding) = style.relative_padding {
                return self.resolve_relative_edges(rel_padding, context);
            }
            if let Some(padding) = style.padding {
                return padding;
            }
        }
        Edges::default()
    }

    /// スタイルからスペーシングを取得
    fn get_spacing_from_style(&self, style: Option<&Style>, context: &LayoutContext) -> f32 {
        if let Some(style) = style {
            if let Some(ref gap) = style.gap {
                return self.resolve_dimension_value(gap, context, true);
            }
            if let Some(ref rel_spacing) = style.relative_spacing {
                return self.resolve_dimension_value(rel_spacing, context, true);
            }
            if let Some(spacing) = style.spacing {
                return spacing;
            }
        }
        10.0 // デフォルトスペーシング
    }

    /// 相対Edgesを絶対値に変換
    fn resolve_relative_edges(&self, edges: &RelativeEdges, context: &LayoutContext) -> Edges {
        edges.to_edges(
            context.window_size[0],
            context.window_size[1],
            context.parent_size[0],
            context.parent_size[1],
            context.root_font_size,
            context.font_size,
        )
    }

    /// テキスト測定（正確版 - TextMeasurementSystemを使用）
    fn measure_text(&self, text: &str, font_size: f32, font_family: &str, max_width: Option<f32>) -> TextMeasurement {
        let system = get_text_measurement_system();
        let mut system_guard = system.lock().unwrap();
        system_guard.measure_text(
            text,
            font_size,
            font_family,
            max_width,
            None, // line_height_multiplier - デフォルト値使用
        )
    }







    /// レイアウトを実行してポジションを計算（再帰的処理）
    pub fn layout_with_positioning<'a, F, G>(
        &mut self,
        nodes: &'a [WithSpan<ViewNode>],
        context: &LayoutContext,
        available_size: [f32; 2],
        start_position: [f32; 2],
        eval: &F,
        get_image_size: &G,
        app: &'a App,
    ) -> Vec<LayoutedNode<'a>>
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        let mut all_results = Vec::new();
        
        match nodes.len() {
            0 => vec![],
            1 => {
                // 単一ノードの場合（子要素も再帰的に処理）
                let node = &nodes[0];
                self.layout_single_node_recursive(node, context, start_position, eval, get_image_size, app, &mut all_results);
                all_results
            }
            _ => {
                // 複数ノードの場合はVStackとして扱う（親スタイルなし）
                self.layout_vstack_recursive(nodes, None, context, available_size, start_position, eval, get_image_size, app)
            }
        }
    }
    
    /// 単一ノードのレイアウト（子要素も含めて再帰的に処理）
    fn layout_single_node_recursive<'a, F, G>(
        &mut self,
        node: &'a WithSpan<ViewNode>,
        context: &LayoutContext,
        position: [f32; 2],
        eval: &F,
        get_image_size: &G,
        app: &'a App,
        results: &mut Vec<LayoutedNode<'a>>,
    )
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        let computed_size = self.compute_node_size(node, context, eval, get_image_size, app);
        
        // 自分自身をレイアウト結果に追加
        results.push(LayoutedNode {
            node,
            position,
            size: [computed_size.width, computed_size.height],
        });
        
        // 子要素がある場合は再帰的に処理
        match &node.node {
            ViewNode::VStack(children) => {
                self.layout_vstack_recursive(children, node.style.as_ref(), context, [computed_size.width, computed_size.height], position, eval, get_image_size, app)
                    .into_iter().for_each(|child| results.push(child));
            }
            ViewNode::HStack(children) => {
                self.layout_hstack_recursive(children, node.style.as_ref(), context, [computed_size.width, computed_size.height], position, eval, get_image_size, app)
                    .into_iter().for_each(|child| results.push(child));
            }
            ViewNode::ComponentCall { name, args: _ } => {
                // コンポーネントの本体を展開（既にcompute_component_size_with_styleでサイズ計算済み）
                if let Some(component) = app.components.iter().find(|c| &c.name == name) {
                    // 重要: ComponentCallで確定したサイズを固定値として使用し、相対値の再計算を避ける
                    let component_context = LayoutContext {
                        window_size: context.window_size,
                        parent_size: [computed_size.width, computed_size.height], // ComponentCallのサイズを使用
                        root_font_size: context.root_font_size,
                        font_size: context.font_size,
                        default_font: context.default_font.clone(),
                    };
                    
                    let mut child_results = self.layout_vstack_recursive(&component.body, component.default_style.as_ref(), &component_context, [computed_size.width, computed_size.height], position, eval, get_image_size, app);
                    results.append(&mut child_results);
                }
            }
            ViewNode::ForEach { var: _, iterable: _, body: _ } => {
                // Foreachは render_foreach_optimized で直接処理されるため、ここでは何もしない

            }
            ViewNode::If { condition, then_body, else_body } => {
                // If文の条件評価と展開処理
                self.layout_if_recursive(condition, then_body, else_body.as_ref(), context, position, eval, get_image_size, app, results);
            }
            _ => {
                // その他のノード（Text, Button, Image等）は子要素なし
            }
        }
    }

    /// VStackレイアウト（再帰的処理版）
    fn layout_vstack_recursive<'a, F, G>(
        &mut self,
        children: &'a [WithSpan<ViewNode>],
        parent_style: Option<&Style>,
        context: &LayoutContext,
        available_size: [f32; 2],
        start_position: [f32; 2],
        eval: &F,
        get_image_size: &G,
        app: &'a App,
    ) -> Vec<LayoutedNode<'a>>
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        let mut results = Vec::new();
        let mut current_y = start_position[1];
        
        // align: "center" の場合、子要素の合計高さを事前に計算
        let align = parent_style.and_then(|s| s.align);
        let total_children_height = if matches!(align, Some(crate::parser::ast::Align::Center)) {
            // パス1: 子要素のサイズを事前計算
            let mut total_height = 0.0;
            
            for (i, child) in children.iter().enumerate() {
                let child_context = LayoutContext {
                    window_size: context.window_size,
                    parent_size: available_size,
                    root_font_size: context.root_font_size,
                    font_size: context.font_size,
                    default_font: context.default_font.clone(),
                };
                
                let child_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
                total_height += child_size.height;
                
                if i < children.len() - 1 {
                    total_height += self.get_spacing_from_style(parent_style, context);
                }
            }
            
            total_height
        } else {
            0.0
        };
        
        // align: "center" の場合、開始位置をオフセット
        if matches!(align, Some(crate::parser::ast::Align::Center)) {
            let center_offset = (available_size[1] - total_children_height) / 2.0;
            current_y = start_position[1] + center_offset.max(0.0);
        }
        
        for (i, child) in children.iter().enumerate() {
            // 子要素のコンテキストを作成
            let child_context = LayoutContext {
                window_size: context.window_size,
                parent_size: available_size,
                root_font_size: context.root_font_size,
                font_size: context.font_size,
                default_font: context.default_font.clone(),
            };
            
            // スペーシング計算（親のスタイルから取得）
            let spacing = if i < children.len() - 1 {
                self.get_spacing_from_style(parent_style, context)
            } else {
                0.0
            };
            
            // 子要素のサイズを計算
            let child_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
            
            // align: "center" の場合、X座標を中央揃えに調整
            let child_x = if matches!(align, Some(crate::parser::ast::Align::Center)) {
                start_position[0] + (available_size[0] - child_size.width) / 2.0
            } else {
                start_position[0]
            };
            
            let child_position = [child_x, current_y];
            let initial_results_len = results.len();
            
            // 特別な処理が必要なノードタイプをチェック
            match &child.node {
                ViewNode::ForEach { var, iterable, body } => {
                    // Foreach文を展開してレイアウト
                    self.layout_foreach_recursive(var, iterable, body, &child_context, child_position, eval, get_image_size, app, &mut results);
                }
                ViewNode::If { condition, then_body, else_body } => {
                    // If文を直接処理
                    self.layout_if_recursive(condition, then_body, else_body.as_ref(), &child_context, child_position, eval, get_image_size, app, &mut results);
                }
                ViewNode::DynamicSection { name: _, body } => {
                    // DynamicSectionを展開してレイアウト
                    let child_results = self.layout_with_positioning(
                        body,
                        &child_context,
                        [child_size.width, available_size[1] - current_y],
                        child_position,
                        eval,
                        get_image_size,
                        app,
                    );
                    results.extend(child_results);
                }
                _ => {
                    // 通常のノードを再帰的にレイアウト
                    self.layout_single_node_recursive(child, &child_context, child_position, eval, get_image_size, app, &mut results);
                }
            }
            
            // 次の子要素の位置を更新（追加されたノード群の最大Y値を計算）
            let new_results_len = results.len();
            if new_results_len > initial_results_len {
                let mut max_bottom = current_y;
                for j in initial_results_len..new_results_len {
                    let node_bottom = results[j].position[1] + results[j].size[1];
                    if node_bottom > max_bottom {
                        max_bottom = node_bottom;
                    }
                }
                current_y = max_bottom + spacing;
            } else {
                current_y += spacing; // フォールバック
            }
        }
        
        results
    }
    
    /// HStackレイアウト（再帰的処理版）
    fn layout_hstack_recursive<'a, F, G>(
        &mut self,
        children: &'a [WithSpan<ViewNode>],
        parent_style: Option<&Style>,
        context: &LayoutContext,
        available_size: [f32; 2],
        start_position: [f32; 2],
        eval: &F,
        get_image_size: &G,
        app: &'a App,
    ) -> Vec<LayoutedNode<'a>>
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        let mut results = Vec::new();
        let mut current_x = start_position[0];
        
        // align: "center" の場合、子要素の合計幅を事前に計算
        let align = parent_style.and_then(|s| s.align);
        let total_children_width = if matches!(align, Some(crate::parser::ast::Align::Center)) {
            // パス1: 子要素のサイズを事前計算
            let mut total_width = 0.0;
            
            for (i, child) in children.iter().enumerate() {
                let mut child_context = LayoutContext {
                    window_size: context.window_size,
                    parent_size: available_size,
                    root_font_size: context.root_font_size,
                    font_size: context.font_size,
                    default_font: context.default_font.clone(),
                };
                
                // ComponentCallの場合、適切な親サイズを設定
                if let ViewNode::ComponentCall { name, .. } = &child.node {
                    if let Some(component) = app.components.iter().find(|c| &c.name == name) {
                        let merged_style = merge_styles(component.default_style.as_ref(), child.style.as_ref());
                        let component_explicit_size = self.get_explicit_size_from_style(Some(&merged_style), &child_context);
                        
                        if component_explicit_size.has_explicit_width {
                            child_context.parent_size = available_size;
                        }
                    }
                }
                
                let child_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
                total_width += child_size.width;
                
                if i < children.len() - 1 {
                    total_width += self.get_spacing_from_style(parent_style, context);
                }
            }
            
            total_width
        } else {
            0.0
        };
        
        // align: "center" の場合、開始位置をオフセット
        if matches!(align, Some(crate::parser::ast::Align::Center)) {
            let center_offset = (available_size[0] - total_children_width) / 2.0;
            current_x = start_position[0] + center_offset.max(0.0);
        }
        
        for (i, child) in children.iter().enumerate() {
            // 子要素のコンテキストを作成（親サイズを適切に設定）
            let mut child_context = LayoutContext {
                window_size: context.window_size,
                parent_size: available_size,
                root_font_size: context.root_font_size,
                font_size: context.font_size,
                default_font: context.default_font.clone(),
            };
            
            // ComponentCallの場合、そのコンポーネントの幅仕様を確認して適切な親サイズを設定
            if let ViewNode::ComponentCall { name, .. } = &child.node {
                if let Some(component) = app.components.iter().find(|c| &c.name == name) {
                    let merged_style = merge_styles(component.default_style.as_ref(), child.style.as_ref());
                    let component_explicit_size = self.get_explicit_size_from_style(Some(&merged_style), &child_context);
                    
                    // パーセンテージベースの幅の場合、HStackの利用可能幅を親サイズとして使用
                    if component_explicit_size.has_explicit_width {
                        child_context.parent_size = available_size;
                    }
                }
            }
            
            // スペーシング計算（親のスタイルから取得）
            let spacing = if i < children.len() - 1 {
                self.get_spacing_from_style(parent_style, context)
            } else {
                0.0
            };
            
            let child_position = [current_x, start_position[1]];
            
            // 子要素のサイズを先に計算してHStackレイアウトで使用
            let child_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
            
            // 子要素を再帰的にレイアウト
            self.layout_single_node_recursive(child, &child_context, child_position, eval, get_image_size, app, &mut results);
            
            // 次の子要素の位置を更新（計算したサイズを使用、ComponentCallの場合も正しい）
            current_x += child_size.width + spacing;
        }
        
        results
    }
    
    /// Foreach文のレイアウト処理（再帰的）
    fn layout_foreach_recursive<'a, F, G>(
        &mut self,
        var: &str,
        iterable: &Expr,
        body: &'a [WithSpan<ViewNode>],
        context: &LayoutContext,
        position: [f32; 2],
        eval: &F,
        get_image_size: &G,
        app: &'a App,
        results: &mut Vec<LayoutedNode<'a>>,
    )
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        // 配列の値を取得
        let iterable_value = eval(iterable);
        
        // 簡単な配列パース：[1, 2, 3] -> ["1", "2", "3"]
        let items: Vec<String> = if iterable_value.starts_with('[') && iterable_value.ends_with(']') {
            let inner = &iterable_value[1..iterable_value.len()-1];
            if inner.trim().is_empty() {
                vec![]
            } else {
                inner.split(',')
                    .map(|s| s.trim().to_string())
                    .collect()
            }
        } else {
            vec![iterable_value]
        };
        
        // パフォーマンス最適化：デバッグ出力を削除
        
        let mut current_y = position[1];
        
        // 各itemを処理
        for (item_index, item) in items.iter().enumerate() {
            for child in body {
                self.process_foreach_node_recursive(child, var, item, &item_index.to_string(), context, [position[0], current_y], eval, get_image_size, app, results, &mut current_y);
            }
        }
    }

    /// foreach内のノードを再帰的に処理（HStack/VStackも展開）- パフォーマンス最適化版
    fn process_foreach_node_recursive<'a, F, G>(
        &mut self,
        node: &'a WithSpan<ViewNode>,
        var: &str,
        item_value: &str,
        item_index_value: &str,
        context: &LayoutContext,
        position: [f32; 2],
        eval: &F,
        get_image_size: &G,
        app: &'a App,
        results: &mut Vec<LayoutedNode<'a>>,
        current_y: &mut f32,
    )
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        match &node.node {
            ViewNode::Text { format, args } => {
                let mut final_format = format.clone();
                
                // 各引数を処理（高速化）
                for arg in args {
                    let value = match arg {
                        Expr::Path(path) if path == var => item_value.to_string(),
                        Expr::Path(path) if path == "item_index" => item_index_value.to_string(),
                        _ => eval(arg),
                    };
                    final_format = final_format.replacen("{}", &value, 1);
                }
                
                // 新しいTextノードを作成
                let new_node = Box::leak(Box::new(WithSpan {
                    node: ViewNode::Text {
                        format: final_format,
                        args: vec![]
                    },
                    line: node.line,
                    column: node.column,
                    style: node.style.clone(),
                }));
                
                // サイズ計算
                let size = self.compute_node_size(new_node, context, eval, get_image_size, app);
                
                // LayoutedNodeを作成
                let layouted = LayoutedNode {
                    node: new_node,
                    position: [position[0], *current_y],
                    size: [size.width, size.height],
                };
                
                results.push(layouted);
                *current_y += size.height + 4.0; // spacing
            },
            ViewNode::HStack(children) => {
                let mut x_offset = position[0];
                for child in children {
                    let child_start_y = *current_y;
                    self.process_foreach_node_recursive(child, var, item_value, item_index_value, context, [x_offset, child_start_y], eval, get_image_size, app, results, current_y);
                    x_offset += 150.0; // 固定幅でX座標を進める
                    *current_y = child_start_y; // Y座標をリセット（横並びのため）
                }
                *current_y += 25.0; // HStack全体の高さ分Y座標を進める
            },
            ViewNode::VStack(children) => {
                for child in children {
                    self.process_foreach_node_recursive(child, var, item_value, item_index_value, context, [position[0], *current_y], eval, get_image_size, app, results, current_y);
                }
            },
            _ => {
                // 他のノードタイプはスキップ
            }
        }
    }

    /// ノード内の変数を展開したノードを作成
    fn expand_node_variables<F>(&self, node: &WithSpan<ViewNode>, eval: &F) -> WithSpan<ViewNode>
    where
        F: Fn(&Expr) -> String,
    {
        let expanded_viewnode = match &node.node {
            ViewNode::Text { format, args } => {
                // format文字列と引数を展開
                let expanded_args: Vec<Expr> = args.iter().map(|arg| {
                    let value = eval(arg);
                    Expr::String(value)
                }).collect();
                
                ViewNode::Text {
                    format: format.clone(),
                    args: expanded_args,
                }
            }
            ViewNode::VStack(children) => {
                let expanded_children: Vec<WithSpan<ViewNode>> = children.iter()
                    .map(|child| self.expand_node_variables(child, eval))
                    .collect();
                ViewNode::VStack(expanded_children)
            }
            ViewNode::HStack(children) => {
                let expanded_children: Vec<WithSpan<ViewNode>> = children.iter()
                    .map(|child| self.expand_node_variables(child, eval))
                    .collect();
                ViewNode::HStack(expanded_children)
            }
            ViewNode::If { condition, then_body, else_body } => {
                // 条件も評価し、bodyも再帰的に展開
                let expanded_then: Vec<WithSpan<ViewNode>> = then_body.iter()
                    .map(|child| self.expand_node_variables(child, eval))
                    .collect();
                let expanded_else = else_body.as_ref().map(|body| {
                    body.iter().map(|child| self.expand_node_variables(child, eval)).collect()
                });
                
                ViewNode::If {
                    condition: condition.clone(),
                    then_body: expanded_then,
                    else_body: expanded_else,
                }
            }
            ViewNode::DynamicSection { name, body } => {
                let expanded_body: Vec<WithSpan<ViewNode>> = body.iter()
                    .map(|child| self.expand_node_variables(child, eval))
                    .collect();
                ViewNode::DynamicSection {
                    name: name.clone(),
                    body: expanded_body,
                }
            }
            // ForEachノードは展開せず、エラーログを出力
            ViewNode::ForEach { .. } => {
                log::warn!("ネストされたForEachが見つかりました。これは現在サポートされていません");
                node.node.clone()
            }
            // その他のノード型はそのまま返す
            _ => node.node.clone(),
        };

        WithSpan {
            node: expanded_viewnode,
            style: node.style.clone(),
            line: node.line,
            column: node.column,
        }
    }

    /// If文のレイアウト処理（再帰的）
    fn layout_if_recursive<'a, F, G>(
        &mut self,
        condition: &Expr,
        then_body: &'a [WithSpan<ViewNode>],
        else_body: Option<&'a Vec<WithSpan<ViewNode>>>,
        context: &LayoutContext,
        position: [f32; 2],
        eval: &F,
        get_image_size: &G,
        app: &'a App,
        results: &mut Vec<LayoutedNode<'a>>,
    )
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        // 条件の評価
        let condition_value = eval(condition);
        let is_true = matches!(condition_value.as_str(), "true"|"1"|"True"|"TRUE") || 
                      condition_value.parse::<f32>().unwrap_or(0.0) != 0.0;
        
        // 選択されたボディを決定
        let selected_body: &[WithSpan<ViewNode>] = if is_true {
            then_body
        } else {
            else_body.map(|v| v.as_slice()).unwrap_or(&[])
        };
        
        // 選択されたボディのレイアウト（子要素を再帰的に処理）
        for child in selected_body {
            // 各子要素（HStackまたはVStack）を再帰的にレイアウト
            self.layout_single_node_recursive(child, context, position, eval, get_image_size, app, results);
        }
    }
    
    /// VStackレイアウト（互換性のため残存）
    fn layout_vstack<'a, F, G>(
        &mut self,
        children: &'a [WithSpan<ViewNode>],
        context: &LayoutContext,
        available_size: [f32; 2],
        start_position: [f32; 2],
        eval: &F,
        get_image_size: &G,
        app: &'a App,
    ) -> Vec<LayoutedNode<'a>>
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        // 再帰的処理版に委譲（親スタイルなし）
        self.layout_vstack_recursive(children, None, context, available_size, start_position, eval, get_image_size, app)
    }


}



// ========================================
// 既存システムとの互換性関数
// ========================================

/// VStackレイアウト（既存システム互換）
pub fn layout_vstack<'a, F, G>(
    nodes: &'a [WithSpan<ViewNode>],
    params: LayoutParams,
    app: &'a App,
    eval: &F,
    get_image_size: &G,
) -> Vec<LayoutedNode<'a>>
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    let mut engine = LayoutEngine::new();
    let context = LayoutContext::from(&params);
    
    engine.layout_with_positioning(
        nodes,
        &context,
        params.parent_size,
        params.start,
        eval,
        get_image_size,
        app,
    )
}

/// レイアウトノード（既存システム互換）
pub fn layout_node<'a>(
    node: &'a WithSpan<ViewNode>,
    available_size: [f32; 2],
    component_context: &crate::engine::state::ComponentContext,
    _state: &impl crate::engine::state::StateAccess,
) -> Option<(Vec<LayoutedNode<'a>>, [f32; 2])> {
    let params = LayoutParams {
        start: [0.0, 0.0],
        spacing: 12.0,
        window_size: available_size,
        parent_size: available_size,
        root_font_size: 16.0,
        font_size: 16.0,
        default_font: "Arial".to_string(),
    };

    let eval = |expr: &Expr| -> String {
        match expr {
            Expr::String(s) => s.clone(),
            Expr::Number(n) => n.to_string(),
            Expr::Bool(b) => b.to_string(),
            Expr::Ident(s) => {
                component_context.get_arg(s).cloned().unwrap_or_else(|| s.clone())
            }
            _ => format!("{:?}", expr),
        }
    };

    let get_image_size = |_path: &str| (100, 100);

    let mut engine = LayoutEngine::new();
    let context = LayoutContext::from(&params);
    
    // 単純なサイズ計算のみ実行
    let computed_size = engine.compute_node_size(node, &context, &eval, &get_image_size, &crate::parser::ast::App {
        flow: crate::parser::ast::Flow {
            start: "start".to_string(),
            transitions: vec![],
        },
        timelines: vec![],
        components: vec![],
    });
    
    let result = vec![LayoutedNode {
        node,
        position: [0.0, 0.0],
        size: [computed_size.width, computed_size.height],
    }];

    let total_size = [available_size[0], computed_size.height];
    Some((result, total_size))
}