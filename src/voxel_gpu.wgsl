struct Globals {
    center: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    view: vec4<f32>,
    wind: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var bark_tex: texture_2d<f32>;
@group(0) @binding(2) var foliage_tex: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) tint: vec4<f32>,
    @location(3) kind: f32,
    @location(4) sway_weight: f32,
    @location(5) phase: f32,
};

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
    @location(2) kind: f32,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var world = input.position;
    let phase = globals.wind.x * globals.wind.z * 3.0 + input.phase;
    let sway = (sin(phase) + 0.3 * cos(phase * 0.6))
        * globals.wind.y * globals.wind.w * input.sway_weight;
    world.x += sway;
    world.y += sway * 0.45;

    let relative = world - globals.center.xyz;
    var output: VertexOutput;
    output.clip = vec4<f32>(
        dot(relative, globals.right.xyz) * globals.right.w,
        dot(relative, globals.up.xyz) * globals.up.w,
        0.5 - dot(relative, globals.view.xyz) * globals.view.w,
        1.0,
    );
    output.uv = input.uv;
    output.tint = input.tint;
    output.kind = input.kind;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var rgba: vec4<f32>;
    if (input.kind < 0.5) {
        rgba = textureSampleLevel(bark_tex, linear_sampler, input.uv, 0.0);
    } else if (input.kind < 1.5) {
        rgba = textureSampleLevel(foliage_tex, linear_sampler, input.uv, 0.0);
        if (rgba.a < 0.5) { discard; }
    } else {
        rgba = vec4<f32>(1.0);
    }
    return vec4<f32>(rgba.rgb * input.tint.rgb, 1.0);
}
