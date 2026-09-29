// ---------------------------------------------------------------- bokeh
// Out-of-focus light discs, as a lens renders distant lights.

// disc of radius r at c, with a slightly brighter rim; 0..~1.2
fn bokeh_disc(p: vec2f, c: vec2f, r: f32, ctx: Ctx) -> f32 {
    let d = length(p - c);
    let edge = max(ctx.px, r * 0.06);
    let body = smoothstep(r + edge, r - edge, d);
    let rim = exp(-sq((d - r * 0.88) / (r * 0.1)));
    return body * (0.8 + 0.4 * rim);
}
// a field of discs on a jittered grid: (coverage, three random numbers for
// colour / brightness). scale = cells per unit, density 0..1
fn bokeh_field(p: vec2f, scale: f32, density: f32, ctx: Ctx) -> vec4f {
    let q = p * scale;
    let c = vec2i(floor(q));
    var best = vec4f(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let cell = c + vec2i(x, y);
            let h = hash_cell2(cell, 0xb0c3u);
            if (h.w > density) { continue; }
            let pos = vec2f(cell) + h.xy;
            let r = 0.35 + 0.45 * h.z;
            let d = length(q - pos);
            let edge = max(ctx.px * scale, 0.04);
            let m = smoothstep(r + edge, r - edge, d) * (0.85 + 0.3 * exp(-sq((d - r * 0.88) / (r * 0.1))));
            if (m > best.x) {
                let g = hash_cell2(cell, 0x77u);
                best = vec4f(m, g.xyz);
            }
        }
    }
    return best;
}
