use crate::stencil::stencil::Stencil;

pub fn filter_visible_stencils(
    all: &[Stencil],
    scroll_offset: [f32; 2],
    viewport_height: f32,
) -> Vec<Stencil> {
    let mut result = Vec::new();
    for s in all {
        let (y, h) = match s {
            Stencil::Rect {
                position,
                height,
                scroll,
                ..
            } => {
                let y = if *scroll {
                    position[1] + scroll_offset[1]
                } else {
                    position[1]
                };
                (y, *height)
            }
            Stencil::Text {
                position,
                size,
                scroll,
                ..
            } => {
                let y = if *scroll {
                    position[1] + scroll_offset[1]
                } else {
                    position[1]
                };
                (y, *size)
            }
            Stencil::Circle {
                center,
                radius,
                scroll,
                ..
            } => {
                let y = if *scroll {
                    center[1] - *radius + scroll_offset[1]
                } else {
                    center[1] - *radius
                };
                (y, *radius * 2.0)
            }
            Stencil::Triangle {
                p1, p2, p3, scroll, ..
            } => {
                let top = p1[1].min(p2[1]).min(p3[1]);
                let bottom = p1[1].max(p2[1]).max(p3[1]);
                let y = if *scroll { top + scroll_offset[1] } else { top };
                (y, bottom - top)
            }
            Stencil::Image {
                position,
                height,
                scroll,
                ..
            } => {
                let y = if *scroll {
                    position[1] + scroll_offset[1]
                } else {
                    position[1]
                };
                (y, *height)
            }
            Stencil::RoundedRect {
                position,
                height,
                scroll,
                ..
            } => {
                let y = if *scroll {
                    position[1] + scroll_offset[1]
                } else {
                    position[1]
                };
                (y, *height)
            }
            Stencil::Group(_) | Stencil::ScrollBar { .. } => {
                result.push(s.clone());
                continue;
            }
        };

        if y + h >= 0.0 && y <= viewport_height {
            result.push(s.clone());
        }
    }
    result
}

pub fn inject_scrollbar(
    mut visible: Vec<Stencil>,
    content_length: f32,
    viewport_height: f32,
    viewport_width: f32,
    scroll_offset: f32,
) -> Vec<Stencil> {
    visible.push(Stencil::ScrollBar {
        content_length,
        viewport_height,
        scroll_offset_y: scroll_offset,
        viewport_width,
        depth: 0.2, // ★ スクロールバーは前面に表示
    });
    visible
}
