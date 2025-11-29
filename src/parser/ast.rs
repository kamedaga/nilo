// ========================================
// Nilo言語 AST定義
// ========================================

use log::debug; // debug!マクロを使用するためのインポートを追加

// ========================================
// メインアプリケーション構造
// ========================================

#[derive(Debug, Clone)]
pub struct App {
    pub flow: Flow,
    pub timelines: Vec<Timeline>,
    pub components: Vec<Component>,
}

#[derive(Debug, Clone)]
pub struct Flow {
    pub start: String,
    pub start_url: Option<String>, // WASM用URL
    pub transitions: Vec<FlowTransition>,
}

impl Flow {
    /// 配列形式の遷移元を展開して正規化する
    /// 例: [A, B, C] -> D を A -> D, B -> D, C -> D に展開
    pub fn normalize(mut self) -> Self {
        let mut expanded_transitions = Vec::new();

        for transition in self.transitions {
            if transition.from.len() == 1 {
                // 単一の遷移元の場合はそのまま追加
                expanded_transitions.push(transition);
            } else {
                // 複数の遷移元の場合は展開
                for from_timeline in &transition.from {
                    expanded_transitions.push(FlowTransition {
                        from: vec![from_timeline.clone()],
                        to: transition.to.clone(),
                    });
                }
            }
        }

        self.transitions = expanded_transitions;
        self
    }
}

#[derive(Debug, Clone)]
pub struct FlowTransition {
    pub from: Vec<String>,
    pub to: Vec<FlowTarget>,
}

#[derive(Debug, Clone)]
pub struct FlowTarget {
    pub timeline: String,
    pub url: Option<String>, // WASM用URL
    pub params: std::collections::HashMap<String, UrlParam>,
}

#[derive(Debug, Clone)]
pub enum UrlParam {
    Required(String), // :userId
    Optional(String), // :id?
    Wildcard,         // *
}

// ========================================
// 階層的フロー糖衣構文用の型定義
// ========================================

/// 階層的フロー定義の中間表現 (flow Login { ... })
#[derive(Debug, Clone)]
pub struct NamespacedFlow {
    pub name: String,
    pub start: String,
    pub transitions: Vec<NamespacedTransition>,
}

#[derive(Debug, Clone)]
pub struct NamespacedTransition {
    pub from: Vec<String>,
    pub to: Vec<String>,
}

/// 名前空間定義 (namespace Login { ... })
#[derive(Debug, Clone)]
pub struct Namespace {
    pub name: String,
    pub timelines: Vec<Timeline>,
    pub components: Vec<Component>,
}

#[derive(Debug, Clone)]
pub struct Timeline {
    pub name: String,
    pub url_pattern: Option<String>, // ★ 追加: タイムラインのURLパターン
    pub font: Option<String>,        // ★ 追加: タイムライン全体で使用するフォント
    pub background: Option<String>,  // ★ 追加: タイムラインの背景色
    pub body: Vec<WithSpan<ViewNode>>,
    pub whens: Vec<When>,
}

#[derive(Debug, Clone)]
pub struct Component {
    pub name: String,
    pub params: Vec<ComponentParam>, // ★ 変更: String から ComponentParam へ
    pub font: Option<String>,        // ★ 追加: コンポーネント全体で使用するフォント
    pub default_style: Option<Style>, // ★ 追加: デフォルトスタイル
    pub body: Vec<WithSpan<ViewNode>>,
    pub whens: Vec<When>,
}

/// コンポーネントパラメータ定義（Phase 2対応）
#[derive(Debug, Clone)]
pub struct ComponentParam {
    pub name: String,
    pub param_type: ComponentParamType,
    pub default_value: Option<Expr>,
    pub optional: bool,
}

/// コンポーネントパラメータの型定義
#[derive(Debug, Clone)]
pub enum ComponentParamType {
    String,
    Number,
    Bool,
    Object,
    Array,
    Function,
    Enum(Vec<String>), // 列挙型: ("small" | "medium" | "large")
    Any,
}

/// コンポーネント呼び出し時の引数（位置引数または名前付き引数）
#[derive(Debug, Clone)]
pub enum ComponentArg {
    Positional(Expr),    // 位置引数: Card("value")
    Named(String, Expr), // 名前付き引数: Card(content: "value")
}

#[derive(Debug, Clone)]
pub struct When {
    pub event: EventExpr,
    pub actions: Vec<WithSpan<ViewNode>>,
}

// ========================================
// イベント式
// ========================================

#[derive(Debug, Clone)]
pub enum EventExpr {
    ButtonPressed(String),

    // ★ 新規追加: テキスト入力関連のイベント
    TextChanged(String),            // テキスト入力フィールドの値が変更された
    TextFocused(String),            // テキスト入力フィールドがフォーカスされた
    TextBlurred(String),            // テキスト入力フィールドがフォーカスを失った
    KeyPressed(String, String),     // キーが押された (field_id, key_name)
    ImeComposition(String, String), // IME変換中のテキスト (field_id, composition_text)
    ImeCommit(String, String),      // IME変換確定 (field_id, committed_text)
}

// ========================================
// スパン情報付きノード
// ========================================

#[derive(Debug, Clone)]
pub struct WithSpan<T> {
    pub node: T,
    pub line: usize,
    pub column: usize,
    pub style: Option<Style>,
}

// ========================================
// ビューノード（制御構造を含む）
// ========================================

#[derive(Debug, Clone)]
pub enum ViewNode {
    // 基本UI要素
    Text {
        format: String,
        args: Vec<Expr>,
    },
    Button {
        id: String,
        label: String,
        onclick: Option<Expr>,
    },
    Image {
        path: String,
    },

    // ★ 新規追加: テキスト入力フィールド（IME対応）
    TextInput {
        id: String,                  // 一意識別子
        placeholder: Option<String>, // プレースホルダーテキスト
        value: Option<Expr>,         // 現在の値（state.field_nameなど）
        on_change: Option<Expr>,     // 値変更時のコールバック
        multiline: bool,             // 複数行入力対応
        max_length: Option<usize>,   // 最大文字数
        ime_enabled: bool,           // IME機能の有効/無効
    },

    // レイアウト要素
    VStack(Vec<WithSpan<ViewNode>>),
    HStack(Vec<WithSpan<ViewNode>>),

    // スペーシング
    Spacing(DimensionValue),
    SpacingAuto,

    // コンポーネント
    ComponentCall {
        name: String,
        args: Vec<ComponentArg>,
        slots: std::collections::HashMap<String, Vec<WithSpan<ViewNode>>>, // ★ Phase 2: スロットコンテンツ
    },

    // ★ Phase 2: スロットシステム
    Slot {
        name: String,
    }, // スロット挿入位置 (slot content)
    SlotCheck {
        name: String,
    }, // スロットの存在チェック (has_slot(footer))

    // 動的セクション
    DynamicSection {
        name: String,
        body: Vec<WithSpan<ViewNode>>,
    },

    // 制御構造
    Match {
        expr: Expr,
        arms: Vec<(Expr, Vec<WithSpan<ViewNode>>)>,
        default: Option<Vec<WithSpan<ViewNode>>>,
    },

    // ★ 新規追加: foreach制御
    ForEach {
        var: String,    // 繰り返し変数名 (e.g., "item")
        iterable: Expr, // 繰り返し対象 (e.g., "state.items")
        body: Vec<WithSpan<ViewNode>>,
    },

    // ★ 新規追加: if制御
    If {
        condition: Expr,                            // 条件式
        then_body: Vec<WithSpan<ViewNode>>,         // trueの場合の内容
        else_body: Option<Vec<WithSpan<ViewNode>>>, // falseの場合の内容（オプション）
    },

    // アクション
    NavigateTo {
        target: String,
    },
    RustCall {
        name: String,
        args: Vec<Expr>,
    },

    // 状態操作
    Set {
        path: String,
        value: Expr,
        inferred_type: Option<NiloType>,
    },
    Toggle {
        path: String,
    },
    ListAppend {
        path: String,
        value: Expr,
    },
    ListInsert {
        path: String,
        index: usize,
        value: Expr,
    },
    ListRemove {
        path: String,
        value: Expr,
    },
    ListClear {
        path: String,
    },

    // ★ 新規追加: ローカル変数宣言
    LetDecl {
        name: String,
        value: Expr,
        mutable: bool,
        declared_type: Option<NiloType>, // 明示的な型注釈
    },

    // イベントハンドラー
    When {
        event: EventExpr,
        actions: Vec<WithSpan<ViewNode>>,
    },

    Stencil(crate::stencil::stencil::Stencil),
}

// ========================================
// 式
// ========================================

#[derive(Debug, Clone)]
pub enum Expr {
    String(String),
    Number(f32),
    Bool(bool),
    Ident(String),
    Path(String),
    Array(Vec<Expr>),
    Object(Vec<(String, Expr)>),
    Dimension(DimensionValue),
    CalcExpr(Box<Expr>), // 計算式（括弧付き）
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
        default: Option<Box<Expr>>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone)]
pub enum BinaryOperator {
    Add,
    Sub,
    Mul,
    Div,
    // 比較演算子
    Eq, // ==
    Ne, // !=
    Lt, // <
    Le, // <=
    Gt, // >
    Ge, // >=
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Expr,
    pub value: Expr,
}

// ========================================
// 値型（単位付き数値）
// ========================================

#[derive(Debug, Clone, Copy)]
pub struct DimensionValue {
    pub value: f32,
    pub unit: Unit,
}

impl DimensionValue {
    pub fn px(value: f32) -> Self {
        Self {
            value,
            unit: Unit::Px,
        }
    }

    /// ビューポートサイズを考慮して実際のピクセル値を計算
    pub fn to_pixels(&self, viewport_w: f32, viewport_h: f32) -> f32 {
        match self.unit {
            Unit::Px => self.value,
            Unit::Vw => {
                let calculated = self.value * viewport_w / 100.0;
                debug!(
                    "🔍 VW DEBUG: {}vw with viewport_w:{} = {}px",
                    self.value, viewport_w, calculated
                );
                calculated
            }
            Unit::Vh => {
                let calculated = self.value * viewport_h / 100.0;
                debug!(
                    "🔍 VH DEBUG: {}vh with viewport_h:{} = {}px",
                    self.value, viewport_h, calculated
                );
                calculated
            }
            Unit::Ww => {
                let calculated = self.value * viewport_w / 100.0;
                debug!(
                    "🔍 WW DEBUG: {}ww with viewport_w:{} = {}px",
                    self.value, viewport_w, calculated
                );
                calculated
            }
            Unit::Wh => {
                let calculated = self.value * viewport_h / 100.0;
                debug!(
                    "🔍 WH DEBUG: {}wh with viewport_h:{} = {}px",
                    self.value, viewport_h, calculated
                );
                calculated
            }
            Unit::Percent => self.value / 100.0, // 通常は親要素のサイズに対する割合
            Unit::PercentHeight => self.value / 100.0, // PercentHeightケースを追加
            Unit::Rem => self.value * 16.0,      // 仮のroot font-size
            Unit::Em => self.value * 16.0,       // 仮のcurrent font-size
            Unit::Auto => viewport_w,            // Autoの場合はビューポート幅をデフォルトとする
        }
    }

    /// より詳細なピクセル変換（親要素サイズやフォントサイズを考慮）
    pub fn to_px(
        &self,
        viewport_w: f32,
        viewport_h: f32,
        parent_w: f32,
        parent_h: f32,
        font_size: f32,
        root_font_size: f32,
    ) -> f32 {
        match self.unit {
            Unit::Px => self.value,
            Unit::Vw => self.value * viewport_w / 100.0,
            Unit::Vh => self.value * viewport_h / 100.0,
            Unit::Ww => self.value * viewport_w / 100.0, // 画面幅基準
            Unit::Wh => self.value * viewport_h / 100.0, // 画面高さ基準
            Unit::Percent => self.value * parent_w / 100.0,
            Unit::PercentHeight => self.value * parent_h / 100.0,
            Unit::Rem => self.value * root_font_size,
            Unit::Em => self.value * font_size,
            Unit::Auto => parent_w, // Autoの場合は親要素の幅を使用
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unit {
    Px,
    Vw,
    Vh,
    Ww, // 画面幅基準の単位
    Wh, // 画面高さ基準の単位
    Percent,
    PercentHeight,
    Rem,
    Em,
    Auto, // 親要素のサイズを自動取得
}

// ========================================
// スタイル関連
// ========================================

#[derive(Debug, Clone, Default)]
pub struct Style {
    pub color: Option<ColorValue>,
    pub background: Option<ColorValue>,
    pub border_color: Option<ColorValue>,
    pub font_size: Option<f32>,
    pub relative_font_size: Option<DimensionValue>,
    pub font: Option<String>,
    pub padding: Option<Edges>,
    pub relative_padding: Option<RelativeEdges>,
    pub margin: Option<Edges>,
    pub relative_margin: Option<RelativeEdges>,
    pub size: Option<[f32; 2]>,
    pub relative_size: Option<[DimensionValue; 2]>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub relative_width: Option<DimensionValue>,
    pub relative_height: Option<DimensionValue>,

    // ★ 新規追加: 計算式を保持するフィールド
    pub width_expr: Option<Expr>,
    pub height_expr: Option<Expr>,

    pub align: Option<Align>,
    pub rounded: Option<Rounded>,
    pub shadow: Option<Shadow>,
    pub card: Option<bool>,
    pub spacing: Option<f32>,
    pub relative_spacing: Option<DimensionValue>,
    pub hover: Option<Box<Style>>,

    // ★ 新規追加: レイアウト関連プロパティ
    pub justify_content: Option<String>,
    pub align_items: Option<String>,
    pub position: Option<String>,
    pub top: Option<DimensionValue>,
    pub left: Option<DimensionValue>,
    pub right: Option<DimensionValue>,
    pub bottom: Option<DimensionValue>,
    pub z_index: Option<i32>,
    pub flex_wrap: Option<String>,
    pub gap: Option<DimensionValue>,
    pub max_width: Option<DimensionValue>,
    pub min_width: Option<DimensionValue>,
    pub max_height: Option<DimensionValue>,
    pub min_height: Option<DimensionValue>,
    pub margin_top: Option<DimensionValue>,
    pub margin_bottom: Option<DimensionValue>,
    pub margin_left: Option<DimensionValue>,
    pub margin_right: Option<DimensionValue>,
    pub line_height: Option<f32>,
    pub text_align: Option<String>,
    pub font_weight: Option<String>,
    pub font_family: Option<String>,
    pub backdrop_filter: Option<String>,
    pub border: Option<String>,

    // ★ テキスト折り返し制御
    pub wrap: Option<WrapMode>,

    // ★ スクロール制御
    pub overflow: Option<OverflowMode>,

    // ★ レスポンシブ対応: 条件付きスタイル
    pub responsive_rules: Vec<ResponsiveRule>,
}

/// テキスト折り返しモード
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WrapMode {
    Auto, // 自動折り返し（デフォルト）
    None, // 折り返ししない
}

/// オーバーフローモード（スクロール制御）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverflowMode {
    Visible, // 内容が溢れても表示（デフォルト）
    Hidden,  // 溢れた部分を非表示
    Scroll,  // スクロール可能
    Auto,    // 必要に応じてスクロール
}

/// レスポンシブデザイン用の条件付きスタイル
#[derive(Debug, Clone)]
pub struct ResponsiveRule {
    pub condition: Expr, // 例: window.width <= 1000
    pub style: Box<Style>,
}

impl Style {
    pub fn merged(&self, other: &Style) -> Style {
        let mut result = self.clone();
        if other.color.is_some() {
            result.color = other.color.clone();
        }
        if other.background.is_some() {
            result.background = other.background.clone();
        }
        if other.border_color.is_some() {
            result.border_color = other.border_color.clone();
        }
        if other.font_size.is_some() {
            result.font_size = other.font_size;
        }
        if other.relative_font_size.is_some() {
            result.relative_font_size = other.relative_font_size;
        }
        if other.font.is_some() {
            result.font = other.font.clone();
        }
        if other.padding.is_some() {
            result.padding = other.padding;
        }
        if other.relative_padding.is_some() {
            result.relative_padding = other.relative_padding.clone();
        }
        if other.margin.is_some() {
            result.margin = other.margin;
        }
        if other.relative_margin.is_some() {
            result.relative_margin = other.relative_margin.clone();
        }
        if other.size.is_some() {
            result.size = other.size;
        }
        if other.relative_size.is_some() {
            result.relative_size = other.relative_size;
        }
        if other.width.is_some() {
            result.width = other.width;
        }
        if other.height.is_some() {
            result.height = other.height;
        }
        if other.relative_width.is_some() {
            result.relative_width = other.relative_width;
        }
        if other.relative_height.is_some() {
            result.relative_height = other.relative_height;
        }
        if other.width_expr.is_some() {
            result.width_expr = other.width_expr.clone();
        }
        if other.height_expr.is_some() {
            result.height_expr = other.height_expr.clone();
        }
        if other.align.is_some() {
            result.align = other.align;
        }
        if other.rounded.is_some() {
            result.rounded = other.rounded;
        }
        if other.shadow.is_some() {
            result.shadow = other.shadow.clone();
        }
        if other.card.is_some() {
            result.card = other.card;
        }
        if other.spacing.is_some() {
            result.spacing = other.spacing;
        }
        if other.relative_spacing.is_some() {
            result.relative_spacing = other.relative_spacing;
        }
        if other.hover.is_some() {
            result.hover = other.hover.clone();
        }

        // ★ 新規追加: 新しいプロパティのマージ
        if other.justify_content.is_some() {
            result.justify_content = other.justify_content.clone();
        }
        if other.align_items.is_some() {
            result.align_items = other.align_items.clone();
        }
        if other.position.is_some() {
            result.position = other.position.clone();
        }
        if other.top.is_some() {
            result.top = other.top.clone();
        }
        if other.left.is_some() {
            result.left = other.left.clone();
        }
        if other.right.is_some() {
            result.right = other.right.clone();
        }
        if other.bottom.is_some() {
            result.bottom = other.bottom.clone();
        }
        if other.z_index.is_some() {
            result.z_index = other.z_index;
        }
        if other.flex_wrap.is_some() {
            result.flex_wrap = other.flex_wrap.clone();
        }
        if other.gap.is_some() {
            result.gap = other.gap.clone();
        }
        if other.max_width.is_some() {
            result.max_width = other.max_width.clone();
        }
        if other.min_width.is_some() {
            result.min_width = other.min_width.clone();
        }
        if other.max_height.is_some() {
            result.max_height = other.max_height.clone();
        }
        if other.min_height.is_some() {
            result.min_height = other.min_height.clone();
        }
        if other.margin_top.is_some() {
            result.margin_top = other.margin_top.clone();
        }
        if other.margin_bottom.is_some() {
            result.margin_bottom = other.margin_bottom.clone();
        }
        if other.margin_left.is_some() {
            result.margin_left = other.margin_left.clone();
        }
        if other.margin_right.is_some() {
            result.margin_right = other.margin_right.clone();
        }
        if other.line_height.is_some() {
            result.line_height = other.line_height;
        }
        if other.text_align.is_some() {
            result.text_align = other.text_align.clone();
        }
        if other.font_weight.is_some() {
            result.font_weight = other.font_weight.clone();
        }
        if other.font_family.is_some() {
            result.font_family = other.font_family.clone();
        }
        if other.backdrop_filter.is_some() {
            result.backdrop_filter = other.backdrop_filter.clone();
        }
        if other.border.is_some() {
            result.border = other.border.clone();
        }
        if other.wrap.is_some() {
            result.wrap = other.wrap;
        }
        if other.overflow.is_some() {
            result.overflow = other.overflow;
        }

        // ★ レスポンシブルール: 他のルールを優先的にマージ
        if !other.responsive_rules.is_empty() {
            result.responsive_rules = other.responsive_rules.clone();
        }

        result
    }
}

#[derive(Debug, Clone)]
pub enum ColorValue {
    Rgba([f32; 4]),
    Hex(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy)]
pub enum Rounded {
    On,
    Px(f32),
}

#[derive(Debug, Clone)]
pub enum Shadow {
    On,
    Spec {
        blur: f32,
        offset: [f32; 2],
        color: Option<ColorValue>,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn vh(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RelativeEdges {
    pub top: Option<DimensionValue>,
    pub right: Option<DimensionValue>,
    pub bottom: Option<DimensionValue>,
    pub left: Option<DimensionValue>,
}

impl RelativeEdges {
    pub fn all(value: DimensionValue) -> Self {
        Self {
            top: Some(value),
            right: Some(value),
            bottom: Some(value),
            left: Some(value),
        }
    }

    pub fn vh(vertical: DimensionValue, horizontal: DimensionValue) -> Self {
        Self {
            top: Some(vertical),
            right: Some(horizontal),
            bottom: Some(vertical),
            left: Some(horizontal),
        }
    }

    pub fn to_edges(
        &self,
        viewport_w: f32,
        viewport_h: f32,
        parent_w: f32,
        parent_h: f32,
        font_size: f32,
        root_font_size: f32,
    ) -> Edges {
        Edges {
            top: self
                .top
                .map(|d| {
                    d.to_px(
                        viewport_w,
                        viewport_h,
                        parent_w,
                        parent_h,
                        font_size,
                        root_font_size,
                    )
                })
                .unwrap_or(0.0),
            right: self
                .right
                .map(|d| {
                    d.to_px(
                        viewport_w,
                        viewport_h,
                        parent_w,
                        parent_h,
                        font_size,
                        root_font_size,
                    )
                })
                .unwrap_or(0.0),
            bottom: self
                .bottom
                .map(|d| {
                    d.to_px(
                        viewport_w,
                        viewport_h,
                        parent_w,
                        parent_h,
                        font_size,
                        root_font_size,
                    )
                })
                .unwrap_or(0.0),
            left: self
                .left
                .map(|d| {
                    d.to_px(
                        viewport_w,
                        viewport_h,
                        parent_w,
                        parent_h,
                        font_size,
                        root_font_size,
                    )
                })
                .unwrap_or(0.0),
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            flow: Flow {
                start: "Default".to_string(),
                start_url: None,
                transitions: vec![],
            },
            timelines: vec![],
            components: vec![],
        }
    }
}

// ========================================
// 型システム
// ========================================

/// Nilo型システムの型定義
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NiloType {
    // プリミティブ型
    Number,
    String,
    Bool,

    // コレクション型
    Array(Box<NiloType>),

    // 特殊型
    Any,     // 任意の型（型チェックをスキップ）
    Unknown, // 未解決の型（推論できない）
}

impl NiloType {
    /// 型の表示名を取得
    pub fn display(&self) -> String {
        match self {
            NiloType::Number => "number".to_string(),
            NiloType::String => "string".to_string(),
            NiloType::Bool => "bool".to_string(),
            NiloType::Array(inner) => format!("{}[]", inner.display()),
            NiloType::Any => "any".to_string(),
            NiloType::Unknown => "unknown".to_string(),
        }
    }

    /// 型の互換性チェック（弱い型付けの特徴）
    pub fn is_compatible_with(&self, other: &NiloType) -> bool {
        match (self, other) {
            // Any型はすべてと互換
            (NiloType::Any, _) | (_, NiloType::Any) => true,

            // Unknown型は解決待ち
            (NiloType::Unknown, _) | (_, NiloType::Unknown) => true,

            // 配列の型チェック
            (NiloType::Array(a), NiloType::Array(b)) => a.is_compatible_with(b),

            // 同じ型
            (a, b) => a == b,
        }
    }
}

/// 型付き式（パーサーで生成）
#[derive(Debug, Clone)]
pub struct TypedExpr {
    pub expr: Expr,
    pub inferred_type: NiloType,
}

impl TypedExpr {
    pub fn new(expr: Expr, inferred_type: NiloType) -> Self {
        Self {
            expr,
            inferred_type,
        }
    }

    /// 型なし式として扱う
    pub fn into_expr(self) -> Expr {
        self.expr
    }
}
