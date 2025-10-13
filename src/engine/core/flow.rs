// src/engine/engine/flow.rs
// フロー制御（ForEach、If等）

use super::render::render_substituted_node_to_stencil_with_context;
use super::utils::*;
use crate::engine::state::{AppState, StateAccess};
use crate::parser::ast::{App, Expr, ViewNode, WithSpan};
use crate::stencil::stencil::Stencil;
use crate::ui::{LayoutParams, LayoutedNode, layout_vstack};

/// foreach制御の最適化描画
pub fn render_foreach_optimized<S>(
    lnode: &LayoutedNode<'_>,
    var: &str,
    iterable: &Expr,
    body: &[WithSpan<ViewNode>],
    app: &App,
    state: &mut AppState<S>,
    stencils: &mut Vec<Stencil>,
    depth_counter: &mut f32,
    window_size: [f32; 2],
    spacing: f32,
) where
    S: StateAccess + 'static,
{
    let iterable_value = state.eval_expr_from_ast(iterable);
    let items: Vec<String> = if iterable_value.starts_with('[') && iterable_value.ends_with(']') {
        serde_json::from_str::<Vec<serde_json::Value>>(&iterable_value)
            .map(|vs| {
                vs.into_iter()
                    .map(|v| match v {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => v.to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        vec![iterable_value]
    };

    let mut current_y_offset: f32 = 0.0;
    for (idx, item) in items.iter().enumerate() {
        state.component_context.enter_foreach();
        state
            .component_context
            .set_foreach_var(var.to_string(), item.clone());
        state
            .component_context
            .set_foreach_var(format!("{}_index", var), idx.to_string());

        let eval_fn = |e: &Expr| -> String {
            match e {
                Expr::String(s) => s.clone(),
                Expr::Number(n) => n.to_string(),
                Expr::Bool(b) => b.to_string(),
                Expr::Ident(name) => {
                    if let Some(value) = state.component_context.get_foreach_var(name) {
                        value.clone()
                    } else {
                        state.eval_expr_from_ast(e)
                    }
                }
                _ => state.eval_expr_from_ast(e),
            }
        };
        let get_img_size = |path: &str| state.get_image_size(path);

        let item_params = LayoutParams {
            start: [lnode.position[0], lnode.position[1] + current_y_offset],
            spacing,
            window_size,
            parent_size: [lnode.size[0], window_size[1]],
            root_font_size: 16.0,
            font_size: 16.0,
            default_font: "default".to_string(),
        };
        let layouted = layout_vstack(body, item_params, app, &eval_fn, &get_img_size);

        let start_y = lnode.position[1] + current_y_offset;
        let mut item_height: f32 = 0.0;
        for ln in &layouted {
            render_substituted_node_to_stencil_with_context(
                ln,
                stencils,
                depth_counter,
                state,
                window_size,
                [lnode.size[0], window_size[1]],
            );
            let bottom = ln.position[1] + ln.size[1];
            let h = bottom - start_y;
            if h > item_height {
                item_height = h;
            }
        }
        current_y_offset += item_height;
        if idx < items.len() - 1 {
            current_y_offset += spacing;
        }
        state.component_context.exit_foreach();
    }
}

/// レイアウト済みのForeachノードをレンダリング
pub fn render_foreach_for_layout<S>(
    lnode: &LayoutedNode<'_>,
    var: &str,
    iterable: &Expr,
    body: &[WithSpan<ViewNode>],
    _app: &App,
    state: &mut AppState<S>,
    stencils: &mut Vec<Stencil>,
    depth_counter: &mut f32,
    _window_size: [f32; 2],
    spacing: f32,
) where
    S: StateAccess + 'static,
{
    log::info!(
        "render_foreach_for_layout: 開始！var={}, iterable={:?}",
        var,
        iterable
    );
    let iterable_value = state.eval_expr_from_ast(iterable);
    let items: Vec<String> = if iterable_value.starts_with('[') && iterable_value.ends_with(']') {
        serde_json::from_str::<Vec<serde_json::Value>>(&iterable_value)
            .map(|vs| {
                vs.into_iter()
                    .map(|v| match v {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => v.to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        vec![iterable_value]
    };

    let item_height = lnode.size[1] / items.len().max(1) as f32;

    for (idx, item) in items.iter().enumerate() {
        // foreach変数を設定
        state.component_context.enter_foreach();
        state
            .component_context
            .set_foreach_var(var.to_string(), item.clone());
        state
            .component_context
            .set_foreach_var(format!("{}_index", var), idx.to_string());

        // 各アイテムの位置を計算
        let item_y = lnode.position[1] + (idx as f32) * (item_height + spacing);

        // bodyの各要素をレンダリング
        for child in body {
            match &child.node {
                ViewNode::Text { format, args } => {
                    // Textノードを直接レンダリング（foreach変数を使用）
                    let format_expanded = state.eval_expr_from_ast(&Expr::String(format.clone()));
                    let args_expanded: Vec<String> = args.iter().map(|arg| {
                        log::info!("render_foreach_for_layout: 評価前 arg={:?}, 現在のforeach変数 {}={}", arg, var, item);
                        let result = state.eval_expr_from_ast(arg);
                        log::info!("render_foreach_for_layout: 評価後 result={}", result);
                        if result == "item" {
                            log::error!("ERROR: foreach変数 'item' が置換されていません！現在の値: {}", item);
                            log::error!("ERROR: component_contextの状態を確認してください");
                        }
                        result
                    }).collect();

                    // 簡単なフォーマット処理
                    let mut text = format_expanded;
                    for arg in args_expanded {
                        text = text.replacen("{}", &arg, 1);
                    }

                    // テキストステンシルを作成
                    let text_stencil = crate::stencil::stencil::Stencil::Text {
                        content: text,
                        position: [lnode.position[0], item_y],
                        size: 14.0,
                        color: [1.0, 1.0, 1.0, 1.0],
                        font: "default".to_string(),
                        max_width: Some(lnode.size[0]),
                        scroll: true,
                        depth: *depth_counter,
                    };
                    stencils.push(text_stencil);
                }
                _ => {
                    // 他のノード型は省略（必要に応じて実装）
                }
            }
        }

        state.component_context.exit_foreach();
    }
}

/// If制御の最適化描画
pub fn render_if_optimized<S>(
    lnode: &LayoutedNode<'_>,
    condition: &Expr,
    then_body: &[WithSpan<ViewNode>],
    else_body: &Option<Vec<WithSpan<ViewNode>>>,
    state: &mut AppState<S>,
    stencils: &mut Vec<Stencil>,
    depth_counter: &mut f32,
    window_size: [f32; 2],
    parent_size: [f32; 2],
) where
    S: StateAccess + 'static,
{
    let v = state.eval_expr_from_ast(condition);

    let truth = matches!(v.as_str(), "true" | "1" | "True" | "TRUE")
        || v.parse::<f32>().unwrap_or(0.0) != 0.0;

    let chosen: &[WithSpan<ViewNode>] = if truth {
        then_body
    } else {
        else_body.as_ref().map(|v| v.as_slice()).unwrap_or(&[])
    };

    if chosen.is_empty() {
        println!("   [IF] ⚠️ 選択された分岐が空です");
        return;
    }

    let mut y = lnode.position[1];
    for (i, node) in chosen.iter().enumerate() {
        match &node.node {
            ViewNode::Text { format, args } => {
                let values: Vec<String> =
                    args.iter().map(|e| state.eval_expr_from_ast(e)).collect();
                let content = format_text_fast(format, &values);
                let style = node.style.clone().unwrap_or_default();
                let font_size = style.font_size.unwrap_or(16.0);

                // パディングを取得
                let padding = style.padding.unwrap_or_default();

                // ★ wrap プロパティを優先的にチェック
                let max_width = if let Some(wrap_mode) = style.wrap {
                    use crate::parser::ast::WrapMode;
                    match wrap_mode {
                        WrapMode::Auto => {
                            // 自動折り返し: 親要素の幅に合わせる
                            let available_width = parent_size[0] - padding.left - padding.right;
                            Some(available_width.max(0.0))
                        }
                        WrapMode::None => {
                            // 折り返ししない
                            None
                        }
                    }
                } else if let Some(max_w) = style.max_width.as_ref() {
                    // wrapが指定されていない場合はmax_widthを使用
                    if max_w.unit == crate::parser::ast::Unit::Auto {
                        let available_width = parent_size[0] - padding.left - padding.right;
                        Some(available_width.max(0.0))
                    } else {
                        let calculated_width = max_w.to_px(
                            window_size[0],
                            window_size[1],
                            parent_size[0],
                            parent_size[1],
                            16.0,
                            font_size,
                        );
                        let available_width =
                            calculated_width.min(parent_size[0] - padding.left - padding.right);
                        Some(available_width.max(0.0))
                    }
                } else {
                    // デフォルトは auto (自動折り返し)
                    let available_width = parent_size[0] - padding.left - padding.right;
                    Some(available_width.max(0.0))
                };

                *depth_counter += 0.001;
                stencils.push(Stencil::Text {
                    content,
                    position: [lnode.position[0], y],
                    size: font_size,
                    color: style
                        .color
                        .as_ref()
                        .map(|c| convert_to_rgba(c))
                        .unwrap_or([0.0, 0.0, 0.0, 1.0]),
                    font: style.font.unwrap_or_else(|| "default".into()),
                    max_width,
                    scroll: true,
                    depth: (1.0 - *depth_counter).max(0.0),
                });
                y += font_size * 1.2 + 8.0;
            }
            _ => {}
        }
        if i < chosen.len() - 1 {
            y += 8.0;
        }
    }
}
