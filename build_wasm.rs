use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("🚀 Building Nilo WASM with HTML generation...");

    // 1. wasm-pack でビルド
    println!("📦 Running wasm-pack build...");
    let status = Command::new("wasm-pack")
        .env("CARGO_INCREMENTAL", "1") // 差分ビルドON🔥
        .env("RUSTFLAGS", "-C codegen-units=256") // 並列ビルド強化💪
        .args(&[
            "build",
            "--dev", // devビルドで高速化
            "--target",
            "web",
            "--out-dir",
            "pkg",
            "--no-default-features",
            "--features",
            "wasm",
            "--bin",
            "nilo", // main.rsをエントリーポイントに指定
        ])
        .status()
        .expect("Failed to execute wasm-pack");

    if !status.success() {
        eprintln!("❌ wasm-pack build failed");
        std::process::exit(1);
    }

    println!("✅ WASM build completed");

    // 2. 絶対パスまたは相対パスを正しく解決
    let pkg_dir = if Path::new("pkg").exists() {
        Path::new("pkg")
    } else {
        // カレントディレクトリがpkgの場合
        Path::new(".")
    };

    // 3. nilo.js を絶対パスに修正
    println!("🔧 Fixing asset paths for SPA routing...");
    fix_asset_paths(pkg_dir);

    // 4. HTML ファイルを生成
    println!("📝 Generating HTML file...");

    // ミニマル版のみ生成
    generate_minimal_html(pkg_dir);

    println!("✅ HTML file generated:");
    println!("   - pkg/index.html");

    println!("\n🎉 Build complete! To test:");
    println!("   cd spa_server ; cargo run --release");
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
        import init from '/nilo.js';
        init().catch(console.error);
    </script>
</body>
</html>
"#;

    fs::write(pkg_dir.join("index.html"), html).expect("Failed to write index.html");
}

fn fix_asset_paths(pkg_dir: &Path) {
    let nilo_js_path = pkg_dir.join("nilo.js");

    if !nilo_js_path.exists() {
        eprintln!("⚠️  Warning: nilo.js not found, skipping path fix");
        return;
    }

    let content = fs::read_to_string(&nilo_js_path).expect("Failed to read nilo.js");

    // new URL('nilo_bg.wasm', import.meta.url) を new URL('/nilo_bg.wasm', window.location.origin) に置換
    let fixed_content = content.replace(
        "new URL('nilo_bg.wasm', import.meta.url)",
        "new URL('/nilo_bg.wasm', window.location.origin)",
    );

    fs::write(&nilo_js_path, fixed_content).expect("Failed to write fixed nilo.js");

    println!("   ✓ Fixed WASM path in nilo.js");
}
