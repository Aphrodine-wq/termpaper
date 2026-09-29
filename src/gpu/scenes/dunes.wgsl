//! name: dunes
//! title: Sahara Dunes
//! category: wilds
//! tags: desert, sand, dunes, caravan, sahara
//! desc: knife-edged Saharan dunes, sand streaming off the crests, a camel caravan crossing
//! themes: golden, noon, moonlit
//! uses: camera, sdf, sky, stars
//! cost: heavy
//! fallback: sand
//! credits: original

// World units are metres. Long sinuous seif dunes run away from the camera
// (along -z), one every LAM metres. We stand on the crest of one of them:
// its knife edge leads the eye up to a summit where a caravan crosses. The
// wind blows from the left, so every slip face is on the right; the low sun
// is on the left, so each ridge splits into a lit face and a cool shadow.

struct Look {
    sun: vec3f,       // toward the key light (sun or moon)
    night: f32,
    haze: f32,
    exposure: f32,
    fogd: f32,        // dust haze per metre
    sun_c: vec3f,     // precomputed from the sky model (with ozone)
    amb: vec3f,
    hz: vec3f,        // horizon airlight
    sand: vec3f,      // sand albedo
    wb: vec3f,        // white balance: a photographer's daylight setting
                      // keeps sunlit sand warm and turns the shadows cool
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            l = Look(sky_sun_dir(56.0, -55.0), 0.0, 2.2, -1.75, 0.00026,
                     vec3f(2.98, 2.67, 2.42), vec3f(0.199, 0.348, 0.625), vec3f(0.3, 0.3, 0.32),
                     col_hex(0xc98f5cu), vec3f(1.0));
        }
        case 2u: {
            l = Look(sky_sun_dir(24.0, -70.0), 1.0, 1.0, 1.0, 0.00016,
                     vec3f(0.05, 0.062, 0.09), vec3f(0.005, 0.007, 0.013), vec3f(0.0),
                     col_hex(0xc9a47cu), vec3f(1.0));
        }
        default: {
            l = Look(sky_sun_dir(7.0, -78.0), 0.0, 2.0, 0.1, 0.00032,
                     vec3f(2.02, 1.04, 0.505), vec3f(0.19, 0.26, 0.43), vec3f(0.08, 0.06, 0.05),
                     col_hex(0xd58a4eu), vec3f(0.9, 0.97, 1.16));
        }
    }
    return l;
}

fn ozone_t(sun_y: f32) -> vec3f {
    let am = inverseSqrt(sun_y * sun_y + 0.00786);
    return exp(-vec3f(0.0195, 0.0564, 0.00256) * am);
}

// ------------------------------------------------------------------ dunes

const LAM: f32 = 170.0;     // ridge spacing
const H_MAX: f32 = 60.0;    // above every crest

// crest line of ridge i at distance s = -z (x offset from i * LAM); the
// fine wiggle fades out far away, where it is sub-pixel
fn crest_off(i: f32, s: f32) -> f32 {
    // sums of sines with per-ridge phases: smooth, sinuous and cheap
    let a = i * 1.7 + 2.4;
    let b = i * 4.13 + 0.7;
    return 24.0 * (sin(s * 0.0105 + a) - sin(a)) + 9.0 * (sin(s * 0.0237 + b) - sin(b)) +
           3.5 * sin(s * 0.061 + i * 2.9) * smoothstep(0.0, 60.0, s);
}

// crest height of ridge i; ours (i = 0) falls away from the camera into a
// saddle and climbs again to a far summit
fn crest_h(i: f32, s: f32) -> f32 {
    let n = 31.0 + 9.0 * sin(s * 0.0071 + i * 3.1) + 5.0 * sin(s * 0.0163 + i * 1.37) + 4.0 * sin(i * 2.71);
    if (i == 0.0) {
        let ours = 56.0 - 22.0 * smoothstep(-5.0, 115.0, s) + 12.0 * smoothstep(120.0, 290.0, s);
        return mix(ours, n, smoothstep(300.0, 520.0, s));
    }
    return n;
}

// interdune corridors: low wind-shaped hummocks
fn base_h(xz: vec2f, s: f32) -> f32 {
    var b = 1.6 + 1.4 * sin(xz.x * 0.031 + sin(xz.y * 0.017) * 2.0) * sin(xz.y * 0.023 + xz.x * 0.004);
    if (s < 700.0) { b += 1.4 * noise_value2(xz * vec2f(0.016, 0.03)) * (1.0 - smoothstep(400.0, 700.0, s)); }
    return b;
}

// one ridge: (height, signed distance from its crest (+ = lee), crest height)
fn ridge(x: f32, s: f32, i: f32) -> vec3f {
    // ridges are not evenly spaced: a fixed per-ridge shift breaks the comb
    let d = x - (i * LAM + 36.0 * sin(i * 2.31) + crest_off(i, s));
    let a = crest_h(i, s);
    // gentle windward face, 33-degree slip face; the crest rounded over ~1 m
    let k = select(0.42, 0.68, d > 0.0);
    return vec3f(a - k * (sqrt(d * d + 0.04) - 0.2), d, a);
}

// (height, crest distance and crest height of the dominant ridge, ridge id)
fn dune(xz: vec2f) -> vec4f {
    let s = -xz.y;
    let i0 = floor(xz.x / LAM);
    let r0 = ridge(xz.x, s, i0);
    let r1 = ridge(xz.x, s, i0 + 1.0);
    var r = vec4f(r0, i0);
    if (r1.x > r0.x) { r = vec4f(r1, i0 + 1.0); }
    r.x = op_smax(op_smax(r0.x, r1.x, 6.0), base_h(xz, s), 7.0);
    return r;
}

fn dune_h(xz: vec2f) -> f32 { return dune(xz).x; }

// sand blowing off a crest (dn = dune() at p): a thin sheet leaving the brink
// downwind, rising and spreading, streaked by gusts that run along the ridge
fn plume(p: vec3f, dn: vec4f, t: f32) -> f32 {
    let d = dn.y;
    if (d < -1.0 || d > 18.0) { return 0.0; }
    let yr = p.y - dn.z;
    if (yr < -3.0 || yr > 4.0) { return 0.0; }
    let s = -p.z;
    let dd = max(d, 0.0);
    let sheet = exp(-dd / 3.5) * exp(-sq((yr - 0.08 * dd + 0.1) / (0.25 + 0.06 * dd))) * smoothstep(-1.0, 0.2, d);
    if (sheet < 0.01) { return 0.0; }
    let gust = smoothstep(0.42, 0.78, noise_value2(vec2f(s * 0.018 - t * 0.07, dn.w * 3.3 + 0.5)));
    let streak = noise_value2(vec2f(s * 0.22 + dn.w * 17.0 + dd * 0.08, dd * 0.16 - t * 1.3));
    return sheet * gust * smoothstep(0.25, 0.85, streak);
}

// heightfield march with a secant finish; (t, plume density along the way)
fn march(ro: vec3f, rd: vec3f, n: i32, jit: f32, time: f32) -> vec2f {
    var t = 0.5 + jit * 0.5;
    var tp = t;
    var hp = 1.0;
    var dens = 0.0;
    for (var i = 0; i < 240; i++) {
        if (i >= n) { break; }
        let p = ro + rd * t;
        if (p.y > H_MAX && rd.y >= 0.0) { return vec2f(-1.0, dens); }
        let dn = dune(p.xz);
        let h = p.y - dn.x;
        if (h < 0.0) {
            return vec2f(tp + (t - tp) * hp / max(hp - h, 1e-4), dens);
        }
        var dt = max(h * 0.72, 0.05 + 0.004 * t);
        // inside a plume the steps shorten so the sheet is sampled
        if (t > 14.0 && t < 450.0) {
            let pd = plume(p, dn, time);
            if (pd > 0.0) {
                dt = min(dt, 0.6 + t * 0.006);
                dens += pd * min(dt, 3.0) * (1.0 - smoothstep(250.0, 450.0, t));
            }
        }
        tp = t;
        hp = h;
        t += dt;
        if (t > 7000.0) { break; }
    }
    return vec2f(-1.0, dens);
}

// low sun, long shadows: a coarse march toward the light
fn sun_shadow(p: vec3f, l: vec3f) -> f32 {
    var res = 1.0;
    var t = 1.0;
    for (var i = 0; i < 18; i++) {
        let q = p + l * t;
        if (q.y > H_MAX) { break; }
        let h = q.y - dune_h(q.xz);
        res = min(res, 9.0 * h / t);
        if (res < 0.01) { break; }
        t += clamp(h * 0.9, 1.5 + t * 0.12, 90.0);
        if (t > 800.0) { break; }
    }
    return smoothstep(0.0, 1.0, res);
}

// ------------------------------------------------------------------ caravan

// A handler and six camels cross our ridge 56 m ahead, left to right, each
// walking the span X0..X1 and fading in and out at its ends.
const CZ: f32 = -56.0;
const CX0: f32 = -64.0;
const CX1: f32 = -3.0;

fn fig_x(k: i32, t: f32) -> f32 { return CX0 + fmod_pos(t * 0.7 - f32(k) * 3.6, CX1 - CX0); }
fn fig_fade(x: f32) -> f32 { return smoothstep(CX0, CX0 + 8.0, x) * sstep(CX1, CX1 - 3.0, x); }

// a dromedary in side view, walking toward +u; feet at v = 0 (metres)
fn camel(q: vec2f, ph: f32, rider: bool) -> f32 {
    var d = (length((q - vec2f(0.0, 1.55)) / vec2f(0.82, 0.4)) - 1.0) * 0.4;
    d = min(d, length(q - vec2f(-0.05, 1.92)) - 0.34);                       // hump
    d = min(d, sdf2_segment(q, vec2f(0.62, 1.55), vec2f(1.05, 1.95)) - 0.13); // neck
    d = min(d, length((q - vec2f(1.24, 1.98)) / vec2f(0.27, 0.12)) * 0.12 - 0.12); // head
    // legs: diagonal pairs swing in opposite phase
    let sw = 0.22 * sin(ph);
    d = min(d, sdf2_segment(q, vec2f(0.5, 1.3), vec2f(0.5 + sw, 0.0)) - 0.07);
    d = min(d, sdf2_segment(q, vec2f(0.38, 1.3), vec2f(0.38 - sw, 0.0)) - 0.07);
    d = min(d, sdf2_segment(q, vec2f(-0.52, 1.3), vec2f(-0.52 - sw, 0.0)) - 0.07);
    d = min(d, sdf2_segment(q, vec2f(-0.64, 1.3), vec2f(-0.64 + sw, 0.0)) - 0.07);
    if (rider) {
        d = min(d, sdf2_segment(q, vec2f(0.1, 2.15), vec2f(0.1, 2.6)) - 0.17);
        d = min(d, length(q - vec2f(0.12, 2.82)) - 0.13);
    }
    return d;
}

fn walker(q: vec2f, ph: f32) -> f32 {
    let sw = 0.2 * sin(ph);
    var d = sdf2_segment(q, vec2f(0.0, 0.85), vec2f(0.02, 1.45)) - 0.2;
    d = min(d, length(q - vec2f(0.04, 1.66)) - 0.12);
    d = min(d, sdf2_segment(q, vec2f(0.0, 0.85), vec2f(sw, 0.0)) - 0.07);
    d = min(d, sdf2_segment(q, vec2f(0.0, 0.85), vec2f(-sw, 0.0)) - 0.07);
    return d;
}

fn figure(k: i32, q: vec2f, t: f32) -> f32 {
    let ph = t * 5.0 + f32(k) * 1.9;
    if (k == 0) { return walker(q, ph); }
    return camel(q, ph, k == 1 || k == 4);
}

// (coverage, depth) of the caravan at screen point p
fn caravan(p: vec2f, cam: Cam, t: f32, pxs: f32) -> vec2f {
    // cheap screen box around the whole walk first
    let a = cam_project(cam, vec3f(CX0 - 3.0, 14.0, CZ));
    let b = cam_project(cam, vec3f(CX1 + 3.0, 54.0, CZ));
    if (p.x < a.x || p.x > b.x || p.y < a.y || p.y > b.y) { return vec2f(0.0, 1e9); }
    var cov = 0.0;
    var depth = 1e9;
    for (var k = 0; k < 7; k++) {
        let x = fig_x(k, t);
        let fade = fig_fade(x);
        if (fade <= 0.0) { continue; }
        // horizontal cull before paying for the terrain height
        let c0 = cam_project(cam, vec3f(x, 34.0, CZ));
        let m0 = c0.z / cam.zoom;
        if (abs(p.x - c0.x) * m0 > 2.0) { continue; }
        let pr = cam_project(cam, vec3f(x, dune_h(vec2f(x, CZ)), CZ));
        let m = pr.z / cam.zoom; // metres per p unit at that depth
        let q = (p - pr.xy) * m;
        if (q.y < -0.4 || q.y > 3.3) { continue; }
        let al = saturate(0.5 - figure(k, q, t) / (pxs * m)) * fade;
        if (al > cov) {
            cov = al;
            depth = pr.z;
        }
    }
    return vec2f(cov, depth);
}

// ------------------------------------------------------------------ sky

fn day_sky(rd: vec3f, l: Look) -> vec3f {
    var c = sky_atmosphere_haze(rd, l.sun, l.haze) * ozone_t(l.sun.y);
    c += l.hz * exp(-max(rd.y, 0.0) * 14.0);
    return c;
}

fn sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    if (l.night > 0.5) {
        var c = sky_night(rd) * 1.3;
        c += vec3f(0.012, 0.016, 0.026) * pow(saturate(dot(rd, l.sun)), 6.0);
        c += star_field(rd, 0.55, ctx) * 0.6 * smoothstep(0.0, 0.12, rd.y);
        c += star_milky_way(rd, normalize(vec3f(0.3, 0.55, 0.78)), normalize(vec3f(-0.4, 0.3, -0.87)), ctx) * 0.7;
        c += sky_moon(rd, l.sun, 0.62, 1.1);
        return c;
    }
    return day_sky(rd, l) + sky_sun_disk(rd, l.sun, 0.53);
}

fn horizon_col(rd: vec3f, l: Look) -> vec3f {
    if (l.night > 0.5) { return sky_night(vec3f(rd.x, 0.02, rd.z)) * 1.3; }
    return day_sky(normalize(vec3f(rd.x, 0.02, rd.z)), l);
}

// ------------------------------------------------------------------ shading

fn shade_sand(p: vec3f, rd: vec3f, t: f32, pxa: f32, l: Look, ctx: Ctx) -> vec3f {
    let e = max(0.25, t * pxa * 0.8);
    let dn = dune(p.xz);
    let hx = dune_h(p.xz + vec2f(e, 0.0)) - dune_h(p.xz - vec2f(e, 0.0));
    let hz = dune_h(p.xz + vec2f(0.0, e)) - dune_h(p.xz - vec2f(0.0, e));
    var n = normalize(vec3f(-hx, 2.0 * e, -hz));
    // wind ripples on everything but the slip faces, faded with the pixel
    // footprint so they never alias into noise
    let fp = t * pxa;
    let lee = smoothstep(0.0, 1.5, dn.y) * smoothstep(0.3, 0.55, 1.0 - n.y);
    let rip_a = (1.0 - lee) * saturate(1.0 - fp / 0.25);
    if (rip_a > 0.0) {
        let w = p.x + 2.2 * noise_value2(p.xz * vec2f(0.18, 0.06)) + 0.6 * noise_value2(p.xz * 0.9);
        let ph = w * (TAU / 0.75);
        n = normalize(n + vec3f(0.14 * rip_a * cos(ph), 0.0, 0.03 * rip_a * sin(ph * 0.5)));
    }
    var alb = l.sand * (0.92 + 0.16 * noise_value2(p.xz * 0.013));
    // moonlight is seen without colour
    if (l.night > 0.5) { alb = col_saturation(alb, 0.35); }
    // the slip faces are finer, slightly darker sand; the corridors paler
    alb *= mix(1.0, 0.9, lee);
    alb = mix(alb, alb * vec3f(1.04, 1.02, 0.98), sstep(8.0, 2.0, p.y));
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    if (dif > 0.0) {
        sh = sun_shadow(p + vec3f(0.0, 0.15, 0.0), l.sun);
    }
    // sand is a rough, bright scatterer: a little wrap keeps the terminator soft
    let wrap = saturate((dot(n, l.sun) + 0.15) / 1.15);
    let sky_occ = 0.6 + 0.4 * n.y;
    // light bounced from the sunlit slopes into the shadows
    let bounce = l.sand * l.sun_c * max(l.sun.y, 0.1) * 0.12 * (1.0 - n.y * 0.5);
    return alb * (l.sun_c * mix(dif, wrap, 0.3) * sh + l.amb * sky_occ + bounce);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let h0 = dune_h(vec2f(0.0, 0.0));
    let ro = vec3f(0.0, h0 + 1.8, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(-0.04, -0.105, -1.0), 0.0, 32.0);
    let rd = cam_ray(cam, p);
    let pxa = ctx.px / cam.zoom;
    let m = march(ro, rd, steps(150.0, ctx), ctx.jitter, ctx.t);
    var col: vec3f;
    var tt = 1e9;
    if (m.x > 0.0) {
        tt = m.x;
        let hp = ro + rd * m.x;
        col = shade_sand(hp, rd, m.x, pxa, l, ctx);
        let k = 1.0 - exp(-m.x * l.fogd);
        col = mix(col, horizon_col(rd, l), k);
    } else {
        col = sky(rd, l, ctx);
    }
    // blowing sand: bright where it is backlit by the sun
    if (m.y > 0.0) {
        // seen edge-on a sheet piles up: cap what one ray can gather
        let a = 1.0 - exp(-min(m.y, 0.7) * 1.4);
        let fwd = pow(saturate(dot(rd, l.sun) * 0.5 + 0.5), 3.0);
        var e = l.sand * (l.sun_c * (0.4 + 1.4 * fwd) + l.amb * 0.9);
        if (l.night > 0.5) { e = col_saturation(e, 0.35); }
        col = mix(col, e, a * 0.75);
    }
    // the caravan, dark against the light
    let cv = caravan(p, cam, ctx.t, ctx.px);
    if (cv.x > 0.0 && tt > cv.y - 2.0) {
        var fig = vec3f(0.035, 0.022, 0.016) * (l.sun_c * 0.15 + l.amb * 0.5);
        if (l.night < 0.5 && l.sun.y > 0.5) { fig = col_hex(0x6b4a33u) * (l.sun_c * 0.35 + l.amb * 0.5); }
        col = mix(col, fig, cv.x);
    }
    return col * l.wb * exp2(l.exposure);
}
