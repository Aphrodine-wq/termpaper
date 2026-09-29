//! name: sakura
//! title: Cherry Blossom Canal
//! category: wilds
//! tags: sakura, cherry blossom, canal, petals, lanterns, spring, japan, tokyo
//! desc: cherry trees arching over a Meguro canal, petals drifting onto the water below
//! themes: day, night, rain
//! uses: sdf, noise, rain
//! cost: medium
//! fallback: canopy
//! credits: original

// Looking down a canal from a bridge in cherry-blossom season, in one-point
// perspective worked out per pixel: world X across the canal, Y up from the
// water, Z away from us, the camera on the bridge four metres up. The water
// runs between two concrete walls; above them each bank has a row of
// cherry trees, every tree a billowing mass of blossom leaning out over the
// water, drawn far to near and mirrored in the canal. Paper lanterns hang
// along both walkways. Petals drift down and gather in pale rafts along
// the walls.

const V: vec2f = vec2f(0.0, 0.0);     // vanishing point
const FOCAL: f32 = 1.0;               // focal length (p units)
const EYE: f32 = 5.0;                 // camera height above the water, m
const HALF: f32 = 6.0;                // canal half width, m
const WALL: f32 = 2.4;                // wall top above the water, m
const WALK: f32 = 4.0;                // walkway width, m
const TREES: i32 = 14;                // trees per bank
const TREE_DZ: f32 = 4.6;
const LANTERN_DZ: f32 = 5.0;

struct Look {
    mode: u32,          // 0 day, 1 night, 2 rain
    sky: vec3f,
    sky_low: vec3f,
    sun: vec3f,         // light on the blossom
    blossom: vec3f,
    lamp: vec3f,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, col_hex(0x060818u) * 0.6, col_hex(0x1c1830u) * 0.7, col_hex(0x4a4a6au) * 0.06,
                        col_hex(0xf6d2e0u), col_hex(0xffc0d0u) * 1.1, 0.35);
        }
        case 2u: {
            return Look(2u, col_hex(0x9aa0acu) * 0.6, col_hex(0xc4c8d0u) * 0.65, col_hex(0xb8bcc8u) * 0.42,
                        col_hex(0xf2c2d2u), col_hex(0xffd8e0u) * 0.35, 0.0);
        }
        default: {
            return Look(0u, col_hex(0x7aa8e0u) * 0.95, col_hex(0xdce8f6u) * 1.0, col_kelvin(5600.0) * 0.85,
                        col_hex(0xf6c0d2u), col_hex(0xffe0e8u) * 0.3, -0.2);
        }
    }
}

// screen position of a world point
fn project(w: vec3f) -> vec2f {
    return V + vec2f(w.x, w.y - EYE) * FOCAL / w.z;
}

// one tree's blossom at `p`: (coverage, shade 0..1, depth z) — `mirror`
// draws its reflection in the water
fn tree(p: vec2f, side: f32, k: i32, mirror: bool, t: f32) -> vec3f {
    let h = hash_cell2(vec2i(k, i32(side)), 0x5a2au);
    let z = 6.5 + f32(k) * TREE_DZ + h.x * 1.5 + select(0.0, TREE_DZ * 0.45, side > 0.0);
    let x = side * (HALF + 1.4 - 1.4 * h.y);
    let y = 7.0 + 0.8 * h.z;
    let r = 4.4 + 0.9 * h.w;
    var c = project(vec3f(x, y, z));
    if (mirror) {
        c = project(vec3f(x, -y, z));
    }
    let rs = r * FOCAL / z;
    var q = (p - c) / rs;
    if (mirror) { q.y = -q.y; }
    q.y /= 0.62;
    let dist = length(q);
    if (dist > 1.4) {
        return vec3f(0.0, 0.0, z);
    }
    // a lumpy outline: clusters of blossom bulging out
    let ang = atan2(q.y, q.x);
    let lump = 0.2 * (noise_value2(vec2f(ang * 3.0 + f32(k) * 3.7, side * 5.0 + t * 0.02)) - 0.5) + 0.12 * (noise_value2(q * 6.0 + f32(k)) - 0.5);
    let cov = sstep(1.02, 0.94, dist + lump);
    // clusters inside, brighter on top, shadowed beneath
    let cl = noise_fbm2(q * 3.6 + vec2f(f32(k) * 7.1, side * 3.0), 4);
    let puff = noise_fbm2(q * 9.0 + vec2f(side * 2.0, f32(k)), 3);
    let shade = saturate(0.25 + 0.5 * q.y + 0.9 * (cl - 0.5) + 0.35 * (puff - 0.5));
    return vec3f(cov, shade, z);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let d = p - V;
    let ax = max(abs(d.x), 1e-4);
    let night = l.mode == 1u;
    let fog = select(0.012, 0.03, l.mode == 2u);

    // ---- sky
    var col = mix(l.sky_low, l.sky, sqrt(saturate(d.y / 0.5)));
    if (l.mode == 2u) {
        col *= 0.92 + 0.16 * noise_fbm2(p * vec2f(2.0, 5.0) + vec2f(t * 0.01, 0.0), 4);
    }
    if (night) {
        // the city's glow low in the sky
        col += col_hex(0x3a2440u) * 0.2 * exp(-max(d.y, 0.0) * 6.0);
    }

    let s = d.y / ax;
    let water = d.y < 0.0 && ax < -d.y * HALF / EYE;
    let on_wall = !water && s > -EYE / HALF && s < (WALL - EYE) / HALF;

    // ---- the banks: the walkway from above, the buildings behind it
    if (!water && !on_wall) {
        let zp = FOCAL * (EYE - WALL) / max(-d.y, 1e-4);
        let xp = d.x * zp / FOCAL;
        let zb = FOCAL * (HALF + WALK + 3.0) / ax;
        let yb = EYE + d.y * zb / FOCAL;
        if (d.y < 0.0 && abs(xp) < HALF + WALK) {
            // paving, a lantern's pool of light every few metres
            var wc = col_hex(0x8a8480u) * (0.85 + 0.15 * noise_value2(vec2f(xp * 2.0, zp * 0.8)));
            let pool = exp(-abs(fract(zp / LANTERN_DZ) - 0.5) * 7.0) * exp(-sq((abs(xp) - HALF - 0.6) / 1.2));
            wc *= l.sun * 0.55 + l.sky_low * 0.3 + l.lamp * select(0.02, 0.35, night) * pool;
            // fallen petals across the paving
            let pf = sstep(0.6, 0.72, noise_fbm2(vec2f(xp * 1.5, zp * 0.6), 4));
            wc = mix(wc, l.blossom * (l.sun * 0.7 + l.sky_low * 0.3 + l.lamp * select(0.0, 0.2, night) * pool), pf * 0.7);
            col = mix(wc, l.sky_low * select(0.85, 0.35, night), 1.0 - exp(-zp * fog));
        } else if (d.y < 0.0 || yb < 14.0) {
            // facades: shops and flats, windows lit at night
            var bc = mix(col_hex(0x9a948eu), col_hex(0x5a5660u), hash_cell2(vec2i(i32(floor(zb / 9.0)), i32(sign(d.x))), 0x3bu).x);
            bc *= l.sun * 0.35 + l.sky_low * 0.3;
            let wg = vec2f(zb * 0.5, yb * 0.9);
            let wh = hash_cell2(vec2i(floor(wg)), 0x3b1du);
            let pane = step(0.25, fract(wg.x)) * step(fract(wg.x), 0.75) * step(0.3, fract(wg.y)) * step(fract(wg.y), 0.7);
            bc = mix(bc, col_hex(0x1a1c24u) * (l.sky_low * 0.5), pane * select(0.6, 0.9, night));
            bc += col_kelvin(2800.0) * pane * step(wh.x, select(0.05, 0.5, night)) * select(0.05, 0.3, night);
            col = mix(bc, l.sky_low * select(0.85, 0.35, night), saturate(1.0 - exp(-zb * fog * 2.0) + select(0.35, 0.0, night)));
        }
    }

    // ---- the walls: grey concrete, stained, moss by the water
    if (on_wall) {
        let zw = FOCAL * HALF / ax;
        let yw = EYE + d.y * zw / FOCAL;
        var wc = col_hex(0x7a7a78u) * (0.8 + 0.2 * noise_value2(vec2f(zw * 0.6, yw * 3.0)));
        wc *= 0.85 + 0.15 * sstep(0.3, 0.7, noise_value2(vec2f(zw * 2.5, 1.0)));
        wc = mix(wc, col_hex(0x3a4a2eu) * 0.8, sstep(0.7, 0.0, yw) * 0.6);
        let dh = length(vec2f((fract(zw / 3.0) - 0.5) * 3.0, yw - 1.1)) - 0.08;
        wc = mix(wc, vec3f(0.04), sstep(0.02, -0.02, dh));
        let wl = l.sun * 0.35 + l.sky_low * 0.3 + l.lamp * select(0.02, 0.12, night) * exp(-abs(fract(zw / LANTERN_DZ) - 0.5) * 4.0);
        col = mix(wc * wl, l.sky_low * select(0.8, 0.35, night), 1.0 - exp(-zw * fog));
    }

    // ---- the water: sky and trees mirrored, rippled; petal rafts
    if (water) {
        let zw = FOCAL * EYE / -d.y;
        let xw = d.x * zw / FOCAL;
        var rip = noise_value2(vec2f(xw * 1.5, zw * 0.5 - t * 0.2)) - 0.5;
        if (l.mode == 2u) {
            rip += 0.9 * (noise_value2(vec2f(xw * 8.0, zw * 3.0 + t * 2.5)) - 0.5);
        }
        let mp = p + vec2f(rip * 0.004, rip * 0.012 * (1.0 + 4.0 / max(zw, 1.0)));
        // mirrored sky
        var mirror = mix(l.sky_low, l.sky, sqrt(saturate(-(mp.y - V.y) / 0.5 + 0.0)));
        // mirrored trees, far to near
        for (var k = TREES - 1; k >= 0; k--) {
            for (var side = 0; side < 2; side++) {
                let sx = select(-1.0, 1.0, side == 1);
                let tr = tree(mp, sx, k, true, t);
                if (tr.x > 0.0) {
                    let bc = l.blossom * (l.sun * (0.5 + 0.6 * tr.y) + l.sky_low * 0.2 + l.lamp * select(0.02, 0.35, night) * (1.0 - tr.y));
                    mirror = mix(mirror, mix(bc, l.sky_low * 0.6, 1.0 - exp(-tr.z * fog)), tr.x);
                }
            }
        }
        var wc = mirror * 0.5 + col_hex(0x14221eu) * 0.1;
        // lantern light on the water, long streaks under each bank
        let lz = fract(zw / LANTERN_DZ);
        wc += l.lamp * select(0.01, 0.3, night) * exp(-abs(lz - 0.5) * 8.0) * exp(-sq((abs(xw) - HALF + 0.5) / 0.7)) * (0.6 + 0.4 * rip);
        // petal rafts drifting along the walls, and a few in open water
        let pr = noise_fbm2(vec2f(xw * 0.8, zw * 0.3 - t * 0.02), 4);
        let along = sstep(HALF - 1.8, HALF - 0.2, abs(xw));
        let raft = sstep(0.72 - along * 0.28, 0.8 - along * 0.28, pr);
        wc = mix(wc, l.blossom * (l.sun * 0.7 + l.sky_low * 0.3 + l.lamp * select(0.02, 0.25, night)), raft * 0.9);
        col = mix(wc, l.sky_low * select(0.7, 0.3, night), 1.0 - exp(-zw * fog));
    }

    // ---- the trees on both banks, far to near: trunks, then blossom
    if (d.y > -0.3) {
        for (var k = TREES - 1; k >= 0; k--) {
            for (var side = 0; side < 2; side++) {
                let sx = select(-1.0, 1.0, side == 1);
                let tr = tree(p, sx, k, false, t);
                // the trunk, leaning out from the bank
                let h = hash_cell2(vec2i(k, i32(sx)), 0x5a2au);
                let z = tr.z;
                let base = project(vec3f(sx * (HALF + 1.6), WALL, z));
                let top = project(vec3f(sx * (HALF + 1.4 - 1.4 * h.y), 5.6, z));
                let tw = 0.25 * FOCAL / z;
                let trunk = sstep(tw, tw * 0.6, sdf2_segment(p, base, top));
                let bark = col_hex(0x2a1c18u) * (l.sun * 0.5 + l.sky_low * 0.15 + l.lamp * select(0.0, 0.1, night));
                col = mix(col, mix(bark, l.sky_low * 0.5, 1.0 - exp(-z * fog)), trunk);
                if (tr.x > 0.0) {
                    let lamp_on = l.lamp * select(0.03, 0.55, night) * (1.0 - tr.y);
                    // deep pink in the shade, near white where the light catches
                    let tone = mix(l.blossom * vec3f(0.9, 0.62, 0.72), mix(l.blossom, vec3f(1.0), 0.45), tr.y);
                    var bc = tone * (l.sun * (0.45 + 0.8 * tr.y) + l.sky_low * 0.22 + lamp_on);
                    col = mix(col, mix(bc, l.sky_low * select(0.85, 0.4, night), 1.0 - exp(-z * fog)), tr.x);
                }
            }
        }
    }

    // ---- lanterns along both walkways, under the trees
    for (var side = 0; side < 2; side++) {
        let sx = select(-1.0, 1.0, side == 1);
        for (var k = 0; k < 10; k++) {
            let z = 3.0 + f32(k) * LANTERN_DZ + select(0.0, LANTERN_DZ * 0.5, side == 1);
            let lp = project(vec3f(sx * (HALF + 0.5), WALL + 2.4, z));
            let r = 0.2 * FOCAL / z;
            let dd = length((p - lp) / vec2f(0.75, 1.0)) / r;
            let glow = select(0.25, 1.2, night);
            col = mix(col, l.lamp * glow * 1.6 + vec3f(0.3) * select(0.7, 0.1, night), sstep(1.05, 0.9, dd));
            col += l.lamp * glow * 0.15 * exp(-dd * 0.8) * select(0.15, 1.0, night);
        }
    }

    // ---- petals drifting down across the picture
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        let sc = 10.0 + fk * 7.0;
        let fall = select(0.05, 0.12, l.mode == 2u);
        let q = vec2f(p.x * sc + sin(t * 0.4 + p.y * 4.0 + fk) * 0.5 + t * 0.12, p.y * sc + t * (fall + fk * 0.03) * sc);
        let h = hash_cell2(vec2i(floor(q)) + vec2i(k * 101, 0), 0x5a4au);
        if (h.w < 0.12) {
            let f = fract(q) - 0.2 - 0.6 * h.xy;
            let spin = rot2(t * (0.7 + h.z) + h.x * 6.0);
            let pd = length((spin * f) * vec2f(1.0, 1.8)) - 0.08;
            let pc = l.blossom * (l.sun * 1.1 + l.sky_low * 0.3 + l.lamp * select(0.02, 0.5, night));
            col = mix(col, pc, sstep(0.03, -0.03, pd) * 0.9);
        }
    }
    if (l.mode == 2u) {
        col += vec3f(0.7, 0.72, 0.78) * rain_streaks(p, ctx, 0.4, 1.3, 0.06, 2) * 0.08;
    }
    return col * exp2(l.exposure);
}
