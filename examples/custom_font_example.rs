/// カスタムフォント使用例
///
/// このサンプルでは、カスタムフォントを埋め込む方法を示します。
use nilo::*;

// オプション1: システムフォントのみ（デフォルト）
// 何も設定しなければシステムフォントが使用されます

// オプション2: カスタムフォントを埋め込む
// フォントファイルがある場合は以下のようにします
// 重要: concat!(env!("CARGO_MANIFEST_DIR"), "/path")でプロジェクトルートからの絶対パスを指定
// const CUSTOM_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP-Regular.ttf"));

nilo_state! {
    struct AppState {
        counter: i32,
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self { counter: 0 }
    }
}

fn main() {
    // ========================================
    // フォント設定（オプション）
    // ========================================

    // 方法1: システムフォントを使用（デフォルト）
    // 何も呼ばなくてOK

    // 方法2: カスタムフォントを埋め込む
    // set_custom_font(Some(CUSTOM_FONT));

    // 方法3: 明示的にシステムフォントを設定
    // set_custom_font(None);

    // ========================================
    // アプリケーション起動
    // ========================================

    let state = AppState::default();
    let cli_args = parse_args();

    // Niloアプリケーションを実行
    // フォント設定は自動的に適用されます
    run_nilo_app!("src/app.nilo", state, &cli_args);
}
