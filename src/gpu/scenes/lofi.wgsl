//! name: lofi
//! title: Late Night Desk
//! category: cozy
//! tags: lofi, desk, lamp, laptop, cat, rain, window, study, night
//! desc: a desk lamp and a laptop's glow, a cat asleep by a rainy window over the city
//! themes: rain, snow, dawn
//! uses: sdf, bokeh, rain, snow, noise
//! cost: light
//! fallback: den
//! credits: original

// A desk pushed against a window, late. A lamp on an arm pools warm light
// on the desk; the laptop's screen throws a cool one; a mug steams beside
// the lamp; the cat sleeps curled on the desk by the glass, breathing. The
// window looks over the city: towers with lit windows, lights melted into
// bokeh, rain on the glass (or snow outside, or the first light of dawn).
// Everything is drawn flat, back to front, lit by those three lights.

const DESK: f32 = -0.17;                    // the desk's back edge
const WIN_C: vec2f = vec2f(0.12, 0.2);      // window centre
const WIN_H: vec2f = vec2f(0.46, 0.25);     // window half size
const LAMP_POOL: vec2f = vec2f(-0.34, -0.3);
const SCREEN_C: vec2f = vec2f(-0.02, -0.07);
const CAT: vec2f = vec2f(0.4, -0.215);

struct Look {
    mode: u32,          // 0 rain, 1 snow, 2 dawn
    sky_top: vec3f,
    sky_low: vec3f,
    city: f32,          // how many windows are lit
    lamp: vec3f,        // the desk lamp's light
    room: vec3f,        // fill light in the room
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, col_hex(0x0e1424u) * 0.35, col_hex(0x3a4660u) * 0.4, 0.5,
                        col_kelvin(2900.0) * 1.2, col_hex(0x7a88b0u) * 0.12, 0.25);
        }
        case 2u: {
            return Look(2u, col_hex(0x3a5a9au) * 0.55, col_hex(0xf0a070u) * 0.9, 0.16,
                        col_kelvin(3000.0) * 0.7, col_hex(0xffc8a0u) * 0.14, -0.1);
        }
        default: {
            return Look(0u, col_hex(0x0a0e1cu) * 0.3, col_hex(0x2a2440u) * 0.35, 0.55,
                        col_kelvin(2800.0) * 1.25, col_hex(0x6a78a8u) * 0.11, 0.3);
        }
    }
}

fn soft_box(p: vec2f, c: vec2f, h: vec2f, blur: f32) -> f32 {
    return smoothstep(blur, -blur, sdf2_box(p - c, h));
}

// ------------------------------------------------------------------ the city
fn city(p: vec2f, l: Look, t: f32, ctx: Ctx) -> vec3f {
    let dawn = l.mode == 2u;
    // the sky, lighter toward the horizon (the city's glow at night)
    let h = saturate((p.y - 0.02) / 0.45);
    var c = mix(l.sky_low, l.sky_top, sqrt(h));
    if (dawn) {
        // a band of warm light low down, thin clouds across it
        c += col_hex(0xffb070u) * 0.5 * exp(-sq((p.y - 0.03) / 0.07));
        let cl = sstep(0.55, 0.75, noise_fbm2(vec2f(p.x * 3.0, p.y * 12.0) + 4.0, 4));
        c = mix(c, col_hex(0xe0907au) * 0.6, cl * sstep(0.02, 0.1, p.y) * sstep(0.35, 0.15, p.y));
    }
    // far towers, then near ones, each a slab of windows
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        let sc = select(1.0, 0.55, k == 1);
        let x = p.x / sc + fk * 3.1;
        let w = 0.07;
        let cell = floor(x / w);
        let hh = hash_cell2(vec2i(i32(cell), k), 0x1f0cu);
        let top = (0.1 + 0.2 * pow(hh.y, 1.5)) * sc + select(0.0, -0.03, k == 1);
        let lx = fract(x / w);
        let inside = lx > 0.08 && lx < 0.92 && p.y < top;
        if (inside) {
            let far = select(0.55, 1.0, k == 1);
            var b = mix(l.sky_low * 0.35, vec3f(0.01, 0.012, 0.02), far);
            if (dawn) { b = mix(col_hex(0x2a2a40u) * 0.35, col_hex(0x3a3048u) * 0.25, far); }
            // windows: a grid, some lit
            let g = vec2f(lx * 6.0, (p.y / sc) * 55.0);
            let wc = vec2i(floor(g)) + vec2i(i32(cell) * 17, k * 31);
            let wh = hash_cell2(wc, 0x77u);
            let wf = fract(g);
            let pane = step(0.2, wf.x) * step(wf.x, 0.8) * step(0.25, wf.y) * step(wf.y, 0.75);
            let lit = step(wh.x, l.city * (0.6 + 0.4 * hh.z));
            b += mix(col_kelvin(2600.0), col_kelvin(4300.0), wh.y) * pane * lit * (0.25 + 0.4 * wh.z) * (0.5 + 0.5 * far);
            c = b;
        }
    }
    // city lights melted into bokeh below the skyline
    let low = sstep(0.12, -0.02, p.y);
    let bf = bokeh_field(p + vec2f(1.7, 0.0), 22.0, 0.4, ctx);
    let tint = mix(col_kelvin(2400.0), mix(col_kelvin(5200.0), col_hex(0xff6090u), step(0.8, bf.z)), bf.y);
    c += tint * bf.x * low * (0.1 + 0.25 * bf.z) * select(1.0, 0.25, dawn);
    // a car's lights far below, crossing now and then
    let ev = hash_event(t, 11.0, 0xca7u);
    if (ev.x < 0.6) {
        let x = -1.2 + ev.y * 2.4;
        c += vec3f(1.0, 0.25, 0.1) * bokeh_disc(p, vec2f(x, -0.03), 0.012, ctx) * 0.8 * select(1.0, 0.3, dawn);
    }
    if (l.mode == 1u) {
        // snow falling past the window
        c = mix(c, vec3f(0.75, 0.8, 0.9) * 0.55, snow_flakes(p * 1.2, ctx, 0.45, 0.03, 2) * 0.8);
    }
    return c;
}

// ------------------------------------------------------------------ steam
fn steam(p: vec2f, base: vec2f, t: f32) -> f32 {
    let s = p.y - base.y;
    if (s < 0.0 || s > 0.26) { return 0.0; }
    let sway = 0.016 * sin(s * 18.0 - t * 0.8) * sstep(0.0, 0.1, s);
    let x = p.x - base.x - sway;
    let w = 0.02 + s * 0.16;
    let plume = exp(-sq(x / w));
    let n = noise_fbm2(vec2f(x * 30.0, s * 14.0 - t * 0.5), 4);
    return plume * sstep(0.45, 0.78, n) * sstep(0.0, 0.03, s) * sstep(0.26, 0.08, s);
}

// ------------------------------------------------------------------ scene
fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let aa = ctx.px;
    let night = l.mode != 2u;

    // light falling on things: the lamp's pool, the screen's glow, the
    // fairy lights over the window, a floor lamp off to the right, the room
    let lamp_at = exp(-sq(length((p - LAMP_POOL) * vec2f(0.9, 1.6)) / 0.28));
    let screen_at = exp(-sq(length((p - SCREEN_C) * vec2f(0.9, 1.3)) / 0.32));
    let screen_c = col_hex(0x7aa0ffu) * 0.35;
    let string_y = WIN_C.y + WIN_H.y + 0.035;
    let fairy_c = col_kelvin(2400.0) * select(0.35, 0.1, !night);
    let fairy_at = exp(-abs(p.y - string_y) * 2.5) * sstep(1.0, 0.4, abs(p.x - WIN_C.x));
    let floor_at = exp(-length((p - vec2f(1.15, 0.02)) * vec2f(0.8, 1.0)) * 2.2)
        + exp(-length((p - vec2f(-1.4, 0.05)) * vec2f(0.8, 1.0)) * 2.2);
    let light = l.lamp * lamp_at + screen_c * screen_at + fairy_c * fairy_at + l.lamp * 0.35 * floor_at + l.room;

    // ---- the wall around the window (continues past any edge)
    let wall_alb = col_hex(0x7a6858u) * (0.9 + 0.1 * noise_value2(p * 40.0));
    var col = wall_alb * (light * 0.6 + l.room * 0.8);
    // the lamp's glow washes up the wall behind it
    col += l.lamp * 0.05 * exp(-length((p - vec2f(-0.44, 0.02)) * vec2f(1.2, 0.8)) * 4.0);
    // a shelf on the far left with a few books, and a poster on the right
    let shelf = soft_box(p, vec2f(-0.78, 0.18), vec2f(0.2, 0.008), aa);
    col = mix(col, col_hex(0x3a2a1eu) * (light + l.room), shelf);
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let bh = hash_cell2(vec2i(i, 3), 0xb00cu);
        let bx = -0.93 + fi * 0.045;
        let book = soft_box(p, vec2f(bx, 0.188 + 0.04 * bh.x), vec2f(0.017, 0.035 + 0.04 * bh.x), aa);
        let bc = mix(col_hex(0x8a3a2au), mix(col_hex(0x2a4a6au), col_hex(0xb09050u), bh.y), bh.z) * 0.6;
        col = mix(col, bc * (light * 0.8 + l.room * 2.0), book);
    }
    let poster = soft_box(p, vec2f(0.78, 0.2), vec2f(0.1, 0.14), aa);
    let pq = (p - vec2f(0.78, 0.2)) / vec2f(0.1, 0.14);
    var pc = mix(col_hex(0x243a5au), col_hex(0xe09a6au), sstep(-0.2, 0.6, pq.y));
    pc = mix(pc, col_hex(0xf6e0b0u), exp(-sq(length(pq - vec2f(0.2, 0.3)) / 0.18)));
    pc = mix(pc, col_hex(0x1a2030u), sstep(-0.1, -0.3, pq.y + 0.1 * sin(pq.x * 5.0)));
    col = mix(col, pc * (light * 0.5 + l.room * 3.0 + 0.02), poster);

    // ---- the window
    let wq = p - WIN_C;
    let in_win = sdf2_box(wq, WIN_H);
    if (in_win < aa * 2.0) {
        var op = p;
        var drop = 0.0;
        if (l.mode == 0u) {
            let rg = rain_glass(p * 1.4, t, ctx);
            op = p + rg.xy * 0.45;
            drop = rg.z;
        }
        var wc = city(op, l, t, ctx);
        wc *= 1.0 + drop * 0.4;
        // the room's reflection in the glass: the lamp, faint
        wc += l.lamp * 0.035 * exp(-length((p - vec2f(-0.2, -0.02)) * vec2f(1.0, 1.4)) * 6.0) * select(1.0, 0.3, !night);
        col = mix(col, wc, aa_fill(in_win, ctx));
        // the frame: painted wood, a cross of glazing bars
        let bars = min(abs(wq.x) - 0.009, abs(wq.y - 0.02) - 0.008);
        let frame = max(in_win, -(in_win + 0.02));
        let fd = min(max(bars, in_win), frame);
        let fc = col_hex(0xd8d0c0u) * (light * 0.45 + l.room * 1.2 + select(vec3f(0.0), l.sky_low * 0.3, !night));
        col = mix(col, fc, aa_fill(fd, ctx));
    }
    // the fairy lights: a sagging string of warm dots over the window
    {
        let sx = p.x - WIN_C.x;
        let sag = 0.018 * cos(sx * 5.2);
        let sy = string_y - sag;
        let wire = aa_fill(abs(p.y - sy) - 0.0015, ctx) * sstep(0.62, 0.55, abs(sx));
        col = mix(col, vec3f(0.02), wire * 0.7);
        let cell = floor(sx / 0.07 + 0.5);
        let bx = cell * 0.07;
        let by = string_y - 0.018 * cos(bx * 5.2) - 0.008;
        let bh = hash_cell2(vec2i(i32(cell), 9), 0xfa1u);
        let tw = 0.75 + 0.25 * sin(t * (0.5 + bh.x) + bh.y * 6.0);
        let bulb = exp(-sq(length(p - vec2f(WIN_C.x + bx, by)) / 0.008)) * step(abs(bx), 0.6);
        let bc = mix(col_kelvin(2300.0), col_hex(0xffc070u), bh.z);
        col += bc * bulb * tw * select(1.6, 0.5, !night);
    }
    // floor lamps past both edges of a 16:9 frame (walls reach them)
    for (var k = 0; k < 2; k++) {
        let lx = select(-1.4, 1.15, k == 0);
        let top = select(0.15, 0.12, k == 0);
        let shade = sdf2_tri(vec2f(p.x - lx, -(p.y - top)), 0.1, 0.12);
        col = mix(col, l.lamp * 0.9 + col_hex(0xf0d8b0u) * 0.1, aa_fill(shade, ctx));
        let pole = aa_fill(max(abs(p.x - lx) - 0.006, p.y - top + 0.1), ctx);
        col = mix(col, col_hex(0x1a1a1eu), pole);
    }
    // snow on the outside ledge
    if (l.mode == 1u) {
        let ledge = soft_box(p, vec2f(WIN_C.x, WIN_C.y - WIN_H.y + 0.02), vec2f(WIN_H.x - 0.02, 0.012 + 0.004 * sin(p.x * 30.0)), aa);
        col = mix(col, vec3f(0.7, 0.75, 0.85) * 0.35, ledge);
    }
    // the sill inside
    let sill = soft_box(p, vec2f(WIN_C.x, WIN_C.y - WIN_H.y - 0.012), vec2f(WIN_H.x + 0.05, 0.012), aa);
    col = mix(col, col_hex(0xd0c8b8u) * (light * 0.55 + l.room * 1.5), sill);

    // ---- the desk
    if (p.y < DESK) {
        let v = saturate((DESK - p.y) / (DESK + 0.5));
        let wx = p.x * (1.0 + v * 0.5);
        let grain = 0.82 + 0.12 * noise_value2(vec2f(wx * 2.5, v * 50.0)) + 0.06 * sin(wx * 5.0 + noise_value2(vec2f(v * 7.0, wx)) * 5.0);
        let alb = col_hex(0x7a5a40u) * 0.5 * grain;
        col = alb * (light * (0.8 + 0.4 * v) + l.room);
        // the desk's edge catches the lamp
        col += l.lamp * 0.12 * exp(-sq((p.y - DESK + 0.004) / 0.006)) * sstep(-0.9, -0.1, p.x);
        // the window's light lying on the desk near the glass
        if (!night) {
            col += alb * l.sky_low * 0.5 * sstep(0.35, 0.0, v) * sstep(-0.4, 0.0, p.x);
        }
    }

    // ---- the lamp: a weighted base, two arm segments, a conical shade
    {
        let base = vec2f(-0.62, -0.25);
        let elbow = vec2f(-0.56, 0.1);
        let head = vec2f(-0.38, 0.06);
        let bd = sdf2_round_box(p - base, vec2f(0.07, 0.012), 0.01);
        let a1 = sdf2_segment(p, base + vec2f(0.0, 0.01), elbow) - 0.007;
        let a2 = sdf2_segment(p, elbow, head) - 0.006;
        let joint = length(p - elbow) - 0.012;
        let metal = min(min(bd, joint), min(a1, a2));
        let mc = col_hex(0x2a2a30u) * (light * 0.9 + l.room) + l.lamp * 0.08 * sstep(0.02, 0.0, abs(p.x - base.x + 0.01) - 0.004);
        col = mix(col, mc, aa_fill(metal, ctx));
        // the shade, tilted toward the desk: dark outside, glowing mouth
        let sq2 = (p - head) * rot2(-0.5);
        let shade = max(sdf2_tri(vec2f(sq2.x, -sq2.y + 0.035), 0.055, 0.075), -sq2.y - 0.04);
        col = mix(col, col_hex(0x3a3a44u) * (light * 0.4 + l.room) + l.lamp * 0.03, aa_fill(shade, ctx));
        let mouth = length((sq2 - vec2f(0.0, -0.038)) * vec2f(1.0, 3.5)) - 0.05;
        col = mix(col, l.lamp * 2.6, aa_fill(mouth, ctx));
        col += l.lamp * 0.1 * exp(-length(sq2 - vec2f(0.0, -0.05)) * 18.0);
    }

    // ---- the laptop: keyboard on the desk, screen up, glowing
    {
        let kq = p - vec2f(-0.03, -0.265);
        let kb = sdf2_round_box(kq, vec2f(0.2 - (kq.y + 0.03) * 0.3, 0.035), 0.01);
        let kc = col_hex(0xa0a4acu) * (light * 0.7 + l.room);
        col = mix(col, kc, aa_fill(kb, ctx));
        // keys: a faint grid
        let kg = fract(vec2f(kq.x * 55.0, kq.y * 60.0));
        let keys = step(0.15, kg.x) * step(kg.x, 0.85) * step(0.2, kg.y) * step(kg.y, 0.8) * sstep(0.0, -0.01, kb + 0.012);
        col = mix(col, kc * 0.55, keys * 0.6);
        // the screen: a lid, bezel and display
        let sq3 = p - SCREEN_C;
        let lid = sdf2_round_box(sq3, vec2f(0.2, 0.125), 0.012);
        col = mix(col, col_hex(0x1a1c22u) * (light * 0.5 + l.room), aa_fill(lid, ctx));
        let disp = sdf2_box(sq3, vec2f(0.185, 0.11));
        if (disp < aa) {
            let u = (sq3 + vec2f(0.185, 0.11)) / vec2f(0.37, 0.22);
            // an editor: dark, lines of coloured text, a cursor blinking
            var sc = col_hex(0x16182au) * 0.9;
            let line = floor(u.y * 16.0);
            let lh = hash_cell2(vec2i(i32(line), 1), 0xc0d3u);
            let indent = floor(lh.x * 4.0) * 0.04;
            let len = 0.15 + 0.6 * lh.y;
            let in_line = step(0.08 + indent, u.x) * step(u.x, 0.08 + indent + len) * step(0.3, fract(u.y * 16.0)) * step(fract(u.y * 16.0), 0.75);
            let word = step(0.25, fract(u.x * 9.0 + lh.z * 3.0));
            let tc = mix(col_hex(0x7aa2f7u), mix(col_hex(0x9ece6au), col_hex(0xbb9af7u), step(0.5, lh.z)), step(0.4, lh.w));
            sc = mix(sc, tc * 0.8, in_line * word * step(0.08, u.y) * step(u.y, 0.92));
            // the cursor, on the line being written, blinking once a second
            let cur = step(line, 4.0) * step(4.0, line) * step(0.08 + 0.2, u.x) * step(u.x, 0.08 + 0.2 + 0.012) * step(0.2, fract(u.y * 16.0)) * step(fract(u.y * 16.0), 0.85);
            sc += vec3f(0.8) * cur * step(0.5, fract(t));
            col = mix(col, sc * 1.4, aa_fill(disp, ctx));
        }
    }

    // ---- the mug, steaming
    {
        let mq = p - vec2f(-0.33, -0.255);
        let body = sdf2_round_box(mq, vec2f(0.028, 0.035), 0.006);
        let handle = abs(length((mq - vec2f(0.03, 0.0)) * vec2f(1.0, 0.9)) - 0.016) - 0.005;
        let md = min(body, max(handle, -mq.x + 0.02));
        let u = clamp(mq.x / 0.028, -1.0, 1.0);
        let mc = col_hex(0xc84a3au) * 0.7 * (l.lamp * lamp_at * (0.6 + 0.6 * saturate(0.4 - u)) + l.room + screen_c * screen_at * 0.5);
        col = mix(col, mc, aa_fill(md, ctx));
        let st = steam(p, vec2f(-0.33, -0.215), t);
        col = mix(col, col + l.lamp * 0.25 + screen_c * 0.2, st * 0.45);
    }

    // ---- the cat, curled asleep, breathing
    {
        let breath = 1.0 + 0.018 * sin(t * 1.2);
        let cq = (p - CAT) / vec2f(breath, 1.0 + (breath - 1.0) * 1.5);
        let body = length(cq * vec2f(1.0, 1.75)) - 0.12;
        let head = length((cq - vec2f(-0.1, 0.012)) * vec2f(1.0, 1.15)) - 0.045;
        let ear1 = sdf2_tri(((cq - vec2f(-0.125, 0.052)) * rot2(0.4)) * vec2f(1.0, -1.0) + vec2f(0.0, 0.012), 0.016, 0.028);
        let ear2 = sdf2_tri(((cq - vec2f(-0.085, 0.056)) * rot2(-0.2)) * vec2f(1.0, -1.0) + vec2f(0.0, 0.012), 0.015, 0.026);
        let tail = sdf2_segment(cq, vec2f(0.09, -0.04), vec2f(-0.07, -0.058)) - 0.013;
        let cat = min(min(body, head), min(min(ear1, ear2), tail));
        if (cat < aa * 2.0) {
            // grey tabby: soft stripes, lit by the lamp side and the screen
            let stripes = 0.85 + 0.15 * sin(atan2(cq.y, cq.x) * 9.0 + length(cq) * 30.0);
            let fur = col_hex(0x8a8078u) * stripes * (0.85 + 0.15 * noise_value2(cq * 90.0));
            var cc = fur * (light * 0.9 + l.room * 1.5);
            // the window's light along its back
            cc += fur * (l.sky_low * select(0.25, 0.9, !night)) * sstep(-0.02, 0.06, cq.y);
            col = mix(col, cc, aa_fill(cat, ctx));
            // a closed eye: a thin dark curve
            let eye = abs(length(cq - vec2f(-0.118, 0.018)) - 0.012) - 0.0025;
            col = mix(col, fur * 0.2 * light, aa_fill(max(eye, cq.y - 0.018), ctx) * 0.8);
        }
    }

    // ---- a small plant on the right of the desk
    {
        let pot = sdf2_round_box(p - vec2f(0.68, -0.215), vec2f(0.035 - (p.y + 0.215) * 0.15, 0.032), 0.005);
        col = mix(col, col_hex(0xb86a4au) * 0.6 * (light + l.room), aa_fill(pot, ctx));
        var leaf = 1e3;
        for (var i = 0; i < 5; i++) {
            let fi = f32(i);
            let a = -0.9 + fi * 0.45 + 0.03 * sin(t * 0.3 + fi);
            let dir = vec2f(sin(a), cos(a));
            let lq = p - vec2f(0.68, -0.185) - dir * 0.045;
            let ll = length((lq * rot2(-a)) * vec2f(2.6, 1.0)) - 0.045;
            leaf = min(leaf, ll);
        }
        col = mix(col, col_hex(0x3a6a3au) * 0.55 * (light + l.room * 1.5), aa_fill(leaf, ctx));
    }
    return col * exp2(l.exposure);
}
