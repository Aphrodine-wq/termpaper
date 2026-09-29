// ---------------------------------------------------------------- fog
// Aerial perspective and fog. Distances are world units.

fn fog_exp(col: vec3f, fog: vec3f, dist: f32, density: f32) -> vec3f {
    return mix(fog, col, exp(-density * dist));
}
// exponential height fog, integrated analytically along the ray: density
// `density` at y = 0, falling off with `falloff` per unit height
fn fog_height(col: vec3f, fog: vec3f, ro: vec3f, rd: vec3f, t: f32, density: f32, falloff: f32) -> vec3f {
    let k = falloff;
    var amount: f32;
    if (abs(rd.y) < 1e-4) {
        amount = density * exp(-k * ro.y) * t;
    } else {
        amount = density * exp(-k * ro.y) * (1.0 - exp(-k * rd.y * t)) / (k * rd.y);
    }
    return mix(col, fog, 1.0 - exp(-max(amount, 0.0)));
}
// the extra glow fog picks up looking toward the sun
fn fog_sun(rd: vec3f, sun: vec3f, sun_col: vec3f, amount: f32) -> vec3f {
    let mu = saturate(dot(rd, sun));
    return sun_col * amount * (0.35 * pow(mu, 8.0) + 0.1 * pow(mu, 2.0));
}
// animated 2D fog band for layered scenes: density at p for a bank centred
// at height y0 with the given thickness (p units)
fn fog_layer2d(p: vec2f, y0: f32, thickness: f32, ctx: Ctx) -> f32 {
    let q = vec2f(p.x * 1.4 + ctx.t * 0.012, p.y * 3.0);
    let n = noise_fbm2(q * 3.0 + vec2f(0.0, ctx.t * 0.004), 5);
    let band = exp(-sq((p.y - y0) / thickness));
    return saturate(band * (0.55 + 0.9 * (n - 0.5)));
}
