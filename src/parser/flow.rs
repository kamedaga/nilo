// ========================================
// フローパーサーモジュール
// ========================================
//
// このモジュールはフロー定義と遷移の解析を担当します。

use crate::parser::ast::*;
use crate::parser::parse::Rule;
use crate::parser::utils::unquote;
use pest::iterators::Pair;

/// フロー定義を解析してFlowASTを生成
pub fn parse_flow_def(pair: Pair<Rule>) -> Result<Flow, String> {
    assert_eq!(pair.as_rule(), Rule::flow_def);

    let mut start = None;
    let mut start_url = None;
    let mut transitions = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::start_def => {
                // 開始状態の定義を取得（URL対応）
                let start_inner = inner.into_inner().next().unwrap();
                match start_inner.as_rule() {
                    Rule::timeline_with_url => {
                        let (timeline, url) = parse_timeline_with_url(start_inner)?;
                        start = Some(timeline);
                        start_url = Some(url);
                    }
                    Rule::qualified_ident => {
                        start = Some(start_inner.as_str().to_string());
                        start_url = None;
                    }
                    _ => return Err("Invalid start definition".to_string()),
                }
            }
            Rule::transition_def => {
                // 遷移定義を実際に解析
                let transition = parse_transition_def(inner)?;
                transitions.push(transition);
            }
            _ => {}
        }
    }

    // バリデーション
    let start = start.ok_or_else(|| "フロー定義にはstart:が必要です".to_string())?;

    // 配列形式の遷移元を展開して正規化
    let flow = Flow {
        start,
        start_url,
        transitions,
    };
    Ok(flow.normalize())
}

/// タイムライン with URL の解析
pub fn parse_timeline_with_url(pair: Pair<Rule>) -> Result<(String, String), String> {
    assert_eq!(pair.as_rule(), Rule::timeline_with_url);

    let mut inner = pair.into_inner();
    let timeline = inner
        .next()
        .ok_or("timeline_with_urlにタイムライン名がありません")?
        .as_str()
        .to_string();
    let url_str = inner
        .next()
        .ok_or("timeline_with_urlにURL文字列がありません")?
        .as_str();
    let url = unquote(url_str);

    Ok((timeline, url))
}

/// フローターゲットの解析
pub fn parse_flow_target(pair: Pair<Rule>) -> Result<FlowTarget, String> {
    match pair.as_rule() {
        Rule::flow_target => {
            // flow_target ルールの場合、内部のルールを解析
            let inner = pair.into_inner().next().ok_or("flow_targetが空です")?;
            match inner.as_rule() {
                Rule::timeline_with_url => {
                    let (timeline, url) = parse_timeline_with_url(inner)?;
                    Ok(FlowTarget {
                        timeline,
                        url: Some(url),
                        params: std::collections::HashMap::new(),
                    })
                }
                Rule::qualified_ident => Ok(FlowTarget {
                    timeline: inner.as_str().to_string(),
                    url: None,
                    params: std::collections::HashMap::new(),
                }),
                _ => Err(format!(
                    "Unknown flow target inner rule: {:?}",
                    inner.as_rule()
                )),
            }
        }
        Rule::timeline_with_url => {
            let (timeline, url) = parse_timeline_with_url(pair)?;
            Ok(FlowTarget {
                timeline,
                url: Some(url),
                params: std::collections::HashMap::new(),
            })
        }
        Rule::qualified_ident => Ok(FlowTarget {
            timeline: pair.as_str().to_string(),
            url: None,
            params: std::collections::HashMap::new(),
        }),
        _ => Err(format!("Unknown flow target rule: {:?}", pair.as_rule())),
    }
}

/// 遷移定義を解析する新しい関数
pub fn parse_transition_def(pair: Pair<Rule>) -> Result<FlowTransition, String> {
    assert_eq!(pair.as_rule(), Rule::transition_def);

    let mut inner = pair.into_inner();

    // 遷移元の解析
    let source_pair = inner.next().ok_or("遷移定義に遷移元がありません")?;
    let from = parse_transition_source(source_pair)?;

    // 遷移先の解析
    let target_pair = inner.next().ok_or("遷移定義に遷移先がありません")?;
    let to = parse_transition_targets_new(target_pair)?;

    Ok(FlowTransition { from, to })
}

/// 遷移元の解析
fn parse_transition_source(pair: Pair<Rule>) -> Result<Vec<String>, String> {
    assert_eq!(pair.as_rule(), Rule::transition_source);

    let inner = pair.into_inner();
    let mut sources = Vec::new();

    for ident_pair in inner {
        if ident_pair.as_rule() == Rule::qualified_ident {
            sources.push(ident_pair.as_str().to_string());
        }
    }

    // 単一要素の場合と配列の場合の両方に対応
    if sources.is_empty() {
        return Err("transition_sourceに識別子がありません".to_string());
    }

    Ok(sources)
}

/// 遷移先の解析（新しいFlowTarget対応）
fn parse_transition_targets_new(pair: Pair<Rule>) -> Result<Vec<FlowTarget>, String> {
    assert_eq!(pair.as_rule(), Rule::transition_targets);

    let mut targets = Vec::new();
    for target_pair in pair.into_inner() {
        if target_pair.as_rule() == Rule::flow_target {
            targets.push(parse_flow_target(target_pair)?);
        }
    }

    if targets.is_empty() {
        return Err("transition_targetsにターゲットがありません".to_string());
    }

    Ok(targets)
}

/// 遷移先の解析（旧式・互換性維持）
#[allow(dead_code)]
fn parse_transition_targets(pair: Pair<Rule>) -> Result<Vec<String>, String> {
    match pair.as_rule() {
        Rule::qualified_ident => {
            // 単一の遷移先
            Ok(vec![pair.as_str().to_string()])
        }
        _ => {
            // 配列形式の遷移先 [target1, target2, ...]
            let mut targets = Vec::new();
            for ident_pair in pair.into_inner() {
                if ident_pair.as_rule() == Rule::qualified_ident {
                    targets.push(ident_pair.as_str().to_string());
                }
            }
            Ok(targets)
        }
    }
}
