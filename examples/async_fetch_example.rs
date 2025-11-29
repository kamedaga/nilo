// 非同期fetchのデモです。ボタンを押すとexample.comからHTMLを取得して表示します。
// Async fetch demo: Click the button to fetch HTML from example.com and display it.

const MY_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fonts/NotoSansJP-Regular.ttf"
));

use nilo::parser::ast::Expr;
use std::collections::HashMap;
use std::future::Future;

nilo::nilo_state! {
    struct State {
        fetch_result: String,
        is_loading: String,
        error_message: String,
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            fetch_result: String::from("ボタンを押してデータを取得してください"),
            is_loading: String::from("false"),
            error_message: String::new(),
        }
    }
}

// 非同期fetchを実行する関数
async fn fetch_example_com() -> Result<String, String> {
    log::info!("Starting async fetch to example.com...");

    // 少し遅延を入れて読み込み中が見えるようにする
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    match reqwest::get("https://example.com").await {
        Ok(response) => {
            log::info!("Response received, status: {}", response.status());

            match response.text().await {
                Ok(text) => {
                    log::info!("Successfully fetched {} bytes", text.len());

                    // HTMLの最初の300文字を返す
                    let preview = if text.len() > 300 {
                        format!(
                            "{}...\n\n（テキストが長いため、最初の300文字のみ表示）",
                            &text[..300]
                        )
                    } else {
                        text
                    };

                    Ok(preview)
                }
                Err(e) => {
                    let error_msg = format!("テキスト取得エラー: {}", e);
                    log::error!("{}", error_msg);
                    Err(error_msg)
                }
            }
        }
        Err(e) => {
            let error_msg = format!("リクエストエラー: {}", e);
            log::error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

// ★ 非同期onclick関数（2段階で実行される）
// 即座に初期状態を返し、バックグラウンドで非同期処理を実行
fn start_fetch_async_impl(
    _initial_state: HashMap<String, String>,
    _args: Vec<Expr>,
) -> (
    HashMap<String, String>,
    impl Future<Output = HashMap<String, String>>,
) {
    log::info!("🚀 Async onclick: start_fetch_async called");

    // ★ STEP 1: 即座に返す初期状態（同期的）
    let mut immediate_updates = HashMap::new();
    immediate_updates.insert("is_loading".to_string(), "true".to_string());
    immediate_updates.insert("error_message".to_string(), String::new());
    immediate_updates.insert("fetch_result".to_string(), "データを取得中...".to_string());

    log::info!("📤 Returning immediate updates (loading state)");

    // ★ STEP 2: バックグラウンドで実行される非同期処理
    let future = async move {
        log::info!("⏳ Background task started, fetching data...");

        let mut final_updates = HashMap::new();

        match fetch_example_com().await {
            Ok(data) => {
                log::info!("✅ Fetch succeeded in background task");
                final_updates.insert("fetch_result".to_string(), data);
                final_updates.insert("is_loading".to_string(), "false".to_string());
            }
            Err(e) => {
                log::error!("❌ Fetch failed in background task: {}", e);
                final_updates.insert("error_message".to_string(), e);
                final_updates.insert("is_loading".to_string(), "false".to_string());
                final_updates.insert(
                    "fetch_result".to_string(),
                    "エラーが発生しました".to_string(),
                );
            }
        }

        log::info!("📥 Background task completed, returning final updates");
        final_updates
    };

    (immediate_updates, future)
}

// ラッパー関数でライフタイムを適切に処理
fn start_fetch_async() -> impl Fn(
    HashMap<String, String>,
    &[Expr],
) -> (
    HashMap<String, String>,
    std::pin::Pin<Box<dyn Future<Output = HashMap<String, String>> + Send>>,
) + Send
+ Sync
+ 'static {
    move |state: HashMap<String, String>, args: &[Expr]| {
        let args_owned = args.to_vec();
        let (immediate, future) = start_fetch_async_impl(state, args_owned);
        (immediate, Box::pin(future))
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        nilo::init_nilo_functions();
        nilo::set_custom_font("japanese", MY_FONT);

        // ★ 非同期onclick関数を登録（ラッパーを使用）
        nilo::register_async_onclick("start_fetch", start_fetch_async());

        let cli_args = nilo::parse_args();
        let state = State::default();

        nilo::run_nilo_app!(
            "examples/async_fetch.nilo",
            state,
            &cli_args,
            Some("Async Fetch Demo")
        );
    }
}
