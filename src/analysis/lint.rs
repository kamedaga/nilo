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
    for transition in &app.flow.transitions {
        for target in &transition.to {
            if !timeline_names.contains(target.timeline.as_str()) {
                diags.push(super::error::Diagnostic::error(format!(
                    "Timeline '{}' referenced in flow but not defined",
                    target.timeline
                )));
            }
        }
    }

    // Flow解析：timeline内のnavigate_toとflowの整合性チェック
    check_flow_consistency(app, &mut diags);

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
            diags.push(super::error::Diagnostic::error(format!(
                "Timeline '{}' is defined more than once ({} times)",
                name, count
            )));
        }
    }

    // 重複コンポーネントチェック
    let mut component_name_counts = std::collections::HashMap::new();
    for c in &app.components {
        *component_name_counts.entry(&c.name).or_insert(0) += 1;
    }
    for (name, count) in &component_name_counts {
        if *count > 1 {
            diags.push(super::error::Diagnostic::error(format!(
                "Component '{}' is defined more than once ({} times)",
                name, count
            )));
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

    // コンポーネント内のボタンIDチェック
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

fn check_flow_consistency(app: &App, diags: &mut Vec<super::error::Diagnostic>) {
    let timeline_names: std::collections::HashSet<_> =
        app.timelines.iter().map(|t| t.name.as_str()).collect();

    // flow定義から遷移マップを作成
    let mut flow_transitions: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();

    for transition in &app.flow.transitions {
        for from in &transition.from {
            let targets: std::collections::HashSet<String> = transition
                .to
                .iter()
                .map(|target| target.timeline.clone())
                .collect();
            // 同じfromからの複数の遷移を統合
            flow_transitions
                .entry(from.clone())
                .or_insert_with(std::collections::HashSet::new)
                .extend(targets);
        }
    }

    // 各timelineでnavigate_toの使用をチェック
    for timeline in &app.timelines {
        let mut used_navigations: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        collect_navigations(&timeline.body, &mut used_navigations);
        collect_navigations_from_whens(&timeline.whens, &mut used_navigations);

        // timeline内のnavigate_toがflowに定義されているかチェック
        for target in &used_navigations {
            // まずtargetが存在するtimelineかチェック
            if !timeline_names.contains(target.as_str()) {
                diags.push(super::error::Diagnostic::error(format!(
                    "Timeline '{}' is referenced in navigate_to but not defined",
                    target
                )));
                continue;
            }

            // flowでこのtimelineからの遷移が定義されているかチェック
            if let Some(allowed_targets) = flow_transitions.get(&timeline.name) {
                if !allowed_targets.contains(target) {
                    diags.push(super::error::Diagnostic::error(
                        format!(
                            "Timeline '{}' navigates to '{}' but this transition is not defined in flow",
                            timeline.name, target
                        ),
                    ));
                }
            } else {
                // このtimelineからの遷移がflowに全く定義されていない
                diags.push(super::error::Diagnostic::error(
                    format!(
                        "Timeline '{}' navigates to '{}' but no transitions from '{}' are defined in flow",
                        timeline.name, target, timeline.name
                    ),
                ));
            }
        }

        // flowに定義された遷移にnavigate_toが実装されているかチェック
        if let Some(flow_targets) = flow_transitions.get(&timeline.name) {
            for flow_target in flow_targets {
                if !used_navigations.contains(flow_target) {
                    diags.push(super::error::Diagnostic::warning(
                        format!(
                            "Flow defines transition from '{}' to '{}' but no navigate_to('{}') found in timeline",
                            timeline.name, flow_target, flow_target
                        ),
                    ));
                }
            }
        }
    }

    // 同様にcomponent内のnavigate_toもチェック
    for component in &app.components {
        let mut used_navigations: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        collect_navigations(&component.body, &mut used_navigations);
        collect_navigations_from_whens(&component.whens, &mut used_navigations);

        for target in &used_navigations {
            if !timeline_names.contains(target.as_str()) {
                diags.push(super::error::Diagnostic::error(format!(
                    "Timeline '{}' is referenced in navigate_to in component '{}' but not defined",
                    target, component.name
                )));
            }
        }
    }
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
            ViewNode::ComponentCall {
                name,
                args: _,
                slots: _,
            } => {
                out.insert(name.clone());

                // 特別なケースのチェック
                if name == "Text" {
                    let loc = Some(format!("line {}, col {}", node.line, node.column));
                    let mut d = super::error::Diagnostic::error(
                        "Using `Text(variable)` style is not supported. \
Use `Text(\"{}\", variable)` instead to display variable values.",
                    );
                    d.location = loc;
                    diags.push(d);
                } else if !defined.contains(name.as_str()) {
                    // 組み込み関数やステンシル関数をチェック
                    let builtin_functions = [
                        "rect",
                        "circle",
                        "triangle",
                        "rounded_rect",
                        "text",
                        "image",
                        "Spacing",
                        "SpacingAuto",
                        "Image",
                    ];

                    if !builtin_functions.contains(&name.as_str()) {
                        let loc = Some(format!("line {}, col {}", node.line, node.column));
                        let mut d = super::error::Diagnostic::error(format!(
                            "Component '{}' is called but not defined",
                            name
                        ));
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

fn collect_navigations(
    nodes: &[WithSpan<ViewNode>],
    navigations: &mut std::collections::HashSet<String>,
) {
    for node in nodes {
        match &node.node {
            ViewNode::NavigateTo { target } => {
                navigations.insert(target.clone());
            }
            ViewNode::VStack(children) | ViewNode::HStack(children) => {
                collect_navigations(children, navigations);
            }
            ViewNode::DynamicSection { body, .. } => {
                collect_navigations(body, navigations);
            }
            ViewNode::Match { arms, default, .. } => {
                for (_, nodes) in arms {
                    collect_navigations(nodes, navigations);
                }
                if let Some(default_nodes) = default {
                    collect_navigations(default_nodes, navigations);
                }
            }
            ViewNode::When { actions, .. } => {
                collect_navigations(actions, navigations);
            }
            ViewNode::ForEach { body, .. } => {
                collect_navigations(body, navigations);
            }
            ViewNode::If {
                then_body,
                else_body,
                ..
            } => {
                collect_navigations(then_body, navigations);
                if let Some(else_nodes) = else_body {
                    collect_navigations(else_nodes, navigations);
                }
            }
            _ => {}
        }
    }
}

fn collect_navigations_from_whens(
    whens: &[When],
    navigations: &mut std::collections::HashSet<String>,
) {
    for when in whens {
        collect_navigations(&when.actions, navigations);
    }
}
