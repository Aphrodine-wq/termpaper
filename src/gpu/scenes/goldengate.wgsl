//! name: goldengate
//! title: Fog over the Golden Gate
//! category: weather
//! tags: san francisco, bridge, fog, karl the fog, ocean
//! desc: the Golden Gate's towers rising out of a rolling fog bank, the city faint beyond
//! themes: morning, sunset, night
//! uses: camera, sky, stars
//! cost: heavy
//! fallback: clouds
//! credits: original

// World units are metres, water at y = 0. The bridge runs along z: the north
// tower at z = 0, the south tower 1280 m away at z = -1280; west (the
// Pacific) is +x. We stand on the Marin headlands 115 m up, west of the
// bridge, looking south along it. Karl the Fog fills the strait below the
// deck and pours in from the ocean on the right.

struct Look {
    sun: vec3f,
    sun_c: vec3f,
    amb: vec3f,
    haze: f32,
    night: f32,
    exposure: f32,
    bank_top: f32,     // mean height of the fog bank
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(sky_sun_dir(3.5, 102.0), vec3f(1.42, 0.47, 0.11), vec3f(0.17, 0.17, 0.21), 1.8, 0.0, 0.35, 62.0);
        }
        case 2u: {
            return Look(normalize(vec3f(-0.4, 0.5, -0.75)), vec3f(0.02, 0.024, 0.034), vec3f(0.0025, 0.003, 0.005), 1.0, 1.0, 0.4, 58.0);
        }
        default: {
            return Look(sky_sun_dir(11.0, -62.0), vec3f(2.36, 1.5, 0.96), vec3f(0.249, 0.337, 0.454), 1.6, 0.0, -0.3, 70.0);
        }
    }
}

fn ozone_t(sun_y: f32) -> vec3f {
    let am = inverseSqrt(sun_y * sun_y + 0.00786);
    return exp(-vec3f(0.0195, 0.0564, 0.00256) * am);
}

// ------------------------------------------------------------------ bridge

const DECK_Y: f32 = 67.0;
const TOWER_H: f32 = 227.0;
const SPAN: f32 = 1280.0;
const HALF_W: f32 = 14.0;       // cable planes at x = +-14

// slab-test a ray against an axis-aligned box: (t_near, t_far)
fn box_hit(ro: vec3f, inv: vec3f, bmin: vec3f, bmax: vec3f) -> vec2f {
    let t0 = (bmin - ro) * inv;
    let t1 = (bmax - ro) * inv;
    let tn = min(t0, t1);
    let tf = max(t0, t1);
    return vec2f(max(max(tn.x, tn.y), tn.z), min(min(tf.x, tf.y), tf.z));
}

// one tower at z = zc: two stepped legs and the portal struts between them.
// Returns (t, normal code: 1 x, 2 y, 3 z) of the nearest hit.
fn tower_hit(ro: vec3f, rd: vec3f, inv: vec3f, zc: f32) -> vec2f {
    var best = vec2f(1e9, 0.0);
    for (var i = 0; i < 2; i++) {
        let sx = select(-1.0, 1.0, i == 1);
        // each leg steps in three times on the way up
        for (var k = 0; k < 3; k++) {
            let fk = f32(k);
            let y0 = fk * 76.0;
            let w = 5.4 - fk * 0.9;
            let d = 8.5 - fk * 1.3;
            let c = vec3f(sx * HALF_W, 0.0, zc);
            let h = box_hit(ro, inv, c + vec3f(-w, y0, -d), c + vec3f(w, min(y0 + 76.0, TOWER_H), d));
            if (h.x < h.y && h.y > 0.0 && h.x < best.x) {
                let hp = ro + rd * h.x - c;
                var code = 3.0;
                if (abs(abs(hp.x) - w) < 0.05 * h.x * 0.002 + 0.02) { code = 1.0; }
                if (abs(hp.y - min(y0 + 76.0, TOWER_H)) < 0.05) { code = 2.0; }
                best = vec2f(h.x, code);
            }
        }
    }
    // portal struts (deck level bracing and three upper portals)
    for (var k = 0; k < 4; k++) {
        let y = select(DECK_Y - 12.0 + f32(k) * 52.0, TOWER_H - 16.0, k == 3);
        let hgt = select(8.0, 12.0, k == 3);
        let h = box_hit(ro, inv, vec3f(-HALF_W, y, zc - 4.0), vec3f(HALF_W, y + hgt, zc + 4.0));
        if (h.x < h.y && h.y > 0.0 && h.x < best.x) {
            best = vec2f(h.x, 3.0);
        }
    }
    return best;
}

fn deck_hit(ro: vec3f, inv: vec3f) -> f32 {
    let h = box_hit(ro, inv, vec3f(-HALF_W - 1.0, DECK_Y - 7.5, -SPAN - 400.0), vec3f(HALF_W + 1.0, DECK_Y, 420.0));
    if (h.x < h.y && h.y > 0.0) { return max(h.x, 0.0); }
    return 1e9;
}

// main cable height at z (catenary approximated by parabolas, incl. the
// side spans down to the anchorages)
fn cable_y(z: f32) -> f32 {
    if (z > 0.0) {
        let u = saturate(z / 345.0);
        return TOWER_H - (TOWER_H - DECK_Y - 2.0) * (1.0 - sq(1.0 - u)) ;
    }
    if (z < -SPAN) {
        let u = saturate((-SPAN - z) / 345.0);
        return TOWER_H - (TOWER_H - DECK_Y - 2.0) * (1.0 - sq(1.0 - u));
    }
    let u = (z + SPAN * 0.5) / (SPAN * 0.5);
    return DECK_Y + 8.0 + (TOWER_H - DECK_Y - 8.0) * u * u;
}

// cables and suspenders as seen through the vertical cable planes: returns
// coverage and the distance of the plane hit
fn cables(ro: vec3f, rd: vec3f, pxa: f32) -> vec2f {
    var cov = 0.0;
    var dist = 1e9;
    if (abs(rd.x) < 1e-5) { return vec2f(0.0, 1e9); }
    for (var i = 0; i < 2; i++) {
        let px = select(-HALF_W, HALF_W, i == 1);
        let t = (px - ro.x) / rd.x;
        if (t <= 0.0) { continue; }
        let q = ro + rd * t;
        if (q.z > 360.0 || q.z < -SPAN - 360.0 || q.y < DECK_Y) { continue; }
        let fp = pxa * t;
        // the main cable, ~0.9 m thick: energy-conserving when sub-pixel
        let cy = cable_y(q.z);
        let dc = abs(q.y - cy);
        let w = max(fp, 0.9);
        var c = saturate(1.0 - dc / w) * (0.9 / w);
        // suspenders every 15 m, much thinner
        if (q.y < cy && q.z < 0.0 && q.z > -SPAN) {
            let ds = abs(fmod_pos(q.z, 15.2) - 7.6) - 7.6;
            let ws = max(fp, 0.12);
            c = max(c, saturate(1.0 - abs(ds + 7.6 - 7.6) / ws) * (0.12 / ws) * 0.0);
            let sx = abs(fmod_pos(q.z + 7.6, 15.2) - 7.6);
            c = max(c, saturate(1.0 - sx / ws) * (0.14 / ws) * 0.8);
        }
        if (c > cov) {
            cov = c;
            dist = t;
        }
    }
    return vec2f(saturate(cov), dist);
}

// ------------------------------------------------------------------ land

// the Marin headland we stand on, falling to the strait; and the hills
// across the water
fn hill_h(xz: vec2f) -> f32 {
    // falls away from just behind where we stand, steepest toward the strait
    let q = (xz - vec2f(160.0, 560.0)) * vec2f(0.8, 1.0);
    return 116.5 - 0.55 * length(q) + 5.0 * noise_value2(xz * 0.03) - 2.5;
}

fn trace_hill(ro: vec3f, rd: vec3f) -> f32 {
    if (rd.y > 0.1) { return -1.0; }
    var t = 2.0;
    for (var i = 0; i < 48; i++) {
        let p = ro + rd * t;
        let h = p.y - hill_h(p.xz);
        if (h < 0.3) { return t; }
        t += max(h * 0.6, 1.0);
        if (t > 900.0 || p.y < 0.0) { break; }
    }
    return -1.0;
}

// ------------------------------------------------------------------ fog

fn bank_top(xz: vec2f, t: f32, l: Look) -> f32 {
    // the bank rolls in from the Pacific (+x) and spills through the gap
    let q = xz * 0.0035 + vec2f(-t * 0.0032, t * 0.0005);
    let n = noise_fbm2(q, 4);
    let billow = noise_fbm2(xz * 0.009 + vec2f(-t * 0.012, 0.0), 3);
    return min(l.bank_top + (n - 0.5) * 80.0 + (billow - 0.5) * 34.0, 104.0);
}

// march the fog slab; (radiance, transmittance, depth of the first dense fog)
struct Fog { c: vec3f, tr: f32, t: f32 }

fn bank_march(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx, n: i32) -> Fog {
    var f: Fog;
    f.c = vec3f(0.0);
    f.tr = 1.0;
    f.t = 1e9;
    let top = l.bank_top + 62.0;
    if (rd.y >= 0.0 && ro.y > top) { return f; }
    var t0 = 0.0;
    if (ro.y > top) { t0 = (top - ro.y) / rd.y; }
    let t1 = min(tmax, 12000.0);
    if (t0 >= t1) { return f; }
    let fwd = pow(saturate(dot(rd, l.sun) * 0.5 + 0.5), 5.0);
    let wisp_c = l.sun_c * (0.45 + 1.6 * fwd) + l.amb;
    var t = t0;
    // a grazing ray can skim one billow and dive into the next: search for
    // the top, integrate through it, and search again
    for (var hop = 0; hop < 3; hop++) {
        if (f.tr < 0.03 || t >= t1) { break; }
        // phase 1: step down to the soft top, collecting stray wisps
        var tp = t;
        var found = false;
        for (var i = 0; i < 40; i++) {
            let p = ro + rd * t;
            let depth = bank_top(p.xz, ctx.t, l) - p.y;
            if (depth > -6.0) {
                found = true;
                break;
            }
            let dt = max((-depth - 4.0) / max(-rd.y, 0.015) * 0.45, 3.0 + t * 0.006);
            if (depth > -45.0) {
                let w = noise_value3(vec3f(p.x - ctx.t * 3.0, p.y * 2.0, p.z) * 0.012);
                let dw = smoothstep(0.62, 0.86, w) * exp(depth / 18.0) * 0.3;
                let st = exp(-dw * min(dt, 60.0) * 0.02);
                f.c += f.tr * wisp_c * (1.0 - st);
                f.tr *= st;
            }
            tp = t;
            t += dt;
            if (t > t1) { break; }
        }
        if (!found) { break; }
        // refine where the ray meets the top
        var a = tp;
        var b = t;
        for (var k = 0; k < 4; k++) {
            let m = 0.5 * (a + b);
            let pm = ro + rd * m;
            if (bank_top(pm.xz, ctx.t, l) - pm.y > -6.0) { b = m; } else { a = m; }
        }
        let te = b;
        let pe = ro + rd * te;
        var shade = 1.0;
        if (hop == 0) {
            f.t = te;
            // the rolling top catches the light on its sunward billows, and
            // the towers cast their shadows across it
            let e = 8.0;
            let gx = bank_top(pe.xz + vec2f(e, 0.0), ctx.t, l) - bank_top(pe.xz - vec2f(e, 0.0), ctx.t, l);
            let gz = bank_top(pe.xz + vec2f(0.0, e), ctx.t, l) - bank_top(pe.xz - vec2f(0.0, e), ctx.t, l);
            let nn = normalize(vec3f(-gx, 2.0 * e, -gz));
            shade = mix(0.55, 1.25, saturate(saturate(dot(nn, l.sun) * 1.6 + 0.35) / (0.35 + 1.6 * max(l.sun.y, 0.05))));
            if (l.night < 0.5) {
                let inv_sun = 1.0 / select(l.sun, vec3f(1e-6), abs(l.sun) < vec3f(1e-6));
                let s0 = tower_hit(pe, l.sun, inv_sun, 0.0);
                let s1 = tower_hit(pe, l.sun, inv_sun, -SPAN);
                if (min(s0.x, s1.x) < 1e8) { shade *= 0.3; }
            }
        }
        // phase 2: integrate through the first few tens of metres
        let span_ = min(t1, te + 220.0) - te;
        if (span_ <= 0.0) { break; }
        for (var i = 0; i < 16; i++) {
            if (i >= n || f.tr < 0.02) { break; }
            let u = (f32(i) + ctx.jitter) / f32(n);
            let tt = te - 6.0 + span_ * u * u;
            let dt = span_ * (2.0 * u + 1.0 / f32(n)) / f32(n);
            let p = ro + rd * tt;
            let depth = bank_top(p.xz, ctx.t, l) - p.y;
            let dens = smoothstep(-8.0, 14.0, depth);
            if (dens <= 0.001) { continue; }
            // sun reaches only the top of the bank; below it glows grey
            let sunk = exp(-max(depth, 0.0) * 0.045);
            var c = l.sun_c * sunk * shade * (0.45 + 1.6 * fwd) + l.amb * (0.55 + 0.45 * sunk);
            // city and bridge lights glow in it at night
            if (l.night > 0.5) {
                let city = exp(-length(p.xz - vec2f(-2600.0, -3200.0)) / 2200.0);
                let bridge = exp(-abs(p.x) / 60.0) * step(p.z, 420.0) * step(-SPAN - 400.0, p.z) * exp(-abs(p.y - DECK_Y) / 40.0);
                c += vec3f(1.0, 0.55, 0.22) * (0.05 * city + 0.035 * bridge);
            }
            let st = exp(-dens * dt * 0.02);
            f.c += f.tr * c * (1.0 - st);
            f.tr *= st;
        }
        t = te + span_;
    }
    return f;
}

// hills standing out of the fog: the Marin headlands on the right, the
// Presidio beyond the south tower, the East Bay far to the left
fn far_hills(rd: vec3f, l: Look, pxa: f32) -> vec4f {
    let az = atan2(rd.x, -rd.z);
    var e = 0.0;
    // Marin: close, high, falling into the strait
    e = max(e, 0.13 * smoothstep(0.12, 0.62, az) - 0.016 + (0.014 * noise_value2(vec2f(az * 14.0, 1.0)) + 0.004 * noise_value2(vec2f(az * 60.0, 2.0))) * smoothstep(0.1, 0.3, az));
    // Presidio and Lands End, low and far
    e = max(e, 0.012 * smoothstep(-0.02, 0.1, az) * sstep(0.32, 0.2, az) + 0.003 * noise_value2(vec2f(az * 40.0, 3.0)) - 0.004);
    // East Bay hills, a faint far line
    e = max(e, 0.014 * smoothstep(-0.95, -0.7, az) * sstep(-0.3, -0.45, az) - 0.002);
    let cov = saturate((e - rd.y) / max(pxa, 1e-5) + 0.5);
    if (cov <= 0.0) { return vec4f(0.0); }
    let near = smoothstep(0.12, 0.5, az);
    // chaparral and dry grass in patches, the slope lit toward its crest
    let scrub = noise_value2(vec2f(az * 70.0, rd.y * 260.0)) * 0.6 + noise_value2(vec2f(az * 220.0, rd.y * 700.0)) * 0.4;
    let crest = smoothstep(e - 0.03, e, rd.y);
    var c = mix(vec3f(0.36, 0.4, 0.34), mix(vec3f(0.13, 0.16, 0.1), vec3f(0.34, 0.3, 0.2), scrub), near);
    c *= l.sun_c * (0.18 + 0.3 * crest) + l.amb * 0.8;
    if (l.night > 0.5) { c = vec3f(0.002, 0.0025, 0.003); }
    // far ones sink into the haze
    let hz = air_col(normalize(vec3f(rd.x, 0.02, rd.z)), l);
    c = mix(c, hz, mix(0.75, 0.25, near));
    return vec4f(c, cov);
}
// ------------------------------------------------------------------ sky

fn air_col(rd: vec3f, l: Look) -> vec3f {
    if (l.night > 0.5) { return sky_night(rd) * 1.3 + vec3f(0.008, 0.005, 0.003) * exp(-max(rd.y, 0.0) * 10.0); }
    return sky_atmosphere_haze(rd, l.sun, l.haze) * ozone_t(l.sun.y) + vec3f(0.08, 0.08, 0.09) * exp(-max(rd.y, 0.0) * 12.0) * saturate(l.sun.y * 5.0);
}

fn sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    var c = air_col(rd, l);
    if (l.night > 0.5) {
        c += star_field(rd, 0.35, ctx) * 0.4 * smoothstep(0.05, 0.25, rd.y);
    } else {
        c += sky_sun_disk(rd, l.sun, 0.53);
    }
    return c;
}

// the city across the strait: a hazy skyline, lit windows at night
fn city(rd: vec3f, l: Look, pxa: f32, ctx: Ctx) -> vec4f {
    let az = atan2(rd.x, -rd.z);
    let e = rd.y;
    if (az > 0.12 || az < -0.55 || e > 0.06) { return vec4f(0.0); }
    let x = (az + 0.55) * 60.0;
    let cell = floor(x);
    let h = hash_cell2(vec2i(i32(cell), 0), 0xc17u);
    var top = 0.004 + 0.012 * h.x * h.x * smoothstep(0.0, 8.0, x) * sstep(40.0, 20.0, x);
    // a pyramid, a tall tower and a few hills
    top = max(top, 0.036 * saturate(1.0 - abs(x - 16.5) / 0.7) - 0.004);
    top = max(top, select(0.0, 0.031, abs(x - 19.2) < 0.25));
    top = max(top, 0.01 * exp(-sq((x - 30.0) / 5.0)) + 0.006 * exp(-sq((x - 5.0) / 4.0)));
    let base = 0.001;
    if (e > base + top) { return vec4f(0.0); }
    let cov = saturate((base + top - e) / pxa + 0.5);
    var c = mix(vec3f(0.55, 0.55, 0.58), vec3f(0.9, 0.85, 0.8), h.y) * (l.sun_c * 0.25 + l.amb * 0.9);
    if (l.night > 0.5) {
        c = vec3f(0.004, 0.004, 0.006);
        let win = hash_cell2(vec2i(i32(x * 9.0), i32(e * 2600.0)), 0x51u);
        c += vec3f(1.0, 0.75, 0.45) * step(0.72, win.x) * 0.15;
    }
    return vec4f(c, cov);
}

// ------------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(150.0, 118.0, 520.0);
    let cam = cam_look_at(ro, ro + vec3f(-0.1, 0.012, -1.0), 0.0, 42.0);
    let rd = cam_ray(cam, p);
    let pxa = ctx.px / cam.zoom;
    let inv = 1.0 / select(rd, vec3f(1e-6), abs(rd) < vec3f(1e-6));
    var col: vec3f;
    var tt = 1e9;
    // the headland under our feet
    let th = trace_hill(ro, rd);
    // the bridge
    let tn = tower_hit(ro, rd, inv, 0.0);
    let ts = tower_hit(ro, rd, inv, -SPAN);
    let td = deck_hit(ro, inv);
    var tb = min(min(tn.x, ts.x), td);
    var code = select(ts.y, tn.y, tn.x < ts.x);
    if (td < min(tn.x, ts.x)) { code = 2.5; }
    let tw = select(1e9, -ro.y / rd.y, rd.y < 0.0);
    if (th > 0.0 && th < min(tb, tw)) {
        tt = th;
        let hp = ro + rd * th;
        let e = 1.5;
        let hx = hill_h(hp.xz + vec2f(e, 0.0)) - hill_h(hp.xz - vec2f(e, 0.0));
        let hz = hill_h(hp.xz + vec2f(0.0, e)) - hill_h(hp.xz - vec2f(0.0, e));
        let n = normalize(vec3f(-hx, 2.0 * e, -hz));
        // coastal scrub: dusty green, brown where it has dried
        let alb = mix(col_hex(0x3d4a2cu), col_hex(0x6e6040u), noise_value2(hp.xz * 0.05)) * (0.7 + 0.3 * noise_value2(hp.xz * 0.6));
        col = alb * (l.sun_c * saturate(dot(n, l.sun)) + l.amb * 0.8);
    } else if (tb < tw) {
        tt = tb;
        let hp = ro + rd * tb;
        // International Orange; the deck's underside dark
        var n = vec3f(0.0, 0.0, 1.0);
        if (code < 1.5) { n = vec3f(sign(hp.x - select(0.0, sign(hp.x) * HALF_W, abs(hp.x) > 2.0)), 0.0, 0.0); }
        if (code > 1.5 && code < 2.8) { n = vec3f(0.0, 1.0, 0.0); }
        if (code > 2.8) { n = vec3f(0.0, 0.0, sign(ro.z - hp.z)); }
        if (abs(hp.x) > 8.0 && code > 2.8) { n = normalize(vec3f(sign(hp.x) * 0.3, 0.0, sign(ro.z - hp.z))); }
        var alb = col_hex(0xb33a26u);
        // seen from above, the deck is a grey roadway between orange rails
        if (code > 2.2 && code < 2.8 && hp.y > DECK_Y - 0.5) {
            alb = mix(col_hex(0x4a4a4cu), alb, smoothstep(HALF_W - 1.5, HALF_W - 0.5, abs(hp.x)));
        }
        let dif = saturate(dot(n, l.sun));
        col = alb * (l.sun_c * dif * 0.9 + l.amb * 0.7);
        if (l.night > 0.5) {
            // floodlit towers
            col += alb * vec3f(1.0, 0.6, 0.3) * 0.02 * sstep(TOWER_H, 80.0, hp.y);
        }
    } else if (tw < 1e8) {
        tt = tw;
        let hp = ro + rd * tw;
        col = vec3f(0.01, 0.018, 0.022) + air_col(reflect(rd, vec3f(0.0, 1.0, 0.0)), l) * 0.06;
        let _u = hp.x;
    } else {
        col = sky(rd, l, ctx);
        let cy = city(rd, l, pxa, ctx);
        let hz = air_col(normalize(vec3f(rd.x, 0.02, rd.z)), l);
        col = mix(col, mix(cy.rgb, hz, 0.55), cy.w);
        let fh = far_hills(rd, l, pxa);
        col = mix(col, fh.rgb, fh.w);
    }
    // haze over the distance
    if (tt < 1e8) {
        col = mix(col, air_col(normalize(vec3f(rd.x, 0.02, rd.z)), l), 1.0 - exp(-tt * 0.00018));
    }
    // cables in front of what they cross
    let cb = cables(ro, rd, pxa);
    if (cb.x > 0.0 && cb.y < tt) {
        let cc = col_hex(0xa8382au) * (l.sun_c * 0.5 + l.amb * 0.7) + vec3f(1.0, 0.6, 0.3) * 0.01 * l.night;
        col = mix(col, cc, cb.x * exp(-cb.y * 0.00018));
    }
    // deck lamps and aviation lights at night
    if (l.night > 0.5) {
        for (var i = 0; i < 2; i++) {
            let px = select(-HALF_W - 1.0, HALF_W + 1.0, i == 1);
            let t = (px - ro.x) / rd.x;
            if (t > 0.0 && t < tt + 5.0) {
                let q = ro + rd * t;
                let dz = abs(fmod_pos(q.z, 50.0) - 25.0) - 25.0;
                let d = length(vec2f(dz + 25.0 - 25.0, q.y - DECK_Y - 9.0));
                let lamp = exp(-sq(fmod_pos(q.z + 25.0, 50.0) - 25.0) / sq(max(pxa * t, 0.6))) * exp(-sq(q.y - DECK_Y - 9.0) / sq(max(pxa * t, 0.6)));
                if (q.z < 420.0 && q.z > -SPAN - 400.0) { col += vec3f(1.2, 0.7, 0.3) * lamp * 1.5; }
                let _d = d;
            }
        }
        for (var k = 0; k < 2; k++) {
            let tp = vec3f(0.0, TOWER_H + 3.0, -SPAN * f32(k));
            let pr = cam_project(cam, tp);
            let dd = length(p - pr.xy);
            col += vec3f(2.0, 0.1, 0.05) * exp(-dd * dd / sq(ctx.px * 1.2)) * (0.6 + 0.4 * sin(ctx.t * 3.0 + f32(k)));
        }
    }
    // the fog bank in front of everything below its top
    let fg = bank_march(ro, rd, tt, l, ctx, steps(12.0, ctx));
    col = col * fg.tr + fg.c;
    return col * exp2(l.exposure);
}
