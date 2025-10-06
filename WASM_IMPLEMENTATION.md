# WASM版実装ドキュメント

## 概要

Nilo UIフレームワークをWebAssembly (WASM)環境で動作させるためのリファクタリングと実装を行いました。

## 実装内容

### 1. アーキテクチャの分離

- **Native環境**: `winit` + `wgpu` を使用したデスクトップアプリケーション
- **WASM環境**: DOM APIを使用したWebブラウザアプリケーション

### 2. 主な変更点

#### 2.1 Cargo.toml

依存関係を環境別に分離：

```toml
[features]
default = ["native"]
native = ["wgpu", "winit", "pollster", "glyphon", "colored", "notify"]
wasm = ["wasm-bindgen", "web-sys", "console_error_panic_hook", "wasm-bindgen-futures", "placeholder", "console_log"]
```

#### 2.2 新しいモジュール構成

- `src/engine/runtime.rs` - Native環境専用のランタイム(winitベース)
- `src/engine/runtime_dom.rs` - WASM環境専用のランタイム(DOM APIベース)
- `src/dom_renderer/` - DOMベースのレンダラー実装

#### 2.3 条件付きコンパイル

主要なモジュールに条件付きコンパイルディレクティブを追加：

```rust
// Native専用
#[cfg(not(target_arch = "wasm32"))]
pub mod runtime;

#[cfg(feature = "wgpu")]
pub mod wgpu_renderer;

#[cfg(feature = "glyphon")]
pub mod text_measurement;

// WASM専用
#[cfg(target_arch = "wasm32")]
pub mod runtime_dom;
```

### 3. DOMレンダラー

`DomRenderer`は以下の機能を提供：

- HTML要素を使用した矩形、円、三角形の描画
- テキストレンダリング
- 画像表示
- イベントハンドリング(マウスイベント)
- スクロールとスケーリングのサポート

### 4. WASMエントリーポイント

`src/main.rs`の`wasm_main`関数がWASM環境のエントリーポイントとして機能：

```rust
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    // パニックハンドラとロガーの初期化
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).expect("error initializing log");
    
    // カスタムフォント登録
    nilo::set_custom_font("japanese", MY_FONT);
    
    // Rust関数登録
    register_rust_call("hello_rust", |_args| {
        log::info!("Hello from Rust!");
    });
    
    // 初期状態作成
    let state = State { ... };
    
    // DOMレンダラーでNiloアプリを実行
    nilo::run_nilo_wasm(APP_NILO, state);
}
```

### 5. ビルド方法

#### Native版(デスクトップ)

```powershell
cargo build
cargo run
```

#### WASM版

```powershell
# wasm-packを使用
wasm-pack build --target web --out-dir pkg

# または build_wasm.ps1を実行
.\build_wasm.ps1
```

### 6. ローカルテスト

```powershell
cd pkg
python -m http.server 8000
```

ブラウザで `http://localhost:8000` を開く

## 現在の状態

### 完了したタスク

- ✅ モジュール構造の条件付きコンパイル対応
- ✅ DOMレンダラーの基本実装
- ✅ WASM専用ランタイムの実装
- ✅ イベントハンドリングの基本機能
- ✅ Cargo.tomlの依存関係分離

### 未完了/進行中のタスク

- ⚠️ text_measurementモジュールのWASM対応(glyphonの代替実装が必要)
- ⚠️ レイアウトシステムのWASM環境でのテスト
- ⚠️ `colored`クレートの使用箇所の条件付きコンパイル対応
- ⚠️ 完全なビルドエラーの解決

### 既知の課題

1. **glyphonクレート**: WASM環境で利用できないため、テキスト測定にブラウザAPIを使用する代替実装が必要
2. **colored**: ターミナル出力用のクレートなので、WASM環境では無効化が必要
3. **テキストレンダリング**: DOM版ではブラウザのフォントレンダリングを使用、Native版とは異なる挙動の可能性

## 次のステップ

1. `src/ui/layout.rs`でのtext_measurement使用箇所を条件付きコンパイル対応
2. WASM環境用のシンプルなテキスト測定関数の実装
3. `colored`の使用箇所を全て条件付きに修正
4. WASMビルドを完全に成功させる
5. ブラウザでの動作テスト
6. パフォーマンス最適化

## 参考資料

- [wasm-bindgen](https://rustwasm.github.io/wasm-bindgen/)
- [web-sys](https://rustwasm.github.io/wasm-bindgen/api/web_sys/)
- [Rust and WebAssembly](https://rustwasm.github.io/docs/book/)
