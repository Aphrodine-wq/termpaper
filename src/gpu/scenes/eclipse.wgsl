//! name: eclipse
//! title: Total Solar Eclipse
//! category: weather
//! tags: eclipse, corona, totality, diamond ring, sky
//! desc: totality over open country: the corona, the diamond ring, a sunset on every horizon
//! themes: totality, desert, mountain
//! uses: camera, sdf, sky, stars
//! cost: light
//! fallback: starfield
//! credits: original

// The Sun is drawn at about seven times its true size, as eclipse
// photographers composite a telephoto frame over the wide one: at true size
// it would be a pixel on a terminal. Everything else is to scale in angle.
//
// A 120 s loop: the Moon slides in from the left (partial phase, the day
// dimming), Baily's beads and the diamond ring at second contact, 73 s of
// totality, the diamond ring again, and out to the right. At both ends of
// the loop the Moon is clear of the Sun (a new moon is invisible), so the
// loop is seamless. Sky and land brightness follow the uncovered area of
// the photosphere.

const RS: f32 = 0.0314;        // solar angular radius (1.8 degrees)
const RM: f32 = 1.03;          // lunar radius / solar radius
const LOOP: f32 = 120.0;

struct Look {
    sun: vec3f,
    sun_c: vec3f,
    amb: vec3f,
    land: u32,        // 0 prairie, 1 desert mesas, 2 mountains and a lake
    exposure: f32,
}

fn look(theme: u32) -> Look {
    let sun = sky_sun_dir(26.0, 8.0);
    switch (theme) {
        case 1u: { return Look(sun, vec3f(2.79, 2.28, 1.89), vec3f(0.247, 0.405, 0.614), 1u, 0.0); }
        case 2u: { return Look(sun, vec3f(2.79, 2.28, 1.89), vec3f(0.247, 0.405, 0.614), 2u, 0.0); }
        default: { return Look(sun, vec3f(2.79, 2.28, 1.89), vec3f(0.247, 0.405, 0.614), 0u, 0.0); }
    }
}

// ------------------------------------------------------------------ eclipse

// Moon centre relative to the Sun, in solar radii, over the loop
fn moon_x(t: f32) -> f32 {
    let u = fmod_pos(t, LOOP);
    if (u < 22.0) { return mix(-2.2, -0.045, 1.0 - sq(1.0 - u / 22.0)); }
    if (u < 25.0) { return mix(-0.045, -0.025, (u - 22.0) / 3.0); }
    if (u < 95.0) { return mix(-0.025, 0.025, (u - 25.0) / 70.0); }
    if (u < 98.0) { return mix(0.025, 0.045, (u - 95.0) / 3.0); }
    return mix(0.045, 2.2, sq((u - 98.0) / 22.0));
}

// fraction of the solar disc left uncovered by the Moon at separation d
fn uncovered(d: f32) -> f32 {
    let r1 = 1.0;
    let r2 = RM;
    if (d >= r1 + r2) { return 1.0; }
    if (d <= r2 - r1) { return 0.0; }
    let a = r1 * r1 * acos(clamp((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1), -1.0, 1.0));
    let b = r2 * r2 * acos(clamp((d * d + r2 * r2 - r1 * r1) / (2.0 * d * r2), -1.0, 1.0));
    let c = 0.5 * sqrt(max((-d + r1 + r2) * (d + r1 - r2) * (d - r1 + r2) * (d + r1 + r2), 0.0));
    return 1.0 - (a + b - c) / PI;
}

struct Ecl {
    m: vec2f,         // moon centre (solar radii)
    vis: f32,         // uncovered photosphere fraction
    day: f32,         // perceived daylight 0 (totality) .. 1
    corona: f32,      // how much of the corona shows through the glare
}

fn eclipse(t: f32) -> Ecl {
    var e: Ecl;
    e.m = vec2f(moon_x(t), 0.012);
    e.vis = uncovered(length(e.m));
    // the eye adapts: a 1% sliver still looks like dim daylight
    e.day = pow(e.vis, 0.45);
    e.corona = sstep(0.004, 0.0, e.vis);
    return e;
}

// sky-plane coordinates around the Sun, in solar radii
fn sun_frame(rd: vec3f, sun: vec3f) -> vec2f {
    let e1 = normalize(cross(sun, vec3f(0.0, 1.0, 0.0)));
    let e2 = cross(e1, sun);
    let z = max(dot(rd, sun), 1e-4);
    return vec2f(dot(rd, e1), dot(rd, e2)) / z / RS;
}

// the corona: a steep radial fall-off, streamers along the solar equator,
// thin plumes at the poles, fine rays everywhere
fn corona(q: vec2f) -> f32 {
    let r = length(q);
    if (r < 0.95) { return 0.0; }
    let th = atan2(q.y, q.x);
    let tilt = th - 0.35;
    let equator = pow(abs(cos(tilt)), 3.0);
    let streamers = 0.55 * exp(-sq(sin(tilt - 0.15) / 0.42)) + 0.45 * exp(-sq(sin(tilt + 2.7) / 0.5)) +
                    0.25 * exp(-sq(sin(tilt + 1.2) / 0.25));
    let plumes = (1.0 - equator) * (0.5 + 0.5 * sin(th * 38.0));
    let rays = 0.75 + 0.5 * noise_value2(vec2f(th * 24.0, r * 0.6)) * noise_value2(vec2f(th * 70.0, 1.3));
    // K corona: bright and smooth close in; streamers carry it far out
    let inner = pow(r, -6.0) * 1.4;
    let fall = pow(r, -(3.2 - 1.3 * streamers)) * (0.35 + 0.5 * equator + 0.9 * streamers) + 0.3 * pow(r, -4.0) * plumes;
    return inner + fall * rays;
}

// ------------------------------------------------------------------ land

// skyline elevation (radians above the horizon) at azimuth az for the theme
fn skyline(az: f32, l: Look) -> vec2f {
    // (near, far) layers
    switch (l.land) {
        case 1u: {
            // desert: flat-topped mesas and buttes far off, a low near rise
            let n = noise_fbm2(vec2f(az * 4.0 + 11.0, 2.0), 4);
            let mesa = 0.05 * smoothstep(0.5, 0.52, n) + 0.018 * smoothstep(0.32, 0.5, n);
            let near = 0.012 + 0.008 * noise_fbm2(vec2f(az * 3.0, 5.0), 3);
            return vec2f(near, mesa + 0.004);
        }
        case 2u: {
            // mountains: a jagged range standing straight out of the plain
            let rn = noise_ridged2(vec2f(az * 3.2 + 4.0, 1.0), 5);
            let range = 0.15 * rn * rn * smoothstep(-0.9, -0.4, az) * sstep(0.9, 0.3, az) + 0.012;
            return vec2f(0.004, range);
        }
        default: {
            // prairie: long swells of grassland
            let near = 0.018 + 0.012 * noise_fbm2(vec2f(az * 2.0 + 3.0, 1.0), 3);
            let far = 0.008 + 0.006 * noise_fbm2(vec2f(az * 5.0, 9.0), 3);
            return vec2f(near, far);
        }
    }
}

// a farm windpump on the prairie rise, in (azimuth, elevation) radians
// relative to its foot; its wheel turns slowly in the breeze
fn windpump(q: vec2f, t: f32) -> f32 {
    let h = 0.052;
    // lattice tower: tapered legs and a few cross braces
    let w = 0.0045 * (1.0 - q.y / h) + 0.0008;
    var d = max(abs(abs(q.x) - w) - 0.0004, max(-q.y, q.y - h));
    let brace = abs(fract(q.y / 0.012) - 0.5) * 0.012;
    d = min(d, max(max(brace - 0.0004, abs(q.x) - w), max(-q.y, q.y - h)));
    // the wheel of many blades, and its tail vane
    let c = q - vec2f(0.0008, h + 0.001);
    let r = length(c);
    let blades = abs(fract(atan2(c.y, c.x) / TAU * 18.0 + t * 0.15) - 0.5);
    d = min(d, max(abs(r - 0.0075) - 0.0035, blades * 0.004 - 0.0006));
    d = min(d, max(abs(r - 0.011) - 0.0004, -1.0));
    d = min(d, sdf2_box(q - vec2f(-0.012, h + 0.001), vec2f(0.006, 0.0004)));
    d = min(d, sdf2_box(q - vec2f(-0.018, h + 0.001), vec2f(0.0015, 0.003)));
    return d;
}

// ------------------------------------------------------------------ sky

fn sky(rd: vec3f, l: Look, e: Ecl, ctx: Ctx) -> vec3f {
    let y = max(rd.y, 0.0);
    // daylight, dimmed by how much Sun is left
    let day = sky_atmosphere_haze(rd, l.sun, 1.0) * e.day;
    // totality: an indigo dome over a sunset on every horizon, lit from
    // outside the Moon's shadow
    var tot = mix(vec3f(0.018, 0.022, 0.05), vec3f(0.004, 0.006, 0.02), pow(y, 0.4));
    tot += vec3f(0.3, 0.13, 0.025) * exp(-y * 22.0) + vec3f(0.06, 0.035, 0.04) * exp(-y * 6.0);
    var c = day + tot * (1.0 - e.day);
    // the Sun, the Moon and the corona
    let q = sun_frame(rd, l.sun);
    let r = length(q);
    let dm = length(q - e.m);
    if (r < 14.0 && dot(rd, l.sun) > 0.0) {
        let in_moon = sstep(RM + 0.02, RM - 0.02, dm);
        // photosphere with limb darkening
        let mu = sqrt(max(1.0 - r * r, 0.0));
        let disc = sstep(1.02, 0.98, r) * (0.4 + 0.6 * mu);
        c += vec3f(1.0, 0.96, 0.9) * 120.0 * disc * (1.0 - in_moon);
        // glare around whatever photosphere is left
        c += vec3f(1.0, 0.95, 0.85) * (e.vis * 5.0 + 0.5 * sqrt(e.vis)) * exp(-r * 2.6) * (1.0 - in_moon * 0.9);
        // the corona, once the glare is gone
        let cor = corona(q) * (1.0 - in_moon);
        c += vec3f(0.85, 0.9, 1.0) * cor * 0.9 * e.corona;
        // prominences: pink tongues just beyond the limb
        if (e.corona > 0.0) {
            let th = atan2(q.y, q.x);
            var pr = 0.0;
            pr += exp(-sq((th - 2.1) / 0.05)) * sstep(1.09, 1.0, r);
            pr += exp(-sq((th + 0.6) / 0.03)) * sstep(1.06, 1.0, r);
            pr += exp(-sq((th - 0.4) / 0.08)) * sstep(1.05, 1.0, r) * 0.6;
            c += vec3f(1.0, 0.2, 0.35) * pr * 1.2 * smoothstep(1.0, 1.02, r) * (1.0 - in_moon) * e.corona;
        }
        // diamond ring and Baily's beads at the contacts: the last light
        // breaking through lunar valleys at the limb
        let ring = smoothstep(0.0, 0.0008, e.vis) * sstep(0.03, 0.002, e.vis);
        if (ring > 0.0) {
            let dir = -normalize(e.m);
            let bp = dir * 1.0;
            let dd = length(q - bp);
            let th = atan2(q.y, q.x) - atan2(dir.y, dir.x);
            let beads = smoothstep(0.55, 0.8, noise_value2(vec2f(th * 40.0, 3.0))) * exp(-sq(sin(th) / 0.25)) * exp(-sq((r - 1.0) / 0.03));
            let spikes = exp(-abs(q.x - bp.x) * 30.0) * exp(-abs(q.y - bp.y) * 2.0) + exp(-abs(q.y - bp.y) * 30.0) * exp(-abs(q.x - bp.x) * 2.0);
            c += vec3f(1.0, 0.97, 0.92) * ring * (60.0 * exp(-dd * dd / 0.01) + 9.0 * exp(-dd * 1.6) + 3.0 * spikes + 20.0 * beads);
        }
        // the Moon's face, faintly lit by the Earth
        // (in daylight the Moon is lost in the bright sky in front of it)
        c = mix(c, vec3f(0.002, 0.0022, 0.0026) + tot * 0.05, in_moon * sq(1.0 - e.day));
    }
    // Venus and Jupiter, and a few bright stars, come out in totality
    let dark = 1.0 - e.day;
    if (dark > 0.05) {
        let venus = normalize(vec3f(-0.52, 0.42, -0.74));
        let jup = normalize(vec3f(0.56, 0.62, -0.55));
        let pw = max(ctx.px * 0.7, 0.0012);
        c += vec3f(1.0, 0.97, 0.9) * 3.0 * exp(-sq(acos(clamp(dot(rd, venus), -1.0, 1.0)) / pw)) * dark;
        c += vec3f(1.0, 0.92, 0.8) * 1.5 * exp(-sq(acos(clamp(dot(rd, jup), -1.0, 1.0)) / pw)) * dark;
        c += star_field(rd, 0.035, ctx) * 0.5 * dark * smoothstep(0.1, 0.4, y);
    }
    return c;
}

// colour of the far skyline layer: lit and hazy by day, a dark silhouette
// in the Moon's shadow (it lies in the umbra too)
fn far_layer(rd: vec3f, ang: f32, l: Look, e: Ecl, ctx: Ctx, sunlit: vec3f, glow: vec3f) -> vec3f {
    var alb = vec3f(0.22, 0.25, 0.3);
    if (l.land == 1u) { alb = col_hex(0xa0583au); }
    if (l.land == 2u) {
        // snow on the high peaks
        alb = mix(col_hex(0x4a5260u), vec3f(0.85, 0.88, 0.92), smoothstep(0.05, 0.1, ang));
    }
    let fc = alb * (sunlit * 0.5 + l.amb * e.day * 0.6) + glow * 0.12 * alb;
    let hz = sky(normalize(vec3f(rd.x, 0.01, rd.z)), l, e, ctx);
    return mix(fc, hz, 0.45 * e.day + 0.08);
}

// ------------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let e = eclipse(ctx.t);
    let ro = vec3f(0.0, 2.0, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.08, 0.26, -1.0), 0.0, 50.0);
    let rd = cam_ray(cam, p);
    let pxa = ctx.px / cam.zoom;
    var col = sky(rd, l, e, ctx);
    // the land: layered silhouettes, lit by the day and, in totality, only
    // by the glow on the horizon
    let az = atan2(rd.x, -rd.z);
    let sl = skyline(az, l);
    let ang = asin(clamp(rd.y, -1.0, 1.0));
    let horizon_glow = vec3f(0.1, 0.055, 0.025) * (1.0 - e.day);
    let sunlit = l.sun_c * saturate(l.sun.y) * e.day;
    // far layer
    let cf = saturate((sl.y - ang) / pxa + 0.5);
    if (cf > 0.0) {
        col = mix(col, far_layer(rd, ang, l, e, ctx, sunlit, horizon_glow), cf);
    }
    // near land and the ground
    let cn = saturate((sl.x - ang) / pxa + 0.5);
    if (cn > 0.0) {
        var alb = col_hex(0x8a8a50u);
        if (l.land == 1u) { alb = col_hex(0xa86a44u); }
        if (l.land == 2u) { alb = col_hex(0x3c4a30u); }
        // the ground below: grass, sand or a lake reflecting the sky
        let tex = noise_value2(vec2f(az * 60.0, ang * 300.0));
        var gc = alb * (0.8 + 0.4 * tex) * (sunlit * 0.45 + l.amb * e.day * 0.5) + horizon_glow * 0.25 * alb;
        if (l.land == 2u && ang < -0.02) {
            // an alpine lake: the range and the sky mirrored
            let rr = normalize(vec3f(rd.x, -rd.y, rd.z));
            var refl = sky(rr, l, e, ctx) * 0.8;
            let rsl = skyline(az, l);
            let rang = asin(clamp(rr.y, -1.0, 1.0));
            let rcf = saturate((rsl.y - rang) / pxa + 0.5);
            refl = mix(refl, far_layer(rr, rang, l, e, ctx, sunlit, horizon_glow) * 0.8, rcf);
            let ripple = 0.9 + 0.1 * noise_value2(vec2f(az * 80.0, rd.y * 400.0 + ctx.t * 0.3));
            gc = mix(vec3f(0.01, 0.015, 0.02) * e.day, refl * ripple, 0.85) * sstep(-0.02, -0.03, ang);
            gc += alb * (sunlit * 0.4) * smoothstep(-0.03, -0.02, ang);
        }
        col = mix(col, gc, cn);
    }
    if (l.land == 0u) {
        let az0 = 0.36;
        let foot = skyline(az0, l).x - 0.002;
        let wd = windpump(vec2f(az - az0, ang - foot), ctx.t);
        let wc = saturate(0.5 - wd / pxa);
        col = mix(col, vec3f(0.05, 0.05, 0.05) * (sunlit * 0.3 + l.amb * e.day * 0.4) + horizon_glow * 0.02, wc);
    }
    return col * exp2(l.exposure);
}
