// Post-processing chain, GPU side.
//
// Every entry point here mirrors a CPU function in `filter.rs`, `color_grade.rs`
// or `canvas.rs`, and the CPU version stays the reference implementation: the
// `gpu_parity` test walks both paths over the same canvas and requires the
// outputs to match within one 8-bit step. Two deliberate exceptions are called
// out at their entry points (`grain`, `pack_*` rounding).
//
// Pixels are packed one per u32 as `r | g<<8 | b<<16 | flags<<24`, where flag
// bit 0 marks "this cell carries a glyph". The CPU owns the glyph characters
// themselves; the GPU only carries the bit so it knows what to preserve, and
// clears it exactly where the CPU's `Canvas::set` would have cleared `ch`.

struct Params {
    // w, h, cell_w, cell_h
    dims: vec4<u32>,
    // cols, rows, crop_x, crop_y
    grid: vec4<u32>,
    // t, p0, p1, p2
    fp: vec4<f32>,
    // p3, p4, p5, p6
    fp2: vec4<f32>,
    // pixel mode (0 half, 1 quad, 2 braille), then three pass-specific
    // values (pack_cells: hysteresis threshold, history valid)
    flags: vec4<u32>,
    // where this buffer sits on the wall: global x0, y0 of its pixel (0, 0)
    // (i32 bits: an apron can start left of the wall), wall W, H. Position-
    // dependent filters work in these coordinates so they run seamlessly
    // across panes; a CPU canvas passes (0, 0, w, h) and nothing changes.
    virt: vec4<u32>,
}

@group(0) @binding(0) var<storage, read>       src:   array<u32>;
@group(0) @binding(1) var<storage, read_write> dst:   array<u32>;
@group(0) @binding(2) var<uniform>             P:     Params;
@group(0) @binding(4) var<storage, read_write> cells: array<u32>;

// One scratch allocation holding three planes — the smoothing history and the
// two bloom blur legs — then the previous frame's packed cells (hysteresis).
// They are separate conceptually but share a binding so the layout needs only
// four storage buffers, which is the downlevel limit — six would have shut out
// every device that only guarantees the minimum.
@group(0) @binding(3) var<storage, read_write> scratch: array<u32>;

fn plane_prev(i: u32) -> u32 { return i; }
fn plane_aux0(i: u32) -> u32 { return P.dims.x * P.dims.y + i; }
fn plane_aux1(i: u32) -> u32 { return 2u * P.dims.x * P.dims.y + i; }
fn plane_hist(i: u32) -> u32 { return 3u * P.dims.x * P.dims.y + i; }

// ---------------------------------------------------------------- primitives

fn W() -> u32 { return P.dims.x; }
fn H() -> u32 { return P.dims.y; }

// Wall coordinates of buffer pixel (x, y), and the wall's size.
fn gx(x: u32) -> i32 { return i32(x) + bitcast<i32>(P.virt.x); }
fn gy(y: u32) -> i32 { return i32(y) + bitcast<i32>(P.virt.y); }
fn VW() -> u32 { return P.virt.z; }
fn VH() -> u32 { return P.virt.w; }

fn idx(x: u32, y: u32) -> u32 { return y * W() + x; }

fn unpack(v: u32) -> vec3<f32> {
    return vec3<f32>(
        f32(v & 0xffu),
        f32((v >> 8u) & 0xffu),
        f32((v >> 16u) & 0xffu),
    );
}

fn flags_of(v: u32) -> u32 { return (v >> 24u) & 0xffu; }

// Pack a colour, dropping the glyph flag. This is the GPU spelling of
// `Canvas::set`, which writes `ch: None` — so every filter that goes through
// here wipes glyphs on the GPU exactly where the CPU wipes them.
fn pack(c: vec3<f32>) -> u32 {
    let q = vec3<u32>(clamp(c, vec3<f32>(0.0), vec3<f32>(255.0)));
    return q.x | (q.y << 8u) | (q.z << 16u);
}

// Pack while carrying the glyph flag through — used by passes the CPU
// implements without `set()` (smoothing) so text cells survive.
fn pack_keep(c: vec3<f32>, fl: u32) -> u32 {
    return pack(c) | (fl << 24u);
}

fn load(x: u32, y: u32) -> vec3<f32> { return unpack(src[idx(x, y)]); }

// `Canvas::get` returns black outside the canvas rather than clamping to the
// edge. The crop path in `pack_cells` can read past the bottom/right edge, so
// it must reproduce that instead of duplicating the last row.
fn load_or_black(x: u32, y: u32) -> vec3<f32> {
    if (x >= W() || y >= H()) { return vec3<f32>(0.0); }
    return unpack(src[idx(x, y)]);
}

fn clampx(x: i32) -> u32 { return u32(clamp(x, 0, i32(W()) - 1)); }
fn clampy(y: i32) -> u32 { return u32(clamp(y, 0, i32(H()) - 1)); }

// The engine's luminance, `(2r + 3g + b) / 6`. Several filters use it and they
// must all agree with the CPU, including the integer truncation in `lum()`.
fn lum6(c: vec3<f32>) -> f32 { return (c.x * 2.0 + c.y * 3.0 + c.z) / 6.0; }

// `canvas::hsv` — h wraps, s and v in 0..=1.
fn hsv(h_in: f32, s: f32, v: f32) -> vec3<f32> {
    let h = fract(fract(h_in) + 1.0) * 6.0;
    let i = i32(floor(h));
    let f = h - floor(h);
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    var rgb: vec3<f32>;
    switch i {
        case 0:  { rgb = vec3<f32>(v, t, p); }
        case 1:  { rgb = vec3<f32>(q, v, p); }
        case 2:  { rgb = vec3<f32>(p, v, t); }
        case 3:  { rgb = vec3<f32>(p, q, v); }
        case 4:  { rgb = vec3<f32>(t, p, v); }
        default: { rgb = vec3<f32>(v, p, q); }
    }
    return clamp(rgb * 255.0, vec3<f32>(0.0), vec3<f32>(255.0));
}

fn hue_rotate(c: vec3<f32>, rot: f32) -> vec3<f32> {
    let n = c / 255.0;
    let mx = max(n.x, max(n.y, n.z));
    let mn = min(n.x, min(n.y, n.z));
    let d = mx - mn;
    if (d < 1e-5) { return c; }
    var h: f32;
    if (mx == n.x) {
        h = fract(fract((n.y - n.z) / d / 6.0) + 1.0);
    } else if (mx == n.y) {
        h = ((n.z - n.x) / d + 2.0) / 6.0;
    } else {
        h = ((n.x - n.y) / d + 4.0) / 6.0;
    }
    var s = 0.0;
    if (mx >= 1e-5) { s = d / mx; }
    return hsv(h + rot, s, mx);
}

// Guard every entry point: dispatches round up to the workgroup size.
fn oob(g: vec3<u32>) -> bool { return g.x >= W() || g.y >= H(); }

// ------------------------------------------------------------- pointwise ops

@compute @workgroup_size(8, 8)
fn copy(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    dst[idx(g.x, g.y)] = src[idx(g.x, g.y)];
}

@compute @workgroup_size(8, 8)
fn scanlines(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let i = idx(g.x, g.y);
    // CPU darkens odd rows only, and leaves even rows byte-identical —
    // including their glyph flag, since it never calls `set` on them.
    // Odd rows of the wall, so the pattern runs on across panes.
    if ((gy(g.y) & 1) == 1) {
        dst[i] = pack(floor(unpack(src[i]) * 0.72));
    } else {
        dst[i] = src[i];
    }
}

@compute @workgroup_size(8, 8)
fn vignette(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    // one vignette around the whole wall, not one per pane
    let cx = f32(VW()) / 2.0;
    let cy = f32(VH()) / 2.0;
    let dx = (f32(gx(g.x)) - cx) / cx;
    let dy = (f32(gy(g.y)) - cy) / cy;
    let d = min(sqrt(dx * dx + dy * dy) / 1.4142135, 1.0);
    let f = 1.0 - d * d * 0.45;
    dst[idx(g.x, g.y)] = pack(floor(load(g.x, g.y) * f));
}

// Deliberate divergence: the CPU seeds `StdRng` per frame and pulls one sample
// per pixel in raster order, which is a sequential dependency with no GPU
// equivalent. This uses a positional hash over the same frame tick and the same
// -14..=14 range, so the grain is statistically identical and still
// deterministic per frame — but it is NOT bit-identical to the CPU path, and
// `gpu_parity` skips it for that reason.
fn hash3(a: u32, b: u32, c: u32) -> u32 {
    var h = a * 0x9e3779b9u ^ b * 0x85ebca6bu ^ c * 0xc2b2ae35u;
    h ^= h >> 16u; h *= 0x7feb352du;
    h ^= h >> 15u; h *= 0x846ca68bu;
    h ^= h >> 16u;
    return h;
}

@compute @workgroup_size(8, 8)
fn grain(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let tick = u32(P.fp.x * 30.0);
    // hashed on wall coordinates: the grain of a pixel is the same whichever
    // pane (or apron) draws it
    let n = f32(hash3(bitcast<u32>(gx(g.x)), bitcast<u32>(gy(g.y)), tick) % 29u) - 14.0;
    dst[idx(g.x, g.y)] = pack(clamp(load(g.x, g.y) + n, vec3<f32>(0.0), vec3<f32>(255.0)));
}

fn shift_mul(g: vec3<u32>, m: vec3<f32>) {
    dst[idx(g.x, g.y)] = pack(floor(load(g.x, g.y) * m));
}

@compute @workgroup_size(8, 8)
fn warm(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    shift_mul(g, vec3<f32>(1.10, 1.0, 0.88));
}

@compute @workgroup_size(8, 8)
fn cool(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    shift_mul(g, vec3<f32>(0.88, 1.0, 1.12));
}

// p0 carries the rotation in turns, so `hue` (fixed 120°) and `spectrum`
// (animated) share one pipeline.
@compute @workgroup_size(8, 8)
fn hue(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    dst[idx(g.x, g.y)] = pack(hue_rotate(load(g.x, g.y), P.fp.y));
}

@compute @workgroup_size(8, 8)
fn duotone(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let accent = vec3<f32>(120.0, 180.0, 255.0);
    let c = load(g.x, g.y);
    let l = (c.x * 2.0 + c.y * 3.0 + c.z) / (6.0 * 255.0);
    dst[idx(g.x, g.y)] = pack(floor(accent * l));
}

@compute @workgroup_size(8, 8)
fn thermal(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    var ramp = array<vec3<f32>, 5>(
        vec3<f32>(0.0, 0.0, 0.0),
        vec3<f32>(90.0, 8.0, 4.0),
        vec3<f32>(255.0, 105.0, 0.0),
        vec3<f32>(255.0, 200.0, 40.0),
        vec3<f32>(255.0, 255.0, 255.0),
    );
    let c = load(g.x, g.y);
    let l = (c.x * 2.0 + c.y * 3.0 + c.z) / (6.0 * 255.0);
    let seg = l * 4.0;
    let i = min(u32(seg), 3u);
    let f = seg - f32(i);
    let a = ramp[i];
    let b = ramp[i + 1u];
    dst[idx(g.x, g.y)] = pack(floor(a + (b - a) * f));
}

@compute @workgroup_size(8, 8)
fn invert(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    dst[idx(g.x, g.y)] = pack(vec3<f32>(255.0) - load(g.x, g.y));
}

@compute @workgroup_size(8, 8)
fn sepia(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let c = load(g.x, g.y);
    let o = vec3<f32>(
        c.x * 0.393 + c.y * 0.769 + c.z * 0.189,
        c.x * 0.349 + c.y * 0.686 + c.z * 0.168,
        c.x * 0.272 + c.y * 0.534 + c.z * 0.131,
    );
    dst[idx(g.x, g.y)] = pack(floor(min(o, vec3<f32>(255.0))));
}

@compute @workgroup_size(8, 8)
fn posterize(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let levels = 5.0;
    let c = load(g.x, g.y);
    // matches the CPU LUT: round to `levels` steps in 0..1, then rescale
    let q = round(c / 255.0 * levels) / levels * 255.0;
    dst[idx(g.x, g.y)] = pack(floor(clamp(q, vec3<f32>(0.0), vec3<f32>(255.0))));
}

@compute @workgroup_size(8, 8)
fn gamma(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let c = load(g.x, g.y) / 255.0;
    dst[idx(g.x, g.y)] = pack(floor(pow(c, vec3<f32>(1.35)) * 255.0));
}

@compute @workgroup_size(8, 8)
fn noir(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let c = load(g.x, g.y);
    let l = (c.x * 0.299 + c.y * 0.587 + c.z * 0.114) / 255.0;
    let t = clamp((l - 0.5) * 1.6 + 0.5, 0.0, 1.0);
    let v = floor(t * 255.0);
    dst[idx(g.x, g.y)] = pack(vec3<f32>(v, v, v));
}

// `canvas::dim` — p0 is the factor. Returns early on the CPU when >= 0.999, so
// the host skips scheduling this pass entirely in that case and glyphs survive.
@compute @workgroup_size(8, 8)
fn dim(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    dst[idx(g.x, g.y)] = pack(floor(load(g.x, g.y) * P.fp.y));
}

// ---------------------------------------------------------- neighbourhood ops

@compute @workgroup_size(8, 8)
fn chroma(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    // fringes grow toward the wall's edges, not each pane's
    let cx = f32(VW()) / 2.0;
    let off = i32((f32(gx(g.x)) - cx) / cx * 2.0);
    let xr = clampx(i32(g.x) - off);
    let xb = clampx(i32(g.x) + off);
    let mid = load(g.x, g.y);
    dst[idx(g.x, g.y)] = pack(vec3<f32>(load(xr, g.y).x, mid.y, load(xb, g.y).z));
}

@compute @workgroup_size(8, 8)
fn pixelate(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    // blocks on the wall's 3 px grid (floor division: an apron can sit at
    // negative wall coordinates), clipped to this buffer
    let block = 3;
    let wx = gx(g.x);
    let wy = gy(g.y);
    let x0 = bitcast<i32>(P.virt.x);
    let y0 = bitcast<i32>(P.virt.y);
    let bx = wx - ((wx % block) + block) % block - x0;
    let by = wy - ((wy % block) + block) % block - y0;
    let xa = u32(max(bx, 0));
    let ya = u32(max(by, 0));
    let xb = u32(min(bx + block, i32(W())));
    let yb = u32(min(by + block, i32(H())));
    var sum = vec3<f32>(0.0);
    var n = 0u;
    for (var y = ya; y < yb; y = y + 1u) {
        for (var x = xa; x < xb; x = x + 1u) {
            sum = sum + load(x, y);
            n = n + 1u;
        }
    }
    // integer-average to match the CPU's u32 accumulate-then-divide
    let avg = floor(sum / f32(n));
    dst[idx(g.x, g.y)] = pack(avg);
}

@compute @workgroup_size(8, 8)
fn edges(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let neon = vec3<f32>(170.0, 255.0, 225.0);
    let yu = clampy(i32(g.y) - 1);
    let yd = clampy(i32(g.y) + 1);
    let xl = clampx(i32(g.x) - 1);
    let xr = clampx(i32(g.x) + 1);
    let gx = (lum6(load(xr, yu)) + 2.0 * lum6(load(xr, g.y)) + lum6(load(xr, yd)))
           - (lum6(load(xl, yu)) + 2.0 * lum6(load(xl, g.y)) + lum6(load(xl, yd)));
    let gy = (lum6(load(xl, yd)) + 2.0 * lum6(load(g.x, yd)) + lum6(load(xr, yd)))
           - (lum6(load(xl, yu)) + 2.0 * lum6(load(g.x, yu)) + lum6(load(xr, yu)));
    let m = clamp(sqrt(gx * gx + gy * gy) / (255.0 * 4.0), 0.0, 1.0);
    dst[idx(g.x, g.y)] = pack(floor(neon * m));
}

@compute @workgroup_size(8, 8)
fn warp(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    // the wave follows wall rows; the 2 px shift reads into the apron
    let shift = i32(round(2.0 * sin(f32(gy(g.y)) * 0.35 + P.fp.x * 1.8)));
    let w = i32(W());
    var xs = (i32(g.x) - shift) % w;
    if (xs < 0) { xs = xs + w; }
    dst[idx(g.x, g.y)] = src[idx(u32(xs), g.y)];
}

@compute @workgroup_size(8, 8)
fn sharpen(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    // CPU leaves the one-pixel border untouched
    if (g.x == 0u || g.y == 0u || g.x + 1u >= W() || g.y + 1u >= H()) {
        dst[idx(g.x, g.y)] = src[idx(g.x, g.y)];
        return;
    }
    let c = lum6(load(g.x, g.y));
    let blur = (lum6(load(g.x - 1u, g.y)) + lum6(load(g.x + 1u, g.y))
              + lum6(load(g.x, g.y - 1u)) + lum6(load(g.x, g.y + 1u))) * 0.25;
    let edge = clamp(c - blur, -80.0, 80.0);
    dst[idx(g.x, g.y)] = pack(floor(clamp(load(g.x, g.y) + edge, vec3<f32>(0.0), vec3<f32>(255.0))));
}

@compute @workgroup_size(8, 8)
fn mirror(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    dst[idx(g.x, g.y)] = src[idx(W() - 1u - g.x, g.y)];
}

// Bloom is four passes because the final add needs the untouched source: the
// bright pass and both blur legs live in aux0/aux1 so they survive the
// ping-pong, and `bloom_add` reads src and aux0 together.
@compute @workgroup_size(8, 8)
fn bloom_bright(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let c = load(g.x, g.y);
    // CPU threshold is on the integer luminance `(2r+3g+b)/6 > 180`
    let l = u32((c.x * 2.0 + c.y * 3.0 + c.z) / 6.0);
    if (l > 180u) {
        scratch[plane_aux0(idx(g.x, g.y))] = pack(c);
    } else {
        scratch[plane_aux0(idx(g.x, g.y))] = 0u;
    }
}

@compute @workgroup_size(8, 8)
fn bloom_h(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let r = 2i;
    let x0 = max(i32(g.x) - r, 0);
    let x1 = min(i32(g.x) + r, i32(W()) - 1);
    var sum = vec3<f32>(0.0);
    var n = 0.0;
    for (var x = x0; x <= x1; x = x + 1) {
        sum = sum + unpack(scratch[plane_aux0(idx(u32(x), g.y))]);
        n = n + 1.0;
    }
    scratch[plane_aux1(idx(g.x, g.y))] = pack(sum / n);
}

@compute @workgroup_size(8, 8)
fn bloom_v_add(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let r = 2i;
    let y0 = max(i32(g.y) - r, 0);
    let y1 = min(i32(g.y) + r, i32(H()) - 1);
    var sum = vec3<f32>(0.0);
    var n = 0.0;
    for (var y = y0; y <= y1; y = y + 1) {
        sum = sum + unpack(scratch[plane_aux1(idx(g.x, u32(y)))]);
        n = n + 1.0;
    }
    let base = load(g.x, g.y);
    dst[idx(g.x, g.y)] = pack(floor(min(base + sum / n * 0.4, vec3<f32>(255.0))));
}

// ------------------------------------------------------------- colour grading

// p0 = hue turns, p1 = saturation, p2 = contrast. The host only schedules this
// when at least one of the three is off its identity value, matching the CPU
// early-out.
fn contrast_ch(v: f32, k: f32) -> f32 {
    let f = v / 255.0;
    return floor(clamp((f - 0.5) * k + 0.5, 0.0, 1.0) * 255.0);
}

@compute @workgroup_size(8, 8)
fn grade(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let rot = P.fp.y;
    let saturation = P.fp.z;
    let contrast = P.fp.w;
    let hue_on = P.flags.y != 0u;
    let sat_on = P.flags.z != 0u;
    let con_on = P.flags.w != 0u;

    var c = load(g.x, g.y);

    if (!hue_on) {
        // CPU fast path: scale saturation in place without an HSV round trip.
        let hi = max(c.x, max(c.y, c.z));
        let lo = min(c.x, min(c.y, c.z));
        if (hi != lo) {
            let mid = min(min(max(c.x, c.y), max(c.x, c.z)), max(c.y, c.z));
            let sat = clamp((hi - lo) / hi * saturation, 0.0, 1.0);
            let new_lo = hi * (1.0 - sat);
            let new_mid = new_lo + (hi - new_lo) * ((mid - lo) / (hi - lo));
            let v_mid = floor(new_mid);
            let v_lo = floor(new_lo);
            var o: vec3<f32>;
            for (var k = 0u; k < 3u; k = k + 1u) {
                let v = c[k];
                if (v == hi)      { o[k] = hi; }
                else if (v == lo) { o[k] = v_lo; }
                else              { o[k] = v_mid; }
            }
            c = o;
        }
    } else {
        let n = c / 255.0;
        let mx = max(n.x, max(n.y, n.z));
        let mn = min(n.x, min(n.y, n.z));
        let d = mx - mn;
        var h = 0.0;
        if (d >= 1e-6) {
            if (mx == n.x)      { h = fract(fract((n.y - n.z) / d / 6.0) + 1.0); }
            else if (mx == n.y) { h = ((n.z - n.x) / d + 2.0) / 6.0; }
            else                { h = ((n.x - n.y) / d + 4.0) / 6.0; }
        }
        var s = 0.0;
        if (mx >= 1e-6) { s = d / mx; }
        // CPU floors saturation at 0.10 so dark greys still take a hue
        if (s < 0.10 && mx > 0.015) { s = 0.10; }
        h = fract(h + rot);
        if (sat_on) { s = clamp(s * saturation, 0.0, 1.0); }
        c = hsv(h, s, mx);
    }

    if (con_on) {
        c = vec3<f32>(contrast_ch(c.x, contrast), contrast_ch(c.y, contrast), contrast_ch(c.z, contrast));
    }
    dst[idx(g.x, g.y)] = pack(c);
}

// ------------------------------------------------------------------ look LUT

// `look::Lut3D::apply`: tetrahedral interpolation through the look's 33^3
// table, 10-bit channels packed `r | g << 10 | b << 20`. The host uploads it
// into the scratch buffer at `flags.y` whenever it changes.
const LUT_N: u32 = 33u;

fn lut_node(r: u32, g: u32, b: u32) -> vec3<f32> {
    let v = scratch[P.flags.y + (b * LUT_N + g) * LUT_N + r];
    return vec3<f32>(f32(v & 1023u), f32((v >> 10u) & 1023u), f32((v >> 20u) & 1023u));
}

@compute @workgroup_size(8, 8)
fn look_lut(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (oob(gid)) { return; }
    let c = load(gid.x, gid.y);
    let f = c * (f32(LUT_N - 1u) / 255.0);
    let i = min(vec3<u32>(floor(f)), vec3<u32>(LUT_N - 2u));
    let fr = f - vec3<f32>(i);
    let c000 = lut_node(i.x, i.y, i.z);
    let c111 = lut_node(i.x + 1u, i.y + 1u, i.z + 1u);
    var w: vec4<f32>;
    var ca: vec3<f32>;
    var cb: vec3<f32>;
    // the tetrahedron holding the point, by the order of its fractions
    if (fr.x > fr.y) {
        if (fr.y > fr.z) {
            w = vec4<f32>(1.0 - fr.x, fr.x - fr.y, fr.y - fr.z, fr.z);
            ca = lut_node(i.x + 1u, i.y, i.z);
            cb = lut_node(i.x + 1u, i.y + 1u, i.z);
        } else if (fr.x > fr.z) {
            w = vec4<f32>(1.0 - fr.x, fr.x - fr.z, fr.z - fr.y, fr.y);
            ca = lut_node(i.x + 1u, i.y, i.z);
            cb = lut_node(i.x + 1u, i.y, i.z + 1u);
        } else {
            w = vec4<f32>(1.0 - fr.z, fr.z - fr.x, fr.x - fr.y, fr.y);
            ca = lut_node(i.x, i.y, i.z + 1u);
            cb = lut_node(i.x + 1u, i.y, i.z + 1u);
        }
    } else if (fr.z > fr.y) {
        w = vec4<f32>(1.0 - fr.z, fr.z - fr.y, fr.y - fr.x, fr.x);
        ca = lut_node(i.x, i.y, i.z + 1u);
        cb = lut_node(i.x, i.y + 1u, i.z + 1u);
    } else if (fr.z > fr.x) {
        w = vec4<f32>(1.0 - fr.y, fr.y - fr.z, fr.z - fr.x, fr.x);
        ca = lut_node(i.x, i.y + 1u, i.z);
        cb = lut_node(i.x, i.y + 1u, i.z + 1u);
    } else {
        w = vec4<f32>(1.0 - fr.y, fr.y - fr.x, fr.x - fr.z, fr.z);
        ca = lut_node(i.x, i.y + 1u, i.z);
        cb = lut_node(i.x + 1u, i.y + 1u, i.z);
    }
    let v = w.x * c000 + w.y * ca + w.z * cb + w.w * c111;
    dst[idx(gid.x, gid.y)] = pack(floor(v * (255.0 / 1023.0) + 0.5));
}

// ------------------------------------------------------------- palette snap

// `look::snap_pixel`: nearest palette colour by a green-weighted distance,
// or (flags.w) the two nearest mixed by a Bayer threshold in wall
// coordinates. Colours sit in the scratch buffer at flags.y, flags.z of them.
var<private> BAYER4: array<u32, 16> = array<u32, 16>(0u, 8u, 2u, 10u, 12u, 4u, 14u, 6u, 3u, 11u, 1u, 9u, 15u, 7u, 13u, 5u);

fn pal_color(k: u32) -> vec3<f32> { return unpack(scratch[P.flags.y + k]); }

fn snap_dist(a: vec3<f32>, b: vec3<f32>) -> f32 {
    let d = a - b;
    return 2.0 * d.x * d.x + 4.0 * d.y * d.y + 3.0 * d.z * d.z;
}

@compute @workgroup_size(8, 8)
fn palette_snap(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let c = load(g.x, g.y);
    let n = P.flags.z;
    var best = 3.0e30;
    var bi = 0u;
    var second = 3.0e30;
    var si = 0u;
    for (var k = 0u; k < 8u; k = k + 1u) {
        if (k >= n) { break; }
        let d = snap_dist(c, pal_color(k));
        if (d < best) {
            second = best;
            si = bi;
            best = d;
            bi = k;
        } else if (d < second) {
            second = d;
            si = k;
        }
    }
    var pick = bi;
    if (P.flags.w != 0u && second < 1.0e30) {
        let d1 = sqrt(best);
        let d2 = sqrt(second);
        var t = 0.0;
        if (d1 + d2 > 0.0) { t = d1 / (d1 + d2); }
        let bx = u32(((gx(g.x) % 4) + 4) % 4);
        let by = u32(((gy(g.y) % 4) + 4) % 4);
        let threshold = (f32(BAYER4[by * 4u + bx]) + 0.5) / 16.0;
        if (t > threshold) { pick = si; }
    }
    dst[idx(g.x, g.y)] = pack(pal_color(pick));
}

// ------------------------------------------------------------------ smoothing

// `Canvas::smooth_blend` — cur = prev*(1-a) + cur*a, glyph cells excluded.
// Writes the blended result to both dst and prev in one pass, which folds the
// CPU's separate `snapshot_into` copy into the same dispatch.
@compute @workgroup_size(8, 8)
fn temporal_smooth(@builtin(global_invocation_id) g: vec3<u32>) {
    if (oob(g)) { return; }
    let i = idx(g.x, g.y);
    let a = P.fp.y;
    let cur = src[i];
    let fl = flags_of(cur);
    if ((fl & 1u) != 0u) {
        // text stays crisp: no blend, and the snapshot still advances
        dst[i] = cur;
        scratch[plane_prev(i)] = cur;
        return;
    }
    let out = floor(unpack(scratch[plane_prev(i)]) * (1.0 - a) + unpack(cur) * a);
    let v = pack_keep(out, fl);
    dst[i] = v;
    scratch[plane_prev(i)] = v;
}

// -------------------------------------------------------------- cell packing

// `render::split` — luminance-threshold a block into a foreground mask plus
// weighted fg/bg means.
struct Split { mask: u32, fg: vec3<f32>, bg: vec3<f32> }

fn split_block(px: array<vec3<f32>, 8>, n: u32) -> Split {
    var lums: array<f32, 8>;
    var lsum = 0.0;
    var tot = vec3<f32>(0.0);
    for (var i = 0u; i < n; i = i + 1u) {
        let c = px[i];
        // CPU `lum()` is integer: 3r + 6g + b over u32
        let l = floor(c.x * 3.0 + c.y * 6.0 + c.z);
        lums[i] = l;
        lsum = lsum + l;
        tot = tot + c;
    }
    let avg = floor(lsum / f32(n));
    var mask = 0u;
    var fg = vec3<f32>(0.0);
    var bg = vec3<f32>(0.0);
    var wf = 0.0;
    var wb = 0.0;
    for (var i = 0u; i < n; i = i + 1u) {
        let c = px[i];
        let l = lums[i];
        if (l > avg) {
            mask = mask | (1u << i);
            let wgt = l - avg + 1.0;
            fg = fg + c * wgt;
            wf = wf + wgt;
        } else {
            let wgt = avg - l + 1.0;
            bg = bg + c * wgt;
            wb = wb + wgt;
        }
    }
    let fallback = floor(tot / f32(n));
    var o: Split;
    o.mask = mask;
    if (wf == 0.0) { o.fg = fallback; } else { o.fg = floor(fg / wf); }
    if (wb == 0.0) { o.bg = fallback; } else { o.bg = floor(bg / wb); }
    return o;
}

fn quad_glyph(mask: u32) -> u32 {
    // U+2580 block elements, indexed by the TL,TR,BL,BR bit pattern
    var t = array<u32, 16>(
        0x20u,   0x2598u, 0x259Du, 0x2580u,
        0x2596u, 0x258Cu, 0x259Eu, 0x259Bu,
        0x2597u, 0x259Au, 0x2590u, 0x259Cu,
        0x2584u, 0x2599u, 0x259Fu, 0x2588u,
    );
    return t[mask & 0xfu];
}

fn braille_glyph(mask: u32) -> u32 {
    var remap = array<u32, 8>(0u, 1u, 2u, 6u, 3u, 4u, 5u, 7u);
    var m = 0u;
    for (var i = 0u; i < 8u; i = i + 1u) {
        if ((mask & (1u << i)) != 0u) { m = m | (1u << remap[i]); }
    }
    return 0x2800u + m;
}

// Every channel of two packed colours within `t` levels.
fn within_levels(a: u32, b: u32, t: u32) -> bool {
    for (var s = 0u; s < 24u; s = s + 8u) {
        let ca = i32((a >> s) & 0xffu);
        let cb = i32((b >> s) & 0xffu);
        if (u32(abs(ca - cb)) > t) { return false; }
    }
    return true;
}

// Write a cell. With hysteresis on (flags.y = threshold) a cell whose fg and
// bg each moved by at most the threshold — keeping its glyph, or being flat
// (fg ≈ bg) so the glyph does not show — re-emits last frame's values, so
// the terminal diff skips it. The history holds what was emitted, so a slow
// drift is sent once it adds up past the threshold: error stays bounded.
fn store_cell(ci: u32, glyph: u32, fg: vec3<f32>, bg: vec3<f32>) {
    var out = vec3<u32>(glyph, pack(fg), pack(bg));
    let t = P.flags.y;
    if (t > 0u) {
        let h = plane_hist(ci * 3u);
        if (P.flags.z != 0u) {
            let prev = vec3<u32>(scratch[h], scratch[h + 1u], scratch[h + 2u]);
            let steady = within_levels(out.y, prev.y, t) && within_levels(out.z, prev.z, t);
            if (steady && (out.x == prev.x || within_levels(out.y, out.z, t))) {
                out = prev;
            }
        }
        scratch[h] = out.x;
        scratch[h + 1u] = out.y;
        scratch[h + 2u] = out.z;
    }
    cells[ci * 3u + 0u] = out.x;
    cells[ci * 3u + 1u] = out.y;
    cells[ci * 3u + 2u] = out.z;
}

// Sentinel: this cell carries a glyph the CPU owns. fg/bg are still valid.
const GLYPH_CELL: u32 = 0xffffffffu;

// One invocation per terminal cell. Reads the finished canvas out of `src` and
// writes (codepoint, fg, bg) triples, so the readback is per-cell rather than
// per-pixel — for braille that is 12 bytes per cell against 32 bytes of pixels.
@compute @workgroup_size(8, 8)
fn pack_cells(@builtin(global_invocation_id) g: vec3<u32>) {
    let cols = P.grid.x;
    let rows = P.grid.y;
    if (g.x >= cols || g.y >= rows) { return; }
    let mode = P.flags.x;
    let pw = P.dims.z;
    let ph = P.dims.w;
    let ci = g.y * cols + g.x;

    let ox = P.grid.z + g.x * pw;
    let oy = P.grid.w + g.y * ph;
    // Out-of-canvas crops read black, matching `Canvas::get`'s OOB behaviour.
    if (ox >= W() || oy >= H()) {
        store_cell(ci, 0x20u, vec3<f32>(0.0), vec3<f32>(0.0));
        return;
    }

    let tl_raw = src[idx(ox, oy)];
    if ((flags_of(tl_raw) & 1u) != 0u) {
        store_cell(ci, GLYPH_CELL, unpack(tl_raw), load_or_black(ox, oy + ph - 1u));
        return;
    }

    var px: array<vec3<f32>, 8>;
    if (mode == 0u) {
        // half: '▀', top colour as fg, bottom as bg, no split
        let bot = load_or_black(ox, oy + 1u);
        store_cell(ci, 0x2580u, unpack(tl_raw), bot);
        return;
    } else if (mode == 1u) {
        px[0] = unpack(tl_raw);
        px[1] = load_or_black(ox + 1u, oy);
        px[2] = load_or_black(ox, oy + 1u);
        px[3] = load_or_black(ox + 1u, oy + 1u);
        let s = split_block(px, 4u);
        store_cell(ci, quad_glyph(s.mask), s.fg, s.bg);
        return;
    } else {
        for (var dx = 0u; dx < 2u; dx = dx + 1u) {
            for (var dy = 0u; dy < 4u; dy = dy + 1u) {
                px[dx * 4u + dy] = load_or_black(ox + dx, oy + dy);
            }
        }
        let s = split_block(px, 8u);
        store_cell(ci, braille_glyph(s.mask), s.fg, s.bg);
        return;
    }
}
