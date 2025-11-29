use nilo::nilo_safe_accessible;
use nilo::parser::ast::Expr;
use serde::Deserialize;
use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;

#[cfg(not(target_arch = "wasm32"))]
use tokio::task::JoinSet;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const MY_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fonts/NotoSansJP-Regular.ttf"
));

/// 天気一覧を管理する状態
nilo::nilo_state! {
    struct WeatherState {
        cards: Vec<String>,        // 一覧カードのテキスト
        updated: String,           // 最終更新時刻
        status: String,            // ステータス表示
        detail_days: Vec<String>,  // 詳細画面の日別テキスト
        detail_updated: String,    // 詳細の更新時刻
        selected_city: String,     // 選択中の都市名
        current_region: String,    // 現在選択中の地域キー
        region_names: Vec<String>, // 地域選択画面の都市名一覧
        region_weather_cards: Vec<String>, // 地域選択画面の天気カード（都市名+天気）
        weather_description: String, // 選択中の都市の天気説明
        tokyo_description: String,  // 東京の天気説明（ホーム表示用）
    }
}

impl Default for WeatherState {
    fn default() -> Self {
        Self {
            cards: vec![],
            updated: "読み込み中...".to_string(),
            status: "取得中...".to_string(),
            detail_days: vec![],
            detail_updated: "N/A".to_string(),
            selected_city: "N/A".to_string(),
            current_region: "all".to_string(),
            region_names: default_region_names(),
            region_weather_cards: vec![],
            weather_description: "読み込み中...".to_string(),
            tokyo_description: "読み込み中...".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    #[serde(rename = "publicTimeFormatted")]
    public_time: String,
    forecasts: Vec<Forecast>,
    location: Location,
    description: Option<Description>,
}

#[derive(Debug, Deserialize)]
struct Description {
    text: String,
}

#[derive(Debug, Deserialize)]
struct Forecast {
    date: String,
    #[serde(rename = "dateLabel")]
    date_label: String,
    telop: String,
}

#[derive(Debug, Deserialize)]
struct Location {
    city: String,
    prefecture: String,
}

#[derive(Debug)]
struct CityCard {
    display: String,
    updated: String,
    city: String,
    detail_days: Vec<String>,
    weather_description: String,
}

#[derive(Clone)]
struct RegionDefinition {
    label: &'static str,
    cities: Vec<(&'static str, &'static str)>,
}

struct RegionFetchResult {
    cards: Vec<String>,
    updated: String,
    detail_days: Vec<String>,
    detail_updated: String,
    selected_city: String,
    success_count: usize,
    region_weather_cards: Vec<String>, // 都市名 + 天気
    weather_description: String,
    tokyo_description: String,
}

struct RegionSnapshot {
    cards: Vec<String>,
    updated: String,
    status: String,
    detail_days: Vec<String>,
    detail_updated: String,
    selected_city: String,
    current_region: String,
    region_names: Vec<String>,
    region_weather_cards: Vec<String>,
    weather_description: String,
    tokyo_description: String,
}

#[cfg(not(target_arch = "wasm32"))]
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("Tokio runtime must be available"))
}

fn region_map() -> HashMap<&'static str, RegionDefinition> {
    HashMap::from([
        (
            "all",
            RegionDefinition {
                label: "全国",
                cities: vec![
                    ("札幌", "016010"),
                    ("仙台", "040010"),
                    ("東京", "130010"),
                    ("名古屋", "230010"),
                    ("大阪", "270000"),
                    ("福岡", "400010"),
                    ("那覇(南部)", "471010"),
                ],
            },
        ),
        (
            "hokkaido_tohoku",
            RegionDefinition {
                label: "北海道・東北",
                cities: vec![("札幌", "016010"), ("青森", "020010"), ("仙台", "040010")],
            },
        ),
        (
            "kanto_chubu",
            RegionDefinition {
                label: "関東・中部",
                cities: vec![("東京", "130010"), ("新潟", "150010"), ("金沢", "170010"), ("名古屋", "230010")],
            },
        ),
        (
            "west_japan",
            RegionDefinition {
                label: "西日本・南西",
                cities: vec![("大阪", "270000"), ("広島", "340010"), ("福岡", "400010"), ("那覇(南部)", "471010")],
            },
        ),
    ])
}

fn default_region_names() -> Vec<String> {
    region_map()
        .get("all")
        .map(|r| region_names_for(r))
        .unwrap_or_default()
}

fn region_names_for(region: &RegionDefinition) -> Vec<String> {
    region.cities.iter().map(|(name, _)| (*name).to_string()).collect()
}

fn replace_list(ctx: &mut nilo::CustomStateContext<WeatherState>, key: &str, values: &[String]) {
    if let Err(e) = ctx.list_clear(key) {
        log::error!("{} のクリアに失敗: {}", key, e);
        return;
    }

    for v in values {
        if let Err(e) = ctx.list_append(key, v.clone()) {
            log::error!("{} への追加に失敗: {}", key, e);
            break;
        }
    }
}

async fn fetch_city(code: &str, name: &str) -> Result<CityCard, String> {
    let client = reqwest::Client::builder()
        .user_agent("nilo-weather-multi-city")
        .build()
        .map_err(|e| format!("クライアントの生成に失敗: {}", e))?;

    let url = format!("https://weather.tsukumijima.net/api/forecast?city={}", code);

    let text = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("{} の取得に失敗: {}", name, e))?
        .text()
        .await
        .map_err(|e| format!("{} のレスポンス読み込みに失敗: {}", name, e))?;

    let parsed: ApiResponse =
        serde_json::from_str(&text).map_err(|e| format!("{} のJSONパースに失敗: {}", name, e))?;

    if parsed.forecasts.is_empty() {
        return Err(format!("{} の予報が空でした", name));
    }

    let detail_days: Vec<String> = parsed
        .forecasts
        .iter()
        .map(|f| format!("{} {} {}", f.date_label, f.date, f.telop))
        .collect();

    let first = &parsed.forecasts[0];

    let display = format!("{} | {} ({}) | {}", name, first.telop, first.date_label, first.date);

    let weather_description = parsed
        .description
        .map(|d| d.text)
        .unwrap_or_else(|| "天気の説明がありません".to_string());

    Ok(CityCard {
        display,
        updated: parsed.public_time,
        city: name.to_string(),
        detail_days,
        weather_description,
    })
}

#[cfg(not(target_arch = "wasm32"))]
async fn fetch_region_parallel(cities: &[(&str, &str)]) -> RegionFetchResult {
    let mut set: JoinSet<(String, Result<CityCard, String>)> = JoinSet::new();

    for (name, code) in cities {
        let name = (*name).to_string();
        let code = (*code).to_string();
        set.spawn(async move { (name.clone(), fetch_city(&code, &name).await) });
    }

    let mut cards = Vec::new();
    let mut updated = String::new();
    let mut detail_days = Vec::new();
    let mut detail_updated = String::new();
    let mut selected_city = String::new();
    let mut success_count = 0usize;
    let mut region_weather_cards = Vec::new();
    let mut weather_description = String::new();
    let mut tokyo_description = String::new();

    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((_name, Ok(card))) => {
                if updated.is_empty() {
                    updated = card.updated.clone();
                }
                if detail_days.is_empty() {
                    detail_days = card.detail_days.clone();
                    detail_updated = card.updated.clone();
                    selected_city = card.city.clone();
                    weather_description = card.weather_description.clone();
                }
                // 東京の説明を保存
                if card.city == "東京" {
                    tokyo_description = card.weather_description.clone();
                }
                // 都市名 + 天気情報を生成
                let weather_info = if !card.detail_days.is_empty() {
                    // 最初の予報から天気を抽出
                    let parts: Vec<&str> = card.detail_days[0].split_whitespace().collect();
                    if parts.len() >= 3 {
                        format!("{} | {}", card.city, parts[2])
                    } else {
                        format!("{} | 天気情報なし", card.city)
                    }
                } else {
                    format!("{} | 天気情報なし", card.city)
                };
                region_weather_cards.push(weather_info);
                cards.push(card.display);
                success_count += 1;
            }
            Ok((name, Err(err))) => {
                log::error!("{}: {}", name, err);
                cards.push(format!("{} | 取得に失敗", name));
                region_weather_cards.push(format!("{} | 取得失敗", name));
            }
            Err(err) => {
                log::error!("並行取得中のタスクが失敗しました: {}", err);
                cards.push("取得に失敗した都市があります".to_string());
                region_weather_cards.push("取得失敗".to_string());
            }
        }
    }

    if cards.is_empty() {
        cards.push("データがありません".to_string());
    }

    RegionFetchResult {
        cards,
        updated: if updated.is_empty() { "N/A".to_string() } else { updated },
        detail_days,
        detail_updated: if detail_updated.is_empty() { "N/A".to_string() } else { detail_updated },
        selected_city: if selected_city.is_empty() { "N/A".to_string() } else { selected_city },
        success_count,
        region_weather_cards,
        weather_description: if weather_description.is_empty() { "天気の説明がありません".to_string() } else { weather_description },
        tokyo_description: if tokyo_description.is_empty() { "東京の天気情報を取得できませんでした".to_string() } else { tokyo_description },
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_region_parallel(cities: &[(&str, &str)]) -> RegionFetchResult {
    // WASM版: 逐次処理（並列化なし）
    let mut cards = Vec::new();
    let mut updated = String::new();
    let mut detail_days = Vec::new();
    let mut detail_updated = String::new();
    let mut selected_city = String::new();
    let mut success_count = 0usize;
    let mut region_weather_cards = Vec::new();
    let mut weather_description = String::new();
    let mut tokyo_description = String::new();

    for (name, code) in cities {
        match fetch_city(code, name).await {
            Ok(card) => {
                if updated.is_empty() {
                    updated = card.updated.clone();
                }
                if detail_days.is_empty() {
                    detail_days = card.detail_days.clone();
                    detail_updated = card.updated.clone();
                    selected_city = card.city.clone();
                    weather_description = card.weather_description.clone();
                }
                // 東京の説明を保存
                if card.city == "東京" {
                    tokyo_description = card.weather_description.clone();
                }
                // 都市名 + 天気情報を生成
                let weather_info = if !card.detail_days.is_empty() {
                    let parts: Vec<&str> = card.detail_days[0].split_whitespace().collect();
                    if parts.len() >= 3 {
                        format!("{} | {}", card.city, parts[2])
                    } else {
                        format!("{} | 天気情報なし", card.city)
                    }
                } else {
                    format!("{} | 天気情報なし", card.city)
                };
                region_weather_cards.push(weather_info);
                cards.push(card.display);
                success_count += 1;
            }
            Err(err) => {
                log::error!("{}: {}", name, err);
                cards.push(format!("{} | 取得に失敗", name));
                region_weather_cards.push(format!("{} | 取得失敗", name));
            }
        }
    }

    if cards.is_empty() {
        cards.push("データがありません".to_string());
    }

    RegionFetchResult {
        cards,
        updated: if updated.is_empty() { "N/A".to_string() } else { updated },
        detail_days,
        detail_updated: if detail_updated.is_empty() { "N/A".to_string() } else { detail_updated },
        selected_city: if selected_city.is_empty() { "N/A".to_string() } else { selected_city },
        success_count,
        region_weather_cards,
        weather_description: if weather_description.is_empty() { "天気の説明がありません".to_string() } else { weather_description },
        tokyo_description: if tokyo_description.is_empty() { "東京の天気情報を取得できませんでした".to_string() } else { tokyo_description },
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_region_snapshot(region_key: &str) -> RegionSnapshot {
    let regions = region_map();
    let (resolved_key, region) = regions
        .get(region_key)
        .map(|def| (region_key, def.clone()))
        .or_else(|| regions.get("all").map(|def| ("all", def.clone())))
        .expect("region map must contain 'all'");

    let fetch_result = runtime().block_on(fetch_region_parallel(&region.cities));
    let status = if fetch_result.success_count > 0 {
        format!("{}の{}都市の最新天気を取得", region.label, fetch_result.success_count)
    } else {
        format!("{}の天気取得に失敗しました", region.label)
    };

    RegionSnapshot {
        cards: fetch_result.cards,
        updated: fetch_result.updated,
        status,
        detail_days: fetch_result.detail_days,
        detail_updated: fetch_result.detail_updated,
        selected_city: fetch_result.selected_city,
        current_region: resolved_key.to_string(),
        region_names: region_names_for(&region),
        region_weather_cards: fetch_result.region_weather_cards,
        weather_description: fetch_result.weather_description,
        tokyo_description: fetch_result.tokyo_description,
    }
}

fn apply_snapshot(ctx: &mut nilo::CustomStateContext<WeatherState>, snapshot: &RegionSnapshot) {
    replace_list(ctx, "cards", &snapshot.cards);
    replace_list(ctx, "detail_days", &snapshot.detail_days);
    replace_list(ctx, "region_names", &snapshot.region_names);
    replace_list(ctx, "region_weather_cards", &snapshot.region_weather_cards);

    for (key, value) in [
        ("updated", snapshot.updated.clone()),
        ("status", snapshot.status.clone()),
        ("detail_updated", snapshot.detail_updated.clone()),
        ("selected_city", snapshot.selected_city.clone()),
        ("current_region", snapshot.current_region.clone()),
        ("weather_description", snapshot.weather_description.clone()),
        ("tokyo_description", snapshot.tokyo_description.clone()),
    ] {
        if let Err(e) = ctx.set(key, value) {
            log::error!("{} の更新に失敗: {}", key, e);
        }
    }
}

#[nilo_safe_accessible(state = WeatherState, name = "load_region")]
fn load_region(ctx: &mut nilo::CustomStateContext<WeatherState>, args: &[Expr]) {
    let region_key = match args.first() {
        Some(Expr::String(s)) => s.as_str(),
        _ => "all",
    };

    let regions = region_map();
    let (resolved_key, region) = regions
        .get(region_key)
        .map(|def| (region_key, def.clone()))
        .or_else(|| regions.get("all").map(|def| ("all", def.clone())))
        .expect("region map must contain 'all'");

    // 先に地域リストと選択中地域を即時反映して、画面遷移後すぐにカードを出す
    replace_list(ctx, "region_names", &region_names_for(&region));
    if let Err(e) = ctx.set("current_region", resolved_key.to_string()) {
        log::error!("current_region の更新に失敗: {}", e);
    }
    if let Err(e) = ctx.set("status", format!("{}の天気を取得中...", region.label)) {
        log::error!("status の更新に失敗: {}", e);
    }

    // 取得結果で最終状態を上書き
    #[cfg(not(target_arch = "wasm32"))]
    {
        let snapshot = load_region_snapshot(resolved_key);
        apply_snapshot(ctx, &snapshot);
    }

    #[cfg(target_arch = "wasm32")]
    {
        // WASMでは非同期処理を別途実行（ここでは初期値のみ設定）
        log::info!("WASM: load_region for {} scheduled", resolved_key);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[nilo_safe_accessible(state = WeatherState, name = "select_city")]
fn select_city(ctx: &mut nilo::CustomStateContext<WeatherState>, args: &[Expr]) {
    let Some(Expr::String(city_name)) = args.first() else {
        log::warn!("select_city: city name is missing");
        return;
    };

    let region_key = ctx.get("current_region").unwrap_or_else(|| "all".to_string());
    let regions = region_map();
    let region = regions
        .get(region_key.as_str())
        .or_else(|| regions.get("all"))
        .cloned();

    let Some(region) = region else {
        log::warn!("select_city: region '{}' not found", region_key);
        return;
    };

    let Some((_, code)) = region.cities.iter().find(|(name, _)| *name == city_name.as_str()) else {
        log::warn!("select_city: '{}' not found in current region", city_name);
        return;
    };

    match runtime().block_on(fetch_city(code, city_name)) {
        Ok(card) => {
            replace_list(ctx, "detail_days", &card.detail_days);
            if let Err(e) = ctx.set("detail_updated", card.updated.clone()) {
                log::error!("detail_updated の更新に失敗: {}", e);
            }
            if let Err(e) = ctx.set("selected_city", card.city.clone()) {
                log::error!("selected_city の更新に失敗: {}", e);
            }
            if let Err(e) = ctx.set("weather_description", card.weather_description.clone()) {
                log::error!("weather_description の更新に失敗: {}", e);
            }
        }
        Err(err) => {
            let message = format!("{} の取得に失敗: {}", city_name, err);
            log::error!("{}", message);
            if let Err(e) = ctx.set("status", message) {
                log::error!("status の更新に失敗: {}", e);
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[nilo_safe_accessible(state = WeatherState, name = "select_city_from_card")]
fn select_city_from_card(ctx: &mut nilo::CustomStateContext<WeatherState>, args: &[Expr]) {
    let Some(Expr::String(card_text)) = args.first() else {
        log::warn!("select_city_from_card: card text is missing");
        return;
    };

    // カードテキストから都市名を抽出 (形式: "都市名 | 天気")
    let city_name = card_text.split('|').next().unwrap_or("").trim();
    
    if city_name.is_empty() {
        log::warn!("select_city_from_card: could not extract city name from '{}'", card_text);
        return;
    };

    let region_key = ctx.get("current_region").unwrap_or_else(|| "all".to_string());
    let regions = region_map();
    let region = regions
        .get(region_key.as_str())
        .or_else(|| regions.get("all"))
        .cloned();

    let Some(region) = region else {
        log::warn!("select_city_from_card: region '{}' not found", region_key);
        return;
    };

    let Some((_, code)) = region.cities.iter().find(|(name, _)| *name == city_name) else {
        log::warn!("select_city_from_card: '{}' not found in current region", city_name);
        return;
    };

    match runtime().block_on(fetch_city(code, city_name)) {
        Ok(card) => {
            replace_list(ctx, "detail_days", &card.detail_days);
            if let Err(e) = ctx.set("detail_updated", card.updated.clone()) {
                log::error!("detail_updated の更新に失敗: {}", e);
            }
            if let Err(e) = ctx.set("selected_city", card.city.clone()) {
                log::error!("selected_city の更新に失敗: {}", e);
            }
            if let Err(e) = ctx.set("weather_description", card.weather_description.clone()) {
                log::error!("weather_description の更新に失敗: {}", e);
            }
        }
        Err(err) => {
            let message = format!("{} の取得に失敗: {}", city_name, err);
            log::error!("{}", message);
            if let Err(e) = ctx.set("status", message) {
                log::error!("status の更新に失敗: {}", e);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[nilo_safe_accessible(state = WeatherState, name = "select_city")]
fn select_city(_ctx: &mut nilo::CustomStateContext<WeatherState>, _args: &[Expr]) {
    log::info!("WASM: select_city called (not implemented)");
}

#[cfg(target_arch = "wasm32")]
#[nilo_safe_accessible(state = WeatherState, name = "select_city_from_card")]
fn select_city_from_card(_ctx: &mut nilo::CustomStateContext<WeatherState>, _args: &[Expr]) {
    log::info!("WASM: select_city_from_card called (not implemented)");
}

#[cfg(target_arch = "wasm32")]
fn register_wasm_functions() {
    // WASM ではマクロ登録が走らないので手動登録
    nilo::register_state_accessible_call(
        "load_region",
        |app_state: &mut nilo::AppState<WeatherState>, args: &[Expr]| {
            nilo::engine::state::with_custom_state(app_state, |ctx| {
                load_region(ctx, args);
            });
        },
    );
    nilo::register_state_accessible_call(
        "select_city",
        |app_state: &mut nilo::AppState<WeatherState>, args: &[Expr]| {
            nilo::engine::state::with_custom_state(app_state, |ctx| {
                select_city(ctx, args);
            });
        },
    );
    nilo::register_state_accessible_call(
        "select_city_from_card",
        |app_state: &mut nilo::AppState<WeatherState>, args: &[Expr]| {
            nilo::engine::state::with_custom_state(app_state, |ctx| {
                select_city_from_card(ctx, args);
            });
        },
    );
}

fn build_initial_state() -> WeatherState {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let snapshot = load_region_snapshot("all");
        let mut state = WeatherState::default();
        state.cards = snapshot.cards;
        state.updated = snapshot.updated;
        state.status = snapshot.status;
        state.detail_days = snapshot.detail_days;
        state.detail_updated = snapshot.detail_updated;
        state.selected_city = snapshot.selected_city;
        state.current_region = snapshot.current_region;
        state.region_names = snapshot.region_names;
        state.region_weather_cards = snapshot.region_weather_cards;
        state.weather_description = snapshot.weather_description;
        state.tokyo_description = snapshot.tokyo_description;
        state
    }
    
    #[cfg(target_arch = "wasm32")]
    {
        WeatherState::default()
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();

        // Nilo関数の自動登録
        nilo::init_nilo_functions();

        let cli_args = nilo::parse_args();
        let state = build_initial_state();

        // カスタムフォントを登録
        nilo::set_custom_font("japanese", MY_FONT);

        nilo::run_nilo_app!(
            "examples/weather_app.nilo",
            state,
            &cli_args,
            Some("Japan Cities Weather")
        );
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
extern crate console_error_panic_hook;

#[cfg(target_arch = "wasm32")]
extern crate console_log;

// WASM版のエントリポイント
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    // パニック時のエラーメッセージをブラウザコンソールに表示
    console_error_panic_hook::set_once();

    // WebAssembly用のロガーを初期化
    console_log::init_with_level(log::Level::Debug).expect("error initializing log");

    log::info!("🚀 WASM Weather App starting...");

    // Nilo関数の自動登録
    nilo::init_nilo_functions();
    
    // WASM: manual registrations (macros don't auto-register here)
    register_wasm_functions();

    let state = build_initial_state();

    // カスタムフォントを登録
    nilo::set_custom_font("japanese", MY_FONT);

    nilo::run_nilo_app!("examples/weather_app.nilo", state);
}
