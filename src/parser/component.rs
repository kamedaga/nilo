// ========================================
// コンポーネントパーサーモジュール
// ========================================
//
// このモジュールはコンポーネント定義とパラメータの解析を担当します。

use pest::iterators::Pair;
use crate::parser::ast::*;
use crate::parser::utils::unquote;
use crate::parser::expr::parse_expr;
use crate::parser::style::style_from_expr;
use crate::parser::parse::Rule;

// view_nodeのパース関数は循環参照を避けるため、後で定義される
use crate::parser::view_node::parse_view_node;

/// コンポーネント定義をパースする
pub fn parse_component_def(pair: Pair<Rule>) -> Component {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();

    let mut params = vec![];
    let mut default_style = None;

    // component_paramsを処理
    if let Some(Rule::component_params) = inner.peek().map(|p| p.as_rule()) {
        let params_pair = inner.next().unwrap();
        for param_pair in params_pair.into_inner() {
            match param_pair.as_rule() {
                Rule::component_param => {
                    let param_inner = param_pair.into_inner().next().unwrap();
                    match param_inner.as_rule() {
                        Rule::style_param => {
                            // スタイル定義: "style" ":" expr
                            let style_inner = param_inner.into_inner();
                            // Rule::style_param = { "style" ~ ":" ~ expr }の構造なので、最後の要素がexpr
                            let mut elements: Vec<_> = style_inner.collect();
                            if let Some(style_expr) = elements.pop() {
                                let parsed_style = style_from_expr(parse_expr(style_expr));
                                default_style = Some(parsed_style);
                            }
                        }
                        // ★ Phase 2: 型付きパラメータ
                        Rule::typed_param => {
                            params.push(parse_typed_param(param_inner));
                        }
                        Rule::optional_param => {
                            params.push(parse_optional_param(param_inner));
                        }
                        Rule::enum_param => {
                            params.push(parse_enum_param(param_inner));
                        }
                        Rule::ident => {
                            // パラメータ名（後方互換）
                            let param_name = param_inner.as_str().to_string();
                            params.push(ComponentParam {
                                name: param_name,
                                param_type: ComponentParamType::Any,
                                default_value: None,
                                optional: false,
                            });
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    let mut font: Option<String> = None;
    let mut body: Vec<WithSpan<ViewNode>> = Vec::new();
    let whens = Vec::new();

    for node_pair in inner {
        match node_pair.as_rule() {
            Rule::font_def => {
                // font: "fonts/font" の形式を解析
                let font_str = node_pair.into_inner().next().unwrap().as_str();
                font = Some(unquote(font_str));
            }
            Rule::view_nodes => {
                for p in node_pair.into_inner() {
                    body.push(parse_view_node(p));
                }
            }
            _ => body.push(parse_view_node(node_pair)),
        }
    }
    // ★ Phase 2: 既存のString形式のparamsをComponentParamに変換は不要（既にComponentParam）
    log::info!("🔍 Parsed component '{}' with {} parameters", name, params.len());
    for (i, param) in params.iter().enumerate() {
        log::info!("  Param {}: name='{}', type={:?}, default={:?}, optional={}", 
                   i, param.name, param.param_type, param.default_value, param.optional);
    }
    Component { name, params, font, default_style, body, whens }
}

/// ★ Phase 2: 型付きパラメータをパース (name: Type = default)
pub fn parse_typed_param(pair: Pair<Rule>) -> ComponentParam {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let param_type = parse_param_type(inner.next().unwrap());
    let default_value = inner.next().map(parse_expr);
    
    ComponentParam {
        name,
        param_type,
        default_value,
        optional: false,
    }
}

/// ★ Phase 2: オプショナルパラメータをパース (name: Type?)
pub fn parse_optional_param(pair: Pair<Rule>) -> ComponentParam {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let param_type = parse_param_type(inner.next().unwrap());
    
    ComponentParam {
        name,
        param_type,
        default_value: None,
        optional: true,
    }
}

/// ★ Phase 2: 列挙型パラメータをパース (name: ("a" | "b" | "c") = "a")
pub fn parse_enum_param(pair: Pair<Rule>) -> ComponentParam {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    
    let mut enum_values = Vec::new();
    let mut default_value = None;
    
    for p in inner {
        match p.as_rule() {
            Rule::string => {
                let val = unquote(p.as_str());
                if default_value.is_none() {
                    enum_values.push(val);
                } else {
                    // 最後のstringはデフォルト値
                    default_value = Some(Expr::String(val));
                }
            }
            _ => {}
        }
    }
    
    // 最後の要素をデフォルト値として取り出す
    if default_value.is_none() && !enum_values.is_empty() {
        let last = enum_values.pop().unwrap();
        default_value = Some(Expr::String(last));
    }
    
    ComponentParam {
        name,
        param_type: ComponentParamType::Enum(enum_values),
        default_value,
        optional: false,
    }
}

/// ★ Phase 2: パラメータ型をパース
pub fn parse_param_type(pair: Pair<Rule>) -> ComponentParamType {
    match pair.as_str() {
        "Object" | "object" => ComponentParamType::Object,
        "Array" | "array" => ComponentParamType::Array,
        "Function" | "function" => ComponentParamType::Function,
        "String" | "string" => ComponentParamType::String,
        "Number" | "number" => ComponentParamType::Number,
        "Bool" | "bool" => ComponentParamType::Bool,
        _ => ComponentParamType::Any,
    }
}
