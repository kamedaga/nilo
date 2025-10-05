use crate::parser::ast::{Style, Edges, DimensionValue, RelativeEdges, Unit, ViewNode, WithSpan, Expr, App};
use crate::engine::state::format_text;
use crate::ui::text_measurement::{TextMeasurement, get_text_measurement_system};
use std::collections::HashMap;

/// レイアウト結果（ノード＋座標・サイズ）
#[derive(Debug, Clone)]
pub struct LayoutedNode<'a> {
    pub node: &'a WithSpan<ViewNode>,
    pub position: [f32; 2],
    pub size: [f32; 2],
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

impl Default for LayoutContext {
    fn default() -> Self {
        Self {
            window_size: [1920.0, 1080.0],
            parent_size: [1920.0, 1080.0],
            root_font_size: 16.0,
            font_size: 16.0,
            default_font: "Arial".to_string(),
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

    /// メインのレイアウト実行関数
    pub fn layout<'a, F, G>(
        &mut self,
        nodes: &'a [WithSpan<ViewNode>],
        context: &LayoutContext,
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> Vec<LayoutedNode<'a>>
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        let mut results = Vec::new();
        let mut current_position = [0.0, 0.0];

        for node in nodes {
            // 各ノードに対して、利用可能な領域をコンテキストで伝達
            let mut node_context = context.clone();
            // ルートレベルでは、ウィンドウサイズを親サイズとして設定
            node_context.parent_size = context.window_size;
            
            let computed_size = self.compute_node_size(node, &node_context, eval, get_image_size, app);
            
            results.push(LayoutedNode {
                node,
                position: current_position,
                size: [computed_size.width, computed_size.height],
            });
            
            // 次のノードの位置を更新（縦に配置）
            current_position[1] += computed_size.height + self.get_spacing_from_style(node.style.as_ref(), context);
        }

        results
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
        // 1. スタイルから明示的なサイズを取得（最優先）
        let mut computed = self.get_explicit_size_from_style(node.style.as_ref(), context);
        
        // 2. 内在的サイズを計算（子要素から計算）
        // このノードの確定したサイズを子要素のコンテキストに渡すため、
        // まず仮のサイズでコンテキストを作成
        let mut child_context = context.clone();
        if computed.has_explicit_width {
            child_context.parent_size[0] = computed.width;
        }
        if computed.has_explicit_height {
            child_context.parent_size[1] = computed.height;
        }
        
        let intrinsic = self.compute_intrinsic_size(node, &child_context, eval, get_image_size, app);
        
        // 3. 明示的でない部分は内在的サイズを使用
        if !computed.has_explicit_width {
            computed.width = intrinsic.width;
            computed.intrinsic_width = intrinsic.width;
        }
        if !computed.has_explicit_height {
            computed.height = intrinsic.height;
            computed.intrinsic_height = intrinsic.height;
        }
        
        // 4. min/max制約を適用
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
                self.compute_component_size(name, args, context, eval, app)
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
        
        // max_widthを考慮
        let max_width = if let Some(style) = style {
            if let Some(ref max_w) = style.max_width {
                if max_w.unit == Unit::Auto {
                    // parent_sizeが有効な場合のみ使用、そうでなければ制限なし
                    if context.parent_size[0] < f32::MAX && context.parent_size[0] > 0.0 {
                        Some(context.parent_size[0])
                    } else {
                        None // パス1では制限なしで計算
                    }
                } else {
                    Some(self.resolve_dimension_value(max_w, context, true))
                }
            } else {
                None
            }
        } else {
            None
        };
        
        // パディングを計算
        let padding = self.get_padding_from_style(style, context);
        
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
        // 2パス方式で計算
        // パス1: 親の制約なしで子要素のサイズを計算して、親の最大幅を決定
        let mut max_width: f32 = 0.0;
        
        // 最初に子要素の自然なサイズを計算
        let mut child_sizes = Vec::new();
        for child in children.iter() {
            let child_context = context.clone();
            let child_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
            child_sizes.push(child_size.clone());
            max_width = max_width.max(child_size.width);
        }
        
        // 親要素に明示的な幅が指定されているかチェック
        let parent_has_explicit_width = context.parent_size[0] < f32::MAX && context.parent_size[0] > 0.0;
        let effective_parent_width = if parent_has_explicit_width {
            context.parent_size[0]
        } else {
            max_width
        };
        
        // パス2: 確定した親の幅で子要素を再計算（max_width: auto対応）
        let mut total_height = 0.0;
        let mut needs_recompute = false;
        
        // max_width: autoを持つ子要素があるかチェック
        for child in children.iter() {
            if let Some(style) = &child.style {
                if let Some(ref max_w) = style.max_width {
                    if max_w.unit == Unit::Auto {
                        needs_recompute = true;
                        break;
                    }
                }
            }
        }
        
        for (i, child) in children.iter().enumerate() {
            let mut child_context = context.clone();
            child_context.parent_size[0] = effective_parent_width;
            
            // max_width: autoを持つ子要素は必ず再計算
            let child_size = if needs_recompute || (child.style.as_ref().and_then(|s| s.max_width.as_ref()).map_or(false, |mw| mw.unit == Unit::Auto)) {
                self.compute_node_size(child, &child_context, eval, get_image_size, app)
            } else {
                child_sizes[i].clone()
            };
            
            total_height += child_size.height;
            
            // スペーシングを追加（最後の要素以外）
            if i < children.len() - 1 {
                total_height += self.get_spacing_from_style(child.style.as_ref(), context);
            }
        }
        
        ComputedSize {
            width: effective_parent_width,
            height: total_height,
            intrinsic_width: effective_parent_width,
            intrinsic_height: total_height,
            has_explicit_width: parent_has_explicit_width,
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
        // 2パス方式で計算
        // パス1: 親の制約なしで子要素のサイズを計算して、親の最大高さを決定
        let mut max_height: f32 = 0.0;
        
        // 最初に子要素の自然なサイズを計算
        let mut child_sizes = Vec::new();
        for child in children.iter() {
            let child_context = context.clone();
            let child_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
            child_sizes.push(child_size.clone());
            max_height = max_height.max(child_size.height);
        }
        
        // 親要素に明示的な高さが指定されているかチェック
        let parent_has_explicit_height = context.parent_size[1] < f32::MAX && context.parent_size[1] > 0.0;
        let effective_parent_height = if parent_has_explicit_height {
            context.parent_size[1]
        } else {
            max_height
        };
        
        // パス2: 確定した親の高さで子要素を再計算
        let mut total_width = 0.0;
        let mut needs_recompute = false;
        
        // max_width: autoを持つ子要素があるかチェック
        for child in children.iter() {
            if let Some(style) = &child.style {
                if let Some(ref max_w) = style.max_width {
                    if max_w.unit == Unit::Auto {
                        needs_recompute = true;
                        break;
                    }
                }
            }
        }
        
        for (i, child) in children.iter().enumerate() {
            let mut child_context = context.clone();
            child_context.parent_size[1] = effective_parent_height;
            // HStackでは親の幅も利用可能幅として設定
            if context.parent_size[0] < f32::MAX && context.parent_size[0] > 0.0 {
                child_context.parent_size[0] = context.parent_size[0];
            }
            
            // max_width: autoを持つ子要素は必ず再計算
            let child_size = if needs_recompute || (child.style.as_ref().and_then(|s| s.max_width.as_ref()).map_or(false, |mw| mw.unit == Unit::Auto)) {
                self.compute_node_size(child, &child_context, eval, get_image_size, app)
            } else {
                child_sizes[i].clone()
            };
            
            total_width += child_size.width;
            
            // スペーシングを追加（最後の要素以外）
            if i < children.len() - 1 {
                total_width += self.get_spacing_from_style(child.style.as_ref(), context);
            }
        }
        
        ComputedSize {
            width: total_width,
            height: effective_parent_height,
            intrinsic_width: total_width,
            intrinsic_height: effective_parent_height,
            has_explicit_width: false,
            has_explicit_height: parent_has_explicit_height,
        }
    }

    /// コンポーネントサイズを計算
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
        // キャッシュをチェック（今回はコンポーネントの動的サイズに対応するため一時的に無効化）
        // if let Some(cached) = self.component_cache.get(name) {
        //     return cached.clone();
        // }
        
        // コンポーネント定義を探す
        if let Some(component) = app.components.iter().find(|c| c.name == name) {
            // コンポーネントのスタイルから明示的なサイズを取得
            let component_explicit_size = self.get_explicit_size_from_style(component.default_style.as_ref(), context);
            
            // コンポーネントのコンテキストを作成
            let mut component_context = context.clone();
            if component_explicit_size.has_explicit_width {
                component_context.parent_size[0] = component_explicit_size.width;
            }
            if component_explicit_size.has_explicit_height {
                component_context.parent_size[1] = component_explicit_size.height;
            }
            
            // コンポーネントの本体を計算
            let body_size = self.compute_vstack_size(&component.body, &component_context, eval, &|_| (100, 100), app);
            
            // コンポーネントスタイルの明示的サイズを優先
            let final_size = ComputedSize {
                width: if component_explicit_size.has_explicit_width {
                    component_explicit_size.width
                } else {
                    body_size.width
                },
                height: if component_explicit_size.has_explicit_height {
                    component_explicit_size.height
                } else {
                    body_size.height
                },
                intrinsic_width: body_size.intrinsic_width,
                intrinsic_height: body_size.intrinsic_height,
                has_explicit_width: component_explicit_size.has_explicit_width || body_size.has_explicit_width,
                has_explicit_height: component_explicit_size.has_explicit_height || body_size.has_explicit_height,
            };
            
            // キャッシュに保存
            self.component_cache.insert(name.to_string(), final_size.clone());
            
            final_size
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
            
            // max_height制約（今回は未実装なのでコメントアウト）
            // if let Some(ref max_h) = style.max_height {
            //     let max_height = self.resolve_dimension_value(max_h, context, false);
            //     computed.height = computed.height.min(max_height);
            // }
        }
    }

    /// 相対単位を絶対値に変換
    fn resolve_dimension_value(&self, dim: &DimensionValue, context: &LayoutContext, is_width: bool) -> f32 {
        match dim.unit {
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
            // Unit::Vmin => dim.value * context.window_size[0].min(context.window_size[1]) / 100.0,
            // Unit::Vmax => dim.value * context.window_size[0].max(context.window_size[1]) / 100.0,
            Unit::Em => dim.value * context.font_size,
            Unit::Rem => dim.value * context.root_font_size,
            Unit::Auto => {
                if is_width {
                    context.parent_size[0]
                } else {
                    context.parent_size[1]
                }
            }
            // Window widthやWindow heightのような単位があれば追加
            _ => dim.value,
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

    /// テキスト測定（より精密な日本語対応版）
    fn measure_text(&self, text: &str, font_size: f32, font_family: &str, max_width: Option<f32>) -> TextMeasurement {
        // デバッグ: テキスト測定の呼び出しをログ
        eprintln!("DEBUG NEW LAYOUT: Measuring text '{}' with font_size={}, max_width={:?}", text, font_size, max_width);
        
        // グローバルテキスト測定システムを使用してより精密に測定
        let system = get_text_measurement_system();
        let mut system_guard = system.lock().unwrap();
        
        let result = if let Some(max_w) = max_width {
            // 最大幅が指定されている場合はより精密な改行処理を使用
            system_guard.measure_text_with_precise_wrapping(text, font_size, font_family, max_w, None)
        } else {
            // 最大幅が指定されていない場合は通常の測定を使用
            system_guard.measure_text(text, font_size, font_family, max_width, None)
        };
        
        eprintln!("DEBUG NEW LAYOUT: Result - width={:.1}, height={:.1}, lines={}", 
                 result.width, result.height, result.line_count);
        
        result
    }

    /// レイアウトを実行してポジションを計算
    pub fn layout_with_positioning<'a, F, G>(
        &mut self,
        nodes: &'a [WithSpan<ViewNode>],
        context: &LayoutContext,
        available_size: [f32; 2],
        start_position: [f32; 2],
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> Vec<LayoutedNode<'a>>
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        let mut results = Vec::new();
        let mut current_y = start_position[1];
        
        for node in nodes {
            let mut node_context = context.clone();
            node_context.parent_size = available_size;
            
            match &node.node {
                ViewNode::VStack(children) | ViewNode::HStack(children) => {
                    // VStack/HStackの場合：まず親コンテナを追加
                    let computed_size = self.compute_node_size(node, &node_context, eval, get_image_size, app);
                    
                    results.push(LayoutedNode {
                        node,
                        position: [start_position[0], current_y],
                        size: [computed_size.width, computed_size.height],
                    });
                    
                    // 子要素をレイアウト
                    let child_results = match &node.node {
                        ViewNode::VStack(_) => {
                            self.layout_vstack(children, &node_context, available_size, [start_position[0], current_y], eval, get_image_size, app)
                        }
                        ViewNode::HStack(_) => {
                            self.layout_hstack(children, &node_context, available_size, [start_position[0], current_y], eval, get_image_size, app)
                        }
                        _ => unreachable!()
                    };
                    
                    results.extend(child_results);
                    current_y += computed_size.height;
                }
                _ => {
                    // 通常のノードの場合
                    let computed_size = self.compute_node_size(node, &node_context, eval, get_image_size, app);
                    
                    results.push(LayoutedNode {
                        node,
                        position: [start_position[0], current_y],
                        size: [computed_size.width, computed_size.height],
                    });
                    
                    current_y += computed_size.height;
                }
            }
        }
        
        results
    }

    /// VStackレイアウト
    fn layout_vstack<'a, F, G>(
        &mut self,
        children: &'a [WithSpan<ViewNode>],
        context: &LayoutContext,
        available_size: [f32; 2],
        start_position: [f32; 2],
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> Vec<LayoutedNode<'a>>
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        let mut results = Vec::new();
        let mut current_y = start_position[1];
        
        for (i, child) in children.iter().enumerate() {
            // 借用の競合を避けるため、子要素のコンテキストを事前に作成
            let child_context = LayoutContext {
                window_size: context.window_size,
                parent_size: available_size,
                root_font_size: context.root_font_size,
                font_size: context.font_size,
                default_font: context.default_font.clone(),
            };
            
            // スペーシング計算も事前に行う
            let spacing = if i < children.len() - 1 {
                if let Some(style) = &child.style {
                    if let Some(ref gap) = style.gap {
                        self.resolve_dimension_value(gap, context, true)
                    } else if let Some(ref rel_spacing) = style.relative_spacing {
                        self.resolve_dimension_value(rel_spacing, context, true)
                    } else if let Some(spacing) = style.spacing {
                        spacing
                    } else {
                        10.0
                    }
                } else {
                    10.0
                }
            } else {
                0.0
            };
            
            let computed_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
            
            // シンプルなレイアウト（複合要素の再帰的処理は一旦除外）
            results.push(LayoutedNode {
                node: child,
                position: [start_position[0], current_y],
                size: [computed_size.width, computed_size.height],
            });
            
            // 次の子要素の位置を更新
            current_y += computed_size.height + spacing;
        }
        
        results
    }

    /// HStackレイアウト
    fn layout_hstack<'a, F, G>(
        &mut self,
        children: &'a [WithSpan<ViewNode>],
        context: &LayoutContext,
        available_size: [f32; 2],
        start_position: [f32; 2],
        eval: &F,
        get_image_size: &G,
        app: &App,
    ) -> Vec<LayoutedNode<'a>>
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        let mut results = Vec::new();
        let mut current_x = start_position[0];
        
        for (i, child) in children.iter().enumerate() {
            // 借用の競合を避けるため、子要素のコンテキストを事前に作成
            let child_context = LayoutContext {
                window_size: context.window_size,
                parent_size: available_size,
                root_font_size: context.root_font_size,
                font_size: context.font_size,
                default_font: context.default_font.clone(),
            };
            
            // スペーシング計算も事前に行う
            let spacing = if i < children.len() - 1 {
                if let Some(style) = &child.style {
                    if let Some(ref gap) = style.gap {
                        self.resolve_dimension_value(gap, context, true)
                    } else if let Some(ref rel_spacing) = style.relative_spacing {
                        self.resolve_dimension_value(rel_spacing, context, true)
                    } else if let Some(spacing) = style.spacing {
                        spacing
                    } else {
                        10.0
                    }
                } else {
                    10.0
                }
            } else {
                0.0
            };
            
            let computed_size = self.compute_node_size(child, &child_context, eval, get_image_size, app);
            
            // シンプルなレイアウト（複合要素の再帰的処理は一旦除外）
            results.push(LayoutedNode {
                node: child,
                position: [current_x, start_position[1]],
                size: [computed_size.width, computed_size.height],
            });
            
            // 次の子要素の位置を更新
            current_x += computed_size.width + spacing;
        }
        
        results
    }

}