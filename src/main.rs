// リリースビルド時(not debug_assertions)にWindowsでコンソールウィンドウを非表示

//#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
const MY_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP-Regular.ttf"));

use log::info;
use nilo::nilo_function;
use nilo::{nilo_state_watcher, nilo_state_validator};
use nilo::register_safe_state_call;
// register_state_accessible_call は自動登録マクロに置き換え
use nilo::{AppState, StateAccess, nilo_safe_accessible};
use nilo::parser::ast::Expr;

nilo::nilo_state! {
    struct State {
        counter: i32,
        name: String,
        ok: bool,
        items: Vec<i32>,
    }
}

impl Default for State {
    fn default() -> Self {
        Self { counter: 0, name: String::new(), ok: false, items: vec![] }
    }
}

// ===== Demo: #[nilo_state_watcher] =====

// counter / name が更新されるたびにログに出す
#[nilo_state_watcher(state = State, fields("counter", "name"))]
fn log_state_changes(state: &mut State) {
    // 単純に読み出してログ
    let c = state.get_field("counter").unwrap_or_else(|| "?".into());
    let n = state.get_field("name").unwrap_or_else(|| "".into());
    log::info!("[watcher] counter={}, name='{}'", c, n);
}

// ===== Demo: #[nilo_state_validator] =====
// name は 0 文字でない、かつ 32 文字以内
#[nilo_state_validator(state = State, field = "name")]
fn validate_name(v: String) -> Result<(), String> {
    if v.trim().is_empty() {
        return Err("name must not be empty".into());
    }
    if v.chars().count() > 32 {
        return Err("name must be <= 32 chars".into());
    }
    Ok(())
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

// #[nilo_state_assign(state = State, field = "counter")]
// fn set_counter_value(_state: &mut State, _value: i32) -> Result<(), String> { unreachable!() }

// ========================================
// onclick用の関数定義（自動登録される）
// ========================================

#[nilo_safe_accessible(state = State, name = "increment_counter")]
fn inc_fn(ctx: &mut nilo::CustomStateContext<State>, _args: &[Expr]) {
    if let Some(current) = ctx.get_as::<i32>("counter") {
        let _ = ctx.set("counter", (current + 1).to_string());
    }
}

#[nilo_safe_accessible(state = State, name = "reset_counter")]
fn reset_fn(ctx: &mut nilo::CustomStateContext<State>, _args: &[Expr]) {
    let _ = ctx.set("counter", "0".to_string());
}

#[nilo_safe_accessible(state = State, name = "set_name")]
fn set_name_fn(ctx: &mut nilo::CustomStateContext<State>, args: &[Expr]) {
    if let Some(Expr::String(name)) = args.first() {
        let _ = ctx.set("name", name.clone());
    }
}

#[nilo_safe_accessible(state = State, name = "toggle_ok")]
fn toggle_ok_fn(ctx: &mut nilo::CustomStateContext<State>, _args: &[Expr]) {
    let current = ctx.get_as::<bool>("ok").unwrap_or(false);
    let _ = ctx.set("ok", (!current).to_string());
}

#[nilo_safe_accessible(state = State, name = "add_item")]
fn add_item_fn(ctx: &mut nilo::CustomStateContext<State>, args: &[Expr]) {
    if let Some(Expr::Number(n)) = args.first() {
        let _ = ctx.list_append("items", n.to_string());
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Nilo関数を自動登録（関数・ウォッチャ・バリデータ含む）
        nilo::init_nilo_functions();

        // onclick 等で使用する安全な Rust 関数群を登録（SAFEレジストリ）
        register_safe_state_call("increment_counter", |ctx: &mut nilo::CustomStateContext<State>, _args| {
            if let Some(current) = ctx.get_as::<i32>("counter") {
                let _ = ctx.set("counter", (current + 1).to_string());
            }
        });
        register_safe_state_call("reset_counter", |ctx: &mut nilo::CustomStateContext<State>, _args| {
            let _ = ctx.set("counter", "0".to_string());
        });

        // ↑ 上記の関数は main 関数外で定義されているため自動登録される
        register_safe_state_call("set_name", |ctx: &mut nilo::CustomStateContext<State>, args| {
            if let Some(nilo::parser::ast::Expr::String(name)) = args.get(0) {
                let _ = ctx.set("name", name.clone());
            }
        });
        register_safe_state_call("toggle_ok", |ctx: &mut nilo::CustomStateContext<State>, _args| {
            let current = ctx.get_as::<bool>("ok").unwrap_or(false);
            let _ = ctx.set("ok", (!current).to_string());
        });
        register_safe_state_call("add_item", |ctx: &mut nilo::CustomStateContext<State>, args| {
            if let Some(nilo::parser::ast::Expr::Number(n)) = args.get(0) {
                let _ = ctx.list_append("items", n.to_string());
            }
        });

        // onclick互換レジストリへの assign ラッパー登録は未使用

        // カスタムフォントを名前付きで登録
        nilo::set_custom_font("japanese", MY_FONT);
        
        let cli_args = nilo::parse_args();

        let state = State::default();
        
        // デモアプリを起動（マクロ側で "src/" を付与するため、ファイル名のみ指定）
        nilo::run_nilo_app!("demo.nilo", state, &cli_args, Some("Nilo State Demo"));
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

    // カスタムフォントを登録
    nilo::set_custom_font("japanese", MY_FONT);

    // 初期状態を作成
    let state = State::default();

    // デモアプリを起動（マクロ側で "src/" を付与）
    nilo::run_nilo_app!("demo.nilo", state);
}
