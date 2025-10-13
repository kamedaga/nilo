# 型付き関数登録 (Typed Function Registration)

## 概要

Niloから呼び出すRust関数を、`&[Expr]`配列ではなく、**型付き引数**で直接定義できるようになりました。

## 従来の方法 vs 新しい方法

### ❌ 従来の方法（配列で受け取る）

```rust
use nilo::engine::rust_call::register_rust_call;
use nilo::parser::ast::Expr;

fn open_url(args: &[Expr]) {
    if let Some(Expr::String(url)) = args.first() {
        // URLを開く処理
        println!("Opening: {}", url);
    } else {
        log::warn!("Invalid argument");
    }
}

// 登録
register_rust_call("open_url", open_url);
```

### ✅ 新しい方法（型付き引数）

```rust
use nilo::register_typed_call_1;

fn open_url(url: String) {
    // URLを開く処理
    println!("Opening: {}", url);
}

// 登録
register_typed_call_1("open_url", open_url);
```

## 使用可能な型

以下の型を引数として使用できます：

- `String` - 文字列
- `i32` - 整数
- `f32` - 浮動小数点数（32bit）
- `f64` - 浮動小数点数（64bit）
- `bool` - 真偽値

## 登録関数一覧

### `register_typed_call_0` - 引数なし

```rust
use nilo::register_typed_call_0;

fn greet() {
    println!("Hello from Nilo!");
}

register_typed_call_0("greet", greet);
```

Niloファイルから呼び出し：
```nilo
timeline Main {
    view {
        onclick: greet()
    }
}
```

### `register_typed_call_1` - 引数1つ

```rust
use nilo::register_typed_call_1;

fn log_message(message: String) {
    println!("Log: {}", message);
}

register_typed_call_1("log_message", log_message);
```

Niloファイルから呼び出し：
```nilo
timeline Main {
    view {
        onclick: log_message("Hello")
    }
}
```

### `register_typed_call_2` - 引数2つ

```rust
use nilo::register_typed_call_2;

fn add_numbers(a: i32, b: i32) {
    println!("{} + {} = {}", a, b, a + b);
}

register_typed_call_2("add_numbers", add_numbers);
```

Niloファイルから呼び出し：
```nilo
timeline Main {
    view {
        onclick: add_numbers(10, 20)
    }
}
```

### `register_typed_call_3` - 引数3つ

```rust
use nilo::register_typed_call_3;

fn calculate(x: f32, y: f32, z: f32) {
    println!("Result: {}", x * y + z);
}

register_typed_call_3("calculate", calculate);
```

Niloファイルから呼び出し：
```nilo
timeline Main {
    view {
        onclick: calculate(1.5, 2.0, 3.0)
    }
}
```

### `register_typed_call_4` - 引数4つ

```rust
use nilo::register_typed_call_4;

fn process_rgba(r: i32, g: i32, b: i32, a: f32) {
    println!("Color: rgba({}, {}, {}, {})", r, g, b, a);
}

register_typed_call_4("process_rgba", process_rgba);
```

Niloファイルから呼び出し：
```nilo
timeline Main {
    view {
        onclick: process_rgba(255, 128, 64, 0.8)
    }
}
```

### `register_typed_call_5` - 引数5つ

```rust
use nilo::register_typed_call_5;

fn create_user(name: String, age: i32, email: String, active: bool, score: f32) {
    println!("User: {} ({}) - {} - active: {} - score: {}", 
             name, age, email, active, score);
}

register_typed_call_5("create_user", create_user);
```

Niloファイルから呼び出し：
```nilo
timeline Main {
    view {
        onclick: create_user("Alice", 30, "alice@example.com", true, 95.5)
    }
}
```

## 完全な例

```rust
use nilo;
use nilo::{register_typed_call_0, register_typed_call_1, register_typed_call_2, register_typed_call_5};

const MY_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP-Regular.ttf"));

// 型付き関数の定義
fn open_url(url: String) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Err(e) = open::that(&url) {
            log::error!("Failed to open URL: {}", e);
        }
    }
    
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.open_with_url_and_target(&url, "_blank");
        }
    }
}

fn log_message(message: String) {
    println!("📝 {}", message);
}

fn add_numbers(a: i32, b: i32) {
    println!("➕ {} + {} = {}", a, b, a + b);
}

fn greet() {
    println!("👋 Hello!");
}

fn create_user(name: String, age: i32, email: String, active: bool, score: f32) {
    println!("User created: {} ({}) - {} - active: {} - score: {}", 
             name, age, email, active, score);
}

nilo::nilo_state! {
    struct State {
        counter: u32,
    }
}

fn main() {
    nilo::set_custom_font("japanese", MY_FONT);
    
    let cli_args = nilo::parse_args();

    // 型付き関数を登録
    register_typed_call_1("open_url", open_url);        // String 1つ
    register_typed_call_1("log_message", log_message);  // String 1つ
    register_typed_call_2("add_numbers", add_numbers);  // i32 2つ
    register_typed_call_0("greet", greet);              // 引数なし
    register_typed_call_5("create_user", create_user);  // 5つの引数

    let state = State { counter: 0 };

    nilo::run_nilo_app!("app.nilo", state, &cli_args);
}
```

## エラーハンドリング

型変換が失敗した場合、自動的にエラーログが出力されます：

```
[ERROR] Function 'add_numbers' expects 2 arguments, got 1
[ERROR] Function 'log_message': Failed to convert argument 1
```

## カスタム型の追加

独自の型を追加したい場合は、`FromExpr`トレイトを実装します：

```rust
use nilo::FromExpr;
use nilo::parser::ast::Expr;

#[derive(Debug)]
struct MyCustomType {
    value: String,
}

impl FromExpr for MyCustomType {
    fn from_expr(expr: &Expr) -> Option<Self> {
        match expr {
            Expr::String(s) => Some(MyCustomType {
                value: s.clone(),
            }),
            _ => None,
        }
    }
}

// 使用例
fn process_custom(data: MyCustomType) {
    println!("Custom: {:?}", data);
}

// 注意: カスタム型用の登録関数は自作する必要があります
```

## まとめ

- ✅ **シンプル**: `fn open_url(url: String)` のように自然に書ける
- ✅ **型安全**: コンパイル時に型チェックされる
- ✅ **明確**: 引数の型が明示的
- ✅ **エラー処理**: 自動的に型変換エラーをハンドリング
- ✅ **複数引数対応**: 0〜5個の引数をサポート

従来の`&[Expr]`方式も引き続き使用できますが、新しい型付き方式の使用を推奨します。

## さらに多くの引数が必要な場合

6個以上の引数が必要な場合は、従来の`register_rust_call`を使用するか、構造体にまとめることを検討してください：

```rust
// 従来の方法で6個以上の引数を処理
use nilo::engine::rust_call::register_rust_call;
use nilo::parser::ast::Expr;

fn complex_function(args: &[Expr]) {
    if args.len() != 6 {
        log::error!("Expected 6 arguments");
        return;
    }
    
    let Some(Expr::String(a)) = args.get(0) else { return; };
    let Some(Expr::Number(b)) = args.get(1) else { return; };
    // ... 残りの引数も同様に処理
}

register_rust_call("complex_function", complex_function);
```
