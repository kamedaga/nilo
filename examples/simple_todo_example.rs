use nilo::nilo_safe_accessible;
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

#[nilo_safe_accessible(state = State, name = "add_todo")]
fn add_todo_fn(ctx: &mut nilo::CustomStateContext<State>, _args: &[Expr]) {
    let input = ctx.get("input").unwrap_or_default();
    let trimmed = input.trim();
    
    if !trimmed.is_empty() {
        let _ = ctx.list_append("todos", trimmed.to_string());
        let _ = ctx.set("input", String::new());
    }
}

#[nilo_safe_accessible(state = State, name = "remove_todo")]
fn remove_todo_fn(ctx: &mut nilo::CustomStateContext<State>, args: &[Expr]) {
    if let Some(Expr::String(task)) = args.first() {
        let _ = ctx.list_remove("todos", task.clone());
    }
}

#[nilo_safe_accessible(state = State, name = "clear_all")]
fn clear_all_fn(ctx: &mut nilo::CustomStateContext<State>, _args: &[Expr]) {
    let _ = ctx.list_clear("todos");
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        nilo::init_nilo_functions();
        
        let cli_args = nilo::parse_args();
        let state = State::default();
        
        nilo::run_nilo_app!("examples/test.nilo", state, &cli_args, Some("Test"));
    }
}
