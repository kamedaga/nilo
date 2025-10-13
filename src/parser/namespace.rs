// ========================================
// 名前空間展開モジュール
// ========================================
//
// このモジュールは名前空間とnamespaced flowの展開処理を担当します。

use crate::parser::ast::*;
use crate::parser::component::parse_component_def;
use crate::parser::parse::Rule;
use crate::parser::timeline::parse_timeline_def;
use pest::iterators::Pair;

/// namespace定義をパース
pub fn parse_namespace_def(pair: Pair<Rule>) -> Result<Namespace, String> {
    assert_eq!(pair.as_rule(), Rule::namespace_def);

    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .ok_or("namespace名がありません")?
        .as_str()
        .to_string();

    let mut timelines = Vec::new();
    let mut components = Vec::new();

    for item in inner {
        match item.as_rule() {
            Rule::timeline_def => {
                timelines.push(parse_timeline_def(item));
            }
            Rule::component_def => {
                components.push(parse_component_def(item));
            }
            _ => {}
        }
    }

    log::info!(
        "Parsed namespace '{}' with {} timelines",
        name,
        timelines.len()
    );

    Ok(Namespace {
        name,
        timelines,
        components,
    })
}

/// namespaced flow定義をパース
pub fn parse_namespaced_flow_def(pair: Pair<Rule>) -> Result<NamespacedFlow, String> {
    assert_eq!(pair.as_rule(), Rule::namespaced_flow_def);

    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .ok_or("flow名がありません")?
        .as_str()
        .to_string();

    let mut start = None;
    let mut transitions = Vec::new();

    for item in inner {
        match item.as_rule() {
            Rule::namespaced_start_def => {
                let start_inner = item.into_inner().next().ok_or("start定義が空です")?;
                start = Some(start_inner.as_str().to_string());
            }
            Rule::namespaced_transition_def => {
                let transition = parse_namespaced_transition_def(item)?;
                transitions.push(transition);
            }
            _ => {}
        }
    }

    let start = start.ok_or("startが定義されていません")?;

    log::info!("Parsed namespaced flow '{}' starting at '{}'", name, start);

    Ok(NamespacedFlow {
        name,
        start,
        transitions,
    })
}

/// namespaced transition定義をパース
fn parse_namespaced_transition_def(pair: Pair<Rule>) -> Result<NamespacedTransition, String> {
    assert_eq!(pair.as_rule(), Rule::namespaced_transition_def);

    let mut inner = pair.into_inner();

    // 遷移元
    let source_pair = inner.next().ok_or("遷移元がありません")?;
    let from = parse_namespaced_transition_source(source_pair)?;

    // 遷移先
    let target_pair = inner.next().ok_or("遷移先がありません")?;
    let to = parse_namespaced_transition_targets(target_pair)?;

    Ok(NamespacedTransition { from, to })
}

fn parse_namespaced_transition_source(pair: Pair<Rule>) -> Result<Vec<String>, String> {
    assert_eq!(pair.as_rule(), Rule::namespaced_transition_source);

    let mut sources = Vec::new();
    for ident in pair.into_inner() {
        sources.push(ident.as_str().to_string());
    }

    if sources.is_empty() {
        Err("遷移元が空です".to_string())
    } else {
        Ok(sources)
    }
}

fn parse_namespaced_transition_targets(pair: Pair<Rule>) -> Result<Vec<String>, String> {
    assert_eq!(pair.as_rule(), Rule::namespaced_transition_targets);

    let mut targets = Vec::new();
    for ident in pair.into_inner() {
        targets.push(ident.as_str().to_string());
    }

    if targets.is_empty() {
        Err("遷移先が空です".to_string())
    } else {
        Ok(targets)
    }
}

/// 名前空間とNamespacedFlowを展開して平坦なFlowに変換
pub fn expand_namespaced_structures(
    namespaced_flows: Vec<NamespacedFlow>,
    namespaces: Vec<Namespace>,
    mut timelines: Vec<Timeline>,
    base_flow: Option<Flow>,
) -> Result<(Flow, Vec<Timeline>), String> {
    // 1. 名前空間内のタイムラインに名前空間プレフィックスを付ける
    for namespace in &namespaces {
        for timeline in &namespace.timelines {
            let mut prefixed_timeline = timeline.clone();
            prefixed_timeline.name = format!("{}::{}", namespace.name, timeline.name);
            log::info!("Registered namespaced timeline: {}", prefixed_timeline.name);
            timelines.push(prefixed_timeline);
        }
    }

    // 2. NamespacedFlowを展開
    let mut all_transitions = Vec::new();
    let mut global_start = None;

    // NamespacedFlowの名前とstartのマップを作成
    let flow_start_map: std::collections::HashMap<String, String> = namespaced_flows
        .iter()
        .map(|nf| (nf.name.clone(), qualify_name(&nf.name, &nf.start)))
        .collect();

    // NamespacedFlow内のすべての状態を収集（フロー名 -> 状態名リスト）
    let mut flow_states_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    // フロー名のセットを作成（これらは状態から除外する）
    let flow_names: std::collections::HashSet<String> =
        namespaced_flows.iter().map(|nf| nf.name.clone()).collect();

    for nf in &namespaced_flows {
        let mut states = std::collections::HashSet::new();

        // startを追加（フロー名でない場合のみ）
        if !flow_names.contains(&nf.start) {
            states.insert(nf.start.clone());
        }

        // 遷移からすべての状態を収集（フロー名を除外）
        for transition in &nf.transitions {
            for from in &transition.from {
                if !flow_names.contains(from) {
                    states.insert(from.clone());
                }
            }
            for to in &transition.to {
                if !flow_names.contains(to) {
                    states.insert(to.clone());
                }
            }
        }

        // 修飾名に変換
        let qualified_states: Vec<String> =
            states.iter().map(|s| qualify_name(&nf.name, s)).collect();

        log::info!(
            "Collected states for flow '{}': {:?}",
            nf.name,
            qualified_states
        );
        flow_states_map.insert(nf.name.clone(), qualified_states);
    }

    // ベースフローがあればそれを使用
    if let Some(base) = base_flow {
        // ベースフローのstartがNamespacedFlow名を指している場合、そのstartタイムラインに展開
        let resolved_start = if let Some(flow_start) = flow_start_map.get(&base.start) {
            log::info!(
                "Resolved base flow start '{}' to '{}'",
                base.start,
                flow_start
            );
            flow_start.clone()
        } else {
            base.start.clone()
        };
        global_start = Some(resolved_start);

        // ベースフローの遷移も展開
        for transition in base.transitions {
            // 遷移元がNamespacedFlow名の場合、そのフローのすべての状態からの遷移に展開
            let expanded_from: Vec<String> = transition
                .from
                .iter()
                .flat_map(|from| {
                    if let Some(flow_states) = flow_states_map.get(from) {
                        log::info!(
                            "Expanding transition from flow '{}' to all its states: {:?}",
                            from,
                            flow_states
                        );
                        flow_states.clone()
                    } else {
                        vec![from.clone()]
                    }
                })
                .collect();

            let resolved_to: Vec<FlowTarget> = transition
                .to
                .iter()
                .map(|target| {
                    // 遷移先がNamespacedFlow名の場合、そのstartタイムラインに展開
                    let resolved_timeline =
                        if let Some(flow_start) = flow_start_map.get(&target.timeline) {
                            log::info!(
                                "Resolved transition target '{}' to '{}'",
                                target.timeline,
                                flow_start
                            );
                            flow_start.clone()
                        } else {
                            target.timeline.clone()
                        };
                    FlowTarget {
                        timeline: resolved_timeline,
                        url: target.url.clone(),
                        params: target.params.clone(),
                    }
                })
                .collect();

            all_transitions.push(FlowTransition {
                from: expanded_from,
                to: resolved_to,
            });
        }
    }

    for nf in &namespaced_flows {
        // flow Login { start: Menu, Menu -> [Login, Signup] }
        // を Login::Menu -> [Login::Login, Login::Signup] に展開

        for transition in &nf.transitions {
            let from_qualified: Vec<String> = transition
                .from
                .iter()
                .map(|f| qualify_name(&nf.name, f))
                .collect();

            let to_qualified: Vec<FlowTarget> = transition
                .to
                .iter()
                .map(|t| {
                    // 遷移先がフロー名の場合、そのstartタイムラインに展開
                    let resolved_timeline = if let Some(flow_start) = flow_start_map.get(t) {
                        log::info!(
                            "Resolved namespaced transition target '{}' to '{}'",
                            t,
                            flow_start
                        );
                        flow_start.clone()
                    } else {
                        qualify_name(&nf.name, t)
                    };
                    FlowTarget {
                        timeline: resolved_timeline,
                        url: None,
                        params: std::collections::HashMap::new(),
                    }
                })
                .collect();

            all_transitions.push(FlowTransition {
                from: from_qualified,
                to: to_qualified,
            });
        }

        // グローバルstartがまだ設定されていなければ、最初のNamespacedFlowのstartを使用
        if global_start.is_none() {
            global_start = Some(qualify_name(&nf.name, &nf.start));
        }
    }

    let start = global_start.ok_or("start定義が見つかりません")?;

    let flow = Flow {
        start,
        start_url: None,
        transitions: all_transitions,
    };

    Ok((flow.normalize(), timelines))
}

/// 名前空間を考慮した名前の修飾
/// - "Login" (ローカル名) -> "NamespaceName::Login"
/// - "Other::Timeline" (既に修飾済み) -> "Other::Timeline"
fn qualify_name(namespace: &str, name: &str) -> String {
    if name.contains("::") {
        // 既に修飾されている
        name.to_string()
    } else {
        // ローカル名なので修飾する
        format!("{}::{}", namespace, name)
    }
}
