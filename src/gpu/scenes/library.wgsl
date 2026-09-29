//! name: library
//! title: Candlelit Library
//! category: cozy
//! tags: library, books, candle, lamp, dust, rain, interior
//! desc: an old reading room: towering shelves, a green banker's lamp, a candle, dust in the air
//! themes: candle, dawn, storm
//! uses: sdf, rain, noise
//! cost: light
//! fallback: den
//! credits: original

// A panelled reading room seen from a chair at the desk. The back wall is
// shelving floor to ceiling around a tall arched window; the desk fills the
// foreground. Surfaces carry a pseudo-3D position (x, y on screen, z toward
// the viewer) and are lit by up to three lights: the banker's lamp, the
// candle, and the window (moonlight, a dawn shaft full of dust, or storm
// light with rare lightning).

const WIN_C: vec2f = vec2f(0.0, 0.14);
const WIN_H: vec2f = vec2f(0.125, 0.25);    // straight part half size; arch on top
const SHELF_H: f32 = 0.13;                  // shelf pitch
const DESK_BACK: f32 = -0.25;
const DESK_FRONT: f32 = -0.46;
const LAMP: vec2f = vec2f(-0.46, -0.1);     // shade centre
const CANDLE: vec2f = vec2f(0.43, -0.2);    // wick

struct Look {
    mode: u32,        // 0 candle, 1 dawn, 2 storm
    lamp: f32,
    candle: f32,
    win_c: vec3f,     // light through the window
    sky_top: vec3f,
    sky_low: vec3f,
    amb: vec3f,
    shaft: f32,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, 0.0, 0.6, col_hex(0xcfe0ffu) * 1.0, col_hex(0x6a86b8u) * 0.6, col_hex(0xf0c8b8u) * 0.9,
                        col_hex(0x8aa0c8u) * 0.09, 1.0, 0.8);
        }
        case 2u: {
            return Look(2u, 0.8, 0.0, col_hex(0x8a9ab8u) * 0.22, col_hex(0x1c2230u) * 0.5, col_hex(0x4a5260u) * 0.45,
                        col_hex(0x607090u) * 0.03, 0.0, 0.7);
        }
        default: {
            return Look(0u, 1.0, 1.0, col_hex(0x6a80b0u) * 0.05, col_hex(0x0a1226u) * 0.4, col_hex(0x1c2848u) * 0.4,
                        col_hex(0x506080u) * 0.01, 0.0, 0.7);
        }
    }
}

// lightning: (flash brightness 0..1) — rare double flicker, closed-form
fn lightning(t: f32) -> f32 {
    let ev = hash_event(t, 19.0, 0x11ceu);
    if (ev.x > 0.55) { return 0.0; }
    let lt = ev.y * 19.0 - 3.0 - ev.x * 8.0;
    if (lt < 0.0 || lt > 0.8) { return 0.0; }
    return exp(-lt * 14.0) + 0.6 * exp(-abs(lt - 0.25) * 22.0);
}

// Light arriving at pseudo-3D point pos with normal n.
fn room_light(pos: vec3f, n: vec3f, l: Look, t: f32, flash: f32) -> vec3f {
    var e = l.amb;
    // banker's lamp: the shade sends light down and forward
    if (l.lamp > 0.0) {
        let lp = vec3f(LAMP.x, LAMP.y - 0.03, -0.55);
        let d = lp - pos;
        let dist2 = dot(d, d);
        let ld = d * inverseSqrt(dist2);
        let down = smoothstep(-0.35, 0.25, ld.y);    // the shade blocks light going up
        e += col_kelvin(2700.0) * l.lamp * (max(dot(n, ld), 0.0) * 0.1 * (0.2 + 0.8 * down) / (dist2 + 0.012)
             + 0.13 / (1.0 + dist2 * 3.0));
    }
    // candle: small, low, warm; breathing slowly
    if (l.candle > 0.0) {
        let fl = 0.85 + 0.15 * (0.6 * sin(t * 1.3) * sin(t * 0.8 + 1.0) + 0.4 * sin(t * 0.5));
        let lp = vec3f(CANDLE.x, CANDLE.y + 0.03, -0.6);
        let d = lp - pos;
        let dist2 = dot(d, d);
        let ld = d * inverseSqrt(dist2);
        e += col_kelvin(1850.0) * l.candle * fl * (max(dot(n, ld), 0.0) * 0.03 / (dist2 + 0.006) + 0.04 / (1.0 + dist2 * 5.0));
    }
    // window: soft light from the back wall
    let wd = vec3f(WIN_C, 0.05) - pos;
    let wdist2 = dot(wd, wd);
    e += l.win_c * (0.25 * max(dot(n, normalize(wd)), 0.0) / (1.0 + wdist2 * 4.0) + 0.12 / (1.0 + wdist2 * 3.0));
    e += col_hex(0xdce8ffu) * flash * 1.2;
    return e;
}

// ------------------------------------------------------------------ window
fn win_d(p: vec2f) -> f32 {
    let q = p - WIN_C;
    let rect = sdf2_box(q, WIN_H);
    let arch = length(q - vec2f(0.0, WIN_H.y)) - WIN_H.x;
    return select(rect, min(rect, arch), q.y > 0.0);
}

// leaded panes: 1 on the lead lines
fn leads(p: vec2f, aa: f32) -> f32 {
    let q = (p - WIN_C) * vec2f(1.0, 1.0);
    // diamond lattice
    let a = (q.x + q.y) / 0.05;
    let b = (q.x - q.y) / 0.05;
    let da = abs(fract(a) - 0.5);
    let db = abs(fract(b) - 0.5);
    let w = max(0.06, aa / 0.05 * 0.7);
    var m = max(smoothstep(w, w * 0.4, 0.5 - da), smoothstep(w, w * 0.4, 0.5 - db)) * 0.8;
    // stone mullion and transom
    m = max(m, smoothstep(0.009, 0.006, abs(q.x)));
    m = max(m, smoothstep(0.009, 0.006, abs(q.y - 0.06)));
    return m;
}

fn outside(p: vec2f, l: Look, t: f32, flash: f32) -> vec3f {
    let q = p - WIN_C;
    var c = mix(l.sky_low, l.sky_top, smoothstep(-0.2, 0.35, q.y));
    // rooftops and a spire across the quad
    let roof = -0.14 + 0.03 * step(0.5, fract(p.x * 9.0 + 0.3)) + 0.12 * saturate(1.0 - abs(p.x - 0.06) / 0.015) * step(abs(p.x - 0.06), 0.015);
    if (q.y < roof) { c = l.sky_low * select(0.15, 0.35, l.mode == 1u); }
    if (l.mode == 0u) {
        // the moon, high in the left pane
        let md = length(p - vec2f(-0.06, 0.26));
        c += vec3f(1.0, 0.97, 0.9) * (smoothstep(0.02, 0.017, md) * 1.2 + 0.1 * exp(-md * 20.0));
    }
    // clouds drifting slowly across the glass: moonlit edges, pink at dawn
    if (l.mode != 2u && q.y > roof) {
        let cq = vec2f(p.x * 5.0 - t * 0.05, p.y * 11.0);
        let cn = noise_fbm2(cq + vec2f(3.0, 7.0), 5);
        let cloud = smoothstep(0.5, 0.72, cn) * smoothstep(-0.05, 0.12, q.y);
        let md = length(p - vec2f(-0.06, 0.26));
        var cc = select(col_hex(0x1a2438u) * 0.35 + vec3f(0.35, 0.36, 0.4) * exp(-md * 9.0), col_hex(0xf0b8a8u) * 0.9, l.mode == 1u);
        c = mix(c, cc, cloud * 0.85);
    }
    c += col_hex(0xe0e8ffu) * flash * 1.5;
    return c;
}

// ------------------------------------------------------------------ shelves
fn book_col(h: f32) -> vec3f {
    let i = u32(h * 11.0);
    switch (i) {
        case 0u: { return col_hex(0x5a1a14u); }  // oxblood
        case 1u: { return col_hex(0x1e3a28u); }  // bottle green
        case 2u: { return col_hex(0x1c2640u); }  // navy
        case 3u: { return col_hex(0x8a6a44u); }  // tan calf
        case 4u: { return col_hex(0x4a2e1cu); }  // brown
        case 5u: { return col_hex(0x141210u); }  // black
        case 6u: { return col_hex(0x7a5a3au); }  // faded
        case 7u: { return col_hex(0x6a2a1eu); }  // rust
        case 8u: { return col_hex(0xa89a78u); }  // vellum
        case 9u: { return col_hex(0x2a3a3au); }  // slate
        default: { return col_hex(0x5a4430u); }
    }
}

// the back wall: shelving. Returns colour (lit).
fn shelves(p: vec2f, l: Look, t: f32, flash: f32, aa: f32) -> vec3f {
    let wood = col_hex(0x3a2414u) * 0.5;
    // bays: uprights every 0.34
    let bay = fract((p.x + 0.17) / 0.34);
    let upr = min(bay, 1.0 - bay) * 0.34;
    let row = floor((p.y + 0.5) / SHELF_H);
    let ry = fract((p.y + 0.5) / SHELF_H);
    let n_face = vec3f(0.0, 0.0, -1.0);
    var c: vec3f;
    // shelf boards and uprights
    let board = step(ry, 0.14);
    if (upr < 0.012 || board > 0.5) {
        var bn = n_face;
        if (board > 0.5) { bn = normalize(vec3f(0.0, select(-0.3, 0.6, ry > 0.1), -1.0)); }
        c = wood * (0.85 + 0.2 * noise_value2(p * vec2f(80.0, 6.0))) * room_light(vec3f(p, -0.02), bn, l, t, flash);
        return c;
    }
    // books: jittered-width slots
    let x = p.x * 1.0;
    let w = 0.024;
    let k = floor(x / w);
    let hb = hash_cell2(vec2i(i32(k), i32(row)), 0xb00cu);
    let hbn = hash_cell2(vec2i(i32(k) + 1, i32(row)), 0xb00cu);
    let x0 = (k + (hb.w - 0.5) * 0.35) * w;
    let x1 = (k + 1.0 + (hbn.w - 0.5) * 0.35) * w;
    var bid = k;
    var h = hb;
    var bx0 = x0;
    var bx1 = x1;
    if (x < x0) {
        bid = k - 1.0;
        h = hash_cell2(vec2i(i32(bid), i32(row)), 0xb00cu);
        bx1 = x0;
        bx0 = (bid + (h.w - 0.5) * 0.35) * w;
    } else if (x > x1) {
        bid = k + 1.0;
        h = hbn;
        bx0 = x1;
        let h2 = hash_cell2(vec2i(i32(bid) + 1, i32(row)), 0xb00cu);
        bx1 = (bid + 1.0 + (h2.w - 0.5) * 0.35) * w;
    }
    let clear = 1.0 - 0.14;
    let top = 0.14 + clear * (0.72 + 0.24 * h.y);
    let u = (x - bx0) / max(bx1 - bx0, 1e-4);
    // back of the case, in deep shadow
    var back = wood * 0.3 * room_light(vec3f(p, 0.05), n_face, l, t, flash);
    // gaps, and now and then a stack lying flat
    let group = hash_cell2(vec2i(i32(floor(x / (w * 5.0))), i32(row)), 0x57acu);
    if (group.x < 0.07) {
        let sy = (ry - 0.14) / clear;
        let layer = floor(sy / 0.13);
        let hs = hash_cell2(vec2i(i32(layer), i32(row) * 31 + i32(floor(x / (w * 5.0)))), 0x5a5au);
        let edge = abs(fract(x / (w * 5.0)) - 0.5);
        if (layer < 3.0 && edge < 0.38 + 0.08 * hs.x) {
            let bc = book_col(hs.y) * 0.6 * (0.8 + 0.2 * smoothstep(0.0, 0.3, fract(sy / 0.13)));
            return bc * room_light(vec3f(p, -0.04), normalize(vec3f(0.0, 0.3, -1.0)), l, t, flash);
        }
        return back;
    }
    if (h.x < 0.05 || ry > top) { return back; }
    var bc = book_col(h.z);
    bc *= 0.75 + 0.35 * hash_f(u32(bid) * 13u + u32(row));
    // gilt bands and a label on some spines
    let sy = (ry - 0.14) / (top - 0.14);
    let gilt = step(0.5, h.y) * (step(abs(sy - 0.9), 0.012) + step(abs(sy - 0.84), 0.008) + step(abs(sy - 0.1), 0.01));
    bc = mix(bc, col_hex(0xb89040u) * 0.9, saturate(gilt) * saturate(1.5 - aa * 200.0));
    let label = step(0.75, h.x) * step(abs(sy - 0.7), 0.06) * step(abs(u - 0.5), 0.3);
    bc = mix(bc, col_hex(0xc8b890u) * 0.6, label);
    // rounded spine
    let across = (u - 0.5) * 2.0;
    let n = normalize(vec3f(across * 0.8, 0.0, -1.0));
    bc *= 0.7 + 0.3 * (1.0 - across * across);
    // the shelf above shades the tops of the books
    let shade = 0.55 + 0.45 * smoothstep(1.0, 0.55, ry);
    c = bc * 0.55 * room_light(vec3f(p, -0.03), n, l, t, flash) * shade;
    // cracks between spines
    c *= smoothstep(0.0, 0.08, min(u, 1.0 - u)) * 0.6 + 0.4;
    return c;
}

// ------------------------------------------------------------------ desk items
fn candle_flame(p: vec2f, t: f32) -> vec4f {
    let sway = 0.003 * sin(t * 1.1) + 0.0015 * sin(t * 2.3 + 1.0);
    let q = p - CANDLE - vec2f(sway * clamp(p.y - CANDLE.y, 0.0, 0.05) * 40.0, 0.0);
    // teardrop: round bottom, tapering to a point that stretches and shrinks
    let tip = 0.032 * (0.88 + 0.12 * sin(t * 1.7) * sin(t * 0.9 + 2.0));
    let yy = q.y - 0.012;
    let r = 0.0058 * sqrt(saturate(1.0 - yy / tip));
    var d = length(vec2f(q.x, min(yy, 0.0) * 1.3)) - 0.0058;
    if (yy > 0.0) { d = abs(q.x) - r; }
    if (yy > tip) { d = length(vec2f(q.x, yy - tip)); }
    let core = smoothstep(0.002, -0.004, d);
    let blue = smoothstep(0.008, 0.0, q.y) * core;
    var c = mix(vec3f(3.2, 2.2, 1.0), vec3f(2.4, 0.9, 0.25), smoothstep(-0.006, 0.0, d));
    c = mix(c, vec3f(0.2, 0.3, 1.0), blue * 0.5);
    return vec4f(c, smoothstep(0.003, -0.002, d));
}

// ------------------------------------------------------------------ scene
fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let aa = ctx.px;
    var flash = 0.0;
    if (l.mode == 2u) { flash = lightning(t); }
    var col: vec3f;

    // ---- back wall: cornice, shelves, window
    col = shelves(p, l, t, flash, aa);
    if (p.y > 0.44) {
        // carved cornice
        let v = (p.y - 0.44) / 0.1;
        let bn = normalize(vec3f(0.0, -0.6 + 0.4 * sin(v * 9.0), -1.0));
        col = col_hex(0x3a2414u) * 0.45 * room_light(vec3f(p, -0.03), bn, l, t, flash) * (0.85 + 0.15 * sin(p.x * 120.0) * step(0.3, fract(v * 3.0)));
    }
    let wd = win_d(p);
    if (wd < 0.03) {
        // stone surround
        let st = col_hex(0x8a8074u) * 0.4 * room_light(vec3f(p, -0.01), vec3f(0.0, 0.0, -1.0), l, t, flash);
        col = mix(col, st, aa_fill(wd - 0.03, ctx));
    }
    if (wd < 0.0) {
        var o = outside(p, l, t, flash);
        if (l.mode == 2u) {
            let rg = rain_glass(p * 2.0, t, ctx);
            o = outside(p + rg.xy * 0.3, l, t, flash) * (1.0 + rg.z * 0.5);
        }
        let ld = leads(p, aa);
        o = mix(o, col_hex(0x1a1612u) * 0.2 * (l.amb * 10.0 + l.win_c * 0.3), ld);
        col = mix(col, o, aa_fill(wd, ctx));
    }

    // ---- rolling ladder leaning on the right-hand shelves
    {
        let lx = 0.66 - (p.y - 0.45) * 0.14;
        let rail = min(abs(p.x - lx + 0.045), abs(p.x - lx - 0.045)) - 0.006;
        let rung = max(abs(fract((p.y + 0.5) / 0.07) - 0.5) * 0.07 - 0.004, abs(p.x - lx) - 0.045);
        let ld = min(rail, rung);
        if (ld < aa && p.y < 0.46) {
            let lc = col_hex(0x4a2e18u) * 0.5 * room_light(vec3f(p, -0.15), normalize(vec3f(-0.3, 0.2, -1.0)), l, t, flash);
            col = mix(col, lc, aa_fill(ld, ctx));
        }
    }

    // ---- dawn: a shaft of light full of dust
    var shaft = 0.0;
    if (l.shaft > 0.0) {
        let dir = normalize(vec2f(0.42, -0.6));
        for (var i = 0; i < 10; i++) {
            let s = (f32(i) + 0.5) * 0.055;
            let q = p - dir * s;
            shaft += aa_fill_w(win_d(q), 0.03) * (1.0 - leads(q, 0.01) * 0.7) * exp(-s * 1.5);
        }
        shaft /= 10.0;
        col += l.win_c * shaft * 0.16 * smoothstep(-0.02, 0.05, win_d(p));
    }

    // ---- the desk
    if (p.y < DESK_BACK) {
        // top surface in perspective: z from far (back edge) to near
        let v = (p.y - DESK_FRONT) / (DESK_BACK - DESK_FRONT);
        let pz = mix(-0.95, -0.35, v);
        let pos = vec3f(p.x, -0.3, pz);
        let dn = vec3f(0.0, 1.0, 0.0);
        let z = 1.0 / (0.6 + v * 1.4);
        var alb = col_hex(0x4a2a16u) * 0.55 * (0.85 + 0.2 * noise_value2(vec2f(p.x * 14.0 / z, v * 3.0)));
        // green leather writing inset with a gilt border
        let inset = sdf2_box(vec2f(p.x * mix(1.0, 1.25, v), p.y - (DESK_FRONT + DESK_BACK) * 0.5), vec2f(0.34, 0.08));
        if (inset < 0.0) {
            alb = col_hex(0x2e5a3cu) * 0.7 * (0.9 + 0.1 * noise_value2(p * 90.0));
            alb = mix(alb, col_hex(0xb89040u) * 0.7, smoothstep(0.004, 0.001, abs(inset + 0.008)));
        }
        var dc = alb * room_light(pos, dn, l, t, flash);
        // polished wood reflects the lamp and the candle
        dc += col_kelvin(2700.0) * l.lamp * 0.05 * exp(-sq((p.x - LAMP.x) / 0.05) - sq((p.y + 0.36) / 0.04)) * step(0.0, inset);
        dc += col_kelvin(1850.0) * l.candle * 0.06 * exp(-sq((p.x - CANDLE.x) / 0.02) - sq((p.y + 0.3) / 0.05));
        // dawn patch on the desk
        dc += l.win_c * shaft * 0.5 * alb * 4.0;
        col = dc;
        // front edge of the desk: moulding and apron
        if (p.y < DESK_FRONT) {
            let fy = (DESK_FRONT - p.y);
            let an = normalize(vec3f(0.0, select(0.8, -0.1, fy > 0.012), -1.0));
            var ac = col_hex(0x3a2010u) * 0.5 * room_light(vec3f(p.x, p.y, -0.3), an, l, t, flash);
            ac *= 1.0 - 0.5 * smoothstep(0.01, 0.2, fy);
            // brass drawer pull
            let pull = length((p - vec2f(0.0, DESK_FRONT - 0.03)) * vec2f(0.5, 1.0)) - 0.008;
            ac = mix(ac, col_hex(0xb8903cu) * 0.6 * room_light(vec3f(p, -0.3), vec3f(0.0, 0.3, -1.0), l, t, flash), aa_fill(pull, ctx));
            col = ac;
        }
    }

    // ---- open book on the leather
    {
        let bq = p - vec2f(-0.01, -0.34);
        let pw = 0.13;
        let ph = 0.05;
        // slight perspective: narrower at the top edge
        let sx = bq.x * (1.0 + bq.y * 1.5);
        let page = sdf2_round_box(vec2f(sx, bq.y), vec2f(pw * 2.0 * 0.5 + pw * 0.0, ph), 0.006);
        let bpage = max(page, -0.0);
        if (bpage < aa) {
            let side = sign(sx);
            let u = abs(sx) / pw;
            // pages curl up toward the spine
            let bulge = 1.0 - 0.35 * exp(-u * 6.0) + 0.08 * (1.0 - u);
            let pn = normalize(vec3f(-side * 0.5 * exp(-u * 4.0), 1.0, -0.2));
            var pc = col_hex(0xe8dcc0u) * 0.8 * room_light(vec3f(p.x, -0.29, -0.7), pn, l, t, flash) * bulge;
            // lines of text: soft grey rows
            let lines = step(0.35, fract((bq.y + ph) / 0.0085)) * step(0.1, u) * step(u, 0.9) * step(abs(bq.y), ph - 0.008);
            pc *= 1.0 - 0.25 * lines * saturate(1.5 - aa * 150.0) - 0.1 * lines;
            // gutter shadow
            pc *= 0.6 + 0.4 * smoothstep(0.0, 0.08, u);
            col = mix(col, pc, aa_fill(bpage, ctx));
        }
    }

    // ---- small stack of books by the candle
    {
        let sq0 = sdf2_box(p - vec2f(0.28, -0.3), vec2f(0.07, 0.012));
        let sq1 = sdf2_box(p - vec2f(0.285, -0.276), vec2f(0.062, 0.012));
        let sq2 = sdf2_box(p - vec2f(0.275, -0.254), vec2f(0.055, 0.01));
        let sd = min(sq0, min(sq1, sq2));
        if (sd < aa) {
            var bc = book_col(0.05);
            if (sq1 < sq0 && sq1 < sq2) { bc = book_col(0.15); }
            if (sq2 < sq0 && sq2 < sq1) { bc = book_col(0.3); }
            let lit = bc * 0.6 * room_light(vec3f(p, -0.62), normalize(vec3f(0.3, 0.4, -1.0)), l, t, flash);
            let pages = step(0.066, abs(p.x - 0.28)) ;
            col = mix(col, lit, aa_fill(sd, ctx));
        }
    }

    // ---- candle in a brass stick
    {
        let wax = sdf2_box(p - vec2f(CANDLE.x, CANDLE.y - 0.045), vec2f(0.009, 0.045));
        let stick = min(sdf2_box(p - vec2f(CANDLE.x, CANDLE.y - 0.1), vec2f(0.005, 0.015)),
                        sdf2_round_box(p - vec2f(CANDLE.x, CANDLE.y - 0.115), vec2f(0.028, 0.006), 0.004));
        if (min(wax, stick) < aa) {
            let wc = col_hex(0xe8dcc0u) * 0.7 * (room_light(vec3f(p, -0.6), vec3f(0.3, 0.2, -1.0), l, t, flash) + col_kelvin(1850.0) * l.candle * 0.12 * smoothstep(CANDLE.y - 0.04, CANDLE.y, p.y));
            let bc = col_hex(0xb8903cu) * 0.6 * room_light(vec3f(p, -0.6), vec3f(0.4, 0.4, -1.0), l, t, flash);
            col = mix(col, bc, aa_fill(stick, ctx));
            col = mix(col, wc, aa_fill(wax, ctx));
        }
        if (l.candle > 0.0) {
            let f = candle_flame(p, t);
            col = mix(col, f.rgb * l.candle, f.a);
            let g = length(p - CANDLE - vec2f(0.0, 0.015));
            col += col_kelvin(1850.0) * l.candle * 0.08 * exp(-g * 30.0) * (0.9 + 0.1 * sin(t * 1.3) * sin(t * 0.8 + 1.0));
        }
    }

    // ---- banker's lamp
    {
        let base = sdf2_round_box(p - vec2f(LAMP.x, -0.305), vec2f(0.06, 0.012), 0.008);
        let stem = sdf2_box(p - vec2f(LAMP.x, -0.21), vec2f(0.006, 0.09));
        let q = p - LAMP;
        // shade: a half cylinder seen from the front
        let shade = max(sdf2_round_box(q, vec2f(0.11, 0.03), 0.02), -q.y - 0.024);
        let brass = min(base, stem);
        if (brass < aa) {
            let bc = col_hex(0xb8903cu) * 0.55 * room_light(vec3f(p, -0.55), normalize(vec3f(0.2, 0.3, -1.0)), l, t, flash);
            col = mix(col, bc + col_kelvin(2700.0) * l.lamp * 0.03 * smoothstep(-0.28, -0.32, p.y), aa_fill(brass, ctx));
        }
        if (shade < aa) {
            // green glass lit from inside, brighter at the rim, a brass trim
            let gy = saturate((q.y + 0.024) / 0.06);
            var sc = col_hex(0x0e6a3au) * (0.08 + l.lamp * (0.35 + 0.6 * exp(-gy * 3.0)));
            sc += col_hex(0x7ad8a0u) * l.lamp * 0.25 * exp(-sq((q.y - 0.02) / 0.01)) * (1.0 - abs(q.x) / 0.13);
            sc += col_hex(0x0e6a3au) * room_light(vec3f(p, -0.55), vec3f(0.0, 0.5, -1.0), l, t, flash) * 0.4;
            let trim = smoothstep(0.006, 0.002, abs(q.y + 0.022));
            sc = mix(sc, col_hex(0xb8903cu) * (0.2 + l.lamp * 0.6), trim);
            col = mix(col, sc, aa_fill(shade, ctx));
        }
        // bright opening under the shade
        let under = exp(-sq(q.x / 0.09) - sq((q.y + 0.03) / 0.008)) * step(q.y, -0.02);
        col += col_kelvin(2900.0) * l.lamp * 0.6 * under;
    }

    // ---- dust motes: drifting in the shaft (and faintly in the lamplight)
    {
        let mq = p * 30.0 + vec2f(t * 0.55, -t * 0.12);
        let c = vec2i(floor(mq));
        let h = hash_cell2(c, 0xd057u);
        if (h.w < 0.2) {
            let wob = vec2f(sin(t * 0.3 + h.x * 20.0), cos(t * 0.23 + h.y * 20.0)) * 0.2;
            let d = length(fract(mq) - 0.5 - (h.xy - 0.5) * 0.5 - wob);
            let r = max(0.06, aa * 30.0 * 0.7);
            let mote = smoothstep(r, r * 0.3, d) * (0.4 + 0.6 * h.z) * (0.06 / r);
            let lampzone = exp(-sq((p.x - LAMP.x) / 0.16) - sq((p.y + 0.18) / 0.12)) * l.lamp;
            let candlezone = exp(-sq((p.x - CANDLE.x) / 0.1) - sq((p.y - CANDLE.y - 0.05) / 0.1)) * l.candle;
            col += (l.win_c * shaft * 2.0 + col_kelvin(2700.0) * lampzone * 1.4 + col_kelvin(1900.0) * candlezone * 0.8) * mote
                   * smoothstep(DESK_BACK - 0.01, DESK_BACK + 0.03, p.y);
        }
    }
    return col * exp2(l.exposure);
}
