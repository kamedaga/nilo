// ========================================
// ユーティリティ関数モジュール
// ========================================
//
// このモジュールは文字列処理、型変換などのヘルパー関数を提供します。

use crate::parser::ast::*;

/// 文字列のクォート記号を除去する関数
pub fn unquote(s: &str) -> String {
    let trimmed = s.trim();
    let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('「') && trimmed.ends_with('」'))
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    // エスケープシーケンスを処理
    process_escape_sequences(unquoted)
}

/// エスケープシーケンスを処理する関数
pub fn process_escape_sequences(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next_ch) = chars.next() {
                match next_ch {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    '\\' => result.push('\\'),
                    '"' => result.push('"'),
                    '\'' => result.push('\''),
                    _ => {
                        // 認識できないエスケープシーケンスはそのまま
                        result.push('\\');
                        result.push(next_ch);
                    }
                }
            } else {
                result.push('\\');
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// 式から色の値を生成する関数
pub fn color_from_expr(expr: &Expr) -> Option<ColorValue> {
    match expr {
        Expr::String(s) => {
            // HEX色文字列をパース
            if s.starts_with('#') {
                Some(ColorValue::Hex(s.clone()))
            } else {
                // 名前付き色の処理
                match s.to_lowercase().as_str() {
                    "red" => Some(ColorValue::Rgba([1.0, 0.0, 0.0, 1.0])),
                    "green" => Some(ColorValue::Rgba([0.0, 1.0, 0.0, 1.0])),
                    "blue" => Some(ColorValue::Rgba([0.0, 0.0, 1.0, 1.0])),
                    "white" => Some(ColorValue::Rgba([1.0, 1.0, 1.0, 1.0])),
                    "black" => Some(ColorValue::Rgba([0.0, 0.0, 0.0, 1.0])),
                    "transparent" => Some(ColorValue::Rgba([0.0, 0.0, 0.0, 0.0])),
                    _ => Some(ColorValue::Hex(s.clone())),
                }
            }
        }
        Expr::Array(vals) => {
            // RGBA配列をパース
            if vals.len() >= 3 {
                let r = if let Expr::Number(n) = &vals[0] {
                    *n
                } else {
                    0.0
                };
                let g = if let Expr::Number(n) = &vals[1] {
                    *n
                } else {
                    0.0
                };
                let b = if let Expr::Number(n) = &vals[2] {
                    *n
                } else {
                    0.0
                };
                let a = if vals.len() >= 4 {
                    if let Expr::Number(n) = &vals[3] {
                        *n
                    } else {
                        1.0
                    }
                } else {
                    1.0
                };
                Some(ColorValue::Rgba([r, g, b, a]))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 式からエッジ値を生成する関数
pub fn edges_from_expr(expr: &Expr) -> Option<Edges> {
    match expr {
        Expr::Number(n) => Some(Edges::all(*n)),
        Expr::Array(vals) => {
            if vals.len() == 2 {
                // [vertical, horizontal]
                let v = if let Expr::Number(n) = &vals[0] {
                    *n
                } else {
                    0.0
                };
                let h = if let Expr::Number(n) = &vals[1] {
                    *n
                } else {
                    0.0
                };
                Some(Edges::vh(v, h))
            } else if vals.len() == 4 {
                // [top, right, bottom, left]
                let top = if let Expr::Number(n) = &vals[0] {
                    *n
                } else {
                    0.0
                };
                let right = if let Expr::Number(n) = &vals[1] {
                    *n
                } else {
                    0.0
                };
                let bottom = if let Expr::Number(n) = &vals[2] {
                    *n
                } else {
                    0.0
                };
                let left = if let Expr::Number(n) = &vals[3] {
                    *n
                } else {
                    0.0
                };
                Some(Edges {
                    top,
                    right,
                    bottom,
                    left,
                })
            } else {
                None
            }
        }
        Expr::Object(kvs) => {
            let mut edges = Edges::default();
            for (k, v) in kvs {
                if let Expr::Number(n) = v {
                    match k.as_str() {
                        "top" => edges.top = *n,
                        "right" => edges.right = *n,
                        "bottom" => edges.bottom = *n,
                        "left" => edges.left = *n,
                        _ => {}
                    }
                }
            }
            Some(edges)
        }
        _ => None,
    }
}

/// 式からサイズを生成する関数
pub fn size_from_expr(expr: &Expr) -> Option<[f32; 2]> {
    match expr {
        Expr::Array(vals) => {
            if vals.len() >= 2 {
                let w = if let Expr::Number(n) = &vals[0] {
                    *n
                } else {
                    0.0
                };
                let h = if let Expr::Number(n) = &vals[1] {
                    *n
                } else {
                    0.0
                };
                Some([w, h])
            } else {
                None
            }
        }
        _ => None,
    }
}
