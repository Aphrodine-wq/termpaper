// ---------------------------------------------------------------- snow
// Falling flakes in parallax layers with lateral sway; ground cover factor.

// density ~0.3-1, wind in frame widths per second (signed), layers 1-5.
// Returns flake coverage 0..1 (larger/brighter layers are "closer").
fn snow_flakes(p: vec2f, ctx: Ctx, density: f32, wind: f32, layers: i32) -> f32 {
    var acc = 0.0;
    for (var i = 0; i < 5; i++) {
        if (i >= layers) { break; }
        let fi = f32(i);
        let scale = 7.0 + fi * 6.0;
        let fall = (0.07 + 0.05 / (1.0 + fi)) * scale;
        var q = p * scale + vec2f(ctx.t * wind * scale * (1.0 - fi * 0.12), ctx.t * fall);
        q.x += 0.35 * sin(q.y * 0.9 + fi * 2.1 + ctx.t * 0.7);
        let c = vec2i(floor(q));
        let h = hash_cell2(c, 0x6a09e667u + u32(i) * 31u);
        if (h.w < density) {
            let pos = 0.2 + 0.6 * h.xy;
            let d = length(fract(q) - pos);
            let r = (0.05 + 0.07 * h.z) * (1.0 + 0.15 * fi);
            let blur = max(ctx.px * scale, 0.02);
            acc += smoothstep(r + blur, r - blur * 0.5, d) * (1.0 - fi * 0.14);
        }
    }
    return saturate(acc);
}
// how much snow settles on a surface with normal n (amount 0..1)
fn snow_cover(n: vec3f, amount: f32) -> f32 {
    return smoothstep(0.55 - amount * 0.4, 0.85 - amount * 0.3, n.y) * amount;
}
