# Nilo言語仕様 (Nilo Language Specification)
(現在動作確認中です)

## 概要 (Overview)

Niloは宣言的UIフレームワーク用のDSL（Domain Specific Language）です。アプリケーションの画面遷移（Flow）、UI画面（Timeline）、再利用可能なコンポーネント（Component）を効率的に記述することができます。

## 基本構成 (Basic Structure)

Niloアプリケーションは以下の要素で構成されます：

- **Flow定義**: アプリケーションの画面遷移を定義
- **Timeline定義**: 各画面のUIレイアウトとロジックを定義  
- **Component定義**: 再利用可能なUIコンポーネントを定義

## 1. 基本文法 (Basic Syntax)

### 1.1 コメント (Comments)
```nilo
// 単行コメント
```

### 1.2 識別子 (Identifiers)

#### 基本識別子 (Basic Identifiers)
```nilo
hello_world
my-component
button123
```
- 英数字、アンダースコア（_）、ハイフン（-）が使用可能

#### 修飾識別子 (Qualified Identifiers)
```nilo
MainFlow::StartScreen
GameFlow::Level1::Boss
Settings::Audio::Volume
```
- 名前空間を表現するために`::`で区切った識別子
- 階層的フロー糖衣構文やタイムライン参照で使用

### 1.3 文字列リテラル (String Literals)

#### 通常の文字列
```nilo
"Hello, World!"
"日本語文字列"
```

#### 三重引用符文字列
```nilo
"""
複数行にわたる
長い文字列を記述できます
エスケープも簡単: "quotes"
"""
```

### 1.4 数値リテラル (Number Literals)
```nilo
42
3.14
-10
```

### 1.5 真偽値リテラル (Boolean Literals)
```nilo
true
false
```

## 2. データ型と式 (Data Types and Expressions)

### 2.1 基本データ型 (Basic Data Types)

#### 文字列 (String)
```nilo
"Hello"
"世界"
"""多行文字列
改行も含められます"""
```

#### 数値 (Number)
```nilo
42
3.14159
-100
```

#### 真偽値 (Boolean)
```nilo
true
false
```

#### 寸法値 (Dimension Value)
```nilo
10px        // ピクセル
50%         // パーセント
100vw       // ビューポート幅
100vh       // ビューポート高
2rem        // rem単位
1.5em       // em単位
```

### 2.2 複合データ型 (Composite Data Types)

#### 配列 (Arrays)
```nilo
[1, 2, 3]
["apple", "banana", "cherry"]
[true, false, true]
```

#### オブジェクト (Objects)
```nilo
{
    name: "John",
    age: 30,
    active: true
}
```

### 2.3 パス式 (Path Expressions)
```nilo
state.user.name
config.theme.color
```

### 2.4 match式 (Match Expressions)
```nilo
match state.theme {
    case "dark" { "#000000" }
    case "light" { "#ffffff" }
    default { "#888888" }
}
```

## 3. アプリケーション構造 (Application Structure)

### 3.1 Flow定義 (Flow Definition)

Flow定義はアプリケーションの画面遷移を記述します。

#### 基本的なフロー定義
```nilo
flow {
    start: MainMenu
    MainMenu -> [GameScreen, Settings]
    GameScreen -> [MainMenu, GameOver]
    GameOver -> [MainMenu, GameScreen]
}
```

#### 階層的フロー糖衣構文 (Namespaced Flow Syntax)
```nilo
flow GameFlow {
    start: MainMenu
    MainMenu -> [Level1, Settings]
    Level1 -> [Level2, MainMenu]
    Level2 -> [Boss, Level1]
    Boss -> [Victory, GameOver]
    Victory -> MainMenu
    GameOver -> [MainMenu, Level1]
}
```

#### 修飾識別子を使った遷移
```nilo
flow {
    start: GameFlow::MainMenu
    GameFlow::MainMenu -> [GameFlow::Level1, Settings::Audio]
    GameFlow::Level1 -> [GameFlow::Level2, GameFlow::MainMenu]
    Settings::Audio -> GameFlow::MainMenu
}
```

#### 複数ソース・複数ターゲット遷移
```nilo
flow {
    start: MainMenu
    [MainMenu, PauseMenu] -> GameScreen
    GameScreen -> [PauseMenu, GameOver, Victory]
    [GameOver, Victory] -> [MainMenu, GameScreen]
}
```

### 3.2 Timeline定義 (Timeline Definition)

Timeline定義は各画面のUIレイアウトとインタラクションを記述します。

#### 基本的なタイムライン
```nilo
timeline MainMenu {
    VStack {
        Text("メインメニュー")
        Button(id: start_btn, label: "ゲーム開始")
        Button(id: settings_btn, label: "設定")
    }
    
    when user.click(start_btn) {
        navigate_to(GameScreen)
    }
}
```

#### 修飾名を持つタイムライン
```nilo
timeline GameFlow::Level1 {
    VStack {
        Text("レベル1")
        Text("敵を倒してレベル2に進もう！")
        Button(id: next_btn, label: "次のレベル")
    }
    
    when user.click(next_btn) {
        navigate_to(GameFlow::Level2)
    }
}
```

#### タイムライン設定パラメータ
```nilo
timeline Settings::Audio (style: { background: "#f8f9fa", padding: 20px }) {
    VStack {
        Text("音声設定")
        // 設定項目...
    }
}
```

#### フォント定義
```nilo
timeline MainScreen {
    font: "assets/fonts/game-font.ttf"
    
    VStack {
        Text("ゲームタイトル", style: { font_size: 32px })
        // その他のUI要素...
    }
}
```

### 3.3 Component定義 (Component Definition)

Component定義は再利用可能なUIコンポーネントを記述します。

```nilo
component PlayerCard(name, score) {
    VStack(style: {
        padding: 10px,
        background: "#f0f0f0",
        rounded: 8px
    }) {
        Text("プレイヤー: {}", name)
        Text("スコア: {}", score)
    }
}
```

## 4. UI要素 (UI Elements)

### 4.1 レイアウト要素 (Layout Elements)

#### VStack (垂直スタック)
```nilo
VStack {
    Text("上")
    Text("下")
}

VStack(style: { gap: 10px }) {
    Text("要素1")
    Text("要素2")
}
```

#### HStack (水平スタック)
```nilo
HStack {
    Text("左")
    Text("右")
}

HStack(style: { align: "center" }) {
    Text("左寄せ")
    Text("右寄せ")
}
```

### 4.2 基本UI要素 (Basic UI Elements)

#### Text (テキスト)
```nilo
Text("固定テキスト")
Text("Hello, {}!", state.user.name)
Text("数値: {}", state.counter, style: {
    color: "#ff0000",
    font_size: 18px
})
```

#### Button (ボタン)
```nilo
Button(id: my_button, label: "クリック")

Button(
    id: styled_button, 
    label: "スタイル付きボタン",
    onclick: my_function!(),
    style: {
        background: "#007bff",
        color: "white",
        padding: "10px 20px",
        rounded: 5px
    }
)
```

#### TextInput (テキスト入力)
```nilo
TextInput("input_id", style: {
    width: 200px,
    padding: 8px,
    border: "1px solid #ccc"
})

TextInput(state.user_input, style: {
    background: "#f8f9fa",
    font_size: 16px
})
```

#### Image (画像)
```nilo
Image("path/to/image.png")

Image("logo.png", style: {
    width: 100px,
    height: 100px
})
```

### 4.3 スペーシング (Spacing)

```nilo
Spacing(20)      // 20ピクセルの固定スペース
SpacingAuto      // 自動スペース
```

## 5. スタイルシステム (Style System)

### 5.1 スタイル指定方法 (Style Specification)

```nilo
Text("スタイル付きテキスト", style: {
    color: "#ff0000",
    font_size: 18px,
    background: "#f0f0f0",
    padding: 10px,
    margin: 5px,
    rounded: 8px
})
```

### 5.2 利用可能なスタイルプロパティ (Available Style Properties)

#### 色 (Colors)
```nilo
color: "#ff0000"           // 16進数
color: "red"               // 色名
background: "#00ff00"      // 背景色
border_color: "blue"       // ボーダー色
```

#### サイズと間隔 (Size and Spacing)
```nilo
width: 100px
width: 50vw                // ビューポート幅の50%
height: 200px
height: 100vh              // ビューポート高の100%

// 絶対値でのサイズ指定
size: [200px, 100px]

// 相対単位でのサイズ指定
width: 80vw
height: 60vh

padding: 10px              // 全方向
padding: "10px 20px"       // 垂直 水平
margin: 5px
margin: [10px, 15px, 20px, 25px]  // 上 右 下 左

// 相対単位での間隔指定
padding: 2rem
margin: 1.5em
gap: 5vw
```

#### フォント (Fonts)
```nilo
font_size: 16px
font_size: 1.2rem          // 相対単位
font_weight: "bold"        // "normal", "bold"
font_family: "Arial"
text_align: "center"       // "left", "center", "right"
```

#### レイアウト (Layout)
```nilo
align: "center"            // "start", "center", "end"
justify_content: "center"  // "flex-start", "center", "flex-end"
align_items: "center"      // "flex-start", "center", "flex-end"
gap: 10px                  // 子要素間の間隔
spacing: 15px              // gap のエイリアス
```

#### 装飾 (Decoration)
```nilo
rounded: 8px               // 角丸
border: "2px solid #000"   // ボーダー
shadow: {                  // 影の詳細設定
    blur: 8px,
    offset: [2px, 2px],
    color: "rgba(0,0,0,0.3)"
}
shadow: true               // 標準の影
```

## 6. 制御構造 (Control Structures)

### 6.1 条件分岐 (Conditional)

#### if文
```nilo
if state.is_logged_in {
    Text("ログイン済み")
} else {
    Text("ログインしてください")
}

// スタイル付きif
if state.error (style: { background: "#ffcccc", padding: 10px }) {
    Text("エラーが発生しました")
}
```

### 6.2 繰り返し (Loops)

#### foreach文
```nilo
foreach item in state.items {
    Text("アイテム: {}", item)
}

foreach user in state.users {
    PlayerCard(user.name, user.score)
}

// スタイル付きforeach
foreach color in ["red", "green", "blue"] (style: { margin: 5px }) {
    Text("色: {}", color, style: { color: color })
}
```

### 6.3 パターンマッチング (Pattern Matching)

```nilo
match state.status {
    case "loading" {
        Text("読み込み中...")
    }
    case "success" {
        Text("完了")
    }
    case "error" {
        Text("エラー")
    }
    default {
        Text("不明な状態")
    }
}

// スタイル付きmatch
match state.theme (style: { padding: 20px }) {
    case "dark" {
        VStack(style: { background: "#000" }) {
            Text("ダークテーマ", style: { color: "white" })
        }
    }
    case "light" {
        VStack(style: { background: "#fff" }) {
            Text("ライトテーマ", style: { color: "black" })
        }
    }
}
```

## 7. イベント処理 (Event Handling)

### 7.1 when文 (When Statements)

```nilo
when user.click(button_id) {
    navigate_to(NextScreen)
}

when user.click(increment_btn) {
    set counter = state.counter + 1
}
```

### 7.2 利用可能なイベント (Available Events)

#### ユーザーイベント (User Events)
```nilo
user.click(button_id)      // ボタンクリック
```

## 8. 状態管理 (State Management)

### 8.1 状態の参照 (State Access)
```nilo
Text("現在の値: {}", state.counter)
Text("ユーザー名: {}", state.user.name)
```

### 8.2 状態の変更 (State Modification)

#### 値の設定 (Set Value)
```nilo
set counter = 10
set user.name = "新しい名前"
set config.theme = "dark"
```

#### 真偽値の切り替え (Toggle Boolean)
```nilo
is_visible = !is_visible
settings.dark_mode = !settings.dark_mode
```

#### リストの操作 (List Operations)
```nilo
// リストに要素を追加
items.append("新しいアイテム")
users.append({ name: "太郎", score: 100 })

// インデックスで要素を削除
items.remove(0)          // 最初の要素を削除
users.remove(2)          // 3番目の要素を削除
```

## 9. アクション (Actions)

### 9.1 画面遷移 (Navigation)
```nilo
navigate_to(GameScreen)
navigate_to(MainMenu)
navigate_to(GameFlow::Level2)  // 修飾名での遷移
```

### 9.2 Rust関数呼び出し (Rust Function Calls)
```nilo
// 引数なし
save_game!()

// 引数あり
load_user_data!(state.user_id)
calculate_score!(state.level, state.time)

// ボタンのonclickイベント
Button(id: save_btn, label: "保存", onclick: save_game!())
```

### 9.3 コンポーネント呼び出し (Component Calls)
```nilo
// パラメータなし
MyComponent()

// パラメータあり
PlayerCard("太郎", 1500)
UserProfile(state.current_user.name, state.current_user.avatar)

// スタイル付きコンポーネント
MyCard("タイトル", "内容", style: {
    background: "#f8f9fa",
    border: "1px solid #dee2e6"
})
```

## 10. 動的セクション (Dynamic Sections)

```nilo
dynamic_section content {
    // 動的に生成される内容
}

dynamic_section sidebar (style: { width: 200px }) {
    // サイドバーの内容
}
```

## 11. Stencil低レベルグラフィック (Stencil Low-level Graphics)

Niloでは低レベルのグラフィック描画にStencilシステムを使用できます。

```nilo
// 基本図形
circle(x: 100, y: 100, radius: 50, r: 1.0, g: 0.0, b: 0.0, a: 1.0)
rect(x: 0, y: 0, width: 200, height: 100, r: 0.5, g: 0.5, b: 0.5, a: 1.0)
triangle(x1: 0, y1: 0, x2: 100, y2: 0, x3: 50, y3: 100, r: 0.0, g: 1.0, b: 0.0, a: 1.0)
rounded_rect(x: 10, y: 10, width: 180, height: 80, radius: 10, r: 0.8, g: 0.8, b: 0.8, a: 1.0)

// 画像とテキスト
image(path: "texture.png", x: 0, y: 0, width: 100, height: 100)
text(content: "Hello", x: 50, y: 50, size: 16, r: 0.0, g: 0.0, b: 0.0, a: 1.0)
```


## 13. 文法リファレンス (Grammar Reference)

### 13.1 完全なPEST文法

```pest
WHITESPACE = _{ " " | "\t" | NEWLINE | COMMENT }
NEWLINE    = _{ "\r\n" | "\n" }
COMMENT    = _{ "//" ~ (!NEWLINE ~ ANY)* ~ NEWLINE? }

ident = @{ (ASCII_ALPHANUMERIC | "_" | "-")+ }

// 階層的フロー糖衣構文用の修飾された識別子
qualified_ident = @{ ident ~ ("::" ~ ident)* }

// 拡張文字列リテラル（通常の引用符と三重引用符）
string           = @{ dq_string | fancy_dq_string }
dq_string        = @{ "\"" ~ ( "\\\"" | "\\\\" | (!"\""  ~ ANY) )* ~ "\"" }
fancy_dq_string  = @{ "\"\"\"" ~ ( "\\\"" | "\\\\" | (!"\"\"\"" ~ ANY) )* ~ "\"\"\"" }

number  = @{ "-"? ~ ASCII_DIGIT+ ~ ("." ~ ASCII_DIGIT+)? }
bool    = { "true" | "false" }

// 相対単位対応の寸法値
dimension_value = { number ~ unit_suffix? }
unit_suffix = { "px" | "vw" | "vh" | "%" | "rem" | "em" }

array   = { "[" ~ (expr ~ ("," ~ expr)*)? ~ "]" }
object  = { "{" ~ (object_entry ~ ("," ~ object_entry)*)? ~ "}" }
object_entry = { ident ~ ":" ~ expr }

expr    = { match_expr | string | dimension_value | number | bool | path | ident | array | object }

style_arg = { "style" ~ ":" ~ expr }

file       = { SOI ~ (flow_def | namespaced_flow_def | timeline_def | component_def)* ~ EOI }

// 基本フロー定義
flow_def   = { "flow" ~ "{" ~ start_def ~ transition_def+ ~ "}" }
// 階層的フロー定義（糖衣構文)
namespaced_flow_def = { "flow" ~ ident ~ "{" ~ namespaced_start_def ~ namespaced_transition_def+ ~ "}" }

start_def  = { "start" ~ ":" ~ qualified_ident }
namespaced_start_def = { "start" ~ ":" ~ ident }

// 複数ソース・複数ターゲット対応遷移定義
transition_def = { transition_source ~ "->" ~ (qualified_ident | ("[" ~ (qualified_ident ~ ("," ~ qualified_ident)*)? ~ "]")) }
namespaced_transition_def = { namespaced_transition_source ~ "->" ~ (transition_target | ("[" ~ (transition_target ~ ("," ~ transition_target)*)? ~ "]")) }

transition_source = { qualified_ident | ("[" ~ qualified_ident ~ ("," ~ qualified_ident)* ~ "]") }
namespaced_transition_source = { ident | ("[" ~ ident ~ ("," ~ ident)* ~ "]") }
transition_target = { qualified_ident | ident }

// タイムライン・コンポーネント定義
timeline_def = { "timeline" ~ qualified_ident ~ timeline_config? ~ "{" ~ font_def? ~ view_nodes? ~ "}" }
timeline_config = { "(" ~ timeline_param ~ ("," ~ timeline_param)* ~ ")" }
timeline_param = { "style" ~ ":" ~ expr }

font_def = { "font" ~ ":" ~ string }

component_def = { "component" ~ ident ~ param_list? ~ "{" ~ font_def? ~ view_nodes? ~ "}" }
param_list = { "(" ~ (ident ~ ("," ~ ident)*)? ~ ")" }

view_nodes = { view_node* }

view_node = _{
      vstack_node
    | hstack_node
    | text
    | button
    | text_input
    | image
    | dynamic_section
    | match_block
    | foreach_node
    | if_node
    | navigate_action
    | spacing_node
    | state_set
    | state_toggle
    | list_append
    | list_remove
    | when_block
    | rust_call
    | component_call
    | stencil_call
}

vstack_node = { "VStack" ~ "(" ~ style_arg? ~ ")" ~ "{" ~ view_nodes? ~ "}" }
hstack_node = { "HStack" ~ "(" ~ style_arg? ~ ")" ~ "{" ~ view_nodes? ~ "}" }

arg_item = { style_arg | expr }

text = { "Text" ~ "(" ~ expr ~ ("," ~ arg_item)* ~ ")" }

button = { "Button" ~ "("
    ~ "id" ~ ":" ~ WHITESPACE* ~ (ident | string)
    ~ "," ~ WHITESPACE* ~ "label" ~ ":" ~ WHITESPACE* ~ string
    ~ ("," ~ WHITESPACE* ~ "onclick" ~ ":" ~ WHITESPACE* ~ rust_call)?
    ~ ("," ~ WHITESPACE* ~ style_arg)?
    ~ ")"
}

image = { "Image" ~ "(" ~ string ~ ("," ~ arg_item)* ~ ")" }

text_input = { "TextInput" ~ "(" ~ expr ~ ("," ~ arg_item)* ~ ")" }

dynamic_section = { "dynamic_section" ~ ident ~ "(" ~ style_arg? ~ ")" ~ "{" ~ view_nodes? ~ "}" }

match_expr = {
    "match" ~ expr ~ "{"
    ~ expr_match_arm* ~ expr_default_arm? ~ "}"
}

match_block = {
    "match" ~ expr ~ ("(" ~ style_arg? ~ ")")? ~ "{"
    ~ match_arm* ~ default_arm? ~ "}"
}
match_arm = { "case" ~ expr ~ "{" ~ view_nodes? ~ "}" }
default_arm = { "default" ~ "{" ~ view_nodes? ~ "}" }

expr_match_arm = { "case" ~ expr ~ "{" ~ expr ~ "}" }
expr_default_arm = { "default" ~ "{" ~ expr ~ "}" }

navigate_action = { "navigate_to" ~ "(" ~ ident ~ ")" }

spacing_node = { "Spacing" ~ "(" ~ number? ~ ")" | "SpacingAuto" }

rust_call = { ident ~ "!" ~ "(" ~ (arg_item ~ ("," ~ arg_item)*)? ~ ")" }

component_call = { ident ~ "(" ~ (arg_item ~ ("," ~ arg_item)*)? ~ ")" }

state_set    = { "set" ~ ident ~ "=" ~ expr }
state_toggle = { ident ~ "=" ~ "!" ~ ident }
list_append  = { ident ~ ".append" ~ "(" ~ expr ~ ")" }
list_remove  = { ident ~ ".remove" ~ "(" ~ number ~ ")" }

// Stencil低レベルグラフィック
stencil_call = { stencil_name ~ "(" ~ stencil_args? ~ ")" }
stencil_name = { "rect" | "circle" | "triangle" | "text" | "image" | "rounded_rect" }
stencil_args = { (stencil_arg ~ ("," ~ stencil_arg)*)? }
stencil_arg  = { ident ~ ":" ~ stencil_value }
stencil_value = { number | string | bool }

when_block = { "when" ~ event_expr ~ "{" ~ view_nodes? ~ "}" }
event_expr = { user_event }
user_event = { "user" ~ "." ~ event_kind ~ "(" ~ ident ~ ")" }
event_kind = { "click" }

foreach_node = { "foreach" ~ ident ~ "in" ~ expr ~ ("(" ~ style_arg? ~ ")")? ~ "{" ~ view_nodes? ~ "}" }
if_node = { "if" ~ expr ~ ("(" ~ style_arg? ~ ")")? ~ "{" ~ view_nodes? ~ "}" ~ ("else" ~ "{" ~ view_nodes? ~ "}")? }

path = @{ ident ~ ("." ~ ident)* }
```

## 14. ベストプラクティス (Best Practices)

### 14.1 階層的フロー設計
- 関連する画面を同じ名前空間にグループ化する
- 深い階層よりも平坦な構造を優先する
- 修飾名を使って画面間の関係を明確にする

```nilo
// 良い例：機能ごとのグループ化
flow GameFlow {
    start: MainMenu
    MainMenu -> [Level1, Settings]
    Level1 -> Level2
    Level2 -> Boss
}

flow SettingsFlow {
    start: Main
    Main -> [Audio, Video, Controls]
}
```

### 14.2 レスポンシブデザイン
- ビューポート単位（vw, vh）を活用する
- 固定値とRelative値を適切に使い分ける
- 様々な画面サイズでのテストを行う

```nilo
// 良い例：レスポンシブレイアウト
VStack(style: {
    width: 90vw,        // ビューポート幅の90%
    max_width: 800px,   // 最大幅制限
    padding: 5vw,       // レスポンシブな余白
    gap: 2vh            // ビューポート高に応じた間隔
}) {
    // コンテンツ
}
```

### 14.3 スタイリング
- 一貫したデザインシステムを使用する
- 色や寸法は変数的に管理する（将来的な機能）
- 階層化されたスタイルを活用する

```nilo
// 良い例：一貫したスタイル
Button(id: primary_btn, label: "主要アクション", style: {
    background: "#007bff",
    color: "white",
    padding: "12px 24px",
    rounded: 8px,
    font_weight: "bold"
})

Button(id: secondary_btn, label: "副次アクション", style: {
    background: "transparent",
    color: "#007bff",
    padding: "12px 24px",
    rounded: 8px,
    border: "1px solid #007bff"
})
```

### 14.4 パフォーマンス
- 大きなリストには注意深く対処する
- 不要な再描画を避ける設計を心がける
- 適切なコンポーネント分割を行う

---

この仕様書は、Nilo言語の最新の機能を含む完全な文法と機能を説明するリファレンスドキュメントです。階層的フロー糖衣構文、修飾識別子、相対単位システム、TextInput要素などの新機能により、より柔軟で表現力豊かなUIアプリケーションの開発が可能になりました。
