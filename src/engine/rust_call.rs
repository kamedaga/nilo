use crate::engine::state::{AppState, CustomStateContext, StateAccess};
use crate::parser::ast::Expr;
use log;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ========================================
// 型付き引数変換トレイト
// ========================================

/// Exprから各型への変換トレイト
pub trait FromExpr: Sized {
    fn from_expr(expr: &Expr) -> Option<Self>;
}

impl FromExpr for String {
    fn from_expr(expr: &Expr) -> Option<Self> {
        match expr {
            Expr::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl FromExpr for i32 {
    fn from_expr(expr: &Expr) -> Option<Self> {
        match expr {
            Expr::Number(n) => Some(*n as i32),
            _ => None,
        }
    }
}

impl FromExpr for f32 {
    fn from_expr(expr: &Expr) -> Option<Self> {
        match expr {
            Expr::Number(n) => Some(*n),
            _ => None,
        }
    }
}

impl FromExpr for f64 {
    fn from_expr(expr: &Expr) -> Option<Self> {
        match expr {
            Expr::Number(n) => Some(*n as f64),
            _ => None,
        }
    }
}

impl FromExpr for bool {
    fn from_expr(expr: &Expr) -> Option<Self> {
        match expr {
            Expr::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

// Niloの無名構造体 Object(Vec<(String, Expr)>) をHashMapに変換
impl FromExpr for HashMap<String, Expr> {
    fn from_expr(expr: &Expr) -> Option<Self> {
        match expr {
            Expr::Object(fields) => {
                let mut map = HashMap::new();
                for (key, value) in fields {
                    map.insert(key.clone(), value.clone());
                }
                Some(map)
            }
            _ => None,
        }
    }
}

/// 従来の引数のみを受け取る関数型
type RustCallFn = dyn Fn(&[Expr]) + Send + Sync;

/// stateアクセス可能な関数型（非推奨 - 危険）
type StateAccessibleFn = dyn Fn(&mut dyn Any, &[Expr]) + Send + Sync;

/// カスタムステートのみに安全にアクセス可能な関数型（推奨）
type SafeStateCallFn = dyn Fn(&mut dyn Any, &[Expr]) + Send + Sync;

lazy_static::lazy_static! {
    static ref RUST_CALL_REGISTRY: Arc<Mutex<HashMap<String, Box<RustCallFn>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // stateアクセス可能な関数の登録（非推奨）
    static ref STATE_ACCESSIBLE_REGISTRY: Arc<Mutex<HashMap<String, Box<StateAccessibleFn>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // カスタムステートのみにアクセス可能な安全な関数の登録（推奨）
    static ref SAFE_STATE_REGISTRY: Arc<Mutex<HashMap<String, Box<SafeStateCallFn>>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

// Distributed-slice bootstrap for state-accessible registrations (native only)
#[cfg(not(target_arch = "wasm32"))]
#[linkme::distributed_slice]
pub static STATE_ACCESSIBLE_BOOTSTRAP: [fn()] = [..];

/// Initialize state-accessible functions registered via linkme
#[cfg(not(target_arch = "wasm32"))]
pub fn initialize_state_accessible_functions() {
    for init_fn in STATE_ACCESSIBLE_BOOTSTRAP {
        init_fn();
    }
}

/// 従来の引数のみを受け取る関数を登録
pub fn register_rust_call<F>(name: &str, func: F)
where
    F: Fn(&[Expr]) + Send + Sync + 'static,
{
    RUST_CALL_REGISTRY
        .lock()
        .unwrap()
        .insert(name.to_string(), Box::new(func));
}

// ========================================
// 型付き引数を受け取る関数の登録（エレガントな実装）
// ========================================

/// 関数の引数をExpr配列から変換するトレイト
pub trait FromExprArgs: Sized {
    fn from_expr_args(args: &[Expr]) -> Result<Self, String>;
}

/// 関数を呼び出し可能にするトレイト
pub trait CallableFn<Args>: Send + Sync + 'static {
    fn call(&self, args: Args);
}

// 引数なし
impl FromExprArgs for () {
    fn from_expr_args(args: &[Expr]) -> Result<Self, String> {
        if args.is_empty() {
            Ok(())
        } else {
            Err(format!("Expected 0 arguments, got {}", args.len()))
        }
    }
}

impl<F> CallableFn<()> for F
where
    F: Fn() + Send + Sync + 'static,
{
    fn call(&self, _args: ()) {
        self()
    }
}

// 1個以上の引数（マクロで一度だけ実装）
macro_rules! impl_typed_call {
    ($($T:ident),+) => {
        #[allow(non_snake_case)]
        impl<$($T: FromExpr),+> FromExprArgs for ($($T,)+) {
            fn from_expr_args(args: &[Expr]) -> Result<Self, String> {
                #[allow(unused_mut, unused_variables)]
                let mut idx = 0;
                $(
                    let $T = $T::from_expr(args.get(idx).ok_or_else(|| {
                        format!("Expected at least {} arguments, got {}", idx + 1, args.len())
                    })?)
                    .ok_or_else(|| format!("Failed to convert argument {}", idx + 1))?;
                    idx += 1;
                )+

                if args.len() != idx {
                    return Err(format!("Expected {} arguments, got {}", idx, args.len()));
                }

                Ok(($($T,)+))
            }
        }

        #[allow(non_snake_case)]
        impl<F, $($T),+> CallableFn<($($T,)+)> for F
        where
            F: Fn($($T),+) + Send + Sync + 'static,
        {
            fn call(&self, ($($T,)+): ($($T,)+)) {
                self($($T),+)
            }
        }
    };
}

// 実用的な範囲（1〜4個）のみサポート
// ※ 5個以上の引数が必要な場合は、構造体にまとめることを推奨
impl_typed_call!(T1);
impl_typed_call!(T1, T2);
impl_typed_call!(T1, T2, T3);
impl_typed_call!(T1, T2, T3, T4);

// 必要に応じてコメントを外して拡張可能
// impl_typed_call!(T1, T2, T3, T4, T5);
// impl_typed_call!(T1, T2, T3, T4, T5, T6);

/// **統一された型付き関数登録API**
///
/// # 使用例
/// ```rust
/// use std::collections::HashMap;
/// use crate::parser::ast::Expr;
///
/// #[nilo_function]
/// fn greet() { println!("Hello!"); }
///
/// #[nilo_function]
/// fn log(msg: String) { println!("{}", msg); }
///
/// #[nilo_function]
/// fn add(a: i32, b: i32) { println!("{}", a + b); }
///
/// // Niloの無名構造体を受け取る例
/// #[nilo_function]
/// fn create_user(data: HashMap<String, Expr>) {
///     // data["name"]、data["age"] などでアクセス可能
///     println!("User data: {:?}", data);
/// }
/// ```
///
/// # Niloでの呼び出し例
/// ```nilo
/// rust_call create_user({ name: "Alice", age: 30, active: true })
/// ```
pub fn register_typed_call<F, Args>(name: &str, func: F)
where
    Args: FromExprArgs,
    F: CallableFn<Args>,
{
    let name_owned = name.to_string();
    let wrapper = move |args: &[Expr]| match Args::from_expr_args(args) {
        Ok(converted_args) => {
            func.call(converted_args);
        }
        Err(e) => {
            log::error!("Function '{}': {}", name_owned, e);
        }
    };
    RUST_CALL_REGISTRY
        .lock()
        .unwrap()
        .insert(name.to_string(), Box::new(wrapper));
}

/// カスタムステートのみに安全にアクセスする関数を登録（推奨）
/// この関数はエンジンの内部状態にアクセスできず、ユーザー定義のカスタムステートのみを扱える
pub fn register_safe_state_call<F, S>(name: &str, func: F)
where
    F: Fn(&mut CustomStateContext<S>, &[Expr]) + Send + Sync + 'static,
    S: StateAccess + 'static,
{
    let name_owned = name.to_string();
    let wrapper = move |state: &mut dyn Any, args: &[Expr]| {
        if let Some(app_state) = state.downcast_mut::<AppState<S>>() {
            let mut ctx = CustomStateContext::from_app_state(app_state);
            func(&mut ctx, args);
        } else {
            log::error!(
                "Error: State type mismatch in safe Rust call '{}'",
                name_owned
            );
        }
    };

    SAFE_STATE_REGISTRY
        .lock()
        .unwrap()
        .insert(name.to_string(), Box::new(wrapper));
}

/// stateにアクセス可能な関数を登録（非推奨 - 危険）
/// ⚠️ この関数はエンジンの内部状態にフルアクセスできるため、使用は推奨されません
/// 代わりに register_safe_state_call を使用してください
#[deprecated(note = "Use register_safe_state_call instead for safer state access")]
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
            log::error!("Error: State type mismatch in Rust call '{}'", name_owned);
        }
    };

    STATE_ACCESSIBLE_REGISTRY
        .lock()
        .unwrap()
        .insert(name.to_string(), Box::new(wrapper));
}

/// 汎用的なstateアクセス関数を登録（Any型での型安全性を犠牲にしてより柔軟に）
pub fn register_generic_state_call<F>(name: &str, func: F)
where
    F: Fn(&mut dyn Any, &[Expr]) + Send + Sync + 'static,
{
    STATE_ACCESSIBLE_REGISTRY
        .lock()
        .unwrap()
        .insert(name.to_string(), Box::new(func));
}

/// 従来の引数のみの関数を実行
pub fn execute_rust_call(name: &str, args: &[Expr]) {
    let registry = RUST_CALL_REGISTRY.lock().unwrap();
    if let Some(func) = registry.get(name) {
        func(args);
    } else {
        log::warn!(
            "Warning: Rust call '{}' is not registered in basic registry",
            name
        );
    }
}

/// 登録されている関数が存在するかチェック
pub fn has_rust_call(name: &str) -> bool {
    let registry = RUST_CALL_REGISTRY.lock().unwrap();
    registry.contains_key(name)
}

/// 安全なカスタムステートアクセス関数を実行（推奨）
pub fn execute_safe_state_call<S>(name: &str, state: &mut AppState<S>, args: &[Expr]) -> bool
where
    S: StateAccess + 'static,
{
    let registry = SAFE_STATE_REGISTRY.lock().unwrap();
    if let Some(func) = registry.get(name) {
        func(state as &mut dyn Any, args);
        true
    } else {
        false
    }
}

/// stateアクセス可能な関数を実行（非推奨）
#[deprecated(note = "Use execute_safe_state_call instead for safer state access")]
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

/// 全てのレジストリから関数を探して実行（安全なカスタムステート優先）
pub fn execute_any_rust_call<S>(name: &str, state: &mut AppState<S>, args: &[Expr]) -> bool
where
    S: StateAccess + 'static,
{
    // まず安全なカスタムステートアクセス関数を試す（最優先）
    if execute_safe_state_call(name, state, args) {
        return true;
    }

    // 次にstateアクセス可能な関数を試す（後方互換性のため）
    #[allow(deprecated)]
    if execute_state_accessible_call(name, state, args) {
        return true;
    }

    // 最後に従来の関数を試す
    let registry = RUST_CALL_REGISTRY.lock().unwrap();
    if let Some(func) = registry.get(name) {
        func(args);
        true
    } else {
        log::warn!(
            "Warning: Rust call '{}' is not registered in any registry",
            name
        );
        false
    }
}

/// 登録されている関数の一覧を取得（デバッグ用）
pub fn list_registered_calls() -> (Vec<String>, Vec<String>, Vec<String>) {
    let basic_registry = RUST_CALL_REGISTRY.lock().unwrap();
    let state_registry = STATE_ACCESSIBLE_REGISTRY.lock().unwrap();
    let safe_registry = SAFE_STATE_REGISTRY.lock().unwrap();

    let basic_calls: Vec<String> = basic_registry.keys().cloned().collect();
    let state_calls: Vec<String> = state_registry.keys().cloned().collect();
    let safe_calls: Vec<String> = safe_registry.keys().cloned().collect();

    (basic_calls, state_calls, safe_calls)
}
