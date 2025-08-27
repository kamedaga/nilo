pub mod lint;
pub mod error;

use crate::parser::ast::App;
use lint::{run_lints, LintWarning};
use error::{AnalysisError, Diagnostic};

pub struct AnalysisResult {
    pub diagnostics: Vec<Diagnostic>,
    pub warnings: Vec<LintWarning>,
}

pub fn analyze_app(app: &App) -> AnalysisResult {
    let mut diagnostics = Vec::new();
    let mut warnings = Vec::new();

    // 1. 必須Lint/診断
    diagnostics.extend(run_lints(app));

    // 2. 追加の警告や注意事項も集約可
    // warnings.push(...)

    AnalysisResult { diagnostics, warnings }
}
