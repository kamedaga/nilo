pub mod component_validator;
pub mod error;
pub mod lint;
pub mod state_type_checker; // ★ Phase 2: コンポーネント型バリデーション

use crate::parser::ast::App;

use error::Diagnostic;
use lint::{LintWarning, run_lints};

pub struct AnalysisResult {
    pub diagnostics: Vec<Diagnostic>,
    pub warnings: Vec<LintWarning>,
}

pub fn analyze_app(app: &App) -> AnalysisResult {
    let mut diagnostics = Vec::new();
    let warnings = Vec::new();

    // 1. 必須Lint/診断
    diagnostics.extend(run_lints(app));

    // 2. 追加の警告や注意事項も集約可
    // warnings.push(...)

    AnalysisResult {
        diagnostics,
        warnings,
    }
}

/// Rustソースコードを含めた型チェック付きの解析
pub fn analyze_app_with_rust_state(app: &App, rust_source: Option<&str>) -> AnalysisResult {
    let mut diagnostics = Vec::new();
    let warnings = Vec::new();

    // 1. 通常のLint
    diagnostics.extend(run_lints(app));

    // 2. Rust状態の型チェック
    if let Some(source) = rust_source {
        if let Some(schema) = state_type_checker::RustStateSchema::parse_from_source(source) {
            let type_warnings = state_type_checker::check_state_access_types(app, &schema);
            for warning in type_warnings {
                log::warn!("[State Type Warning] {}", warning);
            }
        }
    }

    // ★ Phase 2: コンポーネント型バリデーション
    let component_warnings = component_validator::validate_component_calls(app);
    for warning in component_warnings {
        log::warn!("[Component Warning] {}", warning);
    }

    AnalysisResult {
        diagnostics,
        warnings,
    }
}
