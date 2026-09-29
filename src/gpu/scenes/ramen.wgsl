//! name: ramen
//! title: Ramen Counter
//! category: cozy
//! tags: ramen, noodles, steam, lanterns, noren, rain, japan, night, interior
//! desc: a bowl steaming on a ramen counter under paper lanterns, rain beyond the noren
//! themes: rain, night, snow
//! uses: sdf, bokeh, rain, snow, noise
//! cost: light
//! fallback: city
//! credits: original

// A seat at the counter of a small ramen shop at night. In front, a bowl:
// broth, noodles, a halved soft egg, slices of chashu, a sheet of nori,
// scallions, the steam curling up off it. Behind the counter a stockpot
// billows under a hood, wooden menu tags hang along the wall, and red paper
// lanterns glow from the beams. The doorway on the left has its noren half
// drawn, and through it the street: wet, neon smeared on the tarmac, rain
// or snow coming down, somebody's umbrella going by.

const COUNTER: f32 = -0.2;              // the counter's back edge
const BOWL: vec2f = vec2f(0.16, -0.27); // the bowl's rim centre
const DOOR: vec2f = vec2f(-0.62, 0.02); // the doorway's centre
const DOOR_H: vec2f = vec2f(0.2, 0.34);

struct Look {
    mode: u32,          // 0 rain, 1 clear night, 2 snow
    street: vec3f,
    neon: f32,
    lamp: vec3f,        // the lanterns' warm light
    fill: vec3f,        // the shop's ceiling light
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, col_hex(0x14121cu) * 0.4, 1.3, col_kelvin(2300.0) * 1.1, col_hex(0x9aa4c0u) * 0.16, 0.1);
        }
        case 2u: {
            return Look(2u, col_hex(0x3a4258u) * 0.45, 0.7, col_kelvin(2300.0) * 1.1, col_hex(0xa0acc8u) * 0.18, 0.1);
        }
        default: {
            return Look(0u, col_hex(0x10141cu) * 0.4, 1.0, col_kelvin(2300.0) * 1.1, col_hex(0x9aa4c0u) * 0.16, 0.15);
        }
    }
}

fn soft_box(p: vec2f, c: vec2f, h: vec2f, blur: f32) -> f32 {
    return smoothstep(blur, -blur, sdf2_box(p - c, h));
}

// ------------------------------------------------------------------ outside
fn street(p: vec2f, l: Look, t: f32, ctx: Ctx) -> vec3f {
    var c = l.street * (0.6 + 0.6 * sstep(-0.1, 0.35, p.y));
    // the shops across the way: dim fronts, signs lit in neon
    let sx = p.x / 0.16;
    let sid = floor(sx);
    let sh = hash_cell2(vec2i(i32(sid), 2), 0x5e0u);
    let sign = soft_box(vec2f(fract(sx), p.y), vec2f(0.5, 0.12 + 0.08 * sh.x), vec2f(0.2, 0.03 + 0.03 * sh.y), 0.03);
    let nc = mix(col_hex(0xff3a6au), mix(col_hex(0x3ad0ffu), col_hex(0xffb03au), step(0.5, sh.z)), step(0.35, sh.z));
    c += nc * sign * 0.55 * l.neon;
    let front = soft_box(vec2f(fract(sx), p.y), vec2f(0.5, -0.02), vec2f(0.38, 0.07), 0.04);
    c += col_kelvin(3000.0) * front * 0.12 * (0.4 + 0.6 * sh.w);
    // the wet street: signs smeared downward in it
    let road = sstep(-0.1, -0.14, p.y);
    c = mix(c, l.street * 0.5, road);
    let refl = soft_box(vec2f(fract(sx), -0.28 - p.y), vec2f(0.5, 0.12 + 0.08 * sh.x), vec2f(0.16, 0.06), 0.06);
    c += nc * refl * road * 0.25 * l.neon * select(0.4, 1.0, l.mode == 0u) * (0.7 + 0.3 * noise_value2(vec2f(p.x * 40.0, p.y * 6.0)));
    // bokeh of far lights
    let bf = bokeh_field(p + vec2f(4.2, 1.0), 34.0, 0.25, ctx);
    c += mix(col_kelvin(2500.0), nc, bf.y) * bf.x * (0.06 + 0.15 * bf.z) * sstep(-0.05, 0.1, p.y);
    // somebody going by under an umbrella
    let ev = hash_event(t, 14.0, 0x0a1u);
    if (ev.x < 0.65) {
        let x = DOOR.x - 0.4 + ev.y * 0.8;
        let q = (p - vec2f(x, 0.02)) / 0.16;
        let dome = max(length(q * vec2f(1.0, 2.0)) - 0.5, -q.y);
        let body = sdf2_round_box(q - vec2f(0.0, -0.55), vec2f(0.15, 0.5), 0.14);
        let who = smoothstep(0.04, -0.04, min(select(1e3, dome, l.mode != 1u), body) * 0.16);
        c = mix(c, l.street * 0.25, who * 0.8);
    }
    if (l.mode == 0u) {
        c += vec3f(0.6, 0.7, 0.85) * rain_streaks(p, ctx, 0.5, 1.2, 0.08, 2) * 0.12;
    }
    if (l.mode == 2u) {
        c = mix(c, vec3f(0.85, 0.88, 0.95) * 0.6, snow_flakes(p * 1.3, ctx, 0.5, 0.02, 2) * 0.8);
        // snow lying on the street
        c = mix(c, vec3f(0.55, 0.58, 0.66) * 0.5, road * 0.6);
    }
    return c;
}

// ------------------------------------------------------------------ steam
fn steam_plume(p: vec2f, base: vec2f, width: f32, height: f32, t: f32, salt: f32) -> f32 {
    let s = p.y - base.y;
    if (s < 0.0 || s > height) { return 0.0; }
    let sway = width * 0.6 * sin(s * 9.0 / height - t * 0.6 + salt) * sstep(0.0, height * 0.3, s);
    let x = p.x - base.x - sway;
    let w = width * (1.0 + s / height * 2.5);
    let plume = exp(-sq(x / w));
    let n = noise_fbm2(vec2f(x * 12.0 / width * 0.05 + salt, s * 7.0 / height - t * 0.4), 4);
    return plume * sstep(0.42, 0.75, n) * sstep(0.0, height * 0.12, s) * sstep(height, height * 0.35, s);
}

// ------------------------------------------------------------------ scene
fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let aa = ctx.px;

    // the lanterns: a row along the beam (none in front of the doorway),
    // and the light they throw
    var lan_light = vec3f(0.0);
    for (var i = -4; i < 6; i++) {
        let lx = -0.2 + f32(i) * 0.36;
        if (abs(lx - DOOR.x) < DOOR_H.x + 0.05) { continue; }
        lan_light += l.lamp * 0.22 * exp(-length((p - vec2f(lx, 0.3)) * vec2f(0.8, 1.0)) * 3.2);
    }
    let light = lan_light + l.fill;

    // ---- the back wall: dark wood panels
    let panel = fract(p.x * 4.0 + 0.5);
    let wood = col_hex(0x3e2c22u) * (0.75 + 0.15 * noise_value2(vec2f(p.x * 6.0, p.y * 60.0)) + 0.1 * sstep(0.02, 0.0, abs(panel - 0.5) - 0.47));
    var col = wood * light * 0.8;
    // menu tags: a row of pale wooden boards with dark brushed marks
    if (p.y > 0.08 && p.y < 0.2) {
        let tx = (p.x + 3.0) / 0.075;
        let tid = floor(tx);
        let th = hash_cell2(vec2i(i32(tid), 4), 0x7a9u);
        let tag = soft_box(vec2f(fract(tx), p.y), vec2f(0.5, 0.14), vec2f(0.38, 0.055), 0.03);
        let tq = vec2f(fract(tx), (p.y - 0.14) / 0.055);
        // the "writing": short vertical strokes
        let stroke = step(abs(tq.x - 0.5), 0.1) * step(0.5, fract(tq.y * 2.5 + th.x * 3.0)) * step(abs(tq.y), 0.8);
        let tc = mix(col_hex(0xe8d8b0u), col_hex(0xd8a870u), th.y) * 0.8;
        let beside_door = abs(p.x - DOOR.x) > DOOR_H.x + 0.06;
        col = mix(col, mix(tc, col_hex(0x201814u), stroke * 0.85) * light, tag * select(0.0, 1.0, beside_door));
    }
    // shelves of bowls behind the counter
    let shelf_y = -0.02;
    let shelf = soft_box(p, vec2f(0.3, shelf_y), vec2f(0.65, 0.008), aa);
    col = mix(col, col_hex(0x3a2418u) * light, shelf);
    let bx = (p.x - 0.3 + 3.0) / 0.07;
    let bq = vec2f(fract(bx) - 0.5, (p.y - shelf_y - 0.008) / 0.035);
    let bowl_s = step(bq.y, 1.0) * step(0.0, bq.y) * step(abs(bq.x), 0.42 - bq.y * 0.12) * step(abs(p.x - 0.3), 0.6);
    let bc = mix(col_hex(0xe8e0d0u), col_hex(0x2a3a5au), step(0.6, hash_cell2(vec2i(i32(floor(bx)), 1), 0xb0u).x));
    col = mix(col, bc * light * 0.9, bowl_s);

    // ---- the stockpot under its hood, steam boiling off it
    {
        let pot_c = vec2f(0.62, -0.12);
        let pot = sdf2_round_box(p - pot_c, vec2f(0.12, 0.08), 0.01);
        let u = clamp((p.x - pot_c.x) / 0.12, -1.0, 1.0);
        let steel = col_hex(0xb0b4bcu) * (0.5 + 0.5 * (1.0 - u * u)) * (light * 0.8 + l.lamp * 0.05);
        col = mix(col, steel + l.lamp * 0.1 * exp(-sq((u + 0.4) / 0.15)), aa_fill(pot, ctx));
        // a steel hood: wide at the bottom, narrowing to its flue
        let hq = p - vec2f(0.62, 0.3);
        let hood = max(abs(hq.x) - (0.2 - (hq.y + 0.035) * 1.6), abs(hq.y) - 0.035);
        let flue = max(abs(hq.x) - 0.04, abs(hq.y - 0.12) - 0.09);
        let hood_c = col_hex(0x9aa0a8u) * (light * 0.6 + l.fill * 0.5) * (0.7 + 0.3 * sstep(-0.2, 0.2, hq.x));
        col = mix(col, hood_c, aa_fill(min(hood, flue), ctx));
        let st = steam_plume(p, vec2f(0.62, -0.04), 0.06, 0.28, t, 1.7);
        col = mix(col, col + (l.lamp * 0.35 + l.fill * 1.5) * select(1.0, 1.4, l.mode == 2u), st * 0.5);
    }

    // ---- the doorway on the left: the street, the noren, the frame
    {
        let dq = p - DOOR;
        let dd = sdf2_box(dq, DOOR_H);
        if (dd < aa * 2.0) {
            col = mix(col, street(p, l, t, ctx), aa_fill(dd, ctx));
        }
        let frame = max(dd - 0.018, -dd);
        col = mix(col, col_hex(0x2a1a12u) * light, aa_fill(frame, ctx));
        // the noren: indigo cloth panels hanging from the top of the door,
        // swaying a little, a white crest across them
        let top = DOOR.y + DOOR_H.y;
        let nx = (p.x - DOOR.x + DOOR_H.x) / (DOOR_H.x * 2.0);
        let strip = floor(nx * 3.0);
        let sway = 0.006 * sin(t * 0.7 + strip * 1.3) * sstep(top, top - 0.15, p.y);
        let nlen = 0.26 + 0.01 * sin(strip * 3.1);
        let in_n = step(0.0, nx) * step(nx, 1.0) * step(top - nlen, p.y + sway * 0.5) * step(p.y, top);
        // the slits between the panels
        let gap = aa_fill(0.47 - abs(fract(nx * 3.0 + sway * 4.0) - 0.5), ctx);
        if (in_n > 0.0) {
            var nc = col_hex(0x2a4a8au) * (light * 1.3 + l.lamp * 0.12 + l.street * 0.5);
            // the crest: a pale ring across the middle panels
            let crest = abs(length((p - vec2f(DOOR.x, top - 0.11)) * vec2f(1.0, 1.1)) - 0.055) - 0.009;
            nc = mix(nc, col_hex(0xe8e0d0u) * light * 0.9, aa_fill(crest, ctx));
            col = mix(col, nc, in_n * (1.0 - gap * 0.9));
        }
    }

    // ---- the lanterns: red paper, ribbed, glowing
    for (var i = -4; i < 6; i++) {
        let lc = vec2f(-0.2 + f32(i) * 0.36, 0.33);
        if (abs(lc.x - DOOR.x) < DOOR_H.x + 0.05) { continue; }
        let q = (p - lc) / vec2f(0.055, 0.075);
        let body = (length(q) - 1.0) * 0.055;
        if (body < aa * 2.0) {
            let ribs = 0.85 + 0.15 * cos(q.y * 22.0);
            let glow = (1.0 - 0.5 * length(q)) * ribs;
            let lcol = col_hex(0xe03a24u) * 1.6 * glow + l.lamp * 0.35 * exp(-length(q) * 2.0);
            col = mix(col, lcol, aa_fill(body, ctx));
        }
        // black caps and the cord
        let cap = sdf2_box(p - lc - vec2f(0.0, 0.078), vec2f(0.03, 0.008));
        let cap2 = sdf2_box(p - lc + vec2f(0.0, 0.078), vec2f(0.03, 0.008));
        let cord = max(abs(p.x - lc.x) - 0.0015, lc.y + 0.08 - p.y);
        col = mix(col, vec3f(0.015), aa_fill(min(min(cap, cap2), cord), ctx));
        col += l.lamp * 0.05 * exp(-length(p - lc) * 9.0);
    }

    // ---- the counter
    if (p.y < COUNTER) {
        let v = saturate((COUNTER - p.y) / (COUNTER + 0.5));
        let wx = p.x * (1.0 + 0.6 * v);
        let grain = 0.8 + 0.14 * noise_value2(vec2f(wx * 1.5, v * 70.0)) + 0.06 * sin(wx * 3.0 + noise_value2(vec2f(v * 9.0, wx)) * 4.0);
        let alb = col_hex(0xd8bc8cu) * 0.5 * grain;
        col = alb * (light * (0.9 + 0.6 * v) + l.lamp * 0.12);
        col += l.lamp * 0.15 * exp(-sq((p.y - COUNTER + 0.004) / 0.005));
        // the bowl's shadow
        col *= 1.0 - 0.5 * exp(-sq(length((p - BOWL - vec2f(0.0, -0.06)) * vec2f(1.0, 3.0)) / 0.22));
    }

    // ---- the bowl
    {
        let q = p - BOWL;
        // a deep bowl seen from a little above: an elliptic rim, a curved body
        let rim_r = vec2f(0.25, 0.085);
        let body = max(length(q / vec2f(rim_r.x * (1.0 + q.y * 1.2), rim_r.x * 0.62)) - 1.0, q.y);
        let foot = sdf2_round_box(q - vec2f(0.0, -0.13), vec2f(0.08, 0.012), 0.006);
        let bd = min(body * 0.1, foot);
        let u = clamp(q.x / rim_r.x, -1.0, 1.0);
        // black lacquer outside with a red band, lit from the lanterns above
        var bc = col_hex(0x1a0e0cu) * (light * 0.7) + l.lamp * 0.25 * exp(-sq((u + 0.35) / 0.15)) * sstep(-0.12, -0.02, q.y);
        bc = mix(bc, col_hex(0x8a1a12u) * light * 0.9, exp(-sq((q.y + 0.03) / 0.008)));
        col = mix(col, bc, aa_fill(bd, ctx));
        // the broth and what floats in it
        let rim = length(q / rim_r) - 1.0;
        if (rim < 0.05) {
            let inner = length(q / (rim_r - vec2f(0.012, 0.005))) - 1.0;
            var fc = col_hex(0x1a0e0cu) * light;
            if (inner < 0.0) {
                let bq2 = q / (rim_r - vec2f(0.012, 0.005));
                // tonkotsu broth, a sheen of oil catching the lanterns
                var broth = col_hex(0xd8b078u) * 0.55 * (light + l.lamp * 0.2);
                broth += l.lamp * 0.2 * sstep(0.7, 0.9, noise_value2(bq2 * 7.0 + vec2f(t * 0.05, 0.0)));
                // noodles: pale wavy strands
                let nd = abs(sin(bq2.x * 30.0 + sin(bq2.y * 9.0) * 2.0)) * sstep(0.2, -0.6, bq2.y + bq2.x * 0.3);
                broth = mix(broth, col_hex(0xf0dca0u) * 0.7 * (light + l.lamp * 0.2), sstep(0.8, 0.95, nd) * 0.8);
                // chashu: two pink-brown slices on the right
                let ch = min(length((bq2 - vec2f(0.42, 0.1)) / vec2f(0.28, 0.32)), length((bq2 - vec2f(0.55, -0.3)) / vec2f(0.25, 0.3))) - 1.0;
                let chc = mix(col_hex(0xb0705au), col_hex(0xe8b8a0u), sstep(0.0, -0.6, ch)) * 0.6 * (light + l.lamp * 0.2);
                broth = mix(broth, chc, aa_fill_w(ch * 0.06, aa));
                // the egg: a halved ajitama, amber yolk
                let eq = (bq2 - vec2f(-0.35, 0.12)) / vec2f(0.24, 0.36);
                let egg = length(eq) - 1.0;
                let yolk = length(eq * 1.6 + vec2f(0.0, 0.1)) - 1.0;
                broth = mix(broth, col_hex(0xf4ead8u) * 0.75 * (light + l.lamp * 0.25), aa_fill_w(egg * 0.06, aa));
                broth = mix(broth, col_hex(0xf0a020u) * 0.8 * (light + l.lamp * 0.25), aa_fill_w(yolk * 0.04, aa));
                // scallion rings scattered over the middle
                let sc_g = fract(bq2 * vec2f(6.0, 9.0) + vec2f(0.3, 0.1)) - 0.5;
                let sc_h = hash_cell2(vec2i(floor(bq2 * vec2f(6.0, 9.0) + vec2f(0.3, 0.1))), 0x5ca1u);
                let ring = abs(length(sc_g) - 0.18) - 0.06;
                broth = mix(broth, col_hex(0x6ab04au) * 0.7 * (light + l.lamp * 0.2), step(sc_h.x, 0.25) * sstep(0.02, -0.02, ring) * sstep(0.1, -0.3, bq2.y + 0.2));
                fc = broth;
            }
            // the rim itself: a thin bright edge
            let rim_line = aa_fill(abs(rim) * 0.05 - 0.002, ctx);
            fc = mix(fc, col_hex(0x2a1814u) * light + l.lamp * 0.3 * sstep(0.0, 0.05, q.y + 0.02), rim_line);
            col = mix(col, fc, aa_fill(rim * 0.05, ctx));
        }
        // the nori: a dark sheet standing up out of the back of the bowl
        let nq = (p - BOWL - vec2f(-0.07, 0.07)) * rot2(0.12);
        let nori = sdf2_box(nq, vec2f(0.055, 0.06));
        let noc = col_hex(0x1e2a22u) * (light * 0.9 + l.lamp * 0.1) * (0.8 + 0.2 * noise_value2(nq * 120.0));
        col = mix(col, noc, aa_fill(max(nori, -(length(q / rim_r) - 0.92) * 0.05 * select(0.0, 1.0, nq.y < -0.03)), ctx));
        // chopsticks across the bowl
        let cs1 = sdf2_segment(p, BOWL + vec2f(-0.06, 0.07), BOWL + vec2f(0.34, 0.035)) - 0.004;
        let cs2 = sdf2_segment(p, BOWL + vec2f(-0.05, 0.085), BOWL + vec2f(0.35, 0.052)) - 0.0035;
        col = mix(col, col_hex(0xd8b890u) * 0.7 * (light + l.lamp * 0.3), aa_fill(min(cs1, cs2), ctx));
        // steam off the bowl, lit by the lanterns
        let st = steam_plume(p, BOWL + vec2f(0.0, 0.03), 0.06, 0.34, t, 0.3);
        col = mix(col, col + l.lamp * 0.28 + l.fill * 0.8, st * select(0.45, 0.6, l.mode == 2u));
    }
    return col * exp2(l.exposure);
}
