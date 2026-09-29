//! name: cafe
//! title: Coffee Shop Window
//! category: cozy
//! tags: cafe, coffee, steam, rain, bokeh, window, interior
//! desc: a steaming cup at a cafe window, the rainy street outside melting into bokeh
//! themes: rain, snow, morning
//! uses: sdf, bokeh, rain, snow, noise
//! cost: light
//! fallback: rain
//! credits: original

// A seat at the window bar of a cafe. The lens is focused on the cup, so the
// street outside is built directly as a defocused image: soft shop windows,
// bokeh discs for every light, smeared reflections on the wet road, cars and
// umbrellas drifting past as blurred shapes. Over it sit the glass (rain or
// snow outside, condensation inside) and the window frame; in front, the
// counter, the cup and its steam.

const COUNTER: f32 = -0.26;            // back edge of the counter (window line)
const CUP: vec2f = vec2f(0.3, -0.1);   // centre of the cup's rim
const CUP_R: f32 = 0.125;              // rim half-width
const CUP_S: f32 = 1.667;              // cup scale relative to the drawing units
const MUL: f32 = 1.2;                  // mullion spacing

struct Look {
    mode: u32,        // 0 rain, 1 snow, 2 morning
    sky: vec3f,       // outside base (sky / shadowed facades)
    street: vec3f,
    shop: f32,        // shop-window brightness
    inside: vec3f,    // interior light on the counter/cup
    back: vec3f,      // window backlight on the cup
    fog: f32,         // condensation amount
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, col_hex(0x2a3a5cu) * 0.4, col_hex(0x8a98b8u) * 0.25, 1.0,
                        col_hex(0xffd8a8u) * 0.5, col_hex(0x9ab0e0u) * 0.3, 0.75, 0.3);
        }
        case 2u: {
            return Look(2u, col_hex(0xd8b890u) * 0.9, col_hex(0x8a8078u) * 0.55, 0.3,
                        col_hex(0xffe0b8u) * 0.55, col_hex(0xffd8a0u) * 1.4, 0.12, -0.3);
        }
        default: {
            return Look(0u, col_hex(0x1c2a44u) * 0.4, col_hex(0x202838u) * 0.3, 1.0,
                        col_hex(0xffd0a0u) * 0.45, col_hex(0x8098c8u) * 0.25, 0.6, 0.3);
        }
    }
}

// ------------------------------------------------------------------ outside
// a soft (defocused) rectangle: coverage 0..1
fn soft_box(p: vec2f, c: vec2f, h: vec2f, blur: f32) -> f32 {
    let d = sdf2_box(p - c, h);
    return smoothstep(blur, -blur, d);
}

// a person under an umbrella, blurred: coverage
fn umbrella(p: vec2f, c: vec2f, s: f32, blur: f32) -> f32 {
    let q = (p - c) / s;
    let dome = max(length(q * vec2f(1.0, 2.0)) - 0.5, -q.y);
    let body = sdf2_round_box(q - vec2f(0.0, -0.55), vec2f(0.16, 0.5), 0.14);
    let d = min(dome, body) * s;
    return smoothstep(blur, -blur, d);
}

fn outside(p: vec2f, l: Look, t: f32, ctx: Ctx) -> vec3f {
    let morning = l.mode == 2u;
    var c = l.sky * (0.7 + 0.5 * smoothstep(-0.1, 0.5, p.y));
    // the building across the street: dark facade, rows of soft windows
    let fl = floor((p.y - 0.16) / 0.13);
    let fx = floor(p.x / 0.16);
    let wh = hash_cell2(vec2i(i32(fx), i32(fl)), 0xcafeu);
    if (p.y > 0.14) {
        let win = soft_box(vec2f(fract(p.x / 0.16), fract((p.y - 0.16) / 0.13)), vec2f(0.5, 0.45), vec2f(0.28, 0.28), 0.12);
        let lit = step(wh.x, select(0.45, 0.2, morning));
        c += col_kelvin(2600.0 + 1500.0 * wh.y) * win * lit * select(0.18, 0.05, morning) * l.shop;
        if (morning) { c = mix(c, col_hex(0x3a3a44u) * 0.2, win * 0.7); }
    }
    // shopfronts at street level: big warm soft panes, an awning
    let sp = p.x / 0.5;
    let sid = floor(sp);
    let sh = hash_cell2(vec2i(i32(sid), 7), 0x5409u);
    let pane = soft_box(vec2f(fract(sp), p.y), vec2f(0.5, 0.0), vec2f(0.38, 0.1), 0.05);
    let shopc = mix(col_kelvin(2500.0), col_kelvin(4000.0), sh.x);
    c = mix(c, shopc * (0.5 + 0.5 * sh.y) * select(0.9, 0.35, morning) * l.shop + c * 0.3, pane * 0.9);
    // an awning edge over one shop
    let awn = soft_box(vec2f(fract(sp), p.y), vec2f(0.5, 0.13), vec2f(0.45, 0.02), 0.03) * step(0.6, sh.z);
    c = mix(c, mix(col_hex(0x7a2a24u), col_hex(0x2a4a3au), sh.w) * select(0.08, 0.3, morning), awn);
    // the street: wet, below the far kerb
    let road = smoothstep(-0.09, -0.12, p.y);
    c = mix(c, l.street * (0.8 + 0.2 * smoothstep(-0.26, -0.12, p.y)), road);
    // reflections of the shop panes in the wet road, smeared downward
    let refl = soft_box(vec2f(fract(sp), -0.22 - p.y), vec2f(0.5, 0.0), vec2f(0.36, 0.1), 0.07);
    c += shopc * refl * road * select(0.25, 0.08, morning) * l.shop * (0.7 + 0.3 * noise_value2(vec2f(p.x * 30.0, p.y * 4.0)));
    // bokeh: street lamps, signs, a traffic light
    let bf = bokeh_field(p + vec2f(3.1, 0.0), 5.0, 0.16, ctx);
    let bf2 = bokeh_field(p * 1.0 + vec2f(7.7, 2.0), 13.0, 0.1, ctx);
    let upper = smoothstep(0.05, 0.18, p.y);
    c += mix(col_kelvin(2300.0), col_kelvin(4000.0), bf.y) * bf.x * (0.25 + 0.4 * bf.z) * upper * select(0.5, 0.05, morning);
    c += mix(col_kelvin(2500.0), col_kelvin(3600.0), bf2.y) * bf2.x * (0.2 + 0.3 * bf2.z) * upper * select(0.4, 0.04, morning);
    let neon = bokeh_disc(p, vec2f(-0.32, 0.2), 0.05, ctx) * 0.8 + bokeh_disc(p, vec2f(-0.25, 0.21), 0.045, ctx) * 0.6;
    c += col_hex(0xff5a8au) * neon * select(0.35, 0.05, morning);
    let tl_phase = fract(t / 30.0);
    let tl_col = select(select(col_hex(0x40ff80u), col_hex(0xffb020u), tl_phase > 0.55), col_hex(0xff3020u), tl_phase > 0.62);
    c += tl_col * bokeh_disc(p, vec2f(0.62, 0.05), 0.035, ctx) * select(0.9, 0.2, morning);
    c += tl_col * 0.25 * soft_box(p, vec2f(0.62, -0.2), vec2f(0.02, 0.05), 0.03) * road;
    // cars: a pair of lamps and a dark body sliding past every so often
    for (var k = 0; k < 2; k++) {
        let dir = select(-1.0, 1.0, k == 0);
        let ev = hash_event(t + f32(k) * 4.3, 9.0 + f32(k) * 2.0, 0xca5u + u32(k));
        if (ev.x < 0.7) {
            let x = dir * (-1.6 + ev.y * 3.2);
            let y = -0.18 + f32(k) * 0.025;
            let body = soft_box(p, vec2f(x, y + 0.02), vec2f(0.2, 0.05), 0.04);
            c = mix(c, l.street * 0.3, body * 0.7);
            let lc = select(vec3f(1.0, 0.08, 0.03) * 0.8, col_kelvin(5000.0) * 1.3, k == 0);
            let front = x + dir * 0.17;
            c += lc * (bokeh_disc(p, vec2f(front, y), 0.035, ctx) + bokeh_disc(p, vec2f(front - dir * 0.08, y), 0.035, ctx)) * select(1.0, 0.3, morning);
            // their glare in the wet road
            c += lc * 0.2 * soft_box(p, vec2f(front - dir * 0.04, y - 0.06), vec2f(0.06, 0.05), 0.04) * road * select(1.0, 0.2, morning);
        }
    }
    // people walking past: near ones large and very soft
    for (var k = 0; k < 3; k++) {
        let fk = f32(k);
        let near = k == 0;
        let per = 13.0 + fk * 5.0;
        let ev = hash_event(t + fk * 7.7, per, 0x9e0u + u32(k));
        if (ev.x < 0.8) {
            let dir = select(-1.0, 1.0, ev.x < 0.4);
            let span = select(1.4, 2.2, near);
            let x = dir * (-span + ev.y * span * 2.0);
            let s = select(0.16, 0.34, near);
            let y = select(0.02, -0.02, near);
            let blur = select(0.025, 0.05, near);
            var cov = 0.0;
            if (l.mode == 0u) { cov = umbrella(p, vec2f(x, y), s, blur); }
            else {
                // no umbrella: a figure with a hood or hat
                let q = (p - vec2f(x, y - s * 0.3)) / s;
                let d = min(length(q - vec2f(0.0, 0.25)) - 0.14, sdf2_round_box(q - vec2f(0.0, -0.3), vec2f(0.2, 0.45), 0.15)) * s;
                cov = smoothstep(blur, -blur, d);
            }
            let tone = select(l.street * 0.25, col_hex(0x3a3028u) * 0.4, morning);
            c = mix(c, tone, cov * select(0.7, 0.85, near));
        }
    }
    // weather out there
    if (l.mode == 1u) {
        // snow: soft out-of-focus flakes drifting down, plus fine ones
        let big = bokeh_field(p + vec2f(sin(t * 0.3) * 0.05, t * 0.06), 7.0, 0.1, ctx);
        c += vec3f(0.85, 0.9, 1.0) * big.x * 0.12;
        c = mix(c, vec3f(0.8, 0.85, 0.95) * 0.6, snow_flakes(p, ctx, 0.4, 0.02, 2) * 0.5);
    }
    if (morning) {
        // sunlit facade across the street: warm stone, shadowed window reveals,
        // a strip of blue sky above the cornice
        c = mix(c, col_hex(0x8aa8d0u) * 1.0, smoothstep(0.44, 0.47, p.y));
        c += col_hex(0xffd8a0u) * 0.35 * exp(-length(p - vec2f(-0.9, 0.6)) * 2.0);
    }
    return c;
}

// ------------------------------------------------------------------ steam
fn steam(p: vec2f, t: f32) -> f32 {
    let s = p.y - (CUP.y + 0.015);
    if (s < 0.0 || s > 0.4) { return 0.0; }
    let sway = 0.022 * sin(s * 16.0 - t * 0.7) * smoothstep(0.0, 0.12, s) + 0.012 * sin(s * 7.0 + t * 0.31);
    let x = p.x - CUP.x - sway;
    let w = 0.045 + s * 0.24;
    let plume = exp(-sq(x / w));
    let n = noise_fbm2(vec2f(x * 22.0, s * 11.0 - t * 0.45), 4);
    let n2 = noise_value2(vec2f(x * 9.0 + 3.0, s * 5.0 - t * 0.25));
    let wisps = smoothstep(0.45, 0.8, n * 0.7 + n2 * 0.45);
    return plume * wisps * smoothstep(0.0, 0.03, s) * smoothstep(0.4, 0.1, s);
}

// ------------------------------------------------------------------ scene
fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let aa = ctx.px;
    var col: vec3f;

    // ---- the window
    var op = p;
    var drop = 0.0;
    if (l.mode == 0u) {
        let rg = rain_glass(p * 1.3, t, ctx);
        op = p + rg.xy * 0.5;
        drop = rg.z;
    }
    col = outside(op, l, t, ctx);
    if (drop > 0.0) { col *= 1.0 + drop * 0.35; }
    // condensation on the inside: a milky veil, heavier low down and at the edges
    let fogn = noise_fbm2(p * vec2f(3.0, 5.0) + 11.0, 4);
    var fog = l.fog * saturate(0.35 + 0.9 * smoothstep(0.1, -0.3, p.y) + 0.3 * (fogn - 0.5));
    let veil = mix(l.inside * 0.12 + l.sky * 0.4, l.inside * 0.2, 0.5);
    let bloom = outside(p * 0.9 + vec2f(0.0, 0.02), l, t, ctx);
    col = mix(col, veil + bloom * 0.35, fog * 0.7);
    // tiny beads of condensation catching the room light
    let bq = p * 60.0;
    let bh = hash_cell2(vec2i(floor(bq)), 0xbeadu);
    if (bh.w < fog * 0.5) {
        let d = length(fract(bq) - 0.5 - (bh.xy - 0.5) * 0.5);
        col += l.inside * 0.08 * smoothstep(0.2, 0.05, d) * saturate(0.3 / (aa * 60.0));
    }
    // window frame: slim black steel mullions and a transom
    let mx = abs(fmod_pos(p.x + MUL * 0.5, MUL) - MUL * 0.5 - 0.0);
    let mullion = aa_fill(abs(p.x - MUL * round(p.x / MUL) + MUL * 0.5 - MUL * 0.5) - 0.0, ctx);
    let md = min(abs(fmod_pos(p.x - MUL * 0.5, MUL) - MUL * 0.5) - 0.012, abs(p.y - 0.36) - 0.01);
    let fc = vec3f(0.012) + l.back * 0.05 + l.inside * 0.02;
    col = mix(col, fc, aa_fill(md, ctx));
    // the sill at the bottom of the glass
    let sill = aa_fill(abs(p.y - COUNTER - 0.012) - 0.012, ctx);
    col = mix(col, vec3f(0.02) + l.inside * 0.05, sill);

    // ---- the counter: oak, in perspective
    if (p.y < COUNTER) {
        let v = (p.y - COUNTER) / (-0.5 - COUNTER);
        let z = 1.0 / (0.25 + 0.75 * saturate(1.0 - v) + 0.001);
        let wx = p.x * (1.0 + v * 0.6);
        let grain = 0.8 + 0.15 * noise_value2(vec2f(wx * 3.0, v * 40.0)) + 0.08 * sin(wx * 4.0 + noise_value2(vec2f(v * 8.0, wx)) * 6.0);
        let plank = smoothstep(0.004, 0.0, abs(fract(v * 3.0) - 0.5) - 0.495);
        let alb = col_hex(0x9a6a3eu) * 0.55 * grain * (1.0 - 0.3 * plank);
        // light: room pendants from above, the window from behind
        var lit = l.inside * (0.7 + 0.5 * v) + l.back * (0.6 - 0.4 * v);
        // the window's light reflects in the varnish near the glass
        var cc = alb * lit + l.back * 0.12 * smoothstep(0.35, 0.0, v);
        if (l.mode == 2u) {
            // sunlight through the glass: mullion shadows across the counter
            let sx = p.x - (p.y - COUNTER) * 1.3;
            let msh = smoothstep(0.015, 0.06, abs(fmod_pos(sx - MUL * 0.5, MUL) - MUL * 0.5));
            cc += alb * l.back * 0.9 * msh * (1.0 - 0.3 * v);
        }
        col = cc;
        // the cup's shadow and its saucer's contact shadow
        let shq = vec2f(p.x - CUP.x - (p.y - CUP.y + 0.14) * select(0.0, -0.9, l.mode == 2u), (p.y - (CUP.y - 0.17)) * select(3.0, 1.2, l.mode == 2u));
        let shadow = exp(-sq(length(shq) / 0.2));
        col *= 1.0 - 0.55 * shadow;
    }

    // ---- saucer, cup, coffee, drawn in a local frame scaled by CUP_S
    {
        let cup0 = vec2f(0.24, -0.2);
        let cp = cup0 + (p - CUP) / CUP_S;
        var cctx = ctx;
        cctx.px = ctx.px / CUP_S;
        let caa = aa / CUP_S;
        // saucer: a flat ellipse under the cup
        let sq0 = (cp - vec2f(cup0.x, cup0.y - 0.125)) / vec2f(0.15, 0.035);
        let sd = (length(sq0) - 1.0) * 0.035;
        if (sd < caa) {
            let rim = smoothstep(-0.006, 0.0, sd);
            var sc = vec3f(0.88) * (l.inside * 1.2 + l.back * 0.3) * (0.75 + 0.25 * saturate(sq0.y + 0.5));
            sc += l.back * 0.4 * rim * smoothstep(0.0, 0.8, sq0.y);
            col = mix(col, sc, aa_fill(sd, cctx));
        }
        // cup body: a slightly tapered cylinder from the rim down to the foot
        let q = cp - cup0;
        let hb = 0.105;
        let taper = mix(0.075, 0.075 * 0.78, saturate(-q.y / hb));
        let bottom = (length(vec2f(q.x / taper, (q.y + hb) / 0.02)) - 1.0) * 0.02;
        var body = max(abs(q.x) - taper, max(q.y, -q.y - hb));
        body = min(body, max(bottom, -q.y - hb - 0.001) );
        body = max(body, -(q.y + hb + 0.02));
        // handle: a ring on the right
        let hq = q - vec2f(0.075 * 0.95, -0.045);
        let handle = abs(length(hq * vec2f(1.0, 0.85)) - 0.028) - 0.008;
        let hdl = max(handle, -hq.x);
        let cd = min(body, hdl);
        if (cd < caa) {
            // cylindrical shading: lit by the room from the left, the window
            // rims both edges from behind
            let u = clamp(q.x / taper, -1.0, 1.0);
            let nz = sqrt(max(1.0 - u * u, 0.0));
            // a pendant lamp above-left: bright glaze, a soft terminator on the right
            let key = saturate(0.55 - u * 0.75) * nz;
            var cc = vec3f(0.93, 0.92, 0.89) * (l.inside * (0.25 + 1.3 * key) + l.back * 0.12);
            cc += l.back * 1.1 * pow(1.0 - nz, 3.0);
            // glaze highlight: a tall soft streak
            cc += l.inside * 1.4 * exp(-sq((u + 0.42) / 0.07)) * smoothstep(-0.1, -0.03, q.y);
            if (hdl < body) { cc = vec3f(0.85) * (l.inside * 0.4 + l.back * 0.5); }
            col = mix(col, cc, aa_fill(cd, cctx));
        }
        // rim and the coffee inside, seen from a little above
        let rq = q / vec2f(0.075, 0.022);
        let rd = (length(rq) - 1.0) * 0.022;
        if (rd < caa) {
            let inner = (length(q / vec2f(0.075 - 0.006, 0.018)) - 1.0) * 0.018;
            var rc = vec3f(0.92) * (l.inside * 1.1 + l.back * 0.4);
            if (inner < 0.0) {
                // crema with a pale heart of milk foam
                let cq = q / vec2f(0.075 - 0.006, 0.018);
                var coffee = col_hex(0x6a3a1cu) * 0.5 * (l.inside * 0.9 + l.back * 0.3);
                let foam = smoothstep(0.75, 0.55, length(cq * vec2f(1.0, 1.0)));
                let heart = smoothstep(0.02, -0.02, length(vec2f(abs(cq.x) - 0.18, cq.y - 0.1)) - 0.28 + cq.y * 0.3);
                coffee = mix(coffee, col_hex(0xc8a070u) * 0.6 * (l.inside + l.back * 0.3), foam * 0.8);
                coffee = mix(coffee, col_hex(0xf0e0c8u) * 0.7 * (l.inside + l.back * 0.3), heart * foam);
                rc = coffee;
            }
            col = mix(col, rc, aa_fill(rd, cctx));
        }
        // a spoon on the saucer
        let sp = sdf2_segment(cp, vec2f(cup0.x - 0.09, cup0.y - 0.13), vec2f(cup0.x - 0.2, cup0.y - 0.1)) - 0.004;
        let bowl = length((cp - vec2f(cup0.x - 0.08, cup0.y - 0.132)) / vec2f(1.6, 1.0)) - 0.009;
        let spd = min(sp, bowl);
        col = mix(col, vec3f(0.6) * (l.inside * 0.8 + l.back * 0.6), aa_fill(spd, cctx));
    }

    // ---- steam curling up from the cup, lit by the room and backlit by the glass
    let st = steam(p, t);
    if (st > 0.0) {
        let sc = l.inside * select(0.7, 0.35, l.mode == 2u) + l.back * select(0.5, 0.9, l.mode == 2u);
        col = mix(col, col + sc, st * select(0.5, 0.35, l.mode == 2u));
    }
    return col * exp2(l.exposure);
}
