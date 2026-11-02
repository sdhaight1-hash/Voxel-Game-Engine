struct Globals {
    center: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    view: vec4<f32>,
    wind: vec4<f32>,
    season: vec4<f32>,
    leaf_base: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var bark_tex: texture_2d<f32>;
@group(0) @binding(2) var foliage_tex: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;

struct SeasonalPalette {
    // 4 seasons × 4 gradient stops, aligned to vec4 in uniform memory.
    stops: array<vec4<f32>,16>,
};
@group(0) @binding(4) var<uniform> palettes: SeasonalPalette;

// Sample one season's shadow-to-highlight gradient (UV base -> tip).
fn season_gradient(season:u32, amount:f32)->vec3<f32> {
    let x=clamp(amount,0.0,1.0)*3.0;
    let idx=min(u32(x),2u);
    let fraction=x-f32(idx);
    let offset=season*4u+idx;
    return mix(palettes.stops[offset].rgb,
               palettes.stops[offset+1u].rgb,fraction);
}

// Three clustered, five-petal blossoms projected on the existing cutout
// leaf cards. They require no extra textures or tree-mesh regeneration.
// The ornamental Cherry Blossom preset alone activates this overlay.
fn cherry_blossom_mask(uv:vec2<f32>)->f32 {
    let centers = array<vec2<f32>,3>(
        vec2<f32>(0.33,0.33),
        vec2<f32>(0.65,0.54),
        vec2<f32>(0.37,0.73),
    );
    var mask=0.0;
    for (var i=0u; i<3u; i=i+1u) {
        for (var j=0u; j<5u; j=j+1u) {
            let angle=f32(j)*1.25663706;
            let center=centers[i]+vec2<f32>(cos(angle),sin(angle))*0.042;
            let p=(uv-center)/vec2<f32>(0.052,0.045);
            mask=max(mask,1.0-smoothstep(0.83,1.0,length(p)));
        }
        mask=max(mask,1.0-smoothstep(0.012,0.023,length(uv-centers[i])));
    }
    return mask;
}

fn spring_bloom_amount()->f32 {
    if (globals.season.y<0.5) {return 0.0;}
    let progress=fract(globals.season.x/4.0)*4.0;
    // Bloom early in spring, then fall away well before summer.
    return 1.0-smoothstep(0.18,0.72,progress);
}

fn seasonal_foliage(uv_y:f32)->vec3<f32> {
    let progress=fract(globals.season.x/4.0)*4.0;
    let start=u32(floor(progress));
    let next=(start+1u)%4u;
    let blend=fract(progress);
    return mix(season_gradient(start,uv_y),
               season_gradient(next,uv_y),blend);
}

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
        let blossom=spring_bloom_amount()*cherry_blossom_mask(input.uv);
        // Petals can extend outside the permanent green-leaf silhouette.
        // When bloom fades, the original cutout remains unchanged.
        if (rgba.a < 0.5 && blossom < 0.55) { discard; }
        let base=max(globals.leaf_base.rgb,vec3<f32>(0.12));
        let leaf_color=min(rgba.rgb*seasonal_foliage(1.0-input.uv.y)/base,
            vec3<f32>(1.0));
        let petal_color=vec3<f32>(1.0,0.71,0.81);
        rgba=vec4<f32>(mix(leaf_color,petal_color,blossom),1.0);
    } else {
        rgba = vec4<f32>(1.0);
    }
    return vec4<f32>(rgba.rgb * input.tint.rgb, 1.0);
}
