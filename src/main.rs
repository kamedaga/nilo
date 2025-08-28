// ========================================
// Nilo アプリケーション メインエントリーポイント
// ========================================

use nilo;
use nilo::engine::rust_call::register_rust_call;
use nilo::parser::ast::Expr; // Exprをインポート

// ========================================
// アプリケーション状態の定義
// ========================================

// アプリケーションの状態を管理する構造体
// nilo_state!マクロを使用して簡潔に定義
nilo::nilo_state! {
    struct State {
        /// アプリケーション名
        name: String,
        /// 一意識別子
        counter: u32,
    }
}

fn hello_world(args: &[Expr]) {
    println!("Hello from Rust! Args: {:?}", args);
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

    register_rust_call("hello_rust", |_args: &[Expr]| {
        println!("Hello from Rust!");
    });

    register_rust_call("hello_world", hello_world);


    // 初期状態の作成
    let state = State {
        name: "Nilo".to_string(),
        counter: 1,
    };

    let file_path = "src/tutorial.nilo";

    // アプリケーションの実行（lib.rsに移設した関数を使用）
    nilo::run_application(file_path, state, &cli_args, Some("Nilo Tutorial"));
}
