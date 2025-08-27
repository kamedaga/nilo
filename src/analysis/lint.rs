use crate::parser::ast::*;

#[derive(Debug)]
pub struct LintWarning {
    pub message: String,
    pub location: Option<String>,
}

pub fn run_lints(app: &App) -> Vec<super::error::Diagnostic> {
    let mut diags = Vec::new();

    // タイムライン参照チェック
    let timeline_names: std::collections::HashSet<_> =
        app.timelines.iter().map(|t| t.name.as_str()).collect();
    for (_from, to_list) in &app.flow.transitions {
        for to in to_list {
            if !timeline_names.contains(to.as_str()) {
                diags.push(super::error::Diagnostic::error(
                    format!("Timeline '{}' referenced in flow but not defined", to),
                ));
            }
        }
    }

    // コンポーネント使用チェック
    let defined_components: std::collections::HashSet<_> =
        app.components.iter().map(|c| c.name.as_str()).collect();
    let mut called_components = std::collections::HashSet::new();

    // タイムラインとコンポーネントの解析
    for timeline in &app.timelines {
        visit_nodes(
            &timeline.body,
            &mut called_components,
            &defined_components,
            &mut diags,
        );
    }
    for component in &app.components {
        visit_nodes(
            &component.body,
            &mut called_components,
            &defined_components,
            &mut diags,
        );
    }

    // 未使用コンポーネントチェック
    for c in &app.components {
        if !called_components.contains(&c.name) {
            diags.push(super::error::Diagnostic::warning(format!(
                "Component '{}' is defined but never used",
                c.name
            )));
        }
    }

    // 重複タイムラインチェック
    let mut timeline_name_counts = std::collections::HashMap::new();
    for t in &app.timelines {
        *timeline_name_counts.entry(&t.name).or_insert(0) += 1;
    }
    for (name, count) in &timeline_name_counts {
        if *count > 1 {
            diags.push(super::error::Diagnostic::error(
                format!("Timeline '{}' is defined more than once ({} times)", name, count),
            ));
        }
    }

    // 重複コンポーネントチェック
    let mut component_name_counts = std::collections::HashMap::new();
    for c in &app.components {
        *component_name_counts.entry(&c.name).or_insert(0) += 1;
    }
    for (name, count) in &component_name_counts {
        if *count > 1 {
            diags.push(super::error::Diagnostic::error(
                format!("Component '{}' is defined more than once ({} times)", name, count),
            ));
        }
    }

    // タイムライン内のボタンIDチェック
    for timeline in &app.timelines {
        let mut button_ids = std::collections::HashMap::<String, usize>::new();
        collect_button_ids(&timeline.body, &mut button_ids);

        // 重複チェック
        for (id, count) in &button_ids {
            if *count > 1 {
                diags.push(super::error::Diagnostic::warning(format!(
                    "Button id '{}' is defined more than once in timeline '{}'",
                    id, timeline.name
                )));
            }
        }

        // when のターゲットチェック
        for when in &timeline.whens {
            if let EventExpr::ButtonPressed(target) = &when.event {
                if !button_ids.contains_key(target) {
                    diags.push(super::error::Diagnostic::warning(format!(
                        "`when user.click({})` in timeline '{}' refers to a button id that is not defined",
                        target, timeline.name
                    )));
                }
            }
        }
    }

    // コンポーネント内のボタ���IDチェック
    for component in &app.components {
        let mut button_ids = std::collections::HashMap::<String, usize>::new();
        collect_button_ids(&component.body, &mut button_ids);

        for (id, count) in &button_ids {
            if *count > 1 {
                diags.push(super::error::Diagnostic::warning(format!(
                    "Button id '{}' is defined more than once in component '{}'",
                    id, component.name
                )));
            }
        }

        for when in &component.whens {
            if let EventExpr::ButtonPressed(target) = &when.event {
                if !button_ids.contains_key(target) {
                    diags.push(super::error::Diagnostic::warning(format!(
                        "`when user.click({})` in component '{}' refers to a button id that is not defined",
                        target, component.name
                    )));
                }
            }
        }
    }

    diags
}

fn collect_button_ids(
    nodes: &[WithSpan<ViewNode>],
    map: &mut std::collections::HashMap<String, usize>,
) {
    for node in nodes {
        match &node.node {
            ViewNode::Button { id, .. } => {
                *map.entry(id.clone()).or_insert(0) += 1;
            }
            ViewNode::VStack(children) | ViewNode::HStack(children) => {
                collect_button_ids(children, map);
            }
            ViewNode::DynamicSection { body, .. } => {
                collect_button_ids(body, map);
            }
            ViewNode::Match { arms, default, .. } => {
                for (_, nodes) in arms {
                    collect_button_ids(nodes, map);
                }
                if let Some(default_nodes) = default {
                    collect_button_ids(default_nodes, map);
                }
            }
            ViewNode::When { actions, .. } => {
                collect_button_ids(actions, map);
            }
            // 全てのViewNodeバリアントを適切に処理
            ViewNode::ComponentCall { .. }
            | ViewNode::Text { .. }
            | ViewNode::Image { .. }
            | ViewNode::Spacing(_)
            | ViewNode::SpacingAuto
            | ViewNode::NavigateTo { .. }
            | ViewNode::Stencil(_)
            | ViewNode::RustCall { .. }
            | ViewNode::Set { .. }
            | ViewNode::Toggle { .. }
            | ViewNode::ListAppend { .. }
            | ViewNode::ListRemove { .. } => {
                // これらのノードはボタンIDを持たないため何もしない
            }
            _ => {}
        }
    }
}

fn visit_nodes(
    nodes: &[WithSpan<ViewNode>],
    out: &mut std::collections::HashSet<String>,
    defined: &std::collections::HashSet<&str>,
    diags: &mut Vec<super::error::Diagnostic>,
) {
    for node in nodes {
        match &node.node {
            ViewNode::ComponentCall { name, args: _ } => {
                out.insert(name.clone());

                // 特別なケースのチェック
                if name == "Text" {
                    let loc = Some(format!("line {}, col {}", node.line, node.column));
                    let mut d = super::error::Diagnostic::error(
                        "Using `Text(variable)` style is not supported. \
Use `Text(\"{}\", variable)` instead to display variable values."
                    );
                    d.location = loc;
                    diags.push(d);
                } else if !defined.contains(name.as_str()) {
                    // ��み込み関数やステンシル関数をチェック
                    let builtin_functions = [
                        "rect", "circle", "triangle", "rounded_rect", "text", "image",
                        "Spacing", "SpacingAuto", "Image"
                    ];

                    if !builtin_functions.contains(&name.as_str()) {
                        let loc = Some(format!("line {}, col {}", node.line, node.column));
                        let mut d = super::error::Diagnostic::error(
                            format!("Component '{}' is called but not defined", name),
                        );
                        d.location = loc;
                        diags.push(d);
                    }
                }
            }
            ViewNode::RustCall { name: _, args: _ } => {
                // Rust関数呼び出しは定義済みコンポーネントのチェック対象外
                // Rust側で定義された関数なので、lintでのチェックは不要
            }
            ViewNode::VStack(children) | ViewNode::HStack(children) => {
                visit_nodes(children, out, defined, diags);
            }
            ViewNode::DynamicSection { body, .. } => {
                visit_nodes(body, out, defined, diags);
            }
            ViewNode::Match { arms, default, .. } => {
                for (_, nodes) in arms {
                    visit_nodes(nodes, out, defined, diags);
                }
                if let Some(default_nodes) = default {
                    visit_nodes(default_nodes, out, defined, diags);
                }
            }
            ViewNode::When { actions, .. } => {
                visit_nodes(actions, out, defined, diags);
            }
            // ステンシル呼び出しの検証
            ViewNode::Stencil(_) => {
                // ステンシルは既にパースされているので、特別なチェックは不要
            }
            // その他のノードタイプ
            ViewNode::Button { .. }
            | ViewNode::Text { .. }
            | ViewNode::Image { .. }
            | ViewNode::Spacing(_)
            | ViewNode::SpacingAuto
            | ViewNode::NavigateTo { .. }
            | ViewNode::Set { .. }
            | ViewNode::Toggle { .. }
            | ViewNode::ListAppend { .. }
            | ViewNode::ListRemove { .. } => {
                // これらは特別な処理を必要としない
            }
            _ => {}
        }
    }
}
