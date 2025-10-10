// リリースビルド時(not debug_assertions)にWindowsでコンソールウィンドウを非表示
//#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
const MY_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP-Regular.ttf"));
const APP_NILO: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/routing_test.nilo"));

use nilo;
use nilo::engine::rust_call::register_rust_call;
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
    }
}

fn hello_world(args: &[Expr]) {
    info!("Hello from Rust! Args: {:?}", args);
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
    // カスタムフォントを名前付きで登録（プロジェクトルートからの相対パス）
    // Niloファイル内で font: "japanese" として使用可能
    nilo::set_custom_font("japanese", MY_FONT);
    
    let cli_args = nilo::parse_args();

    register_rust_call("hello_rust", |_args: &[Expr]| {
        info!("Hello from Rust!"); // println!をinfo!に変更
    });

    register_rust_call("hello_world", hello_world);

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
    };

    // 自動で埋め込みファイルを使用するマクロを呼び出し
    nilo::run_nilo_app!("routing_test.nilo", state, &cli_args, Some("Nilo Routing Test"));
    }
    
    #[cfg(target_arch = "wasm32")]
    {
        // WASM環境では何もしない（wasm_main関数で処理）
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
    register_rust_call("hello_rust", |_args: &[Expr]| {
        log::info!("Hello from Rust!");
    });
    register_rust_call("hello_world", hello_world);

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
    };

    // DOMレンダラーでNiloアプリを実行（tutorial.niloのルーティング機能付き）
    nilo::run_nilo_wasm(APP_NILO, state);
}