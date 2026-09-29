// ---------------------------------------------------------------- l2d
// Layered 2D landscapes: silhouettes that stack back-to-front with haze.
// x is in p units; results are heights in p units relative to a baseline.

// mountain/hill profile height at x
fn l2d_ridge(x: f32, salt: u32, amp: f32, oct: i32) -> f32 {
    let o = f32(salt % 1000u) * 3.17;
    return amp * (noise_fbm2(vec2f(x * 1.3 + o, o * 0.37), oct) * 1.5 - 0.35 + 0.35 * noise_ridged2(vec2f(x * 2.1 + o, 1.7), oct));
}
// city skyline at x: (roof height, building id, x position inside the
// building 0..1, building width)
fn l2d_skyline(x: f32, salt: u32, ctx: Ctx) -> vec4f {
    let w = 0.045;
    let cell = floor(x / w);
    let h = hash_cell2(vec2i(i32(cell), 0), salt);
    let bw = w * (0.7 + 0.3 * h.x);
    let local = (x - cell * w) / bw;
    let inside = local <= 1.0;
    let height = select(0.0, 0.05 + 0.3 * pow(h.y, 2.2) + 0.03 * h.z, inside);
    return vec4f(height, cell, local, bw);
}
// tree line: kind 0 = conifers (pointed), 1 = broadleaf (rounded)
fn l2d_treeline(x: f32, salt: u32, kind: u32) -> f32 {
    let s = 34.0;
    let cell = floor(x * s);
    var hmax = 0.0;
    for (var k = -1; k <= 1; k++) {
        let c = cell + f32(k);
        let h = hash_cell2(vec2i(i32(c), 7), salt);
        let cx = (c + 0.2 + 0.6 * h.x) / s;
        let tall = (0.02 + 0.03 * h.y);
        let dx = abs(x - cx) * s;
        var th: f32;
        if (kind == 0u) {
            th = tall * saturate(1.0 - dx / (0.55 + 0.3 * h.z)) * (1.0 + 0.08 * sin(dx * 20.0));
        } else {
            th = tall * sqrt(saturate(1.0 - sq(dx / (0.8 + 0.4 * h.z))));
        }
        hmax = max(hmax, th);
    }
    return hmax;
}
// parallax: shift p by camera offset cam scaled by 1/depth
fn l2d_parallax(p: vec2f, depth: f32, cam: vec2f) -> vec2f { return p + cam / max(depth, 1e-3); }
// aerial perspective for layer at depth 0 (near) .. 1 (horizon)
fn l2d_atmos(col: vec3f, haze: vec3f, depth: f32) -> vec3f { return mix(col, haze, saturate(depth)); }
