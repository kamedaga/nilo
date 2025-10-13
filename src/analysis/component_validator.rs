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
            ViewNode::ComponentCall {
                name,
                args,
                slots: _,
            } => {
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
            ViewNode::If {
                then_body,
                else_body,
                ..
            } => {
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
    args: &[ComponentArg],
    warnings: &mut Vec<String>,
) {
    // 必須パラメータのチェック
    for param in comp.params.iter() {
        if !param.optional && param.default_value.is_none() {
            // 名前付き引数または位置引数で値が渡されているかチェック
            let has_value = args.iter().any(|arg| match arg {
                ComponentArg::Named(name, _) => name == &param.name,
                ComponentArg::Positional(_) => true, // 位置引数は後で詳細チェック
            });

            if !has_value {
                warnings.push(format!(
                    "{}:{} - 必須パラメータ '{}' がコンポーネント '{}' に渡されていません",
                    node.line, node.column, param.name, comp.name
                ));
            }
        }
    }

    // 位置引数の数をカウント
    let positional_count = args
        .iter()
        .filter(|arg| matches!(arg, ComponentArg::Positional(_)))
        .count();

    // 位置引数の数チェック
    if positional_count > comp.params.len() {
        warnings.push(format!(
            "{}:{} - コンポーネント '{}' に過剰な位置引数が渡されています（期待: {}, 実際: {}）",
            node.line,
            node.column,
            comp.name,
            comp.params.len(),
            positional_count
        ));
    }

    // 名前付き引数の名前が存在するかチェック
    for arg in args {
        if let ComponentArg::Named(name, _) = arg {
            if !comp.params.iter().any(|p| &p.name == name) {
                warnings.push(format!(
                    "{}:{} - コンポーネント '{}' に未定義のパラメータ '{}' が渡されています",
                    node.line, node.column, comp.name, name
                ));
            }
        }
    }

    // 型チェック
    for arg in args.iter() {
        let (param, expr) = match arg {
            ComponentArg::Positional(expr) => {
                // 位置引数の場合は順序で対応するパラメータを探す
                let positional_index = args
                    .iter()
                    .take_while(|a| !std::ptr::eq(*a, arg))
                    .filter(|a| matches!(a, ComponentArg::Positional(_)))
                    .count();
                if let Some(param) = comp.params.get(positional_index) {
                    (param, expr)
                } else {
                    continue;
                }
            }
            ComponentArg::Named(name, expr) => {
                if let Some(param) = comp.params.iter().find(|p| &p.name == name) {
                    (param, expr)
                } else {
                    continue;
                }
            }
        };

        validate_param_type(node, &param.name, &param.param_type, expr, warnings);
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
