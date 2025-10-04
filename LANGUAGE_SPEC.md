
# Nilo DSL 仕様書

> **想定読者**: Nilo の DSL を書く開発者／パーサ・実行系の実装者。  
> **互換性**: 本仕様は後方互換を優先しつつ拡張予定。未実装の糖衣構文は注記を付す。

---

## 0. ファイル・基本
- 拡張子: `.nilo` / 文字コード: UTF‑8 / 改行: LF 推奨
- コメント: `//` 以降は行末まで無視（複数行コメントなし）
- 先頭/末尾の空白は無視。トレーリングカンマは不可。

---

## 1. 文法（Grammar）

### 1.1 型とリテラル
- **Number**: 整数/小数（先頭に `-` 可）例: `42`, `3.14`, `-10`
- **String**: `"..."` / `"""...複数行..."""`
- **Boolean**: `true` / `false`
- **Dimension**: `px`, `%`, `vw`, `vh`, `rem`, `em` 例: `10px`, `50%`, `1.2rem`

### 1.2 識別子
- 文字: `[A-Za-z][A-Za-z0-9_-]*` を推奨（厳格性は実装依存）
- 名前空間付き: `Name::Sub::Leaf`（`::` 区切り）

### 1.4 式（Expression）
- リテラル / 識別子 / **パス式** `state.user.name`
- 配列 `[e1, e2, ...]` / オブジェクト `{ key: expr, ... }`
- 二項算術 `+ - * /` （例: `state.count + 1`）
- 関数呼び出し糖衣 `rust_func!(arg1, {...})`（§8）

### 1.5 ブロック（Block）
- `{ ... }` 内に「ビュー要素」または「アクション文」を列挙
- 制御ブロック: `if ... {}`, `foreach v in xs {}`, `match e { case ... {} default {} }`
- イベント: `when user.click(id) { ... }`

---

## 2. ビュー定義（Views）

> 全てのビューは末尾に `style: { ... }` を取れる。未指定は実装デフォルト。

### 2.1 テキスト
```
Text("<format>", arg1?, arg2?, ..., style?)
```
- `"<format>"` 内の `{}` を後続引数で差し込み。
- 例: `Text("Score: {}", state.score, style: { font_size: 18 })`

### 2.2 ボタン
```
Button(<id>, "<label>", onclick?, style?)
```
- `<id>` は識別子 or 文字列。イベントは `when user.click(<id>) { ... }`。
- `onclick` に `func_name!()` で Rust コール（任意、§8）。

### 2.3 テキスト入力
```
TextInput(<id_or_state_path>, style?)
```
- 現行は **IDベース** 取得を前提（将来: state 双方向バインド）。

### 2.4 画像
```
Image("<path>", style?)
```
- サイズは `style.size` か `style.width/height` で指定。未指定時は実装既定。

### 2.5 レイアウト
```
VStack(style?) { <children...> }
HStack(style?) { <children...> }
Spacing(<px>)
SpacingAuto
```
- `spacing` / `align` などは §4。

### 2.6 コンポーネント呼び出し
```
ComponentName(arg1?, arg2?, ..., style?)
```
- 事前に `component` 定義が必要（§6.3）。

### 2.7 動的セクション
```
dynamic_section <name>(style?) { <children...> }
```
- 差し替え頻度が高い領域の囲い。ランタイムが外側から更新。

### 2.8 低レベル描画（Stencil）
```
rect(x, y, width, height, color?, scroll?, depth?)
circle(x, y, radius, color?, scroll?, depth?)
triangle(x1, y1, x2, y2, x3, y3, color?, scroll?, depth?)
rounded_rect(x, y, width, height, radius?, color?, scroll?, depth?)
text("<content>", x, y, size?, color?, font?, scroll?, depth?)
image(x, y, width, height, "<path>", scroll?, depth?)
```
- **絶対座標** 描画（親レイアウトの原点に従わない）。
- `depth`: `0.0`（最前）～ `1.0`（背面）。`text` 既定は前面寄り。

---

## 3. スタイル（Style）

> `style: { key: value, ... }` 形式。未知キーは無視。

| キー | 値例 | 説明 |
|---|---|---|
| `color` | `"#333"`, `"red"`, `[1,0,0,1]` | 文字/前景色 |
| `background` | `"#f7f7f7"` | 背景色 |
| `border_color` | `"#ddd"` | 枠線色（将来） |
| `font_size` | `16`, `14px`, `1.2rem` | 文字サイズ |
| `font` | `"Arial"`, `"assets/fonts/ui.ttf"` | フォント指定（タイムライン直下でも可） |
| `padding` | `12` / `[8,12]` / `[8,12,8,12]` / `{ top: 8, left: 12, ... }` | 内側余白 |
| `margin` | 同上 | 外側余白 |
| `width` | `240`, `"50%"`, `20vw` | 固定/相対幅 |
| `height` | `48`, `"40%"`, `20vh` | 固定/相対高 |
| `size` | `[width, height]` | 一括サイズ（相対長は非推奨） |
| `align` | `"left"|"center"|"right"|"top"|"bottom"` | 並びの揃え |
| `rounded` | `true` / `false` / `8` | 角丸（既定 ~8px） |
| `shadow` | `true` / `{ blur: 8, offset: [0,2], color: "#0003" }` | 影 |
| `card` | `true/false` | カード風プリセット（既定 padding 20px） |
| `spacing` | `0..` / `px/%/...` | 子要素間隔（V/HStack用） |
| `hover` | `{ background: "#0056b3" }` | ホバー時差し替え |

---

## 4. レイアウトモデル

- **VStack**: 上→下に追加。既定は左上揃え。`align` で左右揃え変更。
- **HStack**: 左→右に追加。`align` で上下揃え。
- **spacing**: 隣接子要素の間隔を一律に確保。`Spacing(n)` は明示的空白。
- **SpacingAuto**: いまは固定既定間隔として動作（将来: 伸縮空白）。
- **重なり順**: 通常は **記述順が後ほど前面**。Stencil は `depth` で制御。

---

## 5. Flow / Timeline

### 5.1 Flow（遷移グラフ）
```
flow {
  start: Main
  Main -> [Game, Settings]
  [Game, Settings] -> Result
}
```
- `start` に初期タイムライン名。
- `A -> B` で A から B へ遷移許可。`[A,B] -> C` の束ね表記可。

### 5.2 Timeline（画面）
```
timeline Main {
  font: "assets/fonts/ui.ttf"   // 任意: 既定フォント

  VStack(style: { spacing: 12px, align: "center" }) {
    Text("Hello, {}", state.user.name)
    Button(start_btn, "Start")
  }

  when user.click(start_btn) {
    navigate_to(Game)
  }
}
```
- `timeline <Name> { ... }`。Flow の状態名と一致させる。

### 5.3 アクション
- `navigate_to(TargetState)` — Flow で許可された遷移のみ成功
- 状態操作（§6.2）
- Rust 関数呼び出し `func!()`（§8）

---

## 6. 制御構文 / コンポーネント / データ

### 6.1 制御
```
if state.logged_in {
  Text("Welcome")
} else {
  Text("Please sign in")
}

foreach item in state.items (style: { spacing: 6px }) {
  Text("• {}", item.title)
}

match state.route {
  case "home" { Text("Home") }
  case "settings" { Text("Settings") }
  default { Text("Not Found") }
}
```

### 6.2 状態操作（State Ops）
- `set state.path.to.field = <expr>`
- トグル: `state.flag = !state.flag`（同一パス前提）
- 参照透過の関数形式:
    - `append(state.items, <value>)`
    - `remove(state.items, <index>)`（0 起算）
    - `clear(state.items)`

> 状態変更は即座に UI 再評価の対象。存在しないフィールドはエラー。

### 6.3 コンポーネント
```
component Message(name, body) {
  VStack(style: { spacing: 4px }) {
    Text("{}:", name, style: { font: "Arial", font_size: 14 })
    Text("{}", body)
  }
}
```
- 呼び出し: `Message("Sakura", "おはよう")`
- `when` を内部に書けるが、現行は **親 Timeline でイベントを拾う** 設計を推奨。

---

## 7. 低レベル描画（Stencil 詳細）

- 列挙: `Rect`, `Circle`, `Triangle`, `RoundedRect`, `Text`, `Image`
- 主フィールド（概念）:
    - 位置: `position` or `center` / サイズ: `width/height` or `radius`
    - `color [r,g,b,a]` / `font` / `size`（文字サイズ）
    - `scroll: bool`（スクロール追随）/ `depth: f32`
- 色指定は `"#RRGGBB"`, 色名, `[r,g,b,a]` を受容。

> レイアウト計算には **サイズのみ** 影響（位置は絶対座標で描画）。

---

## 8. Rust 連携

### 8.1 登録
```rust
use nilo::engine::rust_call::register_rust_call;
use nilo::parser::ast::Expr;

fn hello_world(args: &[Expr]) {
    info!("Hello from Rust! Args: {:?}", args);
}

register_rust_call("hello_world", hello_world);
```

### 8.2 呼び出し（DSL 側）
```
hello_world!("ping", { k: 1 })
```
- 受け側は `&[Expr]` をパースして利用。
- **状態を書き換える** 関数は `register_state_accessible_call`（実装依存）。

---

## 9. ツール / 開発支援

### 9.1 ホットリロード
- 実行時オプション `--hotreload`（または API で有効化）。
- `.nilo` 保存で自動リロード。**State は可能な限り維持**。

### 9.2 Lint（代表例）
- 未定義 Timeline 参照 / Flow 不整合 `navigate_to` / 重複定義
- 未使用 component 警告 / Button ID 衝突 / 未定義ボタン参照

### 9.3 デバッグ
- `--debug` で詳細ログ。Rust 側で `info!`, `debug!` などを活用。

---

## 10. ミニマル実例

```nilo
flow {
  start: Main
  Main -> Game
  Game -> Result
}

timeline Main {
  VStack(style: { spacing: 12px, align: "center", padding: 20 }) {
    Text("Welcome, {}", state.user.name, style: { font_size: 18 })
    Button(start_btn, "Start")
  }
  when user.click(start_btn) { navigate_to(Game) }
}

timeline Game {
  VStack(style: { spacing: 8px }) {
    Text("Score: {}", state.score)
    HStack(style: { spacing: 8px }) {
      Button(inc, "+1")
      Button(dec, "-1")
      SpacingAuto
      Button(done, "Finish")
    }
  }
  when user.click(inc) { set state.score = state.score + 1 }
  when user.click(dec) { set state.score = state.score - 1 }
  when user.click(done) { navigate_to(Result) }
}

timeline Result {
  VStack(style: { spacing: 8px, align: "center" }) {
    Text("Final: {}", state.score, style: { font_size: 22 })
    Button(back, "Back to Main")
  }
  when user.click(back) { navigate_to(Main) }
}
```

---

## 11. 既知の制約（2025-09 時点）
- `SpacingAuto` は固定扱い（伸縮空白は将来対応）
- コンポーネント内 `when` はイベント伝播未確立の実装がある
- Timeline 直下 `font` の反映は実装差あり（AST には保持）
- Stencil の座標は画面基準（親原点基準切替は将来検討）

---

## 12. 付録：EBNF（抜粋・概念）
```
File       := (Flow | Timeline | Component)*
Flow       := "flow" "{" "start:" Ident (Transition)* "}"
Transition := State "->" (State | "[" State ("," State)* "]")
State      := Ident | Qualified

Timeline   := "timeline" State "{" (TimelineDecl | View | Event)* "}"
TimelineDecl := "font:" String
Event      := "when" "user" "." "click" "(" (Ident | String) ")" Block

Component  := "component" Ident "(" ParamList? ")" Block
ParamList  := Ident ("," Ident)*

View       := Text | Button | TextInput | Image | Stack | SpacingView
           | ComponentCall | DynamicSection | Stencil
Stack      := ("VStack" | "HStack") "(" Style? ")" Block
SpacingView:= "Spacing" "(" Number ")" | "SpacingAuto"
DynamicSection := "dynamic_section" Ident "(" Style? ")" Block

Block      := "{" (View | Action | Control)* "}"
Action     := Navigate | StateOp | RustCall
Navigate   := "navigate_to" "(" State ")"
StateOp    := "set" Path "=" Expr | "append" "(" Path "," Expr ")"
            | "remove" "(" Path "," Number ")" | "clear" "(" Path ")"
RustCall   := Ident "!" "(" ArgList? ")"
Control    := If | Foreach | Match
If         := "if" Expr "(" Style? ")"? Block ("else" Block)?
Foreach    := "foreach" Ident "in" Expr "(" Style? ")"? Block
Match      := "match" Expr "(" Style? ")"? "{" (Case+ Default?) "}"
Case       := "case" Expr Block
Default    := "default" Block

Expr       := ...（リテラル/配列/オブジェクト/パス/二項算術）
Path       := "state" ("." Ident)+
Style      := "style:" Object
ArgList    := Expr ("," Expr)*
Qualified  := Ident ("::" Ident)+
Ident      := /[A-Za-z][A-Za-z0-9_-]*/
```