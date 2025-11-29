use nilo::parser::ast::Expr;
use nilo::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const MY_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fonts/NotoSansJP-Regular.ttf"
));


// Limit messages kept in memory to reduce UI work
const MAX_MESSAGES: usize = 50;
// チャット用の状態定義
nilo::nilo_state! {
    struct ChatState {
        username: String,
        input: String,
        messages: Vec<String>,  // 受信メッセージ一覧
        messages_count: String, // 件数（UI判定用）
        status: String,
        sending: bool,
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            username: String::new(),
            input: String::new(),
            messages: vec![],
            messages_count: "0".to_string(),
            status: String::new(),
            sending: false,
        }
    }
}

// APIレスポンス定義
#[derive(Deserialize)]
struct ApiResponse {
    success: bool,
    data: Option<Vec<ApiMessage>>,
}

#[derive(Deserialize)]
struct ApiMessage {
    id: String,
    name: String,
    message: String,
    timestamp: ApiTimestamp,
}

#[derive(Deserialize)]
struct ApiTimestamp {
    _seconds: i64,
    _nanoseconds: i64,
}

// ランダムな表示名を生成
fn generate_random_name() -> String {
    let names = vec![
        "さくら",
        "ニロ",
        "ニロ",
        "たろう",
        "はな",
        "あかね",
        "そうた",
        "かおる",
        "みどり",
        "ニロ",
    ];
    
    #[cfg(not(target_arch = "wasm32"))]
    {
        let index = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % names.len() as u128) as usize;
        names[index].to_string()
    }
    
    #[cfg(target_arch = "wasm32")]
    {
        // WASM環境ではjs_sys::Date::now()を使用
        use wasm_bindgen::prelude::*;
        #[wasm_bindgen]
        extern "C" {
            #[wasm_bindgen(js_namespace = Date, js_name = now)]
            fn date_now() -> f64;
        }
        let index = (date_now() as u128 % names.len() as u128) as usize;
        names[index].to_string()
    }
}

// 送信待ちメッセージを保持するグローバル変数（Mutexで安全に）
static PENDING_MESSAGE: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn get_pending_message() -> &'static Mutex<Option<String>> {
    PENDING_MESSAGE.get_or_init(|| Mutex::new(None))
}

// メッセージを送信する関数
#[nilo_safe_accessible(state = ChatState, name = "send_message")]
fn send_message_fn(ctx: &mut nilo::CustomStateContext<ChatState>, _args: &[Expr]) {
    let input = ctx.get("input").unwrap_or_default();
    let message_text = input.trim();
    let username = ctx.get("username").unwrap_or_default();

    if !message_text.is_empty() {
        log::info!("📨 Sending message: {}", message_text);

        // 非同期送信用ペイロード
        let payload = serde_json::json!({
            "name": username,
            "message": message_text
        });

        // グローバルな送信キューに追加（最初の1件）
        if let Ok(mut pending) = get_pending_message().lock() {
            *pending = Some(payload.to_string());
        }

        // 入力をクリア
        let _ = ctx.set("input", String::new());
        let _ = ctx.set("status", "送信中...".to_string());
        let _ = ctx.set("sending", "true".to_string());
    }
}

fn register_background_tasks() {
    // メッセージ取得（2000ms ごと）
    register_async_interval("fetch_messages", move |_state| {
        async move {
            log::info!("🔄 Fetching messages...");

            let url = "https://us-central1-nilo-chat-example.cloudfunctions.net/sendMessage";

            match reqwest::get(url).await {
                Ok(response) => match response.text().await {
                    Ok(text) => match serde_json::from_str::<ApiResponse>(&text) {
                        Ok(api_response) => {
                            if api_response.success {
                                if let Some(data) = api_response.data {
                                    let mut messages_display: Vec<String> = Vec::new();
                                    let mut messages_structs: Vec<serde_json::Value> = Vec::new();

                                    let limited: Vec<ApiMessage> = data
                                        .into_iter()
                                        .rev()
                                        .take(MAX_MESSAGES)
                                        .collect();

                                    for msg in limited.into_iter().rev() {
                                        #[cfg(not(target_arch = "wasm32"))]
                                        let now_secs = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs() as i64;

                                        #[cfg(target_arch = "wasm32")]
                                        let now_secs = {
                                            use wasm_bindgen::prelude::*;
                                            #[wasm_bindgen]
                                            extern "C" {
                                                #[wasm_bindgen(js_namespace = Date, js_name = now)]
                                                fn date_now() -> f64;
                                            }
                                            (date_now() / 1000.0) as i64
                                        };

                                        let elapsed = now_secs - msg.timestamp._seconds;

                                        let timestamp = if elapsed < 60 {
                                            format!("{}秒前", elapsed)
                                        } else if elapsed < 3600 {
                                            format!("{}分前", elapsed / 60)
                                        } else {
                                            format!("{}時間前", elapsed / 3600)
                                        };

                                        messages_structs.push(serde_json::json!({
                                            "name": msg.name,
                                            "message": msg.message,
                                            "timestamp": timestamp
                                        }));

                                        messages_display.push(format!(
                                            "【{}】{} ({})",
                                            msg.name, msg.message, timestamp
                                        ));
                                    }

                                    messages_structs.reverse();
                                    messages_display.reverse();

                                    let mut updates = HashMap::new();

                                    let messages_json =
                                        serde_json::to_string(&messages_display).unwrap_or_default();
                                    updates.insert(
                                        "__list_set__messages".to_string(),
                                        messages_json.clone(),
                                    );

                                    updates.insert(
                                        "messages_count".to_string(),
                                        messages_structs.len().to_string(),
                                    );

                                    updates
                                        .insert("status".to_string(), "更新しました".to_string());
                                    log::info!("✅ Fetched {} messages", messages_structs.len());
                                    return updates;
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("❌ Failed to parse response: {}", e);
                        }
                    },
                    Err(e) => {
                        log::error!("❌ Failed to read response text: {}", e);
                    }
                },
                Err(e) => {
                    log::error!("❌ Failed to fetch messages: {}", e);
                }
            }

            let mut updates = HashMap::new();
            updates.insert("status".to_string(), "取得に失敗しました".to_string());
            updates
        }
    });
    // 送信キュー処理（500ms ごと）
    register_async_interval("process_send", move |_state| {
        async move {
            let mut updates = HashMap::new();

            // 送信待ちメッセージを確認
            let pending = if let Ok(mut pending_lock) = get_pending_message().lock() {
                pending_lock.take()
            } else {
                None
            };

            if let Some(payload) = pending {
                log::info!("📤 Processing send...");

                let url = "https://us-central1-nilo-chat-example.cloudfunctions.net/sendMessage";
                let client = reqwest::Client::new();

                match client
                    .post(url)
                    .body(payload)
                    .header("Content-Type", "application/json")
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status().is_success() {
                            log::info!("✅ Message sent");
                            updates.insert("status".to_string(), "送信完了".to_string());
                            updates.insert("sending".to_string(), "false".to_string());

                            // すぐに最新メッセージを再取得
                            start_async_interval("fetch_messages", 100);
                        } else {
                            log::error!("❌ Failed to send message: {}", response.status());
                            updates.insert("status".to_string(), "送信に失敗しました".to_string());
                            updates.insert("sending".to_string(), "false".to_string());
                        }
                    }
                    Err(e) => {
                        log::error!("❌ Failed to send message: {}", e);
                        updates.insert("status".to_string(), "送信に失敗しました".to_string());
                    }
                }
            }

            updates
        }
    });
}

fn build_initial_state(username: String) -> ChatState {
    let mut state = ChatState::default();
    state.username = username;
    state.status = "起動中...".to_string();
    state
}

fn start_intervals() {
    start_async_interval("fetch_messages", 2000);
    start_async_interval("process_send", 500);
}

#[cfg(target_arch = "wasm32")]
fn register_wasm_functions() {
    // WASM ではマクロ登録が走らないので手動登録
    nilo::register_state_accessible_call(
        "send_message",
        |app_state: &mut nilo::AppState<ChatState>, args: &[Expr]| {
            nilo::engine::state::with_custom_state(app_state, |ctx| {
                send_message_fn(ctx, args);
            });
        },
    );
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();

        let username = generate_random_name();
        log::info!("👤 Generated username: {}", username);

        // Nilo関数の自動登録
        nilo::init_nilo_functions();
        register_background_tasks();

        let cli_args = parse_args();
        let state = build_initial_state(username);

        // カスタムフォントを登録
        set_custom_font("japanese", MY_FONT);

        start_intervals();

        run_nilo_app!(
            "examples/chat_example.nilo",
            state,
            &cli_args,
            Some("Nilo Chat Example")
        );
    }
}

#[cfg(target_arch = "wasm32")]
extern crate console_error_panic_hook;

#[cfg(target_arch = "wasm32")]
extern crate console_log;

// WASM版のエントリポイント
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    // パニック時のエラーメッセージをブラウザコンソールに表示
    console_error_panic_hook::set_once();

    // WebAssembly用のロガーを初期化
    console_log::init_with_level(log::Level::Debug).expect("error initializing log");

    log::info!("🚀 WASM Chat Example starting...");

    let username = generate_random_name();
    log::info!("👤 Generated username: {}", username);

    // Nilo関数の手動登録
    init_nilo_functions();
    register_wasm_functions();
    register_background_tasks();

    let state = build_initial_state(username);

    // カスタムフォントを登録
    set_custom_font("japanese", MY_FONT);

    start_intervals();

    run_nilo_app!("examples/chat_example.nilo", state);
}
