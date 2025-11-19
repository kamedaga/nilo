use nilo::*;
use std::collections::HashMap;

// 状態構造体
#[derive(Default, Clone, Debug, nilo_state_access_derive::StateAccess)]
struct IntervalState {
    timestamp: String,
    counter: i32,
    status: String,
}

fn main() {
    env_logger::init();
    
    // 定期実行する非同期関数を登録（1秒ごと）
    register_async_interval("update_timestamp", |_state| async {
        // 現在時刻を取得
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let timestamp = format!("Timestamp: {}", now);
        
        let mut updates = HashMap::new();
        updates.insert("timestamp".to_string(), timestamp);
        updates.insert("status".to_string(), "Updated!".to_string());
        
        log::info!("⏰ Interval function executed");
        updates
    });
    
    // 開始ボタン用の関数（状態不要）
    register_rust_call("start_timer", |_args| {
        if !is_async_interval_running("update_timestamp") {
            start_async_interval("update_timestamp", 1000);
            log::info!("Timer started");
        } else {
            log::info!("Timer already running");
        }
    });
    
    // 停止ボタン用の関数
    register_rust_call("stop_timer", |_args| {
        if stop_async_interval("update_timestamp") {
            log::info!("Timer stopped");
        } else {
            log::info!("Timer not running");
        }
    });
    
    // カウンター開始
    register_rust_call("start_counter", |_args| {
        if !is_async_interval_running("increment_counter") {
            start_async_interval("increment_counter", 500);
            log::info!("Counter started");
        } else {
            log::info!("Counter already running");
        }
    });
    
    // カウンター停止
    register_rust_call("stop_counter", |_args| {
        if stop_async_interval("increment_counter") {
            log::info!("Counter stopped");
        } else {
            log::info!("Counter not running");
        }
    });
    
    // カウンターを増やす定期実行関数（500msごと）
    register_async_interval("increment_counter", |_state| async {
        let mut updates = HashMap::new();
        // counterフィールドを直接数値で更新
        // 現在の値を読み取って+1する必要があるため、
        // 単純に文字列で更新値を返す
        static COUNTER: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
        let new_value = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        updates.insert("counter".to_string(), new_value.to_string());
        updates
    });
    
    // アプリ起動
    let cli_args = parse_args();
    let state = IntervalState::default();
    
    run_application(
        "examples/interval_example.nilo",
        state,
        &cli_args,
        Some("Nilo Interval Example"),
    );
}
