// src/engine/core/mod.rs
// Engine Core モジュール - engine.rsをリファクタリングして分割したモジュール群

pub mod component;
pub mod core;
pub mod dynamic_section;
pub mod event;
pub mod flow;
pub mod layout;
pub mod render;
pub mod utils;

// 公開API
pub use core::Engine;
pub use event::{apply_action, step_whens, sync_button_handlers};
