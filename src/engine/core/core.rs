// src/engine/engine/core.rs
// Engine構造体とメインロジック

use super::component::expand_component_calls_lightweight;
use super::dynamic_section::collect_dynamic_sections;
use super::layout::*;
use crate::engine::state::{AppState, StateAccess};
use crate::parser::ast::App;
use crate::stencil::stencil::Stencil;

pub struct Engine;

impl Engine {
    /// 静的部分のレイアウト（キャッシュ対応）
    pub fn layout_static_part<S>(
        app: &App,
        state: &mut AppState<S>,
        nodes: &[crate::parser::ast::WithSpan<crate::parser::ast::ViewNode>],
        mouse_pos: [f32; 2],
        mouse_down: bool,
        prev_mouse_down: bool,
        window_size: [f32; 2],
    ) -> (
        Vec<Stencil>,
        Vec<(String, [f32; 2], [f32; 2])>,
        Vec<(String, [f32; 2], [f32; 2])>,
    )
    where
        S: StateAccess + 'static,
    {
        // layout_static_partログを削除
        // ★ レスポンシブスタイルを解決
        let resolved_nodes = resolve_responsive_nodes(nodes, state);

        let default_font = if let Some(tl) = state.current_timeline(app) {
            tl.font.clone().unwrap_or_else(|| "default".to_string())
        } else {
            "default".to_string()
        };
        let params = make_layout_params(window_size, default_font);
        layout_nodes_lightweight(
            app,
            state,
            &resolved_nodes,
            params,
            mouse_pos,
            mouse_down,
            prev_mouse_down,
            0,
        )
    }

    /// 動的部分のレイアウト（DynamicSectionのみ）
    /// このメソッドは毎フレーム呼び出され、キャッシュされません
    pub fn layout_dynamic_part<S>(
        app: &App,
        state: &mut AppState<S>,
        nodes: &[crate::parser::ast::WithSpan<crate::parser::ast::ViewNode>],
        mouse_pos: [f32; 2],
        mouse_down: bool,
        prev_mouse_down: bool,
        window_size: [f32; 2],
    ) -> (
        Vec<Stencil>,
        Vec<(String, [f32; 2], [f32; 2])>,
        Vec<(String, [f32; 2], [f32; 2])>,
    )
    where
        S: StateAccess + 'static,
    {
        use crate::parser::ast::Expr;
        use crate::ui::layout_vstack;

        // ★ レスポンシブスタイルを解決
        let resolved_nodes = resolve_responsive_nodes(nodes, state);

        let mut stencils = Vec::new();
        let mut buttons = Vec::new();
        let mut text_inputs = Vec::new();
        let default_font = if let Some(tl) = state.current_timeline(app) {
            tl.font.clone().unwrap_or_else(|| "default".to_string())
        } else {
            "default".to_string()
        };
        let params = make_layout_params(window_size, default_font.clone());
        let eval_fn = |e: &Expr| state.eval_expr_from_ast(e);
        let get_img_size = |path: &str| state.get_image_size(path);
        let layouted_all = layout_vstack(
            &resolved_nodes,
            params.clone(),
            app,
            &eval_fn,
            &get_img_size,
        );

        // DynamicSectionのみを処理（再帰的に探索）
        collect_dynamic_sections(
            &layouted_all,
            app,
            state,
            &mut stencils,
            &mut buttons,
            &mut text_inputs,
            mouse_pos,
            mouse_down,
            prev_mouse_down,
            window_size,
            &params.default_font,
        );

        (stencils, buttons, text_inputs)
    }

    /// メインのレイアウト＆ステンシル変換
    pub fn layout_and_stencil<S>(
        app: &App,
        state: &mut AppState<S>,
        mouse_pos: [f32; 2],
        mouse_down: bool,
        prev_mouse_down: bool,
        window_size: [f32; 2],
    ) -> (
        Vec<Stencil>,
        Vec<(String, [f32; 2], [f32; 2])>,
        Vec<(String, [f32; 2], [f32; 2])>,
    )
    where
        S: StateAccess + 'static,
    {
        state.all_buttons.clear();

        let Some(tl) = state.current_timeline(app) else {
            return (Vec::new(), Vec::new(), Vec::new());
        };

        // ★ ロジック処理: コンポーネント展開とローカル変数初期化（タイムライン変更時のみ）
        if state.expanded_body.is_none() {
            let expanded_nodes = expand_component_calls_lightweight(&tl.body, app, state);

            // ★ ローカル変数初期化（展開直後に1回だけ）
            if !state.local_vars_initialized {
                initialize_local_variables(&expanded_nodes, state);
                state.local_vars_initialized = true;
            }

            state.expanded_body = Some(expanded_nodes);
        } else if !state.local_vars_initialized {
            // ★ expanded_bodyは存在するがローカル変数未初期化の場合
            if let Some(ref expanded) = state.expanded_body {
                let expanded_clone = expanded.clone();
                initialize_local_variables(&expanded_clone, state);
                state.local_vars_initialized = true;
            }
        }

        // ★ レイアウトキャッシュの管理（画面サイズ変更時に無効化）
        let cache_invalid = state.cached_window_size.map_or(true, |cached| {
            (cached[0] - window_size[0]).abs() > 1.0 || (cached[1] - window_size[1]).abs() > 1.0
        });

        if cache_invalid {
            // ★ レイアウトキャッシュのみクリア（ロジックは保持）
            state.static_stencils = None;
            state.static_buttons.clear();
            state.static_text_inputs.clear();
            state.cached_window_size = Some(window_size);
        }

        let expanded = state.expanded_body.as_ref().unwrap().clone();

        // 静的部分はキャッシュを使用
        let (mut stencils, mut buttons, mut text_inputs) = if let Some(cached) = &state.static_stencils {
            (cached.clone(), state.static_buttons.clone(), state.static_text_inputs.clone())
        } else {
            let (s, b, t) = Self::layout_static_part(
                app,
                state,
                &expanded,
                mouse_pos,
                mouse_down,
                prev_mouse_down,
                window_size,
            );
            state.static_stencils = Some(s.clone());
            state.static_buttons = b.clone();
            state.static_text_inputs = t.clone();
            (s, b, t)
        };
        let (ds, db, dt) = Self::layout_dynamic_part(
            app,
            state,
            &expanded,
            mouse_pos,
            mouse_down,
            prev_mouse_down,
            window_size,
        );

        stencils.extend(ds);
        buttons.extend(db);
        text_inputs.extend(dt);

        // ★ タイムラインの背景色を追加（最背面に配置）
        if let Some(bg_color) = &tl.background {
            use crate::engine::state::to_rgba;
            use crate::parser::ast::ColorValue;

            // 背景色をパース（#ffffff形式）
            let color = if bg_color.starts_with('#') {
                to_rgba(&ColorValue::Hex(bg_color.clone()))
            } else {
                // デフォルトは白
                [1.0, 1.0, 1.0, 1.0]
            };

            // 全画面の背景矩形を最背面に挿入
            // depth = 1.0 が最背面（深度テストはLessなので、小さい値が前面に表示される）
            stencils.insert(
                0,
                Stencil::RoundedRect {
                    position: [0.0, 0.0],
                    width: window_size[0],
                    height: window_size[1],
                    radius: 0.0,
                    color,
                    scroll: false,
                    depth: 0.99999, // 最背面（深度テストで他の要素が優先される）
                },
            );
        }

        (stencils, buttons, text_inputs)
    }

    // イベント処理機能（eventモジュールへの委譲）
    pub fn step_whens<S>(
        app: &App,
        state: &mut AppState<S>,
        events: &[crate::ui::event::UIEvent],
    ) -> Option<String>
    where
        S: StateAccess + 'static,
    {
        super::event::step_whens(app, state, events)
    }

    pub fn sync_button_handlers<S, F>(
        nodes: &[crate::parser::ast::WithSpan<crate::parser::ast::ViewNode>],
        components: &[crate::parser::ast::Component],
        handlers: &mut std::collections::HashMap<String, Box<dyn FnMut(&mut AppState<S>)>>,
        default_handler: F,
    ) where
        S: StateAccess + 'static,
        F: Fn(&str) -> Box<dyn FnMut(&mut AppState<S>)>,
    {
        super::event::sync_button_handlers(nodes, components, handlers, default_handler)
    }

    pub fn apply_action<S>(
        app: &App,
        state: &mut AppState<S>,
        action: &crate::parser::ast::WithSpan<crate::parser::ast::ViewNode>,
    ) -> Option<String>
    where
        S: StateAccess + 'static,
    {
        super::event::apply_action(app, state, action)
    }
}
