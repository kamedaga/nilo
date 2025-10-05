pub mod layout;
// pub mod layout_wrapper; // 新しいレイアウトシステムのラッパー - 無効化
// pub mod layout_integration; // 既存システムとの統合 - 無効化
pub mod layout_foreach_fix; // 新しいモジュールを追加
pub mod text_measurement; // テキスト測定システムを追加
pub mod viewport;
pub mod event;

pub use layout::{LayoutedNode, LayoutParams, layout_vstack};
// pub use layout_wrapper::{layout_with_new_engine, compute_single_node_size}; // ラッパー関数をエクスポート - 無効化
// pub use layout_integration::{layout_with_new_system, calculate_node_size_with_new_system, is_new_layout_system_enabled}; // 統合関数をエクスポート - 無効化
pub use layout_foreach_fix::layout_foreach_impl; // 関数をエクスポート
pub use text_measurement::{TextMeasurementSystem, TextMeasurement, measure_text_size, measure_text_with_wrap, measure_text_with_precise_wrap}; // テキスト測定をエクスポート
