struct VertexInput {
    @location(0) position: vec3<f32>, // ★ Z座標追加
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4(input.position, 1.0); // ★ XYZWの形で出力
    out.uv = input.uv;
    return out;
}

@group(0) @binding(0) var img: texture_2d<f32>;
@group(0) @binding(1) var img_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(img, img_sampler, in.uv);
}
