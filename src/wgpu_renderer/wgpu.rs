use std::sync::Arc;
use wgpu::{
    Color, CompositeAlphaMode, Device, Instance, Queue, Surface, SurfaceConfiguration,
    TextureFormat, TextureUsages, PresentMode, TextureViewDescriptor, Texture, TextureView,
};
use winit::window::Window;

use super::text::TextRenderer;
use super::circle::CircleRenderer;
use crate::renderer_abstract::command::{DrawCommand, DrawList};
use super::quad::QuadRenderer;
use super::triangle::TriangleRenderer;
use super::image::ImageRenderer;

pub struct WgpuRenderer {
    window: Arc<Window>,
    device: Device,
    queue: Queue,
    surface: Surface<'static>,
    surface_format: TextureFormat,
    size: winit::dpi::PhysicalSize<u32>,
    quad_renderer: QuadRenderer,
    triangle_renderer: TriangleRenderer,
    circle_renderer: CircleRenderer,
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

        let quad_renderer = QuadRenderer::new(&device, surface_format);
        let triangle_renderer = TriangleRenderer::new(&device, surface_format);
        let circle_renderer = CircleRenderer::new(&device, surface_format);
        
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
            quad_renderer,
            triangle_renderer,
            circle_renderer,
            text_renderer,
            image_renderer,
            depth_texture,
            depth_view,
        };

        renderer.resize(size);
        renderer
    }

    fn create_depth_texture(device: &Device, size: winit::dpi::PhysicalSize<u32>) -> (Texture, TextureView) {
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
            alpha_mode: CompositeAlphaMode::Auto,
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
        let texture_view = surface_texture.texture.create_view(&TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());

        // 画像の事前ロード（必要な場合のみ）
        self.preload_images(&draw_list.0);

        // 深度順にソート（in-place）
        let mut commands = draw_list.0.clone();
        commands.sort_unstable_by(|a, b| {
            self.get_command_depth(a).partial_cmp(&self.get_command_depth(b)).unwrap_or(std::cmp::Ordering::Equal)
        });

        // 単一のRenderPassで全て描画
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Unified Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Color::WHITE),
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

            self.render_batched_commands(&mut rpass, &commands, scroll_offset, scale_factor);
        }

        self.queue.submit(Some(encoder.finish()));

        #[cfg(not(target_arch = "wasm32"))]
        self.window.pre_present_notify();

        surface_texture.present();
    }

    #[inline]
    fn get_command_depth(&self, cmd: &DrawCommand) -> f32 {
        match cmd {
            DrawCommand::Rect { depth, .. } |
            DrawCommand::Circle { depth, .. } |
            DrawCommand::Triangle { depth, .. } |
            DrawCommand::Image { depth, .. } |
            DrawCommand::Text { depth, .. } => *depth,
        }
    }

    fn preload_images(&mut self, commands: &[DrawCommand]) {
        let mut loaded = false;
        for cmd in commands {
            if let DrawCommand::Image { path, .. } = cmd {
                if !loaded {
                    self.image_renderer.load_texture(&self.device, &self.queue, path);
                    loaded = true; // 最初の画像のみロード（最適化）
                }
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
        let mut rect_commands = Vec::new();
        let mut circle_commands = Vec::new();
        let mut triangle_commands = Vec::new();
        let mut image_commands = Vec::new();
        let mut text_commands = Vec::new();

        for cmd in commands {
            match cmd {
                DrawCommand::Rect { .. } => rect_commands.push(cmd.clone()),
                DrawCommand::Circle { .. } => circle_commands.push(cmd.clone()),
                DrawCommand::Triangle { .. } => triangle_commands.push(cmd.clone()),
                DrawCommand::Image { .. } => image_commands.push(cmd.clone()),
                DrawCommand::Text { content, position, size, color, font, max_width, .. } => {
                    // ★ 修正: max_width情報も含める
                    text_commands.push((content.clone(), *position, *size, *color, font.clone(), *max_width));
                }
            }
        }

        // 安全に順次描画
        if !rect_commands.is_empty() {
            let list = DrawList(rect_commands);
            self.quad_renderer.draw(rpass, &list, &self.queue, self.size, scroll_offset, scale_factor);
        }

        if !circle_commands.is_empty() {
            let list = DrawList(circle_commands);
            self.circle_renderer.draw(rpass, &list, &self.queue, self.size, scroll_offset, scale_factor, 0.0);
        }

        if !triangle_commands.is_empty() {
            let list = DrawList(triangle_commands);
            self.triangle_renderer.draw(rpass, &list, &self.queue, self.size, scroll_offset, scale_factor);
        }

        if !image_commands.is_empty() {
            let list = DrawList(image_commands);
            self.image_renderer.draw(&self.device, rpass, &list, &self.queue, self.size, scroll_offset, scale_factor);
        }

        if !text_commands.is_empty() {
            self.text_renderer.render_multiple_texts(
                rpass, &text_commands, scroll_offset, scale_factor,
                &self.queue, &self.device, self.size.width, self.size.height,
            );
        }
    }
}
