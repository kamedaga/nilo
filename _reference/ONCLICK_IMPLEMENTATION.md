# onclick実装ガイド - Buttonから直接Rust関数を呼び出す

## 概要

Niloフレームワークでは、`.nilo`ファイルのButton要素の`onclick`属性に関数を指定することで、直接Rust関数を呼び出すことができます。

## 基本的な使い方

### 1. .niloファイルでの記述

```nilo
Button(
    id: "my_btn",
    label: "Click Me",
    onclick: hello_from_rust(),
    style: {
        background: "#4CAF50",
        color: "#fff",
        padding: 15,
        rounded: on
    }
)
```

### 2. Rust関数の定義と登録

```rust
use nilo::parser::ast::Expr;
use nilo::engine::rust_call::register_rust_call;

// 引数なしの基本的な関数
fn hello_from_rust(args: &[Expr]) {
    println!("Hello from Rust!");
}

// main関数またはアプリ初期化時に登録
fn main() {
    register_rust_call("hello_from_rust", hello_from_rust);
    // ... アプリケーションの起動
}
```

## 引数付き関数の呼び出し

### .niloでの記述

```nilo
Button(
    id: "greet_btn",
    label: "Greet",
    onclick: greet_user("Taro", 25)
)
```

### Rust関数の実装

```rust
fn greet_user(args: &[Expr]) {
    // argsには Expr 型で値が渡される
    // 実際の使用では、Expressionを評価する必要がある
    if args.len() >= 2 {
        // 引数の処理...
        println!("Greeting user with args: {:?}", args);
    }
}

// 登録
register_rust_call("greet_user", greet_user);
```

## Stateにアクセスする関数

アプリケーションの状態（state）を読み書きする関数の場合、`register_state_accessible_call`を使用します。

### State型の定義

```rust
use nilo::engine::state::StateAccess;

#[derive(Debug, Clone, Default)]
pub struct MyAppState {
    pub counter: i32,
    pub username: String,
}

impl StateAccess for MyAppState {
    fn get_var(&self, name: &str) -> Option<String> {
        match name {
            "counter" => Some(self.counter.to_string()),
            "username" => Some(self.username.clone()),
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, value: String) {
        match name {
            "counter" => {
                if let Ok(val) = value.parse::<i32>() {
                    self.counter = val;
                }
            }
            "username" => {
                self.username = value;
            }
            _ => {}
        }
    }
}
```

### State変更関数の実装

```rust
use nilo::parser::ast::Expr;
use nilo::engine::state::AppState;
use nilo::engine::rust_call::register_state_accessible_call;

// カウンターをインクリメント
fn increment_counter<S>(state: &mut AppState<S>, _args: &[Expr])
where
    S: StateAccess,
{
    let current = state.custom_state.get_var("counter")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);
    
    let new_value = current + 1;
    state.custom_state.set_var("counter", new_value.to_string());
    
    println!("Counter incremented: {} -> {}", current, new_value);
}

// 登録
register_state_accessible_call("increment_counter", increment_counter::<MyAppState>);
```

### .niloでの使用

```nilo
timeline Main {
    VStack(style: { gap: 20, padding: 40 }) {
        // カウンターの値を表示
        Text("Counter: {}", state.counter, style: {
            font_size: 20,
            color: "#333"
        })

        // インクリメントボタン
        Button(
            id: "inc_btn",
            label: "Increment",
            onclick: increment_counter(),
            style: {
                background: "#FF9800",
                color: "#fff",
                padding: 15,
                rounded: on
            }
        )
    }
}
```

## 完全な実装例

### `src/onclick_test.nilo`

```nilo
timeline Main {
    VStack(style: { gap: 20, padding: 40, background: "#f5f5f5" }) {
        Text("onclick Test: Rust関数を直接呼び出す", style: {
            font_size: 28,
            color: "#333",
            padding: 10
        })

        // 基本的なRust関数呼び出し
        Button(
            id: "simple_btn",
            label: "Simple Click",
            onclick: hello_from_rust(),
            style: {
                background: "#4CAF50",
                color: "#fff",
                padding: 15,
                rounded: on
            }
        )

        // カウンターをインクリメント
        Button(
            id: "increment_btn",
            label: "Increment Counter",
            onclick: increment_counter(),
            style: {
                background: "#FF9800",
                color: "#fff",
                padding: 15,
                rounded: on
            }
        )

        // カウンター値を表示
        Text("Counter: {}", state.counter, style: {
            font_size: 20,
            color: "#333",
            padding: 10
        })

        // ログ出力
        Button(
            id: "log_btn",
            label: "Log Message",
            onclick: log_message("Button clicked from nilo!"),
            style: {
                background: "#9C27B0",
                color: "#fff",
                padding: 15,
                rounded: on
            }
        )
    }
}

flow {
    start: Main
    Main -> []
}
```

### `examples/onclick_example.rs`

```rust
use nilo::parser::ast::Expr;
use nilo::engine::state::{AppState, StateAccess};
use nilo::engine::rust_call::{register_rust_call, register_state_accessible_call};
use log::info;

// State型の定義
#[derive(Debug, Clone, Default)]
pub struct MyAppState {
    pub counter: i32,
    pub username: String,
}

impl StateAccess for MyAppState {
    fn get_var(&self, name: &str) -> Option<String> {
        match name {
            "counter" => Some(self.counter.to_string()),
            "username" => Some(self.username.clone()),
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, value: String) {
        match name {
            "counter" => {
                if let Ok(val) = value.parse::<i32>() {
                    self.counter = val;
                }
            }
            "username" => {
                self.username = value;
            }
            _ => {}
        }
    }
}

// 基本的な関数
fn hello_from_rust(args: &[Expr]) {
    info!("🎉 Hello from Rust! Called with {} arguments", args.len());
}

fn log_message(args: &[Expr]) {
    if let Some(Expr::String(msg)) = args.first() {
        info!("📝 Log: {}", msg);
    } else {
        info!("📝 Log called with {:?}", args);
    }
}

// State変更関数
fn increment_counter<S>(state: &mut AppState<S>, _args: &[Expr])
where
    S: StateAccess,
{
    let current = state.custom_state.get_var("counter")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);
    
    let new_value = current + 1;
    state.custom_state.set_var("counter", new_value.to_string());
    
    info!("✅ Counter incremented: {} -> {}", current, new_value);
}

// 関数の登録
pub fn register_all_onclick_functions() {
    // 基本的な関数
    register_rust_call("hello_from_rust", hello_from_rust);
    register_rust_call("log_message", log_message);
    
    // State変更関数
    register_state_accessible_call("increment_counter", increment_counter::<MyAppState>);
    
    info!("✅ All onclick functions registered");
}

fn main() {
    env_logger::init();
    
    // Rust関数を登録
    register_all_onclick_functions();
    
    // アプリケーションの初期化
    let my_state = MyAppState {
        counter: 0,
        username: "Guest".to_string(),
    };
    
    let mut app_state = AppState::new(my_state, "Main".to_string());
    
    println!("onclick example ready!");
    println!("Use onclick_test.nilo to test the functionality");
}
```

## 技術詳細

### 内部動作フロー

1. **レンダリング時**: Buttonの`onclick`属性が`button_onclick_map`に登録される
2. **クリック時**: `handle_button_onclick`が呼び出される
3. **式の評価**: `onclick`の式が`Expr::FunctionCall`かチェック
4. **関数実行**: 
   - `execute_onclick_function_call`が呼び出される
   - `execute_state_accessible_call`で state 変更関数を優先実行
   - 見つからない場合は`execute_rust_call`で基本関数を実行

### 関数登録の仕組み

```rust
// 基本関数のレジストリ（stateアクセスなし）
static ref RUST_CALL_REGISTRY: HashMap<String, Box<RustCallFn>>

// State変更可能関数のレジストリ
static ref STATE_ACCESSIBLE_REGISTRY: HashMap<String, Box<StateAccessibleFn>>
```

### onclick式の評価順序

1. **FunctionCall判定**: `onclick`が関数呼び出しかチェック
2. **State関数優先**: `STATE_ACCESSIBLE_REGISTRY`から検索
3. **基本関数フォールバック**: 見つからない場合は`RUST_CALL_REGISTRY`から検索
4. **警告出力**: 両方で見つからない場合は警告ログ

## ベストプラクティス

### 1. 関数は早期に登録する

```rust
fn main() {
    env_logger::init();
    
    // 最初に登録
    register_all_onclick_functions();
    
    // その後アプリ起動
    let app = load_nilo_app();
    run_app(app);
}
```

### 2. State変更が必要な場合は適切な型を使用

```rust
// ❌ 悪い例: stateを変更するのに register_rust_call を使用
register_rust_call("increment", increment);  // stateにアクセスできない

// ✅ 良い例: state変更関数には register_state_accessible_call
register_state_accessible_call("increment_counter", increment_counter::<MyAppState>);
```

### 3. エラーハンドリング

```rust
fn safe_divide<S>(state: &mut AppState<S>, args: &[Expr])
where
    S: StateAccess,
{
    if args.len() < 2 {
        log::error!("❌ safe_divide requires 2 arguments");
        state.custom_state.set_var("error", "Invalid arguments".to_string());
        return;
    }
    
    // 処理...
}
```

### 4. ログ出力で動作確認

```rust
fn my_function<S>(state: &mut AppState<S>, args: &[Expr])
where
    S: StateAccess,
{
    log::info!("🔧 my_function called with {} args", args.len());
    
    // 処理...
    
    log::info!("✅ my_function completed");
}
```

## トラブルシューティング

### 関数が呼ばれない

**症状**: ボタンをクリックしても何も起こらない

**確認事項**:
1. 関数が登録されているか: `register_rust_call` または `register_state_accessible_call` を呼んだか
2. 関数名が一致しているか: `.nilo`ファイルとRust側で同じ名前か
3. ログを確認: `RUST_LOG=info cargo run` でログを確認

### State変更が反映されない

**症状**: 関数は呼ばれるがUIが更新されない

**原因**: `register_rust_call`で登録してしまっている

**解決策**: `register_state_accessible_call`を使用する

```rust
// ❌ これだとstateを変更できない
register_rust_call("update_count", update_count);

// ✅ 正しい登録方法
register_state_accessible_call("update_count", update_count::<MyAppState>);
```

### 型エラーが発生

**エラー**: `the trait bound ... is not satisfied`

**原因**: State型が`StateAccess`を実装していない

**解決策**: State型に`StateAccess`を実装する

```rust
impl StateAccess for MyAppState {
    fn get_var(&self, name: &str) -> Option<String> {
        // 実装
    }

    fn set_var(&mut self, name: &str, value: String) {
        // 実装
    }
}
```

## まとめ

- `onclick`属性で直接Rust関数を呼び出せる
- 基本関数は`register_rust_call`で登録
- State変更関数は`register_state_accessible_call`で登録
- 関数はアプリ起動前に登録する
- エラーハンドリングとログ出力を忘れずに

この機能により、UIとビジネスロジックを綺麗に分離し、宣言的なUIと命令的なロジックを組み合わせることができます。
