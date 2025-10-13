use crate::renderer_abstract::command::{DrawCommand, DrawList};
use log::error;
use std::collections::HashMap;
use std::path::Path;
use wgpu::util::DeviceExt;
use wgpu::*;
use winit::dpi::PhysicalSize; // ログマクロを追加

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct ImageVertex {
    position: [f32; 3], // ★ Z座標を追加
    uv: [f32; 2],
}

impl ImageVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ImageVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                }, // ★ Float32x3に変更
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                }, // ★ オフセット調整
            ],
        }
    }
}

pub struct TextureResource {
    #[allow(dead_code)]
    pub texture: Texture,
    #[allow(dead_code)]
    pub view: TextureView,
    #[allow(dead_code)]
    pub sampler: Sampler,
    pub bind_group: BindGroup,
}

pub struct ImageRenderer {
    pipeline: RenderPipeline,
    bind_group_layout: BindGroupLayout,
    textures: HashMap<String, TextureResource>, // パス→テクスチャ
}

impl ImageRenderer {
    pub fn new(device: &Device, format: TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/image.wgsl"));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Image Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Image Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Image Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[ImageVertex::desc()],
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

        Self {
            pipeline,
            bind_group_layout,
            textures: HashMap::new(),
        }
    }

    pub fn load_texture(&mut self, device: &Device, queue: &Queue, path: &str) {
        if self.textures.contains_key(path) {
            return;
        }

        // ファイル拡張子をチェックしてSVGかどうか判定
        let path_obj = Path::new(path);
        let extension = path_obj
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        let (width, height, raw) = if extension == "svg" {
            // SVGファイルの処理
            self.load_svg_as_rgba(path).unwrap_or_else(|e| {
                error!("Failed to load SVG {}: {}", path, e); // eprintln!をerror!に変更
                // フォールバック: 1x1の透明画像
                (1, 1, vec![0, 0, 0, 0])
            })
        } else {
            // 通常の画像ファイルの処理
            match image::open(path) {
                Ok(img) => {
                    let rgba_img = img.to_rgba8();
                    let (w, h) = rgba_img.dimensions();
                    (w, h, rgba_img.into_raw())
                }
                Err(e) => {
                    error!("Failed to load image {}: {}", path, e); // eprintln!をerror!に変更
                    // フォールバック: 1x1の透明画像
                    (1, 1, vec![0, 0, 0, 0])
                }
            }
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Image Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &raw,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Image BindGroup"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        self.textures.insert(
            path.to_owned(),
            TextureResource {
                texture,
                view,
                sampler,
                bind_group,
            },
        );
    }

    // SVGファイルをRGBAバイト配列に変換するヘルパーメソッド
    fn load_svg_as_rgba(
        &self,
        path: &str,
    ) -> Result<(u32, u32, Vec<u8>), Box<dyn std::error::Error>> {
        let svg_data = std::fs::read(path)?;

        let rtree = resvg::usvg::Tree::from_data(&svg_data, &resvg::usvg::Options::default())?;
        let size = rtree.size();

        // SVGのサイズを取得、デフォルトサイズが小さすぎる場合は調整
        let width = if size.width() > 0.0 {
            size.width() as u32
        } else {
            256
        };
        let height = if size.height() > 0.0 {
            size.height() as u32
        } else {
            256
        };

        let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or("Failed to create pixmap")?;

        resvg::render(
            &rtree,
            tiny_skia::Transform::default(),
            &mut pixmap.as_mut(),
        );

        // tiny-skiaのピクセルデータ（RGBA）を取得
        let pixels = pixmap.take();

        Ok((width, height, pixels))
    }

    #[allow(dead_code)]
    // ★ Z値を指定できる描画メソッド
    pub fn draw_with_depth<'a>(
        &'a self,
        device: &Device,
        pass: &mut wgpu::RenderPass<'a>,
        draw_list: &DrawList,
        _queue: &Queue,
        screen_size: PhysicalSize<u32>,
        scroll_offset: [f32; 2],
        scale_factor: f32,
        depth: f32, // ★ Z値（0.0=最前面、1.0=最背面）
    ) {
        let w = screen_size.width as f32;
        let h = screen_size.height as f32;

        fn to_ndc(pos: [f32; 2], w: f32, h: f32) -> [f32; 2] {
            [(pos[0] / w) * 2.0 - 1.0, 1.0 - (pos[1] / h) * 2.0]
        }

        for cmd in &draw_list.0 {
            if let DrawCommand::Image {
                position,
                width,
                height,
                path,
                scroll,
                ..
            } = cmd
            {
                let tex = match self.textures.get(path) {
                    Some(t) => t,
                    None => continue,
                };

                let scaled_position = [position[0] * scale_factor, position[1] * scale_factor];
                let scaled_width = width * scale_factor;
                let scaled_height = height * scale_factor;
                let scaled_scroll_offset = [
                    scroll_offset[0] * scale_factor,
                    scroll_offset[1] * scale_factor,
                ];

                let (x0, y0, x1, y1) = if *scroll {
                    (
                        scaled_position[0] + scaled_scroll_offset[0],
                        scaled_position[1] + scaled_scroll_offset[1],
                        scaled_position[0] + scaled_width + scaled_scroll_offset[0],
                        scaled_position[1] + scaled_height + scaled_scroll_offset[1],
                    )
                } else {
                    (
                        scaled_position[0],
                        scaled_position[1],
                        scaled_position[0] + scaled_width,
                        scaled_position[1] + scaled_height,
                    )
                };

                let vertices = [
                    ImageVertex {
                        position: [to_ndc([x0, y0], w, h)[0], to_ndc([x0, y0], w, h)[1], depth],
                        uv: [0.0, 0.0],
                    },
                    ImageVertex {
                        position: [to_ndc([x1, y0], w, h)[0], to_ndc([x1, y0], w, h)[1], depth],
                        uv: [1.0, 0.0],
                    },
                    ImageVertex {
                        position: [to_ndc([x0, y1], w, h)[0], to_ndc([x0, y1], w, h)[1], depth],
                        uv: [0.0, 1.0],
                    },
                    ImageVertex {
                        position: [to_ndc([x0, y1], w, h)[0], to_ndc([x0, y1], w, h)[1], depth],
                        uv: [0.0, 1.0],
                    },
                    ImageVertex {
                        position: [to_ndc([x1, y0], w, h)[0], to_ndc([x1, y0], w, h)[1], depth],
                        uv: [1.0, 0.0],
                    },
                    ImageVertex {
                        position: [to_ndc([x1, y1], w, h)[0], to_ndc([x1, y1], w, h)[1], depth],
                        uv: [1.0, 1.0],
                    },
                ];

                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Image Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &tex.bind_group, &[]);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.draw(0..6, 0..1);
            }
        }
    }

    // 既存のdrawメソッドも互換性のために残す
    pub fn draw<'a>(
        &'a self,
        device: &Device,
        pass: &mut wgpu::RenderPass<'a>,
        draw_list: &DrawList,
        _queue: &Queue,
        screen_size: PhysicalSize<u32>,
        scroll_offset: [f32; 2],
        scale_factor: f32,
    ) {
        let w = screen_size.width as f32;
        let h = screen_size.height as f32;

        fn to_ndc(pos: [f32; 2], w: f32, h: f32) -> [f32; 2] {
            [(pos[0] / w) * 2.0 - 1.0, 1.0 - (pos[1] / h) * 2.0]
        }

        for cmd in &draw_list.0 {
            if let DrawCommand::Image {
                position,
                width,
                height,
                path,
                scroll,
                depth,
            } = cmd
            {
                let tex = match self.textures.get(path) {
                    Some(t) => t,
                    None => continue,
                };

                let scaled_position = [position[0] * scale_factor, position[1] * scale_factor];
                let scaled_width = width * scale_factor;
                let scaled_height = height * scale_factor;
                let scaled_scroll_offset = [
                    scroll_offset[0] * scale_factor,
                    scroll_offset[1] * scale_factor,
                ];

                let (x0, y0, x1, y1) = if *scroll {
                    (
                        scaled_position[0] + scaled_scroll_offset[0],
                        scaled_position[1] + scaled_scroll_offset[1],
                        scaled_position[0] + scaled_width + scaled_scroll_offset[0],
                        scaled_position[1] + scaled_height + scaled_scroll_offset[1],
                    )
                } else {
                    (
                        scaled_position[0],
                        scaled_position[1],
                        scaled_position[0] + scaled_width,
                        scaled_position[1] + scaled_height,
                    )
                };

                // ★ DrawCommandのdepth値を直接使用
                let vertices = [
                    ImageVertex {
                        position: [to_ndc([x0, y0], w, h)[0], to_ndc([x0, y0], w, h)[1], *depth],
                        uv: [0.0, 0.0],
                    },
                    ImageVertex {
                        position: [to_ndc([x1, y0], w, h)[0], to_ndc([x1, y0], w, h)[1], *depth],
                        uv: [1.0, 0.0],
                    },
                    ImageVertex {
                        position: [to_ndc([x0, y1], w, h)[0], to_ndc([x0, y1], w, h)[1], *depth],
                        uv: [0.0, 1.0],
                    },
                    ImageVertex {
                        position: [to_ndc([x0, y1], w, h)[0], to_ndc([x0, y1], w, h)[1], *depth],
                        uv: [0.0, 1.0],
                    },
                    ImageVertex {
                        position: [to_ndc([x1, y0], w, h)[0], to_ndc([x1, y0], w, h)[1], *depth],
                        uv: [1.0, 0.0],
                    },
                    ImageVertex {
                        position: [to_ndc([x1, y1], w, h)[0], to_ndc([x1, y1], w, h)[1], *depth],
                        uv: [1.0, 1.0],
                    },
                ];

                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Image Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &tex.bind_group, &[]);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.draw(0..6, 0..1);
            }
        }
    }
}
