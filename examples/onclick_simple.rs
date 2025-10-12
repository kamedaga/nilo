/// Simple onclick demonstration example
/// 
/// This example shows how to register and use onclick handlers with Rust functions

use nilo::parser::parse::parse_nilo;
use nilo::engine::state::{AppState, StateAccess};
use nilo::engine::rust_call::{register_rust_call, register_state_accessible_call};
use nilo::parser::ast::Expr;
use log::info;

// Define your application state
#[derive(Debug, Clone)]
struct MyAppState {
    counter: i32,
    message: String,
}

impl Default for MyAppState {
    fn default() -> Self {
        Self {
            counter: 0,
            message: "Welcome!".to_string(),
        }
    }
}

// Implement StateAccess trait to allow state reading/writing
impl StateAccess for MyAppState {
    fn get_field(&self, name: &str) -> Option<String> {
        match name {
            "counter" => Some(self.counter.to_string()),
            "message" => Some(self.message.clone()),
            _ => None,
        }
    }

    fn set(&mut self, path: &str, value: String) -> Result<(), String> {
        match path {
            "counter" => {
                if let Ok(val) = value.parse::<i32>() {
                    self.counter = val;
                    Ok(())
                } else {
                    Err(format!("Invalid counter value: {}", value))
                }
            }
            "message" => {
                self.message = value;
                Ok(())
            }
            _ => Err(format!("Unknown field: {}", path))
        }
    }

    fn toggle(&mut self, _path: &str) -> Result<(), String> {
        Err("toggle not implemented".to_string())
    }

    fn list_append(&mut self, _path: &str, _value: String) -> Result<(), String> {
        Err("list_append not implemented".to_string())
    }

    fn list_insert(&mut self, _path: &str, _index: usize, _value: String) -> Result<(), String> {
        Err("list_insert not implemented".to_string())
    }

    fn list_remove(&mut self, _path: &str, _value: String) -> Result<(), String> {
        Err("list_remove not implemented".to_string())
    }

    fn list_clear(&mut self, _path: &str) -> Result<(), String> {
        Err("list_clear not implemented".to_string())
    }
}

// ========================================
// Basic Rust functions (no state access)
// ========================================

fn hello_from_rust(_args: &[Expr]) {
    info!("🎉 Hello from Rust! Button was clicked!");
    println!("Hello from Rust function!");
}

fn log_message(args: &[Expr]) {
    if let Some(Expr::String(msg)) = args.first() {
        info!("📝 Log: {}", msg);
        println!("Log: {}", msg);
    } else {
        info!("📝 Log called with {:?}", args);
        println!("Log called with arguments: {:?}", args);
    }
}

fn greet_user(args: &[Expr]) {
    info!("👋 greet_user called with {} arguments", args.len());
    println!("Greeting user with {} arguments", args.len());
    
    // In a real implementation, you would evaluate the Expr arguments
    for (i, arg) in args.iter().enumerate() {
        println!("  Arg {}: {:?}", i, arg);
    }
}

// ========================================
// State-accessible Rust functions
// ========================================

fn increment_counter<S>(state: &mut AppState<S>, _args: &[Expr])
where
    S: StateAccess,
{
    // Get current counter value
    let current = state.custom_state.get_field("counter")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);
    
    // Increment
    let new_value = current + 1;
    
    // Update state
    let _ = state.custom_state.set("counter", new_value.to_string());
    
    info!("✅ Counter incremented: {} -> {}", current, new_value);
    println!("Counter incremented: {} -> {}", current, new_value);
}

fn reset_counter<S>(state: &mut AppState<S>, _args: &[Expr])
where
    S: StateAccess,
{
    let _ = state.custom_state.set("counter", "0".to_string());
    info!("🔄 Counter reset to 0");
    println!("Counter reset to 0");
}

fn update_message<S>(state: &mut AppState<S>, args: &[Expr])
where
    S: StateAccess,
{
    if let Some(Expr::String(msg)) = args.first() {
        let _ = state.custom_state.set("message", msg.clone());
        info!("💬 Message updated to: {}", msg);
        println!("Message updated to: {}", msg);
    } else {
        info!("⚠️ update_message requires a string argument");
        println!("Warning: update_message requires a string argument");
    }
}

// Register all onclick functions
fn register_onclick_functions() {
    // Register basic functions (no state access)
    register_rust_call("hello_from_rust", hello_from_rust);
    register_rust_call("log_message", log_message);
    register_rust_call("greet_user", greet_user);
    
    // Register state-accessible functions
    register_state_accessible_call("increment_counter", increment_counter::<MyAppState>);
    register_state_accessible_call("reset_counter", reset_counter::<MyAppState>);
    register_state_accessible_call("update_message", update_message::<MyAppState>);
    
    info!("✅ All onclick functions registered");
    println!("✅ All onclick functions registered");
}

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    println!("==============================================");
    println!("  onclick Implementation Example");
    println!("==============================================\n");
    
    // Register onclick functions BEFORE loading the app
    register_onclick_functions();
    
    // Create initial state
    let initial_state = MyAppState::default();
    println!("Initial state:");
    println!("  counter: {}", initial_state.counter);
    println!("  message: {}\n", initial_state.message);
    
    // Create AppState
    let app_state = AppState::new(initial_state, "Main".to_string());
    
    println!("Application state created.");
    println!("\nTo use onclick:");
    println!("1. Load onclick_test.nilo file");
    println!("2. Click buttons in the UI");
    println!("3. Rust functions will be called automatically\n");
    
    println!("Example .nilo syntax:");
    println!("  Button(id: \"btn1\", label: \"Click\", onclick: hello_from_rust())");
    println!("  Button(id: \"btn2\", label: \"+1\", onclick: increment_counter())");
    println!("  Button(id: \"btn3\", label: \"Reset\", onclick: reset_counter())\n");
    
    println!("State after initialization:");
    println!("  counter: {:?}", app_state.custom_state.get_field("counter"));
    println!("  message: {:?}", app_state.custom_state.get_field("message"));
    
    println!("\n==============================================");
    println!("  Ready! Use with your nilo application");
    println!("==============================================");
}
