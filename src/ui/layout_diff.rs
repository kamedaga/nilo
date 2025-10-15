// レイアウト差分計算システム
// ノードツリーの変更を検出し、変更があった部分のみを再計算する

use crate::parser::ast::{App, Expr, Style, ViewNode, WithSpan};
use crate::ui::{LayoutParams, LayoutedNode};
use std::collections::HashMap;

/// ノードの識別子を生成
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId {
    path: Vec<String>,
}

impl NodeId {
    pub fn new() -> Self {
        Self { path: Vec::new() }
    }

    pub fn child(&self, index: usize, node_type: &str) -> Self {
        let mut new_path = self.path.clone();
        new_path.push(format!("{}_{}", node_type, index));
        Self { path: new_path }
    }

    pub fn key(&self) -> String {
        self.path.join("/")
    }
}

/// ノードのハッシュ値（変更検出用）
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeHash {
    // ノードの型と主要な属性からハッシュを生成
    hash: String,
}

impl NodeHash {
    pub fn from_node(node: &WithSpan<ViewNode>, eval: &dyn Fn(&Expr) -> String) -> Self {
        let hash = Self::compute_hash(node, eval);
        Self { hash }
    }

    fn compute_hash(node: &WithSpan<ViewNode>, eval: &dyn Fn(&Expr) -> String) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut s = String::new();

        // ノードの型と主要な属性を文字列化
        match &node.node {
            ViewNode::Text { format, args } => {
                s.push_str("Text:");
                s.push_str(format);
                for arg in args {
                    s.push_str(&eval(arg));
                }
            }
            ViewNode::Button { id, label, .. } => {
                s.push_str("Button:");
                s.push_str(id);
                s.push_str(label);
            }
            ViewNode::Image { path } => {
                s.push_str("Image:");
                s.push_str(path);
            }
            ViewNode::TextInput {
                id, placeholder, ..
            } => {
                s.push_str("TextInput:");
                s.push_str(id);
                if let Some(ph) = placeholder {
                    s.push_str(ph);
                }
            }
            ViewNode::VStack(children) => {
                s.push_str(&format!("VStack:{}", children.len()));
            }
            ViewNode::HStack(children) => {
                s.push_str(&format!("HStack:{}", children.len()));
            }
            ViewNode::Spacing(_) => {
                s.push_str("Spacing");
            }
            ViewNode::SpacingAuto => {
                s.push_str("SpacingAuto");
            }
            ViewNode::ForEach {
                var,
                iterable,
                body,
            } => {
                s.push_str("ForEach:");
                s.push_str(var);
                s.push_str(&eval(iterable));
                s.push_str(&format!("{}", body.len()));
            }
            ViewNode::If {
                condition,
                then_body,
                else_body,
            } => {
                s.push_str("If:");
                s.push_str(&eval(condition));
                let else_len = else_body.as_ref().map(|b| b.len()).unwrap_or(0);
                s.push_str(&format!("{}:{}", then_body.len(), else_len));
            }
            ViewNode::ComponentCall {
                name,
                args,
                slots: _,
            } => {
                s.push_str("Component:");
                s.push_str(name);
                for arg in args {
                    match arg {
                        crate::parser::ast::ComponentArg::Positional(expr) => {
                            s.push_str(&eval(expr))
                        }
                        crate::parser::ast::ComponentArg::Named(name, expr) => {
                            s.push_str(name);
                            s.push_str(":");
                            s.push_str(&eval(expr));
                        }
                    }
                }
            }
            ViewNode::Stencil(_) => {
                s.push_str("Stencil");
            }
            ViewNode::RustCall { name, args } => {
                s.push_str("RustCall:");
                s.push_str(name);
                for arg in args {
                    s.push_str(&eval(arg));
                }
            }
            ViewNode::DynamicSection { name, body } => {
                s.push_str("Dynamic:");
                s.push_str(name);
                s.push_str(&format!("{}", body.len()));
            }
            ViewNode::Match {
                expr,
                arms,
                default,
            } => {
                s.push_str("Match:");
                s.push_str(&eval(expr));
                s.push_str(&format!(
                    "arms:{}:default:{}",
                    arms.len(),
                    default.is_some()
                ));
            }
            ViewNode::NavigateTo { target } => {
                s.push_str("NavigateTo:");
                s.push_str(target);
            }
            ViewNode::Set { path, value, .. } => {
                s.push_str("Set:");
                s.push_str(path);
                s.push_str(&eval(value));
            }
            ViewNode::Toggle { path } => {
                s.push_str("Toggle:");
                s.push_str(path);
            }
            ViewNode::ListAppend { path, value } => {
                s.push_str("ListAppend:");
                s.push_str(path);
                s.push_str(&eval(value));
            }
            ViewNode::ListInsert { path, index, value } => {
                s.push_str("ListInsert:");
                s.push_str(path);
                s.push_str(&format!(":{}", index));
                s.push_str(&eval(value));
            }
            ViewNode::ListRemove { path, value } => {
                s.push_str("ListRemove:");
                s.push_str(path);
                s.push_str(&eval(value));
            }
            ViewNode::ListClear { path } => {
                s.push_str("ListClear:");
                s.push_str(path);
            }
            ViewNode::LetDecl {
                name,
                value,
                mutable,
                declared_type: _,
            } => {
                s.push_str(if *mutable { "Let:" } else { "Const:" });
                s.push_str(name);
                s.push_str(&eval(value));
            }
            ViewNode::When { event, actions } => {
                s.push_str("When:");
                s.push_str(&format!("{:?}:{}", event, actions.len()));
            }
            // ★ Phase 2: スロット処理
            ViewNode::Slot { name } => {
                s.push_str("Slot:");
                s.push_str(name);
            }
            ViewNode::SlotCheck { name } => {
                s.push_str("SlotCheck:");
                s.push_str(name);
            }
        }

        // スタイルもハッシュに含める
        if let Some(style) = &node.style {
            s.push_str(&Self::style_hash(style));
        }

        // 最終的なハッシュ値を計算
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    fn style_hash(style: &Style) -> String {
        let mut s = String::new();

        if let Some(c) = &style.color {
            s.push_str(&format!("c:{:?}", c));
        }
        if let Some(bg) = &style.background {
            s.push_str(&format!("bg:{:?}", bg));
        }
        if let Some(w) = style.width {
            s.push_str(&format!("w:{}", w));
        }
        if let Some(h) = style.height {
            s.push_str(&format!("h:{}", h));
        }
        if let Some(fs) = style.font_size {
            s.push_str(&format!("fs:{}", fs));
        }
        if let Some(p) = &style.padding {
            s.push_str(&format!("p:{:?}", p));
        }
        if let Some(m) = &style.margin {
            s.push_str(&format!("m:{:?}", m));
        }

        s
    }
}

/// レイアウト結果のキャッシュエントリ
#[derive(Debug, Clone)]
struct LayoutCacheEntry<'a> {
    hash: NodeHash,
    layouted: LayoutedNode<'a>,
    children_hashes: Vec<NodeHash>,
}

/// レイアウト差分計算エンジン
#[derive(Debug)]
pub struct LayoutDiffEngine<'a> {
    // 前回のレイアウト結果
    prev_cache: HashMap<String, LayoutCacheEntry<'a>>,
    // 現在のレイアウト結果
    current_cache: HashMap<String, LayoutCacheEntry<'a>>,
    // 変更があったノードのID
    dirty_nodes: Vec<String>,
}

impl<'a> LayoutDiffEngine<'a> {
    pub fn new() -> Self {
        Self {
            prev_cache: HashMap::new(),
            current_cache: HashMap::new(),
            dirty_nodes: Vec::new(),
        }
    }

    /// 差分計算を実行
    pub fn compute_diff<F, G>(
        &mut self,
        nodes: &'a [WithSpan<ViewNode>],
        params: &LayoutParams,
        app: &'a App,
        eval: &F,
        get_image_size: &G,
    ) -> Vec<LayoutedNode<'a>>
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        self.dirty_nodes.clear();

        // ルートから差分チェック
        let root_id = NodeId::new();
        let results = self.compute_node_diff(nodes, &root_id, params, app, eval, get_image_size);

        // 現在のキャッシュを前回のキャッシュとして保存
        self.prev_cache = self.current_cache.clone();

        results
    }

    /// 単一ノードの差分計算
    fn compute_node_diff<F, G>(
        &mut self,
        nodes: &'a [WithSpan<ViewNode>],
        parent_id: &NodeId,
        params: &LayoutParams,
        app: &'a App,
        eval: &F,
        get_image_size: &G,
    ) -> Vec<LayoutedNode<'a>>
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        let mut results = Vec::new();

        for (idx, node) in nodes.iter().enumerate() {
            let node_id = parent_id.child(idx, &Self::node_type_name(&node.node));
            let node_key = node_id.key();

            // ハッシュを計算
            let current_hash = NodeHash::from_node(node, eval);

            // 前回のキャッシュと比較
            let needs_recompute = if let Some(prev_entry) = self.prev_cache.get(&node_key) {
                // ハッシュが異なる場合は再計算が必要
                prev_entry.hash != current_hash
            } else {
                // 新規ノードの場合は計算が必要
                true
            };

            if needs_recompute {
                self.dirty_nodes.push(node_key.clone());

                // レイアウトを再計算
                let layouted =
                    self.compute_layout_for_node(node, &node_id, params, app, eval, get_image_size);

                results.push(layouted.clone());

                // キャッシュに保存
                self.current_cache.insert(
                    node_key,
                    LayoutCacheEntry {
                        hash: current_hash,
                        layouted,
                        children_hashes: Vec::new(),
                    },
                );
            } else {
                // キャッシュから取得
                if let Some(prev_entry) = self.prev_cache.get(&node_key) {
                    results.push(prev_entry.layouted.clone());
                    self.current_cache.insert(node_key, prev_entry.clone());
                }
            }
        }

        results
    }

    /// 実際のレイアウト計算（既存のlayout_vstackを呼び出す）
    fn compute_layout_for_node<F, G>(
        &self,
        node: &'a WithSpan<ViewNode>,
        _node_id: &NodeId,
        params: &LayoutParams,
        app: &'a App,
        eval: &F,
        get_image_size: &G,
    ) -> LayoutedNode<'a>
    where
        F: Fn(&Expr) -> String,
        G: Fn(&str) -> (u32, u32),
    {
        // 既存のレイアウトシステムを使用
        use crate::ui::layout_vstack;

        let nodes = std::slice::from_ref(node);
        let layouted_nodes = layout_vstack(nodes, params.clone(), app, eval, get_image_size);

        layouted_nodes.into_iter().next().unwrap_or_else(|| {
            // フォールバック
            LayoutedNode {
                node,
                position: params.start,
                size: [0.0, 0.0],
            }
        })
    }

    /// ノードの型名を取得
    fn node_type_name(node: &ViewNode) -> &'static str {
        match node {
            ViewNode::Text { .. } => "Text",
            ViewNode::Button { .. } => "Button",
            ViewNode::Image { .. } => "Image",
            ViewNode::TextInput { .. } => "TextInput",
            ViewNode::VStack(_) => "VStack",
            ViewNode::HStack(_) => "HStack",
            ViewNode::Spacing(_) => "Spacing",
            ViewNode::SpacingAuto => "SpacingAuto",
            ViewNode::ForEach { .. } => "ForEach",
            ViewNode::If { .. } => "If",
            ViewNode::Match { .. } => "Match",
            ViewNode::ComponentCall { .. } => "Component",
            ViewNode::Slot { .. } => "Slot",           // ★ Phase 2
            ViewNode::SlotCheck { .. } => "SlotCheck", // ★ Phase 2
            ViewNode::Stencil(_) => "Stencil",
            ViewNode::RustCall { .. } => "RustCall",
            ViewNode::DynamicSection { .. } => "Dynamic",
            ViewNode::NavigateTo { .. } => "NavigateTo",
            ViewNode::Set { .. } => "Set",
            ViewNode::Toggle { .. } => "Toggle",
            ViewNode::ListAppend { .. } => "ListAppend",
            ViewNode::ListInsert { .. } => "ListInsert",
            ViewNode::ListRemove { .. } => "ListRemove",
            ViewNode::ListClear { .. } => "ListClear",
            ViewNode::LetDecl { mutable, .. } => {
                if *mutable {
                    "Let"
                } else {
                    "Const"
                }
            }
            ViewNode::When { .. } => "When",
        }
    }

    /// 変更があったノードの数を取得
    pub fn dirty_count(&self) -> usize {
        self.dirty_nodes.len()
    }

    /// キャッシュをクリア
    pub fn clear_cache(&mut self) {
        self.prev_cache.clear();
        self.current_cache.clear();
        self.dirty_nodes.clear();
    }
}

/// 差分計算の統計情報
#[derive(Debug, Clone, Default)]
pub struct DiffStats {
    pub total_nodes: usize,
    pub recomputed_nodes: usize,
    pub cached_nodes: usize,
    pub cache_hit_rate: f32,
}

impl DiffStats {
    pub fn new(total: usize, recomputed: usize) -> Self {
        let cached = total.saturating_sub(recomputed);
        let cache_hit_rate = if total > 0 {
            (cached as f32) / (total as f32) * 100.0
        } else {
            0.0
        };

        Self {
            total_nodes: total,
            recomputed_nodes: recomputed,
            cached_nodes: cached,
            cache_hit_rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id() {
        let root = NodeId::new();
        let child1 = root.child(0, "VStack");
        let child2 = root.child(1, "HStack");

        assert_eq!(child1.key(), "VStack_0");
        assert_eq!(child2.key(), "HStack_1");

        let grandchild = child1.child(0, "Text");
        assert_eq!(grandchild.key(), "VStack_0/Text_0");
    }

    #[test]
    fn test_node_hash() {
        // テスト用のダミー評価関数
        let eval = |_: &Expr| String::from("test");

        let node1 = WithSpan {
            node: ViewNode::Text {
                format: "Hello".to_string(),
                args: vec![],
            },
            line: 1,
            column: 1,
            style: None,
        };

        let node2 = WithSpan {
            node: ViewNode::Text {
                format: "Hello".to_string(),
                args: vec![],
            },
            line: 1,
            column: 1,
            style: None,
        };

        let node3 = WithSpan {
            node: ViewNode::Text {
                format: "World".to_string(),
                args: vec![],
            },
            line: 1,
            column: 1,
            style: None,
        };

        let hash1 = NodeHash::from_node(&node1, &eval);
        let hash2 = NodeHash::from_node(&node2, &eval);
        let hash3 = NodeHash::from_node(&node3, &eval);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
