pub mod ast;
pub mod parse;

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
