# Nilo SPA Server

シンプルなHTTPサーバーで、SPAのクライアントサイドルーティングをサポートします。

## 特徴

- ✅ 静的ファイル配信
- ✅ SPAルーティング（すべてのルートを`index.html`にフォールバック）
- ✅ 適切なMIMEタイプ設定
- ✅ セキュリティ（ディレクトリトラバーサル攻撃防止）
- ✅ CORSヘッダー対応
- ✅ マルチスレッド対応

## 使い方

### 基本的な使用方法

```bash
cd spa_server
cargo run
```

デフォルトで`http://localhost:8000`でサーバーが起動し、`../pkg`ディレクトリを配信します。

### リリースビルド（高速）

```bash
cargo run --release
```

### カスタムポートとディレクトリ

```bash
# コマンドライン引数で指定
cargo run -- 3000 /path/to/directory

# 環境変数で指定
PORT=3000 ROOT_DIR=./dist cargo run
```

## SPAルーティングの仕組み

このサーバーは以下のロジックでリクエストを処理します：

1. ファイルが存在する場合 → そのファイルを返す
2. ファイルが存在せず、拡張子がない、または`.html`の場合 → `index.html`を返す（SPAルート）
3. それ以外 → 404エラー

例：
- `GET /` → `index.html`を返す
- `GET /page1` → `index.html`を返す（SPAルート）
- `GET /nilo.js` → `nilo.js`ファイルを返す
- `GET /not-exist.png` → 404エラー

## Niloプロジェクトでの使用

Niloの開発中にWASMアプリをテストする場合：

```bash
# ルートディレクトリでWASMをビルド
cd /path/to/nilomain
cargo run --bin build_wasm_with_html

# SPAサーバーを起動
cd spa_server
cargo run --release

# ブラウザで開く
# http://localhost:8000
```

## ライセンス

このプロジェクトはNiloフレームワークの一部です。
