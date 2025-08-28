use crate::parser::ast::{
    App, Timeline, ViewNode, Expr, WithSpan, Style, ColorValue, Rounded, Shadow, Edges,
};
use crate::stencil::stencil::Stencil;
use std::collections::HashMap;

/// コンポーネント専用の状態管理構造体（軽量化版）
#[derive(Debug, Clone)]
pub struct ComponentContext {
    pub current_args: HashMap<String, String>,
    pub nest_level: usize,
    pub call_stack: Vec<String>,
    pub args_stack: Vec<HashMap<String, String>>,

    // ★ 新規追加: foreach変数のスタック管理
    pub foreach_vars: HashMap<String, String>,
    pub foreach_vars_stack: Vec<HashMap<String, String>>,
}

impl ComponentContext {
    pub fn new() -> Self {
        Self {
            current_args: HashMap::new(),
            nest_level: 0,
            call_stack: Vec::new(),
            args_stack: Vec::new(),
            foreach_vars: HashMap::new(),
            foreach_vars_stack: Vec::new(),
        }
    }

    /// コンポーネントに入る際の処理（軽量化�������������）
    pub fn enter_component(&mut self, component_name: &str, args: HashMap<String, String>) {
        self.args_stack.push(self.current_args.clone());
        self.current_args = args;
        self.nest_level += 1;
        self.call_stack.push(component_name.to_string());
    }

    /// コンポーネントから出る際の処理（軽量化版）
    pub fn exit_component(&mut self) {
        if let Some(_) = self.call_stack.pop() {
            // デバッグログを削����
        }

        self.nest_level = self.nest_level.saturating_sub(1);

        if let Some(previous_args) = self.args_stack.pop() {
            self.current_args = previous_args;
        } else {
            self.current_args.clear();
        }
    }

    /// 引数を取得（軽量化版）
    #[inline]
    pub fn get_arg(&self, name: &str) -> Option<&String> {
        self.current_args.get(name)
    }

    /// ネストした上位レベルの引数も検索（軽量化版）
    #[inline]
    pub fn get_arg_from_any_level(&self, name: &str) -> Option<&String> {
        // まず現在のレ���ルから検索
        if let Some(value) = self.current_args.get(name) {
            return Some(value);
        }

        // 上位レベルのスタックから検索（逆順���最新から）
        for args in self.args_stack.iter().rev() {
            if let Some(value) = args.get(name) {
                return Some(value);
            }
        }

        None
    }

    /// 引数を設定（軽量化版）
    #[inline]
    pub fn set_arg(&mut self, name: String, value: String) {
        self.current_args.insert(name, value);
    }

    /// 全レ��ルの引数を一���取得（デバッグ用）
    pub fn get_all_args(&self) -> HashMap<String, String> {
        let mut all_args = HashMap::new();

        for args in &self.args_stack {
            all_args.extend(args.clone());
        }

        all_args.extend(self.current_args.clone());
        all_args
    }

    /// 現在のコンテキストをクリア（軽量化版）
    pub fn clear(&mut self) {
        self.current_args.clear();
        self.nest_level = 0;
        self.call_stack.clear();
        self.args_stack.clear();
        self.foreach_vars.clear();
        self.foreach_vars_stack.clear();
    }

    // ★ 新規追加: foreach変数管理メソッド

    /// foreach変数を設定
    pub fn set_foreach_var(&mut self, name: String, value: String) {
        self.foreach_vars.insert(name, value);
    }

    /// foreach変数を取得
    pub fn get_foreach_var(&self, name: &str) -> Option<&String> {
        // 現在のレベルから検索
        if let Some(value) = self.foreach_vars.get(name) {
            return Some(value);
        }

        // 上位レベルのスタックから検索
        for vars in self.foreach_vars_stack.iter().rev() {
            if let Some(value) = vars.get(name) {
                return Some(value);
            }
        }

        None
    }

    /// foreach変数を含めた総合的な変数取得
    pub fn get_var(&self, name: &str) -> Option<&String> {
        // 1. foreach変数を最優先
        if let Some(value) = self.get_foreach_var(name) {
            return Some(value);
        }

        // 2. コンポーネント引数
        if let Some(value) = self.get_arg_from_any_level(name) {
            return Some(value);
        }

        None
    }

    /// foreachレベルに入る
    pub fn enter_foreach(&mut self) {
        self.foreach_vars_stack.push(self.foreach_vars.clone());
    }

    /// foreachレベルから出る
    pub fn exit_foreach(&mut self) {
        if let Some(previous_vars) = self.foreach_vars_stack.pop() {
            self.foreach_vars = previous_vars;
        } else {
            self.foreach_vars.clear();
        }
    }
}

/// ����リ実行時の状態（軽量化版��
#[derive(Debug, Clone)]
pub struct AppState<S> {
    pub custom_state: S,
    pub current_timeline: String,
    pub position: usize,
    pub variables: HashMap<String, String>,
    pub component_context: ComponentContext,
    pub image_size_cache: std::rc::Rc<std::cell::RefCell<HashMap<String, (u32, u32)>>>,
    pub all_buttons: Vec<(String, [f32; 2], [f32; 2])>,

    /// ボタンのonclick情報を保存
    pub button_onclick_map: HashMap<String, Expr>,

    /// 静的パートの描画キャッシュ
    pub static_stencils: Option<Vec<Stencil>>,
    /// 静的ボタン境界��キャッシュ
    pub static_buttons: Vec<(String, [f32; 2], [f32; 2])>,

    pub expanded_body: Option<Vec<WithSpan<ViewNode>>>,

    /// ウィンドウサイズのキ���ッシュ
    pub cached_window_size: Option<[f32; 2]>,
    
    /// 前回のホバーボタンID（ホバー状態変化の検出用）
    pub last_hovered_button: Option<String>,

    // ★ 新規追加: テキスト入力とIME関連の状態管理
    /// 現在フォーカスされているテキスト入力フィールドのID
    pub focused_text_input: Option<String>,
    /// テキスト入力フィールドの値を保存するマップ
    pub text_input_values: HashMap<String, String>,
    /// IME変換中のテキスト（フィールドごと）
    pub ime_composition_text: HashMap<String, String>,
    /// カーソル位置（フィールドごと）
    pub text_cursor_positions: HashMap<String, usize>,
    /// 選択範囲（フィールドごと、開始位置と終了位置）
    pub text_selections: HashMap<String, (usize, usize)>,
}

impl<S> AppState<S> {
    pub fn new(custom_state: S, start_timeline: String) -> Self {
        Self {
            custom_state,
            current_timeline: start_timeline,
            position: 0,
            variables: HashMap::new(),
            image_size_cache: std::rc::Rc::new(std::cell::RefCell::new(HashMap::new())),
            all_buttons: Vec::new(),
            button_onclick_map: HashMap::new(),
            static_stencils: None,
            static_buttons: Vec::new(),
            expanded_body: None,
            cached_window_size: None,
            component_context: ComponentContext::new(),
            last_hovered_button: None,
            focused_text_input: None,
            text_input_values: HashMap::new(),
            ime_composition_text: HashMap::new(),
            text_cursor_positions: HashMap::new(),
            text_selections: HashMap::new(),
        }
    }

    #[inline]
    pub fn current_timeline<'a>(&self, app: &'a App) -> Option<&'a Timeline> {
        app.timelines.iter().find(|t| t.name == self.current_timeline)
    }

    #[inline]
    pub fn current_node<'a>(&self, app: &'a App) -> Option<&'a WithSpan<ViewNode>> {
        self.current_timeline(app).and_then(|tl| tl.body.get(self.position))
    }

    pub fn jump_to_timeline(&mut self, timeline_name: &str) {
        self.current_timeline = timeline_name.to_string();
        self.position = 0;
        // キャッシュクリア
        self.static_stencils = None;
        self.static_buttons.clear();
        self.expanded_body = None;
        self.cached_window_size = None;
    }

    #[inline]
    pub fn get_image_size(&self, path: &str) -> (u32, u32) {
        let cache = self.image_size_cache.borrow();
        cache.get(path).copied().unwrap_or((100, 100))
    }

    pub fn advance(&mut self) {
        self.position += 1;
    }

    // ★ 新規追加: テキスト入力とIME関連のメソッド

    /// テキスト入力フィールドにフォーカスを設定
    pub fn focus_text_input(&mut self, field_id: String) {
        self.focused_text_input = Some(field_id.clone());
        // フィールドが存在しない場合は初期化
        if !self.text_input_values.contains_key(&field_id) {
            self.text_input_values.insert(field_id.clone(), String::new());
        }
        if !self.text_cursor_positions.contains_key(&field_id) {
            self.text_cursor_positions.insert(field_id.clone(), 0);
        }
        if !self.text_selections.contains_key(&field_id) {
            self.text_selections.insert(field_id.clone(), (0, 0));
        }
    }

    /// テキスト入力フィールドのフォーカスを解除
    pub fn blur_text_input(&mut self) {
        self.focused_text_input = None;
    }

    /// 現在フォーカスされているテキスト入力フィールドのIDを取得
    pub fn get_focused_text_input(&self) -> Option<&String> {
        self.focused_text_input.as_ref()
    }

    /// テキスト入力フィールドの値を設定
    pub fn set_text_input_value(&mut self, field_id: String, value: String) {
        self.text_input_values.insert(field_id.clone(), value.clone());
        // カーソル位置を文字列の最後に設定
        let cursor_pos = value.chars().count();
        self.text_cursor_positions.insert(field_id, cursor_pos);
    }

    /// テキスト��力フィールドの値を取得
    pub fn get_text_input_value(&self, field_id: &str) -> String {
        self.text_input_values.get(field_id).cloned().unwrap_or_default()
    }

    /// IME変換中のテキストを設定
    pub fn set_ime_composition_text(&mut self, field_id: &str, composition_text: String) {
        self.ime_composition_text.insert(field_id.to_string(), composition_text);
    }

    /// IME変換中��テキストをクリア
    pub fn clear_ime_composition_text(&mut self, field_id: &str) {
        self.ime_composition_text.remove(field_id);
    }

    /// IME変換中のテキストを取得
    pub fn get_ime_composition_text(&self, field_id: &str) -> Option<&String> {
        self.ime_composition_text.get(field_id)
    }

    /// テキストカーソル位置を設定
    pub fn set_text_cursor_position(&mut self, field_id: &str, position: usize) {
        self.text_cursor_positions.insert(field_id.to_string(), position);
    }

    /// テキストカーソル位置を取得
    pub fn get_text_cursor_position(&self, field_id: &str) -> usize {
        self.text_cursor_positions.get(field_id).copied().unwrap_or(0)
    }

    /// テキスト選択範囲を設定
    pub fn set_text_selection(&mut self, field_id: &str, start: usize, end: usize) {
        self.text_selections.insert(field_id.to_string(), (start, end));
    }

    /// テキスト選択範囲を取得
    pub fn get_text_selection(&self, field_id: &str) -> (usize, usize) {
        self.text_selections.get(field_id).copied().unwrap_or((0, 0))
    }
}

impl<S: StateAccess + 'static> AppState<S> {
    /// 値評価（軽量化版）
    pub fn eval_expr_from_ast(&self, e: &Expr) -> String {
        match e {
            Expr::String(s) => s.clone(),
            Expr::Number(n) => n.to_string(),
            Expr::Bool(b) => if *b { "true".into() } else { "false".into() },
            Expr::Ident(s) => {
                // ★ 修正: foreach変数を最優先で確認
                if let Some(v) = self.component_context.get_var(s) {
                    return v.clone();
                }

                // 識別子をそのまま返す
                s.clone()
            }
            Expr::Path(s) => {
                // ★ 修正: path専用の処理

                // state.プレフィックスがある場合のみカスタム状態を参照
                if s.starts_with("state.") {
                    let field_name = s.strip_prefix("state.").unwrap();
                    if let Some(v) = <S as crate::engine::state::StateAccess>::get_field(&self.custom_state, field_name) {
                        return v;
                    }
                    return s.clone();
                }

                // foreach変数やコンポーネント引数もチェック
                if let Some(v) = self.component_context.get_var(s) {
                    return v.clone();
                }

                // 識別子をそのまま返す
                s.clone()
            }
            Expr::Array(xs) => {
                let vs: Vec<String> = xs.iter().map(|x| {
                    let val = self.eval_expr_from_ast(x);
                    // 文字列の場合はクォートで囲む（JSON形式にする）
                    // 数値やDimensionの場合は、純粋な数値として扱う
                    match x {
                        Expr::String(_) => format!("\"{}\"", val),
                        Expr::Number(_) => val, // 数値はそのまま
                        Expr::Dimension(d) => d.value.to_string(), // Dimensionは数値部分のみ
                        _ => {
                            // その他の場合も数値かどうか判定してクォートを制御
                            if val.parse::<f64>().is_ok() {
                                val // 数値の場合はそのまま
                            } else {
                                format!("\"{}\"", val) // 文字列の場合はクォート
                            }
                        }
                    }
                }).collect();
                format!("[{}]", vs.join(","))
            }
            Expr::Object(_) => "<object>".into(),
            Expr::Dimension(d) => {
                format!("{}{}", d.value, match d.unit {
                    crate::parser::ast::Unit::Px => "px",
                    crate::parser::ast::Unit::Vw => "vw",
                    crate::parser::ast::Unit::Vh => "vh",
                    crate::parser::ast::Unit::Percent => "%",
                    crate::parser::ast::Unit::PercentHeight => "%h",
                    crate::parser::ast::Unit::Rem => "rem",
                    crate::parser::ast::Unit::Em => "em",
                })
            }
            Expr::Match { expr, arms, default } => {
                let match_value = self.eval_expr_from_ast(expr);

                for arm in arms {
                    let pattern_value = self.eval_expr_from_ast(&arm.pattern);
                    if pattern_value == match_value {
                        return self.eval_expr_from_ast(&arm.value);
                    }
                }

                if let Some(default_expr) = default {
                    return self.eval_expr_from_ast(default_expr);
                }

                String::new()
            }
            Expr::FunctionCall { name, args } => {
                // 関数呼び出しを実行
                self.execute_function_call(name, args)
            }
        }
    }

    /// 関数呼び���しの実行
    fn execute_function_call(&self, name: &str, args: &[Expr]) -> String {
        // 引数を評価
        let _arg_values: Vec<String> = args.iter().map(|arg| self.eval_expr_from_ast(arg)).collect();

        // まず登録さ��たRust関数を試す
        use crate::engine::rust_call::{execute_rust_call, has_rust_call};

        // 登録された���数があるかチェック
        if has_rust_call(name) {
            execute_rust_call(name, args);
            return format!("{}_executed", name);
        }

        // フォールバック：内部関数
        match name {
            "test" => {
                println!("test() function called!");
                "test_executed".to_string()
            }
            "click_test" => {
                println!("click_test() function called!");
                "click_test_executed".to_string()
            }
            _ => {
                println!("Unknown function: {}", name);
                format!("unknown_function({})", name)
            }
        }
    }

    /// 軽量化���れたステンシル変換メソッド（従来のAPIとの互換性維持）
    pub fn viewnode_layouted_to_stencil(
        &mut self,
        lnode: &crate::ui::LayoutedNode<'_>,
        _app: &App,
        out: &mut Vec<Stencil>,
        mouse_pos: [f32; 2],
        _mouse_down: bool,
        _prev_mouse_down: bool,
    ) {
        let mut depth_counter = 0.0;
        self.viewnode_layouted_to_stencil_lightweight(lnode, out, mouse_pos, &mut depth_counter);
    }

    /// 軽量化されたステンシル��換（内部用）
    fn viewnode_layouted_to_stencil_lightweight(
        &mut self,
        lnode: &crate::ui::LayoutedNode<'_>,
        out: &mut Vec<Stencil>,
        mouse_pos: [f32; 2],
        depth_counter: &mut f32,
    ) {
        let style = lnode.node.style.clone().unwrap_or_default();
        let is_hover = point_in_rect(mouse_pos, lnode.position, lnode.size);

        // ★ デバッグ出力: レンダリング段階でのサイズ確認
        if let Some(ref rel_width) = style.relative_width {
            println!("🎨 Render DEBUG: position: {:?}, size: {:?}, rel_width: {:?}",
                lnode.position, lnode.size, rel_width);
        }
        if let Some(ref rel_height) = style.relative_height {
            println!("🎨 Render DEBUG: position: {:?}, size: {:?}, rel_height: {:?}",
                lnode.position, lnode.size, rel_height);
        }

        // 借用エラーを修正：hoverスタイルのマージを安全に��う
        let final_style = if is_hover {
            if let Some(ref hover_style) = style.hover {
                style.merged(hover_style)
            } else {
                style
            }
        } else {
            style
        };

        // Card スタイルの適用
        let final_style = if final_style.card.unwrap_or(false) {
            let mut card_style = final_style;
            if card_style.background.is_none() { card_style.background = Some(ColorValue::Hex("#ffffff".into())); }
            if card_style.rounded.is_none()    { card_style.rounded    = Some(Rounded::Px(16.0)); }
            if card_style.padding.is_none()    { card_style.padding    = Some(Edges::all(20.0)); }
            if card_style.shadow.is_none()     { card_style.shadow     = Some(Shadow::On); }
            card_style
        } else {
            final_style
        };

        match &lnode.node.node {
            ViewNode::VStack(_) | ViewNode::HStack(_) | ViewNode::DynamicSection { .. } |
            ViewNode::Match { .. } | ViewNode::When { .. } => {
                self.render_container_background(lnode, &final_style, out, depth_counter);
            }

            ViewNode::Text { format, args } => {
                self.render_text_optimized(lnode, format, args, &final_style, out, depth_counter);
            }

            ViewNode::Button { label, id, onclick: _ } => {
                self.render_button_optimized(lnode, label, id, &final_style, is_hover, out, depth_counter);
            }

            ViewNode::Image { path } => {
                self.render_image_optimized(lnode, path, &final_style, out, depth_counter);
            }

            ViewNode::Stencil(st) => {
                let dx = lnode.position[0];
                let dy = lnode.position[1];
                let mut offset_st = offset_stencil(st, dx, dy);
                adjust_stencil_depth_dynamic(&mut offset_st, depth_counter);
                out.push(offset_st);
            }

            _ => { /* その他のノードは描画なし */ }
        }
    }

    /// 軽量化された��ンテナ背景描画
    fn render_container_background(
        &self,
        lnode: &crate::ui::LayoutedNode<'_>,
        style: &Style,
        out: &mut Vec<Stencil>,
        depth_counter: &mut f32,
    ) {
        if let Some(bg) = &style.background {
            let color = to_rgba(bg);

            // ★ 重要: 透明色の場合は背景を描画しない
            if color[3] == 0.0 {
                return;
            }

            let radius = style.rounded
                .map(|r| match r {
                    Rounded::On => 8.0,
                    Rounded::Px(v) => v,
                })
                .unwrap_or(0.0);

            // 影
            if let Some(sh) = style.shadow.clone() {
                let (off, scol) = match sh {
                    Shadow::On => ([0.0, 2.0], [0.0, 0.0, 0.0, 0.2]),
                    Shadow::Spec { offset, color, .. } => {
                        let scol = color.as_ref().map(to_rgba).unwrap_or([0.0, 0.0, 0.0, 0.2]);
                        (offset, scol)
                    }
                };

                *depth_counter += 0.001;
                out.push(Stencil::RoundedRect {
                    position: [lnode.position[0] + off[0], lnode.position[1] + off[1]],
                    width: lnode.size[0],
                    height: lnode.size[1],
                    radius,
                    color: [scol[0], scol[1], scol[2], (scol[3] * 0.9).min(1.0)],
                    scroll: true,
                    depth: (1.0 - *depth_counter).max(0.0),
                });
            }

            *depth_counter += 0.001;
            out.push(Stencil::RoundedRect {
                position: lnode.position,
                width: lnode.size[0],
                height: lnode.size[1],
                radius,
                color,
                scroll: true,
                depth: (1.0 - *depth_counter).max(0.0),
            });
        }
    }

    /// 軽量化されたテキスト描画
    fn render_text_optimized(
        &self,
        lnode: &crate::ui::LayoutedNode<'_>,
        format: &str,
        args: &[Expr],
        style: &Style,
        out: &mut Vec<Stencil>,
        depth_counter: &mut f32,
    ) {
        let values: Vec<String> = args.iter().map(|e| self.eval_expr_from_ast(e)).collect();
        let content = format_text(format, &values[..]);

        if content.is_empty() && !args.is_empty() {
            return; // 空のテキストは描画しない
        }

        let font_size = style.font_size.unwrap_or(16.0);
        let font = style.font.clone().unwrap_or_else(|| "default".to_string());
        let text_color = style.color.as_ref().map(to_rgba).unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let p = style.padding.unwrap_or(Edges::default());

        *depth_counter += 0.001;
        out.push(Stencil::Text {
            content,
            position: [lnode.position[0] + p.left, lnode.position[1] + p.top],
            size: font_size,
            color: text_color,
            font,
            scroll: true,
            depth: (1.0 - *depth_counter).max(0.0),
        });
    }

    /// 軽量化されたボタン描画
    fn render_button_optimized(
        &mut self,
        lnode: &crate::ui::LayoutedNode<'_>,
        label: &str,
        _id: &str,
        style: &Style,
        is_hover: bool,
        out: &mut Vec<Stencil>,
        depth_counter: &mut f32,
    ) {
        let radius = style.rounded
            .map(|r| match r { Rounded::On => 8.0, Rounded::Px(v) => v })
            .unwrap_or(6.0);

        let bg = style.background.as_ref().map(to_rgba).unwrap_or(
            if is_hover { [0.09, 0.46, 0.82, 1.0] } else { [0.13, 0.59, 0.95, 1.0] }
        );

        let text_color = style.color.as_ref().map(to_rgba).unwrap_or([1.0, 1.0, 1.0, 1.0]);
        let font_size = style.font_size.unwrap_or(16.0);
        let font = style.font.clone().unwrap_or_else(|| "default".to_string());

        // 影
        if let Some(sh) = style.shadow.clone() {
            let (off, scol) = match sh {
                Shadow::On => ([0.0, 2.0], [0.0, 0.0, 0.0, 0.25]),
                Shadow::Spec { offset, color, .. } => {
                    let scol = color.as_ref().map(to_rgba).unwrap_or([0.0, 0.0, 0.0, 0.25]);
                    (offset, scol)
                }
            };

            *depth_counter += 0.001;
            out.push(Stencil::RoundedRect {
                position: [lnode.position[0] + off[0], lnode.position[1] + off[1]],
                width: lnode.size[0],
                height: lnode.size[1],
                radius,
                color: [scol[0], scol[1], scol[2], (scol[3] * 0.9).min(1.0)],
                scroll: true,
                depth: (1.0 - *depth_counter).max(0.0),
            });
        }

        // ★ 重要: 透明色の場合は背景を描画し��い
        if bg[3] > 0.0 {
            // 背景
            *depth_counter += 0.001;
            out.push(Stencil::RoundedRect {
                position: lnode.position,
                width: lnode.size[0],
                height: lnode.size[1],
                radius,
                color: bg,
                scroll: true,
                depth: (1.0 - *depth_counter).max(0.0),
            });
        }

        // テキスト（中央寄せ）
        let text_w = (label.chars().count() as f32) * font_size * 0.55;
        let text_h = font_size * 1.2;
        let tx = lnode.position[0] + (lnode.size[0] - text_w) * 0.5;
        let ty = lnode.position[1] + (lnode.size[1] - text_h) * 0.5;

        *depth_counter += 0.001;
        out.push(Stencil::Text {
            content: label.to_string(),
            position: [tx, ty],
            size: font_size,
            color: text_color,
            font,
            scroll: true,
            depth: (1.0 - *depth_counter).max(0.0),
        });

        // ボタン境界をここでは追加しない（engine.rsで管理）
        // self.all_buttons.push((id.to_string(), lnode.position, lnode.size));
    }

    /// 軽量化された画像描��
    fn render_image_optimized(
        &self,
        lnode: &crate::ui::LayoutedNode<'_>,
        path: &str,
        style: &Style,
        out: &mut Vec<Stencil>,
        depth_counter: &mut f32,
    ) {
        // 背景（あれば���
        if let Some(bg) = &style.background {
            let radius = style.rounded
                .map(|r| match r { Rounded::On => 8.0, Rounded::Px(v) => v })
                .unwrap_or(0.0);

            *depth_counter += 0.001;
            out.push(Stencil::RoundedRect {
                position: lnode.position,
                width: lnode.size[0],
                height: lnode.size[1],
                radius,
                color: to_rgba(bg),
                scroll: true,
                depth: (1.0 - *depth_counter).max(0.0),
            });
        }

        // 画像自体
        *depth_counter += 0.001;
        out.push(Stencil::Image {
            position: lnode.position,
            width: lnode.size[0],
            height: lnode.size[1],
            path: path.to_string(),
            scroll: true,
            depth: (1.0 - *depth_counter).max(0.0),
        });
    }

    // 互換性維持のためのヘルパーメソッド
    pub fn viewnode_layouted_to_stencil_with_depth(
        &mut self,
        lnode: &crate::ui::LayoutedNode<'_>,
        _app: &App,
        out: &mut Vec<Stencil>,
        mouse_pos: [f32; 2],
        _mouse_down: bool,
        _prev_mouse_down: bool,
        nest_level: u32,
    ) {
        let mut depth_counter = (nest_level as f32) * 0.1;
        self.viewnode_layouted_to_stencil_lightweight(lnode, out, mouse_pos, &mut depth_counter);
    }

    pub fn viewnode_layouted_to_stencil_with_depth_counter_helper(
        &mut self,
        lnode: &crate::ui::LayoutedNode<'_>,
        _app: &App,
        out: &mut Vec<Stencil>,
        mouse_pos: [f32; 2],
        _mouse_down: bool,
        _prev_mouse_down: bool,
        _nest_level: u32,
        depth_counter: &mut f32,
    ) {
        self.viewnode_layouted_to_stencil_lightweight(lnode, out, mouse_pos, depth_counter);
    }

    /// Rustコール実行メソッド
    pub fn execute_rust_call(&mut self, name: &str, args: &[Expr]) -> bool {
        // ���ずstateアクセス可能な関数を試す
        let result = crate::engine::rust_call::execute_state_accessible_call(name, self, args);
        if result {
            return true;
        }

        // ���来の関数を試す
        crate::engine::rust_call::execute_rust_call(name, args);
        true
    }

    /// ViewNodeからRustコール��処理
    pub fn handle_rust_call_viewnode(&mut self, name: &str, args: &[Expr]) {
        if !self.execute_rust_call(name, args) {
            eprintln!("Warning: Rust call '{}' failed to execute", name);
        }
    }
}

// 軽量化されたユーティリティ関数群
#[inline]
pub fn format_text(fmt: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(fmt.len() + args.iter().map(|s| s.len()).sum::<usize>());
    let mut i = 0;
    let mut it = fmt.chars().peekable();

    while let Some(c) = it.next() {
        if c == '{' && it.peek() == Some(&'}') {
            it.next();
            if let Some(v) = args.get(i) {
                out.push_str(v);
            } else {
                out.push_str("{}");
            }
            i += 1;
        } else {
            out.push(c);
        }
    }

    if out.is_empty() && !fmt.is_empty() {
        return fmt.to_string();
    }

    out
}

#[inline]
fn point_in_rect(m: [f32; 2], p: [f32; 2], s: [f32; 2]) -> bool {
    m[0] >= p[0] && m[0] <= p[0] + s[0] && m[1] >= p[1] && m[1] <= p[1] + s[1]
}

#[inline]
fn to_rgba(c: &ColorValue) -> [f32; 4] {
    match c {
        ColorValue::Rgba(v) => *v,
        ColorValue::Hex(s) => hex_to_rgba(s),
    }
}

#[inline]
fn hex_to_rgba(s: &str) -> [f32; 4] {
    let t = s.trim().trim_start_matches('#');
    let parse2 = |h: &str| u8::from_str_radix(h, 16).unwrap_or(0) as f32 / 255.0;

    match t.len() {
        3 => {
            let r = parse2(&t[0..1].repeat(2));
            let g = parse2(&t[1..2].repeat(2));
            let b = parse2(&t[2..3].repeat(2));
            [r, g, b, 1.0]
        }
        6 => {
            let r = parse2(&t[0..2]);
            let g = parse2(&t[2..4]);
            let b = parse2(&t[4..6]);
            [r, g, b, 1.0]
        }
        8 => {
            let r = parse2(&t[0..2]);
            let g = parse2(&t[2..4]);
            let b = parse2(&t[4..6]);
            let a = parse2(&t[6..8]);
            [r, g, b, a]
        }
        _ => [0.0, 0.0, 0.0, 1.0],
    }
}

// StateAccess: AppState::custom_state に対する共通アクセス
pub trait StateAccess {
    fn get_field(&self, key: &str) -> Option<String>;
    fn set(&mut self, _path: &str, _value: String) -> Result<(), String>;
    fn toggle(&mut self, _path: &str) -> Result<(), String>;
    fn list_append(&mut self, _path: &str, _value: String) -> Result<(), String>;
    fn list_remove(&mut self, _path: &str, _index: usize) -> Result<(), String>;
}

#[inline]
fn offset_stencil(st: &Stencil, dx: f32, dy: f32) -> Stencil {
    let mut result = st.clone();
    match &mut result {
        Stencil::Rect { position, .. } |
        Stencil::RoundedRect { position, .. } |
        Stencil::Text { position, .. } |
        Stencil::Image { position, .. } => {
            position[0] += dx;
            position[1] += dy;
        }
        Stencil::Circle { center, .. } => {
            center[0] += dx;
            center[1] += dy;
        }
        Stencil::Triangle { p1, p2, p3, .. } => {
            p1[0] += dx; p1[1] += dy;
            p2[0] += dx; p2[1] += dy;
            p3[0] += dx; p3[1] += dy;
        }
        _ => {}
    }
    result
}

#[inline]
fn adjust_stencil_depth_dynamic(stencil: &mut Stencil, depth_counter: &mut f32) {
    *depth_counter += 0.001;
    let new_depth = (1.0 - *depth_counter).max(0.0);

    match stencil {
        Stencil::Rect { depth, .. } |
        Stencil::RoundedRect { depth, .. } |
        Stencil::Circle { depth, .. } |
        Stencil::Triangle { depth, .. } |
        Stencil::Text { depth, .. } |
        Stencil::Image { depth, .. } |
        Stencil::ScrollBar { depth, .. } => {
            *depth = new_depth;
        }
        Stencil::Group(children) => {
            for child in children {
                adjust_stencil_depth_dynamic(child, depth_counter);
            }
        }
    }
}
