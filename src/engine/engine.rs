// src/engine/engine.rs の軽量化版
use crate::parser::ast::{App, ViewNode, WithSpan, Expr, EventExpr, Component};
use crate::stencil::stencil::Stencil;
use crate::ui::{LayoutParams, layout_vstack};
use crate::ui::event::UIEvent;
use std::collections::{HashSet, HashMap};

use super::state::AppState;

pub struct Engine;

impl Engine {
    /// 静的部分のレイアウト（キャッシュ対応）
    pub fn layout_static_part<S>(
        app: &App,
        state: &mut AppState<S>,
        nodes: &[WithSpan<ViewNode>],
        mouse_pos: [f32; 2],
        mouse_down: bool,
        prev_mouse_down: bool,
        window_size: [f32; 2],
    ) -> (Vec<Stencil>, Vec<(String, [f32; 2], [f32; 2])>)
    where
        S: crate::engine::state::StateAccess + 'static,
    {
        // ホバー状態を正確に反映するため、キャッシュを無効化して毎フレーム再描画

        // ★ 追加: 現在のタイムラインのフォント設定を取得
        let default_font = if let Some(tl) = state.current_timeline(app) {
            // ★ デバッグ出力: フォント設定の確認
            tl.font.clone().unwrap_or_else(|| "default".to_string())
        } else {
            "default".to_string()
        };


        let params = LayoutParams {
            start: [20.0, 20.0],
            spacing: 12.0,
            window_size,
            parent_size: window_size,
            root_font_size: 16.0,
            font_size: 16.0,
            default_font: default_font.clone(), // ★ cloneしてから使用
        };

        // 毎回新しく計算して   バー状態を正確に反映
        let (stencils, buttons) = Self::layout_nodes_lightweight(
            app, state, nodes, params, mouse_pos, mouse_down, prev_mouse_down, 0
        );

        (stencils, buttons)
    }

    /// 動的部分のレイアウト（DynamicSectionのみ）
    pub fn layout_dynamic_part<S>(
        app: &App,
        state: &mut AppState<S>,
        nodes: &[WithSpan<ViewNode>],
        mouse_pos: [f32; 2],
        mouse_down: bool,
        prev_mouse_down: bool,
        window_size: [f32; 2],
    ) -> (Vec<Stencil>, Vec<(String, [f32; 2], [f32; 2])>)
    where
        S: crate::engine::state::StateAccess + 'static
    {
        let mut stencils = Vec::new();
        let mut buttons = Vec::new();

        // ★ 追加: 現在のタイムラインのフォント設定を取得
        let default_font = if let Some(tl) = state.current_timeline(app) {
            // ★ デバッグ出力: フォント設定の確認
            tl.font.clone().unwrap_or_else(|| "default".to_string())
        } else {
            "default".to_string()
        };


        let params = LayoutParams {
            start: [20.0, 20.0],
            spacing: 12.0,
            window_size,
            parent_size: window_size,
            root_font_size: 16.0,
            font_size: 16.0,
            default_font: default_font.clone(), // ★ cloneしてから使用
        };

        // 軽量化：DynamicSectionのみを対象とした高速���������������理
        let eval_fn = |e: &Expr| state.eval_expr_from_ast(e);
        let get_img_size = |path: &str| state.get_image_size(path);
        let layouted_all = layout_vstack(nodes, params, app, &eval_fn, &get_img_size);

        for lnode in &layouted_all {
            if let ViewNode::DynamicSection { body, .. } = &lnode.node.node {
                let inner_params = LayoutParams {
                    start: [lnode.position[0] + 16.0, lnode.position[1] + 36.0],
                    spacing: 8.0,
                    window_size,
                    parent_size: lnode.size,
                    root_font_size: 16.0,
                    font_size: 16.0,
                    default_font: default_font.clone(), // ★ フォント設定を継承
                };

                let (inner_stencils, inner_buttons) = Self::layout_nodes_lightweight(
                    app, state, body, inner_params, mouse_pos, mouse_down, prev_mouse_down, 1
                );

                stencils.extend(inner_stencils);
                buttons.extend(inner_buttons);
            }
        }

        (stencils, buttons)
    }

    /// 軽量化されたノードレイアウト処理
    fn layout_nodes_lightweight<S>(
        app: &App,
        state: &mut AppState<S>,
        nodes: &[WithSpan<ViewNode>],
        params: LayoutParams,
        mouse_pos: [f32; 2],
        mouse_down: bool,
        prev_mouse_down: bool,
        nest_level: u32,
    ) -> (Vec<Stencil>, Vec<(String, [f32; 2], [f32; 2])>)
    where
        S: crate::engine::state::StateAccess + 'static,
    {
        let mut stencils = Vec::new();
        let mut buttons = Vec::new();
        let mut depth_counter = (nest_level as f32) * 0.1;

        let eval_fn = |e: &Expr| state.eval_expr_from_ast(e);
        let get_img_size = |path: &str| state.get_image_size(path);

        // 軽量化：レイアウト計算を一��だけ実行
        let layouted = layout_vstack(nodes, params.clone(), app, &eval_fn, &get_img_size);

        // 軽量化：バッチ処理でstencil変換
        for lnode in &layouted {
            match &lnode.node.node {
                ViewNode::DynamicSection { .. } => continue, // 静的��理では無視
                ViewNode::Button { id, onclick, .. } => {
                    // ボタン境界を記録（デバッグ出力付き
                    buttons.push((id.clone(), lnode.position, lnode.size));

                    // onclickがある場合はstateに保存
                    if let Some(onclick_expr) = onclick {
                        state.button_onclick_map.insert(id.clone(), onclick_expr.clone());
                    }

                    // ホバー状態のチェックを最小限に
                    let is_hover = Self::is_point_in_rect(mouse_pos, lnode.position, lnode.size);
                    Self::render_button_lightweight(lnode, &mut stencils, &mut depth_counter, is_hover, &params.default_font);
                }
                ViewNode::TextInput { id, placeholder: _, value: _, on_change: _, multiline: _, max_length: _, ime_enabled: _ } => {
                    // テキスト入力フィールドを記録
                    buttons.push((id.clone(), lnode.position, lnode.size));

                    // テキ��ト入力フィールドの描画
                    Self::render_text_input_lightweight(lnode, state, &mut stencils, &mut depth_counter, mouse_pos);
                }
                ViewNode::Text { .. } => {
                    Self::render_text_lightweight(lnode, state, &mut stencils, &mut depth_counter, params.window_size, &params.default_font);
                }
                ViewNode::Image { .. } => {
                    Self::render_image_lightweight(lnode, &mut stencils, &mut depth_counter);
                }
                ViewNode::Stencil(stencil) => {
                    let mut offset_st = Self::offset_stencil_fast(stencil, lnode.position[0], lnode.position[1]);
                    Self::adjust_stencil_depth(&mut offset_st, &mut depth_counter);
                    stencils.push(offset_st);
                }
                ViewNode::RustCall { name, args } => {
                    // Rustコールを実行（レイアウト時��
                    state.handle_rust_call_viewnode(name, args);
                }
                ViewNode::ForEach { var, iterable, body } => {
                    // ★ 修正: params.window_sizeとparams.spacingを事前に取得
                    let window_size = params.window_size;
                    let spacing = params.spacing;
                    Self::render_foreach_optimized(lnode, var, iterable, body, app, state, &mut stencils, &mut depth_counter, window_size, spacing);
                }
                ViewNode::If { condition, then_body, else_body } => {
                    Self::render_if_optimized(lnode, condition, then_body, else_body, state, &mut stencils, &mut depth_counter);
                }
                _ => {
                    // その他の要素は従来通り
                    state.viewnode_layouted_to_stencil_with_depth_counter_helper(
                        lnode, app, &mut stencils, mouse_pos, mouse_down, prev_mouse_down, nest_level, &mut depth_counter,
                    );
                }
            }
        }
        (stencils, buttons)
    }

    /// 軽量化されたテキスト入力フィールド描画（IME対応）
    fn render_text_input_lightweight<S>(
        lnode: &crate::ui::LayoutedNode<'_>,
        state: &AppState<S>,
        stencils: &mut Vec<Stencil>,
        depth_counter: &mut f32,
        mouse_pos: [f32; 2],
    ) where
        S: crate::engine::state::StateAccess + 'static,
    {
        if let ViewNode::TextInput { id, placeholder, value, on_change, multiline, max_length, ime_enabled } = &lnode.node.node {
            let style = lnode.node.style.clone().unwrap_or_default();

            // スタイル設定
            let bg_color = style.background.as_ref()
                .map(|c| Self::convert_to_rgba(c))
                .unwrap_or([1.0, 1.0, 1.0, 1.0]); // デフォルト白色

            let border_color = style.border_color.as_ref()
                .map(|c| Self::convert_to_rgba(c))
                .unwrap_or([0.8, 0.8, 0.8, 1.0]); // デフォルトグレー

            let text_color = style.color.as_ref()
                .map(|c| Self::convert_to_rgba(c))
                .unwrap_or([0.0, 0.0, 0.0, 1.0]); // デフ��ルト黒色

            let font_size = style.font_size.unwrap_or(16.0);
            let radius = style.rounded
                .map(|r| match r {
                    crate::parser::ast::Rounded::On => 8.0,
                    crate::parser::ast::Rounded::Px(v) => v,
                })
                .unwrap_or(4.0);

            // フォーカス状態をチェック
            let is_focused = state.get_focused_text_input()
                .map(|focused_id| focused_id == id)
                .unwrap_or(false);

            // ホバー状態をチェック
            let is_hover = Self::is_point_in_rect(mouse_pos, lnode.position, lnode.size);

            // フォーカス時のボーダー色調整
            let effective_border_color = if is_focused {
                [0.3, 0.6, 1.0, 1.0] // フォーカス時は青色
            } else if is_hover {
                [0.6, 0.6, 0.6, 1.0] // ホバー時は少し濃いグレー
            } else {
                border_color
            };

            // 影の追加（フォーカス時とホバー時）
            if is_focused || is_hover {
                let shadow_color = if is_focused {
                    [0.2, 0.6, 1.0, 0.3] // フォーカス時: 青い影
                } else {
                    [0.0, 0.0, 0.0, 0.1] // ホバー時: 薄い黒い影
                };

                let shadow_offset = if is_focused { [0.0, 2.0] } else { [0.0, 1.0] };
                let shadow_blur = if is_focused { 8.0 } else { 4.0 };

                *depth_counter += 0.001;
                stencils.push(Stencil::RoundedRect {
                    position: [lnode.position[0] + shadow_offset[0], lnode.position[1] + shadow_offset[1]],
                    width: lnode.size[0],
                    height: lnode.size[1],
                    radius,
                    color: shadow_color,
                    scroll: true,
                    depth: (1.0 - *depth_counter).max(0.0),
                });
            }

            // 背景
            if bg_color[3] > 0.0 {
                *depth_counter += 0.001;
                stencils.push(Stencil::RoundedRect {
                    position: lnode.position,
                    width: lnode.size[0],
                    height: lnode.size[1],
                    radius,
                    color: bg_color,
                    scroll: true,
                    depth: (1.0 - *depth_counter).max(0.0),
                });
            }

            // ボーダー（より洗練されたスタイル）
            if effective_border_color[3] > 0.0 {
                let border_width = if is_focused { 2.0 } else { 1.0 };

                // 外側のボーダー
                *depth_counter += 0.001;
                stencils.push(Stencil::RoundedRect {
                    position: [lnode.position[0] - border_width / 2.0, lnode.position[1] - border_width / 2.0],
                    width: lnode.size[0] + border_width,
                    height: lnode.size[1] + border_width,
                    radius: radius + border_width / 2.0,
                    color: border_color,
                    scroll: true,
                    depth: (1.0 - *depth_counter).max(0.0),
                });

                // 内側の背景（ボーダー効果を作るため）
                *depth_counter += 0.001;
                stencils.push(Stencil::RoundedRect {
                    position: lnode.position,
                    width: lnode.size[0],
                    height: lnode.size[1],
                    radius,
                    color: bg_color,
                    scroll: true,
                    depth: (1.0 - *depth_counter).max(0.0),
                });
            }

            // テキスト描画（プレースホルダー考慮）
            let default_text_color = [0.2, 0.2, 0.2, 1.0]; // より濃いグレー
            let text_color = style.color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or(default_text_color);
            let current_value = state.get_text_input_value(id);
            let placeholder_text = placeholder.as_deref().unwrap_or("");
            let display_text = if current_value.is_empty() { placeholder_text.to_string() } else { current_value.clone() };

            // プレースホルダー表示時の色調整（より洗練された色）
            let effective_text_color = if current_value.is_empty() {
                [0.6, 0.6, 0.6, 1.0] // プレースホルダー用のミディアムグレー
            } else {
                text_color
            };

            // パディングを調整してより美しく
            let padding_x = 16.0;
            let padding_y = (lnode.size[1] - font_size * 1.2) / 2.0;

            *depth_counter += 0.001;
            stencils.push(Stencil::Text {
                content: display_text,
                position: [lnode.position[0] + padding_x, lnode.position[1] + padding_y],
                size: font_size,
                color: effective_text_color,
                font: style.font.unwrap_or_else(|| "default".to_string()),
                scroll: true,
                depth: (1.0 - *depth_counter).max(0.0),
            });

            // フォーカス時のカーソル描画
            if is_focused {
                let cursor_pos = state.get_text_cursor_position(id);
                // 簡易的なカーソル位置計算（実際の実装では文字幅を正確に計算する必要がある）
                let char_width = font_size * 0.6;
                let cursor_x = lnode.position[0] + padding_x + (cursor_pos as f32 * char_width);

                *depth_counter += 0.001;
                stencils.push(Stencil::Rect {
                    position: [cursor_x, lnode.position[1] + padding_y],
                    width: 2.0,
                    height: font_size * 1.2,
                    color: [0.2, 0.6, 1.0, 0.8], // 青いカーソル
                    scroll: true,
                    depth: (1.0 - *depth_counter).max(0.0),
                });
            }
        }
    }

    /// 軽量化されたボタン描画
    fn render_button_lightweight(
        lnode: &crate::ui::LayoutedNode<'_>,
        stencils: &mut Vec<Stencil>,
        depth_counter: &mut f32,
        is_hover: bool,
        default_font: &str, // ★ 追加: デフォルトフォントパラメータ
    ) {
        if let ViewNode::Button { label, .. } = &lnode.node.node {
            let style = lnode.node.style.clone().unwrap_or_default();

            // ★ 修正: スタイルプロパティから背景色を取得
            let bg_color = if let Some(ref bg) = style.background {
                Self::convert_to_rgba(bg)
            } else {
                // デフォルトの青色（スタイルが指定されていない場合のみ）
                if is_hover {
                    [0.09, 0.46, 0.82, 1.0] // ホバー時の青
                } else {
                    [0.13, 0.59, 0.95, 1.0] // 通常時の青
                }
            };

            // ★ 修正: ホバー時の色調整（スタイル指定時も考慮）
            let final_bg_color = if is_hover && style.background.is_some() {
                // スタイル指定時はホバーで少し暗くする
                [
                    (bg_color[0] * 0.8).max(0.0),
                    (bg_color[1] * 0.8).max(0.0),
                    (bg_color[2] * 0.8).max(0.0),
                    bg_color[3]
                ]
            } else {
                bg_color
            };

            // ★ 修正: スタイルからradius、font_sizeを取得
            let radius = style.rounded
                .map(|r| match r {
                    crate::parser::ast::Rounded::On => 8.0,
                    crate::parser::ast::Rounded::Px(v) => v,
                })
                .unwrap_or(6.0);

            let font_size = style.font_size.unwrap_or(16.0);

            // ★ 修正: テキスト色もスタイルから取得
            let text_color = style.color.as_ref()
                .map(|c| Self::convert_to_rgba(c))
                .unwrap_or([1.0, 1.0, 1.0, 1.0]); // デフォルト白色

            // ★ 重要: 透明色の場合は背景を描画しない
            if final_bg_color[3] > 0.0 {
                // 背景
                *depth_counter += 0.001;
                stencils.push(Stencil::RoundedRect {
                    position: lnode.position,
                    width: lnode.size[0],
                    height: lnode.size[1],
                    radius,
                    color: final_bg_color,
                    scroll: true,
                    depth: (1.0 - *depth_counter).max(0.0),
                });
            }

            // ★ 修正: 正確なテキスト幅計算を使用（layout.rsと同じ計算式）
            let text_w = Self::calculate_text_width_accurate(label, font_size);
            let text_h = font_size * 1.2;
            let tx = lnode.position[0] + (lnode.size[0] - text_w) * 0.5;
            let ty = lnode.position[1] + (lnode.size[1] - text_h) * 0.5;

            // ★ 修正: フォント選択の優先順位を正しく設定
            let font = style.font.as_ref()
                .map(|f| f.clone())
                .unwrap_or_else(|| default_font.to_string());

            *depth_counter += 0.001;
            stencils.push(Stencil::Text {
                content: label.clone(),
                position: [tx, ty],
                size: font_size,
                color: text_color,
                font,
                scroll: true,
                depth: (1.0 - *depth_counter).max(0.0),
            });
        }
    }

    /// 軽量化されたテキスト描画
    fn render_text_lightweight<S>(
        lnode: &crate::ui::LayoutedNode<'_>,
        state: &AppState<S>,
        stencils: &mut Vec<Stencil>,
        depth_counter: &mut f32,
        window_size: [f32; 2],
        default_font: &str, // ★ 追加: デフォルトフォントパラメータ
    ) where
        S: crate::engine::state::StateAccess + 'static,
    {
        if let ViewNode::Text { format, args } = &lnode.node.node {
            let style = lnode.node.style.clone().unwrap_or_default();

            // 軽量化：評価を最小限に
            let values: Vec<String> = args.iter().map(|e| state.eval_expr_from_ast(e)).collect();
            let content = Self::format_text_fast(format.as_str(), &values[..]);

            let font_size = if let Some(rel_font_size) = &style.relative_font_size {
                rel_font_size.to_px(
                    window_size[0], window_size[1],
                    lnode.size[0], lnode.size[1],
                    16.0, 16.0,
                )
            } else {
                style.font_size.unwrap_or(16.0)
            };

            let text_color = style.color.as_ref()
                .map(|c| Self::convert_to_rgba(c))
                .unwrap_or([0.0, 0.0, 0.0, 1.0]);

            let padding = style.padding.unwrap_or_default();

            // ★ 修正: フォント選択の優先順位を正しく設定
            let font = style.font.as_ref()
                .map(|f| f.clone())
                .unwrap_or_else(|| default_font.to_string());

            *depth_counter += 0.001;
            stencils.push(Stencil::Text {
                content,
                position: [lnode.position[0] + padding.left, lnode.position[1] + padding.top],
                size: font_size,
                color: text_color,
                font,
                scroll: true,
                depth: (1.0 - *depth_counter).max(0.0),
            });
        }
    }

    /// 軽量化された画像描画
    fn render_image_lightweight(
        lnode: &crate::ui::LayoutedNode<'_>,
        stencils: &mut Vec<Stencil>,
        depth_counter: &mut f32,
    ) {
        if let ViewNode::Image { path } = &lnode.node.node {
            *depth_counter += 0.001;
            stencils.push(Stencil::Image {
                position: lnode.position,
                width: lnode.size[0],
                height: lnode.size[1],
                path: path.clone(),
                scroll: true,
                depth: (1.0 - *depth_counter).max(0.0),
            });
        }
    }

    /// foreach制御の最適化描画
    fn render_foreach_optimized<S>(
        lnode: &crate::ui::LayoutedNode<'_>,
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
        S: crate::engine::state::StateAccess + 'static,
    {
        let iterable_value = state.eval_expr_from_ast(iterable);
        let items: Vec<String> = if iterable_value.starts_with('[') && iterable_value.ends_with(']') {
            serde_json::from_str::<Vec<serde_json::Value>>(&iterable_value)
                .map(|vs| vs.into_iter().map(|v| match v { serde_json::Value::String(s)=>s, serde_json::Value::Number(n)=>n.to_string(), serde_json::Value::Bool(b)=>b.to_string(), _=>v.to_string() }).collect())
                .unwrap_or_default()
        } else { vec![iterable_value] };

        let mut current_y_offset: f32 = 0.0;
        for (idx, item) in items.iter().enumerate() {
            // foreach変数を設定
            state.component_context.enter_foreach();
            state.component_context.set_foreach_var(var.to_string(), item.clone());
            state.component_context.set_foreach_var(format!("{}_index", var), idx.to_string());

            // 専用eval関数を作成（borrowing問題を回避）
            let eval_fn = |e: &Expr| -> String {
                match e {
                    Expr::String(s) => s.clone(),
                    Expr::Number(n) => n.to_string(),
                    Expr::Bool(b) => b.to_string(),
                    Expr::Ident(name) => {
                        // foreach変数を優先してチェック
                        if let Some(value) = state.component_context.get_foreach_var(name) {
                            value.clone()
                        } else {
                            state.eval_expr_from_ast(e)
                        }
                    }
                    _ => state.eval_expr_from_ast(e)
                }
            };
            let get_img_size = |path: &str| state.get_image_size(path);

            let item_params = crate::ui::LayoutParams {
                start: [lnode.position[0], lnode.position[1] + current_y_offset],
                spacing,
                window_size,
                parent_size: [lnode.size[0], window_size[1]],
                root_font_size: 16.0,
                font_size: 16.0,
                default_font: "default".to_string(), // ★ 不足していたフィールドを追加
            };
            let layouted = crate::ui::layout_vstack(body, item_params, app, &eval_fn, &get_img_size);

            // 描画 & 高さ計算（修正: 専用関数でforeach変数を正しく展開）
            let start_y = lnode.position[1] + current_y_offset;
            let mut item_height: f32 = 0.0;
            for ln in &layouted {
                Self::render_substituted_node_to_stencil_with_context(ln, stencils, depth_counter, state);
                let bottom = ln.position[1] + ln.size[1];
                let h = bottom - start_y;
                if h > item_height { item_height = h; }
            }
            current_y_offset += item_height;
            if idx < items.len()-1 { current_y_offset += spacing; }
            state.component_context.exit_foreach();
        }
    }

    fn render_if_optimized<S>(
        lnode: &crate::ui::LayoutedNode<'_>,
        condition: &Expr,
        then_body: &[WithSpan<ViewNode>],
        else_body: &Option<Vec<WithSpan<ViewNode>>>,
        state: &mut AppState<S>,
        stencils: &mut Vec<Stencil>,
        depth_counter: &mut f32,
    ) where S: crate::engine::state::StateAccess + 'static {
        let v = state.eval_expr_from_ast(condition);
        let truth = matches!(v.as_str(), "true"|"1"|"True"|"TRUE") || v.parse::<f32>().unwrap_or(0.0)!=0.0;
        let chosen: &[WithSpan<ViewNode>] = if truth { then_body } else { else_body.as_ref().map(|v| v.as_slice()).unwrap_or(&[]) };
        if chosen.is_empty() { return; }
        // 簡易: 子を縦積みレイアウト（spacing固定8）
        let mut y = lnode.position[1];
        for (i,node) in chosen.iter().enumerate() { match &node.node { ViewNode::Text { format, args } => {
                let values: Vec<String>=args.iter().map(|e| state.eval_expr_from_ast(e)).collect();
                let content = Self::format_text_fast(format, &values);
                let style = node.style.clone().unwrap_or_default();
                let font_size = style.font_size.unwrap_or(16.0);
                *depth_counter+=0.001; stencils.push(Stencil::Text { content, position:[lnode.position[0], y], size: font_size, color: style.color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.0,0.0,0.0,1.0]), font: style.font.unwrap_or_else(||"default".into()), scroll: true, depth:(1.0-*depth_counter).max(0.0)});
                y += font_size * 1.2 + 8.0;
            }
            _ => {}
        }; if i < chosen.len()-1 { y += 8.0; } }
    }

    fn render_substituted_node_to_stencil<S>(
        lnode: &crate::ui::LayoutedNode<'_>,
        stencils: &mut Vec<Stencil>,
        depth_counter: &mut f32,
        state: &AppState<S>,
    ) where S: crate::engine::state::StateAccess + 'static {
        match &lnode.node.node {
            ViewNode::Text { format, args } => {
                let values: Vec<String> = args.iter().map(|e| match e {
                    Expr::String(s) => s.clone(),
                    Expr::Number(n) => n.to_string(),
                    Expr::Bool(b) => b.to_string(),
                    Expr::Ident(id) => state.eval_expr_from_ast(&Expr::Ident(id.clone())),
                    _ => format!("{:?}", e)
                }).collect();
                let content = Self::format_text_fast(format, &values);
                if content.trim().is_empty() { return; }
                let style = lnode.node.style.clone().unwrap_or_default();
                let font_size = style.font_size.unwrap_or(16.0);
                let color = style.color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.0,0.0,0.0,1.0]);
                let padding = style.padding.unwrap_or_default();
                *depth_counter += 0.001;
                stencils.push(Stencil::Text { content, position:[lnode.position[0]+padding.left, lnode.position[1]+padding.top], size: font_size, color, font: style.font.unwrap_or_else(||"default".into()), scroll: true, depth:(1.0-*depth_counter).max(0.0)});
            }
            ViewNode::Button { label, .. } => {
                let style = lnode.node.style.clone().unwrap_or_default();
                let bg = style.background.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.13,0.59,0.95,1.0]);
                let font_size = style.font_size.unwrap_or(16.0);
                let radius = style.rounded.map(|r| match r { crate::parser::ast::Rounded::On=>8.0, crate::parser::ast::Rounded::Px(v)=>v }).unwrap_or(6.0);
                if bg[3] > 0.0 { *depth_counter += 0.001; stencils.push(Stencil::RoundedRect { position: lnode.position, width: lnode.size[0], height: lnode.size[1], radius, color: bg, scroll: true, depth:(1.0-*depth_counter).max(0.0)}); }
                let text_color = style.color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([1.0,1.0,1.0,1.0]);
                *depth_counter += 0.001;
                stencils.push(Stencil::Text { content: label.clone(), position:[lnode.position[0]+10.0, lnode.position[1]+5.0], size: font_size, color: text_color, font: style.font.unwrap_or_else(||"default".into()), scroll: true, depth:(1.0-*depth_counter).max(0.0)});
            }
            ViewNode::TextInput { id, placeholder, value, on_change, multiline, max_length, ime_enabled } => {
                let style = lnode.node.style.clone().unwrap_or_default();
                let bg = style.background.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([1.0, 1.0, 1.0, 1.0]);
                let border_color = style.border_color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.8, 0.8, 0.8, 1.0]);
                let font_size = style.font_size.unwrap_or(16.0);
                let radius = style.rounded.map(|r| match r { crate::parser::ast::Rounded::On=>8.0, crate::parser::ast::Rounded::Px(v)=>v }).unwrap_or(6.0);

                // 背景
                if bg[3] > 0.0 {
                    *depth_counter += 0.001;
                    stencils.push(Stencil::RoundedRect {
                        position: lnode.position,
                        width: lnode.size[0],
                        height: lnode.size[1],
                        radius,
                        color: bg,
                        scroll: true,
                        depth: (1.0 - *depth_counter).max(0.0),
                    });
                }

                // ボーダー
                if border_color[3] > 0.0 {
                    *depth_counter += 0.001;
                    stencils.push(Stencil::RoundedRect {
                        position: lnode.position,
                        width: lnode.size[0],
                        height: lnode.size[1],
                        radius,
                        color: border_color,
                        scroll: true,
                        depth: (1.0 - *depth_counter).max(0.0),
                    });
                }

                // テキスト描画（プレースホルダー考慮）
                let text_color = style.color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.0, 0.0, 0.0, 1.0]);
                let current_value = state.get_text_input_value(id);
                let placeholder_text = placeholder.as_deref().unwrap_or("");
                let display_text = if current_value.is_empty() { placeholder_text.to_string() } else { current_value.clone() };

                // ★ 修正: プレースホルダー表示時の色調整
                let effective_text_color = if current_value.is_empty() {
                    [text_color[0] * 0.7, text_color[1] * 0.7, text_color[2] * 0.7, text_color[3]]
                } else {
                    text_color
                };

                *depth_counter += 0.001;
                stencils.push(Stencil::Text {
                    content: display_text,
                    position: [lnode.position[0] + 8.0, lnode.position[1] + 8.0],
                    size: font_size,
                    color: effective_text_color,
                    font: style.font.unwrap_or_else(|| "default".to_string()),
                    scroll: true,
                    depth: (1.0 - *depth_counter).max(0.0),
                });
            }
            ViewNode::VStack(children) | ViewNode::HStack(children) => {
                for child in children {
                    let child_l = crate::ui::LayoutedNode { node: child, position: lnode.position, size: lnode.size };
                    Self::render_substituted_node_to_stencil(&child_l, stencils, depth_counter, state);
                }
            }
            _ => {}
        }
    }

    /// 軽量化されたノードをコンテキスト付きで描画
    fn render_substituted_node_to_stencil_with_context<S>(
        lnode: &crate::ui::LayoutedNode<'_>,
        stencils: &mut Vec<Stencil>,
        depth_counter: &mut f32,
        state: &AppState<S>,
    ) where S: crate::engine::state::StateAccess + 'static {
        match &lnode.node.node {
            ViewNode::Text { format, args } => {
                // foreach変数を考慮した評価関数を使用
                let values: Vec<String> = args.iter().map(|e| match e {
                    Expr::String(s) => s.clone(),
                    Expr::Number(n) => n.to_string(),
                    Expr::Bool(b) => b.to_string(),
                    Expr::Ident(name) => {
                        // foreach変数を最優先でチェック
                        if let Some(value) = state.component_context.get_foreach_var(name) {
                            value.clone()
                        } else {
                            state.eval_expr_from_ast(e)
                        }
                    }
                    Expr::Path(name) => {
                        // パス式でもforeach変数をチェック
                        if let Some(value) = state.component_context.get_foreach_var(name) {
                            value.clone()
                        } else {
                            state.eval_expr_from_ast(e)
                        }
                    }
                    _ => state.eval_expr_from_ast(e)
                }).collect();
                
                let content = Self::format_text_fast(format, &values);
                if content.trim().is_empty() { return; }
                
                let style = lnode.node.style.clone().unwrap_or_default();
                let font_size = style.font_size.unwrap_or(16.0);
                let color = style.color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.0,0.0,0.0,1.0]);
                let padding = style.padding.unwrap_or_default();
                
                *depth_counter += 0.001;
                stencils.push(Stencil::Text { 
                    content, 
                    position: [lnode.position[0] + padding.left, lnode.position[1] + padding.top], 
                    size: font_size, 
                    color, 
                    font: style.font.unwrap_or_else(||"default".into()), 
                    scroll: true, 
                    depth: (1.0 - *depth_counter).max(0.0)
                });
            }
            ViewNode::Button { label, .. } => {
                let style = lnode.node.style.clone().unwrap_or_default();
                let bg = style.background.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.13,0.59,0.95,1.0]);
                let font_size = style.font_size.unwrap_or(16.0);
                let radius = style.rounded.map(|r| match r { crate::parser::ast::Rounded::On=>8.0, crate::parser::ast::Rounded::Px(v)=>v }).unwrap_or(6.0);
                if bg[3] > 0.0 { *depth_counter += 0.001; stencils.push(Stencil::RoundedRect { position: lnode.position, width: lnode.size[0], height: lnode.size[1], radius, color: bg, scroll: true, depth:(1.0-*depth_counter).max(0.0)}); }
                let text_color = style.color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([1.0,1.0,1.0,1.0]);
                *depth_counter += 0.001;
                stencils.push(Stencil::Text { content: label.clone(), position: [lnode.position[0] + 10.0, lnode.position[1] + 5.0], size: font_size, color: text_color, font: style.font.unwrap_or_else(||"default".into()), scroll: true, depth: (1.0-*depth_counter).max(0.0)});
            }
            ViewNode::TextInput { id, placeholder, value, on_change, multiline, max_length, ime_enabled } => {
                let style = lnode.node.style.clone().unwrap_or_default();
                let bg = style.background.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([1.0, 1.0, 1.0, 1.0]);
                let border_color = style.border_color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.8, 0.8, 0.8, 1.0]);
                let font_size = style.font_size.unwrap_or(16.0);
                let radius = style.rounded.map(|r| match r { crate::parser::ast::Rounded::On=>8.0, crate::parser::ast::Rounded::Px(v)=>v }).unwrap_or(6.0);

                // 背景
                if bg[3] > 0.0 {
                    *depth_counter += 0.001;
                    stencils.push(Stencil::RoundedRect {
                        position: lnode.position,
                        width: lnode.size[0],
                        height: lnode.size[1],
                        radius,
                        color: bg,
                        scroll: true,
                        depth: (1.0 - *depth_counter).max(0.0),
                    });
                }

                // ボーダー
                if border_color[3] > 0.0 {
                    *depth_counter += 0.001;
                    stencils.push(Stencil::RoundedRect {
                        position: lnode.position,
                        width: lnode.size[0],
                        height: lnode.size[1],
                        radius,
                        color: border_color,
                        scroll: true,
                        depth: (1.0 - *depth_counter).max(0.0),
                    });
                }

                // テキスト描画（プレースホルダー考慮）
                let text_color = style.color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.0, 0.0, 0.0, 1.0]);
                let current_value = state.get_text_input_value(id);
                let placeholder_text = placeholder.as_deref().unwrap_or("");
                let display_text = if current_value.is_empty() { placeholder_text.to_string() } else { current_value.clone() };

                // ★ 修正: プレースホルダー表示時の色調整
                let effective_text_color = if current_value.is_empty() {
                    [text_color[0] * 0.7, text_color[1] * 0.7, text_color[2] * 0.7, text_color[3]]
                } else {
                    text_color
                };

                *depth_counter += 0.001;
                stencils.push(Stencil::Text {
                    content: display_text,
                    position: [lnode.position[0] + 8.0, lnode.position[1] + 8.0],
                    size: font_size,
                    color: effective_text_color,
                    font: style.font.unwrap_or_else(|| "default".to_string()),
                    scroll: true,
                    depth: (1.0 - *depth_counter).max(0.0),
                });
            }
            ViewNode::VStack(children) | ViewNode::HStack(children) => {
                for child in children {
                    let child_l = crate::ui::LayoutedNode { node: child, position: lnode.position, size: lnode.size };
                    Self::render_substituted_node_to_stencil_with_context(&child_l, stencils, depth_counter, state);
                }
            }
            _ => {
                // 他の要素は通常の関数で処理
                Self::render_substituted_node_to_stencil(lnode, stencils, depth_counter, state);
            }
        }
    }

    /// メインのレイアウト＆ステンシ���変換（最��化版��
    pub fn layout_and_stencil<S>(
        app: &App,
        state: &mut AppState<S>,
        mouse_pos: [f32; 2],
        mouse_down: bool,
        prev_mouse_down: bool,
        window_size: [f32; 2],
    ) -> (Vec<Stencil>, Vec<(String, [f32; 2], [f32; 2])>)
    where
        S: crate::engine::state::StateAccess + 'static,
    {
        state.all_buttons.clear();

        let Some(tl) = state.current_timeline(app) else {
            return (Vec::new(), Vec::new());
        };

        // 軽量化：   ャッシュの有効性チェックを高速化
        let cache_invalid = state.cached_window_size.map_or(true, |cached| {
            (cached[0] - window_size[0]).abs() > 1.0 || (cached[1] - window_size[1]).abs() > 1.0
        });

        if cache_invalid {
            state.static_stencils = None;
            state.static_buttons.clear();
            state.expanded_body = None;
            state.cached_window_size = Some(window_size);
        }

        // 軽量化���コ  ポーネン  展開のキャッシュ
        if state.expanded_body.is_none() {
            state.expanded_body = Some(expand_component_calls_lightweight(&tl.body, app, state));
        }

        // 借用チェッカ���対応：expandedを一度取��
        let expanded = state.expanded_body.as_ref().unwrap().clone();

        // 静的部分（キャッシュ利用���
        let (mut stencils, mut buttons) = Self::layout_static_part(
            app, state, &expanded, mouse_pos, mouse_down, prev_mouse_down, window_size
        );

        // 動的部分（毎フレーム   新）
        let (ds, db) = Self::layout_dynamic_part(
            app, state, &expanded, mouse_pos, mouse_down, prev_mouse_down, window_size
        );

        stencils.extend(ds);
        buttons.extend(db);

        (stencils, buttons)
    }

    /// 簡略化されたボ  ン同期
    pub fn sync_button_handlers<S>(
        nodes: &[WithSpan<ViewNode>],
        components: &[Component],
        handlers: &mut HashMap<String, Box<dyn FnMut(&mut AppState<S>)>>,
        default_handler: impl Fn(&str) -> Box<dyn FnMut(&mut AppState<S>)>,
    ) {
        let mut current_ids = HashSet::new();
        Self::collect_button_ids_fast(nodes, components, &mut current_ids);

        for id in &current_ids {
            handlers.entry(id.clone()).or_insert_with(|| default_handler(id));
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
                    Self::collect_button_ids_fast(children, components, set);
                }
                ViewNode::ComponentCall { name, .. } => {
                    if let Some(comp) = components.iter().find(|c| c.name == *name) {
                        Self::collect_button_ids_fast(&comp.body, components, set);
                    }
                }
                ViewNode::DynamicSection { body, .. } => {
                    Self::collect_button_ids_fast(body, components, set);
                }
                _ => {}
            }
        }
    }

    /// ホバー状態の変化をチェック
    fn has_hover_state_changed<S>(
        state: &AppState<S>,
        _nodes: &[WithSpan<ViewNode>],
        mouse_pos: [f32; 2],
    ) -> bool
    where
        S: crate::engine::state::StateAccess + 'static,
    {
        //   回のホバー状態と現在  ホバー状態を比較
        let mut current_hover_buttons = Vec::new();

        for (id, pos, size) in &state.static_buttons {
            let is_hover = Self::is_point_in_rect(mouse_pos, *pos, *size);
            if is_hover {
                current_hover_buttons.push(id.clone());
            }
        }

        // 前回記録されたホ  ���状態と比較（簡易   装）
        // 実際のアプリケーションでは   り詳細な状況追跡が必要
        false //    回は常に再描画���確実にホバー���態を反���
    }

    /// ボタンのonclick属性を処理
    fn handle_button_onclick<S>(
        _app: &App,
        state: &mut AppState<S>,
        clicked: &[&str],
    ) where
        S: crate::engine::state::StateAccess + 'static,
    {
        // clickedに含まれるボタンIDに対  するonclick属��を実行
        for id in clicked {
            if let Some(onclick_expr) = state.button_onclick_map.get(*id) {
                // onclick属性の   を評価して実行
                state.eval_expr_from_ast(onclick_expr);
            }
        }
    }

    /// 軽���化されたユー    ィリティ関数群
    #[inline]
    fn is_point_in_rect(point: [f32; 2], pos: [f32; 2], size: [f32; 2]) -> bool {
        point[0] >= pos[0] && point[0] <= pos[0] + size[0] &&
            point[1] >= pos[1] && point[1] <= pos[1] + size[1]
    }

    /// テキストの幅を正確に計算する関数（layout.rsと同じロジック）
    #[inline]
    fn calculate_text_width_accurate(text: &str, font_size: f32) -> f32 {
        let mut width = 0.0;
        for ch in text.chars() {
            if ch.is_ascii() {
                // 英数字・記号
                width += font_size * 0.6;
            } else {
                // 日本語文字（ひらがな、カタカナ、漢字）
                width += font_size * 1.0;
            }
        }
        width
    }

    #[inline]
    fn format_text_fast(fmt: &str, args: &[String]) -> String {
        let mut out = String::with_capacity(fmt.len() + args.iter().map(|s| s.len()).sum::<usize>());
        let mut i = 0;
        let mut chars = fmt.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '{' && chars.peek() == Some(&'}') {
                chars.next();
                if let Some(v) = args.get(i) {
                    out.push_str(v);
                }
                i += 1;
            } else {
                out.push(c);
            }
        }
        out
    }

    #[inline]
    fn convert_to_rgba(c: &crate::parser::ast::ColorValue) -> [f32; 4] {
        match c {
            crate::parser::ast::ColorValue::Rgba(v) => *v,
            crate::parser::ast::ColorValue::Hex(s) => Self::hex_to_rgba_fast(s),
        }
    }

    #[inline]
    fn hex_to_rgba_fast(s: &str) -> [f32; 4] {
        let t = s.trim().trim_start_matches('#');
        let parse2 = |h: &str| u8::from_str_radix(h, 16).unwrap_or(0) as f32 / 255.0;

        match t.len() {
            3 => {
                let r = parse2(&t[0..1].repeat(2));
                let g = parse2(&t[1..2].repeat(2));
                let b = parse2(&t[2..3].repeat(2));
                [r, g, b, 1.0]
            }
            6 => {
                let r = parse2(&t[0..2]);
                let g = parse2(&t[2..4]);
                let b = parse2(&t[4..6]);
                [r, g, b, 1.0]
            }
            8 => {
                let r = parse2(&t[0..2]);
                let g = parse2(&t[2..4]);
                let b = parse2(&t[4..6]);
                let a = parse2(&t[6..8]);
                [r, g, b, a]
            }
            _ => [0.0, 0.0, 0.0, 1.0],
        }
    }

    #[inline]
    fn offset_stencil_fast(stencil: &Stencil, dx: f32, dy: f32) -> Stencil {
        let mut result = stencil.clone();
        match &mut result {
            Stencil::Rect { position, .. } |
            Stencil::RoundedRect { position, .. } |
            Stencil::Text { position, .. } |
            Stencil::Image { position, .. } => {
                position[0] += dx;
                position[1] += dy;
            }
            Stencil::Circle { center, .. } => {
                center[0] += dx;
                center[1] += dy;
            }
            Stencil::Triangle { p1, p2, p3, .. } => {
                p1[0] += dx; p1[1] += dy;
                p2[0] += dx; p2[1] += dy;
                p3[0] += dx; p3[1] += dy;
            }
            _ => {}
        }
        result
    }

    #[inline]
    fn adjust_stencil_depth(stencil: &mut Stencil, depth_counter: &mut f32) {
        *depth_counter += 0.001;
        let depth = (1.0 - *depth_counter).max(0.0);

        match stencil {
            Stencil::Rect { depth: d, .. } |
            Stencil::RoundedRect { depth: d, .. } |
            Stencil::Text { depth: d, .. } |
            Stencil::Circle { depth: d, .. } |
            Stencil::Triangle { depth: d, .. } |
            Stencil::Image { depth: d, .. } => {
                *d = depth;
            }
            _ => {}
        }
    }

    pub fn step_whens<S>(app: &App, state: &mut AppState<S>, events: &[UIEvent]) -> Option<String>
    where
        S: crate::engine::state::StateAccess + 'static
    {
        let Some(tl) = state.current_timeline(app) else { return None; };

        // ButtonPressedイベント��ButtonReleasedイベントの両方��収集
        let clicked: Vec<&str> = events
            .iter()
            .filter_map(|ev| match ev {
                UIEvent::ButtonPressed { id } => Some(id.as_str()),
                UIEvent::ButtonReleased { id } => Some(id.as_str()),
                _ => None,
            })
            .collect();

        if !clicked.is_empty() {
            // ★ onclick��性の処理を追加
            Self::handle_button_onclick(app, state, &clicked);
        }

        for when in &tl.whens {
            if let EventExpr::ButtonPressed(target) = &when.event {
                if clicked.iter().any(|&s| s == target) {
                    for action in &when.actions {
                        if let Some(new_tl) = Self::apply_action(app, state, action) {
                            return Some(new_tl);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn apply_action<S>(_app: &App, state: &mut AppState<S>, action: &WithSpan<ViewNode>) -> Option<String>
    where
        S: crate::engine::state::StateAccess + 'static,
    {
        match &action.node {
            ViewNode::NavigateTo { target } => {
                state.jump_to_timeline(target);
                return Some(target.clone());
            }
            ViewNode::RustCall { name, args } => {
                // RustCallアクションを実行
                state.handle_rust_call_viewnode(name, args);
            }
            ViewNode::Set { path, value } => {
                let key = path.strip_prefix("state.").unwrap_or(path).trim().to_string();
                let v = state.eval_expr_from_ast(value);
                if state.custom_state.set(&key, v.clone()).is_err() {
                    state.variables.insert(key, v);
                }
            }
            ViewNode::Toggle { path } => {
                let key = path.strip_prefix("state.").unwrap_or(path).to_string();
                if state.custom_state.toggle(&key).is_err() {
                    let cur = state.variables.get(&key).cloned().unwrap_or_else(|| "false".into());
                    let b = matches!(cur.as_str(), "true" | "1" | "True" | "TRUE");
                    state.variables.insert(key, (!b).to_string());
                }
            }
            ViewNode::ListAppend { path, value } => {
                let key = path.strip_prefix("state.").unwrap_or(path).to_string();
                let v = state.eval_expr_from_ast(value);
                if state.custom_state.list_append(&key, v.clone()).is_err() {
                    let mut arr: Vec<String> = state
                        .variables
                        .get(&key)
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or_default();
                    arr.push(v);
                    state.variables.insert(key, serde_json::to_string(&arr).unwrap());
                }
            }
            ViewNode::ListRemove { path, index } => {
                let key = path.strip_prefix("state.").unwrap_or(path).to_string();
                if state.custom_state.list_remove(&key, *index).is_err() {
                    let mut arr: Vec<String> = state
                        .variables
                        .get(&key)
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or_default();
                    if *index < arr.len() {
                        arr.remove(*index);
                        state.variables.insert(key, serde_json::to_string(&arr).unwrap());
                    }
                }
            }
            _ => {}
        }
        None
    }
}

/// 軽��化されたコンポーネント   開
fn expand_component_calls_lightweight<S>(
    nodes: &[WithSpan<ViewNode>],
    app: &App,
    state: &mut AppState<S>,
) -> Vec<WithSpan<ViewNode>>
where
    S: crate::engine::state::StateAccess,
{
    // 軽量化：ComponentCallが含まれていない場合は早期リターン
    if !nodes.iter().any(|n| matches!(n.node, ViewNode::ComponentCall { .. })) {
        return nodes.to_vec();
    }

    expand_nodes_with_component_context_lightweight(nodes, app, state)
}

fn expand_nodes_with_component_context_lightweight<S>(
    nodes: &[WithSpan<ViewNode>],
    app: &App,
    state: &mut AppState<S>,
) -> Vec<WithSpan<ViewNode>>
where
    S: crate::engine::state::StateAccess,
{
    let mut result = Vec::with_capacity(nodes.len() * 2); // 予想サイズで事前確保

    for n in nodes {
        match &n.node {
            ViewNode::ComponentCall { name, args } => {
                if let Some(c) = app.components.iter().find(|cc| cc.name == *name) {
                    let mut component_args = HashMap::new();

                    for (param, arg) in c.params.iter().zip(args.iter()) {
                        let arg_value = match arg {
                            Expr::String(s) => s.clone(),
                            Expr::Number(n) => n.to_string(),
                            Expr::Bool(b) => b.to_string(),
                            Expr::Ident(id) => {
                                state.component_context.get_arg(id)
                                    .cloned()
                                    .unwrap_or_else(|| id.clone())
                            },
                            _ => format!("{:?}", arg),
                        };
                        component_args.insert(param.clone(), arg_value);
                    }

                    state.component_context.enter_component(name, component_args);
                    let expanded = expand_nodes_with_component_context_lightweight(&c.body, app, state);
                    state.component_context.exit_component();

                    result.extend(expanded);
                }
            }
            ViewNode::VStack(children) => {
                let expanded_children = expand_nodes_with_component_context_lightweight(children, app, state);
                result.push(WithSpan {
                    node: ViewNode::VStack(expanded_children),
                    line: n.line,
                    column: n.column,
                    style: n.style.clone(),
                });
            }
            ViewNode::HStack(children) => {
                let expanded_children = expand_nodes_with_component_context_lightweight(children, app, state);
                result.push(WithSpan {
                    node: ViewNode::HStack(expanded_children),
                    line: n.line,
                    column: n.column,
                    style: n.style.clone(),
                });
            }
            ViewNode::DynamicSection { name, body } => {
                let expanded_body = expand_nodes_with_component_context_lightweight(body, app, state);
                result.push(WithSpan {
                    node: ViewNode::DynamicSection {
                        name: name.clone(),
                        body: expanded_body,
                    },
                    line: n.line,
                    column: n.column,
                    style: n.style.clone(),
                });
            }
            _ => {
                result.push(substitute_viewnode_lightweight(n, state));
            }
        }
    }

    result
}

fn substitute_viewnode_lightweight<S>(
    node: &WithSpan<ViewNode>,
    state: &AppState<S>,
) -> WithSpan<ViewNode>
where
    S: crate::engine::state::StateAccess,
{
    let substituted_node = match &node.node {
        ViewNode::Text { format, args } => {
            let substituted_args: Vec<Expr> = args.iter().map(|arg| {
                match arg {
                    Expr::Ident(s) => {
                        if let Some(value) = state.component_context.get_arg_from_any_level(s) {
                            Expr::String(value.clone())
                        } else {
                            arg.clone()
                        }
                    },
                    _ => arg.clone(),
                }
            }).collect();

            ViewNode::Text {
                format: format.clone(),
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
