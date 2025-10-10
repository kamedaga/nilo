// src/engine/engine.rs の軽量化版
use crate::parser::ast::{App, ViewNode, WithSpan, Expr, EventExpr, Component, Style};
use crate::stencil::stencil::Stencil;
use crate::ui::{LayoutParams, layout_vstack};
use crate::ui::event::UIEvent;
use std::collections::{HashSet, HashMap};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::state::AppState;

pub struct Engine;

impl Engine {
    /// 状態のハッシュを計算（動的セクションの変更検知用）
    fn compute_state_hash<S>(state: &S, fields: &[&str]) -> u64
    where
        S: crate::engine::state::StateAccess + 'static,
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
    fn extract_state_fields(nodes: &[WithSpan<ViewNode>]) -> Vec<String> {
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
                ViewNode::If { condition, then_body, else_body } => {
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
    
    /// LayoutParams生成の共通化
    fn make_layout_params(window_size: [f32; 2], default_font: String) -> LayoutParams {
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
    fn resolve_responsive_nodes<S>(
        nodes: &[WithSpan<ViewNode>],
        state: &AppState<S>,
    ) -> Vec<WithSpan<ViewNode>>
    where
        S: crate::engine::state::StateAccess + 'static,
    {
        nodes.iter().map(|node| {
            let mut resolved_node = node.clone();
            
            // スタイルがある場合、レスポンシブルールを解決
            if let Some(ref style) = node.style {
                let resolved_style = state.resolve_responsive_style(style);
                resolved_node.style = Some(resolved_style);
            }
            
            // 子ノードも再帰的に解決
            resolved_node.node = match &node.node {
                ViewNode::VStack(children) => {
                    ViewNode::VStack(Self::resolve_responsive_nodes(children, state))
                }
                ViewNode::HStack(children) => {
                    ViewNode::HStack(Self::resolve_responsive_nodes(children, state))
                }
                ViewNode::ForEach { var, iterable, body } => {
                    ViewNode::ForEach {
                        var: var.clone(),
                        iterable: iterable.clone(),
                        body: Self::resolve_responsive_nodes(body, state),
                    }
                }
                ViewNode::If { condition, then_body, else_body } => {
                    ViewNode::If {
                        condition: condition.clone(),
                        then_body: Self::resolve_responsive_nodes(then_body, state),
                        else_body: else_body.as_ref().map(|eb| Self::resolve_responsive_nodes(eb, state)),
                    }
                }
                ViewNode::Match { expr, arms, default } => {
                    ViewNode::Match {
                        expr: expr.clone(),
                        arms: arms.iter().map(|(pattern, body)| {
                            (pattern.clone(), Self::resolve_responsive_nodes(body, state))
                        }).collect(),
                        default: default.as_ref().map(|d| Self::resolve_responsive_nodes(d, state)),
                    }
                }
                ViewNode::DynamicSection { name, body } => {
                    ViewNode::DynamicSection {
                        name: name.clone(),
                        body: Self::resolve_responsive_nodes(body, state),
                    }
                }
                other => other.clone(),
            };
            
            resolved_node
        }).collect()
    }

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
        // layout_static_partログを削除
        // ★ レスポンシブスタイルを解決
        let resolved_nodes = Self::resolve_responsive_nodes(nodes, state);
        
        let default_font = if let Some(tl) = state.current_timeline(app) {
            tl.font.clone().unwrap_or_else(|| "default".to_string())
        } else {
            "default".to_string()
        };
        let params = Self::make_layout_params(window_size, default_font);
        Self::layout_nodes_lightweight(
            app, state, &resolved_nodes, params, mouse_pos, mouse_down, prev_mouse_down, 0
        )
    }

    /// 動的部分のレイアウト（DynamicSectionのみ）
    /// このメソッドは毎フレーム呼び出され、キャッシュされません
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
        // ★ レスポンシブスタイルを解決
        let resolved_nodes = Self::resolve_responsive_nodes(nodes, state);
        
        let mut stencils = Vec::new();
        let mut buttons = Vec::new();
        let default_font = if let Some(tl) = state.current_timeline(app) {
            tl.font.clone().unwrap_or_else(|| "default".to_string())
        } else {
            "default".to_string()
        };
        let params = Self::make_layout_params(window_size, default_font.clone());
        let eval_fn = |e: &Expr| state.eval_expr_from_ast(e);
        let get_img_size = |path: &str| state.get_image_size(path);
        let layouted_all = layout_vstack(&resolved_nodes, params.clone(), app, &eval_fn, &get_img_size);
        
        // DynamicSectionのみを処理（再帰的に探索）
        Self::collect_dynamic_sections(
            &layouted_all,
            app,
            state,
            &mut stencils,
            &mut buttons,
            mouse_pos,
            mouse_down,
            prev_mouse_down,
            window_size,
            &params.default_font,
        );
        
        (stencils, buttons)
    }

    /// DynamicSectionを再帰的に収集して描画
    fn collect_dynamic_sections<S>(
        layouted: &[crate::ui::LayoutedNode<'_>],
        app: &App,
        state: &mut AppState<S>,
        stencils: &mut Vec<Stencil>,
        buttons: &mut Vec<(String, [f32; 2], [f32; 2])>,
        mouse_pos: [f32; 2],
        mouse_down: bool,
        prev_mouse_down: bool,
        window_size: [f32; 2],
        default_font: &str,
    ) where
        S: crate::engine::state::StateAccess + 'static,
    {
        for lnode in layouted {
            match &lnode.node.node {
                ViewNode::DynamicSection { name: _, body: _ } => {
                    // DynamicSectionはレイアウト段階で展開済みなので、ここでは何もしない
                    // 実際の子要素は既にレイアウト済みでrenderingされる
                }
                ViewNode::VStack(children) | ViewNode::HStack(children) => {
                    // VStack/HStack内のDynamicSectionも処理
                    let eval_fn = |e: &Expr| state.eval_expr_from_ast(e);
                    let get_img_size = |path: &str| state.get_image_size(path);
                    let params = LayoutParams {
                        start: lnode.position,
                        spacing: 8.0,
                        window_size,
                        parent_size: lnode.size,
                        root_font_size: 16.0,
                        font_size: 16.0,
                        default_font: default_font.to_string(),
                    };
                    let child_layouted = crate::ui::layout::layout_vstack(
                        children,
                        params,
                        app,
                        &eval_fn,
                        &get_img_size
                    );
                    Self::collect_dynamic_sections(
                        &child_layouted,
                        app,
                        state,
                        stencils,
                        buttons,
                        mouse_pos,
                        mouse_down,
                        prev_mouse_down,
                        window_size,
                        default_font,
                    );
                }
                _ => {}
            }
        }
    }

    /// DynamicSectionの背景を描画
    fn render_dynamic_section_background(
        lnode: &crate::ui::LayoutedNode<'_>,
        style: &crate::parser::ast::Style,
        stencils: &mut Vec<Stencil>,
    ) {
        let mut depth_counter: f32 = 0.5; // DynamicSectionの背景は中間の深度
        
        // 背景色
        if let Some(bg) = &style.background {
            let bg_color = Self::convert_to_rgba(bg);
            if bg_color[3] > 0.0 {
                let radius = style.rounded
                    .map(|r| match r {
                        crate::parser::ast::Rounded::On => 8.0,
                        crate::parser::ast::Rounded::Px(v) => v,
                    })
                    .unwrap_or(0.0);
                
                // 影の描画
                if let Some(sh) = style.shadow.clone() {
                    let (off, scol) = match sh {
                        crate::parser::ast::Shadow::On => ([0.0, 2.0], [0.0, 0.0, 0.0, 0.2]),
                        crate::parser::ast::Shadow::Spec { offset, color, .. } => {
                            let scol = color.as_ref()
                                .map(|c| Self::convert_to_rgba(c))
                                .unwrap_or([0.0, 0.0, 0.0, 0.2]);
                            (offset, scol)
                        }
                    };
                    
                    depth_counter += 0.001;
                    stencils.push(Stencil::RoundedRect {
                        position: [lnode.position[0] + off[0], lnode.position[1] + off[1]],
                        width: lnode.size[0],
                        height: lnode.size[1],
                        radius,
                        color: [scol[0], scol[1], scol[2], (scol[3] * 0.9).min(1.0)],
                        scroll: true,
                        depth: (1.0_f32 - depth_counter).max(0.0),
                    });
                }
                
                // 背景
                depth_counter += 0.001;
                stencils.push(Stencil::RoundedRect {
                    position: lnode.position,
                    width: lnode.size[0],
                    height: lnode.size[1],
                    radius,
                    color: bg_color,
                    scroll: true,
                    depth: (1.0_f32 - depth_counter).max(0.0),
                });
            }
        }
        
        // ボーダー
        if let Some(border_color_ref) = &style.border_color {
            let border_color = Self::convert_to_rgba(border_color_ref);
            let border_width = 1.0;
            
            if border_color[3] > 0.0 && border_width > 0.0 {
                let radius = style.rounded
                    .map(|r| match r {
                        crate::parser::ast::Rounded::On => 8.0,
                        crate::parser::ast::Rounded::Px(v) => v,
                    })
                    .unwrap_or(0.0);
                
                depth_counter += 0.001;
                stencils.push(Stencil::RoundedRect {
                    position: [
                        lnode.position[0] - border_width / 2.0,
                        lnode.position[1] - border_width / 2.0
                    ],
                    width: lnode.size[0] + border_width,
                    height: lnode.size[1] + border_width,
                    radius: radius + border_width / 2.0,
                    color: border_color,
                    scroll: true,
                    depth: (1.0_f32 - depth_counter).max(0.0),
                });
            }
        }
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
        
        let layouted = layout_vstack(nodes, params.clone(), app, &eval_fn, &get_img_size);

        for lnode in &layouted {
            match &lnode.node.node {
                ViewNode::DynamicSection { .. } => continue,
                ViewNode::Button { id, onclick, .. } => {
                    buttons.push((id.clone(), lnode.position, lnode.size));

                    if let Some(onclick_expr) = onclick {
                        state.button_onclick_map.insert(id.clone(), onclick_expr.clone());
                    }

                    let is_hover = Self::is_point_in_rect(mouse_pos, lnode.position, lnode.size);
                    Self::render_button_lightweight(lnode, &mut stencils, &mut depth_counter, is_hover, &params.default_font);
                }
                ViewNode::TextInput { id, .. } => {
                    buttons.push((id.clone(), lnode.position, lnode.size));
                    Self::render_text_input_lightweight(lnode, state, &mut stencils, &mut depth_counter, mouse_pos);
                }
                ViewNode::Text { .. } => {
                    Self::render_text_lightweight(lnode, state, &mut stencils, &mut depth_counter, params.window_size, params.parent_size, &params.default_font);
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
                    state.handle_rust_call_viewnode(name, &args);
                }
                ViewNode::ForEach { var, iterable, body } => {
                    // レンダリング段階でForeach変数を適切に設定してレンダリング
                    let window_size = params.window_size;
                    let spacing = params.spacing;
                    Self::render_foreach_for_layout(lnode, var, iterable, &body, app, state, &mut stencils, &mut depth_counter, window_size, spacing);
                }
                ViewNode::If { condition, then_body, else_body } => {
                    Self::render_if_optimized(lnode, condition, &then_body, else_body, state, &mut stencils, &mut depth_counter, params.window_size, params.parent_size);
                }
                _ => {
                    state.viewnode_layouted_to_stencil_with_depth_counter_helper(
                        lnode, app, &mut stencils, mouse_pos, mouse_down, prev_mouse_down, nest_level, &mut depth_counter,
                    );
                }
            }
        }
        (stencils, buttons)
    }

    /// 軽量化されたテキスト入力フィールド描画
    fn render_text_input_lightweight<S>(
        lnode: &crate::ui::LayoutedNode<'_>,
        state: &AppState<S>,
        stencils: &mut Vec<Stencil>,
        depth_counter: &mut f32,
        mouse_pos: [f32; 2],
    ) where
        S: crate::engine::state::StateAccess + 'static,
    {
        if let ViewNode::TextInput { id, placeholder, .. } = &lnode.node.node {
            let style = lnode.node.style.clone().unwrap_or_default();

            let bg_color = style.background.as_ref()
                .map(|c| Self::convert_to_rgba(c))
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);

            let border_color = style.border_color.as_ref()
                .map(|c| Self::convert_to_rgba(c))
                .unwrap_or([0.8, 0.8, 0.8, 1.0]);

            let font_size = style.font_size.unwrap_or(16.0);
            let radius = style.rounded
                .map(|r| match r {
                    crate::parser::ast::Rounded::On => 8.0,
                    crate::parser::ast::Rounded::Px(v) => v,
                })
                .unwrap_or(4.0);

            let is_focused = state.get_focused_text_input()
                .map(|focused_id| focused_id == id)
                .unwrap_or(false);

            let is_hover = Self::is_point_in_rect(mouse_pos, lnode.position, lnode.size);

            let effective_border_color = if is_focused {
                [0.3, 0.6, 1.0, 1.0]
            } else if is_hover {
                [0.6, 0.6, 0.6, 1.0]
            } else {
                border_color
            };

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

            if effective_border_color[3] > 0.0 {
                let border_width = if is_focused { 2.0 } else { 1.0 };

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

            let text_color = style.color.as_ref()
                .map(|c| Self::convert_to_rgba(c))
                .unwrap_or([0.2, 0.2, 0.2, 1.0]);

            let current_value = state.get_text_input_value(id);
            let placeholder_text = placeholder.as_deref().unwrap_or("");
            let display_text = if current_value.is_empty() {
                placeholder_text.to_string()
            } else {
                current_value.clone()
            };

            let effective_text_color = if current_value.is_empty() {
                [0.6, 0.6, 0.6, 1.0]
            } else {
                text_color
            };

            let padding_x = 16.0;
            let padding_y = (lnode.size[1] - font_size * 1.2) / 2.0;

            *depth_counter += 0.001;
            stencils.push(Stencil::Text {
                content: display_text,
                position: [lnode.position[0] + padding_x, lnode.position[1] + padding_y],
                size: font_size,
                color: effective_text_color,
                font: style.font.unwrap_or_else(|| "default".to_string()),
                max_width: None, // TextInputでは改行しない
                scroll: true,
                depth: (1.0 - *depth_counter).max(0.0),
            });

            if is_focused {
                let cursor_pos = state.get_text_cursor_position(id);
                let char_width = font_size * 0.6;
                let cursor_x = lnode.position[0] + padding_x + (cursor_pos as f32 * char_width);

                *depth_counter += 0.001;
                stencils.push(Stencil::Rect {
                    position: [cursor_x, lnode.position[1] + padding_y],
                    width: 2.0,
                    height: font_size * 1.2,
                    color: [0.2, 0.6, 1.0, 0.8],
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
        default_font: &str,
    ) {
        if let ViewNode::Button { label, .. } = &lnode.node.node {
            let mut style = lnode.node.style.clone().unwrap_or_default();

            // ★ 修正: ホバー状態の場合、hoverスタイルをマージ
            if is_hover {
                if let Some(hover_style) = &style.hover {
                    style = style.merged(hover_style);
                }
            }

            // ★ 修正: 背景色処理を改善
            let bg_color = if let Some(ref bg) = style.background {
                Some(Self::convert_to_rgba(bg))
            } else {
                // ★ デフォルトのボタン背景色を設定（透明ではなく実際の色）
                if is_hover {
                    Some([0.09, 0.46, 0.82, 1.0]) // ホバー時の青色
                } else {
                    Some([0.13, 0.59, 0.95, 1.0]) // 通常時の青色
                }
            };

            let radius = style.rounded
                .map(|r| match r {
                    crate::parser::ast::Rounded::On => 8.0,
                    crate::parser::ast::Rounded::Px(v) => v,
                })
                .unwrap_or(6.0);

            let font_size = style.font_size.unwrap_or(16.0);

            // ★ 修正: テキスト色のデフォルト値を改善
            let text_color = style.color.as_ref()
                .map(|c| Self::convert_to_rgba(c))
                .unwrap_or_else(|| {
                    // 背景色に応じてテキスト色を決定
                    if bg_color.is_some() {
                        [1.0, 1.0, 1.0, 1.0] // 背景がある場合は白文字
                    } else {
                        [0.0, 0.0, 0.0, 1.0] // 背景がない場合は黒文字
                    }
                });

            // ★ 背景を描画（指定されている場合または透明でない場合のみ）
            if let Some(bg_rgba) = bg_color {
                if bg_rgba[3] > 0.0 {
                    // 影の描画
                    if let Some(sh) = style.shadow.clone() {
                        let (off, scol) = match sh {
                            crate::parser::ast::Shadow::On => ([0.0, 2.0], [0.0, 0.0, 0.0, 0.25]),
                            crate::parser::ast::Shadow::Spec { offset, color, .. } => {
                                let scol = color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.0, 0.0, 0.0, 0.25]);
                                (offset, scol)
                            }
                        };

                        *depth_counter += 0.001;
                        stencils.push(Stencil::RoundedRect {
                            position: [lnode.position[0] + off[0], lnode.position[1] + off[1]],
                            width: lnode.size[0],
                            height: lnode.size[1],
                            radius,
                            color: [scol[0], scol[1], scol[2], (scol[3] * 0.9).min(1.0)],
                            scroll: true,
                            depth: (1.0 - *depth_counter).max(0.0),
                        });
                    }

                    // 背景色の描画
                    *depth_counter += 0.001;
                    stencils.push(Stencil::RoundedRect {
                        position: lnode.position,
                        width: lnode.size[0],
                        height: lnode.size[1],
                        radius,
                        color: bg_rgba,
                        scroll: true,
                        depth: (1.0 - *depth_counter).max(0.0),
                    });
                }
            }

            // ボーダーの描画（指定されている場合）
            if let Some(border_color_ref) = &style.border_color {
                let border_color = Self::convert_to_rgba(border_color_ref);
                let border_width = 1.0; // デフォルト値を使用

                if border_color[3] > 0.0 && border_width > 0.0 {
                    *depth_counter += 0.001;
                    stencils.push(Stencil::RoundedRect {
                        position: [
                            lnode.position[0] - border_width / 2.0,
                            lnode.position[1] - border_width / 2.0
                        ],
                        width: lnode.size[0] + border_width,
                        height: lnode.size[1] + border_width,
                        radius: radius + border_width / 2.0,
                        color: border_color,
                        scroll: true,
                        depth: (1.0 - *depth_counter).max(0.0),
                    });
                }
            }

            // テキストの位置計算（中央寄せ）
            use crate::ui::text_measurement::measure_text_size;
            let (text_w, text_h) = measure_text_size(label, font_size, "default", None);
            let tx = lnode.position[0] + (lnode.size[0] - text_w) * 0.5;
            let ty = lnode.position[1] + (lnode.size[1] - text_h) * 0.5;

            let font = style.font.as_ref()
                .map(|f| f.clone())
                .unwrap_or_else(|| default_font.to_string());

            // テキストの描画
            *depth_counter += 0.001;
            stencils.push(Stencil::Text {
                content: label.clone(),
                position: [tx, ty],
                size: font_size,
                color: text_color,
                font,
                max_width: None, // Buttonでは改行しない
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
        _parent_size: [f32; 2],
        default_font: &str,
    ) where
        S: crate::engine::state::StateAccess + 'static,
    {
        if let ViewNode::Text { format, args } = &lnode.node.node {
            let style = lnode.node.style.clone().unwrap_or_default();

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

            let text_align = style.text_align.as_deref().unwrap_or("left");

            let text_color = style.color.as_ref()
                .map(|c| Self::convert_to_rgba(c))
                .unwrap_or([0.0, 0.0, 0.0, 1.0]);

            let padding = style.padding.unwrap_or_default();

            let font = style.font.as_ref()
                .or(style.font_family.as_ref())
                .map(|f| f.clone())
                .unwrap_or_else(|| default_font.to_string());

            // ★ 修正: 背景を一度だけ描画（背景が指定されている場合のみ）
            if let Some(bg) = &style.background {
                let bg_color = Self::convert_to_rgba(bg);

                // 透明でない場合のみ背景を描画
                if bg_color[3] > 0.0 {
                    let radius = style.rounded
                        .map(|r| match r {
                            crate::parser::ast::Rounded::On => 8.0,
                            crate::parser::ast::Rounded::Px(v) => v,
                        })
                        .unwrap_or(0.0);

                    // 影の描画（一度だけ）
                    if let Some(sh) = style.shadow.clone() {
                        let (off, scol) = match sh {
                            crate::parser::ast::Shadow::On => ([0.0, 2.0], [0.0, 0.0, 0.0, 0.2]),
                            crate::parser::ast::Shadow::Spec { offset, color, .. } => {
                                let scol = color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.0, 0.0, 0.0, 0.2]);
                                (offset, scol)
                            }
                        };

                        *depth_counter += 0.001;
                        stencils.push(Stencil::RoundedRect {
                            position: [lnode.position[0] + off[0], lnode.position[1] + off[1]],
                            width: lnode.size[0],
                            height: lnode.size[1],
                            radius,
                            color: [scol[0], scol[1], scol[2], (scol[3] * 0.9).min(1.0)],
                            scroll: true,
                            depth: (1.0 - *depth_counter).max(0.0),
                        });
                    }

                    // 背景の描画（一度だけ）
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
            }

            // ★ テキストアライメントに応じた位置計算
            use crate::ui::text_measurement::measure_text_size;
            let (text_width, _) = measure_text_size(&content, font_size, "default", None);
            let text_x = match text_align {
                "center" => lnode.position[0] + (lnode.size[0] - text_width) * 0.5 + padding.left,
                "right" => lnode.position[0] + lnode.size[0] - text_width - padding.right,
                _ => lnode.position[0] + padding.left, // "left" or default
            };

            // テキストの描画（一度だけ）
            // ★ max_width情報を取得
            // lnode.size[0]ではなく、parent_sizeを使用する必要がある
            let effective_parent_width = lnode.size[0];
            
            // ★ wrap プロパティを優先的にチェック
            let max_width = if let Some(wrap_mode) = style.wrap {
                use crate::parser::ast::WrapMode;
                match wrap_mode {
                    WrapMode::Auto => {
                        // 自動折り返し: 親要素の幅に合わせる
                        let available_width = effective_parent_width - padding.left - padding.right;
                        Some(available_width.max(0.0))
                    }
                    WrapMode::None => {
                        // 折り返ししない
                        None
                    }
                }
            } else if let Some(max_w) = style.max_width.as_ref() {
                // wrap が指定されていない場合は max_width を使用
                if max_w.unit == crate::parser::ast::Unit::Auto {
                    // max-width: autoの場合はparent_sizeを使用（パディングを差し引く）
                    let available_width = effective_parent_width - padding.left - padding.right;
                    Some(available_width.max(0.0))
                } else {
                    let calculated_width = max_w.to_px(
                        window_size[0], window_size[1],
                        effective_parent_width, effective_parent_width,
                        16.0, font_size,
                    );
                    // パディングを考慮した幅を計算
                    let available_width = calculated_width.min(effective_parent_width - padding.left - padding.right);
                    Some(available_width.max(0.0))
                }
            } else {
                // デフォルトは auto (自動折り返し)
                let available_width = effective_parent_width - padding.left - padding.right;
                Some(available_width.max(0.0))
            };
            
            *depth_counter += 0.001;
            stencils.push(Stencil::Text {
                content,
                position: [text_x, lnode.position[1] + padding.top],
                size: font_size,
                color: text_color,
                font,
                max_width,
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
                .map(|vs| vs.into_iter().map(|v| match v {
                    serde_json::Value::String(s) => s,
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => v.to_string()
                }).collect())
                .unwrap_or_default()
        } else {
            vec![iterable_value]
        };

        let mut current_y_offset: f32 = 0.0;
        for (idx, item) in items.iter().enumerate() {
            state.component_context.enter_foreach();
            state.component_context.set_foreach_var(var.to_string(), item.clone());
            state.component_context.set_foreach_var(format!("{}_index", var), idx.to_string());

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
                    _ => state.eval_expr_from_ast(e)
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
                Self::render_substituted_node_to_stencil_with_context(ln, stencils, depth_counter, state, window_size, [lnode.size[0], window_size[1]]);
                let bottom = ln.position[1] + ln.size[1];
                let h = bottom - start_y;
                if h > item_height { item_height = h; }
            }
            current_y_offset += item_height;
            if idx < items.len()-1 { current_y_offset += spacing; }
            state.component_context.exit_foreach();
        }
    }

    /// レイアウト済みのForeachノードをレンダリング
    fn render_foreach_for_layout<S>(
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
        log::info!("render_foreach_for_layout: 開始！var={}, iterable={:?}", var, iterable);
        let iterable_value = state.eval_expr_from_ast(iterable);
        let items: Vec<String> = if iterable_value.starts_with('[') && iterable_value.ends_with(']') {
            serde_json::from_str::<Vec<serde_json::Value>>(&iterable_value)
                .map(|vs| vs.into_iter().map(|v| match v {
                    serde_json::Value::String(s) => s,
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => v.to_string()
                }).collect())
                .unwrap_or_default()
        } else {
            vec![iterable_value]
        };

        let item_height = lnode.size[1] / items.len().max(1) as f32;
        
        for (idx, item) in items.iter().enumerate() {
            // foreach変数を設定
            state.component_context.enter_foreach();
            state.component_context.set_foreach_var(var.to_string(), item.clone());
            state.component_context.set_foreach_var(format!("{}_index", var), idx.to_string());

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

    fn render_if_optimized<S>(
        lnode: &crate::ui::LayoutedNode<'_>,
        condition: &Expr,
        then_body: &[WithSpan<ViewNode>],
        else_body: &Option<Vec<WithSpan<ViewNode>>>,
        state: &mut AppState<S>,
        stencils: &mut Vec<Stencil>,
        depth_counter: &mut f32,
        window_size: [f32; 2],
        parent_size: [f32; 2],
    ) where S: crate::engine::state::StateAccess + 'static {
        let v = state.eval_expr_from_ast(condition);
        
        let truth = matches!(v.as_str(), "true"|"1"|"True"|"TRUE") || v.parse::<f32>().unwrap_or(0.0) != 0.0;
        
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
                    let values: Vec<String> = args.iter().map(|e| state.eval_expr_from_ast(e)).collect();
                    let content = Self::format_text_fast(format, &values);
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
                                window_size[0], window_size[1],
                                parent_size[0], parent_size[1],
                                16.0, font_size,
                            );
                            let available_width = calculated_width.min(parent_size[0] - padding.left - padding.right);
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
                        color: style.color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.0,0.0,0.0,1.0]),
                        font: style.font.unwrap_or_else(||"default".into()),
                        max_width,
                        scroll: true,
                        depth: (1.0-*depth_counter).max(0.0)
                    });
                    y += font_size * 1.2 + 8.0;
                }
                _ => {}
            }
            if i < chosen.len()-1 { y += 8.0; }
        }
    }

    /// 軽量化されたノードをコンテキ��ト付きで描画
    fn render_substituted_node_to_stencil_with_context<S>(
        lnode: &crate::ui::LayoutedNode<'_>,
        stencils: &mut Vec<Stencil>,
        depth_counter: &mut f32,
        state: &AppState<S>,
        window_size: [f32; 2],
        parent_size: [f32; 2],
    ) where S: crate::engine::state::StateAccess + 'static {
        match &lnode.node.node {
            ViewNode::Text { format, args } => {
                let values: Vec<String> = args.iter().map(|e| match e {
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
                    Expr::Path(name) => {
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
                
                // max_width情報を取得（パディングを考慮）
                let max_width = if let Some(max_w) = style.max_width.as_ref() {
                    if max_w.unit == crate::parser::ast::Unit::Auto {
                        let available_width = parent_size[0] - padding.left - padding.right;
                        Some(available_width.max(0.0))
                    } else {
                        let calculated_width = max_w.to_px(
                            window_size[0], window_size[1],
                            parent_size[0], parent_size[1],
                            16.0, font_size,
                        );
                        let available_width = calculated_width.min(parent_size[0] - padding.left - padding.right);
                        Some(available_width.max(0.0))
                    }
                } else {
                    None
                };
                
                *depth_counter += 0.001;
                stencils.push(Stencil::Text { 
                    content, 
                    position: [lnode.position[0] + padding.left, lnode.position[1] + padding.top], 
                    size: font_size, 
                    color, 
                    font: style.font.unwrap_or_else(||"default".into()), 
                    max_width,
                    scroll: true, 
                    depth: (1.0 - *depth_counter).max(0.0)
                });
            }
            ViewNode::Button { label, .. } => {
                let style = lnode.node.style.clone().unwrap_or_default();
                let bg = style.background.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([0.13,0.59,0.95,1.0]);
                let font_size = style.font_size.unwrap_or(16.0);
                let radius = style.rounded.map(|r| match r {
                    crate::parser::ast::Rounded::On => 8.0,
                    crate::parser::ast::Rounded::Px(v) => v
                }).unwrap_or(6.0);

                if bg[3] > 0.0 {
                    *depth_counter += 0.001;
                    stencils.push(Stencil::RoundedRect {
                        position: lnode.position,
                        width: lnode.size[0],
                        height: lnode.size[1],
                        radius,
                        color: bg,
                        scroll: true,
                        depth: (1.0-*depth_counter).max(0.0)
                    });
                }

                let text_color = style.color.as_ref().map(|c| Self::convert_to_rgba(c)).unwrap_or([1.0,1.0,1.0,1.0]);
                *depth_counter += 0.001;
                stencils.push(Stencil::Text {
                    content: label.clone(),
                    position: [lnode.position[0] + 10.0, lnode.position[1] + 5.0],
                    size: font_size,
                    color: text_color,
                    font: style.font.unwrap_or_else(||"default".into()),
                    max_width: None, // CheckBoxでは改行しない
                    scroll: true,
                    depth: (1.0-*depth_counter).max(0.0)
                });
            }
            _ => {}
        }
    }

    /// メインのレイアウト＆ステンシル変換
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

        let cache_invalid = state.cached_window_size.map_or(true, |cached| {
            (cached[0] - window_size[0]).abs() > 1.0 || (cached[1] - window_size[1]).abs() > 1.0
        });

        if cache_invalid {
            state.static_stencils = None;
            state.static_buttons.clear();
            state.expanded_body = None;
            state.cached_window_size = Some(window_size);
        }

        if state.expanded_body.is_none() {
            state.expanded_body = Some(expand_component_calls_lightweight(&tl.body, app, state));
        }

        let expanded = state.expanded_body.as_ref().unwrap().clone();

        let (mut stencils, mut buttons) = Self::layout_static_part(
            app, state, &expanded, mouse_pos, mouse_down, prev_mouse_down, window_size
        );

        let (ds, db) = Self::layout_dynamic_part(
            app, state, &expanded, mouse_pos, mouse_down, prev_mouse_down, window_size
        );

        stencils.extend(ds);
        buttons.extend(db);

        (stencils, buttons)
    }

    /// 簡略化されたボタン同期
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

    /// ボタンのonclick属性��処理
    fn handle_button_onclick<S>(
        _app: &App,
        state: &mut AppState<S>,
        clicked: &[&str],
    ) where
        S: crate::engine::state::StateAccess + 'static,
    {
        for id in clicked {
            if let Some(onclick_expr) = state.button_onclick_map.get(*id) {
                state.eval_expr_from_ast(onclick_expr);
            }
        }
    }

    /// ユーティリティ関数群
    #[inline]
    fn is_point_in_rect(point: [f32; 2], pos: [f32; 2], size: [f32; 2]) -> bool {
        point[0] >= pos[0] && point[0] <= pos[0] + size[0] &&
            point[1] >= pos[1] && point[1] <= pos[1] + size[1]
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
        let Some(tl) = state.current_timeline(app) else {
            return None;
        };

        // ButtonPressedイベントのみを処理対象とする（ButtonReleasedは除外）
        let clicked: Vec<&str> = events
            .iter()
            .filter_map(|ev| match ev {
                UIEvent::ButtonPressed { id } => {
                    Some(id.as_str())
                },
                UIEvent::ButtonReleased { id: _ } => {
                    None  // ButtonReleasedはwhen処理では無視
                },
                _ => None,
            })
            .collect();

        if !clicked.is_empty() {
            Self::handle_button_onclick(app, state, &clicked);
        }

        for (_i, when) in tl.whens.iter().enumerate() {
            if let EventExpr::ButtonPressed(target) = &when.event {
                if clicked.iter().any(|&s| s == target) {
                    for (_j, action) in when.actions.iter().enumerate() {
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
                state.handle_rust_call_viewnode(name, args);
            }
            ViewNode::Set { path, value, .. } => {
                if path.starts_with("state.") {
                    let key = path.strip_prefix("state.").unwrap().trim().to_string();
                    let v = state.eval_expr_from_ast(value);

                    // state.xxxアクセス時はエラーでクラッシュ
                    if let Err(e) = state.custom_state.set(&key, v.clone()) {
                        panic!("Failed to set state.{}: {:?}. State access failed - this should crash the application.", key, e);
                    }
                } else {
                    let key = path.trim().to_string();
                    let v = state.eval_expr_from_ast(value);
                    state.variables.insert(key, v);
                }
            }
            ViewNode::Toggle { path } => {
                if path.starts_with("state.") {
                    let key = path.strip_prefix("state.").unwrap().to_string();

                    // state.xxxアクセス時はエラーでクラッシュ
                    if let Err(e) = state.custom_state.toggle(&key) {
                        panic!("Failed to toggle state.{}: {:?}. State access failed - this should crash the application.", key, e);
                    }
                } else {
                    let key = path.to_string();
                    let cur = state.variables.get(&key).cloned().unwrap_or_else(|| "false".into());
                    let b = matches!(cur.as_str(), "true" | "1" | "True" | "TRUE");
                    state.variables.insert(key, (!b).to_string());
                }
            }
            ViewNode::ListAppend { path, value } => {
                if path.starts_with("state.") {
                    let key = path.strip_prefix("state.").unwrap().to_string();
                    let v = state.eval_expr_from_ast(value);

                    // state.xxxアクセス時はエラーでクラッシュ
                    if let Err(e) = state.custom_state.list_append(&key, v.clone()) {
                        panic!("Failed to append to state.{}: {:?}. State access failed - this should crash the application.", key, e);
                    }
                } else {
                    let key = path.to_string();
                    let v = state.eval_expr_from_ast(value);
                    let mut arr: Vec<String> = state
                        .variables
                        .get(&key)
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or_default();
                    arr.push(v);
                    state.variables.insert(key, serde_json::to_string(&arr).unwrap());
                }
            }
            ViewNode::ListInsert { path, index, value } => {
                if path.starts_with("state.") {
                    let key = path.strip_prefix("state.").unwrap().to_string();
                    let v = state.eval_expr_from_ast(value);

                    // state.xxxアクセス時はエラーでクラッシュ
                    if let Err(e) = state.custom_state.list_insert(&key, *index, v.clone()) {
                        panic!("Failed to insert into state.{}: {:?}. State access failed - this should crash the application.", key, e);
                    }
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
                        state.variables.insert(key, serde_json::to_string(&arr).unwrap());
                    }
                }
            }
            ViewNode::ListRemove { path, value } => {
                if path.starts_with("state.") {
                    let key = path.strip_prefix("state.").unwrap().to_string();
                    let v = state.eval_expr_from_ast(value);

                    // state.xxxアクセス時はエラーでクラッシュ
                    if let Err(e) = state.custom_state.list_remove(&key, v.clone()) {
                        panic!("Failed to remove from state.{}: {:?}. State access failed - this should crash the application.", key, e);
                    }
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
                        state.variables.insert(key, serde_json::to_string(&arr).unwrap());
                    }
                }
            }
            ViewNode::ListClear { path } => {
                if path.starts_with("state.") {
                    let key = path.strip_prefix("state.").unwrap().to_string();

                    // state.xxxアクセス時はエラーでクラッシュ
                    if let Err(e) = state.custom_state.list_clear(&key) {
                        panic!("Failed to clear state.{}: {:?}. State access failed - this should crash the application.", key, e);
                    }
                } else {
                    let key = path.to_string();
                    state.variables.insert(key, "[]".to_string());
                }
            }
            _ => {}
        }
        None
    }
}

/// 軽量化されたコンポーネント展開
fn expand_component_calls_lightweight<S>(
    nodes: &[WithSpan<ViewNode>],
    app: &App,
    _state: &mut AppState<S>,
) -> Vec<WithSpan<ViewNode>>
where
    S: crate::engine::state::StateAccess + 'static,
{
    let mut result = Vec::new();

    for node in nodes {
        match &node.node {
            ViewNode::ComponentCall { name, args } => {
                if let Some(comp) = app.components.iter().find(|c| c.name == *name) {
                    
                    // コンポーネントのボディをクローンして引数を適用
                    let mut expanded_body = comp.body.clone();
                    
                    // パラメータの置換を実行
                    for (i, arg) in args.iter().enumerate() {
                        if let Some(param_name) = comp.params.get(i) {
                            // Parameter substitution
                            substitute_parameter_in_nodes(&mut expanded_body, param_name, arg);
                        }
                    }
                    
                    // デフォルトスタイルを適用（ComponentCallのスタイルがない場合のみ）
                    if let Some(default_style) = &comp.default_style {
                        apply_default_style_to_nodes(&mut expanded_body, default_style);
                    }
                    
                    // ComponentCallにスタイルがある場合は、それを最初のノードに適用（優先）
                    if let Some(call_style) = &node.style {
                        if let Some(first_node) = expanded_body.first_mut() {
                            // ComponentCallのスタイルを最優先で適用
                            match &mut first_node.style {
                                Some(existing_style) => {
                                    // ComponentCallのスタイルを既存のスタイルにマージ（ComponentCallが優先）
                                    merge_styles_prioritize_override(existing_style, call_style);
                                }
                                None => {
                                    first_node.style = Some(call_style.clone());
                                }
                            }
                        }
                    }
                    
                    result.extend(expanded_body);
                }
            }
            ViewNode::VStack(children) => {
                let expanded_children = expand_component_calls_lightweight(children, app, _state);
                result.push(WithSpan {
                    node: ViewNode::VStack(expanded_children),
                    line: node.line,
                    column: node.column,
                    style: node.style.clone(),
                });
            }
            ViewNode::HStack(children) => {
                let expanded_children = expand_component_calls_lightweight(children, app, _state);
                result.push(WithSpan {
                    node: ViewNode::HStack(expanded_children),
                    line: node.line,
                    column: node.column,
                    style: node.style.clone(),
                });
            }
            _ => {
                result.push(node.clone());
            }
        }
    }

    result
}

/// ノード内のパラメータを置換する
fn substitute_parameter_in_nodes(nodes: &mut [WithSpan<ViewNode>], param_name: &str, arg: &Expr) {
    for node in nodes {
        substitute_parameter_in_node(&mut node.node, param_name, arg);
    }
}

/// 単一ノード内のパラメータを置換する
fn substitute_parameter_in_node(node: &mut ViewNode, param_name: &str, arg: &Expr) {
    match node {
        ViewNode::Text { args, .. } => {
            // Text argsの中でパラメータを探す
            for text_arg in args {
                if let Expr::Path(path) = text_arg {
                    if path == param_name {
                        // Text parameter replacement
                        *text_arg = arg.clone();
                    }
                }
            }
        }
        ViewNode::VStack(children) | ViewNode::HStack(children) => {
            substitute_parameter_in_nodes(children, param_name, arg);
        }
        // 他のノードタイプも必要に応じて追加
        _ => {}
    }
}

/// ComponentCallのスタイルを既存のスタイルにマージ（ComponentCallが優先）
fn merge_styles_prioritize_override(existing: &mut Style, override_style: &Style) {
    // ComponentCallで指定されたスタイルを最優先で適用
    if override_style.relative_width.is_some() {
        existing.relative_width = override_style.relative_width;
    }
    if override_style.relative_height.is_some() {
        existing.relative_height = override_style.relative_height;
    }
    if override_style.width.is_some() {
        existing.width = override_style.width;
    }
    if override_style.height.is_some() {
        existing.height = override_style.height;
    }
    if override_style.background.is_some() {
        existing.background = override_style.background.clone();
    }
    if override_style.color.is_some() {
        existing.color = override_style.color.clone();
    }
    if override_style.padding.is_some() {
        existing.padding = override_style.padding.clone();
    }
    if override_style.margin.is_some() {
        existing.margin = override_style.margin.clone();
    }
    if override_style.relative_padding.is_some() {
        existing.relative_padding = override_style.relative_padding.clone();
    }
    if override_style.relative_margin.is_some() {
        existing.relative_margin = override_style.relative_margin.clone();
    }
}

/// ノードにデフォルトスタイルを適用する
fn apply_default_style_to_nodes(nodes: &mut [WithSpan<ViewNode>], default_style: &Style) {
    for node in nodes {
        // ルートノードにデフォルトスタイルをマージ
        match &mut node.style {
            Some(existing_style) => {
                // デフォルトスタイルの値を既存のスタイルにマージ（既存の値を優先）
                if existing_style.relative_width.is_none() && default_style.relative_width.is_some() {
                    existing_style.relative_width = default_style.relative_width;
                }
                if existing_style.relative_height.is_none() && default_style.relative_height.is_some() {
                    existing_style.relative_height = default_style.relative_height;
                }
                if existing_style.width.is_none() && default_style.width.is_some() {
                    existing_style.width = default_style.width;
                }
                if existing_style.height.is_none() && default_style.height.is_some() {
                    existing_style.height = default_style.height;
                }
                if existing_style.background.is_none() && default_style.background.is_some() {
                    existing_style.background = default_style.background.clone();
                }
                if existing_style.color.is_none() && default_style.color.is_some() {
                    existing_style.color = default_style.color.clone();
                }
                if existing_style.padding.is_none() && default_style.padding.is_some() {
                    existing_style.padding = default_style.padding.clone();
                }
                if existing_style.margin.is_none() && default_style.margin.is_some() {
                    existing_style.margin = default_style.margin.clone();
                }
                if existing_style.relative_padding.is_none() && default_style.relative_padding.is_some() {
                    existing_style.relative_padding = default_style.relative_padding.clone();
                }
                if existing_style.relative_margin.is_none() && default_style.relative_margin.is_some() {
                    existing_style.relative_margin = default_style.relative_margin.clone();
                }
            }
            None => {
                // Applying default style to node without existing style
                node.style = Some(default_style.clone());
            }
        }
    }
}
