# Niloでのフォント指定方法

## シンプルなフォントシステム

Niloは**デフォルトフォント**のみを使用するシンプルなフォントシステムを採用しています。

### デフォルトフォント

フォントを指定しない場合、またはに`font: "default"`と指定した場合、システムのデフォルトフォント（SansSerif）が使用されます。

```nilo
timeline MyTimeline {
    VStack(style: {width: 100ww}) {
        // デフォルトフォントが使用される
        Text("こんにちは")
        Text("Hello World")
    }
}
```

### フォント指定

`timeline`レベルで`font`を指定することはできますが、**現在のシステムではフォントファイルからの読み込みに対応しました**。

```nilo
timeline MyTimeline {
    font: "fonts/NotoSansJP-Regular.ttf"  // フォント名を指定(拡張子も必要)
    VStack(style: {width: 100ww}) {
        Text("日本語テキスト")
    }
}
```

## 利用可能なフォント一覧

### Windows環境で推奨される安全なフォント

#### 日本語対応フォント
- `"Yu Gothic UI"` - Windows 10/11標準、美しい日本語表示
- `"Meiryo"` - Windows Vista以降、読みやすい
- `"MS UI Gothic"` - Windows標準、UIに適している
- `"MS Gothic"` - 等幅フォント

#### 英語フォント
- `"Arial"` - 標準的なサンセリフ
- `"Segoe UI"` - Windowsモダンフォント
- `"Tahoma"` - 小さいサイズでも読みやすい
- `"Verdana"` - Webでも使われる

## フォント指定の例

### プログラム例
```nilo
app MyApp {
    // タイトル
    text "アプリケーションタイトル" {
        position: [20, 20]
        size: 32
        font: "Yu Gothic UI"
        color: [0.1, 0.1, 0.1, 1.0]
    }
    
    // サブタイトル
    text "Subtitle in English" {
        position: [20, 70]
        size: 18
        font: "Segoe UI"
        color: [0.3, 0.3, 0.3, 1.0]
    }
    
    // 本文
    text "本文のテキストです。長い文章でも適切に表示されます。" {
        position: [20, 120]
        size: 14
        font: "Meiryo"
        max_width: 400
        color: [0.0, 0.0, 0.0, 1.0]
    }
    
    // コードフォント（等幅）
    text "let code = \"sample\";" {
        position: [20, 200]
        size: 12
        font: "MS Gothic"
        color: [0.2, 0.4, 0.2, 1.0]
    }
}
```

## フォントの問題と対処法

### 問題のあるフォント
以下のフォントは読み込み時にエラーが発生する可能性があります：
- `mstmc.ttf` (Microsoft Tai Le Collection)
- `Myanmar Text`
- `Segoe UI Historic`
- `Segoe UI Emoji`



## カスタムフォントの読み込み

### TTF/OTFファイルの読み込み
```rust
// Rustコード側での実装例
text_renderer.load_font_from_file("MyCustomFont", "path/to/font.ttf")?;
```

### 使用例
```nilo
text "カスタムフォント" {
    position: [100, 100]
    size: 20
    font: "MyCustomFont"
}
```

## パフォーマンス考慮事項

1. **フォントキャッシュ**: 同じフォント・サイズの組み合わせはキャッシュされ、2回目以降の描画が高速化されます
2. **フォント変更**: 頻繁なフォント変更は避け、できるだけ統一されたフォントセットを使用してください
3. **日本語フォント**: 日本語フォントは英語フォントより重いため、必要な場合のみ使用してください

## トラブルシューティング

### フォントが表示されない場合
1. フォント名が正しいかチェック
2. システムにそのフォントがインストールされているかチェック
3. デフォルトフォントで試してみる

### 文字化けが発生する場合
1. UTF-8エンコーディングを使用しているかチェック
2. 日本語対応フォント（Yu Gothic UI, Meiryo等）を使用
3. フォントサイズが適切かチェック

### パフォーマンスが悪い場合
1. 同じテキストの再描画回数を減らす
2. フォント・サイズの組み合わせを統一する
3. `max_width`を適切に設定して不要な再計算を避ける