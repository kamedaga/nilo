// src/engine/engine.rs
// Refactored: Engine functionality split into core submodules
// This file serves as a facade for backward compatibility

pub use super::core::Engine;
pub use super::core::component::expand_component_calls_lightweight;
pub use super::core::{apply_action, step_whens, sync_button_handlers};
