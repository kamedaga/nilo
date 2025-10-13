pub mod layout;
// pub mod layout_wrapper; // 新しいレイアウトシステムのラッパー - 無効化
// pub mod layout_integration; // 既存システムとの統合 - 無効化
pub mod layout_diff;
pub mod layout_foreach_fix; // 新しいモジュールを追加 // レイアウト差分計算システム

// テキスト測定モジュール: 環境に応じて切り替え
#[cfg(all(feature = "glyphon", not(target_arch = "wasm32")))]
pub mod text_measurement; // Native環境: glyphonベース

#[cfg(target_arch = "wasm32")]
pub mod text_measurement_wasm; // WASM環境: DOM APIベース

#[cfg(target_arch = "wasm32")]
pub use text_measurement_wasm as text_measurement; // WASMではtext_measurementとしてエイリアス

pub mod event;
pub mod viewport;

pub use layout::{LayoutParams, LayoutedNode, layout_vstack};
// pub use layout_wrapper::{layout_with_new_engine, compute_single_node_size}; // ラッパー関数をエクスポート - 無効化
// pub use layout_integration::{layout_with_new_system, calculate_node_size_with_new_system, is_new_layout_system_enabled}; // 統合関数をエクスポート - 無効化
pub use layout_diff::{DiffStats, LayoutDiffEngine, NodeHash, NodeId};
pub use layout_foreach_fix::layout_foreach_impl; // 関数をエクスポート // 差分計算をエクスポート

// テキスト測定のエクスポート
#[cfg(any(feature = "glyphon", target_arch = "wasm32"))]
pub use text_measurement::{
    TextMeasurement, TextMeasurementSystem, measure_text_size, measure_text_with_precise_wrap,
    measure_text_with_wrap,
};
