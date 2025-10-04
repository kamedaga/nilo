use glyphon::{
    Attrs, Buffer, FontSystem, Metrics, Shaping, Family, Weight,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// テキスト測定結果
#[derive(Debug, Clone)]
pub struct TextMeasurement {
    /// 実際のテキスト幅（ピクセル）
    pub width: f32,
    /// 実際のテキスト高さ（ピクセル）
    pub height: f32,
    /// 行数
    pub line_count: usize,
    /// 各行の幅
    pub line_widths: Vec<f32>,
    /// 各行の高さ
    pub line_heights: Vec<f32>,
    /// ベースライン位置
    pub baseline: f32,
    /// アセント（ベースライン上の高さ）
    pub ascent: f32,
    /// ディセント（ベースライン下の高さ）
    pub descent: f32,
}

/// テキスト測定システム
pub struct TextMeasurementSystem {
    font_system: Arc<Mutex<FontSystem>>,
    measurement_cache: HashMap<String, TextMeasurement>,
}

impl TextMeasurementSystem {
    pub fn new() -> Self {
        Self {
            font_system: Arc::new(Mutex::new(FontSystem::new())),
            measurement_cache: HashMap::new(),
        }
    }

    /// 日本語文字を考慮したテキスト幅の計算
    fn calculate_text_width(text: &str, font_size: f32) -> f32 {
        let mut width = 0.0;
        
        for ch in text.chars() {
            // 日本語文字（全角文字）かどうかを判定
            if Self::is_fullwidth_char(ch) {
                // 全角文字は半角文字の約2倍の幅
                width += font_size * 1.0; // 全角文字の幅
            } else {
                // 半角文字（英数字など）
                width += font_size * 0.5; // 半角文字の幅
            }
        }
        
        width
    }

    /// 文字が全角文字（CJK、ひらがな、カタカナなど）かどうかを判定
    fn is_fullwidth_char(ch: char) -> bool {
        match ch {
            // ひらがな
            '\u{3040}'..='\u{309F}' => true,
            // カタカナ
            '\u{30A0}'..='\u{30FF}' => true,
            // CJK統合漢字
            '\u{4E00}'..='\u{9FFF}' => true,
            // CJK統合漢字拡張A
            '\u{3400}'..='\u{4DBF}' => true,
            // 全角英数字・記号
            '\u{FF00}'..='\u{FFEF}' => true,
            // ハングル音節
            '\u{AC00}'..='\u{D7AF}' => true,
            // その他の全角文字
            _ => {
                // Unicodeの東アジアの幅プロパティが「Wide」または「Fullwidth」の文字
                // 簡易的な判定として、文字コードが一定範囲内にあるかチェック
                let code = ch as u32;
                (code >= 0x1100 && code <= 0x115F) || // ハングル字母
                (code >= 0x2E80 && code <= 0x2EFF) || // CJK部首補助
                (code >= 0x2F00 && code <= 0x2FDF) || // 康熙部首
                (code >= 0x3000 && code <= 0x303F) || // CJK記号及び句読点
                (code >= 0x31C0 && code <= 0x31EF) || // CJK筆画
                (code >= 0x3200 && code <= 0x32FF) || // 囲み CJK文字・月
                (code >= 0x3300 && code <= 0x33FF) || // CJK互換
                (code >= 0xF900 && code <= 0xFAFF) || // CJK互換漢字
                (code >= 0xFE30 && code <= 0xFE4F)    // CJK互換形
            }
        }
    }

    /// テキストを正確に測定する
    pub fn measure_text(
        &mut self,
        text: &str,
        font_size: f32,
        font_family: &str,
        max_width: Option<f32>,
        line_height_multiplier: Option<f32>,
    ) -> TextMeasurement {
        // キャッシュキーを生成
        let cache_key = format!(
            "{}:{}:{}:{}:{}", 
            text, 
            font_size, 
            font_family, 
            max_width.unwrap_or(-1.0),
            line_height_multiplier.unwrap_or(1.4)
        );

        // キャッシュから確認
        if let Some(cached) = self.measurement_cache.get(&cache_key) {
            return cached.clone();
        }

        let measurement = self.measure_text_internal(
            text, 
            font_size, 
            font_family, 
            max_width,
            line_height_multiplier
        );

        // キャッシュに保存（最大1000エントリまで）
        if self.measurement_cache.len() < 1000 {
            self.measurement_cache.insert(cache_key, measurement.clone());
        }

        measurement
    }

    fn measure_text_internal(
        &mut self,
        text: &str,
        font_size: f32,
        font_family: &str,
        max_width: Option<f32>,
        line_height_multiplier: Option<f32>,
    ) -> TextMeasurement {
        let line_height_mult = line_height_multiplier.unwrap_or(1.4);
        let line_height = font_size * line_height_mult;
        let metrics = Metrics::new(font_size, line_height);

        let mut font_system = self.font_system.lock().unwrap();
        let mut buffer = Buffer::new(&mut *font_system, metrics);

        // max_widthがある時だけ改行制御
        if let Some(width) = max_width {
            buffer.set_size(&mut *font_system, Some(width), None);
        } else {
            buffer.set_size(&mut *font_system, None, None);
        }

        // フォントファミリーを設定
        let family = if font_family == "default" || font_family.is_empty() {
            Family::SansSerif
        } else {
            Family::Name(font_family)
        };

        // テキストを設定
        buffer.set_text(
            &mut *font_system,
            text,
            &Attrs::new().family(family).weight(Weight::NORMAL),
            Shaping::Advanced,
        );

        // シェイピングを実行（レイアウトを計算）
        buffer.shape_until_scroll(&mut *font_system, false);

        // 実際の測定を実行
        self.extract_measurements(&buffer, metrics, text)
    }

    fn extract_measurements(&self, buffer: &Buffer, metrics: Metrics, text: &str) -> TextMeasurement {
        let mut line_widths = Vec::new();
        let mut line_heights = Vec::new();
        let mut max_width = 0.0_f32;
        let mut total_height = 0.0_f32;
        let mut _layout_run_count = 0;

        // glyphonのBufferからレイアウト情報を取得
        for layout_run in buffer.layout_runs() {
            _layout_run_count += 1;
            let line_height = metrics.line_height;
            let mut line_width = 0.0_f32;
            let mut _glyph_count = 0;

            // この行のグリフを走査して幅を計算
            for glyph in layout_run.glyphs {
                _glyph_count += 1;
                // グリフの右端位置を計算
                let glyph_right = glyph.x + glyph.w as f32;
                if glyph_right > line_width {
                    line_width = glyph_right;
                }
            }

            // デバッグ情報
            // println!("DEBUG: Layout run {} - line_width: {:.1}, glyph_count: {}", 
            //          layout_run_count, line_width, glyph_count);

            line_widths.push(line_width);
            line_heights.push(line_height);
            
            if line_width > max_width {
                max_width = line_width;
            }
            
            // ★ 修正: 行の高さを正確に計算（行間も考慮）
            total_height += line_height;
        }

        // 行がない場合または測定に失敗した場合のフォールバック
        if line_widths.is_empty() || max_width == 0.0 {
            // 簡易的なフォールバック計算 - 改行を考慮した文字数ベース
            let lines: Vec<&str> = text.lines().collect();
            let line_count = lines.len().max(1);
            
            for line in lines {
                // 日本語文字（全角）と英語文字（半角）を区別して幅を計算
                let estimated_line_width = Self::calculate_text_width(line, metrics.font_size);
                
                line_widths.push(estimated_line_width);
                line_heights.push(metrics.line_height);
                
                if estimated_line_width > max_width {
                    max_width = estimated_line_width;
                }
            }
            
            // ★ 修正: 複数行の場合の総高さを正確に計算
            total_height = line_count as f32 * metrics.line_height;
            
            // println!("DEBUG: Using fallback for '{}' - lines: {}, max_width: {:.1}, total_height: {:.1}", 
            //          text, line_count, max_width, total_height);
        }

        // println!("DEBUG: Final measurement - width: {:.1}, height: {:.1}, lines: {}", 
        //          max_width, total_height, line_widths.len());

        // ★ 修正: 高さに十分な余裕を持たせる
        let safe_height = total_height.max(metrics.font_size * 2.5);

        TextMeasurement {
            width: max_width,
            height: safe_height,
            line_count: line_widths.len(),
            line_widths,
            line_heights,
            baseline: metrics.font_size * 0.8, // 一般的なベースライン位置
            ascent: metrics.font_size * 0.8,
            descent: metrics.font_size * 0.2,
        }
    }

    /// 指定した幅に収まるようにテキストを測定
    pub fn measure_text_with_wrapping(
        &mut self,
        text: &str,
        font_size: f32,
        font_family: &str,
        max_width: f32,
        line_height_multiplier: Option<f32>,
    ) -> TextMeasurement {
        self.measure_text(text, font_size, font_family, Some(max_width), line_height_multiplier)
    }

    /// 1行でのテキスト幅を測定（改行なし）
    pub fn measure_single_line(
        &mut self,
        text: &str,
        font_size: f32,
        font_family: &str,
    ) -> TextMeasurement {
        self.measure_text(text, font_size, font_family, None, Some(1.0))
    }

    /// フォントメトリクス（アセント、ディセント等）を取得
    pub fn get_font_metrics(&mut self, font_size: f32, font_family: &str) -> (f32, f32, f32) {
        let measurement = self.measure_text("Ag", font_size, font_family, None, Some(1.0));
        (measurement.ascent, measurement.descent, measurement.height)
    }

    /// キャッシュをクリア
    pub fn clear_cache(&mut self) {
        self.measurement_cache.clear();
    }
}

impl Default for TextMeasurementSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// グローバルなテキスト測定システムインスタンス
static GLOBAL_TEXT_MEASUREMENT: OnceLock<Arc<Mutex<TextMeasurementSystem>>> = OnceLock::new();

/// グローバルテキスト測定システムを取得
pub fn get_text_measurement_system() -> Arc<Mutex<TextMeasurementSystem>> {
    GLOBAL_TEXT_MEASUREMENT.get_or_init(|| {
        Arc::new(Mutex::new(TextMeasurementSystem::new()))
    }).clone()
}

/// 便利関数：テキストサイズを測定
pub fn measure_text_size(
    text: &str,
    font_size: f32,
    font_family: &str,
    max_width: Option<f32>,
) -> (f32, f32) {
    let system = get_text_measurement_system();
    let mut system_guard = system.lock().unwrap();
    let measurement = system_guard.measure_text(text, font_size, font_family, max_width, None);
    (measurement.width, measurement.height)
}

/// 便利関数：改行を考慮したテキストサイズを測定
pub fn measure_text_with_wrap(
    text: &str,
    font_size: f32,
    font_family: &str,
    max_width: f32,
) -> (f32, f32, usize) {
    let system = get_text_measurement_system();
    let mut system_guard = system.lock().unwrap();
    let measurement = system_guard.measure_text_with_wrapping(text, font_size, font_family, max_width, None);
    (measurement.width, measurement.height, measurement.line_count)
}