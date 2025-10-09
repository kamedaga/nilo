use crate::parser::ast::{App, ViewNode, Expr, WithSpan, NiloType};
use std::collections::HashMap;

/// Rust側の状態型定義を表す
#[derive(Debug, Clone)]
pub struct RustStateSchema {
    pub fields: HashMap<String, RustFieldType>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RustFieldType {
    String,
    Bool,
    U32,
    I32,
    F32,
    F64,
    VecI32,
    VecU32,
    VecString,
    VecBool,
}

impl RustFieldType {
    /// Rust型からNiloTypeへの変換
    pub fn to_nilo_type(&self) -> NiloType {
        match self {
            RustFieldType::String => NiloType::String,
            RustFieldType::Bool => NiloType::Bool,
            RustFieldType::U32 | RustFieldType::I32 | RustFieldType::F32 | RustFieldType::F64 => {
                NiloType::Number
            }
            RustFieldType::VecI32 | RustFieldType::VecU32 => {
                NiloType::Array(Box::new(NiloType::Number))
            }
            RustFieldType::VecString => NiloType::Array(Box::new(NiloType::String)),
            RustFieldType::VecBool => NiloType::Array(Box::new(NiloType::Bool)),
        }
    }

    /// 型名の表示
    pub fn display(&self) -> String {
        match self {
            RustFieldType::String => "String".to_string(),
            RustFieldType::Bool => "bool".to_string(),
            RustFieldType::U32 => "u32".to_string(),
            RustFieldType::I32 => "i32".to_string(),
            RustFieldType::F32 => "f32".to_string(),
            RustFieldType::F64 => "f64".to_string(),
            RustFieldType::VecI32 => "Vec<i32>".to_string(),
            RustFieldType::VecU32 => "Vec<u32>".to_string(),
            RustFieldType::VecString => "Vec<String>".to_string(),
            RustFieldType::VecBool => "Vec<bool>".to_string(),
        }
    }
}

impl RustStateSchema {
    /// 新しいスキーマを作成
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }

    /// フィールドを追加
    pub fn add_field(&mut self, name: String, field_type: RustFieldType) {
        self.fields.insert(name, field_type);
    }

    /// Rustソースコードから状態スキーマを抽出（簡易実装）
    pub fn parse_from_source(source: &str) -> Option<Self> {
        let mut schema = Self::new();
        
        // nilo_state! マクロの内容を探す
        let macro_start = source.find("nilo_state!")?;
        let after_macro = &source[macro_start..];
        let struct_start = after_macro.find("struct")?;
        let brace_start = after_macro[struct_start..].find('{')?;
        let content_start = struct_start + brace_start + 1;
        
        // 対応する閉じ括弧を探す
        let mut brace_count = 1;
        let mut end_pos = content_start;
        for (i, ch) in after_macro[content_start..].chars().enumerate() {
            match ch {
                '{' => brace_count += 1,
                '}' => {
                    brace_count -= 1;
                    if brace_count == 0 {
                        end_pos = content_start + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        
        let struct_body = &after_macro[content_start..end_pos];
        
        // フィールド定義をパース
        for line in struct_body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            
            // field_name: Type の形式をパース
            if let Some(colon_pos) = line.find(':') {
                let field_name = line[..colon_pos].trim().to_string();
                let type_part = line[colon_pos + 1..]
                    .trim()
                    .trim_end_matches(',')
                    .trim();
                
                let field_type = match type_part {
                    "String" => Some(RustFieldType::String),
                    "bool" => Some(RustFieldType::Bool),
                    "u32" => Some(RustFieldType::U32),
                    "i32" => Some(RustFieldType::I32),
                    "f32" => Some(RustFieldType::F32),
                    "f64" => Some(RustFieldType::F64),
                    "Vec<i32>" => Some(RustFieldType::VecI32),
                    "Vec<u32>" => Some(RustFieldType::VecU32),
                    "Vec<String>" => Some(RustFieldType::VecString),
                    "Vec<bool>" => Some(RustFieldType::VecBool),
                    _ => None,
                };
                
                if let Some(ft) = field_type {
                    schema.add_field(field_name, ft);
                }
            }
        }
        
        Some(schema)
    }
}

/// state.xxx アクセスの型チェック
pub fn check_state_access_types(
    app: &App,
    schema: &RustStateSchema,
) -> Vec<String> {
    let mut warnings = Vec::new();
    
    // すべてのタイムラインとコンポーネントをチェック
    for timeline in &app.timelines {
        check_nodes(&timeline.body, schema, &mut warnings);
    }
    
    for component in &app.components {
        check_nodes(&component.body, schema, &mut warnings);
    }
    
    warnings
}

fn check_nodes(
    nodes: &[WithSpan<ViewNode>],
    schema: &RustStateSchema,
    warnings: &mut Vec<String>,
) {
    for node_span in nodes {
        check_node(&node_span.node, node_span.line, node_span.column, schema, warnings);
    }
}

fn check_node(
    node: &ViewNode,
    line: usize,
    column: usize,
    schema: &RustStateSchema,
    warnings: &mut Vec<String>,
) {
    match node {
        ViewNode::Set { path, value, inferred_type } => {
            // state.xxx の形式をチェック
            if let Some(field_name) = path.strip_prefix("state.") {
                if let Some(rust_type) = schema.fields.get(field_name) {
                    let expected_nilo_type = rust_type.to_nilo_type();
                    
                    // 推論された型をチェック
                    if let Some(inferred) = inferred_type {
                        if !expected_nilo_type.is_compatible_with(inferred) {
                            warnings.push(format!(
                                "{}:{} - 型の不一致: state.{} は Rust で {} 型として定義されていますが、{} 型の値が代入されました",
                                line, column, field_name, rust_type.display(), inferred.display()
                            ));
                        }
                    }
                } else {
                    warnings.push(format!(
                        "{}:{} - 未定義のフィールド: state.{} は Rust の State 構造体に存在しません",
                        line, column, field_name
                    ));
                }
            }
        }
        ViewNode::Toggle { path } => {
            if let Some(field_name) = path.strip_prefix("state.") {
                if let Some(rust_type) = schema.fields.get(field_name) {
                    if *rust_type != RustFieldType::Bool {
                        warnings.push(format!(
                            "{}:{} - 型エラー: state.{} は {} 型なので toggle できません（bool 型のみ可能）",
                            line, column, field_name, rust_type.display()
                        ));
                    }
                } else {
                    warnings.push(format!(
                        "{}:{} - 未定義のフィールド: state.{} は Rust の State 構造体に存在しません",
                        line, column, field_name
                    ));
                }
            }
        }
        ViewNode::ListAppend { path, .. } | ViewNode::ListRemove { path, .. } | ViewNode::ListClear { path } => {
            if let Some(field_name) = path.strip_prefix("state.") {
                if let Some(rust_type) = schema.fields.get(field_name) {
                    if !matches!(rust_type, RustFieldType::VecI32 | RustFieldType::VecU32 | RustFieldType::VecString | RustFieldType::VecBool) {
                        warnings.push(format!(
                            "{}:{} - 型エラー: state.{} は {} 型なのでリスト操作できません（Vec<T> 型のみ可能）",
                            line, column, field_name, rust_type.display()
                        ));
                    }
                } else {
                    warnings.push(format!(
                        "{}:{} - 未定義のフィールド: state.{} は Rust の State 構造体に存在しません",
                        line, column, field_name
                    ));
                }
            }
        }
        // 再帰的にチェック
        ViewNode::VStack(children) | ViewNode::HStack(children) => {
            check_nodes(children, schema, warnings);
        }
        ViewNode::DynamicSection { body, .. } => {
            check_nodes(body, schema, warnings);
        }
        ViewNode::Match { arms, default, .. } => {
            for (_, body) in arms {
                check_nodes(body, schema, warnings);
            }
            if let Some(default_body) = default {
                check_nodes(default_body, schema, warnings);
            }
        }
        ViewNode::ForEach { body, .. } => {
            check_nodes(body, schema, warnings);
        }
        ViewNode::If { then_body, else_body, .. } => {
            check_nodes(then_body, schema, warnings);
            if let Some(else_b) = else_body {
                check_nodes(else_b, schema, warnings);
            }
        }
        _ => {}
    }
    
    // Exprもチェック（state.xxxの読み取りアクセス）
    // TODO: Exprの中のstate.xxxアクセスもチェック
}
