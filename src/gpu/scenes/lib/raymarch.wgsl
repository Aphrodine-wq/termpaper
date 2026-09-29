// ---------------------------------------------------------------- raymarch
// Sphere tracing against the scene's `fn map(p: vec3f, ctx: Ctx) -> vec2f`
// (x = distance, y = material id). Scenes that use this module must define
// map. The hit threshold grows with distance to match the pixel footprint,
// so far geometry costs no more than near geometry.

// → (t, material); t < 0 on a miss
fn rm_march(ro: vec3f, rd: vec3f, tmin: f32, tmax: f32, max_steps: i32, ctx: Ctx) -> vec2f {
    var t = tmin;
    var m = -1.0;
    for (var i = 0; i < 512; i++) {
        if (i >= max_steps) { break; }
        let h = map(ro + rd * t, ctx);
        let eps = max(0.0002, 0.35 * ctx.px * t);
        if (h.x < eps) { m = h.y; break; }
        t += h.x;
        if (t > tmax) { break; }
    }
    if (m < 0.0) { return vec2f(-1.0, -1.0); }
    return vec2f(t, m);
}
// march with a relaxation factor for non-Lipschitz maps (terrain, displaced
// water): k < 1 steps shorter
fn rm_march_k(ro: vec3f, rd: vec3f, tmin: f32, tmax: f32, max_steps: i32, k: f32, ctx: Ctx) -> vec2f {
    var t = tmin;
    var m = -1.0;
    for (var i = 0; i < 512; i++) {
        if (i >= max_steps) { break; }
        let h = map(ro + rd * t, ctx);
        let eps = max(0.0002, 0.35 * ctx.px * t);
        if (h.x < eps) { m = h.y; break; }
        t += h.x * k;
        if (t > tmax) { break; }
    }
    if (m < 0.0) { return vec2f(-1.0, -1.0); }
    return vec2f(t, m);
}
// tetrahedral normal, epsilon scaled by distance
fn rm_normal(p: vec3f, t: f32, ctx: Ctx) -> vec3f {
    let e = max(0.0005, 0.5 * ctx.px * t);
    let k = vec2f(1.0, -1.0);
    return normalize(
        k.xyy * map(p + k.xyy * e, ctx).x +
        k.yyx * map(p + k.yyx * e, ctx).x +
        k.yxy * map(p + k.yxy * e, ctx).x +
        k.xxx * map(p + k.xxx * e, ctx).x);
}
// soft shadow toward rd (k: softness, higher = sharper); 1 = lit
fn rm_shadow(ro: vec3f, rd: vec3f, tmin: f32, tmax: f32, k: f32, ctx: Ctx) -> f32 {
    var res = 1.0;
    var t = tmin;
    var ph = 1e10;
    let n = steps(48.0, ctx);
    for (var i = 0; i < 128; i++) {
        if (i >= n) { break; }
        let h = map(ro + rd * t, ctx).x;
        let y = h * h / (2.0 * ph);
        let d = sqrt(max(h * h - y * y, 0.0));
        res = min(res, k * d / max(1e-4, t - y));
        ph = h;
        t += clamp(h, 0.01, 0.5 + t * 0.05);
        if (res < 0.002 || t > tmax) { break; }
    }
    return saturate(res);
}
// ambient occlusion from 5 samples along the normal
fn rm_ao(p: vec3f, n: vec3f, ctx: Ctx) -> f32 {
    var occ = 0.0;
    var sca = 1.0;
    for (var i = 0; i < 5; i++) {
        let h = 0.01 + 0.12 * f32(i) / 4.0;
        let d = map(p + h * n, ctx).x;
        occ += (h - d) * sca;
        sca *= 0.95;
    }
    return saturate(1.0 - 3.0 * occ);
}
// Crepuscular rays: fraction of the view ray (up to tmax) that can see the
// light direction `sun`, weighted by density. Each sample casts a short,
// coarse occlusion ray, so keep `steps` around 12-24.
fn rm_shafts(ro: vec3f, rd: vec3f, tmax: f32, sun: vec3f, density: f32, n: i32, ctx: Ctx) -> f32 {
    var acc = 0.0;
    var wsum = 0.0;
    let dt = tmax / f32(max(n, 1));
    var t = dt * ctx.jitter;
    for (var i = 0; i < 48; i++) {
        if (i >= n) { break; }
        let p = ro + rd * t;
        var vis = 1.0;
        var s = 0.05;
        for (var j = 0; j < 12; j++) {
            let h = map(p + sun * s, ctx).x;
            if (h < 0.002) { vis = 0.0; break; }
            s += max(h, 0.05 + s * 0.2);
            if (s > 30.0) { break; }
        }
        let w = exp(-density * t);
        acc += vis * w;
        wsum += w;
        t += dt;
    }
    return acc / max(wsum, 1e-5);
}
