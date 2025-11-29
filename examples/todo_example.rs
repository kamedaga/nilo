// Todoアプリのデモです

const MY_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fonts/NotoSansJP-Regular.ttf"
));

use nilo::nilo_safe_accessible;
use nilo::nilo_state_validator;
use nilo::parser::ast::Expr;

nilo::nilo_state! {
    struct State {
        input: String,
        todos: Vec<String>,
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            input: String::new(),
            todos: vec![],
        }
    }
}

// input の文字数制限（200文字まで）
#[nilo_state_validator(state = State, field = "input")]
fn validate_input(v: String) -> Result<(), String> {
    if v.chars().count() > 200 {
        return Err("タスクは200文字以内で入力してください".into());
    }
    Ok(())
}

// Todoを追加する関数
#[nilo_safe_accessible(state = State, name = "add_todo")]
fn add_todo_fn(ctx: &mut nilo::CustomStateContext<State>, _args: &[Expr]) {
    let input = ctx.get("input").unwrap_or_default();
    let trimmed = input.trim();

    if !trimmed.is_empty() {
        // todosリストに追加
        if let Err(e) = ctx.list_append("todos", trimmed.to_string()) {
            log::error!("Failed to add todo: {}", e);
        } else {
            // 追加成功したら入力をクリア
            let _ = ctx.set("input", String::new());
            log::info!("✅ Todo added: {}", trimmed);
        }
    }
}

// Todoを削除する関数
#[nilo_safe_accessible(state = State, name = "remove_todo")]
fn remove_todo_fn(ctx: &mut nilo::CustomStateContext<State>, args: &[Expr]) {
    if let Some(Expr::String(todo)) = args.first() {
        if let Err(e) = ctx.list_remove("todos", todo.clone()) {
            log::error!("Failed to remove todo: {}", e);
        } else {
            log::info!("🗑️ Todo removed: {}", todo);
        }
    }
}

// すべてのTodoをクリアする関数
#[nilo_safe_accessible(state = State, name = "clear_all_todos")]
fn clear_all_todos_fn(ctx: &mut nilo::CustomStateContext<State>, _args: &[Expr]) {
    if let Err(e) = ctx.list_clear("todos") {
        log::error!("Failed to clear todos: {}", e);
    } else {
        log::info!("🧹 All todos cleared");
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Nilo関数を自動登録（関数・ウォッチャ・バリデータ含む）
        nilo::init_nilo_functions();

        // カスタムフォントを名前付きで登録
        nilo::set_custom_font("japanese", MY_FONT);

        let cli_args = nilo::parse_args();

        let state = State::default();

        // Todoアプリを起動
        nilo::run_nilo_app!("examples/todo.nilo", state, &cli_args, Some("Todo App"));
    }
}
