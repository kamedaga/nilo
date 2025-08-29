use wgpu::{
    Device, Queue, RenderPass, RenderPipeline, Buffer, BufferUsages, VertexBufferLayout,
    VertexAttribute, VertexFormat, VertexStepMode,
};
use crate::renderer::command::{DrawCommand, DrawList};
use winit::dpi::PhysicalSize;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct TriangleVertex {
    position: [f32; 3], // ★ Z座標を追加
    color: u32, // packed RGBA8 (A<<24 | R<<16 | G<<8 | B)
}

impl TriangleVertex {
    fn desc<'a>() -> VertexBufferLayout<'a> {
        use std::mem;
        VertexBufferLayout {
            array_stride: mem::size_of::<TriangleVertex>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x3, // ★ Float32x3に変更
                },
                VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress, // ★ オフセット調整
                    shader_location: 1,
                    format: VertexFormat::Uint32,
                },
            ],
        }
    }
}

pub struct TriangleRenderer {
    pipeline: RenderPipeline,
    vertex_buffer: Buffer,
    vertex_count: u32,
}

impl TriangleRenderer {
    pub fn new(device: &Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/triangle.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Triangle Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Triangle Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TriangleVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING), // ★ アルファブレンド対応
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            // ★ Depth Test設定
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Triangle Vertex Buffer"),
            size: 1024 * 1024,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            vertex_count: 0,
        }
    }

    // ★ Z値を指定できる描画メソッド
    #[allow(dead_code)]
    pub fn draw_with_depth<'a>(
        &'a mut self,
        pass: &mut RenderPass<'a>,
        draw_list: &DrawList,
        queue: &Queue,
        screen_size: PhysicalSize<u32>,
        scroll_offset: [f32; 2],
        scale_factor: f32,
        _depth: f32, // ★ Z値（0.0=最前面、1.0=最背面）
    ) {
        let mut vertices: Vec<TriangleVertex> = Vec::new();

        let w = screen_size.width as f32;
        let h = screen_size.height as f32;

        fn to_ndc(pos: [f32; 2], w: f32, h: f32) -> [f32; 2] {
            [
                (pos[0] / w) * 2.0 - 1.0,
                1.0 - (pos[1] / h) * 2.0,
            ]
        }

        fn pack_rgba8(c: [f32; 4]) -> u32 {
            let r = (c[0] * 255.0).round() as u32;
            let g = (c[1] * 255.0).round() as u32;
            let b = (c[2] * 255.0).round() as u32;
            let a = (c[3] * 255.0).round() as u32;
            (a << 24) | (r << 16) | (g << 8) | b
        }

        for cmd in &draw_list.0 {
            if let DrawCommand::Triangle { p1, p2, p3, color, scroll, depth } = *cmd {
                let c_u32 = pack_rgba8(color);

                // DPI対応: 座標をスケールファクターで調整
                let scaled_p1 = [p1[0] * scale_factor, p1[1] * scale_factor];
                let scaled_p2 = [p2[0] * scale_factor, p2[1] * scale_factor];
                let scaled_p3 = [p3[0] * scale_factor, p3[1] * scale_factor];
                let scaled_scroll_offset = [scroll_offset[0] * scale_factor, scroll_offset[1] * scale_factor];

                // スクロールオフセットの適用可否
                let (p1, p2, p3) = if scroll {
                    (
                        [scaled_p1[0] + scaled_scroll_offset[0], scaled_p1[1] + scaled_scroll_offset[1]],
                        [scaled_p2[0] + scaled_scroll_offset[0], scaled_p2[1] + scaled_scroll_offset[1]],
                        [scaled_p3[0] + scaled_scroll_offset[0], scaled_p3[1] + scaled_scroll_offset[1]],
                    )
                } else {
                    (scaled_p1, scaled_p2, scaled_p3)
                };

                // ★ DrawCommandのdepth値を直接使用
                vertices.extend_from_slice(&[
                    TriangleVertex { position: [to_ndc(p1, w, h)[0], to_ndc(p1, w, h)[1], depth], color: c_u32 },
                    TriangleVertex { position: [to_ndc(p2, w, h)[0], to_ndc(p2, w, h)[1], depth], color: c_u32 },
                    TriangleVertex { position: [to_ndc(p3, w, h)[0], to_ndc(p3, w, h)[1], depth], color: c_u32 },
                ]);
            }
        }

        if !vertices.is_empty() {
            self.vertex_count = vertices.len() as u32;
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..self.vertex_count, 0..1);
        }
    }

    // 既存のdrawメソッドも互換性のために残す
    pub fn draw<'a>(
        &'a mut self,
        pass: &mut RenderPass<'a>,
        draw_list: &DrawList,
        queue: &Queue,
        screen_size: PhysicalSize<u32>,
        scroll_offset: [f32; 2],
        scale_factor: f32,
    ) {
        let mut vertices: Vec<TriangleVertex> = Vec::new();

        let w = screen_size.width as f32;
        let h = screen_size.height as f32;

        fn to_ndc(pos: [f32; 2], w: f32, h: f32) -> [f32; 2] {
            [
                (pos[0] / w) * 2.0 - 1.0,
                1.0 - (pos[1] / h) * 2.0,
            ]
        }

        fn pack_rgba8(c: [f32; 4]) -> u32 {
            let r = (c[0] * 255.0).round() as u32;
            let g = (c[1] * 255.0).round() as u32;
            let b = (c[2] * 255.0).round() as u32;
            let a = (c[3] * 255.0).round() as u32;
            (a << 24) | (r << 16) | (g << 8) | b
        }

        for cmd in &draw_list.0 {
            if let DrawCommand::Triangle { p1, p2, p3, color, scroll, depth } = *cmd {
                let c_u32 = pack_rgba8(color);

                // DPI対応: 座標をスケールファクターで調整
                let scaled_p1 = [p1[0] * scale_factor, p1[1] * scale_factor];
                let scaled_p2 = [p2[0] * scale_factor, p2[1] * scale_factor];
                let scaled_p3 = [p3[0] * scale_factor, p3[1] * scale_factor];
                let scaled_scroll_offset = [scroll_offset[0] * scale_factor, scroll_offset[1] * scale_factor];

                // スクロールオフセットの適用可否
                let (p1, p2, p3) = if scroll {
                    (
                        [scaled_p1[0] + scaled_scroll_offset[0], scaled_p1[1] + scaled_scroll_offset[1]],
                        [scaled_p2[0] + scaled_scroll_offset[0], scaled_p2[1] + scaled_scroll_offset[1]],
                        [scaled_p3[0] + scaled_scroll_offset[0], scaled_p3[1] + scaled_scroll_offset[1]],
                    )
                } else {
                    (scaled_p1, scaled_p2, scaled_p3)
                };

                // ★ DrawCommandのdepth値を直接使用
                vertices.extend_from_slice(&[
                    TriangleVertex { position: [to_ndc(p1, w, h)[0], to_ndc(p1, w, h)[1], depth], color: c_u32 },
                    TriangleVertex { position: [to_ndc(p2, w, h)[0], to_ndc(p2, w, h)[1], depth], color: c_u32 },
                    TriangleVertex { position: [to_ndc(p3, w, h)[0], to_ndc(p3, w, h)[1], depth], color: c_u32 },
                ]);
            }
        }

        if !vertices.is_empty() {
            self.vertex_count = vertices.len() as u32;
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..self.vertex_count, 0..1);
        }
    }
}
