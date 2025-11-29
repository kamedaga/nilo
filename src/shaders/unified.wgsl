// Unified shader: draws rect/triangle/circle and a lightweight box shadow

struct ScreenUniform {
    screen_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> screen: ScreenUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,     // NDC position
    @location(1) shape_type: u32,         // 0=Quad, 1=Triangle, 2=Circle, 3=BoxShadow
    @location(2) color: u32,              // packed RGBA8 (Quad/Triangle)
    @location(3) blur: f32,               // shadow blur radius (px)
    @location(4) center: vec2<f32>,       // circle/shadow center in px
    @location(5) radius: f32,             // circle radius / corner radius (px)
    @location(6) half_size: vec2<f32>,    // shadow half size (px)
    @location(7) color_vec: vec4<f32>,    // unpacked color (Circle/Shadow)
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) shape_type: u32,
    @location(2) pixel_pos: vec2<f32>,               // pixel position for distance fields
    @location(3) @interpolate(flat) center: vec2<f32>,
    @location(4) @interpolate(flat) radius: f32,
    @location(5) @interpolate(flat) half_size: vec2<f32>,
    @location(6) @interpolate(flat) blur: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4(input.position, 1.0);
    out.shape_type = input.shape_type;
    out.color = vec4(0.0, 0.0, 0.0, 1.0);
    out.pixel_pos = vec2(0.0, 0.0);
    out.center = vec2(0.0, 0.0);
    out.radius = 0.0;
    out.half_size = vec2(0.0, 0.0);
    out.blur = 0.0;

    // Quad/Triangle: unpack packed color
    if input.shape_type == 0u || input.shape_type == 1u {
        out.color = vec4(
            f32((input.color & 0x00ff0000u) >> 16u) / 255.0,
            f32((input.color & 0x0000ff00u) >> 8u) / 255.0,
            f32(input.color & 0x000000ffu) / 255.0,
            f32((input.color & 0xff000000u) >> 24u) / 255.0,
        );
    }
    // Circle: convert NDC to pixel space
    else if input.shape_type == 2u {
        out.color = input.color_vec;
        out.pixel_pos = vec2(
            (input.position.x + 1.0) * 0.5 * screen.screen_size.x,
            (1.0 - input.position.y) * 0.5 * screen.screen_size.y
        );
        out.center = input.center;
        out.radius = input.radius;
    }
    // Box shadow: keep sRGB color and distance params
    else if input.shape_type == 3u {
        out.color = input.color_vec;
        out.pixel_pos = vec2(
            (input.position.x + 1.0) * 0.5 * screen.screen_size.x,
            (1.0 - input.position.y) * 0.5 * screen.screen_size.y
        );
        out.center = input.center;
        out.radius = input.radius;
        out.half_size = input.half_size;
        out.blur = input.blur;
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
    // Circle alpha
    if input.shape_type == 2u {
        // Half-pixel wide transition for smoother edges
        let dist = distance(input.pixel_pos, input.center);
        let alpha = 1.0 - smoothstep(input.radius - 1.5, input.radius + 1.5, dist);
        if alpha <= 0.001 {
            discard;
        }

        let linear_rgb = vec3(
            srgb_to_linear(input.color.r),
            srgb_to_linear(input.color.g),
            srgb_to_linear(input.color.b),
        );

        return vec4(linear_rgb, input.color.a * alpha);
    }

    // Box shadow: signed distance to rounded box
    if input.shape_type == 3u {
        let p = input.pixel_pos - input.center;
        let q = abs(p) - input.half_size + vec2(input.radius, input.radius);
        let dist = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - input.radius;
        if dist < 0.0 {
            discard;
        }

        let blur = max(input.blur, 0.5);
        let alpha = 1.0 - smoothstep(0.0, blur, dist);
        if alpha <= 0.001 {
            discard;
        }

        let linear_rgb = vec3(
            srgb_to_linear(input.color.r),
            srgb_to_linear(input.color.g),
            srgb_to_linear(input.color.b),
        );

        return vec4(linear_rgb, input.color.a * alpha);
    }

    // Quad/Triangle
    let linear = vec3(
        srgb_to_linear(input.color.r),
        srgb_to_linear(input.color.g),
        srgb_to_linear(input.color.b),
    );

    return vec4(linear, input.color.a);
}
