// ---------------------------------------------------------------- wet
// Rain-soaked surfaces: darker, glossier, with puddles that mirror the world.

// puddle mask on a ground plane: coverage 0 (dry) .. 1 (flooded)
fn wet_puddles(xz: vec2f, coverage: f32, ctx: Ctx) -> f32 {
    let n = noise_fbm2(xz * 0.35, 5);
    let edge = 0.04 + ctx.px * 4.0;
    return smoothstep(1.0 - coverage, 1.0 - coverage + edge, n);
}
// wet material: (darkened albedo rgb, roughness)
fn wet_surface(albedo: vec3f, rough: f32, wet: f32) -> vec4f {
    let a = albedo * mix(1.0, 0.45, wet);
    let r = mix(rough, 0.06, wet);
    return vec4f(a, r);
}
// how far to blur a reflection (0 sharp .. 1 very soft) for a roughness and
// distance from the reflector
fn wet_reflect_blur(rough: f32, dist: f32) -> f32 { return saturate(rough * (0.4 + dist * 0.15)); }
