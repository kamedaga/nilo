# 新しいレイアウトシステム - 設計仕様書

## 概要

Niloプロジェクトに新しいレイアウトシステムを実装しました。このシステムは、子要素から親要素へのボトムアップ方式でサイズを計算し、width、height、max_width、min_width等のスタイルプロパティを優先的に処理する、シンプルで汎用的なレイアウトエンジンです。

## 設計思想

### 1. 子要素からの計算（Bottom-Up）
従来のトップダウン方式ではなく、葉要素（テキスト、画像など）から順に親要素まで計算することで、より正確で自然なレイアウトを実現します。

### 2. スタイル優先度の明確化
```
width/height > relative_width/relative_height > 内在的サイズ
min/max制約は最終的に適用
```

### 3. シンプルで汎用的
どのViewでも基本的に同じレイアウト原理で処理できるよう設計されています。

## 実装構造

### コアファイル構成
```
src/ui/
├── layout_new.rs          # 新レイアウトエンジンの本体
├── layout_wrapper.rs      # 便利関数とテスト
├── layout_integration.rs  # 既存システムとの統合
└── mod.rs                # モジュール公開
```

### 主要構造体

#### `LayoutEngine`
レイアウト計算の中核となるエンジン
- コンポーネントのキャッシュ機能付き
- 再帰的なレイアウト計算に対応

#### `LayoutContext`  
レイアウト計算に必要なコンテキスト情報
```rust
pub struct LayoutContext {
    pub window_size: [f32; 2],      // ビューポートサイズ（vw/vh用）
    pub parent_size: [f32; 2],      // 親要素サイズ（%計算用）
    pub root_font_size: f32,        // remの基準
    pub font_size: f32,             // emの基準
    pub default_font: String,       // デフォルトフォント
}
```

#### `ComputedSize`
計算されたサイズ情報
```rust
pub struct ComputedSize {
    pub width: f32,
    pub height: f32,
    pub intrinsic_width: f32,       // 内在的サイズ
    pub intrinsic_height: f32,
    pub has_explicit_width: bool,   // 明示的指定の有無
    pub has_explicit_height: bool,
}
```

## レイアウト計算フロー

### 1. サイズ計算（`compute_node_size`）
```
1. スタイルから明示的なサイズ取得（width/height優先）
2. 内在的サイズ計算（子要素から積算）
3. 明示的でない部分は内在的サイズを使用
4. min/max制約を適用
```

### 2. 内在的サイズ計算（`compute_intrinsic_size`）
ノードタイプ別の具体的な計算:

#### テキスト（`compute_text_size`）
- フォントサイズとフォントファミリーから正確な測定
- `max_width`による自動改行対応
- パディング込みの最終サイズ

#### VStack（`compute_vstack_size`）
```
最大幅 = max(子要素の幅)
合計高さ = Σ(子要素の高さ) + Σ(スペーシング)
```

#### HStack（`compute_hstack_size`）  
```
合計幅 = Σ(子要素の幅) + Σ(スペーシング)
最大高さ = max(子要素の高さ)
```

### 3. 配置計算（`layout_with_positioning`）
- サイズ確定後の実際の配置座標を計算
- VStack: 縦方向に順次配置
- HStack: 横方向に順次配置

## 使用方法

### 基本的な使用例
```rust
use crate::ui::{LayoutEngine, LayoutContext};

let mut engine = LayoutEngine::new();
let context = LayoutContext {
    window_size: [1920.0, 1080.0],
    parent_size: [800.0, 600.0],
    root_font_size: 16.0,
    font_size: 16.0,
    default_font: "Arial".to_string(),
};

let results = engine.layout_with_positioning(
    nodes,
    &context,
    [800.0, 600.0],
    [0.0, 0.0],
    &eval,
    &get_image_size,
    &app,
);
```

### 既存システムとの統合
```rust
use crate::ui::layout_integration::layout_with_new_system;

// 新しいシステムを有効にする
std::env::set_var("NILO_NEW_LAYOUT", "1");

if is_new_layout_system_enabled() {
    layout_with_new_system(nodes, params, eval, get_image_size, app)
} else {
    // 従来のシステム
    layout_vstack(nodes, params, eval, get_image_size)
}
```

## 対応するスタイルプロパティ

### サイズ関連
- `width`, `height` - 明示的サイズ（最優先）
- `relative_width`, `relative_height` - 相対単位（vw, vh, %, etc.）
- `min_width`, `min_height` - 最小サイズ制約
- `max_width`, `max_height` - 最大サイズ制約（max_heightは未実装）

### スペーシング関連  
- `gap` - 相対単位のスペーシング（最優先）
- `relative_spacing` - 相対単位のスペーシング
- `spacing` - 固定スペーシング

### パディング関連
- `relative_padding` - 相対単位パディング
- `padding` - 固定パディング

## 相対単位対応

### 対応単位
- `px` - ピクセル
- `vw`, `vh` - ビューポート基準（1vw = viewport width / 100）
- `ww`, `wh` - ウィンドウ基準（Nilo独自単位）
- `%` - 親要素基準
- `em` - 現在のフォントサイズ基準
- `rem` - ルートフォントサイズ基準
- `auto` - 自動サイズ

### 計算例
```rust
// 90vw の場合
width = 90.0 * window_size[0] / 100.0  // 1920px * 0.9 = 1728px

// 50% の場合  
width = 50.0 * parent_size[0] / 100.0  // 800px * 0.5 = 400px
```

## テキスト測定

### 簡易測定システム
現在は文字数とフォントサイズから推定計算:
```rust
文字幅 = 文字数 * (フォントサイズ * 0.6)
行高さ = フォントサイズ * 1.2
```

### 改行処理
`max_width`が指定されている場合:
```rust
1行あたり文字数 = max_width / 平均文字幅
行数 = (総文字数 + 1行文字数 - 1) / 1行文字数
最終高さ = 行数 * 行高さ
```

## パフォーマンス最適化

### コンポーネントキャッシュ
```rust
// 同一コンポーネントの再計算を避ける
component_cache: HashMap<String, ComputedSize>
```

### 借用回避設計
- レイアウト関数内で必要な値を事前計算
- `self`の借用競合を回避

## 制限事項と今後の改善点

### 現在の制限
1. **テキスト測定の精度**: 実際のフォントレンダリングとは誤差がある
2. **複合レイアウトの再帰**: VStack/HStack内の複合要素処理は簡略化
3. **max_height**: 未実装
4. **Flexbox様式**: justify-content等の高度な配置オプション未対応

### 改善計画
1. **glyphonベースの正確なテキスト測定**: `TextMeasurementSystem`との統合
2. **レイアウトキャッシュ**: サイズ変更がない限り再計算をスキップ  
3. **アニメーション対応**: レイアウト変更の補間
4. **パフォーマンス計測**: ベンチマークテストの追加

## テストカバレッジ

### 単体テスト
- テキストノードのサイズ計算
- VStackの子要素積算
- 明示的サイズの優先度
- 相対単位の変換精度

### 統合テスト  
- 既存LayoutParamsとの互換性
- 実際のアプリケーションとの統合

### テスト実行
```bash
# 新レイアウトシステムのテスト
cargo test layout_wrapper
cargo test layout_integration

# 全体テスト
cargo test
```

## アプリケーション例での検証

提供されたサンプルコード：
```nilo
VStack(style: {width: 90ww, background: "#000000"}) {
    Text("Hello Nilo", style: {font_size: 40px, color: "#ffffff"})
    Text("Niloと創る新しいUI", style: {font_size: 25px, color: "#a7edffff"})
    HStack(style: {width: 90ww}) {
        Spacing(5)
        Card1()
        Card1() 
        Card1()
    }
    Spacing(10)
}
```

### 計算フロー
1. `Text("Hello Nilo")`: フォントサイズ40px → 推定サイズ計算
2. `Text("Niloと創る新しいUI")`: フォントサイズ25px → 推定サイズ計算
3. `HStack`: 
   - `Spacing(5)`: 5px × 5px
   - `Card1()` × 3: コンポーネントを展開してサイズ計算
   - 横方向に合計: 5 + Card1幅×3 + スペーシング×2
4. `Spacing(10)`: 10px × 10px
5. `VStack`全体: 各要素の高さを縦方向に合計

この新しいレイアウトシステムにより、より予測可能で正確なUIレイアウトが実現されます。