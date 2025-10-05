use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// パフォーマンス監視システム
pub struct PerfMonitor {
    frame_times: VecDeque<Duration>,
    text_render_times: VecDeque<Duration>,
    max_samples: usize,
    current_frame_start: Option<Instant>,
    current_text_start: Option<Instant>,
    total_frames: u64,
    total_text_renders: u64,
}

impl PerfMonitor {
    pub fn new(max_samples: usize) -> Self {
        Self {
            frame_times: VecDeque::with_capacity(max_samples),
            text_render_times: VecDeque::with_capacity(max_samples),
            max_samples,
            current_frame_start: None,
            current_text_start: None,
            total_frames: 0,
            total_text_renders: 0,
        }
    }

    /// フレーム計測開始
    pub fn start_frame(&mut self) {
        self.current_frame_start = Some(Instant::now());
    }

    /// フレーム計測終了
    pub fn end_frame(&mut self) {
        if let Some(start) = self.current_frame_start.take() {
            let duration = start.elapsed();
            self.frame_times.push_back(duration);
            
            if self.frame_times.len() > self.max_samples {
                self.frame_times.pop_front();
            }
            
            self.total_frames += 1;
        }
    }

    /// テキスト描画計測開始
    pub fn start_text_render(&mut self) {
        self.current_text_start = Some(Instant::now());
    }

    /// テキスト描画計測終了
    pub fn end_text_render(&mut self) {
        if let Some(start) = self.current_text_start.take() {
            let duration = start.elapsed();
            self.text_render_times.push_back(duration);
            
            if self.text_render_times.len() > self.max_samples {
                self.text_render_times.pop_front();
            }
            
            self.total_text_renders += 1;
        }
    }

    /// 平均フレーム時間（ミリ秒）
    pub fn avg_frame_time_ms(&self) -> f32 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        
        let sum: Duration = self.frame_times.iter().sum();
        let avg = sum / self.frame_times.len() as u32;
        avg.as_secs_f32() * 1000.0
    }

    /// 平均FPS
    pub fn avg_fps(&self) -> f32 {
        let avg_frame_time = self.avg_frame_time_ms();
        if avg_frame_time > 0.0 {
            1000.0 / avg_frame_time
        } else {
            0.0
        }
    }

    /// 平均テキスト描画時間（ミリ秒）
    pub fn avg_text_render_time_ms(&self) -> f32 {
        if self.text_render_times.is_empty() {
            return 0.0;
        }
        
        let sum: Duration = self.text_render_times.iter().sum();
        let avg = sum / self.text_render_times.len() as u32;
        avg.as_secs_f32() * 1000.0
    }

    /// 最新フレーム時間（ミリ秒）
    pub fn latest_frame_time_ms(&self) -> f32 {
        self.frame_times.back()
            .map(|d| d.as_secs_f32() * 1000.0)
            .unwrap_or(0.0)
    }

    /// 最新テキスト描画時間（ミリ秒）
    pub fn latest_text_render_time_ms(&self) -> f32 {
        self.text_render_times.back()
            .map(|d| d.as_secs_f32() * 1000.0)
            .unwrap_or(0.0)
    }

    /// パフォーマンス統計
    pub fn get_stats(&self) -> PerfStats {
        PerfStats {
            avg_frame_time_ms: self.avg_frame_time_ms(),
            avg_fps: self.avg_fps(),
            avg_text_render_time_ms: self.avg_text_render_time_ms(),
            latest_frame_time_ms: self.latest_frame_time_ms(),
            latest_text_render_time_ms: self.latest_text_render_time_ms(),
            total_frames: self.total_frames,
            total_text_renders: self.total_text_renders,
            frame_samples: self.frame_times.len(),
            text_render_samples: self.text_render_times.len(),
        }
    }

    /// 統計をリセット
    pub fn reset(&mut self) {
        self.frame_times.clear();
        self.text_render_times.clear();
        self.total_frames = 0;
        self.total_text_renders = 0;
    }
}

/// パフォーマンス統計情報
#[derive(Debug, Clone)]
pub struct PerfStats {
    pub avg_frame_time_ms: f32,
    pub avg_fps: f32,
    pub avg_text_render_time_ms: f32,
    pub latest_frame_time_ms: f32,
    pub latest_text_render_time_ms: f32,
    pub total_frames: u64,
    pub total_text_renders: u64,
    pub frame_samples: usize,
    pub text_render_samples: usize,
}

impl std::fmt::Display for PerfStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Frame: {:.2}ms ({:.1} FPS), Text: {:.2}ms, Frames: {}, Text Renders: {}",
            self.avg_frame_time_ms,
            self.avg_fps,
            self.avg_text_render_time_ms,
            self.total_frames,
            self.total_text_renders
        )
    }
}