# nilo

> **宣言的DSLでUIを設計できるRust製UIフレームワーク。WGPUベースのレンダリング、WASM/クロスプラットフォーム対応。**

---

## 概要

**nilo** は、Rustで書かれた次世代UIフレームワークです。
`.nilo` という独自DSLで画面遷移（Flow）やインタラクション（Timeline）を宣言的に記述し、
WGPUによる描画。

従来のGUIライブラリと異なり、「stencil」という中間言語IRでUI構造を抽象化。
WGPU対応を前提とした設計で、**Mac/Linux/Windows**などで動作します。

---
## 言語仕様
言語仕様に関してはLANGUAGE_SPEC.mdおよび、tutorial.niloを参考にしてください

## ライセンス

Apache-2.0

---

## 作者

* [@kamezuki](https://github.com/kamezuki)

---

