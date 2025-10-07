// レイアウトキャッシュ管理 - 差分計算システムを使用// レイアウトキャッシュ管理 - 差分計算システムを使用// レイアウトキャッシュ管理 - 差分計算システムを使用

// 変更があった部分のみを再計算する差分ベースのキャッシュシステム

// 変更があった部分のみを再計算する差分ベースのキャッシュシステム// 変更があった部分のみを再計算する差分ベースのキャッシュシステム

use crate::parser::ast::{ViewNode, WithSpan, Expr, App};

use crate::ui::{LayoutedNode, LayoutParams};

use crate::ui::layout_diff::{LayoutDiffEngine, DiffStats};

use std::sync::Arc;use crate::parser::ast::{ViewNode, WithSpan, Expr, App};use crate::parser::ast::{ViewNode, WithSpan, Expr, App};

use parking_lot::Mutex;

use crate::ui::{LayoutedNode, LayoutParams};use crate::ui::{LayoutedNode, LayoutParams};

/// スレッドセーフなレイアウトキャッシュ（差分計算ベース）

pub struct LayoutCache<'a> {use crate::ui::layout_diff::{LayoutDiffEngine, DiffStats};use crate::ui::layout_diff::{LayoutDiffEngine, DiffStats};

    engine: Arc<Mutex<LayoutDiffEngine<'a>>>,

    stats: Arc<Mutex<DiffStats>>,use std::sync::Arc;use std::sync::Arc;

}

use parking_lot::Mutex;use parking_lot::Mutex;

impl<'a> LayoutCache<'a> {

    /// 新しいキャッシュを作成

    pub fn new() -> Self {

        Self {/// スレッドセーフなレイアウトキャッシュ（差分計算ベース）/// スレッドセーフなレイアウトキャッシュ（差分計算ベース）

            engine: Arc::new(Mutex::new(LayoutDiffEngine::new())),

            stats: Arc::new(Mutex::new(DiffStats::default())),pub struct LayoutCache<'a> {pub struct LayoutCache<'a> {

        }

    }    engine: Arc<Mutex<LayoutDiffEngine<'a>>>,    engine: Arc<Mutex<LayoutDiffEngine<'a>>>,



    /// 差分計算を実行してレイアウトを取得    stats: Arc<Mutex<DiffStats>>,    stats: Arc<Mutex<DiffStats>>,

    pub fn layout_with_diff<F, G>(

        &self,}}

        nodes: &'a [WithSpan<ViewNode>],

        params: &LayoutParams,

        app: &App,

        eval: &F,impl<'a> LayoutCache<'a> {impl<'a> LayoutCache<'a> {

        get_image_size: &G,

    ) -> Vec<LayoutedNode<'a>>    /// 新しいキャッシュを作成    /// 新しいキャッシュを作成

    where

        F: Fn(&Expr) -> String,    pub fn new() -> Self {    pub fn new() -> Self {

        G: Fn(&str) -> (u32, u32),

    {        Self {        Self {

        let mut engine = self.engine.lock();

        let results = engine.compute_diff(nodes, params, app, eval, get_image_size);            engine: Arc::new(Mutex::new(LayoutDiffEngine::new())),            engine: Arc::new(Mutex::new(LayoutDiffEngine::new())),

        

        // 統計情報を更新            stats: Arc::new(Mutex::new(DiffStats::default())),            stats: Arc::new(Mutex::new(DiffStats::default())),

        let total = nodes.len();

        let dirty = engine.dirty_count();        }        }

        let mut stats = self.stats.lock();

        *stats = DiffStats::new(total, dirty);    }    }

        

        results

    }

    /// 差分計算を実行してレイアウトを取得    /// 差分計算を実行してレイアウトを取得

    /// キャッシュをクリア

    pub fn clear(&self) {    pub fn layout_with_diff<F, G>(    pub fn layout_with_diff<F, G>(

        let mut engine = self.engine.lock();

        engine.clear_cache();        &self,        &self,

        

        let mut stats = self.stats.lock();        nodes: &'a [WithSpan<ViewNode>],        nodes: &'a [WithSpan<ViewNode>],

        *stats = DiffStats::default();

    }        params: &LayoutParams,        params: &LayoutParams,



    /// 統計情報を取得        app: &App,        app: &App,

    pub fn get_stats(&self) -> DiffStats {

        let stats = self.stats.lock();        eval: &F,        eval: &F,

        stats.clone()

    }        get_image_size: &G,        get_image_size: &G,



    /// ログ出力（デバッグ用）    ) -> Vec<LayoutedNode<'a>>    ) -> Vec<LayoutedNode<'a>>

    pub fn log_stats(&self) {

        let stats = self.get_stats();    where    where

        log::debug!(

            "Layout diff stats: total={}, recomputed={}, cached={}, hit_rate={:.1}%",        F: Fn(&Expr) -> String,        F: Fn(&Expr) -> String,

            stats.total_nodes,

            stats.recomputed_nodes,        G: Fn(&str) -> (u32, u32),        G: Fn(&str) -> (u32, u32),

            stats.cached_nodes,

            stats.cache_hit_rate    {    {

        );

    }        let mut engine = self.engine.lock();        let mut engine = self.engine.lock();

}

        let results = engine.compute_diff(nodes, params, app, eval, get_image_size);        let results = engine.compute_diff(nodes, params, app, eval, get_image_size);

impl<'a> Default for LayoutCache<'a> {

    fn default() -> Self {                

        Self::new()

    }        // 統計情報を更新        // 統計情報を更新

}

        let total = nodes.len();        let total = nodes.len();

/// グローバルレイアウトキャッシュ（オプション）

static GLOBAL_LAYOUT_CACHE: once_cell::sync::Lazy<Mutex<Option<LayoutCache<'static>>>> =        let dirty = engine.dirty_count();        let dirty = engine.dirty_count();

    once_cell::sync::Lazy::new(|| Mutex::new(None));

        let mut stats = self.stats.lock();        let mut stats = self.stats.lock();

/// グローバルキャッシュを有効化

pub fn enable_global_cache() {        *stats = DiffStats::new(total, dirty);        *stats = DiffStats::new(total, dirty);

    let mut cache = GLOBAL_LAYOUT_CACHE.lock();

    *cache = Some(LayoutCache::new());                

}

        results        results

/// グローバルキャッシュを無効化

pub fn disable_global_cache() {    }    }

    let mut cache = GLOBAL_LAYOUT_CACHE.lock();

    *cache = None;

}

    /// キャッシュをクリア    /// キャッシュをクリア

/// グローバルキャッシュの統計を取得

pub fn get_global_cache_stats() -> Option<DiffStats> {    pub fn clear(&self) {    pub fn clear(&self) {

    let cache = GLOBAL_LAYOUT_CACHE.lock();

    cache.as_ref().map(|c| c.get_stats())        let mut engine = self.engine.lock();        let mut engine = self.engine.lock();

}

        engine.clear_cache();        engine.clear_cache();

/// グローバルキャッシュをクリア

pub fn clear_global_cache() {                

    let cache = GLOBAL_LAYOUT_CACHE.lock();

    if let Some(c) = cache.as_ref() {        let mut stats = self.stats.lock();        let mut stats = self.stats.lock();

        c.clear();

    }        *stats = DiffStats::default();        *stats = DiffStats::default();

}

    }    }



    /// 統計情報を取得    /// 統計情報を取得

    pub fn get_stats(&self) -> DiffStats {    pub fn get_stats(&self) -> DiffStats {

        let stats = self.stats.lock();        let stats = self.stats.lock();

        stats.clone()        stats.clone()

    }    }



    /// ログ出力（デバッグ用）    /// ログ出力（デバッグ用）

    pub fn log_stats(&self) {    pub fn log_stats(&self) {

        let stats = self.get_stats();        let stats = self.get_stats();

        log::debug!(        log::debug!(

            "Layout diff stats: total={}, recomputed={}, cached={}, hit_rate={:.1}%",            "Layout diff stats: total={}, recomputed={}, cached={}, hit_rate={:.1}%",

            stats.total_nodes,            stats.total_nodes,

            stats.recomputed_nodes,            stats.recomputed_nodes,

            stats.cached_nodes,            stats.cached_nodes,

            stats.cache_hit_rate            stats.cache_hit_rate

        );        );

    }    }

}}



impl<'a> Default for LayoutCache<'a> {impl<'a> Default for LayoutCache<'a> {

    fn default() -> Self {    fn default() -> Self {

        Self::new()        Self::new()

    }    }

}}



/// グローバルレイアウトキャッシュ（オプション）/// グローバルレイアウトキャッシュ（オプション）

static GLOBAL_LAYOUT_CACHE: once_cell::sync::Lazy<Mutex<Option<LayoutCache<'static>>>> =static GLOBAL_LAYOUT_CACHE: once_cell::sync::Lazy<Mutex<Option<LayoutCache<'static>>>> =

    once_cell::sync::Lazy::new(|| Mutex::new(None));    once_cell::sync::Lazy::new(|| Mutex::new(None));



/// グローバルキャッシュを有効化/// グローバルキャッシュを有効化

pub fn enable_global_cache() {pub fn enable_global_cache() {

    let mut cache = GLOBAL_LAYOUT_CACHE.lock();    let mut cache = GLOBAL_LAYOUT_CACHE.lock();

    *cache = Some(LayoutCache::new());    *cache = Some(LayoutCache::new());

}}



/// グローバルキャッシュを無効化/// グローバルキャッシュを無効化

pub fn disable_global_cache() {pub fn disable_global_cache() {

    let mut cache = GLOBAL_LAYOUT_CACHE.lock();    let mut cache = GLOBAL_LAYOUT_CACHE.lock();

    *cache = None;    *cache = None;

}}



/// グローバルキャッシュの統計を取得/// グローバルキャッシュの統計を取得

pub fn get_global_cache_stats() -> Option<DiffStats> {pub fn get_global_cache_stats() -> Option<DiffStats> {

    let cache = GLOBAL_LAYOUT_CACHE.lock();    let cache = GLOBAL_LAYOUT_CACHE.lock();

    cache.as_ref().map(|c| c.get_stats())    cache.as_ref().map(|c| c.get_stats())

}}



/// グローバルキャッシュをクリア/// グローバルキャッシュをクリア

pub fn clear_global_cache() {pub fn clear_global_cache() {

    let cache = GLOBAL_LAYOUT_CACHE.lock();    let cache = GLOBAL_LAYOUT_CACHE.lock();

    if let Some(c) = cache.as_ref() {    if let Some(c) = cache.as_ref() {

        c.clear();        c.clear();

    }    }

}}



#[cfg(test)]#[cfg(test)]

mod tests {mod tests {

    use super::*;    use super::*;



    #[test]    #[test]

    fn test_layout_cache_creation() {    fn test_layout_cache_creation() {

        let cache = LayoutCache::new();        let cache = LayoutCache::new();

        let stats = cache.get_stats();        let stats = cache.get_stats();

                

        assert_eq!(stats.total_nodes, 0);        assert_eq!(stats.total_nodes, 0);

        assert_eq!(stats.recomputed_nodes, 0);        assert_eq!(stats.recomputed_nodes, 0);

        assert_eq!(stats.cached_nodes, 0);        assert_eq!(stats.cached_nodes, 0);

    }    }



    #[test]    #[test]

    fn test_global_cache() {    fn test_global_cache() {

        enable_global_cache();        enable_global_cache();

        assert!(get_global_cache_stats().is_some());        assert!(get_global_cache_stats().is_some());

                

        clear_global_cache();        clear_global_cache();

        let stats = get_global_cache_stats().unwrap();        let stats = get_global_cache_stats().unwrap();

        assert_eq!(stats.total_nodes, 0);        assert_eq!(stats.total_nodes, 0);

                

        disable_global_cache();        disable_global_cache();

        assert!(get_global_cache_stats().is_none());        assert!(get_global_cache_stats().is_none());

    }    }

}}


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