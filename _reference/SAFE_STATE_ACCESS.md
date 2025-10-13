# 安全なカスタムステートアクセス

## 概要

従来の`register_state_accessible_call`は`AppState`全体にアクセスでき、エンジンの内部状態を誤って変更してしまう危険性がありました。

新しい`register_safe_state_call`では、**カスタムステートのみ**に安全にアクセスできる`CustomStateContext`を使用します。

## 使用方法

### ✅ 推奨：安全なカスタムステートアクセス

```rust
use nilo::engine::rust_call::register_safe_state_call;
use nilo::engine::state::CustomStateContext;
use nilo::parser::ast::Expr;

// カスタムステートのみにアクセスする安全な関数
fn increment_counter<S>(ctx: &mut CustomStateContext<S>, _args: &[Expr])
where
    S: nilo::engine::state::StateAccess,
{
    // 値を型変換して取得
    let current = ctx.get_as::<u32>("counter").unwrap_or(0);
    let new_value = current + 1;
    
    // 値を型変換して設定
    let _ = ctx.set_value("counter", new_value);
    
    println!("Counter: {} -> {}", current, new_value);
}

fn main() {
    // 安全な関数として登録
    register_safe_state_call("increment_counter", increment_counter::<State>);
    
    // ... アプリケーションを実行
}
```

### ❌ 非推奨：AppState全体にアクセス（危険）

```rust
// 非推奨 - エンジンの内部状態にもアクセスできてしまう
fn increment_counter<S>(state: &mut AppState<S>, _args: &[Expr])
where
    S: nilo::engine::state::StateAccess,
{
    // 危険！ state.current_timeline, state.position などエンジンの重要な状態にもアクセスできる
    let current = state.custom_state.get_field("counter")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    
    // ...
}
```

## CustomStateContext の API

### 値の取得

```rust
// 文字列として取得
let value: Option<String> = ctx.get("key");

// 型変換して取得
let count: Option<u32> = ctx.get_as::<u32>("counter");
let name: Option<String> = ctx.get_as::<String>("name");
let enabled: Option<bool> = ctx.get_as::<bool>("enabled");
```

### 値の設定

```rust
// 文字列として設定
ctx.set("key", "value".to_string())?;

// 型変換して設定（Display traitを実装している型）
ctx.set_value("counter", 42)?;
ctx.set_value("name", "Alice")?;
ctx.set_value("enabled", true)?;
```

### ブール値のトグル

```rust
ctx.toggle("enabled")?;
```

### リスト操作

```rust
// リストに値を追加
ctx.list_append("items", "new_item".to_string())?;

// リストの指定位置に値を挿入
ctx.list_insert("items", 0, "first_item".to_string())?;

// リストから値を削除
ctx.list_remove("items", "old_item".to_string())?;

// リストをクリア
ctx.list_clear("items")?;
```

## メリット

1. **安全性**: エンジンの内部状態（`current_timeline`, `position`, `component_context`など）にアクセスできない
2. **シンプル**: `custom_state.get_field()`のような冗長な書き方が不要
3. **型安全**: `get_as<T>()`や`set_value<T>()`で型変換を簡潔に記述
4. **保守性**: カスタムステートのみを扱うため、意図しない副作用が発生しない

## 移行ガイド

### Before（危険な方法）

```rust
use nilo::engine::rust_call::register_state_accessible_call;
use nilo::engine::state::AppState;

fn my_function<S>(state: &mut AppState<S>, args: &[Expr])
where
    S: nilo::engine::state::StateAccess,
{
    let value = state.custom_state.get_field("key")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    
    let _ = state.custom_state.set("key", (value + 1).to_string());
}

register_state_accessible_call("my_function", my_function::<State>);
```

### After（安全な方法）

```rust
use nilo::engine::rust_call::register_safe_state_call;
use nilo::engine::state::CustomStateContext;

fn my_function<S>(ctx: &mut CustomStateContext<S>, args: &[Expr])
where
    S: nilo::engine::state::StateAccess,
{
    let value = ctx.get_as::<u32>("key").unwrap_or(0);
    let _ = ctx.set_value("key", value + 1);
}

register_safe_state_call("my_function", my_function::<State>);
```

## 注意事項

- `register_state_accessible_call`は非推奨（deprecated）としてマークされています
- 既存のコードとの後方互換性のため、まだ使用可能ですが、新しいコードでは`register_safe_state_call`を使用してください
- `execute_any_rust_call`は両方のレジストリをチェックし、安全な関数を優先的に実行します
