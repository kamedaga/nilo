// CircleRenderer の修正版（DPI対応）

use crate::renderer::command::{DrawCommand, DrawList};
use winit::dpi::PhysicalSize;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ScreenUniform {
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct CircleVertex {
    position: [f32; 3], // ★ Z座標を追加
    center: [f32; 2],
    radius: f32,
    color: [f32; 4],
}

impl CircleVertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<CircleVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3, // ★ Float32x3に変更
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress, // ★ オフセット調整
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() + mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress, // ★ オフセット調整
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() + mem::size_of::<[f32; 2]>() + mem::size_of::<f32>()) as wgpu::BufferAddress, // ★ オフセット調整
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub struct CircleRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_count: u32,
}

impl CircleRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/circle.wgsl"));

        // Uniform Buffer の設定
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Circle Uniform Buffer"),
            size: std::mem::size_of::<ScreenUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Circle Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Circle Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Circle Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Circle Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[CircleVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
            label: Some("Circle Vertex Buffer"),
            size: 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            uniform_buffer,
            bind_group,
            vertex_count: 0,
        }
    }

    // ★ Z値を指定できる描画メソッド
    #[allow(dead_code)]
    pub fn draw_with_depth<'a>(
        &'a mut self,
        pass: &mut wgpu::RenderPass<'a>,
        draw_list: &DrawList,
        queue: &wgpu::Queue,
        screen_size: PhysicalSize<u32>,
        scroll_offset: [f32; 2],
        scale_factor: f32,
        _depth: f32, // ★ Z値（0.0=最前面、1.0=最背面）
    ) {
        let mut vertices: Vec<CircleVertex> = Vec::new();
        let w = screen_size.width as f32;
        let h = screen_size.height as f32;

        let screen_uniform = ScreenUniform {
            screen_size: [w, h],
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[screen_uniform]));

        fn to_ndc(pos: [f32; 2], w: f32, h: f32) -> [f32; 2] {
            [
                (pos[0] / w) * 2.0 - 1.0,
                1.0 - (pos[1] / h) * 2.0,
            ]
        }

        for cmd in &draw_list.0 {
            if let DrawCommand::Circle { center, radius, color, segments: _, scroll, depth } = cmd {
                // DPI対応: 座標とサイズをスケールファクターで調整
                let scaled_center = [center[0] * scale_factor, center[1] * scale_factor];
                let scaled_radius = radius * scale_factor;
                let scaled_scroll_offset = [scroll_offset[0] * scale_factor, scroll_offset[1] * scale_factor];

                let cx = scaled_center[0] + if *scroll { scaled_scroll_offset[0] } else { 0.0 };
                let cy = scaled_center[1] + if *scroll { scaled_scroll_offset[1] } else { 0.0 };
                let r = scaled_radius;

                // 四角形のピクセル座標
                let x0 = cx - r;
                let x1 = cx + r;
                let y0 = cy - r;
                let y1 = cy + r;

                // ★ DrawCommandのdepth値を直接使用
                vertices.extend_from_slice(&[
                    CircleVertex { position: [to_ndc([x0, y0], w, h)[0], to_ndc([x0, y0], w, h)[1], *depth], center: [cx, cy], radius: r, color: *color },
                    CircleVertex { position: [to_ndc([x1, y0], w, h)[0], to_ndc([x1, y0], w, h)[1], *depth], center: [cx, cy], radius: r, color: *color },
                    CircleVertex { position: [to_ndc([x0, y1], w, h)[0], to_ndc([x0, y1], w, h)[1], *depth], center: [cx, cy], radius: r, color: *color },
                    CircleVertex { position: [to_ndc([x0, y1], w, h)[0], to_ndc([x0, y1], w, h)[1], *depth], center: [cx, cy], radius: r, color: *color },
                    CircleVertex { position: [to_ndc([x1, y0], w, h)[0], to_ndc([x1, y0], w, h)[1], *depth], center: [cx, cy], radius: r, color: *color },
                    CircleVertex { position: [to_ndc([x1, y1], w, h)[0], to_ndc([x1, y1], w, h)[1], *depth], center: [cx, cy], radius: r, color: *color },
                ]);
            }
        }

        if !vertices.is_empty() {
            self.vertex_count = vertices.len() as u32;
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..self.vertex_count, 0..1);
        }
    }

    // 既存のdrawメソッドも互換性のために残す
    pub fn draw<'a>(
        &'a mut self,
        pass: &mut wgpu::RenderPass<'a>,
        draw_list: &DrawList,
        queue: &wgpu::Queue,
        screen_size: PhysicalSize<u32>,
        scroll_offset: [f32; 2],
        scale_factor: f32,
        _depth: f32, // ★ Z値（0.0=最前面、1.0=最背面）
    ) {
        let mut vertices: Vec<CircleVertex> = Vec::new();
        let w = screen_size.width as f32;
        let h = screen_size.height as f32;

        // Screen uniform を更新
        let screen_uniform = ScreenUniform {
            screen_size: [w, h],
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[screen_uniform]));

        fn to_ndc(pos: [f32; 2], w: f32, h: f32) -> [f32; 2] {
            [
                (pos[0] / w) * 2.0 - 1.0,
                1.0 - (pos[1] / h) * 2.0,
            ]
        }

        for cmd in &draw_list.0 {
            if let DrawCommand::Circle { center, radius, color, segments: _, scroll, depth } = cmd {
                // DPI対応: 座標とサイズをスケールファクターで調整
                let scaled_center = [center[0] * scale_factor, center[1] * scale_factor];
                let scaled_radius = radius * scale_factor;
                let scaled_scroll_offset = [scroll_offset[0] * scale_factor, scroll_offset[1] * scale_factor];

                let cx = scaled_center[0] + if *scroll { scaled_scroll_offset[0] } else { 0.0 };
                let cy = scaled_center[1] + if *scroll { scaled_scroll_offset[1] } else { 0.0 };
                let r = scaled_radius;

                // 四角形のピクセル座標
                let x0 = cx - r;
                let x1 = cx + r;
                let y0 = cy - r;
                let y1 = cy + r;

                // ★ DrawCommandのdepth値を直接使用
                vertices.extend_from_slice(&[
                    CircleVertex { position: [to_ndc([x0, y0], w, h)[0], to_ndc([x0, y0], w, h)[1], *depth], center: [cx, cy], radius: r, color: *color },
                    CircleVertex { position: [to_ndc([x1, y0], w, h)[0], to_ndc([x1, y0], w, h)[1], *depth], center: [cx, cy], radius: r, color: *color },
                    CircleVertex { position: [to_ndc([x0, y1], w, h)[0], to_ndc([x0, y1], w, h)[1], *depth], center: [cx, cy], radius: r, color: *color },
                    CircleVertex { position: [to_ndc([x0, y1], w, h)[0], to_ndc([x0, y1], w, h)[1], *depth], center: [cx, cy], radius: r, color: *color },
                    CircleVertex { position: [to_ndc([x1, y0], w, h)[0], to_ndc([x1, y0], w, h)[1], *depth], center: [cx, cy], radius: r, color: *color },
                    CircleVertex { position: [to_ndc([x1, y1], w, h)[0], to_ndc([x1, y1], w, h)[1], *depth], center: [cx, cy], radius: r, color: *color },
                ]);
            }
        }

        if !vertices.is_empty() {
            self.vertex_count = vertices.len() as u32;
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..self.vertex_count, 0..1);
        }
    }
}