# Niloコンポーネントシステム仕様書

## 目次
1. [概要](#概要)
2. [コンポーネントの定義](#コンポーネントの定義)
3. [パラメータシステム](#パラメータシステム)
4. [スタイルシステム](#スタイルシステム)
5. [コンポーネント呼び出し](#コンポーネント呼び出し)
6. [使用例](#使用例)
7. [制約と注意事項](#制約と注意事項)

---

## 概要

Niloコンポーネントは、再利用可能なUIパーツを定義するための機能です。コンポーネントは以下の特徴を持ちます：

- **パラメータ化**: 型付きパラメータ、デフォルト値、オプショナルパラメータをサポート
- **スタイル適用**: デフォルトスタイルと呼び出し時のスタイルオーバーライド
- **ネスト対応**: コンポーネント内で他のコンポーネントを呼び出し可能
- **軽量展開**: コンパイル時にインライン展開される静的なコンポーネントシステム

---

## コンポーネントの定義

### 基本構文

```nilo
component ComponentName(param1, param2, ...) {
    VStack(style: { spacing: 8px }) {
        Text("Hello, {}", param1)
    }
}
```

### フル構文（型付きパラメータ）

```nilo
component Card(
    title: String,
    content: String,
    footer: String?,
    style: { width: 300px, padding: 16px, background: "#ffffff" }
) {
    font: "fonts/NotoSansJP-Regular.ttf"
    
    VStack(style: { spacing: 8px }) {
        Text("{}", title, style: { font_size: 18px })
        Text("{}", content)
    }
}
```

### コンポーネント定義の要素

| 要素 | 説明 | 必須 |
|------|------|------|
| `name` | コンポーネント名（PascalCase推奨） | ✓ |
| `params` | パラメータリスト（括弧内） | × |
| `font` | コンポーネント全体で使用するフォント | × |
| `body` | コンポーネントの内容（ViewNode配列） | ✓ |
| `whens` | イベントハンドラー（未実装） | × |


### 定義の構文

```nilo
component <名前>(<パラメータリスト>?) {
    font: "<フォントパス>"?
    <ビューノード>*
}
```

**パラメータの種類**:
- 型なし: `name`
- 型付き: `name: Type`
- デフォルト値: `name: Type = value`
- オプショナル: `name: Type?`
- 列挙型: `name: ("a" | "b" | "c") = "a"`
- スタイル: `style: { ... }`

---

## パラメータシステム

### パラメータ型定義

#### 1. 基本型パラメータ

```nilo
component Example(
    text: String,
    count: Number,
    enabled: Bool,
    items: Array,
    config: Object
) {
    Text("{}", text)
}
```

サポートされる型：
- `String` / `string`: 文字列型
- `Number` / `number`: 数値型（f32）
- `Bool` / `bool`: ブール型
- `Array` / `array`: 配列型
- `Object` / `object`: オブジェクト型
- `Function` / `function`: 関数型（未実装）

#### 2. デフォルト値付きパラメータ

```nilo
component Button(
    label: String = "Click me",
    size: Number = 16,
    enabled: Bool = true
) {
    Button(id: btn_id, label: label)
}
```

#### 3. オプショナルパラメータ

```nilo
component Card(
    title: String,
    subtitle: String?,
    footer: String?
) {
    VStack(style: { spacing: 8px }) {
        Text("{}", title)
        if subtitle {
            Text("{}", subtitle)
        }
    }
}
```

オプショナルパラメータは値が渡されなくても必須エラーにならない。

#### 4. 列挙型パラメータ

```nilo
component Alert(
    type: ("info" | "warning" | "error") = "info",
    size: ("small" | "medium" | "large") = "medium"
) {
    Text("Alert: {}", type)
}
```

#### 5. 型なしパラメータ（後方互換）

```nilo
component SimpleCard(title, content) {
    VStack(style: { spacing: 8px }) {
        Text("{}", title)
        Text("{}", content)
    }
}
```

---

## スタイルシステム

### デフォルトスタイル（パラメータとして定義）

コンポーネント定義のパラメータリスト内で `style: {...}` を指定：

```nilo
component Panel(
    content: String,
    style: {
        width: 300px,
        padding: 16px,
        background: "#f0f0f0",
        rounded: 8px
    }
) {
    VStack(style: { spacing: 8px }) {
        Text("{}", content)
    }
}
```

**重要**: `style` パラメータは特別扱いされ、`Component.default_style` として保存されます。

### スタイル適用の優先順位

1. **ComponentCall時のスタイル（最優先）**
   ```nilo
   Panel("Hello", style: { background: "#ff0000" })
   ```

2. **コンポーネント定義のデフォルトスタイル**
   ```nilo
   component Panel(..., style: { background: "#f0f0f0" }) { ... }
   ```

3. **body内の個別ノードのスタイル**
   ```nilo
   VStack(style: { padding: 8px }) { ... }
   ```

### スタイルの書き方

Niloでは、すべてのビューノードで **引数内に `style: {...}`** としてスタイルを指定します：

```nilo
// ✅ 正しい
Text("Hello", style: { font_size: 18px, color: "#333" })
Button(id: btn, label: "Click", style: { background: "#007bff" })
VStack(style: { spacing: 12px, padding: 20px }) { ... }
Image("path/to/image.png", style: { width: 200px, height: 150px })

// ❌ 間違い（別ブロック構文は存在しない）
Text("Hello") { font_size: 18px }
Button(id: btn, label: "Click") { background: "#007bff" }
```

### コンポーネント呼び出し時のスタイル

```nilo
component MyCard(title: String) {
    VStack(style: { spacing: 8px }) {
        Text("{}", title)
    }
}

timeline Main {
    // ✅ 正しい：style引数として渡す
    MyCard("Welcome", style: { width: 400px, background: "#fff" })
    
    // ❌ 間違い：別ブロックは無効
    MyCard("Welcome") { width: 400px }
}
```



---

## コンポーネント呼び出し

### 基本的な呼び出し

```nilo
// 位置引数のみ
MyComponent("arg1", "arg2")

// 引数 + スタイル
MyComponent("arg1", "arg2", style: { width: 300px })

// スタイルのみ（引数なし）
SimpleComponent(style: { padding: 20px })
```

**注意**: 名前付き引数（`title: "Hello"`）は現在未実装です。位置引数のみサポート。

### スタイル付き呼び出しの詳細

```nilo
Card("Title", "Content", style: {
    width: 300px,
    height: 200px,
    background: "#ffffff",
    padding: 16px,
    rounded: 8px,
    shadow: { blur: 8, offset: [0, 2], color: "#00000033" }
})
```

### ネストした呼び出し

```nilo
component UserCard(name: String, email: String) {
    VStack(style: { spacing: 4px }) {
        Text("{}", name, style: { font_size: 16px })
        Text("{}", email, style: { font_size: 14px, color: "#666" })
    }
}

component UserList(users: Array) {
    VStack(style: { spacing: 8px }) {
        foreach user in users {
            UserCard(user.name, user.email)
        }
    }
}
```

### 呼び出しの構文

```nilo
<コンポーネント名>(<引数>*, style: { ... }?)
```



---

## 使用例

### シンプルなボタンコンポーネント

```nilo
component PrimaryButton(
    label: String = "Click",
    style: {
        background: "#007bff",
        color: "#ffffff",
        padding: [12, 24],
        rounded: 4px
    }
) {
    Button(id: btn_id, label: label)
}

timeline Main {
    VStack(style: { spacing: 12px }) {
        PrimaryButton("Submit", style: { width: 200px })
    }
}
```

### カードコンポーネント

```nilo
component Card(
    title: String,
    content: String,
    footer: String?,
    style: {
        width: 300px,
        background: "#ffffff",
        padding: 16px,
        rounded: 8px,
    }
) {
    VStack(style: { spacing: 8px }) {
        Text("{}", title, style: { font_size: 18px })
        Text("{}", content, style: { margin_top: 8px })
        
        if footer {
            Text("{}", footer, style: { margin_top: 16px, color: "#666" })
        }
    }
}

timeline CardDemo {
    VStack(style: { spacing: 16px, padding: 20px }) {
        Card("Welcome", "This is a card component")
        Card("Custom", "With footer", "Footer text")
        Card("Styled", "Custom style", style: { background: "#f0f0f0" })
    }
}
```

### リストアイテムコンポーネント

```nilo
component UserListItem(user: Object) {
    HStack(style: { padding: 12px, border_bottom: "1px solid #eee" }) {
        VStack(style: { spacing: 4px }) {
            Text("{}", user.name, style: { font_size: 16px })
            Text("{}", user.email, style: { font_size: 14px, color: "#666" })
        }
        SpacingAuto
        Button(id: btn_view, label: "View")
    }
}

timeline UserList {
    VStack(style: { spacing: 0 }) {
        foreach user in state.users {
            UserListItem(user)
        }
    }
}
```

### フォーム入力コンポーネント

```nilo
component FormField(
    label: String,
    placeholder: String = "",
    style: {
        width: 100%,
        spacing: 4px
    }
) {
    VStack(style: { spacing: 4px }) {
        Text("{}", label, style: { font_size: 14px, color: "#333" })
        TextInput(field_input, style: { 
            width: 100%, 
            padding: 8px,
            background: "#fff",
            border: "1px solid #ddd"
        })
    }
}

timeline FormDemo {
    VStack(style: { spacing: 16px, padding: 20px }) {
        FormField("Name", placeholder: "Enter your name")
        FormField("Email", placeholder: "your@email.com")
        Button(id: submit_btn, label: "Submit", style: {
            background: "#007bff",
            color: "#fff",
            padding: [10, 20]
        })
    }
}
```

---

## 今後の拡張予定（Phase 2）

### スロットシステム

```nilo
component Modal(title: String, slot content, slot footer?) {
    VStack(style: { spacing: 12px }) {
        Text("{}", title, style: { font_size: 20px })
        slot content  // スロットコンテンツを挿入
        
        if has_slot(footer) {
            slot footer
        }
    }
}

// 使用例（構文検討中）
Modal(title: "Confirm") {
    content: {
        Text("Are you sure?")
    }
    footer: {
        HStack(style: { gap: 8px }) {
            Button(id: btn_ok, label: "OK")
            Button(id: btn_cancel, label: "Cancel")
        }
    }
}
```

**注意**: スロットシステムはgrammar.pestに定義されていますが、完全な実装は未完了です。

### 名前付き引数

```nilo
// 現在は位置引数のみ
Card("Title", "Content", "Footer")

// Phase 2: 名前付き引数（構文検討中）
Card(
    title: "Title",
    content: "Content",
    footer: "Footer"
)
```

**注意**: 名前付き引数は現在未実装です。すべての引数は位置で指定する必要があります。

---

## 制約と注意事項

### スコープ

- コンポーネント内から**timelineのローカル変数にはアクセス不可**
- コンポーネントパラメータは独立したスコープを持つ
- `state.xxx` などのグローバル状態にはアクセス可能

### 展開タイミング

- コンポーネントは**レンダリング前に静的展開**される
- 動的なコンポーネント選択は不可
- すべての参照はコンパイル時に解決される

### パラメータ

- **位置引数のみ**サポート（名前付き引数は未実装）
- デフォルト値は省略時のみ適用
- オプショナルパラメータは値なしでもエラーにならない

### スタイル

- `style: {...}` は常に**引数内**に記述
- 別ブロック構文（`Component(...) { ... }`）は**存在しない**
- スタイル適用順: 呼び出し時 > デフォルト > body内

### 未実装機能

- スロットシステム（構文のみ定義済み）
- 名前付き引数
- コンポーネントライフサイクル（on_mount/on_unmountなど）

---

## まとめ

Niloのコンポーネントシステムは以下の特徴を持ちます：

✅ **型安全**: 型付きパラメータとデフォルト値  
✅ **スタイル柔軟性**: `style: {...}` パラメータでデフォルトスタイルを定義  
✅ **軽量**: 静的展開によるゼロコストアブストラクション  
✅ **シンプル**: 複雑なライフサイクル管理なし  
✅ **ネスト対応**: コンポーネントの再帰的な組み合わせ  
✅ **一貫性**: すべてのビューノードと同じ `style: {...}` 構文  

### 重要なポイント

1. **スタイルは常に引数内**: `Component(args, style: {...})` 形式
2. **位置引数のみ**: 名前付き引数は未実装
3. **デフォルトスタイル**: パラメータに `style: {...}` を指定
4. **展開タイミング**: レンダリング前に静的展開
5. **スコープ**: コンポーネント内からtimelineのローカル変数にアクセス不可

この設計により、宣言的で再利用可能なUIコンポーネントを効率的に構築できます。
