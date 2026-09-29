//! name: windowrain
//! title: Rain on the Window
//! category: weather
//! tags: rain, city, night, bokeh, window, neon
//! desc: a rain-streaked window at night, the wet city street beyond melted into bokeh
//! themes: city, dusk, neon
//! uses: noise, light
//! cost: medium
//! fallback: rain
//! credits: original

// The lens is focused on the glass, half a metre away. Everything outside
// (a wet street running away from a first-floor window) is so far beyond
// the focus plane that every light spreads into a disc of nearly the same
// size, as a real photograph renders it. Drops on the glass are little
// lenses: each shows a sharp, inverted, minified image of the street, so
// the light that the street spreads into discs is gathered back into
// pinpoints. Condensation fogs the pane and haloes the lights, except where
// sliding drops have wiped clear trails.
//
// Street space `q` is the unrefracted view (the same units as `p`). World
// units are metres; the camera stands CAM_H above the road.

struct Look {
    sky_hi: vec3f,
    sky_lo: vec3f,
    bld: vec3f,       // building mass
    road: vec3f,      // wet asphalt base
    lamp_a: vec3f,    // sodium street lamps
    lamp_b: vec3f,    // LED street lamps
    led: f32,         // fraction of LED lamps
    shop: vec3f,      // shopfront glow at street level
    neon_a: vec3f,
    neon_b: vec3f,
    neon: f32,        // vertical neon sign amount
    win: f32,         // fraction of lit windows
    fog: f32,         // condensation on the glass
    room: vec3f,      // interior light: frame, reflections, fog veil
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            // blue hour: the sky still holds light, the lamps just came on
            l = Look(vec3f(0.012, 0.024, 0.07), vec3f(0.035, 0.055, 0.12), vec3f(0.004, 0.006, 0.013),
                vec3f(0.006, 0.009, 0.018), col_kelvin(2000.0) * 2.6, col_kelvin(4000.0) * 2.2, 0.3,
                col_kelvin(2900.0) * 1.0, vec3f(1.0, 0.22, 0.1), vec3f(0.25, 1.0, 0.5), 0.0, 0.3, 0.75,
                col_kelvin(2900.0) * 0.01, 0.0);
        }
        case 2u: {
            // a narrow street of vertical signs, magenta and cyan
            l = Look(vec3f(0.006, 0.004, 0.012), vec3f(0.024, 0.01, 0.032), vec3f(0.003, 0.0025, 0.005),
                vec3f(0.005, 0.004, 0.008), col_kelvin(2300.0) * 1.8, col_kelvin(6500.0) * 2.0, 0.7,
                col_kelvin(4200.0) * 0.8, vec3f(1.0, 0.07, 0.4), vec3f(0.04, 0.7, 1.0), 1.0, 0.35, 1.0,
                col_kelvin(3400.0) * 0.009, 0.1);
        }
        default: {
            // sodium-lit low cloud, warm lamps, cool LEDs, busy traffic
            l = Look(vec3f(0.006, 0.0055, 0.007), vec3f(0.03, 0.018, 0.011), vec3f(0.003, 0.003, 0.0038),
                vec3f(0.005, 0.005, 0.006), col_kelvin(1900.0) * 2.8, col_kelvin(4300.0) * 2.4, 0.35,
                col_kelvin(2900.0) * 1.0, vec3f(1.0, 0.1, 0.06), vec3f(0.2, 1.0, 0.4), 0.0, 0.45, 1.0,
                col_kelvin(2800.0) * 0.01, 0.1);
        }
    }
    return l;
}

const VP: vec2f = vec2f(0.12, 0.02);  // vanishing point of the street
const FOC: f32 = 0.95;                // focal length in p units
const CAM_H: f32 = 5.5;

fn proj(w: vec3f) -> vec2f { return VP + FOC * vec2f(w.x, w.y - CAM_H) / w.z; }

// Accumulates one light: a disc of radius r and edge softness fw (with the
// slightly bright rim of a real lens) into `acc`, and a wide scattering halo
// (what condensation does to it) into `halo`.
struct Lit {
    acc: vec3f,
    halo: vec3f,
}

fn add_disc(s: ptr<function, Lit>, q: vec2f, c: vec2f, r: f32, fw: f32, col: vec3f) {
    let d = length(q - c);
    (*s).halo += col * (0.004 / (0.004 + d * d));
    if (d < r + fw) {
        var body = sstep(r + fw, r - fw, d);
        // cat's eye: toward the frame edges the lens barrel clips the disc
        let off = c * (0.55 * r);
        body *= sstep(r + fw, r - fw, length(q - c + off));
        (*s).acc += col * body * (0.85 + 0.25 * sstep(r * 0.55, r, d));
    }
}

// a blurred vertical reflection on the wet road, from y0 (top) down to y1,
// broken up by ripples
fn add_streak(s: ptr<function, Lit>, q: vec2f, x: f32, y0: f32, y1: f32, r: f32, fw: f32, col: vec3f) {
    let yy = clamp(q.y, y1, y0);
    let d = length(vec2f(q.x - x, q.y - yy));
    if (d < r + fw) {
        let along = saturate((q.y - y1) / max(y0 - y1, 1e-4));
        let rip = 0.6 + 0.4 * noise_value2(vec2f(x * 40.0, q.y * 60.0));
        (*s).acc += col * sstep(r + fw, r - fw, d) * along * along * rip;
    }
}

// staircase of building heights along z, steps softened by the blur width
fn roofline(z: f32, w: f32, row: f32) -> f32 {
    let s = z / 13.0;
    let f = fract(s);
    let k = floor(s) + sstep(1.0 - min(w, 1.0), 1.0, f);
    let n = noise_value2(vec2f(k, row));
    let tall = 10.0 + 30.0 * n * n * n + 3.0 * noise_value2(vec2f(s * 3.0, row + 2.0));
    // far away the steps are finer than the blur: only the average is left
    return mix(tall, 17.0, saturate(w * 1.5 - 0.5));
}

// The street at q. r = bokeh radius, fw = edge softness (q units), gain
// scales the lights (a drop gathers a disc's energy into a point).
fn street(q: vec2f, r: f32, fw: f32, gain: f32, l: Look, ctx: Ctx) -> Lit {
    var s = Lit(vec3f(0.0), vec3f(0.0));
    let dy = q.y - VP.y;
    let dx = q.x - VP.x;
    // ---- sky and building masses (big, soft shapes)
    let sky = mix(l.sky_lo, l.sky_hi, saturate(dy * 1.8 + 0.05));
    var col = sky;
    let left = dx < 0.0;
    let side = select(10.0, -11.0, left);
    let row = select(3.0, 17.0, left);
    let zf = FOC * abs(side) / max(abs(dx), 1e-3);
    let fz_w = r * zf * zf / (FOC * abs(side));   // blur footprint along z
    let fy_w = r * zf / FOC;                      // and in height
    let roof = roofline(zf, saturate(fz_w / 13.0), row);
    let yroof = VP.y + FOC * (roof - CAM_H) / zf;
    let fy = CAM_H + dy * zf / FOC;
    var facade = l.bld * (0.7 + 0.6 * noise_value2(vec2f(zf / 13.0, row + 5.0)));
    // windows: a filtered grid on the facade (every 3.4 m, floors 3.3 m)
    let cz = zf / 3.4;
    let cy = (fy - 4.4) / 3.3;
    let cell = vec2i(i32(floor(cz)), i32(floor(cy)));
    let hw = hash_cell2(cell + vec2i(select(0, 5000, left), 0), 0x51edu);
    let on = select(0.0, 0.35 + 0.9 * hw.y, hw.x < l.win);
    let fx = abs(fract(cz) - 0.5) * 3.4;
    let fyy = abs(fract(cy) - 0.5) * 3.3;
    let win_a = sstep(0.7 + fz_w, 0.7 - fz_w, fx) * sstep(0.75 + fy_w, 0.75 - fy_w, fyy);
    // once the blur spans several windows only their average survives
    let spread = saturate(max(fz_w / 3.4, fy_w / 3.3) - 0.3);
    let wtone = mix(col_kelvin(2500.0), col_kelvin(3800.0), hw.z) * select(1.0, 0.6, hw.w > 0.9);
    let wl = mix(on * win_a, l.win * 0.25, spread) * step(0.0, cy) * saturate((roof - fy) * 0.6);
    facade += wtone * wl * 0.03;
    // shopfronts: a lit band at street level, continuous along the street
    let shop_n = noise_value2(vec2f(zf / 6.0, row + 9.0));
    let shop_band = sstep(3.8 + fy_w, 3.0 - fy_w, fy) * sstep(0.3 - fy_w, 0.9 + fy_w, fy);
    facade += l.shop * shop_band * (0.004 + 0.05 * sstep(0.35, 0.7, shop_n));
    let wall = sstep(r * 0.7, -r * 0.7, q.y - yroof);
    col = mix(col, facade, wall);
    // road below the kerb line
    let yground = VP.y - FOC * CAM_H / zf;
    let road_m = sstep(yground + r, yground - r, q.y);
    let zr = FOC * CAM_H / max(-dy, 1e-3);
    let road_c = l.road + sky * 0.25 * exp(-zr * 0.01);
    col = mix(col, road_c, road_m);

    // ---- street lamps both sides, 7.5 m high, every 17 m
    for (var sd = 0; sd < 2; sd++) {
        let sx = select(7.8, -8.6, sd == 0);
        for (var k = 0; k < 11; k++) {
            let hj = hash_cell2(vec2i(k, sd), 0x1a3fu);
            if (hj.w < 0.14 && k > 1) { continue; } // a dead lamp
            let z = 9.0 + f32(k) * 17.0 + f32(sd) * 8.0 + (hj.x - 0.5) * 6.0;
            let c = proj(vec3f(sx + (hj.y - 0.5) * 0.8, 7.0 + 1.2 * hj.z, z));
            let hk = hash_f(u32(k * 2 + sd) * 0x9e37u + 11u);
            let tint = select(l.lamp_a, l.lamp_b, hk < l.led);
            let rr = r * (0.85 + 0.3 * exp(-z * 0.04));
            let bright = 0.55 / (1.0 + z * 0.018);
            add_disc(&s, q, c, rr, fw, tint * bright * gain);
            // the wet-road reflection under it
            let g0 = proj(vec3f(sx * 0.9, 0.0, z)).y;
            let g1 = proj(vec3f(sx * 0.9, -5.0, z * 0.75)).y;
            add_streak(&s, q, c.x * 0.96 + VP.x * 0.04, g0, g1, rr * 0.75, fw, tint * bright * 0.1 * gain);
        }
    }
    // ---- lit windows close enough to be discs of their own, and the
    // distant city packed around the end of the street
    for (var k = 0; k < 14; k++) {
        let hw2 = hash_cell2(vec2i(k, 3), 0xd15cu);
        let lf = hw2.x < 0.5;
        let z = 14.0 + 70.0 * hw2.y * hw2.y;
        let c = proj(vec3f(select(10.0, -11.0, lf), 4.5 + floor(hw2.z * 5.0) * 3.3, z));
        let wt = mix(col_kelvin(2500.0), col_kelvin(3600.0), hw2.w) * select(vec3f(1.0), vec3f(0.5, 0.65, 1.0), hw2.w > 0.88);
        add_disc(&s, q, c, r * 0.9, fw, wt * (0.08 + 0.1 * hw2.w) * l.win * 2.0 * gain);
    }
    for (var k = 0; k < 10; k++) {
        let hf = hash_cell2(vec2i(k, 4), 0xfa5u);
        let c = VP + vec2f((hf.x - 0.5) * 0.5, -0.01 + 0.09 * hf.y * hf.y);
        let wt = mix(col_kelvin(2200.0), col_kelvin(5000.0), hf.z);
        add_disc(&s, q, c, r * 0.8, fw, wt * (0.05 + 0.08 * hf.w) * gain);
    }
    // ---- traffic light (right kerb, 30 s cycle: green, amber, red)
    {
        let c = proj(vec3f(6.2, 5.2, 30.0));
        let ph = fract(ctx.t / 30.0 + 0.3);
        var tl = vec3f(0.08, 1.0, 0.4);
        if (ph > 0.47) { tl = vec3f(1.0, 0.42, 0.02); }
        if (ph > 0.56) { tl = vec3f(1.0, 0.03, 0.015); }
        add_disc(&s, q, c, r * 0.9, fw, tl * 0.5 * gain);
        let g0 = proj(vec3f(6.2, 0.0, 30.0)).y;
        let g1 = proj(vec3f(6.2, -5.0, 23.0)).y;
        add_streak(&s, q, c.x, g0, g1, r * 0.7, fw, tl * 0.1 * gain);
    }
    // ---- traffic: oncoming headlights (left lane), taillights (right lane)
    for (var i = 0; i < 6; i++) {
        let lane_in = i < 3;
        let hc = hash_cell2(vec2i(i, 0), 0xca5u);
        let period = 13.0 + 6.0 * hc.x;
        let ev = hash_event(ctx.t + hc.y * period, period, u32(i) * 0x3c6ef372u + 5u);
        if (ev.x > 0.72) { continue; } // an empty slot on the road
        var z = mix(170.0, 6.0, ev.y);
        if (!lane_in) { z = mix(7.0, 190.0, ev.y); }
        let lx = select(1.9, -1.9, lane_in);
        let hy = select(0.9, 0.72, lane_in);
        var tint = col_kelvin(5000.0) * 1.3;
        if (!lane_in) { tint = vec3f(1.0, 0.04, 0.025) * 0.8; }
        let bright = saturate(1.25 - z * 0.005) * gain;
        for (var e = 0; e < 2; e++) {
            let ex = lx + select(-0.72, 0.72, e == 1);
            let c = proj(vec3f(ex, hy, z));
            add_disc(&s, q, c, r * 0.95, fw, tint * bright);
            let g1 = proj(vec3f(ex, -hy * 5.0, z * 0.7)).y;
            add_streak(&s, q, c.x, c.y, g1, r * 0.7, fw, tint * bright * 0.16);
        }
    }
    // ---- vertical signs sticking out of the facades
    if (l.neon > 0.0) {
        for (var k = 0; k < 8; k++) {
            let right = (k & 1) == 1;
            let hn = hash_cell2(vec2i(k, 9), 0x5e7u);
            let z = 11.0 + f32(k) * 7.0 + hn.x * 4.0;
            let sx = select(-10.2, 9.3, right);
            let y0 = 4.5 + hn.y * 2.0;
            let y1 = y0 + 3.5 + hn.z * 5.0;
            let a = proj(vec3f(sx, y0, z));
            let b = proj(vec3f(sx, y1, z));
            let w = FOC * 0.3 / z;
            var tint = select(l.neon_a, l.neon_b, hn.w > 0.5);
            if (hn.w > 0.86) { tint = col_kelvin(3200.0); }
            // an old tube that flickers now and then
            let fl = select(1.0, light_flicker(ctx.t, u32(k) + 3u, 0.8), hn.x > 0.8);
            let d = length(vec2f(q.x - a.x, q.y - clamp(q.y, a.y, b.y)));
            let tcol = tint * (0.3 + 0.15 * hn.z) * fl * gain;
            s.halo += tcol * (0.006 / (0.006 + d * d));
            if (d < w + r + fw) {
                // letters: the sign is not a flat bar
                let letters = 0.75 + 0.25 * sin((q.y - a.y) / max(b.y - a.y, 1e-3) * 22.0);
                s.acc += tcol * sstep(w + r + fw, w + r - fw, d) * letters;
            }
            let g0 = proj(vec3f(sx * 0.95, 0.0, z)).y;
            let g1 = proj(vec3f(sx * 0.95, -y1 * 0.6, z * 0.8)).y;
            add_streak(&s, q, a.x, g0, g1, w + r * 0.7, fw, tcol * 0.12);
        }
    }
    // ---- shop signs at eye level: a green pharmacy cross, a red sign
    {
        let gsgn = proj(vec3f(-10.6, 4.3, 17.0));
        add_disc(&s, q, gsgn, r * 1.05, fw, l.neon_b * 0.35 * gain * (1.0 - l.neon));
        let rsgn = proj(vec3f(9.6, 4.1, 21.0));
        add_disc(&s, q, rsgn, r * 1.1, fw, l.neon_a * 0.3 * gain);
        add_disc(&s, q, rsgn + vec2f(0.035, 0.0), r * 1.1, fw, l.neon_a * 0.25 * gain);
    }
    s.acc += col;
    s.halo *= 0.05;
    return s;
}

// ---------------------------------------------------------------- glass

struct Drop {
    m: f32,        // coverage 0..1 (soft edge)
    c: vec2f,      // centre
    n: vec2f,      // position inside the drop, unit-disc coordinates
    r: f32,
}

fn pick(best: Drop, cand: Drop) -> Drop {
    if (cand.m > best.m) { return cand; }
    return best;
}

// A drop with a slightly heavier bottom; one-pixel soft edge.
fn drop_at(p: vec2f, c: vec2f, r: f32, ctx: Ctx) -> Drop {
    var d = p - c;
    d.y *= select(1.15, 0.92, d.y < 0.0);
    let len = length(d);
    let m = saturate((r - len) / max(ctx.px * 0.9, 1e-4) + 0.5);
    return Drop(m, c, d / max(r, 1e-5), r);
}

// Beads sitting on the glass, two size classes. Each lives 25-70 s: it
// appears in an instant (a raindrop landing) and slowly evaporates.
fn beads(p: vec2f, ctx: Ctx) -> Drop {
    var best = Drop(0.0, vec2f(0.0), vec2f(0.0), 0.0);
    for (var k = 0; k < 2; k++) {
        let small = k == 1;
        let cs = select(0.12, 0.05, small);
        let q = p / cs;
        let c = vec2i(floor(q));
        let h = hash_cell2(c, 0x9e37u + u32(k) * 31u);
        let period = 25.0 + 45.0 * h.z;
        let life = fract(ctx.t / period + h.w);
        let slot = floor(ctx.t / period + h.w);
        let hp = hash_cell2(c + vec2i(i32(slot) * 7, 0), 0x2b1u + u32(k));
        if (hp.w > select(0.4, 0.38, small)) { continue; }
        let r0 = select(0.012 + 0.03 * hp.z * hp.z, 0.004 + 0.009 * hp.z, small);
        let r = r0 * sstep(0.0, 0.003, life) * sstep(1.0, 0.7, life);
        let ctr = (vec2f(c) + 0.3 + 0.4 * hp.xy) * cs;
        best = pick(best, drop_at(p, ctr, r, ctx));
    }
    return best;
}

// Stick-slip: a drop sits, then runs, then stops as it sheds water.
fn slide(t: f32, period: f32) -> f32 {
    let u = t / period;
    let f = saturate(fract(u) / 0.4);
    return floor(u) + 1.0 - pow(1.0 - f, 3.0);
}

fn path_x(y: f32, a: f32, b: f32) -> f32 {
    return 0.006 * sin(y * 7.0 + a * 6.0) + 0.0025 * sin(y * 23.0 + b * 5.0);
}

struct Slide {
    d: Drop,
    clear: f32,
}

// Sliding drops, one per column per pass, each wiping a clear trail that
// fogs back over, with tiny beads left behind in it.
fn sliders(p: vec2f, ctx: Ctx) -> Slide {
    var best = Drop(0.0, vec2f(0.0), vec2f(0.0), 0.0);
    var clear = 0.0;
    let top = ctx.half.y + 0.12;
    let span = 2.0 * top + 0.6;          // the travel plus a pause off-frame
    for (var layer = 0; layer < 2; layer++) {
        let cw = select(0.15, 0.21, layer == 1);
        let ox = select(0.0, 0.071, layer == 1);
        let col = floor((p.x + ox) / cw);
        let hc = hash_cell2(vec2i(i32(col), layer), 0x5d1du);
        let period = 2.5 + 5.0 * hc.x;
        let stepd = 0.04 + 0.07 * hc.y;
        let dist = slide(ctx.t + hc.z * 40.0, period) * stepd + hc.w * span;
        let cyc = floor(dist / span);
        let hd = hash_cell2(vec2i(i32(col), i32(cyc)), 0x71f3u + u32(layer));
        if (hd.w > 0.6) { continue; }
        let yd = top - (dist - cyc * span);
        let r = select(0.016 + 0.01 * hd.z, 0.022 + 0.012 * hd.z, layer == 1);
        let x0 = (col + 0.3 + 0.4 * hd.x) * cw - ox;
        best = pick(best, drop_at(p, vec2f(x0 + path_x(yd, hd.y, hd.x), yd), r, ctx));
        // the trail above the drop
        let above = p.y - yd;
        if (above > 0.0) {
            let tw = r * 0.7;
            let dx = abs(p.x - (x0 + path_x(p.y, hd.y, hd.x)));
            let fade = exp(-above / 0.4);
            clear = max(clear, sstep(tw + ctx.px, tw * 0.5 - ctx.px, dx) * fade * 0.8);
            // tiny beads left behind in the trail
            let bs = 0.04;
            let bi = floor(above / bs);
            let hb = hash_cell2(vec2i(i32(col) * 131 + layer, i32(bi) + i32(cyc) * 97), 0xbeadu);
            if (hb.w < 0.6) {
                let by = yd + (bi + 0.3 + 0.4 * hb.y) * bs;
                let bc = vec2f(x0 + path_x(by, hd.y, hd.x) + (hb.x - 0.5) * tw, by);
                best = pick(best, drop_at(p, bc, r * (0.16 + 0.16 * hb.z) * (0.5 + 0.5 * fade), ctx));
            }
        }
    }
    return Slide(best, clear);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    // window frame: a vertical mullion every 1.7 units (one at x = -0.72),
    // a rail above and below (only tall portrait frames reach them)
    let mxl = fmod_pos(p.x + 0.72 + 0.85, 1.7) - 0.85;
    let myl = abs(p.y) - 0.7;
    let frame_d = min(abs(mxl) - 0.026, abs(myl) - 0.026);
    // ---- glass
    let sl = sliders(p, ctx);
    let clear = sl.clear;
    var bd = beads(p, ctx);
    bd.m *= 1.0 - clear;
    let d = pick(bd, sl.d);
    let near_frame = exp(-max(min(abs(mxl), abs(myl)) - 0.026, 0.0) * 10.0);
    // condensation: patchy, heavier low on the pane and against the frame
    let fog_n = noise_fbm2(p * vec2f(2.0, 2.6) + vec2f(4.0, 1.0), 4);
    var fog = l.fog * saturate(0.25 + 0.7 * fog_n + 0.2 * sstep(0.1, -0.5, p.y) + 0.35 * near_frame);
    fog *= 1.0 - clear;
    let r_bk = 0.04;
    let fw = r_bk * (0.08 + 0.35 * fog) + ctx.px * 0.5;
    let outer = street(p, r_bk, fw, 1.0, l, ctx);
    // fogged glass: contrast drops, lights halo, the room's light veils it
    let veil = l.room * 0.9 + l.sky_lo * 0.2;
    var col = mix(outer.acc, outer.acc * 0.55 + outer.halo + veil, fog);
    if (d.m > 0.0) {
        // through a drop: inverted, minified, in focus. The discs gather
        // back into bright points.
        let spread = 0.22;
        let qd = d.c - d.n * spread;
        let pxd = ctx.px * spread / max(d.r, 1e-3);
        let ri = max(0.008, pxd * 0.7);
        let gain = min(sq(r_bk / ri), 4.0);
        let inner = street(qd, ri, pxd * 0.8, gain, l, ctx);
        let edge = length(d.n);
        // the rim bends in light from far off-axis: a dark ring
        var dc = inner.acc * (1.0 - 0.7 * sstep(0.72, 1.0, edge)) * 0.9;
        // a glint of the room lamp, upper left
        let g = d.n - vec2f(-0.38, 0.42);
        dc += l.room * 2.5 * exp(-dot(g, g) * 22.0);
        col = mix(col, dc, d.m);
    }
    // faint reflection of the lamp behind us in the pane
    col += l.room * 0.25 * exp(-length(p - vec2f(-0.42, 0.3)) * 4.0);
    // ---- frame (in focus: it sits at the glass)
    let fm = sstep(ctx.px, -ctx.px, frame_d);
    let lit_edge = sstep(0.026, 0.0, abs(mxl - 0.02)) * sstep(0.0, 0.03, abs(myl));
    let fcol = l.room * (0.35 + 1.2 * lit_edge * lit_edge) + vec3f(0.0015, 0.0015, 0.002);
    col = mix(col, fcol, fm);
    return col * exp2(l.exposure);
}
