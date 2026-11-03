struct Globals {
    center: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    view: vec4<f32>,
    wind: vec4<f32>,
    season: vec4<f32>,
    leaf_base: vec4<f32>,
    // x = bark meters per repeat; y = metallic PBR, z = studio IBL.
    bark_mapping: vec4<f32>,
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
// Data textures are Rgba8Unorm, *not* sRGB. Roughness and normal values
// must not undergo the albedo color-space transformation.
@group(0) @binding(5) var bark_roughness_tex: texture_2d<f32>;
@group(0) @binding(6) var bark_normal_tex: texture_2d<f32>;
@group(0) @binding(7) var bark_metallic_tex: texture_2d<f32>;
// CPU-prefiltered linear-RGB studio environment, generated only when enabled.
@group(0) @binding(8) var studio_specular_tex: texture_2d<f32>;
@group(0) @binding(9) var studio_irradiance_tex: texture_2d<f32>;

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

// Small five-rayed, white Coffea flowers. This is a temporary material
// overlay on existing leaf cards, not independent flower geometry.
fn coffee_flower_mask(uv:vec2<f32>)->f32 {
    let centers=array<vec2<f32>,3>(
        vec2<f32>(0.39,0.38),
        vec2<f32>(0.59,0.57),
        vec2<f32>(0.40,0.72),
    );
    var mask=0.0;
    for (var i=0u;i<3u;i=i+1u) {
        for (var j=0u;j<5u;j=j+1u) {
            let angle=f32(j)*1.25663706;
            let axis=vec2<f32>(cos(angle),sin(angle));
            let center=centers[i]+axis*0.025;
            let p=uv-center;
            let along=dot(p,axis)/0.036;
            let across=dot(p,vec2<f32>(-axis.y,axis.x))/0.013;
            mask=max(mask,1.0-smoothstep(0.72,1.0,
                length(vec2<f32>(along,across))));
        }
        mask=max(mask,1.0-smoothstep(0.008,0.016,length(uv-centers[i])));
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
    @location(6) bark_circumference_m: f32,
    @location(7) normal: vec3<f32>,
    @location(8) tangent: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
    @location(2) kind: f32,
    @location(3) bark_circumference_m: f32,
    @location(4) normal: vec3<f32>,
    @location(5) tangent: vec4<f32>,
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
    output.bark_circumference_m = input.bark_circumference_m;
    output.normal = input.normal;
    output.tangent = input.tangent;
    return output;
}

// Experimental single-sample screen-door alpha coverage for leaf cards.
// A 4x4 Bayer threshold distributes sampled (linearly filtered) fractional
// alpha spatially. Screen-space coordinates prevent wind/time flicker caused
// by animated per-frame random thresholds. This is NOT MSAA or true alpha
// blending; the user can opt back into the legacy 0.5 alpha test.
fn leaf_coverage_threshold(pixel:vec2<u32>)->f32 {
    let bayer=array<f32,16>(
        0.0,8.0,2.0,10.0,
        12.0,4.0,14.0,6.0,
        3.0,11.0,1.0,9.0,
        15.0,7.0,13.0,5.0,
    );
    let index=(pixel.y & 3u)*4u+(pixel.x & 3u);
    return (bayer[index]+0.5)/16.0;
}

// Albedo RGBA8 is authored in display/sRGB space. The render target is
// Rgba8Unorm (not sRGB), so convert explicitly for dielectric lighting and
// encode again on output. Roughness and normal maps stay linear/UNORM.
fn srgb_to_linear(c:vec3<f32>)->vec3<f32> {
    let x=clamp(c,vec3<f32>(0.0),vec3<f32>(1.0));
    return select(x/12.92,pow((x+0.055)/1.055,vec3<f32>(2.4)),
                  x>vec3<f32>(0.04045));
}
fn linear_to_srgb(c:vec3<f32>)->vec3<f32> {
    let x=max(c,vec3<f32>(0.0));
    return select(12.92*x,1.055*pow(x,vec3<f32>(1.0/2.4))-0.055,
                  x>vec3<f32>(0.0031308));
}

// Lat-long reflection lookup: horizontal wrap, guarded vertical bands.
// Generated CPU atlas: width=48, height=24, band=26, levels=5, total=130.
fn environment_uv(dir:vec3<f32>)->vec2<f32> {
    let d=normalize(dir);
    let u=atan2(d.y,d.x)/(2.0*3.14159265)+0.5;
    let v=acos(clamp(d.z,-1.0,1.0))/3.14159265;
    return vec2<f32>(u,clamp(v,0.5/24.0,1.0-0.5/24.0));
}
fn studio_specular_band(dir:vec3<f32>,level:f32)->vec3<f32> {
    let uv=environment_uv(dir);
    // One padded row on each side prevents roughness bands bleeding.
    let y=(level*26.0+1.5+uv.y*23.0)/130.0;
    return textureSampleLevel(studio_specular_tex,linear_sampler,
                              vec2<f32>(uv.x,y),0.0).rgb;
}
fn studio_reflection(dir:vec3<f32>,roughness:f32)->vec3<f32> {
    let level=clamp(roughness,0.0,1.0)*4.0;
    let lower=floor(level);
    let upper=min(lower+1.0,4.0);
    let a=studio_specular_band(dir,lower);
    let b=studio_specular_band(dir,upper);
    return mix(a,b,level-lower);
}
// F0 scale/bias fit for split-sum environment BRDF (Karis/UE4).
// Approximates BRDF integration without a third runtime lookup texture.
fn studio_brdf_approx(NoV:f32,roughness:f32)->vec2<f32> {
    let c0=vec4<f32>(-1.0,-0.0275,-0.572,0.022);
    let c1=vec4<f32>(1.0,0.0425,1.04,-0.04);
    let r=roughness*c0+c1;
    let a004=min(r.x*r.x,exp2(-9.28*NoV))*r.x+r.y;
    return vec2<f32>(-1.04,1.04)*a004+r.zw;
}
fn studio_indirect(N:vec3<f32>,V:vec3<f32>,base:vec3<f32>,
                   F0:vec3<f32>,metal:f32,roughness:f32)->vec3<f32> {
    let NoV=clamp(dot(N,V),0.001,1.0);
    let R=reflect(-V,N);
    let uv=environment_uv(N);
    let irradiance=textureSampleLevel(
        studio_irradiance_tex,linear_sampler,uv,0.0).rgb;
    let kS=F0+(vec3<f32>(1.0)-F0)*pow(1.0-NoV,5.0);
    let kD=(vec3<f32>(1.0)-kS)*(1.0-metal);
    let diffuse=irradiance*base*kD;
    let prefiltered=studio_reflection(R,roughness);
    let ab=studio_brdf_approx(NoV,roughness);
    let specular=prefiltered*(F0*ab.x+vec3<f32>(ab.y));
    return diffuse+specular;
}

// Metallic-roughness microfacet BRDF with GGX/Smith/Schlick. Conductor
// reflectance uses *linear* base color; dielectric F0 is 0.04. This is
// one fixed local preview light plus diffuse ambient, not environment IBL.
fn shade_bark_metallic_roughness(albedo_srgb:vec3<f32>,roughness:f32,
                                 metallic:f32,tangent_normal:vec3<f32>,
                                 normal:vec3<f32>,tangent:vec4<f32>)
    ->vec3<f32> {
    let V=normalize(globals.view.xyz);
    var N=normalize(normal);
    // The existing foliage pipeline is two-sided: light whichever side
    // of a bark face is visible without reversing the UV tangent axis.
    if (dot(N,V)<0.0) { N=-N; }
    let T=normalize(tangent.xyz-N*dot(N,tangent.xyz));
    let B=normalize(cross(N,T))*tangent.w;
    N=normalize(T*tangent_normal.x+B*tangent_normal.y+N*tangent_normal.z);
    let L=normalize(vec3<f32>(0.46,0.35,0.81));
    let H=normalize(V+L);
    let NoL=max(dot(N,L),0.0);
    let NoV=max(dot(N,V),0.001);
    let NoH=max(dot(N,H),0.0);
    let VoH=max(dot(V,H),0.0);
    let r=clamp(roughness,0.05,1.0);
    let alpha=r*r;
    let alpha2=alpha*alpha;
    let denom=NoH*NoH*(alpha2-1.0)+1.0;
    let D=alpha2/(3.14159265*denom*denom+0.000001);
    let k=(r+1.0)*(r+1.0)/8.0;
    let vis=(NoV/(NoV*(1.0-k)+k)) *
            (NoL/(NoL*(1.0-k)+k));
    let base=srgb_to_linear(albedo_srgb);
    let metal=clamp(metallic,0.0,1.0);
    // A conductor has colored specular reflectance but no diffuse term.
    // 0 metallic is mathematically identical to the Phase 9a dielectric.
    let F0=mix(vec3<f32>(0.04),base,metal);
    let F=F0+(vec3<f32>(1.0)-F0)*pow(1.0-VoH,5.0);
    let specular=(D*vis/max(4.0*NoV*NoL,0.0001))*F;
    let diffuse=(vec3<f32>(1.0)-F)*base*(1.0-metal)/3.14159265;
    var ambient=base*(1.0-metal)*0.16; // Exact prior preview when IBL is off.
    if (globals.bark_mapping.z > 0.5) {
        ambient=studio_indirect(N,V,base,F0,metal,r);
    }
    let lit=ambient+(diffuse+specular)*NoL*2.35;
    return clamp(linear_to_srgb(lit),vec3<f32>(0.0),vec3<f32>(1.0));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Compute derivatives in uniform fragment control flow. The optional
    // mip branch uses the same UVs at the current projected pixel footprint.
    let foliage_uv_dx=dpdx(input.uv);
    let foliage_uv_dy=dpdy(input.uv);
    var rgba: vec4<f32>;
    var bark_uv_main=input.uv;
    var bark_uv_seam=input.uv;
    var bark_seam_mix=0.0;
    if (input.kind < 0.5) {
        let meters = globals.bark_mapping.x;
        if (meters > 0.0 && input.bark_circumference_m > 0.0) {
            // V is the cumulative wood-branch arc length in meters.
            // U is the angular ring fraction; the mesh supplies physical
            // polygon circumference, including taper through clipped wood.
            let ring_tiles = input.bark_circumference_m / meters;
            let uv_main = vec2<f32>(input.uv.x * ring_tiles,
                                    input.uv.y / meters);
            // Noninteger repeat counts cannot wrap around a closed ring.
            // Smoothly realign the last fifth of each ring to its start,
            // so u=1 and u=0 sample identically without repeat-count jumps.
            let uv_seam = vec2<f32>((input.uv.x-1.0) * ring_tiles,
                                    input.uv.y / meters);
            let seam_mix = smoothstep(0.80,1.0,input.uv.x);
            bark_uv_main=uv_main;
            bark_uv_seam=uv_seam;
            bark_seam_mix=seam_mix;
            let a = textureSampleLevel(bark_tex, linear_sampler,uv_main,0.0);
            let b = textureSampleLevel(bark_tex, linear_sampler,uv_seam,0.0);
            rgba = mix(a,b,seam_mix);
        } else {
            rgba = textureSampleLevel(bark_tex,linear_sampler,input.uv,0.0);
        }
    } else if (input.kind < 1.5) {
        if (globals.season.w > 0.5) {
            rgba = textureSampleGrad(foliage_tex, linear_sampler,
                input.uv, foliage_uv_dx, foliage_uv_dy);
        } else {
            // This is the old level-zero sample, unchanged by default.
            rgba = textureSampleLevel(foliage_tex, linear_sampler, input.uv, 0.0);
        }
        let bloom=spring_bloom_amount();
        var blossom=0.0;
        // Do not evaluate the procedural flower mask for any other species
        // or outside the brief early-spring bloom window.
        if (bloom>0.05) {
            if (globals.season.y>1.5) {
                blossom=bloom*coffee_flower_mask(input.uv);
            } else {
                blossom=bloom*cherry_blossom_mask(input.uv);
            }
        }
        // Petals can extend outside the permanent green-leaf silhouette.
        // When bloom fades, the original cutout remains unchanged.
        // Preserve the old full-opacity flower silhouette in all modes.
        let flower_opaque=select(0.0,1.0,blossom>=0.55);
        let coverage=max(rgba.a,flower_opaque);
        if (globals.season.z > 1.5) {
            // 4x MSAA pipeline: pass fractional alpha to hardware
            // alpha-to-coverage; do not hard-discard partial leaf edges.
            if (coverage <= 0.0) { discard; }
        } else if (globals.season.z > 0.5) {
            // Existing experimental single-sample ordered coverage.
            if (coverage < leaf_coverage_threshold(vec2<u32>(input.clip.xy))) {
                discard;
            }
        } else {
            // Keep the legacy 0.5 cutoff unchanged by default.
            if (rgba.a < 0.5 && blossom < 0.55) { discard; }
        }
        let base=max(globals.leaf_base.rgb,vec3<f32>(0.12));
        let leaf_color=min(rgba.rgb*seasonal_foliage(1.0-input.uv.y)/base,
            vec3<f32>(1.0));
        let petal_color=select(vec3<f32>(1.0,0.71,0.81),
            vec3<f32>(0.99,0.98,0.91),globals.season.y>1.5);
        rgba=vec4<f32>(mix(leaf_color,petal_color,blossom),
            select(1.0,coverage,globals.season.z > 1.5));
    } else {
        rgba = vec4<f32>(1.0);
    }
    if (input.kind < 0.5 && globals.bark_mapping.y > 0.5) {
        // Same UV pair/seam fade as albedo; data textures are not sRGB.
        let r0=textureSampleLevel(bark_roughness_tex,linear_sampler,
                                  bark_uv_main,0.0).r;
        let r1=textureSampleLevel(bark_roughness_tex,linear_sampler,
                                  bark_uv_seam,0.0).r;
        let n0=textureSampleLevel(bark_normal_tex,linear_sampler,
                                  bark_uv_main,0.0).rgb;
        let n1=textureSampleLevel(bark_normal_tex,linear_sampler,
                                  bark_uv_seam,0.0).rgb;
        let m0=textureSampleLevel(bark_metallic_tex,linear_sampler,
                                  bark_uv_main,0.0).r;
        let m1=textureSampleLevel(bark_metallic_tex,linear_sampler,
                                  bark_uv_seam,0.0).r;
        let mapped=normalize(mix(n0,n1,bark_seam_mix)*2.0-vec3<f32>(1.0));
        let lit=shade_bark_metallic_roughness(rgba.rgb,
                                     mix(r0,r1,bark_seam_mix),
                                     mix(m0,m1,bark_seam_mix),
                                     mapped,input.normal,input.tangent);
        return vec4<f32>(lit,rgba.a);
    }
    // Legacy bark and ALL foliage modes retain their exact old output path.
    return vec4<f32>(rgba.rgb * input.tint.rgb, rgba.a);
}
