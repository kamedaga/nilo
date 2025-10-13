// src/engine/engine/dynamic_section.rs
// DynamicSection関連

use super::utils::convert_to_rgba;
use crate::engine::state::{AppState, StateAccess};
use crate::parser::ast::{App, Expr, Style, ViewNode};
use crate::stencil::stencil::Stencil;
use crate::ui::{LayoutParams, layout::layout_vstack};

/// DynamicSectionを再帰的に収集して描画
pub fn collect_dynamic_sections<S>(
    layouted: &[crate::ui::LayoutedNode<'_>],
    app: &App,
    state: &mut AppState<S>,
    stencils: &mut Vec<Stencil>,
    buttons: &mut Vec<(String, [f32; 2], [f32; 2])>,
    text_inputs: &mut Vec<(String, [f32; 2], [f32; 2])>,
    mouse_pos: [f32; 2],
    mouse_down: bool,
    prev_mouse_down: bool,
    window_size: [f32; 2],
    default_font: &str,
) where
    S: StateAccess + 'static,
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
                let child_layouted = layout_vstack(children, params, app, &eval_fn, &get_img_size);
                collect_dynamic_sections(
                    &child_layouted,
                    app,
                    state,
                    stencils,
                    buttons,
                    text_inputs,
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
pub fn render_dynamic_section_background(
    lnode: &crate::ui::LayoutedNode<'_>,
    style: &Style,
    stencils: &mut Vec<Stencil>,
) {
    let mut depth_counter: f32 = 0.5; // DynamicSectionの背景は中間の深度

    // 背景色
    if let Some(bg) = &style.background {
        let bg_color = convert_to_rgba(bg);
        if bg_color[3] > 0.0 {
            let radius = style
                .rounded
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
                        let scol = color
                            .as_ref()
                            .map(|c| convert_to_rgba(c))
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
        let border_color = convert_to_rgba(border_color_ref);
        let border_width = 1.0;

        if border_color[3] > 0.0 && border_width > 0.0 {
            let radius = style
                .rounded
                .map(|r| match r {
                    crate::parser::ast::Rounded::On => 8.0,
                    crate::parser::ast::Rounded::Px(v) => v,
                })
                .unwrap_or(0.0);

            depth_counter += 0.001;
            stencils.push(Stencil::RoundedRect {
                position: [
                    lnode.position[0] - border_width / 2.0,
                    lnode.position[1] - border_width / 2.0,
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
