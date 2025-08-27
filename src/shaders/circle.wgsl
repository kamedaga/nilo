struct ScreenUniform {
    screen_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> screen: ScreenUniform;

struct VertexInput {
    @location(0) position: vec3<f32>, // ★ Z座標追加
    @location(1) center: vec2<f32>,
    @location(2) radius: f32,
    @location(3) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) pixel_pos: vec2<f32>,
    @location(1) center: vec2<f32>,
    @location(2) radius: f32,
    @location(3) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4(input.position, 1.0); // ★ XYZWの形で出力

    // NDC座標をピクセル座標に変換
    out.pixel_pos = vec2(
        (input.position.x + 1.0) * 0.5 * screen.screen_size.x,
        (1.0 - input.position.y) * 0.5 * screen.screen_size.y
    );

    out.center = input.center;
    out.radius = input.radius;
    out.color = input.color;
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
    let dist = distance(input.pixel_pos, input.center);
    let aa = 1.0 - smoothstep(input.radius - 1.0, input.radius + 1.0, dist);

    // アルファが0の場合は透明
    if aa <= 0.0 {
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