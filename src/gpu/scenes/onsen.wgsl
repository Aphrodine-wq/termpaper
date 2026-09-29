//! name: onsen
//! title: Mountain Onsen
//! category: cozy
//! tags: onsen, hot spring, steam, snow, lanterns, pines, japan, mountains
//! desc: an outdoor hot spring steaming among snowy rocks, stone lanterns glowing by the pines
//! themes: snow, night, autumn
//! uses: sdf, snow, noise, stars, l2d
//! cost: light
//! fallback: campfire
//! credits: original

// An outdoor bath at a mountain inn. The pool fills the foreground, ringed
// with rounded boulders; its dark water mirrors the sky and the lanterns,
// and steam rolls off it in slow wisps. A stone lantern (a tōrō) glows on
// the right, another further back; pines stand behind, and past them the
// mountains in the last light (or under stars, or in autumn colour).
// Snow lies on the rocks and keeps falling; in autumn it is maples instead,
// and their leaves drifting down onto the water.

const WATER: f32 = -0.12;       // the far edge of the pool
const LANTERN: vec2f = vec2f(0.55, -0.1);

struct Look {
    mode: u32,          // 0 snow (blue hour), 1 clear night, 2 autumn dusk
    sky_top: vec3f,
    sky_low: vec3f,
    ridge: vec3f,
    trees: vec3f,
    lamp: vec3f,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, col_hex(0x05070fu) * 0.5, col_hex(0x1c2440u) * 0.7, col_hex(0x121830u) * 0.6,
                        col_hex(0x070a0eu), col_kelvin(2300.0) * 1.2, 0.7);
        }
        case 2u: {
            return Look(2u, col_hex(0x2a3a64u) * 0.6, col_hex(0xe89060u) * 0.9, col_hex(0x3a3048u) * 0.6,
                        col_hex(0x1a1410u), col_kelvin(2400.0) * 1.1, 0.0);
        }
        default: {
            return Look(0u, col_hex(0x1a2440u) * 0.6, col_hex(0x5a6a94u) * 0.7, col_hex(0x2a3452u) * 0.6,
                        col_hex(0x0c1218u), col_kelvin(2400.0) * 1.15, 0.1);
        }
    }
}

// the sky and everything beyond the pool, also what the water mirrors
fn backdrop(p: vec2f, l: Look, t: f32, ctx: Ctx) -> vec3f {
    let h = saturate((p.y + 0.05) / 0.55);
    var c = mix(l.sky_low, l.sky_top, pow(h, 0.7));
    if (l.mode == 1u) {
        let rd = normalize(vec3f(p.x, p.y + 0.2, 1.4));
        c += star_field(rd, 0.6, ctx) * 0.8;
        // the moon, low over the ridge
        let m = length(p - vec2f(-0.5, 0.33));
        c += vec3f(0.9, 0.92, 1.0) * sstep(0.034, 0.03, m) * 1.4 + vec3f(0.5, 0.6, 0.8) * exp(-m * 9.0) * 0.08;
    }
    if (l.mode == 2u) {
        // the sun just down behind the ridge, thin clouds catching it
        c += col_hex(0xffa060u) * 0.5 * exp(-sq((p.y - 0.06) / 0.1)) * exp(-sq((p.x - 0.2) / 0.6));
        let cl = sstep(0.55, 0.75, noise_fbm2(vec2f(p.x * 2.5, p.y * 9.0) + 2.0, 4));
        c = mix(c, col_hex(0xe8806au) * 0.7, cl * sstep(0.1, 0.2, p.y) * 0.8);
    }
    // two ridges of mountains, the far one paler
    let r1 = 0.09 + l2d_ridge(p.x * 0.8, 0x31u, 0.12, 5);
    let r2 = 0.02 + l2d_ridge(p.x * 1.3 + 4.0, 0x77u, 0.09, 5);
    c = mix(c, mix(l.ridge, l.sky_low, 0.35), sstep(r1 + 0.004, r1 - 0.004, p.y));
    c = mix(c, l.ridge, sstep(r2 + 0.004, r2 - 0.004, p.y));
    if (l.mode == 0u) {
        // snow on the far slopes: patches below the crest
        let patches = sstep(0.45, 0.65, noise_fbm2(vec2f(p.x * 7.0, p.y * 22.0), 4));
        c = mix(c, vec3f(0.55, 0.62, 0.8) * 0.45, sstep(r1 - 0.09, r1 - 0.01, p.y) * patches * 0.6 * sstep(r1 + 0.004, r1 - 0.004, p.y));
    }
    // the pines behind the pool, and in autumn the maples among them
    let tl = -0.02 + l2d_treeline(p.x * 0.9, 0x9e1u, 0u) * 3.2;
    if (p.y < tl) {
        var tc = l.trees;
        if (l.mode == 2u) {
            let m = noise_fbm2(p * vec2f(9.0, 12.0), 4);
            tc = mix(tc, mix(col_hex(0x9a2a14u), col_hex(0xd07a20u), m) * 0.35, sstep(0.45, 0.6, m));
        }
        if (l.mode == 0u) {
            // a dusting of snow: the boughs a shade paler
            tc = mix(tc, vec3f(0.5, 0.56, 0.72) * 0.12, 0.35);
        }
        c = tc;
    }
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let aa = ctx.px;
    let snowy = l.mode == 0u;

    // the lanterns' light on things: the near one, and a far one on the left
    let far_lantern = vec2f(-0.62, -0.03);
    let lamp_at = exp(-length((p - LANTERN) * vec2f(0.8, 1.2)) * 5.0) + 0.5 * exp(-length((p - far_lantern) * vec2f(0.8, 1.2)) * 7.0);
    let ambient = mix(l.sky_low, l.sky_top, 0.5) * 0.5;

    var col = backdrop(p, l, t, ctx);

    // ---- the pool: the backdrop mirrored, darkened, rippled
    if (p.y < WATER) {
        let depth = WATER - p.y;
        let rip = noise_value2(vec2f(p.x * 18.0 + t * 0.15, depth * 60.0 - t * 0.3)) - 0.5;
        let mp = vec2f(p.x + rip * 0.01 * (1.0 + depth * 4.0), 2.0 * WATER - p.y + rip * 0.02);
        var wc = backdrop(mp, l, t, ctx) * 0.32 * vec3f(0.8, 0.95, 1.05);
        // the far edge of the water catches the sky
        wc += mix(l.sky_low, l.sky_top, 0.3) * 0.25 * exp(-depth / 0.01);
        // the lanterns' light on the water: long wavering streaks
        for (var k = 0; k < 2; k++) {
            let lc = select(far_lantern, LANTERN, k == 0);
            let streak = exp(-sq((p.x - lc.x + rip * 0.03) / (0.012 + depth * 0.04))) * sstep(0.0, 0.03, depth);
            let shimmer = 0.6 + 0.4 * sstep(0.3, 0.8, noise_value2(vec2f(p.x * 60.0, depth * 90.0 - t * 1.2)));
            wc += l.lamp * streak * shimmer * select(0.08, 0.16, k == 0) * exp(-depth * 3.0);
        }
        // a dark, mineral-green body under the reflections
        wc = mix(wc, col_hex(0x0e2a26u) * 0.25, 0.3 + 0.3 * sstep(0.0, 0.3, depth));
        col = wc;
        if (l.mode == 2u) {
            // fallen leaves floating, drifting slowly
            let lq = vec2f(p.x * 14.0 + t * 0.02, depth * 26.0);
            let lh = hash_cell2(vec2i(floor(lq)), 0x1eafu);
            let ld = length((fract(lq) - 0.3 - 0.4 * lh.xy) * vec2f(1.0, 1.8));
            col = mix(col, mix(col_hex(0xa02810u), col_hex(0xd08020u), lh.z) * 0.4, step(lh.w, 0.2) * sstep(0.18, 0.1, ld));
        }
    }

    // ---- the boulders round the pool: a near row and the far edge
    for (var i = 0; i < 9; i++) {
        let fi = f32(i);
        let h = hash_cell2(vec2i(i, 2), 0xb01du);
        let near = i < 4;
        var c = select(vec2f(-1.2 + fi * 0.62 + h.x * 0.2, WATER + 0.005), vec2f(-0.95 + fi * 0.62 + h.x * 0.18, -0.46 + h.y * 0.05), near);
        let r = select(vec2f(0.11 + 0.06 * h.z, 0.045 + 0.02 * h.w), vec2f(0.2 + 0.08 * h.z, 0.11 + 0.04 * h.w), near);
        let q = (p - c) / r;
        let lump = 0.08 * (noise_value2(q * 3.0 + fi * 3.0) - 0.5);
        let d = (length(q) - 1.0 + lump) * min(r.x, r.y);
        if (d < aa * 2.0) {
            // dark wet stone, lit from the lantern side and from above
            let up = saturate(q.y * 0.8 + 0.4);
            let toward = saturate(dot(normalize(q + vec2f(0.0, 0.5)), normalize(LANTERN - c)) * 0.5 + 0.5);
            var rc = col_hex(0x3a3a3eu) * (0.7 + 0.3 * noise_value2(q * 9.0 + fi));
            rc *= ambient * (0.4 + 0.8 * up) + l.lamp * lamp_at * (0.6 + 0.8 * toward);
            // a cap of snow on the top
            if (snowy) {
                let cap = sstep(0.1, 0.35, q.y + 0.08 * noise_value2(q * 6.0 + fi));
                rc = mix(rc, vec3f(0.75, 0.8, 0.95) * (ambient * 1.6 + l.lamp * lamp_at * 0.6), cap);
            }
            col = mix(col, rc, aa_fill(d, ctx));
        }
    }

    // ---- the stone lanterns: a pedestal, a firebox glowing, a wide roof
    for (var k = 0; k < 2; k++) {
        let s = select(0.9, 1.9, k == 0);
        let lc = select(far_lantern, LANTERN, k == 0) + select(vec2f(0.0, 0.03), vec2f(0.0, 0.1), k == 0);
        let q = (p - lc) / s;
        let post = sdf2_box(q - vec2f(0.0, -0.08), vec2f(0.012, 0.07));
        let base = sdf2_round_box(q - vec2f(0.0, -0.15), vec2f(0.04, 0.012), 0.004);
        let fire = sdf2_box(q - vec2f(0.0, 0.015), vec2f(0.028, 0.022));
        let roof = max(abs(q.x) - (0.075 - (q.y - 0.045) * 1.6), abs(q.y - 0.055) - 0.016);
        let knob = length(q - vec2f(0.0, 0.085)) - 0.01;
        let stone = min(min(post, base), min(min(roof, knob), fire + 0.008));
        let sc = col_hex(0x4a4a4cu) * (ambient * 0.9 + l.lamp * 0.25);
        col = mix(col, sc, aa_fill(stone * s, ctx));
        // the firebox's paper window, glowing
        let win = sdf2_box(q - vec2f(0.0, 0.015), vec2f(0.018, 0.015));
        col = mix(col, l.lamp * 1.8 * (0.9 + 0.1 * sin(t * 2.3 + f32(k))), aa_fill(win * s, ctx));
        col += l.lamp * 0.12 * exp(-length(q - vec2f(0.0, 0.015)) * 14.0);
        if (snowy) {
            let cap = aa_fill(max(abs(q.x) - (0.07 - (q.y - 0.07) * 1.4), abs(q.y - 0.073) - 0.007) * s, ctx);
            col = mix(col, vec3f(0.78, 0.82, 0.95) * (ambient * 1.5 + l.lamp * 0.15), cap);
        }
    }

    // ---- steam rolling off the water
    {
        let over = p.y - WATER;
        if (over > -0.4 && over < 0.3) {
            let q = vec2f(p.x * 2.2 - t * 0.03, p.y * 3.0 - t * 0.06);
            let n = noise_fbm2(q + vec2f(noise_fbm2(q * 1.3 + 7.0, 3) * 1.2), 5);
            let body = sstep(0.28, -0.05, over) * sstep(-0.38, -0.1, over);
            let wisp = sstep(0.48, 0.78, n) * body;
            let sc = ambient * 1.8 + vec3f(0.08) + l.lamp * lamp_at * 0.35;
            col = mix(col, col + sc * 0.7, wisp * select(0.5, 0.35, l.mode == 2u));
        }
    }

    // ---- what falls: snow, or maple leaves
    if (snowy) {
        col = mix(col, vec3f(0.8, 0.85, 0.95) * (ambient * 2.2 + vec3f(0.05) + l.lamp * lamp_at * 0.3), snow_flakes(p * 1.8, ctx, 0.3, 0.02, 2) * 0.8);
    }
    if (l.mode == 2u) {
        for (var k = 0; k < 2; k++) {
            let fk = f32(k);
            let q = vec2f(p.x * (6.0 + fk * 3.0) + sin(t * 0.3 + p.y * 3.0 + fk) * 0.4, p.y * (6.0 + fk * 3.0) + t * (0.25 + fk * 0.1));
            let lh = hash_cell2(vec2i(floor(q)) + vec2i(k * 57, 0), 0x3a91u);
            if (lh.w < 0.12) {
                let f = fract(q) - 0.2 - 0.6 * lh.xy;
                let spin = rot2(t * (0.5 + lh.z) + lh.x * 6.0);
                let ld = length((spin * f) * vec2f(1.0, 2.2)) - 0.12;
                col = mix(col, mix(col_hex(0xb02810u), col_hex(0xe09030u), lh.z) * (ambient * 1.4 + l.lamp * lamp_at * 0.7), sstep(0.03, -0.03, ld));
            }
        }
    }
    return col * exp2(l.exposure);
}
