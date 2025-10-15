// 計算機のデモです。wasmでも動きますが、エントリポイントを作ってないので自分で作って下さい。
// 基本的な計算機のデモです。小数を使うことはできません。
// 例: 12 + 34 * (56 - 78) / 90

//#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
const MY_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP-Regular.ttf"));

use nilo::{nilo_state_watcher, nilo_state_validator};
use nilo::register_safe_state_call;
// register_state_accessible_call は自動登録マクロに置き換え
use nilo::{StateAccess, nilo_safe_accessible};
use nilo::parser::ast::Expr;

mod calc;

nilo::nilo_state! {
    struct State {
        // --- Calculator fields ---
        left: f64,
        right: f64,
        op: String,
        result: f64,
        error: String,
        expr: String,
        editing: String,
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            left: 0.0,
            right: 0.0,
            op: "+".into(),
            result: 0.0,
            error: String::new(),
            expr: String::new(),
            editing: "left".into(),
        }
    }
}

// ===== Demo: #[nilo_state_watcher] =====

// counter / name が更新されるたびにログに出す
#[nilo_state_watcher(state = State, fields("counter", "name"))]
fn log_state_changes(state: &mut State) {
    let c = state.get_field("counter").unwrap_or_else(|| "?".into());
    let n = state.get_field("name").unwrap_or_else(|| "".into());
    log::info!("[watcher] counter={}, name='{}'", c, n);
}

// ===== Demo: #[nilo_state_validator] =====
// name は 0 文字でない、かつ 32 文字以内
#[nilo_state_validator(state = State, field = "name")]
fn validate_name(v: String) -> Result<(), String> {
    if v.trim().is_empty() {
        return Err("name must not be empty".into());
    }
    if v.chars().count() > 32 {
        return Err("name must be <= 32 chars".into());
    }
    Ok(())
}

// ===== Calculator: validator & watcher =====

// op は + - * / のいずれかのみ許可
#[nilo_state_validator(state = State, field = "op")]
fn validate_op(v: String) -> Result<(), String> {
    match v.as_str() {
        "+" | "-" | "*" | "/" => Ok(()),
        _ => Err("operator must be one of + - * /".into()),
    }
}

// left/right/op が変わったら再計算
#[nilo_state_watcher(state = State, fields("left", "right", "op"))]
fn recalc(state: &mut State) {
    let l = state
        .get_field("left")
        .and_then(|s| s.parse::<f64>().ok());
    let r = state
        .get_field("right")
        .and_then(|s| s.parse::<f64>().ok());
    let op = state.get_field("op").unwrap_or_default();

    let mut err: Option<String> = None;
    let mut out: Option<f64> = None;

    match (l, r, op.as_str()) {
        (Some(a), Some(b), "+") => out = Some(a + b),
        (Some(a), Some(b), "-") => out = Some(a - b),
        (Some(a), Some(b), "*") => out = Some(a * b),
        (Some(_), Some(b), "/") if b == 0.0 => err = Some(String::from("division by zero")),
        (Some(a), Some(b), "/") => out = Some(a / b),
        (None, _, _) | (_, None, _) => err = Some(String::from("left/right must be numbers")),
        _ => err = Some(String::from("invalid operator")),
    }

    if let Some(v) = out {
        let _ = state.set("result", format!("{}", v));
        let _ = state.set("error", String::new());
    } else if let Some(e) = err {
        let _ = state.set("error", e);
    }
}

// expr が変わったら式を解析・評価

#[nilo_state_watcher(state = State, fields("expr"))]
fn recalc_expr(state: &mut State) {
    let expr = state.get_field("expr").unwrap_or_default();
    match crate::calc::eval_expression(&expr) {
        Ok(v) => {
            let _ = state.set("result", format!("{}", v));
            let _ = state.set("error", String::new());
        }
        Err(e) => {
            let _ = state.set("error", e);
        }
    }
}

// ===== Calculator: operator setter =====
#[nilo_safe_accessible(state = State, name = "set_op")]
fn set_op_fn(ctx: &mut nilo::CustomStateContext<State>, args: &[Expr]) {
    if let Some(Expr::String(op)) = args.first() {
        let _ = ctx.set("op", op.clone());
        // expr にも反映（演算子の前後にスペース）
        let expr = ctx.get("expr").unwrap_or_default();
        let new_expr = if expr.is_empty() {
            format!(" {} ", op)
        } else if expr.ends_with(' ') {
            format!("{}{} ", expr, op)
        } else {
            format!("{} {} ", expr, op)
        };
        let _ = ctx.set("expr", new_expr);
    }
}

// 数字トークンを expr に追加
#[nilo_safe_accessible(state = State, name = "push_digit")]
fn push_digit_fn(ctx: &mut nilo::CustomStateContext<State>, args: &[Expr]) {
    if let Some(d) = args.first() {
        let s = match d { Expr::String(s) => s.clone(), Expr::Number(n) => n.to_string(), _ => return };
        if !s.chars().all(|c| c.is_ascii_digit()) { return; }
        let expr = ctx.get("expr").unwrap_or_default();
        let new_expr = format!("{}{}", expr, s);
        let _ = ctx.set("expr", new_expr);
    }
}

// 括弧の追加
#[nilo_safe_accessible(state = State, name = "push_paren")]
fn push_paren_fn(ctx: &mut nilo::CustomStateContext<State>, args: &[Expr]) {
    if let Some(Expr::String(p)) = args.first() {
        if p == "(" || p == ")" {
            let expr = ctx.get("expr").unwrap_or_default();
            let new_expr = format!("{}{}", expr, p);
            let _ = ctx.set("expr", new_expr);
        }
    }
}

// バックスペース: 1文字削除
#[nilo_safe_accessible(state = State, name = "backspace")]
fn backspace_fn(ctx: &mut nilo::CustomStateContext<State>, _args: &[Expr]) {
    let expr = ctx.get("expr").unwrap_or_default();
    let mut chars: Vec<char> = expr.chars().collect();
    if !chars.is_empty() {
        chars.pop();
        let new_expr: String = chars.into_iter().collect();
        let _ = ctx.set("expr", new_expr);
    }
}

// クリア
#[nilo_safe_accessible(state = State, name = "clear_expr")]
fn clear_expr_fn(ctx: &mut nilo::CustomStateContext<State>, _args: &[Expr]) {
    let _ = ctx.set("expr", String::new());
}

// 編集対象の切替: "left" | "right"
#[nilo_safe_accessible(state = State, name = "set_editing")]
fn set_editing_fn(ctx: &mut nilo::CustomStateContext<State>, args: &[Expr]) {
    if let Some(Expr::String(side)) = args.first() {
        if side == "left" || side == "right" {
            let _ = ctx.set("editing", side.clone());
        }
    }
}

// 現在の編集対象に数字を追加（先頭0は置換）
#[nilo_safe_accessible(state = State, name = "append_digit")]
fn append_digit_fn(ctx: &mut nilo::CustomStateContext<State>, args: &[Expr]) {
    let digit = match args.first() {
        Some(Expr::Number(n)) => n.to_string(),
        Some(Expr::String(s)) => s.clone(),
        _ => return,
    };
    if !matches!(digit.as_str(), "0"|"1"|"2"|"3"|"4"|"5"|"6"|"7"|"8"|"9") {
        return;
    }
    let side = ctx.get("editing").unwrap_or_else(|| "left".into());
    let key = if side == "right" { "right" } else { "left" };
    let current = ctx.get(key).unwrap_or_else(|| "0".into());
    let new_val = if current == "0" { digit } else { format!("{}{}", current, digit) };
    let _ = ctx.set(key, new_val);
}

// 現在の編集対象をクリアして 0 にする
#[nilo_safe_accessible(state = State, name = "clear_current")]
fn clear_current_fn(ctx: &mut nilo::CustomStateContext<State>, _args: &[Expr]) {
    let side = ctx.get("editing").unwrap_or_else(|| "left".into());
    let key = if side == "right" { "right" } else { "left" };
    let _ = ctx.set(key, "0".into());
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Nilo関数を自動登録（関数・ウォッチャ・バリデータ含む）
        nilo::init_nilo_functions();

        register_safe_state_call("increment_counter", |ctx: &mut nilo::CustomStateContext<State>, _args| {
            if let Some(current) = ctx.get_as::<i32>("counter") {
                let _ = ctx.set("counter", (current + 1).to_string());
            }
        });
        register_safe_state_call("reset_counter", |ctx: &mut nilo::CustomStateContext<State>, _args| {
            let _ = ctx.set("counter", "0".to_string());
        });

        // ↑ 上記の関数は main 関数外で定義されているため自動登録される
        register_safe_state_call("set_name", |ctx: &mut nilo::CustomStateContext<State>, args| {
            if let Some(nilo::parser::ast::Expr::String(name)) = args.get(0) {
                let _ = ctx.set("name", name.clone());
            }
        });
        register_safe_state_call("toggle_ok", |ctx: &mut nilo::CustomStateContext<State>, _args| {
            let current = ctx.get_as::<bool>("ok").unwrap_or(false);
            let _ = ctx.set("ok", (!current).to_string());
        });
        register_safe_state_call("add_item", |ctx: &mut nilo::CustomStateContext<State>, args| {
            if let Some(nilo::parser::ast::Expr::Number(n)) = args.get(0) {
                let _ = ctx.list_append("items", n.to_string());
            }
        });

        // onclick互換レジストリへの assign ラッパー登録は未使用

        // カスタムフォントを名前付きで登録
        nilo::set_custom_font("japanese", MY_FONT);
        
        let cli_args = nilo::parse_args();

        let state = State::default();
        
        // デモアプリを起動（マクロ側で "src/" を付与するため、ファイル名のみ指定）
        nilo::run_nilo_app!("examples/calc.nilo", state, &cli_args, Some("Nilo State Demo"));
    }
}
