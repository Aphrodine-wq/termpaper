//! name: yosemite
//! title: Yosemite Valley
//! category: wilds
//! tags: granite, valley, waterfall, clouds, california
//! desc: El Capitan, Half Dome and Bridalveil Fall from Tunnel View as cloud drifts up the valley
//! themes: morning, sunset, winter
//! uses: camera, sdf, sky, fog, clouds
//! cost: heavy
//! tonemap: punchy
//! fallback: alpine
//! credits: original

// World units are metres, y up, valley floor at y = 0. The camera stands at
// Tunnel View, 120 m above the floor on the south wall, looking up the
// valley (bearing 73°, along -z). Positions come from the real map: El
// Capitan's summit 4 km out and 14° left, the Nose 7° left, Half Dome 13 km
// out just right of centre, Bridalveil's lip 2.6 km out and 13° right.

struct Look {
    sun: vec3f,
    overcast: f32,   // 0 clear .. 1 fully diffuse light
    haze: f32,       // sky model haze
    cloud: f32,      // valley cloud amount
    band_y: f32,     // height of the cloud band hugging the walls
    band_w: f32,     // its half thickness
    deck: f32,       // high cloud deck coverage
    snow: f32,
    exposure: f32,
    sun_c: vec3f,
    amb: vec3f,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        // clearing storm: sun ahead-right behind shredding cloud
        case 0u: { l = Look(sky_sun_dir(24.0, 40.0), 0.15, 0.9, 1.1, 600.0, 200.0, 0.5, 0.0, -0.15, vec3f(0.0), vec3f(0.0)); }
        // alpenglow on El Cap: the sun low behind the right shoulder
        case 1u: { l = Look(sky_sun_dir(3.5, 170.0), 0.0, 1.4, 0.0, 400.0, 100.0, 0.22, 0.0, 0.0, vec3f(0.0), vec3f(0.0)); }
        // winter: low cloud, snow dusting, flat cold light
        default: { l = Look(sky_sun_dir(14.0, 30.0), 0.75, 1.3, 1.1, 520.0, 150.0, 0.9, 1.0, -0.25, vec3f(0.0), vec3f(0.0)); }
    }
    let sl = sky_sun_light(l.sun);
    let sa = sky_ambient(l.sun);
    l.sun_c = sl * (1.0 - 0.85 * l.overcast);
    // an overcast sky is brighter and greyer than a clear one
    l.amb = mix(sa, vec3f(col_luma(sa + sl * 0.12)) * vec3f(0.92, 0.97, 1.08), l.overcast) * (1.0 + 0.6 * l.overcast);
    // twilight: the sky is bright next to the dimmed sun
    l.amb *= 1.0 + 0.5 * sstep(0.15, 0.03, l.sun.y);
    return l;
}

// ------------------------------------------------------------ geometry

fn seg_dist(p: vec2f, a: vec2f, b: vec2f) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = saturate(dot(pa, ba) / dot(ba, ba));
    return length(pa - ba * h);
}

// Plan polylines of the two wall faces, running up-valley (toward -z).
// seg_acc folds one segment in: acc = (distance so far, face x at q's z).
fn seg_acc(acc: vec2f, q: vec2f, a: vec2f, b: vec2f) -> vec2f {
    let inside = q.y <= a.y && q.y > b.y;
    return vec2f(min(acc.x, seg_dist(q, a, b)), select(acc.y, mix(a.x, b.x, (q.y - a.y) / (b.y - a.y)), inside));
}

// face x at depth z (piecewise linear, no distances): cheap bounds and the
// cloud's cling to the walls
fn seg_x(z: f32, a: vec2f, b: vec2f, fx: f32) -> f32 {
    return select(fx, mix(a.x, b.x, (z - a.y) / (b.y - a.y)), z <= a.y && z > b.y);
}
fn face_x_n(z: f32) -> f32 {
    var fx = select(-3600.0, -1150.0, z <= -11200.0);
    fx = seg_x(z, vec2f(-3600.0, -1500.0), vec2f(-1950.0, -2800.0), fx);
    fx = seg_x(z, vec2f(-1950.0, -2800.0), vec2f(-1100.0, -3480.0), fx);
    fx = seg_x(z, vec2f(-1100.0, -3480.0), vec2f(-500.0, -3860.0), fx);
    fx = seg_x(z, vec2f(-500.0, -3860.0), vec2f(-820.0, -4600.0), fx);
    fx = seg_x(z, vec2f(-820.0, -4600.0), vec2f(-980.0, -5900.0), fx);
    fx = seg_x(z, vec2f(-980.0, -5900.0), vec2f(-1900.0, -8000.0), fx);
    fx = seg_x(z, vec2f(-1900.0, -8000.0), vec2f(-1150.0, -11200.0), fx);
    return fx;
}
fn face_x_s(z: f32) -> f32 {
    var fx = select(260.0, 1300.0, z <= -10500.0);
    fx = seg_x(z, vec2f(260.0, 900.0), vec2f(380.0, -600.0), fx);
    fx = seg_x(z, vec2f(380.0, -600.0), vec2f(650.0, -1950.0), fx);
    fx = seg_x(z, vec2f(650.0, -1950.0), vec2f(660.0, -2600.0), fx);
    fx = seg_x(z, vec2f(660.0, -2600.0), vec2f(840.0, -3300.0), fx);
    fx = seg_x(z, vec2f(840.0, -3300.0), vec2f(1000.0, -5000.0), fx);
    fx = seg_x(z, vec2f(1000.0, -5000.0), vec2f(1080.0, -7600.0), fx);
    fx = seg_x(z, vec2f(1080.0, -7600.0), vec2f(1300.0, -10500.0), fx);
    return fx;
}

// North wall face (El Capitan, Three Brothers, Royal Arches): signed
// distance, negative inside the rock (rock lies at smaller x).
fn plan_n(q: vec2f) -> f32 {
    var a = vec2f(1e9, -3600.0);
    a = seg_acc(a, q, vec2f(-3600.0, -1500.0), vec2f(-1950.0, -2800.0));
    a = seg_acc(a, q, vec2f(-1950.0, -2800.0), vec2f(-1100.0, -3480.0));
    a = seg_acc(a, q, vec2f(-1100.0, -3480.0), vec2f(-500.0, -3860.0));   // the Nose
    a = seg_acc(a, q, vec2f(-500.0, -3860.0), vec2f(-820.0, -4600.0));
    a = seg_acc(a, q, vec2f(-820.0, -4600.0), vec2f(-980.0, -5900.0));
    a = seg_acc(a, q, vec2f(-980.0, -5900.0), vec2f(-1900.0, -8000.0));
    a = seg_acc(a, q, vec2f(-1900.0, -8000.0), vec2f(-1150.0, -11200.0));
    if (q.y <= -11200.0) { a.y = -1150.0; }
    return select(a.x, -a.x, q.x < a.y);
}

// South wall face (Leaning Tower, Cathedral Rocks, Sentinel, Glacier
// Point): rock lies at larger x. Bridalveil's recess cuts back into it.
fn plan_s(q: vec2f) -> f32 {
    var a = vec2f(1e9, 260.0);
    a = seg_acc(a, q, vec2f(260.0, 900.0), vec2f(380.0, -600.0));
    a = seg_acc(a, q, vec2f(380.0, -600.0), vec2f(650.0, -1950.0));
    a = seg_acc(a, q, vec2f(650.0, -1950.0), vec2f(660.0, -2600.0));
    a = seg_acc(a, q, vec2f(660.0, -2600.0), vec2f(840.0, -3300.0));
    a = seg_acc(a, q, vec2f(840.0, -3300.0), vec2f(1000.0, -5000.0));
    a = seg_acc(a, q, vec2f(1000.0, -5000.0), vec2f(1080.0, -7600.0));
    a = seg_acc(a, q, vec2f(1080.0, -7600.0), vec2f(1300.0, -10500.0));
    if (q.y <= -10500.0) { a.y = 1300.0; }
    return select(a.x, -a.x, q.x > a.y) + 150.0 * exp(-sq((q.y + 2630.0) / 170.0));
}

fn bump(q: vec2f, c: vec2f, h: f32, r: f32) -> f32 {
    let d = q - c;
    return h * exp(-dot(d, d) / (r * r));
}
// broad summit that steepens away from its centre (granite domes, mesas)
fn mesa(q: vec2f, c: vec2f, h: f32, r: f32) -> f32 {
    let d = q - c;
    return h * max(1.0 - dot(d, d) / (r * r), 0.0);
}

// rock height behind the north face: El Cap's broad summit dome set back
// from the rim, then the lower walls up-valley
fn height_n(q: vec2f) -> f32 {
    var h = 330.0 + 0.08 * max(-q.x - 1500.0, 0.0);
    h = max(h, mesa(q, vec2f(-850.0, -3720.0), 1090.0, 1450.0));   // El Capitan
    h = max(h, mesa(q, vec2f(-1500.0, -4400.0), 900.0, 2200.0));   // its summit plateau
    // the Nose rolls over in a big rounded shoulder
    h -= 260.0 * smoothstep(-1000.0, -480.0, q.x) * sstep(-3300.0, -3800.0, q.y) * smoothstep(-4600.0, -4100.0, q.y);
    h = max(h, mesa(q, vec2f(-1400.0, -6100.0), 830.0, 1500.0));   // Three Brothers
    h = max(h, mesa(q, vec2f(-2800.0, -8300.0), 820.0, 2000.0));
    h = max(h, mesa(q, vec2f(-2300.0, -11800.0), 1000.0, 2000.0)); // North Dome
    return h;
}
fn height_s(q: vec2f) -> f32 {
    var h = 380.0 + 0.1 * max(q.x - 900.0, 0.0);
    h = max(h, mesa(q, vec2f(950.0, -2000.0), 580.0, 900.0));    // Leaning Tower
    h = max(h, mesa(q, vec2f(1250.0, -3250.0), 800.0, 1500.0));  // Cathedral Rocks
    h = max(h, mesa(q, vec2f(1300.0, -4450.0), 720.0, 1000.0));  // Cathedral Spires
    h = max(h, mesa(q, vec2f(1500.0, -7700.0), 960.0, 1300.0));  // Sentinel
    h = max(h, mesa(q, vec2f(2300.0, -9400.0), 960.0, 2000.0));  // Glacier Point
    // the hanging valley Bridalveil Creek pours out of
    let notch = 175.0 + sq((q.y + 2620.0) / 300.0) * 380.0 + max(q.x - 700.0, 0.0) * 0.12;
    return min(h, notch);
}

// signed value noise: half the cost of gradient noise, fine for rock
fn snoise(p: vec2f) -> f32 { return noise_value2(p) * 2.0 - 1.0; }

fn wall(p: vec3f, pd_in: f32, top: f32, s: f32, lod: i32) -> f32 {
    var pd = pd_in;
    // talus apron: the base flares out in a lumpy, wooded slope; the upper
    // face leans back a little
    let apron = max(240.0 - p.y, 0.0);
    pd -= 0.62 * apron;
    if (apron > 0.0) { pd += 14.0 * snoise(p.xz * 0.012 + p.y * 0.006) * saturate(apron / 80.0); }
    pd += max(p.y - 300.0, 0.0) * 0.06;
    // broad bulges, then sharp-edged buttresses (ridged: arêtes stick out)
    pd += 40.0 * snoise(vec2f(s * 0.0022 + 5.0, p.y * 0.0018));
    pd -= 55.0 * (1.0 - abs(snoise(vec2f(s * 0.0032, 1.3 + p.y * 0.0007))));
    // fine flutes only for normals (they barely move the silhouette)
    if (lod > 1) {
        let face = smoothstep(120.0, 320.0, p.y);
        pd -= 12.0 * (1.0 - abs(snoise(vec2f(s * 0.016, p.y * 0.0024)))) * face;
        pd += 3.5 * snoise(vec2f(s * 0.07, p.y * 0.009)) * face;
    }
    var rim = top;
    if (p.y > top - 260.0) { rim += 35.0 * snoise(vec2f(s * 0.004, 7.1)); }
    return op_smax(pd, p.y - rim, 120.0) * 0.8;
}

// The valley floor is a plane rising gently up-valley (intersected
// analytically: sphere tracing a flat floor at grazing angles is what costs);
// near the camera the slope under the viewpoint rises out of it.
const FLOOR_K: f32 = 0.004;
fn floor_y(z: f32) -> f32 { return -z * FLOOR_K; }
fn ground_h(xz: vec2f) -> f32 {
    return floor_y(xz.y) + 72.0 * pow(saturate((xz.y + 950.0) / 950.0), 1.25) * smoothstep(-1200.0, 150.0, xz.x);
}
fn floor_hit(ro: vec3f, rd: vec3f) -> f32 {
    let den = rd.y + FLOOR_K * rd.z;
    if (den >= -1e-5) { return -1.0; }
    return -(ro.y + FLOOR_K * ro.z) / den;
}

// meadow mask (1 = open grass), shared by geometry and shading
fn meadow(xz: vec2f) -> f32 {
    return meadow_k(xz, 0.57);
}
// (lower k: more open ground; snow shows the smaller clearings too)
fn meadow_k(xz: vec2f, k: f32) -> f32 {
    let m = noise_fbm2(xz * 0.0019 + vec2f(3.1, 7.7), 3) + 0.08 * noise_value2(xz * 0.012);
    let valley = sstep(-1300.0, -2200.0, xz.y);
    return smoothstep(k, k + 0.04, m) * valley;
}

// Half Dome: a domed ellipsoid with its north-west side sheared off
// (seen from Tunnel View: a rounded crown whose left side drops away in the
// sheer north-west face, with the shoulder of the back slope on the right)
fn half_dome(p: vec3f) -> f32 {
    let q = p - vec3f(900.0, -250.0, -13400.0);
    let dome = sdf_ellipsoid(q, vec3f(1150.0, 1750.0, 1100.0));
    let n = normalize(vec3f(-0.82, 0.06, 0.57));
    let face = dot(q, n) - 120.0 + 18.0 * snoise(vec2f(q.z - q.x, q.y) * 0.005);
    // the long back slope falling east
    let back = sdf_ellipsoid(q - vec3f(700.0, -300.0, -500.0), vec3f(1500.0, 1500.0, 1400.0));
    return max(min(dome, back), face);
}

// the high country beyond: Clouds Rest and the Tenaya ridges
fn far_h(q: vec2f) -> f32 {
    var h = 350.0 + 380.0 * noise_fbm2(q * vec2f(0.00035, 0.00025) + 4.0, 2);
    h = max(h, bump(q, vec2f(-1400.0, -18500.0), 1650.0, 3200.0));    // Clouds Rest
    h = max(h, bump(q, vec2f(4200.0, -16000.0), 1100.0, 2000.0));
    return h * sstep(-13200.0, -15500.0, q.y);
}

// → (distance, material): 1 ground/forest, 2 granite, 3 half dome,
// 4 high country, 5 near trees
fn sdf(p: vec3f, lod: i32) -> vec2f {
    if (p.y > 1950.0) { return vec2f(p.y - 1900.0, 0.0); }
    // the slope below the viewpoint (the far floor is a plane)
    var res = vec2f(1e5, 1.0);
    if (p.z > -1000.0) { res = vec2f((p.y - ground_h(p.xz)) * 0.9, 1.0); }
    // the walls, each behind a cheap bound: the horizontal offset from the
    // face (x0.55 covers the most oblique segment), and the highest rim
    var dn = max((p.x - face_x_n(p.z)) * 0.55 - 300.0, p.y - 1180.0);
    if (dn < 360.0) {
        let pn = plan_n(p.xz);
        dn = pn - 300.0;
        if (pn < 360.0) { dn = wall(p, pn, height_n(p.xz), p.x * 0.65 - p.z * 0.76, lod); }
    }
    var ds = max((face_x_s(p.z) - p.x) * 0.55 - 400.0, p.y - 1040.0);
    if (ds < 360.0) {
        let ps = plan_s(p.xz);
        ds = ps - 300.0;
        if (ps < 360.0) { ds = wall(p, ps, height_s(p.xz), -p.z + p.x * 0.2, lod); }
    }
    let rock = min(dn, ds);
    res = vec2f(min(res.x, rock), select(res.y, 2.0, rock < res.x));
    if (p.z < -10500.0 && p.y < 1850.0) {
        let hd = half_dome(p);
        if (hd < res.x) { res = vec2f(hd, 3.0); }
        let fr = (p.y - far_h(p.xz)) * 0.7;
        if (fr < res.x) { res = vec2f(fr, 4.0); }
    }
    return res;
}

// Over-relaxed sphere tracing (steps 1.4x the distance; if two bounding
// spheres stop overlapping, step back and continue plainly).
fn trace(ro: vec3f, rd: vec3f, tmax_in: f32, n: i32, ctx: Ctx) -> vec2f {
    let tf = floor_hit(ro, rd);
    var tmax = select(tmax_in, min(tmax_in, tf), tf > 0.0);
    // looking up: above 1200 m only Half Dome and the high country stand,
    // so a ray that clears them there is done once it passes 1200 m
    if (rd.y > 0.0) {
        let t12 = (1200.0 - ro.y) / rd.y;
        let tfar = select(1e9, (-10500.0 - ro.z) / rd.z, rd.z < -1e-4);
        if (ro.y + rd.y * tfar > 1950.0) { tmax = min(tmax, t12); }
    }
    var t = 15.0 + 4.0 * ctx.jitter;
    var h = vec2f(1e9, 0.0);
    var w = 1.4;
    var last_r = 0.0;
    var last_step = 0.0;
    let foot = ctx.px * 0.25;
    for (var i = 0; i < 256; i++) {
        if (i >= n) { break; }
        let q = ro + rd * t;
        // nothing stands above 1950 m
        if (q.y > 1950.0 && rd.y > 0.0) { return vec2f(-1.0, 0.0); }
        h = sdf(q, 1);
        if (w > 1.0 && h.x + last_r < last_step) {
            t += last_r - last_step;
            last_step = last_r;
            w = 1.0;
            continue;
        }
        if (h.x < foot * t) { return vec2f(t, h.y); }
        last_r = h.x;
        last_step = h.x * w;
        t += last_step;
        if (t > tmax) { break; }
    }
    if (tf > 0.0 && t >= tmax) { return vec2f(tf, 1.0); }
    // out of steps while grazing a surface (the far floor, a canopy top):
    // call it a hit rather than let the sky show through as a seam
    if (h.x < 0.004 * t && rd.y < 0.0) { return vec2f(t, h.y); }
    return vec2f(-1.0, 0.0);
}

fn nrm(p: vec3f, t: f32, ctx: Ctx) -> vec3f {
    let e = max(0.3, t * ctx.px * 0.6);
    let k = vec2f(1.0, -1.0);
    return normalize(
        k.xyy * sdf(p + k.xyy * e, 2).x + k.yyx * sdf(p + k.yyx * e, 2).x +
        k.yxy * sdf(p + k.yxy * e, 2).x + k.xxx * sdf(p + k.xxx * e, 2).x);
}

// soft shadow toward the sun over the coarse field
fn shadow(ro: vec3f, l: vec3f) -> f32 {
    var res = 1.0;
    var t = 6.0;
    for (var i = 0; i < 16; i++) {
        let p = ro + l * t;
        if (p.y > 1300.0) { break; }
        let h = sdf(p, 0).x;
        res = min(res, 7.0 * h / t);
        if (res < 0.01) { break; }
        t += clamp(h, 8.0 + t * 0.04, 600.0);
    }
    return saturate(res);
}

// ------------------------------------------------------------ atmosphere

fn deck_cov(rd: vec3f, l: Look, t: f32) -> vec2f {
    // high cloud deck on a plane: (coverage, lit edge toward the sun)
    let hp = rd.xz / max(rd.y + 0.03, 0.02) * 2.4 + vec2f(t * 0.0025, t * 0.0008);
    let q = hp * vec2f(0.5, 0.9) + vec2f(1.7, 4.2);
    let n = noise_fbm2(q, 5);
    let n2 = noise_fbm2(q + normalize(l.sun.xz + vec2f(1e-3)) * 0.06, 5);
    let k = 1.0 / max(l.deck, 0.1) * 1.7;
    let c = saturate((n - (1.0 - l.deck)) * k);
    let c2 = saturate((n2 - (1.0 - l.deck)) * k);
    return vec2f(c, saturate(0.5 + (c - c2) * 2.5));
}

fn sky(rd: vec3f, l: Look, ctx: Ctx, with_sun: bool) -> vec3f {
    var c = clear_sky(rd, l, with_sun);
    if (rd.y > -0.05 && l.deck > 0.05) {
        let dc = deck_cov(rd, l, ctx.t);
        let mu = saturate(dot(rd, l.sun));
        let fwd = pow(mu, 8.0) * 2.5 + pow(mu, 2.0) * 0.3;
        let base = l.amb * 0.85 + l.sun_c * 0.1;
        // thin edges glow, thick cores go dark
        let lit = base + l.sun_c * (0.35 * dc.y + fwd * (1.0 - dc.x * 0.7));
        let cc = mix(lit, base * 0.6, dc.x * dc.x * mix(0.7, 0.3, l.overcast));
        c = mix(c, cc, dc.x * smoothstep(-0.02, 0.12, rd.y));
    }
    return c;
}

// the sky without the cloud deck (also the colour of the far haze)
fn clear_sky(rd: vec3f, l: Look, with_sun: bool) -> vec3f {
    var c = sky_atmosphere_haze(rd, l.sun, l.haze);
    if (l.sun.y < 0.12) {
        let anti = saturate(-dot(normalize(rd.xz + vec2f(1e-4)), normalize(l.sun.xz)));
        let y = max(rd.y, 0.0);
        let belt = exp(-sq((y - 0.09) / 0.07)) * anti;
        let shadow_band = exp(-y / 0.035) * anti;
        c = mix(c, vec3f(col_luma(c)) * vec3f(1.25, 0.9, 0.95), belt * 0.7);
        c = mix(c, vec3f(col_luma(c)) * vec3f(0.8, 0.9, 1.2), shadow_band * 0.6);
        // (the model has no ozone, which is what keeps a real twilight sky blue)
        c *= vec3f(0.95, 0.84, 1.18);
    }
    let grey = vec3f(col_luma(c)) * vec3f(0.95, 0.98, 1.04);
    c = mix(c, grey * 1.2 + l.amb * 0.3, l.overcast * 0.85);
    if (with_sun) { c += sky_sun_disk(rd, l.sun, 0.5) * (1.0 - l.overcast); }
    // the deck's average veil, so haze under a cloudy sky is not blue
    let base = l.amb * 0.85 + l.sun_c * 0.1;
    c = mix(c, base * 0.9, l.deck * 0.45 * sstep(0.3, 0.0, rd.y));
    return c;
}

// density of the cloud drifting through the valley: a torn band hugging
// the walls at mid-height, and low banks lying on the floor up-valley
fn vcloud(p: vec3f, cling: f32, l: Look, t: f32, oct: i32) -> f32 {
    let hb = exp(-sq((p.y - l.band_y) / l.band_w)) * mix(0.04, 1.0, cling) * sstep(9500.0, 6500.0, -p.z);
    let lowm = exp(-max(p.y - 30.0, 0.0) / 110.0) * smoothstep(4000.0, 9000.0, -p.z) * 0.22;
    // empty air mid-valley costs no noise
    if (max(hb, lowm) < 0.03) { return 0.0; }
    let q = (p + vec3f(t * 5.0, t * 0.3, -t * 1.5)) * vec3f(1.0 / 520.0, 1.0 / 150.0, 1.0 / 760.0);
    let n = noise_fbm3(q, oct);
    let band = saturate((n - 0.5 + 0.12 * cling) * 3.5) * hb;
    let low = saturate((n - 0.58) * 3.0) * lowm;
    return (band + low) * l.cloud;
}

// march the valley cloud up to t_end: (light rgb, transmittance)
fn vclouds(ro: vec3f, rd: vec3f, t_end: f32, p_px: vec2f, l: Look, ctx: Ctx) -> vec4f {
    if (l.cloud < 0.01) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let span = cloud_slab(ro, rd, 30.0, l.band_y + 2.2 * l.band_w);
    let t0 = max(span.x, 200.0);
    let t1 = min(min(span.y, t_end), min(t0 + 5500.0, 12000.0));
    if (t1 <= t0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    // even steps through the band (the ray reaches it far out anyway)
    let dt = (t1 - t0) / 16.0;
    var t = t0 + dt * ctx.jitter;
    var tr = 1.0;
    var acc = vec3f(0.0);
    let mu = dot(rd, l.sun);
    let ph = mix(vol_hg(mu, -0.15), vol_hg(mu, 0.75), 0.55) * 4.0 * PI;
    for (var i = 0; i < 16; i++) {
        let p = ro + rd * t;
        let near = min(p.x - face_x_n(p.z), face_x_s(p.z) - p.x);
        let cling = exp(-max(near * 0.8, 0.0) / 260.0);
        let d = vcloud(p, cling, l, ctx.t, 3);
        if (d > 0.003) {
            // self-shadowing by depth in the band: tops lit, bellies dark
            let hfrac = saturate((p.y - l.band_y) / l.band_w * 0.5 + 0.5);
            let direct = l.sun_c * ph * (0.25 + 0.75 * hfrac) * exp(-d * 1.2) * (1.0 - 0.6 * l.overcast);
            let amb = l.amb * (0.55 + 0.65 * hfrac);
            let st = exp(-d * 0.0035 * dt);
            acc += tr * (direct + amb) * (1.0 - st);
            tr *= st;
            if (tr < 0.03) { break; }
        }
        t += dt;
        if (t > t1) { break; }
    }
    return vec4f(acc, tr);
}

fn aerial(col: vec3f, ro: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let hz = clear_sky(normalize(vec3f(rd.x, 0.04, rd.z)), l, false);
    // haze pools in the valley (lit by the sky: blue in shadow); far air
    // takes the horizon colour
    let vfog = mix(l.amb * 1.1, hz, 0.35);
    var c = fog_height(col, vfog, ro, rd, t, 0.00009 * l.haze, 0.0035);
    c = mix(c, hz, 1.0 - exp(-t * 0.000036 * l.haze));
    c += fog_sun(rd, l.sun, l.sun_c, 0.16 * (1.0 - exp(-t * 0.00015)));
    return c;
}

// ------------------------------------------------------------ surfaces

fn deck_shadow(p: vec3f, l: Look, t: f32) -> f32 {
    if (l.cloud < 0.3) { return 1.0; }
    let q = p.xz + l.sun.xz / max(l.sun.y, 0.1) * (1500.0 - p.y) + vec2f(t * 6.0, -t * 2.0);
    let n = noise_fbm2(q * 0.0006, 3);
    return mix(1.0, sstep(0.62, 0.42, n), 0.75);
}

// conifer canopy seen from afar: (albedo, sky occlusion, sun-side factor).
// Crowns are a bump pattern; sunward sides light up, the gaps go dark.
struct Canopy { alb: vec3f, occ: f32, lit: f32 }
fn canopy(p: vec3f, t: f32, l: Look, ctx: Ctx) -> Canopy {
    // crowns sized so a few pixels see each (groves up close, trees far)
    let f = clamp(0.4 / max(t * ctx.px * 3.0, 1.0), 0.03, 0.13);
    let fade = saturate(t * ctx.px * f * 0.4);
    let h1 = noise_value2(p.xz * f + p.y * 0.05);
    let h2 = noise_value2((p.xz + normalize(l.sun.xz + vec2f(1e-4)) * 0.3 / f) * f + p.y * 0.05);
    let fine = noise_value2(p.xz * 0.37 + 5.0);
    var a = mix(col_hex(0x28361eu), col_hex(0x485c34u), mix(h1 * 0.7 + fine * 0.3, 0.45, fade));
    // black oaks and cedars: a few warmer crowns
    a = mix(a, col_hex(0x4f5230u), smoothstep(0.72, 0.9, noise_value2(p.xz * 0.045)) * 0.45);
    return Canopy(a, mix(0.35 + 0.65 * h1, 0.6, fade), mix(saturate(0.55 + (h1 - h2) * 4.0), 0.6, fade));
}

fn shade(p: vec3f, rd: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    var n = vec3f(0.0, 1.0, FLOOR_K);
    if (mat > 1.5 || p.z > -960.0) { n = nrm(p, t, ctx); }
    let dif0 = dot(n, l.sun);
    var sh = 1.0;
    if (dif0 > 0.0 && l.overcast < 0.95) { sh = shadow(p + n * (2.0 + t * ctx.px * 1.5), l.sun) * deck_shadow(p, l, ctx.t); }
    var alb: vec3f;
    var occ = 1.0;
    var snowable = 1.0;
    var crown_lit = 1.0;
    if (mat > 1.5 && mat < 3.5) {
        // granite: pale grey with vertical rain streaks and dark water stains
        let tg = normalize(vec3f(-n.z, 0.0, n.x) + vec3f(1e-4, 0.0, 0.0));
        let s = dot(p, tg);
        let streak = noise_fbm2(vec2f(s * 0.05, p.y * 0.003), 4);
        let stain = smoothstep(0.5, 0.7, noise_fbm2(vec2f(s * 0.014, p.y * 0.0009 + 3.0), 4));
        alb = col_hex(0xb9b3a8u) * (0.8 + 0.4 * streak);
        alb = mix(alb, col_hex(0x57524bu), stain * 0.7);
        // fresh exfoliation scars, paler and warmer
        alb = mix(alb, col_hex(0xd6cdbdu), smoothstep(0.64, 0.78, noise_fbm2(vec2f(s * 0.022 + 9.0, p.y * 0.005), 3)) * 0.5);
        // ledges and the talus carry trees
        let ledge = smoothstep(0.7, 0.9, n.y) * smoothstep(0.5, 0.7, noise_value2(p.xz * 0.11)) * 0.7;
        let talus = 1.0 - smoothstep(60.0, 230.0, p.y - ground_h(p.xz) + 60.0 * saturate(n.y - 0.5));
        alb = mix(alb, col_hex(0x8e897cu), talus * 0.5);
        let wood = saturate(talus * (0.6 + 0.4 * smoothstep(0.3, 0.55, noise_fbm2(p.xz * 0.025, 3))));
        let cp = canopy(p, t, l, ctx);
        alb = mix(alb, col_hex(0x34422au), ledge);
        alb = mix(alb, cp.alb, wood * 0.95);
        occ = mix(1.0, cp.occ, wood);
        crown_lit = mix(1.0, cp.lit, wood);
        if (mat > 2.5) { alb = col_hex(0xaaa69eu) * (0.85 + 0.3 * streak); }
        occ *= 0.75 + 0.25 * saturate(n.y + 0.6);
    } else if (mat > 3.5 && mat < 4.5) {
        let fr = noise_fbm2(p.xz * 0.0012, 4);
        alb = mix(col_hex(0x9a968cu), col_hex(0x27321fu), smoothstep(0.35, 0.55, fr + (0.5 - saturate(n.y)) * -0.3 + 0.25 * (1.0 - smoothstep(900.0, 1500.0, p.y))));
        occ = 0.8;
    } else if (mat > 4.5) {
        // near conifers: dark needles, lighter tips
        let whorl = 0.5 + 0.5 * sin(p.y * 1.7 + noise_value2(p.xz * 0.9) * 4.0);
        let hh = noise_value2(p.xz * 1.3 + p.y * 0.7) * 0.6 + whorl * 0.4;
        alb = mix(col_hex(0x223018u), col_hex(0x43572eu), hh);
        occ = 0.45 + 0.55 * hh;
        crown_lit = 0.5 + 0.5 * whorl;
        snowable = 0.7;
    } else {
        // valley floor: conifer canopy with meadow clearings. Crowns are a
        // bump pattern; their sunward sides light up, the gaps go dark.
        let open = meadow(p.xz);
        let cp = canopy(p, t, l, ctx);
        let grass = mix(col_hex(0x857a45u), col_hex(0x5f6b36u), noise_fbm2(p.xz * 0.012, 3));
        alb = mix(cp.alb, grass, open);
        occ = mix(cp.occ, 1.0, open);
        crown_lit = mix(cp.lit, 1.0, open);
    }
    // snow: on anything facing up, frosting the trees
    if (l.snow > 0.0) {
        let up = smoothstep(0.3, 0.7, n.y + 0.3 * (noise_value2(p.xz * 0.07 + p.y * 0.05) - 0.5));
        var cover = up * l.snow * snowable;
        if (mat > 4.5) { cover *= smoothstep(0.45, 0.75, crown_lit); }
        if (mat < 1.5) { cover = l.snow * mix(0.04 + 0.22 * occ * occ, 0.95, meadow_k(p.xz, 0.48)); }
        alb = mix(alb * mix(1.0, 0.6, l.snow), vec3f(0.8, 0.83, 0.88), cover);
    }
    let dif = saturate(dif0);
    let sky_occ = (0.7 + 0.3 * n.y) * occ * 1.25;
    var c = alb * (l.sun_c * dif * sh * crown_lit * 1.2 + l.amb * sky_occ);
    // warm bounce from sunlit granite into the shadows
    c += alb * l.sun_c * 0.04 * saturate(1.0 - n.y);
    return c;
}

// Bridalveil Fall: a ribbon hanging from the notch in the south wall
fn fall(p: vec2f, cam: Cam, t_hit: f32, l: Look, ctx: Ctx) -> vec4f {
    let top = vec3f(612.0, 200.0, -2640.0);
    let bot = vec3f(570.0, 40.0, -2600.0);
    let a = cam_project(cam, top);
    let b = cam_project(cam, bot);
    if (a.z > t_hit + 80.0) { return vec4f(0.0); }
    let ab = b.xy - a.xy;
    let along = dot(p - a.xy, ab) / dot(ab, ab);
    let v = saturate(along);
    let k = cam.zoom / a.z;
    // wind sways the lower ribbon
    let sway = (5.0 * sin(ctx.t * 0.31) + 3.0 * sin(ctx.t * 0.77 + 1.0)) * v * v * k;
    let dx = p.x - (a.x + ab.x * v) - sway;
    let half_w = (7.0 + 22.0 * v * v) * k;
    let w = max(half_w, ctx.px * 0.6);
    let inside = sstep(w, w * 0.2, abs(dx)) * smoothstep(-0.02, 0.0, along) * sstep(1.05, 0.95, along);
    let u = dx / w;
    let streak = noise_value2(vec2f(u * 2.5 + 3.0, along * 14.0 - ctx.t * 1.4));
    var dens = inside * (0.6 + 0.4 * streak) * mix(1.0, 0.7, v) * min(1.0, half_w / w + 0.3);
    // spray billowing at the foot
    let foot = b.xy + vec2f(0.0, 16.0 * k);
    let mp = (p - foot) / (vec2f(75.0, 50.0) * k);
    let mist = exp(-dot(mp, mp)) * (0.6 + 0.4 * noise_fbm2(mp * 1.3 + vec2f(ctx.t * 0.2, -ctx.t * 0.12), 3));
    dens = max(dens, mist * 0.65);
    let lit = l.amb * 1.5 + l.sun_c * 0.6 * saturate(0.25 + dot(l.sun, vec3f(-0.8, 0.2, 0.55)));
    return vec4f(lit, saturate(dens));
}

// Treetops on the slope just below the viewpoint, drawn in the frame: the
// camera never moves, so three rows of silhouettes stand in for 3D trees.
// sun2 = the sun's direction in the frame. → (colour, coverage)
fn fg_trees(p: vec2f, sun2: vec2f, lit: f32, l: Look, ctx: Ctx) -> vec4f {
    var col = vec3f(0.0);
    var cov = 0.0;
    for (var k = 0; k < 3; k++) {
        let fk = f32(k);
        let w = 0.03 + 0.03 * fk;
        let base = -0.265 - 0.075 * fk;
        let cell = floor(p.x / w);
        for (var j = -1; j <= 1; j++) {
            let c = cell + f32(j);
            let h = hash_cell2(vec2i(i32(c), k), 0x51eu);
            // trees stand in clumps with gaps between
            let clump = noise_value2(vec2f(c * w * 9.0 + fk * 5.0, fk));
            if (h.w > 0.2 + 0.7 * clump) { continue; }
            let cx = (c + 0.15 + 0.7 * h.x) * w;
            let top = base + (0.004 + 0.05 * h.y * h.y * (0.4 + clump) + 0.045 * step(0.93, h.z)) * (1.0 + fk * 0.7);
            let dy = top - p.y;
            if (dy < 0.0) { continue; }
            // ragged spire: branch clumps of random length
            let tier = 0.011 * (1.0 + fk * 0.8);
            let saw = noise_value2(vec2f(dy / tier, c * 7.31 + fk * 3.0));
            let hw = dy * (0.13 + 0.06 * h.z) * (0.6 + 0.7 * saw) + ctx.px * 0.3;
            let dx = p.x - cx;
            let rag = noise_value2(vec2f(p.y * 260.0 / (1.0 + fk), dx * 200.0 + c)) - 0.5;
            let m = sstep(hw + ctx.px, hw - ctx.px, abs(dx) + 0.35 * hw * rag);
            if (m <= 0.0) { continue; }
            // shading: the sunward side and the tips catch light
            let side = saturate(0.5 + 0.9 * sign(dx) * sun2.x * saturate(abs(dx) / hw));
            let tipk = exp(-dy / (0.03 * (1.0 + fk)));
            let hh = 0.75 + 0.25 * h.z;
            var alb = mix(col_hex(0x2a3824u), col_hex(0x45573au), hh * (0.6 + 0.4 * saw));
            var snow = 0.0;
            if (l.snow > 0.0) { snow = l.snow * smoothstep(0.55, 0.85, saw) * smoothstep(-0.1, 0.3, rag) * 0.45; }
            alb = mix(alb, vec3f(0.75, 0.78, 0.84), snow);
            let occ = mix(0.45, 1.0, saw) * mix(0.55, 1.0, tipk);
            let c3 = alb * (l.amb * occ + l.sun_c * lit * side * (0.15 + 0.85 * tipk) * 0.6);
            col = mix(col, c3, m);
            cov = max(cov, m);
        }
    }
    return vec4f(col, cov);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(0.0, 120.0, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.0, 0.044, -1.0), 0.0, 28.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    var t_hit = 1e6;
    let hit = trace(ro, rd, 30000.0, steps(96.0, ctx), ctx);
    if (hit.x > 0.0) {
        t_hit = hit.x;
        let hp = ro + rd * hit.x;
        col = aerial(shade(hp, rd, hit.x, hit.y, l, ctx), ro, rd, hit.x, l, ctx);
    } else {
        col = sky(rd, l, ctx, true);
    }
    let f = fall(p, cam, t_hit, l, ctx);
    if (f.a > 0.001) { col = mix(col, aerial(f.rgb, ro, rd, 2700.0, l, ctx), f.a); }
    let vc = vclouds(ro, rd, t_hit, p / ctx.px, l, ctx);
    col = col * vc.a + vc.rgb;
    if (p.y < -0.16) {
        // the viewpoint slope is in the wall's shadow once the sun is low
        let lit = select(0.0, 1.0, l.sun.y > 0.15) * (1.0 - l.overcast);
        let sun2 = normalize(vec2f(dot(l.sun, cam.rt), dot(l.sun, cam.up)) + vec2f(1e-4));
        let ft = fg_trees(p, sun2, lit, l, ctx);
        col = mix(col, ft.rgb, ft.a);
    }
    return col * exp2(l.exposure);
}
