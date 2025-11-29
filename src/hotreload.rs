use log::{error, info};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct HotReloader {
    _watcher: RecommendedWatcher,
    reload_callback: Arc<Mutex<Option<Box<dyn Fn() + Send + 'static>>>>,
}

impl HotReloader {
    /// 新しいホットリローダーを作成
    pub fn new<P: AsRef<Path>>(watch_path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, rx): (
            Sender<notify::Result<Event>>,
            Receiver<notify::Result<Event>>,
        ) = mpsc::channel();

        let config = Config::default().with_poll_interval(Duration::from_millis(100));

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                if let Err(e) = tx.send(res) {
                    error!("Error sending watch event: {}", e); // eprintln!をerror!に変更
                }
            },
            config,
        )?;

        watcher.watch(watch_path.as_ref(), RecursiveMode::Recursive)?;

        info!(
            "🔥 Hot reload enabled for: {}",
            watch_path.as_ref().display()
        );

        let reload_callback: Arc<Mutex<Option<Box<dyn Fn() + Send + 'static>>>> =
            Arc::new(Mutex::new(None));

        // ファイル監視を別スレッドで開始
        let callback_clone = Arc::clone(&reload_callback);
        thread::spawn(move || {
            loop {
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(Ok(event)) => {
                        if should_reload(&event) {
                            info!("🔄 File changed, reloading...");

                            // 少し待ってからリロード（ファイル書き込みが完了するのを待つ）
                            thread::sleep(Duration::from_millis(100));

                            if let Ok(callback_guard) = callback_clone.lock() {
                                if let Some(ref cb) = *callback_guard {
                                    cb();
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        error!("Watch error: {:?}", e);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // タイムアウトは正常、続行
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        error!("Watcher disconnected");
                        break;
                    }
                }
            }
        });

        Ok(HotReloader {
            _watcher: watcher,
            reload_callback,
        })
    }

    /// リロード時のコールバック関数を設定
    pub fn set_reload_callback<F>(&self, callback: F)
    where
        F: Fn() + Send + 'static,
    {
        let mut cb = self.reload_callback.lock().unwrap();
        *cb = Some(Box::new(callback));
    }
}

/// ファイル変更イベントがリロードをトリガーすべきかを判断
fn should_reload(event: &Event) -> bool {
    use notify::EventKind;

    match event.kind {
        EventKind::Modify(_) | EventKind::Create(_) => {
            // .niloファイルの変更のみを監視
            event.paths.iter().any(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "nilo")
                    .unwrap_or(false)
            })
        }
        _ => false,
    }
}
