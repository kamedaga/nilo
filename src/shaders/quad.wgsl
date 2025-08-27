struct VertexInput {
    @location(0) position: vec3<f32>, // Z座標追加
    @location(1) color: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4(input.position, 1.0); // XYZWの形で出力
    out.color = vec4(
        f32((input.color & 0x00ff0000u) >> 16u) / 255.0,
        f32((input.color & 0x0000ff00u) >> 8u) / 255.0,
        f32(input.color & 0x000000ffu) / 255.0,
        f32((input.color & 0xff000000u) >> 24u) / 255.0,
    );
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
    let srgb = input.color.rgb;
    let linears = vec3(
        srgb_to_linear(srgb.r),
        srgb_to_linear(srgb.g),
        srgb_to_linear(srgb.b),
    );
    return vec4(linears, input.color.a);
}