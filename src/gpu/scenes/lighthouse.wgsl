//! name: lighthouse
//! title: Lighthouse in the Storm
//! category: coast
//! tags: lighthouse, storm, maine, rain, night, beam, fog
//! desc: a Maine lighthouse on a granite headland, its beam sweeping through rain, spray and storm
//! themes: storm, fog, dusk
//! uses: camera, sdf, sky, stars, water, rain
//! cost: heavy
//! fallback: ocean
//! credits: original

// World units are metres; y up. The camera stands on shore rocks 6 m above
// the sea looking out along the coast. A granite headland runs in from the
// right; the lighthouse stands near its tip with the keeper's house. The
// beam is a pair of rotating cones whose in-scattering is integrated in
// closed form along each view ray: the ray is clipped to the cone, and the
// point-light integral  ∫ ds / (d² + r²)  is an arctangent.

const CAM: vec3f = vec3f(0.0, 5.0, 0.0);
const TOWER: vec3f = vec3f(31.0, 12.0, -122.0);   // tower base (sunk into the rock)
const LAMP: vec3f = vec3f(31.0, 38.6, -122.0);
const HOUSE: vec3f = vec3f(44.0, 14.2, -119.0);
const HEAD_A: vec2f = vec2f(16.0, -118.0);        // headland spine: tip ...
const HEAD_B: vec2f = vec2f(170.0, -165.0);       // ... to the mainland
const ROT: f32 = 0.52;                            // beam rotation, rad/s (12 s)

struct Look {
    amb: vec3f,      // ambient light from the sky
    zen: vec3f,      // sky overhead
    hor: vec3f,      // sky / air at the horizon
    fog: f32,        // extinction per metre (rain, spray, sea fog)
    scat: f32,       // beam in-scattering strength
    rain: f32,
    storm: f32,      // lightning, spray, racing clouds
    chop: f32,       // sea state
    dusk: f32,       // clear blue-hour sky with stars
    exposure: f32,
    flash: f32,      // lightning brightness now
    bolt: vec2f,     // (azimuth, elevation) of the strike
}

fn look(theme: u32, ctx: Ctx) -> Look {
    var l: Look;
    l.flash = 0.0;
    l.bolt = vec2f(0.0);
    switch (theme) {
        case 1u: {
            l.amb = vec3f(0.006, 0.007, 0.009);
            l.zen = vec3f(0.003, 0.0035, 0.0045);
            l.hor = vec3f(0.011, 0.012, 0.014);
            l.fog = 0.011; l.scat = 1.6; l.rain = 0.0; l.storm = 0.0; l.chop = 0.3; l.dusk = 0.0;
            l.exposure = 0.6;
        }
        case 2u: {
            l.amb = vec3f(0.018, 0.028, 0.06);
            l.zen = vec3f(0.006, 0.014, 0.05);
            l.hor = vec3f(0.05, 0.06, 0.1);
            l.fog = 0.0005; l.scat = 0.08; l.rain = 0.0; l.storm = 0.0; l.chop = 0.4; l.dusk = 1.0;
            l.exposure = 0.3;
        }
        default: {
            l.amb = vec3f(0.006, 0.0075, 0.01);
            l.zen = vec3f(0.0025, 0.003, 0.0042);
            l.hor = vec3f(0.012, 0.014, 0.018);
            l.fog = 0.004; l.scat = 0.7; l.rain = 1.0; l.storm = 1.0; l.chop = 1.25; l.dusk = 0.0;
            l.exposure = 0.4;
            // rare lightning: a slot every 6 s, one in four strikes, a
            // double flicker lasting under half a second
            let ev = hash_event(ctx.t, 6.0, 0x1f7u);
            if (ev.x < 0.26) {
                let tt = ev.y * 6.0;
                let f = exp(-tt * 9.0) + 0.7 * exp(-max(tt - 0.16, 0.0) * 12.0) * step(0.16, tt);
                l.flash = f * step(tt, 0.6);
                let h = hash_f(bitcast<u32>(i32(ev.z)) ^ 0x51u);
                l.bolt = vec2f(-0.9 + 1.5 * h, 0.12 + 0.1 * hash_f(bitcast<u32>(i32(ev.z)) ^ 0x77u));
            }
        }
    }
    return l;
}

// ------------------------------------------------------------ land

fn seg_dist(p: vec2f, a: vec2f, b: vec2f) -> vec2f {
    let pa = p - a;
    let ba = b - a;
    let h = saturate(dot(pa, ba) / dot(ba, ba));
    return vec2f(length(pa - ba * h), h);
}

// granite: the headland (a terraced whaleback of tilted strata) and the
// shore rocks in the right foreground
fn land_h(xz: vec2f) -> f32 {
    let sd = seg_dist(xz, HEAD_A, HEAD_B);
    let n = noise_fbm2(xz * 0.035, 4);
    let w = 26.0 + 30.0 * sd.y + 12.0 * n;
    // a granite whaleback: plateau, then broken ledges down to the sea
    let edge = saturate((w - sd.x) / 22.0);
    var h = 17.0 * pow(edge, 0.6) * smoothstep(-0.1, 0.06, sd.y) + 4.0 * sd.y;
    let q = h / 2.6 + n * 1.1;
    let hq = (floor(q) + smoothstep(0.35, 1.0, fract(q))) * 2.6 - n * 2.2 + 1.5 * noise_value2(xz * 0.2);
    h = mix(h - 6.0, hq, smoothstep(0.0, 0.25, edge));
    // shore rocks in the left foreground
    let f = (xz - vec2f(-16.0, -30.0)) / vec2f(22.0, 12.0);
    h = max(h, 4.2 * (1.0 - dot(f, f)) + 1.5 * noise_fbm2(xz * 0.25, 3) - 0.6);
    return h - 3.0;
}

fn land_march(ro: vec3f, rd: vec3f, tmax: f32, ctx: Ctx) -> f32 {
    // the land lies at x > 0; skip rays that never go there
    if (rd.y > 0.12) { return -1.0; }
    var t = 8.0;
    var lh = 0.0;
    var ly = 0.0;
    for (var i = 0; i < 90; i++) {
        let p = ro + rd * t;
        let h = land_h(p.xz);
        let dy = p.y - h;
        if (dy < 0.0) {
            // refine between the last two samples
            return t - (t - lh) * (-dy) / max(ly - dy, 1e-4) * 1.0;
        }
        if (p.y > 22.0 && rd.y > 0.0) { break; }
        lh = t;
        ly = dy;
        t += clamp(dy * 0.45, 0.15 + t * 0.004, 12.0);
        if (t > tmax) { break; }
    }
    return -1.0;
}

fn land_normal(xz: vec2f, t: f32, ctx: Ctx) -> vec3f {
    let e = max(0.08, t * ctx.px * 0.7);
    let hx = land_h(xz + vec2f(e, 0.0)) - land_h(xz - vec2f(e, 0.0));
    let hz = land_h(xz + vec2f(0.0, e)) - land_h(xz - vec2f(0.0, e));
    return normalize(vec3f(-hx, 2.0 * e, -hz));
}

// ------------------------------------------------------------ lighthouse

fn gable(q: vec3f, b: vec3f, rise: f32) -> f32 {
    // box walls plus a roof ridge along x
    let walls = sdf_box(q, b);
    let r = q - vec3f(0.0, b.y, 0.0);
    let s = rise / b.z;
    let roof = max((abs(r.z) * s + r.y - rise) / sqrt(1.0 + s * s), max(-r.y, abs(r.x) - b.x - 0.3));
    return min(walls, roof);
}

// (distance, material): 1 white tower, 2 black iron, 3 lantern glass,
// 4 house walls, 5 house roof
fn lh_sdf(p: vec3f) -> vec2f {
    let q = p - TOWER;
    let tower = sdf_cone_y(q - vec3f(0.0, 12.5, 0.0), 12.5, 3.3, 2.25);
    var d = vec2f(tower, 1.0);
    let gallery = sdf_cyl_y(q - vec3f(0.0, 25.15, 0.0), 0.15, 3.05);
    let rail = max(abs(length(q.xz) - 2.95) - 0.04, abs(q.y - 25.75) - 0.55);
    d = op_umin(d, vec2f(min(gallery, rail), 2.0));
    let glass = sdf_cyl_y(q - vec3f(0.0, 26.6, 0.0), 1.3, 1.75);
    d = op_umin(d, vec2f(glass, 3.0));
    let dome = max(length(q - vec3f(0.0, 27.8, 0.0)) - 1.95, 27.85 - q.y);
    let vent = length(q - vec3f(0.0, 30.05, 0.0)) - 0.32;
    d = op_umin(d, vec2f(min(dome, vent), 2.0));
    let hq = p - HOUSE;
    d = op_umin(d, vec2f(sdf_box(hq, vec3f(5.0, 3.0, 3.6)), 4.0));
    let hr = hq - vec3f(0.0, 3.0, 0.0);
    let s = 3.2 / 3.9;
    let roof = max((abs(hr.z) * s + hr.y - 3.2) / sqrt(1.0 + s * s), max(-hr.y, abs(hr.x) - 5.4));
    d = op_umin(d, vec2f(roof, 5.0));
    // chimney
    d = op_umin(d, vec2f(sdf_box(hq - vec3f(2.8, 5.6, 0.8), vec3f(0.45, 1.4, 0.45)), 5.0));
    return d;
}

fn lh_march(ro: vec3f, rd: vec3f, tmax: f32) -> vec2f {
    // bounding box around tower and house
    let bmin = vec3f(TOWER.x - 4.0, TOWER.y - 2.0, TOWER.z - 5.0);
    let bmax = vec3f(HOUSE.x + 6.0, TOWER.y + 31.0, HOUSE.z + 4.5);
    let inv = 1.0 / rd;
    let a = (bmin - ro) * inv;
    let b = (bmax - ro) * inv;
    let t0 = max(max(min(a.x, b.x), min(a.y, b.y)), min(a.z, b.z));
    let t1 = min(min(max(a.x, b.x), max(a.y, b.y)), max(a.z, b.z));
    if (t1 <= max(t0, 0.0) || t0 > tmax) { return vec2f(-1.0); }
    var t = max(t0, 0.0);
    for (var i = 0; i < 64; i++) {
        let h = lh_sdf(ro + rd * t);
        if (h.x < 0.0015 * t) { return vec2f(t, h.y); }
        t += h.x;
        if (t > min(t1, tmax)) { break; }
    }
    return vec2f(-1.0);
}

fn lh_normal(p: vec3f, t: f32) -> vec3f {
    let e = max(0.01, t * 0.001);
    let k = vec2f(1.0, -1.0);
    return normalize(k.xyy * lh_sdf(p + k.xyy * e).x + k.yyx * lh_sdf(p + k.yyx * e).x +
                     k.yxy * lh_sdf(p + k.yxy * e).x + k.xxx * lh_sdf(p + k.xxx * e).x);
}

// ------------------------------------------------------------ the beam

fn beam_dir(k: f32, ctx: Ctx) -> vec3f {
    let a = ctx.t * ROT + k * PI + 0.8;
    return normalize(vec3f(cos(a), -0.03, sin(a)));
}

// how strongly a beam lights direction v (unit, from the lamp): a sharp
// core, a halo, and the lens' faint all-round glow
fn beam_profile(v: vec3f, b: vec3f) -> f32 {
    // Fresnel lenses spread light far more vertically than horizontally
    let hb = normalize(b.xz);
    let hv = normalize(v.xz);
    let ch = dot(hv, hb);
    let dh = acos(clamp(ch, -1.0, 1.0));
    let dv = asin(clamp(v.y, -1.0, 1.0)) - asin(clamp(b.y, -1.0, 1.0));
    let core = exp(-dh * dh / 0.0012 - dv * dv / 0.012);
    let halo = exp(-dh * dh / 0.012 - dv * dv / 0.05);
    return core + 0.15 * halo + 0.004;
}

// Clip ro + rd*s (s in [0, tmax]) to the forward cone with apex LAMP, axis
// b and cos half-angle ca: returns [lo, hi] (hi <= lo when missed).
fn cone_clip(ro: vec3f, rd: vec3f, tmax: f32, b: vec3f, ca: f32) -> vec2f {
    let v = ro - LAMP;
    let c2 = ca * ca;
    let db = dot(rd, b);
    let vb = dot(v, b);
    let qa = db * db - c2;
    let qb = 2.0 * (vb * db - c2 * dot(v, rd));
    let qc = vb * vb - c2 * dot(v, v);
    let disc = qb * qb - 4.0 * qa * qc;
    if (disc <= 0.0 || abs(qa) < 1e-7) { return vec2f(1.0, 0.0); }
    let sq_ = sqrt(disc);
    var s1 = (-qb - sq_) / (2.0 * qa);
    var s2 = (-qb + sq_) / (2.0 * qa);
    if (s1 > s2) { let tmp = s1; s1 = s2; s2 = tmp; }
    var lo = 0.0;
    var hi = tmax;
    if (qa < 0.0) {
        lo = max(lo, s1);
        hi = min(hi, s2);
        if (vb + db * (0.5 * (lo + hi)) < 0.0) { return vec2f(1.0, 0.0); }
    } else {
        if (vb + db * (s2 + 1.0) > 0.0) { lo = max(lo, s2); } else { hi = min(hi, s1); }
    }
    return vec2f(lo, hi);
}

// In-scattered beam light along a view ray. The ray is clipped to a wide
// cone, then integrated in the lamp's angle u: with s = s0 + h tan(u) the
// 1/d² falloff turns into du/h exactly, so a dozen samples of the beam's
// smooth angular profile (and its extinction) give a clean, band-free shaft.
fn beam_light(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx) -> vec3f {
    let v = ro - LAMP;
    let s0 = -dot(v, rd);
    let h = sqrt(max(dot(v, v) - s0 * s0, 0.0) + 1.0);
    var acc = 0.0;
    for (var k = 0; k < 2; k++) {
        let b = beam_dir(f32(k), ctx);
        let iv = cone_clip(ro, rd, tmax, b, cos(0.2));
        if (iv.y <= iv.x) { continue; }
        let u0 = atan((iv.x - s0) / h);
        let u1 = atan((iv.y - s0) / h);
        let fw = dot(b, -rd);
        let ph = 0.3 + 0.8 * pow(saturate(fw), 8.0) + 0.3 * saturate(fw);
        var sum = 0.0;
        let n = 14;
        for (var i = 0; i < 14; i++) {
            let u = mix(u0, u1, (f32(i) + 0.5) / f32(n));
            let sp = s0 + h * tan(u);
            let x = v + rd * sp;
            let d = length(x);
            let c = dot(x, b) / max(d, 1e-3);
            let th2 = max(1.0 - c * c, 0.0);
            let prof = exp(-th2 / 0.0007) + 0.1 * exp(-th2 / 0.006) + 0.012 * exp(-th2 / 0.025);
            // drifting rain / fog density makes the shaft ragged
            let wp = LAMP + x;
            let turb = noise_value3(wp * 0.045 + vec3f(ctx.t * 0.9, -ctx.t * 0.25 * l.storm, ctx.t * 0.3));
            sum += prof * exp(-l.fog * 0.6 * d) * (0.45 + 1.1 * turb);
        }
        acc += sum * (u1 - u0) / f32(n) / h * ph;
    }
    return col_kelvin(4200.0) * acc * l.scat * 450.0;
}

// ------------------------------------------------------------ sky

fn backdrop(rd: vec3f, l: Look, ctx: Ctx, stars: bool) -> vec3f {
    let y = max(rd.y, 0.0);
    if (l.dusk > 0.5) {
        // blue hour: afterglow low in the west (left), deep blue overhead
        let west = normalize(vec3f(-1.0, 0.0, -0.35));
        let g = pow(saturate(dot(normalize(vec3f(rd.x, 0.0, rd.z)), west) * 0.5 + 0.5), 3.0);
        var c = mix(vec3f(0.09, 0.075, 0.09), l.zen, pow(y, 0.4));
        c += vec3f(0.22, 0.09, 0.03) * g * exp(-y * 9.0);
        c += vec3f(0.04, 0.05, 0.09) * exp(-y * 3.0);
        if (stars) { c += star_field(rd, 0.5, ctx) * smoothstep(0.08, 0.4, y) * 0.5; }
        return c;
    }
    // storm / fog: a low deck of cloud racing in off the sea
    var c = mix(l.hor, l.zen, pow(y, 0.5));
    if (rd.y > 0.0) {
        let p1 = rd.xz / (rd.y + 0.02) * 420.0;
        let wind = vec2f(-16.0, 7.0) * ctx.t * l.storm + vec2f(-2.0, 0.5) * ctx.t;
        let d1 = noise_fbm2((p1 + wind) * 0.0021, 6);
        let p2 = rd.xz / (rd.y + 0.02) * 170.0;
        let d2 = noise_fbm2((p2 + wind * 1.7) * 0.005 + 3.0, 5);
        let scud = smoothstep(0.5, 0.75, d2) * l.storm;
        let fade = exp(-y * 1.5) * 0.6;
        // a hidden moon glows through the thinner parts of the deck and
        // silvers the cloud edges: that is what gives a night storm its shape
        let moon = normalize(vec3f(-0.36, 0.26, -0.9));
        let mg = pow(saturate(dot(rd, moon)), 6.0) * 0.8 + pow(saturate(dot(rd, moon)), 40.0) * 1.5;
        let thin = 1.0 - smoothstep(0.3, 0.72, d1);
        var cl = mix(l.zen * 0.5, l.hor * 1.2, thin * 0.7);
        cl += vec3f(0.03, 0.034, 0.042) * mg * (0.25 + thin * thin) * (1.0 - scud * 0.8);
        cl = mix(cl, l.zen * 0.3, scud);
        c = mix(cl, l.hor, saturate(fade + exp(-y * 12.0)));
        // lightning lights the cloud interiors around the strike
        if (l.flash > 0.0) {
            let az = atan2(rd.x, -rd.z);
            let dd = vec2f(az - l.bolt.x, (y - l.bolt.y) * 1.8);
            let glow = exp(-dot(dd, dd) * 6.0) * (0.4 + 0.9 * smoothstep(0.35, 0.8, d1)) + 0.12;
            c += vec3f(0.55, 0.6, 0.8) * l.flash * glow * 0.6;
        }
    }
    return c;
}

// the bolt itself: a jagged path from the cloud base down to the horizon
fn bolt(rd: vec3f, l: Look, ctx: Ctx) -> f32 {
    if (l.flash <= 0.01 || rd.y < -0.01) { return 0.0; }
    let az = atan2(rd.x, -rd.z);
    let y = rd.y;
    if (y > l.bolt.y) { return 0.0; }
    let seed = l.bolt.x * 37.0;
    let x = l.bolt.x + 0.012 * noise_sfbm2(vec2f(y * 40.0, seed), 4) + 0.02 * (y - l.bolt.y);
    let w = 0.0012 + ctx.px * 0.3;
    return exp(-sq((az - x) / w)) * l.flash * 2.5;
}

// ------------------------------------------------------------ sea

fn sea_h(xz: vec2f, t: f32, chop: f32, oct: i32) -> f32 {
    return water_height(xz * 0.55, t * 0.75, chop, oct) * 1.5;
}

fn sea_march(ro: vec3f, rd: vec3f, l: Look) -> f32 {
    if (rd.y >= -0.001) { return -1.0; }
    let hmax = 2.2 * l.chop + 0.2;
    var ta = (ro.y - hmax) / -rd.y;
    var tb = (ro.y + hmax) / -rd.y;
    let fa = (ro + rd * ta).y - sea_h((ro + rd * ta).xz, 0.0, l.chop, 1);
    // bisection on the heightfield
    for (var i = 0; i < 10; i++) {
        let tm = 0.5 * (ta + tb);
        let p = ro + rd * tm;
        let d = p.y - sea_h(p.xz, 0.0, l.chop, 4);
        if (d > 0.0) { ta = tm; } else { tb = tm; }
    }
    return 0.5 * (ta + tb) + fa * 0.0;
}

fn sea_shade(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let oct = clamp(i32(7.0 - log2(1.0 + t * ctx.px * 30.0)), 2, 7);
    let e = max(0.05, t * ctx.px * 0.6);
    let tt = ctx.t;
    let h0 = sea_h(p.xz, tt, l.chop, oct);
    let hx = sea_h(p.xz + vec2f(e, 0.0), tt, l.chop, oct) - h0;
    let hz = sea_h(p.xz + vec2f(0.0, e), tt, l.chop, oct) - h0;
    var n = normalize(vec3f(-hx, e, -hz));
    n = normalize(mix(n, vec3f(0.0, 1.0, 0.0), saturate(t * ctx.px * 0.8)));
    var r = reflect(rd, n);
    r.y = abs(r.y);
    let f = water_fresnel(dot(-rd, n));
    var refl = backdrop(r, l, ctx, false);
    let body = vec3f(0.006, 0.012, 0.012) * (l.amb * 30.0 + l.flash * 0.3);
    var c = mix(body, refl, f);
    // lamp glitter: the beam's own light on the waves
    let lv = LAMP - p;
    let ld = length(lv);
    let ln = lv / ld;
    let hv = normalize(ln - rd);
    let spec = pow(saturate(dot(n, hv)), 180.0) * f;
    let bi = beam_profile(-ln, beam_dir(0.0, ctx)) + beam_profile(-ln, beam_dir(1.0, ctx));
    c += col_kelvin(4200.0) * spec * bi * 9000.0 / (ld * ld + 100.0);
    // the lens' steady glow makes a thin broken column on the water
    let nh = saturate(dot(n, hv));
    c += col_kelvin(4200.0) * (pow(nh, 900.0) * 1.6 + pow(nh, 200.0) * 0.12) * f * (1.0 + 6.0 * l.scat * 0.0);
    // foam: wind-torn crests, streaks, and breakers boiling around the rocks
    let crest = smoothstep(0.35, 0.9, (h0 + 0.2) / (2.0 * l.chop + 0.2));
    let wq = rot2(0.4) * p.xz;
    let streak = noise_fbm2(wq * vec2f(0.05, 0.4) + vec2f(tt * 0.25, 0.0), 5);
    let caps = noise_fbm2(p.xz * 0.18 + vec2f(tt * 0.2, -tt * 0.1), 4);
    let lh = land_h(p.xz);
    let band = smoothstep(-2.0 - 4.0 * l.chop, -0.3, lh);
    let froth = noise_fbm2(p.xz * 0.35 + vec2f(tt * 0.6, tt * 0.25), 5) + 0.2 * sin(lh * 2.5 - tt * 1.6);
    let surf = band * smoothstep(0.42, 0.62, froth) * (0.35 + 0.65 * smoothstep(0.3, 1.0, l.chop));
    var foam = crest * smoothstep(0.5, 0.7, caps) * 1.4 + smoothstep(0.55, 0.72, streak) * 0.45 * crest;
    foam = saturate(foam * smoothstep(0.4, 1.0, l.chop) + surf * (0.5 + 0.5 * l.chop));
    let foam_c = (l.amb * 11.0 + vec3f(0.5, 0.55, 0.7) * l.flash * 0.3) * (0.7 + 0.5 * noise_value2(p.xz * 1.3 + tt * 0.4));
    c = mix(c, foam_c, foam * 0.85);
    return c;
}

// ------------------------------------------------------------ spray

// wave bursts on the rocks: closed-form events, ballistic plumes of spray
fn spray(p: vec2f, cam: Cam, tscene: f32, l: Look, ctx: Ctx) -> vec4f {
    var acc = vec4f(0.0);
    for (var k = 0; k < 5; k++) {
        let fk = f32(k);
        let per = 4.3 + fk * 0.7;
        let ev = hash_event(ctx.t + fk * 1.9, per, 0x5a7u + u32(k) * 31u);
        if (ev.x > 0.8) { continue; }
        let tau = ev.y * per;
        let life = 3.4;
        if (tau > life) { continue; }
        let hs = hash_f(bitcast<u32>(i32(ev.z)) ^ (u32(k) * 0x9e37u));
        // impact points along the headland's seaward face and the shore rocks
        var wp = vec3f(mix(8.0, 75.0, (fk + hs * 0.8) / 4.8), 1.0, 0.0);
        wp.z = -104.0 - 0.3 * (wp.x - 8.0) + 5.0 * hs;
        if (k == 4) { wp = vec3f(-10.0 + 8.0 * hs, 1.0, -22.0); }
        let pr = cam_project(cam, wp);
        if (pr.z <= 1.0 || pr.z > tscene + 12.0) { continue; }
        let mpp = pr.z / cam.zoom;          // metres per p unit
        let lp = (p - pr.xy) * mpp;         // local metres
        let v0 = 14.0 + 7.0 * hs;
        let top = max(v0 * tau - 4.9 * tau * tau, 0.0) + tau * 3.0;
        let wid = 2.5 + 4.5 * tau + max(lp.y, 0.0) * 0.4;
        if (abs(lp.x) > wid * 1.6 || lp.y < -1.0 || lp.y > top + 6.0) { continue; }
        let n = noise_fbm2(vec2f(lp.x * 0.35, lp.y * 0.3 - tau * 1.2) + vec2f(fk * 7.0, 0.0), 4);
        let body = smoothstep(1.0, 0.2, abs(lp.x) / wid) * smoothstep(top + 2.0, top * 0.5, lp.y) * smoothstep(-1.0, 0.5, lp.y);
        let fade = sq(1.0 - tau / life);
        let a = saturate(body * (0.4 + 1.2 * n) - 0.2) * fade;
        let col = l.amb * 22.0 + vec3f(0.6, 0.65, 0.8) * l.flash * 0.4;
        acc = vec4f(mix(acc.rgb, col, a), max(acc.a, a));
    }
    return acc;
}

// ------------------------------------------------------------ shading

fn shade_land(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = land_normal(p.xz, t, ctx);
    // pink-grey granite in tilted bands, dark and glossy where the sea wets it
    let band = 0.75 + 0.25 * sin(dot(p, vec3f(0.35, 0.9, 0.2)) * 1.3 + noise_fbm2(p.xz * 0.05, 3) * 6.0);
    var alb = col_hex(0x6b625cu) * band * (0.7 + 0.5 * noise_value2(p.xz * 0.4));
    let grass = smoothstep(0.75, 0.9, n.y) * smoothstep(9.0, 13.0, p.y);
    alb = mix(alb, col_hex(0x3c4128u), grass * 0.8);
    let wet = 1.0 - smoothstep(0.5, 5.0, p.y);
    alb *= mix(1.0, 0.4, wet);
    let sky = 0.6 + 0.4 * n.y;
    var c = alb * (l.amb * sky * 1.8 + vec3f(0.55, 0.6, 0.8) * l.flash * 0.3 * saturate(n.y + 0.3));
    // stray lantern light on the ledges around the tower
    let lv = LAMP - p;
    c += alb * col_kelvin(4200.0) * 5.0 * saturate(dot(n, normalize(lv))) / (dot(lv, lv) + 50.0);
    // warm spill from the keeper's windows
    let wv = HOUSE + vec3f(-5.3, 0.0, 0.0) - p;
    c += alb * col_kelvin(2500.0) * 0.8 * saturate(dot(n, normalize(wv))) / (dot(wv, wv) * 0.08 + 1.0);
    // wet sheen
    let r = reflect(rd, n);
    c += backdrop(r, l, ctx, false) * wet * 0.25 * pow(1.0 - saturate(dot(-rd, n)), 3.0);
    return c;
}

fn shade_lh(p: vec3f, rd: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = lh_normal(p, t);
    let q = p - TOWER;
    var alb = vec3f(0.8);
    var emit = vec3f(0.0);
    let lamp_c = col_kelvin(4200.0);
    let tow = normalize(CAM - LAMP);
    let glare = beam_profile(tow, beam_dir(0.0, ctx)) + beam_profile(tow, beam_dir(1.0, ctx));
    if (mat < 1.5) {
        alb = vec3f(0.78, 0.78, 0.76) * (0.92 + 0.08 * noise_value2(vec2f(atan2(q.z, q.x) * 8.0, q.y * 0.5)));
        // a door and two small windows up the tower
        let ang = atan2(q.z, q.x);
        let face = abs(ang - (-2.3));
        let win = step(face, 0.12) * (step(abs(q.y - 8.0), 0.6) + step(abs(q.y - 16.0), 0.6) + step(q.y, 2.6) * step(1.5, q.y));
        alb = mix(alb, vec3f(0.03), saturate(win));
    } else if (mat < 2.5) {
        alb = vec3f(0.03);
    } else if (mat < 3.5) {
        // lantern glass: the lamp and lens glow, fiercely when aimed at us
        alb = vec3f(0.02);
        let mull = step(0.85, fract(atan2(q.z, q.x) * 1.9));
        emit = lamp_c * (1.2 + 60.0 * glare) * (1.0 - mull * 0.8);
    } else if (mat < 4.5) {
        alb = col_hex(0xd8d4c8u);
        // warm windows of the keeper's house
        let hq = p - HOUSE;
        let wx = fract((hq.x + hq.z) * 0.42 + 0.5);
        let win = step(0.35, wx) * step(wx, 0.65) * step(abs(hq.y - 0.2), 0.8) * step(abs(n.y), 0.3);
        emit = col_kelvin(2500.0) * win * 2.2;
    } else {
        alb = col_hex(0x7a2a22u);
    }
    let sky = 0.6 + 0.4 * n.y;
    var c = alb * (l.amb * sky * 2.6 + vec3f(0.55, 0.6, 0.8) * l.flash * 0.35);
    // lantern light falling on the gallery and the top of the tower
    let lv = LAMP - p;
    c += alb * lamp_c * 1.5 * saturate(dot(n, normalize(lv))) / (dot(lv, lv) + 1.0) * step(mat, 2.5);
    return c + emit;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme, ctx);
    let cam = cam_look_at(CAM, CAM + vec3f(0.08, 0.035, -1.0), 0.0, 50.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    var tmax = 3000.0;
    let ts = sea_march(CAM, rd, l);
    let tland = land_march(CAM, rd, select(900.0, ts + 2.0, ts > 0.0), ctx);
    var tl = select(1e9, tland, tland > 0.0);
    let hl = lh_march(CAM, rd, min(tl, 1e5));
    if (hl.x > 0.0) {
        col = shade_lh(CAM + rd * hl.x, rd, hl.x, hl.y, l, ctx);
        tmax = hl.x;
    } else if (tland > 0.0 && (ts < 0.0 || tland < ts)) {
        col = shade_land(CAM + rd * tland, rd, tland, l, ctx);
        tmax = tland;
    } else if (ts > 0.0) {
        col = sea_shade(CAM + rd * ts, rd, ts, l, ctx);
        tmax = ts;
    } else {
        col = backdrop(rd, l, ctx, true) + vec3f(0.8, 0.85, 1.0) * bolt(rd, l, ctx);
    }
    // rain and spray in the air: the world greys out with distance
    // (the sky is only seen through rain as far as the cloud base)
    var dair = min(tmax, 2500.0);
    if (tmax > 2900.0) { dair = min(380.0 / max(rd.y + 0.05, 0.05), 1500.0) * 0.5; }
    let air = 1.0 - exp(-l.fog * dair);
    col = mix(col, l.hor * (1.0 + l.flash * 3.0), air);
    // the beam, integrated along the ray
    col += beam_light(CAM, rd, tmax, l, ctx);
    // spray plumes on the rocks
    if (l.storm > 0.5) {
        let sp = spray(p, cam, tmax, l, ctx);
        col = mix(col, sp.rgb + beam_light(CAM, rd, tmax, l, ctx) * 0.3, sp.a);
    }
    // lamp glare on the lens of the eye
    let lp = cam_project(cam, LAMP);
    let tow = normalize(CAM - LAMP);
    let glare = beam_profile(tow, beam_dir(0.0, ctx)) + beam_profile(tow, beam_dir(1.0, ctx));
    let dl = length(p - lp.xy);
    col += col_kelvin(4200.0) * glare * (0.002 / (dl * dl + 0.00015) * 0.01 + exp(-dl * 18.0) * 0.4) * (0.5 + l.scat);
    // rain streaks, catching the lamp light near the beam
    if (l.rain > 0.0) {
        let rs = rain_streaks(p, ctx, 0.38, 2.3, 0.28, 2);
        col += (l.amb * 1.1 + vec3f(0.12, 0.12, 0.14) * l.flash + col_kelvin(4200.0) * glare * 0.05) * rs * l.rain;
    }
    return col * exp2(l.exposure);
}
