// 新しいレイアウトシステムと既存システムの統合
use crate::ui::layout_new::{LayoutEngine, LayoutContext};
use crate::ui::layout::LayoutParams;
use crate::parser::ast::{ViewNode, WithSpan, Expr, App};

// 既存システムとの互換性のためのLayoutedNode構造体
#[derive(Debug, Clone)]
pub struct LayoutedNode<'a> {
    pub node: &'a WithSpan<ViewNode>,
    pub position: [f32; 2],
    pub size: [f32; 2],
}

/// 新しいレイアウトエンジンを使用してレイアウトを実行
pub fn layout_with_new_system<'a, F, G>(
    nodes: &'a [WithSpan<ViewNode>],
    params: &LayoutParams,
    eval: &F,
    get_image_size: &G,
    app: &App,
) -> Vec<LayoutedNode<'a>>
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    let mut engine = LayoutEngine::new();
    
    // LayoutParamsをLayoutContextに変換
    let context = LayoutContext {
        window_size: params.window_size,
        parent_size: params.parent_size,
        root_font_size: params.root_font_size,
        font_size: params.font_size,
        default_font: params.default_font.clone(),
    };

    // 新しいレイアウトシステムでレイアウトを実行し、結果を変換
    let new_results = engine.layout_with_positioning(
        nodes,
        &context,
        params.parent_size,
        params.start,
        eval,
        get_image_size,
        app,
    );
    
    // 新しいLayoutedNodeから既存のLayoutedNodeに変換
    new_results.into_iter().map(|new_node| LayoutedNode {
        node: new_node.node,
        position: new_node.position,
        size: new_node.size,
    }).collect()
}

/// 単一ノードのサイズを新しいシステムで計算
pub fn calculate_node_size_with_new_system<F, G>(
    node: &WithSpan<ViewNode>,
    params: &LayoutParams,
    eval: &F,
    get_image_size: &G,
    app: &App,
) -> [f32; 2]
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    let mut engine = LayoutEngine::new();
    
    // LayoutParamsをLayoutContextに変換
    let context = LayoutContext {
        window_size: params.window_size,
        parent_size: params.parent_size,
        root_font_size: params.root_font_size,
        font_size: params.font_size,
        default_font: params.default_font.clone(),
    };

    let computed_size = engine.compute_node_size(node, &context, eval, get_image_size, app);
    [computed_size.width, computed_size.height]
}

/// 新しいレイアウトシステムが利用可能かチェック
pub fn is_new_layout_system_enabled() -> bool {
    // 環境変数やフィーチャフラグで制御可能
    std::env::var("NILO_NEW_LAYOUT")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}

/// VStackレイアウトを新しいシステムで実行（簡易版）
pub fn layout_vstack_with_new_system<'a, F, G>(
    children: &'a [WithSpan<ViewNode>],
    params: &LayoutParams,
    eval: &F,
    get_image_size: &G,
    app: &App,
) -> Vec<LayoutedNode<'a>>
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    // 子要素を個別にレイアウト
    let mut results = Vec::new();
    let mut current_y = params.start[1];
    
    for child in children {
        let size = calculate_node_size_with_new_system(child, params, eval, get_image_size, app);
        
        results.push(LayoutedNode {
            node: child,
            position: [params.start[0], current_y],
            size,
        });
        
        current_y += size[1] + params.spacing;
    }
    
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::Style;
    
    #[test]
    fn test_integration_with_existing_params() {
        let params = LayoutParams {
            start: [10.0, 20.0],
            spacing: 15.0,
            window_size: [1920.0, 1080.0],
            parent_size: [800.0, 600.0],
            root_font_size: 16.0,
            font_size: 18.0,
            default_font: "Arial".to_string(),
        };
        
        let text_node = WithSpan {
            node: ViewNode::Text {
                format: "Integration Test".to_string(),
                args: vec![],
            },
            style: Some(Style {
                font_size: Some(24.0),
                ..Default::default()
            }),
            line: 1,
            column: 1,
        };
        
        let nodes = vec![text_node];
        let eval = |_: &Expr| String::new();
        let get_image_size = |_: &str| (100, 100);
        let app = App {
            flow: crate::parser::ast::Flow {
                start: "start".to_string(),
                transitions: vec![],
            },
            timelines: vec![],
            components: vec![],
        };
        
        let results = layout_with_new_system(&nodes, &params, &eval, &get_image_size, &app);
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].position, [10.0, 20.0]); // 開始位置が正しく使用される
        assert!(results[0].size[0] > 0.0);
        assert!(results[0].size[1] > 0.0);
    }
    
    #[test]
    fn test_node_size_calculation() {
        let params = LayoutParams {
            start: [0.0, 0.0],
            spacing: 10.0,
            window_size: [1920.0, 1080.0],
            parent_size: [1920.0, 1080.0],
            root_font_size: 16.0,
            font_size: 16.0,
            default_font: "Arial".to_string(),
        };
        
        let node_with_explicit_size = WithSpan {
            node: ViewNode::Text {
                format: "Fixed Size".to_string(),
                args: vec![],
            },
            style: Some(Style {
                width: Some(300.0),
                height: Some(50.0),
                ..Default::default()
            }),
            line: 1,
            column: 1,
        };
        
        let eval = |_: &Expr| String::new();
        let get_image_size = |_: &str| (100, 100);
        let app = App {
            flow: crate::parser::ast::Flow {
                start: "start".to_string(),
                transitions: vec![],
            },
            timelines: vec![],
            components: vec![],
        };
        
        let size = calculate_node_size_with_new_system(&node_with_explicit_size, &params, &eval, &get_image_size, &app);
        
        // 明示的なサイズが適用されることを確認
        assert_eq!(size[0], 300.0);
        assert_eq!(size[1], 50.0);
    }
}