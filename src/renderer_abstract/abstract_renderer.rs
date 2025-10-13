use crate::stencil::stencil::Stencil;
use std::any::Any;

/// レンダラの種類を定義
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererType {
    Wgpu,
    Dom,
    TinySkia,
    Pdf,
}

/// 抽象化されたレンダラトレイト
pub trait AbstractRenderer: Send + Sync {
    /// レンダラの種類を返す
    fn renderer_type(&self) -> RendererType;

    /// Stencilリストを描画する
    fn render_stencils(&mut self, stencils: &[Stencil], scroll_offset: [f32; 2], scale_factor: f32);

    /// ウィンドウサイズを取得
    fn size(&self) -> (u32, u32);

    /// ウィンドウサイズを変更
    fn resize(&mut self, new_size: (u32, u32));

    /// レンダラ固有の初期化処理
    fn initialize(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// レンダラ固有のクリーンアップ処理
    fn cleanup(&mut self) {}

    /// レンダラ固有のデータにアクセス（必要に応じて）
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// レンダラファクトリー
pub struct RendererFactory;

impl RendererFactory {
    /// 指定されたタイプのレンダラを作成
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn create_renderer(
        renderer_type: RendererType,
        window: Option<std::sync::Arc<winit::window::Window>>,
    ) -> Result<Box<dyn AbstractRenderer>, String> {
        match renderer_type {
            RendererType::Wgpu => {
                if let Some(window) = window {
                    let wgpu_renderer = crate::wgpu_renderer::WgpuRenderer::new(window).await;
                    Ok(Box::new(WgpuRendererAdapter::new(wgpu_renderer)))
                } else {
                    Err("WGPU renderer requires a window".to_string())
                }
            }
            RendererType::Dom => {
                let dom_renderer = crate::dom_renderer::DomRenderer::with_container("container");
                Ok(Box::new(DomRendererAdapter::new(dom_renderer)))
            }
            RendererType::TinySkia => Ok(Box::new(TinySkiaRenderer::new())),
            RendererType::Pdf => Ok(Box::new(PdfRenderer::new())),
        }
    }

    /// WASM環境用のレンダラ作成
    #[cfg(target_arch = "wasm32")]
    pub async fn create_renderer(
        renderer_type: RendererType,
    ) -> Result<Box<dyn AbstractRenderer>, String> {
        match renderer_type {
            RendererType::Dom => {
                let dom_renderer = crate::dom_renderer::DomRenderer::with_container("container");
                Ok(Box::new(DomRendererAdapter::new(dom_renderer)))
            }
            _ => Err(format!(
                "{:?} renderer is not supported in WASM environment",
                renderer_type
            )),
        }
    }
}

/// WGPUレンダラのアダプター
#[cfg(feature = "wgpu")]
pub struct WgpuRendererAdapter {
    inner: crate::wgpu_renderer::WgpuRenderer,
}

#[cfg(feature = "wgpu")]
impl WgpuRendererAdapter {
    pub fn new(renderer: crate::wgpu_renderer::WgpuRenderer) -> Self {
        Self { inner: renderer }
    }
}

#[cfg(feature = "wgpu")]
impl AbstractRenderer for WgpuRendererAdapter {
    fn renderer_type(&self) -> RendererType {
        RendererType::Wgpu
    }

    fn render_stencils(
        &mut self,
        stencils: &[Stencil],
        scroll_offset: [f32; 2],
        scale_factor: f32,
    ) {
        let draw_list = crate::stencil::stencil::stencil_to_wgpu_draw_list(stencils);
        self.inner.render(&draw_list, scroll_offset, scale_factor);
    }

    fn size(&self) -> (u32, u32) {
        let size = self.inner.size();
        (size.width, size.height)
    }

    fn resize(&mut self, new_size: (u32, u32)) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner
                .resize(winit::dpi::PhysicalSize::new(new_size.0, new_size.1));
        }
        #[cfg(target_arch = "wasm32")]
        {
            // WASM環境ではサイズ変更は異なる方法で処理
            let _ = new_size; // 未使用警告を抑制
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// DOMレンダラのアダプター
pub struct DomRendererAdapter {
    inner: crate::dom_renderer::DomRenderer,
}

impl DomRendererAdapter {
    pub fn new(renderer: crate::dom_renderer::DomRenderer) -> Self {
        Self { inner: renderer }
    }

    /// ネイティブ環境でHTMLファイルに保存
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        self.inner.save_to_file(path)
    }
}

impl AbstractRenderer for DomRendererAdapter {
    fn renderer_type(&self) -> RendererType {
        RendererType::Dom
    }

    fn render_stencils(
        &mut self,
        stencils: &[Stencil],
        scroll_offset: [f32; 2],
        scale_factor: f32,
    ) {
        self.inner
            .render_stencils(stencils, scroll_offset, scale_factor);
    }

    fn size(&self) -> (u32, u32) {
        self.inner.size()
    }

    fn resize(&mut self, new_size: (u32, u32)) {
        self.inner.resize(new_size);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// DOMレンダラ（将来実装用のスタブ）
pub struct DomRenderer {
    size: (u32, u32),
}

impl DomRenderer {
    pub fn new() -> Self {
        Self { size: (800, 600) }
    }
}

impl AbstractRenderer for DomRenderer {
    fn renderer_type(&self) -> RendererType {
        RendererType::Dom
    }

    fn render_stencils(
        &mut self,
        _stencils: &[Stencil],
        _scroll_offset: [f32; 2],
        _scale_factor: f32,
    ) {
        // TODO: DOM要素の生成と更新
        log::debug!("DOM rendering not yet implemented");
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }

    fn resize(&mut self, new_size: (u32, u32)) {
        self.size = new_size;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// tiny-skiaレンダラ（将来実装用のスタブ）
pub struct TinySkiaRenderer {
    size: (u32, u32),
}

impl TinySkiaRenderer {
    pub fn new() -> Self {
        Self { size: (800, 600) }
    }
}

impl AbstractRenderer for TinySkiaRenderer {
    fn renderer_type(&self) -> RendererType {
        RendererType::TinySkia
    }

    fn render_stencils(
        &mut self,
        _stencils: &[Stencil],
        _scroll_offset: [f32; 2],
        _scale_factor: f32,
    ) {
        // TODO: tiny-skiaでの描画実装
        log::debug!("tiny-skia rendering not yet implemented");
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }

    fn resize(&mut self, new_size: (u32, u32)) {
        self.size = new_size;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// PDFレンダラ（将来実装用のスタブ）
pub struct PdfRenderer {
    size: (u32, u32),
}

impl PdfRenderer {
    pub fn new() -> Self {
        Self { size: (800, 600) }
    }
}

impl AbstractRenderer for PdfRenderer {
    fn renderer_type(&self) -> RendererType {
        RendererType::Pdf
    }

    fn render_stencils(
        &mut self,
        _stencils: &[Stencil],
        _scroll_offset: [f32; 2],
        _scale_factor: f32,
    ) {
        // TODO: PDF書き出し実装
        log::debug!("PDF rendering not yet implemented");
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }

    fn resize(&mut self, new_size: (u32, u32)) {
        self.size = new_size;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
