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

    /// 日本語文字を考慮したテキスト幅の計算（より正確な版）
    fn calculate_text_width(text: &str, font_size: f32) -> f32 {
        let mut width = 0.0;
        
        for ch in text.chars() {
            // 文字種別による幅の計算（より正確な係数を使用）
            if Self::is_fullwidth_char(ch) {
                // 全角文字: フォントサイズの約0.9倍（実測に基づく調整）
                width += font_size * 0.9;
            } else if ch.is_ascii_alphabetic() || ch.is_ascii_digit() {
                // 英数字: フォントサイズの約0.52倍
                width += font_size * 0.52;
            } else if ch.is_ascii_punctuation() || ch == ' ' {
                // 記号・スペース: フォントサイズの約0.3倍
                width += font_size * 0.3;
            } else {
                // その他の文字: フォントサイズの約0.6倍
                width += font_size * 0.6;
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

        // 幅制約を設定 - max_widthが指定された場合のみ改行を適用
        if let Some(width) = max_width {
            // 指定された幅で改行を適用
            buffer.set_size(&mut *font_system, Some(width), None);
        } else {
            // max_widthが指定されていない場合は改行なし（十分に大きな幅を設定）
            buffer.set_size(&mut *font_system, None, None);
        }

        // より適切なフォント選択
        let family = if font_family == "default" || font_family.is_empty() {
            // システムのデフォルトフォントを使用
            Family::SansSerif
        } else {
            Family::Name(font_family)
        };

        // テキストを設定（CJK文字の改行を改善）
        let attrs = Attrs::new()
            .family(family)
            .weight(Weight::NORMAL);
            
        buffer.set_text(
            &mut *font_system,
            text,
            &attrs,
            Shaping::Advanced,
        );

        // より確実なレイアウト処理
        // 最初に全体をシェイピング
        buffer.shape_until_scroll(&mut *font_system, true);
        
        // 改行処理を強制実行
        if max_width.is_some() {
            // 改行が必要な場合は複数回実行して確実にレイアウト
            for _ in 0..5 {
                buffer.shape_until_scroll(&mut *font_system, false);
            }
        }

        // 実際の測定を実行
        self.extract_measurements(&buffer, metrics, text, max_width)
    }

    fn extract_measurements(&self, buffer: &Buffer, metrics: Metrics, text: &str, max_width: Option<f32>) -> TextMeasurement {
        let mut line_widths = Vec::new();
        let mut line_heights = Vec::new();
        let mut max_text_width = 0.0_f32;
        let mut total_height = 0.0_f32;
        let mut layout_run_count = 0;

        println!("DEBUG: Starting measurement extraction for text: '{}'", text);

        // glyphonのBufferからレイアウト情報を取得
        for layout_run in buffer.layout_runs() {
            layout_run_count += 1;
            let line_height = metrics.line_height;
            let mut line_width = 0.0_f32;
            let mut glyph_count = 0;
            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;

            // この行のグリフを走査して正確な幅を計算
            for glyph in layout_run.glyphs {
                glyph_count += 1;
                let glyph_left = glyph.x;
                let glyph_right = glyph.x + glyph.w as f32;
                
                if glyph_left < min_x {
                    min_x = glyph_left;
                }
                if glyph_right > max_x {
                    max_x = glyph_right;
                }
            }
            
            // 行幅を計算
            if min_x != f32::MAX && max_x != f32::MIN {
                line_width = max_x - min_x;
            } else if glyph_count == 0 {
                line_width = 0.0;
            }

            println!("DEBUG: Layout run {} - line_width: {:.1}, glyph_count: {}, min_x: {:.1}, max_x: {:.1}", 
                     layout_run_count, line_width, glyph_count, min_x, max_x);

            line_widths.push(line_width);
            line_heights.push(line_height);
            
            if line_width > max_text_width {
                max_text_width = line_width;
            }
            
            total_height += line_height;
        }

        println!("DEBUG: Found {} layout runs, max_width: {:.1}, total_height: {:.1}", 
                 layout_run_count, max_text_width, total_height);

        // 測定が失敗した場合のフォールバック処理
        if line_widths.is_empty() || (max_text_width == 0.0 && !text.is_empty()) {
            println!("DEBUG: Using fallback measurement for '{}'", text);
            
            // 改行文字による明示的な分割をまず処理
            let explicit_lines: Vec<&str> = text.lines().collect();
            let line_count = explicit_lines.len();
            
            if line_count > 1 {
                // 明示的な改行がある場合
                for line in explicit_lines {
                    let estimated_width = Self::calculate_text_width(line, metrics.font_size);
                    line_widths.push(estimated_width);
                    line_heights.push(metrics.line_height);
                    
                    if estimated_width > max_text_width {
                        max_text_width = estimated_width;
                    }
                }
                total_height = line_count as f32 * metrics.line_height;
            } else {
                // 単一行の場合 - max_widthが指定されているかで処理を分岐
                let single_line = text.trim();
                let text_width = Self::calculate_text_width(single_line, metrics.font_size);
                
                if let Some(width_limit) = max_width {
                    // max_widthが指定されている場合、簡易的な改行処理を適用
                    if text_width > width_limit {
                        // 文字列を余分に分割して改行をシミュレート
                        let estimated_lines = (text_width / width_limit).ceil() as usize;
                        for _ in 0..estimated_lines {
                            line_widths.push(width_limit.min(text_width));
                            line_heights.push(metrics.line_height);
                        }
                        max_text_width = width_limit;
                        total_height = estimated_lines as f32 * metrics.line_height;
                    } else {
                        // 改行不要
                        line_widths.push(text_width);
                        line_heights.push(metrics.line_height);
                        max_text_width = text_width;
                        total_height = metrics.line_height;
                    }
                } else {
                    // max_widthが指定されていない場合は改行なし
                    line_widths.push(text_width);
                    line_heights.push(metrics.line_height);
                    max_text_width = text_width;
                    total_height = metrics.line_height;
                }
            }
            
            println!("DEBUG: Fallback result - lines: {}, max_width: {:.1}, total_height: {:.1}", 
                     line_widths.len(), max_text_width, total_height);
        }

        println!("DEBUG: Final measurement - width: {:.1}, height: {:.1}, lines: {}", 
                 max_text_width, total_height, line_widths.len());

        // より正確な高さ計算（必要最小限の余裕のみ）
        let safe_height = if total_height > 0.0 {
            total_height
        } else {
            metrics.line_height
        };

        // 幅にも小さな余裕を追加（レンダリング誤差を考慮）
        let safe_width = max_text_width + (metrics.font_size * 0.05);

        TextMeasurement {
            width: safe_width,
            height: safe_height,
            line_count: line_widths.len().max(1),
            line_widths,
            line_heights,
            baseline: metrics.font_size * 0.75, // より正確なベースライン位置
            ascent: metrics.font_size * 0.75,
            descent: metrics.font_size * 0.25,
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

    /// より精密な改行を行うテキスト測定
    pub fn measure_text_with_precise_wrapping(
        &mut self,
        text: &str,
        font_size: f32,
        font_family: &str,
        max_width: f32,
        line_height_multiplier: Option<f32>,
    ) -> TextMeasurement {
        println!("DEBUG: Precise wrapping for '{}' with max_width: {:.1}", text, max_width);
        
        // 複数のアプローチで測定を試行
        let attempts = vec![
            max_width,           // オリジナルの幅
            max_width * 0.95,    // 5%縮小
            max_width * 0.90,    // 10%縮小
        ];
        
        for (i, width) in attempts.iter().enumerate() {
            let result = self.measure_text(text, font_size, font_family, Some(*width), line_height_multiplier);
            
            println!("DEBUG: Attempt {} with width {:.1} -> result width: {:.1}, lines: {}", 
                     i + 1, width, result.width, result.line_count);
            
            // 適切な幅内に収まった、または十分に改行された場合
            if result.width <= max_width * 1.02 || result.line_count > 1 {
                return result;
            }
        }
        
        // 最後の手段として手動改行を試行
        self.manual_text_wrapping(text, font_size, font_family, max_width, line_height_multiplier)
    }
    
    /// 手動でテキストを改行して測定
    fn manual_text_wrapping(
        &mut self,
        text: &str,
        font_size: f32,
        _font_family: &str, // 現在は使用していないが将来の拡張のために保持
        max_width: f32,
        line_height_multiplier: Option<f32>,
    ) -> TextMeasurement {
        let line_height_mult = line_height_multiplier.unwrap_or(1.4);
        let line_height = font_size * line_height_mult;
        
        let mut lines = Vec::new();
        let mut current_line = String::new();
        
        // 文字単位で幅をチェックして改行
        for ch in text.chars() {
            let test_line = current_line.clone() + &ch.to_string();
            let test_width = Self::calculate_text_width(&test_line, font_size);
            
            if test_width > max_width && !current_line.is_empty() {
                lines.push(current_line.clone());
                current_line = ch.to_string();
            } else {
                current_line.push(ch);
            }
        }
        
        if !current_line.is_empty() {
            lines.push(current_line);
        }
        
        // 各行の幅を計算
        let mut line_widths = Vec::new();
        let mut max_line_width = 0.0;
        
        for line in &lines {
            let width = Self::calculate_text_width(line, font_size);
            line_widths.push(width);
            if width > max_line_width {
                max_line_width = width;
            }
        }
        
        let line_count = lines.len().max(1);
        let total_height = line_count as f32 * line_height;
        
        println!("DEBUG: Manual wrapping result - lines: {}, max_width: {:.1}, total_height: {:.1}", 
                 line_count, max_line_width, total_height);
        
        TextMeasurement {
            width: max_line_width,
            height: total_height,
            line_count,
            line_widths,
            line_heights: vec![line_height; line_count],
            baseline: font_size * 0.75,
            ascent: font_size * 0.75,
            descent: font_size * 0.25,
        }
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

/// 便利関数：より精密な改行を考慮したテキストサイズを測定
pub fn measure_text_with_precise_wrap(
    text: &str,
    font_size: f32,
    font_family: &str,
    max_width: f32,
) -> (f32, f32, usize) {
    let system = get_text_measurement_system();
    let mut system_guard = system.lock().unwrap();
    let measurement = system_guard.measure_text_with_precise_wrapping(text, font_size, font_family, max_width, None);
    (measurement.width, measurement.height, measurement.line_count)
}