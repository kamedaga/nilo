// 統合シェーダー - すべての図形タイプを一つのパイプラインで描画

struct ScreenUniform {
    screen_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> screen: ScreenUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,     // XYZ座標
    @location(1) shape_type: u32,         // 0=Quad, 1=Triangle, 2=Circle
    @location(2) color: u32,              // パックされたRGBA8 (Quad/Triangle用)
    // location 3はスキップ（パディング用）
    @location(4) center: vec2<f32>,       // Circle用: 中心座標
    @location(5) radius: f32,             // Circle用: 半径
    // location 6はスキップ（パディング用）
    @location(7) color_vec: vec4<f32>,    // Circle用: カラーベクター
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) shape_type: u32,
    @location(2) pixel_pos: vec2<f32>,         // Circle用: 各頂点のピクセル座標（補間される）
    @location(3) @interpolate(flat) center: vec2<f32>,  // Circle用: 円の中心（補間しない）
    @location(4) @interpolate(flat) radius: f32,        // Circle用: 半径（補間しない）
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4(input.position, 1.0);
    out.shape_type = input.shape_type;
    
    // デフォルト値で初期化
    out.color = vec4(0.0, 0.0, 0.0, 1.0);
    out.pixel_pos = vec2(0.0, 0.0);
    out.center = vec2(0.0, 0.0);
    out.radius = 0.0;
    
    // Quad/Triangle の場合: パックされたカラーをデコード
    if input.shape_type == 0u || input.shape_type == 1u {
        out.color = vec4(
            f32((input.color & 0x00ff0000u) >> 16u) / 255.0,
            f32((input.color & 0x0000ff00u) >> 8u) / 255.0,
            f32(input.color & 0x000000ffu) / 255.0,
            f32((input.color & 0xff000000u) >> 24u) / 255.0,
        );
    }
    // Circle の場合: 元のCircleRendererと同じようにNDC→ピクセル変換
    else if input.shape_type == 2u {
        out.color = input.color_vec;
        // NDC座標をピクセル座標に変換（補間される）
        out.pixel_pos = vec2(
            (input.position.x + 1.0) * 0.5 * screen.screen_size.x,
            (1.0 - input.position.y) * 0.5 * screen.screen_size.y
        );
        out.center = input.center;
        out.radius = input.radius;
    }
    
    return out;
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        return c / 12.92;
    }
    return pow((c + 0.055) / 1.055, 2.4);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Circle の場合: 距離ベースのアンチエイリアシング
    if input.shape_type == 2u {
        let dist = distance(input.pixel_pos, input.center);
        let aa = 1.0 - smoothstep(input.radius - 1.0, input.radius + 1.0, dist);

        // アルファが0以下の場合は透明（円の外側を完全に破棄）
        if aa < 1.0 {
            discard;
        }

        
        // sRGB→linear補正
        let linear_rgb = vec3(
            srgb_to_linear(input.color.r),
            srgb_to_linear(input.color.g),
            srgb_to_linear(input.color.b),
        );

        return vec4(linear_rgb, input.color.a * aa);
    }
    
    // Quad/Triangleの場合: sRGB → Linear変換
    let linear = vec3(
        srgb_to_linear(input.color.r),
        srgb_to_linear(input.color.g),
        srgb_to_linear(input.color.b),
    );
    
    return vec4(linear, input.color.a);
}
