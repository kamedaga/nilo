// TextInputのテスト
const MY_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/NotoSansJP-Regular.ttf"));


nilo::nilo_state! {
    struct State {
    }
}

fn main() {
    env_logger::init();
    
    let cli_args = nilo::parse_args();
    let state = State {};
    
    // run_nilo_app!マクロを使用
    nilo::run_nilo_app!("src/app.nilo", state, &cli_args, Some("Nilo App"));
}
