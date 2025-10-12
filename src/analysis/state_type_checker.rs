use crate::parser::ast::{App, ViewNode, Expr, WithSpan, NiloType, BinaryOperator};
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

/// ローカル変数のコンテキスト
#[derive(Debug, Clone)]
struct LocalVarContext {
    /// 変数名 -> NiloType のマッピング
    variables: HashMap<String, NiloType>,
}

impl LocalVarContext {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }
    
    fn declare_var(&mut self, name: String, var_type: NiloType) {
        self.variables.insert(name, var_type);
    }
    
    fn get_var_type(&self, name: &str) -> Option<&NiloType> {
        self.variables.get(name)
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
        let mut local_ctx = LocalVarContext::new();
        check_nodes(&timeline.body, schema, &mut warnings, &mut local_ctx);
    }
    
    for component in &app.components {
        let mut local_ctx = LocalVarContext::new();
        check_nodes(&component.body, schema, &mut warnings, &mut local_ctx);
    }
    
    warnings
}

fn check_nodes(
    nodes: &[WithSpan<ViewNode>],
    schema: &RustStateSchema,
    warnings: &mut Vec<String>,
    local_ctx: &mut LocalVarContext,
) {
    for node_span in nodes {
        check_node(&node_span.node, node_span.line, node_span.column, schema, warnings, local_ctx);
    }
}

/// 式から型を推論する簡易版
fn infer_expr_type(expr: &Expr, local_ctx: &LocalVarContext) -> NiloType {
    match expr {
        Expr::String(_) => NiloType::String,
        Expr::Number(_) => NiloType::Number,
        Expr::Bool(_) => NiloType::Bool,
        Expr::Path(path) => {
            // ローカル変数の場合
            if !path.starts_with("state.") {
                let var_name = path.trim();
                if let Some(var_type) = local_ctx.get_var_type(var_name) {
                    var_type.clone()
                } else {
                    NiloType::Unknown
                }
            } else {
                // state.xxx の場合、schemaから取得（簡易版）
                NiloType::Unknown
            }
        }
        Expr::Ident(name) => {
            // ローカル変数
            if let Some(var_type) = local_ctx.get_var_type(name) {
                var_type.clone()
            } else {
                NiloType::Unknown
            }
        }
        Expr::BinaryOp { left, op, right } => {
            let left_ty = infer_expr_type(left, local_ctx);
            let right_ty = infer_expr_type(right, local_ctx);
            match op {
                BinaryOperator::Add | BinaryOperator::Sub |
                BinaryOperator::Mul | BinaryOperator::Div => {
                    if left_ty == NiloType::Number && right_ty == NiloType::Number {
                        NiloType::Number
                    } else {
                        NiloType::String
                    }
                }
                BinaryOperator::Eq | BinaryOperator::Ne |
                BinaryOperator::Lt | BinaryOperator::Le |
                BinaryOperator::Gt | BinaryOperator::Ge => {
                    NiloType::Bool
                }
            }
        }
        Expr::Array(items) => {
            if items.is_empty() {
                NiloType::Array(Box::new(NiloType::String)) // デフォルト
            } else {
                let first_type = infer_expr_type(&items[0], local_ctx);
                NiloType::Array(Box::new(first_type))
            }
        }
        _ => NiloType::String, // デフォルト
    }
}

/// 式内の変数アクセスをチェックする関数
fn check_expr(
    expr: &Expr,
    line: usize,
    column: usize,
    schema: &RustStateSchema,
    warnings: &mut Vec<String>,
    local_ctx: &LocalVarContext,
) {
    match expr {
        Expr::Ident(var_name) => {
            // ローカル変数の使用チェック
            if local_ctx.get_var_type(var_name).is_none() {
                warnings.push(format!(
                    "{}:{} - 未定義の変数: ローカル変数 '{}' が宣言されていません",
                    line, column, var_name
                ));
            }
        }
        Expr::Path(path) => {
            // state.xxx の形式をチェック
            if let Some(field_name) = path.strip_prefix("state.") {
                if schema.fields.get(field_name).is_none() {
                    warnings.push(format!(
                        "{}:{} - 未定義のフィールド: state.{} は Rust の State 構造体に存在しません",
                        line, column, field_name
                    ));
                }
            }
        }
        Expr::BinaryOp { left, right, .. } => {
            check_expr(left, line, column, schema, warnings, local_ctx);
            check_expr(right, line, column, schema, warnings, local_ctx);
        }
        Expr::FunctionCall { args, .. } => {
            for arg in args {
                check_expr(arg, line, column, schema, warnings, local_ctx);
            }
        }
        Expr::Array(items) => {
            for item in items {
                check_expr(item, line, column, schema, warnings, local_ctx);
            }
        }
        Expr::Object(fields) => {
            for (_, value) in fields {
                check_expr(value, line, column, schema, warnings, local_ctx);
            }
        }
        Expr::Match { expr: match_expr, arms, default } => {
            check_expr(match_expr, line, column, schema, warnings, local_ctx);
            for arm in arms {
                check_expr(&arm.pattern, line, column, schema, warnings, local_ctx);
                check_expr(&arm.value, line, column, schema, warnings, local_ctx);
            }
            if let Some(default_expr) = default {
                check_expr(default_expr, line, column, schema, warnings, local_ctx);
            }
        }
        Expr::CalcExpr(inner) => {
            check_expr(inner, line, column, schema, warnings, local_ctx);
        }
        _ => {} // String, Number, Bool, Dimension はチェック不要
    }
}

fn check_node(
    node: &ViewNode,
    line: usize,
    column: usize,
    schema: &RustStateSchema,
    warnings: &mut Vec<String>,
    local_ctx: &mut LocalVarContext,
) {
    match node {
        ViewNode::LetDecl { name, value, declared_type, .. } => {
            // ローカル変数の宣言: 型注釈があればそれを使用、なければ値から推論
            let var_type = if let Some(decl_type) = declared_type {
                decl_type.clone()
            } else {
                infer_expr_type(value, local_ctx)
            };
            
            local_ctx.declare_var(name.clone(), var_type);
        }
        ViewNode::Set { path, value, inferred_type } => {
            // ローカル変数への代入チェック
            if !path.starts_with("state.") {
                let var_name = path.trim();
                if let Some(expected_type) = local_ctx.get_var_type(var_name) {
                    // ローカル変数の型チェック: local_ctxを使って型を再推論
                    let actual_inferred = infer_expr_type(value, local_ctx);
                    if !expected_type.is_compatible_with(&actual_inferred) {
                        warnings.push(format!(
                            "{}:{} - 型の不一致: ローカル変数 {} は {} 型として宣言されていますが、{} 型の値が代入されました",
                            line, column, var_name, expected_type.display(), actual_inferred.display()
                        ));
                    }
                }
            } else {
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
            // value の式をチェック
            check_expr(value, line, column, schema, warnings, &*local_ctx);
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
        ViewNode::ListAppend { path, value } => {
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
            check_expr(value, line, column, schema, warnings, local_ctx);
        }
        ViewNode::ListInsert { path, index: _, value } => {
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
            check_expr(value, line, column, schema, warnings, local_ctx);
        }
        ViewNode::ListRemove { path, value } => {
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
            check_expr(value, line, column, schema, warnings, local_ctx);
        }
        ViewNode::ListClear { path } => {
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
        ViewNode::Text { format: _, args } => {
            for arg in args {
                check_expr(arg, line, column, schema, warnings, local_ctx);
            }
        }
        ViewNode::Button { onclick, .. } => {
            if let Some(expr) = onclick {
                check_expr(expr, line, column, schema, warnings, local_ctx);
            }
        }
        ViewNode::TextInput { value, on_change, .. } => {
            if let Some(expr) = value {
                check_expr(expr, line, column, schema, warnings, local_ctx);
            }
            if let Some(expr) = on_change {
                check_expr(expr, line, column, schema, warnings, local_ctx);
            }
        }
        ViewNode::ComponentCall { args, .. } => {
            for arg in args {
                check_expr(arg, line, column, schema, warnings, local_ctx);
            }
        }
        ViewNode::Match { expr, arms, default } => {
            check_expr(expr, line, column, schema, warnings, local_ctx);
            for (_, body) in arms {
                check_nodes(body, schema, warnings, local_ctx);
            }
            if let Some(default_body) = default {
                check_nodes(default_body, schema, warnings, local_ctx);
            }
        }
        ViewNode::ForEach { iterable, body, .. } => {
            check_expr(iterable, line, column, schema, warnings, local_ctx);
            // ForEach内は新しいスコープ
            let mut foreach_ctx = local_ctx.clone();
            check_nodes(body, schema, warnings, &mut foreach_ctx);
        }
        ViewNode::If { condition, then_body, else_body } => {
            check_expr(condition, line, column, schema, warnings, local_ctx);
            check_nodes(then_body, schema, warnings, local_ctx);
            if let Some(else_b) = else_body {
                check_nodes(else_b, schema, warnings, local_ctx);
            }
        }
        ViewNode::RustCall { args, .. } => {
            for arg in args {
                check_expr(arg, line, column, schema, warnings, local_ctx);
            }
        }
        // 再帰的にチェック
        ViewNode::VStack(children) | ViewNode::HStack(children) => {
            check_nodes(children, schema, warnings, local_ctx);
        }
        ViewNode::DynamicSection { body, .. } => {
            check_nodes(body, schema, warnings, local_ctx);
        }
        _ => {}
    }
}
