// リリースビルド時(not debug_assertions)にWindowsでコンソールウィンドウを非表示

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
const MY_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP-Regular.ttf"));

use log::info;
use nilo::nilo_function;

// register_state_accessible_call は自動登録マクロに置き換え


nilo::nilo_state! {
    struct State {
        input: String
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            input: String::new()
        }
    }
}



// #[nilo_state_assign] の直接デモは現在コメントアウト
// #[nilo_state_assign(state = State, field = "counter")]
// fn assign_counter(state: &mut State, value: i32) -> Result<(), String> { unreachable!() }

// ========================================
// Nilo関数の定義（マクロで自動登録）
// ========================================

// URLを開く関数（自動登録される）
#[nilo_function]
fn open_url(url: String) {
    info!("🔗 Opening URL: {}", url);
    
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Err(e) = open::that(&url) {
            log::error!("Failed to open URL: {}", e);
        }
    }
    
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Err(e) = window.open_with_url_and_target(&url, "_blank") {
                log::error!("Failed to open URL: {:?}", e);
            }
        }
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Nilo関数を自動登録（関数・ウォッチャ・バリデータ含む）
        nilo::init_nilo_functions();

        // onclick互換レジストリへの assign ラッパー登録は未使用

        // カスタムフォントを名前付きで登録
        nilo::set_custom_font("japanese", MY_FONT);
        
        let cli_args = nilo::parse_args();

        let state = State::default();
        

        // プロジェクトルート基準のパスを許可する実装に合わせる
        nilo::run_nilo_app!("src/startup.nilo", state, &cli_args, Some("Nilo Startup"));
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

    // Nilo関数を自動登録
    nilo::init_nilo_functions();

    // WASM: manual registrations (macros don't auto-register here)
    // 1) typed Rust functions
    nilo::register_typed_call("open_url", open_url);
    

    // カスタムフォントを登録
    nilo::set_custom_font("japanese", MY_FONT);

    // 初期状態を作成
    let state = State::default();

    // Align WASM entry with desktop so sample tests run consistently
    nilo::run_nilo_app!("src/startup.nilo", state);
}
