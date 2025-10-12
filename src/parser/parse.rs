// ========================================
// Nilo言語パーサーモジュール (リファクタリング済み)
// ========================================
//
// このモジュールはNilo言語の構文解析を担当します。
// Pestパーサーを使用してASTを構築し、各種ノードの解析を行います。
//
// リファクタリング済み: 各機能は以下のモジュールに分割されています:
// - utils: ユーティリティ関数（文字列処理、型変換など）
// - expr: 式の解析
// - flow: フロー定義の解析
// - timeline: タイムライン定義の解析
// - component: コンポーネント定義の解析
// - view_node: ビューノードの解析
// - style: スタイル式の評価
// - namespace: 名前空間の展開
// - types: 型推論とチェック

use pest::Parser;
use pest_derive::Parser;
use log;

use crate::parser::ast::*;

// モジュール化された関数をインポート
pub use super::flow::parse_flow_def;
pub use super::timeline::parse_timeline_def;
pub use super::component::parse_component_def;

use super::namespace::{parse_namespace_def, parse_namespaced_flow_def, expand_namespaced_structures};

// ========================================
// Pestパーサー定義
// ========================================

/// Nilo言語のメインパーサー
/// grammar.pestファイルで定義された構文規則を使用
#[derive(Parser)]
#[grammar = "parser/grammar.pest"]
pub struct NiloParser;

// Rule型を公開（他のモジュールから使用できるように）
pub use pest::iterators::Pair;
pub type ParseRule = Rule;

// ========================================
// メイン解析関数
// ========================================

/// Niloソースコードを解析してAppASTを生成する
///
/// # 引数
/// * `source` - 解析対象のソースコード文字列
///
/// # 戻り値
/// * `Ok(App)` - 解析成功時のAST
/// * `Err(String)` - 解析エラー時のエラーメッセージ
///
/// # 処理フロー
/// 1. Pestパーサーでソースコードを字句解析・構文解析
/// 2. フロー定義、タイムライン定義、コンポーネント定義を抽出
/// 3. 名前空間定義を展開
/// 4. App ASTを構築して返す
pub fn parse_nilo(source: &str) -> Result<App, String> {
    log::debug!("🔍 PARSE DEBUG: Starting to parse nilo file, length: {} chars", source.len());

    // Pestパーサーでファイル全体を解析
    let mut pairs = NiloParser::parse(Rule::file, source)
        .map_err(|e| format!("構文解析エラー: {}", e))?;
    
    let file_pair = pairs.next().expect("ファイルペアが見つかりません");
    assert_eq!(file_pair.as_rule(), Rule::file);

    // 各定義を格納する変数を初期化
    let mut flow: Option<Flow> = None;
    let mut timelines = Vec::new();
    let mut components = Vec::new();
    let mut namespaced_flows = Vec::new();
    let mut namespaces = Vec::new();

    // ファイル内の各定義を解析
    for pair in file_pair.into_inner() {
        match pair.as_rule() {
            Rule::flow_def => {
                // フロー定義は1つまで
                if flow.is_some() {
                    return Err("フロー定義は1つまでしか許可されていません".into());
                }
                flow = Some(parse_flow_def(pair)?);
            }
            Rule::namespaced_flow_def => {
                namespaced_flows.push(parse_namespaced_flow_def(pair)?);
            }
            Rule::namespace_def => {
                let namespace = parse_namespace_def(pair)?;
                namespaces.push(namespace);
            }
            Rule::timeline_def => {
                timelines.push(parse_timeline_def(pair));
            }
            Rule::component_def => {
                let component = parse_component_def(pair);
                components.push(component);
            }
            _ => {} // その他のルールは無視
        }
    }

    // 名前空間とNamespacedFlowを展開して平坦化
    if !namespaces.is_empty() || !namespaced_flows.is_empty() {
        let (expanded_flow, expanded_timelines) = expand_namespaced_structures(
            namespaced_flows, 
            namespaces, 
            timelines,
            flow
        )?;
        flow = Some(expanded_flow);
        timelines = expanded_timelines;
    }

    // フロー定義は必須
    let flow = flow.ok_or_else(|| "フロー定義が見つかりません".to_string())?;
    
    log::debug!("✅ PARSE DEBUG: Successfully parsed nilo file");
    log::debug!("   - Flow start: {}", flow.start);
    log::debug!("   - Timelines: {}", timelines.len());
    log::debug!("   - Components: {}", components.len());
    
    Ok(App { flow, timelines, components })
}

// ========================================
// 後方互換性のための再エクスポート
// ========================================
// 他のモジュールから直接使用されている可能性がある関数を再エクスポート

// ユーティリティ関数
pub use super::utils::{
    unquote,
    process_escape_sequences,
    color_from_expr,
    edges_from_expr,
    size_from_expr,
};

// 式解析関数
pub use super::expr::{
    parse_expr,
    parse_calc_expr,
    parse_comparison_expr,
    parse_condition_string,
};

// フロー関連の解析関数
pub use super::flow::{
    parse_flow_target,
    parse_transition_def,
};

// タイムライン関連の解析関数
pub use super::timeline::{
    parse_when_block,
};

// コンポーネント関連の解析関数
pub use super::component::{
    parse_typed_param,
    parse_optional_param,
    parse_enum_param,
    parse_param_type,
};

// ビューノード解析関数
pub use super::view_node::{
    parse_view_node,
    parse_slot_node,
    parse_text,
    parse_button,
    parse_vstack_node,
    parse_hstack_node,
    parse_component_call,
    parse_dynamic_section,
    parse_match_block,
};

// スタイル関連の解析関数
pub use super::style::{
    style_from_expr,
    eval_calc_expr,
};

// 型関連の解析関数
pub use super::types::{
    infer_expr_type,
    make_typed_expr,
    check_type_compatibility,
};

// 名前空間関連の解析関数（内部使用のためpubで再エクスポートしない）
// expand_namespaced_structures, parse_namespace_defはparse_nilo内でのみ使用

// ========================================
// テスト
// ========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_flow() {
        let source = r#"
            flow {
                start: TopTimeline
                TopTimeline -> NextTimeline
            }

            timeline TopTimeline {
            }

            timeline NextTimeline {
            }
        "#;

        let result = parse_nilo(source);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
        
        let app = result.unwrap();
        assert_eq!(app.flow.start, "TopTimeline");
        assert_eq!(app.flow.transitions.len(), 1);
        assert_eq!(app.timelines.len(), 2);
    }

    #[test]
    fn test_parse_with_component() {
        let source = r#"
            component CustomButton(label: string) {
                Button(btn_id, label)
            }

            flow {
                start: TopTimeline
            }

            timeline TopTimeline {
            }
        "#;

        let result = parse_nilo(source);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
        
        let app = result.unwrap();
        assert_eq!(app.components.len(), 1);
        assert_eq!(app.components[0].name, "CustomButton");
    }

    #[test]
    fn test_parse_with_timeline() {
        let source = r#"
            flow {
                start: TopTimeline
            }

            timeline TopTimeline {
                when user.click(reset_btn) {
                    set count: Number = 0
                }
            }
        "#;

        let result = parse_nilo(source);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
        
        let app = result.unwrap();
        assert_eq!(app.timelines.len(), 1);
        assert_eq!(app.timelines[0].name, "TopTimeline");
    }

    #[test]
    fn test_parse_error_no_flow() {
        let source = r#"
            component TestComponent {
                Text("No flow defined")
            }
        "#;

        let result = parse_nilo(source);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("フロー定義が見つかりません"));
    }

    #[test]
    fn test_parse_error_multiple_flows() {
        let source = r#"
            flow {
                start: Scene1
            }

            timeline Scene1 {
            }

            flow {
                start: Scene2
            }

            timeline Scene2 {
            }
        "#;

        let result = parse_nilo(source);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("フロー定義は1つまで"));
    }
}
