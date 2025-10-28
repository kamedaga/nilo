/// WASMブラウザ実行関連のモジュール

pub mod browser_editor;

// エントリポイントを再エクスポート
pub use browser_editor::run_nilo_code_from_browser;
