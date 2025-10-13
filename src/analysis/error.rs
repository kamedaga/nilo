#[derive(Debug)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub location: Option<String>,
}
#[derive(Debug)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Info,
}

impl Diagnostic {
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Error,
            message: msg.into(),
            location: None,
        }
    }
    pub fn warning(msg: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Warning,
            message: msg.into(),
            location: None,
        }
    }
}
pub type AnalysisError = Diagnostic;
