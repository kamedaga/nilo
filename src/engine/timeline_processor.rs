// ========================================
// Timeline Processor Module
// ========================================
//
// このモジュールは、timelineの処理を3つの明確なフェーズに分離します:
//
// 1. **ロジック処理フェーズ** (timeline初期化時のみ実行)
//    - ローカル変数の宣言 (let, const)
//    - 条件分岐 (if, match)
//    - ループ (foreach)
//    - これらの処理結果は「処理済みノードツリー」として保持
//
// 2. **Dynamic Section更新フェーズ** (毎フレーム実行)
//    - dynamic_section内のロジックを再評価
//    - 状態変化を検知して必要な部分のみ更新
//
// 3. **レイアウト・描画フェーズ** (必要な時のみ実行)
//    - 処理済みノードツリーからレイアウトを計算
//    - 差分表示の適用
//    - ステンシルへの変換と描画

use crate::engine::state::AppState;
use crate::parser::ast::{App, ViewNode, WithSpan};
use std::collections::HashMap;

/// 処理済みノードツリー
/// ロジック処理済みで、レイアウト計算の準備ができた状態
#[derive(Debug, Clone)]
pub struct ProcessedNode {
    pub node: ViewNode,
    pub style: Option<crate::parser::ast::Style>,
    pub line: usize,
    pub column: usize,
}

impl ProcessedNode {
    fn from_with_span(node: WithSpan<ViewNode>) -> Self {
        Self {
            node: node.node,
            style: node.style,
            line: node.line,
            column: node.column,
        }
    }

    fn to_with_span(self) -> WithSpan<ViewNode> {
        WithSpan {
            node: self.node,
            style: self.style,
            line: self.line,
            column: self.column,
        }
    }
}

/// Timeline処理のコンテキスト
/// 処理済みのノードツリーと、dynamic_section用のキャッシュを保持
#[derive(Debug, Clone)]
pub struct TimelineContext {
    /// 処理済みノードツリー（ロジック処理完了後）
    pub processed_nodes: Vec<ProcessedNode>,

    /// dynamic_sectionのキャッシュ (section_name -> 処理済みノード)
    pub dynamic_cache: HashMap<String, Vec<ProcessedNode>>,

    /// 前回の状態ハッシュ（変更検知用）
    pub state_hash: u64,
}

impl TimelineContext {
    pub fn new() -> Self {
        Self {
            processed_nodes: Vec::new(),
            dynamic_cache: HashMap::new(),
            state_hash: 0,
        }
    }
}

/// Timeline Processor
/// timelineの各フェーズを担当
pub struct TimelineProcessor;

impl TimelineProcessor {
    /// フェーズ1: ロジック処理（timeline初期化時のみ）
    ///
    /// このフェーズでは以下を実行:
    /// - ローカル変数の宣言と初期化 (let, const)
    /// - 条件分岐の評価 (if, match)
    /// - ループの展開 (foreach) - 初期状態のみ
    /// - dynamic_sectionはマーカーとして残す（フェーズ2で処理）
    pub fn process_logic<S>(
        nodes: &[WithSpan<ViewNode>],
        state: &mut AppState<S>,
    ) -> Vec<ProcessedNode>
    where
        S: crate::engine::state::StateAccess + 'static,
    {
        let mut result = Vec::new();

        for node in nodes {
            match &node.node {
                // ローカル変数宣言: 一度だけ実行して値を設定
                ViewNode::LetDecl {
                    name,
                    value,
                    mutable,
                    declared_type: _,
                } => {
                    let v = state.eval_expr_from_ast(value);

                    if *mutable {
                        state.component_context.set_local_var(name.clone(), v);
                        log::debug!(
                            "  [Logic] Initialized mutable variable '{}' at timeline load",
                            name
                        );
                    } else {
                        state.component_context.set_const_var(name.clone(), v);
                        log::debug!(
                            "  [Logic] Initialized const variable '{}' at timeline load",
                            name
                        );
                    }

                    // LetDeclノードは処理済みツリーには含めない（既に評価済み）
                }

                // 条件分岐: timeline初期化時に評価
                ViewNode::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    let condition_result = state.eval_expr_from_ast(condition);
                    let is_true =
                        matches!(condition_result.as_str(), "true" | "1" | "True" | "TRUE")
                            || condition_result.parse::<f32>().unwrap_or(0.0) != 0.0;

                    let chosen_branch = if is_true {
                        then_body
                    } else {
                        else_body.as_ref().map(|v| v.as_slice()).unwrap_or(&[])
                    };

                    // 選ばれた分岐を再帰的に処理
                    let processed_branch = Self::process_logic(chosen_branch, state);
                    result.extend(processed_branch);
                }

                // foreach: timeline初期化時は展開せず、マーカーとして保持
                // レイアウト時に実際の値で展開する
                ViewNode::ForEach {
                    var,
                    iterable,
                    body,
                } => {
                    // foreachノードをそのまま保持（レイアウト時に処理）
                    result.push(ProcessedNode {
                        node: ViewNode::ForEach {
                            var: var.clone(),
                            iterable: iterable.clone(),
                            body: body.clone(),
                        },
                        style: node.style.clone(),
                        line: node.line,
                        column: node.column,
                    });
                }

                // dynamic_section: マーカーとして保持（フェーズ2で処理）
                ViewNode::DynamicSection { name, body } => {
                    result.push(ProcessedNode {
                        node: ViewNode::DynamicSection {
                            name: name.clone(),
                            body: body.clone(),
                        },
                        style: node.style.clone(),
                        line: node.line,
                        column: node.column,
                    });
                }

                // コンテナノード: 子を再帰的に処理
                ViewNode::VStack(children) => {
                    let processed_children = Self::process_logic(children, state);
                    result.push(ProcessedNode {
                        node: ViewNode::VStack(
                            processed_children
                                .iter()
                                .map(|p| p.clone().to_with_span())
                                .collect(),
                        ),
                        style: node.style.clone(),
                        line: node.line,
                        column: node.column,
                    });
                }

                ViewNode::HStack(children) => {
                    let processed_children = Self::process_logic(children, state);
                    result.push(ProcessedNode {
                        node: ViewNode::HStack(
                            processed_children
                                .iter()
                                .map(|p| p.clone().to_with_span())
                                .collect(),
                        ),
                        style: node.style.clone(),
                        line: node.line,
                        column: node.column,
                    });
                }

                // その他のノード: そのまま保持
                _ => {
                    result.push(ProcessedNode::from_with_span(node.clone()));
                }
            }
        }

        result
    }

    /// フェーズ2: Dynamic Section更新（毎フレーム）
    ///
    /// このフェーズでは以下を実行:
    /// - dynamic_section内のロジックを再評価
    /// - 状態変化を検知して必要な部分のみ更新
    pub fn update_dynamic_sections<S>(context: &mut TimelineContext, state: &mut AppState<S>)
    where
        S: crate::engine::state::StateAccess + 'static,
    {
        // dynamic_sectionを探索して更新
        Self::update_dynamic_in_tree(
            &mut context.processed_nodes,
            &mut context.dynamic_cache,
            state,
        );
    }

    fn update_dynamic_in_tree<S>(
        nodes: &mut [ProcessedNode],
        cache: &mut HashMap<String, Vec<ProcessedNode>>,
        state: &mut AppState<S>,
    ) where
        S: crate::engine::state::StateAccess + 'static,
    {
        for node in nodes.iter_mut() {
            match &mut node.node {
                ViewNode::DynamicSection { name, body } => {
                    // dynamic_section内のロジックを再評価
                    let updated = Self::process_logic(body, state);
                    cache.insert(name.clone(), updated.clone());

                    // ノードツリーを更新
                    node.node = ViewNode::DynamicSection {
                        name: name.clone(),
                        body: updated.iter().map(|p| p.clone().to_with_span()).collect(),
                    };
                }

                ViewNode::VStack(children) => {
                    let mut child_nodes: Vec<ProcessedNode> = children
                        .iter()
                        .map(|c| ProcessedNode::from_with_span(c.clone()))
                        .collect();
                    Self::update_dynamic_in_tree(&mut child_nodes, cache, state);
                    node.node = ViewNode::VStack(
                        child_nodes
                            .iter()
                            .map(|p| p.clone().to_with_span())
                            .collect(),
                    );
                }

                ViewNode::HStack(children) => {
                    let mut child_nodes: Vec<ProcessedNode> = children
                        .iter()
                        .map(|c| ProcessedNode::from_with_span(c.clone()))
                        .collect();
                    Self::update_dynamic_in_tree(&mut child_nodes, cache, state);
                    node.node = ViewNode::HStack(
                        child_nodes
                            .iter()
                            .map(|p| p.clone().to_with_span())
                            .collect(),
                    );
                }

                _ => {}
            }
        }
    }

    /// フェーズ3: レイアウト計算の準備
    ///
    /// 処理済みノードツリーをWithSpan<ViewNode>形式に変換
    /// これを既存のレイアウトシステムに渡す
    pub fn prepare_for_layout(nodes: &[ProcessedNode]) -> Vec<WithSpan<ViewNode>> {
        nodes.iter().map(|n| n.clone().to_with_span()).collect()
    }
}

/// コンポーネント展開（軽量版）
/// engine.rsの関数を呼び出す
pub fn expand_component_calls_lightweight<S>(
    nodes: &[WithSpan<ViewNode>],
    app: &App,
    state: &mut AppState<S>,
) -> Vec<WithSpan<ViewNode>>
where
    S: crate::engine::state::StateAccess + 'static,
{
    crate::engine::core::component::expand_component_calls_lightweight(nodes, app, state)
}
