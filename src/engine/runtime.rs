use crate::parser::ast::App;
use crate::stencil::stencil::stencil_to_wgpu_draw_list;
use crate::ui::event::{UIEvent, EventQueue};
use crate::renderer::wgpu::WgpuRenderer;
use crate::ui::viewport;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use colored::Colorize;
use winit::{
    event::{WindowEvent, MouseScrollDelta, ElementState, MouseButton},
    event_loop::{EventLoop, ActiveEventLoop, ControlFlow},
    window::{Window, WindowId, WindowAttributes},
    application::ApplicationHandler,
};
use super::state::{AppState, StateAccess};
use super::engine::Engine;

struct AppHandler<S>
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
    window_title: String, // ウィンドウタイトル
}

impl<S> AppHandler<S>
where
    S: StateAccess + 'static + Clone + std::fmt::Debug,
{
    fn new(app: Arc<App>, state: AppState<S>, window_title: String) -> Self {
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
}

impl<S> ApplicationHandler for AppHandler<S>
where
    S: StateAccess + 'static + Clone + std::fmt::Debug,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = WindowAttributes::default()
                .with_title(&self.window_title);
            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            self.renderer = Some(pollster::block_on(WgpuRenderer::new(window.clone())));
            self.window = Some(window);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
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
                self.target_scroll_offset[1] = (self.target_scroll_offset[1] + y).clamp(-max_scroll, 0.0);
                window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let old_mouse_pos = self.mouse_pos;
                self.mouse_pos_raw = [position.x as f32, position.y as f32];
                self.mouse_pos = [
                    self.mouse_pos_raw[0] / scale_factor,
                    (self.mouse_pos_raw[1] / scale_factor) - self.scroll_offset[1]
                ];

                // マウス座標が変わった場合は再描画を要求（ホバー状態が変わる可能性）
                if (old_mouse_pos[0] - self.mouse_pos[0]).abs() > 0.5 ||
                   (old_mouse_pos[1] - self.mouse_pos[1]).abs() > 0.5 {
                    // ホバー状態変化の検出のためキャッシュを無効化
                    self.state.static_stencils = None;
                    window.request_redraw();
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                self.mouse_down = true;
                window.request_redraw(); // マウス押下時も再描画
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                self.mouse_down = false;
                window.request_redraw(); // マウス離し時も再描画
            }
            WindowEvent::RedrawRequested => {
                // スクロール補正
                self.scroll_offset[1] += (self.target_scroll_offset[1] - self.scroll_offset[1]) * self.smoothing;

                let window_size = [
                    window.inner_size().width as f32 / scale_factor,
                    window.inner_size().height as f32 / scale_factor
                ];

                // マウス座標の正確な計算（スクロールオフセット考慮）
                let adjusted_mouse_pos = [
                    self.mouse_pos_raw[0] / scale_factor,
                    (self.mouse_pos_raw[1] / scale_factor) - self.scroll_offset[1]
                ];
                self.mouse_pos = adjusted_mouse_pos;

                // ホバー状態を確実に反映するため、毎フレーム新しくレイアウト
                let (stencils, buttons) = Engine::layout_and_stencil(
                    &self.app, &mut self.state,
                    self.mouse_pos, self.mouse_down, self.prev_mouse_down,
                    window_size
                );

                self.state.all_buttons = buttons.clone();

                // ホバー状態とマウスイベントの処理
                let mut current_hovered = None;
                for (id, pos, size) in &buttons {
                    let hover = {
                        let x = self.mouse_pos[0];
                        let y = self.mouse_pos[1];
                        let in_bounds = x >= pos[0] && x <= pos[0] + size[0] &&
                                       y >= pos[1] && y <= pos[1] + size[1];

                        in_bounds
                    };

                    if hover {
                        current_hovered = Some(id.clone());
                    }

                    // マウスイベントの生成
                    if hover && self.mouse_down && !self.prev_mouse_down {
                        self.event_queue.push(UIEvent::ButtonPressed { id: id.clone() });
                    }
                    if hover && !self.mouse_down && self.prev_mouse_down {
                        self.event_queue.push(UIEvent::ButtonReleased { id: id.clone() });
                    }
                }

                // ホバー状態の変化を検出
                if self.last_hovered_button != current_hovered {
                    self.last_hovered_button = current_hovered;
                    // ホバ��状態変化時はキ��ッシュを無効化
                    self.state.static_stencils = None;
                }

                // イベ��ト処理
                let events_snapshot: Vec<UIEvent> = self.event_queue.queue.iter().cloned().collect();
                if !events_snapshot.is_empty() {

                    // when評価
                    if let Some(new_tl) = Engine::step_whens(&self.app, &mut self.state, &events_snapshot) {
                        println!("[INFO] Timeline changed to {}", new_tl);

                        if let Some(tl) = self.state.current_timeline(&self.app) {
                            Engine::sync_button_handlers(&tl.body, &self.app.components, &mut self.button_handlers, |id| {
                                let id = id.to_owned();
                                Box::new(move |_st| println!("Button '{}' pressed (default handler)", id))
                            });
                        }

                        // タイムライン変更後は描画を更新
                        let (new_stencils, new_buttons) = Engine::layout_and_stencil(
                            &self.app, &mut self.state,
                            self.mouse_pos, self.mouse_down, self.prev_mouse_down,
                            window_size
                        );

                        self.state.all_buttons = new_buttons;

                        // 新しいレイアウトで描画
                        let size = renderer.size();
                        let viewport_h = size.height as f32 / scale_factor;
                        let viewport_w = size.width as f32 / scale_factor;
                        let mut vis = viewport::filter_visible_stencils(&new_stencils, self.scroll_offset, viewport_h);
                        let draw_full = stencil_to_wgpu_draw_list(&new_stencils);
                        self.content_length = draw_full.content_length();
                        vis = viewport::inject_scrollbar(vis, self.content_length, viewport_h, viewport_w, self.scroll_offset[1]);
                        let draw_list = stencil_to_wgpu_draw_list(&vis);
                        renderer.render(&draw_list, self.scroll_offset, scale_factor);

                        self.prev_mouse_down = self.mouse_down;
                        return;
                    }
                }

                // ボタンハンドラ同期
                if let Some(tl) = self.state.current_timeline(&self.app) {
                    Engine::sync_button_handlers(
                        &tl.body, &self.app.components, &mut self.button_handlers,
                        |id| {
                            let id = id.to_owned();
                            Box::new(move |_st| println!("Button '{}' pressed (default handler)", id))
                        }
                    );
                }

                // ハンドラディスパッチ
                for ev in self.event_queue.drain() {
                    if let UIEvent::ButtonPressed { id } = ev {
                        if let Some(h) = self.button_handlers.get_mut(&id) {
                            h(&mut self.state);}
                    }
                }

                // 描画
                let size = renderer.size();
                let viewport_h = size.height as f32 / scale_factor;
                let viewport_w = size.width as f32 / scale_factor;
                let mut vis = viewport::filter_visible_stencils(&stencils, self.scroll_offset, viewport_h);
                let draw_full = stencil_to_wgpu_draw_list(&stencils);
                self.content_length = draw_full.content_length();
                vis = viewport::inject_scrollbar(vis, self.content_length, viewport_h, viewport_w, self.scroll_offset[1]);
                let draw_list = stencil_to_wgpu_draw_list(&vis);
                renderer.render(&draw_list, self.scroll_offset, scale_factor);

                self.prev_mouse_down = self.mouse_down;
            }
            _ => {}
        }
    }
}

pub fn run<S: StateAccess + 'static + Clone + std::fmt::Debug>(app: App, custom_state: S) {
    let start = app.flow.start.clone();
    let state = AppState::new(custom_state, start);
    let app = Arc::new(app);
    run_internal(Arc::clone(&app), state);
}

pub fn run_internal<S>(app: Arc<App>, state: AppState<S>)
where
    S: StateAccess + 'static + Clone + std::fmt::Debug,
{
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app_handler = AppHandler::new(app, state, "My Application".to_string());
    event_loop.run_app(&mut app_handler).unwrap();
}

/// ホットリロード用の再起動フラグ付きrun関数
pub fn run_with_restart_flag<S: StateAccess + 'static + Clone + std::fmt::Debug>(
    app: App,
    custom_state: S,
    restart_flag: Arc<Mutex<bool>>
) {
    let start = app.flow.start.clone();
    let state = AppState::new(custom_state, start);
    let app = Arc::new(app);
    run_internal_with_restart_flag(Arc::clone(&app), state, restart_flag);
}

/// 再起動フラグを監視しながらアプリケーション���実行する内部関数
pub fn run_internal_with_restart_flag<S>(
    app: Arc<App>,
    state: AppState<S>,
    restart_flag: Arc<Mutex<bool>>
)
where
    S: StateAccess + 'static + Clone + std::fmt::Debug,
{
    env_logger::init();
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

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        // 再起�����フラグをチェック
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

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        // 再起動フラグをチェック（ノンブロッキング）
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

/// ホットリロード機能付きでアプリケーションを実行���メインスレッド用）
pub fn run_with_hotreload_support<S: StateAccess + 'static + Clone + std::fmt::Debug>(
    initial_app: Arc<App>,
    initial_state: AppState<S>,
    restart_flag: Arc<Mutex<bool>>,
    updated_app: Arc<Mutex<Option<App>>>
) {
    env_logger::init();

    // 単一のイベントループを作成（一度だけ）
    let event_loop = EventLoop::new().unwrap();

    // ホットリロード対応のAppHandlerを作成
    let mut app_handler = AppHandlerWithDynamicReload::new(
        initial_app,
        initial_state,
        restart_flag,
        updated_app
    );

    // イベ���トループを実行（アプリケーション終了まで継続）
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
}

impl<S> AppHandlerWithDynamicReload<S>
where
    S: StateAccess + 'static + Clone + std::fmt::Debug,
{
    fn new(
        app: Arc<App>,
        state: AppState<S>,
        restart_flag: Arc<Mutex<bool>>,
        updated_app: Arc<Mutex<Option<App>>>
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
        }
    }

    /// ホットリロードされたアプリケーションをチェックして更新
    fn check_and_update_app(&mut self) {
        if let Ok(flag) = self.restart_flag.try_lock() {
            if *flag {
                // 新しいアプリケーションがあるかチェック
                if let Ok(mut app_guard) = self.updated_app.try_lock() {
                    if let Some(new_app) = app_guard.take() {
                        println!("{}","🔄 Applying hot reload update...".yellow());
                        self.current_app = Arc::new(new_app);

                        // 状態をリセット
                        self.state.static_stencils = None;
                        self.state.static_buttons.clear();
                        self.state.expanded_body = None;
                        self.state.cached_window_size = None;
                        self.button_handlers.clear();

                        // ウィンドウの再描画を要求
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }

                        println!("{}","✅ Hot reload update applied successfully!".green());
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
            let window_attributes = WindowAttributes::default()
                .with_title("Nilo Application - Hot Reload Enabled");
            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            self.renderer = Some(pollster::block_on(WgpuRenderer::new(window.clone())));
            self.window = Some(window);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
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
                self.target_scroll_offset[1] = (self.target_scroll_offset[1] + y).clamp(-max_scroll, 0.0);
                window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let old_mouse_pos = self.mouse_pos;
                self.mouse_pos_raw = [position.x as f32, position.y as f32];
                self.mouse_pos = [
                    self.mouse_pos_raw[0] / scale_factor,
                    (self.mouse_pos_raw[1] / scale_factor) - self.scroll_offset[1]
                ];

                if (old_mouse_pos[0] - self.mouse_pos[0]).abs() > 0.5 ||
                   (old_mouse_pos[1] - self.mouse_pos[1]).abs() > 0.5 {
                    self.state.static_stencils = None;
                    window.request_redraw();
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                self.mouse_down = true;
                window.request_redraw();
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                self.mouse_down = false;
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.scroll_offset[1] += (self.target_scroll_offset[1] - self.scroll_offset[1]) * self.smoothing;

                let window_size = [
                    window.inner_size().width as f32 / scale_factor,
                    window.inner_size().height as f32 / scale_factor
                ];

                let adjusted_mouse_pos = [
                    self.mouse_pos_raw[0] / scale_factor,
                    (self.mouse_pos_raw[1] / scale_factor) - self.scroll_offset[1]
                ];
                self.mouse_pos = adjusted_mouse_pos;

                // 現在のアプリケーションでレイアウ���実行
                let (stencils, buttons) = Engine::layout_and_stencil(
                    &self.current_app, &mut self.state,
                    self.mouse_pos, self.mouse_down, self.prev_mouse_down,
                    window_size
                );

                self.state.all_buttons = buttons.clone();

                // マウスイベント処理
                let mut current_hovered = None;
                for (id, pos, size) in &buttons {
                    let hover = {
                        let x = self.mouse_pos[0];
                        let y = self.mouse_pos[1];
                        let in_bounds = x >= pos[0] && x <= pos[0] + size[0] &&
                                       y >= pos[1] && y <= pos[1] + size[1];
                        in_bounds
                    };

                    if hover {
                        current_hovered = Some(id.clone());
                    }

                    if hover && self.mouse_down && !self.prev_mouse_down {
                        self.event_queue.push(UIEvent::ButtonPressed { id: id.clone() });
                    }
                    if hover && !self.mouse_down && self.prev_mouse_down {
                        self.event_queue.push(UIEvent::ButtonReleased { id: id.clone() });
                    }
                }

                if self.last_hovered_button != current_hovered {
                    self.last_hovered_button = current_hovered;
                    self.state.static_stencils = None;
                }

                // イベント処理
                let events_snapshot: Vec<UIEvent> = self.event_queue.queue.iter().cloned().collect();
                if !events_snapshot.is_empty() {
                    if let Some(new_tl) = Engine::step_whens(&self.current_app, &mut self.state, &events_snapshot) {
                        println!("[INFO] Timeline changed to {}", new_tl);

                        if let Some(tl) = self.state.current_timeline(&self.current_app) {
                            Engine::sync_button_handlers(&tl.body, &self.current_app.components, &mut self.button_handlers, |id| {
                                let id = id.to_owned();
                                Box::new(move |_st| println!("Button '{}' pressed (default handler)", id))
                            });
                        }

                        let (new_stencils, new_buttons) = Engine::layout_and_stencil(
                            &self.current_app, &mut self.state,
                            self.mouse_pos, self.mouse_down, self.prev_mouse_down,
                            window_size
                        );

                        self.state.all_buttons = new_buttons;

                        let size = renderer.size();
                        let viewport_h = size.height as f32 / scale_factor;
                        let viewport_w = size.width as f32 / scale_factor;
                        let mut vis = viewport::filter_visible_stencils(&new_stencils, self.scroll_offset, viewport_h);
                        let draw_full = stencil_to_wgpu_draw_list(&new_stencils);
                        self.content_length = draw_full.content_length();
                        vis = viewport::inject_scrollbar(vis, self.content_length, viewport_h, viewport_w, self.scroll_offset[1]);
                        let draw_list = stencil_to_wgpu_draw_list(&vis);
                        renderer.render(&draw_list, self.scroll_offset, scale_factor);

                        self.prev_mouse_down = self.mouse_down;
                        return;
                    }
                }

                // ボタンハンドラ同期
                if let Some(tl) = self.state.current_timeline(&self.current_app) {
                    Engine::sync_button_handlers(
                        &tl.body, &self.current_app.components, &mut self.button_handlers,
                        |id| {
                            let id = id.to_owned();
                            Box::new(move |_st| println!("Button '{}' pressed (default handler)", id))
                        }
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
                let mut vis = viewport::filter_visible_stencils(&stencils, self.scroll_offset, viewport_h);
                let draw_full = stencil_to_wgpu_draw_list(&stencils);
                self.content_length = draw_full.content_length();
                vis = viewport::inject_scrollbar(vis, self.content_length, viewport_h, viewport_w, self.scroll_offset[1]);
                let draw_list = stencil_to_wgpu_draw_list(&vis);
                renderer.render(&draw_list, self.scroll_offset, scale_factor);

                self.prev_mouse_down = self.mouse_down;
            }
            _ => {}
        }
    }
}

/// ウィンドウタイトルを指定してアプリケーションを実行
pub fn run_with_window_title<S: StateAccess + 'static + Clone + std::fmt::Debug>(
    app: App, 
    custom_state: S, 
    window_title: Option<&str>
) {
    let start = app.flow.start.clone();
    let state = AppState::new(custom_state, start);
    let app = Arc::new(app);
    
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let title = window_title.unwrap_or("My Application").to_string();
    let mut app_handler = AppHandler::new(app, state, title);
    event_loop.run_app(&mut app_handler).unwrap();
}

/// ホットリロード機能付きでウィンドウタイトルを指定してアプリケーションを実行
pub fn run_with_hotreload_support_and_title<S: StateAccess + 'static + Clone + std::fmt::Debug>(
    initial_app: Arc<App>,
    initial_state: AppState<S>,
    restart_flag: Arc<Mutex<bool>>,
    updated_app: Arc<Mutex<Option<App>>>,
    window_title: Option<&str>
) {
    env_logger::init();

    // 単一のイベントループを作成（一度だけ）
    let event_loop = EventLoop::new().unwrap();

    // ホットリロード対応のAppHandlerを作成（ウィンドウタイトル付き）
    let mut app_handler = AppHandlerWithDynamicReloadAndTitle::new(
        initial_app,
        initial_state,
        restart_flag,
        updated_app,
        window_title
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
    window_title: String,

    // ホットリロード用
    restart_flag: Arc<Mutex<bool>>,
    updated_app: Arc<Mutex<Option<App>>>,
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
        window_title: Option<&str>
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
        }
    }

    /// ホットリロードされたアプリケーションをチェックして更新
    fn check_and_update_app(&mut self) {
        if let Ok(flag) = self.restart_flag.try_lock() {
            if *flag {
                // 新しいアプリケーションがあるかチェック
                if let Ok(mut app_guard) = self.updated_app.try_lock() {
                    if let Some(new_app) = app_guard.take() {
                        println!("{}","🔄 Applying hot reload update...".yellow());
                        self.current_app = Arc::new(new_app);

                        // 状態をリセット
                        self.state.static_stencils = None;
                        self.state.static_buttons.clear();
                        self.state.expanded_body = None;
                        self.state.cached_window_size = None;
                        self.button_handlers.clear();

                        // ウィンドウの再描画を要求
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }

                        println!("{}","✅ Hot reload update applied successfully!".green());
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
            let window_attributes = WindowAttributes::default()
                .with_title(&self.window_title);
            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            self.renderer = Some(pollster::block_on(WgpuRenderer::new(window.clone())));
            self.window = Some(window);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
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
                self.target_scroll_offset[1] = (self.target_scroll_offset[1] + y).clamp(-max_scroll, 0.0);
                window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let old_mouse_pos = self.mouse_pos;
                self.mouse_pos_raw = [position.x as f32, position.y as f32];
                self.mouse_pos = [
                    self.mouse_pos_raw[0] / scale_factor,
                    (self.mouse_pos_raw[1] / scale_factor) - self.scroll_offset[1]
                ];

                if (old_mouse_pos[0] - self.mouse_pos[0]).abs() > 0.5 ||
                   (old_mouse_pos[1] - self.mouse_pos[1]).abs() > 0.5 {
                    self.state.static_stencils = None;
                    window.request_redraw();
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                self.mouse_down = true;
                window.request_redraw();
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                self.mouse_down = false;
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.scroll_offset[1] += (self.target_scroll_offset[1] - self.scroll_offset[1]) * self.smoothing;

                let window_size = [
                    window.inner_size().width as f32 / scale_factor,
                    window.inner_size().height as f32 / scale_factor
                ];

                let adjusted_mouse_pos = [
                    self.mouse_pos_raw[0] / scale_factor,
                    (self.mouse_pos_raw[1] / scale_factor) - self.scroll_offset[1]
                ];
                self.mouse_pos = adjusted_mouse_pos;

                // 現在のアプリケーションでレイアウト実行
                let (stencils, buttons) = Engine::layout_and_stencil(
                    &self.current_app, &mut self.state,
                    self.mouse_pos, self.mouse_down, self.prev_mouse_down,
                    window_size
                );

                self.state.all_buttons = buttons.clone();

                // マウスイベント処理
                let mut current_hovered = None;
                for (id, pos, size) in &buttons {
                    let hover = {
                        let x = self.mouse_pos[0];
                        let y = self.mouse_pos[1];
                        let in_bounds = x >= pos[0] && x <= pos[0] + size[0] &&
                                       y >= pos[1] && y <= pos[1] + size[1];
                        in_bounds
                    };

                    if hover {
                        current_hovered = Some(id.clone());
                    }

                    if hover && self.mouse_down && !self.prev_mouse_down {
                        self.event_queue.push(UIEvent::ButtonPressed { id: id.clone() });
                    }
                    if hover && !self.mouse_down && self.prev_mouse_down {
                        self.event_queue.push(UIEvent::ButtonReleased { id: id.clone() });
                    }
                }

                if self.last_hovered_button != current_hovered {
                    self.last_hovered_button = current_hovered;
                    self.state.static_stencils = None;
                }

                // イベント処理
                let events_snapshot: Vec<UIEvent> = self.event_queue.queue.iter().cloned().collect();
                if !events_snapshot.is_empty() {
                    if let Some(new_tl) = Engine::step_whens(&self.current_app, &mut self.state, &events_snapshot) {
                        println!("[INFO] Timeline changed to {}", new_tl);

                        if let Some(tl) = self.state.current_timeline(&self.current_app) {
                            Engine::sync_button_handlers(&tl.body, &self.current_app.components, &mut self.button_handlers, |id| {
                                let id = id.to_owned();
                                Box::new(move |_st| println!("Button '{}' pressed (default handler)", id))
                            });
                        }

                        let (new_stencils, new_buttons) = Engine::layout_and_stencil(
                            &self.current_app, &mut self.state,
                            self.mouse_pos, self.mouse_down, self.prev_mouse_down,
                            window_size
                        );

                        self.state.all_buttons = new_buttons;

                        let size = renderer.size();
                        let viewport_h = size.height as f32 / scale_factor;
                        let viewport_w = size.width as f32 / scale_factor;
                        let mut vis = viewport::filter_visible_stencils(&new_stencils, self.scroll_offset, viewport_h);
                        let draw_full = stencil_to_wgpu_draw_list(&new_stencils);
                        self.content_length = draw_full.content_length();
                        vis = viewport::inject_scrollbar(vis, self.content_length, viewport_h, viewport_w, self.scroll_offset[1]);
                        let draw_list = stencil_to_wgpu_draw_list(&vis);
                        renderer.render(&draw_list, self.scroll_offset, scale_factor);

                        self.prev_mouse_down = self.mouse_down;
                        return;
                    }
                }

                // ボタンハンドラ同期
                if let Some(tl) = self.state.current_timeline(&self.current_app) {
                    Engine::sync_button_handlers(
                        &tl.body, &self.current_app.components, &mut self.button_handlers,
                        |id| {
                            let id = id.to_owned();
                            Box::new(move |_st| println!("Button '{}' pressed (default handler)", id))
                        }
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
                let mut vis = viewport::filter_visible_stencils(&stencils, self.scroll_offset, viewport_h);
                let draw_full = stencil_to_wgpu_draw_list(&stencils);
                self.content_length = draw_full.content_length();
                vis = viewport::inject_scrollbar(vis, self.content_length, viewport_h, viewport_w, self.scroll_offset[1]);
                let draw_list = stencil_to_wgpu_draw_list(&vis);
                renderer.render(&draw_list, self.scroll_offset, scale_factor);

                self.prev_mouse_down = self.mouse_down;
            }
            _ => {}
        }
    }
}
