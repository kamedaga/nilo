// src/engine/engine/event.rs
// イベント処理関連

use crate::engine::state::{AppState, StateAccess};
use crate::parser::ast::{App, Component, EventExpr, Expr, ViewNode, WithSpan};
use crate::ui::event::UIEvent;
use std::collections::{HashMap, HashSet};

/// ボタンのonclick属性を処理
pub fn handle_button_onclick<S>(_app: &App, state: &mut AppState<S>, clicked: &[&str])
where
    S: StateAccess + 'static,
{
    for id in clicked {
        if let Some(onclick_expr) = state.button_onclick_map.get(*id).cloned() {
            // onclick式を評価して実行
            match &onclick_expr {
                Expr::FunctionCall { name, args } => {
                    // 関数呼び出しの場合、stateアクセス可能な専用メソッドを使用
                    state.execute_onclick_function_call(name, args);
                }
                _ => {
                    // その他の式は通常の評価
                    state.eval_expr_from_ast(&onclick_expr);
                }
            }
        }
    }
}

/// 簡略化されたボタン同期
pub fn sync_button_handlers<S>(
    nodes: &[WithSpan<ViewNode>],
    components: &[Component],
    handlers: &mut HashMap<String, Box<dyn FnMut(&mut AppState<S>)>>,
    default_handler: impl Fn(&str) -> Box<dyn FnMut(&mut AppState<S>)>,
) {
    let mut current_ids = HashSet::new();
    collect_button_ids_fast(nodes, components, &mut current_ids);

    for id in &current_ids {
        handlers
            .entry(id.clone())
            .or_insert_with(|| default_handler(id));
    }

    handlers.retain(|k, _| current_ids.contains(k));
}

fn collect_button_ids_fast(
    nodes: &[WithSpan<ViewNode>],
    components: &[Component],
    set: &mut HashSet<String>,
) {
    for n in nodes {
        match &n.node {
            ViewNode::Button { id, .. } => {
                set.insert(id.clone());
            }
            ViewNode::VStack(children) | ViewNode::HStack(children) => {
                collect_button_ids_fast(children, components, set);
            }
            ViewNode::ComponentCall { name, .. } => {
                if let Some(comp) = components.iter().find(|c| c.name == *name) {
                    collect_button_ids_fast(&comp.body, components, set);
                }
            }
            ViewNode::DynamicSection { body, .. } => {
                collect_button_ids_fast(body, components, set);
            }
            _ => {}
        }
    }
}

pub fn step_whens<S>(app: &App, state: &mut AppState<S>, events: &[UIEvent]) -> Option<String>
where
    S: StateAccess + 'static,
{
    let Some(tl) = state.current_timeline(app) else {
        return None;
    };

    // ButtonPressedイベントのみを処理対象とする（ButtonReleasedは除外）
    let clicked: Vec<&str> = events
        .iter()
        .filter_map(|ev| match ev {
            UIEvent::ButtonPressed { id } => Some(id.as_str()),
            UIEvent::ButtonReleased { id: _ } => {
                None // ButtonReleasedはwhen処理では無視
            }
            _ => None,
        })
        .collect();

    if !clicked.is_empty() {
        log::info!("Button clicked: {:?}", clicked);
        handle_button_onclick(app, state, &clicked);
    }

    for (_i, when) in tl.whens.iter().enumerate() {
        if let EventExpr::ButtonPressed(target) = &when.event {
            log::info!(
                "Checking when block for target: '{}' against clicked: {:?}",
                target,
                clicked
            );
            if clicked.iter().any(|&s| s == target) {
                log::info!("Processing when block for button: {}", target);
                for (_j, action) in when.actions.iter().enumerate() {
                    log::info!("Executing action: {:?}", action.node);
                    if let Some(new_tl) = apply_action(app, state, action) {
                        return Some(new_tl);
                    }
                }
            }
        }
    }

    None
}

pub fn apply_action<S>(
    _app: &App,
    state: &mut AppState<S>,
    action: &WithSpan<ViewNode>,
) -> Option<String>
where
    S: StateAccess + 'static,
{
    match &action.node {
        ViewNode::NavigateTo { target } => {
            // ルーティング対応のナビゲーション（パラメータなし）
            log::info!("Navigating to: {}", target);
            state.navigate_with_params(target, std::collections::HashMap::new());
            return Some(target.clone());
        }
        ViewNode::RustCall { name, args } => {
            state.handle_rust_call_viewnode(name, args);
        }
        ViewNode::Set { path, value, .. } => {
            // ★ 優先順位: 1. ローカル変数 → 2. state変数 → 3. その他の変数
            let key = path.trim().to_string();
            let v = state.eval_expr_from_ast(value);

            // 1. ローカル変数をチェック
            if state.component_context.get_local_var(&key).is_some() {
                // const変数への再代入チェック
                if state.component_context.is_const_var(&key) {
                    panic!("Cannot reassign to const variable '{}'", key);
                }
                log::debug!("Setting local variable '{}' = '{}'", key, v);
                state.component_context.set_local_var(key, v);
                // ★ ローカル変数変更時も再描画が必要
                state.needs_redraw = true;
                state.static_stencils = None;
            } else if path.starts_with("state.") {
                // 2. state変数
                let state_key = path.strip_prefix("state.").unwrap().trim().to_string();

                // state.xxxアクセス時はエラーでクラッシュ
                if let Err(e) = state.custom_state.set(&state_key, v.clone()) {
                    panic!(
                        "Failed to set state.{}: {:?}. State access failed - this should crash the application.",
                        state_key, e
                    );
                }
                // ★ state変数変更時も再描画が必要
                state.needs_redraw = true;
                state.static_stencils = None;
            } else {
                // 3. その他の変数
                state.variables.insert(key, v);
                // ★ 通常変数変更時も再描画が必要
                state.needs_redraw = true;
                state.static_stencils = None;
            }
        }
        ViewNode::Toggle { path } => {
            if path.starts_with("state.") {
                let key = path.strip_prefix("state.").unwrap().to_string();

                // state.xxxアクセス時はエラーでクラッシュ
                if let Err(e) = state.custom_state.toggle(&key) {
                    panic!(
                        "Failed to toggle state.{}: {:?}. State access failed - this should crash the application.",
                        key, e
                    );
                }
                // ★ state変数toggle時も再描画が必要
                state.needs_redraw = true;
                state.static_stencils = None;
            } else {
                let key = path.to_string();
                let cur = state
                    .variables
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| "false".into());
                let b = matches!(cur.as_str(), "true" | "1" | "True" | "TRUE");
                state.variables.insert(key, (!b).to_string());
                // ★ 通常変数toggle時も再描画が必要
                state.needs_redraw = true;
                state.static_stencils = None;
            }
        }
        ViewNode::ListAppend { path, value } => {
            if path.starts_with("state.") {
                let key = path.strip_prefix("state.").unwrap().to_string();
                let v = state.eval_expr_from_ast(value);

                // state.xxxアクセス時はエラーでクラッシュ
                if let Err(e) = state.custom_state.list_append(&key, v.clone()) {
                    panic!(
                        "Failed to append to state.{}: {:?}. State access failed - this should crash the application.",
                        key, e
                    );
                }
                // ★ state配列追加時も再描画が必要
                state.needs_redraw = true;
                state.static_stencils = None;
            } else {
                let key = path.to_string();
                let v = state.eval_expr_from_ast(value);
                let mut arr: Vec<String> = state
                    .variables
                    .get(&key)
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();
                arr.push(v);
                state
                    .variables
                    .insert(key, serde_json::to_string(&arr).unwrap());
                // ★ 配列追加時も再描画が必要
                state.needs_redraw = true;
                state.static_stencils = None;
            }
        }
        ViewNode::ListInsert { path, index, value } => {
            if path.starts_with("state.") {
                let key = path.strip_prefix("state.").unwrap().to_string();
                let v = state.eval_expr_from_ast(value);

                // state.xxxアクセス時はエラーでクラッシュ
                if let Err(e) = state.custom_state.list_insert(&key, *index, v.clone()) {
                    panic!(
                        "Failed to insert into state.{}: {:?}. State access failed - this should crash the application.",
                        key, e
                    );
                }
                // ★ state配列挿入時も再描画が必要
                state.needs_redraw = true;
                state.static_stencils = None;
            } else {
                let key = path.to_string();
                let v = state.eval_expr_from_ast(value);
                let mut arr: Vec<String> = state
                    .variables
                    .get(&key)
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();
                if *index <= arr.len() {
                    arr.insert(*index, v);
                    state
                        .variables
                        .insert(key, serde_json::to_string(&arr).unwrap());
                    // ★ 配列挿入時も再描画が必要
                    state.needs_redraw = true;
                    state.static_stencils = None;
                }
            }
        }
        ViewNode::ListRemove { path, value } => {
            if path.starts_with("state.") {
                let key = path.strip_prefix("state.").unwrap().to_string();
                let v = state.eval_expr_from_ast(value);

                // state.xxxアクセス時はエラーでクラッシュ
                if let Err(e) = state.custom_state.list_remove(&key, v.clone()) {
                    panic!(
                        "Failed to remove from state.{}: {:?}. State access failed - this should crash the application.",
                        key, e
                    );
                }
                // ★ state配列削除時も再描画が必要
                state.needs_redraw = true;
                state.static_stencils = None;
            } else {
                let key = path.to_string();
                let v = state.eval_expr_from_ast(value);
                let mut arr: Vec<String> = state
                    .variables
                    .get(&key)
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();
                // 値に一致する最初の要素を削除
                if let Some(pos) = arr.iter().position(|x| x == &v) {
                    arr.remove(pos);
                    state
                        .variables
                        .insert(key, serde_json::to_string(&arr).unwrap());
                    // ★ 配列削除時も再描画が必要
                    state.needs_redraw = true;
                    state.static_stencils = None;
                }
            }
        }
        ViewNode::ListClear { path } => {
            if path.starts_with("state.") {
                let key = path.strip_prefix("state.").unwrap().to_string();

                // state.xxxアクセス時はエラーでクラッシュ
                if let Err(e) = state.custom_state.list_clear(&key) {
                    panic!(
                        "Failed to clear state.{}: {:?}. State access failed - this should crash the application.",
                        key, e
                    );
                }
                // ★ state配列クリア時も再描画が必要
                state.needs_redraw = true;
                state.static_stencils = None;
            } else {
                let key = path.to_string();
                state.variables.insert(key, "[]".to_string());
                // ★ 配列クリア時も再描画が必要
                state.needs_redraw = true;
                state.static_stencils = None;
            }
        }
        ViewNode::LetDecl { .. } => {
            // ★ ローカル変数はタイムライン初期化時に一度だけ宣言される
            // 処理は initialize_local_variables() で行われる
            // ここでは何もしない
        }
        _ => {}
    }
    None
}
