/// 差分表示システム - Nilo軽量化用
/// UI要素の変更を追跡し、必要な部分のみを再描画する

use crate::parser::ast::{ViewNode, WithSpan, Style};
use crate::stencil::stencil::Stencil;
use crate::ui::LayoutedNode;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// UI要素の変更を検出するためのハッシュ値
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeHash(u64);

impl NodeHash {
    pub fn new(value: u64) -> Self {
        Self(value)
    }
    
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// ノードの内容からハッシュを生成
pub fn compute_node_hash(node: &WithSpan<ViewNode>) -> NodeHash {
    use std::collections::hash_map::DefaultHasher;
    
    let mut hasher = DefaultHasher::new();
    
    // ノードの種類と主要な属性をハッシュ化
    match &node.node {
        ViewNode::Text { format, args } => {
            "text".hash(&mut hasher);
            format.hash(&mut hasher);
            // args の評価結果も含める（後で実装）
        }
        ViewNode::Button { label, id, onclick: _ } => {
            "button".hash(&mut hasher);
            label.hash(&mut hasher);
            id.hash(&mut hasher);
        }
        ViewNode::Image { path } => {
            "image".hash(&mut hasher);
            path.hash(&mut hasher);
        }
        ViewNode::VStack(children) => {
            "vstack".hash(&mut hasher);
            children.len().hash(&mut hasher);
        }
        ViewNode::HStack(children) => {
            "hstack".hash(&mut hasher);
            children.len().hash(&mut hasher);
        }
        ViewNode::DynamicSection { name, .. } => {
            "dynamic_section".hash(&mut hasher);
            name.hash(&mut hasher);
            // DynamicSectionは差分チェックをスキップ
            return NodeHash::new(0); // 常に変更ありとして扱う
        }
        _ => {
            "other".hash(&mut hasher);
        }
    }
    
    // スタイルも含める
    if let Some(style) = &node.style {
        hash_style(style, &mut hasher);
    }
    
    NodeHash::new(hasher.finish())
}

/// スタイルのハッシュ化
fn hash_style<H: Hasher>(style: &Style, hasher: &mut H) {
    // 主要なスタイル属性のみをハッシュ化（パフォーマンス重視）
    if let Some(color) = &style.color {
        "color".hash(hasher);
        // ColorValueのハッシュ化（簡略版）
        match color {
            crate::parser::ast::ColorValue::Rgb(r, g, b) => {
                (*r as u32).hash(hasher);
                (*g as u32).hash(hasher);
                (*b as u32).hash(hasher);
            }
            crate::parser::ast::ColorValue::Rgba(r, g, b, a) => {
                (*r as u32).hash(hasher);
                (*g as u32).hash(hasher);
                (*b as u32).hash(hasher);
                (*a as u32).hash(hasher);
            }
            crate::parser::ast::ColorValue::Named(name) => name.hash(hasher),
        }
    }
    
    if let Some(bg) = &style.background {
        "background".hash(hasher);
        // backgroundのハッシュ化も同様
    }
    
    if let Some(font_size) = style.font_size {
        "font_size".hash(hasher);
        (font_size as u32).hash(hasher);
    }
    
    if let Some(width) = style.width {
        "width".hash(hasher);
        (width as u32).hash(hasher);
    }
    
    if let Some(height) = style.height {
        "height".hash(hasher);
        (height as u32).hash(hasher);
    }
}

/// 差分検出結果
#[derive(Debug, Clone)]
pub struct DiffResult {
    pub changed_nodes: HashSet<usize>, // 変更されたノードのインデックス
    pub added_nodes: Vec<usize>,       // 新規追加されたノード
    pub removed_nodes: Vec<usize>,     // 削除されたノード
    pub layout_changed: bool,          // レイアウトが変更されたか
}

/// 差分管理システム
#[derive(Debug, Clone)]
pub struct DiffTracker {
    /// 前回のノードハッシュマップ
    pub previous_hashes: HashMap<usize, NodeHash>,
    /// 前回のレイアウト結果
    pub previous_layout: Vec<([f32; 2], [f32; 2])>, // (position, size)
    /// 前回のステンシル数
    pub previous_stencil_count: usize,
    /// 静的部分が変更されたか
    pub static_part_dirty: bool,
}

impl DiffTracker {
    pub fn new() -> Self {
        Self {
            previous_hashes: HashMap::new(),
            previous_layout: Vec::new(),
            previous_stencil_count: 0,
            static_part_dirty: true, // 初回は必ず描画
        }
    }
    
    /// ノード配列の差分を検出
    pub fn detect_changes(&mut self, 
                         current_nodes: &[WithSpan<ViewNode>],
                         current_layout: &[LayoutedNode<'_>]) -> DiffResult {
        let mut result = DiffResult {
            changed_nodes: HashSet::new(),
            added_nodes: Vec::new(),
            removed_nodes: Vec::new(),
            layout_changed: false,
        };
        
        // 現在のハッシュを計算
        let mut current_hashes = HashMap::new();
        let mut current_layout_data = Vec::new();
        
        for (i, node) in current_nodes.iter().enumerate() {
            // DynamicSectionは差分チェックをスキップ
            if matches!(node.node, ViewNode::DynamicSection { .. }) {
                continue;
            }
            
            let hash = compute_node_hash(node);
            current_hashes.insert(i, hash);
            
            // レイアウト情報も記録
            if i < current_layout.len() {
                current_layout_data.push((
                    current_layout[i].position,
                    current_layout[i].size
                ));
            }
        }
        
        // ハッシュの比較
        for (i, &hash) in &current_hashes {
            if let Some(&prev_hash) = self.previous_hashes.get(i) {
                if prev_hash != hash {
                    result.changed_nodes.insert(*i);
                }
            } else {
                result.added_nodes.push(*i);
            }
        }
        
        // 削除されたノードを検出
        for &i in self.previous_hashes.keys() {
            if !current_hashes.contains_key(&i) {
                result.removed_nodes.push(i);
            }
        }
        
        // レイアウトの変更を検出
        if current_layout_data.len() != self.previous_layout.len() {
            result.layout_changed = true;
        } else {
            for (i, &(pos, size)) in current_layout_data.iter().enumerate() {
                if i < self.previous_layout.len() {
                    let (prev_pos, prev_size) = self.previous_layout[i];
                    if (pos[0] - prev_pos[0]).abs() > 0.1 ||
                       (pos[1] - prev_pos[1]).abs() > 0.1 ||
                       (size[0] - prev_size[0]).abs() > 0.1 ||
                       (size[1] - prev_size[1]).abs() > 0.1 {
                        result.layout_changed = true;
                        break;
                    }
                }
            }
        }
        
        // 状態を更新
        self.previous_hashes = current_hashes;
        self.previous_layout = current_layout_data;
        
        result
    }
    
    /// 静的部分の変更をマーク
    pub fn mark_static_dirty(&mut self) {
        self.static_part_dirty = true;
    }
    
    /// 静的部分の変更をクリア
    pub fn clear_static_dirty(&mut self) {
        self.static_part_dirty = false;
    }
    
    /// 静的部分が変更されているかチェック
    pub fn is_static_dirty(&self) -> bool {
        self.static_part_dirty
    }
    
    /// ステンシル数の変更を検出
    pub fn detect_stencil_count_change(&mut self, current_count: usize) -> bool {
        let changed = self.previous_stencil_count != current_count;
        self.previous_stencil_count = current_count;
        changed
    }
    
    /// 完全リセット（ウィンドウリサイズ時など）
    pub fn reset(&mut self) {
        self.previous_hashes.clear();
        self.previous_layout.clear();
        self.previous_stencil_count = 0;
        self.static_part_dirty = true;
    }
}

impl Default for DiffTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// 軽量化のための最適化設定
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    /// 差分表示を有効にするか
    pub enable_diff_rendering: bool,
    /// 静的コンテンツのキャッシュを有効にするか
    pub enable_static_cache: bool,
    /// DynamicSectionで差分表示を無効にするか
    pub disable_diff_in_dynamic_sections: bool,
    /// フレームレート制限（FPS）
    pub target_fps: Option<u32>,
    /// デバッグ情報の表示
    pub show_debug_info: bool,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            enable_diff_rendering: true,
            enable_static_cache: true,
            disable_diff_in_dynamic_sections: true, // 要求通りDynamicSectionでは差分無効
            target_fps: Some(60),
            show_debug_info: false,
        }
    }
}