// WASM専用のエントリーポイント

use wasm_bindgen::prelude::*;
use crate::dom_renderer::DomRenderer;
use crate::stencil::stencil::Stencil;

#[wasm_bindgen(start)]
pub fn main() {
    // パニック時のエラーメッセージをブラウザコンソールに表示
    console_error_panic_hook::set_once();
    
    // WebAssembly用のロガーを初期化
    console_log::init_with_level(log::Level::Debug).expect("error initializing log");

    log::info!("Nilo WASM starting...");
    
    // DOMレンダラーを作成
    let mut renderer = DomRenderer::new();
    
    // テスト用のStencilを作成
    let stencils = vec![
        Stencil::Text {
            content: "Hello from Nilo WASM!".to_string(),
            position: [50.0, 50.0],
            size: 48.0,
            color: [0.0, 0.0, 0.0, 1.0],
            font: "sans-serif".to_string(),
            max_width: Some(600.0),
            scroll: false,
            depth: 0.0,
        },
        Stencil::Text {
            content: "これはWebAssemblyで動作しているNilo UIフレームワークのデモです。".to_string(),
            position: [50.0, 120.0],
            size: 24.0,
            color: [0.3, 0.3, 0.3, 1.0],
            font: "sans-serif".to_string(),
            max_width: Some(600.0),
            scroll: false,
            depth: 0.0,
        },
        Stencil::Rect {
            position: [50.0, 180.0],
            width: 300.0,
            height: 200.0,
            color: [0.2, 0.5, 0.8, 1.0],
            scroll: false,
            depth: 0.0,
        },
        Stencil::Circle {
            center: [450.0, 280.0],
            radius: 80.0,
            color: [0.8, 0.3, 0.2, 1.0],
            scroll: false,
            depth: 0.0,
        },
        Stencil::Triangle {
            p1: [700.0, 180.0],
            p2: [650.0, 340.0],
            p3: [750.0, 340.0],
            color: [0.3, 0.8, 0.3, 1.0],
            scroll: false,
            depth: 0.0,
        },
    ];
    
    // レンダリング
    renderer.render_stencils(&stencils, [0.0, 0.0], 1.0);
    
    log::info!("Nilo WASM initialized successfully!");
}
