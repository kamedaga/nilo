//! Runtime module for Nilo engine
//!
//! This module contains the runtime implementation for native (non-WASM) environments.
//! For WASM environments, see runtime_dom.rs

// Native環境専用のruntime
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::parser::ast::{App, ViewNode};
    use crate::stencil::stencil::stencil_to_wgpu_draw_list;
    use crate::ui::event::{EventQueue, UIEvent};
    use crate::ui::viewport;
    #[cfg(feature = "wgpu")]
    use crate::wgpu_renderer::wgpu::WgpuRenderer;
    use log::{debug, info};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex}; // ログマクロを追加

    use colored::Colorize;

    use winit::{
        application::ApplicationHandler,
        event::{ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
        event_loop::{ActiveEventLoop, EventLoop},
        keyboard::{KeyCode, PhysicalKey},
        window::{Window, WindowAttributes, WindowId},
    };

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
            }
        }

        /// フレームカウントと経過時間を更新（dynamic_section用）
        fn update_frame_state(&mut self) {
            // frame_countフィールドがあれば更新
            if let Some(current_frame_str) = self.state.custom_state.get_field("frame_count") {
                if let Ok(current_frame) = current_frame_str.parse::<u32>() {
                    self.state
                        .custom_state
                        .set("frame_count", (current_frame + 1).to_string());
                }
            }

            // elapsed_timeフィールドがあれば更新
            if let Some(current_time_str) = self.state.custom_state.get_field("elapsed_time") {
                if let Ok(current_time) = current_time_str.parse::<f32>() {
                    // 60FPSを想定して時間を更新（約0.0167秒/フレーム）
                    self.state
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

                    // テキスト入力フィールドのクリック処理
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
                        log::info!("TextInput focus: {}", field_id);
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
                                    }
                                } else {
                                    let current_value = self.state.get_text_input_value(&focused_field);
                                    let cursor_pos = self.state.get_text_cursor_position(&focused_field);
                                    let chars: Vec<char> = current_value.chars().collect();
                                    if cursor_pos < chars.len() {
                                        let mut new_chars = chars;
                                        new_chars.remove(cursor_pos);
                                        let new_value: String = new_chars.into_iter().collect();
                                        self.state.set_text_input_value(focused_field.clone(), new_value.clone());
                                        self.event_queue.push(UIEvent::TextChanged { field_id: focused_field, new_value });
                                    }
                                }
                            }

                            _ => {
                                if let Some(mut comp) = self.state.get_ime_composition_text(&focused_field).cloned() {
                                    if let Some(t) = text {
                                        if !t.is_empty() && t.chars().all(|c| !c.is_control()) {
                                            comp.push_str(&t);
                                            self.state.set_ime_composition_text(&focused_field, comp.clone());
                                            self.event_queue.push(UIEvent::ImeComposition {
                                                field_id: focused_field,
                                                composition_text: comp,
                                                cursor_range: None,
                                            });
                                        }
                                    }
                                } else {
                                    if let Some(text) = text {
                                        if !text.is_empty() && text.chars().all(|c| !c.is_control()) {
                                            let current_value = self.state.get_text_input_value(&focused_field);
                                            let cursor_pos = self.state.get_text_cursor_position(&focused_field);
                                            let mut chars: Vec<char> = current_value.chars().collect();
                                            for (i, c) in text.chars().enumerate() { chars.insert(cursor_pos + i, c); }
                                            let new_value: String = chars.into_iter().collect();
                                            let new_cursor_pos = cursor_pos + text.chars().count();
                                            self.state.set_text_input_value(focused_field.clone(), new_value.clone());
                                            self.state.set_text_cursor_position(&focused_field, new_cursor_pos);
                                            self.event_queue.push(UIEvent::TextChanged { field_id: focused_field, new_value });
                                        }
                                    }
                                }
                            }

