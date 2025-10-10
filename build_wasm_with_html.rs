use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("🚀 Building Nilo WASM with HTML generation...");
    
    // 1. wasm-pack でビルド
    println!("📦 Running wasm-pack build...");
    let status = Command::new("wasm-pack")
        .env("CARGO_INCREMENTAL", "1")                 // 差分ビルドON🔥
        .env("RUSTFLAGS", "-C codegen-units=256")      // 並列ビルド強化💪
        .args(&[
            "build",
            "--dev",                                   // devビルドで高速化
            "--target", "web",
            "--out-dir", "pkg",
            "--no-default-features",
            "--features", "wasm",  
        ])
        .status()
        .expect("Failed to execute wasm-pack");

    if !status.success() {
        eprintln!("❌ wasm-pack build failed");
        std::process::exit(1);
    }
    
    println!("✅ WASM build completed");
    
    // 2. HTML ファイルを生成
    println!("📝 Generating HTML file...");
    
    // 絶対パスまたは相対パスを正しく解決
    let pkg_dir = if Path::new("pkg").exists() {
        Path::new("pkg")
    } else {
        // カレントディレクトリがpkgの場合
        Path::new(".")
    };
    
    // ミニマル版のみ生成
    generate_minimal_html(pkg_dir);
    
    println!("✅ HTML file generated:");
    println!("   - pkg/index.html");
    
    println!("\n🎉 Build complete! To test:");
    println!("   cd pkg && python -m http.server 8000");
    println!("   http-server -p 8000");
    println!("   Or use any static file server of your choice.");
    println!("   Then open: http://localhost:8000");
}

fn generate_minimal_html(pkg_dir: &Path) {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Nilo</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        html, body {
            width: 100%;
            height: 100%;
            overflow: auto;
        }
        #container {
            width: 100%;
            min-height: 100vh;
            position: relative;
        }
    </style>
</head>
<body>
    <div id="container"></div>
    <script type="module">
        import init from './nilo.js';
        init().catch(console.error);
    </script>
</body>
</html>
"#;
    
    fs::write(pkg_dir.join("index.html"), html)
        .expect("Failed to write index.html");
}
