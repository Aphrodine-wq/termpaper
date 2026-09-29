// ---------------------------------------------------------------- light
// Small lighting helpers: glows, neon tubes, flicker, GGX, windows.

// soft glow for a distance d from a light of radius r
fn light_glow(d: f32, r: f32) -> f32 { return r * r / (r * r + d * d); }
// neon tube: hot core plus wide halo, d = distance to the tube's centreline
fn light_neon(d: f32, core: f32, halo: f32) -> f32 {
    return smoothstep(core, core * 0.4, d) * 2.5 + exp(-d / halo) * 0.6 + light_glow(d, halo * 0.5) * 0.3;
}
// smooth low-frequency flicker in ~[0.6, 1] (candles, bad neon); rate ≤ ~1 Hz
// keeps terminal bandwidth down
fn light_flicker(t: f32, id: u32, rate: f32) -> f32 {
    let x = t * rate + f32(id % 1024u) * 1.37;
    let i = floor(x);
    let f = fract(x);
    let a = hash_f(bitcast<u32>(i32(i)) ^ id);
    let b = hash_f(bitcast<u32>(i32(i) + 1) ^ id);
    let u = f * f * (3.0 - 2.0 * f);
    return 0.6 + 0.4 * mix(a, b, u);
}
fn light_fresnel(cos_t: f32, f0: f32) -> f32 { return f0 + (1.0 - f0) * pow(1.0 - saturate(cos_t), 5.0); }
// GGX specular BRDF times n·l (Smith-Schlick G, Schlick F with f0 = 0.04)
fn light_ggx(n: vec3f, v: vec3f, l: vec3f, rough: f32) -> f32 {
    let h = normalize(v + l);
    let a = max(rough * rough, 0.002);
    let a2 = a * a;
    let nh = saturate(dot(n, h));
    let nl = saturate(dot(n, l));
    let nv = saturate(dot(n, v)) + 1e-4;
    let d = a2 / (PI * sq(nh * nh * (a2 - 1.0) + 1.0));
    let k = a * 0.5;
    let g = nl / (nl * (1.0 - k) + k) * nv / (nv * (1.0 - k) + k);
    let f = light_fresnel(saturate(dot(h, v)), 0.04);
    return d * g * f / (4.0 * nv);
}
// point light with inverse-square falloff softened by radius r
fn light_point(p: vec3f, n: vec3f, lp: vec3f, col: vec3f, r: f32) -> vec3f {
    let d = lp - p;
    let dist2 = dot(d, d);
    let l = d * inverseSqrt(max(dist2, 1e-6));
    return col * saturate(dot(n, l)) / (dist2 + r * r);
}
// Lit windows on a building face. uv = position in window units (1 window per
// unit), id = building id. Returns emitted light; occupancy changes rarely
// (every ~40-90 s per window) so the city lives without flickering.
fn light_window_grid(uv: vec2f, id: vec2f, t: f32, ctx: Ctx) -> vec3f {
    let c = vec2i(floor(uv));
    let f = fract(uv);
    let hb = hash_cell2(vec2i(id * 97.0), 0x4d2u);
    let h = hash_cell2(c + vec2i(i32(id.x * 131.0), i32(id.y * 71.0)), 0x5bd1e995u);
    let period = 40.0 + 50.0 * h.z;
    let slot = floor(t / period + h.w);
    let on = hash_f(bitcast<u32>(i32(slot)) ^ bitcast<u32>(c.x * 7919 + c.y * 104729) ^ 0x1234u) < (0.25 + 0.45 * hb.x);
    let frame = step(0.14, f.x) * step(f.x, 0.86) * step(0.18, f.y) * step(f.y, 0.82);
    let warm = mix(col_kelvin(2700.0), col_kelvin(4200.0), h.x);
    let tv = select(vec3f(1.0), vec3f(0.55, 0.7, 1.0), h.y > 0.93);
    return select(vec3f(0.0), warm * tv * (0.5 + 0.8 * h.y) * frame, on);
}
