#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use nilo;
use nilo::engine::rust_call::register_rust_call;
use nilo::parser::ast::Expr;
use log::info; // ログマクロを追加

nilo::nilo_state! {
    struct State {
        name: String,
        counter: u32,
        items: Vec<i32>,
        ifbool: bool
    }
}

fn hello_world(args: &[Expr]) {
    println!("Hello from Rust! Args: {:?}", args); // println!をinfo!に変更
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let cli_args = nilo::parse_args();

    register_rust_call("hello_rust", |_args: &[Expr]| {
        info!("Hello from Rust!"); // println!をinfo!に変更
    });

    register_rust_call("hello_world", hello_world);

    let state = State {
        name: "Nilo".to_string(),
        counter: 1,
        items: vec![1, 2, 3],
        ifbool: true
    };

    // 自動で埋め込みファイルを使用するマクロを呼び出し
    nilo::run_nilo_app!("app.nilo", state, &cli_args, Some("Nilo Tutorial"));
}
