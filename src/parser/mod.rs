pub mod ast;
pub mod parse;

// 新しいモジュール構造
pub mod component;
pub mod expr;
pub mod flow;
pub mod namespace;
pub mod style;
pub mod timeline;
pub mod types;
pub mod utils;
pub mod view_node;

use parse::parse_nilo;
use std::fs;
use std::path::Path;

//niloをパースして返す。
pub fn parse_nilo_file<P: AsRef<Path>>(path: P) -> Result<ast::App, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("IO error: {}", e))?;

    parse_nilo(&source)
}

// 埋め込まれたniloファイルをパースする関数
pub fn parse_embedded_nilo(source: &str) -> Result<ast::App, String> {
    parse_nilo(source)
}
