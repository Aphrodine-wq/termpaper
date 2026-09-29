//! name: fireplace
//! title: Cabin Fireplace
//! category: cozy
//! tags: fire, cabin, stone, snow, winter, warm, interior
//! desc: a crackling fire in a fieldstone hearth, snow falling past the cabin window
//! themes: snow, rain, autumn
//! uses: sdf, fire, snow, rain, noise
//! cost: light
//! fallback: campfire
//! credits: original

// A log cabin at night. The room is drawn in screen space but every surface
// carries a pseudo-3D position (x, y on screen, z toward the viewer) and a
// normal, lit by the fire. Its flicker is weighted by distance, so the hearth
// breathes while the far walls hold steady (a whole room flickering would
// repaint every terminal cell every frame).
//
// Past the frame the room goes on: coats and snowshoes on a peg rail, then a
// second window with an oil lantern on its sill (a steady second light, so
// the far wall never falls to black), a blanket chest under it, joists across
// the ceiling above and the floorboards below. Far left, a shelf with a small
// oil lamp, crocks and a skillet.

const FIRE: vec2f = vec2f(0.12, -0.215);       // base of the flames
const OPEN_C: vec2f = vec2f(0.12, -0.16);      // firebox opening centre
const OPEN_H: vec2f = vec2f(0.19, 0.14);       // opening half size
const FLOOR_Y: f32 = -0.36;
const CEIL_Y: f32 = 0.51;                      // wall meets ceiling
const VP: vec2f = vec2f(0.1, 0.05);            // vanishing point of floor and ceiling
const WIN_C: vec2f = vec2f(-0.6, 0.1);
const WIN_H: vec2f = vec2f(0.17, 0.2);
const WIN2_C: vec2f = vec2f(2.24, 0.1);        // the second window, beyond the frame
const LANT: vec2f = vec2f(2.1, -0.08);         // lantern flame, on that window's sill
const PEG_Y: f32 = 0.235;                      // peg rail right of the armchair
const OIL: vec2f = vec2f(-1.5, 0.215);         // oil lamp flame, on the far-left shelf
const SHELF_Y: f32 = 0.16;                     // top of that shelf

// The entry point continues past the frame's edge as a cylinder, which suits
// a 3D camera; this room is painted flat, so undo it and keep the desk's own
// scale there (a window stays square on a portrait monitor beside the row).
// Identity inside the frame.
fn flat_p(p: vec2f, ctx: Ctx) -> vec2f {
    let hx = ctx.half.x;
    let a = abs(p.x);
    if (a <= hx) { return p; }
    return vec2f(sign(p.x) * (hx + (atan(a) - atan(hx)) * (1.0 + hx * hx)), p.y);
}

struct Look {
    mode: u32,        // 0 snow, 1 rain, 2 autumn
    fill: vec3f,      // cool/warm light from the window side
    amb: vec3f,       // room ambient
    sky_top: vec3f,
    sky_low: vec3f,
    land: vec3f,      // ground outside
    trees: vec3f,
    fire: f32,        // fire intensity
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, col_hex(0x6a7a96u) * 0.014, vec3f(0.008, 0.0085, 0.01),
                        col_hex(0x1a2230u) * 0.35, col_hex(0x3a4452u) * 0.3, col_hex(0x1c2420u) * 0.12,
                        col_hex(0x0c1210u) * 0.1, 1.0, 1.0);
        }
        case 2u: {
            return Look(2u, col_hex(0xffb070u) * 0.03, vec3f(0.012, 0.009, 0.007),
                        col_hex(0x5a6a98u) * 0.5, col_hex(0xffa060u) * 1.2, col_hex(0x6a5a30u) * 0.3,
                        col_hex(0x8a3a14u) * 0.35, 0.85, 0.9);
        }
        default: {
            return Look(0u, col_hex(0x6d8ac8u) * 0.035, vec3f(0.008, 0.009, 0.012),
                        col_hex(0x0c1630u) * 0.5, col_hex(0x2a3c64u) * 0.5, col_hex(0x9eb2d8u) * 0.28,
                        col_hex(0x08101cu) * 0.4, 1.0, 1.0);
        }
    }
}

// fire colour and slow breathing; `near` weights the flicker toward the hearth
fn hearth_c() -> vec3f { return vec3f(1.0, 0.58, 0.28); }

// Light at a pseudo-3D point with normal n. flick: fire_light(t) - 1.
fn hearth_lit(pos: vec3f, n: vec3f, l: Look, flick: f32) -> vec3f {
    // the fire's light spills out of the opening into the room
    let lp = vec3f(FIRE.x + 0.01, FIRE.y + 0.08, -0.22);
    let d = lp - pos;
    let dist2 = dot(d, d);
    let ld = d * inverseSqrt(dist2);
    let near = exp(-sqrt(dist2) * 3.0);
    let k = l.fire * (1.0 + flick * (0.25 + 0.75 * near));
    let direct = max(dot(n, ld), 0.0) * 0.05 / (dist2 + 0.03);
    // warm bounce off the floor and walls fills the room
    let bounce = 0.045 / (1.0 + dist2 * 3.0);
    return hearth_c() * k * (direct + bounce);
}

// the oil lantern: steady (a wick behind glass barely moves), warm, small
fn lant_c() -> vec3f { return vec3f(1.0, 0.64, 0.32); }

fn lantern_lit(pos: vec3f, n: vec3f, l: Look) -> vec3f {
    let d = vec3f(LANT, -0.1) - pos;
    let dist2 = dot(d, d);
    let ld = d * inverseSqrt(dist2);
    // the cap throws a soft shadow upward
    let up = 1.0 - 0.5 * smoothstep(0.2, 0.9, -ld.y);
    // (windowed so it fades out before reaching the frame)
    return lant_c() * l.fire * (max(dot(n, ld), 0.0) * 0.04 * up / (dist2 + 0.015) + 0.032 / (1.0 + dist2 * 4.0)) * exp(-dist2 * 0.9);
}

// the small oil lamp far left: a short reach, and none of it toward the
// frame (so the approved composition is untouched)
fn oil_lit(pos: vec3f, n: vec3f, l: Look) -> vec3f {
    let d = vec3f(OIL, -0.06) - pos;
    let dist2 = dot(d, d);
    let ld = d * inverseSqrt(dist2);
    let reach = exp(-dist2 * 5.0) * exp(-sq(max(pos.x - OIL.x - 0.35, 0.0)) * 25.0);
    return lant_c() * l.fire * (max(dot(n, ld), 0.0) * 0.03 / (dist2 + 0.012) + 0.025 / (1.0 + dist2 * 4.0)) * reach;
}

// Everything away from the hearth: the fire (with its distance-weighted
// flicker), the lanterns, and a warm floor of bounce light that fades slowly
// with distance so the far corners stay readable, never black.
fn room_lit(pos: vec3f, n: vec3f, l: Look, flick: f32) -> vec3f {
    let dx = abs(pos.x - FIRE.x);
    let far = hearth_c() * l.fire * 0.016 * smoothstep(0.92, 1.6, abs(pos.x)) / (1.0 + 0.15 * dx * dx + 0.6 * sq(pos.y - FIRE.y));
    return hearth_lit(pos, n, l, flick) + lantern_lit(pos, n, l) + oil_lit(pos, n, l) + far;
}

// Far left: a plank shelf on brackets with a small oil lamp and two
// stoneware crocks, a cast-iron skillet hung on a nail below.
fn shelf_left(p: vec2f, col_in: vec3f, l: Look, flick: f32, ctx: Ctx) -> vec3f {
    var col = col_in;
    // skillet on its nail
    let sk = p - vec2f(-1.36, -0.07);
    let pan = min(sdf2_circle(sk, 0.062), sdf2_round_box(sk - vec2f(0.0, 0.095), vec2f(0.011, 0.045), 0.006));
    let pa = aa_fill(pan, ctx);
    if (pa > 0.0) {
        let rim = smoothstep(-0.012, -0.004, sdf2_circle(sk, 0.062)) * step(sk.y, 0.06);
        let n = normalize(vec3f(sk.x * 5.0, sk.y * 5.0, -1.0));
        let pc = vec3f(0.012) * (room_lit(vec3f(p, -0.03), n, l, flick) + l.amb) + lant_c() * l.fire * 0.01 * rim * smoothstep(-0.1, 0.05, sk.y);
        col = mix(col, pc, pa);
    }
    // the shelf board and its two brackets
    let board = sdf2_box(p - vec2f(-1.4, SHELF_Y - 0.012), vec2f(0.24, 0.012));
    let brk = min(sdf2_segment(p, vec2f(-1.58, SHELF_Y - 0.02), vec2f(-1.58, SHELF_Y - 0.1)), sdf2_segment(p, vec2f(-1.22, SHELF_Y - 0.02), vec2f(-1.22, SHELF_Y - 0.1))) - 0.009;
    let ba = aa_fill(min(board, brk), ctx);
    if (ba > 0.0) {
        let top = smoothstep(SHELF_Y - 0.006, SHELF_Y, p.y);
        let n = normalize(vec3f(0.0, mix(-0.4, 1.0, top), -1.0));
        let bc = col_hex(0x5a3c22u) * 0.5 * (0.85 + 0.2 * noise_value2(vec2f(p.x * 30.0, p.y * 200.0))) * (room_lit(vec3f(p, -0.05), n, l, flick) + l.amb);
        col = mix(col, bc, ba);
    }
    // two crocks: cream stoneware with brown-glazed shoulders
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        let c = vec2f(-1.34 + fk * 0.075, SHELF_Y + 0.045 - fk * 0.012);
        let hs = 0.045 - fk * 0.012;
        let q = p - c;
        let body = sdf2_round_box(q, vec2f(0.026 - fk * 0.004, hs), 0.018);
        let neck = sdf2_box(q - vec2f(0.0, hs + 0.006), vec2f(0.01, 0.008));
        let ca = aa_fill(min(body, neck), ctx);
        if (ca > 0.0) {
            var alb = mix(col_hex(0xc8b890u), col_hex(0x6a3a1cu), smoothstep(hs * 0.35, hs * 0.5, q.y)) * 0.5;
            let n = normalize(vec3f(q.x / 0.026 * 0.8, 0.2, -1.0));
            col = mix(col, alb * (room_lit(vec3f(p, -0.08), n, l, flick) + l.amb), ca);
        }
    }
    // the oil lamp: glass font, a tall chimney, the flame inside
    let q = p - OIL;
    let font = sdf2_round_box(q - vec2f(0.0, -0.035), vec2f(0.022, 0.016), 0.012);
    let foot = sdf2_box(q - vec2f(0.0, -0.052), vec2f(0.016, 0.004));
    let chim = sdf2_round_box(q - vec2f(0.0, 0.012), vec2f(0.011 + 0.005 * exp(-sq(q.y / 0.012)), 0.034), 0.006);
    let fa = aa_fill(min(font, foot), ctx);
    if (fa > 0.0) {
        col = mix(col, col_hex(0x8a6a3au) * 0.4 * (room_lit(vec3f(p, -0.08), vec3f(0.0, 0.3, -1.0), l, flick) + l.amb) + lant_c() * l.fire * 0.03, fa);
    }
    let cha = aa_fill(chim, ctx);
    if (cha > 0.0) {
        let fq = q * vec2f(1.0, 0.5);
        let fl = exp(-dot(fq, fq) / 0.00005);
        col = mix(col, col * 0.6 + lant_c() * l.fire * (0.18 + 1.5 * fl), cha);
    }
    return col;
}

// ------------------------------------------------------------------ materials
// cellular stones: (F1, F2, cell id hash) at p
fn stones(p: vec2f) -> vec3f {
    let i = vec2i(floor(p));
    let f = fract(p);
    var d1 = 8.0;
    var d2 = 8.0;
    var id = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2i(x, y);
            let h = hash_cell2(i + o, 0x570eu);
            let r = vec2f(o) + 0.15 + 0.7 * h.xy - f;
            // stones are wider than tall
            let dd = length(r * vec2f(0.8, 1.25));
            if (dd < d1) { d2 = d1; d1 = dd; id = h.z; } else if (dd < d2) { d2 = dd; }
        }
    }
    return vec3f(d1, d2, id);
}

// fieldstone wall: (albedo rgb, height) — rounded stones in deep mortar
fn stone_wall(p: vec2f) -> vec4f {
    let s = stones(p * vec2f(9.0, 9.0));
    let edge = s.y - s.x;
    let h = sqrt(saturate(edge * 2.2));
    let tone = s.z;
    var alb = mix(col_hex(0x7a6e62u), col_hex(0x9a9286u), tone);
    alb = mix(alb, col_hex(0x6a5a48u), step(0.8, tone));
    alb = mix(alb, col_hex(0x8c8a84u), step(tone, 0.15));
    alb *= 0.75 + 0.35 * noise_value2(p * 60.0);
    let mortar = smoothstep(0.07, 0.02, edge);
    alb = mix(alb, col_hex(0x5a5248u) * 0.8, mortar);
    return vec4f(alb * 0.55, h);
}

// horizontal logs of the cabin wall: (albedo, log-local v, chinking)
fn log_wall(p: vec2f) -> vec3f {
    let lh = 0.085;
    let v = fract(p.y / lh);
    let idx = floor(p.y / lh);
    let hl = hash_f(u32(idx + 100.0));
    let chink = smoothstep(0.1, 0.04, v) + smoothstep(0.9, 0.96, v);
    return vec3f(v, chink, hl);
}

// ------------------------------------------------------------------ outside
fn outside(p: vec2f, wc: vec2f, l: Look, ctx: Ctx) -> vec3f {
    let q = p - wc;
    var c = mix(l.sky_low, l.sky_top, smoothstep(-0.05, 0.2, q.y));
    if (l.mode == 0u) {
        // moonlit clearing: firs at two distances on bright snow
        var sc = c;
        let gnd = -0.1 + 0.012 * sin(p.x * 11.0);
        for (var layer = 0; layer < 2; layer++) {
            let fl = f32(layer);
            let w = 0.05 - fl * 0.02;
            let cx = (floor(p.x / w) + 0.5) * w;
            let h = hash_cell2(vec2i(i32(floor(p.x / w)), layer), 0xf12u);
            let tall = (0.09 + 0.08 * h.x) * (1.0 - fl * 0.45);
            let base = gnd + 0.005 + fl * 0.02;
            let yy = q.y - base;
            let half = w * (0.35 + 0.15 * h.y) * saturate(1.0 - yy / tall);
            let tier = 1.0 - 0.25 * fract(yy / tall * 4.0);
            if (yy > 0.0 && yy < tall && abs(p.x - cx - (h.z - 0.5) * w * 0.3) < half * tier && h.w < 0.8) {
                sc = mix(col_hex(0x0a1426u) * 0.3, l.sky_low * 0.6, fl * 0.5);
            }
        }
        if (q.y < gnd) { sc = l.land * (0.85 + 0.15 * noise_value2(p * 30.0)) * (0.8 + 0.4 * smoothstep(gnd - 0.1, gnd, q.y)); }
        let s = snow_flakes(p * 1.8, ctx, 0.5, 0.01, 3);
        return mix(sc, col_hex(0xdce6f8u) * 0.45, s * 0.9);
    }
    // distant tree line, snowy/wet/autumn ground
    let tl = -0.07 + 0.05 * noise_fbm2(vec2f(p.x * 14.0, 0.5), 3) + 0.04 * saturate(sin(p.x * 60.0) * 0.5 + 0.5) * select(1.0, 0.3, l.mode == 2u);
    let gnd = -0.12 + 0.01 * sin(p.x * 9.0);
    if (q.y < tl) { c = l.trees; }
    if (l.mode == 2u && q.y < tl + 0.03 && q.y > gnd) {
        // autumn crowns: patches of orange, rust and yellow
        let n = noise_fbm2(p * 40.0, 3);
        let crown = mix(col_hex(0xb4541cu), col_hex(0xd8a032u), n) * 0.35;
        c = mix(c, crown, smoothstep(0.45, 0.55, n + 0.1 * (tl - q.y) * 20.0) * step(q.y, tl + 0.02));
    }
    if (q.y < gnd) { c = l.land * (0.8 + 0.2 * noise_value2(p * 30.0)); }
    // weather
    if (l.mode == 0u) {
        let s = snow_flakes(p, ctx, 0.55, 0.02, 3);
        c = mix(c, col_hex(0xdce6f8u) * 0.5, s);
    } else if (l.mode == 2u) {
        // a few leaves drifting down
        let lq = vec2f(p.x * 12.0 + sin(ctx.t * 0.7 + p.y * 9.0) * 0.4, p.y * 12.0 + ctx.t * 0.9);
        let lc = vec2i(floor(lq));
        let lh = hash_cell2(lc, 0x1eafu);
        if (lh.w < 0.12) {
            let d = length((fract(lq) - 0.3 - 0.4 * lh.xy) * vec2f(1.0, 1.8));
            c = mix(c, mix(col_hex(0xc0501cu), col_hex(0xe0a030u), lh.z) * 0.5, smoothstep(0.12, 0.07, d));
        }
    }
    return c;
}

// ------------------------------------------------------------------ beyond the frame
// Board ceiling on round joists, in perspective: only a portrait monitor (or
// one standing beside the row) looks up far enough to see it.
fn ceiling(p: vec2f, l: Look, flick: f32, aa: f32) -> vec3f {
    let z = (CEIL_Y - VP.y) * 0.29 / max(p.y - VP.y, 0.01);   // 0.29 at the wall, smaller toward us
    let wx = (p.x - VP.x) * z;
    let bi = floor(wx / 0.11);
    let bf = fract(wx / 0.11);
    let hb = hash_f(u32(bi + 700.0));
    let lod = saturate(1.0 - aa * z * 25.0);
    let seam = smoothstep(0.05, 0.0, min(bf, 1.0 - bf)) * lod;
    var alb = mix(col_hex(0x6a4a30u), col_hex(0x80603eu), hb) * 0.4 * (0.85 + 0.2 * noise_value2(vec2f(wx * 20.0, z * 4.0)));
    alb *= 1.0 - 0.55 * seam;
    var n = vec3f(0.0, -1.0, 0.0);
    // joists across the room every 0.045 of depth, hanging below the boards
    let jz = (0.29 - z) / 0.045;
    let u = fract(jz) - 0.5;
    let jw = 0.17;
    let beam = smoothstep(jw + 0.03, jw, abs(u)) * step(0.5, jz);
    if (beam > 0.0) {
        let ja = col_hex(0x5a3a22u) * 0.45 * (0.85 + 0.2 * noise_value2(vec2f(p.x * 10.0, jz * 30.0))) * (0.7 + 0.3 * (1.0 - sq(u / jw)));
        alb = mix(alb, ja, beam);
        n = normalize(mix(n, vec3f(0.0, -1.0, -u / jw * 1.4), beam));
    } else {
        // the boards just behind each joist sit in its shadow
        alb *= 0.6 + 0.4 * smoothstep(0.0, 0.25, u + 0.5 - jw) * step(0.5, jz) + 0.4 * step(jz, 0.5);
    }
    let pos = vec3f(p.x, CEIL_Y + 0.02, -(0.29 - z) * 0.8);
    return alb * (room_lit(pos, n, l, flick) * 2.5 + l.amb);
}

// Pegs right of the armchair: a buffalo-check coat, a striped scarf, a pair
// of snowshoes. Returns (colour, coverage).
fn pegs(p: vec2f, l: Look, flick: f32, ctx: Ctx) -> vec4f {
    let aa = ctx.px;
    var c = vec3f(0.0);
    var a = 0.0;
    // the rail and its three pegs
    let rail = sdf2_box(p - vec2f(1.34, PEG_Y), vec2f(0.28, 0.014));
    let ra = aa_fill(rail, ctx);
    if (ra > 0.0) {
        let rn = normalize(vec3f(0.0, (p.y - PEG_Y) / 0.014 * 0.8, -1.0));
        c = col_hex(0x4a2e18u) * 0.45 * room_lit(vec3f(p, -0.01), rn, l, flick);
        a = ra;
    }
    // coat: shoulders, a long body flaring to the hem, folds down its length
    let cq = p - vec2f(1.15, 0.0);
    let hw = mix(0.082, 0.064, saturate((cq.y + 0.25) / 0.4));
    let body = max(abs(cq.x) - hw, max(cq.y - 0.16, -cq.y - 0.27));
    let coat = min(op_smin(body, sdf2_round_box(cq - vec2f(0.0, 0.17), vec2f(0.07, 0.05), 0.04), 0.03),
                   sdf2_circle(cq - vec2f(0.0, 0.215), 0.022));
    let ca = aa_fill(coat, ctx);
    if (ca > 0.0) {
        let chk = step(0.5, fract((p.x - 1.15) * 15.0 + 0.25)) + step(0.5, fract(p.y * 15.0));
        var alb = mix(col_hex(0xb8301cu), col_hex(0x1a0e0au), chk * 0.42) * 0.6;
        let fold = (0.8 + 0.2 * sin(cq.x * 70.0 + 1.5) * smoothstep(0.12, -0.1, cq.y))
                 * (1.0 - 0.6 * smoothstep(0.008, 0.0, abs(abs(cq.x) - 0.045)) * step(cq.y, 0.13) * step(-0.06, cq.y));
        let cn = normalize(vec3f(cq.x / hw * 0.8, 0.25, -1.0));
        let edge = smoothstep(0.0, -0.02, coat);
        let lit = alb * fold * (0.55 + 0.45 * edge) * (room_lit(vec3f(p, -0.06), cn, l, flick) + l.amb);
        c = mix(c, lit, ca);
        a = max(a, ca);
    }
    // scarf: two tails hanging from the middle peg, green bands on cream
    let sq_ = p - vec2f(1.345, 0.0);
    let tails = min(sdf2_round_box(sq_ - vec2f(-0.022, 0.07), vec2f(0.016, 0.16), 0.006),
                    sdf2_round_box(sq_ - vec2f(0.02, 0.1), vec2f(0.016, 0.13), 0.006));
    let sa = aa_fill(tails, ctx);
    if (sa > 0.0) {
        let band = step(0.5, fract(p.y * 16.0));
        var alb = mix(col_hex(0xd0c4a4u), col_hex(0x3a7a5au), band) * 0.5;
        alb *= 0.8 + 0.2 * step(0.5, fract(p.x * 120.0));
        let lit = alb * (room_lit(vec3f(p, -0.04), vec3f(0.0, 0.2, -1.0), l, flick) + l.amb);
        c = mix(c, lit, sa);
        a = max(a, sa);
    }
    // snowshoes: two bent-ash teardrops laced with rawhide, hung by a strap
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        let o = vec2f(1.515 + fk * 0.045, 0.02 - fk * 0.012);
        let q = rot2(0.1 - fk * 0.2) * (p - o);
        let e = length(q / vec2f(0.058, 0.15)) - 1.0;
        let tail = sdf2_segment(q, vec2f(0.0, -0.14), vec2f(0.0, -0.24)) - 0.006;
        let outer = min(e * 0.058, tail);
        let ea = aa_fill(outer, ctx);
        if (ea > 0.0) {
            let rim = aa_fill(abs(e * 0.058) - 0.006, ctx);
            let lace = max(smoothstep(0.2, 0.0, abs(fract((q.x + q.y) * 34.0) - 0.5) - 0.3),
                           smoothstep(0.2, 0.0, abs(fract((q.x - q.y) * 34.0) - 0.5) - 0.3));
            // the wall shows through the open lacing
            var alb = mix(col_hex(0xc8b088u) * 0.4, col_hex(0xb08850u) * 0.55, max(rim, step(0.0, e)));
            let lit = alb * (room_lit(vec3f(p, -0.05), vec3f(0.0, 0.1, -1.0), l, flick) + l.amb);
            let cov = ea * max(max(rim, step(0.0, e)), lace * 0.85 * saturate(1.5 - aa * 60.0));
            c = mix(c, lit, cov);
            a = max(a, cov);
        }
    }
    // the strap over the peg
    let strap = sdf2_segment(p, vec2f(1.52, PEG_Y), vec2f(1.535, 0.16)) - 0.004;
    let sta = aa_fill(strap, ctx);
    c = mix(c, col_hex(0x2a1a10u) * 0.3 * (room_lit(vec3f(p, -0.03), vec3f(0.0, 0.0, -1.0), l, flick) + l.amb), sta);
    a = max(a, sta);
    // peg knobs in front of everything
    let knob = min(min(sdf2_circle(p - vec2f(1.15, PEG_Y), 0.012), sdf2_circle(p - vec2f(1.345, PEG_Y), 0.012)), sdf2_circle(p - vec2f(1.525, PEG_Y), 0.012));
    let ka = aa_fill(knob, ctx);
    c = mix(c, col_hex(0x5a3a20u) * 0.5 * (room_lit(vec3f(p, -0.03), normalize(vec3f(0.0, 0.5, -1.0)), l, flick) + l.amb), ka);
    a = max(a, ka);
    return vec4f(c, a);
}

// The sill, the lantern standing on it, the blanket chest below.
fn window2_props(p: vec2f, col_in: vec3f, l: Look, flick: f32, ctx: Ctx) -> vec3f {
    let aa = ctx.px;
    var col = col_in;
    // sill: a thick plank with a lit top face
    let sy = WIN2_C.y - WIN_H.y - 0.035;
    let sd = sdf2_box(p - vec2f(WIN2_C.x, sy - 0.012), vec2f(0.24, 0.016));
    if (sd < aa) {
        let top = smoothstep(sy - 0.004, sy + 0.002, p.y);
        let n = normalize(vec3f(0.0, mix(-0.3, 1.2, top), -1.0));
        let sc = col_hex(0x5a3c22u) * 0.5 * (0.85 + 0.2 * noise_value2(vec2f(p.x * 30.0, p.y * 200.0))) * (room_lit(vec3f(p, -0.06), n, l, flick) + l.amb);
        col = mix(col, sc, aa_fill(sd, ctx));
    }
    // blanket chest on the floor, iron straps, a folded quilt on the lid
    let chest = sdf2_round_box(p - vec2f(WIN2_C.x + 0.02, -0.325), vec2f(0.22, 0.075), 0.01);
    if (chest < aa) {
        let lid = smoothstep(-0.26, -0.257, p.y);
        let n = normalize(vec3f(0.0, mix(0.35, 1.0, lid), -1.0));
        let plank = 0.85 + 0.15 * step(0.5, fract((p.y + 0.3) / 0.045));
        var alb = col_hex(0x7a4828u) * 0.55 * plank * (0.85 + 0.2 * noise_value2(vec2f(p.x * 25.0, p.y * 90.0)));
        let strap = step(abs(abs(p.x - WIN2_C.x - 0.02) - 0.13), 0.012);
        alb = mix(alb, vec3f(0.02), strap);
        var cc = alb * (room_lit(vec3f(p, -0.12), n, l, flick) + l.amb);
        cc += lant_c() * l.fire * 0.01 * strap * smoothstep(-0.3, -0.2, p.y);   // iron catches the lantern
        // shadow at the foot where it meets the floor
        cc *= 0.55 + 0.45 * smoothstep(-0.4, -0.36, p.y);
        col = mix(col, cc, aa_fill(chest, ctx));
    }
    let quilt = sdf2_round_box(p - vec2f(WIN2_C.x + 0.08, -0.232), vec2f(0.12, 0.024), 0.01);
    if (quilt < aa) {
        let cell = vec2i(floor(vec2f(p.x * 22.0, (p.y + 0.256) * 60.0)));
        let hq = hash_cell2(cell, 0x9017u);
        var qc = col_hex(0xb8ac90u);
        if (hq.x < 0.3) { qc = col_hex(0x8a2a1eu); } else if (hq.x < 0.5) { qc = col_hex(0x2a4a6au); } else if (hq.x < 0.62) { qc = col_hex(0xb07a2au); }
        let fold = 0.75 + 0.25 * smoothstep(-0.256, -0.216, p.y);
        let lit = qc * 0.4 * fold * (room_lit(vec3f(p, -0.14), normalize(vec3f(0.0, 0.6, -1.0)), l, flick) + l.amb);
        col = mix(col, lit, aa_fill(quilt, ctx));
    }
    // the lantern: tin base and cap, a glass chimney, a wire bail
    let q = p - LANT;
    let glass = sdf2_round_box(q - vec2f(0.0, 0.0), vec2f(0.026, 0.036), 0.014);
    let base = sdf2_round_box(q - vec2f(0.0, -0.043), vec2f(0.034, 0.01), 0.004);
    let cap = min(sdf2_round_box(q - vec2f(0.0, 0.043), vec2f(0.03, 0.008), 0.004), sdf2_box(q - vec2f(0.0, 0.056), vec2f(0.012, 0.008)));
    let bail = abs(length((q - vec2f(0.0, 0.05)) * vec2f(1.0, 1.2)) - 0.04) - 0.0025;
    let tin = min(min(base, cap), max(bail, -(q.y - 0.05)));
    let gm = aa_fill(glass, ctx);
    if (gm > 0.0) {
        // the flame and the glass glowing around it
        let fq = (q - vec2f(0.0, -0.012)) * vec2f(1.0, 0.55);
        let fl = exp(-dot(fq, fq) / 0.00012);
        let gc = lant_c() * l.fire * (0.35 + 1.6 * fl + 0.25 * exp(-dot(q, q) / 0.0012));
        col = mix(col, gc, gm);
    }
    let tm = aa_fill(tin, ctx);
    if (tm > 0.0) {
        let tc = col_hex(0x3a3a38u) * 0.3 * (room_lit(vec3f(p, -0.12), normalize(vec3f(0.0, 0.3, -1.0)), l, flick) + l.amb) + lant_c() * l.fire * 0.012 * smoothstep(0.05, 0.02, abs(q.y));
        col = mix(col, tc, tm);
    }
    return col;
}

// ------------------------------------------------------------------ scene
fn scene(pw: vec2f, ctx: Ctx) -> vec3f {
    let p = flat_p(pw, ctx);
    let l = look(ctx.theme);
    let t = ctx.t;
    let flick = (fire_light(t * 0.7) - 0.95) * 1.4;
    let aa = ctx.px;
    var col = vec3f(0.0);

    // ---- walls: log cabin wall everywhere, stone chimney breast in the middle
    let breast = p.x > -0.28 && p.x < 0.52 && p.y > FLOOR_Y;
    if (p.y > FLOOR_Y) {
        let lw = log_wall(p);
        let v = lw.x;
        // round log: normal bulges toward the viewer across its height
        let ny = (v - 0.5) * 1.6;
        let n = normalize(vec3f(0.0, ny, -1.0));
        let grain = 0.8 + 0.25 * noise_value2(vec2f(p.x * 18.0 + lw.z * 40.0, p.y * 90.0)) + 0.1 * sin(p.x * 70.0 + lw.z * 9.0);
        var alb = mix(col_hex(0x7a5634u), col_hex(0x9c7448u), lw.z) * 0.45 * grain;
        alb = mix(alb, col_hex(0xb0a080u) * 0.35, lw.y);
        let pos = vec3f(p, 0.02);
        // window light: from the first window over the left wall, from the
        // second over the far wall
        let fill = smoothstep(0.1, -0.7, p.x) + 0.8 * exp(-abs(p.x - WIN2_C.x) * 2.5) * smoothstep(1.0, 1.6, p.x);
        col = alb * (room_lit(pos, n, l, flick) + l.amb * (1.0 + ny * 0.5) + l.fill * fill * 0.6);
        // ceiling beam
        if (p.y > 0.43) {
            let bv = saturate((p.y - 0.43) / 0.08);
            let bn = normalize(vec3f(0.0, -1.0 + bv, -1.0));
            // beyond the frame, where the ceiling shows above it, the beam
            // turns its lit underside to the lantern
            let far_k = smoothstep(1.0, 1.9, abs(p.x));
            let ba = mix(col_hex(0x3a2616u) * 0.4, col_hex(0x5a3a22u) * 0.45, far_k) * (0.85 + 0.2 * noise_value2(vec2f(p.x * 12.0, p.y * 60.0)));
            col = ba * (room_lit(vec3f(p, -0.05), bn, l, flick) * (0.8 + 1.7 * far_k * (1.0 - bv)) + l.amb);
            if (p.y > CEIL_Y) { col = ceiling(p, l, flick, aa); }
        }
    }
    if (breast && p.y < 0.43) {
        let sw = stone_wall(p);
        // dome normal from the stone height field
        let e = 0.004;
        let hx = stone_wall(p + vec2f(e, 0.0)).w - sw.w;
        let hy = stone_wall(p + vec2f(0.0, e)).w - sw.w;
        let n = normalize(vec3f(-hx * 1.6, -hy * 1.6, -e * 3.0));
        let pos = vec3f(p, -0.04 - 0.02 * sw.w);
        col = sw.rgb * (hearth_lit(pos, n, l, flick) + l.amb * 1.1 + l.fill * 0.2);
        // soot above the opening
        let soot = exp(-sq((p.x - OPEN_C.x) / 0.16)) * smoothstep(OPEN_C.y + OPEN_H.y - 0.02, OPEN_C.y + OPEN_H.y + 0.1, p.y) * smoothstep(0.06, 0.0, p.y);
        col *= 1.0 - 0.6 * soot;
        // the breast's side edges are a little darker (it stands proud of the wall)
        col *= 0.75 + 0.25 * smoothstep(0.0, 0.02, min(p.x + 0.28, 0.52 - p.x));
    }

    // ---- windows with their view out (the second one lies beyond the frame)
    let second = p.x > 0.9;
    let wc = select(WIN_C, WIN2_C, second);
    let wq = p - wc;
    let wd = sdf2_box(wq, WIN_H);
    if (wd < 0.035) {
        // plank trim around the window, lit a little by the fire
        let trim = col_hex(0x5a3c22u) * 0.45 * (room_lit(vec3f(p, -0.02), vec3f(0.3, 0.0, -1.0), l, flick) + l.amb + l.fill * 0.8);
        col = mix(col, trim, aa_fill(wd - 0.035, ctx));
    }
    if (wd < 0.0) {
        var o = outside(p, wc, l, ctx);
        if (l.mode == 1u) {
            let rg = rain_glass(p * 1.4, t, ctx);
            o = outside(p + rg.xy * 0.6, wc, l, ctx) * (1.0 + rg.z * 0.4);
        }
        // the glass holds a faint reflection of the fire; the second, of
        // the lantern standing in front of it
        if (second) {
            let rl = exp(-sq((p.x - LANT.x - 0.015) / 0.05) - sq((p.y - LANT.y - 0.03) / 0.07));
            o += lant_c() * rl * 0.06 * l.fire;
        } else {
            let rf = exp(-sq((p.x - (WIN_C.x + 0.07)) / 0.035) - sq((p.y - (WIN_C.y - 0.12)) / 0.04));
            o += hearth_c() * rf * 0.05 * l.fire * (1.0 + flick * 0.5);
        }
        // muntins: 2 x 3 panes
        let mx = abs(fract((wq.x + WIN_H.x) / (WIN_H.x * 2.0 / 2.0)) - 0.5);
        let my = abs(fract((wq.y + WIN_H.y) / (WIN_H.y * 2.0 / 3.0)) - 0.5);
        let mun = max(1.0 - smoothstep(0.018, 0.03, mx * WIN_H.x * 2.0 / 2.0 * 2.0 * 0.5), 1.0 - smoothstep(0.012, 0.02, my * WIN_H.y * 2.0 / 3.0 * 2.0 * 0.5));
        let munc = col_hex(0x4a3220u) * 0.4 * (l.amb * 2.0 + l.fill * 1.5 + room_lit(vec3f(p, -0.01), vec3f(0.3, 0.0, -1.0), l, flick) * 0.6);
        o = mix(o, munc, mun);
        // snow banked in the pane corners
        if (l.mode == 0u) {
            let py = fract((wq.y + WIN_H.y) / (WIN_H.y * 2.0 / 3.0));
            let px = fract((wq.x + WIN_H.x) / WIN_H.x);
            let bank = 0.03 + 0.05 * sq(abs(px - 0.5) * 2.0) + 0.012 * noise_value2(p * 90.0);
            let bottom_row = step(wq.y, -WIN_H.y + WIN_H.y * 2.0 / 3.0);
            o = mix(o, col_hex(0xd8e4f4u) * 0.35, smoothstep(0.02, -0.02, py - bank) * (1.0 - mun) * bottom_row);
        }
        col = mix(col, o, aa_fill(wd, ctx));
    }

    // ---- far left: the shelf, the oil lamp, the skillet
    if (p.x < -1.05 && p.x > -1.7 && p.y > -0.16 && p.y < 0.28) {
        col = shelf_left(p, col, l, flick, ctx);
    }

    // ---- coats and snowshoes on the peg rail
    if (p.x > 0.98 && p.x < 1.7 && abs(p.y - 0.0) < 0.3) {
        let pg = pegs(p, l, flick, ctx);
        col = mix(col, pg.rgb, pg.a);
    }

    // ---- mantel beam and what stands on it
    let md = sdf2_box(p - vec2f(0.12, 0.048), vec2f(0.36, 0.028));
    if (md < aa) {
        let v = (p.y - 0.02) / 0.056;
        let n = normalize(vec3f(0.0, mix(-1.2, 0.3, smoothstep(0.0, 0.35, v)), -1.0));
        let ma = col_hex(0x4a2e18u) * 0.5 * (0.85 + 0.25 * noise_value2(vec2f(p.x * 14.0, p.y * 140.0)));
        let mc = ma * (hearth_lit(vec3f(p, -0.09), n, l, flick) + l.amb);
        col = mix(col, mc, aa_fill(md, ctx));
    }
    {
        // a lantern on the left, a stack of books right, a small clock
        let lant = min(sdf2_round_box(p - vec2f(-0.14, 0.115), vec2f(0.022, 0.04), 0.008), sdf2_box(p - vec2f(-0.14, 0.165), vec2f(0.01, 0.01)));
        let books = min(sdf2_box(p - vec2f(0.33, 0.1), vec2f(0.05, 0.026)), sdf2_box(p - vec2f(0.34, 0.135), vec2f(0.04, 0.009)));
        let clock = sdf2_circle(p - vec2f(0.12, 0.12), 0.04);
        let obj = min(min(lant, books), clock);
        if (obj < aa) {
            var oc = col_hex(0x2a2018u) * 0.3;
            if (books < aa) { oc = mix(col_hex(0x5a1e18u), col_hex(0x1e3a2au), step(0.126, p.y)) * 0.35; }
            if (clock < aa) { oc = mix(col_hex(0x6a4a24u) * 0.4, col_hex(0xd8ccb0u) * 0.4, smoothstep(0.0, -0.01, clock + 0.01)); }
            let bottom = smoothstep(0.16, 0.08, p.y);
            let lit = oc * (hearth_lit(vec3f(p, -0.1), normalize(vec3f(0.0, -0.4, -1.0)), l, flick) * (0.4 + 0.6 * bottom) + l.amb);
            col = mix(col, lit, aa_fill(obj, ctx));
        }
    }

    // ---- the firebox opening
    let oq = p - OPEN_C;
    let arch = OPEN_H.y - 0.03 * (1.0 - sq(oq.x / OPEN_H.x));
    let od = max(abs(oq.x) - OPEN_H.x, max(-oq.y - OPEN_H.y, oq.y - arch));
    if (od < aa * 2.0) {
        // sooty back wall and splayed sides, glowing where the fire licks them
        let depth = saturate(1.0 - abs(oq.x) / OPEN_H.x);
        let sw = stone_wall(p * 1.6 + 3.0);
        var inner = sw.rgb * 0.35;
        let heat = exp(-sq((p.x - FIRE.x) / 0.12) - sq((p.y - FIRE.y - 0.05) / 0.1));
        inner *= 0.2 + (hearth_c() * l.fire * (1.0 + flick * 0.8)) * (0.6 * heat + 0.15 * depth) * 3.0;
        inner *= smoothstep(OPEN_C.y + arch + 0.02, OPEN_C.y - 0.02, p.y) * 0.8 + 0.2;
        col = mix(col, inner, aa_fill(od, ctx));
        // coal bed
        let bed = sdf2_round_box(p - vec2f(FIRE.x, FIRE.y - 0.07), vec2f(0.15, 0.018), 0.012);
        if (bed < aa) {
            let n = noise_value2(vec2f(p.x * 90.0, p.y * 160.0) + vec2f(0.0, floor(t * 0.5) * 0.0));
            let pulse = 0.75 + 0.25 * sin(t * 0.9 + n * 12.0);
            let glow = fire_temperature_color(0.25 + 0.25 * n * pulse) * l.fire * 0.8;
            col = mix(col, glow, aa_fill(bed, ctx));
        }
        // andirons
        let andi = min(sdf2_box(p - vec2f(FIRE.x - 0.13, FIRE.y - 0.03), vec2f(0.007, 0.05)), sdf2_box(p - vec2f(FIRE.x + 0.13, FIRE.y - 0.03), vec2f(0.007, 0.05)));
        col = mix(col, vec3f(0.004) + hearth_c() * 0.02, aa_fill(andi, ctx));
        // three logs: charred, glowing in their cracks
        for (var k = 0; k < 3; k++) {
            var a: vec2f;
            var b: vec2f;
            var r: f32;
            if (k == 0) { a = vec2f(0.0, -0.262); b = vec2f(0.24, -0.25); r = 0.026; }
            else if (k == 1) { a = vec2f(0.02, -0.21); b = vec2f(0.16, -0.25); r = 0.02; }
            else { a = vec2f(0.1, -0.245); b = vec2f(0.25, -0.2); r = 0.021; }
            let d = sdf2_segment(p - vec2f(FIRE.x - 0.12, 0.0), a, b) - r;
            if (d < aa) {
                let ba = b - a;
                let along = dot(p - vec2f(FIRE.x - 0.12, 0.0) - a, ba) / dot(ba, ba);
                let across = saturate(0.5 - d / (2.0 * r));
                let crack = smoothstep(0.7, 0.86, noise_value2(vec2f(along * 30.0 + f32(k) * 7.0, across * 4.0)));
                let pulse = 0.7 + 0.3 * sin(t * 0.8 + along * 9.0 + f32(k));
                var lc = col_hex(0x1a120cu) * 0.08 * (0.4 + across);
                lc += fire_temperature_color(0.28 + 0.12 * pulse) * crack * (0.3 + 0.7 * across) * l.fire * 0.8;
                // flames light the logs' upper sides; bark texture
                let bark = 0.7 + 0.3 * noise_value2(vec2f(along * 60.0, across * 12.0 + f32(k)));
                lc += hearth_c() * 0.05 * l.fire * smoothstep(0.35, 0.95, across) * bark;
                // sawn ends show pale wood rings where they face us
                let endd = length(p - vec2f(FIRE.x - 0.12, 0.0) - a) - r * 0.85;
                if (k == 0 && endd < 0.0) {
                    lc = col_hex(0x8a6a44u) * 0.25 * (0.8 + 0.2 * sin(length(p - vec2f(FIRE.x - 0.12, 0.0) - a) * 700.0)) * (0.3 + hearth_c() * 0.5 * l.fire);
                }
                col = mix(col, lc, aa_fill(d, ctx));
            }
        }
        // flames: a few overlapping tongues
        if (od < 0.0 || p.y > OPEN_C.y + arch - 0.02) {
            var fsum = vec3f(0.0);
            var asum = 0.0;
            for (var k = 0; k < 4; k++) {
                var off: vec2f;
                var sc: f32;
                if (k == 0) { off = vec2f(0.0, 0.0); sc = 0.36; }
                else if (k == 1) { off = vec2f(-0.07, -0.005); sc = 0.27; }
                else if (k == 2) { off = vec2f(0.075, -0.004); sc = 0.29; }
                else { off = vec2f(-0.11, -0.012); sc = 0.16; }
                let fq = (p - FIRE - off) / sc;
                if (abs(fq.x) < 0.3 && fq.y > -0.05 && fq.y < 0.75) {
                    let f = fire_flame(fq, t * 0.8 + f32(k) * 3.7, ctx);
                    fsum += f.rgb * f.a;
                    asum = max(asum, f.a);
                }
            }
            let inside = aa_fill(od, ctx);
            col = mix(col, col * 0.4 + fsum * l.fire, asum * inside) + fsum * l.fire * 0.15 * inside;
        }
    }

    // ---- hearth slab
    let hd = sdf2_box(p - vec2f(0.12, FLOOR_Y + 0.03), vec2f(0.42, 0.032));
    if (hd < aa) {
        let top = smoothstep(FLOOR_Y + 0.045, FLOOR_Y + 0.06, p.y);
        let n = normalize(vec3f(0.0, mix(-0.2, 1.0, top), -1.0));
        let ha = col_hex(0x6e665cu) * 0.5 * (0.8 + 0.3 * noise_value2(p * vec2f(20.0, 80.0))) * (0.85 + 0.15 * step(0.5, fract((p.x + 0.3) / 0.14)));
        let hc = ha * (hearth_lit(vec3f(p.x, FLOOR_Y + 0.05, -0.12 + (p.y - FLOOR_Y) * 0.5), n, l, flick) + l.amb);
        col = mix(col, hc, aa_fill(hd, ctx));
    }

    // ---- firewood stacked left of the hearth: log ends
    let wood_box = sdf2_box(p - vec2f(-0.38, -0.25), vec2f(0.085, 0.11));
    if (wood_box < 0.02 && p.y > FLOOR_Y) {
        let r = 0.024;
        let row = floor((p.y - FLOOR_Y) / (r * 1.75));
        let offx = select(0.0, r, (i32(row) & 1) == 1);
        let cx = floor((p.x + 0.5 - offx) / (r * 2.05));
        let c = vec2f((cx + 0.5) * r * 2.05 - 0.5 + offx, FLOOR_Y + (row + 0.5) * r * 1.75);
        let hw = hash_cell2(vec2i(i32(cx), i32(row)), 0xf00du);
        let rr = r * (0.82 + 0.18 * hw.x);
        let d = length(p - c) - rr;
        let inbox = sdf2_round_box(p - vec2f(-0.38, -0.25), vec2f(0.085, 0.11), 0.03) < 0.0;
        if (d < aa && inbox) {
            let rings = 0.85 + 0.15 * sin(length(p - c) * 420.0 + hw.y * 6.0);
            let bark = smoothstep(-0.005, 0.0, d + 0.002);
            var wc = mix(col_hex(0xb08858u) * rings, col_hex(0x3a2818u), bark) * 0.45;
            wc *= hearth_lit(vec3f(p, -0.1), vec3f(0.3, 0.1, -1.0), l, flick) + l.amb;
            col = mix(col, wc, aa_fill(d, ctx));
        } else if (inbox) {
            col *= 0.3;
        }
    }

    // ---- floor: wide planks receding to a vanishing point, a braided rug
    if (p.y < FLOOR_Y) {
        let vp = vec2f(0.1, 0.05);
        let z = 0.12 / max(vp.y - p.y, 0.01);
        let wx = (p.x - vp.x) * z;
        let plank = fract(wx / 0.2);
        let pid = floor(wx / 0.2);
        let hp = hash_f(u32(pid + 500.0));
        let seam = smoothstep(0.02, 0.0, min(plank, 1.0 - plank)) * saturate(1.0 - aa * z * 30.0);
        var fa = mix(col_hex(0x7a4e2cu), col_hex(0x9a6a3cu), hp) * 0.45 * (0.85 + 0.2 * noise_value2(vec2f(wx * 3.0, z * 0.6)));
        fa *= 1.0 - 0.5 * seam;
        let fpos = vec3f(p.x, FLOOR_Y, -(z - 0.3) * 0.25);
        var fc = fa * (room_lit(fpos, vec3f(0.0, 1.0, 0.0), l, flick) * 1.4 + l.amb + l.fill * 0.5 * smoothstep(0.0, -0.8, p.x));
        // glossy floor catches the firelight
        fc += hearth_c() * 0.02 * l.fire * exp(-sq((p.x - FIRE.x) / 0.12)) * smoothstep(FLOOR_Y - 0.12, FLOOR_Y, p.y) * (1.0 + flick * 0.5);
        // the rug: concentric braided rings
        let rq = vec2f(wx - 0.02, (z - 0.55) * 0.9);
        let rd = length(rq / vec2f(0.95, 0.42));
        if (rd < 1.0) {
            // seven broad rings, then (only seen close up, from a portrait
            // monitor) a border of narrow braids in the same colours
            let rb = select(rd * 7.0, 6.0 + (rd - 6.0 / 7.0) * 70.0, rd > 6.0 / 7.0);
            let ring = fract(rb);
            let band = u32(floor(rb));
            var rc = col_hex(0x7a3a2au);
            if (band % 4u == 1u) { rc = col_hex(0x9a8058u); }
            if (band % 4u == 2u) { rc = col_hex(0x3a4450u); }
            if (band % 4u == 3u) { rc = col_hex(0xb8ac90u); }
            if (band >= 6u) { rc = mix(rc, col_hex(0x8a5a40u), 0.65); }
            let braid = 0.85 + 0.15 * sin(atan2(rq.y, rq.x) * 60.0 + ring * 6.0) * saturate(1.0 - aa * z * 20.0);
            let rugc = mix(rc, vec3f(col_luma(rc)), 0.25) * 0.4 * braid * (room_lit(fpos, vec3f(0.0, 1.0, 0.0), l, flick) * 1.4 + l.amb);
            fc = mix(fc, rugc, smoothstep(1.0, 1.0 - clamp(aa * 20.0 * z * z + 0.002, 0.003, 0.03), rd));
        }
        col = fc;
    }

    // ---- the far wall's sill, lantern and chest
    if (abs(p.x - WIN2_C.x) < 0.3 && p.y < 0.0 && p.y > -0.42) {
        col = window2_props(p, col, l, flick, ctx);
    }
    // halo in the air around the lantern and the oil lamp
    let lg = length(p - LANT);
    col += lant_c() * l.fire * (0.05 * exp(-lg * 30.0) + 0.012 * exp(-lg * 6.0));
    let og = length((p - OIL) * vec2f(1.0, 0.8));
    col += lant_c() * l.fire * (0.035 * exp(-og * 32.0) + 0.006 * exp(-og * 7.0)) * exp(-sq(max(p.x - OIL.x - 0.3, 0.0)) * 25.0);

    // ---- leather armchair, right foreground, rim-lit by the fire
    let ch = min(sdf2_round_box(p - vec2f(0.78, -0.2), vec2f(0.2, 0.3), 0.12), sdf2_round_box(p - vec2f(0.6, -0.36), vec2f(0.07, 0.16), 0.06));
    if (ch < aa * 2.0) {
        let e = 0.006;
        let rim = smoothstep(-0.03, 0.0, ch) * smoothstep(0.5, 0.2, p.x - 0.4) ;
        let leather = col_hex(0x6a3418u) * 0.9 * (0.8 + 0.2 * noise_value2(p * 40.0));
        // cylindrical form across the back, a seam where back meets arm
        let across = clamp((p.x - 0.78) / 0.2, -1.0, 1.0);
        let up = clamp((p.y + 0.2) / 0.3, -1.0, 1.0);
        let cn = normalize(vec3f(across * 0.9, 0.15 + 0.5 * smoothstep(0.6, 1.0, up), -0.6));
        var cc = leather * (l.amb * 1.5 + hearth_lit(vec3f(p, -0.35), cn, l, flick) * 1.2);
        cc *= 1.0 - 0.4 * exp(-sq((p.x - 0.665) / 0.01)) * step(p.y, -0.2);
        cc += hearth_c() * l.fire * 0.08 * rim * (1.0 + flick * 0.6);
        // the fire catches the curve of the top
        cc += hearth_c() * l.fire * 0.05 * smoothstep(-0.025, 0.0, ch) * smoothstep(0.0, 0.08, p.y) * smoothstep(0.95, 0.65, p.x);
        // a sheen along the arm roll
        cc += hearth_c() * 0.03 * l.fire * exp(-sq((p.y + 0.21) / 0.012)) * step(p.x, 0.66);
        col = mix(col, cc, aa_fill(ch, ctx));
    }

    // ---- glow in the air around the fire, embers rising into the flue
    let gl = exp(-sq((p.x - FIRE.x) / 0.18) - sq((p.y - FIRE.y - 0.05) / 0.14));
    col += hearth_c() * gl * 0.06 * l.fire * (1.0 + flick * 0.5);
    if (od < 0.0) {
        col += fire_embers((p - FIRE) * 2.2, t, 0.1, ctx) * 0.3 * l.fire * smoothstep(FIRE.y + 0.02, FIRE.y + 0.1, p.y);
    }
    return col * exp2(l.exposure);
}
