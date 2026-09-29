//! name: tromso
//! title: Aurora over Tromsø
//! category: space
//! tags: aurora, northern lights, fjord, norway, snow, night
//! desc: green aurora curtains rippling over a snowy Norwegian fjord and its village lights
//! themes: green, vivid, faint
//! uses: camera, raymarch, noise, stars, water
//! cost: heavy
//! fallback: aurora
//! credits: original

// World units are kilometres, y up, facing north (-z) across a fjord. The
// aurora is volumetric: curtains are wavy sheets standing in the upper
// atmosphere; each ray samples them in horizontal slices from 95 to 320 km
// altitude, with oxygen green at the sharp lower edge and the faint red
// (and, when it is strong, violet) higher up. Rays along a curtain give it
// vertical striations that converge overhead, as in photographs. The
// mountains across the fjord are a raymarched heightfield; over open water
// the march steps by the distance to the nearest shore, so grazing rays
// stay cheap. The fjord mirrors all of it, lightly rippled.

struct Look {
    power: f32,       // aurora brightness
    curtains: i32,    // how many sheets
    pink: f32,        // N2 fringe at the lower edge
    violet: f32,      // sunlit violet tops
    red: f32,         // oxygen red high up
    speed: f32,
    stars: f32,
    town: f32,        // village lights
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: { return Look(2.2, 3, 1.0, 0.8, 0.8, 3.6, 0.25, 1.0, 0.0); }
        case 2u: { return Look(0.45, 1, 0.0, 0.0, 0.35, 1.4, 0.5, 1.0, 0.25); }
        default: { return Look(1.0, 2, 0.15, 0.1, 0.5, 2.5, 0.32, 1.0, 0.0); }
    }
}

const RO: vec3f = vec3f(0.0, 0.0062, 0.0);

// ---------------------------------------------------------------- aurora

fn curtain(pos: vec2f, k: i32, t: f32, l: Look) -> f32 {
    let fk = f32(k);
    let z0 = -select(290.0 + 160.0 * fk, 175.0, k == 2);
    let x = pos.x;
    let fold = 55.0 * sin(x * 0.0045 + t * 0.011 * l.speed + fk * 2.1)
        + 30.0 * noise_grad2(vec2f(x * 0.009 + t * 0.006 * l.speed, fk * 7.0))
        + 10.0 * noise_grad2(vec2f(x * 0.03 - t * 0.02 * l.speed, fk * 3.0 + 1.0));
    let d = pos.y - (z0 + fold);
    let w = 5.0 + 7.0 * noise_value2(vec2f(x * 0.006, fk + 9.0));
    let sheet = exp(-d * d / (w * w));
    if (sheet < 0.01) { return 0.0; }
    // rays: fine structure along the arc, drifting sideways
    let s = x + fold * 0.4;
    let rays = noise_value2(vec2f(s * 0.11 + t * 0.09 * l.speed, fk * 5.0));
    let rays2 = noise_value2(vec2f(s * 0.37 - t * 0.05 * l.speed, fk * 5.0 + 3.0));
    // the arc brightens and fades along its length
    let along = 0.25 + 0.75 * sstep(0.3, 0.75, noise_value2(vec2f(x * 0.0025 + t * 0.004 * l.speed, fk * 2.0 + 4.0)));
    return sheet * (0.2 + 0.8 * rays * rays + 0.35 * rays2) * along * select(1.0, 0.6, k > 0);
}

fn aurora(rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    if (rd.y < 0.012) { return vec3f(0.0); }
    var acc = vec3f(0.0);
    let n = 16;
    for (var i = 0; i < 16; i++) {
        let fi = (f32(i) + ctx.jitter) / f32(n);
        let h = 95.0 + 225.0 * fi * fi;           // denser near the lower edge
        let dh = 450.0 * fi / f32(n) + 1.2;
        let dist = h / rd.y;
        let pos = rd.xz * dist;
        var e = 0.0;
        for (var k = 0; k < 3; k++) {
            if (k >= l.curtains) { break; }
            e += curtain(pos, k, t, l);
        }
        if (e <= 0.0) { continue; }
        // emission by altitude: sharp lower edge, long green decay, red above
        let green = sstep(96.0, 106.0, h) * exp(-(h - 106.0) / 38.0);
        let red = exp(-sq((h - 215.0) / 55.0)) * l.red;
        let pink = exp(-sq((h - 99.0) / 4.5)) * l.pink;
        let violet = sstep(200.0, 300.0, h) * l.violet;
        var c = vec3f(0.12, 1.0, 0.42) * green + vec3f(0.9, 0.08, 0.28) * red * 0.35;
        c += vec3f(1.0, 0.25, 0.55) * pink * 1.2 + vec3f(0.45, 0.2, 1.0) * violet * 0.3;
        // far and low: the atmosphere dims it, the Earth curves away
        acc += c * e * dh * exp(-dist / 1400.0);
    }
    return acc * l.power * 0.009;
}

fn night_sky(rd: vec3f, l: Look, ctx: Ctx, stars: bool) -> vec3f {
    let alt = max(rd.y, 0.0);
    var c = mix(vec3f(0.008, 0.011, 0.018), vec3f(0.004, 0.005, 0.011), pow(alt, 0.35));
    if (stars) {
        let sr = star_rotate(rd, 69.6, ctx.t * 4.0);
        c += star_field(sr, l.stars, ctx) * 0.7 * exp(-0.1 / max(alt + 0.02, 0.02));
    }
    return c;
}

// ---------------------------------------------------------------- land

fn far_shore(x: f32) -> f32 {
    return 2.3 + 0.4 * sin(x * 0.3 + 0.8) + 0.25 * noise_grad2(vec2f(x * 0.6, 2.0));
}
fn near_shore(x: f32) -> f32 {
    return 0.011 + 0.004 * sin(x * 60.0 + 1.0) + 0.006 * noise_grad2(vec2f(x * 30.0, 5.0)) + 0.03 * sq(x * 0.9);
}

// the skyline: a ridged range with one big pyramid right of centre (km)
fn crest(x: f32) -> f32 {
    let pk = pow(saturate(1.0 - abs(x - 0.9) * 1.25), 1.4) * 0.55;
    return (0.45 + 0.45 * noise_ridged2(vec2f(x * 0.55 + 4.0, 1.0), 3) + pk) * 1.45;
}

fn terrain(xz: vec2f, oct: i32) -> f32 {
    return terrain_s(xz, oct, far_shore(xz.x), near_shore(xz.x), crest(xz.x));
}

// terrain with the shorelines and crest already known (they only depend on x)
fn terrain_s(xz: vec2f, oct: i32, fs: f32, ns: f32, cr: f32) -> f32 {
    let n = -xz.y;                                  // distance north
    let d = n - fs;
    var h = -0.03;
    if (d > -0.1 && d < 4.0) {
        let tn = noise_terrain(xz * vec2f(0.9, 1.2) + vec2f(3.0, 1.0), oct);
        let ridge = cr * sstep(0.0, 1.7, d) * (0.7 + 0.5 * tn.x) * sstep(3.9, 2.8, d);
        // a flat coastal strip where the village sits
        let strip = 0.012 * sstep(-0.02, 0.03, d);
        h = mix(-0.03, strip + ridge, sstep(-0.06, 0.02, d));
    }
    // the snowy bank we stand on
    let nb = n - ns;
    if (nb < 0.01) {
        // rising from the water's edge to about 4 m where we stand
        let bank = 0.0003 + 0.0035 * sstep(0.0, -0.014, nb) + 0.0008 * noise_grad2(xz * 160.0) * sstep(0.0, -0.005, nb);
        h = max(h, mix(-0.02, bank, sstep(0.003, -0.003, nb)));
    }
    return h;
}

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    if (p.y > 2.6) { return vec2f(p.y - 2.5, 1.0); }
    let n = -p.z;
    let ns = near_shore(p.x);
    let fs = far_shore(p.x);
    // open water: nothing above the surface nearer than the shores
    // (padded past the march's hit threshold: these bounds are not surfaces)
    let pad = 0.4 * ctx.px * length(p - RO) + 0.002;
    if (n > ns + 0.02 && n < fs - 0.12 && p.y > -0.001) {
        return vec2f(min(n - ns - 0.02, fs - 0.12 - n) * 0.7 + pad, 1.0);
    }
    // beyond the range: open country far below
    if (n > fs + 4.0) { return vec2f(max(p.y + 0.03, (n - fs - 4.0) * 0.5) + pad, 1.0); }
    // above the highest the range reaches here: skip its noise
    let cr = crest(p.x);
    let top = cr * 1.22 + 0.015;
    if (p.y > top + 0.02 && n > 0.1) { return vec2f((p.y - top) * 0.55 + pad, 1.0); }
    let oct = 3;
    return vec2f((p.y - terrain_s(p.xz, oct, fs, ns, cr)) * 0.7, 1.0);
}

// Seen from a few metres above the water, the fjord mirrors the range as
// if from the camera itself: a reflected ray finds the mountains if it
// leaves below their skyline. The skyline along an azimuth is found from a
// handful of samples of the range (instead of a second march).
// Returns (skyline elevation as tan(angle), snow cover near the crest).
fn skyline(ro: vec3f, rd: vec3f) -> vec2f {
    let hdir = normalize(rd.xz);
    var best = -1.0;
    var cover = 0.5;
    for (var i = 0; i < 6; i++) {
        let fs = far_shore(ro.x + hdir.x * 2.6 / max(-hdir.y, 0.2));
        let dd = fs + 0.25 + f32(i) * f32(i) * 0.14;
        let tt = dd / max(-hdir.y, 0.2);
        let q = ro.xz + hdir * tt;
        let h = terrain(q, 3);
        let e = (h - ro.y) / tt;
        if (e > best) {
            best = e;
            cover = noise_value2(q * 60.0);
        }
    }
    return vec2f(best, cover);
}

fn land_normal(p: vec3f, t: f32, ctx: Ctx) -> vec3f {
    let e = max(0.0004, t * ctx.px * 0.8);
    let oct = 6;
    let hx = terrain(p.xz + vec2f(e, 0.0), oct) - terrain(p.xz - vec2f(e, 0.0), oct);
    let hz = terrain(p.xz + vec2f(0.0, e), oct) - terrain(p.xz - vec2f(0.0, e), oct);
    return normalize(vec3f(-hx, 2.0 * e, -hz));
}

// light falling on the land: the aurora overhead (towards the north) plus
// the night sky
fn shade_land(p: vec3f, t: f32, aur: vec3f, ctx: Ctx) -> vec3f {
    let n = land_normal(p, t, ctx);
    let slope = 1.0 - n.y;
    let rough = noise_value2(p.xz * 60.0);
    // snow everywhere it can cling; dark rock ribs and gullies break it up
    let ribs = noise_ridged2(p.xz * vec2f(9.0, 5.0) + vec2f(2.0, 7.0), 3);
    let snow = sstep(0.34, 0.2, slope + 0.35 * (ribs - 0.45) + 0.2 * (rough - 0.5) - 0.1 * sstep(0.2, 0.9, p.y));
    // birch scrub darkens the lower slopes of the far shore
    let scrub = sstep(0.25, 0.05, p.y) * sstep(-0.1, 0.1, -p.z - far_shore(p.x)) * (0.5 + 0.5 * noise_value2(p.xz * 25.0));
    var alb = mix(vec3f(0.03, 0.03, 0.035), vec3f(0.72, 0.78, 0.85), snow * (1.0 - 0.7 * scrub));
    let up = 0.5 + 0.5 * n.y;
    let north = saturate(dot(n, normalize(vec3f(0.0, 0.6, -0.8))));
    var c = alb * (vec3f(0.009, 0.011, 0.02) * up + aur * (0.9 * up + 0.6 * north));
    return c;
}

// village lights strung along the far shore, a few up the hillside
fn town_light(i: i32) -> vec4f {
    let h = hash_cell2(vec2i(i, 17), 0x7055u);
    // houses cluster into hamlets along the shore
    let u = (f32(i) + h.x) / 44.0;
    let x = -3.0 + 6.0 * (u + 0.06 * sin(u * 31.0) + 0.04 * sin(u * 13.0 + 1.0));
    let up = h.y * h.y;
    let z = -(far_shore(x) + 0.02 + 0.14 * up);
    let y = 0.008 + 0.06 * up * up + 0.003 * h.z;
    let kind = select(select(0.0, 1.0, h.w > 0.72), 2.0, h.w > 0.93);   // house, street lamp, white
    return vec4f(x, y, z, kind + h.z * 0.5);
}

fn lights(p: vec2f, cam: Cam, mirror: bool, rip: f32, l: Look, ctx: Ctx) -> vec3f {
    var acc = vec3f(0.0);
    let w = max(ctx.px * 0.7, 0.0012);
    for (var i = 0; i < 44; i++) {
        let tl = town_light(i);
        var wp = tl.xyz;
        if (mirror) { wp.y = -wp.y; }
        let pr = cam_project(cam, wp);
        if (pr.z <= 0.0) { continue; }
        let kind = floor(tl.w);
        var col = mix(col_kelvin(2600.0), col_kelvin(3300.0), fract(tl.w) * 2.0);
        if (kind > 0.5) { col = col_kelvin(2000.0); }
        if (kind > 1.5) { col = col_kelvin(5000.0); }
        var dv = p - pr.xy;
        var inten = 0.5;
        if (mirror) {
            // the reflection smears into a rippled vertical streak
            let top = cam_project(cam, vec3f(wp.x, 0.0, wp.z)).y;
            let len = 0.05 + (top - pr.y) * 6.0;
            if (p.y < top - len || p.y > top + w) { continue; }
            let along = saturate((top - p.y) / max(len, 1e-3));
            // broken into glints by the ripples
            let brk = sstep(0.35, 0.75, noise_value2(vec2f(wp.x * 40.0, p.y * 260.0 - ctx.t * 0.6)));
            inten = 0.22 * (1.0 - along) * (1.0 - along) * (0.25 + 1.2 * brk) * (0.5 + rip);
            dv.y = 0.0;
            dv.x *= 0.8;
        }
        acc += col * exp(-dot(dv, dv) / (w * w)) * inten;
    }
    return acc * l.town;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let cam = cam_look_at(RO, RO + vec3f(0.0, 0.17, -1.0), 0.0, 56.0);
    let rd = cam_ray(cam, p);
    // aurora light on the landscape: its overall glow, taken from a few
    // directions up the northern sky
    // (a closed-form stand-in for the curtains' total glow: sampling them
    // here for every pixel would triple the cost)
    let surge = 0.75 + 0.25 * sin(ctx.t * 0.013 * l.speed + 1.0);
    let aur_amb = (vec3f(0.12, 1.0, 0.45) + vec3f(0.5, 0.1, 0.3) * l.pink) * 0.016 * l.power * surge;
    var col: vec3f;
    let tw = water_intersect(RO, rd, 0.0);
    let tmax = select(40.0, tw + 0.01, tw > 0.0);
    var hit = -1.0;
    // rays climbing above anything the range can reach go straight to the sky
    var under = rd.y < 0.0;
    if (!under && rd.y < 0.36) {
        let hd = rd.xz / max(length(rd.xz), 1e-4);
        let el = rd.y / max(length(rd.xz), 1e-4);
        let n1 = far_shore(hd.x / max(-hd.y, 0.2) * 3.0) + 0.9;
        let n2 = n1 + 2.2;
        let c1 = crest(hd.x / max(-hd.y, 0.2) * n1);
        let c2 = crest(hd.x / max(-hd.y, 0.2) * n2);
        under = el < max(c1, c2) * 1.25 / (n1 - 0.3) + 0.004;
    }
    if (under) {
        let tm = select(tmax, min(tmax, (2.6 - RO.y) / max(rd.y, 1e-4)), rd.y > 0.0);
        hit = rm_march(RO, rd, 0.0005, tm, steps(110.0, ctx), ctx).x;
    }
    if (hit > 0.0) {
        let hp = RO + rd * hit;
        col = shade_land(hp, hit, aur_amb, ctx);
        col = mix(col, night_sky(normalize(vec3f(rd.x, 0.02, rd.z)), l, ctx, false) + aur_amb * 0.3,
            1.0 - exp(-hit * 0.05));
    } else if (tw > 0.0) {
        // the fjord: a dark mirror, rippled by a light breeze
        let hp = RO + rd * tw;
        let wn = water_normal(hp.xz * 1000.0, ctx.t, 0.2, tw * 1000.0, ctx);
        var n = normalize(vec3f(wn.x * 0.18, wn.y, wn.z * 0.18));
        var rr = reflect(rd, n);
        rr.y = abs(rr.y);
        // what the reflected ray sees: the far mountains, or the sky
        let sk = skyline(RO, rr);
        let el = rr.y / max(length(rr.xz), 1e-4);
        var refl: vec3f;
        if (el < sk.x) {
            // the mirrored range: snow and rock under the aurora, softened
            let snow = 0.35 + 0.4 * sk.y;
            refl = vec3f(0.7, 0.76, 0.84) * snow * (vec3f(0.009, 0.011, 0.02) + aur_amb * 1.1);
            refl = mix(refl, night_sky(normalize(vec3f(rr.x, 0.02, rr.z)), l, ctx, false) + aur_amb * 0.3, 0.2);
        } else {
            refl = night_sky(rr, l, ctx, false) + aurora(rr, ctx.t, l, ctx);
            let sr = star_rotate(rr, 69.6, ctx.t * 4.0);
            refl += star_field(sr, l.stars * 0.5, ctx) * 0.5;
        }
        let f = water_fresnel(dot(-rd, n));
        col = mix(vec3f(0.002, 0.003, 0.004) + aur_amb * 0.02, refl, max(f, 0.35));
        let rip = saturate(0.5 + (n.x) * 8.0);
        col += lights(p, cam, true, rip, l, ctx);
    } else {
        col = night_sky(rd, l, ctx, true) + aurora(rd, ctx.t, l, ctx);
    }
    if (hit > 0.0 || tw < 0.0) { col += lights(p, cam, false, 0.0, l, ctx); }
    return col * exp2(l.exposure);
}
