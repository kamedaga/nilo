# Nilo言語仕様 (Nilo Language Specification)

## 概要 (Overview)

Niloは宣言的UIフレームワーク用のDSL（Domain Specific Language）です。アプリケーションの画面遷移（Flow）、UI画面（Timeline）、再利用可能なコンポーネント（Component）を効率的に記述することができます。

## 基本構造 (Basic Structure)

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
```nilo
hello_world
my-component
button123
```
- 英数字、アンダースコア（_）、ハイフン（-）が使用可能

### 1.3 文字列リテラル (String Literals)
```nilo
"Hello, World!"
"""複数行の
文字列も可能"""
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

```nilo
flow {
    start: MainMenu
    MainMenu -> [GameScreen, Settings]
    GameScreen -> [MainMenu, GameOver]
    GameOver -> [MainMenu, GameScreen]
}
```

#### 構文 (Syntax)
- `start: <timeline_name>` - 開始画面を指定
- `<from> -> [<to1>, <to2>, ...]` - 遷移可能な画面を指定

### 3.2 Timeline定義 (Timeline Definition)

Timeline定義は各画面のUIレイアウトとインタラクションを記述します。

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
height: 200px
min_width: 50px
max_width: 300px
min_height: 100vh
max_height: 80vh

padding: 10px              // 全方向
padding: "10px 20px"       // 垂直 水平
padding: "10px 15px 20px 25px"  // 上 右 下 左

margin: 5px
margin_top: 10px
margin_bottom: 15px
margin_left: 8px
margin_right: 12px
```

#### フォント (Fonts)
```nilo
font_size: 16px
font_weight: "bold"        // "normal", "bold"
font_family: "Arial"
text_align: "center"       // "left", "center", "right"
```

#### レイアウト (Layout)
```nilo
align: "center"            // "start", "center", "end"
gap: 10px                  // 子要素間の間隔
```

#### 装飾 (Decoration)
```nilo
rounded: 8px               // 角丸
border: "2px solid #000"   // ボーダー
shadow: "2px 2px 4px rgba(0,0,0,0.3)"  // 影
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
circle(x: 100, y: 100, radius: 50)
quad(x: 0, y: 0, width: 200, height: 100)
triangle(x1: 0, y1: 0, x2: 100, y2: 0, x3: 50, y3: 100)
roundedrect(x: 10, y: 10, width: 180, height: 80, radius: 10)

// 画像とテキスト
image(path: "texture.png", x: 0, y: 0)
text(content: "Hello", x: 50, y: 50, size: 16)
```

## 12. 完全な例 (Complete Examples)

### 12.1 シンプルなアプリケーション

```nilo
flow {
    start: WelcomeScreen
    WelcomeScreen -> [MainApp]
    MainApp -> [WelcomeScreen]
}

timeline WelcomeScreen {
    VStack(style: {
        padding: 40px,
        align: "center",
        background: "#f8f9fa"
    }) {
        Text("Niloへようこそ", style: {
            font_size: 32px,
            font_weight: "bold",
            color: "#212529"
        })
        
        Spacing(20)
        
        Text("モダンなUIフレームワーク", style: {
            font_size: 18px,
            color: "#6c757d"
        })
        
        Spacing(40)
        
        Button(
            id: start_btn,
            label: "開始",
            style: {
                background: "#007bff",
                color: "white",
                padding: "12px 24px",
                rounded: 6px,
                font_size: 16px
            }
        )
    }
    
    when user.click(start_btn) {
        navigate_to(MainApp)
    }
}

timeline MainApp {
    VStack {
        Text("メインアプリケーション")
        Text("カウンター: {}", state.counter)
        
        HStack(style: { gap: 10px }) {
            Button(id: increment, label: "+1")
            Button(id: decrement, label: "-1")
        }
        
        Button(id: back_btn, label: "戻る")
    }
    
    when user.click(increment) {
        set counter = state.counter + 1
    }
    
    when user.click(decrement) {
        set counter = state.counter - 1
    }
    
    when user.click(back_btn) {
        navigate_to(WelcomeScreen)
    }
}
```

### 12.2 コンポーネントを使用したアプリケーション

```nilo
component TodoItem(text, completed) {
    HStack(style: {
        padding: 10px,
        background: "#ffffff",
        border: "1px solid #e9ecef",
        rounded: 4px,
        margin_bottom: 5px
    }) {
        Text("✓", style: {
            color: completed ? "#28a745" : "#dee2e6"
        })
        
        Text("{}", text, style: {
            flex: 1,
            color: completed ? "#6c757d" : "#212529",
            text_decoration: completed ? "line-through" : "none"
        })
        
        Button(id: toggle_btn, label: "切り替え", style: {
            font_size: 12px,
            padding: "4px 8px"
        })
    }
}

flow {
    start: TodoApp
    TodoApp -> [TodoApp]
}

timeline TodoApp {
    VStack(style: {
        padding: 20px,
        max_width: 400px
    }) {
        Text("TODOアプリ", style: {
            font_size: 24px,
            font_weight: "bold",
            margin_bottom: 20px
        })
        
        foreach todo in state.todos {
            TodoItem(todo.text, todo.completed)
        }
        
        if state.todos.length == 0 {
            Text("TODOがありません", style: {
                color: "#6c757d",
                text_align: "center",
                padding: 20px
            })
        }
        
        HStack(style: { margin_top: 20px, gap: 10px }) {
            Button(id: add_btn, label: "追加")
            Button(id: clear_btn, label: "完了済みを削除")
        }
    }
    
    when user.click(add_btn) {
        add_todo!("新しいTODO")
    }
    
    when user.click(clear_btn) {
        clear_completed_todos!()
    }
}
```

## 13. 文法リファレンス (Grammar Reference)

### 13.1 完全なPEST文法

```pest
WHITESPACE = _{ " " | "\t" | NEWLINE | COMMENT }
NEWLINE    = _{ "\r\n" | "\n" }
COMMENT    = _{ "//" ~ (!NEWLINE ~ ANY)* ~ NEWLINE? }

ident = @{ (ASCII_ALPHANUMERIC | "_" | "-")+ }

string           = @{ dq_string | fancy_dq_string }
dq_string        = @{ "\"" ~ ( "\\\"" | "\\\\" | (!"\""  ~ ANY) )* ~ "\"" }
fancy_dq_string  = @{ """  ~ ( "\\""  | "\\\\" | (!"""   ~ ANY) )* ~ """  }

number  = @{ "-"? ~ ASCII_DIGIT+ ~ ("." ~ ASCII_DIGIT+)? }
bool    = { "true" | "false" }

dimension_value = { number ~ unit_suffix? }
unit_suffix = { "px" | "vw" | "vh" | "%" | "rem" | "em" }

array   = { "[" ~ (expr ~ ("," ~ expr)*)? ~ "]" }
object  = { "{" ~ (object_entry ~ ("," ~ object_entry)*)? ~ "}" }
object_entry = { ident ~ ":" ~ expr }

expr    = { match_expr | string | dimension_value | number | bool | path | ident | array | object }

style_arg = { "style" ~ ":" ~ expr }

file       = { SOI ~ (flow_def | timeline_def | component_def)* ~ EOI }

flow_def   = { "flow" ~ "{" ~ start_def ~ transition_def+ ~ "}" }
start_def  = { "start" ~ ":" ~ ident }
transition_def = { ident ~ "->" ~ "[" ~ (ident ~ ("," ~ ident)*)? ~ "]" }

timeline_def = { "timeline" ~ ident ~ "{" ~ view_nodes? ~ "}" }
component_def = { "component" ~ ident ~ param_list? ~ "{" ~ view_nodes? ~ "}" }
param_list = { "(" ~ (ident ~ ("," ~ ident)*)? ~ ")" }

view_nodes = { view_node* }

view_node = _{
      vstack_node
    | hstack_node
    | text
    | button
    | image
    | dynamic_section
    | match_block
    | foreach_node
    | if_node
    | navigate_action
    | spacing_node
    | rust_call
    | component_call
    | stencil_call
    | state_set
    | state_toggle
    | list_append
    | list_remove
    | when_block
}

vstack_node = { "VStack" ~ "(" ~ style_arg? ~ ")" ~ "{" ~ view_nodes? ~ "}" }
hstack_node = { "HStack" ~ "(" ~ style_arg? ~ ")" ~ "{" ~ view_nodes? ~ "}" }

arg_item = { style_arg | expr }

text = { "Text" ~ "(" ~ expr ~ ("," ~ arg_item)* ~ ")" }

button = { "Button" ~ "("
    ~ "id" ~ ":" ~ ident
    ~ "," ~ "label" ~ ":" ~ string
    ~ ("," ~ "onclick" ~ ":" ~ rust_call)?
    ~ ("," ~ style_arg)?
    ~ ")"
}

image = { "Image" ~ "(" ~ string ~ ("," ~ arg_item)* ~ ")" }

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

stencil_call = { ident ~ "(" ~ stencil_args? ~ ")" }
stencil_args = { (stencil_arg ~ ("," ~ stencil_arg)*)? }
stencil_arg  = { ident ~ ":" ~ stencil_value }
stencil_value = { number | string | bool }

when_block = { "when" ~ event_expr ~ "{" ~ view_nodes? ~ "}" }
event_expr = { user_event }
user_event = { "user" ~ "." ~ event_kind ~ "(" ~ ident ~ ")" }
event_kind = { "click" }

path = @{ ident ~ ("." ~ ident)* }

foreach_node = { "foreach" ~ ident ~ "in" ~ expr ~ ("(" ~ style_arg? ~ ")")? ~ "{" ~ view_nodes? ~ "}" }

if_node = { "if" ~ expr ~ ("(" ~ style_arg? ~ ")")? ~ "{" ~ view_nodes? ~ "}" ~ ("else" ~ "{" ~ view_nodes? ~ "}")? }
```

## 14. ベストプラクティス (Best Practices)

### 14.1 コンポーネント設計
- 再利用可能な小さなコンポーネントを作成する
- パラメータを活用して柔軟性を持たせる
- 適切な名前付けを行う

### 14.2 スタイリング
- 一貫したデザインシステムを使用する
- 色や寸法は変数的に管理する
- レスポンシブデザインを考慮する（vw, vh単位の活用）

### 14.3 状態管理
- 状態の構造を整理する
- 必要最小限の状態を保持する
- 適切な初期値を設定する

### 14.4 パフォーマンス
- 不要な再描画を避ける
- 大きなリストには注意深く対処する
- 適切なコンポーネント分割を行う

---

この仕様書は、Nilo言語の完全な文法と機能を説明するリファレンスドキュメントです。実際の開発では、具体的な使用例とともにこの仕様を参照してください。