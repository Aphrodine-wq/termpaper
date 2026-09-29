// ---------------------------------------------------------------- clouds
// Volumetric clouds in a horizontal slab [bottom, top] (world units, y up).
// Density: fbm base shape eroded by Worley noise, shaped by a height
// profile; lighting: a short march toward the sun with Beer's law, the
// "powder" darkening, and a two-lobe Henyey-Greenstein phase for silver
// linings. Wind drifts with ctx.t.

fn vol_hg(cos_t: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / (4.0 * PI * pow(max(1.0 + g2 - 2.0 * g * cos_t, 1e-4), 1.5));
}
fn vol_phase2(cos_t: f32) -> f32 { return mix(vol_hg(cos_t, -0.25), vol_hg(cos_t, 0.72), 0.65); }
fn vol_beer_powder(d: f32) -> f32 { return exp(-d) * (1.0 - exp(-2.0 * d)) * 2.0; }

// ray/slab intersection: (t_enter, t_exit), enter > exit on a miss
fn cloud_slab(ro: vec3f, rd: vec3f, bottom: f32, top: f32) -> vec2f {
    if (abs(rd.y) < 1e-5) {
        if (ro.y > bottom && ro.y < top) { return vec2f(0.0, 1e5); }
        return vec2f(1.0, 0.0);
    }
    let t0 = (bottom - ro.y) / rd.y;
    let t1 = (top - ro.y) / rd.y;
    let a = max(min(t0, t1), 0.0);
    let b = max(t0, t1);
    return vec2f(a, b);
}
// density at p; coverage 0..1 (0.35 fair weather, 0.6 broken, 0.85 overcast).
// `scale` is the size of a cloud cell in world units.
fn cloud_density_s(p: vec3f, bottom: f32, top: f32, coverage: f32, scale: f32, t: f32) -> f32 {
    let h = saturate((p.y - bottom) / (top - bottom));
    let profile = smoothstep(0.0, 0.12, h) * smoothstep(1.0, 0.45, h);
    let wind = vec3f(t * 0.012, 0.0, t * 0.004) * scale;
    let q = (p + wind) / scale;
    let base = noise_fbm3(q * vec3f(1.0, 1.6, 1.0), 4);
    let shape = saturate((base - (1.0 - coverage)) / max(coverage, 0.05));
    if (shape * profile < 0.01) { return 0.0; }
    let detail = noise_worley3(q * 4.0 + vec3f(t * 0.01, 0.0, 0.0)).x;
    let eroded = saturate(shape * profile - (0.35 * detail) * (1.0 - shape * 0.5));
    return eroded;
}
fn cloud_density(p: vec3f, bottom: f32, top: f32, coverage: f32, t: f32) -> f32 {
    return cloud_density_s(p, bottom, top, coverage, (top - bottom) * 1.5, t);
}
// March a slab. Returns (in-scattered light rgb, transmittance).
// sigma: extinction per unit density per world unit (0.02-0.2 for km-ish
// slabs in world units of ~100 m). Early-outs at transmittance < 0.02.
fn cloud_march(ro: vec3f, rd: vec3f, sun: vec3f, sun_col: vec3f, amb: vec3f,
               bottom: f32, top: f32, coverage: f32, sigma: f32, max_dist: f32, n: i32, ctx: Ctx) -> vec4f {
    let span = cloud_slab(ro, rd, bottom, top);
    let t0 = span.x;
    let t1 = min(span.y, max_dist);
    if (t1 <= t0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let dt = (t1 - t0) / f32(max(n, 1));
    var t = t0 + dt * ctx.jitter;
    var tr = 1.0;
    var acc = vec3f(0.0);
    let ph = vol_phase2(dot(rd, sun));
    let scale = (top - bottom) * 1.5;
    for (var i = 0; i < 128; i++) {
        if (i >= n || tr < 0.02) { break; }
        let p = ro + rd * t;
        let d = cloud_density_s(p, bottom, top, coverage, scale, ctx.t);
        if (d > 0.001) {
            // light march toward the sun
            var od = 0.0;
            let ls = (top - bottom) * 0.18;
            for (var j = 0; j < 5; j++) {
                let q = p + sun * ls * (f32(j) + 0.5);
                od += cloud_density_s(q, bottom, top, coverage, scale, ctx.t) * ls;
            }
            let hfrac = saturate((p.y - bottom) / (top - bottom));
            let direct = sun_col * ph * vol_beer_powder(od * sigma) * 1.6;
            let ambient = amb * mix(0.35, 1.0, hfrac);
            let ext = d * sigma;
            let step_tr = exp(-ext * dt);
            acc += tr * (direct + ambient) * (1.0 - step_tr);
            tr *= step_tr;
        }
        t += dt;
    }
    return vec4f(acc, tr);
}
// cheap 2D cloud sheet for distant/high layers: (density, lit fraction)
fn cloud_sheet(p: vec2f, coverage: f32, ctx: Ctx) -> vec2f {
    let q = p + vec2f(ctx.t * 0.004, ctx.t * 0.001);
    let d = saturate((noise_fbm2(q, 6) - (1.0 - coverage)) / max(coverage, 0.05));
    let l = saturate((noise_fbm2(q + vec2f(0.03, 0.02), 6) - (1.0 - coverage)) / max(coverage, 0.05));
    return vec2f(d, saturate(0.6 + (d - l) * 4.0));
}
