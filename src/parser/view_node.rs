// ========================================
// ビューノードパーサーモジュール
// ========================================
//
// このモジュールは各種ビューノード（Text, Button, Image等）の解析を担当します。

use crate::parser::ast::*;
use crate::parser::expr::parse_expr;
use crate::parser::parse::Rule;
use crate::parser::style::style_from_expr;
use crate::parser::types::{infer_expr_type, parse_type_expr};
use crate::parser::utils::unquote;
use crate::stencil::stencil::Stencil;
use pest::iterators::Pair;

/// ビューノードをパースする（メイン関数）
pub fn parse_view_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    match pair.as_rule() {
        Rule::stencil_call => WithSpan {
            node: ViewNode::Stencil(parse_stencil_call(pair)),
            line,
            column: col,
            style: None,
        },
        Rule::text => parse_text(pair),
        Rule::button => parse_button(pair),
        Rule::text_input => parse_text_input(pair),
        Rule::image => parse_image(pair),
        Rule::vstack_node => parse_vstack_node(pair),
        Rule::hstack_node => parse_hstack_node(pair),
        Rule::rust_call => parse_rust_call(pair),
        Rule::component_call => parse_component_call(pair),
        Rule::slot_node => parse_slot_node(pair),
        Rule::dynamic_section => parse_dynamic_section(pair),
        Rule::match_block => parse_match_block(pair),
        Rule::navigate_action => parse_navigate_action(pair),
        Rule::spacing_node => {
            let span = pair.as_span();
            let (line, col) = span.start_pos().line_col();

            let text = pair.as_str();
            let node = if text == "SpacingAuto" {
                ViewNode::SpacingAuto
            } else {
                // "Spacing(...)" の場合は値を解析
                let mut it = pair.into_inner();
                if let Some(p) = it.next() {
                    let dimension_value = match p.as_rule() {
                        Rule::dimension_value => {
                            let expr = parse_expr(p);
                            match expr {
                                Expr::Dimension(dim_val) => dim_val,
                                Expr::Number(n) => DimensionValue {
                                    value: n,
                                    unit: Unit::Px,
                                },
                                _ => DimensionValue {
                                    value: 12.0,
                                    unit: Unit::Px,
                                },
                            }
                        }
                        Rule::number => {
                            let v = p.as_str().parse::<f32>().unwrap_or(12.0);
                            DimensionValue {
                                value: v,
                                unit: Unit::Px,
                            }
                        }
                        _ => DimensionValue {
                            value: 12.0,
                            unit: Unit::Px,
                        },
                    };
                    ViewNode::Spacing(dimension_value)
                } else {
                    ViewNode::SpacingAuto
                }
            };
            WithSpan {
                node,
                line,
                column: col,
                style: None,
            }
        }
        // 状態操作関連のノード
        Rule::state_set => parse_state_set(pair),
        Rule::list_append => parse_list_append(pair),
        Rule::list_insert => parse_list_insert(pair),
        Rule::list_remove => parse_list_remove(pair),
        Rule::list_clear => parse_list_clear(pair),
        Rule::state_toggle => parse_state_toggle(pair),
        Rule::let_decl => parse_let_decl(pair),
        Rule::const_decl => parse_const_decl(pair),
        Rule::foreach_node => parse_foreach_node(pair),
        Rule::if_node => parse_if_node(pair),
        Rule::when_block => {
            // when_blockは表示ノードではないため、ダミーのテキストノードとして処理
            WithSpan {
                node: ViewNode::Text {
                    format: "".to_string(),
                    args: vec![],
                },
                line,
                column: col,
                style: None,
            }
        }
        Rule::font_def => {
            // font定義は表示ノードではないため、ダミーのテキストノードとして処理
            WithSpan {
                node: ViewNode::Text {
                    format: "".to_string(),
                    args: vec![],
                },
                line,
                column: col,
                style: None,
            }
        }
        _ => unreachable!("不明なview_node: {:?}", pair),
    }
}

// ========================================
// 個別ビューノード解析関数群
// ========================================

/// スロットノードをパース (slot content)
pub fn parse_slot_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let name = pair.into_inner().next().unwrap().as_str().to_string();

    WithSpan {
        node: ViewNode::Slot { name },
        line,
        column: col,
        style: None,
    }
}

/// テキストノードの解析
/// 形式: Text("format_string", arg1, arg2, ..., [style: {...}])
pub fn parse_text(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut it = pair.into_inner();
    let format = unquote(it.next().unwrap().as_str());

    let mut args: Vec<Expr> = Vec::new();
    let mut style: Option<Style> = None;

    for p in it {
        match p.as_rule() {
            Rule::arg_item => {
                let inner = p.into_inner().next().unwrap();
                match inner.as_rule() {
                    Rule::expr => args.push(parse_expr(inner)),
                    Rule::style_arg => {
                        let expr = parse_expr(inner.into_inner().next().unwrap());
                        style = Some(style_from_expr(expr));
                    }
                    _ => {}
                }
            }
            Rule::expr => args.push(parse_expr(p)),
            Rule::style_arg => {
                let expr = parse_expr(p.into_inner().next().unwrap());
                style = Some(style_from_expr(expr));
            }
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::Text { format, args },
        line,
        column: col,
        style,
    }
}

/// ボタンノードの解析
/// 形式: Button(id: "button_id", label: "Button Label", [onclick: function!()], [style: {...}])
pub fn parse_button(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut id: Option<String> = None;
    let mut label: Option<String> = None;
    let mut onclick: Option<Expr> = None;
    let mut style: Option<Style> = None;

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::ident if id.is_none() => {
                id = Some(p.as_str().to_string());
            }
            Rule::string => {
                if id.is_none() {
                    id = Some(unquote(p.as_str()));
                } else if label.is_none() {
                    label = Some(unquote(p.as_str()));
                }
            }
            Rule::rust_call => {
                let mut inner = p.into_inner();
                let name = inner.next().unwrap().as_str().to_string();
                let mut args: Vec<Expr> = Vec::new();
                for arg_p in inner {
                    match arg_p.as_rule() {
                        Rule::arg_item => {
                            let mut it = arg_p.into_inner();
                            if let Some(x) = it.next() {
                                if x.as_rule() == Rule::expr {
                                    args.push(parse_expr(x));
                                }
                            }
                        }
                        Rule::expr => args.push(parse_expr(arg_p)),
                        _ => {}
                    }
                }
                onclick = Some(Expr::FunctionCall { name, args });
            }
            Rule::expr if onclick.is_none() => {
                onclick = Some(parse_expr(p));
            }
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::arg_item => {
                let mut it = p.into_inner();
                if let Some(inner) = it.next() {
                    match inner.as_rule() {
                        Rule::style_arg => {
                            style = Some(style_from_expr(parse_expr(
                                inner.into_inner().next().unwrap(),
                            )));
                        }
                        Rule::expr => {
                            let expr = parse_expr(inner);
                            match expr {
                                Expr::String(s) => {
                                    if id.is_none() {
                                        id = Some(s);
                                    } else if label.is_none() {
                                        label = Some(s);
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    let id = id.expect("ボタンにはid:が必要です");
    let label = label.expect("ボタンにはlabel:が必要です");
    WithSpan {
        node: ViewNode::Button { id, label, onclick },
        line,
        column: col,
        style,
    }
}

/// 画像ノードの解析
/// 形式: Image("path", [style: { size: [w,h], ... }])
pub fn parse_image(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut path: Option<String> = None;
    let mut style: Option<Style> = None;

    for arg in pair.into_inner() {
        match arg.as_rule() {
            Rule::string => {
                path = Some(unquote(arg.as_str()));
            }
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(
                    arg.into_inner().next().unwrap(),
                )));
            }
            Rule::arg_item => {
                let mut it = arg.into_inner();
                if let Some(inner) = it.next() {
                    if inner.as_rule() == Rule::style_arg {
                        style = Some(style_from_expr(parse_expr(
                            inner.into_inner().next().unwrap(),
                        )));
                    }
                }
            }
            _ => {}
        }
    }

    let path = path.expect("画像にはパスが必要です");
    WithSpan {
        node: ViewNode::Image { path },
        line,
        column: col,
        style,
    }
}

/// VStackノードの解析
/// 形式: VStack([style: {...}]) { ... }
pub fn parse_vstack_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut style: Option<Style> = None;
    let mut children: Vec<WithSpan<ViewNode>> = Vec::new();

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::view_nodes => {
                children = p.into_inner().map(parse_view_node).collect();
            }
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::VStack(children),
        line,
        column: col,
        style,
    }
}

/// HStackノードの解析
/// 形式: HStack([style: {...}]) { ... }
pub fn parse_hstack_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut style: Option<Style> = None;
    let mut children: Vec<WithSpan<ViewNode>> = Vec::new();

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::view_nodes => {
                children = p.into_inner().map(parse_view_node).collect();
            }
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::HStack(children),
        line,
        column: col,
        style,
    }
}

/// コンポーネント呼び出しの解析
/// 形式: ComponentName(arg1, ..., [style: {...}])
pub fn parse_component_call(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();

    let mut args: Vec<ComponentArg> = Vec::new();
    let mut style: Option<Style> = None;

    for p in inner {
        match p.as_rule() {
            Rule::arg_item => {
                let mut it = p.into_inner();
                if let Some(x) = it.next() {
                    match x.as_rule() {
                        Rule::style_arg => {
                            let expr = parse_expr(x.into_inner().next().unwrap());
                            style = Some(style_from_expr(expr));
                        }
                        Rule::named_arg => {
                            // 名前付き引数: name: expr
                            let mut named_it = x.into_inner();
                            let param_name = named_it.next().unwrap().as_str().to_string();
                            let expr = parse_expr(named_it.next().unwrap());
                            args.push(ComponentArg::Named(param_name, expr));
                        }
                        Rule::expr => {
                            // 位置引数
                            args.push(ComponentArg::Positional(parse_expr(x)));
                        }
                        _ => {}
                    }
                }
            }
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::expr => {
                args.push(ComponentArg::Positional(parse_expr(p)));
            }
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::ComponentCall {
            name,
            args,
            slots: std::collections::HashMap::new(),
        },
        line,
        column: col,
        style,
    }
}

/// dynamic_section の解析
/// 形式: dynamic_section section_name ([style: {...}]) { ... }
pub fn parse_dynamic_section(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut name: Option<String> = None;
    let mut style: Option<Style> = None;
    let mut body: Vec<WithSpan<ViewNode>> = Vec::new();

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::ident => name = Some(p.as_str().to_string()),
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())))
            }
            Rule::view_nodes => body = p.into_inner().map(parse_view_node).collect(),
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::DynamicSection {
            name: name.unwrap(),
            body,
        },
        line,
        column: col,
        style,
    }
}

/// matchブロックの解析
/// 形式: match <expr> ([style: {...}]) { case value1 { ... } case value2 { ... } default { ... } }
pub fn parse_match_block(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut expr: Option<Expr> = None;
    let mut style: Option<Style> = None;
    let mut arms: Vec<(Expr, Vec<WithSpan<ViewNode>>)> = Vec::new();
    let mut default: Option<Vec<WithSpan<ViewNode>>> = None;

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::expr => expr = Some(parse_expr(p)),
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())))
            }
            Rule::match_arm => {
                let mut arm_inner = p.into_inner();
                let case_val = parse_expr(arm_inner.next().unwrap());
                let mut nodes = Vec::new();
                for n in arm_inner {
                    match n.as_rule() {
                        Rule::view_nodes => {
                            for vn in n.into_inner() {
                                nodes.push(parse_view_node(vn));
                            }
                        }
                        _ => nodes.push(parse_view_node(n)),
                    }
                }
                arms.push((case_val, nodes));
            }
            Rule::default_arm => {
                let mut nodes = Vec::new();
                for n in p.into_inner() {
                    match n.as_rule() {
                        Rule::view_nodes => {
                            for vn in n.into_inner() {
                                nodes.push(parse_view_node(vn));
                            }
                        }
                        _ => nodes.push(parse_view_node(n)),
                    }
                }
                default = Some(nodes);
            }
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::Match {
            expr: expr.unwrap(),
            arms,
            default,
        },
        line,
        column: col,
        style,
    }
}

/// navigate actionのパース
/// 形式: navigate_to(TargetState)
pub fn parse_navigate_action(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    let target = inner.next().unwrap().as_str().to_string();
    WithSpan {
        node: ViewNode::NavigateTo { target },
        line,
        column: col,
        style: None,
    }
}

// ========================================
// ステンシル解析
// ========================================

/// ステンシル呼び出しの解析
fn parse_stencil_call(pair: Pair<Rule>) -> Stencil {
    let mut inner = pair.into_inner();
    let kind = inner.next().unwrap().as_str();

    let mut map = std::collections::HashMap::new();

    if let Some(stencil_args) = inner.next() {
        for arg in stencil_args.into_inner() {
            let mut arg_inner = arg.into_inner();
            let key_pair = arg_inner.next().unwrap();
            let key = key_pair.as_str();

            let val_pair = arg_inner.next().unwrap();

            let value = if val_pair.as_rule() == Rule::stencil_value {
                let mut value_inner = val_pair.into_inner();
                if let Some(actual_value) = value_inner.next() {
                    match actual_value.as_rule() {
                        Rule::number => StencilArg::Number(actual_value.as_str().parse().unwrap()),
                        Rule::string => StencilArg::String(unquote(actual_value.as_str())),
                        Rule::bool => StencilArg::Bool(actual_value.as_str() == "true"),
                        Rule::ident => {
                            panic!(
                                "ステンシル引数は変数名は使用できません: key={}, value={}",
                                key,
                                actual_value.as_str()
                            );
                        }
                        _ => panic!("不明な引数タイプ"),
                    }
                } else {
                    panic!("key: {} stencil_valueの値が見つかりません", key);
                }
            } else {
                match val_pair.as_rule() {
                    Rule::number => StencilArg::Number(val_pair.as_str().parse().unwrap()),
                    Rule::string => StencilArg::String(unquote(val_pair.as_str())),
                    Rule::bool => StencilArg::Bool(val_pair.as_str() == "true"),
                    _ => panic!("不明な直接値タイプ"),
                }
            };

            map.insert(key.to_string(), value);
        }
    }

    macro_rules! get_f32 {
        ($k:expr, $def:expr) => {
            map.get($k).and_then(|v| v.as_f32()).unwrap_or($def)
        };
    }
    macro_rules! get_str {
        ($k:expr, $def:expr) => {
            map.get($k)
                .and_then(|v| v.as_str())
                .unwrap_or($def)
                .to_string()
        };
    }
    macro_rules! get_bool {
        ($k:expr, $def:expr) => {
            map.get($k).and_then(|v| v.as_bool()).unwrap_or($def)
        };
    }

    let parse_position_value = |key: &str, default: f32| -> f32 {
        map.get(key).and_then(|v| v.as_f32()).unwrap_or(default)
    };

    match kind {
        "rect" => Stencil::Rect {
            position: [
                parse_position_value("x", 0.0),
                parse_position_value("y", 0.0),
            ],
            width: get_f32!("width", 0.0),
            height: get_f32!("height", 0.0),
            color: [
                get_f32!("r", 1.0),
                get_f32!("g", 1.0),
                get_f32!("b", 1.0),
                get_f32!("a", 1.0),
            ],
            scroll: get_bool!("scroll", true),
            depth: get_f32!("depth", 0.5),
        },
        "circle" => Stencil::Circle {
            center: [
                parse_position_value("x", 0.0),
                parse_position_value("y", 0.0),
            ],
            radius: get_f32!("radius", 1.0),
            color: [
                get_f32!("r", 1.0),
                get_f32!("g", 1.0),
                get_f32!("b", 1.0),
                get_f32!("a", 1.0),
            ],
            scroll: get_bool!("scroll", true),
            depth: get_f32!("depth", 0.5),
        },
        "triangle" => Stencil::Triangle {
            p1: [
                parse_position_value("x1", 0.0),
                parse_position_value("y1", 0.0),
            ],
            p2: [
                parse_position_value("x2", 0.0),
                parse_position_value("y2", 0.0),
            ],
            p3: [
                parse_position_value("x3", 0.0),
                parse_position_value("y3", 0.0),
            ],
            color: [
                get_f32!("r", 1.0),
                get_f32!("g", 1.0),
                get_f32!("b", 1.0),
                get_f32!("a", 1.0),
            ],
            scroll: get_bool!("scroll", true),
            depth: get_f32!("depth", 0.5),
        },
        "text" => Stencil::Text {
            content: get_str!("content", ""),
            position: [
                parse_position_value("x", 0.0),
                parse_position_value("y", 0.0),
            ],
            size: get_f32!("size", 16.0),
            color: [
                get_f32!("r", 0.0),
                get_f32!("g", 0.0),
                get_f32!("b", 0.0),
                get_f32!("a", 1.0),
            ],
            font: get_str!("font", "sans"),
            max_width: None,
            scroll: get_bool!("scroll", true),
            depth: get_f32!("depth", 0.1),
        },
        "image" => Stencil::Image {
            position: [
                parse_position_value("x", 0.0),
                parse_position_value("y", 0.0),
            ],
            width: get_f32!("width", 0.0),
            height: get_f32!("height", 0.0),
            path: get_str!("path", ""),
            scroll: get_bool!("scroll", true),
            depth: get_f32!("depth", 0.5),
        },
        "rounded_rect" => Stencil::RoundedRect {
            position: [
                parse_position_value("x", 0.0),
                parse_position_value("y", 0.0),
            ],
            width: get_f32!("width", 0.0),
            height: get_f32!("height", 0.0),
            radius: get_f32!("radius", 8.0),
            color: [
                get_f32!("r", 1.0),
                get_f32!("g", 1.0),
                get_f32!("b", 1.0),
                get_f32!("a", 1.0),
            ],
            scroll: get_bool!("scroll", true),
            depth: get_f32!("depth", 0.5),
        },
        _ => panic!("不明なステンシルタイプ {}", kind),
    }
}

enum StencilArg {
    Number(f32),
    String(String),
    Bool(bool),
}

impl StencilArg {
    fn as_f32(&self) -> Option<f32> {
        match self {
            StencilArg::Number(f) => Some(*f),
            _ => None,
        }
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            StencilArg::String(s) => Some(s),
            _ => None,
        }
    }
    fn as_bool(&self) -> Option<bool> {
        match self {
            StencilArg::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

// ========================================
// 状態操作ノード
// ========================================

fn parse_state_set(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();

    let path = inner.next().unwrap().as_str().to_string();

    let mut declared_type: Option<NiloType> = None;
    let mut value_pair = None;

    for p in inner {
        match p.as_rule() {
            Rule::type_annotation => {
                let type_inner = p.into_inner().next().unwrap();
                declared_type = Some(parse_type_expr(type_inner));
            }
            Rule::expr => {
                value_pair = Some(p);
            }
            _ => {}
        }
    }

    let value = parse_expr(value_pair.expect("set文に値がありません"));
    let inferred_type = infer_expr_type(&value);

    if let Some(expected_type) = &declared_type {
        if !expected_type.is_compatible_with(&inferred_type) {
            eprintln!(
                "[Type Warning] {}:{} - 型の不一致: 変数 '{}' は {} 型ですが、{} 型の値が代入されました",
                line,
                col,
                path,
                expected_type.display(),
                inferred_type.display()
            );
        }
    }

    WithSpan {
        node: ViewNode::Set {
            path,
            value,
            inferred_type: Some(inferred_type),
        },
        line,
        column: col,
        style: None,
    }
}

fn parse_list_append(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    let path = inner.next().unwrap().as_str().to_string();
    let value = parse_expr(inner.next().unwrap());
    WithSpan {
        node: ViewNode::ListAppend { path, value },
        line,
        column: col,
        style: None,
    }
}

fn parse_list_insert(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    let path = inner.next().unwrap().as_str().to_string();
    let index = inner.next().unwrap().as_str().parse::<usize>().unwrap();
    let value = parse_expr(inner.next().unwrap());
    WithSpan {
        node: ViewNode::ListInsert { path, index, value },
        line,
        column: col,
        style: None,
    }
}

fn parse_list_remove(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    let path = inner.next().unwrap().as_str().to_string();
    let value = parse_expr(inner.next().unwrap());
    WithSpan {
        node: ViewNode::ListRemove { path, value },
        line,
        column: col,
        style: None,
    }
}

fn parse_list_clear(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    let path = inner.next().unwrap().as_str().to_string();
    WithSpan {
        node: ViewNode::ListClear { path },
        line,
        column: col,
        style: None,
    }
}

fn parse_state_toggle(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    let lhs = inner.next().unwrap().as_str().to_string();
    let rhs = inner.next().unwrap().as_str().to_string();
    if lhs != rhs {
        panic!(
            "toggle は `state.foo = !state.foo` の形式で同じパスに対して行ってください（lhs={}, rhs={}）",
            lhs, rhs
        );
    }
    WithSpan {
        node: ViewNode::Toggle { path: lhs },
        line,
        column: col,
        style: None,
    }
}

/// let宣言のパース
fn parse_let_decl(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();
    let mut declared_type: Option<NiloType> = None;
    let mut value_pair = None;

    for p in inner {
        match p.as_rule() {
            Rule::type_annotation => {
                let type_inner = p.into_inner().next().unwrap();
                declared_type = Some(parse_type_expr(type_inner));
            }
            Rule::expr => {
                value_pair = Some(p);
            }
            _ => {}
        }
    }

    let value = parse_expr(value_pair.expect("let文に値がありません"));
    let inferred_type = infer_expr_type(&value);

    if let Some(expected_type) = &declared_type {
        if !expected_type.is_compatible_with(&inferred_type) {
            eprintln!(
                "[Type Warning] {}:{} - 型の不一致: 変数 '{}' は {} 型として宣言されていますが、{} 型の値が代入されました",
                line,
                col,
                name,
                expected_type.display(),
                inferred_type.display()
            );
        }
    }

    WithSpan {
        node: ViewNode::LetDecl {
            name,
            value,
            mutable: true,
            declared_type,
        },
        line,
        column: col,
        style: None,
    }
}

/// const宣言のパース
fn parse_const_decl(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();
    let mut declared_type: Option<NiloType> = None;
    let mut value_pair = None;

    for p in inner {
        match p.as_rule() {
            Rule::type_annotation => {
                let type_inner = p.into_inner().next().unwrap();
                declared_type = Some(parse_type_expr(type_inner));
            }
            Rule::expr => {
                value_pair = Some(p);
            }
            _ => {}
        }
    }

    let value = parse_expr(value_pair.expect("const文に値がありません"));
    let inferred_type = infer_expr_type(&value);

    if let Some(expected_type) = &declared_type {
        if !expected_type.is_compatible_with(&inferred_type) {
            eprintln!(
                "[Type Warning] {}:{} - 型の不一致: 定数 '{}' は {} 型として宣言されていますが、{} 型の値が代入されました",
                line,
                col,
                name,
                expected_type.display(),
                inferred_type.display()
            );
        }
    }

    WithSpan {
        node: ViewNode::LetDecl {
            name,
            value,
            mutable: false,
            declared_type,
        },
        line,
        column: col,
        style: None,
    }
}

fn parse_rust_call(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();

    let mut args: Vec<Expr> = Vec::new();
    for arg_p in inner {
        match arg_p.as_rule() {
            Rule::arg_item => {
                let mut it = arg_p.into_inner();
                if let Some(x) = it.next() {
                    if x.as_rule() == Rule::expr {
                        args.push(parse_expr(x));
                    }
                }
            }
            Rule::expr => args.push(parse_expr(arg_p)),
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::RustCall { name, args },
        line,
        column: col,
        style: None,
    }
}

fn parse_text_input(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut id: Option<String> = None;
    let mut placeholder: Option<String> = None;
    let mut style: Option<Style> = None;
    let mut value: Option<Expr> = None;
    let mut on_change: Option<Expr> = None;
    let multiline = false;
    let max_length: Option<usize> = None;
    let ime_enabled = true;

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::expr if id.is_none() => {
                let expr = parse_expr(p);
                match expr {
                    Expr::String(s) => id = Some(s),
                    Expr::Ident(i) => id = Some(i),
                    Expr::Path(pp) => {
                        if !pp.contains('.') {
                            id = Some(pp);
                        } else {
                            log::warn!("TextInputのidにpathは使えません: {}", pp);
                        }
                    }
                    _ => {
                        log::warn!("TextInputの最初の引数は文字列または識別子が必要です");
                    }
                }
            }
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::arg_item => {
                let mut inner = p.into_inner();
                if let Some(key_pair) = inner.next() {
                    let key_rule = key_pair.as_rule();
                    
                    // style_arg の場合
                    if key_rule == Rule::style_arg {
                        style = Some(style_from_expr(parse_expr(key_pair.into_inner().next().unwrap())));
                    }
                    // named_arg の場合
                    else if key_rule == Rule::named_arg {
                        let mut named_arg_inner = key_pair.into_inner();
                        if let Some(actual_key) = named_arg_inner.next() {
                            let actual_key_str = actual_key.as_str();
                            if let Some(val_pair) = named_arg_inner.next() {
                                match actual_key_str {
                                    "placeholder" => {
                                        if let Expr::String(s) = parse_expr(val_pair) {
                                            placeholder = Some(s);
                                        }
                                    }
                                    "style" => {
                                        style = Some(style_from_expr(parse_expr(val_pair)));
                                    }
                                    "value" => {
                                        value = Some(parse_expr(val_pair));
                                    }
                                    "on_change" => {
                                        on_change = Some(parse_expr(val_pair));
                                    }
                                    "bind" => {
                                        value = Some(parse_expr(val_pair));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let id = id.expect("TextInputにidが必要です");
    if let Some(ref st) = style {
        let w = st.width;
        let h = st.height;
        let rw = st.relative_width.map(|d| (d.value, format!("{:?}", d.unit)));
        let rh = st.relative_height.map(|d| (d.value, format!("{:?}", d.unit)));
        log::info!(
            "[PARSE] TextInput id={} style present: width={:?} rel_width={:?} height={:?} rel_height={:?}",
            id, w, rw, h, rh
        );
    } else {
        log::info!("[PARSE] TextInput id={} style: None", id);
    }

    WithSpan {
        node: ViewNode::TextInput {
            id,
            placeholder,
            value,
            on_change,
            multiline,
            max_length,
            ime_enabled,
        },
        line,
        column: col,
        style,
    }
}

fn parse_foreach_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut var: Option<String> = None;
    let mut iterable: Option<Expr> = None;
    let mut style: Option<Style> = None;
    let mut body: Vec<WithSpan<ViewNode>> = Vec::new();

    let mut inner = pair.into_inner();

    if let Some(var_pair) = inner.next() {
        if var_pair.as_rule() == Rule::ident {
            var = Some(var_pair.as_str().to_string());
        }
    }

    if let Some(iterable_pair) = inner.next() {
        match iterable_pair.as_rule() {
            Rule::foreach_iterable => {
                let mut iterable_inner = iterable_pair.into_inner();
                if let Some(actual_iterable) = iterable_inner.next() {
                    match actual_iterable.as_rule() {
                        Rule::path => {
                            iterable = Some(Expr::Path(actual_iterable.as_str().to_string()));
                        }
                        Rule::ident => {
                            iterable = Some(Expr::Ident(actual_iterable.as_str().to_string()));
                        }
                        _ => {
                            iterable = Some(parse_expr(actual_iterable));
                        }
                    }
                }
            }
            _ => {
                iterable = Some(parse_expr(iterable_pair));
            }
        }
    }

    for p in inner {
        match p.as_rule() {
            Rule::foreach_style => {
                let mut style_inner = p.into_inner();
                if let Some(style_arg) = style_inner.next() {
                    if style_arg.as_rule() == Rule::style_arg {
                        let mut style_arg_inner = style_arg.into_inner();
                        if let Some(expr_pair) = style_arg_inner.next() {
                            style = Some(style_from_expr(parse_expr(expr_pair)));
                        }
                    }
                }
            }
            Rule::view_nodes => {
                body = p.into_inner().map(parse_view_node).collect();
            }
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::ForEach {
            var: var.expect("foreach variable not found"),
            iterable: iterable.expect("foreach iterable not found"),
            body,
        },
        line,
        column: col,
        style,
    }
}

fn parse_if_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut condition: Option<Expr> = None;
    let mut style: Option<Style> = None;
    let mut then_body: Vec<WithSpan<ViewNode>> = Vec::new();
    let mut else_body: Option<Vec<WithSpan<ViewNode>>> = None;

    let mut inner = pair.into_inner();

    condition = Some(parse_expr(inner.next().unwrap()));

    for p in inner {
        match p.as_rule() {
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::view_nodes => {
                if then_body.is_empty() {
                    then_body = p.into_inner().map(parse_view_node).collect();
                } else {
                    else_body = Some(p.into_inner().map(parse_view_node).collect());
                }
            }
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::If {
            condition: condition.unwrap(),
            then_body,
            else_body,
        },
        line,
        column: col,
        style,
    }
}
