//! name: mongkok
//! title: Mong Kok Neon
//! category: city
//! tags: hong kong, neon, signs, street, taxi, bus, rain, night, city
//! desc: neon signs stacked over a wet Mong Kok street, red taxis and buses passing below
//! themes: rain, night, fog
//! uses: sdf, noise, rain
//! cost: medium
//! fallback: city
//! credits: original

// A Kowloon street at night, seen from a footbridge. The facades run away
// on both sides to a vanishing point; from them the signboards jut out over
// the road, dozens of them, each a flat board facing us at its own depth
// and height: neon tube borders, blocky characters, some lit panels, some
// dark with glowing strokes. They are placed and drawn far to near in
// closed form, and drawn again upside down in the wet road, smeared along
// it. Red taxis and a double-decker bus pass in both directions, their
// lights streaking on the tarmac. Rain, a dry night, or fog that swells
// every sign into a halo.

const V: vec2f = vec2f(0.0, -0.02);  // vanishing point
const FOCAL: f32 = 1.05;
const EYE: f32 = 6.0;                // footbridge height, m
const HALF: f32 = 9.0;               // facade to facade half width, m
const SIGNS: i32 = 30;               // signboards per side

struct Look {
    mode: u32,        // 0 rain, 1 dry night, 2 fog
    sky: vec3f,
    wet: f32,         // how mirror-like the road is
    halo: f32,        // glow around the signs
    haze: f32,        // distance fog density
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, col_hex(0x0a0814u) * 0.5, 0.25, 0.4, 0.008, 0.1);
        }
        case 2u: {
            return Look(2u, col_hex(0x2a2430u) * 0.6, 0.55, 1.4, 0.03, 0.15);
        }
        default: {
            return Look(0u, col_hex(0x100c1cu) * 0.55, 0.85, 0.6, 0.012, 0.2);
        }
    }
}

fn project(w: vec3f) -> vec2f {
    return V + vec2f(w.x, w.y - EYE) * FOCAL / w.z;
}

fn neon(k: f32) -> vec3f {
    // the palette of Mong Kok: red, magenta, amber, cyan, green, white
    let i = u32(k * 6.0) % 6u;
    switch (i) {
        case 0u: { return col_hex(0xff2a3au); }
        case 1u: { return col_hex(0xff3ad0u); }
        case 2u: { return col_hex(0xffa82au); }
        case 3u: { return col_hex(0x2ae0ffu); }
        case 4u: { return col_hex(0x4aff7au); }
        default: { return col_hex(0xf0f0ffu); }
    }
}

struct Sign {
    lo: vec2f,        // screen rectangle
    hi: vec2f,
    z: f32,
    col: vec3f,
    style: f32,       // < 0.7 dark board with neon strokes, else a lit panel
    size: vec2f,      // metres: across, up
    seed: vec4f,
}

fn sign_at(side: f32, k: i32) -> Sign {
    let h = hash_cell2(vec2i(k, i32(side)), 0x4b0cu);
    let h2 = hash_cell2(vec2i(k, i32(side) + 7), 0x9e37u);
    let z = 14.0 + f32(k) * 2.4 + h.x * 1.6;
    let y0 = 3.8 + h.y * 15.0;
    // tall boards with a column of characters, or wide ones with a row
    let tall = h.z > 0.45;
    let height = select(1.0 + 1.3 * h.w, 3.0 + 4.0 * h.w, tall);
    let reach = select(2.2 + 3.0 * h.w, 1.0 + 0.7 * h.z, tall);
    let x_out = side * HALF;
    let x_in = side * (HALF - reach);
    let a = project(vec3f(min(x_out, x_in), y0, z));
    let b = project(vec3f(max(x_out, x_in), y0 + height, z));
    return Sign(a, b, z, neon(h2.x), h2.y, vec2f(reach, height), h2);
}

// a sign's colour at `p` (and coverage in .a); `mirror` is its reflection
fn draw_sign(p: vec2f, s: Sign, l: Look, t: f32, ctx: Ctx) -> vec4f {
    let c = (s.lo + s.hi) * 0.5;
    let hs = (s.hi - s.lo) * 0.5;
    let d = sdf2_box(p - c, hs);
    // the glow reaches past the board
    let glow = exp(-max(d, 0.0) / (0.012 + 0.03 * l.halo * 8.0 / s.z)) * l.halo;
    if (d > 0.0) {
        return vec4f(s.col * glow * 0.12, 0.0);
    }
    let uv = (p - s.lo) / max(s.hi - s.lo, vec2f(1e-4));
    // the tube border
    let edge = min(min(uv.x, 1.0 - uv.x) * hs.x, min(uv.y, 1.0 - uv.y) * hs.y);
    let tube = sstep(ctx.px * 1.5 + 0.004, 0.0, abs(edge - 0.006));
    // characters: square cells of 0.8 m, each a 4x4 pixel glyph with a
    // margin round it
    let w = uv * s.size;
    let inset = step(0.15, w.x) * step(w.x, s.size.x - 0.15) * step(0.15, w.y) * step(w.y, s.size.y - 0.15);
    let cell = floor((w - 0.15) / 0.8);
    let local = fract((w - 0.15) / 0.8);
    let px = floor(local * 5.0);
    let in_glyph = step(0.5, px.x) * step(px.x, 3.5) * step(0.5, px.y) * step(px.y, 3.5);
    let gh = hash_cell2(vec2i(cell * 7.0 + px) + vec2i(i32(s.seed.w * 100.0), 0), 0x61fu);
    let stroke = step(gh.x, 0.5) * inset * in_glyph;
    // a few signs flicker or chase, slowly and rarely
    let flick = select(1.0, 0.55 + 0.45 * step(0.3, fract(t * 0.25 + s.seed.x * 7.0)), s.seed.w > 0.93);
    var col: vec3f;
    if (s.style < 0.7) {
        // a dark board, characters in glowing tube
        col = vec3f(0.02, 0.018, 0.025) + s.col * stroke * 1.6 * flick;
    } else {
        // a lit panel (backlit plastic), characters dark on it
        let panel = mix(s.col * 0.9, vec3f(0.95, 0.92, 0.85), 0.25 * s.seed.z);
        col = mix(panel * 1.1, vec3f(0.03), stroke * 0.85) * flick;
    }
    col = mix(col, s.col * 2.2, tube);
    return vec4f(col, 1.0);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let d = p - V;
    let ax = max(abs(d.x), 1e-4);

    // ---- the sky strip, lit from below by the street
    var col = l.sky * (1.0 + 2.0 * exp(-max(d.y, 0.0) * 4.0));
    // ---- the facades: X = ±HALF, windows and shopfronts
    let zf = FOCAL * HALF / ax;
    let yf = EYE + d.y * zf / FOCAL;
    let road = d.y < 0.0 && yf < 0.0;
    if (!road && yf < 60.0) {
        var fc = col_hex(0x2a2630u) * 0.25;
        // storeys of windows, some lit
        let wg = vec2f(zf * 0.55, yf * 0.38);
        let wh = hash_cell2(vec2i(floor(wg)) + vec2i(i32(sign(d.x)) * 91, 0), 0x77a1u);
        let pane = step(0.3, fract(wg.x)) * step(fract(wg.x), 0.7) * step(0.35, fract(wg.y)) * step(fract(wg.y), 0.65);
        fc *= 0.8 + 0.4 * noise_value2(vec2f(zf * 0.8, yf * 1.5));
        fc += mix(col_kelvin(2800.0), col_kelvin(5000.0), wh.y) * pane * step(wh.x, 0.35) * (0.03 + 0.05 * wh.z);
        // the signs' colour spilling onto the walls
        fc += col_hex(0xff3a9au) * 0.015 + col_hex(0x3ad0ffu) * 0.01 * sstep(0.3, 0.7, noise_value2(vec2f(zf * 0.2, yf * 0.3)));
        // shopfronts at street level: one after another, lit windows
        // between the piers
        let shop = sstep(3.0, 2.7, yf) * sstep(0.3, 0.6, yf);
        let bay = fract(zf / 5.0);
        let sh = hash_cell2(vec2i(i32(floor(zf / 5.0)), i32(sign(d.x))), 0x5b0u);
        let glass = step(0.12, bay) * step(bay, 0.88);
        fc += mix(col_kelvin(3600.0 + 2000.0 * sh.x), neon(sh.y), 0.35 * step(0.6, sh.z)) * shop * glass * (0.08 + 0.1 * sh.w);
        col = fc;
        col = mix(col, l.sky * 3.0, 1.0 - exp(-zf * l.haze));
    }
    // ---- the road: wet tarmac, lane lines, what it mirrors
    if (road) {
        let zr = FOCAL * EYE / -d.y;
        let xr = d.x * zr / FOCAL;
        var rc = col_hex(0x16161au) * 0.4;
        // lane markings and the kerbs
        let lane = sstep(0.12, 0.05, abs(abs(xr) - 3.2)) * step(0.5, fract(zr / 6.0));
        rc += vec3f(0.3) * lane * 0.15;
        let kerb = sstep(0.2, 0.0, abs(abs(xr) - 6.5));
        rc += vec3f(0.2) * kerb * 0.2;
        // pavement beyond the kerbs: shop light pooled on it
        rc += col_kelvin(3800.0) * 0.05 * sstep(6.5, 8.5, abs(xr));
        col = rc;
    }

    // ---- the signboards, far to near, both sides; and their reflections
    var refl = vec3f(0.0);
    for (var k = SIGNS - 1; k >= 0; k--) {
        for (var side = 0; side < 2; side++) {
            let sx = select(-1.0, 1.0, side == 1);
            let s = sign_at(sx, k);
            // direct: only over what is nearer than the facades (always)
            if (p.x > s.lo.x - 0.2 && p.x < s.hi.x + 0.2) {
                let sc = draw_sign(p, s, l, t, ctx);
                let fog = 1.0 - exp(-s.z * l.haze);
                if (sc.a > 0.0) {
                    col = mix(mix(sc.rgb, l.sky * 3.0, fog), col, 0.0);
                } else {
                    col += sc.rgb * (1.0 - fog);
                }
                // mirrored in the wet road: flipped about the road plane and
                // smeared downward
                if (road) {
                    let y_lo = V.y + (-(EYE + (s.hi.y - V.y) * s.z / FOCAL - EYE) - EYE) * FOCAL / s.z;
                    let y_hi = V.y + (-(EYE + (s.lo.y - V.y) * s.z / FOCAL - EYE) - EYE) * FOCAL / s.z;
                    let smear = (y_hi - y_lo) * 0.35 + 0.01;
                    let inx = sstep(0.01, 0.0, max(s.lo.x - p.x, p.x - s.hi.x));
                    let iny = sstep(y_lo - smear, y_lo, p.y) * sstep(y_hi + 0.004, y_hi - 0.004, p.y);
                    let ripple = 0.7 + 0.3 * noise_value2(vec2f(p.x * 90.0, p.y * 30.0 + t * 0.5));
                    refl += s.col * inx * iny * ripple * 0.18 * (1.0 - fog);
                }
            }
        }
    }
    if (road) {
        col += refl * l.wet;
    }

    // ---- traffic: red taxis and a double-decker, both directions
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let h = hash_cell2(vec2i(i, 3), 0x7a41u);
        let away = i % 2 == 0;
        let lane_x = select(-1.0, 1.0, away) * (1.6 + 3.2 * step(0.5, h.x));
        let speed = 6.0 + 4.0 * h.y;
        let span = 110.0;
        let ph = fract((t * speed + h.z * span) / span);
        let z = select(120.0 - ph * span, 8.0 + ph * span, away);
        let bus = i == 4;
        let w = select(0.9, 1.25, bus);
        let tall = select(1.5, 4.4, bus);
        let a = project(vec3f(lane_x - w, 0.0, z));
        let b = project(vec3f(lane_x + w, tall, z));
        let inside = p.x > a.x && p.x < b.x && p.y > a.y && p.y < b.y;
        let uv = (p - a) / max(b - a, vec2f(1e-4));
        let fog = 1.0 - exp(-z * l.haze);
        if (inside) {
            // the body: red taxi with a silver roof, or a cream bus
            var body = select(col_hex(0xb81e1au), col_hex(0xd8d0c0u), bus) * 0.25;
            if (!bus) {
                body = mix(body, col_hex(0x9a9ca4u) * 0.25, step(0.72, uv.y));
            } else {
                // the bus's two rows of lit windows
                let wrow = step(0.35, uv.y) * step(uv.y, 0.48) + step(0.65, uv.y) * step(uv.y, 0.82);
                body = mix(body, col_kelvin(4000.0) * 0.5, wrow * step(0.08, uv.x) * step(uv.x, 0.92));
            }
            // lights: headlights (coming) or tail lights (going)
            let lx = min(abs(uv.x - 0.18), abs(uv.x - 0.82));
            let lamp = sstep(0.1, 0.04, lx) * sstep(0.12, 0.0, abs(uv.y - select(0.3, 0.12, bus)));
            let lc = select(col_kelvin(5500.0) * 2.5, vec3f(1.0, 0.08, 0.04) * 1.6, away);
            col = mix(mix(body + lc * lamp, l.sky * 3.0, fog), col, 0.0);
        }
        // their lights streaking in the wet road
        if (road) {
            let lp = project(vec3f(lane_x, 0.5, z));
            let dx = abs(p.x - lp.x) - (b.x - a.x) * 0.3;
            let streak = sstep(0.01, 0.0, dx) * sstep(lp.y - 0.25 * FOCAL * 6.0 / z, lp.y, p.y) * step(p.y, lp.y);
            let lc = select(col_kelvin(5500.0), vec3f(1.0, 0.1, 0.05), away);
            col += lc * streak * 0.25 * l.wet * (1.0 - fog);
        }
    }

    // ---- weather
    if (l.mode == 0u) {
        // rain shows where there is light to catch it
        let lit = saturate(col_luma(col) * 3.0);
        col += mix(vec3f(0.6, 0.62, 0.75), col, 0.5) * rain_streaks(p, ctx, 0.5, 1.4, 0.06, 2) * 0.25 * lit;
    }
    if (l.mode == 2u) {
        // fog: everything lifts toward the city's glow
        col = mix(col, l.sky * 3.5, 0.25 * sstep(-0.3, 0.3, p.y));
    }
    return col * exp2(l.exposure);
}
