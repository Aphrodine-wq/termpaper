// ---------------------------------------------------------------- rain
// Rain as seen by a camera: 2D streak layers (depth-sorted by size/speed),
// ground splashes, and droplets on a window pane. All closed-form in ctx.t.

// density ~0.3-1, speed in frame heights per second (~1.5-3), angle in
// radians (wind slant), layers 1-4. Returns streak intensity 0..~1.
fn rain_streaks(p: vec2f, ctx: Ctx, density: f32, speed: f32, angle: f32, layers: i32) -> f32 {
    var acc = 0.0;
    let q0 = rot2(angle) * p;
    for (var i = 0; i < 4; i++) {
        if (i >= layers) { break; }
        let fi = f32(i);
        let scale = 26.0 + fi * 22.0;
        let q = vec2f(q0.x * scale, q0.y * scale * 0.18 + ctx.t * speed * (1.0 + fi * 0.35) * scale * 0.18);
        let c = vec2i(floor(q));
        let f = fract(q);
        let h = hash_cell2(c, 0x51a3u + u32(i) * 101u);
        if (h.w < density) {
            let x = abs(f.x - (0.2 + 0.6 * h.x));
            let len = 0.35 + 0.5 * h.y;
            let y = f.y - h.z * (1.0 - len);
            let w = max(ctx.px * scale * 0.6, 0.035);
            let s = smoothstep(w, 0.0, x) * smoothstep(0.0, 0.08, y) * smoothstep(len, len - 0.2, y);
            acc += s * (0.55 - fi * 0.1);
        }
    }
    return acc;
}
// splash crowns on a ground plane, 0..1
fn rain_splashes(xz: vec2f, t: f32, density: f32) -> f32 {
    let q = xz * density;
    let c = vec2i(floor(q));
    let h = hash_cell2(c, 0x3b9ac9u);
    let period = 0.35 + h.z * 0.4;
    let ph = fract(t / period + h.w);
    let d = length(fract(q) - (0.25 + 0.5 * h.xy));
    let ring = ph * 0.22;
    return exp(-sq((d - ring) * 40.0)) * (1.0 - ph) * step(ph, 0.6);
}
// Window pane droplets. uv in any unit (≈ frame height = 1). Returns
// (refraction offset xy to sample the background with, drop coverage 0..1).
// Static beads + sliding drops that leave thinning trails.
fn rain_glass(uv: vec2f, t: f32, ctx: Ctx) -> vec3f {
    var off = vec2f(0.0);
    var mask = 0.0;
    // static beads
    for (var k = 0; k < 2; k++) {
        let s = 18.0 + f32(k) * 17.0;
        let q = uv * s;
        let c = vec2i(floor(q));
        let h = hash_cell2(c, 0x9e37u + u32(k) * 7u);
        let life = fract(t * (0.05 + h.z * 0.08) + h.w);
        let r = (0.12 + 0.2 * h.z) * smoothstep(0.0, 0.1, life) * smoothstep(1.0, 0.7, life);
        let d = fract(q) - (0.25 + 0.5 * h.xy);
        let m = smoothstep(r, r * 0.7, length(d));
        off += d * m * 0.8 / s;
        mask = max(mask, m);
    }
    // sliding drops in columns
    let cols = 9.0;
    let cx = floor(uv.x * cols);
    let hc = hash_cell2(vec2i(i32(cx), 0), 0x2545f491u);
    let speed = 0.08 + hc.x * 0.12;
    let yy = uv.y + t * speed + hc.y * 7.0;
    let seg = floor(yy * 1.6);
    let hs = hash_cell2(vec2i(i32(cx), i32(seg)), 0x68bc21u);
    let local = vec2f(fract(uv.x * cols) - 0.5 - (hs.x - 0.5) * 0.5, fract(yy * 1.6) - 0.5);
    // wobble path
    let lx = local.x + 0.08 * sin(local.y * 9.0 + hs.y * 6.0);
    let drop_y = 0.2 - 0.6 * fract(t * 0.25 + hs.z);
    let dd = vec2f(lx * 2.0, (local.y - drop_y) * 1.2);
    let dm = smoothstep(0.16, 0.1, length(dd)) * step(0.4, hs.w);
    off += dd * dm * 0.06;
    mask = max(mask, dm);
    // trail above the drop
    let trail = smoothstep(0.05, 0.0, abs(lx)) * smoothstep(drop_y, drop_y + 0.45, local.y) * step(local.y, 0.5) * step(0.4, hs.w);
    let beads = smoothstep(0.35, 0.2, fract(local.y * 9.0)) * trail;
    off += vec2f(lx, 0.0) * beads * 0.02;
    mask = max(mask, beads * 0.6);
    return vec3f(off, mask);
}
