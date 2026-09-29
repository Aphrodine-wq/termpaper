//! name: cloudsea
//! title: Above the Cloud Sea
//! category: weather
//! tags: mountains, clouds, sunrise, summit, inversion
//! desc: sunrise from a rocky summit over a sea of cloud, far peaks standing out like islands
//! themes: sunrise, sunset, moon
//! uses: camera, sky, stars
//! cost: heavy
//! fallback: clouds
//! credits: original

// World units are metres. We stand on a summit 3000 m up, above an
// inversion: a sea of cloud 650 m below fills every valley to the horizon
// (which dips with the curve of the Earth). A few peaks stand out of it.
// The sun rises low on the left, raking the billows so every head casts a
// shadow; the big peak throws its own shadow across the cloud sea.

struct Look {
    sun: vec3f,
    sun_c: vec3f,
    amb: vec3f,
    haze: f32,
    night: f32,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(sky_sun_dir(3.0, 34.0), vec3f(1.55, 0.5, 0.2), vec3f(0.17, 0.16, 0.22), 1.0, 0.0, 0.1);
        }
        case 2u: {
            return Look(sky_sun_dir(9.0, -30.0), vec3f(0.05, 0.058, 0.075), vec3f(0.004, 0.006, 0.012), 1.0, 1.0, 0.9);
        }
        default: {
            return Look(sky_sun_dir(3.0, -28.0), vec3f(1.6, 0.62, 0.18), vec3f(0.19, 0.19, 0.25), 1.0, 0.0, 0.0);
        }
    }
}

fn ozone_t(sun_y: f32) -> vec3f {
    let am = inverseSqrt(sun_y * sun_y + 0.00786);
    return exp(-vec3f(0.0195, 0.0564, 0.00256) * am);
}

// ------------------------------------------------------------------ terrain

const RE: f32 = 6371000.0;
const CLOUD: f32 = 2350.0;

struct Peak { c: vec2f, h: f32, r: f32 }

fn peak(i: i32) -> Peak {
    switch (i) {
        case 0: { return Peak(vec2f(7800.0, -15500.0), 3700.0, 6000.0); }   // the big one, right
        case 1: { return Peak(vec2f(-23000.0, -47000.0), 3400.0, 10000.0); } // far left, under the sun
        case 2: { return Peak(vec2f(3000.0, -64000.0), 3200.0, 16000.0); }  // a long far ridge
        default: { return Peak(vec2f(-5200.0, -9000.0), 2700.0, 3200.0); }  // a rocky island, left
    }
}

// the far massifs: arêtes and faces come from ridged noise multiplied into
// each mountain's bulk, so no two flanks match
fn massif(xz: vec2f) -> f32 {
    var h = 0.0;
    for (var i = 0; i < 4; i++) {
        let pk = peak(i);
        let q = (xz - pk.c) / pk.r;
        let d = length(q * vec2f(1.0, 0.8));
        if (d > 1.3) { continue; }
        let w = vec2f(noise_value2(q * 1.5 + f32(i) * 3.1), noise_value2(q * 1.5 + 7.0 + f32(i))) - 0.5;
        let rn = noise_ridged2(q * 2.3 + w * 0.8 + vec2f(f32(i) * 11.0, 0.0), 3);
        let bulk = pow(max(1.0 - d, 0.0), 1.25);
        h = max(h, pk.h * bulk * (0.45 + 0.75 * rn));
    }
    return h;
}

// the massifs' bulk alone, for their shadows on the cloud sea
fn massif_lo(xz: vec2f) -> f32 {
    var h = 0.0;
    for (var i = 0; i < 4; i++) {
        let pk = peak(i);
        let d = length((xz - pk.c) / pk.r * vec2f(1.0, 0.8));
        h = max(h, pk.h * pow(max(1.0 - d, 0.0), 1.25) * 0.95);
    }
    return h;
}

// our summit: a broken granite top a few metres wide, falling away steeply
// on every side toward the cloud sea
fn summit(xz: vec2f) -> f32 {
    let q = xz - vec2f(-1.0, -1.0);
    let r = length(q * vec2f(0.75, 1.0));
    // a crag on the left, the top shelving away to the right
    var h = 2999.0 - 0.1 * r - 1.2 * max(r - 7.0, 0.0) - 0.004 * r * r - 0.05 * xz.x;
    h += 1.4 * exp(-dot(xz - vec2f(-9.0, -6.0), xz - vec2f(-9.0, -6.0)) / 8.0);
    // fractured granite: sharp ridges and stepped slabs, not pebbles
    let rg = noise_ridged2(xz * 0.28 + vec2f(3.0, 1.0), 3);
    let slab = noise_value2(xz * 0.22) * 3.0;
    h += 0.55 * rg + 0.45 * (floor(slab) + smoothstep(0.8, 1.0, fract(slab))) - 0.8;
    // jointing: blocks split along cracks
    h -= 0.3 * smoothstep(0.08, 0.0, abs(noise_grad2(xz * 0.45 + vec2f(1.7, 0.4))));
    return h;
}

fn terrain(xz: vec2f, fine: bool) -> f32 {
    var h = massif(xz);
    if (dot(xz, xz) < 3600.0) { h = max(h, summit(xz)); }
    if (fine && h > CLOUD - 300.0 && dot(xz, xz) > 1.6e5) {
        h += 40.0 * (noise_value2(xz * 0.004) - 0.5);
    }
    return h;
}

// a cheap upper bound of terrain(): no noise, the noise's largest reach added
fn terrain_hi(xz: vec2f) -> f32 {
    var h = 0.0;
    for (var i = 0; i < 4; i++) {
        let pk = peak(i);
        let d = length((xz - pk.c) / pk.r * vec2f(1.0, 0.8));
        h = max(h, pk.h * pow(max(1.0 - d, 0.0), 1.25) * 1.2 + 20.0);
    }
    if (dot(xz, xz) < 3600.0) {
        let q = xz - vec2f(-1.0, -1.0);
        let r = length(q * vec2f(0.75, 1.0));
        h = max(h, 2999.0 - 0.1 * r - 1.2 * max(r - 7.0, 0.0) - 0.004 * r * r - 0.05 * xz.x + 3.2);
    }
    return h;
}

fn curve_drop(p: vec3f, ro: vec3f) -> f32 {
    let d = p.xz - ro.xz;
    return dot(d, d) / (2.0 * RE);
}

// the summit and far peaks, marched only above the cloud sea
fn trace_rock(ro: vec3f, rd: vec3f, tmax: f32) -> f32 {
    var t = 0.3;
    for (var i = 0; i < 140; i++) {
        let p = ro + rd * t;
        let hgt = p.y + curve_drop(p, ro);
        // below the top of the cloud sea the rock is hidden anyway, and
        // above the highest peak there is nothing left to hit
        if (hgt < CLOUD + 120.0 && rd.y < 0.0) { break; }
        if (hgt > 3800.0 && rd.y > 0.0) { break; }
        let hb = hgt - terrain_hi(p.xz);
        if (hb > 0.4) {
            t += max(hb * 0.8, 0.05 + t * 0.004);
            if (t > tmax) { break; }
            continue;
        }
        let h = hgt - terrain(p.xz, true);
        if (h < 0.0015 * t + 0.02) { return t; }
        t += max(h * select(0.5, 0.7, t > 80.0), 0.05 + t * 0.004);
        if (t > tmax) { break; }
    }
    return -1.0;
}

// ------------------------------------------------------------------ clouds

// height of the cloud sea's top: broad swells, rounded heads with soft
// troughs between them, turbulent fringes; drifting slowly
fn deck_top_lo(xz: vec2f, t: f32) -> f32 {
    let w = xz + vec2f(t * 1.2, t * 0.4);
    var h = CLOUD + 240.0 * (noise_fbm2(w * 0.00007, 2) - 0.5);
    let b = noise_fbm2(w * 0.0008 + vec2f(0.0, t * 0.0006), 3);
    let c = noise_fbm2(w * 0.0026 + vec2f(t * 0.0015, 0.0), 2);
    return h + 300.0 * b * b + 110.0 * c * c - 100.0;
}

fn deck_top(xz: vec2f, t: f32) -> f32 {
    let w = xz + vec2f(t * 1.2, t * 0.4);
    return deck_top_lo(xz, t) + 45.0 * (noise_fbm2(w * 0.0035 + vec2f(t * 0.003, 0.0), 2) - 0.5);
}
struct Cloud { c: vec3f, tr: f32 }

fn deck_march(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx, n: i32) -> Cloud {
    var cl: Cloud;
    cl.c = vec3f(0.0);
    cl.tr = 1.0;
    let top = CLOUD + 330.0;
    // curvature: the deck falls away with distance; rays toward the horizon
    // meet it (the drop is added to the ray height)
    var t = max((top - ro.y) / min(rd.y, -1e-4), 0.0);
    if (rd.y > 0.02) { return cl; }
    t = min(t, 150000.0);
    var tp = t;
    var found = false;
    for (var i = 0; i < 48; i++) {
        let p = ro + rd * t;
        let hgt = p.y + curve_drop(p, ro);
        // the coarse top (fine fringes stay within +-23 m of it)
        let depth = deck_top_lo(p.xz, ctx.t) - hgt;
        if (depth > -34.0) {
            found = true;
            break;
        }
        tp = t;
        let climb = max(-rd.y + t / RE, 0.0008);
        t += max((-depth - 30.0) / climb * 0.62, 4.0 + t * 0.005);
        if (t > tmax || t > 250000.0) { break; }
    }
    if (!found) { return cl; }
    var a = tp;
    var b = t;
    for (var k = 0; k < 5; k++) {
        let m = 0.5 * (a + b);
        let pm = ro + rd * m;
        if (deck_top(pm.xz, ctx.t) - pm.y - curve_drop(pm, ro) > -10.0) { b = m; } else { a = m; }
    }
    let te = b;
    let pe = ro + rd * te;
    // shading of the top: its billows face the low sun or turn away
    let e = max(15.0, te * 0.0012);
    let gx = deck_top(pe.xz + vec2f(e, 0.0), ctx.t) - deck_top(pe.xz - vec2f(e, 0.0), ctx.t);
    let gz = deck_top(pe.xz + vec2f(0.0, e), ctx.t) - deck_top(pe.xz - vec2f(0.0, e), ctx.t);
    let nn = normalize(vec3f(-gx, 2.0 * e, -gz));
    // neighbouring heads shade the troughs; a peak shadows a whole swath
    let sxz = normalize(l.sun.xz);
    let stan = l.sun.y / max(length(l.sun.xz), 1e-3);
    var sh = 1.0;
    let ptop = deck_top(pe.xz, ctx.t);
    for (var k = 1; k < 4; k++) {
        let dd = f32(k * k) * 120.0;
        let ht = deck_top(pe.xz + sxz * dd, ctx.t);
        sh = min(sh, saturate(1.0 - (ht - ptop - dd * stan) / 140.0));
    }
    for (var k = 1; k < 7; k++) {
        let dd = f32(k) * f32(k) * 900.0;
        let q = pe.xz + sxz * dd;
        let th = massif_lo(q);
        if (th > ptop + dd * stan) { sh *= 0.25; break; }
    }
    let fwd = pow(saturate(dot(rd, l.sun)), 8.0);
    let dif = saturate(dot(nn, l.sun) * 1.3 + 0.12);
    let lit = l.sun_c * (dif * sh * 1.1 + fwd * 1.8 * sh) + l.amb * (0.8 + 0.3 * nn.y);
    // integrate the soft top
    let span_ = 120.0 + te * 0.004;
    for (var i = 0; i < 12; i++) {
        if (i >= n || cl.tr < 0.02) { break; }
        let u = (f32(i) + ctx.jitter) / f32(n);
        let tt = te - 8.0 + span_ * u * u;
        let dt = span_ * (2.0 * u + 1.0 / f32(n)) / f32(n);
        let p = ro + rd * tt;
        let depth = deck_top(p.xz, ctx.t) - p.y - curve_drop(p, ro);
        let dens = smoothstep(-12.0, 25.0, depth);
        if (dens <= 0.001) { continue; }
        let c = mix(lit, l.amb * 0.55, smoothstep(0.0, 120.0, depth));
        let st = exp(-dens * dt * 0.035);
        cl.c += cl.tr * c * (1.0 - st);
        cl.tr *= st;
    }
    // a ray that only skimmed a head meets the next one further on: every
    // downward ray ends in cloud
    cl.c += cl.tr * mix(lit, l.amb * 0.55, 0.35);
    cl.tr = 0.0;
    return cl;
}

// ------------------------------------------------------------------ sky

fn air_col(rd: vec3f, l: Look) -> vec3f {
    if (l.night > 0.5) { return sky_night(rd) * 1.4; }
    // seen from 3 km up the horizon is bright haze, not the dark band the
    // sea-level sky model gives at zero elevation
    let r = normalize(vec3f(rd.x, max(rd.y, 0.035), rd.z));
    return sky_atmosphere_haze(r, l.sun, l.haze) * ozone_t(l.sun.y) + vec3f(0.05, 0.045, 0.05) * exp(-max(rd.y, 0.0) * 12.0);
}

fn sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    var c = air_col(rd, l);
    if (l.night > 0.5) {
        c += star_field(rd, 0.9, ctx) * smoothstep(-0.02, 0.1, rd.y);
        c += star_milky_way(rd, normalize(vec3f(0.35, 0.45, 0.82)), normalize(vec3f(-0.6, 0.2, -0.77)), ctx);
        c += sky_moon(rd, l.sun, 0.55, 1.4);
        c += vec3f(0.02, 0.025, 0.035) * pow(saturate(dot(rd, l.sun)), 12.0);
    } else {
        c += sky_sun_disk(rd, l.sun, 0.53);
    }
    return c;
}

// ------------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(0.0, 3001.4, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.02, -0.07, -1.0), 0.0, 44.0);
    let rd = cam_ray(cam, p);
    let pxa = ctx.px / cam.zoom;
    var col = vec3f(0.0);
    var tt = 1e9;
    let tr = trace_rock(ro, rd, 90000.0);
    // the far haze colour, shared by rock and cloud (open sky needs none)
    var haze = vec3f(0.0);
    if (tr > 0.0 || rd.y < 0.02) { haze = air_col(normalize(vec3f(rd.x, 0.03, rd.z)), l) * 1.15; }
    if (tr > 0.0) {
        tt = tr;
        let hp = ro + rd * tr;
        let e = max(0.3, tr * pxa);
        let hx = terrain(hp.xz + vec2f(e, 0.0), true) - terrain(hp.xz - vec2f(e, 0.0), true);
        let hz = terrain(hp.xz + vec2f(0.0, e), true) - terrain(hp.xz - vec2f(0.0, e), true);
        let n = normalize(vec3f(-hx, 2.0 * e, -hz));
        // granite, with snow held on the gentler faces of the high peaks
        let snow = smoothstep(0.55, 0.8, n.y) * smoothstep(2900.0, 3300.0, hp.y) * step(200.0, tr);
        var alb = mix(col_hex(0x3b3835u), col_hex(0x5a5550u), noise_value2(hp.xz * 0.01));
        if (tr < 200.0) {
            // grey granite with pale lichen patches up close
            alb = mix(col_hex(0x55524eu), col_hex(0x77736cu), noise_value2(hp.xz * 0.9));
            alb = mix(alb, col_hex(0x8a8a6eu), smoothstep(0.62, 0.8, noise_value2(hp.xz * 0.5 + 9.0)) * 0.7);
        }
        alb = mix(alb, vec3f(0.8, 0.82, 0.86), snow);
        let dif = saturate(dot(n, l.sun));
        // rock in its own shadow except where the low sun rims its edges
        var sh = 1.0;
        if (tr < 200.0 && dif > 0.0) {
            for (var k = 1; k < 7; k++) {
                let q = hp + l.sun * (f32(k) * 1.6);
                if (terrain(q.xz, true) > q.y) { sh = 0.0; break; }
            }
        }
        col = alb * (l.sun_c * dif * sh + l.amb * (0.5 + 0.5 * n.y));
        // distance haze toward the horizon
        col = mix(col, haze, 1.0 - exp(-tr * 0.000045));
    }
    let cl = deck_march(ro, rd, tt, l, ctx, steps(8.0, ctx));
    if (tr < 0.0 && cl.tr > 0.0) {
        // only what neither rock nor cloud covers needs the sky
        col = sky(rd, l, ctx);
    }
    if (cl.tr < 1.0) {
        // the cloud sea, its far reaches fading into the horizon glow
        col = col * cl.tr + cl.c;
        let dist = min(tt, (CLOUD - ro.y) / min(rd.y, -1e-4));
        col = mix(col, haze, (1.0 - cl.tr) * (1.0 - exp(-abs(dist) * 0.000022)));
    }
    return col * exp2(l.exposure);
}
