// ========================================
// スタイルパーサーモジュール
// ========================================
//
// このモジュールはスタイル式からStyleオブジェクトへの変換を担当します。

use crate::parser::ast::*;
use crate::parser::expr::parse_condition_string;
use crate::parser::utils::{color_from_expr, edges_from_expr, size_from_expr};

/// 計算式を静的評価する
pub fn eval_calc_expr(expr: &Expr) -> Option<DimensionValue> {
    match expr {
        Expr::CalcExpr(inner) => {
            // CalcExpr内の式を評価
            eval_calc_expr(inner)
        }
        Expr::Dimension(d) => {
            // そのままDimensionValueを返す
            Some(*d)
        }
        Expr::Number(n) => {
            // 数値の場合はpx単位として扱う
            Some(DimensionValue {
                value: *n,
                unit: Unit::Px,
            })
        }
        Expr::BinaryOp { left, op, right } => {
            // 二項演算の場合、左辺と右辺を評価
            let left_dim = eval_calc_expr(left)?;
            let right_dim = eval_calc_expr(right)?;

            // 同じ単位の場合のみ計算を実行
            if left_dim.unit == right_dim.unit {
                let result_value = match op {
                    BinaryOperator::Add => left_dim.value + right_dim.value,
                    BinaryOperator::Sub => left_dim.value - right_dim.value,
                    BinaryOperator::Mul => left_dim.value * right_dim.value,
                    BinaryOperator::Div => {
                        if right_dim.value != 0.0 {
                            left_dim.value / right_dim.value
                        } else {
                            0.0
                        }
                    }
                    _ => return None,
                };
                Some(DimensionValue {
                    value: result_value,
                    unit: left_dim.unit,
                })
            } else {
                // 異なる単位の場合は、実行時評価のためNoneを返す
                // レイアウトエンジンで実行時に評価する
                log::debug!(
                    "🔧 計算式で異なる単位が使用されています: {:?} と {:?} - 実行時に評価します",
                    left_dim.unit,
                    right_dim.unit
                );
                None
            }
        }
        _ => None,
    }
}

/// 式からスタイルを生成する
pub fn style_from_expr(expr: Expr) -> Style {
    match expr {
        Expr::Object(kvs) => {
            let mut s = Style::default();

            for (k, v) in kvs {
                // ★ 新規追加: match式を含むプロパティを特別処理
                let resolved_value = match &v {
                    Expr::Match { .. } => {
                        // match式は実行時に評価されるため、ここでは既存の値を設定
                        // 実際の評価はAppState::eval_expr_from_astで行われる
                        v.clone()
                    }
                    _ => v.clone(),
                };

                // ★ レスポンシブ対応: window.width や window.height を含む条件をチェック
                // 簡易実装: キーが "window.width <= 1000" のようなパターンの場合
                if (k.contains("window.width") || k.contains("window.height"))
                    && (k.contains("<=")
                        || k.contains(">=")
                        || k.contains("<")
                        || k.contains(">")
                        || k.contains("=="))
                {
                    eprintln!("🔍 [PARSE] レスポンシブ条件を検出: {}", k);

                    // 条件式を解析してResponsiveRuleを作成
                    if let Some(condition_expr) = parse_condition_string(&k) {
                        eprintln!("   [PARSE] 条件式パース成功: {:?}", condition_expr);

                        if let Expr::Object(_) = &resolved_value {
                            let conditional_style = style_from_expr(resolved_value);
                            eprintln!("   [PARSE] 条件付きスタイルを追加");
                            s.responsive_rules.push(crate::parser::ast::ResponsiveRule {
                                condition: condition_expr,
                                style: Box::new(conditional_style),
                            });
                            continue;
                        } else {
                            eprintln!(
                                "   [PARSE] ⚠️ 条件の値がオブジェクトではありません: {:?}",
                                resolved_value
                            );
                        }
                    } else {
                        eprintln!("   [PARSE] ⚠️ 条件式のパースに失敗: {}", k);
                    }
                }

                match k.as_str() {
                    "color" => s.color = color_from_expr(&resolved_value),
                    "background" => s.background = color_from_expr(&resolved_value),
                    "border_color" => s.border_color = color_from_expr(&resolved_value),
                    "padding" => s.padding = edges_from_expr(&resolved_value),
                    "margin" => s.margin = edges_from_expr(&resolved_value),
                    "size" => s.size = size_from_expr(&resolved_value),

                    // 実際のStyleフィールドに合わせて修正
                    "width" => {
                        if let Some(Expr::Number(w)) = Some(&resolved_value) {
                            s.width = Some(*w);
                        } else if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.relative_width = Some(*d);
                        } else if let Some(Expr::CalcExpr(_)) = Some(&resolved_value) {
                            // CalcExprの場合は、まず静的評価を試みる
                            if let Some(d) = eval_calc_expr(&resolved_value) {
                                s.relative_width = Some(d);
                            } else {
                                // 静的評価に失敗した場合（異なる単位の計算式など）、
                                // 計算式を保存して実行時に評価
                                s.width_expr = Some(resolved_value.clone());
                            }
                        }
                    }
                    "height" => {
                        if let Some(Expr::Number(h)) = Some(&resolved_value) {
                            s.height = Some(*h);
                        } else if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.relative_height = Some(*d);
                        } else if let Some(Expr::CalcExpr(_)) = Some(&resolved_value) {
                            // CalcExprの場合は、まず静的評価を試みる
                            if let Some(d) = eval_calc_expr(&resolved_value) {
                                s.relative_height = Some(d);
                            } else {
                                // 静的評価に失敗した場合（異なる単位の計算式など）、
                                // 計算式を保存して実行時に評価
                                s.height_expr = Some(resolved_value.clone());
                            }
                        }
                    }
                    "font_size" => {
                        if let Some(Expr::Number(fs)) = Some(&resolved_value) {
                            s.font_size = Some(*fs);
                        } else if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.relative_font_size = Some(*d);
                        }
                    }
                    "font" => {
                        if let Some(Expr::String(f)) = Some(&resolved_value) {
                            s.font = Some(f.clone());
                        }
                    }
                    "spacing" => {
                        if let Some(Expr::Number(sp)) = Some(&resolved_value) {
                            s.spacing = Some(*sp);
                        } else if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.relative_spacing = Some(*d);
                        }
                    }
                    "gap" => {
                        // gap プロパティは spacing と同じ機能を提供
                        if let Some(Expr::Number(sp)) = Some(&resolved_value) {
                            s.spacing = Some(*sp);
                            s.gap = None; // Numberの場合はgapフィールドは使わない
                        } else if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.gap = Some(*d);
                            s.relative_spacing = Some(*d);
                        }
                    }
                    "card" => {
                        if let Some(Expr::Bool(c)) = Some(&resolved_value) {
                            s.card = Some(*c);
                        }
                    }
                    // ★ 新規追加: rounded プロパティの処理
                    "rounded" => match &resolved_value {
                        Expr::Number(r) => {
                            s.rounded = Some(Rounded::Px(*r));
                        }
                        Expr::Bool(true) => {
                            s.rounded = Some(Rounded::On);
                        }
                        Expr::Bool(false) => {
                            s.rounded = None;
                        }
                        Expr::Dimension(d) => {
                            s.rounded = Some(Rounded::Px(d.value));
                        }
                        _ => {}
                    },
                    // ★ 新規追加: max_width, min_width, min_height プロパティの処理
                    "max_width" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.max_width = Some(*d);
                        }
                    }
                    "min_width" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.min_width = Some(*d);
                        }
                    }
                    "max_height" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.max_height = Some(*d);
                        }
                    }
                    "min_height" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.min_height = Some(*d);
                        }
                    }
                    // マージン系プロパティ
                    "margin_top" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.margin_top = Some(*d);
                        }
                    }
                    "margin_bottom" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.margin_bottom = Some(*d);
                        }
                    }
                    "margin_left" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.margin_left = Some(*d);
                        }
                    }
                    "margin_right" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.margin_right = Some(*d);
                        }
                    }
                    // その他のプロパティ
                    "line_height" => {
                        if let Some(Expr::Number(lh)) = Some(&resolved_value) {
                            s.line_height = Some(*lh);
                        }
                    }
                    "font_weight" => {
                        if let Some(Expr::String(fw)) = Some(&resolved_value) {
                            s.font_weight = Some(fw.clone());
                        }
                    }
                    "font_family" => {
                        if let Some(Expr::String(ff)) = Some(&resolved_value) {
                            s.font_family = Some(ff.clone());
                        }
                    }
                    "wrap" => {
                        if let Some(Expr::String(w)) = Some(&resolved_value) {
                            s.wrap = match w.to_lowercase().as_str() {
                                "auto" => Some(WrapMode::Auto),
                                "none" => Some(WrapMode::None),
                                _ => Some(WrapMode::Auto), // デフォルトはAuto
                            };
                        }
                    }
                    "align" => {
                        if let Some(Expr::String(a)) = Some(&resolved_value) {
                            s.align = match a.to_lowercase().as_str() {
                                "left" => Some(Align::Left),
                                "center" => Some(Align::Center),
                                "right" => Some(Align::Right),
                                "top" => Some(Align::Top),
                                "bottom" => Some(Align::Bottom),
                                _ => None,
                            };
                        }
                    }
                    "overflow" => {
                        if let Some(Expr::String(o)) = Some(&resolved_value) {
                            s.overflow = match o.to_lowercase().as_str() {
                                "visible" => Some(OverflowMode::Visible),
                                "hidden" => Some(OverflowMode::Hidden),
                                "scroll" => Some(OverflowMode::Scroll),
                                "auto" => Some(OverflowMode::Auto),
                                _ => None,
                            };
                        }
                    }
                    _ => {
                        // 未知のプロパティは無視
                    }
                }
            }
            s
        }
        _ => Style::default(),
    }
}
