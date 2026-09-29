//! name: mesa
//! title: Monument Valley
//! category: wilds
//! tags: desert, buttes, red rock, star trails, arizona
//! desc: the Mittens and Merrick Butte rising from red sand, a dirt road leading in
//! themes: sunset, noon, night
//! uses: camera, raymarch, sky, clouds
//! cost: heavy
//! tonemap: punchy
//! fallback: alpine
//! credits: original

// World units are metres. The camera stands on a low rise beside the valley
// road looking north: West Mitten on the left, East Mitten behind it to the
// right (thumbs facing each other), Merrick Butte closer on the right. The
// sunset sun sits low on the left, so the buttes throw kilometre-long
// shadows across the valley floor.

struct Look {
    sun: vec3f,       // toward the key light (sun or moon)
    night: f32,
    haze: f32,        // sky-model haze
    exposure: f32,
    shimmer: f32,     // heat shimmer / mirage strength
    clouds: f32,      // high cloud coverage
    fogd: f32,        // aerial perspective density per metre
    // sun and sky light, precomputed from the sky model (sky_sun_light and
    // sky_ambient times ozone): per pixel they would cost two sky integrals
    sun_c: vec3f,
    amb: vec3f,
    // multiply-scattered airlight the single-scattering sky lacks at the
    // horizon (without it a noon horizon turns olive instead of pale)
    hz: vec3f,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            l = Look(sky_sun_dir(62.0, 150.0), 0.0, 1.8, -0.9, 1.0, 0.16, 0.00007,
                     vec3f(2.99, 2.70, 2.46), vec3f(0.207, 0.348, 0.628), vec3f(0.24, 0.29, 0.38));
        }
        case 2u: {
            l = Look(sky_sun_dir(32.0, -128.0), 1.0, 1.0, 1.3, 0.0, 0.0, 0.00006,
                     vec3f(0.03, 0.036, 0.052), vec3f(0.004, 0.0055, 0.0105), vec3f(0.0));
        }
        default: {
            l = Look(sky_sun_dir(4.0, -118.0), 0.0, 1.4, 0.1, 0.3, 0.4, 0.00006,
                     vec3f(1.67, 0.72, 0.158), vec3f(0.2, 0.19, 0.21), vec3f(0.07, 0.045, 0.04));
        }
    }
    return l;
}

// Ozone: the library sky has none, and without its Chappuis absorption a low
// sun turns the sky olive. Transmittance along the grazing sun path.
fn ozone_t(sun_y: f32) -> vec3f {
    let am = inverseSqrt(sun_y * sun_y + 0.00786);
    return exp(-vec3f(0.0195, 0.0564, 0.00256) * am);
}

// ------------------------------------------------------------------ land

const TOP_ALL: f32 = 322.0;

struct Butte { c: vec2f, b: vec2f, rot: f32, ht: f32, top: f32, rr: f32, salt: f32 }

// footprint centre, half size, rotation, talus height, cliff top, talus reach
fn butte(i: i32) -> Butte {
    switch (i) {
        case 0: { return Butte(vec2f(-470.0, -2000.0), vec2f(128.0, 66.0), 0.06, 120.0, 302.0, 290.0, 1.0); }
        case 1: { return Butte(vec2f(-284.0, -1990.0), vec2f(21.0, 16.0), 0.0, 104.0, 282.0, 150.0, 2.0); }
        case 2: { return Butte(vec2f(170.0, -2600.0), vec2f(150.0, 76.0), -0.05, 126.0, 308.0, 320.0, 3.0); }
        case 3: { return Butte(vec2f(-28.0, -2590.0), vec2f(24.0, 18.0), 0.0, 112.0, 292.0, 160.0, 4.0); }
        default: { return Butte(vec2f(640.0, -1850.0), vec2f(130.0, 100.0), 0.35, 132.0, 236.0, 370.0, 5.0); }
    }
}

// the valley road: centre x at distance s = -z ahead
fn road_x(s: f32) -> f32 {
    return 6.0 - 0.036 * s + 26.0 * sin(s * 0.0031) + 9.0 * sin(s * 0.011 + 2.0);
}

fn floor_h(xz: vec2f) -> f32 {
    let n = noise_fbm2(xz * 0.0015 + vec2f(3.1, 7.7), 3);
    let near = smoothstep(60.0, 500.0, length(xz));
    return (n - 0.5) * 18.0 * near;
}

// Distance to the buttes: talus skirts (resting on the floor) and cliff
// columns → (distance, material 1 talus / 2 cliff). Material 0 marks a
// conservative bound to step by, never a surface: outside a skirt, or far
// enough from a butte that its noise cannot matter — then the noise is not
// evaluated at all. The open floor itself is traced separately.
fn buttes(p: vec3f, surf: bool) -> vec2f {
    if (p.y > TOP_ALL && !surf) { return vec2f(p.y - TOP_ALL + 4.0, 0.0); }
    var res = vec2f(1e5, 0.0);
    var fl = 0.0;
    var have_fl = false;
    for (var i = 0; i < 5; i++) {
        let b = butte(i);
        let q = p.xz - b.c;
        let rad = length(b.b) + 22.0;
        let lq = length(q);
        if (lq > rad + b.rr) {
            if (!surf) { res = op_umin(res, vec2f(lq - rad - b.rr + 1.0, 0.0)); }
            continue;
        }
        let qr = rot2(b.rot) * q;
        let amp = min(b.b.x, b.b.y) * 0.16;
        let fp0 = sdf2_round_box(qr, b.b, min(b.b.x, b.b.y) * 0.55);
        let lean = max(p.y - b.ht, 0.0) * 0.055;
        // coarse lower bound: outline noise moves fp by <= amp, gullies raise
        // the talus by <= 22 %, flutes cut <= 9 m, the floor stays below 9 m
        let s_hi = saturate(1.0 - max(fp0 - amp, 0.0) / b.rr);
        let g_lo = (p.y - 9.0 - b.ht * s_hi * s_hi * 1.22) * 0.62;
        let c_lo = max(fp0 - amp - 9.0 + lean, p.y - b.top - 4.0) * 0.8;
        if (fp0 - amp > b.rr) {
            if (!surf) { res = op_umin(res, vec2f(max(fp0 - amp - b.rr + 0.5, c_lo), 0.0)); }
            continue;
        }
        let lo = min(g_lo, c_lo);
        if (lo > 12.0) {
            if (!surf) { res = op_umin(res, vec2f(lo, 0.0)); }
            continue;
        }
        if (!have_fl) {
            fl = floor_h(p.xz);
            have_fl = true;
        }
        let fp = fp0 + amp * noise_grad2(qr * 0.011 + vec2f(b.salt * 7.1, 1.3));
        // concave talus: steep under the cliff, flattening into the floor,
        // cut by gullies running downslope
        let r = max(fp, 0.0);
        let s = saturate(1.0 - r / b.rr);
        if (s > 0.0) {
            let gul = noise_grad2(vec2f(qr.x * 0.019, r * 0.016 + b.salt)) + noise_grad2(vec2f(qr.y * 0.019 + 9.0, r * 0.016));
            let tal = b.ht * s * s * (1.0 + 0.055 * gul) - 2.0 * (1.0 - s);
            // step factor from the steepest slope within reach: the skirt is
            // gentle far from the cliff, so rays over it need not crawl
            let sm = min(s + 0.2, 1.0);
            let slope = 3.0 * b.ht * sm / b.rr + 0.006 * b.ht * sm * sm;
            res = op_umin(res, vec2f((p.y - fl - tal) * inverseSqrt(1.0 + slope * slope), 1.0));
        } else {
            // fp carries outline noise (gradient up to ~1.45): scale the bound
            if (!surf) { res = op_umin(res, vec2f((r - b.rr) * 0.75 + 0.5, 0.0)); }
        }
        if (lq < rad + 30.0) {
            var flute = 0.0;
            {
                flute = 3.4 * (noise_grad2(vec2f(qr.x * 0.06, p.y * 0.006 + b.salt)) +
                               noise_grad2(vec2f(qr.y * 0.06 + 5.0, p.y * 0.006)));
            }
            // walls lean in a little toward the top; rounded shoulders
            let top = b.top + 3.0 * noise_grad2(qr * 0.02 + b.salt);
            res = op_umin(res, vec2f(op_smax(fp + flute + lean, p.y - top, 14.0) * 0.8, 2.0));
        } else {
            if (!surf) { res = op_umin(res, vec2f(lq - rad, 0.0)); }
        }
    }
    return res;
}

// the span of a ray inside any butte's bounding circle (xz)
fn butte_span(ro: vec3f, rd: vec3f) -> vec2f {
    var t0 = 1e9;
    var t1 = -1e9;
    let d = rd.xz;
    let a = max(dot(d, d), 1e-8);
    for (var i = 0; i < 5; i++) {
        let b = butte(i);
        let rr = length(b.b) + 22.0 + b.rr;
        let oc = ro.xz - b.c;
        let hb = dot(oc, d);
        let disc = hb * hb - a * (dot(oc, oc) - rr * rr);
        if (disc > 0.0) {
            let sq = sqrt(disc);
            t0 = min(t0, (-hb - sq) / a);
            t1 = max(t1, (-hb + sq) / a);
        }
    }
    return vec2f(t0, t1);
}

fn map(p: vec3f, ctx: Ctx) -> vec2f { return buttes(p, true); }

// The open floor: gentle (slope < 0.12), so step by height over slope and
// finish with a secant between the last two samples. A downward ray always
// lands: no sky can leak through below the horizon.
fn trace_floor(ro: vec3f, rd: vec3f) -> f32 {
    if (rd.y > -1e-5) { return -1.0; }
    var t = max((9.2 - ro.y) / rd.y, 0.0);
    let tend = (-9.5 - ro.y) / rd.y;
    var tp = t;
    var hp = 1.0;
    for (var i = 0; i < 72; i++) {
        let p = ro + rd * t;
        let h = p.y - floor_h(p.xz);
        if (h < 0.0) {
            if (i == 0) { return t; }
            return tp + (t - tp) * hp / (hp - h);
        }
        tp = t;
        hp = h;
        t += max(h / (0.12 - rd.y), 0.004 * t + 0.2);
        if (t > tend) { return tend; }
    }
    return t;
}

fn trace_buttes(ro: vec3f, rd: vec3f, tmax_in: f32, pxa: f32, n: i32) -> vec2f {
    let span = butte_span(ro, rd);
    var tmax = min(tmax_in, span.y);
    if (rd.y > 0.0) { tmax = min(tmax, (TOP_ALL - ro.y) / rd.y); }
    var t = max(4.0, span.x);
    if (t > tmax) { return vec2f(-1.0); }
    for (var i = 0; i < 200; i++) {
        if (i >= n) { break; }
        let h = buttes(ro + rd * t, false);
        let eps = max(0.05, 0.45 * pxa * t);
        if (h.x < eps && h.y > 0.5) { return vec2f(t, h.y); }
        t += max(h.x, eps);
        if (t > tmax) { break; }
    }
    return vec2f(-1.0);
}

fn floor_normal(xz: vec2f, t: f32, pxa: f32) -> vec3f {
    let e = max(0.5, t * pxa);
    let hx = floor_h(xz + vec2f(e, 0.0)) - floor_h(xz - vec2f(e, 0.0));
    let hz = floor_h(xz + vec2f(0.0, e)) - floor_h(xz - vec2f(0.0, e));
    return normalize(vec3f(-hx, 2.0 * e, -hz));
}

// distance to the cliff columns alone, without noise, for the sun
// visibility march: the talus skirts' own shadows are negligible beside the
// cliffs' (and would self-shadow through the noise)
fn obstacle(p: vec3f) -> f32 {
    var d = 1e5;
    for (var i = 0; i < 5; i++) {
        let b = butte(i);
        let q = p.xz - b.c;
        let rad = length(b.b) + 22.0;
        let lq = length(q);
        if (lq > rad + 20.0) {
            d = min(d, lq - rad);
            continue;
        }
        let qr = rot2(b.rot) * q;
        let fp = sdf2_round_box(qr, b.b, min(b.b.x, b.b.y) * 0.55) +
                 min(b.b.x, b.b.y) * 0.16 * noise_grad2(qr * 0.011 + vec2f(b.salt * 7.1, 1.3));
        d = min(d, max(fp + 6.0 + max(p.y - b.ht, 0.0) * 0.055, p.y - b.top) * 0.75);
    }
    return d;
}

fn butte_shadow(ro: vec3f, l: vec3f) -> f32 {
    var res = 1.0;
    var t = 3.0;
    for (var i = 0; i < 32; i++) {
        let p = ro + l * t;
        if (p.y > TOP_ALL) { break; }
        let h = obstacle(p);
        res = min(res, 14.0 * h / t);
        if (res < 0.01) { break; }
        t += clamp(h, 1.5 + t * 0.03, 400.0);
        if (t > 7000.0) { break; }
    }
    return smoothstep(0.0, 1.0, res);
}

// sagebrush: (height at xz, offset from the bush centre / radius, cell random)
fn sage(xz: vec2f, s: f32) -> vec4f {
    let cs = 2.6;
    let g = rot2(0.52) * xz;
    let c = floor(g / cs);
    let h = hash_cell2(vec2i(c), 0x5a9eu);
    // the road and its verges are bare; elsewhere the scrub grows in clumps
    let rd = abs(xz.x - road_x(s));
    let clump = noise_value2(xz * 0.045);
    if (h.w > 0.62 * smoothstep(0.25, 0.7, clump) || rd < 5.5) { return vec4f(0.0, 1.0, 0.0, h.x); }
    let ctr = (c + 0.5 + (h.xy - 0.5) * 0.5) * cs;
    let rad = 0.35 + 0.5 * h.z * h.z + 0.2 * clump;
    let o = (g - ctr) / rad;
    let height = rad * (0.9 + 0.5 * h.x) * sqrt(saturate(1.0 - dot(o, o)));
    return vec4f(height, o, h.x);
}

// ------------------------------------------------------------------ sky

fn far_mesas(rd: vec3f) -> f32 {
    let az = atan2(rd.x, -rd.z);
    let n = noise_fbm2(vec2f(az * 5.0 + 11.0, 2.0), 4);
    let m = noise_fbm2(vec2f(az * 1.7 + 3.0, 5.0), 3);
    let cliff = smoothstep(0.5, 0.515, n);
    let talus = smoothstep(0.32, 0.5, n);
    return (0.013 * cliff + 0.009 * talus) * (0.6 + 0.8 * m);
}

// star trails: stars on rows of constant distance from the celestial pole,
// each drawn as the arc it swept over the last `trail` radians
fn trails(rd: vec3f, pxa: f32, t: f32) -> vec3f {
    let pole = sky_sun_dir(37.0, 4.0);
    let e1 = normalize(cross(pole, vec3f(0.0, 1.0, 0.0)));
    let e2 = cross(e1, pole);
    let th = acos(clamp(dot(rd, pole), -1.0, 1.0));
    let ph = atan2(dot(rd, e2), dot(rd, e1));
    let rate = TAU / 86164.0 * 80.0;
    let trail = radians(22.0);
    let dth = radians(1.05);
    let rowf = th / dth;
    let row0 = floor(rowf);
    let other = select(row0 - 1.0, row0 + 1.0, fract(rowf) > 0.5);
    let sig = max(pxa * 0.42, 0.00025);
    var col = vec3f(0.0);
    for (var k = 0; k < 2; k++) {
        let row = select(other, row0, k == 0);
        if (row < 1.0) { continue; }
        let thc = (row + 0.5) * dth;
        let nph = max(floor(TAU * sin(thc) / dth), 1.0);
        let cw = TAU / nph;
        let a0 = ph - rate * t;
        let j0 = floor(a0 / cw);
        let nc = i32(ceil(trail / cw)) + 1;
        for (var j = 0; j < 24; j++) {
            if (j >= nc) { break; }
            let jj = j0 + f32(j);
            let jm = fmod_pos(jj, nph);
            let h = hash_cell2(vec2i(i32(row), i32(jm)), 0x7ea1u);
            if (h.w > 0.3) { continue; }
            let phi0 = (jj + h.x) * cw;
            // how far along its trail the star has come past this pixel
            let f = (phi0 - a0) / trail;
            if (f < 0.0 || f > 1.0) { continue; }
            let ths = (row + 0.15 + 0.7 * h.y) * dth;
            let d = th - ths;
            let b = (0.006 + 0.5 * pow(h.z, 10.0)) * 0.0016 / sig;
            let temp = mix(3300.0, 10000.0, fract(h.z * 7.13 + h.x));
            let ends = smoothstep(0.0, 0.04, 1.0 - f) * mix(1.0, 0.35, f);
            col += mix(vec3f(1.0), col_kelvin(temp), 0.6) * b * ends * exp(-d * d / (2.0 * sig * sig));
        }
    }
    return col;
}

// day sky with ozone; with a low sun, a faint pink band (the Belt of Venus)
// rises opposite it
fn day_sky(rd: vec3f, l: Look) -> vec3f {
    var c = sky_atmosphere_haze(rd, l.sun, l.haze) * ozone_t(l.sun.y);
    let y = max(rd.y, 0.0);
    c += l.hz * exp(-y * 16.0);
    let low = sstep(0.2, 0.03, l.sun.y);
    if (low > 0.0) {
        let anti = saturate(-dot(normalize(rd.xz + vec2f(1e-4)), normalize(l.sun.xz)) * 0.5 + 0.5);
        c += vec3f(0.3, 0.12, 0.16) * exp(-abs(y - 0.04) * 28.0) * anti * anti * low * col_luma(c);
    }
    return c;
}

fn sky(rd: vec3f, l: Look, pxa: f32, ctx: Ctx) -> vec3f {
    if (l.night > 0.5) {
        var c = sky_night(rd) * 1.4;
        // faint glow from the moon behind the camera and from distant towns
        c += vec3f(0.006, 0.008, 0.014) * pow(saturate(dot(rd, l.sun) * 0.5 + 0.5), 3.0);
        c += vec3f(0.004, 0.0035, 0.003) * exp(-max(rd.y, 0.0) * 30.0);
        c += trails(rd, pxa, ctx.t) * smoothstep(0.0, 0.05, rd.y);
        return c;
    }
    var c = day_sky(rd, l);
    c += sky_sun_disk(rd, l.sun, 0.53);
    if (rd.y > 0.0 && l.clouds > 0.0) {
        let hp = rd.xz / (rd.y + 0.06) * 1.3;
        let cs = cloud_sheet(hp * vec2f(0.35, 1.0) + vec2f(4.0, 1.0), l.clouds, ctx);
        let fwd = pow(saturate(dot(rd, l.sun) * 0.5 + 0.5), 3.0);
        let lit = l.sun_c * (0.35 + 1.2 * fwd) * mix(0.55, 1.15, cs.y) + l.amb * 0.7;
        c = mix(c, lit, cs.x * 0.6 * smoothstep(0.05, 0.2, rd.y));
    }
    return c;
}

// ------------------------------------------------------------------ shading

fn aerial(col: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    var hor: vec3f;
    if (l.night > 0.5) {
        hor = sky_night(vec3f(rd.x, 0.01, rd.z)) * 1.4 + vec3f(0.004, 0.0035, 0.003);
    } else {
        hor = day_sky(normalize(vec3f(rd.x, 0.015, rd.z)), l);
    }
    let k = 1.0 - exp(-t * l.fogd);
    return mix(col, hor, k);
}

fn shade_rock(p: vec3f, n: vec3f, mat: f32, l: Look) -> vec3f {
    let fl = floor_h(p.xz);
    let hy = p.y - fl;
    // De Chelly sandstone cliff, varnish streaks, grey Moenkopi cap
    let sand = col_hex(0xb4532cu);
    let varn = col_hex(0x5e3024u);
    let cap = col_hex(0x7c5a48u);
    let shale = col_hex(0x7e4030u);
    var alb: vec3f;
    if (mat > 1.5) {
        let streak = smoothstep(0.55, 0.8, noise_fbm2(vec2f((p.x + p.z) * 0.09, p.y * 0.006), 4));
        let band = 0.9 + 0.1 * sin(p.y * 0.21 + noise_value2(vec2f(p.x * 0.01, p.y * 0.05)) * 3.0);
        alb = mix(sand * band, varn, streak * 0.55);
        alb = mix(alb, cap, smoothstep(268.0, 285.0, p.y + 8.0 * noise_value2(p.xz * 0.03)));
        alb = mix(alb, shale, sstep(140.0, 120.0, p.y) * 0.6);
    } else {
        // talus: shale slopes, paler where the scree is fresh
        // scree streaks run downslope, paler where the rock fall is fresh
        let n1 = noise_fbm2(vec2f((p.x - p.z * 0.3) * 0.045, p.y * 0.012), 3);
        alb = mix(shale, col_hex(0x9a5a40u), smoothstep(0.3, 0.8, n1));
        alb = mix(col_hex(0xb26a42u), alb, smoothstep(2.0, 18.0, hy));
    }
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    if (dif > 0.0) { sh = butte_shadow(p + n * 3.0, l.sun); }
    // warm bounce from the red sand
    let bounce = col_hex(0xb4643au) * (l.sun_c * max(l.sun.y, 0.05) * 0.55 + l.amb * 0.4) * saturate(0.6 - 0.6 * n.y);
    let sky_occ = 0.55 + 0.45 * saturate(n.y * 0.5 + 0.5);
    return alb * (l.sun_c * dif * sh * 1.15 + l.amb * sky_occ * 1.25 + bounce);
}

fn shade_floor(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let s = -p.z;
    // bush occluding the ray just before it lands: step back along the ray at
    // a few heights above the ground (a cheap 2.5D layer)
    let tana = max(-rd.y, 0.004) / max(length(rd.xz), 1e-3);
    let dir = normalize(rd.xz);
    var bush = vec4f(0.0);
    var bush_y = 0.0;
    var at = p.xz;
    if (t < 900.0) {
        for (var k = 0; k < 6; k++) {
            let y = 1.35 - f32(k) * 0.24;
            let q = p.xz - dir * (y / tana);
            let b = sage(q, -q.y);
            if (b.x > y) {
                bush = b;
                bush_y = y;
                at = q;
                break;
            }
        }
    }
    let rdist = abs(p.x - road_x(s));
    // sun visibility: butte shadow, then the sagebrush shadows
    var sh = butte_shadow(p + vec3f(0.0, 1.0, 0.0), l.sun);
    let sdir = normalize(l.sun.xz);
    let stan = l.sun.y / max(length(l.sun.xz), 1e-3);
    if (sh > 0.0 && t < 600.0 && bush.x <= 0.0) {
        for (var k = 0; k < 8; k++) {
            let y = 0.12 + f32(k) * 0.17;
            let q = p.xz + sdir * (y / max(stan, 0.02));
            if (sage(q, -q.y).x > y) {
                sh *= 0.12;
                break;
            }
        }
    }
    if (bush.x > 0.0) {
        // grey-green sagebrush dome, darker inside and underneath
        let cy = saturate(bush_y / max(bush.x, 1e-3));
        let nb = normalize(vec3f(bush.y, sqrt(saturate(1.0 - dot(bush.yz, bush.yz))) + 0.2, bush.z));
        let alb = mix(col_hex(0x4f5a3eu), col_hex(0x7c8466u), bush.w) * (0.5 + 0.5 * cy);
        let dif = saturate(dot(nb, l.sun) * 0.6 + 0.4);
        return alb * (l.sun_c * dif * sh * 0.9 + l.amb * 1.1);
    }
    // sand: orange-red with pebbly darker patches and paler drifts
    let n1 = noise_fbm2(p.xz * 0.013, 4);
    let n2 = noise_value2(p.xz * 0.17);
    var alb = mix(col_hex(0xa0532eu), col_hex(0xc27a4au), smoothstep(0.3, 0.75, n1));
    alb *= 0.85 + 0.25 * n2;
    // the road: packed, paler, with darker tyre ruts
    let road = sstep(4.6, 3.4, rdist);
    let rut = exp(-sq((rdist - 1.15) / 0.35)) * 0.35 * road;
    alb = mix(alb, col_hex(0xc98f6au), road * 0.8);
    alb *= 1.0 - rut;
    let n = floor_normal(p.xz, t, ctx.px / 2.35);
    // rough sand scatters like regolith (Lommel-Seeliger): a grazing sun
    // still lights it brightly, so shadows read long and crisp
    let mu0 = saturate(dot(n, l.sun));
    let mu = saturate(dot(n, -rd));
    let dif = mix(mu0, 2.0 * mu0 / (mu0 + mu + 1e-3), 0.32);
    // sparse scrub darkens the far floor where the bushes are sub-pixel
    let far = smoothstep(400.0, 2000.0, t);
    alb = mix(alb, alb * 0.9, far);
    return alb * (l.sun_c * dif * sh * 1.1 + l.amb * 1.2);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(0.0, 10.0, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.0, 0.058, -1.0), 0.0, 24.0);
    var rd = cam_ray(cam, p);
    let pxa = ctx.px / cam.zoom;
    // heat shimmer in the air just above the far floor
    if (l.shimmer > 0.0 && abs(rd.y) < 0.03) {
        let w = noise_grad2(vec2f(rd.x * 260.0, rd.y * 1400.0 - ctx.t * 2.2));
        rd.y += l.shimmer * 0.0012 * w * exp(-abs(rd.y + 0.004) * 150.0);
        rd = normalize(rd);
    }
    var col: vec3f;
    let tf = trace_floor(ro, rd);
    var hit = vec2f(-1.0);
    if (rd.y < 0.16) {
        hit = trace_buttes(ro, rd, select(30000.0, tf + 2.0, tf > 0.0), pxa, steps(110.0, ctx));
    }
    var is_rock = hit.x > 0.0;
    if (is_rock && tf > 0.0 && hit.y < 1.5) {
        // a talus hit right at the floor is the floor
        let hp = ro + rd * hit.x;
        is_rock = hp.y - floor_h(hp.xz) > 1.5;
    }
    if (!is_rock && tf > 0.0) { hit = vec2f(tf, 0.0); }
    if (hit.x > 0.0) {
        let hp = ro + rd * hit.x;
        if (is_rock) {
            let n = rm_normal(hp, hit.x, ctx);
            col = shade_rock(hp, n, hit.y, l);
        } else {
            col = shade_floor(hp, rd, hit.x, l, ctx);
            // noon mirage: the far flat floor turns into a sheet of sky
            if (l.shimmer > 0.5 && hit.x > 2500.0) {
                let m = smoothstep(2500.0, 9000.0, hit.x) * 0.55;
                col = mix(col, sky(normalize(vec3f(rd.x, -rd.y, rd.z)), l, pxa, ctx), m);
            }
        }
        col = aerial(col, rd, hit.x, l, ctx);
    } else {
        col = sky(rd, l, pxa, ctx);
        // the far mesas along the horizon, lost in haze
        let mh = far_mesas(rd);
        if (rd.y < mh + pxa) {
            let cov = saturate((mh - rd.y) / max(pxa, 1e-5) + 0.5);
            var rock: vec3f;
            if (l.night > 0.5) {
                rock = vec3f(0.003, 0.004, 0.007);
            } else {
                rock = col_hex(0x9a5a40u) * (l.sun_c * 0.5 * saturate(l.sun.y + 0.3) + l.amb);
            }
            let hz = aerial(rock, rd, 24000.0, l, ctx);
            col = mix(col, hz, cov);
        }
    }
    return col * exp2(l.exposure);
}
