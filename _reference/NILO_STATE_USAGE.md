# Nilo State: Attribute Macros and Usage

このドキュメントは、Nilo で状態(State)や Rust 関数を連携させるための属性マクロの使い方をまとめたリファレンスです。内部実装のコードは掲載しません。使い方と挙動のみを簡潔に示します。

## 1) 状態の定義と初期化

- マクロ: `nilo::nilo_state!`
- 例: `State { counter: i32, name: String, ok: bool, items: Vec<i32> }`
- 初期化: `let state = State::default();`（もしくは独自のデフォルト実装）
- アプリ起動前に必ず `nilo::init_nilo_functions()` を呼び出してください（自動登録が有効になります）。

## 2) 変更監視: `#[nilo_state_watcher]`

- 目的: 特定フィールドが変更された直後に関数を呼ぶ
- シグネチャ: `fn(&mut State)`
- 例: `#[nilo_state_watcher(state = State, fields("counter", "name"))]`
- 用途: ログ出力・副作用処理など

## 3) 値の検証: `#[nilo_state_validator]`

- 目的: 特定フィールドが変更された直後に値を検証する
- シグネチャ: `fn(T) -> bool` または `fn(T) -> Result<(), String>`
- サポート型: `String, bool, 整数, 浮動小数`
- 例: `#[nilo_state_validator(state = State, field = "name")]`
- 挙動: NG の場合はエラーログ（値の差し戻しは行わない非侵襲型）

## 4) 代入ヘルパー: `#[nilo_state_assign]`

- 目的: Rust 値を Nilo の State に自動代入する関数ボディを生成
- シグネチャ: `fn(&mut State, T) -> Result<(), String>`
- サポート型: `String, &str, bool, 各整数, f32, f64`
- 例: `#[nilo_state_assign(state = State, field = "counter")]`
- 備考: リスト(Vec)への代入は対象外。リスト操作は list_* API を利用してください。

## 5) onclick 連携（安全版推奨）

### a) 安全版（推奨）: `#[nilo_safe_accessible]`
- 目的: AppState への直接アクセスを禁止し、`CustomStateContext<State>` 経由の安全な連携を行う
- ユーザー関数のシグネチャ:
  - `fn(ctx: &mut CustomStateContext<State>, args: &[Expr])`
- 属性: `#[nilo_safe_accessible(state = State, name = "func_name")]`
- 自動登録: 起動時に自動登録（ネイティブ）
- onclick 側: `onclick: func_name(...)`
- 備考: WASM では自動登録の代わりに手動登録（safe 経路）を併用してください。

### b) 互換版: `#[nilo_state_accessible]`
- 目的: 既存の onclick 実行経路（state-accessible レジストリ）と互換を保つ
- ユーザー関数のシグネチャ:
  - `fn(state: &mut AppState<S>, args: &[Expr]) where S: StateAccess`
- 属性: `#[nilo_state_accessible(state = State, name = "func_name")]`
- 注意: AppState へ直接アクセスできるため、安全な設計が必要。通常は安全版の使用を推奨。

## 6) Nilo ファイル側の書き方

- `flow { start: Main }` を定義してください
- `timeline Main { ... }` 内で `Button(..., onclick: func_name(args...))` を指定
- 例: `Button(id: "inc", label: "+1", onclick: increment_counter())`

## 7) 起動時の初期化

- `nilo::init_nilo_functions()` を最初に呼びます
  - ネイティブ環境では linkme による自動登録（関数・ウォッチャ・バリデータ）が有効になります
- カスタムフォント等のグローバル設定があればこの前後で行ってください

## 8) よくある質問（FAQ）

- Q: onclick で安全に State を触りたい
  - A: `#[nilo_safe_accessible]` を使い、`CustomStateContext` 経由で操作してください
- Q: 変更を強制的に差し戻したい
  - A: `#[nilo_state_validator]` は非侵襲型です。差し戻しを行いたい場合は、onclick 側（呼び出し元）で検証→OK のときだけ `set` する、という運用にしてください
- Q: WASM でも自動登録を使いたい
  - A: 現状はネイティブのみ自動登録対応です。WASM は手動登録 or 直接呼び出しをご利用ください

---

このドキュメントは、使い方に特化した簡易リファレンスです。詳細な API はエディタの補完や既存のサンプルコード（main.rs / demo.nilo）を参考にしてください。
