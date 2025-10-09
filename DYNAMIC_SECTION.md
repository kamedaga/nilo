# Dynamic Section 仕様

## 概要

`dynamic_section`は、毎フレーム再描画される特別なセクションです。通常のUI要素は初回描画後にキャッシュされますが、`dynamic_section`内のコンテンツは常に最新の状態を反映します。

## 構文

```nilo
dynamic_section <name>(style: {...}) {
    // 毎フレーム更新されるコンテンツ
}
```

## パラメータ

- **name**: セクションの識別子（文字列）
- **style**: オプションのスタイル定義（背景、パディング、ボーダーなど）

## 使用例

### 基本的な使用例

```nilo
timeline MainTimeline {
    state.frame_count = 0
    
    VStack(style: {width: 100ww, height: 100wh}) {
        // 静的コンテンツ（キャッシュされる）
        Text("タイトル", style: {font_size: 32px})
        
        // 動的コンテンツ（毎フレーム更新）
        dynamic_section fps_display(style: {
            background: "#1a1a2e",
            padding: 15,
            rounded: 8px
        }) {
            Text("フレーム数: {}", state.frame_count, style: {
                font_size: 18px,
                color: "#ffffff"
            })
        }
    }
}
```

### アニメーションの例

```nilo
timeline Animation {
    state.time = 0.0
    state.x_position = 0.0
    
    dynamic_section animated_content(style: {
        width: 100ww,
        height: 100wh
    }) {
        // timeに基づいてx_positionを更新
        Text("Moving Object", style: {
            position: [state.x_position, 100],
            font_size: 24px
        })
        
        Text("Time: {:.2}s", state.time, style: {
            position: [10, 10],
            font_size: 16px
        })
    }
}
```

### リアルタイムデータ表示

```nilo
timeline Dashboard {
    state.cpu_usage = 0.0
    state.memory_usage = 0.0
    state.network_speed = 0.0
    
    VStack(style: {width: 100ww, padding: 20}) {
        Text("System Monitor", style: {
            font_size: 32px,
            padding: [0, 0, 20, 0]
        })
        
        dynamic_section system_stats(style: {
            background: "#0f3460",
            padding: 15,
            rounded: 8px,
            border_color: "#00d4ff"
        }) {
            Text("CPU使用率: {:.1}%", state.cpu_usage, style: {
                font_size: 18px,
                color: "#ffffff"
            })
            Text("メモリ使用率: {:.1}%", state.memory_usage, style: {
                font_size: 18px,
                color: "#ffffff"
            })
            Text("ネットワーク速度: {:.2} MB/s", state.network_speed, style: {
                font_size: 18px,
                color: "#ffffff"
            })
        }
    }
}
```

## 動作の仕組み

### レンダリングフロー

1. **静的パート（`layout_static_part`）**
   - 通常のUI要素を描画
   - `DynamicSection`は**スキップ**される（`continue`）
   - 結果はキャッシュされる

2. **動的パート（`layout_dynamic_part`）**
   - **毎フレーム実行**される
   - `DynamicSection`のみを探索・描画
   - ネストした`DynamicSection`も再帰的に処理

3. **最終合成**
   ```rust
   let (static_stencils, static_buttons) = layout_static_part(...);
   let (dynamic_stencils, dynamic_buttons) = layout_dynamic_part(...); // 毎フレーム
   
   stencils.extend(static_stencils);
   stencils.extend(dynamic_stencils); // 動的コンテンツを上に重ねる
   ```

### キャッシュ戦略

```rust
// 静的パート
if cache_invalid || state.static_stencils.is_none() {
    state.static_stencils = Some(layout_static_part(...));
}

// 動的パート（常に実行）
let dynamic_stencils = layout_dynamic_part(...);
```

## パフォーマンス考慮事項

### ✅ 推奨される使用方法

1. **頻繁に変更される小さなセクション**
   ```nilo
   dynamic_section timer() {
       Text("Time: {}", state.current_time)
   }
   ```

2. **リアルタイムフィードバック**
   ```nilo
   dynamic_section mouse_position() {
       Text("Mouse: ({}, {})", state.mouse_x, state.mouse_y)
   }
   ```

3. **アニメーション**
   ```nilo
   dynamic_section animation() {
       Circle(radius: state.animated_radius)
   }
   ```

### ❌ 避けるべき使用方法

1. **静的コンテンツ全体を動的セクションにする**
   ```nilo
   // 悪い例：全体が毎フレーム再描画される
   dynamic_section entire_ui() {
       VStack() {
           Text("Static Title")
           Text("Static Description")
           Button("Static Button")
       }
   }
   ```

2. **重いレイアウト計算**
   ```nilo
   // 悪い例：複雑なレイアウトを毎フレーム計算
   dynamic_section heavy_layout() {
       VStack() {
           foreach item in state.large_list {
               ComplexComponent(item)
           }
       }
   }
   ```

### 最適化のヒント

1. **必要な部分だけを動的にする**
   ```nilo
   VStack() {
       Text("Static Header")  // キャッシュされる
       
       dynamic_section live_data() {  // この部分だけ毎フレーム更新
           Text("Live: {}", state.live_value)
       }
       
       Text("Static Footer")  // キャッシュされる
   }
   ```

2. **状態の更新頻度を制御**
   ```rust
   // Rust側で更新頻度を制限
   if frame_count % 10 == 0 {  // 10フレームに1回だけ更新
       state.set("display_value", new_value.to_string());
   }
   ```

## ネストされたDynamicSection

DynamicSectionは入れ子にできます：

```nilo
dynamic_section outer(style: {background: "#1a1a2e"}) {
    Text("Outer: {}", state.outer_value)
    
    dynamic_section inner(style: {background: "#0f3460"}) {
        Text("Inner: {}", state.inner_value)
    }
}
```

両方とも毎フレーム再描画されます。

## スタイルサポート

DynamicSectionは以下のスタイルプロパティをサポートします：

- `background`: 背景色
- `padding`: 内側の余白
- `rounded`: 角の丸み
- `border_color`: ボーダー色
- `shadow`: 影効果
- `width`, `height`: サイズ指定

```nilo
dynamic_section styled(style: {
    background: "#1a1a2e",
    padding: [20, 15],
    rounded: 12px,
    border_color: "#00d4ff",
    shadow: {blur: 10, offset: [0, 2], color: "#00000044"}
}) {
    // コンテンツ
}
```

## まとめ

- **用途**: 頻繁に変更される小さなUIセクション
- **特徴**: 毎フレーム再描画（キャッシュなし）
- **最適化**: 必要最小限の範囲を動的にする
- **ネスト**: サポート（再帰的に処理）
- **スタイル**: 通常のノードと同じスタイル適用可能
