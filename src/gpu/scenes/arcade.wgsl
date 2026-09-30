//! name: arcade
//! title: Arcade Hall
//! category: games
//! tags: arcade, cabinets, neon, 80s, attract mode, carpet
//! desc: a dark arcade hall, cabinets in attract mode under neon marquees on a starry carpet
//! themes: night, neon, closing
//! params: glow, crowd
//! uses: sdf, noise, light, bokeh, glyph, game
//! cost: medium
//! fallback: den
//! credits: original

// An arcade after dark, drawn flat and back to front. A row of cabinets
// stands against the back wall, each screen running its own attract mode
// at a few frames a second and pooling its colour on the carpet; neon
// marquees rim the tops; dim lamps and one red EXIT sign hang above. The
// row repeats forever by cell, so any wall width is covered.

const FLOOR_Y: f32 = -0.1;                   // where the carpet meets the wall
const PITCH: f32 = 0.27;                     // cabinet spacing
const CAB_HW: f32 = 0.105;                   // cabinet half width
const CAB_TOP: f32 = 0.31;
const SCREEN_C: vec2f = vec2f(0.0, 0.14);    // cabinet-local
const SCREEN_H: vec2f = vec2f(0.072, 0.06);
const MARQ_C: vec2f = vec2f(0.0, 0.276);
const MARQ_H: vec2f = vec2f(0.092, 0.024);
const EXIT_C: vec2f = vec2f(0.44, 0.43);
const LAMP_PITCH: f32 = 0.75;

struct Look {
    mode: u32,          // 0 night, 1 neon, 2 closing
    wall: vec3f,
    carpet: vec3f,
    star_a: vec3f,
    star_b: vec3f,
    marq_gain: f32,
    off_frac: f32,      // share of cabinets switched off
    lamp: vec3f,        // hanging lamp radiance
    ceil_wash: vec3f,   // ambient on the upper wall / ceiling
    floor_wash: vec3f,  // ambient on the carpet
    exit_gain: f32,
}

struct Cab {
    on: f32,
    kind: u32,
    seed: f32,
    body: vec3f,
    marq: vec3f,        // marquee radiance (0 when off)
    tint: vec3f,        // the colour its screen throws
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, col_hex(0x1a1030u) * 0.4, col_hex(0x1c0a34u) * 0.55,
                        col_hex(0xc060ffu) * 0.7, col_hex(0x40ffe0u) * 0.6, 1.6, 0.0,
                        col_hex(0xff40c0u) * 0.35, col_hex(0x8030ffu) * 0.12, col_hex(0x6020c0u) * 0.1, 1.0);
        }
        case 2u: {
            return Look(2u, col_hex(0x2a3038u) * 0.45, col_hex(0x0c0c12u) * 0.6,
                        col_hex(0x604a90u) * 0.35, col_hex(0x3a7a80u) * 0.3, 0.8, 0.5,
                        col_hex(0xd8ecffu) * 1.6, col_hex(0x9ab0c8u) * 0.14, col_hex(0x8098b0u) * 0.07, 2.6);
        }
        default: {
            return Look(0u, col_hex(0x1a1c2eu) * 0.35, col_hex(0x0a0810u) * 0.5,
                        col_hex(0x8040e0u) * 0.35, col_hex(0x30c0b0u) * 0.3, 1.0, 0.0,
                        col_kelvin(2400.0) * 0.6, col_hex(0x302050u) * 0.05, vec3f(0.0), 1.0);
        }
    }
}

// ------------------------------------------------------------------ cabinets
fn cab_info(idx: i32, l: Look, t: f32, glow_k: f32) -> Cab {
    let h = hash_cell2(vec2i(idx, 0), 0xcabu);
    let m = ((idx % 4) + 4) % 4;
    let mi = (((idx * 3 + 1) % 4) + 4) % 4;
    var pal = array<vec3f, 4>(col_hex(0xff30c8u), col_hex(0x20e8ffu), col_hex(0xff8a20u), col_hex(0x50ff70u));
    var tints = array<vec3f, 4>(col_hex(0x7090ffu), col_hex(0x90b0ffu), col_hex(0x50e070u), col_hex(0x40c8ffu));
    if (l.mode == 1u) {
        pal = array<vec3f, 4>(col_hex(0x20f0ffu), col_hex(0xff30d0u), col_hex(0xa040ffu), col_hex(0x40ffe8u));
        tints = array<vec3f, 4>(col_hex(0x40e8ffu), col_hex(0xff50d0u), col_hex(0x40e8ffu), col_hex(0xff50d0u));
    }
    var on = 1.0;
    if (l.mode == 2u) { on = step(l.off_frac, h.y); }
    var flick = 1.0;
    if (idx == 1) { flick = mix(0.3, 1.0, light_flicker(t, 77u, 1.3)); }    // one tired tube
    let body = mix(col_hex(0x14141cu), col_hex(0x1e1420u), h.z) * 0.5;
    return Cab(on, u32(m), h.x, body, pal[mi] * l.marq_gain * glow_k * on * flick, tints[m]);
}

// what one screen shows; uv in 0..1 over the glass
fn screen_game(uv: vec2f, cab: Cab, l: Look, t: f32, ctx: Ctx) -> vec3f {
    let aa = ctx.px / (SCREEN_H.x * 2.0);
    let neon = l.mode == 1u;
    var bg = col_hex(0x0a1a58u);
    if (neon) { bg = col_hex(0x24083cu); }
    var c = bg * 0.35;
    let ts = t + cab.seed * 37.0;
    let tq = floor(ts * 6.0) / 6.0;                        // 6 fps motion
    let ink = select(col_hex(0xe8f4ffu), col_hex(0x40f0ffu), neon);
    switch (cab.kind) {
        case 1u: {
            // a ball and two paddles
            let bx = game_bounce(tq, 0.08, 0.92, 0.32, 0.0);
            let by = game_bounce(tq, 0.12, 0.88, 0.23, 0.4);
            c += ink * game_rect(uv, vec2f(bx, by), vec2f(0.035, 0.045), aa);
            c += ink * 0.8 * game_rect(uv, vec2f(0.05, by), vec2f(0.02, 0.12), aa);
            let py = clamp(by + 0.12 * sin(tq * 0.7), 0.15, 0.85);
            c += ink * 0.8 * game_rect(uv, vec2f(0.95, py), vec2f(0.02, 0.12), aa);
            let dash = step(0.5, fract(uv.y * 6.0)) * game_rect(uv, vec2f(0.5, 0.5), vec2f(0.012, 0.5), aa);
            c += ink * 0.3 * dash;
        }
        case 2u: {
            // pseudo-text scrolling up a row at a time
            let tick = floor(ts * 3.0);
            let row = i32(floor(uv.y * 7.0)) - i32(tick);
            let lyf = fract(uv.y * 7.0);
            let colx = i32(floor(uv.x * 9.0));
            let lxf = fract(uv.x * 9.0) * 4.0;
            let hr = hash_cell2(vec2i(0, row), 0x7e7u);
            let len = 2 + i32(hr.x * 7.0);
            let hc = hash_cell2(vec2i(colx, row), 0x7e8u);
            let g = glyph_pseudo(u32(hc.x * 4096.0));
            var ink2 = col_hex(0x50ff70u);
            if (neon) { ink2 = col_hex(0xff50d0u); }
            if (colx < len && hr.y > 0.2) {
                c += ink2 * 0.9 * glyph_sample(g, lxf, (1.0 - lyf) * 6.0);
            }
            c += ink2 * 0.9 * game_blink(t, 1.0, 0.5) * game_rect(uv, vec2f(0.06, 0.07), vec2f(0.03, 0.05), aa);
        }
        case 3u: {
            // a formation of invaders stepping side to side, a ship below
            let tick = game_tick(ts, 2.0);
            let ph = fmod_pos(tick.x, 8.0);
            let sx = (abs(ph - 4.0) - 2.0) * 0.03;
            let g = (uv - vec2f(0.12 + sx, 0.42)) / vec2f(0.15, 0.16);
            let cell = floor(g);
            let f = fract(g);
            if (cell.x >= 0.0 && cell.x < 5.0 && cell.y >= 0.0 && cell.y < 3.0) {
                let round = floor(ts / 15.0);
                let prog = fract(ts / 15.0);
                let h = hash_cell2(vec2i(cell) + vec2i(i32(round) * 7, 0), 0x1a5u);
                let alive = step(prog * 0.85, h.x);
                let frame = fmod_pos(tick.x, 2.0) - 0.5;
                let body = game_rect(f, vec2f(0.5, 0.55), vec2f(0.32, 0.18), aa * 6.0);
                let legs = game_rect(f, vec2f(0.5 + 0.22 * frame, 0.3), vec2f(0.1, 0.1), aa * 6.0)
                         + game_rect(f, vec2f(0.5 - 0.22 * frame, 0.3), vec2f(0.1, 0.1), aa * 6.0);
                c += cab.tint * 1.3 * alive * saturate(body + legs);
            }
            let shx = game_bounce(tq, 0.1, 0.9, 0.25, 0.0);
            c += ink * game_rect(uv, vec2f(shx, 0.1), vec2f(0.06, 0.03), aa);
            let shot = fract(tq * 0.8 + cab.seed);
            c += ink * 0.8 * game_rect(uv, vec2f(shx, 0.16 + shot * 0.6), vec2f(0.01, 0.03), aa);
        }
        default: {
            // a block grid that fills a row every two seconds, then flashes and clears
            let tick = game_tick(ts, 0.5);
            let filled = fmod_pos(tick.x, 9.0);
            let g = (uv - vec2f(0.08, 0.06)) / vec2f(0.14, 0.14);
            let cell = floor(g);
            let f = fract(g);
            if (cell.x >= 0.0 && cell.x < 6.0 && cell.y >= 0.0 && cell.y < 6.0) {
                let h = hash_cell2(vec2i(cell) + vec2i(0, i32(floor(tick.x / 9.0)) * 11), 0xb10u);
                let inside = game_rect(f, vec2f(0.5, 0.5), vec2f(0.4, 0.4), aa * 7.0);
                let on = step(cell.y + 0.5, filled);
                var bc = col_hsv(fract(h.y * 0.6 + cell.y * 0.13), 0.75, 1.0);
                if (!neon) { bc = mix(bc, col_hex(0x60a0ffu), 0.45); }
                let flash = mix(1.0, 1.8, step(7.5, filled) * game_blink(t, 1.0, 0.5));
                c += bc * inside * on * flash;
            }
            let bx = game_bounce(tq, 0.1, 0.9, 0.3, 0.2);
            let by = game_bounce(tq, 0.9, 0.96, 0.05, 0.0);
            c += ink * game_rect(uv, vec2f(bx, by), vec2f(0.03, 0.035), aa);
        }
    }
    let vg = 1.0 - 0.6 * dot(uv - 0.5, uv - 0.5);
    return c * vg;
}

// the cabinet of the cell we are in; lp is cabinet-local. returns (rgb, coverage)
fn cabinet(lp: vec2f, cab: Cab, l: Look, t: f32, glow_k: f32, ctx: Ctx) -> vec4f {
    let body_d = sdf2_box(lp - vec2f(0.0, (FLOOR_Y + CAB_TOP) * 0.5), vec2f(CAB_HW, (CAB_TOP - FLOOR_Y) * 0.5));
    let cover = aa_fill(body_d, ctx);
    if (cover <= 0.0) { return vec4f(0.0); }
    var c = cab.body;
    // edge trim in the marquee colour (side art), a darker kickplate
    let trim = sstep(0.018, 0.008, abs(abs(lp.x) - 0.094)) * sstep(0.245, 0.21, lp.y) * sstep(-0.1, -0.03, lp.y);
    c += cab.marq * 0.05 * trim;
    c = mix(c, c * 0.5, sstep(-0.05, -0.07, lp.y));
    // coin door with a lit slot
    let coin_d = sdf2_round_box(lp - vec2f(0.0, -0.035), vec2f(0.03, 0.02), 0.004);
    c = mix(c, cab.body * 1.4 + vec3f(0.01), aa_fill(coin_d, ctx));
    c += col_hex(0xff9030u) * 0.6 * cab.on * aa_fill(sdf2_box(lp - vec2f(0.0, -0.03), vec2f(0.003, 0.006)), ctx);
    // the slanted control panel, lit by the screen
    let hw = CAB_HW + 0.008 - (lp.y - 0.02) * 0.35;
    let panel_d = max(abs(lp.x) - hw, abs(lp.y - 0.045) - 0.025);
    let panel_col = cab.body * 1.5 + cab.tint * 0.22 * glow_k * cab.on;
    c = mix(c, panel_col, aa_fill(panel_d, ctx));
    c += col_hex(0xff3030u) * 0.5 * aa_fill(sdf2_circle(lp - vec2f(0.025, 0.045), 0.007), ctx);
    c += col_hex(0xffd030u) * 0.5 * aa_fill(sdf2_circle(lp - vec2f(0.045, 0.04), 0.007), ctx);
    c += col_hex(0xd03030u) * 0.6 * aa_fill(sdf2_circle(lp - vec2f(-0.04, 0.05), 0.009), ctx);
    // bezel and glass
    let bez_d = sdf2_round_box(lp - SCREEN_C, SCREEN_H + 0.014, 0.012);
    c = mix(c, col_hex(0x08080cu), aa_fill(bez_d, ctx));
    let scr_d = sdf2_round_box(lp - SCREEN_C, SCREEN_H, 0.006);
    let scr = aa_fill(scr_d, ctx);
    let uv = (lp - SCREEN_C + SCREEN_H) / (SCREEN_H * 2.0);
    var pic = col_hex(0x0c0c14u) * 0.5;
    if (cab.on > 0.5) {
        pic = screen_game(clamp(uv, vec2f(0.0), vec2f(1.0)), cab, l, t, ctx) * glow_k;
    } else {
        pic += l.lamp * 0.03 * sstep(0.2, 1.0, uv.y);          // dead glass reflecting the lamps
    }
    c = mix(c, pic, scr);
    c += cab.tint * glow_k * cab.on * 0.25 * light_glow(max(scr_d, 0.0), 0.04) * (1.0 - scr);
    // marquee: a lit sign with a neon rim
    let mq_d = sdf2_box(lp - MARQ_C, MARQ_H);
    let sign = cab.marq * (0.35 + 0.5 * exp(-sq((lp.y - MARQ_C.y) / 0.012)));
    c = mix(c, col_hex(0x0a0a10u) * 0.5 + sign, aa_fill(mq_d, ctx));
    c += cab.marq * light_neon(abs(mq_d), 0.004, 0.02) * 0.35;
    return vec4f(c, cover);
}

// a dark figure standing at the cabinet; returns (rgb, coverage)
fn player(lp: vec2f, idx: i32, cab: Cab, crowd_k: f32, t: f32, ctx: Ctx) -> vec4f {
    let h = hash_cell2(vec2i(idx, 11), 0x5e1u);
    if (h.x >= crowd_k || cab.on < 0.5) { return vec4f(0.0); }
    let off = mix(-0.035, 0.035, h.y);
    let sway = select(0.0, 0.012 * sin(t * TAU * 0.2 + h.z * TAU), h.w < 0.4);
    let s = 0.9 + 0.2 * h.z;
    let q = lp - vec2f(off + sway, 0.0);
    // shoulders and torso (a rounded slab) standing nearer than the cabinet, head at panel height
    let body_d = sdf2_round_box(q - vec2f(sway * 0.4, -0.09 * s - 0.11), vec2f(0.05 * s, 0.12 * s), 0.045 * s);
    let head_d = sdf2_circle(q - vec2f(sway * 1.2, 0.07 * s - 0.05), 0.032 * s);
    let d = min(body_d, head_d);
    let rim = sstep(-0.012, 0.0, d) * sstep(-0.16, 0.04, q.y);
    let c = col_hex(0x04040au) * 0.5 + cab.tint * 0.28 * rim;
    return vec4f(c, aa_fill(d, ctx));
}

// ------------------------------------------------------------------ backdrop
fn carpet(p: vec2f, l: Look, ctx: Ctx) -> vec3f {
    var c = l.carpet + l.floor_wash;
    let cellw = 0.034;
    let q = p / cellw;
    let h = hash_cell2(vec2i(floor(q)), 0x5a7u);
    let r = (fract(q) - 0.5) * cellw - (h.zw - 0.5) * 0.5 * cellw;
    var d = 1.0;
    if (h.x < 0.3) {
        d = (abs(r.x) + abs(r.y)) - 0.005 - 0.004 * h.y;          // a little star
    } else if (h.x < 0.5) {
        let rr = rot2(h.y * TAU) * r;                                 // a squiggle
        d = max(abs(rr.y - 0.004 * sin(rr.x * 500.0)) - 0.0018, abs(rr.x) - 0.011);
    }
    let tint = mix(l.star_a, l.star_b, step(0.5, h.w));
    return mix(c, tint, aa_fill(d, ctx) * (0.3 + 0.6 * h.y));
}

fn backdrop(p: vec2f, l: Look, ctx: Ctx) -> vec3f {
    let floor_c = carpet(p, l, ctx);
    let up = sstep(FLOOR_Y, 0.9, p.y);
    var wall = l.wall * mix(1.0, 0.2, up);
    wall += l.ceil_wash * sstep(0.15, 0.5, p.y) * mix(1.0, 0.3, sstep(0.5, 1.1, p.y));
    return mix(floor_c, wall, sstep(FLOOR_Y - 0.006, FLOOR_Y + 0.006, p.y));
}

// ------------------------------------------------------------------ scene
fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let k = ctx.params;
    let glow_k = 0.3 + 1.4 * k.x;
    let crowd_k = k.y * 0.9 * select(1.0, 0.4, l.mode == 2u);

    let fi = floor(p.x / PITCH + 0.5);
    let idx = i32(fi);
    let lp = vec2f(p.x - fi * PITCH, p.y);

    var col = backdrop(p, l, ctx);

    // light from this cabinet and its two neighbours onto the wall and the carpet
    let down = max(FLOOR_Y - p.y, 0.0);
    let on_floor = step(p.y, FLOOR_Y);
    for (var j = -1; j <= 1; j++) {
        let cj = cab_info(idx + j, l, t, glow_k);
        let q = vec2f(p.x - (fi + f32(j)) * PITCH, p.y);
        let mq_d = max(sdf2_box(q - MARQ_C, MARQ_H), 0.0);
        col += cj.marq * (light_glow(mq_d, 0.07) * 0.2 + exp(-mq_d / 0.05) * 0.1);
        let spread = 0.085 + 0.5 * down;
        let pool = exp(-sq(q.x / spread)) * exp(-down / 0.16) * on_floor;
        col += cj.tint * cj.on * glow_k * 0.32 * pool;
        col += cj.marq * 0.04 * exp(-sq(q.x / (0.14 + 0.6 * down))) * exp(-down / 0.35) * on_floor;
        let scr_d = max(sdf2_round_box(q - SCREEN_C, SCREEN_H, 0.006), 0.0);
        col += cj.tint * cj.on * glow_k * 0.12 * light_glow(scr_d, 0.06) * (1.0 - on_floor);
    }

    // hanging lamps along the ceiling
    let lfi = floor(p.x / LAMP_PITCH + 0.5);
    let lq = vec2f(p.x - lfi * LAMP_PITCH, p.y);
    if (l.mode == 2u) {
        let tube_d = sdf2_round_box(lq - vec2f(0.0, 0.46), vec2f(0.13, 0.006), 0.006);
        col += l.lamp * (aa_fill(tube_d, ctx) + light_glow(max(tube_d, 0.0), 0.05) * 0.25);
    } else {
        let cord = aa_fill(abs(lq.x) - 0.0025, ctx) * sstep(0.49, 0.51, lq.y);
        col = mix(col, col_hex(0x08080au) * 0.5, cord);
        let bulb = vec2f(0.0, 0.455);
        let dome_d = max(sdf2_circle(lq - vec2f(0.0, 0.46), 0.045), 0.46 - lq.y);
        let shade = col_hex(0x14100cu) * 0.5 + l.lamp * 0.25 * sstep(0.5, 0.462, lq.y);
        col = mix(col, shade, aa_fill(dome_d, ctx));
        let bd = length(lq - bulb);
        col += l.lamp * (light_glow(bd, 0.025) * 0.6 + light_glow(bd, 0.12) * 0.12 * sstep(0.47, 0.35, lq.y));
    }

    // the EXIT sign
    {
        let q = p - EXIT_C;
        let d = sdf2_round_box(q, vec2f(0.1, 0.04), 0.006);
        let red = col_hex(0xff2818u) * l.exit_gain;
        col += red * (light_glow(max(d, 0.0), 0.06) * 0.22 + exp(-max(d, 0.0) / 0.03) * 0.12);
        var word = array<u32, 4>(69u, 88u, 73u, 84u);
        let loc = glyph_locate(p, vec2f(EXIT_C.x - 0.0825, EXIT_C.y + 0.033), 0.011);
        let ci = i32(loc.x);
        var ink = 0.0;
        if (ci >= 0 && ci < 4 && loc.y < 3.0 && loc.z < 5.0) { ink = glyph_sample(glyph_font(word[ci]), loc.y, loc.z); }
        col = mix(col, col_hex(0x1a0808u) * 0.5 + red * 1.6 * ink, aa_fill(d, ctx));
    }

    // the cabinet in front of us, and whoever stands at it
    let cab = cab_info(idx, l, t, glow_k);
    let cb = cabinet(lp, cab, l, t, glow_k, ctx);
    col = mix(col, cb.rgb, cb.a);
    let pl = player(lp, idx, cab, crowd_k, t, ctx);
    col = mix(col, pl.rgb, pl.a);

    // closing time: a mop bucket left by the wall
    if (l.mode == 2u) {
        let bq = p - vec2f(-0.675, -0.2);
        let hd = sdf2_segment(bq, vec2f(0.01, 0.0), vec2f(0.045, 0.34)) - 0.005;
        col = mix(col, col_hex(0x8a8a90u) * 0.3, aa_fill(hd, ctx));
        let head_d = sdf2_circle(bq - vec2f(0.0, 0.02), 0.03);
        col = mix(col, col_hex(0xb0b0b8u) * 0.22, aa_fill(head_d, ctx));
        let bd = sdf2_round_box(bq, vec2f(0.042, 0.03), 0.008);
        let bucket = col_hex(0xd8b428u) * (0.18 + 0.3 * sstep(-0.03, 0.03, bq.y)) + l.lamp * 0.03;
        col = mix(col, bucket, aa_fill(bd, ctx));
    }

    // dust motes drifting through the light
    for (var i = 0; i < 7; i++) {
        let h = hash_cell2(vec2i(i, 5), 0xd0e5u);
        let y = -0.55 + fmod_pos(h.y + t * 0.012 * (0.6 + h.w), 1.0);
        let x = mix(-0.95, 0.95, h.x) + 0.03 * sin(t * 0.25 + h.z * TAU);
        col += col_hex(0xffe0c0u) * 0.3 * bokeh_disc(p, vec2f(x, y), 0.007, ctx);
    }

    return max(col, vec3f(0.0));
}
