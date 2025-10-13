use log::info;
use nilo::engine::rust_call::{register_rust_call, register_state_accessible_call};
use nilo::engine::state::{AppState, StateAccess};
/// onclick_example.rs
///
/// onclick属性からRust関数を呼び出す実装例
///
/// 使い方:
/// 1. Rust関数を定義
/// 2. register_rust_call または register_state_accessible_call で登録
/// 3. .niloファイルのButton onclick属性で関数名を指定
///
/// 例:
/// ```nilo
/// Button(id: "test_btn", label: "Click", onclick: my_function("arg1", 42))
/// ```
use nilo::parser::ast::Expr;

// ========================================
// State型の定義（アプリケーション固有の状態）
// ========================================

#[derive(Debug, Clone, Default)]
pub struct MyAppState {
    pub counter: i32,
    pub username: String,
}

impl StateAccess for MyAppState {
    fn get_field(&self, name: &str) -> Option<String> {
        match name {
            "counter" => Some(self.counter.to_string()),
            "username" => Some(self.username.clone()),
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
            "username" => {
                self.username = value;
                Ok(())
            }
            _ => Err(format!("Unknown field: {}", path)),
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
// 基本的なRust関数（引数のみを受け取る）
// ========================================

/// 引数なしの単純な関数
fn hello_from_rust(args: &[Expr]) {
    info!("🎉 Hello from Rust! Called with {} arguments", args.len());
}

/// 引数ありの関数（文字列と数値を受け取る）
fn greet_user(args: &[Expr]) {
    if args.len() >= 2 {
        // 引数は Expr 型なので、必要に応じて評価する
        info!("Greeting user with args: {:?}", args);
        // 実際の使用では eval_expr_from_ast で評価する必要がある
    } else {
        info!("⚠️ greet_user requires at least 2 arguments");
    }
}

/// ログ出力関数
fn log_message(args: &[Expr]) {
    if let Some(Expr::String(msg)) = args.first() {
        info!("📝 Log: {}", msg);
    } else {
        info!("📝 Log called with {:?}", args);
    }
}

// ========================================
// stateにアクセスできるRust関数
// ========================================

/// カウンターをインクリメント（stateを変更）
fn increment_counter<S>(state: &mut AppState<S>, _args: &[Expr])
where
    S: StateAccess,
{
    // stateからcounter値を取得
    let current = state
        .custom_state
        .get_field("counter")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);

    let new_value = current + 1;

    // stateを更新
    let _ = state.custom_state.set("counter", new_value.to_string());

    info!("✅ Counter incremented: {} -> {}", current, new_value);
}

/// カウンターをリセット
fn reset_counter<S>(state: &mut AppState<S>, _args: &[Expr])
where
    S: StateAccess,
{
    let _ = state.custom_state.set("counter", "0".to_string());
    info!("🔄 Counter reset to 0");
}

/// ユーザー名を設定
fn set_username<S>(state: &mut AppState<S>, args: &[Expr])
where
    S: StateAccess,
{
    if let Some(Expr::String(name)) = args.first() {
        let _ = state.custom_state.set("username", name.clone());
        info!("👤 Username set to: {}", name);
    }
}

// ========================================
// 関数の登録
// ========================================

/// すべてのRust関数を登録する
pub fn register_all_onclick_functions() {
    // 基本的な関数（stateにアクセスしない）
    register_rust_call("hello_from_rust", hello_from_rust);
    register_rust_call("greet_user", greet_user);
    register_rust_call("log_message", log_message);

    // stateにアクセスする関数
    register_state_accessible_call("increment_counter", increment_counter::<MyAppState>);
    register_state_accessible_call("reset_counter", reset_counter::<MyAppState>);
    register_state_accessible_call("set_username", set_username::<MyAppState>);

    info!("✅ All onclick functions registered");
}

// ========================================
// メイン関数での使用例
// ========================================

fn main() {
    // ロギング初期化
    env_logger::init();

    // Rust関数を登録
    register_all_onclick_functions();

    // アプリケーションの初期化
    let my_state = MyAppState {
        counter: 0,
        username: "Guest".to_string(),
    };

    // AppStateを作成
    let _app_state = AppState::new(my_state, "Main".to_string());

    // この後、niloファイルをロードしてアプリケーションを実行
    // ...

    println!("onclick example ready!");
    println!("Use onclick_test.nilo to test the functionality");
}

// ========================================
// 高度な使用例
// ========================================

/// 複数の引数を受け取る複雑な関数
fn complex_function<S>(state: &mut AppState<S>, args: &[Expr])
where
    S: StateAccess,
{
    info!("🔧 Complex function called with {} args", args.len());

    // 引数を評価（実際の実装では state.eval_expr_from_ast を使用）
    for (i, arg) in args.iter().enumerate() {
        info!("  Arg {}: {:?}", i, arg);
    }

    // stateの値を読み取り
    if let Some(counter) = state.custom_state.get_field("counter") {
        info!("  Current counter: {}", counter);
    }

    // 何かしらの処理...
    // let _ = state.custom_state.set("result", "processed".to_string());
}

/// 非同期処理を行う関数（将来的な拡張例）
#[allow(dead_code)]
fn async_operation<S>(_state: &mut AppState<S>, _args: &[Expr])
where
    S: StateAccess,
{
    info!("🚀 Starting async operation...");

    // 実際の非同期処理はここに実装
    // 例: APIリクエスト、ファイル読み込み、データベースアクセスなど

    info!("✅ Async operation completed");
}

// ========================================
// エラーハンドリングの例
// ========================================

/// エラーハンドリング付きの関数
fn safe_division<S>(_state: &mut AppState<S>, args: &[Expr])
where
    S: StateAccess,
{
    if args.len() < 2 {
        info!("❌ Error: safe_division requires 2 arguments");
        return;
    }

    // 引数から数値を取得（実際の実装では eval_expr_from_ast を使用）
    // let numerator = ...;
    // let denominator = ...;

    // if denominator == 0.0 {
    //     info!("❌ Error: Division by zero");
    //     let _ = state.custom_state.set("error", "Division by zero".to_string());
    //     return;
    // }

    // let result = numerator / denominator;
    // let _ = state.custom_state.set("result", result.to_string());
    // info!("✅ Division result: {}", result);
}
