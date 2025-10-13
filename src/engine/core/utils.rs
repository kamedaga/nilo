// src/engine/engine/utils.rs
// ユーティリティ関数

use crate::parser::ast::ColorValue;
use crate::stencil::stencil::Stencil;

/// ユーティリティ関数群
#[inline]
pub fn is_point_in_rect(point: [f32; 2], pos: [f32; 2], size: [f32; 2]) -> bool {
    point[0] >= pos[0]
        && point[0] <= pos[0] + size[0]
        && point[1] >= pos[1]
        && point[1] <= pos[1] + size[1]
}

#[inline]
pub fn format_text_fast(fmt: &str, args: &[String]) -> String {
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
pub fn convert_to_rgba(c: &ColorValue) -> [f32; 4] {
    match c {
        ColorValue::Rgba(v) => *v,
        ColorValue::Hex(s) => hex_to_rgba_fast(s),
    }
}

#[inline]
pub fn hex_to_rgba_fast(s: &str) -> [f32; 4] {
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
pub fn offset_stencil_fast(stencil: &Stencil, dx: f32, dy: f32) -> Stencil {
    let mut result = stencil.clone();
    match &mut result {
        Stencil::Rect { position, .. }
        | Stencil::RoundedRect { position, .. }
        | Stencil::Text { position, .. }
        | Stencil::Image { position, .. } => {
            position[0] += dx;
            position[1] += dy;
        }
        Stencil::Circle { center, .. } => {
            center[0] += dx;
            center[1] += dy;
        }
        Stencil::Triangle { p1, p2, p3, .. } => {
            p1[0] += dx;
            p1[1] += dy;
            p2[0] += dx;
            p2[1] += dy;
            p3[0] += dx;
            p3[1] += dy;
        }
        _ => {}
    }
    result
}

#[inline]
pub fn adjust_stencil_depth(stencil: &mut Stencil, depth_counter: &mut f32) {
    *depth_counter += 0.001;
    let depth = (1.0 - *depth_counter).max(0.0);

    match stencil {
        Stencil::Rect { depth: d, .. }
        | Stencil::RoundedRect { depth: d, .. }
        | Stencil::Text { depth: d, .. }
        | Stencil::Circle { depth: d, .. }
        | Stencil::Triangle { depth: d, .. }
        | Stencil::Image { depth: d, .. } => {
            *d = depth;
        }
        _ => {}
    }
}
