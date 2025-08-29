// ========================================
// Nilo言語パーサーモジュール
// ========================================
//
// このモジュールはNilo言語の構文解析を担当します。
// Pestパーサーを使用してASTを構築し、各種ノードの解析を行います。

use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;

use crate::parser::ast::*;
use crate::stencil::stencil::Stencil;

// ========================================
// ユーティリティ関数
// ========================================

fn unquote(s: &str) -> String {
    let trimmed = s.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"')) ||
       (trimmed.starts_with('「') && trimmed.ends_with('」')) {
        trimmed[1..trimmed.len()-1].to_string()
    } else {
        trimmed.to_string()
    }
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
    println!("🔍 PARSE DEBUG: Starting to parse nilo file, length: {} chars", source.len());

    // Pestパーサーでファイル全体を解析
    let mut pairs = NiloParser::parse(Rule::file, source)
        .map_err(|e| format!("構文解析エラー: {}", e))?;

    let file_pair = pairs.next().expect("ファイルペアが見つかりません");
    assert_eq!(file_pair.as_rule(), Rule::file);

    println!("🔍 PARSE DEBUG: Successfully parsed file structure");

    // 各定義を格納する変数を初期化
    let mut flow: Option<Flow> = None;
    let mut timelines = Vec::new();
    let mut components = Vec::new();
    // TODO: 階層的フロー糖衣構文の変数は後で追加
    // let mut namespaced_flows = Vec::new();

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
                let namespaced_flow = parse_namespaced_flow_def(pair)?;
                // 階層的フローを平坦なフローに変換
                let (expanded_flow, expanded_timelines) = expand_namespaced_flow(namespaced_flow, timelines)?;
                flow = Some(expanded_flow);
                timelines = expanded_timelines;
            }
            Rule::timeline_def => {
                timelines.push(parse_timeline_def(pair));
            }
            Rule::component_def => {
                components.push(parse_component_def(pair));
            }
            _ => {} // その他のルールは無視
        }
    }

    // TODO: 階層的フロー糖衣構文は後で実装
    // if !namespaced_flows.is_empty() {
    //     let (expanded_flow, expanded_timelines) = expand_namespaced_flows(namespaced_flows, timelines)?;
    //     flow = Some(expanded_flow);
    //     timelines = expanded_timelines;
    // }

    // フロー定義は必須
    let flow = flow.ok_or_else(|| "フロー定義が見つかりません".to_string())?;
    Ok(App { flow, timelines, components })
}

/// フロー定義を解析してFlowASTを生成
pub fn parse_flow_def(pair: Pair<Rule>) -> Result<Flow, String> {
    assert_eq!(pair.as_rule(), Rule::flow_def);

    let mut start = None;
    let mut transitions = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::start_def => {
                // 開始状態の定義を取得
                let ident = inner.into_inner().next().unwrap(); // qualified_ident
                start = Some(ident.as_str().to_string());
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
    if transitions.is_empty() {
        return Err("フロー定義には少なくとも1つの遷移が必要です".to_string());
    }
    Ok(Flow { start, transitions })
}

/// 遷移定義を解析する新しい関数
fn parse_transition_def(pair: Pair<Rule>) -> Result<(String, Vec<String>), String> {
    assert_eq!(pair.as_rule(), Rule::transition_def);

    let mut inner = pair.into_inner();

    // 遷移元の解析
    let source_pair = inner.next().ok_or("遷移定義に遷移元がありません")?;
    let source = parse_transition_source(source_pair)?;

    // 遷移先の解析
    let target_pair = inner.next().ok_or("遷移定義に遷移先がありません")?;
    let targets = parse_transition_targets(target_pair)?;

    // 現在のFlow構造では単一の遷移元のみサポートしているため、
    // 複数の遷移元がある場合は各々を個別の遷移として扱う
    if source.len() == 1 {
        Ok((source[0].clone(), targets))
    } else {
        Ok((source[0].clone(), targets))
    }
}

/// 遷移元の解析
fn parse_transition_source(pair: Pair<Rule>) -> Result<Vec<String>, String> {
    assert_eq!(pair.as_rule(), Rule::transition_source);

    let inner = pair.into_inner().next().ok_or("transition_sourceが空です")?;

    match inner.as_rule() {
        Rule::qualified_ident => {
            // 単一の識別子
            Ok(vec![inner.as_str().to_string()])
        }
        _ => {
            // 配列形式 [ident1, ident2, ...]
            let mut sources = Vec::new();
            for ident_pair in inner.into_inner() {
                if ident_pair.as_rule() == Rule::qualified_ident {
                    sources.push(ident_pair.as_str().to_string());
                }
            }
            Ok(sources)
        }
    }
}

/// 遷移先の解析
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
    let font: Option<String> = None;
    let mut body: Vec<WithSpan<ViewNode>> = Vec::new();
    let mut whens = Vec::new(); // whenイベントを正しく解析するように修正

    for node_pair in inner {
        match node_pair.as_rule() {
            // Rule::font_def => {  // 一時的にコメントアウト
            //     // font: "fonts/font" の形式を解析
            //     let font_str = node_pair.into_inner().next().unwrap().as_str();
            //     font = Some(unquote(font_str));
            // }
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
            _ => body.push(parse_view_node(node_pair)),
        }
    }
    Timeline { name, font, body, whens }
}

pub fn parse_component_def(pair: Pair<Rule>) -> Component {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();

    let params = match inner.peek().map(|p| p.as_rule()) {
        Some(Rule::param_list) => inner.next().unwrap().into_inner().map(|p| p.as_str().to_string()).collect(),
        _ => vec![],
    };

    let font: Option<String> = None;
    let mut body: Vec<WithSpan<ViewNode>> = Vec::new();
    let whens = Vec::new();

    for node_pair in inner {
        match node_pair.as_rule() {
            // Rule::font_def => {  // 一時的にコメントアウト
            //     // font: "fonts/font" の形式を解析
            //     let font_str = node_pair.into_inner().next().unwrap().as_str();
            //     font = Some(unquote(font_str));
            // }
            Rule::view_nodes => {
                for p in node_pair.into_inner() {
                    body.push(parse_view_node(p));
                }
            }
            _ => body.push(parse_view_node(node_pair)),
        }
    }
    Component { name, params, font, body, whens }
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
                    let v = p.as_str().parse::<f32>().unwrap_or(12.0);
                    ViewNode::Spacing(v)
                } else {
                    ViewNode::SpacingAuto
                }
            };
            WithSpan { node, line, column: col, style: None }
        }
        // 状態操作関連のノード
        Rule::state_set    => parse_state_set(pair),
        Rule::list_append  => parse_list_append(pair),
        Rule::list_remove  => parse_list_remove(pair),
        Rule::state_toggle => parse_state_toggle(pair),
        Rule::foreach_node => parse_foreach_node(pair),
        Rule::if_node => parse_if_node(pair),
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

            // rust_call（onclick属性）
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

    WithSpan { node: ViewNode::ComponentCall { name, args }, line, column: col, style }
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
            // dimension_valueは number ~ unit_suffix? の形
            let mut inner = pair.into_inner();
            let number_str = inner.next().unwrap().as_str();
            let value: f32 = number_str.parse().unwrap();

            // unit_suffixがあるかチェック
            if let Some(unit_pair) = inner.next() {
                let unit_str = unit_pair.as_str();
                let unit = match unit_str {
                    "px" => Unit::Px,
                    "vw" => {
                        println!("🔍 PARSER DEBUG: Found {}vw in parsing", value);
                        Unit::Vw
                    },
                    "vh" => {
                        println!("🔍 PARSER DEBUG: Found {}vh in parsing", value);
                        Unit::Vh
                    },
                    "%" => Unit::Percent,
                    "rem" => Unit::Rem,
                    "em" => Unit::Em,
                    _ => Unit::Px, // デフォルト
                };
                let result = Expr::Dimension(DimensionValue { value, unit });
                println!("🔍 PARSER DEBUG: Created DimensionValue: {:?}", result);
                result
            } else {
                // ★ 修正: 単位がない場合は純粋な数値として扱う（pxに変換しない）
                Expr::Number(value)
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
                let k = it.next().unwrap().as_str().to_string();
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
            // 算術式として解析を試行
            parse_arithmetic_expr(pair)
        }
    }
}

fn parse_arithmetic_expr(pair: Pair<Rule>) -> Expr {
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
            let number_str = inner.next().unwrap().as_str();
            let value: f32 = number_str.parse().unwrap();

            if let Some(unit_pair) = inner.next() {
                let unit_str = unit_pair.as_str();
                let unit = match unit_str {
                    "px" => Unit::Px,
                    "vw" => Unit::Vw,
                    "vh" => Unit::Vh,
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
        Rule::number => {
            let v: f32 = pair.as_str().parse().unwrap();
            Expr::Number(v)
        }
        Rule::bool => Expr::Bool(pair.as_str() == "true"),
        Rule::path => Expr::Path(pair.as_str().to_string()),
        Rule::ident => Expr::Ident(pair.as_str().to_string()),
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
    let mut inner = pair.into_inner();                // ident_path, expr
    let path = inner.next().unwrap().as_str().to_string();
    let value = parse_expr(inner.next().unwrap());
    WithSpan { node: ViewNode::Set { path, value }, line, column: col, style: None }
}

fn parse_list_append(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();                // ident_path, expr
    let path = inner.next().unwrap().as_str().to_string();
    let value = parse_expr(inner.next().unwrap());
    WithSpan { node: ViewNode::ListAppend { path, value }, line, column: col, style: None }
}

fn parse_list_remove(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();                // ident_path, number
    let path = inner.next().unwrap().as_str().to_string();
    let index = inner.next().unwrap().as_str().parse::<usize>().unwrap();
    WithSpan { node: ViewNode::ListRemove { path, index }, line, column: col, style: None }
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

// ========================================
// スタイル取り回し
// ========================================


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

                match k.as_str() {
                    "color"        => s.color        = color_from_expr(&resolved_value),
                    "background"   => s.background   = color_from_expr(&resolved_value),
                    "border_color" => s.border_color = color_from_expr(&resolved_value),


                    "justify_content" => {

                        match &resolved_value {
                            Expr::Match { .. } => {
                                // match式をそのまま保持
                            },
                            Expr::String(align_val) => {
                                // 静的な値の場合は即座に処理
                                s.align = Some(match align_val.as_str() {
                                    "flex-start" | "start" => Align::Left,
                                    "flex-end" | "end" => Align::Right,
                                    "center" => Align::Center,
                                    _ => Align::Left,
                                });
                            },
                            _ => {}
                        }
                    },
                    "align_items" => {
                        match &resolved_value {
                            Expr::String(align_val) => {
                                s.align = Some(match align_val.as_str() {
                                    "center" => Align::Center,
                                    "flex-start" | "start" => Align::Top,
                                    "flex-end" | "end" => Align::Bottom,
                                    _ => Align::Left,
                                });
                            },
                            _ => {}
                        }
                    },

                    // ★ 個別のwidth/heightの処理を追加
                    "width" => {
                        match resolved_value {
                            Expr::Number(n) => s.width = Some(n),
                            Expr::Dimension(d) => s.relative_width = Some(d),
                            _ => {}
                        }
                    }
                    "height" => {
                        match resolved_value {
                            Expr::Number(n) => s.height = Some(n),
                            Expr::Dimension(d) => s.relative_height = Some(d),
                            _ => {}
                        }
                    }

                    "rounded" => {
                        s.rounded = Some(match v {
                            Expr::Bool(true)  => Rounded::On,
                            Expr::Number(n)   => Rounded::Px(n),
                            Expr::Dimension(d) => Rounded::Px(d.value),
                            _ => Rounded::Px(8.0),
                        });
                    }

                    "padding" => s.padding = edges_from_expr(&v),
                    "margin"  => s.margin  = edges_from_expr(&v),

                    // 相対単位対応のpadding/margin
                    "relative_padding" => s.relative_padding = relative_edges_from_expr(&v),
                    "relative_margin"  => s.relative_margin  = relative_edges_from_expr(&v),

                    "size" => {
                        // 従来の絶対値size
                        if let Some([w,h]) = size_from_expr(&v) {
                            s.size = Some([w,h]);
                        }
                        if let Some([w,h]) = relative_size_from_expr(&v) {
                            s.relative_size = Some([w,h]);
                        }
                    }

                    "hover" => {
                        if let Expr::Object(_) = v {
                            s.hover = Some(Box::new(style_from_expr(v)));
                        }
                    }

                    "font_size" => {
                        match v {
                            Expr::Number(n) => s.font_size = Some(n),
                            Expr::Dimension(d) => s.relative_font_size = Some(d),
                            _ => {}
                        }
                    }
                    "font" => {
                        if let Expr::String(t) = v { s.font = Some(t); }
                    }
                    "align" => {
                        s.align = Some(match v {
                            Expr::String(ref t) if t.eq_ignore_ascii_case("center") => Align::Center,
                            Expr::String(ref t) if t.eq_ignore_ascii_case("right")  => Align::Right,
                            Expr::String(ref t) if t.eq_ignore_ascii_case("top")    => Align::Top,
                            Expr::String(ref t) if t.eq_ignore_ascii_case("bottom") => Align::Bottom,
                            _ => Align::Left
                        });
                    },
                    "spacing" => {
                        match v {
                            Expr::Number(n) => s.spacing = Some(n),
                            Expr::Dimension(d) => s.relative_spacing = Some(d),
                            _ => {}
                        }
                    },
                    "gap" => {
                        // spacingのエイリアス
                        match v {
                            Expr::Number(n) => s.spacing = Some(n),
                            Expr::Dimension(d) => s.relative_spacing = Some(d),
                            _ => {}
                        }
                    },
                    "card"    => { if let Expr::Bool(b)   = v { s.card    = Some(b); } },

                    "shadow" => {
                        s.shadow = Some(match v {
                            Expr::Bool(true) => Shadow::On,
                            Expr::Object(inner) => {
                                let mut blur = 8.0;
                                let mut offset = [0.0, 2.0];
                                let mut color: Option<ColorValue> = None;
                                for (kk, vv) in inner {
                                    match kk.as_str() {
                                        "blur" => if let Expr::Number(n) = vv { blur = n; },
                                        "offset" => {
                                            if let Expr::Array(xs) = vv {
                                                if xs.len() >= 2 {
                                                    if let (Expr::Number(x), Expr::Number(y)) = (&xs[0], &xs[1]) {
                                                        offset = [*x, *y];
                                                    }
                                                }
                                            }
                                        }
                                        "color" => color = color_from_expr(&vv),
                                        _ => {}
                                    }
                                }
                                Shadow::Spec { blur, offset, color }
                            }
                            _ => Shadow::On
                        });
                    }

                    _ => { /* 未知キーは無視 */ }
                }
            }
            s
        }
        _ => Style::default()
    }
}

fn relative_edges_from_expr(e: &Expr) -> Option<RelativeEdges> {
    match e {
        Expr::Number(_n) => {
            // ★ 修正: 純粋な数値はpxに自動変換しない
            // 相対単位のエッジは明示的にDimensionValueを持つもののみ
            None
        },
        Expr::Dimension(d) => Some(RelativeEdges::all(*d)),
        Expr::Array(xs) => {
            // [v, h] 形式
            if xs.len() == 2 {
                let v = dimension_from_expr(&xs[0])?;
                let h = dimension_from_expr(&xs[1])?;
                return Some(RelativeEdges::vh(v, h));
            }
            None
        }
        Expr::Object(kvs) => {
            let mut ed = RelativeEdges::default();
            for (k,v) in kvs {
                let dim = dimension_from_expr(v)?;
                match k.as_str() {
                    "top"    => ed.top = Some(dim),
                    "right"  => ed.right = Some(dim),
                    "bottom" => ed.bottom = Some(dim),
                    "left"   => ed.left = Some(dim),
                    _ => {}
                }
            }
            Some(ed)
        }
        _ => None
    }
}

/// 式から相対単位対応のサイズを生成
fn relative_size_from_expr(e: &Expr) -> Option<[DimensionValue; 2]> {
    if let Expr::Array(xs) = e {
        if xs.len() >= 2 {
            let w = dimension_from_expr(&xs[0])?;
            let h = dimension_from_expr(&xs[1])?;
            return Some([w, h]);
        }
    }
    None
}

/// 式からDimensionValueを抽出
fn dimension_from_expr(e: &Expr) -> Option<DimensionValue> {
    match e {
        Expr::Number(_n) => {
            // ★ 修正: 純粋な数値はpxに自動変換しない
            // DimensionValueは明示的にDimensionを持つExprのみから作成
            None
        },
        Expr::Dimension(d) => Some(*d),
        _ => None
    }
}

/// 形式: function_name!(arg1, ..., [style: {...}])
/// Rust側で定義された関数の呼び出し
fn parse_rust_call(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();

    let mut args: Vec<Expr> = Vec::new();
    let mut style: Option<Style> = None;

    // rust_callはarg_itemの列を返す
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
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::expr => args.push(parse_expr(p)),
            _ => {}
        }
    }

    WithSpan { node: ViewNode::RustCall { name, args }, line, column: col, style }
}

/// foreach制御ノードの解析
///
/// 形式: foreach item in expr ([style: {...}]) { ... }
fn parse_foreach_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut var: Option<String> = None;
    let mut iterable: Option<Expr> = None;
    let mut style: Option<Style> = None;
    let mut body: Vec<WithSpan<ViewNode>> = Vec::new();

    let mut inner = pair.into_inner();
    
    // 第1引数: 繰り返し変数名
    if let Some(var_pair) = inner.next() {
        var = Some(var_pair.as_str().to_string());
    }


    if let Some(expr_pair) = inner.next() {
        iterable = Some(parse_expr(expr_pair));
    }

    // 残りの要素を処理
    for p in inner {
        match p.as_rule() {
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::view_nodes => {
                body = p.into_inner().map(parse_view_node).collect();
            }
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::ForEach {
            var: var.expect("foreach には変数名が必ず必要です"),
            iterable: iterable.expect("foreach には繰り返し対象必要です"),
            body,
        },
        line,
        column: col,
        style,
    }
}

/// if制御ノードの解析
fn parse_if_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut condition: Option<Expr> = None;
    let mut style: Option<Style> = None;
    let mut then_body: Vec<WithSpan<ViewNode>> = Vec::new();
    let mut else_body: Option<Vec<WithSpan<ViewNode>>> = None;

    let mut inner = pair.into_inner();
    
    // 第1引数: 条件式
    if let Some(condition_pair) = inner.next() {
        condition = Some(parse_expr(condition_pair));
    }

    let mut in_else = false;

    for p in inner {
        match p.as_rule() {
            Rule::style_arg => {
                style = Some(style_from_expr(parse_expr(p.into_inner().next().unwrap())));
            }
            Rule::view_nodes => {
                let nodes = p.into_inner().map(parse_view_node).collect();
                if in_else {
                    else_body = Some(nodes);
                } else {
                    then_body = nodes;
                    in_else = true;
                }
            }
            _ => {}
        }
    }

    WithSpan {
        node: ViewNode::If {
            condition: condition.expect("if には条件式が必要です"),
            then_body,
            else_body,
        },
        line,
        column: col,
        style,
    }
}

/// テキスト入力フィールドの解析
///
/// 形式: TextInput(id: "field_id", placeholder: "hint", [value: "initial"], [ime_enabled: true], [style: {...}])
fn parse_text_input(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut id: Option<String> = None;
    let mut placeholder: Option<String> = None;
    let value: Option<Expr> = None;
    let on_change: Option<Expr> = None;
    let multiline = false;
    let max_length: Option<usize> = None;
    let ime_enabled = true; // デフォルトでIME有効
    let mut style: Option<Style> = None;

    // パラメータを順次解析
    let inner = pair.into_inner();
    let mut param_index = 0;

    for p in inner {
        match p.as_rule() {
            Rule::arg_item => {
                let inner_item = p.into_inner().next().unwrap();
                match inner_item.as_rule() {
                    Rule::expr => {
                        // 位置引数として処理
                        match param_index {
                            0 => {
                                if let Expr::String(s) = parse_expr(inner_item) {
                                    id = Some(s);
                                } else {
                                    panic!("TextInputの第1引数（id）は文字列である必要があります");
                                }
                            }
                            1 => {
                                // 第2引数: placeholder（オプション）
                                if let Expr::String(s) = parse_expr(inner_item) {
                                    placeholder = Some(s);
                                }
                            }
                            _ => {
                                // その他の引数は名前付きで処理
                            }
                        }
                        param_index += 1;
                    }
                    Rule::style_arg => {
                        let expr = parse_expr(inner_item.into_inner().next().unwrap());
                        style = Some(style_from_expr(expr));
                    }
                    _ => {}
                }
            }
            Rule::expr => {
                match param_index {
                    0 => {
                        if let Expr::String(s) = parse_expr(p) {
                            id = Some(s);
                        }
                    }
                    1 => {
                        if let Expr::String(s) = parse_expr(p) {
                            placeholder = Some(s);
                        }
                    }
                    _ => {}
                }
                param_index += 1;
            }
            Rule::style_arg => {
                let expr = parse_expr(p.into_inner().next().unwrap());
                style = Some(style_from_expr(expr));
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
            ime_enabled,
        },
        line,
        column: col,
        style,
    }
}

/// 階層的フロー定義を解析
pub fn parse_namespaced_flow_def(pair: Pair<Rule>) -> Result<NamespacedFlow, String> {
    assert_eq!(pair.as_rule(), Rule::namespaced_flow_def);

    let mut inner = pair.into_inner();

    // フロー名を取得
    let name = inner.next().unwrap().as_str().to_string();

    let mut start = None;
    let mut transitions = Vec::new();

    for flow_inner in inner {
        match flow_inner.as_rule() {
            Rule::namespaced_start_def => {
                let ident = flow_inner.into_inner().next().unwrap(); // ident
                start = Some(ident.as_str().to_string());
            }
            Rule::namespaced_transition_def => {
                // 遷移定義を実際に解析
                let transition = parse_namespaced_transition_def(flow_inner)?;
                transitions.push(transition);
            }
            _ => {}
        }
    }

    // バリデーション
    let start = start.ok_or_else(|| "階層的フロー定義にはstart:が必要です".to_string())?;
    if transitions.is_empty() {
        return Err("階層的フロー定義には少なくとも1つの遷移が必要です".to_string());
    }

    Ok(NamespacedFlow { name, start, transitions })
}

/// 階層的フローの遷移定義を解析する関数
fn parse_namespaced_transition_def(pair: Pair<Rule>) -> Result<(String, Vec<String>), String> {
    assert_eq!(pair.as_rule(), Rule::namespaced_transition_def);

    let mut inner = pair.into_inner();

    let source_pair = inner.next().ok_or("階層的遷移定義に遷移元がありません")?;
    let source = parse_namespaced_transition_source(source_pair)?;

    // 遷移先の解析
    let target_pair = inner.next().ok_or("階層的遷移定義に遷移先がありません")?;
    let targets = parse_namespaced_transition_targets(target_pair)?;

    // 現在のFlow構造では単一の遷移元のみサポートしているため、
    // 複数の遷移元がある場合は各々を個別の遷移として扱う
    if source.len() == 1 {
        Ok((source[0].clone(), targets))
    } else {
        // 複数遷移元の場合は最初のもので代表（後で改善予定）
        Ok((source[0].clone(), targets))
    }
}

/// 階層的フローの遷移元の解析
fn parse_namespaced_transition_source(pair: Pair<Rule>) -> Result<Vec<String>, String> {
    assert_eq!(pair.as_rule(), Rule::namespaced_transition_source);

    let inner = pair.into_inner().next().ok_or("namespaced_transition_sourceが空です")?;

    match inner.as_rule() {
        Rule::ident => {
            // 単一の識別子
            Ok(vec![inner.as_str().to_string()])
        }
        _ => {
            // 配列形式 [ident1, ident2, ...]
            let mut sources = Vec::new();
            for ident_pair in inner.into_inner() {
                if ident_pair.as_rule() == Rule::ident {
                    sources.push(ident_pair.as_str().to_string());
                }
            }
            Ok(sources)
        }
    }
}

/// 階層的フローの遷移先の解析
fn parse_namespaced_transition_targets(pair: Pair<Rule>) -> Result<Vec<String>, String> {
    match pair.as_rule() {
        Rule::qualified_ident | Rule::ident => {
            // 単一の遷移先
            Ok(vec![pair.as_str().to_string()])
        }
        _ => {
            // 配列形式の遷移先 [target1, target2, ...]
            let mut targets = Vec::new();
            for ident_pair in pair.into_inner() {
                match ident_pair.as_rule() {
                    Rule::qualified_ident | Rule::ident => {
                        targets.push(ident_pair.as_str().to_string());
                    }
                    _ => {}
                }
            }
            Ok(targets)
        }
    }
}

fn expand_namespaced_flow(
    namespaced_flow: NamespacedFlow,
    existing_timelines: Vec<Timeline>
) -> Result<(Flow, Vec<Timeline>), String> {
    let namespace = &namespaced_flow.name;

    // 新しい開始状態は namespace::start の形式
    let expanded_start = format!("{}::{}", namespace, namespaced_flow.start);

    // 遷移を展開
    let mut expanded_transitions = Vec::new();

    for (source, targets) in namespaced_flow.transitions {
        // 遷移元を修飾
        let qualified_source = format!("{}::{}", namespace, source);

        let qualified_targets: Vec<String> = targets.into_iter()
            .map(|target| {
                if target.contains("::") {
                    // 既に修飾されている場合はそのまま
                    target
                } else {
                    // ローカル名の場合は現在の名前空間で修飾
                    format!("{}::{}", namespace, target)
                }
            })
            .collect();

        expanded_transitions.push((qualified_source, qualified_targets));
    }

    // 例：階層化されたタイムラインが見つからない場合のデフォルト処理
    // この実装では既存のタイムラインをそのまま使用

    let expanded_flow = Flow {
        start: expanded_start,
        transitions: expanded_transitions,
    };

    Ok((expanded_flow, existing_timelines))
}
