#[derive(Debug, Clone)] // ★ Cloneトレイトを追加
pub enum DrawCommand {
    //primitiveView
    Rect {
        position: [f32; 2],
        width: f32,
        height: f32,
        color: [f32; 4],
        scroll: bool,
        depth: f32, // ★ depth値を追加
    },
    Triangle {
        p1: [f32; 2],
        p2: [f32; 2],
        p3: [f32; 2],
        color: [f32; 4],
        scroll: bool,
        depth: f32, // ★ depth値を追加
    },
    Circle {
        center: [f32; 2],
        radius: f32,
        color: [f32; 4],
        segments: usize, // 分割数（例: 32など）
        scroll: bool,
        depth: f32, // ★ depth値を追加
    },
    Text {
        content: String,
        position: [f32; 2],
        size: f32,
        color: [f32; 4],
        font: String,           // フォント名 or ID
        max_width: Option<f32>, // ★ max_width制約を追加
        scroll: bool,
        depth: f32, // ★ depth値を追加
    },
    Image {
        position: [f32; 2],
        width: f32,
        height: f32,
        path: String,
        scroll: bool,
        depth: f32, // ★ depth値を追加
    },
    /// スクロールコンテナ（クリッピング領域を定義）
    ScrollContainer {
        id: String,  // ★ ScrollContainerの一意なID
        position: [f32; 2],
        width: f32,
        height: f32,
        children: Vec<DrawCommand>,
        scroll_offset: [f32; 2],  // ScrollContainer専用のローカルスクロール
        depth: f32,
    },
}

#[derive(Debug)]
pub struct DrawList(pub Vec<DrawCommand>);

impl DrawList {
    pub fn new() -> Self {
        Self(Vec::new())
    }
    pub fn push(&mut self, cmd: DrawCommand) {
        self.0.push(cmd);
    }
    
    /// ScrollContainerのscroll_offsetをAppStateから更新する
    pub fn update_scroll_offsets(&mut self, scroll_offsets: &std::collections::HashMap<String, [f32; 2]>) {
        for cmd in &mut self.0 {
            update_command_scroll_offset(cmd, scroll_offsets);
        }
    }
    
    pub fn content_length(&self) -> f32 {
        self.0
            .iter()
            .map(|cmd| match cmd {
                DrawCommand::Rect {
                    position, height, ..
                } => position[1] + *height,
                DrawCommand::Triangle { p1, p2, p3, .. } => p1[1].max(p2[1]).max(p3[1]),
                DrawCommand::Circle { center, radius, .. } => center[1] + *radius,
                DrawCommand::Text { position, size, .. } => position[1] + *size,
                DrawCommand::Image {
                    position, height, ..
                } => position[1] + *height,
                DrawCommand::ScrollContainer {
                    position, height, ..
                } => position[1] + *height,
            })
            .fold(0.0, f32::max)
    }
}

/// DrawCommand内のScrollContainerのscroll_offsetを再帰的に更新
fn update_command_scroll_offset(
    cmd: &mut DrawCommand,
    scroll_offsets: &std::collections::HashMap<String, [f32; 2]>
) {
    match cmd {
        DrawCommand::ScrollContainer {
            id,
            scroll_offset,
            children,
            ..
        } => {
            // AppStateからscroll_offsetを取得して更新
            if let Some(offset) = scroll_offsets.get(id) {
                *scroll_offset = *offset;
            }
            
            // 子要素も再帰的に更新
            for child in children {
                update_command_scroll_offset(child, scroll_offsets);
            }
        }
        _ => {}
    }
}
