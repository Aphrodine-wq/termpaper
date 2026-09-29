//! name: lagoon
//! title: Bora Bora Lagoon
//! category: coast
//! tags: tropical, lagoon, island, bungalows, turquoise, polynesia
//! desc: turquoise shallows off an overwater-bungalow deck, Mount Otemanu rising across the lagoon
//! themes: noon, golden, night
//! uses: camera, sdf, sky, stars, water
//! cost: heavy
//! fallback: ocean
//! credits: original

// World units are metres; y up, the camera looks down -z from a bungalow
// deck 3.2 m above the lagoon. The main island lies ~4 km west (ahead) with
// Otemanu slightly left of centre; a row of overwater bungalows recedes on
// the right toward it. The water is shaded physically: a refracted ray to a
// white coral-sand floor, Beer-Lambert absorption (red dies first, so 1-3 m
// of water reads turquoise and 20 m reads deep blue), Fresnel reflection of
// the sky, island and bungalows on top.

const CAM: vec3f = vec3f(0.0, 3.2, 0.0);
const OTE: vec2f = vec2f(-520.0, -3900.0);    // Otemanu summit (xz)
const PAHIA: vec2f = vec2f(-1500.0, -4250.0); // Mount Pahia
const ISL_C: vec2f = vec2f(-950.0, -4500.0);  // island centre
const ROW0: vec2f = vec2f(19.0, -34.0);       // first bungalow of the row
const ROWD: vec2f = vec2f(-0.0995, -0.995);   // row direction (unit)
const ROWP: vec2f = vec2f(0.995, -0.0995);    // across the row, away from us
const ROWN: f32 = 14.0;
const ROWS: f32 = 17.0;
// water absorption per metre (clear tropical water): red dies in a few metres
const SIG: vec3f = vec3f(0.42, 0.072, 0.034);

struct Look {
    sun: vec3f,      // key light (sun, or moon at night)
    night: f32,
    haze: f32,
    exposure: f32,
    sun_c: vec3f,
    amb: vec3f,
    hor: vec3f,      // air colour at the horizon toward the island
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            l.sun = sky_sun_dir(5.5, 96.0);
            l.night = 0.0; l.haze = 1.1; l.exposure = 0.0;
        }
        case 2u: {
            l.sun = sky_sun_dir(15.0, -13.0);
            l.night = 1.0; l.haze = 1.0; l.exposure = 0.9;
        }
        default: {
            l.sun = sky_sun_dir(60.0, 150.0);
            l.night = 0.0; l.haze = 1.0; l.exposure = -0.75;
        }
    }
    if (l.night > 0.5) {
        l.sun_c = vec3f(0.055, 0.068, 0.1);
        l.amb = vec3f(0.005, 0.008, 0.016);
        l.hor = vec3f(0.008, 0.012, 0.022);
    } else {
        l.sun_c = sky_sun_light(l.sun);
        l.amb = sky_ambient(l.sun);
        l.hor = day_sky(normalize(vec3f(-0.1, 0.02, -1.0)), l.sun, l.haze);
    }
    return l;
}

// clean maritime air: a touch more saturated than the model's default, and
// the model has no ozone, which at low sun leaves the horizon olive
fn day_sky(rd: vec3f, sun: vec3f, haze: f32) -> vec3f {
    let c = col_saturation(sky_atmosphere_haze(rd, sun, haze), 1.3);
    return c * mix(vec3f(1.0), vec3f(1.07, 0.86, 1.1), saturate(1.0 - sun.y * 3.0));
}

fn box_span(ro: vec3f, rd: vec3f, bmin: vec3f, bmax: vec3f) -> vec2f {
    let inv = 1.0 / rd;
    let a = (bmin - ro) * inv;
    let b = (bmax - ro) * inv;
    let lo = min(a, b);
    let hi = max(a, b);
    return vec2f(max(max(lo.x, lo.y), max(lo.z, 0.0)), min(hi.x, min(hi.y, hi.z)));
}

// ------------------------------------------------------------ the island

fn isl_ground(xz: vec2f, oct: i32) -> f32 {
    let c = (xz - ISL_C) / vec2f(2600.0, 1500.0);
    let n = noise_fbm2(xz * 0.0011 + 3.0, 3);
    let m = saturate(1.0 - length(c) + (n - 0.5) * 0.45);
    let rid = noise_ridged2(xz * 0.0024, oct);
    var h = 330.0 * pow(m, 0.9) * (0.3 + 0.85 * rid);
    // Mount Pahia: a rougher, lower summit to the south
    let pa = (xz - PAHIA) / 300.0;
    let pah = 630.0 * exp(-dot(pa, pa) * 0.5) * (0.74 + 0.34 * noise_ridged2(xz * 0.005 + 9.0, 3));
    h = max(h, pah);
    // knife-edge saddle from Pahia up to the Otemanu massif
    let sd = xz - mix(PAHIA, OTE, saturate(dot(xz - PAHIA, OTE - PAHIA) / dot(OTE - PAHIA, OTE - PAHIA)));
    h = max(h, 400.0 * exp(-dot(sd, sd) / 60000.0) * (0.75 + 0.35 * rid));
    return h - 12.0 * (1.0 - smoothstep(0.0, 0.06, m));
}

// Otemanu: a basalt plug, vertical walls fluted by erosion, a tilted crown
fn otemanu(p: vec3f) -> f32 {
    let q = p - vec3f(OTE.x, 0.0, OTE.y);
    let qr = rot2(0.45) * q.xz;
    let hy = saturate((q.y - 150.0) / 577.0);
    let r = vec2f(mix(430.0, 250.0, sqrt(hy)), mix(320.0, 180.0, sqrt(hy)));
    let e = length(qr / r);
    let ang = atan2(qr.y, qr.x);
    let flute = noise_grad2(vec2f(ang * 7.0, q.y * 0.004)) * 0.10 + noise_grad2(vec2f(ang * 23.0, q.y * 0.006)) * 0.05;
    let side = (e - 1.0 - flute) * min(r.x, r.y) * 0.8;
    let crown = 727.0 - 70.0 * saturate(qr.x / 260.0 + 0.3) - 45.0 * sq(qr.y / 190.0) + 12.0 * noise_grad2(qr * 0.012);
    return max(side, q.y - crown);
}

fn isl_sdf(p: vec3f, oct: i32) -> f32 {
    let g = (p.y - isl_ground(p.xz, oct)) * 0.5;
    if (p.y > 740.0) { return p.y - 730.0; }
    return op_smin(g, otemanu(p), 70.0);
}

fn isl_march(ro: vec3f, rd: vec3f, n: i32, ctx: Ctx) -> f32 {
    let sp = box_span(ro, rd, vec3f(-4200.0, -20.0, -6800.0), vec3f(2400.0, 745.0, -2600.0));
    if (sp.y <= sp.x) { return -1.0; }
    var t = sp.x;
    for (var i = 0; i < 128; i++) {
        if (i >= n) { break; }
        let p = ro + rd * t;
        let d = isl_sdf(p, 4);
        if (d < 0.0015 * t) { return t; }
        t += max(d, 0.4 + t * 0.0008);
        if (t > sp.y) { break; }
    }
    return -1.0;
}

fn isl_normal(p: vec3f, t: f32) -> vec3f {
    let e = max(1.5, t * 0.002);
    let k = vec2f(1.0, -1.0);
    return normalize(k.xyy * isl_sdf(p + k.xyy * e, 5) + k.yyx * isl_sdf(p + k.yyx * e, 5) +
                     k.yxy * isl_sdf(p + k.yxy * e, 5) + k.xxx * isl_sdf(p + k.xxx * e, 5));
}

fn isl_shadow(p: vec3f, l: vec3f) -> f32 {
    var res = 1.0;
    var t = 12.0;
    for (var i = 0; i < 18; i++) {
        let q = p + l * t;
        if (q.y > 740.0) { break; }
        let h = isl_sdf(q, 3);
        res = min(res, 8.0 * h / t);
        if (res < 0.02) { break; }
        t += max(h, 15.0);
    }
    return saturate(res);
}

fn shade_island(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx, cheap: bool) -> vec3f {
    let n = isl_normal(p, t);
    let steep = 1.0 - n.y;
    let forest = col_hex(0x1d3b17u);
    let forest2 = col_hex(0x3a5f1fu);
    let basalt = col_hex(0x45403au);
    let n1 = noise_fbm2(p.xz * 0.012, 3);
    var alb = mix(forest, forest2, n1 * n1);
    // bare basalt on the walls, streaked with lichen and water stains
    let streak = noise_value2(vec2f(atan2(p.z - OTE.y, p.x - OTE.x) * 70.0, p.y * 0.015));
    let rock = basalt * (0.6 + 0.7 * streak);
    alb = mix(alb, mix(rock, forest, smoothstep(0.6, 0.85, streak) * 0.6), smoothstep(0.4, 0.6, steep + (n1 - 0.5) * 0.3));
    // pale beach at the shoreline
    alb = mix(col_hex(0xcfc3a0u), alb, smoothstep(2.0, 7.0, p.y));
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    if (!cheap && dif > 0.0) { sh = isl_shadow(p + n * 4.0, l.sun); }
    let occ = 0.5 + 0.5 * saturate(n.y * 0.7 + 0.3);
    var c = alb * (l.sun_c * dif * sh * 1.2 + l.amb * occ);
    // tropical haze: the island sits in humid air 3-5 km away
    let fog = 1.0 - exp(-t * 0.00011 * l.haze);
    return mix(c, l.hor * 0.9, fog);
}

// ------------------------------------------------------------ bungalows

// in the row frame: x = across (away from camera), y, z = along the row
fn to_row(p: vec3f) -> vec3f {
    let d = p.xz - ROW0;
    return vec3f(dot(d, ROWP), p.y, dot(d, ROWD));
}
fn to_row_dir(v: vec3f) -> vec3f { return vec3f(dot(v.xz, ROWP), v.y, dot(v.xz, ROWD)); }

// hip roof, base centred at the origin: eave half-extents e, height h
fn hip_roof(q: vec3f, e: vec2f, h: f32) -> f32 {
    let sa = h / e.x;
    let sb = h / e.y;
    let d1 = (abs(q.x) * sa + q.y - h) / sqrt(1.0 + sa * sa);
    let d2 = (abs(q.z) * sb + q.y - h) / sqrt(1.0 + sb * sb);
    return max(max(d1, d2), -q.y);
}

fn bung_sdf(r: vec3f) -> vec2f {
    // walkway behind the row, on posts
    let wz = clamp(r.z, -10.0, (ROWN - 1.0) * ROWS + 4.0);
    var d = vec2f(sdf_box(vec3f(r.x - 8.0, r.y - 1.72, r.z - wz), vec3f(1.1, 0.09, 0.2)), 3.0);
    let i = clamp(round(r.z / ROWS), 0.0, ROWN - 1.0);
    let q = vec3f(r.x, r.y, r.z - i * ROWS);
    let deck = sdf_box(q - vec3f(-1.2, 1.72, 0.0), vec3f(4.8, 0.1, 4.4));
    let spur = sdf_box(q - vec3f(5.3, 1.72, 0.0), vec3f(1.8, 0.09, 1.0));
    d = op_umin(d, vec2f(min(deck, spur), 3.0));
    let body = sdf_box(q - vec3f(1.0, 3.05, 0.0), vec3f(2.7, 1.3, 3.1));
    d = op_umin(d, vec2f(body, 2.0));
    let roof = hip_roof(q - vec3f(1.0, 4.25, 0.0), vec2f(4.3, 4.7), 4.3);
    d = op_umin(d, vec2f(roof, 1.0));
    // railing along the deck front
    let rail = sdf_box(q - vec3f(-5.9, 2.3, 0.0), vec3f(0.05, 0.05, 4.3));
    d = op_umin(d, vec2f(rail, 3.0));
    // stilts: a 3 x 3 grid under the bungalow, and a row under the walkway
    let g = vec2f(q.x + 0.9, q.z);
    let c = g - vec2f(4.3, 4.0) * clamp(round(g / vec2f(4.3, 4.0)), vec2f(-1.0), vec2f(1.0));
    let post = max(length(c) - 0.17, abs(q.y + 1.2) - 2.9);
    let wg = r.z - 3.0 * round(r.z / 3.0);
    let wpost = max(length(vec2f(r.x - 8.0, wg)) - 0.13, abs(r.y + 1.2) - 2.9);
    d = op_umin(d, vec2f(min(post, wpost), 4.0));
    return d;
}

const ROW_MIN: vec3f = vec3f(-6.4, -4.2, -11.0);
const ROW_MAX: vec3f = vec3f(9.3, 8.7, 226.0);

// march in the row frame: returns (t, material)
fn bung_march(ro: vec3f, rd: vec3f, tmax: f32, n: i32) -> vec2f {
    let rl = to_row(ro);
    let dl = to_row_dir(rd);
    let sp = box_span(rl, dl, ROW_MIN, ROW_MAX);
    if (sp.y <= sp.x || sp.x > tmax) { return vec2f(-1.0); }
    var t = sp.x;
    let te = min(sp.y, tmax);
    for (var i = 0; i < 96; i++) {
        if (i >= n) { break; }
        let h = bung_sdf(rl + dl * t);
        if (h.x < 0.002 * t + 0.004) { return vec2f(t, h.y); }
        t += h.x;
        if (t > te) { break; }
    }
    return vec2f(-1.0);
}

fn bung_normal(r: vec3f, t: f32) -> vec3f {
    let e = max(0.01, t * 0.0015);
    let k = vec2f(1.0, -1.0);
    return normalize(k.xyy * bung_sdf(r + k.xyy * e).x + k.yyx * bung_sdf(r + k.yyx * e).x +
                     k.yxy * bung_sdf(r + k.yxy * e).x + k.xxx * bung_sdf(r + k.xxx * e).x);
}

// soft shadow cast by the bungalows (row frame in, world light dir)
fn bung_shadow(r: vec3f, ll: vec3f) -> f32 {
    let sp = box_span(r, ll, ROW_MIN, ROW_MAX);
    if (sp.y <= sp.x) { return 1.0; }
    var res = 1.0;
    var t = max(sp.x, 0.05);
    for (var i = 0; i < 28; i++) {
        let h = bung_sdf(r + ll * t).x;
        res = min(res, 6.0 * h / t);
        if (res < 0.02) { break; }
        t += clamp(h, 0.08, 2.0);
        if (t > sp.y) { break; }
    }
    return saturate(res);
}

fn row_index(r: vec3f) -> f32 { return clamp(round(r.z / ROWS), 0.0, ROWN - 1.0); }

fn shade_bung(r: vec3f, dl: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = bung_normal(r, t);
    let ls = to_row_dir(l.sun);
    let i = row_index(r);
    let q = vec3f(r.x, r.y, r.z - i * ROWS);
    let hb = hash_cell2(vec2i(i32(i), 5), 0xb0u);
    var alb = col_hex(0x6e5236u);
    var emit = vec3f(0.0);
    if (mat < 1.5) {
        // pandanus thatch, bleached grey-gold, laid in courses
        let streak = noise_value2(vec2f((q.x + q.z) * 7.0, q.y * 1.3));
        let course = 0.85 + 0.15 * smoothstep(0.1, 0.5, fract(q.y * 1.6));
        alb = col_hex(0x7d6547u) * (0.7 + 0.45 * streak) * course * (0.9 + 0.2 * hb.x);
    } else if (mat < 2.5) {
        alb = col_hex(0x5a3f2bu) * (0.85 + 0.2 * noise_value2(vec2f(q.z * 9.0, q.y * 2.0)));
        // sliding glass doors on the deck side, a window on each side wall
        let front = step(0.5, -n.x) * step(abs(q.z), 2.3) * step(2.0, q.y) * step(q.y, 3.95);
        let side = step(0.5, abs(n.z)) * step(abs(q.x - 0.4), 1.2) * step(2.6, q.y) * step(q.y, 3.8);
        let win = max(front, side);
        if (win > 0.5) {
            alb = vec3f(0.02, 0.025, 0.03);
            if (l.night > 0.5) {
                let on = step(0.18, hb.y);
                emit = col_kelvin(2600.0) * (1.4 + 1.2 * hb.z) * on * (0.8 + 0.2 * noise_value2(vec2f(q.z * 3.0, q.y * 4.0)));
            }
        }
    } else if (mat < 3.5) {
        alb = col_hex(0x7c5739u) * (0.8 + 0.3 * noise_value2(vec2f(r.z * 6.0, r.x * 0.7)));
    } else {
        alb = col_hex(0x3b2e24u);
    }
    let dif = saturate(dot(n, ls));
    var sh = 1.0;
    if (dif > 0.0) { sh = bung_shadow(r + n * 0.05, ls); }
    // turquoise light bounced up from the lagoon onto eaves and deck undersides
    let bounce = l.sun_c * max(l.sun.y, 0.0) * vec3f(0.12, 0.34, 0.36) * saturate(-n.y * 0.8 + 0.3);
    let sky = l.amb * (0.55 + 0.45 * n.y);
    var c = alb * (l.sun_c * dif * sh + sky + bounce);
    if (l.night > 0.5) {
        // porch lamp over the deck
        let lp = vec3f(-1.95, 3.9, 2.0 * select(-1.0, 1.0, hb.w > 0.5));
        let dv = lp - q;
        let d2 = dot(dv, dv);
        c += alb * col_kelvin(2400.0) * 1.6 * saturate(dot(n, dv * inverseSqrt(d2))) / (d2 + 0.5);
    }
    c += emit;
    return c;
}

// ------------------------------------------------------------ sky & clouds

fn backdrop(rd: vec3f, l: Look, ctx: Ctx, full: bool) -> vec3f {
    if (l.night > 0.5) {
        var c = sky_night(rd) * 1.3 + vec3f(0.0, 0.002, 0.004);
        c += vec3f(0.02, 0.028, 0.045) * pow(saturate(dot(rd, l.sun)), 10.0);
        if (full) {
            c += star_field(rd, 0.9, ctx) * smoothstep(0.02, 0.25, rd.y);
            c += sky_moon(rd, l.sun, 0.46, 1.1);
        }
        return c;
    }
    var c = day_sky(rd, l.sun, l.haze);
    if (full) { c += sky_sun_disk(rd, l.sun, 0.55); }
    return c;
}

// Trade-wind cumulus drawn in angle space (u = azimuth, v = elevation, in
// radians): a flat base and a crown of cauliflower turrets, shaded from the
// boundary gradient as if it were a soft solid. Cheap, and at terminal size
// indistinguishable from a volume.
fn cu_shape(q: vec2f, seed: u32, detail: bool) -> f32 {
    var top = -1.0;
    for (var k = 0; k < 6; k++) {
        let fk = f32(k);
        let hk3 = hash_pcg3(vec3u(seed, u32(k), 77u));
        let hx = hash_unorm(hk3.x);
        let hy = hash_unorm(hk3.y);
        let hz = hash_unorm(hk3.z);
        let xk = (fk - 2.5) * 0.3 + (hx - 0.5) * 0.22;
        let rk = 0.2 + 0.2 * hy;
        let hk = rk * (0.8 + 1.0 * hz) * (1.15 - abs(fk - 2.5) * 0.17) + 0.25 * (1.0 - abs(fk - 2.5) / 2.5);
        let dx = (q.x - xk) / rk;
        top = max(top, hk * sqrt(max(1.0 - dx * dx, 0.0)) - select(0.0, 1.0, abs(dx) > 1.0));
    }
    var wob = 0.0;
    if (detail) {
        wob = (noise_fbm2(q * 6.0 + vec2f(f32(seed % 97u)), 5) - 0.5) * 0.34;
    } else {
        wob = (noise_value2(q * 7.0 + vec2f(f32(seed % 97u))) - 0.5) * 0.2;
    }
    // ragged, slightly lifted base
    let base = -q.y * 2.5 + (noise_value2(vec2f(q.x * 6.0, f32(seed % 13u))) - 0.5) * 0.12;
    return max(q.y - top + wob, base);
}

fn cu_light(q: vec2f, seed: u32, d: f32, sun2: vec3f, l: Look, far: f32, detail: bool) -> vec3f {
    var lit = 0.6;
    if (detail) {
        let e = 0.025;
        let g = vec2f(cu_shape(q + vec2f(e, 0.0), seed, true) - d, cu_shape(q + vec2f(0.0, e), seed, true) - d) / e;
        let inner = saturate(-d * 2.5);
        let n = normalize(vec3f(g * (1.0 - inner * 0.6), 0.3 + inner * 0.5));
        lit = saturate(dot(n, sun2) * 0.6 + 0.4);
    }
    let hgt = saturate(q.y / 1.1);
    // flat grey-blue base, brilliant sunlit turrets
    var col = l.sun_c * (0.15 + 1.0 * lit) * mix(0.25, 1.0, sqrt(hgt)) + l.amb * mix(0.6, 1.2, hgt);
    if (l.night > 0.5) { col = l.sun_c * (0.1 + 0.5 * lit) * 0.8 + l.amb * 1.0; }
    return mix(col, l.hor, far);
}

// (rgb, alpha) of the cumulus field for view direction rd (rd.y > 0)
fn cumulus(rd: vec3f, l: Look, ctx: Ctx, detail: bool) -> vec4f {
    let u = atan2(rd.x, -rd.z) + ctx.t * 0.0004;
    let v = asin(clamp(rd.y, -1.0, 1.0));
    var acc = vec4f(0.0);
    let saz = atan2(l.sun.x, -l.sun.z);
    // the hero: a towering cumulus building behind Otemanu
    {
        let s = 0.13;
        let uc = -0.085 + ctx.t * 0.0002;
        let q = vec2f((u - uc) / (s * 1.5), (v - 0.1) / s);
        if (abs(q.x) < 1.2 && q.y < 1.9 && q.y > -0.3) {
            let d = cu_shape(q, 9u, detail);
            let a = smoothstep(0.04, -0.1, d);
            if (a > 0.0) {
                let sun2 = normalize(vec3f(sin(saz - uc), l.sun.y * 1.5, -cos(saz - uc)));
                acc = vec4f(cu_light(q, 9u, d, sun2, l, 0.12, detail), a);
            }
        }
    }
    if (v > 0.25) { return acc; }
    let cw = 0.3;
    let cell = floor(u / cw);
    for (var k = -1; k <= 1; k++) {
        let c = cell + f32(k);
        let h = hash_cell2(vec2i(i32(c), 11), 0xc10du);
        if (h.w > 0.6 || (c > -2.0 && c < 0.0)) { continue; }
        // farther clouds sit lower and look smaller
        let vb = 0.008 + 0.06 * h.y * h.y;
        let s = 0.018 + vb * 1.1;
        let uc = (c + 0.25 + 0.5 * h.x) * cw;
        let q = vec2f((u - uc) / (s * 1.4), (v - vb) / s);
        if (abs(q.x) > 1.2 || q.y > 1.9 || q.y < -0.3) { continue; }
        let seed = hash_u(u32(i32(c) + 1000));
        let d = cu_shape(q, seed, detail);
        let a = smoothstep(0.04, -0.1, d);
        if (a <= 0.0) { continue; }
        let sun2 = normalize(vec3f(sin(saz - uc), l.sun.y * 1.5, -cos(saz - uc)));
        let col = cu_light(q, seed, d, sun2, l, saturate(1.0 - vb * 16.0) * 0.55, detail);
        acc = vec4f(mix(acc.rgb, col, a * (1.0 - acc.a)) , acc.a + a * (1.0 - acc.a));
    }
    return acc;
}

// low motu (coral islets) fringed with coconut palms along the far reef
fn motu(rd: vec3f) -> f32 {
    let u = atan2(rd.x, -rd.z);
    let v = rd.y;
    let span = smoothstep(0.26, 0.34, u) * (1.0 - smoothstep(1.5, 1.6, u)) + smoothstep(-0.62, -0.7, u) * (1.0 - smoothstep(-2.0, -2.1, u));
    if (span <= 0.0) { return 0.0; }
    let crowns = 0.0022 + 0.0016 * noise_value2(vec2f(u * 260.0, 1.0)) + 0.0012 * noise_value2(vec2f(u * 40.0, 3.0));
    return step(v, crowns * span - 0.0004) * step(-0.002, v);
}

fn lag_sky(rd: vec3f, l: Look, ctx: Ctx, full: bool) -> vec3f {
    var c = backdrop(rd, l, ctx, full);
    if (rd.y > 0.0) {
        let cu = cumulus(rd, l, ctx, full);
        c = mix(c, cu.rgb, cu.a);
    }
    let m = motu(rd);
    if (m > 0.0) {
        let tree = col_hex(0x243a1cu) * (l.sun_c * 0.5 + l.amb);
        c = mix(c, mix(tree, l.hor, 0.55), m);
    }
    return c;
}

// ------------------------------------------------------------ the lagoon

fn lag_depth(xz: vec2f) -> f32 {
    let fwd = -xz.y;
    var d = 1.2 + 1.4 * sq(noise_value2(xz * 0.05 + 1.7)) + 0.3 * noise_value2(xz * 0.15);
    // Sand shelves under our bungalow and along the row; between them the
    // floor slopes away into the deep-blue channel of the lagoon, which
    // climbs again to the island's fringing reef.
    let across = dot(xz - ROW0, ROWP);
    let wob = 5.0 * noise_value2(xz * 0.03 + 7.0);
    let shelf_row = smoothstep(-24.0, -13.0, across + wob);
    let shelf_cam = 1.0 - smoothstep(9.0, 22.0, fwd + wob * 0.8 - max(xz.x, 0.0) * 0.3);
    let reef = smoothstep(1500.0, 2300.0, fwd);
    let shelf = max(max(shelf_row, shelf_cam), reef);
    d += (1.0 - shelf) * (11.0 + 7.0 * noise_value2(xz * 0.01));
    return d;
}

// view ray under the surface: turquoise over sand, blue over the channel
fn lagoon_body(wp: vec3f, rd: vec3f, n: vec3f, dist: f32, l: Look, ctx: Ctx) -> vec3f {
    let rr = refract(rd, n, 0.752);
    let dn = max(-rr.y, 0.04);
    var tb = lag_depth(wp.xz) / dn;
    tb = lag_depth(wp.xz + rr.xz * tb) / dn;
    tb = min(tb, 90.0);
    let bp = wp + rr * tb;
    let depth = -bp.y;
    // floor: white coral sand in wind ripples, dark coral heads here and there
    let rdir = vec2f(0.83, 0.56);
    let ph = dot(bp.xz, rdir) * 9.0 + 3.0 * noise_value2(bp.xz * 0.5);
    let rip = 0.18 * cos(ph) * (1.0 - smoothstep(0.02, 0.06, dist * ctx.px));
    let sn = normalize(vec3f(-rip * rdir.x, 1.0, -rip * rdir.y));
    var alb = col_hex(0xe9dfc6u) * (0.82 + 0.25 * noise_value2(bp.xz * 1.7));
    let cn = noise_fbm2(bp.xz * 0.11 + 4.0, 3);
    let coral = smoothstep(0.64, 0.72, cn) * (1.0 - smoothstep(4.0, 7.0, depth));
    alb = mix(alb, col_hex(0x4f4232u) * (0.7 + 0.6 * noise_value2(bp.xz * 3.0)), coral * 0.9);
    // sparse turtle grass darkens some of the sand
    alb *= 1.0 - 0.25 * smoothstep(0.5, 0.62, cn) * (1.0 - coral);
    // sunlight through the surface, focused into caustics
    let ls = normalize(vec3f(l.sun.x * 0.75, max(l.sun.y, 0.05), l.sun.z * 0.75));
    let foot = dist * ctx.px;
    let cau_k = (1.0 - smoothstep(0.02, 0.12, foot)) * (1.0 - 0.8 * l.night);
    let cau = water_caustics(bp.xz * 1.9 - ls.xz * depth * 1.9, ctx.t * 1.1) * (1.0 - smoothstep(3.0, 9.0, depth));
    var sh = 1.0;
    let rl = to_row(bp);
    if (rl.x > -30.0 && rl.x < 30.0 && rl.z > -30.0 && rl.z < 250.0 && l.night < 0.5) {
        sh = bung_shadow(rl, to_row_dir(ls));
    }
    let tsun = exp(-SIG * depth / ls.y);
    let tamb = exp(-SIG * depth * 1.3);
    let dif = saturate(dot(sn, ls));
    var e_in = l.sun_c * tsun * dif * sh * mix(1.0, 0.6 + 0.8 * cau, cau_k) + l.amb * tamb * 0.9;
    // at night: underwater floodlights beneath the bungalows
    if (l.night > 0.5) {
        let i = row_index(rl);
        let lp = vec3f(-1.0, -0.4, i * ROWS);
        let dv = lp - vec3f(rl.x, -depth, rl.z);
        e_in = e_in * 0.5 + vec3f(0.45, 0.95, 1.0) * 9.0 / (dot(dv, dv) + 4.0);
    }
    let floor_c = alb * e_in;
    let tv = exp(-SIG * tb);
    // in-scattered light of the water column itself
    var deep = (l.sun_c * (0.3 + 0.7 * max(l.sun.y, 0.0)) + l.amb * 1.6) * vec3f(0.004, 0.034, 0.068);
    if (l.night > 0.5) {
        let i = row_index(rl);
        let dv = vec3f(-1.0, -0.8, i * ROWS) - vec3f(rl.x, -1.0, rl.z);
        deep += vec3f(0.02, 0.22, 0.24) * 5.0 / (dot(dv, dv) * 0.25 + 2.0);
    }
    return floor_c * tv + deep * (1.0 - tv);
}

fn reflect_col(wp: vec3f, r: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let bh = bung_march(wp + r * 0.02, r, 300.0, 40);
    if (bh.x > 0.0) {
        let rl = to_row(wp) + to_row_dir(r) * bh.x;
        return shade_bung(rl, to_row_dir(r), bh.x + 20.0, bh.y, l, ctx);
    }
    if (r.y < 0.22) {
        let ti = isl_march(wp, r, 48, ctx);
        if (ti > 0.0) { return shade_island(wp + r * ti, r, ti, l, ctx, true); }
    }
    return lag_sky(r, l, ctx, false);
}

// Wind ripples with analytic slopes. Ripples smaller than a pixel are not
// drawn; their slope variance comes back as roughness, so distant water
// blurs and darkens its reflection (as real rippled water does) instead of
// turning into a mirror or aliasing into noise.
fn lag_normal(xz: vec2f, t: f32, foot: f32) -> vec4f {
    var g = vec2f(0.0);
    var vu = 0.0;
    let q = xz + vec2f(noise_value2(xz * 0.06), noise_value2(xz * 0.06 + 5.3)) * 5.0;
    var ang = 0.5;
    var k = 0.8;
    var a = 0.065;
    for (var i = 0; i < 7; i++) {
        let dir = vec2f(cos(ang), sin(ang));
        let ph = dot(q, dir) * k - sqrt(9.81 * k) * t * 0.55 + f32(i) * 1.7;
        let s = a * k;
        let res = saturate(1.6 - foot * k * 0.5);
        g += dir * (s * cos(ph) * res);
        vu += 0.5 * s * s * (1.0 - res * res);
        ang += 2.05;
        k *= 1.57;
        a *= 0.62;
    }
    return vec4f(normalize(vec3f(-g.x, 1.0, -g.y)), sqrt(vu));
}

fn shade_lagoon(wp: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let foot = t * ctx.px / sqrt(max(-rd.y, 0.02));
    let wn = lag_normal(wp.xz, ctx.t, foot);
    let n = wn.xyz;
    let rough = wn.w;
    let body = lagoon_body(wp, rd, n, t, l, ctx);
    var r = reflect(rd, n);
    r.y = abs(r.y) + 0.002 + rough * 0.6;
    var refl = reflect_col(wp, normalize(r), l, ctx);
    let f = water_fresnel(dot(-rd, n) + rough * 0.9);
    var c = mix(body, refl, f);
    // sun (moon) glitter, widened by the unresolved ripples
    let hv = normalize(l.sun - rd);
    let nh = saturate(dot(n, hv));
    let s2 = rough * rough + 0.00035;
    let th2 = max(1.0 / max(nh * nh, 1e-4) - 1.0, 0.0);
    let d = exp(-th2 / (2.0 * s2)) / (s2 * 25.0);
    if (l.night > 0.5) {
        c += vec3f(0.85, 0.9, 1.0) * d * f * 0.05;
    } else {
        c += l.sun_c * d * f * 2.0;
    }
    // haze over the far lagoon
    let fog = 1.0 - exp(-t * 0.00022 * l.haze);
    return mix(c, l.hor, fog);
}

// ------------------------------------------------------------ palm

// a leaning coconut palm framing the upper left (p space; the camera is
// static, so a flat cut-out reads exactly like a near silhouette)
fn palm(p: vec2f, l: Look, ctx: Ctx) -> vec4f {
    if (p.x > -0.24 || p.y < -1.3) { return vec4f(0.0); }
    let sway = 0.018 * sin(ctx.t * 0.5) + 0.008 * sin(ctx.t * 1.27 + 1.0);
    let crown = vec2f(-0.67 + sway * 0.3, 0.45);
    let base = vec2f(-1.08, -1.2);
    // trunk: a curve leaning in from the lower left, thinning to the crown
    let tt = saturate((p.y - base.y) / (crown.y - base.y));
    let bend = pow(1.0 - tt, 1.7);
    let cx = crown.x + (base.x - crown.x) * bend;
    let dxdy = -1.7 * pow(1.0 - tt, 0.7) * (base.x - crown.x) / (crown.y - base.y);
    let w = mix(0.034, 0.015, tt);
    let dtr = (abs(p.x - cx) - w) / sqrt(1.0 + dxdy * dxdy);
    var a = aa_fill(dtr, ctx) * step(p.y, crown.y + 0.01);
    let across = saturate((p.x - cx) / w * 0.5 + 0.5);
    let rings = 0.8 + 0.2 * smoothstep(0.25, 0.6, fract(p.y * 80.0 + noise_value2(vec2f(p.y * 30.0, 1.0))));
    var col = col_hex(0x6d6253u) * rings * (l.amb * (0.5 + 0.5 * across) * 1.2 + l.sun_c * 0.35 * across);
    // fronds: parabolic rachises that leave the crown and droop under
    // gravity, pinnate leaflets hanging from both sides
    let d = p - crown;
    var fr = 0.0;
    var fshade = 0.0;
    if (dot(d, d) < 0.2) {
        for (var k = 0; k < 12; k++) {
            let fk = f32(k);
            let hh = hash_pcg3(vec3u(u32(k), 5u, 9u));
            // a starburst: most fronds reach sideways, old ones hang, the
            // few upright ones are young and short
            var fa = array<f32, 12>(10.0, 38.0, 68.0, 104.0, 138.0, 166.0, 192.0, 218.0, -18.0, -48.0, -128.0, -158.0);
            let a0 = radians(fa[k]) + 0.15 * (hash_unorm(hh.x) - 0.5) + sway * (1.5 + hash_unorm(hh.z));
            let dir = vec2f(cos(a0), sin(a0));
            let len = (0.27 + 0.1 * hash_unorm(hh.y)) * (1.0 - 0.45 * saturate(dir.y));
            let g = (0.8 + 0.9 * hash_unorm(hh.z)) / len;
            var s0 = clamp(dot(d, dir), 0.0, len);
            for (var it = 0; it < 3; it++) {
                let sp = dir * s0 + vec2f(0.0, -g * s0 * s0);
                let tg = normalize(dir + vec2f(0.0, -2.0 * g * s0));
                s0 = clamp(s0 + dot(d - sp, tg), 0.0, len);
            }
            let sp = dir * s0 + vec2f(0.0, -g * s0 * s0);
            let tg = normalize(dir + vec2f(0.0, -2.0 * g * s0));
            let off = d - sp;
            let side = off.x * tg.y - off.y * tg.x;
            let below = side * tg.x > 0.0;
            let u = s0 / len;
            let lw = len * 0.075 * sin(PI * pow(u, 0.5)) * select(0.55, 1.35, below);
            let as_ = length(off);
            // leaflets slant toward the tip; unresolved they average to a veil
            let stripe = fract(u * 34.0 - as_ / max(lw, 1e-4) * 1.4 + fk * 0.37);
            let leaf = smoothstep(0.02, 0.12, stripe) * smoothstep(0.62, 0.5, stripe);
            let resolve = saturate(len / 34.0 / (2.5 * ctx.px) - 0.3);
            let cov = aa_fill(as_ - lw, ctx) * mix(0.8, 0.35 + 0.65 * leaf, resolve) * (1.0 - smoothstep(0.93, 1.0, u));
            let rach = aa_fill(as_ - max(0.0022, ctx.px * 0.4), ctx) * step(u, 0.97);
            let c = max(cov, rach);
            if (c > fr) {
                fr = c;
                fshade = (0.55 + 0.45 * saturate(0.5 - side / max(lw, 1e-4) * 0.5)) * (0.75 + 0.25 * hash_unorm(hh.x));
            }
        }
    }
    let fcol = col_hex(0x3d5a26u) * fshade * (l.amb * 1.1 + l.sun_c * 0.4);
    col = mix(col, fcol, fr);
    a = max(a, fr);
    // a cluster of coconuts under the crown
    let nut = min(length(d - vec2f(0.006, -0.02)) - 0.011, length(d - vec2f(-0.012, -0.016)) - 0.01);
    let na = aa_fill(nut, ctx);
    col = mix(col, col_hex(0x4a5a26u) * (l.amb + l.sun_c * 0.2) * 0.8, na);
    a = max(a, na);
    if (l.night > 0.5) { col = col * 0.3 + vec3f(0.0006, 0.0008, 0.0012); }
    return vec4f(col, a);
}

// ------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let cam = cam_look_at(CAM, CAM + vec3f(0.02, -0.105, -1.0), 0.0, 50.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    let tw = water_intersect(CAM, rd, 0.0);
    let bh = bung_march(CAM, rd, select(1e4, tw, tw > 0.0), steps(64.0, ctx));
    if (bh.x > 0.0) {
        col = shade_bung(to_row(CAM) + to_row_dir(rd) * bh.x, to_row_dir(rd), bh.x, bh.y, l, ctx);
        col = mix(col, l.hor, 1.0 - exp(-bh.x * 0.00022 * l.haze));
    } else {
        var ti = -1.0;
        if (rd.y > -0.01 && rd.y < 0.2) { ti = isl_march(CAM, rd, steps(80.0, ctx), ctx); }
        if (ti > 0.0) {
            col = shade_island(CAM + rd * ti, rd, ti, l, ctx, false);
        } else if (tw > 0.0) {
            col = shade_lagoon(CAM + rd * tw, rd, tw, l, ctx);
        } else {
            col = lag_sky(rd, l, ctx, true);
        }
    }
    let pa = palm(p, l, ctx);
    col = mix(col, pa.rgb, pa.a);
    return col * exp2(l.exposure);
}
