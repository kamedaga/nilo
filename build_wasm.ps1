# Nilo WASM Build Script

Write-Host "Building Nilo for WebAssembly..." -ForegroundColor Green

# wasm-packがインストールされているか確認
if (-not (Get-Command wasm-pack -ErrorAction SilentlyContinue)) {
    Write-Host "wasm-pack is not installed. Installing..." -ForegroundColor Yellow
    cargo install wasm-pack
}

# WASMビルド
Write-Host "Running wasm-pack build..." -ForegroundColor Cyan
wasm-pack build --target web --out-dir pkg

if ($LASTEXITCODE -eq 0) {
    Write-Host "Build successful!" -ForegroundColor Green
    
    # pkgディレクトリにindex.htmlをコピー
    Write-Host "Copying index.html to pkg directory..." -ForegroundColor Cyan
    Copy-Item "src\wasm\index.html" "pkg\index.html" -Force
    
    Write-Host "`nWASM build complete!" -ForegroundColor Green
    Write-Host "To test locally, run:" -ForegroundColor Yellow
    Write-Host "  cd pkg" -ForegroundColor White
    Write-Host "  python -m http.server 8000" -ForegroundColor White
    Write-Host "Then open http://localhost:8000 in your browser" -ForegroundColor White
} else {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}
