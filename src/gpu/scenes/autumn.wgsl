//! name: autumn
//! title: Maple Lake
//! category: wilds
//! tags: autumn, maple, lake, reflection, leaves, mist, fall, forest
//! desc: red and gold maples around a still lake, mist on the water, leaves drifting down
//! themes: morning, golden, rain
//! uses: sdf, noise, rain, l2d
//! cost: medium
//! fallback: alpine
//! credits: original

// A still lake in autumn. Far hills of maple and birch in reds, oranges and
// golds rise to a ridge of darker pines and a mountain beyond; the shore is
// mirrored in the lake, broken only by slow ripples and, in the rain, by
// rings everywhere. Mist lies on the water in the morning; late in the day
// the low sun sets the leaves glowing. Branches of a maple overhang the
// top of the picture, and its leaves spin down past the lens onto the
// water, where a few already float.

const SHORE: f32 = -0.1;       // the far shore's waterline

struct Look {
    mode: u32,          // 0 morning, 1 golden, 2 rain
    sky_top: vec3f,
    sky_low: vec3f,
    sun: vec3f,         // light on the foliage
    haze: vec3f,
    mist: f32,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, col_hex(0x5a7ab8u) * 0.8, col_hex(0xf8c890u) * 1.1, col_kelvin(3000.0) * 1.3,
                        col_hex(0xe8b890u) * 0.7, 0.15, -0.3);
        }
        case 2u: {
            return Look(2u, col_hex(0x6a7280u) * 0.6, col_hex(0x9aa0a8u) * 0.65, col_hex(0xa8aab0u) * 0.55,
                        col_hex(0x8a9098u) * 0.6, 0.35, 0.1);
        }
        default: {
            return Look(0u, col_hex(0x8aa8d0u) * 0.8, col_hex(0xd8dce4u) * 0.9, col_kelvin(5000.0) * 0.85,
                        col_hex(0xc8d0dcu) * 0.8, 0.7, -0.1);
        }
    }
}

// autumn foliage colour at `p`: reds, oranges, golds, a few evergreens
fn foliage(p: vec2f, depth: f32, l: Look, t: f32) -> vec3f {
    let q = p * vec2f(26.0, 34.0) / (0.6 + depth);
    let crowns = noise_fbm2(q, 4);
    let kind = noise_value2(q * 0.25 + 3.0);
    var c = col_ramp4(col_hex(0x8a1a10u), col_hex(0xc8401cu), col_hex(0xe08a24u), col_hex(0xe8c040u), 0.3, 0.6, kind);
    // a few darker crowns (oaks still green-brown) among them
    c = mix(c, col_hex(0x4a3a1eu), sstep(0.7, 0.8, noise_fbm2(q * 0.7 + 9.0, 3)) * 0.6);
    // crowns: lit tops, shadowed gaps
    let lit = 0.3 + 1.0 * sstep(0.3, 0.72, crowns);
    return c * 0.55 * (l.sun * lit + l.sky_low * 0.25);
}

// everything above the water: sky, mountain, hills, shore
fn above(p: vec2f, l: Look, t: f32, ctx: Ctx) -> vec3f {
    let h = saturate((p.y - SHORE) / 0.6);
    var c = mix(l.sky_low, l.sky_top, pow(h, 0.8));
    if (l.mode == 1u) {
        // the low sun off to the left, its glow
        c += col_hex(0xffc080u) * 0.6 * exp(-length((p - vec2f(-0.75, 0.12)) * vec2f(0.6, 1.0)) * 3.0);
    }
    if (l.mode == 2u) {
        c *= 0.85 + 0.25 * noise_fbm2(p * vec2f(1.5, 4.0) + vec2f(t * 0.008, 0.0), 4);
    }
    // a mountain far off, hazy
    let m = 0.14 + l2d_ridge(p.x * 0.6 + 1.3, 0x44u, 0.16, 5);
    c = mix(c, mix(col_hex(0x4a5068u) * 0.8, l.haze, 0.55), sstep(m + 0.004, m - 0.004, p.y));
    // the far ridge of pines
    let r = 0.05 + l2d_ridge(p.x * 1.1 + 7.0, 0x91u, 0.07, 5) + l2d_treeline(p.x * 1.4, 0x2cu, 0u) * 1.5;
    if (p.y < r) {
        c = mix(col_hex(0x1a2a24u) * (l.sun * 0.35 + l.sky_low * 0.3), l.haze, 0.45);
    }
    // the maple hills down to the shore, rounded crowns along their top
    let hills = 0.0 + l2d_ridge(p.x * 1.6 + 2.0, 0x17u, 0.06, 4) + l2d_treeline(p.x * 2.2 + 1.0, 0x7fu, 1u) * 1.6;
    if (p.y < hills) {
        let depth = saturate((hills - p.y) / 0.15);
        c = mix(foliage(p, 0.4, l, t), l.haze, 0.18 * (1.0 - depth));
    }
    // mist hanging along the shore
    c = mix(c, l.haze, l.mist * 0.6 * exp(-max(p.y - SHORE, 0.0) / 0.06));
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    var col = above(p, l, t, ctx);

    // ---- the lake
    if (p.y < SHORE) {
        let depth = SHORE - p.y;
        var rip = noise_value2(vec2f(p.x * 12.0 + t * 0.05, depth * 70.0 - t * 0.25)) - 0.5;
        rip *= 0.5 + depth * 2.0;
        if (l.mode == 2u) {
            // rain rings everywhere on the water
            let rq = vec2f(p.x * 30.0, depth * 90.0);
            let rc = floor(rq);
            let rh = hash_cell2(vec2i(rc), 0x5a1u);
            let ph = fract(t * 0.9 + rh.x);
            let ring = abs(length((fract(rq) - 0.5) * vec2f(1.0, 2.2)) - ph * 0.5) - 0.04;
            rip += sstep(0.04, 0.0, ring) * (1.0 - ph) * 0.8;
        }
        let mp = vec2f(p.x + rip * 0.006, 2.0 * SHORE - p.y + rip * 0.012);
        var wc = above(mp, l, t, ctx) * 0.72;
        // darker toward us, the sky's sheen farther out
        wc *= 0.8 + 0.2 * exp(-depth * 5.0);
        if (l.mode == 1u) {
            // the sun's path across the water
            wc += col_hex(0xffc080u) * 0.4 * exp(-sq((p.x + 0.75 + rip * 0.05) / (0.04 + depth * 0.4))) * sstep(0.5, 0.0, depth) * (0.6 + 0.4 * sstep(0.3, 0.8, noise_value2(vec2f(p.x * 80.0, depth * 120.0 - t))));
        }
        // mist lying on the water
        let mist = noise_fbm2(vec2f(p.x * 2.0 - t * 0.02, depth * 8.0), 4);
        wc = mix(wc, l.haze, l.mist * sstep(0.35, 0.75, mist) * exp(-depth * 4.0) * 0.8);
        // leaves floating, drifting slowly
        let lq = vec2f(p.x * 18.0 / (0.3 + depth * 3.0) + t * 0.01, depth * 60.0 / (0.3 + depth * 3.0));
        let lh = hash_cell2(vec2i(floor(lq)), 0x1ea4u);
        let ld = length((fract(lq) - 0.3 - 0.4 * lh.xy) * vec2f(1.0, 2.0));
        let leaf_c = mix(col_hex(0xb02a14u), col_hex(0xe0a030u), lh.z) * 0.6 * (l.sun * 0.8 + l.sky_low * 0.2);
        wc = mix(wc, leaf_c, step(lh.w, 0.07) * sstep(0.2, 0.12, ld));
        col = wc;
    }

    // ---- the overhanging maple: a canopy of leaves across the top, its
    // lower edge scalloped, sky showing through the gaps
    {
        let top = ctx.half.y;
        let edge = top - 0.11 - 0.07 * noise_fbm2(vec2f(p.x * 2.6, 1.0), 3) - 0.02 * sin(p.x * 11.0);
        if (p.y > edge - 0.03) {
            let q = p * vec2f(20.0, 26.0) + vec2f(0.0, sin(t * 0.3) * 0.05);
            let n = noise_fbm2(q, 4);
            let fine = noise_value2(q * 3.0);
            // clusters of leaves; gaps where the sky shows through
            let clusters = sstep(0.34, 0.46, n + 0.15 * (fine - 0.5));
            let cover = sstep(edge - 0.015, edge + 0.02, p.y + 0.03 * (n - 0.5)) * clusters;
            let kind = noise_value2(p * vec2f(3.0, 4.0) + 5.0);
            let tone = col_ramp4(col_hex(0x8a160eu), col_hex(0xc8361au), col_hex(0xe0801eu), col_hex(0xecb83au), 0.3, 0.65, kind);
            // leaves glow where the light comes through them
            let glow = select(0.2, 0.7, l.mode == 1u) * sstep(0.5, 0.8, n);
            let leaf_c = tone * 0.6 * (l.sun * (0.35 + 0.8 * n + glow) + l.sky_low * 0.25);
            col = mix(col, leaf_c, cover);
        }
    }

    // ---- leaves spinning down past the lens
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        let sc = 7.0 + fk * 5.0;
        let q = vec2f(p.x * sc + sin(t * 0.35 + p.y * 3.0 + fk * 2.0) * 0.6 - t * 0.05, p.y * sc + t * (0.12 + fk * 0.06) * sc);
        let h = hash_cell2(vec2i(floor(q)) + vec2i(k * 71, 0), 0x6e1fu);
        if (h.w < 0.08) {
            let f = fract(q) - 0.2 - 0.6 * h.xy;
            let spin = rot2(t * (0.8 + h.z * 1.5) + h.x * 6.0);
            let lp = (spin * f) * vec2f(1.0, 1.0 + 0.8 * abs(sin(t * 1.3 + h.y * 6.0)));
            let ang = atan2(lp.y, lp.x);
            let leaf = length(lp) - 0.12 * (0.75 + 0.25 * cos(ang * 5.0));
            let tone = mix(col_hex(0xb82010u), col_hex(0xe8a030u), h.z);
            col = mix(col, tone * 0.7 * (l.sun * 0.9 + l.sky_low * 0.3), sstep(0.03, -0.03, leaf));
        }
    }
    if (l.mode == 2u) {
        col += vec3f(0.7, 0.72, 0.76) * rain_streaks(p, ctx, 0.45, 1.3, 0.05, 2) * 0.08;
    }
    return col * exp2(l.exposure);
}
