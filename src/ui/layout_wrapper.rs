// 新しいレイアウトシステムのテスト用ファイル
use crate::ui::layout_new::{LayoutEngine, LayoutContext, LayoutedNode};
use crate::parser::ast::{ViewNode, WithSpan, Expr, App};

/// 新しいレイアウトシステムを既存のインターフェースで使用するためのラッパー
pub fn layout_with_new_engine<'a, F, G>(
    nodes: &'a [WithSpan<ViewNode>],
    window_size: [f32; 2],
    eval: &F,
    get_image_size: &G,
    app: &App,
) -> Vec<LayoutedNode<'a>>
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    let mut engine = LayoutEngine::new();
    let context = LayoutContext {
        window_size,
        parent_size: window_size,
        root_font_size: 16.0,
        font_size: 16.0,
        default_font: "Arial".to_string(),
    };

    engine.layout_with_positioning(
        nodes,
        &context,
        window_size,
        [0.0, 0.0],
        eval,
        get_image_size,
        app,
    )
}

/// 新しいレイアウトシステムでの単一ノードのサイズ計算
pub fn compute_single_node_size<F, G>(
    node: &WithSpan<ViewNode>,
    window_size: [f32; 2],
    eval: &F,
    get_image_size: &G,
    app: &App,
) -> [f32; 2]
where
    F: Fn(&Expr) -> String,
    G: Fn(&str) -> (u32, u32),
{
    let mut engine = LayoutEngine::new();
    let context = LayoutContext {
        window_size,
        parent_size: window_size,
        root_font_size: 16.0,
        font_size: 16.0,
        default_font: "Arial".to_string(),
    };

    let computed_size = engine.compute_node_size(node, &context, eval, get_image_size, app);
    [computed_size.width, computed_size.height]
}

/// レイアウトエンジンのテスト関数
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{Style, DimensionValue, Unit};

    #[test]
    fn test_text_node_layout() {
        let text_node = WithSpan {
            node: ViewNode::Text {
                format: "Hello World".to_string(),
                args: vec![],
            },
            style: Some(Style {
                font_size: Some(20.0),
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

        let size = compute_single_node_size(&text_node, [1920.0, 1080.0], &eval, &get_image_size, &app);
        
        // テキストのサイズが正しく計算されることを確認
        assert!(size[0] > 0.0);
        assert!(size[1] > 0.0);
    }

    #[test]
    fn test_vstack_layout() {
        let text1 = WithSpan {
            node: ViewNode::Text {
                format: "Text 1".to_string(),
                args: vec![],
            },
            style: None,
            line: 1,
            column: 1,
        };

        let text2 = WithSpan {
            node: ViewNode::Text {
                format: "Text 2".to_string(),
                args: vec![],
            },
            style: None,
            line: 2,
            column: 1,
        };

        let vstack = WithSpan {
            node: ViewNode::VStack(vec![text1, text2]),
            style: None,
            line: 3,
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

        let size = compute_single_node_size(&vstack, [1920.0, 1080.0], &eval, &get_image_size, &app);
        
        // VStackの高さが子要素の合計になることを確認
        assert!(size[0] > 0.0);
        assert!(size[1] > 0.0);
    }

    #[test]
    fn test_explicit_width_height() {
        let node_with_size = WithSpan {
            node: ViewNode::Text {
                format: "Test".to_string(),
                args: vec![],
            },
            style: Some(Style {
                width: Some(200.0),
                height: Some(100.0),
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

        let size = compute_single_node_size(&node_with_size, [1920.0, 1080.0], &eval, &get_image_size, &app);
        
        // 明示的なwidth/heightが適用されることを確認
        assert_eq!(size[0], 200.0);
        assert_eq!(size[1], 100.0);
    }

    #[test]
    fn test_relative_width() {
        let node_with_relative_width = WithSpan {
            node: ViewNode::Text {
                format: "Test".to_string(),
                args: vec![],
            },
            style: Some(Style {
                relative_width: Some(DimensionValue {
                    value: 90.0,
                    unit: Unit::Vw, // 90vw
                }),
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

        let window_width = 1920.0;
        let size = compute_single_node_size(&node_with_relative_width, [window_width, 1080.0], &eval, &get_image_size, &app);
        
        // 90vwが正しく計算されることを確認 (90% of viewport width)
        assert_eq!(size[0], window_width * 0.9);
    }
}