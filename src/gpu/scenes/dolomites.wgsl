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
// them on the left.

struct Look {
    sun: vec3f,
    night: f32,
    haze: f32,
    cloud: f32,      // cloud drifting along the wall
    lit_line: f32,   // first light: height above which the low sun reaches
    exposure: f32,
    sun_c: vec3f,
    amb: vec3f,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        // first light from the east, over the camera's right shoulder
        case 0u: { l = Look(sky_sun_dir(2.5, 118.0), 0.0, 1.5, 0.6, 150.0, 0.1, vec3f(0.0), vec3f(0.0)); }
        case 1u: { l = Look(sky_sun_dir(48.0, 145.0), 0.0, 1.2, 0.85, -1e4, -0.5, vec3f(0.0), vec3f(0.0)); }
        default: { l = Look(sky_sun_dir(-25.0, 120.0), 1.0, 1.0, 0.3, -1e4, 1.1, vec3f(0.0), vec3f(0.0)); }
    }
    if (l.night > 0.5) {
        // moonless: starlight and airglow only
        l.sun_c = vec3f(0.0);
        l.amb = vec3f(0.008, 0.011, 0.02);
    } else {
        l.sun_c = sky_sun_light(l.sun);
        l.amb = sky_ambient(l.sun);
        if (l.sun.y < 0.1) {
            // enrosadira: first light turns the pale rock rose and orange
            l.sun_c *= vec3f(1.6, 1.0, 0.9) * 2.4;
            l.amb *= 1.5;
        }
    }
    return l;
}

// ------------------------------------------------------------ geometry

fn snoise(p: vec2f) -> f32 { return noise_value2(p) * 2.0 - 1.0; }

// Tre Cime di Lavaredo, north faces: (x, z) of each tower's centre, plan
// half extents, height above the scree. Cima Ovest, Cima Grande, Cima
// Piccola and its needle, the Piccolissima.
const T0: vec4f = vec4f(-320.0, -2080.0, 150.0, 120.0);
const T1: vec4f = vec4f(110.0, -2160.0, 175.0, 135.0);
const T2: vec4f = vec4f(470.0, -2210.0, 95.0, 90.0);
const T3: vec4f = vec4f(610.0, -2260.0, 45.0, 45.0);
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

// pasture, the basin, the grassy shoulders and the scree under the towers
fn terrain_h(xz: vec2f, oct: i32) -> f32 {
    let x = xz.x;
    let d = max(-xz.y, 0.0);
    var h = -28.0 * smoothstep(0.0, 520.0, d) + 150.0 * pow(smoothstep(650.0, 1950.0, d), 1.6);
    // talus cones under the north faces
    h += 95.0 * max(1.0 - length((xz - vec2f(-300.0, -1860.0)) / vec2f(430.0, 330.0)), 0.0);
    h += 110.0 * max(1.0 - length((xz - vec2f(120.0, -1920.0)) / vec2f(480.0, 360.0)), 0.0);
    h += 80.0 * max(1.0 - length((xz - vec2f(520.0, -1990.0)) / vec2f(380.0, 320.0)), 0.0);
    // the plateau falls away beyond the towers
    h -= 400.0 * smoothstep(2500.0, 3600.0, d);
    // a grassy ridge on the right, rocky shoulders on the left
    h += 170.0 * smoothstep(700.0, 2200.0, x) * smoothstep(300.0, 1400.0, d) * sstep(3200.0, 2400.0, d);
    // rolling pasture
    h += 12.0 * snoise(xz * 0.004) + 5.0 * snoise(xz * 0.013 + 3.0);
    if (oct > 1) { h += 1.2 * snoise(xz * 0.06 + 7.0); }
    return h;
}

// a tower: tapering rounded block, vertical cracks, a rough domed summit
fn tower(p: vec3f, t: vec4f, tall: f32, k: f32, lod: i32) -> f32 {
    let q = p - vec3f(t.x, TOWER_BASE, t.y);
    let y = saturate(q.y / tall);
    // tapering, with shoulders and steps where layers broke away
    let step_out = 1.0 + 0.1 * snoise(vec2f(q.y * 0.0035 + k, k * 1.7)) - 0.3 * y;
    let half = t.zw * step_out;
    // the plan drifts a little with height: the towers are not plumb
    let qx = q.xz - vec2f(12.0 * snoise(vec2f(q.y * 0.003, k)), 0.0);
    var pd = sdf2_round_box(qx, half, min(half.x, half.y) * 0.4);
    // vertical cracks and chimneys
    pd += 8.0 * snoise(vec2f((q.x - q.z * 0.5) * 0.02 + k, q.y * 0.0022));
    if (lod > 1) {
        pd += 2.5 * snoise(vec2f((q.x - q.z * 0.5) * 0.07 + k, q.y * 0.008));
    }
    // a rough, notched summit dome
    let r2 = dot(qx / t.zw, qx / t.zw);
    let dome = tall - 90.0 * r2 + 45.0 * snoise(qx * 0.01 + k) + 25.0 * snoise(qx * 0.03 + k * 2.0);
    return max(pd, (q.y - dome) * 0.55);
}

// a tower behind its own bounding box: the noise only runs close by
fn tower_b(p: vec3f, t: vec4f, tall: f32, k: f32, lod: i32) -> f32 {
    let q = p - vec3f(t.x, TOWER_BASE, t.y);
    let b = max(sdf2_box(q.xz, t.zw * 1.1 + 35.0), q.y - tall - 70.0);
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
fn ridge_l(p: vec3f) -> f32 {
    let zf = -1750.0 - 0.28 * (p.x + 700.0);
    let back = zf - p.z;
    let top = 330.0 + 0.1 * (p.x + 2000.0) + peaks(p.x, 260.0, 140.0, 1.6, 0x71u) + max(peaks(p.x, 70.0, 45.0, 2.8, 0x93u), -20.0);
    let df = -back + (p.y - 150.0) * 0.35 + 20.0 * snoise(vec2f(p.x * 0.015, p.y * 0.004));
    return op_smax(max(df, back - 600.0), p.y - top, 20.0) * 0.6;
}

// the hut, 7 x 5 m, logs under a steep shingle roof
const HUT: vec2f = vec2f(-80.0, -250.0);
fn hut(p: vec3f, gy: f32) -> vec2f {
    let q0 = p - vec3f(HUT.x, gy, HUT.y);
    let q = vec3f(q0.x * 0.9 - q0.z * 0.44, q0.y, q0.x * 0.44 + q0.z * 0.9);
    let walls = sdf_box(q - vec3f(0.0, 1.8, 0.0), vec3f(5.0, 3.0, 3.6));
    let rq = vec3f(q.x, q.y - 4.8, abs(q.z));
    let roof = max(max(abs(rq.x) - 5.8, dot(rq.yz, vec2f(0.72, 0.69)) - 2.9), -rq.y - 1.1);
    return select(vec2f(walls, 5.0), vec2f(roof, 6.0), roof < walls);
}

// → (distance, material): 1 terrain, 2 tower rock, 5 hut wall, 6 roof
fn sdf(p: vec3f, lod: i32) -> vec2f {
    if (p.y > 820.0) { return vec2f(p.y - 790.0, 0.0); }
    let th = terrain_h(p.xz, lod);
    var res = vec2f((p.y - th) * 0.75, 1.0);
    // the towers, behind a bounding box
    let bq = abs(p.xz - vec2f(150.0, -2160.0)) - vec2f(720.0, 330.0);
    let bd = length(max(bq, vec2f(0.0)));
    if (bd < 60.0) {
        let tw = towers(p, lod);
        if (tw < res.x) { res = vec2f(tw, 2.0); }
    } else {
        res.x = min(res.x, bd - 30.0);
    }
    // the ridge on the left
    let rfront = p.z - (-1750.0 - 0.28 * (p.x + 700.0));
    if (p.x < -450.0 && rfront < 450.0 && p.y > 60.0) {
        let rl = ridge_l(p);
        if (rl < res.x) { res = vec2f(rl, 2.0); }
    } else {
        res.x = min(res.x, max(max((p.x + 450.0) * 0.8, (rfront - 380.0) * 0.55), (60.0 - p.y) * 0.5) + 20.0);
    }
    // the hut
    let hq = p.xz - HUT;
    if (dot(hq, hq) < 400.0) {
        let hb = hut(p, terrain_h(HUT, 2) - 0.5);
        if (hb.x < res.x) { res = hb; }
    } else {
        res.x = min(res.x, length(hq) - 16.0);
    }
    return res;
}

fn trace(ro: vec3f, rd: vec3f, n: i32, ctx: Ctx) -> vec2f {
    var t = 1.0;
    let foot = ctx.px * 0.4;
    var h = vec2f(1e9, 0.0);
    var w = 1.4;
    var last_r = 0.0;
    var last_step = 0.0;
    for (var i = 0; i < 200; i++) {
        if (i >= n) { break; }
        let q = ro + rd * t;
        if (q.y > 820.0 && rd.y > 0.0) { return vec2f(-1.0, 0.0); }
        // past the towers there is only sky
        if (q.z < -3300.0 && rd.z < 0.0) { return vec2f(-1.0, 0.0); }
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
        if (t > 20000.0) { break; }
    }
    if (h.x < 0.03 * t) { return vec2f(t, h.y); }
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
    for (var i = 0; i < 18; i++) {
        let p = ro + l * t;
        if (p.y > 800.0) { break; }
        let h = sdf(p, 0).x;
        res = min(res, 8.0 * h / t);
        if (res < 0.01) { break; }
        t += clamp(h, 3.0 + t * 0.04, 400.0);
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
            c += star_milky_way(sr, pole, core, ctx) * 1.8 * smoothstep(0.0, 0.2, rd.y);
            c += star_field(sr, 0.9, ctx) * smoothstep(0.0, 0.12, rd.y);
        }
        return c;
    }
    var c = sky_atmosphere_haze(rd, l.sun, l.haze);
    if (full) { c += sky_sun_disk(rd, l.sun, 0.5); }
    // (no ozone in the model: keep a low-sun sky blue, not teal)
    if (l.sun.y < 0.1) { c *= vec3f(0.95, 0.86, 1.15); }
    return c;
}

// fair-weather cumulus, a cheap sheet
fn cumulus(c_in: vec3f, rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    if (rd.y < 0.0 || l.night > 0.5) { return c_in; }
    let hp = rd.xz / (rd.y + 0.08) * 1.6 + vec2f(ctx.t * 0.0025, ctx.t * 0.0008);
    let q = hp * vec2f(0.9, 1.5) + 2.0;
    let n = noise_fbm2(q, 5);
    let cov = saturate((n - 0.55) * 3.2) * smoothstep(0.02, 0.12, rd.y);
    let n2 = noise_fbm2(q + normalize(l.sun.xz + vec2f(1e-4)) * 0.05, 5);
    let edge = saturate(0.5 + (n - n2) * 5.0);
    let sunc = select(l.sun_c, l.sun_c * 0.25, l.sun.y < 0.1);
    let lit = sunc * (0.35 + 0.6 * edge) * 0.8 + l.amb * mix(1.0, 0.7, cov);
    return mix(c_in, lit, cov * 0.9);
}

// cloud drifting along the wall at half height, marched only across the
// slab in front of the face
fn wall_cloud(ro: vec3f, rd: vec3f, t_end: f32, l: Look, ctx: Ctx) -> vec4f {
    if (l.cloud < 0.01 || rd.y <= 0.0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let t0 = max((300.0 - ro.y) / rd.y, 1300.0);
    let t1 = min(min((700.0 - ro.y) / rd.y, t_end), 3600.0);
    if (t1 <= t0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let dt = (t1 - t0) / 12.0;
    var t = t0 + dt * ctx.jitter;
    var tr = 1.0;
    var acc = vec3f(0.0);
    let mu = dot(rd, l.sun);
    let ph = 0.7 + 1.8 * pow(saturate(mu), 5.0);
    for (var i = 0; i < 12; i++) {
        let p = ro + rd * t;
        let hb = exp(-sq((p.y - 450.0) / 120.0)) * exp(-sq((p.z + 2250.0) / 300.0));
        if (hb > 0.03) {
            let q = (p + vec3f(ctx.t * 5.0, ctx.t * 0.2, ctx.t * 0.6)) * vec3f(1.0 / 300.0, 1.0 / 90.0, 1.0 / 260.0);
            let d = saturate((noise_fbm3(q, 3) - 0.7 + 0.2 * l.cloud) * 5.0) * hb;
            if (d > 0.003) {
                let hf = saturate((p.y - 370.0) / 200.0);
                var lc = l.amb * (0.7 + 0.5 * hf) + l.sun_c * ph * (0.3 + 0.7 * hf) * 0.7;
                if (l.night > 0.5) { lc = l.amb * 1.5; }
                let st = exp(-d * 0.02 * dt);
                acc += tr * lc * (1.0 - st);
                tr *= st;
            }
        }
        t += dt;
    }
    return vec4f(acc, tr);
}

fn aerial(col: vec3f, ro: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let hz = sky(normalize(vec3f(rd.x, 0.04, rd.z)), l, ctx, false);
    var c = mix(col, hz, 1.0 - exp(-t * 0.00005 * l.haze));
    if (l.night < 0.5) { c += fog_sun(rd, l.sun, l.sun_c, 0.15 * (1.0 - exp(-t * 0.0002))); }
    return c;
}

// ------------------------------------------------------------ surfaces

fn shade(p: vec3f, rd: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = nrm(p, t, ctx);
    var alb: vec3f;
    var occ = 1.0;
    if (mat > 5.5) {
        alb = col_hex(0x625e57u) * (0.8 + 0.2 * noise_value2(vec2f(p.x + p.z, p.y * 6.0)));
    } else if (mat > 4.5) {
        // weathered larch logs
        alb = col_hex(0x7a5436u) * (0.75 + 0.25 * noise_value2(vec2f((p.x + p.z) * 2.0, p.y * 7.0)));
    } else if (mat > 1.5) {
        // dolomite: pale, bedded in horizontal layers, streaked by rain
        let bed = 0.88 + 0.12 * smoothstep(0.3, 0.7, fract(p.y / 31.0 + 0.3 * snoise(vec2f(p.x * 0.008, 1.0))));
        let streak = noise_fbm2(vec2f((p.x - p.z * 0.5) * 0.04, p.y * 0.004), 4);
        alb = col_hex(0xd2c9b8u) * bed * (0.78 + 0.4 * streak);
        // dark lichen and water stains running down
        alb = mix(alb, col_hex(0x6e6a62u), smoothstep(0.6, 0.8, noise_fbm2(vec2f((p.x - p.z * 0.5) * 0.06, p.y * 0.0018), 4)) * 0.55);
        // warm iron tint in patches
        alb = mix(alb, col_hex(0xd8ae84u), smoothstep(0.55, 0.8, noise_value2(vec2f(p.x * 0.012, p.y * 0.004))) * 0.4);
        // scree on the ledges
        alb = mix(alb, col_hex(0xbdb8adu), smoothstep(0.75, 0.95, n.y) * 0.6);
        occ = 0.65 + 0.35 * saturate(n.y + 0.5);
    } else {
        // pasture → scree with height and slope
        let slope = 1.0 - n.y;
        let g1 = noise_fbm2(p.xz * 0.012, 4);
        var grass = mix(col_hex(0x4a5e2au), col_hex(0x6f7a40u), g1);
        // tussocks, dwarf pine and rock outcrops break up the pasture
        grass = mix(grass, col_hex(0x2f4220u), smoothstep(0.62, 0.75, noise_fbm2(p.xz * 0.035 + 5.0, 3)) * 0.8);
        grass = mix(grass, col_hex(0x9a927eu), smoothstep(0.72, 0.8, noise_fbm2(p.xz * 0.05 + 11.0, 3)) * 0.8);
        grass *= 0.85 + 0.3 * noise_value2(p.xz * 0.3);
        let scree = col_hex(0xaaa498u) * (0.75 + 0.35 * noise_value2(vec2f(p.x * 0.08, p.z * 0.02)) + 0.15 * noise_value2(p.xz * 0.4));
        let rocky = saturate(smoothstep(40.0, 110.0, p.y + 50.0 * (g1 - 0.5)) + smoothstep(0.45, 0.7, slope));
        alb = mix(grass, scree, rocky);
    }
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    // (only the rock and the scree under it are near enough to cast on)
    if (dif > 0.0 && l.night < 0.5 && p.z < -1200.0) { sh = shadow(p + n * 0.5, l.sun); }
    // first light only reaches above the shadow of the ridges to the east
    if (l.lit_line > 0.0) { sh *= smoothstep(l.lit_line - 80.0, l.lit_line + 120.0, p.y - 0.08 * p.x); }
    var c = alb * (l.sun_c * dif * sh + l.amb * (0.65 + 0.35 * n.y) * occ * 1.2);
    if (l.night > 0.5) {
        let lp = vec3f(HUT.x, terrain_h(HUT, 2) + 2.0, HUT.y) + vec3f(1.5, 0.0, 5.0);
        c += alb * light_point(p, n, lp, col_kelvin(3000.0) * 60.0, 4.0) * light_flicker(ctx.t, 7u, 0.4);
    }
    if (mat > 4.5 && mat < 5.5) {
        // the hut's windows: dark by day, lamplight at night
        let hq = p - vec3f(HUT.x, terrain_h(HUT, 2), HUT.y);
        let u = hq.x * 0.9 - hq.z * 0.44;
        let win = step(abs(u - 1.2), 1.1) * step(abs(hq.y - 2.1), 0.7);
        let lamp = select(0.0, 1.0, l.night > 0.5) * light_flicker(ctx.t, 7u, 0.4);
        c = mix(c, col_kelvin(3000.0) * 14.0 * lamp + c * 0.25 * (1.0 - lamp), win);
    }
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(0.0, terrain_h(vec2f(0.0), 2) + 9.0, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.03, 0.1, -1.0), 0.0, 31.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    var t_hit = 1e6;
    let hit = trace(ro, rd, steps(110.0, ctx), ctx);
    if (hit.x > 0.0) {
        t_hit = hit.x;
        col = aerial(shade(ro + rd * hit.x, rd, hit.x, hit.y, l, ctx), ro, rd, hit.x, l, ctx);
    } else {
        col = cumulus(sky(rd, l, ctx, true), rd, l, ctx);
    }
    let wc = wall_cloud(ro, rd, t_hit, l, ctx);
    col = col * wc.a + wc.rgb;
    // the lamplit window glows a little into the night air
    if (l.night > 0.5) {
        let hp = cam_project(cam, vec3f(HUT.x, terrain_h(HUT, 2) + 1.8, HUT.y));
        let d = length(p - hp.xy) / (5.0 * cam.zoom / hp.z);
        col += col_kelvin(3400.0) * (0.03 * exp(-d * d * 0.5) + 0.25 * exp(-d * d * 8.0)) * light_flicker(ctx.t, 7u, 0.4);
    }
    return col * exp2(l.exposure);
}
