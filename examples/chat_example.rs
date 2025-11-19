use nilo::*;
use nilo::nilo_safe_accessible;
use nilo::parser::ast::Expr;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use serde::Deserialize;

const MY_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP-Regular.ttf"));

// 状態構造体
nilo::nilo_state! {
    struct ChatState {
        username: String,
        input: String,
        messages: Vec<String>,  // メッセージを文字列の配列として保存
        status: String,
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            username: String::new(),
            input: String::new(),
            messages: vec![],
            status: String::new(),
        }
    }
}

// APIレスポンスの構造体
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

// ランダムな名前を生成
fn generate_random_name() -> String {
    let names = vec![
        "太郎", "花子", "次郎", "美咲", "健太",
        "さくら", "陽介", "あかり", "大輔", "結衣"
    ];
    let index = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() % names.len() as u128) as usize;
    names[index].to_string()
}

// 送信待ちメッセージを保存するグローバル変数（Mutexで安全に）
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
        log::info!("📤 Sending message: {}", message_text);
        
        // 非同期送信をトリガー
        let payload = serde_json::json!({
            "name": username,
            "message": message_text
        });
        
        // グローバルな送信キューに追加（後で処理）
        if let Ok(mut pending) = get_pending_message().lock() {
            *pending = Some(payload.to_string());
        }
        
        // 入力をクリア
        let _ = ctx.set("input", String::new());
        let _ = ctx.set("status", "📤 送信中...".to_string());
    }
}

fn main() {
    env_logger::init();
    
    let username = generate_random_name();
    log::info!("🎭 Generated username: {}", username);
    
    // Nilo関数を自動登録
    nilo::init_nilo_functions();
    
    // メッセージを取得する定期実行関数（2000msごと）
    register_async_interval("fetch_messages", move |_state| {
        async move {
            log::info!("📥 Fetching messages...");
            
            let url = "https://us-central1-nilo-chat-example.cloudfunctions.net/sendMessage";
            
            match reqwest::get(url).await {
                Ok(response) => {
                    match response.text().await {
                        Ok(text) => {
                            match serde_json::from_str::<ApiResponse>(&text) {
                                Ok(api_response) => {
                                    if api_response.success {
                                        if let Some(data) = api_response.data {
                                            // メッセージを整形して文字列に変換
                                            let messages: Vec<String> = data.into_iter().map(|msg| {
                                                // タイムスタンプをフォーマット
                                                let elapsed = std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap()
                                                    .as_secs() as i64 - msg.timestamp._seconds;
                                                
                                                let timestamp = if elapsed < 60 {
                                                    format!("{}秒前", elapsed)
                                                } else if elapsed < 3600 {
                                                    format!("{}分前", elapsed / 60)
                                                } else {
                                                    format!("{}時間前", elapsed / 3600)
                                                };
                                                
                                                // メッセージを単一の文字列として整形
                                                format!("【{}】{} ({})", msg.name, msg.message, timestamp)
                                            }).collect();
                                            
                                            // 古い順に並び替え（最新が下）
                                            let mut messages = messages;
                                            messages.reverse();
                                            
                                            let mut updates = HashMap::new();
                                            
                                            // ★ __list_set__ プレフィックスを使ってリストを一括設定
                                            let messages_json = serde_json::to_string(&messages).unwrap_or_default();
                                            updates.insert("__list_set__messages".to_string(), messages_json);
                                            updates.insert("status".to_string(), "✅ 更新済み".to_string());
                                            
                                            log::info!("✅ Fetched {} messages", messages.len());
                                            return updates;
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("❌ Failed to parse response: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("❌ Failed to read response text: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("❌ Failed to fetch messages: {}", e);
                }
            }
            
            let mut updates = HashMap::new();
            updates.insert("status".to_string(), "❌ 取得失敗".to_string());
            updates
        }
    });
    
    // 送信処理を行う定期実行関数
    register_async_interval("process_send", move |_state| {
        async move {
            let mut updates = HashMap::new();
            
            // 送信待ちメッセージがあるか確認
            let pending = if let Ok(mut pending_lock) = get_pending_message().lock() {
                pending_lock.take()
            } else {
                None
            };
            
            if let Some(payload) = pending {
                log::info!("📤 Sending message...");
                
                let url = "https://us-central1-nilo-chat-example.cloudfunctions.net/sendMessage";
                let client = reqwest::Client::new();
                
                match client.post(url).body(payload)
                    .header("Content-Type", "application/json")
                    .send().await {
                    Ok(response) => {
                        if response.status().is_success() {
                            log::info!("✅ Message sent successfully");
                            updates.insert("status".to_string(), "✅ 送信成功".to_string());
                            
                            // すぐにメッセージを再取得
                            start_async_interval("fetch_messages", 100);
                        } else {
                            log::error!("❌ Failed to send message: {}", response.status());
                            updates.insert("status".to_string(), "❌ 送信失敗".to_string());
                        }
                    }
                    Err(e) => {
                        log::error!("❌ Failed to send message: {}", e);
                        updates.insert("status".to_string(), "❌ 送信失敗".to_string());
                    }
                }
            }
            
            updates
        }
    });
    
    // アプリ起動
    let cli_args = parse_args();
    let mut state = ChatState::default();
    state.username = username.clone();
    state.status = "起動中...".to_string();
    
    // カスタムフォントを登録
    set_custom_font("japanese", MY_FONT);
    
    // メッセージ取得を開始（2000msごと）
    start_async_interval("fetch_messages", 2000);
    
    // 送信処理を開始（500msごとにチェック）
    start_async_interval("process_send", 500);
    
    run_nilo_app!("examples/chat_example.nilo", state, &cli_args, Some("Nilo Chat Example"));
}
