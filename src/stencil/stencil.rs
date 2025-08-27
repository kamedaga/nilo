use crate::renderer::command::{DrawCommand, DrawList};

#[derive(Clone, Debug)]
pub enum Stencil {
    Rect {
        position: [f32; 2],
        width: f32,
        height: f32,
        color: [f32; 4],
        scroll: bool,
        depth: f32, // ★ Z値追加（0.0=最前面、1.0=最背面）
    },
    Circle {
        center: [f32; 2],
        radius: f32,
        color: [f32; 4],
        scroll: bool,
        depth: f32, // ★ Z値追加
    },
    Triangle {
        p1: [f32; 2],
        p2: [f32; 2],
        p3: [f32; 2],
        color: [f32; 4],
        scroll: bool,
        depth: f32, // ★ Z値追加
    },
    Text {
        content: String,
        position: [f32; 2],
        size: f32,
        color: [f32; 4],
        font: String,
        scroll: bool,
        depth: f32, // ★ Z値追加
    },
    Image {
        position: [f32; 2],
        width: f32,
        height: f32,
        path: String,
        scroll: bool,
        depth: f32, // ★ Z値追加
    },
    Group(Vec<Stencil>),

    ScrollBar {
        content_length: f32,
        viewport_height: f32,
        scroll_offset_y: f32,
        viewport_width: f32,
        depth: f32, // ★ Z値追加
    },

    RoundedRect {
        position: [f32; 2],
        width: f32,
        height: f32,
        radius: f32,
        color: [f32; 4],
        scroll: bool,
        depth: f32, // ★ Z値追加
    },
}

pub fn stencil_to_wgpu_draw_list(stencils: &[Stencil]) -> DrawList {
    let mut draw_list = DrawList::new();

    // ★ depth値でソート（大きい=背面から先に描画）
    let mut sorted_stencils: Vec<&Stencil> = stencils.iter().collect();
    sorted_stencils.sort_by(|a, b| {
        let depth_a = get_stencil_depth(a);
        let depth_b = get_stencil_depth(b);
        depth_b.partial_cmp(&depth_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    fn recurse(stencil: &Stencil, draw_list: &mut DrawList) {
        match stencil {
            Stencil::Rect { position, width, height, color, scroll, depth } => draw_list.push(DrawCommand::Rect {
                position: *position, width: *width, height: *height, color: *color, scroll: *scroll, depth: *depth,
            }),
            Stencil::Circle { center, radius, color, scroll, depth } => draw_list.push(DrawCommand::Circle {
                center: *center, radius: *radius, color: *color, segments: 32, scroll: *scroll, depth: *depth,
            }),
            Stencil::Triangle { p1, p2, p3, color, scroll, depth } => draw_list.push(DrawCommand::Triangle {
                p1: *p1, p2: *p2, p3: *p3, color: *color, scroll: *scroll, depth: *depth,
            }),
            Stencil::Text { content, position, size, color, font, scroll, depth } => draw_list.push(DrawCommand::Text {
                content: content.clone(), position: *position, size: *size, color: *color, font: font.clone(), scroll: *scroll, depth: *depth,
            }),
            Stencil::Image { position, width, height, path, scroll, depth } => {
                draw_list.push(DrawCommand::Image {
                    position: *position,
                    width: *width,
                    height: *height,
                    path: path.clone(),
                    scroll: *scroll,
                    depth: *depth,
                });
            }
            Stencil::Group(children) => {
                for child in children {
                    recurse(child, draw_list);
                }
            }
            Stencil::ScrollBar { content_length, viewport_height, scroll_offset_y, viewport_width, depth } => {
                if content_length > viewport_height {
                    let max_scroll = content_length - viewport_height;
                    let scroll_y = (-scroll_offset_y).clamp(0.0, max_scroll);

                    let bar_width = 8.0;
                    let bar_x = viewport_width - bar_width;
                    let mut thumb_height = (viewport_height / content_length) * viewport_height;

                    // つまみの最小高さを設定（バーの幅の2.5倍以上にする）
                    let min_thumb_height = bar_width * 2.5;
                    thumb_height = thumb_height.max(min_thumb_height);

                    let max_thumb_y = viewport_height - thumb_height;

                    let radius = bar_width / 2.0;
                    let thumb_y = if max_scroll > 0.0 {
                        (scroll_y / max_scroll) * max_thumb_y
                    } else {
                        0.0
                    };

                    // スクロールバーの背景（より後ろに配置）
                    draw_list.push(DrawCommand::Rect {
                        position: [bar_x, 0.0],
                        width: bar_width,
                        height: *viewport_height,
                        color: [0.85, 0.85, 0.85, 1.0],
                        scroll: false,
                        depth: *depth + 0.1, // 背景は少し後ろに
                    });

                    // つまみの中央部分（高さが正の値の場合のみ描画）
                    let middle_height = thumb_height - bar_width;
                    if middle_height > 0.0 {
                        draw_list.push(DrawCommand::Rect {
                            position: [bar_x, thumb_y + radius],
                            width: bar_width,
                            height: middle_height,
                            color: [0.3, 0.3, 0.3, 1.0],
                            scroll: false,
                            depth: *depth, // つまみは前面に
                        });
                    }

                    // つまみの上端（円）
                    draw_list.push(DrawCommand::Circle {
                        center: [bar_x + radius, thumb_y + radius],
                        radius,
                        color: [0.3, 0.3, 0.3, 1.0],
                        segments: 32,
                        scroll: false,
                        depth: *depth, // つまみは前面に
                    });

                    // つまみの下端（円）
                    draw_list.push(DrawCommand::Circle {
                        center: [bar_x + radius, thumb_y + thumb_height - radius],
                        radius,
                        color: [0.3, 0.3, 0.3, 1.0],
                        segments: 32,
                        scroll: false,
                        depth: *depth, // つまみは前面に
                    });
                }
            }
            Stencil::RoundedRect { position, width, height, radius, color, scroll, depth } => {
                let [x, y] = *position;
                let w = width.max(0.0);
                let h = height.max(0.0);

                if w < 1.0 || h < 1.0 {
                    return;
                }

                let r = radius.min(w * 0.5).min(h * 0.5).max(0.0);

                if r <= 0.0 {
                    draw_list.push(DrawCommand::Rect {
                        position: [x, y],
                        width: w,
                        height: h,
                        color: *color,
                        scroll: *scroll,
                        depth: *depth,
                    });
                    return;
                }

                // 中央の大きな矩形
                draw_list.push(DrawCommand::Rect {
                    position: [x + r, y + r],
                    width: w - 2.0 * r,
                    height: h - 2.0 * r,
                    color: *color,
                    scroll: *scroll,
                    depth: *depth,
                });

                // 上下の矩形
                draw_list.push(DrawCommand::Rect {
                    position: [x + r, y],
                    width: w - 2.0 * r,
                    height: r,
                    color: *color,
                    scroll: *scroll,
                    depth: *depth,
                });
                draw_list.push(DrawCommand::Rect {
                    position: [x + r, y + h - r],
                    width: w - 2.0 * r,
                    height: r,
                    color: *color,
                    scroll: *scroll,
                    depth: *depth,
                });

                // 左右の矩形
                draw_list.push(DrawCommand::Rect {
                    position: [x, y + r],
                    width: r,
                    height: h - 2.0 * r,
                    color: *color,
                    scroll: *scroll,
                    depth: *depth,
                });
                draw_list.push(DrawCommand::Rect {
                    position: [x + w - r, y + r],
                    width: r,
                    height: h - 2.0 * r,
                    color: *color,
                    scroll: *scroll,
                    depth: *depth,
                });

                // 4つの角を円で描画
                // 左上の角
                draw_list.push(DrawCommand::Circle {
                    center: [x + r, y + r],
                    radius: r,
                    color: *color,
                    segments: 32,
                    scroll: *scroll,
                    depth: *depth,
                });

                // 右上の角
                draw_list.push(DrawCommand::Circle {
                    center: [x + w - r, y + r],
                    radius: r,
                    color: *color,
                    segments: 32,
                    scroll: *scroll,
                    depth: *depth,
                });

                // 左下の角
                draw_list.push(DrawCommand::Circle {
                    center: [x + r, y + h - r],
                    radius: r,
                    color: *color,
                    segments: 32,
                    scroll: *scroll,
                    depth: *depth,
                });

                // 右下の角
                draw_list.push(DrawCommand::Circle {
                    center: [x + w - r, y + h - r],
                    radius: r,
                    color: *color,
                    segments: 32,
                    scroll: *scroll,
                    depth: *depth,
                });
            }
        }
    }

    for stencil in &sorted_stencils {
        recurse(stencil, &mut draw_list);
    }

    draw_list
}

// ★ Stencilからdepth値を取得するヘルパー関数
fn get_stencil_depth(stencil: &Stencil) -> f32 {
    match stencil {
        Stencil::Rect { depth, .. } => *depth,
        Stencil::Circle { depth, .. } => *depth,
        Stencil::Triangle { depth, .. } => *depth,
        Stencil::Text { depth, .. } => *depth,
        Stencil::Image { depth, .. } => *depth,
        Stencil::ScrollBar { depth, .. } => *depth,
        Stencil::RoundedRect { depth, .. } => *depth,
        Stencil::Group(_) => 0.5, // デフォルト値
    }
}

// ★ f32の順序付けのためのWrapper
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct OrderedFloat(f32);

impl Eq for OrderedFloat {}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}
