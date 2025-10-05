// 新しいレイアウトシステム - 完全に再設計
// 子要素から親要素への計算（Bottom-Up）を基本とし、
// width/height の優先度を明確化した汎用レイアウトエンジン

use crate::parser::ast::{Style, Edges, DimensionValue, RelativeEdges, Unit};
use crate::parser::ast::{ViewNode, WithSpan, Expr, App};
use crate::engine::state::format_text;
use crate::stencil::stencil::Stencil as DrawStencil;
use crate::ui::text_measurement::{TextMeasurementSystem, TextMeasurement};
use std::collections::HashMap;
use std::cell::RefCell;

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
            start: [20.0, 20.0],
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
            start: [20.0, 20.0],
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
    /// テキスト測定システム（内部変更可能）
    text_measurement_system: RefCell<TextMeasurementSystem>,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            component_cache: HashMap::new(),
            text_measurement_system: RefCell::new(TextMeasurementSystem::new()),
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
            return self.compute_component_size_with_style(node, name, context, eval, app);
        }
        
        // 1. スタイルから明示的なサイズを取得（最優先）
        let mut computed = self.get_explicit_size_from_style(node.style.as_ref(), context);
        
        // 2. 明示的な幅がある場合は子要素のコンテキストに適用（VStack/HStackのみでなく全ノード対象）
        let child_context = if computed.has_explicit_width {
            let mut new_context = context.clone();
            new_context.parent_size[0] = computed.width;
            println!("DEBUG: Node setting child context width: {:.1} (node type: {:?})", computed.width, std::mem::discriminant(&node.node));
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
            // width の優先順位: width > relative_width
            if let Some(width) = style.width {
                computed.width = width;
                computed.has_explicit_width = true;
            } else if let Some(ref relative_width) = style.relative_width {
                computed.width = self.resolve_dimension_value(relative_width, context, true);
                computed.has_explicit_width = true;
            }
            
            // height の優先順位: height > relative_height
            if let Some(height) = style.height {
                computed.height = height;
                computed.has_explicit_height = true;
            } else if let Some(ref relative_height) = style.relative_height {
                computed.height = self.resolve_dimension_value(relative_height, context, false);
                computed.has_explicit_height = true;
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
                self.compute_vstack_size(children, context, eval, get_image_size, app)
            }
            ViewNode::HStack(children) => {
                self.compute_hstack_size(children, context, eval, get_image_size, app)
            }
            ViewNode::ComponentCall { name, args } => {
                self.compute_component_size_with_style(node, name, context, eval, app)
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
        
        // パディングを計算
        let padding = self.get_padding_from_style(style, context);
        
        // max_widthを考慮（パディングを差し引く）
        // 注意: ウィンドウサイズは使用せず、常に親要素のサイズを基準とする
        let max_width = if let Some(style) = style {
            if let Some(ref max_w) = style.max_width {
                if max_w.unit == Unit::Auto {
                    // 親要素の幅を常に利用可能幅として使用（>0でなければ0を許容）
                    let available_width = (context.parent_size[0] - padding.left - padding.right).max(0.0);
                    println!("DEBUG: Text max_width:auto - parent_size: {:.1}, available: {:.1}", context.parent_size[0], available_width);
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
                None
            }
        } else {
            None
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
        context: &LayoutContext,
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        // パス1: 子要素のサイズを計算してVStackの幅を決定
        let mut max_width: f32 = 0.0;
        let mut child_sizes = Vec::new();
        
        for child in children.iter() {
            // パス1では現在のコンテキストをそのまま使用
            let child_size = self.compute_node_size(child, context, eval, get_image_size, app);
            child_sizes.push(child_size.clone());
            max_width = max_width.max(child_size.width);
        }
        
        // VStackの最終的な幅を決定
        // ポイント: 親が利用可能幅を提示している場合（>0）は、それを優先して子へ伝播する。
        // ウィンドウ幅かどうかは関係なく、トップレベルでも親幅（=ウィンドウ幅）を使う。
        let final_width = if context.parent_size[0] > 0.0 {
            context.parent_size[0]
        } else {
            // 親幅が不明な場合のみ、子要素の最大幅を採用
            max_width
        };
        
        // パス2: 確定したVStackの幅を子要素に伝えて再計算（max_width: auto対応）
        let mut total_height: f32 = 0.0;
        let mut needs_recompute = false;
        
        // max_width: autoを持つ子要素があるかチェック
        for child in children.iter() {
            if let Some(style) = &child.style {
                if let Some(ref max_w) = style.max_width {
                    if max_w.unit == crate::parser::ast::Unit::Auto {
                        needs_recompute = true;
                        break;
                    }
                }
            }
        }
        
        for (i, child) in children.iter().enumerate() {
            let child_size = if needs_recompute || final_width != max_width {
                // max_width: autoがある場合、または幅が変更された場合は再計算
                let mut child_context = context.clone();
                child_context.parent_size = [final_width, context.parent_size[1]];
                println!("DEBUG: VStack child {} context - parent_width: {:.1}", i, final_width);
                self.compute_node_size(child, &child_context, eval, get_image_size, app)
            } else {
                // そうでなければパス1の結果を使用
                child_sizes[i].clone()
            };
            
            total_height += child_size.height;
            
            // スペーシングを追加（最後の要素以外）
            if i < children.len() - 1 {
                total_height += self.get_spacing_from_style(child.style.as_ref(), context);
            }
        }
        
        ComputedSize {
            width: final_width,
            height: total_height,
            intrinsic_width: max_width,
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
        
        // パス1: 子要素のサイズを計算してHStackのサイズを決定
        let mut child_sizes = Vec::new();
        
        for (i, child) in children.iter().enumerate() {
            // パス1では現在のコンテキストをそのまま使用
            let child_size = self.compute_node_size(child, context, eval, get_image_size, app);
            child_sizes.push(child_size.clone());
            total_width += child_size.width;
            if i < children.len() - 1 {
                // レイアウト時と同じスペーシングの取り扱いに合わせる
                total_width += self.get_spacing_from_style(child.style.as_ref(), context);
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
        
        // HStackの最終的な幅も決定（子要素への幅制約として使用）
        // 親が利用可能幅を提示している場合（>0）は常にそれを使用する
        let final_width = if context.parent_size[0] > 0.0 {
            context.parent_size[0]
        } else {
            total_width
        };
        
    // パス2: 確定したHStackのサイズを子要素に伝えて再計算（max_width: auto対応）
    // HStackでは、子要素が明示的な幅を持つ場合、その幅を尊重する
    let mut needs_recompute = false;
        
        // max_width: autoを持つ子要素があるかチェック
        for child in children.iter() {
            if let Some(style) = &child.style {
                if let Some(ref max_w) = style.max_width {
                    if max_w.unit == crate::parser::ast::Unit::Auto {
                        needs_recompute = true;
                        break;
                    }
                }
            }
        }
        
    // 親の幅によって初期合計幅と異なる場合も再計算（子の折り返し等に対応）
    if needs_recompute || final_width != total_width {
            // max_width: autoがある場合、子要素のコンテキストでHStackの幅を利用可能幅として設定
            total_width = 0.0; // 再計算
            max_height = 0.0;  // 高さも再計算（ラップにより高さが変わるため）
            
            for (i, child) in children.iter().enumerate() {
                let mut child_context = context.clone();
                
                // 重要: 子要素が相対幅（パーセンテージなど）を持つ場合、
                // HStackの確定幅を基準に再計算する必要がある
                let child_has_relative_width = if let Some(style) = &child.style {
                    style.relative_width.is_some()
                } else {
                    false
                };
                
                // ComponentCallの場合、そのコンポーネント定義のスタイルもチェック
                let is_component_with_relative_width = if let ViewNode::ComponentCall { name, .. } = &child.node {
                    if let Some(component) = app.components.iter().find(|c| &c.name == name) {
                        // コンポーネントのデフォルトスタイルとノードのスタイルをマージ
                        let merged = merge_styles(component.default_style.as_ref(), child.style.as_ref());
                        merged.relative_width.is_some()
                    } else {
                        false
                    }
                } else {
                    false
                };
                
                let child_parent_width = if child_has_relative_width || is_component_with_relative_width {
                    // 相対幅の場合、HStack全体の幅を親幅として使用
                    final_width
                } else {
                    // 固定幅の場合、パス1の計算結果を使用
                    child_sizes[i].width
                };
                
                child_context.parent_size = [child_parent_width, final_height];
                println!("DEBUG: HStack child {} context - parent_width: {:.1} (relative: {})", i, child_parent_width, child_has_relative_width || is_component_with_relative_width);
                
                let child_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
                total_width += child_size.width;
                if i < children.len() - 1 {
                    total_width += self.get_spacing_from_style(child.style.as_ref(), context);
                }
                max_height = max_height.max(child_size.height);
            }
        }
        
        ComputedSize {
            // パス2で再計算した場合でも、HStack自体の幅はfinal_widthを使用
            // （total_widthは子要素の合計幅で、HStackの幅制約とは異なる）
            width: if needs_recompute || final_width != total_width {
                // 親から明示的な幅が指定されている場合はそれを優先
                final_width
            } else {
                // 子要素の合計幅をそのまま使用
                total_width
            },
            height: max_height,
            intrinsic_width: total_width,
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
            // 変数置換のための評価関数を作成
            let foreach_eval = |expr: &Expr| -> String {
                match expr {
                    Expr::Ident(s) if s == var => item.clone(),
                    Expr::Ident(s) if s == &format!("{}_index", var) => index.to_string(),
                    _ => eval(expr),
                }
            };

            // bodyの各ノードのサイズを直接計算（再帰を避ける）
            for child in body {
                let child_size = self.compute_node_size_safe(child, context, &foreach_eval, get_image_size, app);
                
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
                self.compute_component_size_with_style(node, name, context, eval, app)
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
    fn compute_component_size_with_priority<F>(
        &mut self,
        name: &str,
        _args: &[Expr], 
        context: &LayoutContext,
        eval: &F,
        app: &App,
        override_width: Option<bool>,
        override_height: Option<bool>,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
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
                    style: merged_style,
                };
                
                self.compute_node_size_safe(&modified_node, context, eval, &|_| (100, 100), app)
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
    fn compute_component_size_with_style<F>(
        &mut self,
        node: &WithSpan<ViewNode>,
        name: &str,
        context: &LayoutContext,
        eval: &F,
        app: &App,
    ) -> ComputedSize
    where
        F: Fn(&Expr) -> String,
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
                let calculated = if is_width {
                    dim.value * context.parent_size[0] / 100.0
                } else {
                    dim.value * context.parent_size[1] / 100.0
                };

                calculated
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
        self.text_measurement_system.borrow_mut().measure_text(
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
                // 複数ノードの場合はVStackとして扱う
                self.layout_vstack_recursive(nodes, context, available_size, start_position, eval, get_image_size, app)
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
                self.layout_vstack_recursive(children, context, [computed_size.width, computed_size.height], position, eval, get_image_size, app)
                    .into_iter().for_each(|child| results.push(child));
            }
            ViewNode::HStack(children) => {
                self.layout_hstack_recursive(children, context, [computed_size.width, computed_size.height], position, eval, get_image_size, app)
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
                    
                    let mut child_results = self.layout_vstack_recursive(&component.body, &component_context, [computed_size.width, computed_size.height], position, eval, get_image_size, app);
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
        
        for (i, child) in children.iter().enumerate() {
            // 子要素のコンテキストを作成
            let child_context = LayoutContext {
                window_size: context.window_size,
                parent_size: available_size,
                root_font_size: context.root_font_size,
                font_size: context.font_size,
                default_font: context.default_font.clone(),
            };
            
            // スペーシング計算
            let spacing = if i < children.len() - 1 {
                self.get_spacing_from_style(child.style.as_ref(), context)
            } else {
                0.0
            };
            
            let child_position = [start_position[0], current_y];
            let initial_results_len = results.len();
            
            // 特別な処理が必要なノードタイプをチェック
            match &child.node {
                ViewNode::ForEach { var: _, iterable: _, body: _ } => {
                    // Foreach文用のプレースホルダー LayoutedNode を作成
                    // 実際の展開は render_foreach_optimized で行う

                    
                    let layouted_node = LayoutedNode {
                        node: child,
                        position: child_position,
                        size: [0.0, 0.0], // プレースホルダーサイズ
                    };
                    results.push(layouted_node);
                }
                ViewNode::If { condition, then_body, else_body } => {
                    // If文を直接処理
                    self.layout_if_recursive(condition, then_body, else_body.as_ref(), &child_context, child_position, eval, get_image_size, app, &mut results);
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
            
            // スペーシング計算
            let spacing = if i < children.len() - 1 {
                self.get_spacing_from_style(child.style.as_ref(), context)
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
        

        
        let mut current_y = position[1];
        
        // 各アイテムに対してボディを展開
        for (index, item) in items.iter().enumerate() {
            // 変数置換のための評価関数を作成
            let foreach_eval = |expr: &Expr| -> String {
                match expr {
                    Expr::Ident(s) if s == var => item.clone(),
                    Expr::Ident(s) if s == &format!("{}_index", var) => index.to_string(),
                    _ => eval(expr),
                }
            };
            
            // bodyの各ノードを処理（再帰を避けるため直接処理）
            for child in body {
                let child_context = LayoutContext {
                    window_size: context.window_size,
                    parent_size: context.parent_size,
                    root_font_size: context.root_font_size,
                    font_size: context.font_size,
                    default_font: context.default_font.clone(),
                };
                
                let child_size = self.compute_node_size(child, &child_context, &foreach_eval, get_image_size, app);
                
                // LayoutedNodeを直接作成（再帰を避ける）
                let layouted_node = LayoutedNode {
                    node: child,
                    position: [position[0], current_y],
                    size: [child_size.width, child_size.height],
                };

                results.push(layouted_node);
                
                // Y座標を更新
                current_y += child_size.height + context.root_font_size * 0.5; // スペーシング
            }
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
        
        // 選択されたボディのレイアウト（再帰を避けるため直接処理）
        let mut current_y = position[1];
        for child in selected_body {
            let child_context = LayoutContext {
                window_size: context.window_size,
                parent_size: context.parent_size,
                root_font_size: context.root_font_size,
                font_size: context.font_size,
                default_font: context.default_font.clone(),
            };
            
            let child_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
            
            // LayoutedNodeを直接作成（再帰を避ける）
            results.push(LayoutedNode {
                node: child,
                position: [position[0], current_y],
                size: [child_size.width, child_size.height],
            });
            
            // Y座標を更新
            current_y += child_size.height + context.root_font_size * 0.5; // スペーシング
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
        // 再帰的処理版に委譲
        self.layout_vstack_recursive(children, context, available_size, start_position, eval, get_image_size, app)
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