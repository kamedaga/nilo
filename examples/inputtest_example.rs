// TextInputのテスト
const MY_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP-Regular.ttf"));

use nilo::{AppState, StateAccess};

nilo::nilo_state! {
    struct State {
        input: String,
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            input: String::new(),
        }
    }
}

fn main() {
    env_logger::init();
    
    // カスタムフォントを登録
    nilo::set_custom_font("japanese", MY_FONT);
    
    let cli_args = nilo::parse_args();
    let state = State::default();
    
    // run_nilo_app!マクロを使用
    nilo::run_nilo_app!("src/inputtest.nilo", state, &cli_args, Some("TextInput Test"));
}
