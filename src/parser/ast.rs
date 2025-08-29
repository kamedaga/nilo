// ========================================
// Nilo言語 AST定義
// ========================================

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
    pub transitions: Vec<(String, Vec<String>)>,
}

// ========================================
// 階層的フロー糖衣構文用の型定義
// ========================================

/// 階層的フロー定義の中間表現
#[derive(Debug, Clone)]
pub struct NamespacedFlow {
    pub name: String,
    pub start: String,
    pub transitions: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Clone)]
pub struct Timeline {
    pub name: String,
    pub font: Option<String>,  // ★ 追加: タイムライン全体で使用するフォント
    pub body: Vec<WithSpan<ViewNode>>,
    pub whens: Vec<When>,
}

#[derive(Debug, Clone)]
pub struct Component {
    pub name: String,
    pub params: Vec<String>,
    pub font: Option<String>,  // ★ 追加: コンポーネント全体で使用するフォント
    pub body: Vec<WithSpan<ViewNode>>,
    pub whens: Vec<When>,
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
    TextChanged(String),          // テキスト入力フィールドの値が変更された
    TextFocused(String),          // テキスト入力フィールドがフォーカスされた
    TextBlurred(String),          // テキスト入力フィールドがフォーカスを失った
    KeyPressed(String, String),   // キーが押された (field_id, key_name)
    ImeComposition(String, String), // IME変換中のテキスト (field_id, composition_text)
    ImeCommit(String, String),    // IME変換確定 (field_id, committed_text)
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
    Text { format: String, args: Vec<Expr> },
    Button { id: String, label: String, onclick: Option<Expr> },
    Image { path: String },
    
    // ★ 新規追加: テキスト入力フィールド（IME対応）
    TextInput { 
        id: String,                    // 一意識別子
        placeholder: Option<String>,   // プレースホルダーテキスト
        value: Option<Expr>,          // 現在の値（state.field_nameなど）
        on_change: Option<Expr>,      // 値変更時のコールバック
        multiline: bool,              // 複数行入力対応
        max_length: Option<usize>,    // 最大文字数
        ime_enabled: bool,            // IME機能の有効/無効
    },
    
    // レイアウト要素
    VStack(Vec<WithSpan<ViewNode>>),
    HStack(Vec<WithSpan<ViewNode>>),
    
    // スペーシング
    Spacing(f32),
    SpacingAuto,
    
    // コンポーネント
    ComponentCall { name: String, args: Vec<Expr> },
    
    // 動的セクション
    DynamicSection { name: String, body: Vec<WithSpan<ViewNode>> },
    
    // 制御構造
    Match { expr: Expr, arms: Vec<(Expr, Vec<WithSpan<ViewNode>>)>, default: Option<Vec<WithSpan<ViewNode>>> },
    
    // ★ 新規追加: foreach制御
    ForEach { 
        var: String,           // 繰り返し変数名 (e.g., "item")
        iterable: Expr,        // 繰り返し対象 (e.g., "state.items")
        body: Vec<WithSpan<ViewNode>>,
    },
    
    // ★ 新規追加: if制御
    If { 
        condition: Expr,       // 条件式
        then_body: Vec<WithSpan<ViewNode>>,  // trueの場合の内容
        else_body: Option<Vec<WithSpan<ViewNode>>>,  // falseの場合の内容（オプション）
    },
    
    // アクション
    NavigateTo { target: String },
    RustCall { name: String, args: Vec<Expr> },
    
    // 状態操作
    Set { path: String, value: Expr },
    Toggle { path: String },
    ListAppend { path: String, value: Expr },
    ListRemove { path: String, index: usize },
    
    // イベントハンドラー
    When { event: EventExpr, actions: Vec<WithSpan<ViewNode>> },

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
    Match { expr: Box<Expr>, arms: Vec<MatchArm>, default: Option<Box<Expr>> },
    FunctionCall { name: String, args: Vec<Expr> },
    BinaryOp { left: Box<Expr>, op: BinaryOperator, right: Box<Expr> },
}

#[derive(Debug, Clone)]
pub enum BinaryOperator {
    Add,
    Sub,
    Mul,
    Div,
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
        Self { value, unit: Unit::Px }
    }
    
    pub fn to_px(&self, viewport_w: f32, viewport_h: f32, _parent_w: f32, _parent_h: f32, _root_font_size: f32, _font_size: f32) -> f32 {
        let result = match self.unit {
            Unit::Px => self.value,
            Unit::Vw => {
                let calculated = (self.value / 100.0) * viewport_w;
                println!("🔍 VW DEBUG: {}vw with viewport_w:{} = {}px", self.value, viewport_w, calculated);
                calculated
            },
            Unit::Vh => {
                let calculated = (self.value / 100.0) * viewport_h;
                println!("🔍 VH DEBUG: {}vh with viewport_h:{} = {}px", self.value, viewport_h, calculated);
                calculated
            },
            Unit::Percent => self.value, // 実装は親要素に依存
            Unit::PercentHeight => self.value,
            Unit::Rem => self.value * _root_font_size, // ルートフォントサイズを使用
            Unit::Em => self.value * _font_size,  // 現在のフォントサイズを使用
        };
        result
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Unit {
    Px,
    Vw,
    Vh,
    Percent,
    PercentHeight,
    Rem,
    Em,
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
    pub align: Option<Align>,
    pub rounded: Option<Rounded>,
    pub shadow: Option<Shadow>,
    pub card: Option<bool>,
    pub spacing: Option<f32>,
    pub relative_spacing: Option<DimensionValue>,
    pub hover: Option<Box<Style>>,
}

impl Style {
    pub fn merged(&self, other: &Style) -> Style {
        let mut result = self.clone();
        if other.color.is_some() { result.color = other.color.clone(); }
        if other.background.is_some() { result.background = other.background.clone(); }
        if other.border_color.is_some() { result.border_color = other.border_color.clone(); }
        if other.font_size.is_some() { result.font_size = other.font_size; }
        if other.relative_font_size.is_some() { result.relative_font_size = other.relative_font_size; }
        if other.font.is_some() { result.font = other.font.clone(); }
        if other.padding.is_some() { result.padding = other.padding; }
        if other.relative_padding.is_some() { result.relative_padding = other.relative_padding.clone(); }
        if other.margin.is_some() { result.margin = other.margin; }
        if other.relative_margin.is_some() { result.relative_margin = other.relative_margin.clone(); }
        if other.size.is_some() { result.size = other.size; }
        if other.relative_size.is_some() { result.relative_size = other.relative_size; }
        if other.width.is_some() { result.width = other.width; }
        if other.height.is_some() { result.height = other.height; }
        if other.relative_width.is_some() { result.relative_width = other.relative_width; }
        if other.relative_height.is_some() { result.relative_height = other.relative_height; }
        if other.align.is_some() { result.align = other.align; }
        if other.rounded.is_some() { result.rounded = other.rounded; }
        if other.shadow.is_some() { result.shadow = other.shadow.clone(); }
        if other.card.is_some() { result.card = other.card; }
        if other.spacing.is_some() { result.spacing = other.spacing; }
        if other.relative_spacing.is_some() { result.relative_spacing = other.relative_spacing; }
        if other.hover.is_some() { result.hover = other.hover.clone(); }
        result
    }
}

#[derive(Debug, Clone)]
pub enum ColorValue {
    Rgba([f32; 4]),
    Hex(String),
}

#[derive(Debug, Clone, Copy)]
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
    Spec { blur: f32, offset: [f32; 2], color: Option<ColorValue> },
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
        Self { top: value, right: value, bottom: value, left: value }
    }
    
    pub fn vh(vertical: f32, horizontal: f32) -> Self {
        Self { top: vertical, right: horizontal, bottom: vertical, left: horizontal }
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
            top: self.top.map(|d| d.to_px(viewport_w, viewport_h, parent_w, parent_h, font_size, root_font_size)).unwrap_or(0.0),
            right: self.right.map(|d| d.to_px(viewport_w, viewport_h, parent_w, parent_h, font_size, root_font_size)).unwrap_or(0.0),
            bottom: self.bottom.map(|d| d.to_px(viewport_w, viewport_h, parent_w, parent_h, font_size, root_font_size)).unwrap_or(0.0),
            left: self.left.map(|d| d.to_px(viewport_w, viewport_h, parent_w, parent_h, font_size, root_font_size)).unwrap_or(0.0),
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            flow: Flow {
                start: "Default".to_string(),
                transitions: vec![],
            },
            timelines: vec![],
            components: vec![],
        }
    }
}
