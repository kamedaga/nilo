// src/engine/core/component.rs
// コンポーネント展開・パラメータ置換関連

use crate::engine::state::{AppState, StateAccess};
use crate::parser::ast::{App, ComponentArg, Expr, Style, ViewNode, WithSpan};

/// 軽量化されたコンポーネント展開
pub fn expand_component_calls_lightweight<S>(
    nodes: &[WithSpan<ViewNode>],
    app: &App,
    _state: &mut AppState<S>,
) -> Vec<WithSpan<ViewNode>>
where
    S: StateAccess + 'static,
{
    let mut result = Vec::new();

    for node in nodes {
        match &node.node {
            ViewNode::ComponentCall {
                name,
                args,
                slots: _,
            } => {
                if let Some(comp) = app.components.iter().find(|c| c.name == *name) {
                    // コンポーネントのボディをクローンして引数を適用
                    let mut expanded_body = comp.body.clone();

                    // ★ Phase 2: デフォルト値を考慮したパラメータ置換
                    // 名前付き引数と位置引数を両方サポート
                    let mut positional_index = 0;

                    for param in comp.params.iter() {
                        let arg_value =
                            match find_argument_value(args, &param.name, positional_index) {
                                Some(value) => value,
                                None => {
                                    // 引数が見つからない場合
                                    if let Some(default) = &param.default_value {
                                        // デフォルト値を使用
                                        default.clone()
                                    } else if param.optional {
                                        // オプショナルで値がない場合はスキップ
                                        positional_index += 1;
                                        continue;
                                    } else {
                                        // 必須パラメータで値がない場合はスキップ
                                        positional_index += 1;
                                        continue;
                                    }
                                }
                            };

                        substitute_parameter_in_nodes(&mut expanded_body, &param.name, &arg_value);
                        positional_index += 1;
                    }

                    // デフォルトスタイルを適用（ComponentCallのスタイルがない場合のみ）
                    if let Some(default_style) = &comp.default_style {
                        apply_default_style_to_nodes(&mut expanded_body, default_style);
                    }

                    // ComponentCallにスタイルがある場合は、それを最初のノードに適用（優先）
                    if let Some(call_style) = &node.style {
                        if let Some(first_node) = expanded_body.first_mut() {
                            // ComponentCallのスタイルを最優先で適用
                            match &mut first_node.style {
                                Some(existing_style) => {
                                    // ComponentCallのスタイルを既存のスタイルにマージ（ComponentCallが優先）
                                    merge_styles_prioritize_override(existing_style, call_style);
                                }
                                None => {
                                    first_node.style = Some(call_style.clone());
                                }
                            }
                        }
                    }

                    result.extend(expanded_body);
                }
            }
            ViewNode::VStack(children) => {
                let expanded_children = expand_component_calls_lightweight(children, app, _state);
                result.push(WithSpan {
                    node: ViewNode::VStack(expanded_children),
                    line: node.line,
                    column: node.column,
                    style: node.style.clone(),
                });
            }
            ViewNode::HStack(children) => {
                let expanded_children = expand_component_calls_lightweight(children, app, _state);
                result.push(WithSpan {
                    node: ViewNode::HStack(expanded_children),
                    line: node.line,
                    column: node.column,
                    style: node.style.clone(),
                });
            }
            _ => {
                result.push(node.clone());
            }
        }
    }

    result
}

/// ノード内のパラメータを置換する
pub fn substitute_parameter_in_nodes(
    nodes: &mut [WithSpan<ViewNode>],
    param_name: &str,
    arg: &Expr,
) {
    for node in nodes {
        substitute_parameter_in_node(&mut node.node, param_name, arg);
    }
}

/// 単一ノード内のパラメータを置換する
fn substitute_parameter_in_node(node: &mut ViewNode, param_name: &str, arg: &Expr) {
    match node {
        ViewNode::Text { args, .. } => {
            // Text argsの中でパラメータを探す
            for text_arg in args {
                match text_arg {
                    Expr::Path(path) => {
                        // 完全一致の場合: user == user
                        if path == param_name {
                            *text_arg = arg.clone();
                        }
                        // パスの先頭一致の場合: user.name の user部分を置換
                        else if path.starts_with(&format!("{}.", param_name)) {
                            let property_path = &path[(param_name.len() + 1)..]; // "name" or "email"

                            // 引数がObjectの場合、プロパティアクセスを解決
                            if let Expr::Object(fields) = arg {
                                // property_pathから値を取得（ドット区切りサポート）
                                let parts: Vec<&str> = property_path.split('.').collect();
                                if let Some(value) = get_nested_object_property(fields, &parts) {
                                    *text_arg = value.clone();
                                }
                            }
                        }
                    }
                    Expr::Ident(ident) => {
                        // 識別子の場合も同様
                        if ident == param_name {
                            *text_arg = arg.clone();
                        }
                    }
                    _ => {}
                }
            }
        }
        ViewNode::VStack(children) | ViewNode::HStack(children) => {
            substitute_parameter_in_nodes(children, param_name, arg);
        }
        ViewNode::If {
            condition,
            then_body,
            else_body,
        } => {
            // if文の条件式内のパラメータを置換
            substitute_expr_parameter(condition, param_name, arg);
            // then/elseブランチの子ノードも置換
            substitute_parameter_in_nodes(then_body, param_name, arg);
            if let Some(else_nodes) = else_body {
                substitute_parameter_in_nodes(else_nodes, param_name, arg);
            }
        }
        // 他のノードタイプも必要に応じて追加
        _ => {}
    }
}

/// 式内のパラメータを置換する
fn substitute_expr_parameter(expr: &mut Expr, param_name: &str, arg: &Expr) {
    match expr {
        Expr::Ident(ident) if ident == param_name => {
            *expr = arg.clone();
        }
        Expr::Path(path) if path == param_name => {
            *expr = arg.clone();
        }
        // 他の式タイプも必要に応じて追加
        _ => {}
    }
}

/// オブジェクトからネストされたプロパティを取得
fn get_nested_object_property<'a>(fields: &'a [(String, Expr)], path: &[&str]) -> Option<&'a Expr> {
    if path.is_empty() {
        return None;
    }

    let first_key = path[0];
    for (key, value) in fields {
        if key == first_key {
            if path.len() == 1 {
                // 最後のキー
                return Some(value);
            } else {
                // ネストアクセス
                if let Expr::Object(nested_fields) = value {
                    return get_nested_object_property(nested_fields, &path[1..]);
                }
                return None;
            }
        }
    }
    None
}

/// ComponentCallのスタイルを既存のスタイルにマージ（ComponentCallが優先）
fn merge_styles_prioritize_override(existing: &mut Style, override_style: &Style) {
    // ComponentCallで指定されたスタイルを最優先で適用
    if override_style.relative_width.is_some() {
        existing.relative_width = override_style.relative_width;
    }
    if override_style.relative_height.is_some() {
        existing.relative_height = override_style.relative_height;
    }
    if override_style.width.is_some() {
        existing.width = override_style.width;
    }
    if override_style.height.is_some() {
        existing.height = override_style.height;
    }
    if override_style.background.is_some() {
        existing.background = override_style.background.clone();
    }
    if override_style.color.is_some() {
        existing.color = override_style.color.clone();
    }
    if override_style.padding.is_some() {
        existing.padding = override_style.padding.clone();
    }
    if override_style.margin.is_some() {
        existing.margin = override_style.margin.clone();
    }
    if override_style.relative_padding.is_some() {
        existing.relative_padding = override_style.relative_padding.clone();
    }
    if override_style.relative_margin.is_some() {
        existing.relative_margin = override_style.relative_margin.clone();
    }
}

/// ノードにデフォルトスタイルを適用する
fn apply_default_style_to_nodes(nodes: &mut [WithSpan<ViewNode>], default_style: &Style) {
    for node in nodes {
        // ルートノードにデフォルトスタイルをマージ
        match &mut node.style {
            Some(existing_style) => {
                // デフォルトスタイルの値を既存のスタイルにマージ（既存の値を優先）
                if existing_style.relative_width.is_none() && default_style.relative_width.is_some()
                {
                    existing_style.relative_width = default_style.relative_width;
                }
                if existing_style.relative_height.is_none()
                    && default_style.relative_height.is_some()
                {
                    existing_style.relative_height = default_style.relative_height;
                }
                if existing_style.width.is_none() && default_style.width.is_some() {
                    existing_style.width = default_style.width;
                }
                if existing_style.height.is_none() && default_style.height.is_some() {
                    existing_style.height = default_style.height;
                }
                if existing_style.background.is_none() && default_style.background.is_some() {
                    existing_style.background = default_style.background.clone();
                }
                if existing_style.color.is_none() && default_style.color.is_some() {
                    existing_style.color = default_style.color.clone();
                }
                if existing_style.padding.is_none() && default_style.padding.is_some() {
                    existing_style.padding = default_style.padding.clone();
                }
                if existing_style.margin.is_none() && default_style.margin.is_some() {
                    existing_style.margin = default_style.margin.clone();
                }
                if existing_style.relative_padding.is_none()
                    && default_style.relative_padding.is_some()
                {
                    existing_style.relative_padding = default_style.relative_padding.clone();
                }
                if existing_style.relative_margin.is_none()
                    && default_style.relative_margin.is_some()
                {
                    existing_style.relative_margin = default_style.relative_margin.clone();
                }
            }
            None => {
                // Applying default style to node without existing style
                node.style = Some(default_style.clone());
            }
        }
    }
}

/// 引数リストから指定されたパラメータの値を見つける
/// 名前付き引数を優先し、見つからない場合は位置引数から取得
fn find_argument_value(
    args: &[ComponentArg],
    param_name: &str,
    positional_index: usize,
) -> Option<Expr> {
    // まず名前付き引数を探す
    for arg in args {
        if let ComponentArg::Named(name, expr) = arg {
            if name == param_name {
                return Some(expr.clone());
            }
        }
    }

    // 名前付き引数が見つからない場合、位置引数を探す
    let mut current_positional = 0;
    for arg in args {
        if let ComponentArg::Positional(expr) = arg {
            if current_positional == positional_index {
                return Some(expr.clone());
            }
            current_positional += 1;
        }
    }

    None
}
