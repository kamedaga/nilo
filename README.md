# nilo

> **宣言的DSLでUIを設計できるRust製UIフレームワーク。WGPUベースのレンダリング、WASM/クロスプラットフォーム対応。**

---

## 概要

**nilo** は、Rustで書かれた次世代UIフレームワークです。
`.nilo` という独自DSLで画面遷移（Flow）やインタラクション（Timeline）を宣言的に記述し、
WGPUによる描画。

従来のGUIライブラリと異なり、「stencil」という中間言語IRでUI構造を抽象化。
WGPU対応を前提とした設計で、**Mac/Windows**で動作します。(動作確認していないがLinuxでも動くはず)

---

## 新機能、改善点
* foreach、ifが正しくレイアウトされない問題を解決しました
* ifで比較演算子を使えるように
* 不要なログを削除しました
* 汎用的なレイアウトシステムに根本から変更しました。 詳しくはLAYOUT_SYSTEM_NEW.md

---

## 開発者用ツール

* ホットリロード
* Lintツール
* デバッガー

--- 

## 言語仕様

言語仕様に関してはLANGUAGE_SPEC.mdおよび、tutorial.niloを参考にしてください

---

## ライセンス

Apache-2.0

---

## 予定

dom(wasm), tiny-skiaレンダラ 
モダンStencil(影やすりガラスのようなモダンな描画をレンダラから作る)
レンダラの軽量化


モバイルへの対応予定はありません。(技術的に難しいため）


## 作者

* [@kamezuki](https://github.com/kamezuki)

---

