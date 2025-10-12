// ========================================
// 式パーサーモジュール
// ========================================
//
// このモジュールは各種式（算術式、比較式、関数呼び出しなど）の解析を担当します。

use pest::iterators::Pair;
use crate::parser::ast::*;
use crate::parser::utils::unquote;
use crate::parser::parse::{Rule, NiloParser};

/// イベント式を解析する関数
pub fn parse_event_expr(pair: Pair<Rule>) -> EventExpr {
    let mut inner = pair.into_inner();
    let user_event = inner.next().expect("event_exprにuser_eventがありません");
    let mut ev_inner = user_event.into_inner();
    let kind = ev_inner.next().expect("user_eventにevent_kindがありません").as_str();
    let target = ev_inner.next().expect("user_eventにidentがありません").as_str().to_string();
    match kind {
        "click" => EventExpr::ButtonPressed(target),
        _ => panic!("不明なevent_kind: {:?}", kind),
    }
}

/// 計算式（calc_expr）をパースする
/// 例: (100% - 10px) -> CalcExpr(BinaryOp { left: Dimension(100%), op: Sub, right: Dimension(10px) })
pub fn parse_calc_expr(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut left = parse_calc_term(inner.next().unwrap());
    
    while let Some(op_pair) = inner.next() {
        let op = op_pair.as_str();
        let right = parse_calc_term(inner.next().unwrap());
        
        left = match op {
            "+" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Add,
                right: Box::new(right),
            },
            "-" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Sub,
                right: Box::new(right),
            },
            "*" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Mul,
                right: Box::new(right),
            },
            "/" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Div,
                right: Box::new(right),
            },
            _ => panic!("不明な計算演算子: {}", op),
        };
    }
    
    // 計算式全体をCalcExprでラップして返す
    Expr::CalcExpr(Box::new(left))
}

/// 計算式内の項（数値と単位）をパースする
pub fn parse_calc_term(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let number_pair = inner.next().unwrap();
    let value: f32 = number_pair.as_str().parse().unwrap();
    
    // 単位があるかチェック
    if let Some(unit_pair) = inner.next() {
        let unit_str = unit_pair.as_str();
        let unit = match unit_str {
            "px" => Unit::Px,
            "vw" => Unit::Vw,
            "vh" => Unit::Vh,
            "ww" => Unit::Ww,
            "wh" => Unit::Wh,
            "%" => Unit::Percent,
            "rem" => Unit::Rem,
            "em" => Unit::Em,
            _ => Unit::Px,
        };
        Expr::Dimension(DimensionValue { value, unit })
    } else {
        Expr::Number(value)
    }
}

/// 式をパースする
pub fn parse_expr(pair: Pair<Rule>) -> Expr {
    match pair.as_rule() {
        Rule::arg_item => {
            let inner = pair.into_inner().next().expect("arg_item");
            return parse_expr(inner);
        }
        Rule::expr => {
            // exprの中身を直接処理
            let inner = pair.into_inner().next().unwrap();
            parse_expr(inner)
        }

        Rule::string => Expr::String(unquote(pair.as_str())),
        Rule::number => {
            let v: f32 = pair.as_str().parse().unwrap();
            Expr::Number(v)
        }
        Rule::dimension_value => {
            let mut inner = pair.into_inner();
            let first_token = inner.next().unwrap();
            
            match first_token.as_rule() {
                Rule::auto_keyword => {
                    // "auto"キーワードが指定された場合
                    Expr::Dimension(DimensionValue { value: 0.0, unit: Unit::Auto })
                }
                Rule::calc_expr => {
                    // 計算式が指定された場合
                    parse_calc_expr(first_token)
                }
                Rule::number => {
                    // 数値が指定された場合
                    let value: f32 = first_token.as_str().parse().unwrap();

                    // unit_suffixがあるかチェック
                    if let Some(unit_pair) = inner.next() {
                        let unit_str = unit_pair.as_str();
                        let unit = match unit_str {
                            "px" => Unit::Px,
                            "vw" => {
                                log::debug!("🔍 PARSER DEBUG: Found {}vw in parsing", value);
                                Unit::Vw
                            },
                            "vh" => {
                                log::debug!("🔍 PARSER DEBUG: Found {}vh in parsing", value);
                                Unit::Vh
                            },
                            "ww" => {
                                log::debug!("🔍 PARSER DEBUG: Found {}ww in parsing", value);
                                Unit::Ww
                            },
                            "wh" => {
                                log::debug!("🔍 PARSER DEBUG: Found {}wh in parsing", value);
                                Unit::Wh
                            },
                            "%" => Unit::Percent,
                            "rem" => Unit::Rem,
                            "em" => Unit::Em,
                            _ => Unit::Px, // デフォルト
                        };
                        let result = Expr::Dimension(DimensionValue { value, unit });
                        log::debug!("🔍 PARSER DEBUG: Created DimensionValue: {:?}", result);
                        result
                    } else {
                        // ★ 修正: 単位がない場合は純粋な数値として扱う（pxに変換しない）
                        Expr::Number(value)
                    }
                }
                _ => {
                    panic!("Unexpected token in dimension_value: {:?}", first_token.as_rule());
                }
            }
        }
        Rule::bool => Expr::Bool(pair.as_str() == "true"),
        Rule::ident => Expr::Ident(pair.as_str().to_string()),
        Rule::path => Expr::Path(pair.as_str().to_string()),
        Rule::array => {
            let xs = pair.into_inner().map(parse_expr).collect();
            Expr::Array(xs)
        }
        Rule::object => {
            let mut kvs = Vec::new();
            for kv in pair.into_inner() {
                let mut it = kv.into_inner();
                let k_pair = it.next().unwrap();
                
                // キーは識別子または文字列
                let k = match k_pair.as_rule() {
                    Rule::ident => k_pair.as_str().to_string(),
                    Rule::string => unquote(k_pair.as_str()),
                    _ => k_pair.as_str().to_string(),
                };
                
                let v = parse_expr(it.next().unwrap());
                kvs.push((k, v));
            }
            Expr::Object(kvs)
        }
        Rule::match_expr => {
            let mut inner = pair.into_inner();
            let expr = Box::new(parse_expr(inner.next().unwrap()));

            let mut arms = Vec::new();
            let mut default = None;

            for arm_pair in inner {
                match arm_pair.as_rule() {
                    Rule::expr_match_arm => {
                        let mut arm_inner = arm_pair.into_inner();
                        let pattern = parse_expr(arm_inner.next().unwrap());
                        let value = parse_expr(arm_inner.next().unwrap());
                        arms.push(MatchArm { pattern, value });
                    }
                    Rule::expr_default_arm => {
                        let mut default_inner = arm_pair.into_inner();
                        let default_value = parse_expr(default_inner.next().unwrap());
                        default = Some(Box::new(default_value));
                    }
                    _ => {}
                }
            }

            Expr::Match { expr, arms, default }
        }
        _ => {
            // 比較式として解析を試行
            parse_comparison_expr(pair)
        }
    }
}

/// 比較式をパースする
pub fn parse_comparison_expr(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut left = parse_arithmetic_expr_direct(inner.next().unwrap());

    while let Some(op_pair) = inner.next() {
        let op = op_pair.as_str();
        let right = parse_arithmetic_expr_direct(inner.next().unwrap());

        left = match op {
            "==" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Eq,
                right: Box::new(right),
            },
            "!=" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Ne,
                right: Box::new(right),
            },
            "<" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Lt,
                right: Box::new(right),
            },
            "<=" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Le,
                right: Box::new(right),
            },
            ">" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Gt,
                right: Box::new(right),
            },
            ">=" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Ge,
                right: Box::new(right),
            },
            _ => panic!("不明な比較演算子: {}", op),
        };
    }

    left
}

/// 算術式を直接パースする
pub fn parse_arithmetic_expr_direct(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut left = parse_term(inner.next().unwrap());

    while let Some(op_pair) = inner.next() {
        let op = op_pair.as_str();
        let right = parse_term(inner.next().unwrap());

        left = match op {
            "+" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Add,
                right: Box::new(right),
            },
            "-" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Sub,
                right: Box::new(right),
            },
            _ => panic!("不明な算術演算: {}", op),
        };
    }

    left
}

/// 項をパースする
pub fn parse_term(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut left = parse_factor(inner.next().unwrap());

    while let Some(op_pair) = inner.next() {
        let op = op_pair.as_str();
        let right = parse_factor(inner.next().unwrap());

        left = match op {
            "*" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Mul,
                right: Box::new(right),
            },
            "/" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Div,
                right: Box::new(right),
            },
            _ => panic!("不明な乗除演算子: {}", op),
        };
    }

    left
}

/// 因子をパースする
pub fn parse_factor(pair: Pair<Rule>) -> Expr {
    parse_primary(pair.into_inner().next().unwrap())
}

/// プライマリをパースする
pub fn parse_primary(pair: Pair<Rule>) -> Expr {
    match pair.as_rule() {
        Rule::match_expr => {
            let mut inner = pair.into_inner();
            let expr = Box::new(parse_expr(inner.next().unwrap()));

            let mut arms = Vec::new();
            let mut default = None;

            for arm_pair in inner {
                match arm_pair.as_rule() {
                    Rule::expr_match_arm => {
                        let mut arm_inner = arm_pair.into_inner();
                        let pattern = parse_expr(arm_inner.next().unwrap());
                        let value = parse_expr(arm_inner.next().unwrap());
                        arms.push(MatchArm { pattern, value });
                    }
                    Rule::expr_default_arm => {
                        let mut default_inner = arm_pair.into_inner();
                        let default_value = parse_expr(default_inner.next().unwrap());
                        default = Some(Box::new(default_value));
                    }
                    _ => {}
                }
            }

            Expr::Match { expr, arms, default }
        }
        Rule::string => Expr::String(unquote(pair.as_str())),
        Rule::dimension_value => {
            let mut inner = pair.into_inner();
            let first_token = inner.next().unwrap();
            
            match first_token.as_rule() {
                Rule::auto_keyword => {
                    // "auto"キーワードが指定された場合
                    Expr::Dimension(DimensionValue { value: 0.0, unit: Unit::Auto })
                }
                Rule::calc_expr => {
                    // 計算式が指定された場合
                    parse_calc_expr(first_token)
                }
                Rule::number => {
                    let value: f32 = first_token.as_str().parse().unwrap();

                    if let Some(unit_pair) = inner.next() {
                        let unit_str = unit_pair.as_str();
                        let unit = match unit_str {
                            "px" => Unit::Px,
                            "vw" => Unit::Vw,
                            "vh" => Unit::Vh,
                            "ww" => Unit::Ww,
                            "wh" => Unit::Wh,
                            "%" => Unit::Percent,
                            "rem" => Unit::Rem,
                            "em" => Unit::Em,
                            _ => Unit::Px,
                        };
                        Expr::Dimension(DimensionValue { value, unit })
                    } else {
                        Expr::Number(value)
                    }
                }
                _ => {
                    panic!("Unexpected token in dimension_value: {:?}", first_token.as_rule());
                }
            }
        }
        Rule::number => {
            let v: f32 = pair.as_str().parse().unwrap();
            Expr::Number(v)
        }
        Rule::bool => Expr::Bool(pair.as_str() == "true"),
        Rule::path => Expr::Path(pair.as_str().to_string()),
        Rule::ident => Expr::Ident(pair.as_str().to_string()),
        // ★ 通常の関数呼び出し（onclick用）
        Rule::function_call => {
            let mut inner = pair.into_inner();
            let name = inner.next().unwrap().as_str().to_string();
            let args = inner.map(parse_expr).collect();
            Expr::FunctionCall { name, args }
        }
        // ★ Phase 2: has_slot() チェック
        Rule::has_slot_check => {
            let slot_name = pair.into_inner().next().unwrap().as_str().to_string();
            Expr::FunctionCall {
                name: "has_slot".to_string(),
                args: vec![Expr::String(slot_name)],
            }
        }
        Rule::array => {
            let xs = pair.into_inner().map(parse_expr).collect();
            Expr::Array(xs)
        }
        Rule::object => {
            let mut kvs = Vec::new();
            for kv in pair.into_inner() {
                let mut it = kv.into_inner();
                let k = it.next().unwrap().as_str().to_string();
                let v = parse_expr(it.next().unwrap());
                kvs.push((k, v));
            }
            Expr::Object(kvs)
        }
        Rule::expr => parse_expr(pair),
        _ => {
            // デフォルトケース：知らないルールを適切に処理
            // 子要素があれば再帰的に処理、なければ文字列として扱う
            let text = pair.as_str().to_string(); // 先に文字列を取得
            let mut inner = pair.into_inner();
            if let Some(child) = inner.next() {
                parse_primary(child)
            } else {
                // 子要素がない場合は文字列として扱う
                Expr::String(text)
            }
        }
    }
}

/// 条件文字列をパースする
pub fn parse_condition_string(condition: &str) -> Option<Expr> {
    use pest::Parser;
    
    // まずpest parserで解析を試みる
    if let Ok(mut pairs) = NiloParser::parse(Rule::expr, condition) {
        if let Some(pair) = pairs.next() {
            return Some(parse_expr(pair));
        }
    }
    
    // 失敗した場合、単純な文字列として解釈する
    Some(Expr::String(condition.to_string()))
}
