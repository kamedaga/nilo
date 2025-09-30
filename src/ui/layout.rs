use crate::parser::ast::{Style, Edges, Align, DimensionValue, RelativeEdges};
use crate::parser::ast::{ViewNode, WithSpan, Expr, App};
use crate::engine::state::format_text;
use crate::stencil::stencil::Stencil as DrawStencil;

/// レイアウト結果（ノード＋座標・サイズ）
#[derive(Debug, Clone)]
pub struct LayoutedNode<'a> {
    pub node: &'a WithSpan<ViewNode>,
    pub position: [f32; 2],
    pub size: [f32; 2],
}

/// レイアウトの初期パラメータ
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
    /// ★ 追加: デフォルトフォント名
    pub default_font: String,
}

impl Default for LayoutParams {
    fn default() -> Self {
        Self {
            start: [0.0, 0.0],
            spacing: 12.0,
            window_size: [1920.0, 1080.0], // デフォルトサイズ
            parent_size: [1920.0, 1080.0],
            root_font_size: 16.0,
            font_size: 16.0,
            default_font: "Arial".to_string(), // デフォルトフォント
        }
    }
}

const FONT_SIZE: f32 = 18.0;
const MIN_TEXT_WIDTH: f32 = 100.0;
const MAX_TEXT_WIDTH: f32 = 400.0;
const MIN_BUTTON_WIDTH: f32 = 120.0;
const MAX_BUTTON_WIDTH: f32 = 220.0;
const BUTTON_HEIGHT: f32 = 48.0;
const TEXT_PADDING: f32 = 12.0;
const CARD_DEFAULT_PADDING: f32 = 20.0;

pub type ImageSizeFn = dyn Fn(&str) -> (u32, u32);

/// 相対単位を絶対値に変換する
fn resolve_dimension_value(dim: &DimensionValue, params: &LayoutParams) -> f32 {
    dim.to_px(
        params.window_size[0],
        params.window_size[1],
        params.parent_size[0],
        params.parent_size[1],
        params.root_font_size,
        params.font_size,
    )
}

fn resolve_relative_edges(edges: &RelativeEdges, params: &LayoutParams) -> Edges {
    edges.to_edges(
        params.window_size[0],
        params.window_size[1],
        params.parent_size[0],
        params.parent_size[1],
        params.root_font_size,
        params.font_size,
    )
}

/// 相対サイズを絶対サイズに変換する
fn resolve_relative_size(size: &[DimensionValue; 2], params: &LayoutParams) -> [f32; 2] {
    [
        resolve_dimension_value(&size[0], params),
        resolve_dimension_value(&size[1], params),
    ]
}

fn effective_spacing(params: &LayoutParams, style: Option<&Style>) -> f32 {
    if let Some(s) = style {
        // 相対単位のスペーシングをチェック
        if let Some(rel_spacing) = &s.relative_spacing {
            return resolve_dimension_value(rel_spacing, params);
        }
        // 絶対値のスペーシングをチェック
        if let Some(spacing) = s.spacing {
            return spacing;
        }
    }
    params.spacing
}

fn effective_padding(style: Option<&Style>, params: &LayoutParams) -> Edges {
    if let Some(s) = style {
        if let Some(rel_padding) = &s.relative_padding {
            return resolve_relative_edges(rel_padding, params);
        }
        // 絶対値のパディングをチェック
        if let Some(p) = s.padding {
            return p;
        }
        // カードスタイルのデフォルトパディング
        if s.card.unwrap_or(false) {
            return Edges::all(CARD_DEFAULT_PADDING);
        }
    }
    Edges::default()
}

/// ノードのサイズを計算（相対単位対応）
fn calculate_node_size_with_params<F, G>(
    node: &WithSpan<ViewNode>,
    params: &LayoutParams,
    eval: &F,
    get_image_size: &G,
) -> [f32; 2]
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    let style = node.style.as_ref();

    // ★ 修正: 個別のwidth/heightフィールドをチェック
    let mut size = [0.0, 0.0];
    let mut has_explicit_size = false;

    if let Some(s) = style {
        // 相対単位のwidth/heightをチェック
        if let Some(rel_width) = &s.relative_width {
            let calculated_width = resolve_dimension_value(rel_width, params);
            size[0] = calculated_width;
            has_explicit_size = true;
        } else if let Some(width) = s.width {
            size[0] = width;
            has_explicit_size = true;
        }

        if let Some(rel_height) = &s.relative_height {
            let calculated_height = resolve_dimension_value(rel_height, params);
            size[1] = calculated_height;
            has_explicit_size = true;
        } else if let Some(height) = s.height {
            size[1] = height;
            has_explicit_size = true;
        }

        // 従来のrelative_sizeとsizeもチェック
        if !has_explicit_size {
            if let Some(rel_size) = &s.relative_size {
                return resolve_relative_size(rel_size, params);
            }
            if let Some(abs_size) = s.size {
                return abs_size;
            }
        }
    }

    if has_explicit_size {
        let default_size = calculate_node_size_with_style(node, params, eval, get_image_size);
        if size[0] == 0.0 { size[0] = default_size[0]; }
        if size[1] == 0.0 { size[1] = default_size[1]; }
        return size;
    }

    // デフォルトサイズ計算（相対単位対応）
    calculate_node_size_with_style(node, params, eval, get_image_size)
}

/// スタイルを考慮してノードサイズを計算する関数
fn calculate_node_size_with_style<F, G>(
    node: &WithSpan<ViewNode>,
    params: &LayoutParams,
    eval: &F,
    get_image_size: &G,
) -> [f32; 2]
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    let style = node.style.as_ref();

    // スタイルから有効なフォントサイズを取得
    let effective_font_size = if let Some(s) = style {
        if let Some(font_size) = s.font_size {
            font_size
        } else {
            params.font_size
        }
    } else {
        params.font_size
    };

    match &node.node {
        ViewNode::Text { format, args } => {
            let values: Vec<String> = args.iter().map(|e| eval(e)).collect();
            let text = format_text(format, &values);
            // ★ 修正: パディングを考慮したテキスト幅計算
            let text_width = calculate_text_width(&text, effective_font_size);
            let padding = effective_padding(style, params);
            let total_width = text_width + padding.left + padding.right;
            let total_height = effective_font_size * 1.2 + padding.top + padding.bottom;

            // ★ 修正: 最小幅のみ適用し、最大幅制限を削除してテキストが切れないようにする
            let final_width = total_width.max(MIN_TEXT_WIDTH);
            [final_width, total_height]
        }
        ViewNode::Button { label, .. } => {
            // ★ 修正: ボタンのフォントサイズを考慮
            let text_width = calculate_button_text_width(label, effective_font_size);
            let width = text_width.clamp(MIN_BUTTON_WIDTH, MAX_BUTTON_WIDTH);
            let height = if let Some(s) = style {
                if let Some(h) = s.size.map(|sz| sz[1]) {
                    h
                } else {
                    effective_font_size * 2.0 + 16.0
                }
            } else {
                BUTTON_HEIGHT
            };
            [width, height]
        }
        ViewNode::Image { path } => {
            let (img_w, img_h) = get_image_size(path);
            [img_w as f32, img_h as f32]
        }
        ViewNode::ComponentCall { name, args } => {
            // ★ 修正: コンポーネント名の正確な幅計算
            let name_width = calculate_text_width(name, effective_font_size);
            let args_width = (args.len() as f32) * effective_font_size * 0.5;
            let total_width = name_width + args_width + 20.0;
            [total_width.clamp(150.0, 350.0), effective_font_size * 1.2 + 10.0]
        }
        ViewNode::DynamicSection { name, body } => {
            let name_width = calculate_text_width(name, effective_font_size) + 40.0;
            let width = name_width.clamp(200.0, 400.0);

            let mut total_body_height = 32.0;
            for body_node in body {
                let body_size = calculate_node_size_with_style(body_node, params, eval, get_image_size);
                total_body_height += body_size[1] + 4.0;
            }
            [width, (total_body_height + 16.0).max(56.0)]
        }
        ViewNode::Match { arms, default, .. } => {
            let mut total_height = 0.0;
            for (_, nodes) in arms {
                for node in nodes {
                    let sz = calculate_node_size_with_style(node, params, eval, get_image_size);
                    total_height += sz[1] + 4.0;
                }
            }
            if let Some(def_nodes) = default {
                for node in def_nodes {
                    let sz = calculate_node_size_with_style(node, params, eval, get_image_size);
                    total_height += sz[1] + 4.0;
                }
            }
            [300.0, total_height]
        }
        ViewNode::NavigateTo { target } => {
            let width = calculate_text_width(target, effective_font_size) + 20.0;
            [width.clamp(100.0, 250.0), effective_font_size * 1.2 + 6.0]
        }
        ViewNode::Stencil(st) => size_of_stencil(st),
        // ★ 修正: foreach制御のサイズ計算 - 実際の子要素を考慮
        ViewNode::ForEach { var, iterable, body } => {
            calculate_foreach_size(var, iterable, body, params, eval, get_image_size)
        }
        _ => [160.0, effective_font_size * 1.2 + 8.0],
    }
}

/// テキストの幅を正確に計算する関数
fn calculate_text_width(text: &str, font_size: f32) -> f32 {
    let mut width = 0.0;
    for ch in text.chars() {
        if ch.is_ascii() {
            // 英数字・記号 - より正確な係数を使用
            width += font_size * 0.65;
        } else {
            // 日本語文字（ひらがな、カタカナ、漢字） - より正確な係数を使用
            width += font_size * 1.1;
        }
    }
    // ★ 修正: 少し余裕を持たせる
    width * 1.05
}

/// ボタンラベルの幅を計算する関数
fn calculate_button_text_width(text: &str, font_size: f32) -> f32 {
    let text_width = calculate_text_width(text, font_size);
    let padding = 20.0;
    text_width + padding
}

/// foreach文のサイズを計算する関数
fn calculate_foreach_size<F, G>(
    var: &str,
    iterable: &Expr,
    body: &[WithSpan<ViewNode>],
    params: &LayoutParams,
    eval: &F,
    get_image_size: &G,
) -> [f32; 2]
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    // イテラブルを評価してアイテムリストを取得
    let items = evaluate_iterable(iterable, eval);

    if items.is_empty() {
        return [0.0, 0.0];
    }

    let mut total_width = 0.0f32;
    let mut total_height = 0.0f32;

    // 各アイテムについて子要素のサイズを計算
    for (index, item) in items.iter().enumerate() {
        for body_node in body {
            // 変数を置換したノードを作成
            let substituted_node = substitute_foreach_variables_in_node(body_node, var, item, index);

            let size = calculate_node_size_with_params(&substituted_node, params, eval, get_image_size);

            // VStack形式で積み上げる想定で高さを累積
            total_height += size[1];
            total_width = total_width.max(size[0]);

            // ボディノード間のスペーシング
            if body.len() > 1 {
                total_height += params.spacing;
            }
        }

        if index < items.len() - 1 {
            total_height += params.spacing;
        }
    }

    [total_width, total_height]
}

/// イテラブル式を評価してアイテムリストを取得する関数
fn evaluate_iterable<F>(iterable: &Expr, eval: &F) -> Vec<String>
where
    F: Fn(&Expr) -> String,
{
    match iterable {
        Expr::Array(items) => {
            items.iter().map(|item| eval(item)).collect()
        }
        Expr::Ident(_) => {
            // 変数名から値を取得（現在は簡単な実装）
            let value = eval(iterable);
            // 配列形式の文字列をパース（例: "[Apple, Banana, Cherry]"）
            if value.starts_with('[') && value.ends_with(']') {
                let content = &value[1..value.len()-1];
                content.split(',')
                    .map(|s| s.trim().trim_matches('"').to_string())
                    .collect()
            } else {
                vec![value]
            }
        }
        _ => {
            let value = eval(iterable);
            vec![value]
        }
    }
}

/// foreach文の変数を置換する関数（新版）
fn substitute_foreach_variables_in_node(
    node: &WithSpan<ViewNode>,
    var: &str,
    item: &str,
    index: usize,
) -> WithSpan<ViewNode> {
    let substituted_node = match &node.node {
        ViewNode::Text { format, args } => {
            let substituted_args: Vec<Expr> = args.iter().map(|arg| {
                substitute_expr_variables(arg, var, item, index)
            }).collect();

            ViewNode::Text {
                format: format.clone(),
                args: substituted_args,
            }
        },
        ViewNode::Button { id, label, onclick } => {
            // ★ 修正: ボタンのIDとラベルも置換対象に
            let substituted_id = if id == var {
                format!("{}_{}", item, index)
            } else {
                id.clone()
            };

            let substituted_label = if label == var {
                item.to_string()
            } else {
                label.clone()
            };

            let substituted_onclick = onclick.as_ref().map(|expr| {
                substitute_expr_variables(expr, var, item, index)
            });

            ViewNode::Button {
                id: substituted_id,
                label: substituted_label,
                onclick: substituted_onclick,
            }
        },
        ViewNode::Image { path } => {
            // ★ 修正: 画像パスも置換対象に
            let substituted_path = if path == var {
                item.to_string()
            } else {
                path.clone()
            };

            ViewNode::Image {
                path: substituted_path,
            }
        },
        ViewNode::VStack(children) => {
            let substituted_children: Vec<WithSpan<ViewNode>> = children.iter().map(|child| {
                substitute_foreach_variables_in_node(child, var, item, index)
            }).collect();
            ViewNode::VStack(substituted_children)
        },
        ViewNode::HStack(children) => {
            let substituted_children: Vec<WithSpan<ViewNode>> = children.iter().map(|child| {
                substitute_foreach_variables_in_node(child, var, item, index)
            }).collect();
            ViewNode::HStack(substituted_children)
        },
        ViewNode::ComponentCall { name, args } => {
            let substituted_args: Vec<Expr> = args.iter().map(|arg| {
                substitute_expr_variables(arg, var, item, index)
            }).collect();

            ViewNode::ComponentCall {
                name: name.clone(),
                args: substituted_args,
            }
        },
        _ => node.node.clone(),
    };

    WithSpan {
        node: substituted_node,
        line: node.line,
        column: node.column,
        style: node.style.clone(),
    }
}

/// 式の変数を置換する関数
fn substitute_expr_variables(expr: &Expr, var: &str, item: &str, index: usize) -> Expr {
    match expr {
        Expr::Ident(s) if s == var => {
            // 変数名が一致する場合、アイテム値を適切な型に変換
            if let Ok(num) = item.parse::<f64>() {
                Expr::Number(num as f32)
            } else if item.eq_ignore_ascii_case("true") {
                Expr::Bool(true)
            } else if item.eq_ignore_ascii_case("false") {
                Expr::Bool(false)
            } else {
                // 文字列の場合、引用符を除去
                let clean_item = if item.starts_with('"') && item.ends_with('"') {
                    item[1..item.len()-1].to_string()
                } else {
                    item.to_string()
                };
                Expr::String(clean_item)
            }
        },
        Expr::Ident(s) if s == &format!("{}_index", var) => Expr::Number(index as f32),
        Expr::Array(items) => {
            // 配列内の要素も再帰的に置換
            let substituted_items: Vec<Expr> = items.iter().map(|item_expr| {
                substitute_expr_variables(item_expr, var, item, index)
            }).collect();
            Expr::Array(substituted_items)
        }
        _ => expr.clone(),
    }
}

fn bounds_of(slice: &[LayoutedNode]) -> ([f32;2],[f32;2]) {
    if slice.is_empty() { return ([0.0,0.0],[0.0,0.0]); }
    let min_x = slice.iter().map(|n| n.position[0]).fold(f32::INFINITY, f32::min);
    let min_y = slice.iter().map(|n| n.position[1]).fold(f32::INFINITY, f32::min);
    let max_x = slice.iter().map(|n| n.position[0] + n.size[0]).fold(f32::NEG_INFINITY, f32::max);
    let max_y = slice.iter().map(|n| n.position[1] + n.size[1]).fold(f32::NEG_INFINITY, f32::max);
    ([min_x, min_y], [max_x - min_x, max_y - min_y])
}

// ★ 修正: VStackブロック処理で子要素もすべて出力
fn layout_vstack_block<'a, F, G>(
    owner: &'a WithSpan<ViewNode>,
    children: &'a [WithSpan<ViewNode>],
    params: LayoutParams,
    result: &mut Vec<LayoutedNode<'a>>,
    cursor: &mut [f32; 2],
    app: &'a App,
    eval: &F,
    get_image_size: &G,
) -> [f32; 2]
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    let style = owner.style.as_ref();
    let gap   = effective_spacing(&params, style);
    let pad   = effective_padding(style, &params);

    let idx_container = result.len();
    result.push(LayoutedNode { node: owner, position: *cursor, size: [0.0, 0.0] });

    // ★ 新規追加: 子要素の最大幅を事前計算
    let mut max_child_width = 0.0f32;
    let mut total_child_height = 0.0f32;

    // 子要素のサイズを事前に計算して最大幅を求める
    let inner_params = LayoutParams {
        start: [cursor[0] + pad.left, cursor[1] + pad.top],
        spacing: gap,
        window_size: params.window_size,
        parent_size: params.parent_size, // 親のサイズを継承
        root_font_size: params.root_font_size,
        font_size: params.font_size,
        default_font: params.default_font.clone(),
    };

    // 子要素のサイズを計算
    for (i, child) in children.iter().enumerate() {
        let child_size = match &child.node {
            ViewNode::Spacing(v) => {
                total_child_height += *v;
                [0.0, *v]
            }
            ViewNode::SpacingAuto => {
                total_child_height += gap;
                [0.0, gap]
            }
            ViewNode::VStack(grandchildren) => {
                // ネストしたVStackのサイズを再帰的に計算
                calculate_vstack_content_size(grandchildren, &inner_params, app, eval, get_image_size)
            }
            ViewNode::HStack(grandchildren) => {
                // ネストしたHStackのサイズを計算
                calculate_hstack_content_size(grandchildren, &inner_params, app, eval, get_image_size)
            }
            ViewNode::ComponentCall { name, .. } => {
                if let Some(c) = app.components.iter().find(|c| c.name == *name) {
                    calculate_vstack_content_size(&c.body, &inner_params, app, eval, get_image_size)
                } else {
                    calculate_node_size_with_params(child, &inner_params, eval, get_image_size)
                }
            }
            _ => calculate_node_size_with_params(child, &inner_params, eval, get_image_size)
        };

        max_child_width = max_child_width.max(child_size[0]);
        total_child_height += child_size[1];

        // 子要素間のスペーシング
        if i < children.len() - 1 {
            total_child_height += gap;
        }
    }

    // ★ 修正: 明示的なサイズが指定されている場合はそれを優先
    let container_width = if let Some(s) = style {
        if let Some(width) = s.width {
            width
        } else if let Some(rel_width) = &s.relative_width {
            resolve_dimension_value(rel_width, &params)
        } else {
            max_child_width + pad.left + pad.right
        }
    } else {
        max_child_width + pad.left + pad.right
    };

    let container_height = total_child_height + pad.top + pad.bottom;

    // ★ 修正: 計算されたコンテナサイズを使用して子要素をレイアウト
    let mut inner_cursor = [cursor[0] + pad.left, cursor[1] + pad.top];

    let updated_inner_params = LayoutParams {
        start: inner_cursor,
        spacing: gap,
        window_size: params.window_size,
        parent_size: [container_width, container_height], // 計算されたコンテナサイズを親サイズとして使用
        root_font_size: params.root_font_size,
        font_size: params.font_size,
        default_font: params.default_font.clone(),
    };

    let _start_ix = result.len();
    // 子要素をすべてレイアウトして result に追加
    layout_vstack_impl(children, updated_inner_params, result, &mut inner_cursor, app, eval, get_image_size);

    // ★ 修正: 計算されたサイズを使用
    let final_size = [container_width, container_height];
    result[idx_container].size = final_size;


    final_size
}

// ★ 修正: HStackブロック処理で子要素もすべて出力
fn layout_hstack_block<'a, F, G>(
    owner: &'a WithSpan<ViewNode>,
    children: &'a [WithSpan<ViewNode>],
    params: LayoutParams,
    result: &mut Vec<LayoutedNode<'a>>,
    cursor: &mut [f32; 2],
    app: &'a App,
    eval: &F,
    get_image_size: &G,
) -> ([f32; 2], [f32; 2])
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    let style = owner.style.as_ref();
    let gap   = effective_spacing(&params, style);
    let pad   = effective_padding(style, &params);

    let valign = style.and_then(|s| s.align).unwrap_or(Align::Center);

    let idx_container = result.len();
    result.push(LayoutedNode { node: owner, position: *cursor, size: [0.0, 0.0] });

    let base_x = cursor[0] + pad.left;
    let base_y = cursor[1] + pad.top;

    let mut cur_x = base_x;
    let mut child_ranges: Vec<(usize, usize, f32)> = Vec::new();

    let container_size = calculate_node_size_with_params(owner, &params, eval, get_image_size);

    for (i, n) in children.iter().enumerate() {
        match &n.node {
            ViewNode::SpacingAuto => { cur_x += gap; }
            ViewNode::Spacing(v)  => { cur_x += *v; }
            _ => {
                let start_idx = result.len();

                let child_params = LayoutParams {
                    start: [cur_x, base_y],
                    spacing: gap,
                    window_size: params.window_size, // ★ ビューポートサイズは維持
                    parent_size: container_size, // ★ %単位計算用の親サイズ
                    root_font_size: params.root_font_size,
                    font_size: params.font_size,
                    default_font: params.default_font.clone(),
                };

                match &n.node {
                    ViewNode::VStack(grandchildren) => {
                        let mut child_cursor = [cur_x, base_y];
                        let size = layout_vstack_block(n, grandchildren, child_params, result, &mut child_cursor, app, eval, get_image_size);
                        cur_x += size[0];
                        child_ranges.push((start_idx, result.len(), size[1]));
                    }
                    ViewNode::HStack(grandchildren) => {
                        let mut child_cursor = [cur_x, base_y];
                        let (_origin, size) = layout_hstack_block(n, grandchildren, child_params, result, &mut child_cursor, app, eval, get_image_size);
                        cur_x += size[0];
                        child_ranges.push((start_idx, result.len(), size[1]));
                    }
                    ViewNode::ComponentCall { name, .. } => {
                        if let Some(c) = app.components.iter().find(|c| c.name == *name) {
                            let mut child_cursor = [cur_x, base_y];
                            layout_vstack_impl(&c.body, child_params, result, &mut child_cursor, app, eval, get_image_size);
                            let (_min, sz) = bounds_of(&result[start_idx..]);
                            cur_x += sz[0];
                            child_ranges.push((start_idx, result.len(), sz[1]));
                        }
                    }
                    _ => {
                        // ★ 修正: 単体ノードでも正しいparent_sizeを使用
                        let sz = calculate_node_size_with_params(n, &child_params, eval, get_image_size);
                        result.push(LayoutedNode { node: n, position: [cur_x, base_y], size: sz });
                        cur_x += sz[0];
                        child_ranges.push((start_idx, result.len(), sz[1]));
                    }
                }

                if i < children.len() - 1 { cur_x += gap; }
            }
        }
    }

    let line_h: f32 = child_ranges
        .iter()
        .fold(0.0, |m, &(_, _, h)| m.max(h));

    let y_offset = |h: f32| -> f32 {
        match valign {
            Align::Top    => 0.0,
            Align::Bottom => (line_h - h).max(0.0),
            _             => ((line_h - h) * 0.5).max(0.0),
        }
    };
    for (s, e, h) in child_ranges {
        let dy = y_offset(h);
        if dy.abs() > 0.001 {
            for n in &mut result[s..e] {
                n.position[1] += dy;
            }
        }
    }

    let content_w = (cur_x - base_x).max(0.0);
    let size = [content_w + pad.left + pad.right, line_h + pad.top + pad.bottom];
    result[idx_container].size = size;

    (*cursor, size)
}

/// VStackの内容サイズを計算するヘルパー関数
fn calculate_vstack_content_size<F, G>(
    children: &[WithSpan<ViewNode>],
    params: &LayoutParams,
    app: &App,
    eval: &F,
    get_image_size: &G,
) -> [f32; 2]
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    let mut max_width = 0.0f32;
    let mut total_height = 0.0f32;

    for (i, child) in children.iter().enumerate() {
        let child_size = match &child.node {
            ViewNode::Spacing(v) => {
                total_height += *v;
                [0.0, *v]
            }
            ViewNode::SpacingAuto => {
                total_height += params.spacing;
                [0.0, params.spacing]
            }
            ViewNode::VStack(grandchildren) => {
                calculate_vstack_content_size(grandchildren, params, app, eval, get_image_size)
            }
            ViewNode::HStack(grandchildren) => {
                calculate_hstack_content_size(grandchildren, params, app, eval, get_image_size)
            }
            ViewNode::ComponentCall { name, .. } => {
                if let Some(c) = app.components.iter().find(|c| c.name == *name) {
                    calculate_vstack_content_size(&c.body, params, app, eval, get_image_size)
                } else {
                    calculate_node_size_with_params(child, params, eval, get_image_size)
                }
            }
            _ => calculate_node_size_with_params(child, params, eval, get_image_size)
        };

        max_width = max_width.max(child_size[0]);
        total_height += child_size[1];

        // 子要素間のスペーシング
        if i < children.len() - 1 {
            total_height += params.spacing;
        }
    }

    [max_width, total_height]
}

/// HStackの内容サイズを計算するヘルパー関数
fn calculate_hstack_content_size<F, G>(
    children: &[WithSpan<ViewNode>],
    params: &LayoutParams,
    app: &App,
    eval: &F,
    get_image_size: &G,
) -> [f32; 2]
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    let mut total_width = 0.0f32;
    let mut max_height = 0.0f32;

    for (i, child) in children.iter().enumerate() {
        let child_size = match &child.node {
            ViewNode::Spacing(v) => {
                total_width += *v;
                [*v, 0.0]
            }
            ViewNode::SpacingAuto => {
                total_width += params.spacing;
                [params.spacing, 0.0]
            }
            ViewNode::VStack(grandchildren) => {
                calculate_vstack_content_size(grandchildren, params, app, eval, get_image_size)
            }
            ViewNode::HStack(grandchildren) => {
                calculate_hstack_content_size(grandchildren, params, app, eval, get_image_size)
            }
            ViewNode::ComponentCall { name, .. } => {
                if let Some(c) = app.components.iter().find(|c| c.name == *name) {
                    calculate_vstack_content_size(&c.body, params, app, eval, get_image_size)
                } else {
                    calculate_node_size_with_params(child, params, eval, get_image_size)
                }
            }
            _ => calculate_node_size_with_params(child, params, eval, get_image_size)
        };

        total_width += child_size[0];
        max_height = max_height.max(child_size[1]);

        // 子要素間のスペーシング
        if i < children.len() - 1 {
            total_width += params.spacing;
        }
    }

    [total_width, max_height]
}
fn calculate_node_size<F, G>(
    node: &WithSpan<ViewNode>,
    eval: &F,
    get_image_size: &G,
) -> [f32; 2]
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    match &node.node {
        ViewNode::Text { format, args } => {
            let values: Vec<String> = args.iter().map(|e| eval(e)).collect();
            let text = format_text(format, &values);
            // ★ 修正: より正確なテキスト幅計算
            let width = calculate_text_width(&text, FONT_SIZE);
            let final_width = width.clamp(MIN_TEXT_WIDTH, MAX_TEXT_WIDTH);
            [final_width, FONT_SIZE * 1.2 + TEXT_PADDING]
        }
        ViewNode::Button { label, .. } => {
            // ★ 修正: ボタンテキストの正確な幅計算
            let text_width = calculate_button_text_width(label, FONT_SIZE);
            let width = text_width.clamp(MIN_BUTTON_WIDTH, MAX_BUTTON_WIDTH);
            [width, BUTTON_HEIGHT]
        }
        ViewNode::Image { path } => {
            let (img_w, img_h) = get_image_size(path);
            [img_w as f32, img_h as f32]
        }
        ViewNode::ComponentCall { name, args } => {
            let name_width = calculate_text_width(name, FONT_SIZE);
            let args_width = (args.len() as f32) * FONT_SIZE * 0.5;
            let total_width = name_width + args_width + 20.0;
            [total_width.clamp(150.0, 350.0), FONT_SIZE * 1.2 + 10.0]
        }
        ViewNode::DynamicSection { name, body } => {
            let name_width = calculate_text_width(name, FONT_SIZE) + 40.0;
            let width = name_width.clamp(200.0, 400.0);

            let mut total_body_height = 32.0;
            for body_node in body {
                let body_size = calculate_node_size(body_node, eval, get_image_size);
                total_body_height += body_size[1] + 4.0;
            }
            [width, (total_body_height + 16.0).max(56.0)]
        }
        ViewNode::Match { arms, default, .. } => {
            let mut total_height = 0.0;
            for (_, nodes) in arms {
                for node in nodes {
                    let sz = calculate_node_size(node, eval, get_image_size);
                    total_height += sz[1] + 4.0;
                }
            }
            if let Some(def_nodes) = default {
                for node in def_nodes {
                    let sz = calculate_node_size(node, eval, get_image_size);
                    total_height += sz[1] + 4.0;
                }
            }
            [300.0, total_height]
        }
        ViewNode::NavigateTo { target } => {
            let width = calculate_text_width(target, FONT_SIZE) + 20.0;
            [width.clamp(100.0, 250.0), FONT_SIZE * 1.2 + 6.0]
        }
        ViewNode::Stencil(st) => size_of_stencil(st),
        // ★ 新規追加: foreach制御のサイズ計算
        ViewNode::ForEach { var: _, iterable: _, body: _ } => {
            // foreachの処理はengine.rsで行うため、ここでは固定サイズを返す
            [300.0, 100.0]
        }
        _ => [160.0, FONT_SIZE * 1.2 + 8.0],
    }
}

// ★ 修正: ネストした要素も完全にレイアウト
fn layout_vstack_impl<'a, F, G>(
    nodes: &'a [WithSpan<ViewNode>],
    params: LayoutParams,
    result: &mut Vec<LayoutedNode<'a>>,
    cursor: &mut [f32; 2],
    app: &'a App,
    eval: &F,
    get_image_size: &G,
) where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    for (i, node) in nodes.iter().enumerate() {
        match &node.node {
            ViewNode::Spacing(v) => { cursor[1] += *v; }
            ViewNode::SpacingAuto => { cursor[1] += params.spacing; }

            ViewNode::ComponentCall { name, .. } => {
                if let Some(c) = app.components.iter().find(|c| c.name == *name) {
                    // ★ 修正: コンポーネント展開でも子要素をすべて処理
                    layout_vstack_impl(&c.body, params.clone(), result, cursor, app, eval, get_image_size);
                }
            }

            ViewNode::VStack(children) => {
                // ★ 修正: VStackの子要素もすべて result に追加
                let size = layout_vstack_block(node, children, params.clone(), result, cursor, app, eval, get_image_size);
                cursor[1] += size[1];
            }

            ViewNode::HStack(children) => {
                // ★ 修正: HStackの子要素もすべて result に追加
                let (_origin, size) = layout_hstack_block(node, children, params.clone(), result, cursor, app, eval, get_image_size);
                cursor[1] += size[1];
            }

            ViewNode::Match { expr, arms, default } => {
                let val = eval(expr);
                let mut matched = false;
                for (pat, body) in arms {
                    if eval(pat) == val {
                        // ★ 修正: Matchの分岐内容もすべて処理
                        layout_vstack_impl(body, params.clone(), result, cursor, app, eval, get_image_size);
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    if let Some(body) = default {
                        layout_vstack_impl(body, params.clone(), result, cursor, app, eval, get_image_size);
                    }
                }
            }

            // ★ 修正: foreach制御のレイアウト処理 - engine.rsに処理を委譲
            ViewNode::ForEach { var, iterable, body } => {
                // ★ 修正: foreach全体のサイズを事前計算し、正しい高さを確保
                let foreach_size = calculate_foreach_size(var, iterable, body, &params, eval, get_image_size);
                result.push(LayoutedNode {
                    node,
                    position: *cursor,
                    size: foreach_size,
                });
                cursor[1] += foreach_size[1];
            }
            // ★ 新規追加: if制御のレイアウト処理
            ViewNode::If { condition, then_body, else_body } => {
                // 条件式を評価
                let condition_value = eval(condition);
                let is_true = matches!(condition_value.as_str(), "true" | "1" | "True" | "TRUE") ||
                             (condition_value.parse::<f32>().unwrap_or(0.0) != 0.0);

                // 条件に応じて表示するボディを選択
                let active_body = if is_true {
                    then_body
                } else if let Some(else_nodes) = else_body {
                    else_nodes
                } else {
                    return; // 条件がfalseでelse節がない場合は何もしない
                };

                // 選択されたボディをレイアウト
                layout_vstack_impl(active_body, params.clone(), result, cursor, app, eval, get_image_size);
            }

            _ => {
                // ★ 修正: 単体ノードでも正しい親サイズパラメータを使用
                let size = calculate_node_size_with_params(node, &params, eval, get_image_size);
                result.push(LayoutedNode { node, position: *cursor, size });
                cursor[1] += size[1];
            }
        }

        if i < nodes.len() - 1 { cursor[1] += params.spacing; }
    }
}

// ★ 修正: HStackでもネストした要素を完全に処理
fn layout_hstack_impl<'a, F, G>(
    nodes: &'a [WithSpan<ViewNode>],
    params: LayoutParams,
    result: &mut Vec<LayoutedNode<'a>>,
    cursor: &mut [f32; 2],
    app: &'a App,
    eval: &F,
    get_image_size: &G,
) -> ([f32; 2], [f32; 2])
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    let start = *cursor;
    let row_start = result.len();
    let mut line_h: f32 = 0.0;

    for (i, node) in nodes.iter().enumerate() {
        let child_start = result.len();

        match &node.node {
            ViewNode::VStack(children) => {
                // ★ 修正: VStackを含む場合、その子要素もすべて処理
                let child_params = LayoutParams { start: *cursor, spacing: params.spacing, ..params.clone() };
                let mut child_cursor = *cursor;
                let size = layout_vstack_block(node, children, child_params, result, &mut child_cursor, app, eval, get_image_size);
                cursor[0] += size[0];
                line_h = line_h.max(size[1]);
            }
            ViewNode::HStack(children) => {
                // ★ 修正: HStackを含む場合、その子要素もすべて処理
                let child_params = LayoutParams { start: *cursor, spacing: params.spacing, ..params.clone() };
                let mut child_cursor = *cursor;
                let (_origin, size) = layout_hstack_block(node, children, child_params, result, &mut child_cursor, app, eval, get_image_size);
                cursor[0] += size[0];
                line_h = line_h.max(size[1]);
            }
            ViewNode::ComponentCall { name, .. } => {
                if let Some(c) = app.components.iter().find(|c| c.name == *name) {
                    let child_params = LayoutParams { start: *cursor, spacing: params.spacing, ..params.clone() };
                    let mut child_cursor = *cursor;
                    layout_vstack_impl(&c.body, child_params, result, &mut child_cursor, app, eval, get_image_size);
                    let slice = &result[child_start..];
                    if !slice.is_empty() {
                        let (_min, sz) = bounds_of(slice);
                        cursor[0] += sz[0];
                        line_h = line_h.max(sz[1]);
                    }
                }
            }
            ViewNode::Match { expr, arms, default } => {
                let val = eval(expr);
                let mut matched = false;
                for (pat, body) in arms {
                    if eval(pat) == val {
                        let child_params = LayoutParams { start: *cursor, spacing: params.spacing, ..params.clone() };
                        let mut child_cursor = *cursor;
                        layout_vstack_impl(body, child_params, result, &mut child_cursor, app, eval, get_image_size);
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    if let Some(body) = default {
                        let child_params = LayoutParams { start: *cursor, spacing: params.spacing, ..params.clone() };
                        let mut child_cursor = *cursor;
                        layout_vstack_impl(body, child_params, result, &mut child_cursor, app, eval, get_image_size);
                    }
                }
                let slice = &result[child_start..];
                if !slice.is_empty() {
                    let (_min, sz) = bounds_of(slice);
                    cursor[0] += sz[0];
                    line_h = line_h.max(sz[1]);
                }
            }
            _ => {
                // ★ 修正: 単体ノードでも正しい親サイズパラメータを使用
                let size = calculate_node_size_with_params(node, &params, eval, get_image_size);
                result.push(LayoutedNode { node, position: *cursor, size });
                cursor[0] += size[0];
                line_h = line_h.max(size[1]);
            }
        }

        if i < nodes.len() - 1 {
            cursor[0] += params.spacing;
        }
    }

    let row_nodes = &result[row_start..];
    if row_nodes.is_empty() {
        return (start, [0.0, 0.0]);
    }
    let min_x = row_nodes.iter().map(|n| n.position[0]).fold(f32::INFINITY, f32::min);
    let min_y = row_nodes.iter().map(|n| n.position[1]).fold(f32::INFINITY, f32::min);
    let max_x = row_nodes.iter().map(|n| n.position[0] + n.size[0]).fold(f32::NEG_INFINITY, f32::max);
    let max_y = row_nodes.iter().map(|n| n.position[1] + n.size[1]).fold(f32::NEG_INFINITY, f32::max);

    ([min_x, min_y], [max_x - min_x, max_y - min_y])
}

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
    let mut result = Vec::new();
    let mut cursor = params.start;
    layout_vstack_impl(nodes, params, &mut result, &mut cursor, app, eval, get_image_size);
    result
}

pub fn layout_hstack<'a, F, G>(
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
    let mut result = Vec::new();
    let mut cursor = params.start;
    layout_hstack_impl(nodes, params, &mut result, &mut cursor, app, eval, get_image_size);
    result
}

fn size_of_stencil(st: &DrawStencil) -> [f32; 2] {
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
        _ => {[0.0, 0.0]}
    }
}

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
        default_font: "Arial".to_string(), // デフォルトフォント
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

    // 単純なサイズ計算のみ実行
    let size = calculate_node_size_with_params(node, &params, &eval, &get_image_size);
    let result = vec![LayoutedNode {
        node,
        position: [0.0, 0.0],
        size
    }];

    let total_size = [available_size[0], size[1]];
    Some((result, total_size))
}
