/// Phase 2: コンポーネントパラメータの型バリデーション

use crate::parser::ast::*;

/// コンポーネント呼び出しの型バリデーション
pub fn validate_component_calls(app: &App) -> Vec<String> {
    let mut warnings = Vec::new();
    
    for timeline in &app.timelines {
        validate_nodes_recursive(&timeline.body, app, &mut warnings);
    }
    
    for component in &app.components {
        validate_nodes_recursive(&component.body, app, &mut warnings);
    }
    
    warnings
}

fn validate_nodes_recursive(nodes: &[WithSpan<ViewNode>], app: &App, warnings: &mut Vec<String>) {
    for node in nodes {
        match &node.node {
            ViewNode::ComponentCall { name, args, slots: _ } => {
                if let Some(comp) = app.components.iter().find(|c| &c.name == name) {
                    validate_component_call(node, comp, args, warnings);
                }
            }
            ViewNode::VStack(children) | ViewNode::HStack(children) => {
                validate_nodes_recursive(children, app, warnings);
            }
            ViewNode::ForEach { body, .. } => {
                validate_nodes_recursive(body, app, warnings);
            }
            ViewNode::If { then_body, else_body, .. } => {
                validate_nodes_recursive(then_body, app, warnings);
                if let Some(else_nodes) = else_body {
                    validate_nodes_recursive(else_nodes, app, warnings);
                }
            }
            ViewNode::Match { arms, default, .. } => {
                for (_, arm_body) in arms {
                    validate_nodes_recursive(arm_body, app, warnings);
                }
                if let Some(default_body) = default {
                    validate_nodes_recursive(default_body, app, warnings);
                }
            }
            _ => {}
        }
    }
}

fn validate_component_call(
    node: &WithSpan<ViewNode>,
    comp: &Component,
    args: &[Expr],
    warnings: &mut Vec<String>,
) {
    // 必須パラメータのチェック
    for (i, param) in comp.params.iter().enumerate() {
        if !param.optional && param.default_value.is_none() {
            if args.get(i).is_none() {
                warnings.push(format!(
                    "{}:{} - 必須パラメータ '{}' がコンポーネント '{}' に渡されていません",
                    node.line, node.column, param.name, comp.name
                ));
            }
        }
    }
    
    // 引数の数チェック
    if args.len() > comp.params.len() {
        warnings.push(format!(
            "{}:{} - コンポーネント '{}' に過剰な引数が渡されています（期待: {}, 実際: {}）",
            node.line, node.column, comp.name, comp.params.len(), args.len()
        ));
    }
    
    // 型チェック
    for (i, arg) in args.iter().enumerate() {
        if let Some(param) = comp.params.get(i) {
            validate_param_type(node, &param.name, &param.param_type, arg, warnings);
        }
    }
}

fn validate_param_type(
    node: &WithSpan<ViewNode>,
    param_name: &str,
    expected_type: &ComponentParamType,
    arg: &Expr,
    warnings: &mut Vec<String>,
) {
    let actual_type = infer_expr_type(arg);
    
    if !is_type_compatible(expected_type, &actual_type) {
        warnings.push(format!(
            "{}:{} - パラメータ '{}' の型が一致しません（期待: {:?}, 実際: {:?}）",
            node.line, node.column, param_name, expected_type, actual_type
        ));
    }
}

fn infer_expr_type(expr: &Expr) -> ComponentParamType {
    match expr {
        Expr::String(_) => ComponentParamType::String,
        Expr::Number(_) => ComponentParamType::Number,
        Expr::Bool(_) => ComponentParamType::Bool,
        Expr::Array(_) => ComponentParamType::Array,
        Expr::Object(_) => ComponentParamType::Object,
        Expr::FunctionCall { .. } => ComponentParamType::Function,
        _ => ComponentParamType::Any,
    }
}

fn is_type_compatible(expected: &ComponentParamType, actual: &ComponentParamType) -> bool {
    match (expected, actual) {
        (ComponentParamType::Any, _) | (_, ComponentParamType::Any) => true,
        (ComponentParamType::String, ComponentParamType::String) => true,
        (ComponentParamType::Number, ComponentParamType::Number) => true,
        (ComponentParamType::Bool, ComponentParamType::Bool) => true,
        (ComponentParamType::Array, ComponentParamType::Array) => true,
        (ComponentParamType::Object, ComponentParamType::Object) => true,
        (ComponentParamType::Function, ComponentParamType::Function) => true,
        (ComponentParamType::Enum(values), ComponentParamType::String) => {
            // 列挙型の場合、文字列型は互換性がある（実行時チェック必要）
            !values.is_empty()
        }
        _ => false,
    }
}
