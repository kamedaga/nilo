//! Runtime module for Nilo engine
//!
//! This module contains the runtime implementation for native (non-WASM) environments.
//! For WASM environments, see runtime_dom.rs

// Native環境専用のruntime
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::parser::ast::App;
    use crate::stencil::stencil::stencil_to_wgpu_draw_list;
    use crate::ui::event::{EventQueue, UIEvent};
    use crate::ui::viewport;
    #[cfg(feature = "wgpu")]
    use crate::wgpu_renderer::wgpu::WgpuRenderer;
    use log::{debug, info};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex}; // ログマクロを追加


    use winit::{
        application::ApplicationHandler,
        event::{ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
        event_loop::{ActiveEventLoop, EventLoop},
        keyboard::{KeyCode, PhysicalKey},
        window::{Window, WindowAttributes, WindowId},
    };
    use winit::dpi::PhysicalSize;

    use crate::engine::core::Engine;
    use crate::engine::state::{AppState, StateAccess};

    pub struct AppHandler<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        app: Arc<App>,
        state: AppState<S>,
        window: Option<Arc<Window>>,
        renderer: Option<WgpuRenderer>,
        event_queue: EventQueue,
        button_handlers: HashMap<String, Box<dyn FnMut(&mut AppState<S>)>>,
        scroll_offset: [f32; 2],
        content_length: f32,
        target_scroll_offset: [f32; 2],
        smoothing: f32,
        mouse_pos_raw: [f32; 2],
        mouse_pos: [f32; 2],
        mouse_down: bool,
        prev_mouse_down: bool,
        last_hovered_button: Option<String>, // ホバー状態追跡用
        window_title: String,                // ウィンドウタイトル
        // リサイズのデバウンス用（直近サイズを保持し、描画前に一度だけ適用）
        pending_resize: Option<PhysicalSize<u32>>,
    }

    impl<S> AppHandler<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        pub fn new(app: Arc<App>, state: AppState<S>, window_title: String) -> Self {
            Self {
                app,
                state,
                window: None,
                renderer: None,
                event_queue: EventQueue::new(),
                button_handlers: HashMap::new(),
                scroll_offset: [0.0, 0.0],
                content_length: 0.0,
                target_scroll_offset: [0.0, 0.0],
                smoothing: 0.5,
                mouse_pos_raw: [0.0, 0.0],
                mouse_pos: [0.0, 0.0],
                mouse_down: false,
                prev_mouse_down: false,
                last_hovered_button: None, // 初期化
                window_title,
                pending_resize: None,
            }
        }

        /// フレームカウントと経過時間を更新（dynamic_section用）
        #[allow(dead_code)]
        fn update_frame_state(&mut self) {
            // frame_countフィールドがあれば更新
            if let Some(current_frame_str) = self.state.custom_state.get_field("frame_count") {
                if let Ok(current_frame) = current_frame_str.parse::<u32>() {
                    let _ = self
                        .state
                        .custom_state
                        .set("frame_count", (current_frame + 1).to_string());
                }
            }

            // elapsed_timeフィールドがあれば更新
            if let Some(current_time_str) = self.state.custom_state.get_field("elapsed_time") {
                if let Ok(current_time) = current_time_str.parse::<f32>() {
                    // 60FPSを想定して時間を更新（約0.0167秒/フレーム）
                    let _ = self
                        .state
                        .custom_state
                        .set("elapsed_time", format!("{:.3}", current_time + 0.0167));
                }
            }
        }
    }

    impl<S> ApplicationHandler for AppHandler<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_none() {
                let window_attributes = WindowAttributes::default().with_title(&self.window_title);
                let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

                // ★ IME対応: ウィンドウでIMEを有効化
                window.set_ime_allowed(true);

                self.renderer = Some(pollster::block_on(WgpuRenderer::new(window.clone())));
                self.window = Some(window);
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            let window = match &self.window {
                Some(window) => window,
                None => return,
            };
            let renderer = match &mut self.renderer {
                Some(renderer) => renderer,
                None => return,
            };

            let scale_factor = window.scale_factor() as f32;

            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(size) => {
                    // ウィンドウサイズが0の場合は何もしない（最小化時など）
                    if size.width == 0 || size.height == 0 {
                        return;
                    }
                    // ここでは重い再設定は行わず、直近サイズを記録して再描画要求のみ行う
                    self.pending_resize = Some(size);
                    window.request_redraw();
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let viewport_height = renderer.size().height as f32 / scale_factor;
                    let max_scroll = (self.content_length - viewport_height).max(0.0);
                    let y = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y * 15.0,
                        MouseScrollDelta::PixelDelta(pos) => -pos.y as f32 / scale_factor,
                    };
                    self.target_scroll_offset[1] =
                        (self.target_scroll_offset[1] + y).clamp(-max_scroll, 0.0);
                    window.request_redraw();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let old_mouse_pos = self.mouse_pos;
                    self.mouse_pos_raw = [position.x as f32, position.y as f32];
                    self.mouse_pos = [
                        self.mouse_pos_raw[0] / scale_factor,
                        (self.mouse_pos_raw[1] / scale_factor) - self.scroll_offset[1],
                    ];

                    // マウス座標が変わった場合は再描画を要求（ホバー状態が変わる可能性）
                    if (old_mouse_pos[0] - self.mouse_pos[0]).abs() > 0.5
                        || (old_mouse_pos[1] - self.mouse_pos[1]).abs() > 0.5
                    {
                        // ホバー状態変化の検出のためキャッシュを無効化
                        self.state.static_stencils = None;
                        self.state.static_text_inputs.clear();
                        window.request_redraw();
                    }
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => {
                    self.mouse_down = true;
                    window.request_redraw(); // マウス押下時も再描画
                }
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Left,
                    ..
                } => {
                    self.mouse_down = false;

                    // テキスト入力フィールドのクリック処理（直近レイアウトのall_text_inputsを使用）
                    let mut text_input_clicked = None;
                    for (id, pos, size) in &self.state.all_text_inputs {
                        let hover = {
                            let x = self.mouse_pos[0];
                            let y = self.mouse_pos[1];
                            x >= pos[0]
                                && x <= pos[0] + size[0]
                                && y >= pos[1]
                                && y <= pos[1] + size[1]
                        };

                        if hover {
                            text_input_clicked = Some(id.clone());
                            break;
                        }
                    }

                    // テキスト入力フィールドがクリックされた場合
                    if let Some(field_id) = text_input_clicked {
                        self.state.focus_text_input(field_id.clone());
                        self.event_queue.push(UIEvent::TextFocused { field_id });
                    } else {
                        // 他の場所がクリックされた場合はフォーカスを解除
                        if self.state.get_focused_text_input().is_some() {
                            if let Some(prev_focused) = self.state.get_focused_text_input().cloned()
                            {
                                self.state.blur_text_input();
                                self.event_queue.push(UIEvent::TextBlurred {
                                    field_id: prev_focused,
                                });
                            }
                        }
                    }

                    window.request_redraw(); // マウス離し時も再描画
                }
                // ★ IME対応: キーボード入力とIME関連のイベント処理を追加
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key,
                            state: ElementState::Pressed,
                            text,
                            ..
                        },
                    ..
                } => {
                    if let Some(focused_field) = self.state.get_focused_text_input().cloned() {
                        match physical_key {
                            PhysicalKey::Code(KeyCode::Backspace) => {
                                // IMEプレエディット中はBackspaceはIME側に任せる
                                if self
                                    .state
                                    .get_ime_composition_text(&focused_field)
                                    .is_none()
                                {
                                    // バックスペース処理
                                    let current_value = self.state.get_text_input_value(&focused_field);
                                    if !current_value.is_empty() {
                                        let cursor_pos =
                                            self.state.get_text_cursor_position(&focused_field);
                                        if cursor_pos > 0 {
                                            let mut chars: Vec<char> = current_value.chars().collect();
                                            chars.remove(cursor_pos - 1);
                                            let new_value: String = chars.into_iter().collect();
                                            self.state.set_text_input_value(
                                                focused_field.clone(),
                                                new_value.clone(),
                                            );
                                            self.state.set_text_cursor_position(
                                                &focused_field,
                                                cursor_pos - 1,
                                            );
                                            self.event_queue.push(UIEvent::TextChanged {
                                                field_id: focused_field,
                                                new_value,
                                            });
                                        }
                                    }
                                }
                            }
                            PhysicalKey::Code(KeyCode::Delete) => {
                                // IMEプレエディット中はDeleteはIME側に任せる
                                if self
                                    .state
                                    .get_ime_composition_text(&focused_field)
                                    .is_none()
                                {
                                    // Delete処理
                                    let current_value = self.state.get_text_input_value(&focused_field);
                                    let cursor_pos =
                                        self.state.get_text_cursor_position(&focused_field);
                                    let chars: Vec<char> = current_value.chars().collect();
                                    if cursor_pos < chars.len() {
                                        let mut new_chars = chars;
                                        new_chars.remove(cursor_pos);
                                        let new_value: String = new_chars.into_iter().collect();
                                        self.state.set_text_input_value(
                                            focused_field.clone(),
                                            new_value.clone(),
                                        );
                                        self.event_queue.push(UIEvent::TextChanged {
                                            field_id: focused_field,
                                            new_value,
                                        });
                                    }
                                }
                            }
                            PhysicalKey::Code(KeyCode::Enter) => {
                                // エンター押下時の処理
                                self.event_queue.push(UIEvent::TextSubmitted {
                                    field_id: focused_field,
                                });
                            }
                            PhysicalKey::Code(KeyCode::Escape) => {
                                // エスケープでフォーカス解除
                                self.state.blur_text_input();
                                self.event_queue.push(UIEvent::TextBlurred {
                                    field_id: focused_field,
                                });
                            }
                            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                                // カーソル移動（左）
                                let current_pos =
                                    self.state.get_text_cursor_position(&focused_field);
                                if current_pos > 0 {
                                    self.state
                                        .set_text_cursor_position(&focused_field, current_pos - 1);
                                }
                            }
                            PhysicalKey::Code(KeyCode::ArrowRight) => {
                                // カーソル移動（右）
                                let current_value = self.state.get_text_input_value(&focused_field);
                                let current_pos =
                                    self.state.get_text_cursor_position(&focused_field);
                                let max_pos = current_value.chars().count();
                                if current_pos < max_pos {
                                    self.state
                                        .set_text_cursor_position(&focused_field, current_pos + 1);
                                }
                            }
                            PhysicalKey::Code(KeyCode::Home) => {
                                // 行の先頭に移動
                                self.state.set_text_cursor_position(&focused_field, 0);
                            }
                            PhysicalKey::Code(KeyCode::End) => {
                                // 行の末尾に移動
                                let current_value = self.state.get_text_input_value(&focused_field);
                                let max_pos = current_value.chars().count();
                                self.state.set_text_cursor_position(&focused_field, max_pos);
                            }
                            _ => {
                                // 通常の文字入力（textがある場合）
                                if let Some(text) = text {
                                    if !text.is_empty() && text.chars().all(|c| !c.is_control()) {
                                        let current_value =
                                            self.state.get_text_input_value(&focused_field);
                                        let cursor_pos =
                                            self.state.get_text_cursor_position(&focused_field);

                                        // カーソル位置に文字を挿入
                                        let mut chars: Vec<char> = current_value.chars().collect();
                                        for (i, c) in text.chars().enumerate() {
                                            chars.insert(cursor_pos + i, c);
                                        }

                                        let new_value: String = chars.into_iter().collect();
                                        let new_cursor_pos = cursor_pos + text.chars().count();

                                        self.state.set_text_input_value(
                                            focused_field.clone(),
                                            new_value.clone(),
                                        );
                                        self.state.set_text_cursor_position(
                                            &focused_field,
                                            new_cursor_pos,
                                        );
                                        self.event_queue.push(UIEvent::TextChanged {
                                            field_id: focused_field,
                                            new_value,
                                        });
                                    }
                                }
                            }
                        }
                        window.request_redraw(); // テキスト入力時は再描画
                    }
                }
                // ★ IME対応: IME関連のイベント処理
                WindowEvent::Ime(ime_event) => {
                    if let Some(focused_field) = self.state.get_focused_text_input().cloned() {
                        match ime_event {
                            Ime::Preedit(preedit_text, cursor_range) => {
                                // IME変換中のテキスト（下線付きテキスト）
                                self.state
                                    .set_ime_composition_text(&focused_field, preedit_text.clone());
                                self.event_queue.push(UIEvent::ImeComposition {
                                    field_id: focused_field,
                                    composition_text: preedit_text,
                                    cursor_range: cursor_range.map(|(start, end)| (start, end)),
                                });
                            }
                            Ime::Commit(committed_text) => {
                                // IME確定テキスト - カーソル位置に挿入
                                self.state.clear_ime_composition_text(&focused_field);
                                let current_value = self.state.get_text_input_value(&focused_field);
                                let cursor_pos =
                                    self.state.get_text_cursor_position(&focused_field);

                                // カーソル位置に確定テキストを挿入
                                let mut chars: Vec<char> = current_value.chars().collect();
                                for (i, c) in committed_text.chars().enumerate() {
                                    chars.insert(cursor_pos + i, c);
                                }

                                let new_value: String = chars.into_iter().collect();
                                let new_cursor_pos = cursor_pos + committed_text.chars().count();

                                self.state
                                    .set_text_input_value(focused_field.clone(), new_value.clone());
                                self.state
                                    .set_text_cursor_position(&focused_field, new_cursor_pos);

                                self.event_queue.push(UIEvent::ImeCommit {
                                    field_id: focused_field.clone(),
                                    committed_text: committed_text.clone(),
                                });
                                self.event_queue.push(UIEvent::TextChanged {
                                    field_id: focused_field,
                                    new_value,
                                });
                            }
                            Ime::Enabled => {
                                // IME有効化
                                self.event_queue.push(UIEvent::ImeEnabled {
                                    field_id: focused_field,
                                });
                            }
                            Ime::Disabled => {
                                // IME無効化
                                self.state.clear_ime_composition_text(&focused_field);
                                self.event_queue.push(UIEvent::ImeDisabled {
                                    field_id: focused_field,
                                });
                            }
                        }
                        window.request_redraw();
                    }
                }
                WindowEvent::RedrawRequested => {
                    // リサイズが保留されていればここで一度だけ適用（デバウンス）
                    if let Some(size) = self.pending_resize.take() {
                        let viewport_height = size.height as f32 / scale_factor;
                        let max_scroll = (self.content_length - viewport_height).max(0.0);
                        renderer.resize(size);
                        if max_scroll <= 0.0 {
                            self.scroll_offset[1] = 0.0;
                            self.target_scroll_offset[1] = 0.0;
                        } else if self.scroll_offset[1] < -max_scroll {
                            self.scroll_offset[1] = -max_scroll;
                            self.target_scroll_offset[1] = -max_scroll;
                        }
                        // レイアウトキャッシュを無効化
                    self.state.static_stencils = None;
                    self.state.static_buttons.clear();
                    self.state.static_text_inputs.clear();
                        self.state.static_text_inputs.clear();
                    }
                    // ウィンドウサイズを正しく取得
                    let size = renderer.size();
                    let window_size = [
                        size.width as f32 / scale_factor,
                        size.height as f32 / scale_factor,
                    ];

                    // フレーム状態を更新（dynamic_section用）
                    // frame_countフィールドがあれば更新
                    if let Some(current_frame_str) =
                        self.state.custom_state.get_field("frame_count")
                    {
                        if let Ok(current_frame) = current_frame_str.parse::<u32>() {
                            let _ = self
                                .state
                                .custom_state
                                .set("frame_count", (current_frame + 1).to_string());
                        }
                    }
                    // elapsed_timeフィールドがあれば更新
                    if let Some(current_time_str) =
                        self.state.custom_state.get_field("elapsed_time")
                    {
                        if let Ok(current_time) = current_time_str.parse::<f32>() {
                            let _ = self
                                .state
                                .custom_state
                                .set("elapsed_time", format!("{:.3}", current_time + 0.0167));
                        }
                    }

                    // スクロール補正
                    self.scroll_offset[1] +=
                        (self.target_scroll_offset[1] - self.scroll_offset[1]) * self.smoothing;

                    // マウス座標の正確な計算（スクロールオフセット考慮）
                    let adjusted_mouse_pos = [
                        self.mouse_pos_raw[0] / scale_factor,
                        (self.mouse_pos_raw[1] / scale_factor) - self.scroll_offset[1],
                    ];
                    self.mouse_pos = adjusted_mouse_pos;

                    // ★ 最適化: needs_redrawフラグがtrueの場合のみレイアウトを再計算
                    // ホバー状態の変化やウィンドウサイズ変更時は必ず再計算
                    let cache_invalid = self.state.cached_window_size.map_or(true, |cached| {
                        (cached[0] - window_size[0]).abs() > 1.0
                            || (cached[1] - window_size[1]).abs() > 1.0
                    });

                    let should_relayout = self.state.needs_redraw
                        || cache_invalid
                        || self.state.static_stencils.is_none();

                    let (stencils, buttons, text_inputs) = if should_relayout {
                        // 状態変更があった場合のみレイアウトを再計算
                        let result = Engine::layout_and_stencil(
                            &self.app,
                            &mut self.state,
                            self.mouse_pos,
                            self.mouse_down,
                            self.prev_mouse_down,
                            window_size,
                        );
                        // フラグをリセット
                        self.state.needs_redraw = false;
                        result
                    } else {
                        // キャッシュを使用（ホバー状態のみ更新が必要な場合）
                        // 簡易版: 毎フレーム計算（後で最適化可能）
                        Engine::layout_and_stencil(
                            &self.app,
                            &mut self.state,
                            self.mouse_pos,
                            self.mouse_down,
                            self.prev_mouse_down,
                            window_size,
                        )
                    };

                    self.state.all_buttons = buttons.clone();
                    self.state.all_text_inputs = text_inputs.clone();

                    // ホバー状態とマウスイベントの処理
                    let mut current_hovered = None;
                    for (id, pos, size) in &buttons {
                        let hover = {
                            let x = self.mouse_pos[0];
                            let y = self.mouse_pos[1];
                            let in_bounds = x >= pos[0]
                                && x <= pos[0] + size[0]
                                && y >= pos[1]
                                && y <= pos[1] + size[1];

                            in_bounds
                        };

                        if hover {
                            current_hovered = Some(id.clone());
                        }

                        // マウスイベントの生成
                        if hover && self.mouse_down && !self.prev_mouse_down {
                            self.event_queue
                                .push(UIEvent::ButtonPressed { id: id.clone() });
                        }
                        if hover && !self.mouse_down && self.prev_mouse_down {
                            self.event_queue
                                .push(UIEvent::ButtonReleased { id: id.clone() });
                        }
                    }

                    // ホバー状態の変化を検出
                    if self.last_hovered_button != current_hovered {
                        self.last_hovered_button = current_hovered;
                        self.state.static_stencils = None;
                        self.state.static_text_inputs.clear();
                    }

                    let events_snapshot: Vec<UIEvent> =
                        self.event_queue.queue.iter().cloned().collect();
                    if !events_snapshot.is_empty() {
                        // when評価
                        if let Some(new_tl) =
                            Engine::step_whens(&self.app, &mut self.state, &events_snapshot)
                        {
                            info!("[INFO] Timeline changed to {}", new_tl);

                            if let Some(tl) = self.state.current_timeline(&self.app) {
                                Engine::sync_button_handlers(
                                    &tl.body,
                                    &self.app.components,
                                    &mut self.button_handlers,
                                    |id| {
                                        let id = id.to_owned();
                                        Box::new(move |_st| {
                                            debug!("Button '{}' pressed (default handler)", id)
                                        }) // println!をdebug!に変更
                                    },
                                );
                            }

                            // タイムライン変更後は描画を更新
                            let (new_stencils, new_buttons, _new_text_inputs) =
                                Engine::layout_and_stencil(
                                    &self.app,
                                    &mut self.state,
                                    self.mouse_pos,
                                    self.mouse_down,
                                    self.prev_mouse_down,
                                    window_size,
                                );

                            self.state.all_buttons = new_buttons;

                            // 新しいレイアウトで描画
                            let size = renderer.size();
                            let viewport_h = size.height as f32 / scale_factor;
                            let viewport_w = size.width as f32 / scale_factor;
                            let mut vis = viewport::filter_visible_stencils(
                                &new_stencils,
                                self.scroll_offset,
                                viewport_h,
                            );
                            let draw_full = stencil_to_wgpu_draw_list(&new_stencils);
                            self.content_length = draw_full.content_length();
                            vis = viewport::inject_scrollbar(
                                vis,
                                self.content_length,
                                viewport_h,
                                viewport_w,
                                self.scroll_offset[1],
                            );
                            let draw_list = stencil_to_wgpu_draw_list(&vis);
                            renderer.render(&draw_list, self.scroll_offset, scale_factor);

                            self.prev_mouse_down = self.mouse_down;
                            return;
                        }
                    }

                    // ボタンハンドラ同期
                    if let Some(tl) = self.state.current_timeline(&self.app) {
                        Engine::sync_button_handlers(
                            &tl.body,
                            &self.app.components,
                            &mut self.button_handlers,
                            |id| {
                                let id = id.to_owned();
                                Box::new(move |_st| {
                                    debug!("Button '{}' pressed (default handler)", id)
                                }) // println!をdebug!に変更
                            },
                        );
                    }

                    // ハンドラディスパッチ
                    for ev in self.event_queue.drain() {
                        if let UIEvent::ButtonPressed { id } = ev {
                            if let Some(h) = self.button_handlers.get_mut(&id) {
                                h(&mut self.state);
                            }
                        }
                    }

                    // ★ イベント処理後、needs_redrawフラグをチェック
                    // 状態変更があった場合は再描画を要求
                    if self.state.needs_redraw {
                        window.request_redraw();
                    }

                    // 描画
                    let size = renderer.size();
                    let viewport_h = size.height as f32 / scale_factor;
                    let viewport_w = size.width as f32 / scale_factor;
                    let mut vis = viewport::filter_visible_stencils(
                        &stencils,
                        self.scroll_offset,
                        viewport_h,
                    );
                    let draw_full = stencil_to_wgpu_draw_list(&stencils);
                    self.content_length = draw_full.content_length();
                    vis = viewport::inject_scrollbar(
                        vis,
                        self.content_length,
                        viewport_h,
                        viewport_w,
                        self.scroll_offset[1],
                    );
                    let draw_list = stencil_to_wgpu_draw_list(&vis);
                    renderer.render(&draw_list, self.scroll_offset, scale_factor);

                    self.prev_mouse_down = self.mouse_down;
                }
                _ => {}
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn run<S: StateAccess + 'static + Clone + std::fmt::Debug>(app: App, custom_state: S) {
        let start = app.flow.start.clone();
        let mut state = AppState::new(custom_state, start);
        state.initialize_router(&app.flow);
        let app = Arc::new(app);
        run_internal(Arc::clone(&app), state);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn run_internal<S>(app: Arc<App>, state: AppState<S>)
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        let event_loop = EventLoop::new().unwrap();
        let mut app_handler = AppHandler::new(app, state, "My Application".to_string());
        event_loop.run_app(&mut app_handler).unwrap();
    }

    /// ホットリロード用の再起動フラグ付きrun関数
    #[cfg(not(target_arch = "wasm32"))]
    pub fn run_with_restart_flag<S: StateAccess + 'static + Clone + std::fmt::Debug>(
        app: App,
        custom_state: S,
        restart_flag: Arc<Mutex<bool>>,
    ) {
        let start = app.flow.start.clone();
        let mut state = AppState::new(custom_state, start);
        state.initialize_router(&app.flow);
        let app = Arc::new(app);
        run_internal_with_restart_flag(Arc::clone(&app), state, restart_flag);
    }

    /// 再起動フラグを監視しながらアプリケーションを実行する内部関数
    #[cfg(not(target_arch = "wasm32"))]
    pub fn run_internal_with_restart_flag<S>(
        app: Arc<App>,
        state: AppState<S>,
        restart_flag: Arc<Mutex<bool>>,
    ) where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        let event_loop = EventLoop::new().unwrap();

        // 再起動フラグ付きのAppHandlerを作成
        let mut app_handler = AppHandlerWithRestart::new(app, state, restart_flag);
        event_loop.run_app(&mut app_handler).unwrap();
    }

    /// 再起動フラグ付きのAppHandler
    struct AppHandlerWithRestart<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        inner: AppHandler<S>,
        restart_flag: Arc<Mutex<bool>>,
    }

    impl<S> AppHandlerWithRestart<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        fn new(app: Arc<App>, state: AppState<S>, restart_flag: Arc<Mutex<bool>>) -> Self {
            Self {
                inner: AppHandler::new(app, state, "My Application".to_string()),
                restart_flag,
            }
        }
    }

    impl<S> ApplicationHandler for AppHandlerWithRestart<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.inner.resumed(event_loop);
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: WindowId,
            event: WindowEvent,
        ) {
            // 再起動フラグをチェック
            if let Ok(flag) = self.restart_flag.try_lock() {
                if *flag {
                    // 再起動が要求されている場合はイベントループを終了
                    event_loop.exit();
                    return;
                }
            }

            // 通常のイベント処理を委譲
            self.inner.window_event(event_loop, window_id, event);
        }
    }

    /// ホットリロード機能付きのAppHandler
    #[allow(dead_code)]
    struct AppHandlerWithHotReload<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        inner: AppHandler<S>,
        restart_flag: Arc<Mutex<bool>>,
    }

    impl<S> AppHandlerWithHotReload<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        fn new(app: Arc<App>, state: AppState<S>, restart_flag: Arc<Mutex<bool>>) -> Self {
            Self {
                inner: AppHandler::new(app, state, "My Application".to_string()),
                restart_flag,
            }
        }
    }

    impl<S> ApplicationHandler for AppHandlerWithHotReload<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.inner.resumed(event_loop);
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: WindowId,
            event: WindowEvent,
        ) {
            // 再起動フラグをチェック（ノンブロッキング）
            if let Ok(flag) = self.restart_flag.try_lock() {
                if *flag {
                    event_loop.exit();
                    return;
                }
            }

            // 通常のイベント処理を委譲
            self.inner.window_event(event_loop, window_id, event);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn run_with_hotreload_support<S: StateAccess + 'static + Clone + std::fmt::Debug>(
        initial_app: Arc<App>,
        initial_state: AppState<S>,
        restart_flag: Arc<Mutex<bool>>,
        updated_app: Arc<Mutex<Option<App>>>,
    ) {
        // env_logger::init(); // 削除: lib.rsで既に初期化されている

        // 単一のイベントループを作成（一度だけ）
        let event_loop = EventLoop::new().unwrap();

        // ホットリロード対応のAppHandlerを作成
        let mut app_handler =
            AppHandlerWithDynamicReload::new(initial_app, initial_state, restart_flag, updated_app);

        let _ = event_loop.run_app(&mut app_handler);
    }

    /// 動的リロード機能付きのAppHandler
    struct AppHandlerWithDynamicReload<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        current_app: Arc<App>,
        state: AppState<S>,
        window: Option<Arc<Window>>,
        renderer: Option<WgpuRenderer>,
        event_queue: EventQueue,
        button_handlers: HashMap<String, Box<dyn FnMut(&mut AppState<S>)>>,
        scroll_offset: [f32; 2],
        content_length: f32,
        target_scroll_offset: [f32; 2],
        smoothing: f32,
        mouse_pos_raw: [f32; 2],
        mouse_pos: [f32; 2],
        mouse_down: bool,
        prev_mouse_down: bool,
        last_hovered_button: Option<String>,

        // ホットリロード用
        restart_flag: Arc<Mutex<bool>>,
        updated_app: Arc<Mutex<Option<App>>>,
        // リサイズのデバウンス用
        pending_resize: Option<PhysicalSize<u32>>,
    }

    impl<S> AppHandlerWithDynamicReload<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        fn new(
            app: Arc<App>,
            state: AppState<S>,
            restart_flag: Arc<Mutex<bool>>,
            updated_app: Arc<Mutex<Option<App>>>,
        ) -> Self {
            Self {
                current_app: app,
                state,
                window: None,
                renderer: None,
                event_queue: EventQueue::new(),
                button_handlers: HashMap::new(),
                scroll_offset: [0.0, 0.0],
                content_length: 0.0,
                target_scroll_offset: [0.0, 0.0],
                smoothing: 0.5,
                mouse_pos_raw: [0.0, 0.0],
                mouse_pos: [0.0, 0.0],
                mouse_down: false,
                prev_mouse_down: false,
                last_hovered_button: None,
                restart_flag,
                updated_app,
                pending_resize: None,
            }
        }

        /// ホットリロードされたアプリケーションをチェックして更新
        fn check_and_update_app(&mut self) {
            if let Ok(flag) = self.restart_flag.try_lock() {
                if *flag {
                    // 新しいアプリケーションがあるかチェック
                    if let Ok(mut app_guard) = self.updated_app.try_lock() {
                        if let Some(new_app) = app_guard.take() {
                            info!("🔄 Applying hot reload update..."); // println!をinfo!に変更、coloredの使用を削除
                            self.current_app = Arc::new(new_app);

                            // 状態をリセット
                            self.state.static_stencils = None;
                            self.state.static_buttons.clear();
                            self.state.static_text_inputs.clear();
                            self.state.static_text_inputs.clear();
                            self.state.expanded_body = None;
                            self.state.cached_window_size = None;
                            self.button_handlers.clear();

                            // ウィンドウの再描画を要求
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }

                            info!("✅ Hot reload update applied successfully!"); // println!をinfo!に変更、coloredの使用を削除
                        }
                    }

                    // フラグをリセット
                    drop(flag);
                    *self.restart_flag.lock().unwrap() = false;
                }
            }
        }
    }

    impl<S> ApplicationHandler for AppHandlerWithDynamicReload<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_none() {
                let window_attributes =
                    WindowAttributes::default().with_title("Nilo Application - Hot Reload Enabled");
                let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

                // ★ IME対応: ウィンドウでIMEを有効化
                window.set_ime_allowed(true);

                self.renderer = Some(pollster::block_on(WgpuRenderer::new(window.clone())));
                self.window = Some(window);
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            // ホットリロードのチェック（各イベント処理前に実行）
            self.check_and_update_app();

            let window = match &self.window {
                Some(window) => window,
                None => return,
            };
            let renderer = match &mut self.renderer {
                Some(renderer) => renderer,
                None => return,
            };

            let scale_factor = window.scale_factor() as f32;

            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(size) => {
                    // ウィンドウサイズが0の場合は何もしない（最小化時など）
                    if size.width == 0 || size.height == 0 {
                        return;
                    }
                    // 重い処理はRedrawRequestedで一度だけ行う
                    self.pending_resize = Some(size);
                    window.request_redraw();
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let viewport_height = renderer.size().height as f32 / scale_factor;
                    let max_scroll = (self.content_length - viewport_height).max(0.0);
                    let y = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y * 15.0,
                        MouseScrollDelta::PixelDelta(pos) => -pos.y as f32 / scale_factor,
                    };
                    self.target_scroll_offset[1] =
                        (self.target_scroll_offset[1] + y).clamp(-max_scroll, 0.0);
                    window.request_redraw();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let old_mouse_pos = self.mouse_pos;
                    self.mouse_pos_raw = [position.x as f32, position.y as f32];
                    self.mouse_pos = [
                        self.mouse_pos_raw[0] / scale_factor,
                        (self.mouse_pos_raw[1] / scale_factor) - self.scroll_offset[1],
                    ];

                    if (old_mouse_pos[0] - self.mouse_pos[0]).abs() > 0.5
                        || (old_mouse_pos[1] - self.mouse_pos[1]).abs() > 0.5
                    {
                        self.state.static_stencils = None;
                        self.state.static_text_inputs.clear();
                        window.request_redraw();
                    }
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => {
                    self.mouse_down = true;
                    window.request_redraw();
                }
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Left,
                    ..
                } => {
                    self.mouse_down = false;

                    // テキスト入力フィールドのクリック処理（直近レイアウトのall_text_inputsを使用）
                    let mut text_input_clicked = None;
                    for (id, pos, size) in &self.state.all_text_inputs {
                        let hover = {
                            let x = self.mouse_pos[0];
                            let y = self.mouse_pos[1];
                            x >= pos[0]
                                && x <= pos[0] + size[0]
                                && y >= pos[1]
                                && y <= pos[1] + size[1]
                        };

                        if hover {
                            text_input_clicked = Some(id.clone());
                            break;
                        }
                    }

                    // テキスト入力フィールドがクリックされた場合
                    if let Some(field_id) = text_input_clicked {
                        self.state.focus_text_input(field_id.clone());
                        self.event_queue.push(UIEvent::TextFocused { field_id });
                    } else {
                        // 他の場所がクリックされた場合はフォーカスを解除
                        if self.state.get_focused_text_input().is_some() {
                            if let Some(prev_focused) = self.state.get_focused_text_input().cloned()
                            {
                                self.state.blur_text_input();
                                self.event_queue.push(UIEvent::TextBlurred {
                                    field_id: prev_focused,
                                });
                            }
                        }
                    }

                    window.request_redraw(); // マウス離し時も再描画
                }
                // ★ IME対応: キーボード入力とIME関連のイベント処理を追加
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key,
                            state: ElementState::Pressed,
                            text,
                            ..
                        },
                    ..
                } => {
                    if let Some(focused_field) = self.state.get_focused_text_input().cloned() {
                        match physical_key {
                            PhysicalKey::Code(KeyCode::Backspace) => {
                                if self
                                    .state
                                    .get_ime_composition_text(&focused_field)
                                    .is_none()
                                {
                                    let current_value = self.state.get_text_input_value(&focused_field);
                                    if !current_value.is_empty() {
                                        let cursor_pos =
                                            self.state.get_text_cursor_position(&focused_field);
                                        if cursor_pos > 0 {
                                            let mut chars: Vec<char> = current_value.chars().collect();
                                            chars.remove(cursor_pos - 1);
                                            let new_value: String = chars.into_iter().collect();
                                            self.state.set_text_input_value(
                                                focused_field.clone(),
                                                new_value.clone(),
                                            );
                                            self.state.set_text_cursor_position(
                                                &focused_field,
                                                cursor_pos - 1,
                                            );
                                            self.event_queue.push(UIEvent::TextChanged {
                                                field_id: focused_field,
                                                new_value,
                                            });
                                        }
                                    }
                                }
                            }
                            PhysicalKey::Code(KeyCode::Delete) => {
                                if self
                                    .state
                                    .get_ime_composition_text(&focused_field)
                                    .is_none()
                                {
                                    let current_value = self.state.get_text_input_value(&focused_field);
                                    let cursor_pos =
                                        self.state.get_text_cursor_position(&focused_field);
                                    let chars: Vec<char> = current_value.chars().collect();
                                    if cursor_pos < chars.len() {
                                        let mut new_chars = chars;
                                        new_chars.remove(cursor_pos);
                                        let new_value: String = new_chars.into_iter().collect();
                                        self.state.set_text_input_value(
                                            focused_field.clone(),
                                            new_value.clone(),
                                        );
                                        self.event_queue.push(UIEvent::TextChanged {
                                            field_id: focused_field,
                                            new_value,
                                        });
                                    }
                                }
                            }
                            PhysicalKey::Code(KeyCode::Enter) => {
                                // エンター押下時の処理
                                self.event_queue.push(UIEvent::TextSubmitted {
                                    field_id: focused_field,
                                });
                            }
                            PhysicalKey::Code(KeyCode::Escape) => {
                                // エスケープでフォーカス解除
                                self.state.blur_text_input();
                                self.event_queue.push(UIEvent::TextBlurred {
                                    field_id: focused_field,
                                });
                            }
                            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                                // カーソル移動（左）
                                let current_pos =
                                    self.state.get_text_cursor_position(&focused_field);
                                if current_pos > 0 {
                                    self.state
                                        .set_text_cursor_position(&focused_field, current_pos - 1);
                                }
                            }
                            PhysicalKey::Code(KeyCode::ArrowRight) => {
                                // カーソル移動（右）
                                let current_value = self.state.get_text_input_value(&focused_field);
                                let current_pos =
                                    self.state.get_text_cursor_position(&focused_field);
                                let max_pos = current_value.chars().count();
                                if current_pos < max_pos {
                                    self.state
                                        .set_text_cursor_position(&focused_field, current_pos + 1);
                                }
                            }
                            PhysicalKey::Code(KeyCode::Home) => {
                                // 行の先頭に移動
                                self.state.set_text_cursor_position(&focused_field, 0);
                            }
                            PhysicalKey::Code(KeyCode::End) => {
                                // 行の末尾に移動
                                let current_value = self.state.get_text_input_value(&focused_field);
                                let max_pos = current_value.chars().count();
                                self.state.set_text_cursor_position(&focused_field, max_pos);
                            }
                            _ => {
                                // 通常の文字入力（textがある場合）
                                if let Some(text) = text {
                                    if !text.is_empty() && text.chars().all(|c| !c.is_control()) {
                                        let current_value =
                                            self.state.get_text_input_value(&focused_field);
                                        let cursor_pos =
                                            self.state.get_text_cursor_position(&focused_field);

                                        // カーソル位置に文字を挿入
                                        let mut chars: Vec<char> = current_value.chars().collect();
                                        for (i, c) in text.chars().enumerate() {
                                            chars.insert(cursor_pos + i, c);
                                        }

                                        let new_value: String = chars.into_iter().collect();
                                        let new_cursor_pos = cursor_pos + text.chars().count();

                                        self.state.set_text_input_value(
                                            focused_field.clone(),
                                            new_value.clone(),
                                        );
                                        self.state.set_text_cursor_position(
                                            &focused_field,
                                            new_cursor_pos,
                                        );
                                        self.event_queue.push(UIEvent::TextChanged {
                                            field_id: focused_field,
                                            new_value,
                                        });
                                    }
                                }
                            }
                        }
                        window.request_redraw(); // テキスト入力時は再描画
                    }
                }
                // ★ IME対応: IME関連のイベント処理
                WindowEvent::Ime(ime_event) => {
                    if let Some(focused_field) = self.state.get_focused_text_input().cloned() {
                        match ime_event {
                            Ime::Preedit(preedit_text, cursor_range) => {
                                // IME変換中のテキスト（下線付きテキスト）
                                self.state
                                    .set_ime_composition_text(&focused_field, preedit_text.clone());
                                self.event_queue.push(UIEvent::ImeComposition {
                                    field_id: focused_field,
                                    composition_text: preedit_text,
                                    cursor_range: cursor_range.map(|(start, end)| (start, end)),
                                });
                            }
                            Ime::Commit(committed_text) => {
                                // IME確定テキスト - カーソル位置に挿入
                                self.state.clear_ime_composition_text(&focused_field);
                                let current_value = self.state.get_text_input_value(&focused_field);
                                let cursor_pos =
                                    self.state.get_text_cursor_position(&focused_field);

                                // カーソル位置に確定テキストを挿入
                                let mut chars: Vec<char> = current_value.chars().collect();
                                for (i, c) in committed_text.chars().enumerate() {
                                    chars.insert(cursor_pos + i, c);
                                }

                                let new_value: String = chars.into_iter().collect();
                                let new_cursor_pos = cursor_pos + committed_text.chars().count();

                                self.state
                                    .set_text_input_value(focused_field.clone(), new_value.clone());
                                self.state
                                    .set_text_cursor_position(&focused_field, new_cursor_pos);

                                self.event_queue.push(UIEvent::ImeCommit {
                                    field_id: focused_field.clone(),
                                    committed_text: committed_text.clone(),
                                });
                                self.event_queue.push(UIEvent::TextChanged {
                                    field_id: focused_field,
                                    new_value,
                                });
                            }
                            Ime::Enabled => {
                                // IME有効化
                                self.event_queue.push(UIEvent::ImeEnabled {
                                    field_id: focused_field,
                                });
                            }
                            Ime::Disabled => {
                                // IME無効化
                                self.state.clear_ime_composition_text(&focused_field);
                                self.event_queue.push(UIEvent::ImeDisabled {
                                    field_id: focused_field,
                                });
                            }
                        }
                        window.request_redraw(); // IME状態変化時は再描画
                    }
                }
                WindowEvent::RedrawRequested => {
                    // リサイズをここで一度だけ適用
                    if let Some(size) = self.pending_resize.take() {
                        let viewport_height = size.height as f32 / scale_factor;
                        let max_scroll = (self.content_length - viewport_height).max(0.0);
                        renderer.resize(size);
                        if max_scroll <= 0.0 {
                            self.scroll_offset[1] = 0.0;
                            self.target_scroll_offset[1] = 0.0;
                        } else if self.scroll_offset[1] < -max_scroll {
                            self.scroll_offset[1] = -max_scroll;
                            self.target_scroll_offset[1] = -max_scroll;
                        }
                        self.state.static_stencils = None;
                        self.state.static_buttons.clear();
                        self.state.static_text_inputs.clear();
                    }
                    // ウィンドウサイズを正しく取得
                    let size = renderer.size();
                    let window_size = [
                        size.width as f32 / scale_factor,
                        size.height as f32 / scale_factor,
                    ];

                    // スクロール補正
                    self.scroll_offset[1] +=
                        (self.target_scroll_offset[1] - self.scroll_offset[1]) * self.smoothing;

                    // マウス座標の正確な計算（スクロールオフセット考慮）
                    let adjusted_mouse_pos = [
                        self.mouse_pos_raw[0] / scale_factor,
                        (self.mouse_pos_raw[1] / scale_factor) - self.scroll_offset[1],
                    ];
                    self.mouse_pos = adjusted_mouse_pos;

                    // ホバー状態を確実に反映するため、毎フレーム新しくレイアウト
                    let (stencils, buttons, text_inputs) = Engine::layout_and_stencil(
                        &self.current_app,
                        &mut self.state,
                        self.mouse_pos,
                        self.mouse_down,
                        self.prev_mouse_down,
                        window_size,
                    );

                    self.state.all_buttons = buttons.clone();
                    self.state.all_text_inputs = text_inputs.clone();

                    // マウスイベント処理
                    let mut current_hovered = None;
                    for (id, pos, size) in &buttons {
                        let hover = {
                            let x = self.mouse_pos[0];
                            let y = self.mouse_pos[1];
                            let in_bounds = x >= pos[0]
                                && x <= pos[0] + size[0]
                                && y >= pos[1]
                                && y <= pos[1] + size[1];
                            in_bounds
                        };

                        if hover {
                            current_hovered = Some(id.clone());
                        }

                        if hover && self.mouse_down && !self.prev_mouse_down {
                            self.event_queue
                                .push(UIEvent::ButtonPressed { id: id.clone() });
                        }
                        if hover && !self.mouse_down && self.prev_mouse_down {
                            self.event_queue
                                .push(UIEvent::ButtonReleased { id: id.clone() });
                        }
                    }

                    if self.last_hovered_button != current_hovered {
                        self.last_hovered_button = current_hovered;
                        self.state.static_stencils = None;
                        self.state.static_text_inputs.clear();
                    }

                    // イベント処理
                    let events_snapshot: Vec<UIEvent> =
                        self.event_queue.queue.iter().cloned().collect();
                    if !events_snapshot.is_empty() {
                        if let Some(new_tl) =
                            Engine::step_whens(&self.current_app, &mut self.state, &events_snapshot)
                        {
                            info!("[INFO] Timeline changed to {}", new_tl);

                            if let Some(tl) = self.state.current_timeline(&self.current_app) {
                                Engine::sync_button_handlers(
                                    &tl.body,
                                    &self.current_app.components,
                                    &mut self.button_handlers,
                                    |id| {
                                        let id = id.to_owned();
                                        Box::new(move |_st| {
                                            debug!("Button '{}' pressed (default handler)", id)
                                        }) // println!をdebug!に変更
                                    },
                                );
                            }

                            let (new_stencils, new_buttons, _new_text_inputs) =
                                Engine::layout_and_stencil(
                                    &self.current_app,
                                    &mut self.state,
                                    self.mouse_pos,
                                    self.mouse_down,
                                    self.prev_mouse_down,
                                    window_size,
                                );

                            self.state.all_buttons = new_buttons;

                            let size = renderer.size();
                            let viewport_h = size.height as f32 / scale_factor;
                            let viewport_w = size.width as f32 / scale_factor;
                            let mut vis = viewport::filter_visible_stencils(
                                &new_stencils,
                                self.scroll_offset,
                                viewport_h,
                            );
                            let draw_full = stencil_to_wgpu_draw_list(&new_stencils);
                            self.content_length = draw_full.content_length();
                            vis = viewport::inject_scrollbar(
                                vis,
                                self.content_length,
                                viewport_h,
                                viewport_w,
                                self.scroll_offset[1],
                            );
                            let draw_list = stencil_to_wgpu_draw_list(&vis);
                            renderer.render(&draw_list, self.scroll_offset, scale_factor);

                            self.prev_mouse_down = self.mouse_down;
                            return;
                        }
                    }

                    // ボタンハンドラ同期
                    if let Some(tl) = self.state.current_timeline(&self.current_app) {
                        Engine::sync_button_handlers(
                            &tl.body,
                            &self.current_app.components,
                            &mut self.button_handlers,
                            |id| {
                                let id = id.to_owned();
                                Box::new(move |_st| {
                                    debug!("Button '{}' pressed (default handler)", id)
                                }) // println!をdebug!に変更
                            },
                        );
                    }

                    // ハンドラディスパッチ
                    for ev in self.event_queue.drain() {
                        if let UIEvent::ButtonPressed { id } = ev {
                            if let Some(h) = self.button_handlers.get_mut(&id) {
                                h(&mut self.state);
                            }
                        }
                    }

                    // 描画
                    let size = renderer.size();
                    let viewport_h = size.height as f32 / scale_factor;
                    let viewport_w = size.width as f32 / scale_factor;
                    let mut vis = viewport::filter_visible_stencils(
                        &stencils,
                        self.scroll_offset,
                        viewport_h,
                    );
                    let draw_full = stencil_to_wgpu_draw_list(&stencils);
                    self.content_length = draw_full.content_length();
                    vis = viewport::inject_scrollbar(
                        vis,
                        self.content_length,
                        viewport_h,
                        viewport_w,
                        self.scroll_offset[1],
                    );
                    let draw_list = stencil_to_wgpu_draw_list(&vis);
                    renderer.render(&draw_list, self.scroll_offset, scale_factor);

                    self.prev_mouse_down = self.mouse_down;
                }
                _ => {}
            }
        }
    }

    pub fn run_with_window_title<S: StateAccess + 'static + Clone + std::fmt::Debug>(
        app: App,
        custom_state: S,
        window_title: Option<&str>,
    ) {
        let start = app.flow.start.clone();
        let mut state = AppState::new(custom_state, start);
        state.initialize_router(&app.flow);
        let app = Arc::new(app);

        // env_logger::init(); // 削除: lib.rsで既に初期化されている
        let event_loop = EventLoop::new().unwrap();
        let title = window_title.unwrap_or("My Application").to_string();
        let mut app_handler = AppHandler::new(app, state, title);
        event_loop.run_app(&mut app_handler).unwrap();
    }

    /// ホットリロード機能付きでウィンドウタイトルを指定してアプリケーションを実行
    pub fn run_with_hotreload_support_and_title<
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    >(
        initial_app: Arc<App>,
        initial_state: AppState<S>,
        restart_flag: Arc<Mutex<bool>>,
        updated_app: Arc<Mutex<Option<App>>>,
        window_title: Option<&str>,
    ) {
        // env_logger::init(); // 削除: lib.rsで既に初期化されている

        // 単一のイベントループを作成（一度だけ）
        let event_loop = EventLoop::new().unwrap();

        let mut app_handler = AppHandlerWithDynamicReloadAndTitle::new(
            initial_app,
            initial_state,
            restart_flag,
            updated_app,
            window_title,
        );

        // イベントループを実行（アプリケーション終了まで継続）
        let _ = event_loop.run_app(&mut app_handler);
    }

    /// ウィンドウタイトル対応の動的リロード機能付きのAppHandler
    struct AppHandlerWithDynamicReloadAndTitle<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        current_app: Arc<App>,
        state: AppState<S>,
        window: Option<Arc<Window>>,
        renderer: Option<WgpuRenderer>,
        event_queue: EventQueue,
        button_handlers: HashMap<String, Box<dyn FnMut(&mut AppState<S>)>>,
        scroll_offset: [f32; 2],
        content_length: f32,
        target_scroll_offset: [f32; 2],
        smoothing: f32,
        mouse_pos_raw: [f32; 2],
        mouse_pos: [f32; 2],
        mouse_down: bool,
        prev_mouse_down: bool,
        last_hovered_button: Option<String>,
        #[allow(dead_code)]
        window_title: String,

        restart_flag: Arc<Mutex<bool>>,
        updated_app: Arc<Mutex<Option<App>>>,
        // リサイズのデバウンス用
        pending_resize: Option<PhysicalSize<u32>>,
    }

    impl<S> AppHandlerWithDynamicReloadAndTitle<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        fn new(
            app: Arc<App>,
            state: AppState<S>,
            restart_flag: Arc<Mutex<bool>>,
            updated_app: Arc<Mutex<Option<App>>>,
            window_title: Option<&str>,
        ) -> Self {
            let title = if let Some(title) = window_title {
                format!("{} - Hot Reload Enabled", title)
            } else {
                "Nilo Application - Hot Reload Enabled".to_string()
            };

            Self {
                current_app: app,
                state,
                window: None,
                renderer: None,
                event_queue: EventQueue::new(),
                button_handlers: HashMap::new(),
                scroll_offset: [0.0, 0.0],
                content_length: 0.0,
                target_scroll_offset: [0.0, 0.0],
                smoothing: 0.5,
                mouse_pos_raw: [0.0, 0.0],
                mouse_pos: [0.0, 0.0],
                mouse_down: false,
                prev_mouse_down: false,
                last_hovered_button: None,
                window_title: title,
                restart_flag,
                updated_app,
                pending_resize: None,
            }
        }

        /// ホットリロードされたアプリケーションをチェックして更新
        fn check_and_update_app(&mut self) {
            if let Ok(flag) = self.restart_flag.try_lock() {
                if *flag {
                    // 新しいアプリケーションがあるかチェック
                    if let Ok(mut app_guard) = self.updated_app.try_lock() {
                        if let Some(new_app) = app_guard.take() {
                            info!("🔄 Applying hot reload update..."); // println!をinfo!に変更、coloredの使用を削除
                            self.current_app = Arc::new(new_app);

                            // 状態をリセット
                            self.state.static_stencils = None;
                            self.state.static_buttons.clear();
                            self.state.static_text_inputs.clear();
                            self.state.expanded_body = None;
                            self.state.cached_window_size = None;
                            self.button_handlers.clear();

                            // ウィンドウの再描画を要求
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }

                            info!("✅ Hot reload update applied successfully!"); // println!をinfo!に変更、coloredの使用を削除
                        }
                    }

                    // フラグをリセット
                    drop(flag);
                    *self.restart_flag.lock().unwrap() = false;
                }
            }
        }
    }

    impl<S> ApplicationHandler for AppHandlerWithDynamicReloadAndTitle<S>
    where
        S: StateAccess + 'static + Clone + std::fmt::Debug,
    {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_none() {
                let window_attributes =
                    WindowAttributes::default().with_title("Nilo Application - Hot Reload Enabled");
                let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

                // ★ IME対応: ウィンドウでIMEを有効化
                window.set_ime_allowed(true);

                self.renderer = Some(pollster::block_on(WgpuRenderer::new(window.clone())));
                self.window = Some(window);
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            // ホットリロードのチェック（各イベント処理前に実行）
            self.check_and_update_app();

            let window = match &self.window {
                Some(window) => window,
                None => return,
            };
            let renderer = match &mut self.renderer {
                Some(renderer) => renderer,
                None => return,
            };

            let scale_factor = window.scale_factor() as f32;

            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(size) => {
                    // ウィンドウサイズが0の場合は何もしない（最小化時など）
                    if size.width == 0 || size.height == 0 {
                        return;
                    }
                    self.pending_resize = Some(size);
                    window.request_redraw();
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let viewport_height = renderer.size().height as f32 / scale_factor;
                    let max_scroll = (self.content_length - viewport_height).max(0.0);
                    let y = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y * 15.0,
                        MouseScrollDelta::PixelDelta(pos) => -pos.y as f32 / scale_factor,
                    };
                    self.target_scroll_offset[1] =
                        (self.target_scroll_offset[1] + y).clamp(-max_scroll, 0.0);
                    window.request_redraw();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let old_mouse_pos = self.mouse_pos;
                    self.mouse_pos_raw = [position.x as f32, position.y as f32];
                    self.mouse_pos = [
                        self.mouse_pos_raw[0] / scale_factor,
                        (self.mouse_pos_raw[1] / scale_factor) - self.scroll_offset[1],
                    ];

                    if (old_mouse_pos[0] - self.mouse_pos[0]).abs() > 0.5
                        || (old_mouse_pos[1] - self.mouse_pos[1]).abs() > 0.5
                    {
                        self.state.static_stencils = None;
                        window.request_redraw();
                    }
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => {
                    self.mouse_down = true;
                    window.request_redraw();
                }
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Left,
                    ..
                } => {
                    self.mouse_down = false;

                    // テキスト入力フィールドのクリック処理（直近レイアウトのall_text_inputsを使用）
                    let mut text_input_clicked = None;
                    for (id, pos, size) in &self.state.all_text_inputs {
                        let hover = {
                            let x = self.mouse_pos[0];
                            let y = self.mouse_pos[1];
                            x >= pos[0]
                                && x <= pos[0] + size[0]
                                && y >= pos[1]
                                && y <= pos[1] + size[1]
                        };

                        if hover {
                            text_input_clicked = Some(id.clone());
                            break;
                        }
                    }

                    // テキスト入力フィールドがクリックされた場合
                    if let Some(field_id) = text_input_clicked {
                        self.state.focus_text_input(field_id.clone());
                        self.event_queue.push(UIEvent::TextFocused { field_id });
                    } else {
                        // 他の場所がクリックされた場合はフォーカスを解除
                        if self.state.get_focused_text_input().is_some() {
                            if let Some(prev_focused) = self.state.get_focused_text_input().cloned()
                            {
                                self.state.blur_text_input();
                                self.event_queue.push(UIEvent::TextBlurred {
                                    field_id: prev_focused,
                                });
                            }
                        }
                    }

                    window.request_redraw(); // マウス離し時も再描画
                }
                // ★ IME対応: キーボード入力とIME関連のイベント処理を追加
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key,
                            state: ElementState::Pressed,
                            text,
                            ..
                        },
                    ..
                } => {
                    if let Some(focused_field) = self.state.get_focused_text_input().cloned() {
                        match physical_key {
                            PhysicalKey::Code(KeyCode::Backspace) => {
                                if self
                                    .state
                                    .get_ime_composition_text(&focused_field)
                                    .is_none()
                                {
                                    let current_value = self.state.get_text_input_value(&focused_field);
                                    if !current_value.is_empty() {
                                        let cursor_pos =
                                            self.state.get_text_cursor_position(&focused_field);
                                        if cursor_pos > 0 {
                                            let mut chars: Vec<char> = current_value.chars().collect();
                                            chars.remove(cursor_pos - 1);
                                            let new_value: String = chars.into_iter().collect();
                                            self.state.set_text_input_value(
                                                focused_field.clone(),
                                                new_value.clone(),
                                            );
                                            self.state.set_text_cursor_position(
                                                &focused_field,
                                                cursor_pos - 1,
                                            );
                                            self.event_queue.push(UIEvent::TextChanged {
                                                field_id: focused_field,
                                                new_value,
                                            });
                                        }
                                    }
                                }
                            }
                            PhysicalKey::Code(KeyCode::Delete) => {
                                if self
                                    .state
                                    .get_ime_composition_text(&focused_field)
                                    .is_none()
                                {
                                    let current_value = self.state.get_text_input_value(&focused_field);
                                    let cursor_pos =
                                        self.state.get_text_cursor_position(&focused_field);
                                    let chars: Vec<char> = current_value.chars().collect();
                                    if cursor_pos < chars.len() {
                                        let mut new_chars = chars;
                                        new_chars.remove(cursor_pos);
                                        let new_value: String = new_chars.into_iter().collect();
                                        self.state.set_text_input_value(
                                            focused_field.clone(),
                                            new_value.clone(),
                                        );
                                        self.event_queue.push(UIEvent::TextChanged {
                                            field_id: focused_field,
                                            new_value,
                                        });
                                    }
                                }
                            }
                            PhysicalKey::Code(KeyCode::Enter) => {
                                // エンター押下時の処理
                                self.event_queue.push(UIEvent::TextSubmitted {
                                    field_id: focused_field,
                                });
                            }
                            PhysicalKey::Code(KeyCode::Escape) => {
                                // エスケープでフォーカス解除
                                self.state.blur_text_input();
                                self.event_queue.push(UIEvent::TextBlurred {
                                    field_id: focused_field,
                                });
                            }
                            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                                // カーソル移動（左）
                                let current_pos =
                                    self.state.get_text_cursor_position(&focused_field);
                                if current_pos > 0 {
                                    self.state
                                        .set_text_cursor_position(&focused_field, current_pos - 1);
                                }
                            }
                            PhysicalKey::Code(KeyCode::ArrowRight) => {
                                // カーソル移動（右）
                                let current_value = self.state.get_text_input_value(&focused_field);
                                let current_pos =
                                    self.state.get_text_cursor_position(&focused_field);
                                let max_pos = current_value.chars().count();
                                if current_pos < max_pos {
                                    self.state
                                        .set_text_cursor_position(&focused_field, current_pos + 1);
                                }
                            }
                            PhysicalKey::Code(KeyCode::Home) => {
                                // 行の先頭に移動
                                self.state.set_text_cursor_position(&focused_field, 0);
                            }
                            PhysicalKey::Code(KeyCode::End) => {
                                // 行の末尾に移動
                                let current_value = self.state.get_text_input_value(&focused_field);
                                let max_pos = current_value.chars().count();
                                self.state.set_text_cursor_position(&focused_field, max_pos);
                            }
                            _ => {
                                // 通常の文字入力（textがある場合）
                                if let Some(text) = text {
                                    if !text.is_empty() && text.chars().all(|c| !c.is_control()) {
                                        let current_value =
                                            self.state.get_text_input_value(&focused_field);
                                        let cursor_pos =
                                            self.state.get_text_cursor_position(&focused_field);

                                        // カーソル位置に文字を挿入
                                        let mut chars: Vec<char> = current_value.chars().collect();
                                        for (i, c) in text.chars().enumerate() {
                                            chars.insert(cursor_pos + i, c);
                                        }

                                        let new_value: String = chars.into_iter().collect();
                                        let new_cursor_pos = cursor_pos + text.chars().count();

                                        self.state.set_text_input_value(
                                            focused_field.clone(),
                                            new_value.clone(),
                                        );
                                        self.state.set_text_cursor_position(
                                            &focused_field,
                                            new_cursor_pos,
                                        );
                                        self.event_queue.push(UIEvent::TextChanged {
                                            field_id: focused_field,
                                            new_value,
                                        });
                                    }
                                }
                            }
                        }
                        window.request_redraw(); // テキスト入力時は再描画
                    }
                }
                // ★ IME対応: IME関連のイベント処理
                WindowEvent::Ime(ime_event) => {
                    if let Some(focused_field) = self.state.get_focused_text_input().cloned() {
                        match ime_event {
                            Ime::Preedit(preedit_text, cursor_range) => {
                                // IME変換中のテキスト（下線付きテキスト）
                                self.state
                                    .set_ime_composition_text(&focused_field, preedit_text.clone());
                                self.event_queue.push(UIEvent::ImeComposition {
                                    field_id: focused_field,
                                    composition_text: preedit_text,
                                    cursor_range: cursor_range.map(|(start, end)| (start, end)),
                                });
                            }
                            Ime::Commit(committed_text) => {
                                // IME確定テキスト - カーソル位置に挿入
                                self.state.clear_ime_composition_text(&focused_field);
                                let current_value = self.state.get_text_input_value(&focused_field);
                                let cursor_pos =
                                    self.state.get_text_cursor_position(&focused_field);

                                // カーソル位置に確定テキストを挿入
                                let mut chars: Vec<char> = current_value.chars().collect();
                                for (i, c) in committed_text.chars().enumerate() {
                                    chars.insert(cursor_pos + i, c);
                                }

                                let new_value: String = chars.into_iter().collect();
                                let new_cursor_pos = cursor_pos + committed_text.chars().count();

                                self.state
                                    .set_text_input_value(focused_field.clone(), new_value.clone());
                                self.state
                                    .set_text_cursor_position(&focused_field, new_cursor_pos);

                                self.event_queue.push(UIEvent::ImeCommit {
                                    field_id: focused_field.clone(),
                                    committed_text: committed_text.clone(),
                                });
                                self.event_queue.push(UIEvent::TextChanged {
                                    field_id: focused_field,
                                    new_value,
                                });
                            }
                            Ime::Enabled => {
                                // IME有効化
                                self.event_queue.push(UIEvent::ImeEnabled {
                                    field_id: focused_field,
                                });
                            }
                            Ime::Disabled => {
                                // IME無効化
                                self.state.clear_ime_composition_text(&focused_field);
                                self.event_queue.push(UIEvent::ImeDisabled {
                                    field_id: focused_field,
                                });
                            }
                        }
                        window.request_redraw(); // IME状態変化時は再描画
                    }
                }
                WindowEvent::RedrawRequested => {
                    // リサイズが保留されていればここで適用
                    if let Some(size) = self.pending_resize.take() {
                        let viewport_height = size.height as f32 / scale_factor;
                        let max_scroll = (self.content_length - viewport_height).max(0.0);
                        renderer.resize(size);
                        if max_scroll <= 0.0 {
                            self.scroll_offset[1] = 0.0;
                            self.target_scroll_offset[1] = 0.0;
                        } else if self.scroll_offset[1] < -max_scroll {
                            self.scroll_offset[1] = -max_scroll;
                            self.target_scroll_offset[1] = -max_scroll;
                        }
                        self.state.static_stencils = None;
                        self.state.static_buttons.clear();
                        self.state.static_text_inputs.clear();
                    }
                    // ウィンドウサイズを正しく取得
                    let size = renderer.size();
                    let window_size = [
                        size.width as f32 / scale_factor,
                        size.height as f32 / scale_factor,
                    ];

                    // スクロール補正
                    self.scroll_offset[1] +=
                        (self.target_scroll_offset[1] - self.scroll_offset[1]) * self.smoothing;

                    // マウス座標の正確な計算（スクロールオフセット考慮）
                    let adjusted_mouse_pos = [
                        self.mouse_pos_raw[0] / scale_factor,
                        (self.mouse_pos_raw[1] / scale_factor) - self.scroll_offset[1],
                    ];
                    self.mouse_pos = adjusted_mouse_pos;

                    // ホバー状態を確実に反映するため、毎フレーム新しくレイアウト
                    let (stencils, buttons, text_inputs) = Engine::layout_and_stencil(
                        &self.current_app,
                        &mut self.state,
                        self.mouse_pos,
                        self.mouse_down,
                        self.prev_mouse_down,
                        window_size,
                    );

                    self.state.all_buttons = buttons.clone();
                    self.state.all_text_inputs = text_inputs.clone();

                    // マウスイベント処理
                    let mut current_hovered = None;
                    for (id, pos, size) in &buttons {
                        let hover = {
                            let x = self.mouse_pos[0];
                            let y = self.mouse_pos[1];
                            let in_bounds = x >= pos[0]
                                && x <= pos[0] + size[0]
                                && y >= pos[1]
                                && y <= pos[1] + size[1];
                            in_bounds
                        };

                        if hover {
                            current_hovered = Some(id.clone());
                        }

                        if hover && self.mouse_down && !self.prev_mouse_down {
                            self.event_queue
                                .push(UIEvent::ButtonPressed { id: id.clone() });
                        }
                        if hover && !self.mouse_down && self.prev_mouse_down {
                            self.event_queue
                                .push(UIEvent::ButtonReleased { id: id.clone() });
                        }
                    }

                    if self.last_hovered_button != current_hovered {
                        self.last_hovered_button = current_hovered;
                        self.state.static_stencils = None;
                    }

                    // イベント処理
                    let events_snapshot: Vec<UIEvent> =
                        self.event_queue.queue.iter().cloned().collect();
                    if !events_snapshot.is_empty() {
                        if let Some(new_tl) =
                            Engine::step_whens(&self.current_app, &mut self.state, &events_snapshot)
                        {
                            info!("[INFO] Timeline changed to {}", new_tl);

                            if let Some(tl) = self.state.current_timeline(&self.current_app) {
                                Engine::sync_button_handlers(
                                    &tl.body,
                                    &self.current_app.components,
                                    &mut self.button_handlers,
                                    |id| {
                                        let id = id.to_owned();
                                        Box::new(move |_st| {
                                            debug!("Button '{}' pressed (default handler)", id)
                                        }) // println!をdebug!に変更
                                    },
                                );
                            }

                            let (new_stencils, new_buttons, _new_text_inputs) =
                                Engine::layout_and_stencil(
                                    &self.current_app,
                                    &mut self.state,
                                    self.mouse_pos,
                                    self.mouse_down,
                                    self.prev_mouse_down,
                                    window_size,
                                );

                            self.state.all_buttons = new_buttons;

                            let size = renderer.size();
                            let viewport_h = size.height as f32 / scale_factor;
                            let viewport_w = size.width as f32 / scale_factor;
                            let mut vis = viewport::filter_visible_stencils(
                                &new_stencils,
                                self.scroll_offset,
                                viewport_h,
                            );
                            let draw_full = stencil_to_wgpu_draw_list(&new_stencils);
                            self.content_length = draw_full.content_length();
                            vis = viewport::inject_scrollbar(
                                vis,
                                self.content_length,
                                viewport_h,
                                viewport_w,
                                self.scroll_offset[1],
                            );
                            let draw_list = stencil_to_wgpu_draw_list(&vis);
                            renderer.render(&draw_list, self.scroll_offset, scale_factor);

                            self.prev_mouse_down = self.mouse_down;
                            return;
                        }
                    }

                    // ボタンハンドラ同期
                    if let Some(tl) = self.state.current_timeline(&self.current_app) {
                        Engine::sync_button_handlers(
                            &tl.body,
                            &self.current_app.components,
                            &mut self.button_handlers,
                            |id| {
                                let id = id.to_owned();
                                Box::new(move |_st| {
                                    debug!("Button '{}' pressed (default handler)", id)
                                }) // println!をdebug!に変更
                            },
                        );
                    }

                    // ハンドラディスパッチ
                    for ev in self.event_queue.drain() {
                        if let UIEvent::ButtonPressed { id } = ev {
                            if let Some(h) = self.button_handlers.get_mut(&id) {
                                h(&mut self.state);
                            }
                        }
                    }

                    // 描画
                    let size = renderer.size();
                    let viewport_h = size.height as f32 / scale_factor;
                    let viewport_w = size.width as f32 / scale_factor;
                    let mut vis = viewport::filter_visible_stencils(
                        &stencils,
                        self.scroll_offset,
                        viewport_h,
                    );
                    let draw_full = stencil_to_wgpu_draw_list(&stencils);
                    self.content_length = draw_full.content_length();
                    vis = viewport::inject_scrollbar(
                        vis,
                        self.content_length,
                        viewport_h,
                        viewport_w,
                        self.scroll_offset[1],
                    );
                    let draw_list = stencil_to_wgpu_draw_list(&vis);
                    renderer.render(&draw_list, self.scroll_offset, scale_factor);

                    self.prev_mouse_down = self.mouse_down;
                }
                _ => {}
            }
        }
    }
}

// Re-export all public functions for native environments
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
