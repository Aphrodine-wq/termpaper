//! name: skyislands
//! title: Sky Islands
//! category: aerial
//! tags: floating islands, cloud sea, waterfall, ruins, fantasy, aerial
//! desc: grass-topped islands floating over a sea of cloud, a waterfall pouring off the nearest
//! themes: sunrise, noon, dusk, storm
//! params: islands, clouds, sunlight
//! uses: camera, raymarch, sdf, sky, noise, clouds, fog, light
//! cost: heavy
//! fallback: clouds
//! credits: original

// World units are metres. The camera hangs in still air above a sea of
// cloud, looking out across a field of floating islands: grass on top,
// rock tapering to a root underneath, the nearest with a stand of pines and
// a waterfall pouring off its edge into mist; a ruined tower on the next.
// The cloud sea is a soft heightfield lit by the sun, drifting slowly; the
// islands shadow it. Only the water, the cloud drift and a passing flock of
// birds move: the camera and the islands are still.

const DECK: f32 = -42.0;        // mean height of the cloud sea's top
const HERO_C: vec3f = vec3f(-12.0, -6.0, -84.0);
const HERO_R: f32 = 40.0;
const TOWER_C: vec3f = vec3f(74.0, 4.0, -176.0);
const TOWER_R: f32 = 24.0;
const FALL_XZ: vec2f = vec2f(17.5, -62.0);   // where the stream leaves the hero island's rim
const POOL_XZ: vec2f = vec2f(11.0, -66.0);   // the tarn it drains

struct Look {
    sun: vec3f,
    sun_c: vec3f,
    amb: vec3f,
    haze: f32,
    storm: f32,
    deck_cov: f32,     // cloud sea: how thick and billowy
    exposure: f32,
    fogc: vec3f,       // aerial perspective colour near the horizon
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: { l.sun = sky_sun_dir(56.0, -125.0); l.haze = 1.3; l.storm = 0.0; l.exposure = -0.2; }
        case 2u: { l.sun = sky_sun_dir(2.0, -44.0); l.haze = 2.2; l.storm = 0.0; l.exposure = 0.35; }
        case 3u: { l.sun = sky_sun_dir(34.0, 20.0); l.haze = 6.0; l.storm = 1.0; l.exposure = -0.3; }
        default: { l.sun = sky_sun_dir(5.0, 38.0); l.haze = 1.7; l.storm = 0.0; l.exposure = 0.05; }
    }
    l.sun_c = sky_sun_light(l.sun);
    l.amb = sky_ambient(l.sun);
    if (l.storm > 0.5) {
        // under an overcast: diffuse grey light, the sun a bright patch
        l.sun_c = l.sun_c * 0.15 + l.amb * 0.3;
        l.amb = l.amb * 0.55 + vec3f(0.015);
    }
    l.fogc = l.amb * 0.8 + l.sun_c * 0.1;
    l.deck_cov = 1.0;
    return l;
}

// the sea-level sky model has no ozone and goes olive at a low sun: the
// absorption it lacks, by the sun's air mass
fn ozone_t(sun_y: f32) -> vec3f {
    let am = inverseSqrt(sun_y * sun_y + 0.00786);
    return exp(-vec3f(0.0195, 0.0564, 0.00256) * am);
}

// ------------------------------------------------------------------ islands

struct Isl { c: vec3f, r: f32, kind: u32, seed: f32 }

// the field of islands: one candidate per 150 x 170 m cell, most present;
// the two placed islands keep their neighbourhood clear
fn isl_cell(id: vec2i, ctx: Ctx) -> Isl {
    let h = hash_cell2(id, 0x15a7u);
    var isl: Isl;
    isl.r = 0.0;
    let cx = (f32(id.x) + 0.5 + (h.x - 0.5) * 0.36) * 150.0;
    let cz = (f32(id.y) + 0.5 + (h.y - 0.5) * 0.36) * 170.0;
    let cy = (h.z - 0.5) * 70.0 - 8.0;
    let c = vec3f(cx, cy, cz);
    let present = h.w < 0.2 + 0.5 * ctx.params.x;
    if (!present) { return isl; }
    if (distance(c.xz, HERO_C.xz) < 170.0 || distance(c.xz, TOWER_C.xz) < 110.0) { return isl; }
    if (length(c.xz) < 120.0) { return isl; }
    isl.c = c;
    isl.r = 13.0 + 22.0 * hash_f(hash_u2(vec2u(bitcast<u32>(id.x), bitcast<u32>(id.y))) ^ 0x33u);
    isl.kind = select(0u, 2u, h.x > 0.55);
    isl.seed = h.y * 40.0;
    return isl;
}

// one island's body: a flattened grass dome over a rock root, roughened
fn isl_body(q: vec3f, r: f32, seed: f32, fine: bool) -> f32 {
    // bounding first: everything below is inside this
    let b = length(q * vec3f(1.0, 1.9, 1.1)) - r * 1.75;
    if (b > 6.0) { return b; }
    var top = sdf_ellipsoid(q, vec3f(r, r * 0.26, r * 0.86));
    let depth = r * 1.35;
    var root = sdf_cone_y(q - vec3f(0.0, -depth * 0.5 + r * 0.05, 0.0), depth * 0.5, r * 0.08, r * 0.92);
    let coarse = op_smin(top, root, r * 0.12);
    // the noise only matters near the surface
    if (!fine || coarse > r * 0.3) { return coarse; }
    // meadow bumps on top, broken rock on the root
    top -= (noise_fbm2(q.xz * (2.2 / r) + seed, 3) - 0.5) * r * 0.13;
    // the rim breaks into crags
    top -= (noise_ridged2(q.xz * (1.3 / r) + seed * 0.7, 2) - 0.45) * r * 0.07 * smoothstep(r * 0.5, r * 0.95, length(q.xz));
    let rock = noise_fbm3(q * (4.0 / r) + seed, 2);
    root -= (rock - 0.5) * r * 0.17 + (noise_value3(q * (14.0 / r)) - 0.5) * r * 0.035;
    return op_smin(top, root, r * 0.12);
}

// a pine: trunk and a roughened two-tier crown, rooted at `base`
fn pine(p: vec3f, base: vec3f, h: f32, seed: f32) -> f32 {
    let q = p - base;
    let bb = length(q - vec3f(0.0, h * 0.5, 0.0)) - h * 0.8;
    if (bb > 2.0) { return bb; }
    let trunk = sdf_cyl_y(q - vec3f(0.0, h * 0.2, 0.0), h * 0.2, h * 0.035);
    var crown = sdf_cone_y(q - vec3f(0.0, h * 0.58, 0.0), h * 0.42, h * 0.3, h * 0.01);
    let lower = sdf_cone_y(q - vec3f(0.0, h * 0.4, 0.0), h * 0.18, h * 0.36, h * 0.2);
    crown = min(crown, lower);
    crown += (noise_value3(q * 1.3 + seed) - 0.5) * h * 0.1;
    return min(trunk, crown);
}

// the stand of pines on the hero island: (offset from its centre, height);
// the turf there is the island's dome, computed once by hand
fn pine_at(i: i32) -> vec4f {
    switch (i) {
        case 0: { return vec4f(-14.0, 7.6, -8.0, 15.0); }
        case 1: { return vec4f(-7.0, 8.4, -13.0, 13.0); }
        case 2: { return vec4f(-20.0, 6.3, 2.0, 11.0); }
        case 3: { return vec4f(-2.0, 8.8, -4.0, 16.0); }
        default: { return vec4f(-11.0, 8.3, 4.0, 9.0); }
    }
}

// the ruined watchtower on the second island
fn tower(p: vec3f) -> f32 {
    let base = TOWER_C + vec3f(2.0, TOWER_R * 0.2, -1.0);
    let q = p - base;
    let bb = length(q - vec3f(0.0, 9.0, 0.0)) - 15.0;
    if (bb > 2.0) { return bb; }
    let ht = 17.0;
    var d = sdf_cyl_y(q - vec3f(0.0, ht * 0.5, 0.0), ht * 0.5, 5.2);
    // hollow, the far wall fallen; a doorway
    d = max(d, -sdf_cyl_y(q - vec3f(0.0, ht * 0.5 + 1.0, 0.0), ht * 0.5, 4.0));
    d = max(d, -(length(q - vec3f(4.5, 13.0, 3.5)) - 7.0));
    d = max(d, -sdf_box(q - vec3f(0.0, 2.2, 5.0), vec3f(1.2, 2.2, 3.0)));
    // crenellations
    let a = atan2(q.z, q.x);
    let s = abs(fmod_pos(a, TAU / 8.0) - TAU / 16.0) * 5.2;
    d = max(d, -max(ht - 1.6 - q.y, 1.0 - s));
    // weathered stone
    d += (noise_value3(q * 0.9) - 0.5) * 0.5;
    return d;
}

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    if (p.y > 70.0) { return vec2f(p.y - 64.0, 1.0); }
    if (p.y < DECK - 8.0) { return vec2f(DECK - 2.0 - p.y, 1.0); }
    var d = 1e5;
    var m = 1.0;
    // the hero island, its pines and the waterfall's notch
    let qh = p - HERO_C;
    var hero = isl_body(qh, HERO_R, 3.0, true);
    if (hero < 12.0) {
        hero = max(hero, -(length((p.xz - FALL_XZ) * vec2f(1.0, 1.4)) - 3.0 + max(HERO_C.y - 1.5 - p.y, 0.0) * 1.5));
        // the tarn that feeds it: a shallow basin in the turf
        hero = max(hero, -(length((p.xz - POOL_XZ) * vec2f(1.0, 1.3)) - 7.0 + max(p.y - (HERO_C.y + 2.6), 0.0) * 4.0));
        var pines = 1e5;
        for (var i = 0; i < 5; i++) {
            let s = pine_at(i);
            pines = min(pines, pine(p, HERO_C + s.xyz, s.w, f32(i) * 7.3));
        }
        if (pines < hero) { d = pines; m = 3.0; } else { d = hero; m = 2.0; }
    } else {
        d = hero;
        m = 2.0;
    }
    // the tower island
    let qt = p - TOWER_C;
    let ti = isl_body(qt, TOWER_R, 11.0, true);
    if (ti < d) { d = ti; m = 2.0; }
    let tw = tower(p);
    if (tw < d) { d = tw; m = 4.0; }
    // the field: an island never leaves its cell, so only this cell's is
    // tested; the distance to the cell wall (padded past the march's hit
    // tolerance) bounds the step so the next cell gets its turn
    let size = vec2f(150.0, 170.0);
    let cell = floor(p.xz / size);
    let local = p.xz - (cell + 0.5) * size;
    let edge = min(size.x * 0.5 - abs(local.x), size.y * 0.5 - abs(local.y)) + 6.0;
    var df = edge;
    var isl: Isl;
    isl.r = 0.0;
    if (p.y < 46.0) { isl = isl_cell(vec2i(cell), ctx); }
    if (isl.r > 0.0) {
        let near = length(p.xz - isl.c.xz) < 400.0;
        df = min(df, isl_body(p - isl.c, isl.r, isl.seed, near));
    }
    if (df < d) { d = df; m = 1.0; }
    return vec2f(d, m);
}

// ------------------------------------------------------------------ cloud sea

fn deck_h(xz: vec2f, t: f32, fine: bool) -> f32 {
    let w = xz + vec2f(t * 3.2, t * 1.1);
    var h = DECK + 26.0 * (noise_fbm2(w * 0.0035, 2) - 0.5);
    let b = noise_value2(w * 0.011 + vec2f(0.0, t * 0.002));
    h += 22.0 * b * b;
    if (fine) { h += 5.0 * (noise_value2(w * 0.045 + vec2f(t * 0.004, 0.0)) - 0.5); }
    return h;
}

// does the sun reach deck point d? the placed islands and the nearest field
// islands throw shadows across the sea
fn deck_shadow(d: vec3f, sun: vec3f, ctx: Ctx) -> f32 {
    var sh = 1.0;
    // sphere tests against the two placed islands
    for (var i = 0; i < 2; i++) {
        let c = select(TOWER_C, HERO_C, i == 0);
        let r = select(TOWER_R, HERO_R, i == 0) * 0.9;
        let oc = d - c - vec3f(0.0, -r * 0.3, 0.0);
        let b = dot(oc, sun);
        let h = b * b - dot(oc, oc) + r * r;
        if (h > 0.0 && -b > 0.0) { sh *= 0.18; }
    }
    for (var k = 1; k < 4; k++) {
        let q = d + sun * (f32(k * k) * 22.0);
        let isl = isl_cell(vec2i(floor(q.xz / vec2f(150.0, 170.0))), ctx);
        if (isl.r > 0.0 && length((q - isl.c) * vec3f(1.0, 1.6, 1.1)) < isl.r * 0.95) { sh *= 0.25; }
    }
    return sh;
}

struct Cloud { c: vec3f, tr: f32, t: f32 }

fn deck_march(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx) -> Cloud {
    var cl: Cloud;
    cl.c = vec3f(0.0);
    cl.tr = 1.0;
    cl.t = tmax;
    if (rd.y > -0.004) { return cl; }
    let top = DECK + 24.0;
    var t = max((top - ro.y) / rd.y, 0.0);
    var tp = t;
    var found = false;
    for (var i = 0; i < 32; i++) {
        let p = ro + rd * t;
        let depth = deck_h(p.xz, ctx.t, false) - p.y;
        if (depth > -3.0) { found = true; break; }
        tp = t;
        t += max((-depth - 2.0) / (-rd.y) * 0.7, 1.5 + t * 0.006);
        if (t > tmax) { break; }
    }
    if (!found) { return cl; }
    var a = tp;
    var b = t;
    for (var k = 0; k < 5; k++) {
        let m = 0.5 * (a + b);
        let pm = ro + rd * m;
        if (deck_h(pm.xz, ctx.t, true) - pm.y > -1.5) { b = m; } else { a = m; }
    }
    let te = b;
    cl.t = te;
    let pe = ro + rd * te;
    let e = max(2.5, te * ctx.px * 1.2);
    let gx = deck_h(pe.xz + vec2f(e, 0.0), ctx.t, true) - deck_h(pe.xz - vec2f(e, 0.0), ctx.t, true);
    let gz = deck_h(pe.xz + vec2f(0.0, e), ctx.t, true) - deck_h(pe.xz - vec2f(0.0, e), ctx.t, true);
    let nn = normalize(vec3f(-gx, 2.0 * e, -gz));
    // the billows shade each other toward the sun
    let sxz = normalize(l.sun.xz + vec2f(1e-4, 0.0));
    let stan = l.sun.y / max(length(l.sun.xz), 1e-3);
    var sh = 1.0;
    let ptop = deck_h(pe.xz, ctx.t, false);
    for (var k = 1; k < 4; k++) {
        let dd = f32(k * k) * 14.0;
        let ht = deck_h(pe.xz + sxz * dd, ctx.t, false);
        sh = min(sh, saturate(1.0 - (ht - ptop - dd * stan) / 12.0));
    }
    sh *= deck_shadow(pe, l.sun, ctx);
    let fwd = pow(saturate(dot(rd, l.sun)), 6.0);
    let dif = saturate(dot(nn, l.sun) * 1.25 + 0.1);
    let lit = l.sun_c * (dif * sh * 0.95 + fwd * 1.3 * sh) + l.amb * (0.8 + 0.35 * nn.y);
    // the soft top: a few samples through the fringe
    let n = steps(4.0, ctx);
    let span_ = 14.0 + te * 0.03;
    for (var i = 0; i < 12; i++) {
        if (i >= n || cl.tr < 0.02) { break; }
        let u = (f32(i) + ctx.jitter) / f32(n);
        let tt = te - 1.0 + span_ * u * u;
        let dt = span_ * (2.0 * u + 1.0 / f32(n)) / f32(n);
        let p = ro + rd * tt;
        let depth = deck_h(p.xz, ctx.t, true) - p.y;
        let dens = smoothstep(-1.5, 4.0, depth);
        if (dens <= 0.001) { continue; }
        let c = mix(lit, l.amb * 0.5, smoothstep(0.0, 14.0, depth));
        let st = exp(-dens * dt * 0.3);
        cl.c += cl.tr * c * (1.0 - st);
        cl.tr *= st;
    }
    // every downward ray ends in cloud
    cl.c += cl.tr * mix(lit, l.amb * 0.5, 0.4);
    cl.tr = 0.0;
    return cl;
}

// ------------------------------------------------------------------ sky

fn sky(rd: vec3f, l: Look, ctx: Ctx, with_sun: bool) -> vec3f {
    let r = normalize(vec3f(rd.x, max(rd.y, 0.012), rd.z));
    var c = sky_atmosphere_haze(r, l.sun, l.haze) * ozone_t(l.sun.y);
    // and lean the horizon warm
    c = mix(c, c * vec3f(1.1, 0.95, 0.97), exp(-max(rd.y, 0.0) * 9.0));
    if (l.storm > 0.5) {
        // under the overcast the air is grey, not the clear model's horizon
        c = mix(l.amb * 1.3, c, 0.25);
        // an overcast deck overhead, lit from behind, thinning toward the sun
        let hp = rd.xz / (max(rd.y, 0.03) + 0.02) * 0.9;
        let cs = cloud_sheet(hp * 0.35 + vec2f(5.0, 2.0), 0.9, ctx);
        let glow = pow(saturate(dot(rd, l.sun)), 3.0);
        let lit = l.amb * (0.35 + 0.5 * cs.y) + l.sun_c * glow * 0.5;
        c = mix(c * 0.7, lit, cs.x * 0.97 * smoothstep(0.0, 0.06, rd.y));
        c += l.sun_c * glow * 0.12;
        return c;
    }
    if (with_sun) { c += sky_sun_disk(rd, l.sun, 0.55); }
    if (rd.y > 0.0) {
        // high cirrus
        let hp = rd.xz / (rd.y + 0.05) * 1.4;
        let cs = cloud_sheet(hp * 0.7 + vec2f(2.0, 7.0), 0.38 + 0.2 * ctx.params.y, ctx);
        let lit = l.sun_c * (0.25 + 0.9 * pow(saturate(dot(rd, l.sun) * 0.5 + 0.5), 4.0)) + l.amb * 0.6;
        c = mix(c, lit * mix(0.6, 1.15, cs.y), cs.x * 0.5 * smoothstep(0.0, 0.1, rd.y));
    }
    return c;
}

// ------------------------------------------------------------------ shading

// a short, coarse shadow march toward the sun: the pines on the turf, the
// tower on its island (the full soft shadow is ten times the cost)
fn sun_shadow(p: vec3f, sun: vec3f, ctx: Ctx) -> f32 {
    var res = 1.0;
    var t = 0.6;
    for (var i = 0; i < 14; i++) {
        let h = map(p + sun * t, ctx).x;
        res = min(res, 6.0 * h / t);
        if (res < 0.02) { break; }
        t += clamp(h, 0.5, 6.0);
        if (t > 90.0) { break; }
    }
    return saturate(res);
}

fn shade(p: vec3f, rd: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = rm_normal(p, t, ctx);
    let slope = 1.0 - n.y;
    var alb: vec3f;
    if (mat > 2.5 && mat < 3.5) {
        // pines: dark needles, lighter tips
        alb = mix(col_hex(0x203a1eu), col_hex(0x4a6b2cu), noise_value3(p * 0.8) * 0.8);
    } else if (mat > 3.5) {
        // the tower's masonry: blocks, mortar, ivy on the shaded side
        let blocks = noise_value3(vec3f(p.x * 0.9, p.y * 1.6, p.z * 0.9));
        alb = mix(col_hex(0x7a7066u), col_hex(0x9c9284u), blocks);
        alb = mix(alb, col_hex(0x3c5a2au), smoothstep(0.55, 0.75, noise_fbm3(p * 0.5, 3)) * 0.7);
    } else {
        let grass = mix(col_hex(0x5c7a2eu), col_hex(0x8fa44au), noise_fbm2(p.xz * 0.15, 3));
        let rock = col_hex(0x6d6259u);
        let strata = 0.82 + 0.18 * sin(p.y * 0.7 + noise_fbm3(p * 0.05, 2) * 10.0);
        let rocky = smoothstep(0.35, 0.6, slope + (noise_value3(p * 0.3) - 0.5) * 0.3);
        alb = mix(grass, rock * strata, rocky);
        // bare earth at the lip where the turf breaks
        alb = mix(alb, col_hex(0x6b5540u), smoothstep(0.2, 0.35, slope) * (1.0 - rocky) * 0.5);
        alb *= 0.8 + 0.4 * noise_value3(p * 1.7);
    }
    // the tarn: dark water with the sky in it
    let pool = smoothstep(7.0, 5.5, length((p.xz - POOL_XZ) * vec2f(1.0, 1.3))) * step(HERO_C.y - 1.0, p.y) * step(p.y, HERO_C.y + 3.0) * step(2.5, mat) * step(mat, 2.5);
    var water = vec3f(0.0);
    if (pool > 0.0) {
        let wn = normalize(vec3f(0.0, 1.0, 0.0) + (noise_value3(vec3f(p.x * 2.0, ctx.t * 0.8, p.z * 2.0)) - 0.5) * 0.08);
        let glint = pow(saturate(dot(reflect(rd, wn), l.sun)), 200.0) * 5.0;
        water = col_hex(0x1c3a3cu) * l.amb * 1.2 + l.amb * 0.45 + l.sun_c * glint;
    }
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    if (dif > 0.0 && t < 260.0 && l.storm < 0.5) {
        sh = sun_shadow(p + n * (0.4 + t * ctx.px * 1.5), l.sun, ctx);
    }
    var occ = 0.55 + 0.45 * saturate(n.y);
    if (t < 220.0) { occ *= rm_ao(p, n, ctx); }
    var c = alb * (l.sun_c * dif * sh * 1.15 + l.amb * occ * 1.3);
    // a little light bouncing up off the cloud sea onto the undersides
    c += alb * l.amb * 0.4 * saturate(-n.y);
    // wet rock near the waterfall glints
    let wet = exp(-length(p.xz - FALL_XZ) * 0.12) * step(p.y, HERO_C.y + 2.0);
    c += l.sun_c * light_ggx(n, -rd, l.sun, 0.35) * wet * 0.3;
    return mix(c, water, pool);
}

// the stream pouring off the hero island: (colour, alpha, distance)
fn waterfall(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx) -> vec4f {
    let top = HERO_C.y - 1.0;
    let bottom = DECK - 4.0;
    // ray against a vertical cylinder that widens as it falls
    let oc = ro.xz - FALL_XZ;
    let a = dot(rd.xz, rd.xz);
    if (a < 1e-6) { return vec4f(0.0); }
    let b = dot(oc, rd.xz);
    let rad = 3.4;
    let h = b * b - a * (dot(oc, oc) - rad * rad * 4.0);
    if (h < 0.0) { return vec4f(0.0); }
    let sq_ = sqrt(h);
    let t0 = max((-b - sq_) / a, 0.0);
    let t1 = min((-b + sq_) / a, tmax);
    if (t1 <= t0) { return vec4f(0.0); }
    var col = vec3f(0.0);
    var alpha = 0.0;
    let n = 6;
    for (var i = 0; i < 6; i++) {
        let tt = mix(t0, t1, (f32(i) + ctx.jitter) / f32(n));
        let p = ro + rd * tt;
        if (p.y > top + 0.5 || p.y < bottom) { continue; }
        let fall = top - p.y;
        let r = length(p.xz - FALL_XZ);
        // the sheet: narrow and dense at the lip, loosening into spray
        let width = rad * (0.55 + fall * 0.04);
        let core = smoothstep(width, width * 0.3, r);
        let ang = atan2(p.z - FALL_XZ.y, p.x - FALL_XZ.x);
        let streak = noise_value2(vec2f(ang * 2.2 + r * 0.6, (fall * 0.18 + ctx.t * 2.4)));
        let spray = noise_value3(vec3f(p.x * 0.5, fall * 0.12 + ctx.t * 1.6, p.z * 0.5));
        let dens = core * (0.5 + 0.7 * smoothstep(0.3, 0.7, streak)) * smoothstep(0.0, 1.5, fall) * 1.6
            + 0.3 * smoothstep(1.3, 0.3, r / width) * smoothstep(0.4, 0.7, spray) * smoothstep(5.0, 18.0, fall);
        if (dens <= 0.001) { continue; }
        // mist at the foot, glowing where the sun is behind it
        let fwd = pow(saturate(dot(rd, l.sun)), 4.0);
        let c = vec3f(0.92, 0.96, 1.0) * (l.sun_c * (0.5 + 0.6 * fwd) + l.amb * 1.6);
        let st = exp(-dens * (t1 - t0) / f32(n) * 0.9);
        col += (1.0 - alpha) * c * (1.0 - st);
        alpha += (1.0 - alpha) * (1.0 - st);
    }
    return vec4f(col, alpha);
}

// a flock crossing the sky now and then, in a loose V
fn birds(p: vec2f, ctx: Ctx) -> f32 {
    let ev = hash_event(ctx.t, 64.0, 0xb1adu);
    if (ev.x > 0.6) { return 0.0; }
    let dir = select(1.0, -1.0, ev.x > 0.3);
    let span_ = ctx.half.x * 2.0 + 0.6;
    let lead = vec2f(dir * (ev.y * span_ - span_ * 0.5), 0.12 + ev.x * 0.5);
    var m = 0.0;
    for (var i = 0; i < 7; i++) {
        let k = f32((i + 1) / 2) * select(1.0, -1.0, (i & 1) == 1);
        let c = lead - vec2f(dir * abs(k) * 0.035, abs(k) * 0.012 + k * 0.004);
        let q = p - c;
        if (abs(q.x) > 0.02 || abs(q.y) > 0.02) { continue; }
        let flap = sin(ctx.t * 7.0 + f32(i) * 1.9) * 0.006;
        let d = min(sdf2_segment(q, vec2f(0.0), vec2f(-0.009, flap + 0.003)),
                    sdf2_segment(q, vec2f(0.0), vec2f(0.009, flap + 0.003)));
        m = max(m, 1.0 - sstep(0.0007, 0.0007 + ctx.px, d));
    }
    return m;
}

// ------------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let shift = cam_depth_offset(ctx) * 4.0;
    let ro = vec3f(2.0, 24.0, 4.0) + shift;
    var cam = cam_look_at(ro, ro + vec3f(-0.06, -0.16, -1.0), 0.0, 52.0);
    cam = cam_portal(cam, ctx, 90.0);
    let rd = cam_ray(cam, p);
    // nothing stands above 70 m, and the cloud sea hides whatever lies
    // below its top: bound the march by the ray's height
    var far = 2600.0;
    if (rd.y > 0.002) { far = min(far, (70.0 - ro.y) / rd.y); }
    if (rd.y < -0.002) { far = min(far, (DECK + 22.0 - ro.y) / rd.y); }
    let hit = rm_march_k(ro, rd, 0.5, far, steps(66.0, ctx), 0.85, ctx);
    // (the bound was for the march: the cloud sea and the haze run on)
    var t_scene = 2600.0;
    var col: vec3f;
    // one sky evaluation a pixel: the horizon colour for anything hit (the
    // haze it fades into), the real sky for a miss
    var horizon: vec3f;
    if (hit.x > 0.0) {
        t_scene = hit.x;
        horizon = sky(normalize(vec3f(rd.x, 0.02, rd.z)), l, ctx, false);
        let hp = ro + rd * hit.x;
        col = shade(hp, rd, hit.x, hit.y, l, ctx);
        col = mix(col, horizon, 1.0 - exp(-hit.x * 0.00075 * (0.6 + l.haze * 0.25)));
    } else if (rd.y < -0.004) {
        horizon = sky(normalize(vec3f(rd.x, 0.02, rd.z)), l, ctx, false);
        col = horizon;
    } else {
        col = sky(rd, l, ctx, true);
        horizon = col;
    }
    // the waterfall in front of whatever the ray reached
    let wf = waterfall(ro, rd, t_scene, l, ctx);
    col = col * (1.0 - wf.w) + wf.rgb;
    // the cloud sea, in front of anything beyond it
    let cl = deck_march(ro, rd, t_scene, l, ctx);
    if (cl.tr < 1.0) {
        var sea = cl.c;
        sea = mix(sea, horizon, 1.0 - exp(-cl.t * 0.00045 * (0.6 + l.haze * 0.25)));
        col = col * cl.tr + sea;
    }
    if (l.storm < 0.5) { col += fog_sun(rd, l.sun, l.sun_c, 0.08 * (1.0 - exp(-t_scene * 0.0015))); }
    // birds, as silhouettes against the sky only
    if (hit.x < 0.0 && cl.tr > 0.5) {
        col *= 1.0 - birds(p, ctx) * 0.75;
    }
    // a focused monitor: a touch more light on the hero
    col *= 1.0 + 0.06 * ctx.focus_pulse * ctx.reaction * exp(-length(p - ctx.focus) * 3.0);
    return col * exp2(l.exposure + (ctx.params.z - 0.5) * 0.8);
}
