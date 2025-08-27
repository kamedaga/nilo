// ========================================
// Nilo アプリケーション メインエントリーポイント
// ========================================

use nilo;
use nilo::engine::rust_call::register_rust_call;
use nilo::parser::ast::Expr; // Exprをインポート
use colored::*;

// ========================================
// アプリケーション状態の定義
// ========================================

/// アプリケーションの状態を管理する構造体
/// StateAccessトレイトを自動実装するためのderive属性を使用
#[derive(Debug, Clone, serde::Serialize, nilo_state_access_derive::StateAccess)]
#[state_access(trait_path = "::nilo::engine::state::StateAccess")] // StateAccessトレイトの実際のパス
struct State {
    /// アプリケーション名
    name: String,
    /// 一意識別子
    id: u32,
}

fn test(args: &[Expr]) {
    println!("test called with {} arguments", args.len());
    for (i, arg) in args.iter().enumerate() {
        println!("  arg[{}]: {:?}", i, arg);
    }
}

// ========================================
// メイン関数（WebAssembly以外のターゲット用）
// ========================================

/// アプリケーションのエントリーポイント
/// WebAssembly以外のプラットフォームでのみコンパイルされる
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // コマンドライン引数の解析
    let cli_args = nilo::parse_args();

    // Rust関数の登録
    register_rust_call("test", test);

    // 初期状態の作成
    let state = State {
        name: "Nilo framework".to_string(),
        id: 1,
    };

    let file_path = "src/test_control_flow.nilo";

    // アプリケーションの実行（lib.rsに移設した関数を使用）
    nilo::run_application(file_path, state, &cli_args, Some("Nilo Demo Application"));
}
