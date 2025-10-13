use nilo::dom_renderer::DomRenderer;
use nilo::stencil::stencil::Stencil;

fn main() {
    env_logger::init();

    println!("DOMレンダラーのテストを開始します...");

    // DOMレンダラーを作成
    let mut renderer = DomRenderer::new();

    // テスト用のStencilを作成
    let test_stencils = vec![
        // Rect
        Stencil::Rect {
            position: [50.0, 50.0],
            width: 200.0,
            height: 100.0,
            color: [0.2, 0.5, 0.8, 1.0],
            scroll: false,
            depth: 0.0,
        },
        // Circle
        Stencil::Circle {
            center: [250.0, 250.0],
            radius: 50.0,
            color: [0.8, 0.3, 0.2, 1.0],
            scroll: false,
            depth: 0.0,
        },
        // Text
        Stencil::Text {
            content: "Hello, DOM Renderer!".to_string(),
            position: [50.0, 20.0],
            size: 24.0,
            color: [0.0, 0.0, 0.0, 1.0],
            font: "Noto Sans JP".to_string(),
            max_width: None,
            scroll: false,
            depth: 0.0,
        },
        // Triangle
        Stencil::Triangle {
            p1: [400.0, 100.0],
            p2: [350.0, 200.0],
            p3: [450.0, 200.0],
            color: [0.3, 0.8, 0.3, 1.0],
            scroll: false,
            depth: 0.0,
        },
    ];

    // Stencilをレンダリング
    renderer.render_stencils(&test_stencils, [0.0, 0.0], 1.0);

    // HTMLファイルに保存
    if let Err(e) = renderer.save_to_file("dom_output.html") {
        eprintln!("HTMLファイルの保存に失敗しました: {}", e);
    } else {
        println!("HTMLファイルを保存しました: dom_output.html");
        println!("ブラウザで開いて確認してください。");
    }
}
