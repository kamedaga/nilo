use crate::engine::state::{AppState, CustomStateContext, StateAccess};

use crate::parser::ast::Expr;

use log;

use std::any::Any;

use std::collections::HashMap;

use std::future::Future;

use std::pin::Pin;

use std::sync::atomic::{AtomicBool, Ordering};

use std::sync::{Arc, Mutex};

/// カスタムイベント型（WinitのEventLoopProxyで使用）

#[derive(Debug, Clone)]

pub enum AsyncEvent {
    /// 非同期結果が準備完了
    AsyncResultReady,

    /// 定期実行タイマーのティック
    IntervalTick(String),
}

// 非同期タスクの結果を保存するキュー

lazy_static::lazy_static! {

    static ref ASYNC_RESULT_QUEUE: Arc<Mutex<Vec<AsyncResult>>> = Arc::new(Mutex::new(Vec::new()));

    // ★ 新規追加: 非同期結果が待機中であることを示すフラグ

    static ref ASYNC_RESULT_PENDING: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    // ★ EventLoopProxyを保存（型を消去するためAny + Send + Syncを使用）

    static ref EVENT_LOOP_PROXY: Arc<Mutex<Option<Box<dyn Any + Send + Sync>>>> = Arc::new(Mutex::new(None));

}

/// EventLoopProxyを設定（アプリ起動時に一度だけ呼ばれる）

pub fn set_event_loop_proxy<T: 'static + Send + Sync>(proxy: T) {
    if let Ok(mut p) = EVENT_LOOP_PROXY.lock() {
        *p = Some(Box::new(proxy));

        log::info!("EventLoopProxy registered for async notifications");
    }
}

/// EventLoopProxyを取得して非同期イベントを送信

fn notify_async_result_ready() {
    if let Ok(proxy_guard) = EVENT_LOOP_PROXY.lock() {
        if let Some(proxy_box) = proxy_guard.as_ref() {
            #[cfg(feature = "winit")]
            if let Some(proxy) =
                proxy_box.downcast_ref::<winit::event_loop::EventLoopProxy<AsyncEvent>>()
            {
                if let Err(e) = proxy.send_event(AsyncEvent::AsyncResultReady) {
                    log::error!("Failed to send async event: {:?}", e);
                } else {
                    log::debug!("?? Sent AsyncResultReady event to main thread");
                }
            }
        }
    }
}

/// 非同期タスクの結果

#[derive(Debug, Clone)]

pub struct AsyncResult {
    pub state_updates: HashMap<String, String>,
}

/// 非同期結果をキューに追加

pub fn queue_async_result(updates: HashMap<String, String>) {
    if let Ok(mut queue) = ASYNC_RESULT_QUEUE.lock() {
        let update_count = updates.len();

        queue.push(AsyncResult {
            state_updates: updates,
        });

        // ★ フラグを立てる

        ASYNC_RESULT_PENDING.store(true, Ordering::SeqCst);

        log::info!(
            "🔔 Queued async result with {} updates, flag set",
            update_count
        );

        // ★ メインスレッドに通知を送信

        notify_async_result_ready();
    }
}

/// 非同期結果が待機中かチェック

pub fn has_pending_async_results() -> bool {
    ASYNC_RESULT_PENDING.load(Ordering::SeqCst)
}

/// キューから結果を取得して状態に適用

pub fn apply_async_results<S>(state: &mut AppState<S>) -> bool
where
    S: StateAccess + 'static,
{
    let results = {
        if let Ok(mut queue) = ASYNC_RESULT_QUEUE.lock() {
            if queue.is_empty() {
                return false;
            }

            std::mem::take(&mut *queue)
        } else {
            return false;
        }
    };

    if results.is_empty() {
        return false;
    }

    // ★ フラグをクリア

    ASYNC_RESULT_PENDING.store(false, Ordering::SeqCst);

    log::debug!("Applying {} async results", results.len());

    for result in results {
        for (key, value) in result.state_updates {
            log::debug!("  Setting {} = {}", key, value);

            // state.xxx形式の場合はstate.をstripしてからset

            let field_name = if key.starts_with("state.") {
                key.strip_prefix("state.").unwrap()
            } else {
                &key
            };

            // ★ 特別な構文をサポート: __list_set__field_name でリストを一括設定

            if field_name.starts_with("__list_set__") {
                let actual_field = field_name.strip_prefix("__list_set__").unwrap();

                // JSON配列として解析

                // まず Vec<Value> として解釈し、String に落として格納する
                let parsed_items: Result<Vec<String>, _> =
                    serde_json::from_str::<Vec<serde_json::Value>>(&value).map(|values| {
                        values
                            .into_iter()
                            .map(|v| match v {
                                serde_json::Value::String(s) => s,
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::Bool(b) => b.to_string(),
                                serde_json::Value::Null => "null".to_string(),
                                other => other.to_string(),
                            })
                            .collect()
                    });

                let items: Vec<String> = match parsed_items {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!(
                            "Failed to parse list as JSON values ({}), falling back to Vec<String>",
                            e
                        );
                        match serde_json::from_str::<Vec<String>>(&value) {
                            Ok(v) => v,
                            Err(e2) => {
                                log::error!(
                                    "Failed to parse list for {} as Vec<String>: {}",
                                    actual_field,
                                    e2
                                );
                                continue;
                            }
                        }
                    }
                };

                // リストをクリアしてから追加
                if let Err(e) = state.custom_state.list_clear(actual_field) {
                    log::error!("Failed to clear list {}: {}", actual_field, e);
                    continue;
                }

                for item in items {
                    if let Err(e) = state.custom_state.list_append(actual_field, item.clone()) {
                        log::error!("Failed to append to {}: {}", actual_field, e);
                    }
                }

                log::info!("✅ Successfully updated list: {}", actual_field);
            } else {
                // 通常のフィールド設定

                if let Err(e) = state.custom_state.set(field_name, value) {
                    log::error!("Failed to set {}: {}", field_name, e);
                }
            }
        }
    }

    true
}

// 非同期関数の結果を処理するコールバック型

type AsyncCallback = Box<dyn FnOnce(&mut dyn Any, Result<String, String>) + Send + 'static>;

// 非同期関数: Future<Output = Result<String, String>> を返す
type AsyncFn =
    dyn Fn(&[Expr]) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> + Send + Sync;

// State付き非同期関数
type AsyncStateFn = dyn Fn(&mut dyn Any, &[Expr]) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>>
    + Send
    + Sync;

// onclick用非同期関数
type AsyncOnClickFn = dyn Fn(
        HashMap<String, String>,
        &[Expr],
    ) -> (
        HashMap<String, String>,
        Pin<Box<dyn Future<Output = HashMap<String, String>> + Send>>,
    ) + Send
    + Sync;

#[cfg(not(target_arch = "wasm32"))]
type AsyncIntervalFn = dyn Fn(HashMap<String, String>) -> Pin<Box<dyn Future<Output = HashMap<String, String>> + Send>>
    + Send
    + Sync;

#[cfg(target_arch = "wasm32")]
type AsyncIntervalFn = dyn Fn(HashMap<String, String>) -> Pin<Box<dyn Future<Output = HashMap<String, String>>>>
    + Send
    + Sync;

#[cfg(target_arch = "wasm32")]
trait IntervalFutureBound {}

#[cfg(target_arch = "wasm32")]
impl<T> IntervalFutureBound for T {}

#[cfg(not(target_arch = "wasm32"))]
trait IntervalFutureBound: Send {}

#[cfg(not(target_arch = "wasm32"))]
impl<T: Send> IntervalFutureBound for T {}

#[derive(Clone)]

struct IntervalTimer {
    name: String,

    interval_ms: u64,

    running: Arc<AtomicBool>,
}

lazy_static::lazy_static! {

    static ref ASYNC_CALL_REGISTRY: Arc<Mutex<HashMap<String, Box<AsyncFn>>>> =

        Arc::new(Mutex::new(HashMap::new()));

    static ref ASYNC_STATE_REGISTRY: Arc<Mutex<HashMap<String, Box<AsyncStateFn>>>> =

        Arc::new(Mutex::new(HashMap::new()));

    static ref ASYNC_ONCLICK_REGISTRY: Arc<Mutex<HashMap<String, Box<AsyncOnClickFn>>>> =

        Arc::new(Mutex::new(HashMap::new()));

    // ★ 定期実行用のレジストリ

    static ref ASYNC_INTERVAL_REGISTRY: Arc<Mutex<HashMap<String, Box<AsyncIntervalFn>>>> =

        Arc::new(Mutex::new(HashMap::new()));

    // ★ 実行中のタイマー管理

    static ref ACTIVE_INTERVALS: Arc<Mutex<HashMap<String, IntervalTimer>>> =

        Arc::new(Mutex::new(HashMap::new()));

}

/// 非同期onclick関数を登録（即座に初期状態を返す + 非同期処理）

pub fn register_async_onclick<F, Fut>(name: &str, func: F)
where
    F: Fn(HashMap<String, String>, &[Expr]) -> (HashMap<String, String>, Fut)
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = HashMap<String, String>> + Send + 'static,
{
    let wrapper = move |initial_state: HashMap<String, String>,

                        args: &[Expr]|
          -> (
        HashMap<String, String>,
        Pin<Box<dyn Future<Output = HashMap<String, String>> + Send>>,
    ) {
        let (immediate_result, future) = func(initial_state, args);

        (immediate_result, Box::pin(future))
    };

    ASYNC_ONCLICK_REGISTRY
        .lock()
        .unwrap()
        .insert(name.to_string(), Box::new(wrapper));

    log::debug!("Registered async onclick function: {}", name);
}

/// 非同期onclick関数を実行（即座に初期状態を適用、バックグラウンドで非同期処理）

pub fn execute_async_onclick<S>(name: &str, state: &mut AppState<S>, args: &[Expr]) -> bool
where
    S: StateAccess + 'static,
{
    let registry = ASYNC_ONCLICK_REGISTRY.lock().unwrap();

    if let Some(func) = registry.get(name) {
        let current_state = HashMap::new();

        let args_clone: Vec<Expr> = args.to_vec();

        // ★ 即座に初期状態とFutureを取得

        let (immediate_updates, future) = func(current_state, &args_clone);

        drop(registry);

        // ★ 1. 即座に初期状態を適用（同期的）

        log::info!("🔥 Applying immediate updates from async onclick: {}", name);

        for (key, value) in immediate_updates {
            let field_name = if key.starts_with("state.") {
                key.strip_prefix("state.").unwrap()
            } else {
                &key
            };

            if let Err(e) = state.custom_state.set(field_name, value.clone()) {
                log::error!("Failed to set immediate {}: {}", field_name, e);
            } else {
                log::debug!("  Immediate: {} = {}", field_name, value);
            }
        }

        // ★ レイアウトを無効化して即座に再描画

        state.needs_redraw = true;

        state.static_stencils = None;

        state.static_buttons.clear();

        state.static_text_inputs.clear();

        // ★ 2. バックグラウンドで非同期処理を実行

        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                log::info!("🚀 Starting background async task");

                let rt = tokio::runtime::Runtime::new().unwrap();

                let result = rt.block_on(future);

                log::info!("✅ Background async task completed, queuing result");

                queue_async_result(result);
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = future.await;

                queue_async_result(result);
            });
        }

        log::debug!("Spawned async onclick: {}", name);

        true
    } else {
        false
    }
}

/// 非同期関数を登録（引数のみ）

pub fn register_async_call<F, Fut>(name: &str, func: F)
where
    F: Fn(&[Expr]) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<String, String>> + Send + 'static,
{
    let wrapper =
        move |args: &[Expr]| -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> {
            Box::pin(func(args))
        };

    ASYNC_CALL_REGISTRY
        .lock()
        .unwrap()
        .insert(name.to_string(), Box::new(wrapper));

    log::debug!("Registered async function: {}", name);
}

/// 安全な非同期State付き関数を登録

pub fn register_async_safe_state_call<F, Fut, S>(name: &str, func: F)
where
    F: Fn(&mut CustomStateContext<S>, &[Expr]) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<String, String>> + Send + 'static,
    S: StateAccess + 'static,
{
    let name_owned = name.to_string();

    let wrapper = move |state: &mut dyn Any,

                        args: &[Expr]|
          -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> {
        if let Some(app_state) = state.downcast_mut::<AppState<S>>() {
            // CustomStateContextを作成してクローン可能なデータを取得

            let mut ctx = CustomStateContext::from_app_state(app_state);

            // 非同期処理を開始

            Box::pin(func(&mut ctx, args))
        } else {
            log::error!("State type mismatch in async call '{}'", name_owned);

            Box::pin(async { Err("State type mismatch".to_string()) })
        }
    };

    ASYNC_STATE_REGISTRY
        .lock()
        .unwrap()
        .insert(name.to_string(), Box::new(wrapper));

    log::debug!("Registered async safe state function: {}", name);
}

/// 非同期関数を実行（ネイティブ版）

#[cfg(not(target_arch = "wasm32"))]

pub fn execute_async_call<S>(
    name: &str,

    state: &mut AppState<S>,

    args: &[Expr],

    _on_complete: impl FnOnce(&mut AppState<S>, Result<String, String>) + Send + 'static,
) -> bool
where
    S: StateAccess + 'static,
{
    let name_owned = name.to_string();

    // まずState付き非同期関数を試す

    let state_registry = ASYNC_STATE_REGISTRY.lock().unwrap();

    if let Some(func) = state_registry.get(name) {
        let future = func(state as &mut dyn Any, args);

        drop(state_registry);

        // 非同期タスクをスポーン

        std::thread::spawn(move || {
            let result = pollster::block_on(future);

            log::info!(
                "Async function '{}' completed with result: {:?}",
                name_owned,
                result
            );
        });

        return true;
    }

    drop(state_registry);

    // 次に引数のみの非同期関数を試す

    let registry = ASYNC_CALL_REGISTRY.lock().unwrap();

    if let Some(func) = registry.get(name) {
        let future = func(args);

        drop(registry);

        let name_owned2 = name_owned.clone();

        std::thread::spawn(move || {
            let result = pollster::block_on(future);

            log::info!(
                "Async function '{}' completed with result: {:?}",
                name_owned2,
                result
            );
        });

        return true;
    }

    false
}

/// 非同期関数を実行（WASM版）

#[cfg(target_arch = "wasm32")]

pub fn execute_async_call<S>(
    name: &str,

    state: &mut AppState<S>,

    args: &[Expr],

    field_to_update: Option<String>,
) -> bool
where
    S: StateAccess + Clone + 'static,
{
    use wasm_bindgen_futures::spawn_local;

    let name_owned = name.to_string();

    // State付き非同期関数を試す

    let state_registry = ASYNC_STATE_REGISTRY.lock().unwrap();

    if let Some(func) = state_registry.get(name) {
        let future = func(state as &mut dyn Any, args);

        let field = field_to_update.clone();

        let name_clone = name_owned.clone();

        drop(state_registry);

        spawn_local(async move {
            match future.await {
                Ok(result) => {
                    log::info!(
                        "Async function '{}' completed successfully: {}",
                        name_clone,
                        result
                    );

                    if let Some(field_name) = field {
                        store_async_result(&field_name, result);
                    }
                }

                Err(e) => {
                    log::error!("Async function '{}' failed: {}", name_clone, e);
                }
            }
        });

        return true;
    }

    drop(state_registry);

    // 引数のみの非同期関数を試す

    let registry = ASYNC_CALL_REGISTRY.lock().unwrap();

    if let Some(func) = registry.get(name) {
        let future = func(args);

        let field = field_to_update;

        drop(registry);

        spawn_local(async move {
            match future.await {
                Ok(result) => {
                    log::info!(
                        "Async function '{}' completed successfully: {}",
                        name_owned,
                        result
                    );

                    if let Some(field_name) = field {
                        store_async_result(&field_name, result);
                    }
                }

                Err(e) => {
                    log::error!("Async function '{}' failed: {}", name_owned, e);
                }
            }
        });

        return true;
    }

    false
}

// WASM用: 非同期結果を一時保存

#[cfg(target_arch = "wasm32")]

thread_local! {

    static ASYNC_RESULTS: std::cell::RefCell<HashMap<String, String>> = std::cell::RefCell::new(HashMap::new());

}

#[cfg(target_arch = "wasm32")]

fn store_async_result(field: &str, result: String) {
    ASYNC_RESULTS.with(|results| {
        results.borrow_mut().insert(field.to_string(), result);
    });
}

#[cfg(target_arch = "wasm32")]

pub fn get_async_result(field: &str) -> Option<String> {
    ASYNC_RESULTS.with(|results| results.borrow_mut().remove(field))
}

/// 登録されている非同期関数が存在するかチェック

pub fn has_async_call(name: &str) -> bool {
    let state_registry = ASYNC_STATE_REGISTRY.lock().unwrap();

    if state_registry.contains_key(name) {
        return true;
    }

    drop(state_registry);

    let registry = ASYNC_CALL_REGISTRY.lock().unwrap();

    registry.contains_key(name)
}

/// 登録されている非同期onclick関数が存在するかチェック

pub fn has_async_onclick(name: &str) -> bool {
    let registry = ASYNC_ONCLICK_REGISTRY.lock().unwrap();

    registry.contains_key(name)
}

/// 定期的に実行される非同期関数を登録

///

/// # Arguments

/// * `name` - 関数の識別名

/// * `interval_ms` - 実行間隔（ミリ秒）

/// * `func` - 定期実行する非同期関数

///

/// # Example

/// ```rust

/// nilo::register_async_interval("update_data", 1000, || async {

///     // 1秒ごとに実行される処理

///     let mut updates = HashMap::new();

///     updates.insert("timestamp".to_string(), chrono::Utc::now().to_string());

///     updates

/// });

/// ```

pub fn register_async_interval<F, Fut>(name: &str, func: F)
where
    F: Fn(HashMap<String, String>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = HashMap<String, String>> + IntervalFutureBound + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    let wrapper = move |state: HashMap<String, String>| -> Pin<Box<dyn Future<Output = HashMap<String, String>> + Send>> {
        Box::pin(func(state))
    };

    #[cfg(target_arch = "wasm32")]
    let wrapper = move |state: HashMap<String, String>| -> Pin<Box<dyn Future<Output = HashMap<String, String>>>> {
        Box::pin(func(state))
    };

    ASYNC_INTERVAL_REGISTRY
        .lock()
        .unwrap()
        .insert(name.to_string(), Box::new(wrapper));

    log::info!("⏰ Registered async interval function: {}", name);
}

/// 定期実行タイマーを開始

///

/// # Arguments

/// * `name` - 登録された関数名

/// * `interval_ms` - 実行間隔（ミリ秒）

///

/// # Returns

/// タイマーが正常に開始された場合は`true`

pub fn start_async_interval(name: &str, interval_ms: u64) -> bool {
    let registry = ASYNC_INTERVAL_REGISTRY.lock().unwrap();

    if !registry.contains_key(name) {
        log::error!("Interval function '{}' is not registered", name);

        return false;
    }

    drop(registry);

    // 既に実行中かチェック

    {
        let active = ACTIVE_INTERVALS.lock().unwrap();

        if active.contains_key(name) {
            log::warn!("Interval '{}' is already running", name);

            return false;
        }
    }

    let running = Arc::new(AtomicBool::new(true));

    let timer = IntervalTimer {
        name: name.to_string(),

        interval_ms,

        running: Arc::clone(&running),
    };

    {
        let mut active = ACTIVE_INTERVALS.lock().unwrap();

        active.insert(name.to_string(), timer);
    }

    let name_owned = name.to_string();

    // バックグラウンドスレッドでタイマーを実行

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::spawn(move || {
            log::info!(
                "⏰ Started interval timer '{}' with {}ms interval",
                name_owned,
                interval_ms
            );

            let rt = tokio::runtime::Runtime::new().unwrap();

            rt.block_on(async {
                let mut interval =
                    tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));

                while running.load(Ordering::SeqCst) {
                    interval.tick().await;

                    if !running.load(Ordering::SeqCst) {
                        break;
                    }

                    log::debug!("⏰ Interval tick for '{}'", name_owned);

                    // 関数を実行

                    let registry = ASYNC_INTERVAL_REGISTRY.lock().unwrap();

                    if let Some(func) = registry.get(&name_owned) {
                        let current_state = HashMap::new();

                        let future = func(current_state);

                        drop(registry);

                        let result = future.await;

                        log::debug!(
                            "⏰ Interval '{}' completed, queuing {} updates",
                            name_owned,
                            result.len()
                        );

                        queue_async_result(result);
                    } else {
                        log::error!("Interval function '{}' not found", name_owned);

                        break;
                    }
                }

                log::info!("⏰ Stopped interval timer '{}'", name_owned);
            });
        });
    }

    #[cfg(target_arch = "wasm32")]
    {
        use gloo_timers::future::TimeoutFuture;

        use wasm_bindgen_futures::spawn_local;

        spawn_local(async move {
            log::info!(
                "⏰ Started interval timer '{}' with {}ms interval",
                name_owned,
                interval_ms
            );

            while running.load(Ordering::SeqCst) {
                TimeoutFuture::new(interval_ms as u32).await;

                if !running.load(Ordering::SeqCst) {
                    break;
                }

                log::debug!("⏰ Interval tick for '{}'", name_owned);

                let registry = ASYNC_INTERVAL_REGISTRY.lock().unwrap();

                if let Some(func) = registry.get(&name_owned) {
                    let current_state = HashMap::new();

                    let future = func(current_state);

                    drop(registry);

                    let result = future.await;

                    log::debug!(
                        "⏰ Interval '{}' completed, queuing {} updates",
                        name_owned,
                        result.len()
                    );

                    queue_async_result(result);
                } else {
                    log::error!("Interval function '{}' not found", name_owned);

                    break;
                }
            }

            log::info!("⏰ Stopped interval timer '{}'", name_owned);
        });
    }

    true
}

/// 定期実行タイマーを停止

///

/// # Arguments

/// * `name` - 停止する関数名

///

/// # Returns

/// タイマーが正常に停止された場合は`true`

pub fn stop_async_interval(name: &str) -> bool {
    let mut active = ACTIVE_INTERVALS.lock().unwrap();

    if let Some(timer) = active.remove(name) {
        timer.running.store(false, Ordering::SeqCst);

        log::info!("⏰ Stopping interval timer '{}'", name);

        true
    } else {
        log::warn!("Interval '{}' is not running", name);

        false
    }
}

/// すべての定期実行タイマーを停止

pub fn stop_all_async_intervals() {
    let mut active = ACTIVE_INTERVALS.lock().unwrap();

    for (name, timer) in active.drain() {
        timer.running.store(false, Ordering::SeqCst);

        log::info!("⏰ Stopping interval timer '{}'", name);
    }
}

/// 定期実行タイマーが実行中かチェック

pub fn is_async_interval_running(name: &str) -> bool {
    let active = ACTIVE_INTERVALS.lock().unwrap();

    active.contains_key(name)
}
