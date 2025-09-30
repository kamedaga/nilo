# Nilo言語仕様 (Nilo Language Specification)

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

VStack(style: { spacing: 10px }) {
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
        padding: 10px,
        rounded: 5px
    }
)
```

#### TextInput (テキスト入力)
```nilo
TextInput("input_id", style: {
    width: 200px,
    padding: 8px
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

## 5. 制御構造 (Control Structures)

### 5.1 条件分岐 (Conditional)

#### match ブロック
```nilo
match state.theme {
    case "dark" {
        Text("ダークテーマ", style: { color: "white" })
    }
    case "light" {
        Text("ライトテーマ", style: { color: "black" })
    }
    default {
        Text("デフォルトテーマ")
    }
}
```

#### if ブロック
```nilo
if state.is_logged_in {
    VStack {
        Text("ようこそ、{}さん", state.user.name)
        Button(id: logout_btn, label: "ログアウト")
    }
}

if state.count > 0 {
    Text("カウント: {}", state.count)
} else {
    Text("カウントがありません")
}
```

### 5.2 繰り返し処理 (Iteration)

#### foreach ブロック
```nilo
foreach item in state.items {
    VStack(style: { padding: 10px }) {
        Text("アイテム: {}", item.name)
        Text("価格: {}円", item.price)
    }
}

foreach player in state.players (style: { spacing: 5px }) {
    PlayerCard(player.name, player.score)
}
```

## 6. スタイルシステム (Style System)

### 6.1 基本スタイル指定

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

### 6.2 色指定 (Color System)

```nilo
// 16進数カラーコード
color: "#ff0000"           // 赤
color: "#ffffff"           // 白
color: "#000000"           // 黒

// 名前付きカラー
color: "red"
color: "blue"
color: "green"
color: "white"
color: "black"
color: "transparent"

// RGBA配列形式
color: [1.0, 0.0, 0.0, 1.0]     // 赤（RGBA）
color: [0.0, 1.0, 0.0, 0.5]     // 半透明の緑
```

### 6.3 サイズと位置

#### 基本サイズ指定
```nilo
width: 100px               // 固定幅
height: 200px              // 固定高さ
size: [200px, 100px]       // 幅と高さを配列で指定
```

#### 相対単位
```nilo
width: 50vw                // ビューポート幅の50%
height: 100vh              // ビューポート高の100%
font_size: 2rem            // ルート要素の2倍
padding: 1.5em             // 親要素基準
```

### 6.4 間隔とパディング

```nilo
// パディング
padding: 10px              // 全方向
padding: [10px, 15px, 20px, 25px]  // 上右下左
padding: {
    top: 10px,
    right: 15px,
    bottom: 20px,
    left: 25px
}

// マージン
margin: 5px
margin: [10px, 15px, 20px, 25px]

// スペーシング（子要素間の間隔）
spacing: 10px
spacing: 2rem
```

### 6.5 装飾

#### 角丸 (Rounded)
```nilo
rounded: true              // デフォルト角丸
rounded: 8px               // 固定値
```

#### 影 (Shadow)
```nilo
shadow: true               // デフォルト影
shadow: {
    blur: 8px,
    offset: [0px, 2px],
    color: "#000000"
}
```

#### アライメント
```nilo
align: "left"              // 左揃え
align: "center"            // 中央揃え
align: "right"             // 右揃え
align: "top"               // 上揃え
align: "bottom"            // 下揃え
```

### 6.6 ホバーエフェクト

```nilo
Button(id: btn, label: "ホバー", style: {
    background: "#007bff",
    hover: {
        background: "#0056b3"
    }
})
```

## 7. 状態操作 (State Management)

### 7.1 状態設定
```nilo
set state.user.name = "田中太郎"
set state.counter = 42
set state.is_active = true
```

### 7.2 状態トグル
```nilo
state.is_visible = !state.is_visible
state.dark_mode = !state.dark_mode
```

### 7.3 リスト操作
```nilo
// リストに要素を追加
append(state.items, "新しいアイテム")
append(state.players, { name: "プレイヤー", score: 100 })

// リストから要素を削除（インデックス指定）
remove(state.items, 0)    // 最初の要素を削除
remove(state.players, 2)  // 3番目の要素を削除

// リストをクリア（全要素を削除）
clear(state.items)        // リストを空にする
clear(state.players)      // プレイヤーリストをクリア
```

## 8. イベント処理 (Event Handling)

### 8.1 基本イベント

```nilo
when user.click(button_id) {
    navigate_to(NextScreen)
    set state.clicked = true
}
```

## 9. 動的セクション (Dynamic Sections)

```nilo
timeline GameScreen {
    VStack {
        Text("ゲーム画面")
        dynamic_section game_area {
            // この部分は実行時に動的に変更される
            Text("ゲームコンテンツ")
        }
    }
}
```

## 10. ステンシル（低レベルグラフィック）

Niloではrect、circle、triangleなどの基本図形を直接描画できます。

```nilo
rect(x: 10, y: 10, width: 100, height: 50, r: 1.0, g: 0.0, b: 0.0, a: 1.0)
circle(x: 50, y: 50, radius: 25, r: 0.0, g: 1.0, b: 0.0, a: 1.0)
triangle(x1: 0, y1: 0, x2: 50, y2: 0, x3: 25, y3: 50, r: 0.0, g: 0.0, b: 1.0, a: 1.0)
rounded_rect(x: 10, y: 10, width: 100, height: 50, radius: 8, r: 1.0, g: 1.0, b: 1.0, a: 1.0)
```

## 11. Rust関数呼び出し

```nilo
// Rust側で定義された関数を呼び出し
my_function!()
calculate!(state.value1, state.value2)
api_call!("https://api.example.com", { method: "GET" })
```

## 12. ナビゲーション

```nilo
navigate_to(TargetScreen)
navigate_to(GameFlow::Level1)
```

## 13. 完全な例

```nilo
flow GameFlow {
    start: MainMenu
    MainMenu -> [Game, Settings]
    Game -> [MainMenu, GameOver]
    GameOver -> [MainMenu, Game]
}

component PlayerCard(name, score) {
    VStack(style: {
        padding: 10px,
        background: "#f0f0f0",
        rounded: 8px,
        margin: 5px
    }) {
        Text("プレイヤー: {}", name, style: { font_size: 16px })
        Text("スコア: {}", score, style: { color: "#007bff" })
    }
}

timeline GameFlow::MainMenu {
    VStack(style: { 
        width: 90vw, 
        max_width: 600px,
        spacing: 20px,
        align: "center"
    }) {
        Text("ゲームタイトル", style: {
            font_size: 32px,
            color: "#333"
        })
        
        VStack(style: { spacing: 10px }) {
            Button(id: start_btn, label: "ゲーム開始", style: {
                background: "#007bff",
                color: "white",
                padding: 15px,
                rounded: 8px
            })
            
            Button(id: settings_btn, label: "設定", style: {
                background: "#6c757d",
                color: "white",
                padding: 15px,
                rounded: 8px
            })
        }
        
        if state.players {
            VStack(style: { spacing: 5px }) {
                Text("プレイヤー一覧")
                foreach player in state.players {
                    PlayerCard(player.name, player.score)
                }
            }
        }
    }
    
    when user.click(start_btn) {
        navigate_to(GameFlow::Game)
    }
    
    when user.click(settings_btn) {
        navigate_to(GameFlow::Settings)
    }
}

timeline GameFlow::Game {
    VStack {
        Text("ゲーム中...")
        Button(id: back_btn, label: "メニューに戻る")
    }
    
    when user.click(back_btn) {
        navigate_to(GameFlow::MainMenu)
    }
}
```

## 14. 文法リファレンス (Grammar Reference)

### 14.1 PEST文法（抜粋）

```pest
// 基本要素
ident = @{ (ASCII_ALPHANUMERIC | "_" | "-")+ }
qualified_ident = @{ ident ~ ("::" ~ ident)* }
string = @{ dq_string | fancy_dq_string }
number = @{ "-"? ~ ASCII_DIGIT+ ~ ("." ~ ASCII_DIGIT+)? }
bool = { "true" | "false" }

// 寸法値（単位付き数値）
dimension_value = { number ~ unit_suffix? }
unit_suffix = { "px" | "vw" | "vh" | "%" | "rem" | "em" }

// データ構造
array = { "[" ~ (expr ~ ("," ~ expr)*)? ~ "]" }
object = { "{" ~ (object_entry ~ ("," ~ object_entry)*)? ~ "}" }
object_entry = { ident ~ ":" ~ expr }

// 式
expr = { match_expr | string | dimension_value | number | bool | path | ident | array | object }

// フロー定義
flow_def = { "flow" ~ "{" ~ start_def ~ transition_def+ ~ "}" }
namespaced_flow_def = { "flow" ~ ident ~ "{" ~ namespaced_start_def ~ namespaced_transition_def+ ~ "}" }

// UI要素
text = { "Text" ~ "(" ~ expr ~ ("," ~ arg_item)* ~ ")" }
button = { "Button" ~ "(" ~ "id" ~ ":" ~ (ident | string) ~ "," ~ "label" ~ ":" ~ string ~ ("," ~ "onclick" ~ ":" ~ rust_call)? ~ ("," ~ style_arg)? ~ ")" }
vstack_node = { "VStack" ~ "(" ~ style_arg? ~ ")" ~ "{" ~ view_nodes? ~ "}" }
hstack_node = { "HStack" ~ "(" ~ style_arg? ~ ")" ~ "{" ~ view_nodes? ~ "}" }

// 制御構造
foreach_node = { "foreach" ~ ident ~ "in" ~ expr ~ ("(" ~ style_arg? ~ ")")? ~ "{" ~ view_nodes? ~ "}" }
if_node = { "if" ~ expr ~ ("(" ~ style_arg? ~ ")")? ~ "{" ~ view_nodes? ~ "}" ~ ("else" ~ "{" ~ view_nodes? ~ "}")? }
match_block = { "match" ~ expr ~ ("(" ~ style_arg? ~ ")")? ~ "{" ~ match_arm* ~ default_arm? ~ "}" }

// リスト操作
list_append = { "append" ~ "(" ~ path ~ "," ~ expr ~ ")" }
list_remove = { "remove" ~ "(" ~ path ~ "," ~ number ~ ")" }
list_clear = { "clear" ~ "(" ~ path ~ ")" }

// イベント
when_block = { "when" ~ event_expr ~ "{" ~ view_nodes? ~ "}" }
event_expr = { user_event }
user_event = { "user" ~ "." ~ event_kind ~ "(" ~ ident ~ ")" }
event_kind = { "click" }
```

---

この仕様書は、実際のNiloパーサーとAST実装に基づいて作成されており、現在サポートされている機能のみを含んでいます。
