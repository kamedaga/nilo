pub mod renderer;
pub mod stencil;
pub mod ui;
pub mod parser;
pub mod engine;   // ← ここはただの1行にする（中身はdsl/mod.rsで定義）
pub mod hotreload;

pub mod analysis;

use parser::{parse_nilo_file, ast::App};
use colored::*;
use std::env;

pub use engine::exec::{AppState, StateAccess}; // 互換 re-export
pub use engine::runtime::run; // runtimeを外から使えるようにする

// ========================================
// コマンドライン引数構造体
// ========================================

/// コマンドライン引数の設定
#[derive(Debug)]
pub struct CliArgs {
    /// lint機能を有効にするかどうか
    pub enable_lint: bool,
    /// デバッグ機能を有効にするかどうか
    pub enable_debug: bool,
    /// ホットリロード機能を有効にするかどうか
    pub enable_hotreload: bool,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            enable_lint: true,  // デフォルトはlint有効
            enable_debug: false,
            enable_hotreload: false,
        }
    }
}

/// コマンドライン引数を解析する
pub fn parse_args() -> CliArgs {
    let args: Vec<String> = env::args().collect();
    let mut cli_args = CliArgs::default();

    let mut i = 1; // 最初の引数（プログラム名）をスキップ
    while i < args.len() {
        match args[i].as_str() {
            "--no-lint" => {
                cli_args.enable_lint = false;
                println!("🚫 Lint checks disabled via command line");
            }
            "--lint" => {
                cli_args.enable_lint = true;
                println!("✅ Lint checks enabled via command line");
            }
            "--debug" => {
                cli_args.enable_debug = true;
                println!("🐛 Debug mode enabled via command line");
            }
            "--hotreload" => {
                cli_args.enable_hotreload = true;
                println!("🔄 Hot reload enabled via command line");
            }
            "--no-hotreload" => {
                cli_args.enable_hotreload = false;
                println!("🚫 Hot reload disabled via command line");
            }
            "--help" | "-h" => {
                show_help();
                std::process::exit(0);
            }
            unknown => {
                eprintln!("⚠️  Warning: Unknown argument: {}", unknown);
            }
        }
        i += 1;
    }

    cli_args
}

/// ヘルプメッセージを表示する
pub fn show_help() {
    println!("🚀 Nilo Application Runner");
    println!();
    println!("USAGE:");
    println!("    nilo [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    --lint       Enable lint checks (default)");
    println!("    --no-lint    Disable lint checks");
    println!("    --debug      Enable debug mode with detailed analysis and hot reload");
    println!("    --hotreload   Enable hot reloading of the application");
    println!("    --no-hotreload  Disable hot reloading");
    println!("    --help, -h   Show this help message");
    println!();
    println!("EXAMPLES:");
    println!("    nilo                        # Run with default settings");
    println!("    nilo --no-lint              # Run without lint checks");
    println!("    nilo --debug                # Run with debug info and hot reload");
    println!("    nilo --debug --no-lint      # Run with debug but no lint");
    println!("    nilo --hotreload             # Run with hot reloading only");
    println!("    nilo --no-hotreload          # Run without hot reloading");
    println!();
    println!("NOTE:");
    println!("    Debug mode automatically enables hot reload for development convenience.");
}

/// lint機能を有効にしてアプリケーションをロード


/// lint機能とデバッグ機能の有効/無効を指定してアプリケーションをロード
pub fn load_nilo_app<P: AsRef<std::path::Path>>(
    path: P,
    enable_lint: bool,
    enable_debug: bool
) -> Result<App, String> {
    if enable_debug {
        println!("{}", "🐛 DEBUG MODE ENABLED".bright_cyan().bold());
        println!("{}", "��� Loading Nilo application...".cyan());
    }

    // パースフェーズ
    if enable_debug {
        println!("{}", "⚙️  Phase 1: Parsing file...".cyan());
    }

    let app = parse_nilo_file(&path)?;

    if enable_debug {
        print_debug_info(&app);
    }

    // Lintフェーズ
    if enable_lint {
        if enable_debug {
            println!("{}", "⚙️  Phase 2: Running lint analysis...".cyan());
        }

        let analysis_result = analysis::analyze_app(&app);
        let mut has_error = false;

        if enable_debug {
            println!("📊 Found {} diagnostic(s)", analysis_result.diagnostics.len());
        }

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
            eprintln!("\n⚠️  Lint errors found. Use --no-lint to skip lint checks.");
        } else if enable_debug {
            println!("{}", "✅ Lint analysis completed successfully".green());
        }
    } else {
        if enable_debug {
            println!("{}", "⚠️  Phase 2: Lint analysis skipped".yellow());
        } else {
            println!("📝 Lint checks disabled by command line option");
        }
    }

    if enable_debug {
        println!("{}", "✅ Application loaded successfully".bright_green().bold());
        println!("{}", "🚀 Ready to start runtime...".cyan());
        println!("{}", "🎯 Starting application runtime...".cyan());
    }

    Ok(app)
}

/// デ���ッグ情報を表示する関数
fn print_debug_info(app: &App) {
    println!("{}", "📊 DEBUG: Application Analysis".bright_cyan().bold());
    println!("├─ 📋 Components: {}", app.components.len());

    for (i, component) in app.components.iter().enumerate() {
        println!("│  ├─ [{}] {} (params: {})",
            i + 1,
            component.name.bright_white(),
            component.params.len()
        );
    }

    println!("├─ 🎬 Timelines: {}", app.timelines.len());

    for (i, timeline) in app.timelines.iter().enumerate() {
        let when_count = timeline.whens.len();
        println!("│  ├─ [{}] {} (events: {})",
            i + 1,
            timeline.name.bright_white(),
            when_count
        );

        let node_count = count_nodes_in_timeline(&timeline.body);
        println!("│  │   └─ UI nodes: {}", node_count);
    }

    // Rust関数の統計
    let rust_call_count = count_rust_calls_in_app(app);
    println!("├─ 🦀 Rust function calls: {}", rust_call_count);

    // 全体の統計
    let total_nodes = app.timelines.iter()
        .map(|tl| count_nodes_in_timeline(&tl.body))
        .sum::<usize>();

    println!("└─ 📈 Total UI nodes: {}", total_nodes);
    println!();
}

/// タイムライン内のノード数をカウント
fn count_nodes_in_timeline(nodes: &[parser::ast::WithSpan<parser::ast::ViewNode>]) -> usize {
    nodes.iter().map(|node| count_nodes_recursive(&node.node)).sum()
}

/// ノードを再帰的にカウント
fn count_nodes_recursive(node: &parser::ast::ViewNode) -> usize {
    use parser::ast::ViewNode;

    match node {
        ViewNode::VStack(children) | ViewNode::HStack(children) => {
            1 + children.iter().map(|child| count_nodes_recursive(&child.node)).sum::<usize>()
        }
        ViewNode::DynamicSection { body, .. } => {
            1 + body.iter().map(|child| count_nodes_recursive(&child.node)).sum::<usize>()
        }
        _ => 1,
    }
}

/// アプリ全体のRust関数呼び出し数をカウント
fn count_rust_calls_in_app(app: &App) -> usize {
    let mut count = 0;

    // コンポーネント内のRust呼び出しをカウント
    for component in &app.components {
        count += count_rust_calls_in_nodes(&component.body);
    }

    // タイムライン内のRust呼び出しをカウント
    for timeline in &app.timelines {
        count += count_rust_calls_in_nodes(&timeline.body);

        // when節のアクション内のRust呼び出しもカウント
        for when in &timeline.whens {
            for action in &when.actions {
                if matches!(action.node, parser::ast::ViewNode::RustCall { .. }) {
                    count += 1;
                }
            }
        }
    }

    count
}

/// ノード���スト内のRust関数呼び出し数������ウント
fn count_rust_calls_in_nodes(nodes: &[parser::ast::WithSpan<parser::ast::ViewNode>]) -> usize {
    nodes.iter().map(|node| count_rust_calls_recursive(&node.node)).sum()
}

/// ノードを再帰的にチェックしてRust関数呼び出し数をカウント
fn count_rust_calls_recursive(node: &parser::ast::ViewNode) -> usize {
    use parser::ast::ViewNode;

    match node {
        ViewNode::RustCall { .. } => 1,
        ViewNode::VStack(children) | ViewNode::HStack(children) => {
            children.iter().map(|child| count_rust_calls_recursive(&child.node)).sum()
        }
        ViewNode::DynamicSection { body, .. } => {
            body.iter().map(|child| count_rust_calls_recursive(&child.node)).sum()
        }
        _ => 0,
    }
}

// ========================================
// アプリケーション実行機能
// ========================================

/// アプリケーションを実行する
/// デバッグモードの場合はホットリロード機能��自動的に有効化
pub fn run_application<S, P>(
    file_path: P,
    state: S,
    cli_args: &CliArgs,
    window_title: Option<&str>,
) where
    S: StateAccess + Clone + Send + 'static + std::fmt::Debug,
    P: AsRef<std::path::Path> + Send + 'static,
{
    // デバッグモードの場合はホットリロード機能を有効化
    if cli_args.enable_debug {
        println!("{}", "🔥 Debug mode: Hot reload enabled automatically".bright_green());
        run_with_hotreload(
            file_path,
            state,
            cli_args.enable_lint,
            cli_args.enable_debug,
            window_title
        );
    } else {
        // 通常の実行
        let app = load_nilo_app(
            file_path,
            cli_args.enable_lint,
            cli_args.enable_debug
        ).expect("Failed to parse Nilo file");

        // アプリケーションの実行
        engine::runtime::run_with_window_title(app, state, window_title);
    }
}

// ========================================
// ホットリロード機能
// ========================================

/// ホットリロード機能付きでアプリケーションを実行
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

    // 初期アプリケーションのロード
    let app = match load_nilo_app(&file_path, enable_lint, enable_debug) {
        Ok(app) => {
            println!("{}", "✅ Initial application loaded successfully!".green());
            app
        }
        Err(e) => {
            eprintln!("Failed to load initial application: {}", e);
            return;
        }
    };

    // ホットリローダーの設定
    let watch_dir = file_path.parent().unwrap_or_else(|| std::path::Path::new("src"));
    let hotreloader = match HotReloader::new(watch_dir) {
        Ok(hr) => hr,
        Err(e) => {
            eprintln!("Failed to setup hot reloader: {}", e);
            return;
        }
    };

    // リロードコールバックの設定
    let restart_flag = Arc::clone(&should_restart);
    let file_path_clone = file_path.clone();
    let app_ref = Arc::clone(&current_app);

    hotreloader.set_reload_callback(move || {
        match load_nilo_app(&file_path_clone, enable_lint, enable_debug) {
            Ok(new_app) => {
                // 新しいアプリケーションを保存
                *app_ref.lock().unwrap() = Some(new_app);
                *restart_flag.lock().unwrap() = true;
                println!("{}", "✅ Application reloaded successfully! Restarting...".green());
            }
            Err(e) => {
                eprintln!("{}", format!("❌ Failed to reload application: {}", e).red());
            }
        }
    });

    println!("{}", "🚀 Starting application with hot reload enabled...".cyan());
    println!("{}", "💡 File changes will be detected automatically. Press Ctrl+C to exit.".blue());

    // メインスレッドでホットリロード機能付きのアプリケーションを実行
    let start = app.flow.start.clone();
    let state = engine::state::AppState::new(initial_state, start);
    let app = Arc::new(app);

    engine::runtime::run_with_hotreload_support_and_title(app, state, should_restart, current_app, window_title);
}
