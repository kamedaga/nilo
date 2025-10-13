// 統合レンダラ - 一つのパイプラインですべての図形を描画

use crate::renderer_abstract::command::{DrawCommand, DrawList};
use wgpu::{
    Buffer, BufferUsages, Device, Queue, RenderPass, RenderPipeline, VertexAttribute,
    VertexBufferLayout, VertexFormat, VertexStepMode,
};
use winit::dpi::PhysicalSize;

// 統合頂点構造体
// Rustの構造体はアライメントルールに従うため、明示的にパディングを追加
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct UnifiedVertex {
    position: [f32; 3], // offset: 0, size: 12
    shape_type: u32,    // offset: 12, size: 4
    color: u32,         // offset: 16, size: 4
    _padding: f32,      // offset: 20, size: 4 (パディング)
    center: [f32; 2],   // offset: 24, size: 8
    radius: f32,        // offset: 32, size: 4
    _padding2: f32,     // offset: 36, size: 4 (パディング)
    color_vec: [f32; 4], // offset: 40, size: 16
                        // 合計: 56 bytes
}

impl UnifiedVertex {
    fn desc<'a>() -> VertexBufferLayout<'a> {
        use std::mem;
        VertexBufferLayout {
            array_stride: mem::size_of::<UnifiedVertex>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                // @location(0) position: vec3<f32>
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x3,
                },
                // @location(1) shape_type: u32
                VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: VertexFormat::Uint32,
                },
                // @location(2) color: u32
                VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: VertexFormat::Uint32,
                },
                // @location(4) center: vec2<f32> (location 3はパディング用にスキップ)
                VertexAttribute {
                    offset: 24,
                    shader_location: 4,
                    format: VertexFormat::Float32x2,
                },
                // @location(5) radius: f32
                VertexAttribute {
                    offset: 32,
                    shader_location: 5,
                    format: VertexFormat::Float32,
                },
                // @location(7) color_vec: vec4<f32> (location 6はパディング用にスキップ)
                VertexAttribute {
                    offset: 40,
                    shader_location: 7,
                    format: VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ScreenUniform {
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

pub struct UnifiedRenderer {
    pipeline: RenderPipeline,
    vertex_buffer: Buffer,
    uniform_buffer: Buffer,
    bind_group: wgpu::BindGroup,
}

impl UnifiedRenderer {
    pub fn new(device: &Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/unified.wgsl"));

        // Uniform Buffer の設定
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Unified Uniform Buffer"),
            size: std::mem::size_of::<ScreenUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Unified Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Unified Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Unified Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Unified Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[UnifiedVertex::desc()],
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
            label: Some("Unified Vertex Buffer"),
            size: 2 * 1024 * 1024, // 2MB
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            uniform_buffer,
            bind_group,
        }
    }

    pub fn draw(
        &mut self,
        rpass: &mut RenderPass,
        draw_list: &DrawList,
        queue: &Queue,
        size: PhysicalSize<u32>,
        scroll_offset: [f32; 2],
        scale_factor: f32,
    ) {
        // スクリーンサイズをUniformに設定
        let uniform = ScreenUniform {
            screen_size: [size.width as f32, size.height as f32],
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));

        // 全コマンドを統合頂点バッファに変換
        let mut vertices = Vec::new();

        for cmd in &draw_list.0 {
            match cmd {
                DrawCommand::Rect {
                    position,
                    width,
                    height,
                    color,
                    depth,
                    scroll,
                    ..
                } => {
                    self.add_rect_vertices(
                        &mut vertices,
                        position[0],
                        position[1],
                        *width,
                        *height,
                        *color,
                        *depth,
                        *scroll,
                        scroll_offset,
                        size,
                        scale_factor,
                    );
                }
                DrawCommand::Triangle {
                    p1,
                    p2,
                    p3,
                    color,
                    depth,
                    scroll,
                    ..
                } => {
                    self.add_triangle_vertices(
                        &mut vertices,
                        p1[0],
                        p1[1],
                        p2[0],
                        p2[1],
                        p3[0],
                        p3[1],
                        *color,
                        *depth,
                        *scroll,
                        scroll_offset,
                        size,
                        scale_factor,
                    );
                }
                DrawCommand::Circle {
                    center,
                    radius,
                    color,
                    depth,
                    scroll,
                    ..
                } => {
                    self.add_circle_vertices(
                        &mut vertices,
                        center[0],
                        center[1],
                        *radius,
                        *color,
                        *depth,
                        *scroll,
                        scroll_offset,
                        size,
                        scale_factor,
                    );
                }
                _ => {} // Text/Imageは別レンダラで処理
            }
        }

        if vertices.is_empty() {
            return;
        }

        // 頂点バッファに書き込み
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        // 描画
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.draw(0..vertices.len() as u32, 0..1);
    }

    fn add_rect_vertices(
        &self,
        vertices: &mut Vec<UnifiedVertex>,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        depth: f32,
        scroll: bool,
        scroll_offset: [f32; 2],
        size: PhysicalSize<u32>,
        scale_factor: f32,
    ) {
        let packed_color = pack_rgba8(color);

        let scaled_scroll = if scroll {
            [
                scroll_offset[0] * scale_factor,
                scroll_offset[1] * scale_factor,
            ]
        } else {
            [0.0, 0.0]
        };

        let x = (x * scale_factor) + scaled_scroll[0];
        let y = (y * scale_factor) + scaled_scroll[1];
        let w = width * scale_factor;
        let h = height * scale_factor;

        let (x1, y1) = screen_to_ndc(x, y, size);
        let (x2, y2) = screen_to_ndc(x + w, y + h, size);

        // 2つの三角形で矩形を構成
        let verts = [
            [x1, y1, depth],
            [x2, y1, depth],
            [x1, y2, depth],
            [x2, y1, depth],
            [x2, y2, depth],
            [x1, y2, depth],
        ];

        for pos in verts {
            vertices.push(UnifiedVertex {
                position: pos,
                shape_type: 0, // Quad
                color: packed_color,
                _padding: 0.0,
                center: [0.0, 0.0],
                radius: 0.0,
                _padding2: 0.0,
                color_vec: [0.0, 0.0, 0.0, 0.0],
            });
        }
    }

    fn add_triangle_vertices(
        &self,
        vertices: &mut Vec<UnifiedVertex>,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
        color: [f32; 4],
        depth: f32,
        scroll: bool,
        scroll_offset: [f32; 2],
        size: PhysicalSize<u32>,
        scale_factor: f32,
    ) {
        let packed_color = pack_rgba8(color);

        let scaled_scroll = if scroll {
            [
                scroll_offset[0] * scale_factor,
                scroll_offset[1] * scale_factor,
            ]
        } else {
            [0.0, 0.0]
        };

        let x1 = (x1 * scale_factor) + scaled_scroll[0];
        let y1 = (y1 * scale_factor) + scaled_scroll[1];
        let x2 = (x2 * scale_factor) + scaled_scroll[0];
        let y2 = (y2 * scale_factor) + scaled_scroll[1];
        let x3 = (x3 * scale_factor) + scaled_scroll[0];
        let y3 = (y3 * scale_factor) + scaled_scroll[1];

        let (nx1, ny1) = screen_to_ndc(x1, y1, size);
        let (nx2, ny2) = screen_to_ndc(x2, y2, size);
        let (nx3, ny3) = screen_to_ndc(x3, y3, size);

        let verts = [[nx1, ny1, depth], [nx2, ny2, depth], [nx3, ny3, depth]];

        for pos in verts {
            vertices.push(UnifiedVertex {
                position: pos,
                shape_type: 1, // Triangle
                color: packed_color,
                _padding: 0.0,
                center: [0.0, 0.0],
                radius: 0.0,
                _padding2: 0.0,
                color_vec: [0.0, 0.0, 0.0, 0.0],
            });
        }
    }

    fn add_circle_vertices(
        &self,
        vertices: &mut Vec<UnifiedVertex>,
        x: f32,
        y: f32,
        radius: f32,
        color: [f32; 4],
        depth: f32,
        scroll: bool,
        scroll_offset: [f32; 2],
        size: PhysicalSize<u32>,
        scale_factor: f32,
    ) {
        let scaled_scroll = if scroll {
            [
                scroll_offset[0] * scale_factor,
                scroll_offset[1] * scale_factor,
            ]
        } else {
            [0.0, 0.0]
        };

        let cx = (x * scale_factor) + scaled_scroll[0];
        let cy = (y * scale_factor) + scaled_scroll[1];
        let r = radius * scale_factor;

        let center_px = [cx, cy];

        // 四角形の範囲
        let x0 = cx - r;
        let x1 = cx + r;
        let y0 = cy - r;
        let y1 = cy + r;

        let (nx0, ny0) = screen_to_ndc(x0, y0, size);
        let (nx1, ny1) = screen_to_ndc(x1, y1, size);

        // 元のCircleRendererと同じく、深度オフセットは使わない
        // DrawCommandのdepth値を直接使用

        // 2つの三角形で矩形を構成
        let positions = [
            [nx0, ny0, depth],
            [nx1, ny0, depth],
            [nx0, ny1, depth],
            [nx1, ny0, depth],
            [nx1, ny1, depth],
            [nx0, ny1, depth],
        ];

        for pos in positions {
            let v = UnifiedVertex {
                position: pos,
                shape_type: 2, // Circle
                color: 0,      // 使用しない
                _padding: 0.0,
                center: center_px,
                radius: r,
                _padding2: 0.0,
                color_vec: color,
            };

            vertices.push(v);
        }
    }
}

// ヘルパー関数
fn pack_rgba8(c: [f32; 4]) -> u32 {
    let r = (c[0] * 255.0).round() as u32;
    let g = (c[1] * 255.0).round() as u32;
    let b = (c[2] * 255.0).round() as u32;
    let a = (c[3] * 255.0).round() as u32;
    (a << 24) | (r << 16) | (g << 8) | b
}

fn screen_to_ndc(x: f32, y: f32, size: PhysicalSize<u32>) -> (f32, f32) {
    let nx = (x / size.width as f32) * 2.0 - 1.0;
    let ny = 1.0 - (y / size.height as f32) * 2.0;
    (nx, ny)
}
