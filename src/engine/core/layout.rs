// src/engine/core/layout.rs
// レイアウト関連

use super::flow::*;
use super::render::*;
use super::utils::*;
use crate::engine::state::{AppState, StateAccess};
use crate::parser::ast::{App, Expr, ViewNode, WithSpan};
use crate::stencil::stencil::Stencil;
use crate::ui::{LayoutParams, layout_vstack};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

/// 状態のハッシュを計算（動的セクションの変更検知用）
pub fn compute_state_hash<S>(state: &S, fields: &[&str]) -> u64
where
    S: StateAccess + 'static,
{
    let mut hasher = DefaultHasher::new();
    for field in fields {
        if let Some(value) = state.get_field(field) {
            value.hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// ViewNode内で使用されているstate変数を抽出
pub fn extract_state_fields(nodes: &[WithSpan<ViewNode>]) -> Vec<String> {
    let mut fields = HashSet::new();

    fn extract_from_expr(expr: &Expr, fields: &mut HashSet<String>) {
        match expr {
            Expr::Path(path) if path.starts_with("state.") => {
                if let Some(field) = path.strip_prefix("state.") {
                    fields.insert(field.to_string());
                }
            }
            Expr::BinaryOp { left, right, .. } => {
                extract_from_expr(left, fields);
                extract_from_expr(right, fields);
            }
            Expr::CalcExpr(inner) => {
                extract_from_expr(inner, fields);
            }
            Expr::Array(items) => {
                for item in items {
                    extract_from_expr(item, fields);
                }
            }
            Expr::FunctionCall { args, .. } => {
                for arg in args {
                    extract_from_expr(arg, fields);
                }
            }
            _ => {}
        }
    }

    fn extract_from_node(node: &ViewNode, fields: &mut HashSet<String>) {
        match node {
            ViewNode::Text { args, .. } => {
                for expr in args {
                    extract_from_expr(expr, fields);
                }
            }
            ViewNode::Set { path, value, .. } => {
                if path.starts_with("state.") {
                    if let Some(field) = path.strip_prefix("state.") {
                        fields.insert(field.to_string());
                    }
                }
                extract_from_expr(value, fields);
            }
            ViewNode::VStack(children) | ViewNode::HStack(children) => {
                for child in children {
                    extract_from_node(&child.node, fields);
                }
            }
            ViewNode::DynamicSection { body, .. } => {
                for child in body {
                    extract_from_node(&child.node, fields);
                }
            }
            ViewNode::ForEach { iterable, body, .. } => {
                extract_from_expr(iterable, fields);
                for child in body {
                    extract_from_node(&child.node, fields);
                }
            }
            ViewNode::If {
                condition,
                then_body,
                else_body,
            } => {
                extract_from_expr(condition, fields);
                for child in then_body {
                    extract_from_node(&child.node, fields);
                }
                if let Some(else_nodes) = else_body {
                    for child in else_nodes {
                        extract_from_node(&child.node, fields);
                    }
                }
            }
            _ => {}
        }
    }

    for node in nodes {
        extract_from_node(&node.node, &mut fields);
    }

    fields.into_iter().collect()
}

/// タイムライン初期化時にローカル変数を一度だけ宣言
/// レイアウト再計算時には呼ばれない
pub fn initialize_local_variables<S>(nodes: &[WithSpan<ViewNode>], state: &mut AppState<S>)
where
    S: StateAccess + 'static,
{
    for node in nodes {
        match &node.node {
            ViewNode::LetDecl {
                name,
                value,
                mutable,
                declared_type: _,
            } => {
                // ローカル変数を評価して設定
                let v = state.eval_expr_from_ast(value);

                if *mutable {
                    // let変数（可変）
                    state.component_context.set_local_var(name.clone(), v);
                    log::debug!("Initialized mutable variable '{}' at timeline load", name);
                } else {
                    // const変数（不変）
                    state.component_context.set_const_var(name.clone(), v);
                    log::debug!("Initialized const variable '{}' at timeline load", name);
                }
            }
            ViewNode::VStack(children) | ViewNode::HStack(children) => {
                initialize_local_variables(children, state);
            }
            ViewNode::ForEach { body, .. } => {
                initialize_local_variables(body, state);
            }
            ViewNode::If {
                then_body,
                else_body,
                ..
            } => {
                initialize_local_variables(then_body, state);
                if let Some(else_nodes) = else_body {
                    initialize_local_variables(else_nodes, state);
                }
            }
            ViewNode::Match { arms, default, .. } => {
                for (_, body) in arms {
                    initialize_local_variables(body, state);
                }
                if let Some(default_body) = default {
                    initialize_local_variables(default_body, state);
                }
            }
            ViewNode::DynamicSection { body, .. } => {
                initialize_local_variables(body, state);
            }
            _ => {}
        }
    }
}

/// LayoutParams生成の共通化
pub fn make_layout_params(window_size: [f32; 2], default_font: String) -> LayoutParams {
    LayoutParams {
        start: [0.0, 0.0],
        spacing: 12.0,
        window_size,
        parent_size: window_size,
        root_font_size: 16.0,
        font_size: 16.0,
        default_font,
    }
}

/// レスポンシブスタイルを解決したノードツリーを作成
pub fn resolve_responsive_nodes<S>(
    nodes: &[WithSpan<ViewNode>],
    state: &AppState<S>,
) -> Vec<WithSpan<ViewNode>>
where
    S: StateAccess + 'static,
{
    nodes
        .iter()
        .map(|node| {
            let mut resolved_node = node.clone();

            // スタイルがある場合、レスポンシブルールを解決
            if let Some(ref style) = node.style {
                let resolved_style = state.resolve_responsive_style(style);
                resolved_node.style = Some(resolved_style);
            }

            // 子ノードも再帰的に解決
            resolved_node.node = match &node.node {
                ViewNode::VStack(children) => {
                    ViewNode::VStack(resolve_responsive_nodes(children, state))
                }
                ViewNode::HStack(children) => {
                    ViewNode::HStack(resolve_responsive_nodes(children, state))
                }
                ViewNode::ForEach {
                    var,
                    iterable,
                    body,
                } => ViewNode::ForEach {
                    var: var.clone(),
                    iterable: iterable.clone(),
                    body: resolve_responsive_nodes(body, state),
                },
                ViewNode::If {
                    condition,
                    then_body,
                    else_body,
                } => ViewNode::If {
                    condition: condition.clone(),
                    then_body: resolve_responsive_nodes(then_body, state),
                    else_body: else_body
                        .as_ref()
                        .map(|eb| resolve_responsive_nodes(eb, state)),
                },
                ViewNode::Match {
                    expr,
                    arms,
                    default,
                } => ViewNode::Match {
                    expr: expr.clone(),
                    arms: arms
                        .iter()
                        .map(|(pattern, body)| {
                            (pattern.clone(), resolve_responsive_nodes(body, state))
                        })
                        .collect(),
                    default: default.as_ref().map(|d| resolve_responsive_nodes(d, state)),
                },
                ViewNode::DynamicSection { name, body } => ViewNode::DynamicSection {
                    name: name.clone(),
                    body: resolve_responsive_nodes(body, state),
                },
                other => other.clone(),
            };

            resolved_node
        })
        .collect()
}

/// 軽量化されたノードレイアウト処理
pub fn layout_nodes_lightweight<S>(
    app: &App,
    state: &mut AppState<S>,
    nodes: &[WithSpan<ViewNode>],
    params: LayoutParams,
    mouse_pos: [f32; 2],
    mouse_down: bool,
    prev_mouse_down: bool,
    nest_level: u32,
) -> (
    Vec<Stencil>,
    Vec<(String, [f32; 2], [f32; 2])>,
    Vec<(String, [f32; 2], [f32; 2])>,
)
where
    S: StateAccess + 'static,
{
    let mut stencils = Vec::new();
    let mut buttons = Vec::new();
    let mut text_inputs = Vec::new();
    let mut depth_counter = (nest_level as f32) * 0.1;

    let eval_fn = |e: &Expr| state.eval_expr_from_ast(e);
    let get_img_size = |path: &str| state.get_image_size(path);

    let layouted = layout_vstack(nodes, params.clone(), app, &eval_fn, &get_img_size);

    for lnode in &layouted {
        match &lnode.node.node {
            ViewNode::DynamicSection { .. } => continue,
            ViewNode::Button { id, onclick, .. } => {
                buttons.push((id.clone(), lnode.position, lnode.size));

                if let Some(onclick_expr) = onclick {
                    state
                        .button_onclick_map
                        .insert(id.clone(), onclick_expr.clone());
                }

                let is_hover = is_point_in_rect(mouse_pos, lnode.position, lnode.size);
                render_button_lightweight(
                    lnode,
                    &mut stencils,
                    &mut depth_counter,
                    is_hover,
                    &params.default_font,
                );
            }
            ViewNode::TextInput { id, value, .. } => {
                let st = lnode.node.style.as_ref();
                let (_w,_h,_relw,_relh) = if let Some(s) = st {
                    (
                        s.width,
                        s.height,
                        s.relative_width.map(|d| (d.value, format!("{:?}", d.unit))),
                        s.relative_height.map(|d| (d.value, format!("{:?}", d.unit)))
                    )
                } else { (None, None, None, None) };
                if let Some(Expr::Path(p)) = value {
                    if let Some(field) = p.strip_prefix("state.") {
                        state.set_text_input_binding(id, field);
                        if let Some(v) = state.custom_state.get_field(field) {
                            if state.text_input_values.get(id).is_none() {
                                state.set_text_input_value(id.clone(), v);
                            }
                        }
                    }
                }
                text_inputs.push((id.clone(), lnode.position, lnode.size));
                render_text_input_lightweight(
                    lnode,
                    state,
                    &mut stencils,
                    &mut depth_counter,
                    mouse_pos,
                    &params.default_font,
                );
            }
            ViewNode::Text { .. } => {
                render_text_lightweight(
                    lnode,
                    state,
                    &mut stencils,
                    &mut depth_counter,
                    params.window_size,
                    params.parent_size,
                    &params.default_font,
                );
            }
            ViewNode::Image { .. } => {
                render_image_lightweight(lnode, &mut stencils, &mut depth_counter);
            }
            ViewNode::Stencil(stencil) => {
                let mut offset_st =
                    offset_stencil_fast(stencil, lnode.position[0], lnode.position[1]);
                adjust_stencil_depth(&mut offset_st, &mut depth_counter);
                stencils.push(offset_st);
            }
            ViewNode::RustCall { name, args } => {
                state.handle_rust_call_viewnode(name, &args);
            }
            ViewNode::LetDecl { .. } => {
                // ★ ローカル変数はタイムライン初期化時に一度だけ宣言される
                // レイアウト再計算時には何もしない（既に宣言済み）
                // 処理は initialize_local_variables() で行われる
            }
            ViewNode::ForEach {
                var,
                iterable,
                body,
            } => {
                // レンダリング段階でForeach変数を適切に設定してレンダリング
                let window_size = params.window_size;
                let spacing = params.spacing;
                render_foreach_for_layout(
                    lnode,
                    var,
                    iterable,
                    &body,
                    app,
                    state,
                    &mut stencils,
                    &mut depth_counter,
                    window_size,
                    spacing,
                );
            }
            ViewNode::If {
                condition,
                then_body,
                else_body,
            } => {
                render_if_optimized(
                    lnode,
                    condition,
                    &then_body,
                    else_body,
                    state,
                    &mut stencils,
                    &mut depth_counter,
                    params.window_size,
                    params.parent_size,
                );
            }
            _ => {
                state.viewnode_layouted_to_stencil_with_depth_counter_helper(
                    lnode,
                    app,
                    &mut stencils,
                    mouse_pos,
                    mouse_down,
                    prev_mouse_down,
                    nest_level,
                    &mut depth_counter,
                );
            }
        }
    }
    (stencils, buttons, text_inputs)
}
