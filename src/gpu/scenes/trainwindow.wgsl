//! name: trainwindow
//! title: Train Window
//! category: city
//! tags: train, countryside, golden-hour, parallax, travel, reflection
//! desc: golden-hour countryside streaming past a train window, poles flicking by, the sun low
//! themes: golden, night, snow
//! uses: sdf, rain, stars, noise
//! cost: light
//! tonemap: punchy
//! fallback: drive
//! credits: original

// A seat on a regional train, looking out of the left-hand window. Outside is
// a true ground plane seen side-on: a point at screen x and ground depth z is
// world X = x * z / FOC + SPEED * t, so every layer gets exactly the right
// parallax (near verge blurring past, hedgerows gliding, hills barely moving)
// without any per-layer speed tuning. Motion blur is analytic: textures are
// stretched along the motion, poles are box-blurred over a frame.

const HY: f32 = 0.1;       // horizon height on screen
const EYE: f32 = 7.0;      // eye height above the fields (m); the line runs on an embankment
const FOC: f32 = 1.1;      // focal length in p units
const SPEED: f32 = 17.0;   // m/s (~60 km/h)
const WIN_C: vec2f = vec2f(0.0, 0.06);
const WIN_H: vec2f = vec2f(0.66, 0.29);
const WIN_R: f32 = 0.1;
const PITCH: f32 = 1.9;    // window spacing along the carriage

struct Look {
    mode: u32,        // 0 golden, 1 night, 2 snow
    sun: vec2f,       // sun (or moon) position on screen
    sun_c: vec3f,     // sun colour/intensity
    sky_hi: vec3f,
    sky_lo: vec3f,
    haze: vec3f,      // aerial haze
    land: vec3f,      // light falling on the land
    inside: vec3f,    // interior ambient
    refl: f32,        // glass reflection strength
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, vec2f(0.42, 0.27), vec3f(0.5, 0.55, 0.65),
                        col_hex(0x0b1226u) * 0.5, col_hex(0x1d2640u) * 0.6, col_hex(0x1a2238u) * 0.5,
                        vec3f(0.012, 0.015, 0.024), col_hex(0xe6e0d6u) * 0.22, 0.2, 0.4);
        }
        case 2u: {
            return Look(2u, vec2f(-0.3, 0.17), col_hex(0xffe6c8u) * 1.8,
                        col_hex(0x8e98b0u) * 0.6, col_hex(0xdcc8c2u) * 1.0, col_hex(0xd2c6c8u) * 0.95,
                        col_hex(0xdcd8e2u) * 0.62, col_hex(0xd8dadeu) * 0.19, 0.08, 0.0);
        }
        default: {
            return Look(0u, vec2f(-0.3, 0.2), col_hex(0xffb466u) * 6.0,
                        col_hex(0x7c9ccau) * 0.8, col_hex(0xf8c27cu) * 1.6, col_hex(0xf3b674u) * 1.3,
                        col_hex(0xffc88au) * 0.9, col_hex(0xdfdcd6u) * 0.2, 0.08, 0.0);
        }
    }
}

// ------------------------------------------------------------------ outside
fn world_x(x: f32, z: f32, t: f32) -> f32 { return x * z / FOC + SPEED * t; }
// screen y of a point h metres above the fields at depth z
fn scr_y(h: f32, z: f32) -> f32 { return HY - (EYE - h) * FOC / z; }
fn fogk(z: f32, l: Look) -> f32 {
    let k = select(0.0011, 0.0016, l.mode == 2u);
    return 1.0 - exp(-z * k);
}

fn tw_sky(p: vec2f, l: Look, ctx: Ctx) -> vec3f {
    let e = p.y - HY;
    var c = mix(l.sky_lo, l.sky_hi, smoothstep(-0.01, 0.3, e));
    let d = length((p - l.sun) * vec2f(1.0, 1.25));
    if (l.mode == 1u) {
        // moonlit night: faint afterglow low on the left, stars, a pale moon
        c += col_hex(0x3a3048u) * 0.08 * exp(-max(e, 0.0) * 12.0) * saturate(0.3 - p.x);
        let sd = normalize(vec3f(p.x * 0.9, e + 0.05, 1.0));
        c += star_field(sd, 0.3, ctx) * smoothstep(0.02, 0.1, e) * 0.7;
        c += l.sun_c * (smoothstep(0.024, 0.02, d) * 3.0 + 0.05 * exp(-d * 14.0));
        return c;
    }
    // sun and its glow; high thin cloud catching the light
    c += l.sun_c * (0.5 * exp(-d * 11.0) + 0.1 * exp(-d * 3.5) + 0.03 * exp(-d * 1.2));
    let disk = smoothstep(0.024, 0.017, d);
    c += l.sun_c * disk * select(10.0, 1.5, l.mode == 2u);
    let q = vec2f(p.x * 1.2, e * 6.0);
    let cl = noise_fbm2(vec2f(q.x * 2.0 + 3.0, q.y * 2.5) + vec2f(ctx.t * 0.0015, 0.0), 5);
    let band = smoothstep(0.5, 0.75, cl) * smoothstep(0.05, 0.12, e) * smoothstep(0.42, 0.2, e);
    let lit = mix(l.sky_lo * 1.1, l.sun_c * 0.35, exp(-d * 3.0));
    c = mix(c, lit, band * select(0.55, 0.35, l.mode == 2u));
    return c;
}

// rolling hills far away: height in metres at world X
fn hill_h(x: f32, layer: i32) -> f32 {
    let o = f32(layer) * 13.7;
    if (layer == 0) {
        return 260.0 * noise_fbm2(vec2f(x / 3200.0 + o, 0.3), 4) + 60.0;
    }
    return 70.0 * noise_fbm2(vec2f(x / 900.0 + o, 0.7), 4) + 10.0;
}

// village on a rise: (coverage, lit-window glow)
fn village(p: vec2f, z: f32, t: f32, aa: f32) -> vec2f {
    let x = world_x(p.x, z, t);
    let per = 1500.0;
    let cell = floor(x / per);
    let hv = hash_cell2(vec2i(i32(cell), 0), 0x71au);
    let lx = x - (cell * per + per * 0.5 + (hv.x - 0.5) * 300.0);
    if (abs(lx) > 260.0) { return vec2f(0.0); }
    // the rise the village sits on
    let rise = 14.0 * saturate(1.0 - sq(lx / 300.0));
    let yh = (p.y - HY) * z / FOC + EYE;          // height in metres at this pixel
    var d = yh - rise;
    // houses: 13 m plots
    let hc = floor(lx / 13.0);
    let hh = hash_cell2(vec2i(i32(hc), i32(cell)), 0x40u);
    let fx = lx - (hc + 0.5) * 13.0;
    var glow = 0.0;
    if (hh.w > 0.25 && abs(lx) < 230.0) {
        let wall = 5.0 + 3.0 * hh.x;
        let hw = 4.0 + 1.5 * hh.y;
        let roof = wall + hw * 0.7 - abs(fx) * 0.7;
        let top = rise + min(wall + hw * 0.7, roof);
        d = min(d, max(yh - top, abs(fx) - hw));
        // a lit window or two
        if (hh.z > 0.55) {
            let wy = yh - rise - 2.5;
            let wd = max(abs(fx - (hh.x - 0.5) * 4.0) - 0.6, abs(wy) - 0.7);
            glow = max(glow, smoothstep(aa, 0.0, wd));
        }
    }
    // church: tower and spire near the middle
    let cx = lx - (hv.y - 0.5) * 120.0;
    let tower = max(abs(cx) - 3.2, yh - rise - 21.0);
    let spire = max(yh - rise - 21.0 - 16.0 * (1.0 - abs(cx) / 3.4), -(yh - rise - 21.0));
    d = min(d, min(tower, max(spire, abs(cx) - 3.4)));
    // trees around the houses
    let tc = floor(lx / 21.0);
    let th = hash_cell2(vec2i(i32(tc), i32(cell) + 77), 0x7eu);
    if (th.x > 0.45) {
        let tx = lx - (tc + 0.5 + (th.y - 0.5) * 0.5) * 21.0;
        let ty = yh - rise - (6.0 + 5.0 * th.z);
        d = min(d, length(vec2f(tx, ty * 1.2)) - (4.5 + 2.0 * th.z));
    }
    return vec2f(saturate(0.5 - d / aa), glow);
}

// hedgerow and trees of row r: (coverage)
fn hedgerow(p: vec2f, z: f32, r: i32, t: f32, aa: f32, bare: bool) -> f32 {
    let dens = select(1.0, 0.6, bare);
    let x = world_x(p.x, z, t);
    let yh = (p.y - HY) * z / FOC + EYE;           // metres above the field at the row
    if (yh < -0.5) { return 0.0; }
    let o = f32(r) * 31.0;
    // hedge with gaps (gates, breaks)
    let hn = noise_value2(vec2f(x / 7.0 + o, 0.5));
    let gap = smoothstep(0.18, 0.28, noise_value2(vec2f(x / 40.0 + o, 3.5)));
    let hedge_h = (1.5 + 1.1 * hn) * gap * select(1.0, 0.55, r == 0);
    var d = yh - hedge_h;
    // standard trees along the hedge: oaks and ash
    let cs = 26.0;
    let c = floor(x / cs);
    for (var k = -1; k <= 1; k++) {
        let ck = c + f32(k);
        let h = hash_cell2(vec2i(i32(ck), r), 0x0a4u);
        if (h.w > 0.55) { continue; }
        let tx = x - (ck + 0.5 + (h.x - 0.5) * 0.6) * cs;
        let top = 8.0 + 7.0 * h.y;
        let rad = 3.8 + 2.6 * h.z;
        let cy = top - rad;
        let q = vec2f(tx, yh - cy);
        let ang = atan2(q.y, q.x);
        var rr = rad * (1.0 + 0.13 * sin(ang * 3.0 + h.x * 20.0) + 0.08 * sin(ang * 7.0 + h.y * 9.0));
        var dc = length(q * vec2f(1.0, 1.1)) - rr;
        if (bare) {
            // winter: twiggy crowns read as a see-through haze of branches
            dc += 1.2 * (1.0 - noise_value2(q * 0.35 + h.xy * 50.0));
        }
        let trunk = max(abs(tx) - 0.45, yh - cy);
        d = min(d, min(dc + select(0.0, 0.0, bare), trunk));
    }
    return saturate(0.5 - d / aa) * select(1.0, dens + (1.0 - dens) * step(yh, 2.6), bare);
}

// ground: field patchwork bounded by the hedgerows, verge in front
fn field(p: vec2f, z: f32, t: f32, l: Look, ctx: Ctx, rows: array<f32, 5>) -> vec3f {
    let x = world_x(p.x, z, t);
    let fpx = ctx.px * z / FOC + SPEED * 0.03;
    let fpz = ctx.px * z * z / (EYE * FOC);
    let snow = l.mode == 2u;
    // which strip between hedges
    var zone = 0;
    for (var i = 0; i < 5; i++) { if (z > rows[i]) { zone = i + 1; } }
    var alb: vec3f;
    if (zone == 0) {
        // the verge: long grass streaming past, blurred along the motion
        let n = noise_value2(vec2f(x / max(1.5, fpx * 2.0), z * 2.2)) * 0.6 + noise_value2(vec2f(x / 9.0, z * 0.5)) * 0.4;
        alb = mix(col_hex(0x7c7434u), col_hex(0xd2b866u), n);
        if (snow) { alb = mix(col_hex(0xd4d8e2u), col_hex(0xf0f2f6u), n); }
    } else {
        let len = 110.0 + 60.0 * f32(zone);
        let jit = noise_value2(vec2f(z / 30.0, f32(zone) * 3.0)) * 40.0;
        let fc = floor((x + jit) / len);
        let h = hash_cell2(vec2i(i32(fc), zone), 0xf1du);
        var base: vec3f;
        if (h.x < 0.3) { base = col_hex(0xc9a55cu); }          // stubble
        else if (h.x < 0.55) { base = col_hex(0x7a8a3cu); }    // pasture
        else if (h.x < 0.75) { base = col_hex(0x8c6e46u); }    // ploughed
        else { base = col_hex(0xa3a24eu); }                    // young barley
        if (snow) { base = mix(col_hex(0xdfe3eeu), col_hex(0xc6c9d6u), h.x); }
        // drill rows / furrows parallel to the line, fading with distance
        let rows_k = saturate(1.0 - fpz / 1.2);
        let furrow = 0.5 + 0.5 * sin(z * (4.0 + 3.0 * h.y));
        base *= 1.0 - 0.18 * furrow * rows_k * step(0.55, h.x);
        let tex = noise_value2(vec2f(x / max(8.0, fpx), z / max(4.0, fpz)) + h.zw * 50.0);
        alb = base * (0.85 + 0.3 * tex);
    }
    // long shadows of the hedges toward us (the sun is ahead of us)
    var sh = 1.0;
    if (l.mode != 1u) {
        for (var i = 0; i < 5; i++) {
            let zr = rows[i];
            if (z < zr && z > zr - 26.0 - 0.12 * zr) {
                let gap = smoothstep(0.18, 0.28, noise_value2(vec2f((x + (zr - z) * 0.6) / 40.0 + f32(i) * 31.0, 3.5)));
                sh = min(sh, mix(1.0, 0.45, gap * smoothstep(zr - 26.0 - 0.12 * zr, zr - 4.0, z)));
            }
        }
    }
    // backlit sheen toward the sun, strongest on the far fields
    let dxs = abs(p.x - l.sun.x);
    let sheen = (exp(-dxs * 2.5) * 1.1 + exp(-dxs * 7.0) * 1.2) * smoothstep(20.0, 300.0, z);
    // the verge's seed heads glow where the sun shines through them
    let verge_glow = select(0.0, 0.25 + 0.6 * exp(-dxs * 1.6), zone == 0);
    var c = alb * l.land * (0.3 * sh + (sheen + verge_glow) * sh + 0.05);
    if (snow) {
        let shade = mix(1.0, sh, 0.6);
        c = alb * l.land * (0.95 * shade + sheen * 0.25);
        c *= mix(vec3f(0.82, 0.88, 1.08), vec3f(1.06, 1.0, 0.94), saturate(shade * 0.7 + sheen * 0.5));
    }
    if (l.mode == 1u) { c = alb * l.land * 1.2; }
    return c;
}

fn outside(p: vec2f, l: Look, ctx: Ctx) -> vec3f {
    let t = ctx.t;
    var rows = array<f32, 5>(34.0, 62.0, 115.0, 200.0, 360.0);
    var c = tw_sky(p, l, ctx);
    let night = l.mode == 1u;
    let bare = l.mode == 2u;
    // far downs and near hills
    for (var layer = 0; layer < 2; layer++) {
        let z = select(6000.0, 1400.0, layer == 1);
        let x = world_x(p.x, z, t);
        let top = scr_y(hill_h(x, layer), z);
        let a = saturate(0.5 - (p.y - top) / ctx.px);
        if (a > 0.0) {
            var hc = mix(l.land * 0.18, l.haze, fogk(z, l) * 0.9);
            if (night) { hc = mix(col_hex(0x0d1322u) * 0.25, l.sky_lo, 0.35 - f32(layer) * 0.15); }
            if (bare) { hc = mix(col_hex(0xb8b8c8u) * 0.6, l.haze, fogk(z, l)); }
            c = mix(c, hc, a);
        }
    }
    // village on its rise
    {
        let z = 520.0;
        let aa = ctx.px * z / FOC;
        let v = village(p, z, t, aa);
        if (v.x > 0.0) {
            var vc = mix(l.land * 0.05, l.haze, fogk(z, l) * 0.75);
            if (night) { vc = col_hex(0x080c16u) * 0.3; }
            if (bare) { vc = mix(col_hex(0x6a6570u) * 0.5, l.haze, fogk(z, l) * 0.8); }
            c = mix(c, vc, v.x);
        }
        let lamp = select(0.4, 3.0, night);
        c += col_kelvin(2400.0) * v.y * lamp * (1.0 - fogk(z, l) * 0.5);
    }
    // the land below the horizon
    let below = HY - p.y;
    var zg = 1e6;
    if (below > 0.0) {
        zg = EYE * FOC / below;
        var g = field(p, zg, t, l, ctx, rows);
        var fk = fogk(zg, l);
        if (night) { fk *= 0.6; }
        g = mix(g, l.haze * select(0.9, 0.6, night), fk);
        // soften the far land into the horizon
        c = mix(g, c, smoothstep(2500.0, 12000.0, zg) * 0.0);
        c = g;
        // hills sit on the horizon: redo them over the far fields
        for (var layer = 0; layer < 2; layer++) {
            let z = select(6000.0, 1400.0, layer == 1);
            if (zg < z) { continue; }
            let x = world_x(p.x, z, t);
            let a = saturate(0.5 - (p.y - scr_y(hill_h(x, layer), z)) / ctx.px);
            var hc = mix(l.land * 0.18, l.haze, fogk(z, l) * 0.9);
            if (night) { hc = mix(col_hex(0x0d1322u) * 0.25, l.sky_lo, 0.35 - f32(layer) * 0.15); }
            if (bare) { hc = mix(col_hex(0xb8b8c8u) * 0.6, l.haze, fogk(z, l)); }
            c = mix(c, hc, a);
        }
    }
    // hedgerows, far to near, only those in front of the ground point
    for (var i = 4; i >= 0; i--) {
        let z = rows[i];
        if (z > zg) { continue; }
        let speed = SPEED * FOC / z;
        let aa = (ctx.px + speed * 0.03) * z / FOC;
        let a = hedgerow(p, z, i, t, aa, bare);
        if (a <= 0.0) { continue; }
        // backlit foliage: dark, glowing at the edge nearest the sun
        let glow = exp(-length(p - l.sun) * 2.5);
        let ah = hedgerow(p + vec2f(0.0, ctx.px * 1.5), z, i, t, aa, bare);
        let rim = saturate(a - ah) * (0.3 + 1.5 * glow);
        var hc = col_hex(0x10160au) * l.land * (0.1 + 0.3 * glow) + l.sun_c * 0.004 * glow;
        hc += l.sun_c * 0.06 * rim * select(1.0, 0.25, bare);
        if (night) { hc = col_hex(0x05080cu) * 0.4; }
        if (bare) {
            // frost and snow lie on the hedge tops
            let yh = (p.y - HY) * z / FOC + EYE;
            hc = col_hex(0x4a4444u) * l.land * 0.8;
            hc = mix(hc, col_hex(0xe8ecf4u) * l.land, smoothstep(0.6, 1.8, yh) * smoothstep(3.2, 2.0, yh) * 0.7);
        }
        hc = mix(hc, l.haze, fogk(z, l) * 0.9);
        c = mix(c, hc, a);
        // farm lights at night
        if (night && i >= 2) {
            let x = world_x(p.x, z, t);
            let cell = floor(x / 140.0);
            let h = hash_cell2(vec2i(i32(cell), i), 0xfa7u);
            if (h.x < 0.4) {
                let lx = x - (cell + h.y) * 140.0;
                let ly = (p.y - HY) * z / FOC + EYE - 2.5;
                let r = max(0.6, ctx.px * z / FOC);
                c += col_kelvin(2300.0) * 1.2 * exp(-(lx * lx + ly * ly) / (r * r)) * sq(0.6 / r);
            }
        }
    }
    // telegraph poles and their wires, close by: box-blurred over a frame
    {
        let z = 9.0;
        let x = world_x(p.x, z, t);
        let span = 64.0;
        let cell = floor(x / span);
        let h = hash_cell2(vec2i(i32(cell), 0), 0x7e1u);
        let dx = x - (cell + 0.5) * span;
        let blur = SPEED * 0.033 + ctx.px * z / FOC;
        let w = 0.15;
        let cov = max(0.0, min(dx + blur * 0.5, w) - max(dx - blur * 0.5, -w)) / blur;
        let yh = (p.y - HY) * z / FOC + EYE;
        var pole = cov * step(h.x, 0.85) * step(yh, EYE + 2.4);
        // cross-arm
        let arm = max(0.0, min(dx + blur * 0.5, 1.1) - max(dx - blur * 0.5, -1.1)) / blur;
        pole = max(pole, arm * step(abs(yh - EYE - 2.15), 0.1) * step(h.x, 0.85));
        // three wires sagging between poles
        let u = dx / (span * 0.5);
        var wire = 0.0;
        for (var k = 0; k < 3; k++) {
            let hw = EYE + 1.3 + f32(k) * 0.3 - 0.55 * (1.0 - u * u);
            let wy = scr_y(hw, z + 0.2);
            wire = max(wire, 0.45 * exp(-sq((p.y - wy) / max(ctx.px * 0.7, 0.0015))));
        }
        var pc = col_hex(0x1a120cu) * l.land * 0.5;
        if (night) { pc = vec3f(0.002); }
        c = mix(c, pc, saturate(pole) * 0.95);
        c = mix(c, pc, wire * select(1.0, 0.5, night));
    }
    // snow streaming past the glass
    if (bare) {
        c = mix(c, vec3f(0.95, 0.96, 1.0), flurry(p, ctx));
    }
    return c;
}

// snowflakes rushing past the glass: short soft streaks drifting down
fn flurry(p: vec2f, ctx: Ctx) -> f32 {
    var acc = 0.0;
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let sc = 9.0 + fi * 7.0;
        let q = p * sc + vec2f(ctx.t * (1.6 - fi * 0.35) * sc, ctx.t * 0.25 * sc);
        let c = vec2i(floor(q));
        let h = hash_cell2(c, 0x5eedu + u32(i) * 17u);
        if (h.w < 0.22) {
            let d = fract(q) - (0.25 + 0.5 * h.xy);
            let len = 0.14 + 0.12 * h.z;
            let w = max(0.035, ctx.px * sc * 0.5);
            let along = (d.x + d.y * 0.3) / len;
            acc += exp(-along * along - sq(d.y / w)) * (0.8 - fi * 0.2);
        }
    }
    return saturate(acc);
}

// ------------------------------------------------------------------ inside
// window aperture: signed distance (negative inside the glass), repeated
fn win_d(p: vec2f) -> f32 {
    let q = vec2f(p.x - PITCH * round(p.x / PITCH), p.y) - WIN_C;
    return sdf2_round_box(q, WIN_H, WIN_R);
}

// seat back in front of us, on the right of each window: distance
fn seat_d(p: vec2f) -> f32 {
    let q = vec2f(p.x - PITCH * round(p.x / PITCH), p.y) - vec2f(0.9, -0.2);
    return sdf2_round_box(q, vec2f(0.21, 0.52), 0.11);
}

// What the glass reflects: the far side of the carriage, in radiance. Its
// ceiling strip and seat row run parallel to the window, so in the mirror
// they stay horizontal and all at one distance (smaller than our own frame).
fn reflection(p: vec2f, l: Look, ctx: Ctx) -> vec3f {
    let inside = l.inside;
    let lightc = select(col_hex(0xfff0dcu), col_hex(0xeef2ffu), l.mode == 2u);
    // lit lining of the far wall
    var r = col_hex(0xc4c5c6u) * inside * 0.9;
    // the far windows: dark, in a row, with a lighter reveal
    let ox = fmod_pos(p.x + 0.23, 0.86) - 0.43;
    let od = sdf2_round_box(vec2f(ox, p.y - 0.12), vec2f(0.3, 0.12), 0.05);
    r = mix(r, col_hex(0xd4d3d0u) * inside * 1.1, smoothstep(0.03, 0.0, od));
    r = mix(r, l.sky_lo * 0.05 + vec3f(0.002), smoothstep(0.008, -0.004, od));
    // luggage rack and the ceiling light strip above it
    r = mix(r, col_hex(0x8a8c90u) * inside * 0.7, smoothstep(0.012, 0.004, abs(p.y - 0.26)));
    r += lightc * 5.0 * exp(-sq((p.y - 0.315) / 0.009));
    r += lightc * 0.25 * exp(-sq((p.y - 0.315) / 0.05));
    // seat backs across the aisle: blue with cream headrest covers
    let sx = fmod_pos(p.x + 0.05, 0.42) - 0.21;
    let sb = sdf2_round_box(vec2f(sx, p.y + 0.2), vec2f(0.15, 0.2), 0.07);
    let seat = smoothstep(0.012, -0.012, sb);
    let cover = seat * smoothstep(-0.075, -0.065, p.y);
    r = mix(r, col_hex(0x34488au) * inside * 1.0, seat);
    r = mix(r, col_hex(0xf2ecdeu) * inside * 1.5, cover);
    return r;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let wd = win_d(p);
    let sd = seat_d(p);
    let aa = ctx.px;
    var col: vec3f;

    // --- the view out
    if (wd < aa * 2.0) {
        var o = outside(p, l, ctx);
        // glass: slight green tint, faint grime, the carriage reflected
        o *= vec3f(0.92, 0.96, 0.93);
        let grime = noise_fbm2(p * vec2f(6.0, 9.0) + 11.0, 4);
        let sunglow = exp(-length(p - l.sun) * 4.0);
        o += l.sun_c * 0.03 * grime * sunglow * select(1.0, 0.0, l.mode == 1u);
        o += reflection(p, l, ctx) * l.refl;
        if (l.mode == 2u) {
            // snow caught in the lower corners of the glass
            let q = vec2f(p.x - PITCH * round(p.x / PITCH), p.y) - WIN_C;
            let bank = -WIN_H.y + 0.03 + 0.025 * noise_value2(vec2f(q.x * 14.0, 1.0)) + 0.06 * sq(abs(q.x) / WIN_H.x);
            o = mix(o, col_hex(0xe6ebf5u) * 0.75, smoothstep(0.004, -0.004, q.y - bank));
        }
        col = o;
    }

    // --- the carriage
    if (wd > -aa) {
        let q = vec2f(p.x - PITCH * round(p.x / PITCH), p.y);
        let inside = l.inside;
        // light grey wall lining, warmer and brighter near the window
        let near = exp(-max(wd, 0.0) * 5.0);
        let sunk = select(0.0, 1.0, l.mode != 1u);
        var wall = col_hex(0xc4c5c6u) * (inside * (0.9 + 0.2 * p.y) + l.sun_c * 0.01 * near * sunk);
        if (l.mode == 1u) { wall = col_hex(0xd2d2d0u) * inside * (1.0 + 0.4 * smoothstep(0.1, 0.5, p.y)); }
        // the window reveal: a moulded frame ~0.05 deep, lit from outside
        let rev = smoothstep(0.055, 0.045, wd) * smoothstep(-aa, aa, wd);
        let dir = q - WIN_C;
        let bottom = smoothstep(0.0, -0.2, dir.y);
        let top = smoothstep(0.1, 0.25, dir.y);
        var revc = col_hex(0xd4d3d0u) * (inside * 1.2 + (l.sky_lo * 0.12 + l.sun_c * 0.02) * sunk * (0.4 + 0.8 * bottom - 0.3 * top));
        if (l.mode == 1u) { revc = col_hex(0xd4d3d0u) * inside * (1.25 - 0.5 * bottom); }
        wall = mix(wall, revc, rev);
        // rubber gasket
        wall = mix(wall, vec3f(0.01), smoothstep(0.012, 0.006, wd) * smoothstep(-aa, 0.0, wd));
        // luggage rack along the top: dark shelf with a lit lip
        let rack = smoothstep(0.43, 0.435, p.y) * smoothstep(0.55, 0.545, p.y);
        wall = mix(wall, col_hex(0x8a8c90u) * inside * (0.8 + 0.6 * smoothstep(0.44, 0.5, p.y)), rack);
        wall *= 1.0 - 0.35 * smoothstep(0.43, 0.36, p.y) * smoothstep(0.3, 0.42, p.y);
        wall += col_hex(0xfff0dcu) * inside * 2.0 * exp(-sq((p.y - 0.435) / 0.004));
        if (p.y > 0.55) { wall = col_hex(0xe6e2d8u) * inside * (1.3 + 0.5 * smoothstep(0.7, 0.62, p.y)); }
        // table below the window: grey laminate, a hard sun patch cast
        // through the glass
        if (p.y < -0.34) {
            let tb = col_hex(0x9c9890u);
            var tl = inside * 1.1;
            if (l.mode != 1u) {
                let sx = q.x + (p.y + 0.34) * 1.2 - 0.12;
                let spot = smoothstep(0.03, 0.0, abs(sx) - 0.45) * smoothstep(-0.36, -0.37, p.y) * smoothstep(-0.52, -0.46, p.y);
                tl += mix(l.sun_c, vec3f(max3(l.sun_c)), 0.35) * 0.18 * spot * select(1.0, 0.25, l.mode == 2u);
            }
            wall = tb * tl;
            wall = mix(wall, col_hex(0x2a2826u) * inside, smoothstep(-0.338, -0.345, p.y) * smoothstep(-0.36, -0.35, p.y));
            if (p.y < -0.62) { wall *= 0.35; }
        }
        let a = saturate(0.5 + wd / aa);
        col = mix(col, wall, select(a, 1.0, wd > aa * 2.0));
    }

    // --- the seat back in front, overlapping the frame: mostly in its own
    // shadow, rim-lit along the window side and the top
    let sa = saturate(0.5 - sd / aa);
    if (sa > 0.0) {
        let q = vec2f(p.x - PITCH * round(p.x / PITCH), p.y) - vec2f(0.9, -0.2);
        let day = l.mode != 1u;
        // moquette: deep blue, a fine woven check
        let weave = 0.9 + 0.1 * step(0.5, fract(q.x * 70.0)) + 0.08 * step(0.5, fract(q.y * 70.0));
        let fabric = col_hex(0x3c5290u) * weave;
        // form: rounder toward the edges, darker low down
        let across = q.x / 0.21;
        let form = (0.75 + 0.25 * (1.0 - across * across)) * (0.55 + 0.45 * smoothstep(-0.7, 0.3, q.y));
        var light = l.inside * select(1.4, 1.25, day) * form * (0.75 + 0.35 * smoothstep(0.24, -0.24, q.x));
        // window light wraps the near edge and the top
        let edge = smoothstep(-0.06, -0.004, sd);
        let rimk = edge * (smoothstep(0.0, -0.22, q.x) + smoothstep(0.3, 0.5, q.y));
        let rim = (l.sun_c * 0.05 + l.sky_lo * 0.08) * rimk * select(0.25, 1.0, day);
        var sc = fabric * light + fabric * rim * 7.0 + rim * 0.02;
        // cream headrest cover with a soft shadow under its hem
        let cover = smoothstep(0.35, 0.36, q.y) * smoothstep(0.205, 0.195, abs(q.x));
        let hem = exp(-sq((q.y - 0.345) / 0.012)) * smoothstep(0.2, 0.17, abs(q.x));
        let cc = col_hex(0xf2ecdeu) * (l.inside * 1.5 * (0.8 + 0.2 * smoothstep(0.24, -0.24, q.x)) + rim * 1.5);
        sc = mix(sc * (1.0 - 0.35 * hem), cc, cover);
        col = mix(col, sc, sa);
    }
    return col * exp2(l.exposure);
}
