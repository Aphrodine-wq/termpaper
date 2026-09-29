//! name: dolomites
//! title: Dolomites Alpenglow
//! category: wilds
//! tags: mountains, meadow, alpenglow, stars, italy
//! desc: the Tre Cime towers above green alpine meadows and a lone hut as cloud drifts past
//! themes: alpenglow, midday, starry
//! uses: camera, sdf, sky, stars, fog, light
//! cost: heavy
//! tonemap: punchy
//! fallback: alpine
//! credits: original

// World units are metres, y up, the meadow under the camera at y ~ 0. The
// view is the one from the Locatelli meadows: pasture with a hut, a basin,
// grey talus cones, and 2 km off the three north faces of the Tre Cime,
// pale bedded dolomite 600 m high, with the jagged Paterno ridge beside
// them on the left. Each tower is a stack of ~50 m beds; every bed has its
// own faceted plan, so the walls step in at ledges and out at overhangs,
// arêtes run where facets meet, and chimneys and flutes score the faces.

struct Look {
    sun: vec3f,
    night: f32,
    haze: f32,
    mist: f32,       // haze lying in the basin at the towers' feet
    lit_line: f32,   // first light: height above which the low sun reaches
    exposure: f32,
    sun_c: vec3f,
    amb: vec3f,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        // first light from the east, raking across the faces from the right
        case 0u: { l = Look(sky_sun_dir(3.0, 104.0), 0.0, 1.5, 0.32, 395.0, 0.0, vec3f(0.0), vec3f(0.0)); }
        // early afternoon from the left: the faces stand in half light
        case 1u: { l = Look(sky_sun_dir(38.0, -122.0), 0.0, 1.2, 0.3, -1e4, -0.45, vec3f(0.0), vec3f(0.0)); }
        default: { l = Look(sky_sun_dir(-25.0, 120.0), 1.0, 1.0, 0.25, -1e4, 1.1, vec3f(0.0), vec3f(0.0)); }
    }
    if (l.night > 0.5) {
        // moonless: starlight and airglow only
        l.sun_c = vec3f(0.0);
        l.amb = vec3f(0.02, 0.027, 0.05);
    } else {
        l.sun_c = sky_sun_light(l.sun);
        l.amb = sky_ambient(l.sun);
        if (l.sun.y < 0.1) {
            // enrosadira: first light turns the pale rock rose and orange,
            // while the basin still lies in cold blue shadow
            l.sun_c *= vec3f(1.7, 0.95, 0.85) * 2.3;
            l.amb = vec3f(col_luma(l.amb)) * vec3f(0.66, 0.82, 1.4) * 1.6;
        }
    }
    return l;
}

// ------------------------------------------------------------ geometry

fn snoise(p: vec2f) -> f32 { return noise_value2(p) * 2.0 - 1.0; }

// Tre Cime di Lavaredo, north faces: (x, z) of each tower's centre, plan
// half extents. Cima Ovest, Cima Grande, Cima Piccola and its needle, the
// Piccolissima.
const T0: vec4f = vec4f(-320.0, -2080.0, 150.0, 115.0);
const T1: vec4f = vec4f(110.0, -2160.0, 175.0, 130.0);
const T2: vec4f = vec4f(470.0, -2210.0, 100.0, 88.0);
const T3: vec4f = vec4f(612.0, -2250.0, 44.0, 40.0);
const TOWER_BASE: f32 = 120.0;

// one octave of a jagged skyline: steep-sided peaks in cells of width cw
fn peaks(x: f32, cw: f32, amp: f32, slope: f32, salt: u32) -> f32 {
    let c0 = floor(x / cw);
    var top = -1e4;
    for (var i = -1; i <= 1; i++) {
        let c = c0 + f32(i);
        let hu = hash_u(bitcast<u32>(i32(c)) ^ salt);
        let hv = hash_u(hu);
        let h = vec3f(f32(hu & 0xffffu), f32(hu >> 16u), f32(hv & 0xffffu)) * (1.0 / 65536.0);
        let cx = (c + 0.2 + 0.6 * h.x) * cw;
        top = max(top, amp * (0.55 + 0.45 * h.y) - abs(x - cx) * slope * (0.7 + 0.6 * h.z));
    }
    return top;
}

// a talus cone: concave, steepest right under the wall
fn cone(xz: vec2f, c: vec2f, r: vec2f, h: f32) -> f32 {
    let s = max(1.0 - length((xz - c) / r), 0.0);
    return h * s * (0.35 + 0.65 * s);
}

// pasture, the basin, the grassy shoulders and the scree under the towers
fn terrain_h(xz: vec2f, oct: i32) -> f32 {
    let x = xz.x;
    let d = max(-xz.y, 0.0);
    var h = -28.0 * smoothstep(0.0, 520.0, d) + 150.0 * pow(smoothstep(650.0, 1950.0, d), 1.6);
    // talus cones under the north faces and out of the gullies between them
    if (d > 1350.0) {
        h += cone(xz, vec2f(-320.0, -2070.0), vec2f(420.0, 340.0), 190.0);
        h += cone(xz, vec2f(110.0, -2150.0), vec2f(460.0, 380.0), 200.0);
        h += cone(xz, vec2f(480.0, -2200.0), vec2f(330.0, 340.0), 170.0);
        // scree gullies raking down the cones
        h -= 7.0 * sq(1.0 - abs(snoise(vec2f(x * 0.03, xz.y * 0.006)))) * smoothstep(1500.0, 1800.0, d);
    }
    // the plateau falls away beyond the towers
    h -= 400.0 * smoothstep(2500.0, 3600.0, d);
    // a grassy ridge on the right, rocky shoulders on the left
    h += 170.0 * smoothstep(700.0, 2200.0, x) * smoothstep(300.0, 1400.0, d) * sstep(3200.0, 2400.0, d);
    // rolling pasture
    h += 12.0 * snoise(xz * 0.004) + 5.0 * snoise(xz * 0.013 + 3.0);
    if (oct > 1) { h += 1.2 * snoise(xz * 0.06 + 7.0); }
    return h;
}

// one facet's offset at bed height bf: each facet steps at its own period,
// so a ledge runs along one or two faces, never round the whole tower
fn facet_off(bf: f32, i: u32, salt: u32) -> f32 {
    let x = bf / (1.7 + f32(i % 3u) * 1.1) + f32(i) * 0.37;
    let b = floor(x);
    let s = salt ^ (i * 0x85ebca6bu);
    let h0 = hash_f((bitcast<u32>(i32(b)) * 0x9e3779b9u) ^ s);
    let h1 = hash_f((bitcast<u32>(i32(b) + 1) * 0x9e3779b9u) ^ s);
    return 0.8 + 0.2 * mix(h0, h1, smoothstep(0.86, 1.0, x - b));
}

// a faceted convex plan (q in units of the half extents): seven irregular
// half-planes
fn facets(q: vec2f, bf: f32, salt: u32) -> f32 {
    var d = dot(q, vec2f(0.9903, 0.1392)) - facet_off(bf, 0u, salt);
    d = max(d, dot(q, vec2f(0.6157, 0.788)) - facet_off(bf, 1u, salt));
    d = max(d, dot(q, vec2f(-0.0523, 0.9986)) - facet_off(bf, 2u, salt));
    d = max(d, dot(q, vec2f(-0.6561, 0.7547)) - facet_off(bf, 3u, salt));
    d = max(d, dot(q, vec2f(-0.9994, 0.0349)) - facet_off(bf, 4u, salt));
    d = max(d, dot(q, vec2f(-0.5299, -0.848)) - facet_off(bf, 5u, salt));
    d = max(d, dot(q, vec2f(0.5, -0.866)) - facet_off(bf, 6u, salt));
    return d;
}

// a tower: beds of faceted plan, tapering, fluted, with a crenellated top
fn tower(p: vec3f, t: vec4f, tall: f32, k: f32, lod: i32) -> f32 {
    let q = p - vec3f(t.x, TOWER_BASE, t.y);
    let y = saturate(q.y / tall);
    // the towers are not plumb
    let qx = q.xz - vec2f(10.0 * snoise(vec2f(q.y * 0.003, k)), 0.0);
    // beds ~50 m thick, dipping gently across the tower
    let bf = q.y / 50.0 + qx.x * 0.002 + 0.25 * snoise(vec2f(qx.x * 0.008 + k, k));
    let salt = u32(k * 97.0) * 0x2545f491u;
    let qn = rot2(k * 0.4 - 1.2) * (qx / t.zw) * (1.0 + 0.2 * y);
    var pd = facets(qn, bf, salt) * min(t.z, t.w);
    // chimneys and flutes running down the faces
    let s = atan2(qx.x, qx.y) * (t.z + t.w) * 0.5;
    let g = snoise(vec2f(s * 0.019 + k * 3.0, q.y * 0.002 + k));
    pd += 9.0 * pow(1.0 - abs(g), 6.0) + 1.5 * g;
    if (lod > 1) {
        let g2 = snoise(vec2f(s * 0.06 + k, q.y * 0.006));
        pd += 2.5 * pow(1.0 - abs(g2), 6.0);
    }
    // a broken, crenellated summit: blocks and teeth
    let r2 = dot(qx / t.zw, qx / t.zw);
    let top = tall - 45.0 * r2 + 28.0 * snoise(qx * 0.009 + k) + max(peaks(s, 32.0, 46.0, 2.4, salt), -6.0) - 22.0;
    return max(pd * 0.55, (q.y - top) * 0.38);
}

// a tower behind its own bounding box: the noise only runs close by
fn tower_b(p: vec3f, t: vec4f, tall: f32, k: f32, lod: i32) -> f32 {
    let q = p - vec3f(t.x, TOWER_BASE, t.y);
    let b = max(sdf2_box(q.xz, t.zw * 1.3 + 30.0), q.y - tall - 50.0);
    if (b > 30.0) { return b; }
    return tower(p, t, tall, k, lod);
}
fn towers(p: vec3f, lod: i32) -> f32 {
    var d = tower_b(p, T1, 640.0, 1.0, lod);
    d = min(d, tower_b(p, T0, 600.0, 4.0, lod));
    d = min(d, tower_b(p, T2, 530.0, 7.0, lod));
    d = min(d, tower_b(p, T3, 430.0, 9.0, lod));
    return d;
}

// the jagged Paterno ridge on the left: a slab with a crown of spires
fn ridge_l(p: vec3f, lod: i32) -> f32 {
    let zf = -1750.0 - 0.28 * (p.x + 700.0);
    let back = zf - p.z;
    let top = 330.0 + 0.1 * (p.x + 2000.0) + peaks(p.x, 260.0, 140.0, 1.6, 0x71u) + max(peaks(p.x, 70.0, 45.0, 2.8, 0x93u), -20.0);
    // buttresses: planar facets between sharp arêtes and gullies
    let bx = p.x / 120.0 + 0.6 * snoise(vec2f(p.x * 0.004, p.y * 0.002));
    let gl = 1.0 - abs(snoise(vec2f(p.x * 0.02, p.y * 0.003)));
    var df = -back + (p.y - 150.0) * 0.35 + 22.0 * tri(bx) + 10.0 * snoise(vec2f(p.x * 0.015, p.y * 0.004)) + 10.0 * gl * gl * gl;
    // broken rock: cracks and blocks, for the normals
    if (lod > 1) { df += 6.0 * abs(snoise(vec2f(p.x * 0.045 + p.z * 0.02, p.y * 0.012))) + 3.0 * snoise(vec2f(p.x * 0.09, p.y * 0.05)); }
    return op_smax(max(df, back - 600.0), p.y - top, 20.0) * 0.6;
}

// the hut: a rifugio, stone ground floor, timber above, a steep roof
const HUT: vec2f = vec2f(-48.0, -178.0);
fn hut_local(p: vec3f, gy: f32) -> vec3f {
    let q0 = p - vec3f(HUT.x, gy, HUT.y);
    return vec3f(q0.x * 0.87 - q0.z * 0.5, q0.y, q0.x * 0.5 + q0.z * 0.87);
}
fn hut(p: vec3f, gy: f32) -> vec2f {
    let q = hut_local(p, gy);
    let walls = sdf_box(q - vec3f(0.0, 2.5, 0.0), vec3f(8.0, 5.5, 5.0));
    let rq = vec3f(q.x, q.y - 8.0, abs(q.z));
    let roof = max(max(abs(rq.x) - 8.6, dot(rq.yz, vec2f(0.811, 0.584)) - 3.0), -rq.y - 0.3);
    return select(vec2f(walls, 5.0), vec2f(roof, 6.0), roof < walls);
}

// everything but the terrain → (distance, material): 2 tower rock, 5 hut
// wall, 6 roof; 0 marks a conservative bound, never a surface
fn objects(p: vec3f, lod: i32) -> vec2f {
    var res = vec2f(1e5, 0.0);
    // the towers, behind a bounding box
    let bq = abs(p.xz - vec2f(150.0, -2165.0)) - vec2f(720.0, 330.0);
    let bd = length(max(bq, vec2f(0.0)));
    if (bd < 60.0) {
        res = vec2f(towers(p, lod), 2.0);
    } else {
        res.x = bd - 30.0;
    }
    // the ridge on the left
    let rfront = p.z - (-1750.0 - 0.28 * (p.x + 700.0));
    if (p.x < -450.0 && rfront < 450.0 && p.y > 60.0) {
        let rl = ridge_l(p, lod);
        if (rl < res.x) { res = vec2f(rl, 2.0); }
    } else {
        let rb = max(max((p.x + 450.0) * 0.8, (rfront - 380.0) * 0.55), (60.0 - p.y) * 0.5) + 20.0;
        if (rb < res.x) { res = vec2f(rb, 0.0); }
    }
    // the hut
    let hq = p.xz - HUT;
    if (dot(hq, hq) < 400.0) {
        let hb = hut(p, terrain_h(HUT, 2) - 0.5);
        if (hb.x < res.x) { res = hb; }
    } else if (length(hq) - 16.0 < res.x) {
        res = vec2f(length(hq) - 16.0, 0.0);
    }
    return res;
}

// → (distance, material): 1 terrain, else as objects()
fn sdf(p: vec3f, lod: i32) -> vec2f {
    if (p.y > 820.0) { return vec2f(p.y - 790.0, 0.0); }
    let o = objects(p, lod);
    let tr = (p.y - terrain_h(p.xz, lod)) * 0.72;
    return select(o, vec2f(tr, 1.0), tr < o.x);
}

// Sphere tracing the objects, and the ground as a heightfield: over the
// open pasture (gentle) a ray may step its height over the steepest slope,
// far longer than a distance bound would let it; a secant between the last
// two samples lands it.
fn trace(ro: vec3f, rd: vec3f, n: i32, ctx: Ctx) -> vec2f {
    var t = 1.0;
    let foot = ctx.px * 0.4;
    let lxz = length(rd.xz);
    var tp = t;
    var dyp = 1e4;
    for (var i = 0; i < 200; i++) {
        if (i >= n) { break; }
        let q = ro + rd * t;
        if (q.y > 820.0 && rd.y > 0.0) { return vec2f(-1.0, 0.0); }
        // past the towers there is only sky
        if (q.z < -3300.0 && rd.z < 0.0) { return vec2f(-1.0, 0.0); }
        let dy = q.y - terrain_h(q.xz, 1);
        if (dy < foot * t) {
            if (dy < 0.0 && dyp < 1e4) { return vec2f(tp + (t - tp) * dyp / (dyp - dy), 1.0); }
            return vec2f(t, 1.0);
        }
        let o = objects(q, 1);
        if (o.x < foot * t && o.y > 0.5) { return vec2f(t, o.y); }
        let g = select(1.4, 0.5, q.z > -1550.0);
        let den = g * lxz - rd.y;
        let st = min(select(1e4, dy / den, den > 1e-4), o.x);
        tp = t;
        dyp = dy;
        t += max(st, foot * t * 0.7);
        if (t > 20000.0) { break; }
    }
    // out of steps grazing a crest: call it the crest
    if (dyp < 0.03 * tp) { return vec2f(tp, 1.0); }
    return vec2f(-1.0, 0.0);
}

fn nrm(p: vec3f, t: f32, ctx: Ctx) -> vec3f {
    let e = max(0.02, t * ctx.px * 0.5);
    let k = vec2f(1.0, -1.0);
    return normalize(
        k.xyy * sdf(p + k.xyy * e, 2).x + k.yyx * sdf(p + k.yyx * e, 2).x +
        k.yxy * sdf(p + k.yxy * e, 2).x + k.xxx * sdf(p + k.xxx * e, 2).x);
}

fn shadow(ro: vec3f, l: vec3f) -> f32 {
    var res = 1.0;
    var t = 2.0;
    for (var i = 0; i < 15; i++) {
        let p = ro + l * t;
        if (p.y > 800.0) { break; }
        let h = sdf(p, 0).x;
        res = min(res, 8.0 * h / t);
        if (res < 0.01) { break; }
        t += clamp(h, 3.0 + t * 0.06, 400.0);
    }
    return saturate(res);
}

// ------------------------------------------------------------ sky

fn sky(rd: vec3f, l: Look, ctx: Ctx, full: bool) -> vec3f {
    if (l.night > 0.5) {
        var c = sky_night(rd) * 1.2;
        if (full) {
            let sr = star_rotate(rd, 46.5, (ctx.t - 1800.0) * 2.0);
            // the summer Milky Way arching up from behind the wall
            let pole = normalize(vec3f(0.92, 0.2, 0.33));
            let core = normalize(vec3f(-0.3, 0.12, -0.95));
            c += star_milky_way(sr, pole, core, ctx) * 2.6 * smoothstep(0.0, 0.2, rd.y);
            c += star_field(sr, 0.9, ctx) * smoothstep(0.0, 0.12, rd.y);
        }
        return c;
    }
    var c = sky_atmosphere_haze(rd, l.sun, l.haze);
    if (full) { c += sky_sun_disk(rd, l.sun, 0.5); }
    // the model has no ozone: its Chappuis absorption keeps a low-sun sky
    // blue and rose instead of olive
    return c * ozone(l.sun.y);
}

fn ozone(sun_y: f32) -> vec3f {
    let am = inverseSqrt(sun_y * sun_y + 0.00786);
    return exp(-vec3f(0.0195, 0.0564, 0.00256) * am);
}

// fair-weather cumulus, a cheap sheet
fn cumulus(c_in: vec3f, rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    if (rd.y < 0.0 || l.night > 0.5) { return c_in; }
    let hp = rd.xz / (rd.y + 0.08) * 1.6 + vec2f(ctx.t * 0.0025, ctx.t * 0.0008);
    let q = hp * vec2f(0.9, 1.5) + 2.0;
    let n = noise_fbm2(q, 5);
    let cov = saturate((n - 0.55) * 3.2) * smoothstep(0.02, 0.12, rd.y);
    if (cov <= 0.0) { return c_in; }
    let n2 = noise_fbm2(q + normalize(l.sun.xz + vec2f(1e-4)) * 0.05, 5);
    let edge = saturate(0.5 + (n - n2) * 5.0);
    let sunc = select(l.sun_c, l.sun_c * 0.25, l.sun.y < 0.1);
    let lit = sunc * (0.35 + 0.6 * edge) * 0.8 + l.amb * mix(1.0, 0.7, cov);
    return mix(c_in, lit, cov * 0.9);
}

// the horizon colour in the ray's azimuth
fn horizon(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    return sky(normalize(vec3f(rd.x, 0.04, rd.z)), l, ctx, false);
}

fn aerial(col: vec3f, rd: vec3f, t: f32, l: Look, hz: vec3f) -> vec3f {
    var c = mix(col, hz, 1.0 - exp(-t * 0.00005 * l.haze));
    if (l.night < 0.5) { c += fog_sun(rd, l.sun, l.sun_c, 0.15 * (1.0 - exp(-t * 0.0002))); }
    return c;
}

// haze lying in the basin under the towers: it lifts the far scree and the
// towers' feet off the nearer ground and fills the gaps between them
fn basin_mist(col: vec3f, y: f32, t: f32, l: Look, hz: vec3f) -> vec3f {
    let a = l.mist * smoothstep(700.0, 2000.0, t) * exp(-max(y - 150.0, 0.0) / 120.0);
    // (at first light the basin air is in shadow, lit by the blue sky)
    let fc = select(hz * 1.05, l.amb * 0.75 + hz * 0.15, l.lit_line > 0.0);
    return mix(col, fc, saturate(a));
}

// ------------------------------------------------------------ surfaces

// the trail: up from the foreground past the hut and on toward the towers
fn trail_x(z: f32) -> f32 {
    return mix(14.0, -28.0, smoothstep(-20.0, -178.0, z)) + mix(0.0, 110.0, smoothstep(-178.0, -1000.0, z)) + 12.0 * sin(z * 0.011);
}
// shadows of the fair-weather cumulus drifting over the ground
fn cumulus_shade(xz: vec2f, l: Look, ctx: Ctx) -> f32 {
    if (l.sun.y < 0.3) { return 1.0; }
    let n = noise_fbm2(xz * 0.0009 + vec2f(ctx.t * 0.004, ctx.t * 0.0015) + 3.0, 3);
    return 1.0 - 0.6 * smoothstep(0.47, 0.62, n);
}

fn shade(p: vec3f, rd: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = nrm(p, t, ctx);
    var alb: vec3f;
    var occ = 1.0;
    let gy = terrain_h(HUT, 2) - 0.5;
    if (mat > 5.5) {
        // dark shingles
        alb = col_hex(0x6e6a64u) * (0.8 + 0.2 * noise_value2(vec2f(p.x + p.z, p.y * 6.0)));
    } else if (mat > 4.5) {
        // whitewashed stone below, weathered larch boards above
        let hq = hut_local(p, gy);
        let stone = col_hex(0xc4bdb0u) * (0.85 + 0.15 * noise_value2(p.xz * 3.0 + p.y * 2.0));
        let wood = col_hex(0x6b4a30u) * (0.8 + 0.2 * noise_value2(vec2f((p.x + p.z) * 2.0, p.y * 7.0)));
        alb = mix(stone, wood, step(4.4, hq.y));
    } else if (mat > 1.5) {
        // dolomite: pale grey, bedded, streaked by water: black streaks
        // where it runs, yellow-orange scars where rock fell away
        let u = p.x - 0.45 * p.z;
        let band = 0.88 + 0.2 * noise_value2(vec2f(u * 0.003, p.y * 0.05));
        alb = col_hex(0xcbc6bcu) * band * (0.9 + 0.2 * noise_value2(vec2f(u * 0.05, p.y * 0.02)));
        let yel = smoothstep(0.55, 0.78, noise_fbm2(vec2f(u * 0.011, p.y * 0.007) + 7.0, 3));
        alb = mix(alb, col_hex(0xcf9c5eu), yel * 0.75);
        let blk = smoothstep(0.62, 0.86, noise_value2(vec2f(u * 0.075, p.y * 0.0032 + 0.3 * noise_value2(vec2f(u * 0.01, 2.0)))));
        alb = mix(alb, col_hex(0x3c3a37u), blk * 0.8 * sstep(0.6, 0.2, n.y));
        // grey lichen low on the walls, scree lying on the ledges
        alb = mix(alb, alb * vec3f(0.8, 0.8, 0.78), smoothstep(380.0, 200.0, p.y) * 0.5);
        alb = mix(alb, col_hex(0xa9a59cu), smoothstep(0.6, 0.9, n.y) * 0.7);
        // occlusion in the chimneys and under the overhangs
        let a1 = sdf(p + n * 9.0, 1).x;
        let a2 = sdf(p + n * 28.0, 1).x;
        occ = saturate(0.35 + 0.65 * (saturate(a1 / 9.0) * 0.45 + saturate(a2 / 28.0) * 0.55));
        occ *= 0.7 + 0.3 * saturate(n.y + 0.6);
    } else {
        // pasture → scree with height and slope
        let slope = 1.0 - n.y;
        let g1 = noise_fbm2(p.xz * 0.012, 4);
        var grass = mix(col_hex(0x4b5a2cu), col_hex(0x77783fu), g1);
        // tussocks, dwarf pine and rock outcrops break up the pasture
        grass = mix(grass, col_hex(0x2c3d1eu), smoothstep(0.62, 0.75, noise_fbm2(p.xz * 0.035 + 5.0, 3)) * 0.8);
        grass = mix(grass, col_hex(0x8f897au), smoothstep(0.72, 0.8, noise_fbm2(p.xz * 0.05 + 11.0, 3)) * 0.8);
        grass *= 0.8 + 0.35 * noise_value2(p.xz * 0.3) * (0.6 + 0.4 * noise_value2(p.xz * 1.7));
        // darker clumps of sedge and dwarf shrubs, close up
        grass = mix(grass, col_hex(0x3a4a22u), smoothstep(0.5, 0.8, noise_value2(rot2(0.5) * p.xz * 0.45 + 17.0) * 0.65 + noise_value2(p.xz * 1.3) * 0.35) * 0.45 * smoothstep(400.0, 60.0, t));
        // sun-bleached tufts and brown patches of old grass
        grass = mix(grass, col_hex(0x7a6c48u), smoothstep(0.55, 0.8, noise_fbm2(p.xz * 0.02 + 21.0, 3)) * 0.5);
        // scree: raked into streaks down the fall line, fresh and pale in
        // the gullies, darker and lichened between, boulders at the foot
        let streak = noise_value2(vec2f(p.x * 0.07, p.z * 0.012));
        var scree = mix(col_hex(0x77736bu), col_hex(0xa29d92u), smoothstep(0.3, 0.75, streak));
        scree *= 0.8 + 0.3 * noise_value2(p.xz * 0.35) + 0.15 * noise_value2(p.xz * 1.3);
        let rocky = saturate(smoothstep(40.0, 120.0, p.y + 60.0 * (g1 - 0.5)) + smoothstep(0.45, 0.7, slope));
        // grass tongues climbing the cone feet
        let tongue = smoothstep(0.55, 0.3, noise_fbm2(vec2f(p.x * 0.012, p.z * 0.004), 3)) * sstep(230.0, 150.0, p.y);
        alb = mix(grass, scree, rocky * (1.0 - tongue * 0.8));
        // the trail: worn earth and limestone grit
        let fp = t * ctx.px * 0.56;
        let w = 0.75;
        let tr = saturate((w + 0.5 * fp - abs(p.x - trail_x(p.z))) / fp) * min(1.0, 2.0 * w / fp) * smoothstep(-1100.0, -700.0, p.z);
        alb = mix(alb, col_hex(0x9c917au) * (0.85 + 0.3 * noise_value2(p.xz * 2.0)), tr * 0.85);
        occ = 0.85 + 0.15 * n.y;
    }
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    // (only the rock and the scree under it are near enough to cast on)
    if (dif > 0.0 && l.night < 0.5 && p.z < -1200.0) { sh = shadow(p + n * (0.5 + t * ctx.px * 1.5), l.sun); }
    sh *= cumulus_shade(p.xz, l, ctx);
    // first light only reaches above the shadow of the ridges to the east,
    // a ragged line across the towers
    if (l.lit_line > 0.0) {
        let line = l.lit_line + 0.07 * p.x + 45.0 * snoise(vec2f(p.x * 0.004, 1.5)) + 18.0 * snoise(vec2f(p.x * 0.02, 4.0));
        sh *= smoothstep(line - 25.0, line + 25.0, p.y);
    }
    // sky light: blue from above, a warm bounce off the lit scree and meadow
    let bounce = select(vec3f(0.0), l.sun_c * max(l.sun.y, 0.0) * 0.12 * saturate(-n.y * 0.5 + 0.5), l.night < 0.5);
    var c = alb * (l.sun_c * dif * sh + (l.amb * (0.55 + 0.45 * n.y) * 1.2 + bounce) * occ);
    if (l.night > 0.5) {
        let lp = vec3f(HUT.x, gy + 2.5, HUT.y) + vec3f(4.0, 0.0, 7.5);
        c += alb * light_point(p, n, lp, col_kelvin(3000.0) * 90.0, 4.0) * light_flicker(ctx.t, 7u, 0.4);
    }
    if (mat > 4.5 && mat < 5.5) {
        // the hut's windows: dark by day, lamplight at night
        let hq = hut_local(p, gy);
        let along = select(hq.x, hq.z, abs(hq.x) > 7.9);
        let row = step(abs(hq.y - 2.0), 0.6) + step(abs(hq.y - 5.9), 0.55);
        let win = step(abs(fract(along / 3.2 + 0.5) - 0.5), 0.14) * row * step(abs(along), 6.8);
        let lamp = select(0.0, 1.0, l.night > 0.5) * light_flicker(ctx.t, 7u, 0.4);
        c = mix(c, col_kelvin(3000.0) * 10.0 * lamp + c * 0.2 * (1.0 - lamp), win);
    }
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(0.0, terrain_h(vec2f(0.0), 2) + 9.0, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.03, 0.1, -1.0), 0.0, 31.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    let hit = trace(ro, rd, steps(110.0, ctx), ctx);
    if (hit.x > 0.0) {
        let hz = horizon(rd, l, ctx);
        let hp = ro + rd * hit.x;
        col = aerial(shade(hp, rd, hit.x, hit.y, l, ctx), rd, hit.x, l, hz);
        col = basin_mist(col, hp.y, hit.x, l, hz);
    } else {
        col = cumulus(sky(rd, l, ctx, true), rd, l, ctx);
        // (the basin haze only reaches the sky low down)
        if (rd.y < 0.2) { col = basin_mist(col, ro.y + rd.y * 3200.0, 3200.0, l, horizon(rd, l, ctx)); }
    }
    // the lamplit windows glow a little into the night air
    if (l.night > 0.5) {
        let hp = cam_project(cam, vec3f(HUT.x, terrain_h(HUT, 2) + 2.5, HUT.y));
        let d = length(p - hp.xy) / (7.0 * cam.zoom / hp.z);
        col += col_kelvin(3400.0) * (0.02 * exp(-d * d * 0.5)) * light_flicker(ctx.t, 7u, 0.4);
    }
    return col * exp2(l.exposure);
}
