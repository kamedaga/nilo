/// レイアウトキャッシュシステム - Nilo軽量化用
/// 同じサイズ・パラメータでのレイアウト結果をキャッシュして再計算を削減

use crate::parser::ast::{ViewNode, WithSpan};
use crate::ui::LayoutedNode;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// レイアウトパラメータのハッシュ用キー
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LayoutCacheKey {
    pub window_size: [u32; 2], // f32をu32に変換してハッシュ化
    pub parent_size: [u32; 2],
    pub font_size: u32,
    pub node_count: usize,
    pub content_hash: u64,
}

impl LayoutCacheKey {
    pub fn from_params(
        window_size: [f32; 2],
        parent_size: [f32; 2], 
        font_size: f32,
        nodes: &[WithSpan<ViewNode>]
    ) -> Self {
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        
        // ノードの内容をハッシュ化（簡略版）
        for node in nodes {
            match &node.node {
                ViewNode::Text { format, .. } => {
                    "text".hash(&mut hasher);
                    format.hash(&mut hasher);
                }
                ViewNode::Button { label, id, .. } => {
                    "button".hash(&mut hasher);
                    label.hash(&mut hasher);
                    id.hash(&mut hasher);
                }
                ViewNode::VStack(_) => "vstack".hash(&mut hasher),
                ViewNode::HStack(_) => "hstack".hash(&mut hasher),
                ViewNode::DynamicSection { .. } => continue, // DynamicSectionはキャッシュしない
                _ => "other".hash(&mut hasher),
            }
        }
        
        Self {
            window_size: [window_size[0] as u32, window_size[1] as u32],
            parent_size: [parent_size[0] as u32, parent_size[1] as u32],
            font_size: font_size as u32,
            node_count: nodes.len(),
            content_hash: hasher.finish(),
        }
    }
}

/// キャッシュされたレイアウト結果
#[derive(Debug, Clone)]
pub struct CachedLayoutResult {
    pub positions: Vec<[f32; 2]>,
    pub sizes: Vec<[f32; 2]>,
    pub total_size: [f32; 2],
    pub timestamp: std::time::Instant,
}

/// レイアウトキャッシュシステム
#[derive(Debug, Clone)]
pub struct LayoutCache {
    cache: HashMap<LayoutCacheKey, CachedLayoutResult>,
    max_entries: usize,
    max_age: std::time::Duration,
}

impl LayoutCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            max_entries: 100, // 最大100個のレイアウト結果をキャッシュ
            max_age: std::time::Duration::from_secs(30), // 30秒でキャッシュ期限切れ
        }
    }
    
    /// キャッシュからレイアウト結果を取得
    pub fn get(&self, key: &LayoutCacheKey) -> Option<&CachedLayoutResult> {
        if let Some(result) = self.cache.get(key) {
            // 期限切れチェック
            if result.timestamp.elapsed() < self.max_age {
                Some(result)
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// レイアウト結果をキャッシュに保存
    pub fn insert(&mut self, key: LayoutCacheKey, result: CachedLayoutResult) {
        // キャッシュサイズ制限
        if self.cache.len() >= self.max_entries {
            self.cleanup_old_entries();
        }
        
        self.cache.insert(key, result);
    }
    
    /// 古いキャッシュエントリを削除
    fn cleanup_old_entries(&mut self) {
        let now = std::time::Instant::now();
        self.cache.retain(|_, result| now.duration_since(result.timestamp) < self.max_age);
        
        // まだ制限を超えている場合は、古いものから削除
        if self.cache.len() >= self.max_entries {
            let entries: Vec<_> = self.cache.iter().map(|(k, v)| (k.clone(), v.timestamp)).collect();
            let mut sorted_entries = entries;
            sorted_entries.sort_by_key(|(_, timestamp)| *timestamp);
            
            let remove_count = self.cache.len() - self.max_entries / 2;
            let keys_to_remove: Vec<_> = sorted_entries.iter()
                .take(remove_count)
                .map(|(key, _)| key.clone())
                .collect();
                
            for key in keys_to_remove {
                self.cache.remove(&key);
            }
        }
    }
    
    /// キャッシュをクリア
    pub fn clear(&mut self) {
        self.cache.clear();
    }
    
    /// キャッシュ統計情報
    pub fn stats(&self) -> (usize, usize) {
        (self.cache.len(), self.max_entries)
    }
    
    /// レイアウト結果をキャッシュ結果に変換
    pub fn layout_to_cache_result(layouted_nodes: &[LayoutedNode<'_>]) -> CachedLayoutResult {
        let positions: Vec<[f32; 2]> = layouted_nodes.iter().map(|n| n.position).collect();
        let sizes: Vec<[f32; 2]> = layouted_nodes.iter().map(|n| n.size).collect();
        
        let total_size = if layouted_nodes.is_empty() {
            [0.0, 0.0]
        } else {
            let max_x = layouted_nodes.iter()
                .map(|n| n.position[0] + n.size[0])
                .fold(0.0, f32::max);
            let max_y = layouted_nodes.iter()
                .map(|n| n.position[1] + n.size[1])
                .fold(0.0, f32::max);
            [max_x, max_y]
        };
        
        CachedLayoutResult {
            positions,
            sizes,
            total_size,
            timestamp: std::time::Instant::now(),
        }
    }
}

impl Default for LayoutCache {
    fn default() -> Self {
        Self::new()
    }
}