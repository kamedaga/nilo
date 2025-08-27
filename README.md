# nilo

> **宣言的DSLでUIを設計できるRust製UIフレームワーク。WGPUベースの超高速レンダリング、WASM/クロスプラットフォーム対応。**

---

## 概要

**nilo** は、Rustで書かれた次世代UIフレームワークです。
`.nilo` という独自DSLで画面遷移（Flow）やインタラクション（Timeline）を宣言的に記述し、
WGPUによる超高速な描画。

従来のGUIライブラリと異なり、「stencil」という中間言語IRでUI構造を抽象化。
WGPU対応を前提とした設計で、**Mac/Linux/Windows**などで動作します。

---

## 特徴

* **宣言的DSL** (`.nilo`): アプリの画面遷移やUIレイアウト・ロジックを1つのDSLで記述可能
* **高速WGPU描画**: GPUを直接叩く高速・軽量なレンダラ。直接WGPUを意識せずに複雑UI構築
* **stencil中間表現**: `circle` `image` `quad` `text` `triangle` `roundedrect` などを抽象命令化、再利用性・移植性が高い
* **クロスプラットフォーム**: macOSで検証中、Windows/Linux対応も進行
* **タイムライン/フロー構造**: 画面遷移・インタラクションの分離設計

---

## サンプル

### 1. .nilo DSLの例

```nilo
flow {
    start: HelloWorld
    HelloWorld -> None
}

timeline HelloWorld {
    Text("Welcome to the nilo framework!")
}

timeline None {
    Text("This is a placeholder for the None timeline.")
}
```

### 2. Rustからの利用例

```rust
use nilo;

fn main() {
    let app = nilo::load_nilo_app("src/app.nilo")
        .expect("Failed to parse .nilo file");

    println!("{:#?}", app);
    nilo::run(app);
}
```

---

## UI定義DSLの文法例

* 画面遷移定義: `flow { ... }`
* 画面ごとのUI/ロジック: `timeline XXX { ... }`
* UIプリミティブ: `Text`, `Button`, `Image`, `VStack`, `HStack` など
* 制御構造: `match`, `when`, `dynamic_section` 等
* イベント・アクション: `when user.click(btn) { navigate_to(NextScreen) }`
* 中間命令: `stencil.circle(...)` など

### DSL詳細サンプル

```nilo
timeline MainMenu {
    VStack {
        Text("Hello, World!")
        Button(id: start, label: "Start")
    }
    when user.click(start) {
        navigate_to(AppScreen)
    }
}
```

---

## Stencil中間表現

niloのコアは、UIを「stencil」と呼ばれる中間命令列（IR）に変換してGPU描画。
例えば `stencil.circle(...)`, `stencil.image(...)`, `stencil.text(...)` など
→ これによりDSL→多様なプラットフォーム描画を高速に仲介できます。

対応中のstencil命令一覧:

* `circle`
* `image`
* `quad`
* `text`
* `triangle`
* `roundedrect`

---

## 動作環境

* **Rust (1.75+)**
* **WGPU (0.26.1+)**
* **macOS**で動作確認済み

---

## ビルド・実行

1. `.nilo` ファイルを用意
2. Rustアプリで `nilo::load_nilo_app()` でパース
3. `nilo::run(app)` で即座に実行

```sh
cargo run
```

---

## 今後の開発ロードマップ

* [ ] wasm、webgpuへの対応
* [ ] stencil命令の拡張（shadow, gradient等）
* [ ] DSLの静的解析・LSP連携
* [ ] マルチプラットフォームの正式サポート
* [ ] 高度なアニメーション・状態遷移
* [ ] UIコンポーネントの増強（List, Modal, Menu…）
* [ ] ユーザー定義コンポーネントの柔軟化

---

## 貢献・コントリビュート

Pull Request/Issue大歓迎です！
バグ報告・ドキュメント修正・機能追加の提案などもお待ちしています。

---

## ライセンス

MIT

---

## 作者

* [@kamezuki](https://github.com/kamezuki)

---

###### chatgptで生成しました
