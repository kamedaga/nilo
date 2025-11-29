// DOM Renderer用のランタイム（WASM環境）
#[cfg(target_arch = "wasm32")]
pub fn run_dom<S>(app: crate::parser::ast::App, state: super::state::AppState<S>)
where
    S: super::state::StateAccess + 'static + Clone + std::fmt::Debug,
{
    use super::engine::Engine;
    use crate::dom_renderer::DomRenderer;
    use crate::ui::event::{EventQueue, UIEvent};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::window;
    log::info!("Starting DOM renderer...");
    let app = Arc::new(app);
    let state = Arc::new(Mutex::new(state));
    let renderer = Arc::new(Mutex::new(DomRenderer::with_container("container")));
    let event_queue = Arc::new(Mutex::new(EventQueue::new()));
    let button_handlers: Arc<
        Mutex<HashMap<String, Box<dyn FnMut(&mut super::state::AppState<S>)>>>,
    > = Arc::new(Mutex::new(HashMap::new()));
    let mouse_pos = Arc::new(Mutex::new([0.0f32, 0.0f32]));
    let mouse_down = Arc::new(Mutex::new(false));
    let prev_mouse_down = Arc::new(Mutex::new(false));
    // ウィンドウサイズを取得する関数
    let get_window_size = || -> [f32; 2] {
        if let Some(window_obj) = window() {
            if let Some(document) = window_obj.document() {
                // #containerの実際のサイズを取得
                if let Some(container) = document.get_element_by_id("container") {
                    let width = container.client_width() as f32;
                    let height = container.client_height() as f32;
                    if width > 0.0 && height > 0.0 {
                        return [width, height];
                    }
                }
            }
            // フォールバック: windowサイズを使用
            let width = window_obj
                .inner_width()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(800.0) as f32;
            let height = window_obj
                .inner_height()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(600.0) as f32;
            [width, height]
        } else {
            [800.0, 600.0]
        }
    };
    // 初期レンダリング
    {
        let mut state_guard = state.lock().unwrap();
        let mut renderer_guard = renderer.lock().unwrap();
        // 初期ビューをレンダリング
        let window_size = get_window_size();
        let mouse_pos_val = *mouse_pos.lock().unwrap();
        let mouse_down_val = *mouse_down.lock().unwrap();
        let prev_mouse_down_val = *prev_mouse_down.lock().unwrap();
        let (stencils, buttons, text_inputs) = Engine::layout_and_stencil(
            &app,
            &mut state_guard,
            mouse_pos_val,
            mouse_down_val,
            prev_mouse_down_val,
            window_size,
        );
        state_guard.all_buttons = buttons;
        state_guard.all_text_inputs = text_inputs;
        // コンテンツの高さを計算
        let draw_list = crate::stencil::stencil::stencil_to_wgpu_draw_list(&stencils);
        let content_h = draw_list.content_length();
        // scroll_offset[1]にcontent_heightを設定（DOM版専用の仕様）
        renderer_guard.render_stencils(&stencils, [0.0, content_h], 1.0);
        #[cfg(target_arch = "wasm32")]
        {
            let container_id = renderer_guard.container_id().to_string();
            let text_inputs_snapshot = state_guard.all_text_inputs.clone();
            sync_dom_text_inputs(
                &container_id,
                &text_inputs_snapshot,
                &state_guard,
                &state,
                &event_queue,
            );
        }
        log::info!("Initial DOM render complete");
    }
    // イベントリスナーの設定（#preview-containerに登録して永続化）
    if let Some(window_obj) = window() {
        if let Some(document) = window_obj.document() {
            // Register event listeners directly on `#container` so event.clientX/Y
            // coordinates and the container's bounding rect share the same origin.
            // Registering on `#preview-container` caused offset mismatches.
            let event_target = document.get_element_by_id("container");
            if let Some(target) = event_target {
                // マウスムーブイベント
                let mouse_pos_clone = Arc::clone(&mouse_pos);
                let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
                    // #container要素内の相対座標を取得
                    if let Some(window) = window() {
                        if let Some(document) = window.document() {
                            if let Some(container) = document.get_element_by_id("container") {
                                use wasm_bindgen::JsCast;
                                if let Ok(element) = container.dyn_into::<web_sys::Element>() {
                                    let rect = element.get_bounding_client_rect();
                                    let mut pos = mouse_pos_clone.lock().unwrap();
                                    pos[0] = (event.client_x() as f64 - rect.left()) as f32;
                                    pos[1] = (event.client_y() as f64 - rect.top()) as f32;
                                }
                            }
                        }
                    }
                }) as Box<dyn FnMut(_)>);
                target
                    .add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())
                    .ok();
                closure.forget();
                // マウスダウンイベント
                let mouse_down_clone = Arc::clone(&mouse_down);
                let event_queue_clone = Arc::clone(&event_queue);
                let state_clone = Arc::clone(&state);
                let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
                    *mouse_down_clone.lock().unwrap() = true;
                    // #container要素内の相対座標を取得
                    let pos = if let Some(window) = window() {
                        if let Some(document) = window.document() {
                            if let Some(container) = document.get_element_by_id("container") {
                                let rect = container.get_bounding_client_rect();
                                [
                                    (event.client_x() as f64 - rect.left()) as f32,
                                    (event.client_y() as f64 - rect.top()) as f32,
                                ]
                            } else {
                                [0.0, 0.0]
                            }
                        } else {
                            [0.0, 0.0]
                        }
                    } else {
                        [0.0, 0.0]
                    };
                    // ボタンのヒットテストを行う
                    let state_guard = state_clone.lock().unwrap();
                    for (id, button_pos, button_size) in &state_guard.all_buttons {
                        let in_bounds = pos[0] >= button_pos[0]
                            && pos[0] <= button_pos[0] + button_size[0]
                            && pos[1] >= button_pos[1]
                            && pos[1] <= button_pos[1] + button_size[1];
                        if in_bounds {
                            log::info!(
                                "Button clicked: {} at pos={:?}, button_pos={:?}, button_size={:?}",
                                id,
                                pos,
                                button_pos,
                                button_size
                            );
                            event_queue_clone
                                .lock()
                                .unwrap()
                                .push(UIEvent::ButtonPressed { id: id.clone() });
                            break;
                        }
                    }
                }) as Box<dyn FnMut(_)>);
                target
                    .add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())
                    .ok();
                closure.forget();
                // マウスアップイベント
                let mouse_down_clone = Arc::clone(&mouse_down);
                let closure = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                    *mouse_down_clone.lock().unwrap() = false;
                }) as Box<dyn FnMut(_)>);
                target
                    .add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())
                    .ok();
                closure.forget();
                log::info!("Event listeners registered on preview-container");
            }
        }
    }
    // レンダリングループを設定
    let app_clone = Arc::clone(&app);
    let state_clone = Arc::clone(&state);
    let renderer_clone = Arc::clone(&renderer);
    let event_queue_clone = Arc::clone(&event_queue);
    let button_handlers_clone = Arc::clone(&button_handlers);
    let mouse_pos_clone = Arc::clone(&mouse_pos);
    let mouse_down_clone = Arc::clone(&mouse_down);
    let prev_mouse_down_clone = Arc::clone(&prev_mouse_down);
    let f = std::rc::Rc::new(std::cell::RefCell::new(None::<Closure<dyn FnMut()>>));
    let g = f.clone();
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let mut state_guard = state_clone.lock().unwrap();
        let mut renderer_guard = renderer_clone.lock().unwrap();
        let mut event_queue_guard = event_queue_clone.lock().unwrap();
        let mut handlers_guard = button_handlers_clone.lock().unwrap();
        // Apply any async interval/call results before handling events.
        let async_updates_applied =
            crate::engine::async_call::apply_async_results(&mut state_guard);
        if async_updates_applied {
            state_guard.static_stencils = None;
            state_guard.static_buttons.clear();
            state_guard.static_text_inputs.clear();
            state_guard.needs_redraw = true;
        }
        // 前回のマウス状態を更新
        let current_mouse_down = *mouse_down_clone.lock().unwrap();
        let prev_down = *prev_mouse_down_clone.lock().unwrap();
        // イベント処理
        let events: Vec<UIEvent> = event_queue_guard.queue.iter().cloned().collect();
        if !events.is_empty() {
            if let Some(new_tl) = Engine::step_whens(&app_clone, &mut state_guard, &events) {
                log::info!("Timeline changed to {}", new_tl);
            }
            // ボタンハンドラディスパッチ
            for ev in event_queue_guard.drain() {
                if let UIEvent::ButtonPressed { id } = ev {
                    if let Some(h) = handlers_guard.get_mut(&id) {
                        h(&mut state_guard);
                    }
                }
            }
        }
        // レンダリング - コンテナサイズを動的に取得
        let window_size = if let Some(window_obj) = window() {
            if let Some(document) = window_obj.document() {
                // #containerの実際のサイズを取得
                if let Some(container) = document.get_element_by_id("container") {
                    let width = container.client_width() as f32;
                    let height = container.client_height() as f32;
                    if width > 0.0 && height > 0.0 {
                        [width, height]
                    } else {
                        // フォールバック
                        let width = window_obj
                            .inner_width()
                            .ok()
                            .and_then(|v| v.as_f64())
                            .unwrap_or(800.0) as f32;
                        let height = window_obj
                            .inner_height()
                            .ok()
                            .and_then(|v| v.as_f64())
                            .unwrap_or(600.0) as f32;
                        [width, height]
                    }
                } else {
                    [800.0, 600.0]
                }
            } else {
                [800.0, 600.0]
            }
        } else {
            [800.0, 600.0]
        };
        let mouse_pos_val = *mouse_pos_clone.lock().unwrap();
        let (stencils, buttons, text_inputs) = Engine::layout_and_stencil(
            &app_clone,
            &mut state_guard,
            mouse_pos_val,
            current_mouse_down,
            prev_down,
            window_size,
        );
        state_guard.all_buttons = buttons;
        state_guard.all_text_inputs = text_inputs;
        // コンテンツの高さを計算
        let draw_list = crate::stencil::stencil::stencil_to_wgpu_draw_list(&stencils);
        let content_h = draw_list.content_length();
        // scroll_offset[1]にcontent_heightを設定（DOM版専用の仕様）
        renderer_guard.render_stencils(&stencils, [0.0, content_h], 1.0);
        #[cfg(target_arch = "wasm32")]
        {
            let container_id = renderer_guard.container_id().to_string();
            let text_inputs_snapshot = state_guard.all_text_inputs.clone();
            sync_dom_text_inputs(
                &container_id,
                &text_inputs_snapshot,
                &state_guard,
                &state_clone,
                &event_queue_clone,
            );
        }
        // マウス状態を更新
        *prev_mouse_down_clone.lock().unwrap() = current_mouse_down;
        // 次のフレームを要求
        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));
    fn request_animation_frame(f: &Closure<dyn FnMut()>) {
        window()
            .unwrap()
            .request_animation_frame(f.as_ref().unchecked_ref())
            .expect("should register `requestAnimationFrame` OK");
    }
    // 最初のフレームを要求
    request_animation_frame(g.borrow().as_ref().unwrap());
    log::info!("DOM render loop started");
}
// WASM����DOM�̕����Ńe�L�X�g�r���[�p�̃C���|�[�g��`������
// �L���b�V���n���h�����鎞�ɂ��̂����ݒ肷��
#[cfg(target_arch = "wasm32")]
fn sync_dom_text_inputs<S: super::state::StateAccess + 'static>(
    container_id: &str,
    text_inputs: &[(String, [f32; 2], [f32; 2])],
    state_for_value: &super::state::AppState<S>,
    state_arc: &std::sync::Arc<std::sync::Mutex<super::state::AppState<S>>>,
    event_queue_arc: &std::sync::Arc<std::sync::Mutex<crate::ui::event::EventQueue>>,
) {
    use std::collections::HashSet;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    use web_sys::{window, FocusEvent, HtmlElement, HtmlInputElement, KeyboardEvent};
    let document = if let Some(doc) = window().and_then(|w| w.document()) {
        doc
    } else {
        return;
    };
    let container = if let Some(el) = document.get_element_by_id(container_id) {
        el
    } else {
        return;
    };
    let active_ids: HashSet<String> = text_inputs.iter().map(|(id, _, _)| id.clone()).collect();
    // �����TextInput DOM�v�f��掃��
    let existing = container.get_elements_by_class_name("nilo-dom-textinput");
    let mut idx = existing.length();
    while idx > 0 {
        idx -= 1;
        if let Some(node) = existing.item(idx) {
            if let Some(el) = node.dyn_ref::<HtmlElement>() {
                let data_id = el.get_attribute("data-nilo-id").unwrap_or_default();
                if !active_ids.contains(&data_id) {
                    let _ = container.remove_child(&el);
                }
            }
        }
    }
    for (id, position, size) in text_inputs {
        // �����v�f�̒l���Q�Ƃ�
        let mut maybe_input: Option<HtmlInputElement> = None;
        let current_list = container.get_elements_by_class_name("nilo-dom-textinput");
        for i in 0..current_list.length() {
            if let Some(node) = current_list.item(i) {
                if let Ok(el) = node.dyn_into::<HtmlInputElement>() {
                    if el.get_attribute("data-nilo-id").as_deref() == Some(id.as_str()) {
                        maybe_input = Some(el);
                        break;
                    }
                }
            }
        }
        let input = if let Some(el) = maybe_input {
            el
        } else {
            let el = match document.create_element("input") {
                Ok(el) => el,
                Err(_) => continue,
            };
            let input: HtmlInputElement = match el.dyn_into() {
                Ok(v) => v,
                Err(_) => continue,
            };
            input.set_attribute("type", "text").ok();
            input.set_attribute("autocomplete", "off").ok();
            input.set_attribute("spellcheck", "false").ok();
            input.set_attribute("data-nilo-id", id).ok();
            input.set_class_name("nilo-dom-textinput");
            input.set_attribute("inputmode", "text").ok();
            input.set_attribute("aria-label", "text input").ok();
            // focus�ƃt�H�[�J�X�C�x���g
            let focus_id = id.clone();
            let focus_state = state_arc.clone();
            let focus_queue = event_queue_arc.clone();
            let focus_cb = Closure::wrap(Box::new(move |_event: FocusEvent| {
                if let Ok(mut state) = focus_state.lock() {
                    state.focus_text_input(focus_id.clone());
                }
                if let Ok(mut queue) = focus_queue.lock() {
                    queue.push(crate::ui::event::UIEvent::TextFocused {
                        field_id: focus_id.clone(),
                    });
                }
            }) as Box<dyn FnMut(_)>);
            input
                .add_event_listener_with_callback("focus", focus_cb.as_ref().unchecked_ref())
                .ok();
            focus_cb.forget();
            // blur�C�x���g
            let blur_id = id.clone();
            let blur_state = state_arc.clone();
            let blur_queue = event_queue_arc.clone();
            let blur_cb = Closure::wrap(Box::new(move |_event: FocusEvent| {
                if let Ok(mut state) = blur_state.lock() {
                    state.blur_text_input();
                }
                if let Ok(mut queue) = blur_queue.lock() {
                    queue.push(crate::ui::event::UIEvent::TextBlurred {
                        field_id: blur_id.clone(),
                    });
                }
            }) as Box<dyn FnMut(_)>);
            input
                .add_event_listener_with_callback("blur", blur_cb.as_ref().unchecked_ref())
                .ok();
            blur_cb.forget();
            // input�C�x���g
            let input_id = id.clone();
            let input_state = state_arc.clone();
            let input_queue = event_queue_arc.clone();
            let input_cb = Closure::wrap(Box::new(move |event: web_sys::Event| {
                if let Some(target) = event
                    .target()
                    .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                {
                    let value = target.value();
                    let cursor_pos = target
                        .selection_end()
                        .ok()
                        .flatten()
                        .unwrap_or(value.chars().count() as u32) as usize;
                    if let Ok(mut state) = input_state.lock() {
                        state.set_text_input_value(input_id.clone(), value.clone());
                        state.set_text_cursor_position(&input_id, cursor_pos);
                    }
                    if let Ok(mut queue) = input_queue.lock() {
                        queue.push(crate::ui::event::UIEvent::TextChanged {
                            field_id: input_id.clone(),
                            new_value: value,
                        });
                    }
                }
            }) as Box<dyn FnMut(_)>);
            input
                .add_event_listener_with_callback("input", input_cb.as_ref().unchecked_ref())
                .ok();
            input_cb.forget();
            // �J�[�\���ړ��̂炵�փL���b�V���ɐݒ�
            let cursor_id = id.clone();
            let cursor_state = state_arc.clone();
            let cursor_cb = Closure::wrap(Box::new(move |event: web_sys::Event| {
                if let Some(target) = event
                    .target()
                    .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                {
                    let cursor_pos = target
                        .selection_end()
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| target.value().chars().count() as u32)
                        as usize;
                    if let Ok(mut state) = cursor_state.lock() {
                        state.set_text_cursor_position(&cursor_id, cursor_pos);
                    }
                }
            }) as Box<dyn FnMut(_)>);
            input
                .add_event_listener_with_callback("keyup", cursor_cb.as_ref().unchecked_ref())
                .ok();
            input
                .add_event_listener_with_callback("click", cursor_cb.as_ref().unchecked_ref())
                .ok();
            cursor_cb.forget();
            // Enter�L�[�Ŗ���C�x���g
            let enter_id = id.clone();
            let enter_queue = event_queue_arc.clone();
            let enter_cb = Closure::wrap(Box::new(move |event: KeyboardEvent| {
                if event.key() == "Enter" {
                    event.prevent_default();
                    if let Ok(mut queue) = enter_queue.lock() {
                        queue.push(crate::ui::event::UIEvent::TextSubmitted {
                            field_id: enter_id.clone(),
                        });
                    }
                }
            }) as Box<dyn FnMut(_)>);
            input
                .add_event_listener_with_callback("keydown", enter_cb.as_ref().unchecked_ref())
                .ok();
            enter_cb.forget();
            if container.append_child(&input).is_err() {
                continue;
            }
            input
        };
        // ��ʒu�����ݒ�
        let style = input.style();
        let _ = style.set_property("position", "absolute");
        let _ = style.set_property("left", &format!("{}px", position[0]));
        let _ = style.set_property("top", &format!("{}px", position[1]));
        let _ = style.set_property("width", &format!("{}px", size[0]));
        let _ = style.set_property("height", &format!("{}px", size[1]));
        let _ = style.set_property("padding", "0px");
        let _ = style.set_property("margin", "0px");
        let _ = style.set_property("border", "none");
        let _ = style.set_property("outline", "none");
        let _ = style.set_property("background", "transparent");
        // 非表示でフォーカスのみ使う（描画はエンジン側で行う）
        let _ = style.set_property("color", "transparent");
        let _ = style.set_property("caret-color", "#1a1a1a");
        let _ = style.set_property("box-sizing", "border-box");
        let _ = style.set_property("z-index", "5000");
        let _ = style.set_property("opacity", "0.01");
        let _ = style.set_property("pointer-events", "auto");
        // �e�L�X�g�l���X�V���Ă��邩�`�F�b�N���ăJ�[�\���ʒu��ݒ�
        let state_value = state_for_value.get_text_input_value(id);
        if input.value() != state_value {
            input.set_value(&state_value);
        }
        let cursor_pos = state_for_value.get_text_cursor_position(id) as u32;
        let _ = input.set_selection_range(cursor_pos, cursor_pos);
    }
}
