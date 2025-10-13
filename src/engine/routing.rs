// ========================================
// Nilo フレームワーク: ルーティングシステム
// ========================================

use crate::parser::ast::{App, Flow};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RouteInfo {
    pub pattern: String,
}

// WASM環境でのルーター実装
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone)]
pub struct WasmRouter {
    routes: HashMap<String, RouteInfo>, // timeline_name -> RouteInfo
    url_to_timeline: HashMap<String, String>, // url_pattern -> timeline_name
    current_route: Option<String>,
    history: Vec<String>,
}

#[cfg(target_arch = "wasm32")]
impl WasmRouter {
    /// AppからタイムラインのURL定義を読み取ってルーターを構築
    pub fn from_app(app: &App) -> Self {
        let mut routes = HashMap::new();
        let mut url_to_timeline = HashMap::new();

        // 各タイムラインからURL定義を取得
        for timeline in &app.timelines {
            if let Some(url_pattern) = &timeline.url_pattern {
                routes.insert(
                    timeline.name.clone(),
                    RouteInfo {
                        pattern: url_pattern.clone(),
                    },
                );
                url_to_timeline.insert(url_pattern.clone(), timeline.name.clone());
                log::info!("Registered route: {} -> {}", url_pattern, timeline.name);
            }
        }

        Self {
            routes,
            url_to_timeline,
            current_route: None,
            history: Vec::new(),
        }
    }

    /// 現在のブラウザURLから対応するタイムラインを検索
    pub fn get_timeline_from_current_url(&self) -> Option<String> {
        use web_sys::window;

        if let Some(window) = window() {
            let location = window.location();
            if let Ok(pathname) = location.pathname() {
                log::info!("Current URL pathname: {}", pathname);
                return self.match_route(&pathname).map(|(timeline, _)| timeline);
            }
        }
        None
    }

    /// タイムラインに遷移し、ブラウザのURLを更新
    pub fn navigate_to_timeline(
        &mut self,
        timeline: &str,
        params: HashMap<String, String>,
    ) -> Result<(), String> {
        log::info!("Navigating to timeline: {}", timeline);

        if let Some(route_info) = self.routes.get(timeline) {
            let url = self.build_url(&route_info.pattern, &params)?;
            log::info!("Built URL: {}", url);
            self.update_browser_url(&url);
            self.current_route = Some(url.clone());
            self.history.push(url);
            Ok(())
        } else {
            // URL定義がない場合はネイティブ風に動作
            log::warn!(
                "Timeline '{}' has no URL mapping, using native navigation",
                timeline
            );
            Ok(())
        }
    }

    pub fn get_current_route(&self) -> Option<&str> {
        self.current_route.as_deref()
    }

    fn build_url(&self, pattern: &str, params: &HashMap<String, String>) -> Result<String, String> {
        let mut url = pattern.to_string();

        // パラメータ置換（:param形式）
        for (key, value) in params {
            let placeholder = format!(":{}", key);
            let optional_placeholder = format!("{}?", placeholder);

            if url.contains(&placeholder) {
                url = url.replace(&placeholder, value);
            } else if url.contains(&optional_placeholder) {
                url = url.replace(&optional_placeholder, value);
            }
        }

        // オプショナルパラメータで値が提供されていない場合の処理
        // :param? を空文字に置換（簡単な実装）
        while let Some(start) = url.find("/:") {
            if let Some(end) = url[start..].find('?') {
                let full_end = start + end + 1;
                url.replace_range(start..full_end, "");
            } else {
                break;
            }
        }

        Ok(url)
    }

    fn update_browser_url(&self, url: &str) {
        use web_sys::window;

        log::info!("Updating browser URL to: {}", url);

        if let Some(window) = window() {
            if let Ok(history) = window.history() {
                match history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(url)) {
                    Ok(_) => log::info!("Browser URL updated successfully"),
                    Err(e) => log::error!("Failed to update browser URL: {:?}", e),
                }
            }
        }
    }

    pub fn handle_browser_navigation(&mut self) -> Option<(String, HashMap<String, String>)> {
        // ブラウザの戻る/進むボタン対応
        use web_sys::window;

        if let Some(window) = window() {
            let location = window.location();
            if let Ok(pathname) = location.pathname() {
                return self.match_route(&pathname);
            }
        }
        None
    }

    fn match_route(&self, pathname: &str) -> Option<(String, HashMap<String, String>)> {
        log::info!("Matching route for pathname: {}", pathname);

        // URL正規化: 末尾の/を削除（ルートパス以外）
        let normalized_path = if pathname != "/" && pathname.ends_with('/') {
            pathname.trim_end_matches('/')
        } else {
            pathname
        };

        log::info!("Normalized pathname: {}", normalized_path);

        // まず完全一致を試みる
        if let Some(timeline) = self.url_to_timeline.get(normalized_path) {
            log::info!("Found exact match: {} -> {}", normalized_path, timeline);
            return Some((timeline.clone(), HashMap::new()));
        }

        // URL パターンマッチング
        for (timeline, route_info) in &self.routes {
            if let Some(params) = self.extract_params(&route_info.pattern, normalized_path) {
                log::info!(
                    "Found pattern match: {} matches {} -> {}",
                    normalized_path,
                    route_info.pattern,
                    timeline
                );
                return Some((timeline.clone(), params));
            }
        }

        log::warn!("No matching route found for: {}", normalized_path);
        None
    }

    fn extract_params(&self, pattern: &str, pathname: &str) -> Option<HashMap<String, String>> {
        // 単純なパターンマッチング実装
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let path_parts: Vec<&str> = pathname.split('/').collect();

        if pattern_parts.len() != path_parts.len() {
            return None;
        }

        let mut params = HashMap::new();

        for (pattern_part, path_part) in pattern_parts.iter().zip(path_parts.iter()) {
            if pattern_part.starts_with(':') {
                let param_name = pattern_part.trim_start_matches(':').trim_end_matches('?');
                params.insert(param_name.to_string(), path_part.to_string());
            } else if pattern_part != path_part {
                return None;
            }
        }

        Some(params)
    }
}

// ネイティブ環境での簡易ルーター
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct NativeRouter {
    current_timeline: String,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeRouter {
    pub fn new(flow: &Flow) -> Self {
        Self {
            current_timeline: flow.start.clone(),
        }
    }

    pub fn navigate_to_timeline(
        &mut self,
        timeline: &str,
        _params: HashMap<String, String>,
    ) -> Result<(), String> {
        self.current_timeline = timeline.to_string();
        Ok(())
    }

    pub fn get_current_timeline(&self) -> &str {
        &self.current_timeline
    }
}

// プラットフォーム統合ルーター
#[derive(Debug, Clone)]
pub enum Router {
    #[cfg(target_arch = "wasm32")]
    Wasm(WasmRouter),
    #[cfg(not(target_arch = "wasm32"))]
    Native(NativeRouter),
}

impl Router {
    pub fn new(flow: &Flow) -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            log::warn!("Router::new(flow) is deprecated. Use Router::from_app(app) instead.");
            // ダミーのWasmRouterを作成（URL定義なし）
            Router::Wasm(WasmRouter {
                routes: HashMap::new(),
                url_to_timeline: HashMap::new(),
                current_route: None,
                history: Vec::new(),
            })
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Router::Native(NativeRouter::new(flow))
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn from_app(app: &App) -> Self {
        Router::Wasm(WasmRouter::from_app(app))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_app(app: &App) -> Self {
        // ネイティブでは最初のタイムラインをデフォルトに
        let start_timeline = app
            .timelines
            .first()
            .map(|t| t.name.clone())
            .unwrap_or_else(|| "Home".to_string());
        Router::Native(NativeRouter {
            current_timeline: start_timeline,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn get_timeline_from_current_url(&self) -> Option<String> {
        match self {
            Router::Wasm(router) => router.get_timeline_from_current_url(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_timeline_from_current_url(&self) -> Option<String> {
        None // ネイティブ版ではブラウザURLがないので常にNone
    }

    pub fn navigate_to_timeline(
        &mut self,
        timeline: &str,
        params: HashMap<String, String>,
    ) -> Result<(), String> {
        match self {
            #[cfg(target_arch = "wasm32")]
            Router::Wasm(router) => router.navigate_to_timeline(timeline, params),
            #[cfg(not(target_arch = "wasm32"))]
            Router::Native(router) => router.navigate_to_timeline(timeline, params),
        }
    }
}
