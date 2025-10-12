// ========================================
// 型システムモジュール
// ========================================
//
// このモジュールはNilo言語の型推論と型チェックを担当します。

use pest::iterators::Pair;
use crate::parser::ast::*;
use crate::parser::parse::Rule;

/// 式から基本的な型を推論する（パーサーレベル）
pub fn infer_expr_type(expr: &Expr) -> NiloType {
    match expr {
        // プリミティブ型の推論
        Expr::Number(_) => NiloType::Number,
        Expr::String(_) => NiloType::String,
        Expr::Bool(_) => NiloType::Bool,
        
        // 配列の型推論
        Expr::Array(items) => {
            if items.is_empty() {
                // 空配列はAny[]
                NiloType::Array(Box::new(NiloType::Any))
            } else {
                // 最初の要素の型を配列の型とする（簡易版）
                let first_type = infer_expr_type(&items[0]);
                NiloType::Array(Box::new(first_type))
            }
        }
        
        // 二項演算の型推論
        Expr::BinaryOp { left, op, right } => {
            let left_ty = infer_expr_type(left);
            let right_ty = infer_expr_type(right);
            
            match op {
                BinaryOperator::Add | BinaryOperator::Sub |
                BinaryOperator::Mul | BinaryOperator::Div => {
                    // 算術演算: 両方がNumberならNumber、それ以外はString（暗黙変換）
                    if left_ty == NiloType::Number && right_ty == NiloType::Number {
                        NiloType::Number
                    } else {
                        NiloType::String
                    }
                }
                BinaryOperator::Eq | BinaryOperator::Ne |
                BinaryOperator::Lt | BinaryOperator::Le |
                BinaryOperator::Gt | BinaryOperator::Ge => {
                    // 比較演算: 常にBool
                    NiloType::Bool
                }
            }
        }
        
        // その他の式は型が不明
        Expr::Path(_) | Expr::Ident(_) => NiloType::Unknown,
        Expr::Object(_) => NiloType::Unknown,
        Expr::Dimension(_) => NiloType::Number,  // 次元値は数値として扱う
        Expr::CalcExpr(inner) => infer_expr_type(inner),
        Expr::Match { .. } => NiloType::Unknown,  // Matchは複雑なので後で実装
        Expr::FunctionCall { .. } => NiloType::Unknown,  // 関数の戻り値は不明
    }
}

/// 型付き式を作成（パーサーで使用）
pub fn make_typed_expr(expr: Expr) -> TypedExpr {
    let inferred_type = infer_expr_type(&expr);
    TypedExpr::new(expr, inferred_type)
}

/// 型の互換性をチェック
pub fn check_type_compatibility(expected: &NiloType, actual: &NiloType) -> Result<(), String> {
    if expected.is_compatible_with(actual) {
        Ok(())
    } else {
        Err(format!(
            "型エラー: {} 型が期待されていますが、{} 型が見つかりました",
            expected.display(),
            actual.display()
        ))
    }
}

/// 型式をパースする
pub fn parse_type_expr(pair: Pair<Rule>) -> NiloType {
    let type_str = pair.as_str();
    let mut inner = pair.into_inner();
    let primitive_pair = inner.next().unwrap();
    
    // プリミティブ型を取得（大文字小文字両対応）
    let mut base_type = match primitive_pair.as_str() {
        "Number" | "number" => NiloType::Number,
        "String" | "string" => NiloType::String,
        "Bool" | "bool" => NiloType::Bool,
        "Any" | "any" => NiloType::Any,
        _ => {
            eprintln!("Unknown type: {}", primitive_pair.as_str());
            NiloType::Unknown
        }
    };
    
    // "[]" の数だけ配列でラップ
    let remaining_text = type_str[primitive_pair.as_str().len()..].trim();
    let array_depth = remaining_text.matches("[]").count();
    
    for _ in 0..array_depth {
        base_type = NiloType::Array(Box::new(base_type));
    }
    
    base_type
}
