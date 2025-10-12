// リリースビルド時(not debug_assertions)にWindowsでコンソールウィンドウを非表示
//#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
const MY_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP-Regular.ttf"));

use nilo;
use nilo::engine::rust_call::{register_rust_call, register_state_accessible_call};
use nilo::engine::state::AppState;
use nilo::parser::ast::Expr;
use log::info; // ログマクロを追加

nilo::nilo_state! {
    struct State {
        name: String,
        counter: u32,
        items: Vec<i32>,
        ifbool: bool,
        frame_count: u32,
        elapsed_time: f32,
        show_section: bool,
        items_count: i32,
        filter_enabled: bool,
        next_item_value: i32,
        user_name: String,
    }
}

// onclick用の基本的な関数
fn hello_from_rust(_args: &[Expr]) {
    info!("🎉 hello_from_rust called!");
    println!("Hello from Rust!");
}

fn hello_world(args: &[Expr]) {
    info!("Hello from Rust! Args: {:?}", args);
}

fn greet_user(args: &[Expr]) {
    info!("👋 greet_user called with {} arguments", args.len());
    println!("Greeting user!");
}

fn log_message(args: &[Expr]) {
    if let Some(Expr::String(msg)) = args.first() {
        info!("📝 Log: {}", msg);
        println!("Log: {}", msg);
    }
}

// State変更可能な関数
fn increment_counter<S>(state: &mut AppState<S>, _args: &[Expr])
where
    S: nilo::engine::state::StateAccess,
{
    // カウンター値を取得
    let current = state.custom_state.get_field("counter")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    
    let new_value = current + 1;
    
    // stateを更新
    let _ = state.custom_state.set("counter", new_value.to_string());
    
    info!("✅ Counter incremented: {} -> {}", current, new_value);
    println!("Counter: {} -> {}", current, new_value);
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
    // カスタムフォントを名前付きで登録（プロジェクトルートからの相対パス）
    // Niloファイル内で font: "japanese" として使用可能
    nilo::set_custom_font("japanese", MY_FONT);
    
    let cli_args = nilo::parse_args();

    // onclick用の関数を登録
    register_rust_call("hello_from_rust", hello_from_rust);
    register_rust_call("hello_rust", |_args: &[Expr]| {
        info!("Hello from Rust!"); // println!をinfo!に変更
    });
    register_rust_call("hello_world", hello_world);
    register_rust_call("greet_user", greet_user);
    register_rust_call("log_message", log_message);
    
    // State変更可能な関数を登録
    register_state_accessible_call("increment_counter", increment_counter::<State>);

    let state = State {
        name: "Nilo".to_string(),
        counter: 1,
        items: vec![1, 2, 3],
        ifbool: true,
        frame_count: 0,
        elapsed_time: 0.0,
        show_section: true,
        items_count: 3,
        filter_enabled: false,
        next_item_value: 4,
        user_name: "Test User".to_string(),
    };

    // 自動で埋め込みファイルを使用するマクロを呼び出し
    nilo::run_nilo_app!("onclick_test.nilo", state, &cli_args, Some("Nilo Phase 2: Components"));
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
extern crate console_error_panic_hook;

#[cfg(target_arch = "wasm32")]
extern crate console_log;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    // パニック時のエラーメッセージをブラウザコンソールに表示
    console_error_panic_hook::set_once();
    
    // WebAssembly用のロガーを初期化
    console_log::init_with_level(log::Level::Debug).expect("error initializing log");

    log::info!("Nilo WASM main entry point starting...");

    // カスタムフォントを登録
    nilo::set_custom_font("japanese", MY_FONT);

    // Rust関数を登録
    register_rust_call("hello_from_rust", hello_from_rust);
    register_rust_call("hello_rust", |_args: &[Expr]| {
        log::info!("Hello from Rust!");
    });
    register_rust_call("hello_world", hello_world);
    register_rust_call("greet_user", greet_user);
    register_rust_call("log_message", log_message);
    
    // State変更可能な関数を登録
    register_state_accessible_call("increment_counter", increment_counter::<State>);

    // 初期状態を作成
    let state = State {
        name: "Nilo".to_string(),
        counter: 1,
        items: vec![1, 2, 3],
        ifbool: true,
        frame_count: 0,
        elapsed_time: 0.0,
        show_section: true,
        items_count: 3,
        filter_enabled: false,
        next_item_value: 4,
        user_name: "Test User".to_string(),
    };

    // run_nilo_appマクロを使用（WASM版でも統一）
    nilo::run_nilo_app!("local_vars_test.nilo", state);
}