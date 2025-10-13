// src/engine/engine/render.rs
// レンダリング関連（各要素の描画）

use super::utils::*;
use crate::engine::state::{AppState, StateAccess};
use crate::parser::ast::{Expr, ViewNode};
use crate::stencil::stencil::Stencil;
use crate::ui::LayoutedNode;

/// 軽量化されたテキスト入力フィールド描画
pub fn render_text_input_lightweight<S>(
    lnode: &LayoutedNode<'_>,
    state: &AppState<S>,
    stencils: &mut Vec<Stencil>,
    depth_counter: &mut f32,
    mouse_pos: [f32; 2],
) where
    S: StateAccess + 'static,
{
    if let ViewNode::TextInput {
        id, placeholder, ..
    } = &lnode.node.node
    {
        let style = lnode.node.style.clone().unwrap_or_default();

        let bg_color = style
            .background
            .as_ref()
            .map(|c| convert_to_rgba(c))
            .unwrap_or([1.0, 1.0, 1.0, 1.0]);

        let border_color = style
            .border_color
            .as_ref()
            .map(|c| convert_to_rgba(c))
            .unwrap_or([0.8, 0.8, 0.8, 1.0]);

        let font_size = style.font_size.unwrap_or(16.0);
        let radius = style
            .rounded
            .map(|r| match r {
                crate::parser::ast::Rounded::On => 8.0,
                crate::parser::ast::Rounded::Px(v) => v,
            })
            .unwrap_or(4.0);

        let is_focused = state
            .get_focused_text_input()
            .map(|focused_id| focused_id == id)
            .unwrap_or(false);

        let is_hover = is_point_in_rect(mouse_pos, lnode.position, lnode.size);

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
                position: [
                    lnode.position[0] - border_width / 2.0,
                    lnode.position[1] - border_width / 2.0,
                ],
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

        let text_color = style
            .color
            .as_ref()
            .map(|c| convert_to_rgba(c))
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
pub fn render_button_lightweight(
    lnode: &LayoutedNode<'_>,
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
            Some(convert_to_rgba(bg))
        } else {
            // ★ デフォルトのボタン背景色を設定（透明ではなく実際の色）
            if is_hover {
                Some([0.09, 0.46, 0.82, 1.0]) // ホバー時の青色
            } else {
                Some([0.13, 0.59, 0.95, 1.0]) // 通常時の青色
            }
        };

        let radius = style
            .rounded
            .map(|r| match r {
                crate::parser::ast::Rounded::On => 8.0,
                crate::parser::ast::Rounded::Px(v) => v,
            })
            .unwrap_or(6.0);

        let font_size = style.font_size.unwrap_or(16.0);

        // ★ 修正: テキスト色のデフォルト値を改善
        let text_color = style
            .color
            .as_ref()
            .map(|c| convert_to_rgba(c))
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
                            let scol = color
                                .as_ref()
                                .map(|c| convert_to_rgba(c))
                                .unwrap_or([0.0, 0.0, 0.0, 0.25]);
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
            let border_color = convert_to_rgba(border_color_ref);
            let border_width = 1.0; // デフォルト値を使用

            if border_color[3] > 0.0 && border_width > 0.0 {
                *depth_counter += 0.001;
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
                    depth: (1.0 - *depth_counter).max(0.0),
                });
            }
        }

        // テキストの位置計算（中央寄せ）
        use crate::ui::text_measurement::measure_text_size;
        let (text_w, text_h) = measure_text_size(label, font_size, "default", None);
        let tx = lnode.position[0] + (lnode.size[0] - text_w) * 0.5;
        let ty = lnode.position[1] + (lnode.size[1] - text_h) * 0.5;

        let font = style
            .font
            .as_ref()
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
pub fn render_text_lightweight<S>(
    lnode: &LayoutedNode<'_>,
    state: &AppState<S>,
    stencils: &mut Vec<Stencil>,
    depth_counter: &mut f32,
    window_size: [f32; 2],
    _parent_size: [f32; 2],
    default_font: &str,
) where
    S: StateAccess + 'static,
{
    if let ViewNode::Text { format, args } = &lnode.node.node {
        let style = lnode.node.style.clone().unwrap_or_default();

        let values: Vec<String> = args.iter().map(|e| state.eval_expr_from_ast(e)).collect();
        let content = format_text_fast(format.as_str(), &values[..]);

        let font_size = if let Some(rel_font_size) = &style.relative_font_size {
            rel_font_size.to_px(
                window_size[0],
                window_size[1],
                lnode.size[0],
                lnode.size[1],
                16.0,
                16.0,
            )
        } else {
            style.font_size.unwrap_or(16.0)
        };

        let text_align = style.text_align.as_deref().unwrap_or("left");

        let text_color = style
            .color
            .as_ref()
            .map(|c| convert_to_rgba(c))
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);

        let padding = style.padding.unwrap_or_default();

        let font = style
            .font
            .as_ref()
            .or(style.font_family.as_ref())
            .map(|f| f.clone())
            .unwrap_or_else(|| default_font.to_string());

        // ★ 修正: 背景を一度だけ描画（背景が指定されている場合のみ）
        if let Some(bg) = &style.background {
            let bg_color = convert_to_rgba(bg);

            // 透明でない場合のみ背景を描画
            if bg_color[3] > 0.0 {
                let radius = style
                    .rounded
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
                            let scol = color
                                .as_ref()
                                .map(|c| convert_to_rgba(c))
                                .unwrap_or([0.0, 0.0, 0.0, 0.2]);
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
                    window_size[0],
                    window_size[1],
                    effective_parent_width,
                    effective_parent_width,
                    16.0,
                    font_size,
                );
                // パディングを考慮した幅を計算
                let available_width =
                    calculated_width.min(effective_parent_width - padding.left - padding.right);
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
pub fn render_image_lightweight(
    lnode: &LayoutedNode<'_>,
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

/// 軽量化されたノードをコンテキスト付きで描画
pub fn render_substituted_node_to_stencil_with_context<S>(
    lnode: &LayoutedNode<'_>,
    stencils: &mut Vec<Stencil>,
    depth_counter: &mut f32,
    state: &AppState<S>,
    window_size: [f32; 2],
    parent_size: [f32; 2],
) where
    S: StateAccess + 'static,
{
    match &lnode.node.node {
        ViewNode::Text { format, args } => {
            let values: Vec<String> = args
                .iter()
                .map(|e| match e {
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
                    _ => state.eval_expr_from_ast(e),
                })
                .collect();

            let content = format_text_fast(format, &values);
            if content.trim().is_empty() {
                return;
            }

            let style = lnode.node.style.clone().unwrap_or_default();
            let font_size = style.font_size.unwrap_or(16.0);
            let color = style
                .color
                .as_ref()
                .map(|c| convert_to_rgba(c))
                .unwrap_or([0.0, 0.0, 0.0, 1.0]);
            let padding = style.padding.unwrap_or_default();

            // max_width情報を取得（パディングを考慮）
            let max_width = if let Some(max_w) = style.max_width.as_ref() {
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
                None
            };

            *depth_counter += 0.001;
            stencils.push(Stencil::Text {
                content,
                position: [
                    lnode.position[0] + padding.left,
                    lnode.position[1] + padding.top,
                ],
                size: font_size,
                color,
                font: style.font.unwrap_or_else(|| "default".into()),
                max_width,
                scroll: true,
                depth: (1.0 - *depth_counter).max(0.0),
            });
        }
        ViewNode::Button { label, .. } => {
            let style = lnode.node.style.clone().unwrap_or_default();
            let bg = style
                .background
                .as_ref()
                .map(|c| convert_to_rgba(c))
                .unwrap_or([0.13, 0.59, 0.95, 1.0]);
            let font_size = style.font_size.unwrap_or(16.0);
            let radius = style
                .rounded
                .map(|r| match r {
                    crate::parser::ast::Rounded::On => 8.0,
                    crate::parser::ast::Rounded::Px(v) => v,
                })
                .unwrap_or(6.0);

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

            let text_color = style
                .color
                .as_ref()
                .map(|c| convert_to_rgba(c))
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);
            *depth_counter += 0.001;
            stencils.push(Stencil::Text {
                content: label.clone(),
                position: [lnode.position[0] + 10.0, lnode.position[1] + 5.0],
                size: font_size,
                color: text_color,
                font: style.font.unwrap_or_else(|| "default".into()),
                max_width: None, // CheckBoxでは改行しない
                scroll: true,
                depth: (1.0 - *depth_counter).max(0.0),
            });
        }
        _ => {}
    }
}
