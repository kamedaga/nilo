// ========================================
// Nilo言語パーサーモジュール
// ========================================
//
// このモジュールはNilo言語の構文解析を担当します。
// Pestパーサーを使用してASTを構築し、各種ノードの解析を行います。

use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;
use log;

use crate::parser::ast::*;
use crate::stencil::stencil::Stencil;

// ========================================
// ユーティリティ関数
// ========================================

fn unquote(s: &str) -> String {
    let trimmed = s.trim();
    let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"')) ||
        (trimmed.starts_with('「') && trimmed.ends_with('」')) {
        &trimmed[1..trimmed.len()-1]
    } else {
        trimmed
    };
    
    // エスケープシーケンスを処理
    process_escape_sequences(unquoted)
}

/// エスケープシーケンスを処理する関数
fn process_escape_sequences(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next_ch) = chars.next() {
                match next_ch {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    '\\' => result.push('\\'),
                    '"' => result.push('"'),
                    '\'' => result.push('\''),
                    _ => {
                        // 認識できないエスケープシーケンスはそのまま
                        result.push('\\');
                        result.push(next_ch);
                    }
                }
            } else {
                result.push('\\');
            }
        } else {
            result.push(ch);
        }
    }
    
    result
}

/// 式から色の値を生成する関数
fn color_from_expr(expr: &Expr) -> Option<ColorValue> {
    match expr {
        Expr::String(s) => {
            // HEX色文字列をパース
            if s.starts_with('#') {
                Some(ColorValue::Hex(s.clone()))
            } else {
                // 名前付き色の処理
                match s.to_lowercase().as_str() {
                    "red" => Some(ColorValue::Rgba([1.0, 0.0, 0.0, 1.0])),
                    "green" => Some(ColorValue::Rgba([0.0, 1.0, 0.0, 1.0])),
                    "blue" => Some(ColorValue::Rgba([0.0, 0.0, 1.0, 1.0])),
                    "white" => Some(ColorValue::Rgba([1.0, 1.0, 1.0, 1.0])),
                    "black" => Some(ColorValue::Rgba([0.0, 0.0, 0.0, 1.0])),
                    "transparent" => Some(ColorValue::Rgba([0.0, 0.0, 0.0, 0.0])),
                    _ => Some(ColorValue::Hex(s.clone()))
                }
            }
        }
        Expr::Array(vals) => {
            // RGBA配列をパース
            if vals.len() >= 3 {
                let r = if let Expr::Number(n) = &vals[0] { *n } else { 0.0 };
                let g = if let Expr::Number(n) = &vals[1] { *n } else { 0.0 };
                let b = if let Expr::Number(n) = &vals[2] { *n } else { 0.0 };
                let a = if vals.len() >= 4 {
                    if let Expr::Number(n) = &vals[3] { *n } else { 1.0 }
                } else { 1.0 };
                Some(ColorValue::Rgba([r, g, b, a]))
            } else {
                None
            }
        }
        _ => None
    }
}

/// 式からエッジ値を生成する関数
fn edges_from_expr(expr: &Expr) -> Option<Edges> {
    match expr {
        Expr::Number(n) => Some(Edges::all(*n)),
        Expr::Array(vals) => {
            if vals.len() == 2 {
                // [vertical, horizontal]
                let v = if let Expr::Number(n) = &vals[0] { *n } else { 0.0 };
                let h = if let Expr::Number(n) = &vals[1] { *n } else { 0.0 };
                Some(Edges::vh(v, h))
            } else if vals.len() == 4 {
                // [top, right, bottom, left]
                let top = if let Expr::Number(n) = &vals[0] { *n } else { 0.0 };
                let right = if let Expr::Number(n) = &vals[1] { *n } else { 0.0 };
                let bottom = if let Expr::Number(n) = &vals[2] { *n } else { 0.0 };
                let left = if let Expr::Number(n) = &vals[3] { *n } else { 0.0 };
                Some(Edges { top, right, bottom, left })
            } else {
                None
            }
        }
        Expr::Object(kvs) => {
            let mut edges = Edges::default();
            for (k, v) in kvs {
                if let Expr::Number(n) = v {
                    match k.as_str() {
                        "top" => edges.top = *n,
                        "right" => edges.right = *n,
                        "bottom" => edges.bottom = *n,
                        "left" => edges.left = *n,
                        _ => {}
                    }
                }
            }
            Some(edges)
        }
        _ => None
    }
}

/// 式からサイズを生成する関数
fn size_from_expr(expr: &Expr) -> Option<[f32; 2]> {
    match expr {
        Expr::Array(vals) => {
            if vals.len() >= 2 {
                let w = if let Expr::Number(n) = &vals[0] { *n } else { 0.0 };
                let h = if let Expr::Number(n) = &vals[1] { *n } else { 0.0 };
                Some([w, h])
            } else {
                None
            }
        }
        _ => None
    }
}

// ========================================
// パーサー定義
// ========================================

/// Nilo言語のメインパーサー
/// grammar.pestファイルで定義された構文規則を使用
#[derive(Parser)]
#[grammar = "parser/grammar.pest"]
pub struct NiloParser;

// ========================================
// メイン解析関数
// ========================================

/// Niloソースコードを解析してAppASTを生成する
///
/// * `source` - 解析対象のソースコード文字列
///
/// # 戻り値
/// * `Ok(App)` - 解析成功時のAST
/// * `Err(String)` - 解析エラー時のエラーメッセージ
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
    
    
    Ok(App { flow, timelines, components })
}

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
    let flow = Flow { start, start_url, transitions };
    Ok(flow.normalize())
}

/// タイムライン with URL の解析
fn parse_timeline_with_url(pair: Pair<Rule>) -> Result<(String, String), String> {
    assert_eq!(pair.as_rule(), Rule::timeline_with_url);
    
    let mut inner = pair.into_inner();
    let timeline = inner.next().ok_or("timeline_with_urlにタイムライン名がありません")?.as_str().to_string();
    let url_str = inner.next().ok_or("timeline_with_urlにURL文字列がありません")?.as_str();
    let url = unquote(url_str);
    
    Ok((timeline, url))
}

/// フローターゲットの解析
fn parse_flow_target(pair: Pair<Rule>) -> Result<FlowTarget, String> {
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
                Rule::qualified_ident => {
                    Ok(FlowTarget {
                        timeline: inner.as_str().to_string(),
                        url: None,
                        params: std::collections::HashMap::new(),
                    })
                }
                _ => Err(format!("Unknown flow target inner rule: {:?}", inner.as_rule())),
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
        Rule::qualified_ident => {
            Ok(FlowTarget {
                timeline: pair.as_str().to_string(),
                url: None,
                params: std::collections::HashMap::new(),
            })
        }
        _ => Err(format!("Unknown flow target rule: {:?}", pair.as_rule())),
    }
}

/// 遷移定義を解析する新しい関数
fn parse_transition_def(pair: Pair<Rule>) -> Result<FlowTransition, String> {
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

pub fn parse_component_def(pair: Pair<Rule>) -> Component {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();

    let mut params = vec![];
    let mut default_style = None;

    // component_paramsを処理
    if let Some(Rule::component_params) = inner.peek().map(|p| p.as_rule()) {
        let params_pair = inner.next().unwrap();
        for param_pair in params_pair.into_inner() {
            match param_pair.as_rule() {
                Rule::component_param => {
                    let param_inner = param_pair.into_inner().next().unwrap();
                    match param_inner.as_rule() {
                        Rule::style_param => {
                            // スタイル定義: "style" ":" expr
                            let style_inner = param_inner.into_inner();
                            // Rule::style_param = { "style" ~ ":" ~ expr }の構造なので、最後の要素がexpr
                            let mut elements: Vec<_> = style_inner.collect();
                            if let Some(style_expr) = elements.pop() {
                                let parsed_style = style_from_expr(parse_expr(style_expr));
                                default_style = Some(parsed_style);
                            }
                        }
                        // ★ Phase 2: 型付きパラメータ
                        Rule::typed_param => {
                            params.push(parse_typed_param(param_inner));
                        }
                        Rule::optional_param => {
                            params.push(parse_optional_param(param_inner));
                        }
                        Rule::enum_param => {
                            params.push(parse_enum_param(param_inner));
                        }
                        Rule::ident => {
                            // パラメータ名（後方互換）
                            let param_name = param_inner.as_str().to_string();
                            params.push(ComponentParam {
                                name: param_name,
                                param_type: ComponentParamType::Any,
                                default_value: None,
                                optional: false,
                            });
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    let mut font: Option<String> = None;
    let mut body: Vec<WithSpan<ViewNode>> = Vec::new();
    let whens = Vec::new();

    for node_pair in inner {
        match node_pair.as_rule() {
            Rule::font_def => {
                // font: "fonts/font" の形式を解析
                let font_str = node_pair.into_inner().next().unwrap().as_str();
                font = Some(unquote(font_str));
            }
            Rule::view_nodes => {
                for p in node_pair.into_inner() {
                    body.push(parse_view_node(p));
                }
            }
            _ => body.push(parse_view_node(node_pair)),
        }
    }
    // ★ Phase 2: 既存のString形式のparamsをComponentParamに変換は不要（既にComponentParam）
    log::info!("🔍 Parsed component '{}' with {} parameters", name, params.len());
    for (i, param) in params.iter().enumerate() {
        log::info!("  Param {}: name='{}', type={:?}, default={:?}, optional={}", 
                   i, param.name, param.param_type, param.default_value, param.optional);
    }
    Component { name, params, font, default_style, body, whens }
}

/// ★ Phase 2: 型付きパラメータをパース (name: Type = default)
fn parse_typed_param(pair: Pair<Rule>) -> ComponentParam {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let param_type = parse_param_type(inner.next().unwrap());
    let default_value = inner.next().map(parse_expr);
    
    ComponentParam {
        name,
        param_type,
        default_value,
        optional: false,
    }
}

/// ★ Phase 2: オプショナルパラメータをパース (name: Type?)
fn parse_optional_param(pair: Pair<Rule>) -> ComponentParam {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let param_type = parse_param_type(inner.next().unwrap());
    
    ComponentParam {
        name,
        param_type,
        default_value: None,
        optional: true,
    }
}

/// ★ Phase 2: 列挙型パラメータをパース (name: ("a" | "b" | "c") = "a")
fn parse_enum_param(pair: Pair<Rule>) -> ComponentParam {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    
    let mut enum_values = Vec::new();
    let mut default_value = None;
    
    for p in inner {
        match p.as_rule() {
            Rule::string => {
                let val = unquote(p.as_str());
                if default_value.is_none() {
                    enum_values.push(val);
                } else {
                    // 最後のstringはデフォルト値
                    default_value = Some(Expr::String(val));
                }
            }
            _ => {}
        }
    }
    
    // 最後の要素をデフォルト値として取り出す
    if default_value.is_none() && !enum_values.is_empty() {
        let last = enum_values.pop().unwrap();
        default_value = Some(Expr::String(last));
    }
    
    ComponentParam {
        name,
        param_type: ComponentParamType::Enum(enum_values),
        default_value,
        optional: false,
    }
}

/// ★ Phase 2: パラメータ型をパース
fn parse_param_type(pair: Pair<Rule>) -> ComponentParamType {
    match pair.as_str() {
        "Object" | "object" => ComponentParamType::Object,
        "Array" | "array" => ComponentParamType::Array,
        "Function" | "function" => ComponentParamType::Function,
        "String" | "string" => ComponentParamType::String,
        "Number" | "number" => ComponentParamType::Number,
        "Bool" | "bool" => ComponentParamType::Bool,
        _ => ComponentParamType::Any,
    }
}

/// ★ Phase 2: スロットノードをパース (slot content)
fn parse_slot_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    
    let name = pair.into_inner().next().unwrap().as_str().to_string();
    
    WithSpan {
        node: ViewNode::Slot { name },
        line,
        column: col,
        style: None,
    }
}


fn parse_view_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    match pair.as_rule() {
        Rule::stencil_call => {
            WithSpan { node: ViewNode::Stencil(parse_stencil_call(pair)), line, column: col, style: None }
        }
        Rule::text => parse_text(pair),
        Rule::button => parse_button(pair),
        Rule::text_input => parse_text_input(pair), // コメントアウトを解除
        Rule::image => parse_image(pair),
        Rule::vstack_node => parse_vstack_node(pair),
        Rule::hstack_node => parse_hstack_node(pair),
        Rule::rust_call => parse_rust_call(pair),
        Rule::component_call => parse_component_call(pair),
        Rule::slot_node => parse_slot_node(pair),  // ★ Phase 2: スロット
        Rule::dynamic_section => parse_dynamic_section(pair),
        Rule::match_block => parse_match_block(pair),
        Rule::navigate_action => parse_navigate_action(pair),
        // when_blockは削除：タイムライン解析で直接処理
        Rule::spacing_node => {
            let span = pair.as_span();
            let (line, col) = span.start_pos().line_col();

            let text = pair.as_str();
            let node = if text == "SpacingAuto" {
                ViewNode::SpacingAuto
            } else {
                // "Spacing(...)" の場合は値を解析
                let mut it = pair.into_inner();
                if let Some(p) = it.next() {
                    let dimension_value = match p.as_rule() {
                        Rule::dimension_value => {
                            // dimension_valueから実際のDimensionValueを抽出
                            let expr = parse_expr(p);
                            match expr {
                                Expr::Dimension(dim_val) => dim_val,
                                Expr::Number(n) => DimensionValue { value: n, unit: Unit::Px },
                                _ => DimensionValue { value: 12.0, unit: Unit::Px }
                            }
                        },
                        Rule::number => {
                            let v = p.as_str().parse::<f32>().unwrap_or(12.0);
                            DimensionValue { value: v, unit: Unit::Px }
                        },
                        _ => DimensionValue { value: 12.0, unit: Unit::Px }
                    };
                    ViewNode::Spacing(dimension_value)
                } else {
                    ViewNode::SpacingAuto
                }
            };
            WithSpan { node, line, column: col, style: None }
        }
        // 状態操作関連のノード
        Rule::state_set    => parse_state_set(pair),
        Rule::list_append  => parse_list_append(pair),
        Rule::list_insert  => parse_list_insert(pair),
        Rule::list_remove  => parse_list_remove(pair),
        Rule::list_clear   => parse_list_clear(pair),
        Rule::state_toggle => parse_state_toggle(pair),
        Rule::let_decl     => parse_let_decl(pair),
        Rule::const_decl   => parse_const_decl(pair),
        Rule::foreach_node => parse_foreach_node(pair),
        Rule::if_node => parse_if_node(pair),
        Rule::when_block => {
            // when_blockは表示ノードではないため、ダミーのテキストノードとして処理
            // タイムライン解析で直接処理されるべきなので、ここでは無視
            WithSpan {
                node: ViewNode::Text { format: "".to_string(), args: vec![] },
                line,
                column: col,
                style: None
            }
        }
        Rule::font_def => {
            // font定義は表示ノードではないため、ダミーのテキストノードとして処理
            // タイムライン解析で直接処理されるべきなので、ここでは無視
            WithSpan {
                node: ViewNode::Text { format: "".to_string(), args: vec![] },
                line,
                column: col,
                style: None
            }
        }
        _ => unreachable!("不明なview_node: {:?}", pair),
    }
}

// ========================================
// 個別ビューノード解析関数群
// ========================================

/// テキストノードの解析
///
/// 形式: Text("format_string", arg1, arg2, ..., [style: {...}])
fn parse_text(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut it = pair.into_inner();

    let format = unquote(it.next().unwrap().as_str());

    let mut args: Vec<Expr> = Vec::new();
    let mut style: Option<Style> = None;

    for p in it {
        match p.as_rule() {
            Rule::arg_item => {
                let inner = p.into_inner().next().unwrap();
                match inner.as_rule() {
                    Rule::expr => args.push(parse_expr(inner)),
                    Rule::style_arg => {
                        let expr = parse_expr(inner.into_inner().next().unwrap());
                        style = Some(style_from_expr(expr));
                    }
                    _ => {}
                }
            }
            // 後方互換性のための直接解析
            Rule::expr => args.push(parse_expr(p)),
            Rule::style_arg => {
                let expr = parse_expr(p.into_inner().next().unwrap());
                style = Some(style_from_expr(expr));
            }
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::Text { format, args },
        line, column: col, style
    }
}

/// 形式: Button(id: "button_id", label: "Button Label", [onclick: function!()], [style: {...}])
fn parse_button(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut id: Option<String> = None;
    let mut label: Option<String> = None;
    let mut onclick: Option<Expr> = None;
    let mut style: Option<Style> = None;

    for p in pair.into_inner() {
        match p.as_rule() {
            // 明示的なキーワード引数
            Rule::button_id => { /* 既存の処理 */ }
            Rule::button_label => { /* 既存の処理 */ }

            // 位置引数（後方互換性）
            Rule::ident if id.is_none() => { id = Some(p.as_str().to_string()); }
            Rule::string => {
                // 文字列型の引数を処理
                // idまたはlabelとして解釈
                if id.is_none() {
                    id = Some(unquote(p.as_str()));
                } else if label.is_none() {
                    label = Some(unquote(p.as_str()));
                }
            }

            // rust_call（onclick属性）または expr
            Rule::rust_call => {
                let mut inner = p.into_inner();
                let name = inner.next().unwrap().as_str().to_string();

                let mut args: Vec<Expr> = Vec::new();
                for arg_p in inner {
                    match arg_p.as_rule() {
                        Rule::arg_item => {
                            let mut it = arg_p.into_inner();
                            if let Some(x) = it.next() {
                                if x.as_rule() == Rule::expr {
                                    args.push(parse_expr(x));
                                }
                            }
                        }
                        Rule::expr => args.push(parse_expr(arg_p)),
                        _ => {}
                    }
                }

                onclick = Some(Expr::FunctionCall { name, args });
            }
            
            // expr（onclick属性として）
            Rule::expr if onclick.is_none() => {
                // onclick属性としてexprを受け取る場合
                onclick = Some(parse_expr(p));
            }

            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }

            // arg_item経由のスタイル解析
            Rule::arg_item => {
                let mut it = p.into_inner();
                if let Some(inner) = it.next() {
                    match inner.as_rule() {
                        Rule::style_arg => {
                            style = Some(style_from_expr(parse_expr(inner.into_inner().next().unwrap())));
                        }
                        Rule::expr => {
                            let expr = parse_expr(inner);
                            match expr {
                                Expr::String(s) => {
                                    if id.is_none() {
                                        id = Some(s);
                                    } else if label.is_none() {
                                        label = Some(s);
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    let id = id.expect("ボタンにはid:が必要です");
    let label = label.expect("ボタンにはlabel:が必要です");
    WithSpan { node: ViewNode::Button { id, label, onclick }, line, column: col, style }
}

/// 画像ノードの解析
///
/// 旧仕様: Image(path: "...", x: , y:, width:, height:, scroll:)
/// 新仕様（推奨）: Image("path", [style: { size: [w,h], ... }])
/// パスは必須、その他の属性はスタイルで制御
fn parse_image(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut path: Option<String> = None;
    let mut style: Option<Style> = None;

    for arg in pair.into_inner() {
        match arg.as_rule() {
            Rule::string => { path = Some(unquote(arg.as_str())); }
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(arg.into_inner().next().unwrap())));
            }
            // arg_item経由のスタイル解析
            Rule::arg_item => {
                let mut it = arg.into_inner();
                if let Some(inner) = it.next() {
                    if inner.as_rule() == Rule::style_arg {
                        style = Some(style_from_expr(parse_expr(inner.into_inner().next().unwrap())));
                    } else if inner.as_rule() == Rule::expr {
                        let _ = parse_expr(inner);
                    }
                }
            }

            // 旧仕様との互換性（必要に応じて保持）
            Rule::path_arg => { /* 互換性処理 */ }
            Rule::size_arg => { /* 互換性処理：style.sizeに反映 */ }
            _ => {}
        }
    }

    let path = path.expect("画像にはパスが必要です");
    WithSpan { node: ViewNode::Image { path }, line, column: col, style }
}

///
/// 形式: VStack([style: {...}]) { ... }
fn parse_vstack_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut style: Option<Style> = None;
    let mut children: Vec<WithSpan<ViewNode>> = Vec::new();

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::view_nodes => {
                children = p.into_inner().map(parse_view_node).collect();
            }
            _ => {}
        }
    }

    WithSpan { node: ViewNode::VStack(children), line, column: col, style }
}

fn parse_hstack_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut style: Option<Style> = None;
    let mut children: Vec<WithSpan<ViewNode>> = Vec::new();

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::view_nodes => {
                children = p.into_inner().map(parse_view_node).collect();
            }
            _ => {}
        }
    }

    WithSpan { node: ViewNode::HStack(children), line, column: col, style }
}

/// コンポーネント呼び出しの解析
///
/// 形式: ComponentName(arg1, ..., [style: {...}])
fn parse_component_call(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();

    let mut args: Vec<Expr> = Vec::new();
    let mut style: Option<Style> = None;

    for p in inner {
        match p.as_rule() {
            Rule::arg_item => {
                let mut it = p.into_inner();
                if let Some(x) = it.next() {
                    match x.as_rule() {
                        Rule::style_arg => {
                            let expr = parse_expr(x.into_inner().next().unwrap());
                            style = Some(style_from_expr(expr));
                        }
                        Rule::expr => args.push(parse_expr(x)),
                        _ => {}
                    }
                }
            }
            // 後方互換性
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::expr => args.push(parse_expr(p)),
            _ => {}
        }
    }

    WithSpan { 
        node: ViewNode::ComponentCall { 
            name, 
            args,
            slots: std::collections::HashMap::new(),  // ★ Phase 2: 空のスロットマップ（後で実装）
        }, 
        line, 
        column: col, 
        style 
    }
}

///
/// 形式: dynamic_section section_name ([style: {...}]) { ... }
/// 動的に内容が変更される領域
fn parse_dynamic_section(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut name: Option<String> = None;
    let mut style: Option<Style> = None;
    let mut body: Vec<WithSpan<ViewNode>> = Vec::new();

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::ident => name = Some(p.as_str().to_string()),
            Rule::style_arg => style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap()))),
            Rule::view_nodes => body = p.into_inner().map(parse_view_node).collect(),
            _ => {}
        }
    }

    WithSpan { node: ViewNode::DynamicSection { name: name.unwrap(), body }, line, column: col, style }
}

/// 形式: match <expr> ([style: {...}]) { case value1 { ... } case value2 { ... } default { ... } }
fn parse_match_block(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut expr: Option<Expr> = None;
    let mut style: Option<Style> = None;
    let mut arms: Vec<(Expr, Vec<WithSpan<ViewNode>>)> = Vec::new();
    let mut default: Option<Vec<WithSpan<ViewNode>>> = None;

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::expr => expr = Some(parse_expr(p)),
            Rule::style_arg => style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap()))),
            Rule::match_arm => {
                let mut arm_inner = p.into_inner();
                let case_val = parse_expr(arm_inner.next().unwrap());

                let mut nodes = Vec::new();
                for n in arm_inner {
                    match n.as_rule() {
                        Rule::view_nodes => {
                            for vn in n.into_inner() {
                                nodes.push(parse_view_node(vn));
                            }
                        }
                        _ => nodes.push(parse_view_node(n)),
                    }
                }
                arms.push((case_val, nodes));
            }
            Rule::default_arm => {
                let mut nodes = Vec::new();
                for n in p.into_inner() {
                    match n.as_rule() {
                        Rule::view_nodes => {
                            for vn in n.into_inner() {
                                nodes.push(parse_view_node(vn));
                            }
                        }
                        _ => nodes.push(parse_view_node(n)),
                    }
                }
                default = Some(nodes);
            }
            _ => {}
        }
    }

    WithSpan { node: ViewNode::Match { expr: expr.unwrap(), arms, default }, line, column: col, style }
}

///
/// 形式: navigate_to(TargetState)
/// 指定された状態への遷移アクション
fn parse_navigate_action(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    let target = inner.next().unwrap().as_str().to_string(); // qualified_identに対応
    WithSpan { node: ViewNode::NavigateTo { target }, line, column: col, style: None }
}

/// Whenブロック（イベントハンドラー）の解析
fn parse_when_block(pair: Pair<Rule>) -> When {
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

// ========================================
// イベント/式の解析
// ========================================

fn parse_event_expr(pair: Pair<Rule>) -> EventExpr {
    let mut inner = pair.into_inner();
    let user_event = inner.next().expect("event_exprにuser_eventがありません");
    let mut ev_inner = user_event.into_inner();
    let kind = ev_inner.next().expect("user_eventにevent_kindがありません").as_str();
    let target = ev_inner.next().expect("user_eventにidentがありません").as_str().to_string();
    match kind {
        "click" => EventExpr::ButtonPressed(target),
        _ => panic!("不明なevent_kind: {:?}", kind),
    }
}

/// 計算式（calc_expr）をパースする
/// 例: (100% - 10px) -> CalcExpr(BinaryOp { left: Dimension(100%), op: Sub, right: Dimension(10px) })
fn parse_calc_expr(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut left = parse_calc_term(inner.next().unwrap());
    
    while let Some(op_pair) = inner.next() {
        let op = op_pair.as_str();
        let right = parse_calc_term(inner.next().unwrap());
        
        left = match op {
            "+" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Add,
                right: Box::new(right),
            },
            "-" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Sub,
                right: Box::new(right),
            },
            "*" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Mul,
                right: Box::new(right),
            },
            "/" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Div,
                right: Box::new(right),
            },
            _ => panic!("不明な計算演算子: {}", op),
        };
    }
    
    // 計算式全体をCalcExprでラップして返す
    Expr::CalcExpr(Box::new(left))
}

/// 計算式内の項（数値と単位）をパースする
fn parse_calc_term(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let number_pair = inner.next().unwrap();
    let value: f32 = number_pair.as_str().parse().unwrap();
    
    // 単位があるかチェック
    if let Some(unit_pair) = inner.next() {
        let unit_str = unit_pair.as_str();
        let unit = match unit_str {
            "px" => Unit::Px,
            "vw" => Unit::Vw,
            "vh" => Unit::Vh,
            "ww" => Unit::Ww,
            "wh" => Unit::Wh,
            "%" => Unit::Percent,
            "rem" => Unit::Rem,
            "em" => Unit::Em,
            _ => Unit::Px,
        };
        Expr::Dimension(DimensionValue { value, unit })
    } else {
        Expr::Number(value)
    }
}


fn parse_expr(pair: Pair<Rule>) -> Expr {
    match pair.as_rule() {
        Rule::arg_item => {
            let inner = pair.into_inner().next().expect("arg_item");
            return parse_expr(inner);
        }
        Rule::expr => {
            // exprの中身を直接処理
            let inner = pair.into_inner().next().unwrap();
            parse_expr(inner)
        }

        Rule::string => Expr::String(unquote(pair.as_str())),
        Rule::number => {
            let v: f32 = pair.as_str().parse().unwrap();
            Expr::Number(v)
        }
        Rule::dimension_value => {
            let mut inner = pair.into_inner();
            let first_token = inner.next().unwrap();
            
            match first_token.as_rule() {
                Rule::auto_keyword => {
                    // "auto"キーワードが指定された場合
                    Expr::Dimension(DimensionValue { value: 0.0, unit: Unit::Auto })
                }
                Rule::calc_expr => {
                    // 計算式が指定された場合
                    parse_calc_expr(first_token)
                }
                Rule::number => {
                    // 数値が指定された場合
                    let value: f32 = first_token.as_str().parse().unwrap();

                    // unit_suffixがあるかチェック
                    if let Some(unit_pair) = inner.next() {
                        let unit_str = unit_pair.as_str();
                        let unit = match unit_str {
                            "px" => Unit::Px,
                            "vw" => {
                                log::debug!("🔍 PARSER DEBUG: Found {}vw in parsing", value);
                                Unit::Vw
                            },
                            "vh" => {
                                log::debug!("🔍 PARSER DEBUG: Found {}vh in parsing", value);
                                Unit::Vh
                            },
                            "ww" => {
                                log::debug!("🔍 PARSER DEBUG: Found {}ww in parsing", value);
                                Unit::Ww
                            },
                            "wh" => {
                                log::debug!("🔍 PARSER DEBUG: Found {}wh in parsing", value);
                                Unit::Wh
                            },
                            "%" => Unit::Percent,
                            "rem" => Unit::Rem,
                            "em" => Unit::Em,
                            _ => Unit::Px, // デフォルト
                        };
                        let result = Expr::Dimension(DimensionValue { value, unit });
                        log::debug!("🔍 PARSER DEBUG: Created DimensionValue: {:?}", result);
                        result
                    } else {
                        // ★ 修正: 単位がない場合は純粋な数値として扱う（pxに変換しない）
                        Expr::Number(value)
                    }
                }
                _ => {
                    panic!("Unexpected token in dimension_value: {:?}", first_token.as_rule());
                }
            }
        }
        Rule::bool => Expr::Bool(pair.as_str() == "true"),
        Rule::ident => Expr::Ident(pair.as_str().to_string()),
        Rule::path => Expr::Path(pair.as_str().to_string()),
        Rule::array => {
            let xs = pair.into_inner().map(parse_expr).collect();
            Expr::Array(xs)
        }
        Rule::object => {
            let mut kvs = Vec::new();
            for kv in pair.into_inner() {
                let mut it = kv.into_inner();
                let k_pair = it.next().unwrap();
                
                // キーは識別子または文字列
                let k = match k_pair.as_rule() {
                    Rule::ident => k_pair.as_str().to_string(),
                    Rule::string => unquote(k_pair.as_str()),
                    _ => k_pair.as_str().to_string(),
                };
                
                let v = parse_expr(it.next().unwrap());
                kvs.push((k, v));
            }
            Expr::Object(kvs)
        }
        Rule::match_expr => {
            let mut inner = pair.into_inner();
            let expr = Box::new(parse_expr(inner.next().unwrap()));

            let mut arms = Vec::new();
            let mut default = None;

            for arm_pair in inner {
                match arm_pair.as_rule() {
                    Rule::expr_match_arm => {
                        let mut arm_inner = arm_pair.into_inner();
                        let pattern = parse_expr(arm_inner.next().unwrap());
                        let value = parse_expr(arm_inner.next().unwrap());
                        arms.push(MatchArm { pattern, value });
                    }
                    Rule::expr_default_arm => {
                        let mut default_inner = arm_pair.into_inner();
                        let default_value = parse_expr(default_inner.next().unwrap());
                        default = Some(Box::new(default_value));
                    }
                    _ => {}
                }
            }

            Expr::Match { expr, arms, default }
        }
        _ => {
            // 比較式として解析を試行
            parse_comparison_expr(pair)
        }
    }
}

fn parse_comparison_expr(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut left = parse_arithmetic_expr_direct(inner.next().unwrap());

    while let Some(op_pair) = inner.next() {
        let op = op_pair.as_str();
        let right = parse_arithmetic_expr_direct(inner.next().unwrap());

        left = match op {
            "==" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Eq,
                right: Box::new(right),
            },
            "!=" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Ne,
                right: Box::new(right),
            },
            "<" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Lt,
                right: Box::new(right),
            },
            "<=" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Le,
                right: Box::new(right),
            },
            ">" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Gt,
                right: Box::new(right),
            },
            ">=" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Ge,
                right: Box::new(right),
            },
            _ => panic!("不明な比較演算子: {}", op),
        };
    }

    left
}

fn parse_arithmetic_expr_direct(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut left = parse_term(inner.next().unwrap());

    while let Some(op_pair) = inner.next() {
        let op = op_pair.as_str();
        let right = parse_term(inner.next().unwrap());

        left = match op {
            "+" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Add,
                right: Box::new(right),
            },
            "-" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Sub,
                right: Box::new(right),
            },
            _ => panic!("不明な算術演算: {}", op),
        };
    }

    left
}

fn parse_term(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let mut left = parse_factor(inner.next().unwrap());

    while let Some(op_pair) = inner.next() {
        let op = op_pair.as_str();
        let right = parse_factor(inner.next().unwrap());

        left = match op {
            "*" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Mul,
                right: Box::new(right),
            },
            "/" => Expr::BinaryOp {
                left: Box::new(left),
                op: BinaryOperator::Div,
                right: Box::new(right),
            },
            _ => panic!("不明な乗除演算子: {}", op),
        };
    }

    left
}

fn parse_factor(pair: Pair<Rule>) -> Expr {
    parse_primary(pair.into_inner().next().unwrap())
}

fn parse_primary(pair: Pair<Rule>) -> Expr {
    match pair.as_rule() {
        Rule::match_expr => {
            let mut inner = pair.into_inner();
            let expr = Box::new(parse_expr(inner.next().unwrap()));

            let mut arms = Vec::new();
            let mut default = None;

            for arm_pair in inner {
                match arm_pair.as_rule() {
                    Rule::expr_match_arm => {
                        let mut arm_inner = arm_pair.into_inner();
                        let pattern = parse_expr(arm_inner.next().unwrap());
                        let value = parse_expr(arm_inner.next().unwrap());
                        arms.push(MatchArm { pattern, value });
                    }
                    Rule::expr_default_arm => {
                        let mut default_inner = arm_pair.into_inner();
                        let default_value = parse_expr(default_inner.next().unwrap());
                        default = Some(Box::new(default_value));
                    }
                    _ => {}
                }
            }

            Expr::Match { expr, arms, default }
        }
        Rule::string => Expr::String(unquote(pair.as_str())),
        Rule::dimension_value => {
            let mut inner = pair.into_inner();
            let first_token = inner.next().unwrap();
            
            match first_token.as_rule() {
                Rule::auto_keyword => {
                    // "auto"キーワードが指定された場合
                    Expr::Dimension(DimensionValue { value: 0.0, unit: Unit::Auto })
                }
                Rule::calc_expr => {
                    // 計算式が指定された場合
                    parse_calc_expr(first_token)
                }
                Rule::number => {
                    let value: f32 = first_token.as_str().parse().unwrap();

                    if let Some(unit_pair) = inner.next() {
                        let unit_str = unit_pair.as_str();
                        let unit = match unit_str {
                            "px" => Unit::Px,
                            "vw" => Unit::Vw,
                            "vh" => Unit::Vh,
                            "ww" => Unit::Ww,
                            "wh" => Unit::Wh,
                            "%" => Unit::Percent,
                            "rem" => Unit::Rem,
                            "em" => Unit::Em,
                            _ => Unit::Px,
                        };
                        Expr::Dimension(DimensionValue { value, unit })
                    } else {
                        Expr::Number(value)
                    }
                }
                _ => {
                    panic!("Unexpected token in dimension_value: {:?}", first_token.as_rule());
                }
            }
        }
        Rule::number => {
            let v: f32 = pair.as_str().parse().unwrap();
            Expr::Number(v)
        }
        Rule::bool => Expr::Bool(pair.as_str() == "true"),
        Rule::path => Expr::Path(pair.as_str().to_string()),
        Rule::ident => Expr::Ident(pair.as_str().to_string()),
        // ★ 通常の関数呼び出し（onclick用）
        Rule::function_call => {
            let mut inner = pair.into_inner();
            let name = inner.next().unwrap().as_str().to_string();
            let args = inner.map(parse_expr).collect();
            Expr::FunctionCall { name, args }
        }
        // ★ Phase 2: has_slot() チェック
        Rule::has_slot_check => {
            let slot_name = pair.into_inner().next().unwrap().as_str().to_string();
            Expr::FunctionCall {
                name: "has_slot".to_string(),
                args: vec![Expr::String(slot_name)],
            }
        }
        Rule::array => {
            let xs = pair.into_inner().map(parse_expr).collect();
            Expr::Array(xs)
        }
        Rule::object => {
            let mut kvs = Vec::new();
            for kv in pair.into_inner() {
                let mut it = kv.into_inner();
                let k = it.next().unwrap().as_str().to_string();
                let v = parse_expr(it.next().unwrap());
                kvs.push((k, v));
            }
            Expr::Object(kvs)
        }
        Rule::expr => parse_expr(pair),
        _ => {
            // デフォルトケース：知らないルールを適切に処理
            // 子要素があれば再帰的に処理、なければ文字列として扱う
            let text = pair.as_str().to_string(); // 先に文字列を取得
            let mut inner = pair.into_inner();
            if let Some(child) = inner.next() {
                parse_primary(child)
            } else {
                // 子要素がない場合は文字列として扱う
                Expr::String(text)
            }
        }
    }
}

// ========================================
// ステンシル解析（グラフィックプリミティブ）
// ========================================

/// ステンシル呼び出しの解析
///
/// rect, circle, triangle, text, image, rounded_rect などの
/// 低レベルグラフィック要素を解析
fn parse_stencil_call(pair: Pair<Rule>) -> Stencil {
    let mut inner = pair.into_inner();
    let kind = inner.next().unwrap().as_str(); // rect, circle, ...


    let mut map = std::collections::HashMap::new();

    if let Some(stencil_args) = inner.next() {
        for arg in stencil_args.into_inner() {
            let mut arg_inner = arg.into_inner();
            let key_pair = arg_inner.next().unwrap();
            let key = key_pair.as_str();

            let val_pair = arg_inner.next().unwrap();

            let value = if val_pair.as_rule() == Rule::stencil_value {
                let mut value_inner = val_pair.into_inner();
                if let Some(actual_value) = value_inner.next() {
                    match actual_value.as_rule() {
                        Rule::number => StencilArg::Number(actual_value.as_str().parse().unwrap()),
                        Rule::string => StencilArg::String(unquote(actual_value.as_str())),
                        Rule::bool => StencilArg::Bool(actual_value.as_str() == "true"),
                        Rule::ident => {
                            panic!("ステンシル引数は変数名は使用できません: key={}, value={}", key, actual_value.as_str());
                        }
                        _ => panic!("不明な引数タイプ"),
                    }
                } else {
                    panic!("key: {} stencil_valueの値が見つかりません", key);
                }
            } else {
                match val_pair.as_rule() {
                    Rule::number => StencilArg::Number(val_pair.as_str().parse().unwrap()),
                    Rule::string => StencilArg::String(unquote(val_pair.as_str())),
                    Rule::bool => StencilArg::Bool(val_pair.as_str() == "true"),
                    _ => panic!("不明な直接値タイプ"),
                }
            };

            map.insert(key.to_string(), value);
        }
    }

    macro_rules! get_f32 { ($k:expr, $def:expr) => { map.get($k).and_then(|v| v.as_f32()).unwrap_or($def) } }
    macro_rules! get_str { ($k:expr, $def:expr) => { map.get($k).and_then(|v| v.as_str()).unwrap_or($def).to_string() } }
    macro_rules! get_bool { ($k:expr, $def:expr) => { map.get($k).and_then(|v| v.as_bool()).unwrap_or($def) } }

    let parse_position_value = |key: &str, default: f32| -> f32 {
        map.get(key).and_then(|v| v.as_f32()).unwrap_or(default)
    };

    match kind {
        "rect" => Stencil::Rect {
            position: [parse_position_value("x", 0.0), parse_position_value("y", 0.0)],
            width: get_f32!("width", 0.0),
            height: get_f32!("height", 0.0),
            color: [
                get_f32!("r", 1.0), get_f32!("g", 1.0),
                get_f32!("b", 1.0), get_f32!("a", 1.0),
            ],
            scroll: get_bool!("scroll", true),
            depth: get_f32!("depth", 0.5),
        },
        "circle" => Stencil::Circle {
            center: [parse_position_value("x", 0.0), parse_position_value("y", 0.0)],
            radius: get_f32!("radius", 1.0),
            color: [
                get_f32!("r", 1.0), get_f32!("g", 1.0),
                get_f32!("b", 1.0), get_f32!("a", 1.0),
            ],
            scroll: get_bool!("scroll", true),
            depth: get_f32!("depth", 0.5),
        },
        "triangle" => Stencil::Triangle {
            p1: [parse_position_value("x1", 0.0), parse_position_value("y1", 0.0)],
            p2: [parse_position_value("x2", 0.0), parse_position_value("y2", 0.0)],
            p3: [parse_position_value("x3", 0.0), parse_position_value("y3", 0.0)],
            color: [
                get_f32!("r", 1.0), get_f32!("g", 1.0),
                get_f32!("b", 1.0), get_f32!("a", 1.0),
            ],
            scroll: get_bool!("scroll", true),
            depth: get_f32!("depth", 0.5),
        },
        "text" => Stencil::Text {
            content: get_str!("content", ""),
            position: [parse_position_value("x", 0.0), parse_position_value("y", 0.0)],
            size: get_f32!("size", 16.0),
            color: [
                get_f32!("r", 0.0), get_f32!("g", 0.0),
                get_f32!("b", 0.0), get_f32!("a", 1.0),
            ],
            font: get_str!("font", "sans"),
            max_width: None, // パーサーでは改行制御なし
            scroll: get_bool!("scroll", true),
            depth: get_f32!("depth", 0.1),
        },
        "image" => Stencil::Image {
            position: [parse_position_value("x", 0.0), parse_position_value("y", 0.0)],
            width: get_f32!("width", 0.0),
            height: get_f32!("height", 0.0),
            path: get_str!("path", ""),
            scroll: get_bool!("scroll", true),
            depth: get_f32!("depth", 0.5),
        },
        "rounded_rect" => Stencil::RoundedRect {
            position: [parse_position_value("x", 0.0), parse_position_value("y", 0.0)],
            width: get_f32!("width", 0.0),
            height: get_f32!("height", 0.0),
            radius: get_f32!("radius", 8.0),
            color: [
                get_f32!("r", 1.0), get_f32!("g", 1.0),
                get_f32!("b", 1.0), get_f32!("a", 1.0),
            ],
            scroll: get_bool!("scroll", true),
            depth: get_f32!("depth", 0.5),
        },
        _ => panic!("不明なステンシルタイプ {}", kind),
    }
}

enum StencilArg {
    Number(f32),
    String(String),
    Bool(bool),
}
impl StencilArg {
    fn as_f32(&self) -> Option<f32> {
        match self { StencilArg::Number(f) => Some(*f), _ => None }
    }
    fn as_str(&self) -> Option<&str> {
        match self { StencilArg::String(s) => Some(s), _ => None }
    }
    fn as_bool(&self) -> Option<bool> {
        match self { StencilArg::Bool(b) => Some(*b), _ => None }
    }
}

// ========================================
// 状態操作
// ========================================

fn parse_state_set(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    
    let path = inner.next().unwrap().as_str().to_string();
    
    // 型アノテーションのパース（オプション）
    let mut declared_type: Option<NiloType> = None;
    let mut value_pair = None;
    
    for p in inner {
        match p.as_rule() {
            Rule::type_annotation => {
                // 型アノテーションをパース
                let type_inner = p.into_inner().next().unwrap();
                declared_type = Some(parse_type_expr(type_inner));
            }
            Rule::expr => {
                value_pair = Some(p);
            }
            _ => {}
        }
    }
    
    let value = parse_expr(value_pair.expect("set文に値がありません"));
    
    // 型推論と型チェック
    let inferred_type = infer_expr_type(&value);
    
    // 型アノテーションがある場合は型チェック
    if let Some(expected_type) = &declared_type {
        if !expected_type.is_compatible_with(&inferred_type) {
            // 警告を出力（パースエラーではない）
            eprintln!(
                "[Type Warning] {}:{} - 型の不一致: 変数 '{}' は {} 型ですが、{} 型の値が代入されました",
                line, col, path, expected_type.display(), inferred_type.display()
            );
        }
    }
    
    WithSpan { 
        node: ViewNode::Set { 
            path, 
            value, 
            inferred_type: Some(inferred_type) 
        }, 
        line, 
        column: col, 
        style: None 
    }
}

fn parse_list_append(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();                // path, expr
    let path = inner.next().unwrap().as_str().to_string();
    let value = parse_expr(inner.next().unwrap());
    WithSpan { node: ViewNode::ListAppend { path, value }, line, column: col, style: None }
}

fn parse_list_insert(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();                // path, number, expr
    let path = inner.next().unwrap().as_str().to_string();
    let index = inner.next().unwrap().as_str().parse::<usize>().unwrap();
    let value = parse_expr(inner.next().unwrap());
    WithSpan { node: ViewNode::ListInsert { path, index, value }, line, column: col, style: None }
}

fn parse_list_remove(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();                // path, expr
    let path = inner.next().unwrap().as_str().to_string();
    let value = parse_expr(inner.next().unwrap());
    WithSpan { node: ViewNode::ListRemove { path, value }, line, column: col, style: None }
}

fn parse_list_clear(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();                // path
    let path = inner.next().unwrap().as_str().to_string();
    WithSpan { node: ViewNode::ListClear { path }, line, column: col, style: None }
}

/// let宣言のパース
/// 形式: let variable_name = expression
fn parse_let_decl(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    
    let name = inner.next().unwrap().as_str().to_string();
    
    // 型注釈のパース（オプション）
    let mut declared_type: Option<NiloType> = None;
    let mut value_pair = None;
    
    for p in inner {
        match p.as_rule() {
            Rule::type_annotation => {
                let type_inner = p.into_inner().next().unwrap();
                declared_type = Some(parse_type_expr(type_inner));
            }
            Rule::expr => {
                value_pair = Some(p);
            }
            _ => {}
        }
    }
    
    let value = parse_expr(value_pair.expect("let文に値がありません"));
    
    // 型推論と型チェック
    let inferred_type = infer_expr_type(&value);
    
    // 型注釈がある場合は型チェック
    if let Some(expected_type) = &declared_type {
        if !expected_type.is_compatible_with(&inferred_type) {
            eprintln!(
                "[Type Warning] {}:{} - 型の不一致: 変数 '{}' は {} 型として宣言されていますが、{} 型の値が代入されました",
                line, col, name, expected_type.display(), inferred_type.display()
            );
        }
    }
    
    WithSpan { 
        node: ViewNode::LetDecl { 
            name, 
            value, 
            mutable: true,  // letは可変
            declared_type,
        }, 
        line, 
        column: col, 
        style: None 
    }
}

/// const宣言のパース
/// 形式: const variable_name = expression
fn parse_const_decl(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    
    let name = inner.next().unwrap().as_str().to_string();
    
    // 型注釈のパース（オプション）
    let mut declared_type: Option<NiloType> = None;
    let mut value_pair = None;
    
    for p in inner {
        match p.as_rule() {
            Rule::type_annotation => {
                let type_inner = p.into_inner().next().unwrap();
                declared_type = Some(parse_type_expr(type_inner));
            }
            Rule::expr => {
                value_pair = Some(p);
            }
            _ => {}
        }
    }
    
    let value = parse_expr(value_pair.expect("const文に値がありません"));
    
    // 型推論と型チェック
    let inferred_type = infer_expr_type(&value);
    
    // 型注釈がある場合は型チェック
    if let Some(expected_type) = &declared_type {
        if !expected_type.is_compatible_with(&inferred_type) {
            eprintln!(
                "[Type Warning] {}:{} - 型の不一致: 定数 '{}' は {} 型として宣言されていますが、{} 型の値が代入されました",
                line, col, name, expected_type.display(), inferred_type.display()
            );
        }
    }
    
    WithSpan { 
        node: ViewNode::LetDecl { 
            name, 
            value, 
            mutable: false,  // constは不変
            declared_type,
        }, 
        line, 
        column: col, 
        style: None 
    }
}

fn parse_state_toggle(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();                // ident_path(lhs), ident_path(rhs)
    let lhs = inner.next().unwrap().as_str().to_string();
    let rhs = inner.next().unwrap().as_str().to_string();
    if lhs != rhs {
        panic!("toggle は `state.foo = !state.foo` の形式で同じパスに対して行ってください（lhs={}, rhs={}）", lhs, rhs);
    }
    WithSpan { node: ViewNode::Toggle { path: lhs }, line, column: col, style: None }
}

fn parse_rust_call(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();

    let mut args: Vec<Expr> = Vec::new();
    for arg_p in inner {
        match arg_p.as_rule() {
            Rule::arg_item => {
                let mut it = arg_p.into_inner();
                if let Some(x) = it.next() {
                    if x.as_rule() == Rule::expr {
                        args.push(parse_expr(x));
                    }
                }
            }
            Rule::expr => args.push(parse_expr(arg_p)),
            _ => {}
        }
    }

    WithSpan { node: ViewNode::RustCall { name, args }, line, column: col, style: None }
}

fn parse_text_input(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut id: Option<String> = None;
    let mut placeholder: Option<String> = None;
    let mut style: Option<Style> = None;
    let value: Option<Expr> = None;
    let on_change: Option<Expr> = None;
    let multiline = false;
    let max_length: Option<usize> = None;
    let ime_enabled = true;

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::expr if id.is_none() => {
                // 最初のexprからIDを取得
                let expr = parse_expr(p);
                match expr {
                    Expr::String(s) => { id = Some(s); }
                    Expr::Ident(i) => { id = Some(i); }
                    _ => {
                        log::warn!("TextInputの最初の引数は文字列または識別子である必要があります");
                    }
                }
            }
            Rule::arg_item => {
                let mut inner = p.into_inner();
                if let Some(key_pair) = inner.next() {
                    let key = key_pair.as_str();
                    if let Some(val_pair) = inner.next() {
                        match key {
                            "placeholder" => {
                                let expr = parse_expr(val_pair);
                                if let Expr::String(s) = expr {
                                    placeholder = Some(s);
                                }
                            }
                            "style" => {
                                style = Some(style_from_expr(parse_expr(val_pair)));
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let id = id.expect("TextInputにはidが必要です");
    WithSpan {
        node: ViewNode::TextInput {
            id,
            placeholder,
            value,
            on_change,
            multiline,
            max_length,
            ime_enabled
        },
        line,
        column: col,
        style
    }
}

fn parse_foreach_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut var: Option<String> = None;
    let mut iterable: Option<Expr> = None;
    let mut style: Option<Style> = None;
    let mut body: Vec<WithSpan<ViewNode>> = Vec::new();

    let mut inner = pair.into_inner();

    log::debug!("🔍 FOREACH DEBUG: Parsing foreach node with grammar rules");
    
    // foreach variable の解析
    if let Some(var_pair) = inner.next() {
        if var_pair.as_rule() == Rule::ident {
            var = Some(var_pair.as_str().to_string());
            log::debug!("🔍 FOREACH DEBUG: var = '{}'", var.as_ref().unwrap());
        }
    }
    
    // "in" キーワード（暗黙的にスキップ - 文法で処理済み）
    
    // foreach_iterable の解析
    if let Some(iterable_pair) = inner.next() {
        log::debug!("🔍 FOREACH DEBUG: iterable_pair rule = {:?}, content = '{}'", iterable_pair.as_rule(), iterable_pair.as_str());
        
        match iterable_pair.as_rule() {
            Rule::foreach_iterable => {
                // foreach_iterable内部の実際のpath/identを取得
                let mut iterable_inner = iterable_pair.into_inner();
                if let Some(actual_iterable) = iterable_inner.next() {
                    match actual_iterable.as_rule() {
                        Rule::path => {
                            iterable = Some(Expr::Path(actual_iterable.as_str().to_string()));
                            log::debug!("🔍 FOREACH DEBUG: parsed as path = '{}'", actual_iterable.as_str());
                        }
                        Rule::ident => {
                            iterable = Some(Expr::Ident(actual_iterable.as_str().to_string()));
                            log::debug!("🔍 FOREACH DEBUG: parsed as ident = '{}'", actual_iterable.as_str());
                        }
                        _ => {
                            iterable = Some(parse_expr(actual_iterable));
                            log::debug!("🔍 FOREACH DEBUG: parsed as expr = {:?}", iterable.as_ref().unwrap());
                        }
                    }
                }
            }
            _ => {
                // fallback: 既存の処理
                iterable = Some(parse_expr(iterable_pair));
                log::debug!("🔍 FOREACH DEBUG: fallback parsed iterable = {:?}", iterable.as_ref().unwrap());
            }
        }
    }

    // 残りの要素を処理
    for p in inner {
        log::debug!("🔍 FOREACH DEBUG: processing rule: {:?}, content: '{}'", p.as_rule(), p.as_str());
        match p.as_rule() {
            Rule::foreach_style => {
                // foreach_style内部のstyle_argを取得
                let mut style_inner = p.into_inner();
                if let Some(style_arg) = style_inner.next() {
                    if style_arg.as_rule() == Rule::style_arg {
                        let mut style_arg_inner = style_arg.into_inner();
                        if let Some(expr_pair) = style_arg_inner.next() {
                            style = Some(style_from_expr(parse_expr(expr_pair)));
                            log::debug!("🔍 FOREACH DEBUG: parsed foreach_style = {:?}", style.as_ref().unwrap());
                        }
                    }
                }
            }
            Rule::view_nodes => {
                body = p.into_inner().map(parse_view_node).collect();
                log::debug!("🔍 FOREACH DEBUG: parsed {} view_nodes", body.len());
            }
            _ => {
                log::debug!("🔍 FOREACH DEBUG: ignoring rule: {:?}", p.as_rule());
            }
        }
    }

    let result = WithSpan {
        node: ViewNode::ForEach {
            var: var.expect("foreach variable not found"),
            iterable: iterable.expect("foreach iterable not found"),
            body
        },
        line,
        column: col,
        style
    };

    log::debug!("🔍 FOREACH DEBUG: Final result - var: {:?}, iterable: {:?}, style: {:?}, body_len: {}", 
        result.node.clone(), 
        match &result.node { ViewNode::ForEach { iterable, .. } => Some(iterable), _ => None },
        result.style.as_ref().map(|_| "Some(Style)"),
        match &result.node { ViewNode::ForEach { body, .. } => body.len(), _ => 0 }
    );

    result
}

fn parse_if_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut condition: Option<Expr> = None;
    let mut style: Option<Style> = None;
    let mut then_body: Vec<WithSpan<ViewNode>> = Vec::new();
    let mut else_body: Option<Vec<WithSpan<ViewNode>>> = None;

    let mut inner = pair.into_inner();

    // if condition の解析
    condition = Some(parse_expr(inner.next().unwrap()));

    // 残りの引数を処理
    for p in inner {
        match p.as_rule() {
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::view_nodes => {
                if then_body.is_empty() {
                    then_body = p.into_inner().map(parse_view_node).collect();
                } else {
                    // else部分
                    else_body = Some(p.into_inner().map(parse_view_node).collect());
                }
            }
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::If {
            condition: condition.unwrap(),
            then_body,
            else_body
        },
        line,
        column: col,
        style
    }
}

/// 計算式（CalcExpr）を評価してDimensionValueに変換する
/// 例: CalcExpr(BinaryOp { left: Dimension(100%), op: Sub, right: Dimension(10px) })
///     -> 異なる単位の計算式の場合は実行時評価のためNoneを返す
fn eval_calc_expr(expr: &Expr) -> Option<DimensionValue> {
    match expr {
        Expr::CalcExpr(inner) => {
            // CalcExpr内の式を評価
            eval_calc_expr(inner)
        }
        Expr::Dimension(d) => {
            // そのままDimensionValueを返す
            Some(*d)
        }
        Expr::Number(n) => {
            // 数値の場合はpx単位として扱う
            Some(DimensionValue { value: *n, unit: Unit::Px })
        }
        Expr::BinaryOp { left, op, right } => {
            // 二項演算の場合、左辺と右辺を評価
            let left_dim = eval_calc_expr(left)?;
            let right_dim = eval_calc_expr(right)?;
            
            // 同じ単位の場合のみ計算を実行
            if left_dim.unit == right_dim.unit {
                let result_value = match op {
                    BinaryOperator::Add => left_dim.value + right_dim.value,
                    BinaryOperator::Sub => left_dim.value - right_dim.value,
                    BinaryOperator::Mul => left_dim.value * right_dim.value,
                    BinaryOperator::Div => {
                        if right_dim.value != 0.0 {
                            left_dim.value / right_dim.value
                        } else {
                            0.0
                        }
                    }
                    _ => return None,
                };
                Some(DimensionValue { value: result_value, unit: left_dim.unit })
            } else {
                // 異なる単位の場合は、実行時評価のためNoneを返す
                // レイアウトエンジンで実行時に評価する
                log::debug!("🔧 計算式で異なる単位が使用されています: {:?} と {:?} - 実行時に評価します", left_dim.unit, right_dim.unit);
                None
            }
        }
        _ => None,
    }
}

fn style_from_expr(expr: Expr) -> Style {
    match expr {
        Expr::Object(kvs) => {
            let mut s = Style::default();

            for (k, v) in kvs {
                // ★ 新規追加: match式を含むプロパティを特別処理
                let resolved_value = match &v {
                    Expr::Match { .. } => {
                        // match式は実行時に評価されるため、ここでは既存の値を設定
                        // 実際の評価はAppState::eval_expr_from_astで行われる
                        v.clone()
                    },
                    _ => v.clone()
                };

                // ★ レスポンシブ対応: window.width や window.height を含む条件をチェック
                // 簡易実装: キーが "window.width <= 1000" のようなパターンの場合
                if (k.contains("window.width") || k.contains("window.height")) && 
                   (k.contains("<=") || k.contains(">=") || k.contains("<") || k.contains(">") || k.contains("==")) {
                    eprintln!("🔍 [PARSE] レスポンシブ条件を検出: {}", k);
                    
                    // 条件式を解析してResponsiveRuleを作成
                    if let Some(condition_expr) = parse_condition_string(&k) {
                        eprintln!("   [PARSE] 条件式パース成功: {:?}", condition_expr);
                        
                        if let Expr::Object(_) = &resolved_value {
                            let conditional_style = style_from_expr(resolved_value);
                            eprintln!("   [PARSE] 条件付きスタイルを追加");
                            s.responsive_rules.push(crate::parser::ast::ResponsiveRule {
                                condition: condition_expr,
                                style: Box::new(conditional_style),
                            });
                            continue;
                        } else {
                            eprintln!("   [PARSE] ⚠️ 条件の値がオブジェクトではありません: {:?}", resolved_value);
                        }
                    } else {
                        eprintln!("   [PARSE] ⚠️ 条件式のパースに失敗: {}", k);
                    }
                }

                match k.as_str() {
                    "color"        => s.color        = color_from_expr(&resolved_value),
                    "background"   => s.background   = color_from_expr(&resolved_value),
                    "border_color" => s.border_color = color_from_expr(&resolved_value),
                    "padding"      => s.padding      = edges_from_expr(&resolved_value),
                    "margin"       => s.margin       = edges_from_expr(&resolved_value),
                    "size"         => s.size         = size_from_expr(&resolved_value),

                    // 実際のStyleフィールドに合わせて修正
                    "width" => {
                        if let Some(Expr::Number(w)) = Some(&resolved_value) {
                            s.width = Some(*w);
                        } else if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.relative_width = Some(*d);
                        } else if let Some(Expr::CalcExpr(_)) = Some(&resolved_value) {
                            // CalcExprの場合は、まず静的評価を試みる
                            if let Some(d) = eval_calc_expr(&resolved_value) {
                                s.relative_width = Some(d);
                            } else {
                                // 静的評価に失敗した場合（異なる単位の計算式など）、
                                // 計算式を保存して実行時に評価
                                s.width_expr = Some(resolved_value.clone());
                            }
                        }
                    }
                    "height" => {
                        if let Some(Expr::Number(h)) = Some(&resolved_value) {
                            s.height = Some(*h);
                        } else if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.relative_height = Some(*d);
                        } else if let Some(Expr::CalcExpr(_)) = Some(&resolved_value) {
                            // CalcExprの場合は、まず静的評価を試みる
                            if let Some(d) = eval_calc_expr(&resolved_value) {
                                s.relative_height = Some(d);
                            } else {
                                // 静的評価に失敗した場合（異なる単位の計算式など）、
                                // 計算式を保存して実行時に評価
                                s.height_expr = Some(resolved_value.clone());
                            }
                        }
                    }
                    "font_size" => {
                        if let Some(Expr::Number(fs)) = Some(&resolved_value) {
                            s.font_size = Some(*fs);
                        } else if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.relative_font_size = Some(*d);
                        }
                    }
                    "font" => {
                        if let Some(Expr::String(f)) = Some(&resolved_value) {
                            s.font = Some(f.clone());
                        }
                    }
                    "spacing" => {
                        if let Some(Expr::Number(sp)) = Some(&resolved_value) {
                            s.spacing = Some(*sp);
                        } else if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.relative_spacing = Some(*d);
                        }
                    }
                    "gap" => {
                        // gap プロパティは spacing と同じ機能を提供
                        if let Some(Expr::Number(sp)) = Some(&resolved_value) {
                            s.spacing = Some(*sp);
                            s.gap = None; // Numberの場合はgapフィールドは使わない
                        } else if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.gap = Some(*d);
                            s.relative_spacing = Some(*d);
                        }
                    }
                    "card" => {
                        if let Some(Expr::Bool(c)) = Some(&resolved_value) {
                            s.card = Some(*c);
                        }
                    }
                    // ★ 新規追加: rounded プロパティの処理
                    "rounded" => {
                        match &resolved_value {
                            Expr::Number(r) => {
                                s.rounded = Some(Rounded::Px(*r));
                            }
                            Expr::Bool(true) => {
                                s.rounded = Some(Rounded::On);
                            }
                            Expr::Bool(false) => {
                                s.rounded = None;
                            }
                            Expr::Dimension(d) => {
                                s.rounded = Some(Rounded::Px(d.value));
                            }
                            _ => {}
                        }
                    }
                    // ★ 新規追加: max_width, min_width, min_height プロパティの処理
                    "max_width" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.max_width = Some(*d);
                        }
                    }
                    "min_width" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.min_width = Some(*d);
                        }
                    }
                    "max_height" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.max_height = Some(*d);
                        }
                    }
                    "min_height" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.min_height = Some(*d);
                        }
                    }
                    // マージン系プロパティ
                    "margin_top" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.margin_top = Some(*d);
                        }
                    }
                    "margin_bottom" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.margin_bottom = Some(*d);
                        }
                    }
                    "margin_left" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.margin_left = Some(*d);
                        }
                    }
                    "margin_right" => {
                        if let Some(Expr::Dimension(d)) = Some(&resolved_value) {
                            s.margin_right = Some(*d);
                        }
                    }
                    // その他のプロパティ
                    "line_height" => {
                        if let Some(Expr::Number(lh)) = Some(&resolved_value) {
                            s.line_height = Some(*lh);
                        }
                    }
                    "font_weight" => {
                        if let Some(Expr::String(fw)) = Some(&resolved_value) {
                            s.font_weight = Some(fw.clone());
                        }
                    }
                    "font_family" => {
                        if let Some(Expr::String(ff)) = Some(&resolved_value) {
                            s.font_family = Some(ff.clone());
                        }
                    }
                    "wrap" => {
                        if let Some(Expr::String(w)) = Some(&resolved_value) {
                            s.wrap = match w.to_lowercase().as_str() {
                                "auto" => Some(WrapMode::Auto),
                                "none" => Some(WrapMode::None),
                                _ => Some(WrapMode::Auto), // デフォルトはAuto
                            };
                        }
                    }
                    "align" => {
                        if let Some(Expr::String(a)) = Some(&resolved_value) {
                            s.align = match a.to_lowercase().as_str() {
                                "left" => Some(Align::Left),
                                "center" => Some(Align::Center),
                                "right" => Some(Align::Right),
                                "top" => Some(Align::Top),
                                "bottom" => Some(Align::Bottom),
                                _ => None,
                            };
                        }
                    }
                    _ => {
                        // 未知のプロパティは無視
                    }
                }
            }
            s
        }
        _ => Style::default()
    }
}

// (階層的フロー関連の関数は後方で実装)

/// 条件文字列を解析してExprに変換する
/// 例: "window.width <= 1000" -> BinaryOp { left: Path("window.width"), op: Le, right: Number(1000) }
fn parse_condition_string(condition: &str) -> Option<Expr> {
    // 先頭と末尾のダブルクォートを除去（文字列として渡される場合）
    let condition = condition.trim().trim_matches('"').trim();
    
    eprintln!("   [parse_condition_string] 入力: '{}'", condition);
    
    // 比較演算子を検出
    let (op, op_str) = if condition.contains("<=") {
        (BinaryOperator::Le, "<=")
    } else if condition.contains(">=") {
        (BinaryOperator::Ge, ">=")
    } else if condition.contains("==") {
        (BinaryOperator::Eq, "==")
    } else if condition.contains("!=") {
        (BinaryOperator::Ne, "!=")
    } else if condition.contains("<") {
        (BinaryOperator::Lt, "<")
    } else if condition.contains(">") {
        (BinaryOperator::Gt, ">")
    } else {
        eprintln!("   [parse_condition_string] 演算子が見つかりません");
        return None;
    };
    
    // 演算子で分割
    let parts: Vec<&str> = condition.split(op_str).collect();
    if parts.len() != 2 {
        eprintln!("   [parse_condition_string] 分割に失敗: parts.len() = {}", parts.len());
        return None;
    }
    
    let left_str = parts[0].trim();
    let right_str = parts[1].trim();
    
    eprintln!("   [parse_condition_string] left='{}', op={:?}, right='{}'", left_str, op, right_str);
    
    // 左辺を解析（通常はwindow.widthやwindow.height）
    let left = if left_str.contains('.') {
        Expr::Path(left_str.to_string())
    } else {
        Expr::Ident(left_str.to_string())
    };
    
    // 右辺を解析（数値）
    let right = if let Ok(num) = right_str.parse::<f32>() {
        Expr::Number(num)
    } else {
        Expr::String(right_str.to_string())
    };
    
    let result = Expr::BinaryOp {
        left: Box::new(left),
        op,
        right: Box::new(right),
    };
    
    eprintln!("   [parse_condition_string] 結果: {:?}", result);
    
    Some(result)
}

// ========================================
// 型推論関数
// ========================================

/// 式から基本的な型を推論する（パーサーレベル）
pub fn infer_expr_type(expr: &Expr) -> NiloType {
    match expr {
        // プリミティブ型の推論
        Expr::Number(_) => NiloType::Number,
        Expr::String(_) => NiloType::String,
        Expr::Bool(_) => NiloType::Bool,
        
        // 配列の型推論
        Expr::Array(items) => {
            if items.is_empty() {
                // 空配列はAny[]
                NiloType::Array(Box::new(NiloType::Any))
            } else {
                // 最初の要素の型を配列の型とする（簡易版）
                let first_type = infer_expr_type(&items[0]);
                NiloType::Array(Box::new(first_type))
            }
        }
        
        // 二項演算の型推論
        Expr::BinaryOp { left, op, right } => {
            let left_ty = infer_expr_type(left);
            let right_ty = infer_expr_type(right);
            
            match op {
                BinaryOperator::Add | BinaryOperator::Sub |
                BinaryOperator::Mul | BinaryOperator::Div => {
                    // 算術演算: 両方がNumberならNumber、それ以外はString（暗黙変換）
                    if left_ty == NiloType::Number && right_ty == NiloType::Number {
                        NiloType::Number
                    } else {
                        NiloType::String
                    }
                }
                BinaryOperator::Eq | BinaryOperator::Ne |
                BinaryOperator::Lt | BinaryOperator::Le |
                BinaryOperator::Gt | BinaryOperator::Ge => {
                    // 比較演算: 常にBool
                    NiloType::Bool
                }
            }
        }
        
        // その他の式は型が不明
        Expr::Path(_) | Expr::Ident(_) => NiloType::Unknown,
        Expr::Object(_) => NiloType::Unknown,
        Expr::Dimension(_) => NiloType::Number,  // 次元値は数値として扱う
        Expr::CalcExpr(inner) => infer_expr_type(inner),
        Expr::Match { .. } => NiloType::Unknown,  // Matchは複雑なので後で実装
        Expr::FunctionCall { .. } => NiloType::Unknown,  // 関数の戻り値は不明
    }
}

/// 型付き式を作成（パーサーで使用）
pub fn make_typed_expr(expr: Expr) -> TypedExpr {
    let inferred_type = infer_expr_type(&expr);
    TypedExpr::new(expr, inferred_type)
}

/// 型の互換性をチェック
pub fn check_type_compatibility(expected: &NiloType, actual: &NiloType) -> Result<(), String> {
    if expected.is_compatible_with(actual) {
        Ok(())
    } else {
        Err(format!(
            "型エラー: {} 型が期待されていますが、{} 型が見つかりました",
            expected.display(),
            actual.display()
        ))
    }
}

/// 型式をパースする
fn parse_type_expr(pair: Pair<Rule>) -> NiloType {
    let type_str = pair.as_str();
    let mut inner = pair.into_inner();
    let primitive_pair = inner.next().unwrap();
    
    // プリミティブ型を取得（大文字小文字両対応）
    let mut base_type = match primitive_pair.as_str() {
        "Number" | "number" => NiloType::Number,
        "String" | "string" => NiloType::String,
        "Bool" | "bool" => NiloType::Bool,
        "Any" | "any" => NiloType::Any,
        _ => {
            eprintln!("Unknown type: {}", primitive_pair.as_str());
            NiloType::Unknown
        }
    };
    
    // "[]" の数だけ配列でラップ
    let remaining_text = type_str[primitive_pair.as_str().len()..].trim();
    let array_depth = remaining_text.matches("[]").count();
    
    for _ in 0..array_depth {
        base_type = NiloType::Array(Box::new(base_type));
    }
    
    base_type
}

// ========================================
// 名前空間展開処理
// ========================================

/// namespace定義をパース
fn parse_namespace_def(pair: Pair<Rule>) -> Result<Namespace, String> {
    assert_eq!(pair.as_rule(), Rule::namespace_def);
    
    let mut inner = pair.into_inner();
    let name = inner.next().ok_or("namespace名がありません")?.as_str().to_string();
    
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
    
    log::info!("Parsed namespace '{}' with {} timelines", name, timelines.len());
    
    Ok(Namespace { name, timelines, components })
}

/// namespaced flow定義をパース
fn parse_namespaced_flow_def(pair: Pair<Rule>) -> Result<NamespacedFlow, String> {
    assert_eq!(pair.as_rule(), Rule::namespaced_flow_def);
    
    let mut inner = pair.into_inner();
    let name = inner.next().ok_or("flow名がありません")?.as_str().to_string();
    
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
    
    Ok(NamespacedFlow { name, start, transitions })
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
fn expand_namespaced_structures(
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
    
    // ベースフローがあればそれを使用
    if let Some(base) = base_flow {
        global_start = Some(base.start.clone());
        all_transitions.extend(base.transitions);
    }
    
    for nf in &namespaced_flows {
        // flow Login { start: Menu, Menu -> [Login, Signup] }
        // を Login::Menu -> [Login::Login, Login::Signup] に展開
        
        for transition in &nf.transitions {
            let from_qualified: Vec<String> = transition.from.iter()
                .map(|f| qualify_name(&nf.name, f))
                .collect();
            
            let to_qualified: Vec<FlowTarget> = transition.to.iter()
                .map(|t| FlowTarget {
                    timeline: qualify_name(&nf.name, t),
                    url: None,
                    params: std::collections::HashMap::new(),
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
