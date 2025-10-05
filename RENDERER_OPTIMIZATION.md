# テキストレンダリング軽量化実装

## 概要
テキスト描画のパフォーマンス向上を目的とした軽量化を実装しました。

## 実装した軽量化機能

### 1. テキストバッファキャッシュシステム
- **ファイル**: `src/wgpu_renderer/text.rs`
- **機能**: 
  - テキストコンテンツ、サイズ、フォント、最大幅をキーとしたキャッシュ
  - LRU (Least Recently Used) キャッシュ管理
  - フレームベースのクリーンアップ（60フレームに1回）
  - 最大1000個のテキストをキャッシュ

### 2. テキスト測定キャッシュシステム
- **ファイル**: `src/ui/text_measurement.rs`
- **機能**:
  - テキスト測定結果のキャッシュ
  - 高速近似計算メソッド (`measure_text_width_fast`, `measure_text_height_fast`)
  - キャッシュヒット/ミス統計

### 3. パフォーマンス監視システム
- **ファイル**: `src/wgpu_renderer/perf_monitor.rs`
- **機能**:
  - フレーム時間測定
  - テキスト描画時間測定
  - FPS統計
  - リアルタイムパフォーマンス統計

### 4. 軽量描画メソッド
- **`render_cached_texts_only`**: キャッシュされたテキストのみを高速描画
- **`render_multiple_texts_with_depth`**: キャッシュ優先の階層描画

## パフォーマンス改善のポイント

### Before (改善前)
```rust
// 毎回新しいBufferを作成
let mut buffer = Buffer::new(&mut self.font_system, metrics);
// 個別にprepare/render実行
self.renderer.prepare(...);
self.renderer.render(...);
```

### After (改善後)
```rust
// キャッシュから取得または作成
let (buffer, metrics) = self.get_or_create_buffer(...);
// バッチ処理で一括prepare/render
self.renderer.prepare(..., text_areas.iter().cloned(), ...);
```

## 使用方法

### パフォーマンス統計の取得
```rust
let stats = renderer.get_perf_stats();
println!("FPS: {:.1}, Text Render: {:.2}ms", stats.avg_fps, stats.avg_text_render_time_ms);
```

### キャッシュの活用
```rust
// 通常の描画（自動でキャッシュ活用）
text_renderer.render_multiple_texts(...);

// キャッシュのみの高速描画
let success = text_renderer.render_cached_texts_only(...);
if !success {
    // フォールバック: 通常の描画
    text_renderer.render_multiple_texts(...);
}
```

## 期待される効果

1. **初回描画後のテキスト描画速度向上**: 同じテキストの再描画が大幅に高速化
2. **メモリ使用量の最適化**: LRUキャッシュによる適切なメモリ管理
3. **フレームレート安定化**: バッチ処理による描画コールの削減
4. **CPU使用率削減**: 文字計算の重複処理を排除

## 設定可能なパラメータ

- `max_cache_size`: キャッシュする最大テキスト数（デフォルト: 1000）
- `cleanup_interval`: キャッシュクリーンアップ間隔（デフォルト: 60フレーム）
- `perf_samples`: パフォーマンス統計のサンプル数（デフォルト: 60）

## 注意事項

1. **メモリ使用量**: 多数の異なるテキストを表示する場合、メモリ使用量が増加する可能性があります
2. **初回描画**: キャッシュがない初回描画は従来と同じ速度です
3. **動的テキスト**: 頻繁に変更されるテキストではキャッシュの効果が薄れます

## 今後の改善案

1. **フォントアトラス最適化**: 頻繁に使用される文字の事前ロード
2. **GPU側キャッシュ**: テクスチャアトラスでの文字キャッシュ
3. **レベル別キャッシュ**: 文字、単語、行レベルでの階層キャッシュ
4. **非同期処理**: テキスト処理の一部を別スレッドで実行