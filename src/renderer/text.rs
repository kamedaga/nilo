use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer as GlyphonTextRenderer, Viewport, Weight,
};
use wgpu::{
    Device, Queue, RenderPass, TextureFormat, MultisampleState, DepthStencilState,
};

pub struct TextRenderer {
    renderer: GlyphonTextRenderer,
    atlas: TextAtlas,
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    cache: Cache,
}

impl TextRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        format: TextureFormat,
        multisample: MultisampleState,
        depth_stencil: Option<DepthStencilState>,
        width: u32,
        height: u32,
    ) -> Self {
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let renderer = GlyphonTextRenderer::new(
            &mut atlas,
            device,
            multisample,
            depth_stencil,
        );
        let viewport = Viewport::new(device, &cache);

        Self {
            renderer,
            atlas,
            font_system,
            swash_cache,
            viewport,
            cache,
        }
    }

    pub fn resize(&mut self, device: &Device, queue: &Queue, width: u32, height: u32) {
        self.viewport.update(
            queue,
            Resolution {
                width,
                height,
            },
        );
    }

    pub fn render_multiple_texts(
        &mut self,
        pass: &mut RenderPass,
        text_commands: &[(String, [f32; 2], f32, [f32; 4])],
        scroll_offset: [f32; 2],
        scale_factor: f32, // ← 追加されたスケ���ル係数
        queue: &Queue,
        device: &Device,
        screen_width: u32,
        screen_height: u32,
    ) {
        let mut buffers = Vec::new();
        let mut text_areas = Vec::new();

        for (content, position, size, _color) in text_commands {
            let scaled_size = *size * scale_factor;
            let metrics = Metrics::new(scaled_size, scaled_size * 1.4);
            let mut buffer = Buffer::new(&mut self.font_system, metrics);

            buffer.set_size(
                &mut self.font_system,
                Some(screen_width as f32),
                Some(screen_height as f32),
            );

            buffer.set_text(
                &mut self.font_system,
                content,
                &Attrs::new().family(Family::SansSerif).weight(Weight::NORMAL),
                Shaping::Advanced,
            );

            buffer.shape_until_scroll(&mut self.font_system, false);
            buffers.push((buffer, metrics));
        }

        for (i, (_, position, _, color)) in text_commands.iter().enumerate() {
            let (buffer, metrics) = &buffers[i];

            // DPI対応: スクロールオフセットと位置にスケーリングを適用
            let scaled_pos = [
                (position[0] + scroll_offset[0]) * scale_factor,
                (position[1] + scroll_offset[1]) * scale_factor,
            ];
            
            // ★ 修正: テキストを垂直中央に配置するため、ベースラインを調整
            let adjusted_top = scaled_pos[1] - (metrics.line_height - metrics.font_size) * 0.5;

            let text_area = TextArea {
                buffer,
                left: scaled_pos[0],
                top: adjusted_top,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: screen_width as i32,
                    bottom: screen_height as i32,
                },
                default_color: Color::rgb(
                    (color[0] * 255.0) as u8,
                    (color[1] * 255.0) as u8,
                    (color[2] * 255.0) as u8,
                ),
                custom_glyphs: &[],
            };


            text_areas.push(text_area);
        }

        if !text_areas.is_empty() {
            self.renderer
                .prepare(
                    device,
                    queue,
                    &mut self.font_system,
                    &mut self.atlas,
                    &self.viewport,
                    text_areas.iter().cloned(),
                    &mut self.swash_cache,
                )
                .expect("Failed to prepare text renderer");

            self.renderer
                .render(&self.atlas, &self.viewport, pass)
                .expect("Failed to render text");
        }
    }


    pub fn render_text(
        &mut self,
        pass: &mut RenderPass,
        content: &str,
        position: [f32; 2],
        size: f32,
        color: [f32; 4],
        scroll_offset: [f32; 2],
        queue: &Queue,
        device: &Device,
        screen_width: u32,
        screen_height: u32,
    ) {
        let metrics = Metrics::new(size, size * 1.4);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        buffer.set_size(
            &mut self.font_system,
            Some(screen_width as f32),
            Some(screen_height as f32),
        );

        buffer.set_text(
            &mut self.font_system,
            content,
            &Attrs::new().family(Family::SansSerif).weight(Weight::NORMAL),
            Shaping::Advanced,
        );

        buffer.shape_until_scroll(&mut self.font_system, false);

        // ★ 修正: テキストを垂直中央に配置するため、ベースラインを調整
        let adjusted_top = position[1] + scroll_offset[1] - (metrics.line_height - metrics.font_size) * 0.5;

        let text_area = TextArea {
            buffer: &buffer,
            left: position[0] + scroll_offset[0],
            top: adjusted_top,
            scale: 1.0,
            bounds: TextBounds {
                left: 0,
                top: 0,
                right: screen_width as i32,
                bottom: screen_height as i32,
            },
            default_color: Color::rgb(
                color[0] as u8,
                color[1] as u8,
                color[2] as u8,
            ),
            custom_glyphs: &[],
        };

        let text_areas = [text_area];

        

        self.renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                text_areas.iter().cloned(),
                &mut self.swash_cache,
            )
            .expect("Failed to prepare text renderer");

        self.renderer
            .render(&self.atlas, &self.viewport, pass)
            .expect("Failed to render text");
    }

    pub fn draw<'a>(
        &'a mut self,
        pass: &mut RenderPass<'a>,
        text_areas: &[TextArea<'a>],
        queue: &Queue,
        device: &Device,
    ) {
        self.renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                text_areas.iter().cloned(),
                &mut self.swash_cache,
            )
            .expect("Failed to prepare text renderer");

        self.renderer
            .render(&self.atlas, &self.viewport, pass)
            .expect("Failed to render text");
    }

    pub fn create_buffer(&mut self, text: &str, metrics: Metrics) -> Buffer {
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_text(
            &mut self.font_system,
            text,
            &Attrs::new().family(Family::SansSerif).weight(Weight::NORMAL),
            Shaping::Advanced,
        );
        buffer
    }

    pub fn create_text_area<'a>(
        &self,
        buffer: &'a Buffer,
        left: f32,
        top: f32,
        scale: f32,
        bounds: TextBounds,
        default_color: Color,
    ) -> TextArea<'a> {
        TextArea {
            buffer,
            left,
            top,
            scale,
            bounds,
            default_color,
            custom_glyphs: &[],
        }
    }

    pub fn font_system(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }

    pub fn atlas(&mut self) -> &mut TextAtlas {
        &mut self.atlas
    }

    pub fn cache(&mut self) -> &mut Cache {
        &mut self.cache
    }

    pub fn viewport(&mut self) -> &mut Viewport {
        &mut self.viewport
    }

    // ★ Z値を指定できるテキスト描画メソッド
    pub fn render_multiple_texts_with_depth(
        &mut self,
        pass: &mut RenderPass,
        text_commands: &[(String, [f32; 2], f32, [f32; 4])],
        scroll_offset: [f32; 2],
        scale_factor: f32,
        queue: &Queue,
        device: &Device,
        screen_width: u32,
        screen_height: u32,
        depth: f32, // ★ Z値（0.0=最前面、1.0=最背面）
    ) {
        // glyphonは内部的にdepth testingを���理するため、
        // 現在の実装では直接Z値を制御できないが、
        // 描画順序でZ値の効果を模擬できる
        self.render_multiple_texts(
            pass,
            text_commands,
            scroll_offset,
            scale_factor,
            queue,
            device,
            screen_width,
            screen_height,
        );
    }
}
