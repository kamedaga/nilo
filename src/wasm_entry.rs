// WASM専用のエントリーポイント

use wasm_bindgen::prelude::*;
use crate::parser::parse::parse_nilo;
use crate::engine::state::AppState;

// dynamic_foreach_test.niloファイルの内容をコンパイル時に埋め込み
const APP_NILO_SOURCE: &str = include_str!("dynamic_foreach_test.nilo");

// dynamic_foreach_test.nilo用のState構造体
#[derive(Debug, Clone)]
struct TestState {
    items: Vec<i32>,
    next_item_value: i32,
}

impl Default for TestState {
    fn default() -> Self {
        Self {
            items: vec![1, 2, 3],
            next_item_value: 4,
        }
    }
}

impl crate::engine::state::StateAccess for TestState {
    fn get_field(&self, key: &str) -> Option<String> {
        match key {
            "items" => Some(format!("{:?}", self.items)),
            "next_item_value" => Some(self.next_item_value.to_string()),
            _ => None,
        }
    }
    
    fn set(&mut self, path: &str, value: String) -> Result<(), String> {
        match path {
            "next_item_value" => {
                self.next_item_value = value.parse().map_err(|e| format!("Failed to parse next_item_value: {}", e))?;
                Ok(())
            }
            _ => Err(format!("Unknown field: {}", path))
        }
    }
    
    fn toggle(&mut self, _path: &str) -> Result<(), String> {
        Ok(())
    }
    
    fn list_append(&mut self, path: &str, value: String) -> Result<(), String> {
        match path {
            "items" => {
                let item: i32 = value.parse().map_err(|e| format!("Failed to parse item: {}", e))?;
                self.items.push(item);
                Ok(())
            }
            _ => Err(format!("Unknown list field: {}", path))
        }
    }
    
    fn list_remove(&mut self, path: &str, index: usize) -> Result<(), String> {
        match path {
            "items" => {
                if index < self.items.len() {
                    self.items.remove(index);
                    Ok(())
                } else {
                    Err("Index out of bounds".to_string())
                }
            }
            _ => Err(format!("Unknown list field: {}", path))
        }
    }
    
    fn list_clear(&mut self, path: &str) -> Result<(), String> {
        match path {
            "items" => {
                self.items.clear();
                Ok(())
            }
            _ => Err(format!("Unknown list field: {}", path))
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    // パニック時のエラーメッセージをブラウザコンソールに表示
    console_error_panic_hook::set_once();
    
    // WebAssembly用のロガーを初期化
    console_log::init_with_level(log::Level::Debug).expect("error initializing log");

    log::info!("Nilo WASM starting...");
    log::info!("Loading dynamic_foreach_test.nilo...");
    
    // Niloソースコードをパース
    let app = match parse_nilo(APP_NILO_SOURCE) {
        Ok(app) => {
            log::info!("Successfully parsed dynamic_foreach_test.nilo");
            app
        }
        Err(e) => {
            let error_msg = format!("Failed to parse dynamic_foreach_test.nilo: {}", e);
            log::error!("{}", error_msg);
            show_error(&error_msg);
            return;
        }
    };
    
    // 開始タイムラインを取得
    let start_timeline = app.flow.start.clone();
    
    // 状態を初期化
    let mut state = AppState::new(TestState::default(), start_timeline);
    state.initialize_router(&app.flow);
    
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
