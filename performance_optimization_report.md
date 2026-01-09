# パフォーマンス/軽量化の観点での調査メモ

## 目的
現在のコードベースで、**特に時間がかかりやすい処理（致命的なボトルネック候補）**と、軽量化・最適化の余地がある箇所を整理しました。

---

## 致命的に時間がかかっている可能性が高い箇所

### 1) レイアウト計算の二重・多重実行（`compute_node_size`の多重呼び出し）
- `layout_single_node_recursive`や`layout_vstack_recursive`/`layout_hstack_recursive`内で、各ノードに対して**再度 `compute_node_size` を呼び出し**ています。レイアウト計算の前段階（`compute_vstack_size`/`compute_hstack_size`）でも同様にノードのサイズを計算しており、**同一ノードに対して複数回のサイズ計算が発生**しています。特に子要素の多いツリーでは致命的なオーダー増加になりやすい構造です。【F:src/ui/layout.rs†L225-L300】【F:src/ui/layout.rs†L761-L917】【F:src/ui/layout.rs†L1520-L1972】
- **改善方針例**
  - レイアウトフェーズで計算したサイズをキャッシュし、`layout_*` で再利用する（`node id + style hash + context` 等をキーにしたメモ化）。
  - `compute_node_size` が多重呼び出しにならないよう、レイアウト計算とサイズ計算の統合（1パス化）を検討。

### 2) `HStack` の2パス計算での重複サイズ計算
- `compute_hstack_size` では固定幅の子要素を一度計算した後、**固定幅要素について再度 `compute_node_size` を呼んで高さ等を取得**しています。固定幅要素は**最低2回計算**されるため、子要素数が増えるほどコストが増大します。【F:src/ui/layout.rs†L809-L938】
- **改善方針例**
  - 1回目の計算結果を `child_sizes` に保持し、2回目は使い回す（高さやintrinsic系を再計算しない）。

### 3) `ForEach` の展開・評価がアイテム数×ノード数で重くなる
- `compute_foreach_size` と `layout_foreach_recursive` の両方で `iterable` を評価し、**配列のパースや要素展開を都度実行**しています。さらに `process_foreach_node_recursive` では `Text` などのノードを `Box::leak` で都度生成し、**ノードのクローンやJSONパースが大量に発生**します。【F:src/ui/layout.rs†L984-L1078】【F:src/ui/layout.rs†L1975-L2264】
- `expand_foreach_variables` 内でも JSON 文字列に対して `serde_json::from_str` を実行しており、**各アイテムで再パース**されます。【F:src/ui/layout.rs†L2169-L2264】
- **改善方針例**
  - `iterable` の評価結果を一度だけ解析し、サイズ計算とレイアウトで共通利用する。
  - JSONのパース結果をキャッシュし、同一文字列に対する再パースを避ける。
  - `Box::leak` で生成したノードの使い回し、またはテンプレート化して差分のみ評価する。

### 4) テキスト測定のロック・測定コストの集中
- `compute_text_size`/`compute_button_size` から `measure_text` が頻繁に呼び出され、`TextMeasurementSystem` の **`Mutex` をロックして毎回測定**します。キャッシュはありますが、**同一テキスト・同一スタイルでない場合は毎回測定**が走るため、テキスト数が多いUIでは支配的になります。【F:src/ui/layout.rs†L505-L649】【F:src/ui/layout.rs†L1449-L1466】【F:src/ui/text_measurement.rs†L164-L203】
- **改善方針例**
  - レイアウトパス内でテキスト計測結果をキャッシュ（`LayoutEngine`側に短期キャッシュを持つ）。
  - `compute_node_size` の多重呼び出しを削減することでテキスト測定回数を削減。

---

## 軽量化・最適化の余地がある箇所

### A) `VStack`/`HStack` サイズ計算時のオブジェクトクローンの削減
- `compute_vstack_size` で `child_size.clone()` を `child_sizes` に保存しているため、計算結果のコピーが増えます。必要なフィールドだけ保持するなどで軽量化可能です。【F:src/ui/layout.rs†L761-L807】

### B) `layout_vstack_recursive` の再計算
- `layout_vstack_recursive` 内でも `compute_node_size` を全子要素に対して実行しています。もし前段でサイズ計算済みであればキャッシュ利用で再計算を回避できます。【F:src/ui/layout.rs†L1735-L1847】

---

## まとめ
- 現状の**致命的ボトルネック候補**は「レイアウト計算の多重化」「ForEach展開の重複」「テキスト計測の集中」です。特に `compute_node_size` の多重呼び出しが、**レイアウト全体の時間増大に直結**している可能性が高いです。【F:src/ui/layout.rs†L225-L300】【F:src/ui/layout.rs†L1520-L1972】
- まずは**サイズ計算のキャッシュ化・1パス化**と、**ForEach展開の共通化/キャッシュ**を優先的に検討すると効果が高いはずです。【F:src/ui/layout.rs†L984-L1078】【F:src/ui/layout.rs†L1975-L2264】
