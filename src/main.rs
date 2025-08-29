use nilo;
use nilo::engine::rust_call::register_rust_call;
use nilo::parser::ast::Expr;

nilo::nilo_state! {
    struct State {
        name: String,
        counter: u32,
    }
}

fn hello_world(args: &[Expr]) {
    println!("Hello from Rust! Args: {:?}", args);
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let cli_args = nilo::parse_args();

    register_rust_call("hello_rust", |_args: &[Expr]| {
        println!("Hello from Rust!");
    });

    register_rust_call("hello_world", hello_world);

    let state = State {
        name: "Nilo".to_string(),
        counter: 1,
    };

    nilo::run_application("src/tutorial.nilo", state, &cli_args, Some("Nilo Tutorial"));
}
