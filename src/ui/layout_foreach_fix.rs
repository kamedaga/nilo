// foreach文の修正版処理
use crate::engine::state::format_text;
use crate::parser::ast::{Expr, ViewNode, WithSpan};
use crate::ui::layout::{LayoutParams, LayoutedNode};
use crate::ui::text_measurement::measure_text_size;
use log::debug;

/// foreach文専用のレイアウト処理関数
pub fn layout_foreach_impl<'a, F, G>(
    var: &str,
    iterable: &Expr,
    body: &'a [WithSpan<ViewNode>],
    params: LayoutParams,
    result: &mut Vec<LayoutedNode<'a>>,
    cursor: &mut [f32; 2],
    eval: &F,
    get_image_size: &G,
) where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    // 繰り返し対象を評価
    let iterable_value = eval(iterable);
    debug!(
        "🔄 Layout: foreach var={}, iterable_value={}",
        var, iterable_value
    ); // println!をdebug!に変更

    let items: Vec<String> = if iterable_value.starts_with('[') && iterable_value.ends_with(']') {
        // JSON配列として解析を試行
        match serde_json::from_str::<Vec<serde_json::Value>>(&iterable_value) {
            Ok(parsed) => {
                debug!("✅ Layout: Successfully parsed {} items", parsed.len()); // println!をdebug!に変更
                parsed
                    .into_iter()
                    .map(|v| match v {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => v.to_string().trim_matches('"').to_string(),
                    })
                    .collect()
            }
            Err(e) => {
                debug!("❌ Layout: JSON parse error: {}", e); // println!をdebug!に変更
                vec![iterable_value]
            }
        }
    } else {
        vec![iterable_value]
    };

    // 各アイテムに対してボディを展開してレイアウト
    for (index, item) in items.iter().enumerate() {
        debug!("  🔸 Layout foreach[{}]: item='{}'", index, item); // println!をdebug!に変更

        // 各アイテムに対してボディの各ノードを処理
        for child in body {
            // 変数置換のための評価関数を作成
            let foreach_eval = |expr: &Expr| -> String {
                match expr {
                    Expr::Ident(s) if s == var => item.clone(),
                    Expr::Ident(s) if s == &format!("{}_index", var) => index.to_string(),
                    _ => eval(expr),
                }
            };

            // 置換された評価関数でノードサイズを計算
            let size = calculate_node_size_foreach(child, &params, &foreach_eval, get_image_size);
            result.push(LayoutedNode {
                node: child,
                position: *cursor,
                size,
            });
            cursor[1] += size[1];

            // スペーシングを追加
            if index < items.len() - 1 {
                cursor[1] += params.spacing / 2.0;
            }
        }
    }
}

/// foreach文用のノードサイズ計算（正確なテキスト測定使用）
fn calculate_node_size_foreach<F, G>(
    node: &WithSpan<ViewNode>,
    _params: &LayoutParams,
    eval: &F,
    _get_image_size: &G,
) -> [f32; 2]
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    match &node.node {
        ViewNode::Text { format, args } => {
            let values: Vec<String> = args.iter().map(|e| eval(e)).collect();
            let text = format_text(format, &values);
            // 正確なテキスト測定を使用
            let (width, height) = measure_text_size(&text, 18.0, "default", None);
            [width.clamp(100.0, 400.0), height]
        }
        _ => [160.0, 24.0],
    }
}
