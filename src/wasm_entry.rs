// WASM専用のエントリーポイント

use wasm_bindgen::prelude::*;
use crate::parser::parse::parse_nilo;
use crate::engine::state::AppState;

// app.niloファイルの内容をコンパイル時に埋め込み
const APP_NILO_SOURCE: &str = include_str!("app.nilo");

// 空のState構造体（アプリケーションに状態が必要な場合は拡張）
#[derive(Debug, Clone, Default)]
struct EmptyState;

impl crate::engine::state::StateAccess for EmptyState {
    fn get_field(&self, _key: &str) -> Option<String> {
        None
    }
    
    fn set(&mut self, _path: &str, _value: String) -> Result<(), String> {
        Ok(())
    }
    
    fn toggle(&mut self, _path: &str) -> Result<(), String> {
        Ok(())
    }
    
    fn list_append(&mut self, _path: &str, _value: String) -> Result<(), String> {
        Ok(())
    }
    
    fn list_remove(&mut self, _path: &str, _index: usize) -> Result<(), String> {
        Ok(())
    }
    
    fn list_clear(&mut self, _path: &str) -> Result<(), String> {
        Ok(())
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    // パニック時のエラーメッセージをブラウザコンソールに表示
    console_error_panic_hook::set_once();
    
    // WebAssembly用のロガーを初期化
    console_log::init_with_level(log::Level::Debug).expect("error initializing log");

    log::info!("Nilo WASM starting...");
    log::info!("Loading app.nilo...");
    
    // Niloソースコードをパース
    let app = match parse_nilo(APP_NILO_SOURCE) {
        Ok(app) => {
            log::info!("Successfully parsed app.nilo");
            app
        }
        Err(e) => {
            let error_msg = format!("Failed to parse app.nilo: {}", e);
            log::error!("{}", error_msg);
            show_error(&error_msg);
            return;
        }
    };
    
    // 開始タイムラインを取得
    let start_timeline = app.flow.start.clone();
    
    // 状態を初期化
    let state = AppState::new(EmptyState::default(), start_timeline);
    
    // runtime_domを使用してNiloアプリケーションを実行
    log::info!("Starting Nilo WASM app...");
    crate::engine::runtime_dom::run_dom(app, state);
    log::info!("Nilo WASM app initialized successfully!");
}

fn show_error(error: &str) {
    use web_sys::window;
    
    // HTMLエスケープ（簡易版）
    let escaped = error
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;");
    
    if let Some(window) = window() {
        if let Some(document) = window.document() {
            if let Some(container) = document.get_element_by_id("container") {
                let error_html = format!(
                    "<div style='padding: 20px; color: #ff4444; font-family: monospace; background: #1a1a1a; border: 2px solid #ff4444; border-radius: 8px;'>\
                    <h2 style='margin-top: 0;'>❌ Nilo Error</h2>\
                    <pre style='white-space: pre-wrap; word-wrap: break-word;'>{}</pre>\
                    </div>",
                    escaped
                );
                container.set_inner_html(&error_html);
            }
        }
    }
}
