# Nilo WASM Build Script with Auto HTML Generation

Write-Host "🚀 Building Nilo for WebAssembly..." -ForegroundColor Green

# wasm-packがインストールされているか確認
if (-not (Get-Command wasm-pack -ErrorAction SilentlyContinue)) {
    Write-Host "wasm-pack is not installed. Installing..." -ForegroundColor Yellow
    cargo install wasm-pack
}

# Rustビルドスクリプトを実行（WASM + HTML生成）
Write-Host "📦 Running build script with HTML generation..." -ForegroundColor Cyan
cargo run --bin build_wasm_with_html

if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ Build successful!" -ForegroundColor Green
    
    Write-Host "`n📁 Generated files in pkg/:" -ForegroundColor Cyan
    Write-Host "  - index.html      (フルスクリーン表示)" -ForegroundColor White
    Write-Host "  - nilo.js         (WASM bindings)" -ForegroundColor White
    Write-Host "  - nilo_bg.wasm    (WASM binary)" -ForegroundColor White
    
    Write-Host "`n🌐 To test locally:" -ForegroundColor Yellow
    Write-Host "  cd pkg" -ForegroundColor White
    Write-Host "  python -m http.server 8000" -ForegroundColor White
    Write-Host "`n📱 Then open in your browser:" -ForegroundColor Yellow
    Write-Host "  http://localhost:8000" -ForegroundColor White
} else {
    Write-Host "❌ Build failed!" -ForegroundColor Red
    exit 1
}

