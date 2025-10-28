/// ブラウザエディタ専用のWASMエントリポイント

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn run_nilo_code_from_browser(nilo_source: &str) {
    log::info!("run_nilo_code_from_browser: starting...");

    // Niloソースの解析
    let app = match crate::parser::parse::parse_nilo(nilo_source) {
        Ok(app) => app,
        Err(e) => {
            log::error!("Failed to parse Nilo source: {:?}", e);
            return;
        }
    };

    log::info!("Nilo app parsed successfully");

    // DOMコンテナの準備（既存のレンダリングを完全にリセット）
    crate::prepare_dom_container("container");

    // 初期状態でアプリを実行
    let state = crate::WasmTestState::default();
    let start_view = app.flow.start.clone();
    let mut app_state = crate::engine::state::AppState::new(state, start_view.clone());
    
    let initial_timeline = app_state.initialize_router_from_app(&app);

    // URLから初期タイムライン指定があれば適用
    if let Some(timeline) = initial_timeline {
        log::info!("Setting initial timeline from URL: {}", timeline);
        app_state.jump_to_timeline(&timeline);
    }

    log::info!("Running Nilo app with DOM renderer...");

    // DOMレンダラーでアプリを実行
    crate::engine::runtime_dom::run_dom(app, app_state);
}
