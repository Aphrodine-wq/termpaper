//! name: snowfall
//! title: Snowfall in the Pines
//! category: weather
//! tags: snow, forest, night, lamp, winter, pines
//! desc: an old lamp by a footpath in a snowy pine wood, its warm cone full of falling snow
//! themes: night, bluehour, blizzard
//! uses: camera, raymarch, noise
//! cost: heavy
//! fallback: frost
//! credits: original

// World units are metres, y up. The camera stands on a trodden footpath that
// winds into a pine wood; an old cast-iron lamp stands beside the path.
// Light: dim moonlight through thin cloud (blue), the lamp (warm point light
// with a cap, so nothing above it is lit), and the glow of the lamp in the
// snow-filled air, integrated analytically along each ray. Snowflakes are
// drawn in depth layers, each lit by the lamp where it actually is in 3D,
// and defocused like a lens focused on the lamp.

struct Look {
    moon: vec3f,      // direction to the moon
    moon_c: vec3f,    // moonlight
    amb: vec3f,       // sky ambient
    sky_hi: vec3f,
    sky_lo: vec3f,
    lamp_c: vec3f,    // lamp colour x intensity
    fog: f32,         // extinction per metre
    snow: f32,        // flake density
    wind: f32,        // m/s sideways
    fall: f32,        // m/s down
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            // blue hour: the last daylight, the lamp just lit
            l = Look(normalize(vec3f(-0.5, 0.35, -0.6)), vec3f(0.02, 0.03, 0.06), vec3f(0.035, 0.055, 0.11),
                vec3f(0.05, 0.08, 0.2), vec3f(0.1, 0.13, 0.24), col_kelvin(2300.0) * 5.0,
                0.022, 0.55, 0.25, 0.8, -0.2);
        }
        case 2u: {
            // blizzard: wind-driven snow, the lamp a glow in the murk
            l = Look(normalize(vec3f(-0.5, 0.6, -0.4)), vec3f(0.006, 0.008, 0.012), vec3f(0.012, 0.016, 0.024),
                vec3f(0.018, 0.022, 0.032), vec3f(0.03, 0.036, 0.048), col_kelvin(2100.0) * 7.0,
                0.12, 1.0, 5.5, 2.2, 0.0);
        }
        default: {
            // night: thin cloud, a hidden moon, steady snow
            l = Look(normalize(vec3f(-0.55, 0.5, -0.55)), vec3f(0.03, 0.042, 0.075), vec3f(0.006, 0.009, 0.018),
                vec3f(0.01, 0.014, 0.028), vec3f(0.024, 0.03, 0.05), col_kelvin(2100.0) * 7.0,
                0.035, 0.7, 0.35, 0.9, 0.0);
        }
    }
    return l;
}

const LAMP: vec3f = vec3f(1.1, 3.25, -8.5);   // lantern centre
const RO: vec3f = vec3f(0.0, 1.62, 0.0);
const TREE_W: f32 = 0.17;   // base radius per metre of height

fn path_x(z: f32) -> f32 { return 0.9 * sin(z * 0.06 + 0.4) - 0.35 + 0.8 * sin(z * 0.021 + 1.3) - 0.77; }

fn ground_h(xz: vec2f, fine: bool) -> f32 {
    var h = 0.2 * noise_grad2(xz * 0.11) + 0.05 * noise_grad2(xz * 0.47 + 3.0);
    let dx = xz.x - path_x(xz.y);
    // trodden path, with soft drifts along its edges
    h -= 0.1 * sstep(1.1, 0.2, abs(dx));
    h += 0.04 * exp(-sq((abs(dx) - 1.05) * 2.5));
    if (fine && abs(dx) < 0.7 && xz.y < 0.5 && xz.y > -30.0) {
        // footprints of two walkers; the older set half filled with snow
        for (var w = 0; w < 2; w++) {
            let s = select(0.76, 0.7, w == 1);
            let ph = select(0.0, 0.31, w == 1);
            let u = xz.y / s + ph;
            let k = floor(u);
            let side = select(-1.0, 1.0, (i32(k) & 1) == 0);
            let hk = hash_cell2(vec2i(i32(k), w), 0xf007u);
            let lat = side * (0.16 + 0.04 * hk.x) + select(-0.13, 0.15, w == 1) + (hk.y - 0.5) * 0.05;
            let q = vec2f((dx - lat) / 0.09, (fract(u) - 0.5) * s / 0.17);
            let dent = sstep(1.0, 0.35, length(q));
            // the toe kicks a little rim of snow forward
            let rim = exp(-sq((length(q) - 1.1) * 4.0)) * 0.3;
            h += (rim - dent) * select(0.07, 0.035, w == 1);
        }
    }
    return h;
}

// whorls every ~0.6 m of trunk
fn tiers_of(h: f32, seed: f32) -> f32 { return h / (0.5 + 0.22 * seed); }

// One spruce: a narrow cone of drooping branch whorls, each a star of
// branches sloping down from the trunk. q relative to the base.
fn pine(q: vec3f, h: f32, r0: f32, seed: f32, lod: i32) -> f32 {
    let rho = length(q.xz);
    let hn = saturate(q.y / h);
    // cheap bound: the widest the tree can be at this height
    let rmax = r0 * (1.0 - hn) * 1.22 + 0.2;
    let bound = max((rho - rmax) * 0.9, q.y - h - 0.1);
    if (bound > 0.2) { return bound; }
    let u = hn * tiers_of(h, seed) + seed * 3.7;
    let ty = fract(u);                  // 0 at the top of a whorl, 1 at its tips
    let env = r0 * (1.0 - hn);
    // droop: each whorl is a shallow cone widening downward
    var r = env * (0.58 + 0.42 * sqrt(ty));
    let tier_id = floor(u);
    let hw = hash_cell2(vec2i(i32(tier_id), i32(seed * 997.0)), 0xb4a7u);
    if (lod < 2) {
        let ang = atan2(q.z, q.x);
        let br = 0.5 + 0.5 * cos(ang * (5.0 + floor(hw.x * 4.0)) + hw.y * TAU);
        r *= (0.82 + 0.3 * hw.z) * (0.78 + 0.22 * br) + 0.22 * (noise_value2(vec2f(ang * 4.0 + seed * 40.0, q.y * 3.1)) - 0.5);
        // ragged branch tips
        if (lod == 0) { r += env * 0.12 * (noise_value2(vec2f(ang * 13.0 + seed * 9.0, q.y * 9.0)) - 0.5); }
    } else {
        // far away a whorl is just its average reach
        r *= (0.82 + 0.3 * hw.z) * 0.89;
    }
    var d = (rho - r) * 0.6;
    d = max(d, 0.6 - q.y);
    d = max(d, q.y - h);
    let trunk = max(rho - 0.14, q.y - h * 0.7);
    return min(d, trunk);
}

// Trees on a 5 m grid, jittered within the middle half of each cell and at
// most ~3.5 m wide, so the 2x2 cells nearest p always hold every tree that
// can reach it. None on the path, near the lamp or the camera.
const GRID: f32 = 5.0;

fn tree_cell(c: vec2i) -> vec4f {
    let hc = hash_cell2(c, 0x7ee5u);
    let pos = (vec2f(c) + 0.25 + 0.5 * hc.xy) * GRID;
    let h = 8.0 + 8.0 * hc.z;
    let r0 = TREE_W * h;
    if (hc.w > 0.92 || pos.y > 2.0) { return vec4f(0.0, 0.0, -1.0, 0.0); }
    let clear_path = abs(pos.x - path_x(pos.y));
    let near_lamp = length(pos - LAMP.xz);
    let near_cam = length(pos - RO.xz);
    if (clear_path < 1.4 + r0 || near_lamp < 1.2 + r0 || near_cam < 4.0 + r0) {
        return vec4f(0.0, 0.0, -1.0, 0.0);
    }
    return vec4f(pos, h, fract(hc.w * 7.31));
}

fn cell_of(p: vec3f, k: i32) -> vec2i {
    let g = p.xz / GRID;
    let c0 = vec2i(floor(g));
    let o = vec2i(select(-1, 1, fract(g.x) >= 0.5), select(-1, 1, fract(g.y) >= 0.5));
    return c0 + vec2i(select(0, o.x, (k & 1) == 1), select(0, o.y, (k & 2) == 2));
}

// where p sits inside the nearest tree: (position in its whorl 0 top .. 1
// tips, radial fraction, height fraction, found)
fn tree_local(p: vec3f) -> vec4f {
    var best = 1e5;
    var res = vec4f(0.0);
    for (var k = 0; k < 4; k++) {
        let tc = tree_cell(cell_of(p, k));
        if (tc.z < 0.0) { continue; }
        let q = p - vec3f(tc.x, 0.0, tc.y);
        let r0 = TREE_W * tc.z;
        let hn = saturate(q.y / tc.z);
        let rho = length(q.xz);
        let dd = rho - r0 * (1.0 - hn);
        if (dd < best) {
            best = dd;
            res = vec4f(fract(hn * tiers_of(tc.z, tc.w) + tc.w * 3.7), rho / max(r0 * (1.0 - hn), 0.05), hn, 1.0);
        }
    }
    return res;
}

fn trees(p: vec3f) -> f32 {
    // fine branch tips only where a pixel can resolve them
    let r2 = dot(p - RO, p - RO);
    let lod = select(select(2, 1, r2 < 1100.0), 0, r2 < 250.0);
    var d = 1e5;
    for (var k = 0; k < 4; k++) {
        let tc = tree_cell(cell_of(p, k));
        if (tc.z < 0.0) { continue; }
        let q = p - vec3f(tc.x, 0.0, tc.y);
        let r0 = TREE_W * tc.z;
        let bound = length(q.xz) - r0 * 1.22 - 0.2;
        if (bound > d) { continue; }
        d = min(d, pine(q, tc.z, r0, tc.w, lod));
    }
    return d;
}

fn lamp_sdf(p: vec3f) -> vec2f {
    let q = p - vec3f(LAMP.x, 0.0, LAMP.z);
    let post = max(length(q.xz) - 0.055 + 0.02 * saturate(q.y - 0.3), abs(q.y - 1.5) - 1.5);
    let base = max(length(q.xz) - 0.13, q.y - 0.5);
    let head = sdf_box(q - vec3f(0.0, 3.25, 0.0), vec3f(0.16, 0.22, 0.16));
    let cap = sdf_cone_y(q - vec3f(0.0, 3.56, 0.0), 0.1, 0.3, 0.03);
    var r = vec2f(min(min(post, base), cap), 3.0);
    r = op_umin(r, vec2f(head, 4.0));
    return r;
}

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    // the snow surface never rises above 0.35 m: skip its noise up high
    var g = p.y - 0.35;
    if (g < 0.6) { g = p.y - ground_h(p.xz, false); }
    var r = vec2f(g * 0.8, 1.0);
    if (p.y < 17.0) {
        r = op_umin(r, vec2f(trees(p), 2.0));
    } else {
        r = op_umin(r, vec2f(p.y - 16.5, 2.0));
    }
    // the lamp: exact near it (scaled to survive the march's over-relaxation),
    // a safe proxy cylinder elsewhere so no step can jump across it
    let dl = length(p.xz - LAMP.xz);
    if (dl < 1.0) {
        let lp = lamp_sdf(p);
        r = op_umin(r, vec2f(lp.x * 0.75, lp.y));
    } else {
        r = op_umin(r, vec2f(dl - 0.5, 3.0));
    }
    return r;
}

// The lantern's emission pattern: a reflector under the cap throws most of
// the light down in a broad cone; the panes leak a little sideways; nothing
// goes up. dn = unit direction from the lamp.
fn lamp_cone(dn: vec3f) -> f32 {
    let down = -dn.y;
    return sstep(-0.2, 0.05, down) * (0.09 + 0.91 * sstep(0.12, 0.85, down));
}

// lamp light reaching p (no normal term): inverse square times the cone
fn lamp_at(p: vec3f, l: Look) -> vec3f {
    let d = p - LAMP;
    let d2 = dot(d, d);
    return l.lamp_c * lamp_cone(d * inverseSqrt(max(d2, 1e-6))) / (d2 + 0.25);
}

// soft shadow toward the lamp: trees and the post (the snow surface is too
// gentle to matter and would band itself; the lantern cannot shadow itself)
fn lamp_shadow(p: vec3f, ctx: Ctx) -> f32 {
    let to = LAMP - p;
    let dist = length(to);
    let dir = to / dist;
    var res = 1.0;
    var t = 0.15;
    for (var i = 0; i < 18; i++) {
        if (t > dist - 1.0) { break; }
        let q = p + dir * t;
        var h = 1e5;
        if (q.y < 17.0) { h = trees(q); }
        h = min(h, max(length(q.xz - LAMP.xz) - 0.06, q.y - 2.4));
        res = min(res, 5.0 * h / t);
        if (res < 0.02) { break; }
        t += clamp(h, 0.08, 1.2);
    }
    return saturate(res);
}

// Single scattering of the lamp along ro + rd*[0, tmax], equiangular
// samples (dense where the ray passes the lamp) so the cone reads in the air.
fn inscatter(ro: vec3f, rd: vec3f, tmax: f32, ctx: Ctx) -> f32 {
    let tc = dot(LAMP - ro, rd);
    let h = sqrt(max(dot(LAMP - ro, LAMP - ro) - tc * tc, 0.0) + 0.02);
    let ta = atan(-tc / h);
    let tb = atan((tmax - tc) / h);
    let n = 14;
    var acc = 0.0;
    for (var i = 0; i < 14; i++) {
        let th = mix(ta, tb, (f32(i) + ctx.jitter) / f32(n));
        let t = tc + h * tan(th);
        let q = ro + rd * t;
        acc += lamp_cone(normalize(q - LAMP));
    }
    return acc * (tb - ta) / (f32(n) * h);
}

fn sky(rd: vec3f, l: Look) -> vec3f {
    let y = max(rd.y, 0.0);
    var c = mix(l.sky_lo, l.sky_hi, sqrt(y));
    // the moon behind thin cloud
    let m = saturate(dot(rd, l.moon));
    c += l.moon_c * (0.35 * pow(m, 30.0) + 0.08 * pow(m, 4.0));
    return c;
}

fn shade(p: vec3f, rd: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    var n: vec3f;
    var alb: vec3f;
    var occ = 1.0;
    if (mat < 1.5) {
        let e = max(0.01, t * ctx.px * 0.7);
        let fine = t < 16.0;
        let hx = ground_h(p.xz + vec2f(e, 0.0), fine) - ground_h(p.xz - vec2f(e, 0.0), fine);
        let hz = ground_h(p.xz + vec2f(0.0, e), fine) - ground_h(p.xz - vec2f(0.0, e), fine);
        n = normalize(vec3f(-hx, 2.0 * e, -hz));
        // trodden snow is a touch greyer
        let dx = abs(p.x - path_x(p.z));
        alb = mix(vec3f(0.82, 0.86, 0.92), vec3f(0.56, 0.58, 0.62), sstep(0.9, 0.3, dx));
    } else if (mat < 2.5) {
        n = rm_normal(p, t, ctx);
        // snow lies along the top of every whorl and on whatever faces the
        // sky, in clumps; dark needles elsewhere
        let tl = tree_local(p);
        let clump = noise_value2(vec2f(atan2(p.z, p.x) * 9.0 + p.y * 3.0, p.y * 4.0 + p.x));
        let band = sstep(0.62, 0.2, tl.x) * sstep(0.35, 0.8, tl.y);
        let s = saturate(max(band * sstep(0.25, 0.6, clump + 0.15), sstep(0.25, 0.6, n.y + 0.3 * (clump - 0.5))));
        alb = mix(vec3f(0.012, 0.02, 0.015), vec3f(0.78, 0.82, 0.88), s);
        // deep inside the crown and under each whorl it is dark
        occ = saturate(0.25 + 0.75 * sstep(0.3, 1.0, tl.y)) * (1.0 - 0.45 * sstep(0.6, 1.0, tl.x));
    } else {
        n = rm_normal(p, t, ctx);
        let s = sstep(0.4, 0.8, n.y);
        alb = mix(vec3f(0.02, 0.02, 0.022), vec3f(0.8, 0.83, 0.88), s);
        if (mat > 3.5) {
            // the lantern: glowing panes in an iron frame
            let lq = p - LAMP;
            let side = select(abs(lq.z), abs(lq.x), abs(lq.z) > abs(lq.x));
            let bars = max(sstep(0.12, 0.15, side), max(sstep(0.17, 0.2, abs(lq.y)), sstep(0.012, 0.0, abs(lq.y - 0.05))));
            return mix(l.lamp_c * 1.6, l.lamp_c * 0.03, bars);
        }
    }
    var c = alb * l.amb * (0.55 + 0.45 * n.y) * occ * 2.0;
    c += alb * l.moon_c * saturate(dot(n, l.moon)) * occ;
    let li = lamp_at(p, l) * mix(1.0, occ, 0.6);
    if (max3(li) > 0.015) {
        let ld = normalize(LAMP - p);
        let ndl = saturate(dot(n, ld));
        if (ndl > 0.0) {
            c += alb * li * ndl * lamp_shadow(p + n * 0.05, ctx);
        }
    }
    return c;
}

// Snowflakes in layers of depth along the view axis. Each layer is a plane
// at distance `d`; flakes sit on a world-size grid in it and fall.
fn flakes(ro: vec3f, rd: vec3f, cam: Cam, tmax: f32, l: Look, ctx: Ctx) -> vec3f {
    var acc = vec3f(0.0);
    let cosv = dot(rd, cam.fw);
    let n = select(10, 12, ctx.detail > 0u);
    let focus = 11.0;
    for (var i = 0; i < 12; i++) {
        if (i >= n) { break; }
        // geometric close to the lens, then even steps through the lamp light
        let d = select(3.0 + f32(i - 3) * 1.25, 0.75 * pow(1.58, f32(i)), i < 4);
        let t = d / cosv;
        if (t > tmax) { break; }
        let pw = ro + rd * t;
        let uv = vec2f(dot(pw - ro, cam.rt), dot(pw - ro, cam.up));
        let cs = 0.42;
        // fall and drift, with a slow sway per layer
        let sway = 0.25 * sin(ctx.t * 0.6 + f32(i) * 1.7) * (0.3 + l.wind * 0.1);
        let mv = vec2f(-l.wind * ctx.t + sway, l.fall * ctx.t);
        let g = (uv + mv) / cs;
        let c = vec2i(floor(g));
        let h = hash_cell2(c + vec2i(i * 7919, i * 131), 0x5e0fu);
        if (h.w > l.snow) { continue; }
        let fp = (vec2f(c) + 0.15 + 0.7 * h.xy) * cs - mv;
        var dv = uv - fp;
        // motion streaks when the wind drives the snow
        let vel = vec2f(-l.wind, -l.fall);
        let vn = normalize(vel);
        let along = dot(dv, vn);
        let streak = 1.0 + saturate(length(vel) * 0.02 / d) * 6.0 * saturate(l.wind * 0.3);
        dv = dv - vn * along + vn * along / streak;
        let pxw = ctx.px * d / cam.zoom;
        let rw = 0.008 + 0.012 * h.z;
        // defocus: the lens is focused on the lamp
        let blur = 0.011 * abs(1.0 - d / focus) * 1.5;
        let rt = sqrt(rw * rw + blur * blur + pxw * pxw * 0.4);
        let m = sstep(rt, rt * 0.35, length(dv)) * sq(rw / rt) * 1.8;
        if (m <= 0.001) { continue; }
        // lit where the flake really is
        let li = lamp_at(pw, l);
        let to_l = normalize(LAMP - pw);
        let fwd = 1.0 + 2.5 * pow(saturate(dot(-rd, -to_l)), 6.0);
        let lit = li * fwd * 0.9 + l.amb * 2.2 + l.moon_c * 0.6;
        let fogk = exp(-t * l.fog);
        acc += lit * m * fogk;
    }
    return acc;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let cam = cam_look_at(RO, vec3f(0.2, 2.6, -12.0), 0.0, 50.0);
    let rd = cam_ray(cam, p);
    let tmax = 90.0;
    let hit = rm_march_k(RO, rd, 0.1, tmax, steps(110.0, ctx), 1.25, ctx);
    var col: vec3f;
    var t = tmax;
    let fogc = mix(l.sky_lo, l.amb * 1.4, 0.4);
    if (hit.x > 0.0) {
        t = hit.x;
        col = shade(RO + rd * t, rd, t, hit.y, l, ctx);
        col = mix(fogc, col, exp(-t * l.fog));
    } else {
        col = sky(rd, l);
        col = mix(fogc, col, exp(-60.0 * l.fog * 0.3));
    }
    // the lamp's glow in the snowy air
    col += l.lamp_c * inscatter(RO, rd, t, ctx) * (0.002 + 0.3 * l.fog);
    // a little bloom around the lantern itself
    let ang = 1.0 - dot(rd, normalize(LAMP - RO));
    col += l.lamp_c * (0.00001 / (ang + 0.00004)) * (0.3 + 3.0 * l.fog);
    col += flakes(RO, rd, cam, t, l, ctx);
    return col * exp2(l.exposure);
}
