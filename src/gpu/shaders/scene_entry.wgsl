// ---------------------------------------------------------------- entry
// Studio scene entry point, appended after the library and the scene.
//
// One invocation per output pixel of this pane's window. Each pixel is
// supersampled (R2 low-discrepancy offsets), every sample tonemapped before
// averaging, then encoded to sRGB with static dither and packed for the post
// chain (`r | g<<8 | b<<16`, glyph flag clear).

struct Frame {
    // origin.xy: p at the window's top-left pixel corner; step.xy: p per pixel
    map: vec4f,
    // half.xy of the whole wall frame, px (AA width), aspect (half.x/half.y)
    view: vec4f,
    // t, t of the previous hour-cycle, blend (1 = current only), speed
    time: vec4f,
    // window w, h, spp, flags (bit 0: mirror, bit 1: kaleido)
    size: vec4u,
    // seed lo, seed hi, theme, detail
    ids: vec4u,
    // march scale, exposure (stops), unused, unused
    qual: vec4f,
    params: array<vec4f, 2>,
}

@group(0) @binding(0) var<storage, read_write> out_px: array<u32>;
@group(0) @binding(1) var<uniform> F: Frame;

fn entry_ctx() -> Ctx {
    var c: Ctx;
    c.t = F.time.x;
    c.px = F.view.z;
    c.half = F.view.xy;
    c.aspect = F.view.w;
    c.theme = F.ids.z;
    c.detail = F.ids.w;
    c.march = F.qual.x;
    c.seed = F.ids.xy;
    c.seedf = vec2f(F.ids.xy >> vec2u(8u)) * (1.0 / 16777216.0);
    c.jitter = 0.0;
    c.sample = 0u;
    c.spp = max(F.size.z, 1u);
    c.params = F.params[0];
    return c;
}

// Past the frame's left/right edge (a portrait monitor beside a landscape
// row, say), plain perspective packs fewer and fewer degrees into each
// pixel, so everything smears sideways — stars turn into dashes 60-70° off
// axis. Beyond the edge, continue as a cylinder instead: the view angle
// keeps growing at the rate it had at the edge. Only x changes, so horizons
// stay level across monitors; identity inside the frame (single monitors are
// untouched) and continuous at its edge, so the seams stay exact. z is a
// typical camera (about 53° across the short side); flat 2D scenes simply
// continue a little faster.
fn entry_wide(p: vec2f, hx: f32) -> vec2f {
    let a = abs(p.x);
    if (a <= hx) { return p; }
    let z = 1.0;
    let t = min(atan(hx / z) + (a - hx) * z / (z * z + hx * hx), 1.45);
    return vec2f(sign(p.x) * z * tan(t), p.y);
}

fn entry_eval(p: vec2f, ctx: Ctx) -> vec3f {
    var c = scene(p, ctx);
    if (F.time.z < 1.0) {
        // first seconds of a new hour: dissolve from where the old hour's
        // motion would have been, so nothing visibly jumps at the wrap
        var o = ctx;
        o.t = F.time.y;
        c = mix(scene(p, o), c, F.time.z);
    }
    // NaN / inf guard: one bad sample must not blacken a pixel forever
    c = select(c, vec3f(0.0), c != c);
    c = clamp(c, vec3f(0.0), vec3f(65000.0));
    return tm_apply(c * exp2(F.qual.y + SCENE_EXPOSURE));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= F.size.x || gid.y >= F.size.y) { return; }
    var ctx = entry_ctx();
    let lp = vec2f(gid.xy);
    var spp = clamp(F.size.z, 1u, 64u);
    if (SS_MANUAL) { spp = 1u; }
    var acc = vec3f(0.0);
    // global pixel index (identical on every pane of a wall), for dither
    let pc = vec2f(F.map.x + (lp.x + 0.5) * F.map.z, F.map.y - (lp.y + 0.5) * F.map.w);
    let gp = floor(vec2f((pc.x + F.view.x) / F.map.z, (F.view.y - pc.y) / F.map.w));
    let ign = dither_ign(gp);
    for (var i = 0u; i < 64u; i++) {
        if (i >= spp) { break; }
        var o = vec2f(0.5);
        if (spp > 1u) { o = fract(vec2f(0.5) + vec2f(0.7548776662, 0.5698402910) * f32(i)); }
        var p = vec2f(F.map.x + (lp.x + o.x) * F.map.z, F.map.y - (lp.y + o.y) * F.map.w);
        if ((F.size.w & 1u) != 0u) { p.x = -p.x; }
        // kaleido: the top-left quarter everywhere, as `filter::kaleido`
        // does to a Classic canvas (y is up here)
        if ((F.size.w & 2u) != 0u) { p = vec2f(-abs(p.x), abs(p.y)); }
        p = entry_wide(p, F.view.x);
        ctx.jitter = fract(ign + f32(i) * 0.6180340);
        ctx.sample = i;
        acc += entry_eval(p, ctx);
    }
    let lin = acc / f32(spp);
    let s = col_linear_to_srgb(lin) + (dither_ign(gp + vec2f(17.0, 29.0)) - 0.5) / 255.0;
    let q = vec3u(clamp(s * 255.0 + 0.5, vec3f(0.0), vec3f(255.0)));
    out_px[gid.y * F.size.x + gid.x] = q.x | (q.y << 8u) | (q.z << 16u);
}
