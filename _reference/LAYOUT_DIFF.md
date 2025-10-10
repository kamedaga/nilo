# レイアウト差分計算システム

## 概要

レイアウト差分計算システムは、UIノードツリーの変更を検出し、**変更があった部分のみ**を再計算することで、レイアウト計算のパフォーマンスを大幅に向上させるシステムです。

従来のキャッシュシステムとは異なり、ノードのハッシュ値を使って前回の状態と比較し、変更があったノードのみを識別して再計算します。

## 主な特徴

### 1. **差分ベースの再計算**
- ノードの内容（テキスト、スタイルなど）をハッシュ化
- 前回のレイアウト結果と比較
- 変更があったノードのみを再計算

### 2. **高速な変更検出**
- ハッシュ値による高速比較
- ノードIDによる階層的な管理
- O(n)の時間複雑度（nはノード数）

### 3. **統計情報の提供**
- 再計算されたノード数
- キャッシュヒット率
- パフォーマンス分析に有用

## 使用方法

### 基本的な使い方

```rust
use crate::ui::layout_diff::LayoutDiffEngine;
use crate::ui::{LayoutParams, LayoutedNode};

// エンジンの初期化
let mut diff_engine = LayoutDiffEngine::new();

// 差分計算を実行
let layouted_nodes = diff_engine.compute_diff(
    &nodes,        // UIノードの配列
    &params,       // レイアウトパラメータ
    &app,          // アプリケーション定義
    &eval_fn,      // 式評価関数
    &get_img_size, // 画像サイズ取得関数
);

// 変更されたノード数を確認
let dirty_count = diff_engine.dirty_count();
println!("再計算されたノード数: {}", dirty_count);
```

### AppStateへの統合

`AppState`には差分エンジンのフィールドが追加されています：

```rust
pub struct AppState<S> {
    // ...既存のフィールド...
    
    /// 静的部分のレイアウト差分エンジン
    pub layout_diff_static: Option<Rc<RefCell<LayoutDiffEngine<'static>>>>,
    /// 動的部分のレイアウト差分エンジン
    pub layout_diff_dynamic: Option<Rc<RefCell<LayoutDiffEngine<'static>>>>,
}
```

### Engine内での使用例

```rust
// 静的部分のレイアウト（差分計算を使用）
pub fn layout_static_part_with_diff<S>(
    app: &App,
    state: &mut AppState<S>,
    nodes: &[WithSpan<ViewNode>],
    params: LayoutParams,
) -> Vec<LayoutedNode>
where
    S: StateAccess + 'static,
{
    // 差分エンジンを初期化（初回のみ）
    if state.layout_diff_static.is_none() {
        state.layout_diff_static = Some(Rc::new(RefCell::new(LayoutDiffEngine::new())));
    }
    
    // 差分計算を実行
    let engine = state.layout_diff_static.as_ref().unwrap();
    let mut engine_ref = engine.borrow_mut();
    
    let eval_fn = |e: &Expr| state.eval_expr_from_ast(e);
    let get_img_size = |path: &str| state.get_image_size(path);
    
    engine_ref.compute_diff(nodes, &params, app, &eval_fn, &get_img_size)
}
```

## 動作原理

### 1. ノードハッシュの計算

各UIノードの内容からハッシュ値を計算します：

- ノードの型（Text, Button, Image など）
- 主要な属性（テキスト内容、ID、パスなど）
- スタイル情報（色、サイズ、パディングなど）

```rust
// 例: Textノードのハッシュ
ViewNode::Text { format, args } => {
    hash_str = "Text:" + format + eval(args)
}

// 例: Buttonノードのハッシュ
ViewNode::Button { id, label } => {
    hash_str = "Button:" + id + label
}
```

### 2. ノードIDによる階層管理

ノードツリー内での位置を識別するIDを生成：

```rust
// ルート
NodeId::new() → ""

// 1番目のVStack
NodeId.child(0, "VStack") → "VStack_0"

// その子の2番目のText
NodeId.child(1, "Text") → "VStack_0/Text_1"
```

### 3. 差分検出プロセス

```
1. 現在のノードのハッシュを計算
2. 前回のキャッシュからハッシュを取得
3. ハッシュ値を比較
   - 一致 → キャッシュから結果を取得（再計算なし）
   - 不一致 → レイアウトを再計算
4. 結果をキャッシュに保存
```

## パフォーマンス向上の例

### ケース1: テキストの一部のみ変更

```nilo
VStack {
    Text { "タイトル" }              // 変更なし → キャッシュ使用
    Text { "{state.counter}" }      // 変更あり → 再計算
    Button { id: "btn", "クリック" } // 変更なし → キャッシュ使用
}
```

**結果**: 3ノード中1ノードのみ再計算（67%削減）

### ケース2: 動的リストの追加

```nilo
ForEach { item in state.items } {
    Text { "{item.name}" }
}
```

新しいアイテムが追加された場合：
- 既存のアイテム → キャッシュ使用
- 新しいアイテム → 再計算

**結果**: 新規ノードのみ計算

## 統計情報

`DiffStats`構造体で詳細な統計を取得できます：

```rust
pub struct DiffStats {
    pub total_nodes: usize,        // 総ノード数
    pub recomputed_nodes: usize,   // 再計算されたノード数
    pub cached_nodes: usize,       // キャッシュから取得したノード数
    pub cache_hit_rate: f32,       // キャッシュヒット率（%）
}

// 使用例
let stats = DiffStats::new(100, 20);
println!("キャッシュヒット率: {:.1}%", stats.cache_hit_rate); // 80.0%
```

## 制限事項と注意点

### 1. ライフタイム

差分エンジンは`'a`ライフタイムを持つため、ノードの参照が有効である必要があります。

### 2. メモリ使用量

キャッシュは前回の結果を保持するため、メモリを消費します。必要に応じて`clear_cache()`でクリアできます。

```rust
diff_engine.clear_cache(); // キャッシュをクリア
```

### 3. 動的コンテンツ

以下のような動的コンテンツでは効果が限定的です：
- アニメーション（毎フレーム変更）
- リアルタイムデータ（常に更新）

## 今後の拡張予定

1. **粒度の細かい差分検出**
   - スタイル変更のみの場合の最適化
   - 子ノードの追加/削除の効率的な処理

2. **統計ベースの最適化**
   - 頻繁に変更されるノードの検出
   - 自動的なキャッシュ戦略の調整

3. **並列処理**
   - 独立したサブツリーの並列計算
   - マルチスレッド対応

## ベンチマーク例

```rust
// 100ノードのUIで1ノードのみ変更
// 従来: 100ノード全て再計算 = 100%
// 差分: 1ノードのみ再計算 = 1%
// → 99%の計算を削減
```

## まとめ

レイアウト差分計算システムは、変更検出とキャッシュを組み合わせることで、UIレイアウトのパフォーマンスを大幅に向上させます。特に大規模なUIや部分的な更新が多いアプリケーションで効果を発揮します。

---

実装: `src/ui/layout_diff.rs`  
統合: `src/ui/mod.rs`, `src/engine/state.rs`
