//! name: outrun
//! title: Outrun Highway
//! category: games
//! tags: retro, racing, arcade, synthwave, palms, sunset, road
//! desc: a pseudo-3D arcade highway curving into a striped sunset, palms whipping past
//! themes: sunset, night, neon
//! params: speed, palms
//! uses: noise, light, game, sdf
//! cost: medium
//! fallback: drive
//! credits: original

// The 1986 arcade road. The ground is a plane seen from a camera CAM_H units
// up: a screen row h below the horizon is at depth z = CAM_H / h, and a world
// offset x at that depth lands at x / z on screen. The track winds as a sine
// of the distance travelled; the camera looks along the road's tangent, so
// the near road is always straight under the car while the far road and its
// vanishing point sway left and right. Palms are sprites whose SDF is
// evaluated in world units at their own depth; the sun never moves.

const HY: f32 = 0.05;         // horizon row
const CAM_H: f32 = 1.5;       // camera height (world units)
const ROAD_W: f32 = 1.3;      // road half-width
const RUMBLE_W: f32 = 0.28;   // rumble strip width
const SEG: f32 = 3.0;         // one road segment (stripes alternate per segment)
const WIND_A: f32 = 5.3;      // track winding amplitude
const WIND_W: f32 = 0.06;     // track winding frequency (per world unit)

struct Look {
    mode: u32,            // 0 sunset, 1 night, 2 neon
    sky_top: vec3f,
    sky_mid: vec3f,
    sky_low: vec3f,
    sun_c: vec2f,         // sun (or moon) centre
    sun_r: f32,
    sun_top: vec3f,
    sun_bot: vec3f,
    band: vec3f,          // colour inside the sun's cut bands ...
    band_sky: f32,        // ... or 1 = the sky shows through
    mount: vec3f,
    ground_a: vec3f,
    ground_b: vec3f,
    road_a: vec3f,
    road_b: vec3f,
    rumble_a: vec3f,
    rumble_b: vec3f,
    line: vec3f,
    palm: vec3f,
    car: vec3f,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            // moonless-blue night: stars, a small moon, headlights on the road, cyan paint
            return Look(1u,
                col_hex(0x040818u) * 0.7, col_hex(0x0c1434u) * 0.8, col_hex(0x2a2050u) * 0.9,
                vec2f(0.3, 0.3), 0.06, col_hex(0xe8ecffu) * 1.6, col_hex(0xc8d0f0u) * 1.2,
                vec3f(0.0), 0.0,
                col_hex(0x2a1048u) * 0.45,
                col_hex(0x121a2cu) * 0.5, col_hex(0x0c1220u) * 0.5,
                col_hex(0x34343eu) * 0.32, col_hex(0x2a2a33u) * 0.32,
                col_hex(0x40f0ffu) * 0.9, col_hex(0x102040u) * 0.6,
                col_hex(0x40f0ffu) * 1.6, col_hex(0x06040cu) * 0.3, col_hex(0x0a0810u) * 0.4);
        }
        case 2u: {
            // neon: black sky, a magenta/cyan grid instead of ground, wireframe palms
            return Look(2u,
                vec3f(0.0), col_hex(0x0a0018u) * 0.5, col_hex(0x2a0040u) * 0.8,
                vec2f(0.0, HY), 0.27, col_hex(0xffe066u) * 3.0, col_hex(0xff20a0u) * 2.4,
                col_hex(0x20e0ffu) * 1.6, 0.0,
                col_hex(0x12041eu) * 0.6,
                col_hex(0x050208u) * 0.5, col_hex(0x050208u) * 0.5,
                col_hex(0x0c0814u) * 0.5, col_hex(0x100a1au) * 0.5,
                col_hex(0xff20c0u) * 1.2, col_hex(0x20e0ffu) * 1.2,
                col_hex(0x20e0ffu) * 1.8, col_hex(0xff40d0u) * 1.5, col_hex(0x08040cu) * 0.5);
        }
        default: {
            // sunset: magenta to orange sky, the striped sun, green verges
            return Look(0u,
                col_hex(0x3c1060u) * 0.9, col_hex(0xd8307au) * 1.0, col_hex(0xff9a3cu) * 1.3,
                vec2f(0.0, HY), 0.27, col_hex(0xffe066u) * 3.0, col_hex(0xff3c78u) * 2.2,
                vec3f(0.0), 1.0,
                col_hex(0x4a1d6eu) * 0.55,
                col_hex(0x3aa044u) * 0.42, col_hex(0x2a7a32u) * 0.42,
                col_hex(0x6a6878u) * 0.32, col_hex(0x5a5866u) * 0.32,
                col_hex(0xd83030u) * 0.9, col_hex(0xf0f0f0u) * 0.9,
                col_hex(0xf6f6f0u) * 1.2, col_hex(0x1a0a24u) * 0.35, col_hex(0x14101cu) * 0.4);
        }
    }
}

// ------------------------------------------------------------------ the track
// screen x of the road centre at depth z after s units of travel: the winding
// sine minus its value and tangent at the camera, so the near road is straight
fn road_x(z: f32, s: f32) -> f32 {
    let a = s * WIND_W;
    let w = WIND_A * (sin((s + z) * WIND_W) - sin(a) - z * WIND_W * cos(a));
    return w / max(z, 0.5);
}

// ------------------------------------------------------------------ sky
fn sky(p: vec2f, l: Look, ctx: Ctx) -> vec3f {
    let e = p.y - HY;
    var c = mix(l.sky_low, l.sky_mid, sstep(0.0, 0.2, e));
    c = mix(c, l.sky_top, sstep(0.16, 0.75, e));
    if (l.mode == 1u) {
        // hashed stars, thinning toward the horizon haze
        let cell = floor(p * 48.0);
        let hs = hash_cell2(vec2i(cell), 21u);
        let sp = (cell + 0.15 + hs.xy * 0.7) / 48.0;
        let star = aa_fill(length(p - sp) - 0.0035, ctx) * step(0.72, hs.z) * sstep(0.0, 0.12, e);
        c += vec3f(0.9, 0.95, 1.0) * star * (0.7 + 0.6 * hs.w);
    }
    // the sun: a gradient disc with the retro cut bands over its lower visible half
    let d = length(p - l.sun_c) - l.sun_r;
    let lo = max(l.sun_c.y - l.sun_r, HY);
    let v = saturate((p.y - lo) / max(l.sun_c.y + l.sun_r - lo, 1e-3));   // 0 bottom .. 1 top of what shows
    var sun = mix(l.sun_bot, l.sun_top, v);
    if (l.mode != 1u) {
        // five bands over the visible disc, thick at the bottom, gone near the top
        let bf = fmod_pos(v, 0.16) / 0.16;
        let thr = 0.5 * (1.0 - v / 0.8);
        let w = ctx.px / (0.16 * l.sun_r);
        let cut = 1.0 - smoothstep(thr - w, thr + w, bf);
        sun = mix(sun, mix(l.band, c, l.band_sky), cut);
    }
    c = mix(c, sun, aa_fill(d, ctx));
    c += l.sun_bot * 0.18 * exp(-max(d, 0.0) * 5.0) * select(1.0, 0.3, l.mode == 1u);
    return c;
}

// far ridges in purple; they slide a little against the sun as the road swings
fn mountains(p: vec2f, vp: f32, l: Look, sky_c: vec3f, ctx: Ctx) -> vec3f {
    var c = sky_c;
    let x = p.x - 0.35 * vp;
    let r1 = HY + 0.025 + 0.085 * noise_fbm2(vec2f(x * 1.6 + 11.0, 2.0), 4);
    c = mix(c, mix(l.mount, sky_c, 0.45), aa_fill(p.y - r1, ctx));
    let r2 = HY + 0.01 + 0.05 * noise_fbm2(vec2f(x * 3.6 + 40.0, 5.0), 4);
    c = mix(c, l.mount, aa_fill(p.y - r2, ctx));
    return c;
}

// ------------------------------------------------------------------ ground and road
fn ground(p: vec2f, s: f32, l: Look, ctx: Ctx) -> vec3f {
    let h = max(HY - p.y, 1e-4);
    let z = CAM_H / h;
    let xw = (p.x - road_x(z, s)) * z;           // world x from the road centre
    let rw = abs(xw);
    let aa = ctx.px * z;                          // one pixel in world units at this depth
    let along = s + z;
    // alternating segments, fading to their mean once a segment is under ~1 px tall
    let band = step(1.0, fmod_pos(along / SEG, 2.0));
    let seg_px = CAM_H * SEG / (z * z) / ctx.px;
    let vis = sstep(0.7, 2.5, seg_px);
    let bk = mix(0.5, band, vis);

    // headlight cone at night
    var light = 1.0;
    if (l.mode == 1u) {
        let cw = 1.2 + 0.32 * z;
        light = 0.1 + 0.9 * exp(-z * 0.06) * sstep(cw + 0.8, cw - 0.8, rw);
    }

    var base = mix(l.ground_a, l.ground_b, bk) * light;
    if (l.mode == 2u) {
        // perspective grid bending with the road: magenta x-lines, cyan depth lines
        let mag = col_hex(0xff20c0u);
        let cyan = col_hex(0x20e0ffu);
        let gx = abs(fmod_pos(xw + 1.5, 3.0) - 1.5) / z;
        let gz = abs(fmod_pos(along + SEG * 0.5, SEG) - SEG * 0.5) * CAM_H / (z * z);
        let fx = sstep(1.5, 5.0, (3.0 / z) / ctx.px);
        let fz = sstep(1.5, 5.0, seg_px);
        base = l.ground_a
            + mag * 1.6 * (light_glow(gx, ctx.px) + 0.3 * light_glow(gx, 4.0 * ctx.px)) * fx
            + cyan * 1.6 * (light_glow(gz, ctx.px) + 0.3 * light_glow(gz, 4.0 * ctx.px)) * fz
            + mag * 0.18 * (1.0 - fx) + cyan * 0.12 * (1.0 - fz);
    }

    var col = base;
    let rumble = mix(l.rumble_a, l.rumble_b, bk) * light;
    let asphalt = mix(l.road_a, l.road_b, bk) * light;
    col = mix(col, rumble, saturate(0.5 - (rw - (ROAD_W + RUMBLE_W)) / aa));
    col = mix(col, asphalt, saturate(0.5 - (rw - ROAD_W) / aa));

    // dashed centre line and two thinner lane lines, on for half of each segment
    let dash = mix(0.5, step(0.5, fmod_pos(along / SEG, 1.0)), vis);
    let cov_c = saturate(0.5 - (rw - 0.07) / aa) * saturate(0.14 / aa);
    let cov_l = saturate(0.5 - (abs(rw - 0.62) - 0.05) / aa) * saturate(0.1 / aa) * 0.7;
    col = mix(col, l.line * (0.5 + 0.5 * light), max(cov_c, cov_l) * dash);

    if (l.mode == 2u) {
        let de = abs(rw - (ROAD_W + RUMBLE_W * 0.5)) / z;
        col += rumble * 0.45 * light_glow(de, 3.0 * ctx.px);
    }

    // distance haze toward the horizon colour
    let haze = 1.0 - exp(-z * 0.012);
    return mix(col, l.sky_low * 0.85, haze * 0.85);
}

// ------------------------------------------------------------------ palms
// silhouette distance in world units; q is relative to the trunk base, y up
fn palm_sdf(q: vec2f, hgt: f32, side: f32) -> f32 {
    let top = vec2f(side * 0.5, hgt);                     // the trunk leans away from the road
    var d = sdf2_segment(q, vec2f(0.0), top) - 0.16;
    d = min(d, sdf2_circle(q - top, 0.35));
    for (var j = 0; j < 5; j++) {
        let k = f32(j) - 2.0;
        let a = PI * 0.5 + k * 0.8;                       // fan of frond directions
        let fq = rot2(PI * 0.5 - a) * (q - top);          // frond points along +y in tri space
        // outer fronds are longer and droop: a bowl-shaped crown rather than a star
        let droop = vec2f(0.18 * sign(k) * fq.y * fq.y, 0.0);
        d = min(d, sdf2_tri(fq + droop, 0.3, 1.5 + 0.45 * abs(k)));
    }
    return d;
}

fn palms(p: vec2f, s: f32, spacing: f32, l: Look, col: vec3f, ctx: Ctx) -> vec3f {
    var c = col;
    let i0 = i32(floor(s / spacing)) + 1;                 // the first palm ahead of the camera
    for (var n = 0; n < 14; n++) {
        let i = i0 + 13 - n;                              // far to near
        let zp = f32(i) * spacing - s;
        if (zp < 0.6) { continue; }
        let side = select(-1.0, 1.0, (i & 1) == 0);
        let hp = hash_cell2(vec2i(i, 7), 3u);
        let hgt = 4.0 + 2.0 * hp.x;
        let xb = road_x(zp, s) + side * (ROAD_W + 1.6) / zp;
        let yb = HY - CAM_H / zp;
        let q = (p - vec2f(xb, yb)) * zp;
        if (abs(q.x) > 3.2 || q.y > hgt + 2.8 || q.y < -0.2) { continue; }
        let d = palm_sdf(q, hgt, side) / zp;
        if (l.mode == 2u) {
            c = mix(c, l.palm, aa_stroke(d, 0.7 * ctx.px, ctx));
            c += l.palm * 0.35 * light_glow(abs(d), 2.5 * ctx.px);
        } else {
            c = mix(c, l.palm, aa_fill(d, ctx));
        }
    }
    return c;
}

// ------------------------------------------------------------------ the player
fn car(p: vec2f, lean: f32, l: Look, col: vec3f, ctx: Ctx) -> vec3f {
    var c = col;
    let q = rot2(lean) * (p - vec2f(0.0, -0.40));
    let body = sdf2_round_box(q, vec2f(0.115, 0.032), 0.014);
    let cabin = sdf2_round_box(q - vec2f(0.0, 0.045), vec2f(0.062, 0.026), 0.012);
    let wheels = min(sdf2_box(q - vec2f(-0.085, -0.036), vec2f(0.022, 0.014)),
                     sdf2_box(q - vec2f(0.085, -0.036), vec2f(0.022, 0.014)));
    // a soft shadow under the car
    c *= 1.0 - 0.45 * sstep(0.05, -0.02, sdf2_box(q - vec2f(0.0, -0.04), vec2f(0.12, 0.02)));
    c = mix(c, l.car, aa_fill(min(min(body, cabin), wheels), ctx));
    c = mix(c, l.car * 2.2 + 0.02, aa_stroke(cabin, 0.4 * ctx.px, ctx) * 0.6);
    // tail lights
    let red = col_hex(0xff2424u) * 3.0;
    let dl = min(length(q - vec2f(0.082, 0.0)), length(q + vec2f(0.082, 0.0))) - 0.012;
    c = mix(c, red, aa_fill(dl, ctx));
    c += red * 0.22 * light_glow(max(dl, 0.0), 0.025);
    return c;
}

// ------------------------------------------------------------------ scene
fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let k = ctx.params;
    let spd = select(10.0, 3.0, l.mode == 2u) * exp2(2.0 * (k.x - 0.5));             // world units per second
    let s = ctx.t * spd;
    let spacing = 6.0 * exp2(1.0 - 2.0 * k.y);            // more palms = closer together
    let a = s * WIND_W;
    let vp = -WIND_A * WIND_W * cos(a);                   // where the far road meets the horizon

    var col: vec3f;
    if (p.y < HY) {
        col = ground(p, s, l, ctx);
    } else {
        col = sky(p, l, ctx);
        if (p.y < HY + 0.26) { col = mountains(p, vp, l, col, ctx); }
    }
    col = palms(p, s, spacing, l, col, ctx);
    col = car(p, -0.1 * sin(a), l, col, ctx);
    return max(col, vec3f(0.0));
}
