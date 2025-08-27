#[derive(Debug, Clone)]
pub enum UIEvent {
    ButtonPressed { id: String },
    ButtonReleased { id: String },
    // 今後追加で...
    // MouseMoved { pos: [f32; 2] },
    // KeyPressed { key: ... },
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
