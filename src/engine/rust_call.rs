use crate::parser::ast::Expr;
use crate::engine::state::{AppState, StateAccess};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::any::Any;

// 従来の引数のみを受け取る関数型
type RustCallFn = dyn Fn(&[Expr]) + Send + Sync;

// stateアクセス可能な関数型
type StateAccessibleFn = dyn Fn(&mut dyn Any, &[Expr]) + Send + Sync;

lazy_static::lazy_static! {
    static ref RUST_CALL_REGISTRY: Arc<Mutex<HashMap<String, Box<RustCallFn>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    
    // stateアクセス可能な関数の登録
    static ref STATE_ACCESSIBLE_REGISTRY: Arc<Mutex<HashMap<String, Box<StateAccessibleFn>>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

/// 従来の引数のみを受け取る関数を登録
pub fn register_rust_call<F>(name: &str, func: F)
where
    F: Fn(&[Expr]) + Send + Sync + 'static,
{
    RUST_CALL_REGISTRY.lock().unwrap().insert(name.to_string(), Box::new(func));
}

/// stateにアクセス可能な関数を登録
pub fn register_state_accessible_call<F, S>(name: &str, func: F)
where
    F: Fn(&mut AppState<S>, &[Expr]) + Send + Sync + 'static,
    S: StateAccess + 'static,
{
    let name_owned = name.to_string(); // nameをクローンして所有権を取得
    let wrapper = move |state: &mut dyn Any, args: &[Expr]| {
        if let Some(typed_state) = state.downcast_mut::<AppState<S>>() {
            func(typed_state, args);
        } else {
            eprintln!("Error: State type mismatch in Rust call '{}'", name_owned);
        }
    };
    
    STATE_ACCESSIBLE_REGISTRY.lock().unwrap().insert(name.to_string(), Box::new(wrapper));
}

/// 汎用的なstateアクセス関数を登録（Any型での型安全性を犠牲にしてより柔軟に）
pub fn register_generic_state_call<F>(name: &str, func: F)
where
    F: Fn(&mut dyn Any, &[Expr]) + Send + Sync + 'static,
{
    STATE_ACCESSIBLE_REGISTRY.lock().unwrap().insert(name.to_string(), Box::new(func));
}

/// 従来の引数のみの関数を実行
pub fn execute_rust_call(name: &str, args: &[Expr]) {
    let registry = RUST_CALL_REGISTRY.lock().unwrap();
    if let Some(func) = registry.get(name) {
        func(args);
    } else {
        eprintln!("Warning: Rust call '{}' is not registered in basic registry", name);
    }
}

/// 登録されている関数が存在するかチェック
pub fn has_rust_call(name: &str) -> bool {
    let registry = RUST_CALL_REGISTRY.lock().unwrap();
    registry.contains_key(name)
}

/// stateアクセス可能な関数を実行
pub fn execute_state_accessible_call<S>(name: &str, state: &mut AppState<S>, args: &[Expr]) -> bool
where
    S: StateAccess + 'static,
{
    let registry = STATE_ACCESSIBLE_REGISTRY.lock().unwrap();
    if let Some(func) = registry.get(name) {
        func(state as &mut dyn Any, args);
        true
    } else {
        false
    }
}

/// 両方のレジストリから関数を探して実行（stateアクセス優先）
pub fn execute_any_rust_call<S>(name: &str, state: &mut AppState<S>, args: &[Expr]) -> bool
where
    S: StateAccess + 'static,
{
    // まずstateアクセス可能な関数を試す
    if execute_state_accessible_call(name, state, args) {
        return true;
    }
    
    // 従来の関数を試す
    let registry = RUST_CALL_REGISTRY.lock().unwrap();
    if let Some(func) = registry.get(name) {
        func(args);
        return true;
    }
    
    eprintln!("Error: Rust call '{}' is not registered in any registry", name);
    false
}

/// 登録されている関数の一覧を取得（デバッグ用）
pub fn list_registered_calls() -> (Vec<String>, Vec<String>) {
    let basic_registry = RUST_CALL_REGISTRY.lock().unwrap();
    let state_registry = STATE_ACCESSIBLE_REGISTRY.lock().unwrap();
    
    let basic_calls: Vec<String> = basic_registry.keys().cloned().collect();
    let state_calls: Vec<String> = state_registry.keys().cloned().collect();
    
    (basic_calls, state_calls)
}
