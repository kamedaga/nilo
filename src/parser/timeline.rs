// ========================================
// タイムラインパーサーモジュール
// ========================================
//
// このモジュールはタイムライン定義とwhenブロックの解析を担当します。

use pest::iterators::Pair;
use crate::parser::ast::*;
use crate::parser::utils::unquote;
use crate::parser::expr::parse_event_expr;
use crate::parser::parse::Rule;

// view_nodeのパース関数は循環参照を避けるため、後で定義される
// この関数は view_node モジュールで定義される
use crate::parser::view_node::parse_view_node;

/// タイムライン定義をパースする
pub fn parse_timeline_def(pair: Pair<Rule>) -> Timeline {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let mut url_pattern: Option<String> = None;
    let mut font: Option<String> = None;
    let mut body: Vec<WithSpan<ViewNode>> = Vec::new();
    let mut whens = Vec::new(); // whenイベントを正しく解析するように修正

    for node_pair in inner {
        match node_pair.as_rule() {
            Rule::timeline_url => {
                // timeline_url: タイムラインのURLパターンを解析
                let url_str = node_pair.into_inner().next().unwrap().as_str();
                url_pattern = Some(unquote(url_str));
            }
            Rule::timeline_config => {
                // timeline_config: 今は無視（将来の拡張用）
            }
            Rule::font_def => {
                // font: "fonts/font" の形式を解析
                let font_str = node_pair.into_inner().next().unwrap().as_str();
                font = Some(unquote(font_str));
            }
            Rule::view_nodes => {
                // view_nodesラッパーを剥がして個別のノードを処理
                for p in node_pair.into_inner() {
                    match p.as_rule() {
                        Rule::when_block => {
                            // whenイベントを解析
                            whens.push(parse_when_block(p));
                        }
                        _ => {
                            body.push(parse_view_node(p));
                        }
                    }
                }
            }
            Rule::when_block => {
                // 直接のwhenブロックを解析
                whens.push(parse_when_block(node_pair));
            }
            _ => {
                body.push(parse_view_node(node_pair));
            },
        }
    }
    log::info!("Creating timeline '{}' with {} when blocks, url_pattern: {:?}", name, whens.len(), url_pattern);

    Timeline { name, url_pattern, font, body, whens }
}

/// Whenブロック（イベントハンドラー）の解析
pub fn parse_when_block(pair: Pair<Rule>) -> When {
    let mut inner = pair.into_inner();
    let event = parse_event_expr(inner.next().unwrap());

    let mut actions = Vec::new();
    for p in inner {
        match p.as_rule() {
            Rule::view_nodes => {
                for vn in p.into_inner() {
                    actions.push(parse_view_node(vn));
                }
            }
            _ => actions.push(parse_view_node(p)),
        }
    }

    When { event, actions }
}
