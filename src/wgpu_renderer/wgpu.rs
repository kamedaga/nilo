use std::sync::Arc;
use wgpu::{
    CompositeAlphaMode, Device, Instance, PresentMode, Queue, Surface, SurfaceConfiguration,
    Texture, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
};
use winit::window::Window;

use super::image::ImageRenderer;
use super::text::TextRenderer;
use super::unified::UnifiedRenderer;
use crate::renderer_abstract::command::{DrawCommand, DrawList};
pub struct WgpuRenderer {
    window: Arc<Window>,
    device: Device,
    queue: Queue,
    surface: Surface<'static>,
    surface_format: TextureFormat,
    size: winit::dpi::PhysicalSize<u32>,
    unified_renderer: UnifiedRenderer,
    text_renderer: TextRenderer,
    image_renderer: ImageRenderer,
    depth_texture: Texture,
    depth_view: TextureView,
}

impl WgpuRenderer {
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("Failed to find a suitable adapter");

        let (device, queue) = adapter
            .request_device(&Default::default())
            .await
            .expect("Failed to create device");

        let size = window.inner_size();
        let surface_format = surface
            .get_capabilities(&adapter)
            .formats
            .first()
            .copied()
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);

        let (depth_texture, depth_view) = Self::create_depth_texture(&device, size);

        let unified_renderer = UnifiedRenderer::new(&device, surface_format);

        // グローバルに登録された全カスタムフォントを使用
        let custom_fonts = crate::get_all_custom_fonts();
        let text_renderer = if !custom_fonts.is_empty() {
            TextRenderer::with_multiple_fonts(
                &device,
                &queue,
                surface_format,
                wgpu::MultisampleState::default(),
                Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                custom_fonts,
            )
        } else {
            TextRenderer::new(
                &device,
                &queue,
                surface_format,
                wgpu::MultisampleState::default(),
                Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                size.width,
                size.height,
            )
        };

        let image_renderer = ImageRenderer::new(&device, surface_format);

        let mut renderer = Self {
            window,
            device,
            queue,
            surface,
            surface_format,
            size,
            unified_renderer,
            text_renderer,
            image_renderer,
            depth_texture,
            depth_view,
        };

        renderer.resize(size);
        renderer
    }

    fn create_depth_texture(
        device: &Device,
        size: winit::dpi::PhysicalSize<u32>,
    ) -> (Texture, TextureView) {
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        (depth_texture, depth_view)
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.size
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        // ウィンドウサイズが0の場合は何もしない）
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        self.size = new_size;
        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            view_formats: vec![self.surface_format.add_srgb_suffix()],
            alpha_mode: CompositeAlphaMode::Opaque,
            width: self.size.width,
            height: self.size.height,
            desired_maximum_frame_latency: 2,
            present_mode: PresentMode::Fifo,
        };
        self.surface.configure(&self.device, &surface_config);
        self.text_renderer
            .resize(&self.device, &self.queue, self.size.width, self.size.height);

        let (depth_texture, depth_view) = Self::create_depth_texture(&self.device, new_size);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
    }

    pub fn render(&mut self, draw_list: &DrawList, scroll_offset: [f32; 2], scale_factor: f32) {
        let surface_texture = self
            .surface
            .get_current_texture()
            .expect("Failed to acquire next surface texture");
        let texture_view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());

        // 画像の事前ロード（必要な場合のみ）
        self.preload_images(&draw_list.0);

        // 単一のRenderPassで全て描画
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Unified Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // 背景はアプリ側でフルスクリーン矩形を最背面に描くためカラークリア不要
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // stencil_to_wgpu_draw_list側で深度ソート済み。追加のソートやクローンを避けてオーバーヘッド削減。
            self.render_batched_commands(&mut rpass, &draw_list.0, scroll_offset, scale_factor);
        }

        self.queue.submit(Some(encoder.finish()));

        #[cfg(not(target_arch = "wasm32"))]
        self.window.pre_present_notify();

        surface_texture.present();
    }

    #[inline]
    fn get_command_depth(&self, cmd: &DrawCommand) -> f32 {
        match cmd {
            DrawCommand::Rect { depth, .. }
            | DrawCommand::Circle { depth, .. }
            | DrawCommand::Triangle { depth, .. }
            | DrawCommand::Image { depth, .. }
            | DrawCommand::Text { depth, .. } => *depth,
        }
    }

    fn preload_images(&mut self, commands: &[DrawCommand]) {
        for cmd in commands {
            if let DrawCommand::Image { path, .. } = cmd {
                self.image_renderer
                    .load_texture(&self.device, &self.queue, path);
            }
        }
    }

    fn render_batched_commands<'a>(
        &'a mut self,
        rpass: &mut wgpu::RenderPass<'a>,
        commands: &[DrawCommand],
        scroll_offset: [f32; 2],
        scale_factor: f32,
    ) {
        // 型ごとに事前分類（借用問題を回避）
        let mut shape_commands = Vec::new();
        let mut image_commands = Vec::new();
        let mut text_commands = Vec::new();

        for cmd in commands {
            match cmd {
                DrawCommand::Rect { .. }
                | DrawCommand::Circle { .. }
                | DrawCommand::Triangle { .. } => shape_commands.push(cmd.clone()),
                DrawCommand::Image { .. } => image_commands.push(cmd.clone()),
                DrawCommand::Text {
                    content,
                    position,
                    size,
                    color,
                    font,
                    max_width,
                    ..
                } => {
                    text_commands.push((
                        content.clone(),
                        *position,
                        *size,
                        *color,
                        font.clone(),
                        *max_width,
                    ));
                }
            }
        }

        // 統合レンダラで図形を一括描画（一つのパイプライン）
        if !shape_commands.is_empty() {
            let list = DrawList(shape_commands);
            self.unified_renderer.draw(
                rpass,
                &list,
                &self.queue,
                self.size,
                scroll_offset,
                scale_factor,
            );
        }

        // 画像描画
        if !image_commands.is_empty() {
            let list = DrawList(image_commands);
            self.image_renderer.draw(
                &self.device,
                rpass,
                &list,
                &self.queue,
                self.size,
                scroll_offset,
                scale_factor,
            );
        }

        // テキスト描画
        if !text_commands.is_empty() {
            self.text_renderer.render_multiple_texts(
                rpass,
                &text_commands,
                scroll_offset,
                scale_factor,
                &self.queue,
                &self.device,
                self.size.width,
                self.size.height,
            );
        }
    }
}
