// ---------------------------------------------------------------- water
// Open water: a sum of directional waves (long swell + chop octaves) with
// distance-based LOD, Schlick Fresnel, and helpers for ripples and caustics.
// Heights are world units; `choppy` ~0.3 calm lake … 1.5 rough sea.

fn water_wave(xz: vec2f, dir: vec2f, freq: f32, speed: f32, t: f32) -> f32 {
    let ph = dot(xz, dir) * freq + t * speed;
    // sharpened sine: peaks pinched, troughs broad
    let s = sin(ph) * 0.5 + 0.5;
    return pow(s, 2.2) * 2.0 - 1.0;
}
// height at xz; oct 1-8 (fewer far away)
fn water_height(xz: vec2f, t: f32, choppy: f32, oct: i32) -> f32 {
    var h = 0.0;
    var amp = 0.5 * choppy;
    var freq = 0.16;
    var ang = 0.35;
    for (var i = 0; i < 8; i++) {
        if (i >= oct) { break; }
        let dir = vec2f(cos(ang), sin(ang));
        let spd = sqrt(9.81 * freq) * 0.9;
        h += amp * water_wave(xz + vec2f(f32(i) * 11.3, 0.0), dir, freq, spd, t);
        let n = noise_grad2(xz * freq * 0.35 + vec2f(t * 0.05, f32(i)));
        h += amp * 0.25 * n;
        amp *= 0.52;
        freq *= 1.85;
        ang += 1.7;
    }
    return h;
}
// Gerstner displacement of one wave: (dx, dy, dz)
fn water_gerstner(xz: vec2f, t: f32, dir: vec2f, wavelength: f32, steep: f32) -> vec3f {
    let k = TAU / wavelength;
    let c = sqrt(9.81 / k);
    let d = normalize(dir);
    let f = k * (dot(d, xz) - c * t);
    let a = steep / k;
    return vec3f(d.x * a * cos(f), a * sin(f), d.y * a * cos(f));
}
fn water_lod(dist: f32, ctx: Ctx) -> i32 {
    return clamp(i32(8.0 - log2(1.0 + dist * ctx.px * 40.0) * 1.6), 2, 8);
}
// surface normal by central differences; fewer octaves with distance
fn water_normal(xz: vec2f, t: f32, choppy: f32, dist: f32, ctx: Ctx) -> vec3f {
    let oct = water_lod(dist, ctx);
    let e = max(0.02, dist * ctx.px * 0.6);
    let hx = water_height(xz + vec2f(e, 0.0), t, choppy, oct) - water_height(xz - vec2f(e, 0.0), t, choppy, oct);
    let hz = water_height(xz + vec2f(0.0, e), t, choppy, oct) - water_height(xz - vec2f(0.0, e), t, choppy, oct);
    // flatten toward the horizon so distant water reads as a mirror, not noise
    let flat_k = saturate(dist * ctx.px * 1.5);
    return normalize(vec3f(-hx, 2.0 * e * (1.0 + flat_k * 6.0), -hz));
}
fn water_fresnel(cos_t: f32) -> f32 {
    let f0 = 0.02;
    return f0 + (1.0 - f0) * pow(1.0 - saturate(cos_t), 5.0);
}
// ray/plane y = h; t < 0 on a miss
fn water_intersect(ro: vec3f, rd: vec3f, h: f32) -> f32 {
    if (rd.y >= -1e-5) { return -1.0; }
    return (h - ro.y) / rd.y;
}
// shade: reflection `refl` (sampled by the scene along reflect(rd, n)),
// body colour from deep→shallow by `depth` (0 shallow .. 1 deep)
fn water_color(rd: vec3f, n: vec3f, refl: vec3f, deep: vec3f, shallow: vec3f, depth: f32) -> vec3f {
    let f = water_fresnel(dot(-rd, n));
    let body = mix(shallow, deep, saturate(depth));
    return mix(body, refl, f);
}
// raindrop ripple rings: (dx, dz) normal perturbation and ring mask
fn water_rain_ripples(xz: vec2f, t: f32, density: f32) -> vec3f {
    var acc = vec3f(0.0);
    let q = xz * density;
    let c = vec2i(floor(q));
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let cell = c + vec2i(x, y);
            let h = hash_cell2(cell, 0x7a1b2c3du);
            let period = 0.9 + h.z * 0.8;
            let ph = fract(t / period + h.w);
            let pos = vec2f(cell) + h.xy;
            let d = q - pos;
            let r = length(d);
            let ring = ph * 1.2;
            let w = exp(-sq((r - ring) * 9.0)) * (1.0 - ph) * (1.0 - ph);
            let dir = d / max(r, 1e-4);
            acc += vec3f(dir * w * sin((r - ring) * 30.0), w);
        }
    }
    return acc;
}
// bright caustic network on the bottom of shallow water (0..~1.5)
fn water_caustics(xz: vec2f, t: f32) -> f32 {
    let a = noise_worley2(xz * 1.3 + vec2f(t * 0.11, t * 0.07));
    let b = noise_worley2(xz * 1.7 - vec2f(t * 0.09, -t * 0.12) + 7.0);
    let ca = pow(saturate(1.0 - (a.y - a.x) * 3.0), 6.0);
    let cb = pow(saturate(1.0 - (b.y - b.x) * 3.0), 6.0);
    return (ca + cb) * 0.75;
}
