#[derive(Debug, Clone)]
pub enum UIEvent {
    ButtonPressed { id: String },
    ButtonReleased { id: String },
    
    // ★ 新規追加: テキスト入力とIME関連のイベント
    TextChanged { field_id: String, new_value: String },
    TextFocused { field_id: String },
    TextBlurred { field_id: String },
    TextSubmitted { field_id: String },
    KeyPressed { field_id: String, key: String, modifiers: KeyModifiers },
    
    // IME関連イベント
    ImeComposition { field_id: String, composition_text: String, cursor_range: Option<(usize, usize)> },
    ImeCommit { field_id: String, committed_text: String },
    ImeCancel { field_id: String },
    ImeEnabled { field_id: String },
    ImeDisabled { field_id: String },
    
    // 今後追加で...
    // MouseMoved { pos: [f32; 2] },
}

/// キーボード修飾キーの状態
#[derive(Debug, Clone, Default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub cmd: bool,  // Macのコマンドキー
}

use std::collections::{HashMap, VecDeque};

pub struct EventQueue {
    pub queue: VecDeque<UIEvent>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self { queue: VecDeque::new() }
    }
    pub fn push(&mut self, event: UIEvent) {
        self.queue.push_back(event);
    }
    pub fn pop(&mut self) -> Option<UIEvent> {
        self.queue.pop_front()
    }
    pub fn drain(&mut self) -> Vec<UIEvent> {
        self.queue.drain(..).collect()
    }
}
