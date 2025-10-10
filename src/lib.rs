pub mod renderer_abstract;
#[cfg(feature = "wgpu")]
pub mod wgpu_renderer;
pub mod dom_renderer;
pub mod stencil;
pub mod ui;
pub mod parser;
pub mod engine;
#[cfg(not(target_arch = "wasm32"))]
pub mod hotreload;
pub mod analysis;

use parser::{parse_nilo_file, parse_embedded_nilo, ast::App};
#[cfg(feature = "colored")]
use colored::*;
use std::env;
use log::{info, error}; // ログマクロを追加
use std::sync::{RwLock, OnceLock};
use std::collections::HashMap;

// WASM用の依存関係
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub use engine::exec::{AppState, StateAccess};
#[cfg(not(target_arch = "wasm32"))]
pub use engine::runtime::run;
pub use renderer_abstract::{RendererType}; // レンダラータイプを追加

// グローバルなフォント設定（名前付きフォントマップ）
static CUSTOM_FONTS: OnceLock<RwLock<HashMap<String, &'static [u8]>>> = OnceLock::new();

fn get_font_map() -> &'static RwLock<HashMap<String, &'static [u8]>> {
    CUSTOM_FONTS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// カスタムフォントを名前付きでグローバルに登録
/// 
/// アプリケーション起動前に呼び出してください。
/// 複数のフォントを登録でき、Niloファイルから名前で参照できます。
/// 
/// # Example
/// ```rust
/// const FONT_JP: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP.ttf"));
/// const FONT_EN: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/Roboto.ttf"));
/// 
/// nilo::set_custom_font("japanese", FONT_JP);
/// nilo::set_custom_font("english", FONT_EN);
/// 
/// // Niloファイル内で: font: "japanese" や font: "english" で使用可能
/// ```
pub fn set_custom_font(name: &str, font_data: &'static [u8]) {
    if let Ok(mut map) = get_font_map().write() {
        map.insert(name.to_string(), font_data);
    } else {
        error!("Failed to register custom font '{}'", name);
    }
}

/// 複数のカスタムフォントを一度に登録
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

/// 登録されたカスタムフォントを名前で取得
pub(crate) fn get_custom_font(name: &str) -> Option<&'static [u8]> {
    get_font_map().read().ok().and_then(|map| map.get(name).copied())
}

/// 登録されたすべてのカスタムフォントを取得
pub(crate) fn get_all_custom_fonts() -> Vec<(String, &'static [u8])> {
    get_font_map()
        .read()
        .ok()
        .map(|map| map.iter().map(|(k, v)| (k.clone(), *v)).collect())
        .unwrap_or_default()
}

// カラー表示のヘルパー関数（環境に応じて切り替え）
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

// 自動的に埋め込みファイルを使用する便利関数
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
    run_application_with_embedded(file_path, state, cli_args, window_title, Some(embedded_source));
}

// 埋め込みファイルを自動で使用するマクロ
#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! run_nilo_app {
    ($file_path:expr, $state:expr, $cli_args:expr, $window_title:expr) => {{
        const EMBEDDED_NILO: &str = include_str!($file_path);
        $crate::run_application_auto_embedded($file_path, $state, $cli_args, $window_title, EMBEDDED_NILO)
    }};
    ($file_path:expr, $state:expr, $cli_args:expr) => {{
        const EMBEDDED_NILO: &str = include_str!($file_path);
        $crate::run_application_auto_embedded($file_path, $state, $cli_args, None, EMBEDDED_NILO)
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
// コマンドライン引数構造体
// ========================================

/// コマンドライン引数の設定
#[derive(Debug)]
pub struct CliArgs {
    pub enable_lint: bool,
    pub enable_debug: bool,
    pub enable_hotreload: bool,
    pub quiet: bool,  // panic以外のログを抑制
    pub log_level: LogLevel,
    pub renderer_type: RendererType, // レンダラータイプを追加
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Off,      // ログを一切表示しない（panicは除く）
    Error,    // エラーレベルのみ
    Warn,     // 警告レベル以上
    Info,     // 情報レベル以上
    Debug,    // デバッグレベル以上
    Trace,    // 全てのログ
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            enable_lint: true,
            enable_debug: false,
            enable_hotreload: false,
            quiet: false,
            log_level: LogLevel::Info,
            renderer_type: RendererType::Wgpu, // デフォルトはWGPU
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
    info!("Nilo Application Runner

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
    --help, -h               Show this help");
}

/// ログレベルを初期化する関数
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
            // quietモードの場合、何も出力しない（panicは別途処理される）
            builder
                .filter_level(LevelFilter::Off)
                .format(|_, _| Ok(()))
                .try_init()
                .ok(); // エラーを無視
        } else {
            builder
                .filter_level(level)
                // VulkanやWGPU関連のInfoログを抑制
                .filter_module("wgpu_core", LevelFilter::Warn)
                .filter_module("wgpu_hal", LevelFilter::Warn)
                .filter_module("vulkano", LevelFilter::Warn)
                .filter_module("ash", LevelFilter::Warn)
                .filter_module("gfx_backend_vulkan", LevelFilter::Warn)
                .filter_module("winit", LevelFilter::Warn)
                .format_timestamp_secs()
                .try_init()
                .ok(); // エラーを無視
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
        
        // Rust状態の型チェック（main.rsを読み込む）
        if let Ok(main_rs_content) = std::fs::read_to_string("src/main.rs") {
            let _ = analysis::analyze_app_with_rust_state(&app, Some(&main_rs_content));
        }
    }

    Ok(app)
}

// 埋め込まれたniloファイルをロードする関数
pub fn load_embedded_nilo_app(
    source: &str,
    enable_lint: bool,
    quiet: bool,
) -> Result<App, String> {
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
        
        // Rust状態の型チェック（main.rsを読み込む）
        if let Ok(main_rs_content) = std::fs::read_to_string("src/main.rs") {
            let _ = analysis::analyze_app_with_rust_state(&app, Some(&main_rs_content));
        }
    }

    Ok(app)
}

// 埋め込まれたniloアプリを実行する関数
#[cfg(not(target_arch = "wasm32"))]
pub fn run_embedded_application<S>(
    embedded_source: &str,
    state: S,
    cli_args: &CliArgs,
    window_title: Option<&str>,
) where
    S: StateAccess + Clone + Send + 'static + std::fmt::Debug,
{
    // ログレベルを初期化
    init_logger(&cli_args.log_level);
    
    let app = load_embedded_nilo_app(embedded_source, cli_args.enable_lint, cli_args.quiet)
        .expect("Failed to parse embedded Nilo source");
    engine::runtime::run_with_window_title(app, state, window_title);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_application<S, P>(
    file_path: P,
    state: S,
    cli_args: &CliArgs,
    window_title: Option<&str>,
) where
    S: StateAccess + Clone + Send + 'static + std::fmt::Debug,
    P: AsRef<std::path::Path> + Send + 'static,
{
    run_application_with_embedded(file_path, state, cli_args, window_title, None);
}

// 埋め込みファイルオプション付きの内部実装
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
    // ログレベルを初期化
    init_logger(&cli_args.log_level);

    #[cfg(not(target_arch = "wasm32"))]
    {
        if cli_args.enable_debug || cli_args.enable_hotreload {
            // デバッグ/ホットリロードモードでは常にファイルから読み込み
            // main.rsから呼び出される場合、srcディレクトリ内のファイルを指すようにパス修正
            let file_path_ref = file_path.as_ref();
            let adjusted_path = if file_path_ref.file_name().is_some() && !file_path_ref.exists() {
                let src_path = std::path::Path::new("src").join(file_path_ref);
                if src_path.exists() {
                    src_path
                } else {
                    file_path_ref.to_path_buf()
                }
            } else {
                file_path_ref.to_path_buf()
            };
            
            run_with_hotreload(adjusted_path, state, cli_args.enable_lint, cli_args.enable_debug, cli_args.quiet, window_title);
            return;
        }
    }
    
    // 埋め込みソースが提供されている場合は埋め込みを使用
    // リリースビルドでファイルが存在しない場合も埋め込みを試行
    let use_embedded = embedded_source.is_some() && {
        #[cfg(not(debug_assertions))]
        { true }
        #[cfg(debug_assertions)]
        { !std::path::Path::new(file_path.as_ref()).exists() }
    };
    
    if use_embedded {
        if let Some(source) = embedded_source {
            let app = load_embedded_nilo_app(source, cli_args.enable_lint, cli_args.quiet)
                .expect("Failed to parse embedded Nilo source");
            engine::runtime::run_with_window_title(app, state, window_title);
            return;
        }
    }
    
    // 通常のファイル読み込み
    let app = load_nilo_app(file_path, cli_args.enable_lint, cli_args.enable_debug, cli_args.quiet)
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

    let watch_dir = file_path.parent().unwrap_or_else(|| std::path::Path::new("src"));
    let hotreloader = HotReloader::new(watch_dir).expect("Failed to setup hot reloader");

    let restart_flag = Arc::clone(&should_restart);
    let file_path_clone = file_path.clone();
    let app_ref = Arc::clone(&current_app);

    hotreloader.set_reload_callback(move || {
        println!("🔥 HOTRELOAD CALLBACK TRIGGERED!");
        if let Ok(new_app) = load_nilo_app(&file_path_clone, enable_lint, enable_debug, quiet) {
            println!("🔥 NEW APP LOADED SUCCESSFULLY");
            *app_ref.lock().unwrap() = Some(new_app);
            *restart_flag.lock().unwrap() = true;
        } else {
            println!("🔥 FAILED TO LOAD NEW APP");
        }
    });

    let start = app.flow.start.clone();
    let mut state = engine::state::AppState::new(initial_state, start);
    state.initialize_router(&app.flow);
    let app = Arc::new(app);

    engine::runtime::run_with_hotreload_support_and_title(app, state, should_restart, current_app, window_title);
}

// ========================================
// WASM用エントリーポイント
// ========================================

#[cfg(target_arch = "wasm32")]
pub fn run_nilo_wasm<S>(nilo_source: &str, initial_state: S)
where
    S: StateAccess + Clone + Send + 'static + std::fmt::Debug,
{
    use wasm_bindgen::JsCast;
    use web_sys::{window, HtmlElement};

    log::info!("Nilo WASM starting...");

    // Niloソースをパース
    let app = match parser::parse::parse_nilo(nilo_source) {
        Ok(app) => app,
        Err(e) => {
            log::error!("Failed to parse Nilo source: {:?}", e);
            return;
        }
    };

    log::info!("Nilo app parsed successfully");

    // DOMコンテナを作成
    let container_id = "nilo-renderer-container";
    if let Some(window) = window() {
        if let Some(document) = window.document() {
            if let Some(body) = document.body() {
                // コンテナがまだ作成されていない場合は作成
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

    // アプリケーション状態を作成
    let start_view = app.flow.start.clone();
    let mut state = engine::state::AppState::new(initial_state, start_view.clone());
    
    // ルーターを初期化し、URLから初期タイムラインを取得
    let initial_timeline = state.initialize_router_from_app(&app);
    
    // URLで指定されたタイムラインがあればそれに移動
    if let Some(timeline) = initial_timeline {
        log::info!("Setting initial timeline from URL: {}", timeline);
        state.jump_to_timeline(&timeline);
    }

    log::info!("Running Nilo app with DOM renderer...");
    
    // DOM環境でアプリを実行
    engine::runtime_dom::run_dom(app, state);
}

// WASM専用のエントリーポイント

// dynamic_foreach_test.niloの内容をコンパイル時に埋め込み
#[cfg(target_arch = "wasm32")]
const WASM_NILO_SOURCE: &str = include_str!("routing_test.nilo");

// フォントデータを埋め込み
#[cfg(target_arch = "wasm32")]
const WASM_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP-Regular.ttf"));

// dynamic_foreach_test.nilo用のState構造体
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone)]
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
                self.next_item_value = value.parse().map_err(|e| format!("Failed to parse next_item_value: {}", e))?;
                Ok(())
            }
            _ => Err(format!("Unknown field: {}", path))
        }
    }
    
    fn toggle(&mut self, _path: &str) -> Result<(), String> {
        Ok(())
    }
    
    fn list_append(&mut self, path: &str, value: String) -> Result<(), String> {
        match path {
            "items" => {
                let item: i32 = value.parse().map_err(|e| format!("Failed to parse item: {}", e))?;
                self.items.push(item);
                Ok(())
            }
            _ => Err(format!("Unknown list field: {}", path))
        }
    }
    
    fn list_insert(&mut self, path: &str, index: usize, value: String) -> Result<(), String> {
        match path {
            "items" => {
                let item: i32 = value.parse().map_err(|e| format!("Failed to parse item: {}", e))?;
                if index <= self.items.len() {
                    self.items.insert(index, item);
                    Ok(())
                } else {
                    Err("Index out of bounds".to_string())
                }
            }
            _ => Err(format!("Unknown list field: {}", path))
        }
    }
    
    fn list_remove(&mut self, path: &str, value: String) -> Result<(), String> {
        match path {
            "items" => {
                let item: i32 = value.parse().map_err(|e| format!("Failed to parse item: {}", e))?;
                if let Some(pos) = self.items.iter().position(|x| *x == item) {
                    self.items.remove(pos);
                    Ok(())
                } else {
                    Err(format!("Item {} not found", item))
                }
            }
            _ => Err(format!("Unknown list field: {}", path))
        }
    }
    
    fn list_clear(&mut self, path: &str) -> Result<(), String> {
        match path {
            "items" => {
                self.items.clear();
                Ok(())
            }
            _ => Err(format!("Unknown list field: {}", path))
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn run_nilo_wasm_with_state() {
    // パニック時のエラーメッセージをブラウザコンソールに表示
    console_error_panic_hook::set_once();
    
    // WebAssembly用のロガーを初期化（Infoレベルに設定してDebugログを抑制）
    console_log::init_with_level(log::Level::Info).expect("error initializing log");

    log::info!("Nilo WASM starting...");
    log::info!("Loading list_operations_test.nilo...");

    // カスタムフォントを登録
    set_custom_font("japanese", WASM_FONT);

    // 初期状態を作成
    let state = WasmTestState::default();

    // DOMレンダラーでNiloアプリを実行
    run_nilo_wasm(WASM_NILO_SOURCE, state);
}
