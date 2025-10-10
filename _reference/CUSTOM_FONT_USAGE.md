# カスタムフォント使用ガイド

## 概要

TextRendererはフォント管理において完全な柔軟性を提供します。
- **デフォルト**: OSのシステムフォントを使用
- **名前付きカスタムフォント**: 必要に応じて任意のフォントを名前で登録・使用可能

## 使用方法

### 1. システムフォントのみを使用（推奨・最もシンプル）

```rust
use nilo::*;

fn main() {
    let state = MyState::default();
    let cli_args = parse_args();
    
    run_nilo_app!("src/app.nilo", state, &cli_args);
}
```

この場合、OSにインストールされているフォント（SansSerif、Serifなど）が使用されます。
埋め込みフォントは一切含まれないため、バイナリサイズが最小になります。

### 2. 名前付きカスタムフォントを使用（超簡単！）

main.rsで名前を指定して登録：

```rust
use nilo::*;

// フォントを埋め込む（プロジェクトルートからの相対パス）
const MY_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/MyFont.ttf"));

fn main() {
    // カスタムフォントを名前付きで登録
    set_custom_font("myfont", MY_FONT);
    
    let state = MyState::default();
    let cli_args = parse_args();
    
    run_nilo_app!("src/app.nilo", state, &cli_args);
}
```

Niloファイルで名前を指定して使用：

```nilo
Text {
    content: "Hello World"
    font: "myfont"  // 登録した名前で使用！
    size: 24
}
```

**これだけです！**

### 3. 複数のフォントを登録

```rust
use nilo::*;

const FONT_JP: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP.ttf"));
const FONT_EN: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/Roboto.ttf"));
const FONT_EMOJI: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoEmoji.ttf"));

fn main() {
    // 複数のフォントを登録
    set_custom_font("japanese", FONT_JP);
    set_custom_font("english", FONT_EN);
    set_custom_font("emoji", FONT_EMOJI);
    
    // または一括登録
    set_custom_fonts(&[
        ("japanese", FONT_JP),
        ("english", FONT_EN),
        ("emoji", FONT_EMOJI),
    ]);
    
    let state = MyState::default();
    let cli_args = parse_args();
    
    run_nilo_app!("src/app.nilo", state, &cli_args);
}
```

Niloファイルで使い分け：

```nilo
VStack {
    Text {
        content: "こんにちは"
        font: "japanese"  // 日本語フォント
    }
    Text {
        content: "Hello World"
        font: "english"   // 英語フォント
    }
    Text {
        content: "😀🎉"
        font: "emoji"     // 絵文字フォント
    }
}
```

## 設計の利点

### ✅ ハードコードなし
- フォントファイルパスやフォント名は一切ハードコードされていません
- フレームワークはフォントを強制しません

### ✅ 超シンプルなAPI
- `set_custom_font("name", FONT_DATA)` で名前付き登録
- Niloファイルで `font: "name"` と指定するだけ
- device、queue、formatなどの低レベルAPIは完全に隠蔽

### ✅ 複数フォント対応
- 好きなだけフォントを登録可能
- 用途別にフォントを使い分けられる
- `set_custom_fonts` で一括登録も可能

### ✅ 完全な柔軟性
- デフォルト: システムフォントで軽量に
- カスタム: 必要なフォントを名前付きで登録

### ✅ 型安全性
- フォントファミリー名は実行時に動的に取得
- コンパイル時の型チェックでエラーを防止

### ✅ バイナリサイズの最適化
- デフォルトでは埋め込みフォントなし
- ユーザーが明示的に選択した場合のみ埋め込み

## API詳細

### `set_custom_font(name: &str, font_data: &'static [u8])`

カスタムフォントを名前付きでグローバルに登録します。

- **引数**: 
  - `name`: Niloファイルで使用する名前
  - `font_data`: フォントデータ
- **呼び出し**: アプリケーション起動前に呼び出し
- **スレッドセーフ**: はい（RwLockを使用）

```rust
const MY_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/font.ttf"));
set_custom_font("myfont", MY_FONT);
```

### `set_custom_fonts(fonts: &[(&str, &'static [u8])])`

複数のカスタムフォントを一括登録します。

```rust
set_custom_fonts(&[
    ("japanese", FONT_JP),
    ("english", FONT_EN),
]);
```

## フォントの指定方法

### テキスト描画時のフォント指定

```rust
// デフォルト（システムフォント）
text_renderer.render_text(..., "default", ...);

// システムにインストールされているフォント名を指定
text_renderer.render_text(..., "Arial", ...);

// .ttf/.otfパスが指定された場合は埋め込みフォントを使用
text_renderer.render_text(..., "fonts/MyFont.ttf", ...);
```

フォント名が `.ttf` または `.otf` で終わる場合、
`with_embedded_font()` で登録した埋め込みフォントが使用されます。

## 外部フォントファイルをランタイムでロード

将来的な拡張として、`load_and_register_font` メソッドを使用できます：

```rust
impl TextRenderer {
    pub fn load_external_font(&mut self, font_path: &str) -> Option<String> {
        Self::load_and_register_font(&mut self.font_system, font_path)
    }
}
```

## マルチフォント対応

現在のバージョンでは1つのグローバルフォントのみサポートしていますが、
将来的には複数フォントの登録をサポート予定です。

### 現在の回避策

異なる言語に異なるフォントを使いたい場合は、
複数の文字セットを含むフォント（例: Noto Sans）を使用してください。

```rust
// Noto Sansは多言語対応
const NOTO_SANS: &[u8] = include_bytes!("fonts/NotoSans-Regular.ttf");
set_custom_font(Some(NOTO_SANS));
```

## ベストプラクティス

1. **デフォルトはシステムフォント**: 特別な理由がない限り `new()` を使用
2. **埋め込みは必要な場合のみ**: ブランディングや特殊文字が必要な場合のみ
3. **フォントサイズに注意**: 日本語フォントは大きい（数MB）ことが多い
4. **フォールバック設計**: 埋め込みフォントが失敗した場合のフォールバックを考慮
5. **パス指定の統一**: `include_bytes!`では必ず`concat!(env!("CARGO_MANIFEST_DIR"), "/path")`を使用

### パス指定の理由

Niloでは、ファイルパス指定は常にプロジェクトルートからの相対パスを使用します：

```rust
// ✅ 正しい: プロジェクトルートから
const FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/MyFont.ttf"));
run_nilo_app!("src/app.nilo", state, &cli_args);  // これもルートから

// ❌ 間違い: 現在のファイルからの相対パス
const FONT: &[u8] = include_bytes!("../fonts/MyFont.ttf");  // main.rsの位置に依存
```

`env!("CARGO_MANIFEST_DIR")`を使用することで、ファイルがどこにあっても常にプロジェクトルートからの
一貫したパス指定が可能になります。

## トラブルシューティング

### 日本語が表示されない
- OSにインストールされている日本語フォントを使用するか
- 日本語フォント（NotoSansJP等）を明示的に埋め込む

### バイナリサイズが大きい
- 埋め込みフォントのサイズを確認
- システムフォントのみを使用することを検討
- フォントのサブセット化を検討（必要な文字のみ含める）

## 今後の拡張案

1. **複数フォントの同時登録**
   ```rust
   set_custom_fonts(&[
       ("japanese", FONT_JP),
       ("english", FONT_EN),
       ("emoji", FONT_EMOJI),
   ]);
   ```

2. **フォントフォールバックチェーン**
   ```rust
   set_font_fallback_chain(&["My Font", "Noto Sans JP", "sans-serif"]);
   ```

3. **動的フォントロード API**
   ```rust
   load_font_from_file("path/to/font.ttf")?;
   load_font_from_bytes(&font_data)?;
   ```

4. **フォント設定ビルダーパターン**
   ```rust
   FontConfig::new()
       .primary(MY_FONT)
       .fallback(FALLBACK_FONT)
       .build();
   ```
