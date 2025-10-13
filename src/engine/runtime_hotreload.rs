use super::runtime::AppHandler;
use super::state::{AppState, StateAccess};
use crate::parser::ast::App;
use std::sync::{Arc, Mutex};
use winit::{
    application::ApplicationHandler, event::WindowEvent, event_loop::ActiveEventLoop,
    window::WindowId,
};

/// 再起動フラグ付きのAppHandler
pub struct AppHandlerWithRestart<S>
where
    S: StateAccess + 'static + Clone + std::fmt::Debug,
{
    pub inner: AppHandler<S>,
    pub restart_flag: Arc<Mutex<bool>>,
}

impl<S> AppHandlerWithRestart<S>
where
    S: StateAccess + 'static + Clone + std::fmt::Debug,
{
    pub fn new(app: Arc<App>, state: AppState<S>, restart_flag: Arc<Mutex<bool>>) -> Self {
        Self {
            inner: AppHandler::new(app, state, "Nilo Application".to_string()),
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
pub struct AppHandlerWithHotReload<S>
where
    S: StateAccess + 'static + Clone + std::fmt::Debug,
{
    pub inner: AppHandler<S>,
    pub restart_flag: Arc<Mutex<bool>>,
}

impl<S> AppHandlerWithHotReload<S>
where
    S: StateAccess + 'static + Clone + std::fmt::Debug,
{
    pub fn new(app: Arc<App>, state: AppState<S>, restart_flag: Arc<Mutex<bool>>) -> Self {
        Self {
            inner: AppHandler::new(app, state, "Nilo Application".to_string()),
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
                // 再起動が要求されている場合はイベントループを終了
                event_loop.exit();
                return;
            }
        }

        // 通常のイベント処理を委譲
        self.inner.window_event(event_loop, window_id, event);
    }
}
