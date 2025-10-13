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

        // コンテンツの高さを計算
        let draw_list = crate::stencil::stencil::stencil_to_wgpu_draw_list(&stencils);
        let content_h = draw_list.content_length();

        // scroll_offset[1]にcontent_heightを設定（DOM版専用の仕様）
        renderer_guard.render_stencils(&stencils, [0.0, content_h], 1.0);

        log::info!("Initial DOM render complete");
    }

    // イベントリスナーの設定
    if let Some(window_obj) = window() {
        if let Some(document) = window_obj.document() {
            if let Some(body) = document.body() {
                // マウスムーブイベント
                let mouse_pos_clone = Arc::clone(&mouse_pos);
                let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
                    let mut pos = mouse_pos_clone.lock().unwrap();
                    pos[0] = event.client_x() as f32;
                    pos[1] = event.client_y() as f32;
                }) as Box<dyn FnMut(_)>);
                body.add_event_listener_with_callback(
                    "mousemove",
                    closure.as_ref().unchecked_ref(),
                )
                .ok();
                closure.forget();

                // マウスダウンイベント
                let mouse_down_clone = Arc::clone(&mouse_down);
                let event_queue_clone = Arc::clone(&event_queue);
                let state_clone = Arc::clone(&state);
                let mouse_pos_clone = Arc::clone(&mouse_pos);
                let closure = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                    *mouse_down_clone.lock().unwrap() = true;

                    // ボタンのヒットテストを行う
                    let state_guard = state_clone.lock().unwrap();
                    let pos = *mouse_pos_clone.lock().unwrap();

                    for (id, button_pos, button_size) in &state_guard.all_buttons {
                        let in_bounds = pos[0] >= button_pos[0]
                            && pos[0] <= button_pos[0] + button_size[0]
                            && pos[1] >= button_pos[1]
                            && pos[1] <= button_pos[1] + button_size[1];

                        if in_bounds {
                            event_queue_clone
                                .lock()
                                .unwrap()
                                .push(UIEvent::ButtonPressed { id: id.clone() });
                            break;
                        }
                    }
                }) as Box<dyn FnMut(_)>);
                body.add_event_listener_with_callback(
                    "mousedown",
                    closure.as_ref().unchecked_ref(),
                )
                .ok();
                closure.forget();

                // マウスアップイベント
                let mouse_down_clone = Arc::clone(&mouse_down);
                let closure = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                    *mouse_down_clone.lock().unwrap() = false;
                }) as Box<dyn FnMut(_)>);
                body.add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())
                    .ok();
                closure.forget();

                log::info!("Event listeners registered");
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

        // レンダリング - ウィンドウサイズを動的に取得
        let window_size = if let Some(window_obj) = window() {
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

        // コンテンツの高さを計算
        let draw_list = crate::stencil::stencil::stencil_to_wgpu_draw_list(&stencils);
        let content_h = draw_list.content_length();

        // scroll_offset[1]にcontent_heightを設定（DOM版専用の仕様）
        renderer_guard.render_stencils(&stencils, [0.0, content_h], 1.0);

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
