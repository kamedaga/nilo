use crate::renderer_abstract::command::{DrawCommand, DrawList};

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
        max_width: Option<f32>, // ★ max_width制約を追加
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

    /// スクロールコンテナ（overflow: scroll対応）
    ScrollContainer {
        id: String,  // ★ ScrollContainerの一意なID
        position: [f32; 2],
        width: f32,
        height: f32,
        overflow_mode: crate::parser::ast::OverflowMode,
        children: Vec<Stencil>,
        depth: f32,
    },
}

pub fn stencil_to_wgpu_draw_list(stencils: &[Stencil]) -> DrawList {
    let mut draw_list = DrawList::new();

    // ★ depth値でソート（大きい=背面から先に描画）
    let mut sorted_stencils: Vec<&Stencil> = stencils.iter().collect();
    sorted_stencils.sort_by(|a, b| {
        let depth_a = get_stencil_depth(a);
        let depth_b = get_stencil_depth(b);
        depth_b
            .partial_cmp(&depth_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    fn recurse(stencil: &Stencil, draw_list: &mut DrawList) {
        match stencil {
            Stencil::Rect {
                position,
                width,
                height,
                color,
                scroll,
                depth,
            } => draw_list.push(DrawCommand::Rect {
                position: *position,
                width: *width,
                height: *height,
                color: *color,
                scroll: *scroll,
                depth: *depth,
            }),
            Stencil::Circle {
                center,
                radius,
                color,
                scroll,
                depth,
            } => draw_list.push(DrawCommand::Circle {
                center: *center,
                radius: *radius,
                color: *color,
                segments: 32,
                scroll: *scroll,
                depth: *depth,
            }),
            Stencil::Triangle {
                p1,
                p2,
                p3,
                color,
                scroll,
                depth,
            } => draw_list.push(DrawCommand::Triangle {
                p1: *p1,
                p2: *p2,
                p3: *p3,
                color: *color,
                scroll: *scroll,
                depth: *depth,
            }),
            Stencil::Text {
                content,
                position,
                size,
                color,
                font,
                max_width,
                scroll,
                depth,
            } => draw_list.push(DrawCommand::Text {
                content: content.clone(),
                position: *position,
                size: *size,
                color: *color,
                font: font.clone(),
                max_width: *max_width,
                scroll: *scroll,
                depth: *depth,
            }),
            Stencil::Image {
                position,
                width,
                height,
                path,
                scroll,
                depth,
            } => {
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
            Stencil::ScrollBar {
                content_length,
                viewport_height,
                scroll_offset_y,
                viewport_width,
                depth,
            } => {
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
            Stencil::RoundedRect {
                position,
                width,
                height,
                radius,
                color,
                scroll,
                depth,
            } => {
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
            Stencil::ScrollContainer {
                position,
                width,
                height,
                overflow_mode,
                children,
                depth,
                ..
            } => {
                use crate::parser::ast::OverflowMode;
                
                // ★ ScrollContainerをDrawCommandとして出力
                match overflow_mode {
                    OverflowMode::Visible => {
                        // visible: 子要素をそのまま描画（クリッピングなし）
                        for child in children {
                            recurse(child, draw_list);
                        }
                    }
                    OverflowMode::Hidden | OverflowMode::Scroll | OverflowMode::Auto => {
                        // ★ ScrollContainerとして出力（クリッピング有効）
                        let mut child_commands = Vec::new();
                        let mut temp_list = DrawList::new();
                        
                        for child in children {
                            recurse(child, &mut temp_list);
                        }
                        
                        child_commands.extend(temp_list.0);
                        
                        // ★ IDを生成（position + size のハッシュから生成）
                        let id = format!("scroll_{}_{}_{}_{}", 
                            (position[0] * 10.0) as i32,
                            (position[1] * 10.0) as i32,
                            (*width * 10.0) as i32,
                            (*height * 10.0) as i32
                        );
                        
                        draw_list.push(DrawCommand::ScrollContainer {
                            id,
                            position: *position,
                            width: *width,
                            height: *height,
                            children: child_commands,
                            scroll_offset: [0.0, 0.0], // 初期値（後でランタイムが更新）
                            depth: *depth,
                        });
                    }
                }
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
        Stencil::ScrollContainer { depth, .. } => *depth,
        Stencil::Group(_) => 0.5, // デフォルト値
    }
}

// ★ クリッピング付きでStencilを再帰的に処理
fn recurse_with_clipping(
    stencil: &Stencil,
    draw_list: &mut DrawList,
    clip_pos: [f32; 2],
    clip_width: f32,
    clip_height: f32,
) {
    // クリッピング領域の境界
    let clip_left = clip_pos[0];
    let clip_top = clip_pos[1];
    let clip_right = clip_pos[0] + clip_width;
    let clip_bottom = clip_pos[1] + clip_height;

    // 要素がクリッピング領域と交差するかチェック
    let intersects = |pos: [f32; 2], w: f32, h: f32| -> bool {
        let right = pos[0] + w;
        let bottom = pos[1] + h;
        !(right < clip_left || pos[0] > clip_right || bottom < clip_top || pos[1] > clip_bottom)
    };

    match stencil {
        Stencil::Rect {
            position,
            width,
            height,
            color,
            scroll,
            depth,
        } => {
            if intersects(*position, *width, *height) {
                draw_list.push(DrawCommand::Rect {
                    position: *position,
                    width: *width,
                    height: *height,
                    color: *color,
                    scroll: *scroll,
                    depth: *depth,
                });
            }
        }
        Stencil::Circle {
            center,
            radius,
            color,
            scroll,
            depth,
        } => {
            let pos = [center[0] - radius, center[1] - radius];
            let size = radius * 2.0;
            if intersects(pos, size, size) {
                draw_list.push(DrawCommand::Circle {
                    center: *center,
                    radius: *radius,
                    color: *color,
                    segments: 32,
                    scroll: *scroll,
                    depth: *depth,
                });
            }
        }
        Stencil::Triangle {
            p1,
            p2,
            p3,
            color,
            scroll,
            depth,
        } => {
            // 三角形のバウンディングボックスを計算
            let min_x = p1[0].min(p2[0]).min(p3[0]);
            let max_x = p1[0].max(p2[0]).max(p3[0]);
            let min_y = p1[1].min(p2[1]).min(p3[1]);
            let max_y = p1[1].max(p2[1]).max(p3[1]);
            
            if intersects([min_x, min_y], max_x - min_x, max_y - min_y) {
                draw_list.push(DrawCommand::Triangle {
                    p1: *p1,
                    p2: *p2,
                    p3: *p3,
                    color: *color,
                    scroll: *scroll,
                    depth: *depth,
                });
            }
        }
        Stencil::Text {
            content,
            position,
            size,
            color,
            font,
            max_width,
            scroll,
            depth,
        } => {
            // テキストの概算サイズでチェック
            let text_height = size * 1.2;
            let text_width = max_width.unwrap_or(size * content.len() as f32 * 0.6);
            
            if intersects(*position, text_width, text_height) {
                draw_list.push(DrawCommand::Text {
                    content: content.clone(),
                    position: *position,
                    size: *size,
                    color: *color,
                    font: font.clone(),
                    max_width: *max_width,
                    scroll: *scroll,
                    depth: *depth,
                });
            }
        }
        Stencil::Image {
            position,
            width,
            height,
            path,
            scroll,
            depth,
        } => {
            if intersects(*position, *width, *height) {
                draw_list.push(DrawCommand::Image {
                    position: *position,
                    width: *width,
                    height: *height,
                    path: path.clone(),
                    scroll: *scroll,
                    depth: *depth,
                });
            }
        }
        Stencil::RoundedRect {
            position,
            width,
            height,
            radius,
            color,
            scroll,
            depth,
        } => {
            if intersects(*position, *width, *height) {
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

                // RoundedRectの各パーツを描画
                draw_list.push(DrawCommand::Rect {
                    position: [x + r, y + r],
                    width: w - 2.0 * r,
                    height: h - 2.0 * r,
                    color: *color,
                    scroll: *scroll,
                    depth: *depth,
                });

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

                // 角の円
                draw_list.push(DrawCommand::Circle {
                    center: [x + r, y + r],
                    radius: r,
                    color: *color,
                    segments: 32,
                    scroll: *scroll,
                    depth: *depth,
                });

                draw_list.push(DrawCommand::Circle {
                    center: [x + w - r, y + r],
                    radius: r,
                    color: *color,
                    segments: 32,
                    scroll: *scroll,
                    depth: *depth,
                });

                draw_list.push(DrawCommand::Circle {
                    center: [x + r, y + h - r],
                    radius: r,
                    color: *color,
                    segments: 32,
                    scroll: *scroll,
                    depth: *depth,
                });

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
        Stencil::Group(children) => {
            for child in children {
                recurse_with_clipping(child, draw_list, clip_pos, clip_width, clip_height);
            }
        }
        Stencil::ScrollContainer {
            position,
            width,
            height,
            overflow_mode,
            children,
            ..
        } => {
            // ネストされたScrollContainer: 新しいクリッピング領域を使用
            use crate::parser::ast::OverflowMode;
            
            match overflow_mode {
                OverflowMode::Visible => {
                    for child in children {
                        recurse_with_clipping(child, draw_list, clip_pos, clip_width, clip_height);
                    }
                }
                OverflowMode::Hidden | OverflowMode::Scroll | OverflowMode::Auto => {
                    // 新しいクリッピング領域を計算（親のクリッピング領域と交差）
                    let new_clip_left = clip_pos[0].max(position[0]);
                    let new_clip_top = clip_pos[1].max(position[1]);
                    let new_clip_right = (clip_pos[0] + clip_width).min(position[0] + width);
                    let new_clip_bottom = (clip_pos[1] + clip_height).min(position[1] + height);
                    
                    if new_clip_right > new_clip_left && new_clip_bottom > new_clip_top {
                        let new_clip_width = new_clip_right - new_clip_left;
                        let new_clip_height = new_clip_bottom - new_clip_top;
                        
                        for child in children {
                            recurse_with_clipping(
                                child,
                                draw_list,
                                [new_clip_left, new_clip_top],
                                new_clip_width,
                                new_clip_height,
                            );
                        }
                    }
                }
            }
        }
        _ => {
            // その他のStencilタイプ（ScrollBarなど）は通常通り処理
        }
    }
}

#[allow(dead_code)]
// ★ f32の順序付けのためのWrapper
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct OrderedFloat(f32);

impl Eq for OrderedFloat {}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}
