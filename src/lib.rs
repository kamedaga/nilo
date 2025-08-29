pub mod renderer;
pub mod stencil;
pub mod ui;
pub mod parser;
pub mod engine;
pub mod hotreload;
pub mod analysis;

use parser::{parse_nilo_file, ast::App};
use colored::*;
use std::env;

pub use engine::exec::{AppState, StateAccess};
pub use engine::runtime::run;

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
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            enable_lint: true,
            enable_debug: false,
            enable_hotreload: false,
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
    println!("Nilo Application Runner

USAGE:
    nilo [OPTIONS]

OPTIONS:
    --lint/--no-lint    Enable/disable lint checks (default: enabled)
    --debug             Enable debug mode
    --hotreload         Enable hot reloading
    --help, -h          Show this help");
}

pub fn load_nilo_app<P: AsRef<std::path::Path>>(
    path: P,
    enable_lint: bool,
    _enable_debug: bool
) -> Result<App, String> {
    let app = parse_nilo_file(&path)?;

    if enable_lint {
        let analysis_result = analysis::analyze_app(&app);
        let mut has_error = false;

        for diag in &analysis_result.diagnostics {
            let loc = diag.location.as_deref().unwrap_or("");
            let msg = match diag.level {
                analysis::error::DiagnosticLevel::Error => {
                    has_error = true;
                    format!("{} {}", loc, diag.message).red().bold()
                }
                analysis::error::DiagnosticLevel::Warning => format!("{} {}", loc, diag.message).yellow().bold(),
                analysis::error::DiagnosticLevel::Info => format!("{} {}", loc, diag.message).blue(),
            };
            eprintln!("[{:?}] {}", diag.level, msg);
        }

        if has_error {
            eprintln!("\nLint errors found. Use --no-lint to skip lint checks.");
        }
    }

    Ok(app)
}

pub fn run_application<S, P>(
    file_path: P,
    state: S,
    cli_args: &CliArgs,
    window_title: Option<&str>,
) where
    S: StateAccess + Clone + Send + 'static + std::fmt::Debug,
    P: AsRef<std::path::Path> + Send + 'static,
{
    if cli_args.enable_debug || cli_args.enable_hotreload {
        run_with_hotreload(file_path, state, cli_args.enable_lint, cli_args.enable_debug, window_title);
    } else {
        let app = load_nilo_app(file_path, cli_args.enable_lint, cli_args.enable_debug)
            .expect("Failed to parse Nilo file");
        engine::runtime::run_with_window_title(app, state, window_title);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_with_hotreload<S, P>(
    path: P,
    initial_state: S,
    enable_lint: bool,
    enable_debug: bool,
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

    let app = load_nilo_app(&file_path, enable_lint, enable_debug)
        .expect("Failed to load initial application");

    let watch_dir = file_path.parent().unwrap_or_else(|| std::path::Path::new("src"));
    let hotreloader = HotReloader::new(watch_dir).expect("Failed to setup hot reloader");

    let restart_flag = Arc::clone(&should_restart);
    let file_path_clone = file_path.clone();
    let app_ref = Arc::clone(&current_app);

    hotreloader.set_reload_callback(move || {
        if let Ok(new_app) = load_nilo_app(&file_path_clone, enable_lint, enable_debug) {
            *app_ref.lock().unwrap() = Some(new_app);
            *restart_flag.lock().unwrap() = true;
        }
    });

    let start = app.flow.start.clone();
    let state = engine::state::AppState::new(initial_state, start);
    let app = Arc::new(app);

    engine::runtime::run_with_hotreload_support_and_title(app, state, should_restart, current_app, window_title);
}
