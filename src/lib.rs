pub mod analysis;
pub mod dom_renderer;
pub mod engine;
#[cfg(not(target_arch = "wasm32"))]
pub mod hotreload;
pub mod parser;
pub mod renderer_abstract;
pub mod stencil;
pub mod ui;
#[cfg(target_arch = "wasm32")]
pub mod wasm;
#[cfg(feature = "wgpu")]
pub mod wgpu_renderer;

#[cfg(feature = "colored")]
#[allow(unused_imports)]
use colored::*;
use log::{error, info}; 
use parser::{ast::App, parse_embedded_nilo, parse_nilo_file};
use std::collections::HashMap;
use std::env;
use std::sync::{OnceLock, RwLock};

// Niloアプリの埋め込み版実行用マクロ
// ========================================
#[cfg(not(target_arch = "wasm32"))]
#[linkme::distributed_slice]
pub static NILO_FUNCTION_REGISTRY: [fn()] = [..];

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub use engine::exec::{AppState, StateAccess};
#[cfg(not(target_arch = "wasm32"))]
pub use engine::runtime::run;
pub use renderer_abstract::RendererType;

// 安全なカスタムステートアクセス API の公開
pub use engine::state::CustomStateContext;
pub use engine::state::register_state_watcher;

// 非同期関数登録 API の公開
pub use engine::async_call::{
    register_async_call,
    register_async_onclick,
    register_async_safe_state_call,
    has_async_call,
    has_async_onclick,
    has_pending_async_results,
    set_event_loop_proxy,
    AsyncEvent,
    // 定期実行用の関数
    register_async_interval,
    start_async_interval,
    stop_async_interval,
    stop_all_async_intervals,
    is_async_interval_running,
};

// 型付き関数登録 API の公開
#[allow(deprecated)]
pub use engine::rust_call::{
    CallableFn,
    FromExpr,
    FromExprArgs,
    register_rust_call,
    register_safe_state_call,
    register_state_accessible_call,
    register_typed_call,
};

pub use nilo_state_access_derive::{
    nilo_function,
    nilo_state_watcher,
    nilo_state_assign,
    nilo_state_validator,
    nilo_state_accessible,
    nilo_safe_accessible,
};

/// Nilo 関数を自動登録する初期化関数
pub fn init_nilo_functions() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        for register_fn in NILO_FUNCTION_REGISTRY {
            register_fn();
        }
        // state-accessible 関数の自動登録初期化
        crate::engine::rust_call::initialize_state_accessible_functions();
    }
    crate::engine::state::initialize_state_watchers();
    // WASM版では手動登録が基本
}

// カスタムフォント管理用のグローバル静的変数
static CUSTOM_FONTS: OnceLock<RwLock<HashMap<String, &'static [u8]>>> = OnceLock::new();

fn get_font_map() -> &'static RwLock<HashMap<String, &'static [u8]>> {
    CUSTOM_FONTS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// フォント設定用のマクロ
/// # Example
/// ```rust
/// const FONT_JP: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP.ttf"));
/// const FONT_EN: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/Roboto.ttf"));
///
/// nilo::set_custom_font("japanese", FONT_JP);
/// nilo::set_custom_font("english", FONT_EN);
///
/// // Niloアプリでのフォント設定例
/// ```
pub fn set_custom_font(name: &str, font_data: &'static [u8]) {
    if let Ok(mut map) = get_font_map().write() {
        map.insert(name.to_string(), font_data);
    } else {
        error!("Failed to register custom font '{}'", name);
    }
}

/// 埋め込み版Niloアプリ実行関数
///
/// # Example
/// ```rust
/// const FONT_JP: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP.ttf"));
/// const FONT_EN: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/Roboto.ttf"));
///
/// nilo::set_custom_fonts(&[
///     ("japanese", FONT_JP),
///     ("english", FONT_EN),
/// ]);
/// ```
pub fn set_custom_fonts(fonts: &[(&str, &'static [u8])]) {
    if let Ok(mut map) = get_font_map().write() {
        for (name, data) in fonts {
            map.insert(name.to_string(), *data);
        }
    } else {
        error!("Failed to register custom fonts");
    }
}

/// 登録済みの指定されたカスタムフォントを取得する内部関数
#[allow(dead_code)]
pub(crate) fn get_custom_font(name: &str) -> Option<&'static [u8]> {
    get_font_map()
        .read()
        .ok()
        .and_then(|map| map.get(name).copied())
}

/// 登録されているすべてのカスタムフォント一覧を取得する内部関数
pub(crate) fn get_all_custom_fonts() -> Vec<(String, &'static [u8])> {
    get_font_map()
        .read()
        .ok()
        .map(|map| map.iter().map(|(k, v)| (k.clone(), *v)).collect())
        .unwrap_or_default()
}

#[cfg(feature = "colored")]
fn format_colored_message(msg: String, level: &analysis::error::DiagnosticLevel) -> String {
    use colored::Colorize;
    match level {
        analysis::error::DiagnosticLevel::Error => format!("{}", msg.red().bold()),
        analysis::error::DiagnosticLevel::Warning => format!("{}", msg.yellow().bold()),
        analysis::error::DiagnosticLevel::Info => format!("{}", msg.blue()),
    }
}

#[cfg(not(feature = "colored"))]
fn format_colored_message(msg: String, _level: &analysis::error::DiagnosticLevel) -> String {
    msg
}

// 埋め込みファイル自動実行用マクロ（ネイティブ版）
// ========================================
#[cfg(not(target_arch = "wasm32"))]
pub fn run_application_auto_embedded<S, P>(
    file_path: P,
    state: S,
    cli_args: &CliArgs,
    window_title: Option<&str>,
    embedded_source: &str,
) where
    S: StateAccess + Clone + Send + 'static + std::fmt::Debug,
    P: AsRef<std::path::Path> + Send + 'static,
{
    run_application_with_embedded(
        file_path,
        state,
        cli_args,
        window_title,
        Some(embedded_source),
    );
}

// 埋め込みファイル自動実行用マクロ（ネイティブ版）
#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! run_nilo_app {
    ($file_path:expr, $state:expr, $cli_args:expr, $window_title:expr) => {{
        const EMBEDDED_NILO: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $file_path));
        $crate::run_application_auto_embedded(
            $file_path,
            $state,
            $cli_args,
            $window_title,
            EMBEDDED_NILO,
        )
    }};
    ($file_path:expr, $state:expr, $cli_args:expr) => {{
        // 多分rust-analyzerのバグでエラーが表示されちゃいます。
        // 参考: https://github.com/rust-lang/rust-analyzer/issues/10647
        const EMBEDDED_NILO: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $file_path));
        $crate::run_application_auto_embedded($file_path, $state, $cli_args, None, EMBEDDED_NILO)
    }};
}

// 埋め込みファイル自動実行用マクロ（WASM版）
#[cfg(target_arch = "wasm32")]
#[macro_export]
macro_rules! run_nilo_app {
    ($file_path:expr, $state:expr, $cli_args:expr, $window_title:expr) => {{
        const EMBEDDED_NILO: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $file_path));
        $crate::run_nilo_wasm(EMBEDDED_NILO, $state, $window_title)
    }};
    ($file_path:expr, $state:expr, $cli_args:expr) => {{
        const EMBEDDED_NILO: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $file_path));
        $crate::run_nilo_wasm(EMBEDDED_NILO, $state, None)
    }};
    ($file_path:expr, $state:expr) => {{
        const EMBEDDED_NILO: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $file_path));
        $crate::run_nilo_wasm(EMBEDDED_NILO, $state, None)
    }};
}

#[macro_export]
macro_rules! nilo_state {
    (
        $(#[$meta:meta])*
        struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field:ident: $ty:ty
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, serde::Serialize, nilo_state_access_derive::StateAccess)]
        #[state_access(trait_path = "::nilo::engine::state::StateAccess")]
        struct $name {
            $(
                $(#[$field_meta])*
                $field: $ty,
            )*
        }
    };
}

// ========================================
// 埋め込みファイル自動実行用マクロ（ネイティブ版）
// ========================================

/// #[derive(Debug)]
pub struct CliArgs {
    pub enable_lint: bool,
    pub enable_debug: bool,
    pub enable_hotreload: bool,
    pub quiet: bool, // panic時のみログ出力
    pub log_level: LogLevel,
    pub renderer_type: RendererType, // レンダラータイプ
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Off,   // ログ出力無効化、panicのみ表示
    Error, // エラーレベルのみ
    Warn,  // 警告レベル以上
    Info,  // 情報レベル以上
    Debug, // デバッグレベル以上
    Trace, // 詳細なログ
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            enable_lint: true,
            enable_debug: false,
            enable_hotreload: false,
            quiet: false,
            log_level: LogLevel::Info,
            renderer_type: RendererType::Wgpu, //WGPU
        }
    }
}

pub fn parse_args() -> CliArgs {
    let args: Vec<String> = env::args().collect();
    let mut cli_args = CliArgs::default();

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--no-lint" => cli_args.enable_lint = false,
            "--lint" => cli_args.enable_lint = true,
            "--debug" => cli_args.enable_debug = true,
            "--hotreload" => cli_args.enable_hotreload = true,
            "--no-hotreload" => cli_args.enable_hotreload = false,
            "--quiet" | "-q" => {
                cli_args.quiet = true;
                cli_args.log_level = LogLevel::Off;
            }
            "--log-level=off" => cli_args.log_level = LogLevel::Off,
            "--log-level=error" => cli_args.log_level = LogLevel::Error,
            "--log-level=warn" => cli_args.log_level = LogLevel::Warn,
            "--log-level=info" => cli_args.log_level = LogLevel::Info,
            "--log-level=debug" => cli_args.log_level = LogLevel::Debug,
            "--log-level=trace" => cli_args.log_level = LogLevel::Trace,
            "--renderer=wgpu" => cli_args.renderer_type = RendererType::Wgpu,
            "--renderer=dom" => cli_args.renderer_type = RendererType::Dom,
            "--renderer=tiny-skia" => cli_args.renderer_type = RendererType::TinySkia,
            "--renderer=pdf" => cli_args.renderer_type = RendererType::Pdf,
            "--help" | "-h" => {
                show_help();
                std::process::exit(0);
            }
            _ => {}
        }
    }
    cli_args
}

pub fn show_help() {
    info!(
        "Nilo Application Runner

USAGE:
    nilo [OPTIONS]

OPTIONS:
    --lint/--no-lint         Enable/disable lint checks (default: enabled)
    --debug                  Enable debug mode
    --hotreload              Enable hot reloading
    --quiet, -q              Suppress all logs except panics
    --silent                 Same as --quiet
    --log-level=LEVEL        Set log level (off/error/warn/info/debug/trace)
    --renderer=TYPE          Set renderer type (wgpu/dom/tiny-skia/pdf, default: wgpu)
    --help, -h               Show this help"
    );
}

pub fn init_logger(log_level: &LogLevel) {
    use env_logger::Builder;
    use log::LevelFilter;
    use std::sync::Once;

    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let level = match log_level {
            LogLevel::Off => LevelFilter::Off,
            LogLevel::Error => LevelFilter::Error,
            LogLevel::Warn => LevelFilter::Warn,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Debug => LevelFilter::Debug,
            LogLevel::Trace => LevelFilter::Trace,
        };

        let mut builder = Builder::from_default_env();

        if matches!(log_level, LogLevel::Off) {
            // quietモードでは全てのログを無効化、panicのみ表示
            builder
                .filter_level(LevelFilter::Off)
                .format(|_, _| Ok(()))
                .try_init()
                .ok(); // エラーは無視
        } else {
            builder
                .filter_level(level)
                // Vulkan用のGPUログレベル設定
                .filter_module("wgpu_core", LevelFilter::Warn)
                .filter_module("wgpu_hal", LevelFilter::Warn)
                .filter_module("vulkano", LevelFilter::Warn)
                .filter_module("ash", LevelFilter::Warn)
                .filter_module("gfx_backend_vulkan", LevelFilter::Warn)
                .filter_module("winit", LevelFilter::Warn)
                .format_timestamp_secs()
                .try_init()
                .ok(); // エラーは無視
        }
    });
}

pub fn load_nilo_app<P: AsRef<std::path::Path>>(
    path: P,
    enable_lint: bool,
    _enable_debug: bool,
    quiet: bool,
) -> Result<App, String> {
    let app = parse_nilo_file(&path)?;

    if enable_lint && !quiet {
        let analysis_result = analysis::analyze_app(&app);
        let mut has_error = false;

        for diag in &analysis_result.diagnostics {
            let loc = diag.location.as_deref().unwrap_or("");
            let msg_text = format!("{} {}", loc, diag.message);
            let msg = format_colored_message(msg_text, &diag.level);

            if matches!(diag.level, analysis::error::DiagnosticLevel::Error) {
                has_error = true;
            }

            error!("[{:?}] {}", diag.level, msg);
        }

        if has_error {
            error!("\nLint errors found. Use --no-lint to skip lint checks.");
        }

        // Rust側の状態解析も実行（main.rs存在時のみ）
        if let Ok(main_rs_content) = std::fs::read_to_string("src/main.rs") {
            let _ = analysis::analyze_app_with_rust_state(&app, Some(&main_rs_content));
        }
    }

    Ok(app)
}

// 埋め込み用のniloファイル解析関数
pub fn load_embedded_nilo_app(source: &str, enable_lint: bool, quiet: bool) -> Result<App, String> {
    let app = parse_embedded_nilo(source)?;

    if enable_lint && !quiet {
        let analysis_result = analysis::analyze_app(&app);
        let mut has_error = false;

        for diag in &analysis_result.diagnostics {
            let loc = diag.location.as_deref().unwrap_or("");
            let msg_text = format!("{} {}", loc, diag.message);
            let msg = format_colored_message(msg_text, &diag.level);

            if matches!(diag.level, analysis::error::DiagnosticLevel::Error) {
                has_error = true;
            }

            error!("[{:?}] {}", diag.level, msg);
        }

        if has_error {
            error!("\nLint errors found. Use --no-lint to skip lint checks.");
        }

        // Rust側の状態解析も実行（main.rs存在時のみ）
        if let Ok(main_rs_content) = std::fs::read_to_string("src/main.rs") {
            let _ = analysis::analyze_app_with_rust_state(&app, Some(&main_rs_content));
        }
    }

    Ok(app)
}

// 埋め込み版niloアプリ実行関数
#[cfg(not(target_arch = "wasm32"))]
pub fn run_embedded_application<S>(
    embedded_source: &str,
    state: S,
    cli_args: &CliArgs,
    window_title: Option<&str>,
) where
    S: StateAccess + Clone + Send + 'static + std::fmt::Debug,
{
    // ロガーの初期化
    init_logger(&cli_args.log_level);

    let app = load_embedded_nilo_app(embedded_source, cli_args.enable_lint, cli_args.quiet)
        .expect("Failed to parse embedded Nilo source");
    engine::runtime::run_with_window_title(app, state, window_title);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_application<S, P>(file_path: P, state: S, cli_args: &CliArgs, window_title: Option<&str>)
where
    S: StateAccess + Clone + Send + 'static + std::fmt::Debug,
    P: AsRef<std::path::Path> + Send + 'static,
{
    run_application_with_embedded(file_path, state, cli_args, window_title, None);
}

// 埋め込み版Niloアプリ実行関数
#[cfg(not(target_arch = "wasm32"))]
pub fn run_application_with_embedded<S, P>(
    file_path: P,
    state: S,
    cli_args: &CliArgs,
    window_title: Option<&str>,
    embedded_source: Option<&str>,
) where
    S: StateAccess + Clone + Send + 'static + std::fmt::Debug,
    P: AsRef<std::path::Path> + Send + 'static,
{
    init_logger(&cli_args.log_level);
    info!(
        "[RUNNER] run_application_with_embedded: file_path='{}' debug={} hotreload={} quiet={}",
        file_path.as_ref().display(),
        cli_args.enable_debug,
        cli_args.enable_hotreload,
        cli_args.quiet
    );

    #[cfg(not(target_arch = "wasm32"))]
    {
        if cli_args.enable_debug || cli_args.enable_hotreload {
            let file_path_ref = file_path.as_ref();
            let adjusted_path = if file_path_ref.file_name().is_some() && !file_path_ref.exists() {
                let src_path = std::path::Path::new("").join(file_path_ref);
                if (src_path.exists()) {
                    src_path
                } else {
                    file_path_ref.to_path_buf()
                }
            } else {
                file_path_ref.to_path_buf()
            };

            info!(
                "[RUNNER] hotreload/debug mode -> watching path '{}'",
                adjusted_path.display()
            );
            run_with_hotreload(
                adjusted_path,
                state,
                cli_args.enable_lint,
                cli_args.enable_debug,
                cli_args.quiet,
                window_title,
            );
            return;
        }
    }

    // 埋め込みソースの使用判定: デバッグモードでは外部ファイル優先、
    // リリースモードではファイル存在に関係なく埋め込みソースを使用
    let use_embedded = embedded_source.is_some() && {
        #[cfg(not(debug_assertions))]
        {
            true
        }
        #[cfg(debug_assertions)]
        {
            !std::path::Path::new(file_path.as_ref()).exists()
        }
    };

    if use_embedded {
        if let Some(source) = embedded_source {
            info!("[RUNNER] Using embedded Nilo source (debug or release fallback)" );
            let app = load_embedded_nilo_app(source, cli_args.enable_lint, cli_args.quiet)
                .expect("Failed to parse embedded Nilo source");
            engine::runtime::run_with_window_title(app, state, window_title);
            return;
        }
    }

    // 埋め込み版Niloアプリ実行関数
    info!("[RUNNER] Using file source: '{}'", file_path.as_ref().display());
    let app = load_nilo_app(
        file_path,
        cli_args.enable_lint,
        cli_args.enable_debug,
        cli_args.quiet,
    )
    .expect("Failed to parse Nilo file");
    engine::runtime::run_with_window_title(app, state, window_title);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_with_hotreload<S, P>(
    path: P,
    initial_state: S,
    enable_lint: bool,
    enable_debug: bool,
    quiet: bool,
    window_title: Option<&str>,
) where
    S: StateAccess + Clone + Send + 'static + std::fmt::Debug,
    P: AsRef<std::path::Path> + Send + 'static,
{
    use hotreload::HotReloader;
    use std::sync::{Arc, Mutex};

    let file_path = path.as_ref().to_path_buf();
    let should_restart = Arc::new(Mutex::new(false));
    let current_app = Arc::new(Mutex::new(None));

    let app = load_nilo_app(&file_path, enable_lint, enable_debug, quiet)
        .expect("Failed to load initial application");

    let watch_dir = file_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""));
    let hotreloader = HotReloader::new(watch_dir).expect("Failed to setup hot reloader");

    let restart_flag = Arc::clone(&should_restart);
    let file_path_clone = file_path.clone();
    let app_ref = Arc::clone(&current_app);

    hotreloader.set_reload_callback(move || {
        if let Ok(new_app) = load_nilo_app(&file_path_clone, enable_lint, enable_debug, quiet) {
            *app_ref.lock().unwrap() = Some(new_app);
            *restart_flag.lock().unwrap() = true;
        }
    });

    let start = app.flow.start.clone();
    let mut state = engine::state::AppState::new(initial_state, start);
    state.initialize_router(&app.flow);
    let app = Arc::new(app);

    engine::runtime::run_with_hotreload_support_and_title(
        app,
        state,
        should_restart,
        current_app,
        window_title,
    );
}

// ========================================
// WASM版エントリポイント（main.rsに移動）
// ========================================

/// DOMコンテナの準備（スクロール位置保存付きで再作成）
#[cfg(target_arch = "wasm32")]
pub fn prepare_dom_container(container_id: &str) {
    use wasm_bindgen::JsCast;
    use web_sys::{HtmlElement, window};

    if let Some(window) = window() {
        if let Some(document) = window.document() {
            // 既存のコンテナを完全に削除して再作成
            if let Some(existing) = document.get_element_by_id(container_id) {
                // 親要素のスクロール位置を保存
                let parent_scroll_top = if let Some(parent) = existing.parent_element() {
                    parent.scroll_top()
                } else {
                    0
                };
                
                let parent_scroll_left = if let Some(parent) = existing.parent_element() {
                    parent.scroll_left()
                } else {
                    0
                };
                
                // 既存のコンテナを削除
                let _ = existing.remove();
                
                // 新しいコンテナを作成
                if let Ok(new_container) = document.create_element("div") {
                    let _ = new_container.set_attribute("id", container_id);
                    
                    // スタイルを設定
                    if let Some(html_element) = new_container.dyn_ref::<HtmlElement>() {
                        let style = html_element.style();
                        let _ = style.set_property("width", "100%");
                        let _ = style.set_property("min-height", "100%");
                        let _ = style.set_property("position", "relative");
                    }
                    
                    // #preview-containerに追加
                    if let Some(preview_container) = document.get_element_by_id("preview-container") {
                        let _ = preview_container.append_child(&new_container);
                        
                        // スクロール位置を復元
                        preview_container.set_scroll_top(parent_scroll_top);
                        preview_container.set_scroll_left(parent_scroll_left);
                        
                        log::info!("Recreated DOM container (scroll position preserved)");
                    }
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn run_nilo_wasm<S>(nilo_source: &str, initial_state: S, window_title: Option<&str>)
where
    S: StateAccess + Clone + Send + 'static + std::fmt::Debug,
{
    use wasm_bindgen::JsCast;
    use web_sys::{HtmlElement, window};

    log::info!("Nilo WASM starting...");

    // ウィンドウタイトルの設定
    if let Some(title) = window_title {
        if let Some(window) = window() {
            if let Some(document) = window.document() {
                document.set_title(title);
                log::info!("Set window title to: {}", title);
            }
        }
    }

    // Niloソースの解析
    let app = match parser::parse::parse_nilo(nilo_source) {
        Ok(app) => app,
        Err(e) => {
            log::error!("Failed to parse Nilo source: {:?}", e);
            return;
        }
    };

    log::info!("Nilo app parsed successfully");

    // DOMコンテナの作成
    let container_id = "container";
    if let Some(window) = window() {
        if let Some(document) = window.document() {
            if let Some(body) = document.body() {
                // DOMコンテナの作成
                if document.get_element_by_id(container_id).is_none() {
                    if let Ok(container) = document.create_element("div") {
                        let _ = container.set_attribute("id", container_id);
                        if let Some(html_element) = container.dyn_ref::<HtmlElement>() {
                            let style = html_element.style();
                            let _ = style.set_property("position", "relative");
                            let _ = style.set_property("width", "100vw");
                            let _ = style.set_property("height", "100vh");
                            let _ = style.set_property("overflow", "hidden");
                        }
                        let _ = body.append_child(&container);
                        log::info!("Created DOM container: {}", container_id);
                    }
                }
            }
        }
    }

    // 初期ビューの設定
    let start_view = app.flow.start.clone();
    let mut state = engine::state::AppState::new(initial_state, start_view.clone());

    let initial_timeline = state.initialize_router_from_app(&app);

    // URLから初期タイムライン指定があれば適用
    if let Some(timeline) = initial_timeline {
        log::info!("Setting initial timeline from URL: {}", timeline);
        state.jump_to_timeline(&timeline);
    }

    log::info!("Running Nilo app with DOM renderer...");

    // DOMレンダラーでアプリを実行
    engine::runtime_dom::run_dom(app, state);
}

// WASM版用のエントリポイント
// app.niloの内容を解析し、デフォルト埋め込みソース
//#[cfg(target_arch = "wasm32")]
//const WASM_NILO_SOURCE: &str = include_str!("inputtest.nilo");

// フォントファイルの埋め込み
#[cfg(target_arch = "wasm32")]
const WASM_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fonts/NotoSansJP-Regular.ttf"
));

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct WasmTestState {
    items: Vec<i32>,
    next_item_value: i32,
}

#[cfg(target_arch = "wasm32")]
impl Default for WasmTestState {
    fn default() -> Self {
        Self {
            items: vec![1, 2, 3],
            next_item_value: 4,
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl engine::state::StateAccess for WasmTestState {
    fn get_field(&self, key: &str) -> Option<String> {
        match key {
            "items" => Some(format!("{:?}", self.items)),
            "next_item_value" => Some(self.next_item_value.to_string()),
            _ => None,
        }
    }

    fn set(&mut self, path: &str, value: String) -> Result<(), String> {
        match path {
            "next_item_value" => {
                self.next_item_value = value
                    .parse()
                    .map_err(|e| format!("Failed to parse next_item_value: {}", e))?;
                Ok(())
            }
            _ => Err(format!("Unknown field: {}", path)),
        }
    }

    fn toggle(&mut self, _path: &str) -> Result<(), String> {
        Ok(())
    }

    fn list_append(&mut self, path: &str, value: String) -> Result<(), String> {
        match path {
            "items" => {
                let item: i32 = value
                    .parse()
                    .map_err(|e| format!("Failed to parse item: {}", e))?;
                self.items.push(item);
                Ok(())
            }
            _ => Err(format!("Unknown list field: {}", path)),
        }
    }

    fn list_insert(&mut self, path: &str, index: usize, value: String) -> Result<(), String> {
        match path {
            "items" => {
                let item: i32 = value
                    .parse()
                    .map_err(|e| format!("Failed to parse item: {}", e))?;
                if index <= self.items.len() {
                    self.items.insert(index, item);
                    Ok(())
                } else {
                    Err("Index out of bounds".to_string())
                }
            }
            _ => Err(format!("Unknown list field: {}", path)),
        }
    }

    fn list_remove(&mut self, path: &str, value: String) -> Result<(), String> {
        match path {
            "items" => {
                let item: i32 = value
                    .parse()
                    .map_err(|e| format!("Failed to parse item: {}", e))?;
                if let Some(pos) = self.items.iter().position(|x| *x == item) {
                    self.items.remove(pos);
                    Ok(())
                } else {
                    Err(format!("Item {} not found", item))
                }
            }
            _ => Err(format!("Unknown list field: {}", path)),
        }
    }

    fn list_clear(&mut self, path: &str) -> Result<(), String> {
        match path {
            "items" => {
                self.items.clear();
                Ok(())
            }
            _ => Err(format!("Unknown list field: {}", path)),
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn run_nilo_wasm_with_state() {
    console_error_panic_hook::set_once();

    //   console_log::init_with_level(log::Level::Info).expect("error initializing log");

    log::info!("Nilo WASM starting...");
    log::info!("Loading list_operations_test.nilo...");

    // 埋め込み版Niloアプリの初期化
    set_custom_font("japanese", WASM_FONT);

    let state = WasmTestState::default();

    // DOMコンテナの作成
}



