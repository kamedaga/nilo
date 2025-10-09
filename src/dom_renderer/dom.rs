use crate::renderer_abstract::command::DrawCommand;
use crate::stencil::stencil::Stencil;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// DOM要素のID生成用カウンター
static DOM_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// DOMレンダラ
/// HTMLのDOM要素を使用してStencilを描画する
pub struct DomRenderer {
    /// レンダリングターゲットとなるコンテナ要素のID
    container_id: String,
    /// ウィンドウサイズ
    size: (u32, u32),
    /// 各Stencil用のDOM要素IDマップ（キャッシング用）
    element_cache: HashMap<String, String>,
    /// スケールファクター
    scale_factor: f32,
    /// HTML要素のバッファ（ネイティブ環境用）
    #[cfg(not(target_arch = "wasm32"))]
    html_buffer: Vec<String>,
}

impl DomRenderer {
    /// 新しいDOMレンダラを作成
    pub fn new() -> Self {
        Self::with_container("nilo-renderer-container")
    }

    /// 指定されたコンテナIDで新しいDOMレンダラを作成
    pub fn with_container(container_id: &str) -> Self {
        Self {
            container_id: container_id.to_string(),
            size: (800, 600),
            element_cache: HashMap::new(),
            scale_factor: 1.0,
            #[cfg(not(target_arch = "wasm32"))]
            html_buffer: Vec::new(),
        }
    }

    /// 一意なDOM要素IDを生成
    fn generate_element_id() -> String {
        let id = DOM_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("nilo-element-{}", id)
    }

    /// Stencilリストを描画
    /// scroll_offset[0]は未使用、scroll_offset[1]をcontent_heightとして扱う
    pub fn render_stencils(&mut self, stencils: &[Stencil], scroll_offset: [f32; 2], scale_factor: f32) {
        self.scale_factor = scale_factor;
        
        // scroll_offset[1]をcontent_heightとして扱う（DOM版専用の仕様）
        let content_height = scroll_offset[1];

        // Stencilをコマンドリストに変換
        let draw_list = crate::stencil::stencil::stencil_to_wgpu_draw_list(stencils);
        
        // コンテナの高さを設定してブラウザネイティブのスクロールを有効化
        #[cfg(target_arch = "wasm32")]
        {
            use web_sys::window;
            if let Some(window) = window() {
                if let Some(document) = window.document() {
                    if let Some(container) = document.get_element_by_id(&self.container_id) {
                        if let Some(element) = container.dyn_ref::<web_sys::HtmlElement>() {
                            let style = element.style();
                            // ビューポートの高さを取得
                            let viewport_height = window.inner_height()
                                .ok()
                                .and_then(|v| v.as_f64())
                                .unwrap_or(600.0) as f32;
                            
                            // コンテナの高さをコンテンツ高さに設定（最小でもビューポート+1pxにしてスクロール確認用）
                            let height_px = content_height.max(viewport_height + 1.0);
                            log::info!("Setting container height to: {}px (content_height: {}, viewport: {})", 
                                      height_px, content_height, viewport_height);
                            let _ = style.set_property("height", &format!("{}px", height_px));
                        }
                    }
                }
            }
        }
        
        // ネイティブ環境では警告を抑制
        #[cfg(not(target_arch = "wasm32"))]
        let _ = content_height;
        
        // WASM環境では差分更新、ネイティブ環境では全体を再構築
        #[cfg(target_arch = "wasm32")]
        {
            // コンテナをクリア（差分更新の実装は複雑なので、まずは全クリアだが将来的に改善可能）
            self.clear_container();
        }
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.clear_container();
        }

        // 各描画コマンドをDOM要素として生成
        for command in draw_list.0.iter() {
            self.render_command(command);
        }
    }

    /// コンテナをクリア
    fn clear_container(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            use web_sys::window;
            if let Some(window) = window() {
                if let Some(document) = window.document() {
                    if let Some(container) = document.get_element_by_id(&self.container_id) {
                        container.set_inner_html("");
                    }
                }
            }
        }

        // ネイティブ環境ではHTMLバッファをクリア
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.html_buffer.clear();
            log::debug!("DOM clear container: {}", self.container_id);
        }
    }

    /// 単一の描画コマンドをレンダリング
    fn render_command(&mut self, command: &DrawCommand) {
        match command {
            DrawCommand::Rect { position, width, height, color, scroll, depth } => {
                self.render_rect(*position, *width, *height, *color, *scroll, *depth);
            }
            DrawCommand::Circle { center, radius, color, segments: _, scroll, depth } => {
                self.render_circle(*center, *radius, *color, *scroll, *depth);
            }
            DrawCommand::Triangle { p1, p2, p3, color, scroll, depth } => {
                self.render_triangle(*p1, *p2, *p3, *color, *scroll, *depth);
            }
            DrawCommand::Text { content, position, size, color, font, max_width, scroll, depth } => {
                self.render_text(content, *position, *size, *color, font, *max_width, *scroll, *depth);
            }
            DrawCommand::Image { position, width, height, path, scroll, depth } => {
                self.render_image(*position, *width, *height, path, *scroll, *depth);
            }
        }
    }

    /// 矩形を描画
    fn render_rect(&mut self, position: [f32; 2], width: f32, height: f32, color: [f32; 4], scroll: bool, depth: f32) {
        let pos = self.apply_transform(position, scroll);
        let rgba = format!("rgba({}, {}, {}, {})", 
            (color[0] * 255.0) as u8,
            (color[1] * 255.0) as u8,
            (color[2] * 255.0) as u8,
            color[3]
        );

        #[cfg(target_arch = "wasm32")]
        {
            use web_sys::{window, HtmlElement};
            if let Some(window) = window() {
                if let Some(document) = window.document() {
                    if let Some(container) = document.get_element_by_id(&self.container_id) {
                        if let Ok(element) = document.create_element("div") {
                            if let Ok(element) = element.dyn_into::<HtmlElement>() {
                                let style = element.style();
                                let _ = style.set_property("position", "absolute");
                                let _ = style.set_property("left", &format!("{}px", pos[0]));
                                let _ = style.set_property("top", &format!("{}px", pos[1]));
                                let _ = style.set_property("width", &format!("{}px", width * self.scale_factor));
                                let _ = style.set_property("height", &format!("{}px", height * self.scale_factor));
                                let _ = style.set_property("background-color", &rgba);
                                let _ = style.set_property("z-index", &format!("{}", (1000.0 * (1.0 - depth)) as i32));
                                let _ = container.append_child(&element);
                            }
                        }
                    }
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let html = format!(
                r#"<div style="position: absolute; left: {}px; top: {}px; width: {}px; height: {}px; background-color: {}; z-index: {};"></div>"#,
                pos[0], pos[1], width * self.scale_factor, height * self.scale_factor, rgba, (1000.0 * (1.0 - depth)) as i32
            );
            self.html_buffer.push(html);
        }
    }

    /// 円を描画
    fn render_circle(&mut self, center: [f32; 2], radius: f32, color: [f32; 4], scroll: bool, depth: f32) {
        let pos = self.apply_transform(center, scroll);
        let rgba = format!("rgba({}, {}, {}, {})", 
            (color[0] * 255.0) as u8,
            (color[1] * 255.0) as u8,
            (color[2] * 255.0) as u8,
            color[3]
        );

        #[cfg(target_arch = "wasm32")]
        {
            use web_sys::{window, HtmlElement};
            if let Some(window) = window() {
                if let Some(document) = window.document() {
                    if let Some(container) = document.get_element_by_id(&self.container_id) {
                        if let Ok(element) = document.create_element("div") {
                            if let Ok(element) = element.dyn_into::<HtmlElement>() {
                                let style = element.style();
                                let diameter = radius * 2.0 * self.scale_factor;
                                let _ = style.set_property("position", "absolute");
                                let _ = style.set_property("left", &format!("{}px", pos[0] - radius * self.scale_factor));
                                let _ = style.set_property("top", &format!("{}px", pos[1] - radius * self.scale_factor));
                                let _ = style.set_property("width", &format!("{}px", diameter));
                                let _ = style.set_property("height", &format!("{}px", diameter));
                                let _ = style.set_property("background-color", &rgba);
                                let _ = style.set_property("border-radius", "50%");
                                let _ = style.set_property("z-index", &format!("{}", (1000.0 * (1.0 - depth)) as i32));
                                let _ = container.append_child(&element);
                            }
                        }
                    }
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let html = format!(
                r#"<div style="position: absolute; left: {}px; top: {}px; width: {}px; height: {}px; background-color: {}; border-radius: 50%; z-index: {};"></div>"#,
                pos[0] - radius * self.scale_factor, pos[1] - radius * self.scale_factor,
                radius * 2.0 * self.scale_factor, radius * 2.0 * self.scale_factor,
                rgba, (1000.0 * (1.0 - depth)) as i32
            );
            self.html_buffer.push(html);
        }
    }

    /// 三角形を描画（SVGを使用）
    fn render_triangle(&mut self, p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], color: [f32; 4], scroll: bool, depth: f32) {
        let p1_t = self.apply_transform(p1, scroll);
        let p2_t = self.apply_transform(p2, scroll);
        let p3_t = self.apply_transform(p3, scroll);
        
        let rgba = format!("rgba({}, {}, {}, {})", 
            (color[0] * 255.0) as u8,
            (color[1] * 255.0) as u8,
            (color[2] * 255.0) as u8,
            color[3]
        );

        #[cfg(target_arch = "wasm32")]
        {
            use web_sys::window;
            if let Some(window) = window() {
                if let Some(document) = window.document() {
                    if let Some(container) = document.get_element_by_id(&self.container_id) {
                        // SVG要素を作成
                        if let Ok(svg) = document.create_element_ns(Some("http://www.w3.org/2000/svg"), "svg") {
                            let style_str = format!(
                                "position: absolute; left: 0; top: 0; width: 100%; height: 100%; pointer-events: none; z-index: {};",
                                (1000.0 * (1.0 - depth)) as i32
                            );
                            let _ = svg.set_attribute("style", &style_str);

                            // polygon要素を作成
                            if let Ok(polygon) = document.create_element_ns(Some("http://www.w3.org/2000/svg"), "polygon") {
                                let points = format!("{},{} {},{} {},{}",
                                    p1_t[0], p1_t[1],
                                    p2_t[0], p2_t[1],
                                    p3_t[0], p3_t[1]
                                );
                                let _ = polygon.set_attribute("points", &points);
                                let _ = polygon.set_attribute("fill", &rgba);
                                let _ = svg.append_child(&polygon);
                            }

                            let _ = container.append_child(&svg);
                        }
                    }
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let svg = format!(
                r#"<svg style="position: absolute; left: 0; top: 0; width: 100%; height: 100%; pointer-events: none; z-index: {};"><polygon points="{},{} {},{} {},{}" fill="{}" /></svg>"#,
                (1000.0 * (1.0 - depth)) as i32,
                p1_t[0], p1_t[1],
                p2_t[0], p2_t[1],
                p3_t[0], p3_t[1],
                rgba
            );
            self.html_buffer.push(svg);
        }
    }

    /// テキストを描画
    fn render_text(&mut self, content: &str, position: [f32; 2], size: f32, color: [f32; 4], font: &str, _max_width: Option<f32>, scroll: bool, depth: f32) {
        let pos = self.apply_transform(position, scroll);
        let rgba = format!("rgba({}, {}, {}, {})", 
            (color[0] * 255.0) as u8,
            (color[1] * 255.0) as u8,
            (color[2] * 255.0) as u8,
            color[3]
        );

        #[cfg(target_arch = "wasm32")]
        {
            use web_sys::{window, HtmlElement};
            if let Some(window) = window() {
                if let Some(document) = window.document() {
                    if let Some(container) = document.get_element_by_id(&self.container_id) {
                        if let Ok(element) = document.create_element("div") {
                            if let Ok(element) = element.dyn_into::<HtmlElement>() {
                                element.set_inner_text(content);
                                let style = element.style();
                                let _ = style.set_property("position", "absolute");
                                let _ = style.set_property("left", &format!("{}px", pos[0]));
                                let _ = style.set_property("top", &format!("{}px", pos[1]));
                                let _ = style.set_property("font-size", &format!("{}px", size * self.scale_factor));
                                let _ = style.set_property("color", &rgba);
                                let _ = style.set_property("font-family", font);
                                
                                // DOM版では常に親要素の幅に合わせて自動折り返し
                                // max_widthの値は無視し、ブラウザのレイアウトエンジンに任せる
                                let _ = style.set_property("white-space", "pre-wrap");
                                let _ = style.set_property("word-wrap", "break-word");
                                let _ = style.set_property("overflow-wrap", "break-word");
                                
                                let _ = style.set_property("z-index", &format!("{}", (1000.0 * (1.0 - depth)) as i32));
                                
                                let _ = container.append_child(&element);
                            }
                        }
                    }
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // DOM版ではmax_widthを無視し、widthで自動折り返し
            // ネイティブ環境では警告を抑制
            let _ = _max_width;
            
            // HTMLエスケープ
            let escaped_content = content
                .replace("&", "&amp;")
                .replace("<", "&lt;")
                .replace(">", "&gt;")
                .replace("\"", "&quot;");
            
            let html = format!(
                r#"<div style="position: absolute; left: {}px; top: {}px; font-size: {}px; color: {}; font-family: {}; white-space: pre-wrap; word-wrap: break-word; overflow-wrap: break-word; z-index: {};">{}</div>"#,
                pos[0], pos[1], size * self.scale_factor, rgba, font, (1000.0 * (1.0 - depth)) as i32, escaped_content
            );
            self.html_buffer.push(html);
        }
    }

    /// 画像を描画
    fn render_image(&mut self, position: [f32; 2], width: f32, height: f32, path: &str, scroll: bool, depth: f32) {
        let pos = self.apply_transform(position, scroll);

        #[cfg(target_arch = "wasm32")]
        {
            use web_sys::{window, HtmlImageElement};
            if let Some(window) = window() {
                if let Some(document) = window.document() {
                    if let Some(container) = document.get_element_by_id(&self.container_id) {
                        if let Ok(element) = document.create_element("img") {
                            if let Ok(element) = element.dyn_into::<HtmlImageElement>() {
                                let _ = element.set_src(path);
                                let style = element.style();
                                let _ = style.set_property("position", "absolute");
                                let _ = style.set_property("left", &format!("{}px", pos[0]));
                                let _ = style.set_property("top", &format!("{}px", pos[1]));
                                let _ = style.set_property("width", &format!("{}px", width * self.scale_factor));
                                let _ = style.set_property("height", &format!("{}px", height * self.scale_factor));
                                let _ = style.set_property("z-index", &format!("{}", (1000.0 * (1.0 - depth)) as i32));
                                let _ = container.append_child(&element);
                            }
                        }
                    }
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let html = format!(
                r#"<img src="{}" style="position: absolute; left: {}px; top: {}px; width: {}px; height: {}px; z-index: {};" />"#,
                path, pos[0], pos[1], width * self.scale_factor, height * self.scale_factor, (1000.0 * (1.0 - depth)) as i32
            );
            self.html_buffer.push(html);
        }
    }

    /// スケールファクターを適用した座標変換（スクロールはブラウザが管理）
    fn apply_transform(&self, position: [f32; 2], _apply_scroll: bool) -> [f32; 2] {
        // スクロールはブラウザのネイティブスクロールで処理されるため、
        // ここではスケールファクターのみを適用
        [position[0] * self.scale_factor, position[1] * self.scale_factor]
    }

    /// ウィンドウサイズを取得
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// ウィンドウサイズを変更
    pub fn resize(&mut self, new_size: (u32, u32)) {
        self.size = new_size;
        
        #[cfg(target_arch = "wasm32")]
        {
            use web_sys::window;
            if let Some(window) = window() {
                if let Some(document) = window.document() {
                    if let Some(container) = document.get_element_by_id(&self.container_id) {
                        if let Some(element) = container.dyn_ref::<web_sys::HtmlElement>() {
                            let style = element.style();
                            let _ = style.set_property("width", &format!("{}px", new_size.0));
                            let _ = style.set_property("height", &format!("{}px", new_size.1));
                        }
                    }
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            log::debug!("DOM resize: {:?}", new_size);
        }
    }

    /// コンテナIDを取得
    pub fn container_id(&self) -> &str {
        &self.container_id
    }

    /// ネイティブ環境でHTMLを取得
    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_html(&self) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Nilo DOM Renderer</title>
    <style>
        body {{
            margin: 0;
            padding: 0;
            overflow: hidden;
        }}
        #{} {{
            position: relative;
            width: 100vw;
            height: 100vh;
        }}
    </style>
</head>
<body>
    <div id="{}">
{}
    </div>
</body>
</html>"#,
            self.container_id,
            self.container_id,
            self.html_buffer.join("\n")
        )
    }

    /// ネイティブ環境でHTMLファイルに保存
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;
        
        let mut file = File::create(path)?;
        file.write_all(self.get_html().as_bytes())?;
        log::info!("HTMLファイルを保存しました: {}", path);
        Ok(())
    }
}

impl Default for DomRenderer {
    fn default() -> Self {
        Self::new()
    }
}
