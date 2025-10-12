pub mod ast;
pub mod parse;

// 新しいモジュール構造
pub mod utils;
pub mod expr;
pub mod flow;
pub mod timeline;
pub mod component;
pub mod style;
pub mod namespace;
pub mod types;
pub mod view_node;

use std::path::Path;
use std::fs;
use parse::parse_nilo;

//niloをパースして返す。
pub fn parse_nilo_file<P: AsRef<Path>>(path: P) -> Result<ast::App, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("IO error: {}", e))?;

    parse_nilo(&source)
}

// 埋め込まれたniloファイルをパースする関数
pub fn parse_embedded_nilo(source: &str) -> Result<ast::App, String> {
    parse_nilo(source)
}
