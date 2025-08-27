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

/// 文字列から引用符を除去する関数
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
/// # 引数
/// * `source` - 解析対象のソースコード文字列
///
/// # 戻り値
/// * `Ok(App)` - 解析成功時のAST
/// * `Err(String)` - 解析エラー時のエラーメッセージ
pub fn parse_nilo(source: &str) -> Result<App, String> {
    // Pestパーサーでファイル全体を解析
    let mut pairs = NiloParser::parse(Rule::file, source)
        .map_err(|e| format!("構文解析エラー: {}", e))?;

    let file_pair = pairs.next().expect("ファイルペアが見つかりません");
    assert_eq!(file_pair.as_rule(), Rule::file);

    // 各定義を格納する変数を初期化
    let mut flow: Option<Flow> = None;
    let mut timelines = Vec::new();
    let mut components = Vec::new();

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
            Rule::timeline_def => {
                timelines.push(parse_timeline_def(pair));
            }
            Rule::component_def => {
                components.push(parse_component_def(pair));
            }
            _ => {} // その他のルールは無視
        }
    }

    // フロー定義は必須
    let flow = flow.ok_or_else(|| "フロー定義が見つかりません".to_string())?;
    Ok(App { flow, timelines, components })
}

// ========================================
// フロー/タイムライン/コンポーネント解析
// ========================================

/// フロー定義を解析してFlowASTを生成
///
/// フローは開始点と状態遷移を定義します
pub fn parse_flow_def(pair: Pair<Rule>) -> Result<Flow, String> {
    assert_eq!(pair.as_rule(), Rule::flow_def);

    let mut start = None;
    let mut transitions = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::start_def => {
                // 開始状態の定義を取得
                let ident = inner.into_inner().next().unwrap(); // ident
                start = Some(ident.as_str().to_string());
            }
            Rule::transition_def => {
                // 状態遷移の定義を解析
                let mut parts = inner.into_inner();
                let from = parts.next().unwrap().as_str().to_string();

                // 遷移先のリストを取得（空の場合もある）
                let to_list = if let Some(list_part) = parts.next() {
                    list_part.into_inner()
                        .map(|p| p.as_str().to_string())
                        .collect()
                } else {
                    Vec::new()
                };

                transitions.push((from, to_list));
            }
            _ => {}
        }
    }

    // バリデーション
    let start = start.ok_or_else(|| "フロー定義にはstart:が必要です".to_string())?;
    if transitions.is_empty() {
        return Err("フローには少なくとも1つの遷移（例: A -> B）が必要です".into());
    }
    Ok(Flow { start, transitions })
}

/// タイムライン定義を解析してTimelineASTを生成
///
/// タイムラインは名前付きのビューノード集合とイベントハンドラーを定義します
pub fn parse_timeline_def(pair: Pair<Rule>) -> Timeline {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let mut body: Vec<WithSpan<ViewNode>> = Vec::new();
    let whens = Vec::new(); // 空のまま保持（when_blockは通常のview_nodeとして処理）

    for node_pair in inner {
        match node_pair.as_rule() {
            Rule::view_nodes => {
                // view_nodesラッパーを剥がして個別のノードを処理
                for p in node_pair.into_inner() {
                    body.push(parse_view_node(p));
                }
            }
            _ => body.push(parse_view_node(node_pair)),
        }
    }
    Timeline { name, body, whens }
}

/// コンポーネント定義を解析してComponentASTを生成
///
/// コンポーネントは再利用可能なビュー要素を定義します
pub fn parse_component_def(pair: Pair<Rule>) -> Component {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();

    // パラメータリストの解析（オプション）
    let params = match inner.peek().map(|p| p.as_rule()) {
        Some(Rule::param_list) => inner.next().unwrap().into_inner().map(|p| p.as_str().to_string()).collect(),
        _ => vec![],
    };

    let mut body: Vec<WithSpan<ViewNode>> = Vec::new();
    let whens = Vec::new(); // 空のまま保持（when_blockは通常のview_nodeとして処理）

    for node_pair in inner {
        match node_pair.as_rule() {
            Rule::view_nodes => {
                // view_nodesラッパーを剥がして個別のノードを処理
                for p in node_pair.into_inner() {
                    body.push(parse_view_node(p));
                }
            }
            _ => body.push(parse_view_node(node_pair)),
        }
    }
    Component { name, params, body, whens }
}

// ========================================
// ビューノード解析（WithSpan + style サポート）
// ========================================

/// ビューノードを解析してWithSpan<ViewNode>を生成
///
/// 各ノードには位置情報（行・列）とオプションのスタイル情報が付与されます
fn parse_view_node(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    match pair.as_rule() {
        Rule::stencil_call => {
            WithSpan { node: ViewNode::Stencil(parse_stencil_call(pair)), line, column: col, style: None }
        }
        Rule::text => parse_text(pair),
        Rule::button => parse_button(pair),
        Rule::image => parse_image(pair),
        Rule::vstack_node => parse_vstack_node(pair),
        Rule::hstack_node => parse_hstack_node(pair),
        Rule::rust_call => parse_rust_call(pair),
        Rule::component_call => parse_component_call(pair),
        Rule::dynamic_section => parse_dynamic_section(pair),
        Rule::match_block => parse_match_block(pair),
        Rule::navigate_action => parse_navigate_action(pair),
        Rule::when_block => parse_when_block(pair),
        Rule::spacing_node => {
            let span = pair.as_span();
            let (line, col) = span.start_pos().line_col();

            // スペーシングの種類を判別（固定値 or 自動）
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
        _ => unreachable!("不明なview_node: {:?}", pair),
    }
}

// ========================================
// 個別ビューノード解析関数群
// ========================================

/// テキストノードの解析
///
/// 形式: Text("format_string", arg1, arg2, ..., [style: {...}])
/// フォーマット文字列と引数リスト、オプションのスタイルを解析
fn parse_text(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();

    let mut it = pair.into_inner();

    // 最初の引数は必ずフォーマット文字列
    let format = unquote(it.next().unwrap().as_str());

    // 残りの引数を位置引数とスタイル引数に振り分け
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

/// ボタンノードの解析
///
/// 形式: Button(id: "button_id", label: "Button Label", [onclick: function!()], [style: {...}])
/// ID、ラベル、オプションのonclick、オプションのスタイルを解析
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
            Rule::string if label.is_none() => { label = Some(unquote(p.as_str())); }

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
                    if inner.as_rule() == Rule::style_arg {
                        style = Some(style_from_expr(parse_expr(inner.into_inner().next().unwrap())));
                    }
                }
            }
            _ => {}
        }
    }

    // 必須フィールドの検証
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
                        // 将来の互換性のため（現在は未使用）
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

/// 垂直スタックノードの解析
///
/// 形式: VStack([style: {...}]) { ... }
/// 子要素を垂直方向に配置するコンテナ
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

/// 水平スタックノードの解析
///
/// 形式: HStack([style: {...}]) { ... }
/// 子要素を水平方向に配置するコンテナ
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
/// 定義済みコンポーネントの呼び出し
fn parse_component_call(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();

    let mut args: Vec<Expr> = Vec::new();
    let mut style: Option<Style> = None;

    // component_callはarg_itemの列を返す
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

/// 動的セクションの解析
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

/// マッチブロックの解析
///
/// 形式: match <expr> ([style: {...}]) { case value1 { ... } case value2 { ... } default { ... } }
/// 条件分岐による表示制御
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

/// ナビゲーションアクションの解析
///
/// 形式: navigate_to(TargetState)
/// 指定された状態への遷移アクション
fn parse_navigate_action(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();
    let target = inner.next().unwrap().as_str().to_string();
    WithSpan { node: ViewNode::NavigateTo { target }, line, column: col, style: None }
}

/// Whenブロック（イベントハンドラー）の解析
fn parse_when_block(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    
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
    
    WithSpan { 
        node: ViewNode::When { event, actions }, 
        line, 
        column: col, 
        style: None 
    }
}

// ========================================
// イベント/式の解析
// ========================================

/// イベント式の解析
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

/// 式の解析（リテラル、識別子、配列、オブジェクトなど）
fn parse_expr(pair: Pair<Rule>) -> Expr {
    match pair.as_rule() {
        // arg_itemは中の1要素（style_arg or expr）にフォールスルー
        Rule::arg_item => {
            let inner = pair.into_inner().next().expect("空のarg_item");
            return parse_expr(inner);
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
                    "vw" => Unit::Vw,
                    "vh" => Unit::Vh,
                    "%" => Unit::Percent,
                    "rem" => Unit::Rem,
                    "em" => Unit::Em,
                    _ => Unit::Px, // デフォルト
                };
                Expr::Dimension(DimensionValue { value, unit })
            } else {
                // 単位なしの場合はpxとして扱う
                Expr::Dimension(DimensionValue::px(value))
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
        Rule::expr => parse_expr(pair.into_inner().next().unwrap()),
        _ => panic!("不明なexpr rule: {:?}", pair.as_rule()),
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

    // 引数の解析
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
                            panic!("ステンシル引数に変数名は使用できません: key={}, value={}", key, actual_value.as_str());
                        }
                        _ => panic!("不明な引数タイプ"),
                    }
                } else {
                    panic!("key: {} にstencil_valueの値が見つかりません", key);
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

    // 値取得用マクロ（デフォルト値付き）
    macro_rules! get_f32 { ($k:expr, $def:expr) => { map.get($k).and_then(|v| v.as_f32()).unwrap_or($def) } }
    macro_rules! get_str { ($k:expr, $def:expr) => { map.get($k).and_then(|v| v.as_str()).unwrap_or($def).to_string() } }
    macro_rules! get_bool { ($k:expr, $def:expr) => { map.get($k).and_then(|v| v.as_bool()).unwrap_or($def) } }

    // 位置指定システムのヘルパー
    let parse_position_value = |key: &str, default: f32| -> f32 {
        map.get(key).and_then(|v| v.as_f32()).unwrap_or(default)
    };

    // ステンシルの種類ごとの解析
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

/// ステンシル引数の補助enum
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

/// 状態セットノードの解析
fn parse_state_set(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();                // ident_path, expr
    let path = inner.next().unwrap().as_str().to_string();
    let value = parse_expr(inner.next().unwrap());
    WithSpan { node: ViewNode::Set { path, value }, line, column: col, style: None }
}

/// リスト追加ノードの解析
fn parse_list_append(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();                // ident_path, expr
    let path = inner.next().unwrap().as_str().to_string();
    let value = parse_expr(inner.next().unwrap());
    WithSpan { node: ViewNode::ListAppend { path, value }, line, column: col, style: None }
}

/// ��スト削除ノードの解析
fn parse_list_remove(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();                // ident_path, number
    let path = inner.next().unwrap().as_str().to_string();
    let index = inner.next().unwrap().as_str().parse::<usize>().unwrap();
    WithSpan { node: ViewNode::ListRemove { path, index }, line, column: col, style: None }
}

/// 状態トグルノードの解析
fn parse_state_toggle(pair: Pair<Rule>) -> WithSpan<ViewNode> {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let mut inner = pair.into_inner();                // ident_path(lhs), ident_path(rhs)
    let lhs = inner.next().unwrap().as_str().to_string();
    let rhs = inner.next().unwrap().as_str().to_string();
    if lhs != rhs {
        panic!("toggle は `state.foo = !state.foo` の形式で同一パスに対して行ってください（lhs={}, rhs={}）", lhs, rhs);
    }
    WithSpan { node: ViewNode::Toggle { path: lhs }, line, column: col, style: None }
}

// ========================================
// スタイル取り回し
// ========================================

/// 式からスタイルを生成
fn style_from_expr(expr: Expr) -> Style {
    match expr {
        Expr::Object(kvs) => {
            let mut s = Style::default();

            for (k, v) in kvs {
                // ★ 新規追加: match式を含むプロパティを特別処理
                let resolved_value = match &v {
                    Expr::Match { .. } => {
                        // match式は実行時に評価されるため、ここでは仮の値を設定
                        // 実際の評価はAppState::eval_expr_from_astで行われる
                        v.clone()
                    },
                    _ => v.clone()
                };

                match k.as_str() {
                    "color"        => s.color        = color_from_expr(&resolved_value),
                    "background"   => s.background   = color_from_expr(&resolved_value),
                    "border_color" => s.border_color = color_from_expr(&resolved_value),

                    // ★ justify_contentなどのレイアウトプロパティを追加
                    "justify_content" => {
                        // match式の場合は実行時評価のため、ここでは何もしない
                        // 実際の処理はレイアウトエンジンで行われる
                        match &resolved_value {
                            Expr::Match { .. } => {
                                // match式をそのまま保持（実行時評価用）
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
                            Expr::Dimension(d) => Rounded::Px(d.value), // 相対単位も受け付ける
                            _ => Rounded::Px(8.0),
                        });
                    }

                    // 従来の絶対値padding/margin
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
                        // 相対単位のsizeもチェック
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

/// 式から相対単位対応のエッジを生成
fn relative_edges_from_expr(e: &Expr) -> Option<RelativeEdges> {
    match e {
        Expr::Number(n) => Some(RelativeEdges::all(DimensionValue::px(*n))),
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
        Expr::Number(n) => Some(DimensionValue::px(*n)),
        Expr::Dimension(d) => Some(*d),
        _ => None
    }
}

/// Rust関数呼び出しの解析
///
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
            // 後方互換性
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
/// 繰り返し処理による動的コンテンツ生成
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
    
    // "in"キーワードをスキップ（パーサーが自動処理）
    
    // 第2引数: 繰り返し対象の式
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
            var: var.expect("foreach には変数名が必要です"),
            iterable: iterable.expect("foreach には繰り返し対象が必要です"),
            body,
        },
        line,
        column: col,
        style,
    }
}

/// if制御ノードの解析
///
/// 形式: if condition ([style: {...}]) { ... } [else { ... }]
/// 条件分岐による表示制御
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
    
    // 残りの要素を処理
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
                    in_else = true; // 次のview_nodesはelse部分
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
