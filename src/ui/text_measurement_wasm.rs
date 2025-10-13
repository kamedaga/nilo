// WASM環境用のテキスト測定
// ブラウザのDOM APIを使用してテキストサイズを測定

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

/// グローバルなテキスト測定システムインスタンス
static TEXT_MEASUREMENT: OnceLock<Mutex<TextMeasurementSystem>> = OnceLock::new();

/// テキスト測定システム
pub struct TextMeasurementSystem {
    /// 測定結果のキャッシュ
    cache: HashMap<String, TextMeasurement>,
}

impl TextMeasurementSystem {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// テキストを測定（キャッシュあり） - Native版との互換性のため
    pub fn measure_text(
        &mut self,
        text: &str,
        font_size: f32,
        font_family: &str,
        max_width: Option<f32>,
        line_height_multiplier: Option<f32>,
    ) -> TextMeasurement {
        self.measure(
            text,
            font_size,
            font_family,
            max_width,
            line_height_multiplier,
        )
    }

    /// テキストを測定（キャッシュあり）
    pub fn measure(
        &mut self,
        text: &str,
        font_size: f32,
        font_family: &str,
        max_width: Option<f32>,
        line_height_multiplier: Option<f32>,
    ) -> TextMeasurement {
        let cache_key = format!(
            "{}:{}:{}:{:?}:{:?}",
            text, font_size, font_family, max_width, line_height_multiplier
        );

        if let Some(cached) = self.cache.get(&cache_key) {
            return cached.clone();
        }

        let measurement = Self::measure_text_dom(
            text,
            font_size,
            font_family,
            max_width,
            line_height_multiplier,
        );
        self.cache.insert(cache_key, measurement.clone());
        measurement
    }

    /// DOM APIを使用してテキストを測定
    fn measure_text_dom(
        text: &str,
        font_size: f32,
        font_family: &str,
        max_width: Option<f32>,
        line_height_multiplier: Option<f32>,
    ) -> TextMeasurement {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use web_sys::{HtmlElement, window};

            // 測定用の一時的なDOM要素を作成
            if let Some(window) = window() {
                if let Some(document) = window.document() {
                    if let Ok(element) = document.create_element("div") {
                        if let Some(html_element) = element.dyn_ref::<HtmlElement>() {
                            let style = html_element.style();
                            let _ = style.set_property("position", "absolute");
                            let _ = style.set_property("visibility", "hidden");
                            let _ = style.set_property("white-space", "pre-wrap");
                            let _ = style.set_property("font-size", &format!("{}px", font_size));
                            let _ = style.set_property("font-family", font_family);

                            if let Some(width) = max_width {
                                let _ = style.set_property("max-width", &format!("{}px", width));
                                let _ = style.set_property("overflow-wrap", "break-word");
                            } else {
                                let _ = style.set_property("white-space", "nowrap");
                            }

                            html_element.set_inner_text(text);

                            if let Some(body) = document.body() {
                                let _ = body.append_child(&element);

                                let width = html_element.offset_width() as f32;
                                let height = html_element.offset_height() as f32;

                                let _ = body.remove_child(&element);

                                // line_height_multiplierを使用（デフォルト1.2）
                                let multiplier = line_height_multiplier.unwrap_or(1.2);
                                let line_height = font_size * multiplier;
                                let line_count = (height / line_height).ceil().max(1.0) as usize;

                                return TextMeasurement {
                                    width,
                                    height,
                                    line_count,
                                    line_widths: vec![width; line_count],
                                    line_heights: vec![line_height; line_count],
                                    baseline: font_size * 0.8,
                                    ascent: font_size * 0.8,
                                    descent: font_size * 0.2,
                                };
                            }
                        }
                    }
                }
            }
        }

        // フォールバック: 簡易計算
        let char_count = text.chars().count();
        let estimated_width = if let Some(max_w) = max_width {
            max_w.min(char_count as f32 * font_size * 0.6)
        } else {
            char_count as f32 * font_size * 0.6
        };

        let multiplier = line_height_multiplier.unwrap_or(1.2);
        let line_height = font_size * multiplier;

        let line_count = if let Some(max_w) = max_width {
            ((char_count as f32 * font_size * 0.6) / max_w)
                .ceil()
                .max(1.0) as usize
        } else {
            1
        };

        TextMeasurement {
            width: estimated_width,
            height: line_height * line_count as f32,
            line_count,
            line_widths: vec![estimated_width; line_count],
            line_heights: vec![line_height; line_count],
            baseline: font_size * 0.8,
            ascent: font_size * 0.8,
            descent: font_size * 0.2,
        }
    }

    /// キャッシュをクリア
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

/// グローバルなテキスト測定システムを取得
pub fn get_text_measurement_system() -> &'static Mutex<TextMeasurementSystem> {
    TEXT_MEASUREMENT.get_or_init(|| Mutex::new(TextMeasurementSystem::new()))
}

/// テキストサイズを測定（簡易版）
pub fn measure_text_size(
    text: &str,
    font_size: f32,
    font_family: &str,
    max_width: Option<f32>,
) -> (f32, f32) {
    let system = get_text_measurement_system();
    let mut system = system.lock().unwrap();
    let measurement = system.measure(text, font_size, font_family, max_width, None);
    (measurement.width, measurement.height)
}

/// テキストを折り返しありで測定
pub fn measure_text_with_wrap(text: &str, font_size: f32, max_width: f32) -> TextMeasurement {
    let system = get_text_measurement_system();
    let mut system = system.lock().unwrap();
    system.measure(text, font_size, "sans-serif", Some(max_width), None)
}

/// テキストを精密に測定（折り返しあり）
pub fn measure_text_with_precise_wrap(
    text: &str,
    font_size: f32,
    font_family: &str,
    max_width: Option<f32>,
) -> TextMeasurement {
    let system = get_text_measurement_system();
    let mut system = system.lock().unwrap();
    system.measure(text, font_size, font_family, max_width, None)
}
