//! name: fjord
//! title: Norwegian Fjord
//! category: wilds
//! tags: fjord, waterfall, mountains, snow, aurora, norway
//! desc: sheer walls dropping into a still green fjord as a ferry draws its wake past the falls
//! themes: summer, winter, overcast
//! uses: camera, sdf, sky, fog, stars
//! cost: heavy
//! fallback: alpine
//! credits: original

// World units are metres, y up, the fjord's surface at y = 0. The camera
// stands 150 m up at the head of the fjord (Geiranger) looking down its
// winding length: rock walls rise at 70-80 degrees to a snowy plateau
// 1100-1400 m up, the Seven Sisters fall off the left wall, and a ferry
// heads out, its Kelvin wake spreading behind it across the mirror.

struct Look {
    sun: vec3f,
    night: f32,      // 1 = polar twilight with aurora
    overcast: f32,
    haze: f32,
    cloud: f32,      // low cloud on the walls
    snow: f32,       // snow line lowered to the water
    exposure: f32,
    sun_c: vec3f,
    amb: vec3f,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: { l = Look(sky_sun_dir(-7.0, -40.0), 1.0, 0.0, 1.0, 0.25, 1.0, 1.1, vec3f(0.0), vec3f(0.0)); }
        case 2u: { l = Look(sky_sun_dir(22.0, -30.0), 0.0, 0.85, 1.3, 0.7, 0.0, -0.35, vec3f(0.0), vec3f(0.0)); }
        default: { l = Look(sky_sun_dir(36.0, -60.0), 0.0, 0.1, 1.2, 0.3, 0.0, -0.2, vec3f(0.0), vec3f(0.0)); }
    }
    if (l.night > 0.5) {
        l.sun_c = vec3f(0.0);
        l.amb = vec3f(0.012, 0.018, 0.035);
    } else {
        let sl = sky_sun_light(l.sun);
        let sa = sky_ambient(l.sun);
        l.sun_c = sl * (1.0 - 0.85 * l.overcast);
        l.amb = mix(sa, vec3f(col_luma(sa + sl * 0.12)) * vec3f(0.94, 0.98, 1.04), l.overcast) * (1.0 + 0.5 * l.overcast);
    }
    return l;
}

// ------------------------------------------------------------ geometry

fn snoise(p: vec2f) -> f32 { return noise_value2(p) * 2.0 - 1.0; }
// signed 1D value noise: two integer hashes (for things that vary along z)
fn n1(x: f32) -> f32 {
    let i = floor(x);
    let f = x - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash_f(bitcast<u32>(i32(i)));
    let b = hash_f(bitcast<u32>(i32(i) + 1));
    return mix(a, b, u) * 2.0 - 1.0;
}

// the fjord winds: centre line and half width along its length
fn fj_x(z: f32) -> f32 { return 520.0 * sin(z * 0.00038 + 0.2) + 160.0 * sin(z * 0.0011 + 1.0) - 150.0; }
fn fj_w(z: f32) -> f32 { return 380.0 + 110.0 * sin(z * 0.0007 + 2.0); }

// plateau height on either side
fn plateau(z: f32, side: f32) -> f32 {
    return 1150.0 + 220.0 * sin(z * 0.0004 + side * 1.7) + 140.0 * n1(z * 0.0009 + side * 31.0);
}

// terrain height and its steepness bound: (height, |gradient| estimate)
fn terrain(xz: vec2f, lod: i32) -> vec2f {
    let dx = xz.x - fj_x(xz.y);
    let side = sign(dx);
    // spurs and bays along the shore
    let d = abs(dx) - fj_w(xz.y) + 130.0 * n1(xz.y * 0.0022 + side * 57.0) + 40.0 * n1(xz.y * 0.008 + side * 91.0);
    let top = plateau(xz.y, side);
    let L = 230.0 + 60.0 * n1(xz.y * 0.0012 + side * 13.0);
    let e = exp(-max(d, 0.0) / L);
    var h = top * (1.0 - e);
    var g = top / L * e;
    // glacial benches: the wall steps back in ledges
    let bench = h / 150.0 + 0.4 * n1(xz.y * 0.002 + side * 71.0);
    h += 38.0 * (smoothstep(0.35, 0.65, fract(bench)) - 0.5) * smoothstep(40.0, 200.0, d);
    // below the waterline the walls keep plunging
    h = select(h, d * 3.0, d < 0.0);
    // gullies and buttresses running down the fall line
    let gul = 1.0 - abs(snoise(vec2f(xz.y * 0.006, d * 0.0012 + side * 5.0)));
    h -= 110.0 * gul * gul * smoothstep(0.0, 120.0, d) * (1.0 - smoothstep(700.0, 1500.0, d));
    // knobs and peaks on the plateau
    if (d > 300.0) { h += 180.0 * snoise(xz * 0.0011 + side * 9.0) * smoothstep(300.0, 1200.0, d); }
    if (lod > 0) {
        let gul2 = 1.0 - abs(snoise(vec2f(xz.y * 0.018, d * 0.003 + side * 2.0)));
        h -= 35.0 * gul2 * gul2 * smoothstep(0.0, 60.0, d) * (1.0 - smoothstep(500.0, 1000.0, d));
        h += 20.0 * snoise(xz * 0.006);
    }
    if (lod > 1) { h += 7.0 * snoise(xz * 0.025 + 3.0); }
    return vec2f(h, g + 2.2);
}

// the ferry: hull and superstructure, heading down-fjord
fn ferry_pos(t: f32) -> vec4f {
    let period = 1100.0;
    let u = fmod_pos(t + 300.0, period) / period;
    let z = -350.0 - u * 4200.0;
    // fade in as it sails into view, out as it vanishes down the fjord
    let vis = smoothstep(0.0, 0.03, u) * (1.0 - smoothstep(0.9, 1.0, u));
    return vec4f(fj_x(z) + 40.0, 0.0, z, vis);
}
fn ferry(p: vec3f, f: vec4f) -> f32 {
    let q = p - f.xyz;
    let hull = sdf_round_box(q - vec3f(0.0, 2.5, 0.0), vec3f(10.0, 4.5, 50.0), 3.0);
    let deck = sdf_box(q - vec3f(0.0, 10.5, 6.0), vec3f(8.0, 4.0, 28.0));
    return min(hull, deck);
}

// → (distance, material): 1 terrain, 3 ferry
fn sdf(p: vec3f, f: vec4f, lod: i32) -> vec2f {
    if (p.y > 1650.0) { return vec2f(p.y - 1600.0, 0.0); }
    let tr = terrain(p.xz, lod);
    var res = vec2f((p.y - tr.x) / sqrt(1.0 + tr.y * tr.y) * 0.9, 1.0);
    let fq = p - f.xyz;
    if (f.w > 0.01) {
        if (dot(fq, fq) < 6400.0) {
            let fd = ferry(p, f);
            if (fd < res.x) { res = vec2f(fd, 3.0); }
        } else {
            res.x = min(res.x, length(fq) - 70.0);
        }
    }
    return res;
}

fn trace(ro: vec3f, rd: vec3f, tmax: f32, n: i32, f: vec4f, lod: i32, ctx: Ctx) -> vec2f {
    var t = 2.0;
    let foot = ctx.px * 0.4;
    var h = vec2f(1e9, 0.0);
    var w = 1.3;
    var last_r = 0.0;
    var last_step = 0.0;
    for (var i = 0; i < 200; i++) {
        if (i >= n) { break; }
        let q = ro + rd * t;
        if (q.y > 1650.0 && rd.y > 0.0) { return vec2f(-1.0, 0.0); }
        h = sdf(q, f, lod);
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
    if (h.x < 0.02 * t) { return vec2f(t, h.y); }
    return vec2f(-1.0, 0.0);
}

fn nrm(p: vec3f, t: f32, f: vec4f, ctx: Ctx) -> vec3f {
    let e = max(0.05, t * ctx.px * 0.5);
    let k = vec2f(1.0, -1.0);
    return normalize(
        k.xyy * sdf(p + k.xyy * e, f, 2).x + k.yyx * sdf(p + k.yyx * e, f, 2).x +
        k.yxy * sdf(p + k.yxy * e, f, 2).x + k.xxx * sdf(p + k.xxx * e, f, 2).x);
}

fn shadow(ro: vec3f, l: vec3f, f: vec4f) -> f32 {
    var res = 1.0;
    var t = 8.0;
    for (var i = 0; i < 12; i++) {
        let p = ro + l * t;
        if (p.y > 1600.0) { break; }
        let h = sdf(p, f, 0).x;
        res = min(res, 8.0 * h / t);
        if (res < 0.01) { break; }
        t += clamp(h, 15.0 + t * 0.06, 600.0);
    }
    return saturate(res);
}

// ------------------------------------------------------------ sky

fn aurora(rd: vec3f, ctx: Ctx) -> vec3f {
    if (rd.y < 0.0) { return vec3f(0.0); }
    // curtains hanging along an arc across the northern sky
    let a = atan2(rd.x, -rd.z);
    let el = rd.y;
    let arc = 0.2 + 0.08 * sin(a * 1.7 + ctx.t * 0.02) + 0.04 * sin(a * 4.3 - ctx.t * 0.035);
    let band = exp(-sq((el - arc) / 0.06)) + 0.5 * exp(-sq((el - arc - 0.1) / 0.12));
    let rays = 0.55 + 0.45 * noise_value2(vec2f(a * 40.0 + ctx.t * 0.05, 0.5));
    let fold = 0.6 + 0.4 * sin(a * 9.0 + ctx.t * 0.04 + 3.0 * noise_value2(vec2f(a * 3.0, ctx.t * 0.01)));
    let top = sstep(arc + 0.25, arc, el);
    let c = mix(vec3f(0.1, 1.0, 0.45), vec3f(0.5, 0.2, 0.8), smoothstep(arc + 0.05, arc + 0.2, el));
    return c * band * rays * fold * top * 0.05 * smoothstep(-0.6, 0.4, -a);
}

fn sky(rd: vec3f, l: Look, ctx: Ctx, full: bool) -> vec3f {
    if (l.night > 0.5) {
        let y = max(rd.y, 0.0);
        // polar twilight: a deep blue sky, paler toward the hidden sun
        var c = mix(vec3f(0.03, 0.05, 0.11), vec3f(0.006, 0.012, 0.035), pow(y, 0.5));
        c += vec3f(0.04, 0.035, 0.05) * pow(saturate(dot(normalize(vec3f(rd.x, 0.0, rd.z)), normalize(vec3f(l.sun.x, 0.0, l.sun.z))) * 0.5 + 0.5), 4.0) * exp(-y * 6.0);
        if (full) {
            c += star_field(rd, 0.6, ctx) * smoothstep(0.05, 0.3, rd.y) * 0.6;
        }
        c += aurora(rd, ctx);
        return c;
    }
    var c = sky_atmosphere_haze(rd, l.sun, l.haze);
    let grey = vec3f(col_luma(c)) * vec3f(0.95, 0.98, 1.03);
    c = mix(c, grey * 1.15 + l.amb * 0.2, l.overcast * 0.9);
    // (no disk, and so no glint on the water, through an overcast)
    if (full && l.overcast < 0.5) { c += sky_sun_disk(rd, l.sun, 0.5) * (1.0 - l.overcast); }
    // cumulus, or the overcast's lumpy base (not in the haze colour: at a
    // fixed elevation it would streak)
    if (full && rd.y > -0.02) {
        let hp = rd.xz / (rd.y + 0.07) * 1.5 + vec2f(ctx.t * 0.002, ctx.t * 0.0007);
        let n = noise_fbm2(hp * vec2f(0.8, 1.3) + 5.0, 5);
        let covk = mix(0.6, 0.3, l.overcast);
        let cov = saturate((n - covk) * 3.0) * smoothstep(-0.02, 0.1, rd.y);
        let n2 = noise_fbm2(hp * vec2f(0.8, 1.3) + 5.0 + normalize(l.sun.xz + vec2f(1e-4)) * 0.05, 5);
        let lit = l.amb * mix(1.0, 0.7, cov) + l.sun_c * (0.3 + 0.6 * saturate(0.5 + (n - n2) * 5.0)) * 0.8;
        c = mix(c, lit, cov * 0.9);
    }
    return c;
}

// the low cloud clinging to the walls: (light, transmittance)
fn wall_cloud(ro: vec3f, rd: vec3f, t_end: f32, l: Look, ctx: Ctx) -> vec4f {
    if (l.cloud < 0.01) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    var t0 = 900.0;
    var t1 = min(t_end, 9000.0);
    // the band: 60..250 m up, below the viewpoint
    if (abs(rd.y) > 1e-4) {
        let ta = (60.0 - ro.y) / rd.y;
        let tb = (250.0 - ro.y) / rd.y;
        t0 = max(t0, min(ta, tb));
        t1 = min(t1, max(ta, tb));
    }
    if (t1 <= t0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    t1 = min(t1, t0 + 5000.0);
    let dt = (t1 - t0) / 11.0;
    var t = t0 + dt * ctx.jitter;
    var tr = 1.0;
    var acc = vec3f(0.0);
    let ph = 0.8 + 1.2 * pow(saturate(dot(rd, l.sun)), 4.0);
    for (var i = 0; i < 11; i++) {
        let p = ro + rd * t;
        let d = abs(p.x - fj_x(p.z)) - fj_w(p.z);
        // hugging the walls: most dense just off the rock
        let cling = exp(-sq((d + 40.0) / 150.0));
        let hb = exp(-sq((p.y - 155.0) / 55.0)) * cling;
        if (hb > 0.03) {
            let q = (p + vec3f(ctx.t * 1.2, 0.0, -ctx.t * 2.5)) * vec3f(1.0 / 420.0, 1.0 / 55.0, 1.0 / 700.0);
            let den = saturate((noise_fbm3(q, 3) - 0.62 + 0.2 * l.cloud) * 4.0) * hb;
            if (den > 0.003) {
                let hf = saturate((p.y - 100.0) / 110.0);
                var lc = l.amb * (0.65 + 0.55 * hf) + l.sun_c * ph * (0.25 + 0.6 * hf) * 0.7;
                if (l.night > 0.5) { lc = l.amb * 1.4 + vec3f(0.0, 0.004, 0.002); }
                let st = exp(-den * 0.012 * dt);
                acc += tr * lc * (1.0 - st);
                tr *= st;
            }
        }
        t += dt;
    }
    return vec4f(acc, tr);
}

fn aerial(col: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    // cool, blue-grey distance (the model's khaki horizon is its lack of ozone)
    let hz = sky(normalize(vec3f(rd.x, 0.07, rd.z)), l, ctx, false) * select(vec3f(0.92, 0.97, 1.08), vec3f(1.0), l.night > 0.5);
    var c = mix(col, hz * 0.95, 1.0 - exp(-t * 0.00007 * l.haze));
    if (l.night < 0.5) { c += fog_sun(rd, l.sun, l.sun_c, 0.15 * (1.0 - exp(-t * 0.0002))); }
    return c;
}

// ------------------------------------------------------------ surfaces

fn rock_col(p: vec3f, n: vec3f, t: f32, l: Look, f: vec4f, shadows: bool, ctx: Ctx) -> vec3f {
    let slope = 1.0 - n.y;
    let n1 = noise_fbm2(p.xz * 0.004 + p.y * 0.001, 4);
    // dark wet gneiss, streaked
    var alb = mix(col_hex(0x282a2cu), col_hex(0x4c4c4au), n1);
    alb *= 0.75 + 0.4 * noise_value2(vec2f((p.x + p.z) * 0.02, p.y * 0.006));
    // dark seeps and lichen streaks running down the fall line
    let u = p.x + p.z;
    alb *= 1.0 - 0.45 * smoothstep(0.55, 0.8, noise_value2(vec2f(u * 0.035, p.y * 0.0025)));
    alb *= 0.85 + 0.3 * noise_value2(vec2f(u * 0.15, p.y * 0.01));
    // birch and pine wherever the slope lets them hold, up to ~700 m
    // (they hold on surprisingly steep ground: every ledge and gully)
    let patchy = noise_value2(p.xz * 0.012 + p.y * 0.01);
    let veg = sstep(0.97, 0.78, slope + 0.3 * (patchy - 0.5)) * (1.0 - smoothstep(550.0, 850.0, p.y + 200.0 * n1));
    let wood = mix(col_hex(0x18261au), col_hex(0x34462au), noise_value2(p.xz * 0.02) * 0.6 + n1 * 0.4);
    alb = mix(alb, wood, veg * (1.0 - l.snow * 0.75) * smoothstep(8.0, 40.0, p.y));
    // bare fell above the trees: grey-olive tundra and rock
    let fell = smoothstep(650.0, 950.0, p.y + 150.0 * n1) * sstep(0.6, 0.3, slope);
    alb = mix(alb, mix(col_hex(0x4a4c40u), col_hex(0x6a6a5au), n1), fell * 0.8);
    // snow: on the plateau, and on every ledge in winter
    let line = mix(1000.0, -50.0, l.snow);
    let snow = smoothstep(line - 80.0, line + 80.0, p.y + 180.0 * (n1 - 0.5)) * sstep(0.75, 0.35, slope + 0.2 * l.snow * (1.0 - n1));
    alb = mix(alb, vec3f(0.8, 0.83, 0.88), snow);
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    if (shadows && dif > 0.0 && l.night < 0.5 && l.overcast < 0.9) { sh = shadow(p + n * (3.0 + t * ctx.px * 1.5), l.sun, f); }
    var c = alb * (l.sun_c * dif * sh * 1.2 + l.amb * (0.6 + 0.4 * n.y) * 1.2);
    if (l.night > 0.5) { c += alb * vec3f(0.0, 0.02, 0.01) * snow; }
    return c;
}

fn ferry_col(p: vec3f, n: vec3f, l: Look, f: vec4f, ctx: Ctx) -> vec3f {
    let q = p - f.xyz;
    var alb = vec3f(0.75);
    // dark hull stripe and a row of windows on the superstructure
    alb = mix(alb, col_hex(0x1c2230u), step(q.y, 3.0));
    let win = step(8.5, q.y) * step(q.y, 11.0) * step(0.5, fract(q.z * 0.2));
    alb = mix(alb, vec3f(0.05), win * 0.8);
    var c = alb * (l.sun_c * saturate(dot(n, l.sun)) + l.amb * 1.1);
    if (l.night > 0.5) { c += col_kelvin(3000.0) * 1.5 * win; }
    return c * f.w + vec3f(0.0);
}

// the ferry's wake on the water: (normal tilt x, z, foam)
fn wake(xz: vec2f, f: vec4f) -> vec3f {
    if (f.w < 0.01) { return vec3f(0.0); }
    let r = xz - f.xz;
    let behind = r.y;                      // it heads toward -z
    if (behind < -40.0 || behind > 2200.0) { return vec3f(0.0); }
    let lat = r.x;
    let s = max(behind, 1.0);
    // Kelvin arms at 19.5 degrees, fading with distance
    let arm = abs(lat) - 0.354 * s;
    let aw = 3.0 + 0.008 * s;
    let fade = exp(-s / 320.0) * f.w;
    let arms = exp(-sq(arm / aw)) * fade;
    // transverse ripples between the arms
    let inside = sstep(0.0, -20.0, arm) * fade * 0.4;
    let trans = inside * sin(behind * 0.12);
    // churned track right behind the stern
    let track = exp(-sq(lat / (5.0 + 0.02 * s))) * exp(-s / 350.0) * f.w;
    return vec3f(sign(lat) * arms * 0.035, trans * 0.02, track * 0.8 + arms * 0.03);
}

fn fjord_water(p: vec3f, rd: vec3f, t: f32, l: Look, f: vec4f, ctx: Ctx) -> vec3f {
    // nearly still: faint cat's-paws only
    let e = 0.6;
    let cat = smoothstep(0.55, 0.8, noise_value2(p.xz * 0.004 + vec2f(ctx.t * 0.003, 0.0)));
    let wn = vec2f(snoise(p.xz * 0.12 + vec2f(ctx.t * 0.08, 0.0)), snoise(p.xz * 0.12 + vec2f(3.0, ctx.t * 0.07))) * (0.002 + 0.01 * cat);
    let wk = wake(p.xz, f);
    let n = normalize(vec3f(wn.x + wk.x, 1.0, wn.y + wk.y));
    let r = reflect(rd, n);
    var refl: vec3f;
    // the reflected world: a cheaper trace of the walls
    let h = trace(p + vec3f(0.0, 0.5, 0.0), r, 16000.0, steps(34.0, ctx), f, 0, ctx);
    if (h.x > 0.0) {
        let hp = p + r * h.x;
        let hn = nrm(hp, h.x + t, f, ctx);
        var c: vec3f;
        if (h.y > 2.5) { c = ferry_col(hp, hn, l, f, ctx); } else { c = rock_col(hp, hn, h.x + t, l, f, false, ctx); }
        refl = aerial(c, r, h.x, l, ctx);
    } else {
        refl = sky(r, l, ctx, true);
    }
    let fres = 0.02 + 0.98 * pow(1.0 - saturate(dot(n, -rd)), 5.0);
    // deep glacial green below
    var body = col_hex(0x0f3a36u) * (l.amb * 0.9 + l.sun_c * 0.05);
    if (l.night > 0.5) { body = vec3f(0.002, 0.006, 0.008); }
    var c = mix(body, refl, saturate(fres * 1.3 + 0.25));
    c = mix(c, (l.amb + l.sun_c * 0.5) * 0.9, saturate(wk.z) * 0.6);
    return c;
}

// the Seven Sisters: threads of water down the left wall
fn falls(p: vec2f, cam: Cam, t_hit: f32, l: Look, ctx: Ctx) -> vec4f {
    let z = -2350.0;
    let x0 = fj_x(z) - fj_w(z) - 30.0;
    let a = cam_project(cam, vec3f(x0, 330.0, z));
    let b = cam_project(cam, vec3f(x0 + 10.0, 5.0, z + 10.0));
    if (a.z > t_hit + 150.0) { return vec4f(0.0); }
    let k = cam.zoom / a.z;
    let ab = b.xy - a.xy;
    let along = dot(p - a.xy, ab) / dot(ab, ab);
    if (along < -0.05 || along > 1.05) { return vec4f(0.0); }
    var dens = 0.0;
    for (var i = 0; i < 7; i++) {
        let fi = f32(i);
        let off = (fi - 3.0) * 22.0 * k + 3.0 * k * sin(fi * 2.1);
        let start = 0.08 * hash_f(u32(i) * 7u + 3u);
        let cx = a.x + ab.x * along + off;
        let w = (2.0 + 5.0 * along) * k;
        let s = sstep(w + ctx.px * 0.5, w * 0.3, abs(p.x - cx)) * smoothstep(start, start + 0.05, along);
        let fl = 0.6 + 0.4 * noise_value2(vec2f(fi * 3.0, along * 20.0 - ctx.t * 1.5));
        dens = max(dens, s * fl);
    }
    dens *= sstep(1.05, 0.9, along);
    let lit = l.amb * 1.6 + l.sun_c * 0.4;
    return vec4f(lit, saturate(dens * 0.8));
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let f = ferry_pos(ctx.t);
    let ro = vec3f(fj_x(900.0) + 30.0, 300.0, 900.0);
    let cam = cam_look_at(ro, ro + vec3f(-0.18, -0.08, -1.0), 0.0, 44.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    let tw = select(1e6, -ro.y / rd.y, rd.y < 0.0);
    let hit = trace(ro, rd, min(tw, 30000.0), steps(92.0, ctx), f, 1, ctx);
    var t_hit = 1e6;
    if (hit.x > 0.0) {
        t_hit = hit.x;
        let hp = ro + rd * hit.x;
        let n = nrm(hp, hit.x, f, ctx);
        var c: vec3f;
        if (hit.y > 2.5) { c = ferry_col(hp, n, l, f, ctx); } else { c = rock_col(hp, n, hit.x, l, f, true, ctx); }
        col = aerial(c, rd, hit.x, l, ctx);
    } else if (tw < 1e5) {
        t_hit = tw;
        col = aerial(fjord_water(ro + rd * tw, rd, tw, l, f, ctx), rd, tw, l, ctx);
    } else {
        col = sky(rd, l, ctx, true);
    }
    let fa = falls(p, cam, t_hit, l, ctx);
    if (fa.a > 0.0) { col = mix(col, aerial(fa.rgb, rd, 2500.0, l, ctx), fa.a); }
    let wc = wall_cloud(ro, rd, t_hit, l, ctx);
    col = col * wc.a + wc.rgb;
    if (l.overcast > 0.5 && t_hit > 2500.0) {
        // a rain shower hanging down the fjord: a streaked grey veil
        let rc = cam_project(cam, vec3f(fj_x(-5200.0), 300.0, -5200.0));
        let across = (p.x - rc.x) / 0.22;
        let top = rc.y + 0.12;
        let veil = exp(-across * across) * sstep(top, top - 0.08, p.y) * smoothstep(3000.0, 6000.0, t_hit);
        let streak = 0.7 + 0.3 * noise_value2(vec2f(p.x * 90.0 + p.y * 12.0, p.y * 3.0 + ctx.t * 0.6));
        col = mix(col, l.amb * 1.05, saturate(veil * streak * 0.75));
    }
    return col * exp2(l.exposure);
}
