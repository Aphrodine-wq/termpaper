//! name: redwoods
//! title: Redwood Fog
//! category: wilds
//! tags: forest, fog, god-rays, california
//! desc: shafts of morning sun slanting through fog between old-growth redwood trunks
//! themes: morning, overcast, dusk
//! uses: camera, sky, fog
//! cost: heavy
//! fallback: canopy
//! credits: original

// World units are metres, y up, the forest floor at y = 0. The camera stands
// on the floor of an old-growth grove looking in between trunks 2-5 m
// across. Trunks are vertical cylinders found by walking the cell grid in
// plan (exact and cheap); the canopy is a layer of foliage 50-80 m up whose
// gaps let the sun through: the same gap pattern, projected along the sun,
// lights the fog in shafts, dapples the floor and patches the bark.

struct Look {
    sun: vec3f,
    sun_c: vec3f,    // direct sun reaching the canopy gaps
    amb: vec3f,      // light in the fog under the canopy
    fog_c: vec3f,    // colour the fog scatters (without sun)
    fog: f32,        // extinction per metre
    shafts: f32,     // strength of the light shafts
    lit_y: f32,      // dusk: sun only reaches above this height
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            // overcast: soft, green, misty
            l.sun = sky_sun_dir(40.0, 20.0);
            l.sun_c = vec3f(0.0);
            l.amb = vec3f(0.30, 0.36, 0.33);
            l.fog_c = vec3f(0.42, 0.48, 0.44);
            l.fog = 0.02;
            l.shafts = 0.0;
            l.lit_y = 0.0;
            l.exposure = -0.1;
        }
        case 2u: {
            // dusk: deep blue air, the last warm light high in the canopy
            l.sun = sky_sun_dir(6.0, 35.0);
            l.sun_c = sky_sun_light(sky_sun_dir(6.0, 35.0)) * 2.2;
            l.amb = vec3f(0.035, 0.045, 0.1);
            l.fog_c = vec3f(0.05, 0.06, 0.13);
            l.fog = 0.013;
            l.shafts = 0.5;
            l.lit_y = 38.0;
            l.exposure = 0.6;
        }
        default: {
            // morning: warm shafts through fog
            l.sun = sky_sun_dir(42.0, 12.0);
            l.sun_c = sky_sun_light(sky_sun_dir(42.0, 12.0)) * vec3f(1.15, 1.0, 0.8) * 1.4;
            l.amb = vec3f(0.085, 0.085, 0.08);
            l.fog_c = vec3f(0.11, 0.105, 0.095);
            l.fog = 0.017;
            l.shafts = 1.0;
            l.lit_y = 0.0;
            l.exposure = 0.55;
        }
    }
    return l;
}

// ------------------------------------------------------------ trunks

const CELL: f32 = 13.0;

// trunk in a cell: (centre x, centre z, radius, 0 = none)
fn trunk_in(c: vec2i) -> vec4f {
    let h = hash_cell2(c, 0x7ed3u);
    if (h.w > 0.68) { return vec4f(0.0); }
    let r = 1.1 + 1.6 * h.z * h.z;
    let m = r + 0.6;
    let ctr = (vec2f(c) * CELL) + vec2f(m) + (CELL - 2.0 * m) * h.xy;
    return vec4f(ctr, r, 1.0);
}

// the two giants framing the view, and one further in
fn hero(i: i32) -> vec4f {
    switch (i) {
        case 0: { return vec4f(-9.5, -12.0, 3.2, 1.0); }
        case 1: { return vec4f(10.0, -18.0, 2.6, 1.0); }
        default: { return vec4f(3.0, -42.0, 3.0, 1.0); }
    }
}

// flare of the trunk toward its foot (redwoods stand on wide buttresses)
// with buttress lobes around it (ang = bearing round the trunk)
fn flare(y: f32, ang: f32, k: f32) -> f32 {
    let e = exp(-max(y, 0.0) / 2.5);
    return 1.0 + e * (0.6 + 0.25 * sin(ang * 5.0 + k) + 0.15 * sin(ang * 3.0 + k * 2.3));
}

// ray vs vertical cylinder in plan, radius grown by the foot flare:
// intersect the wide bound, then walk in with a few steps on the true shape
fn cyl_hit(ro: vec3f, rd: vec3f, c: vec2f, r: f32) -> f32 {
    let o = ro.xz - c;
    let d = rd.xz;
    let a = dot(d, d);
    let b = dot(o, d);
    let rb = r * 2.05;
    let cc = dot(o, o) - rb * rb;
    let disc = b * b - a * cc;
    if (disc < 0.0 || a < 1e-8) { return -1.0; }
    let sq_d = sqrt(disc);
    var t = (-b - sq_d) / a;
    let t_out = (-b + sq_d) / a;
    if (t_out < 0.0) { return -1.0; }
    t = max(t, 0.0);
    // the flare only matters near the ground: take the plain cylinder
    // where the ray is high, march the flared one where it is low
    for (var i = 0; i < 8; i++) {
        let p = ro + rd * t;
        let rel = p.xz - c;
        let dd = length(rel) - r * flare(p.y, atan2(rel.y, rel.x), c.x * 0.37);
        if (dd < 0.01) { return t; }
        t += dd * 0.8 / max(length(d), 0.05);
        if (t > t_out) { return -1.0; }
    }
    return t;
}

struct Hit { t: f32, c: vec2f, r: f32 }

fn trace_trunks(ro: vec3f, rd: vec3f, tmax: f32) -> Hit {
    var best = Hit(tmax, vec2f(0.0), 0.0);
    for (var i = 0; i < 3; i++) {
        let hh = hero(i);
        let t = cyl_hit(ro, rd, hh.xy, hh.z);
        if (t > 0.0 && t < best.t) { best = Hit(t, hh.xy, hh.z); }
    }
    // walk the cells the ray crosses in plan (2D DDA)
    let d = rd.xz;
    var cell = vec2i(floor(ro.xz / CELL));
    let stp = vec2i(sign(d));
    let inv = 1.0 / max(abs(d), vec2f(1e-6));
    let fr = ro.xz / CELL - vec2f(cell);
    var tm = vec2f(
        select(fr.x, 1.0 - fr.x, d.x > 0.0) * CELL * inv.x,
        select(fr.y, 1.0 - fr.y, d.y > 0.0) * CELL * inv.y);
    let dt = CELL * inv;
    var t_cell = 0.0;
    for (var i = 0; i < 24; i++) {
        if (t_cell > best.t) { break; }
        let tr = trunk_in(cell);
        if (tr.w > 0.0) {
            let t = cyl_hit(ro, rd, tr.xy, tr.z);
            if (t > 0.0 && t < best.t) { best = Hit(t, tr.xy, tr.z); }
        }
        if (tm.x < tm.y) {
            t_cell = tm.x;
            tm.x += dt.x;
            cell.x += stp.x;
        } else {
            t_cell = tm.y;
            tm.y += dt.y;
            cell.y += stp.y;
        }
    }
    return best;
}

// ------------------------------------------------------------ light

// Canopy openings, named by where their beam lands on the floor (g, plan):
// a few placed to cross the view, each broken into sub-beams by the
// foliage, plus sparse random ones further off. Sways with the wind.
fn spot(q: vec2f, c: vec2f, r: f32) -> f32 { let d = q - c; return exp(-dot(d, d) / (r * r)); }
fn gaps(g: vec2f, t: f32) -> f32 {
    let sway = vec2f(sin(t * 0.21), cos(t * 0.17)) * 0.35;
    let q = g + sway;
    var v = spot(q, vec2f(0.5, -17.0), 1.3);
    v = max(v, spot(q, vec2f(-2.2, -21.0), 0.9));
    v = max(v, spot(q, vec2f(-4.0, -25.0), 1.5));
    v = max(v, spot(q, vec2f(3.0, -24.0), 0.8));
    v = max(v, spot(q, vec2f(5.0, -31.0), 1.6));
    v = max(v, spot(q, vec2f(-1.0, -34.0), 1.2));
    v = max(v, spot(q, vec2f(1.8, -40.0), 1.8) * 0.8);
    v = max(v, spot(q, vec2f(-6.5, -45.0), 2.0) * 0.7);
    let far = smoothstep(0.76, 0.82, noise_value2(q * 0.06 + vec2f(3.1, 1.7))) * smoothstep(40.0, 70.0, -q.y) * 0.6;
    v = max(smoothstep(0.2, 0.7, v), far);
    // leaves break each opening into finer rays
    let fine = noise_value2((q + sway * 2.0) * 1.6 + vec2f(7.3, 2.9));
    return v * (0.35 + 0.65 * smoothstep(0.3, 0.6, fine));
}

// how much sun reaches point p through the canopy (50 m up)
fn sun_at(p: vec3f, l: Look, t: f32) -> f32 {
    if (l.shafts <= 0.0) { return 0.0; }
    let g = p.xz - l.sun.xz / l.sun.y * p.y;
    var v = gaps(g, t);
    if (l.lit_y > 0.0) { v = smoothstep(l.lit_y - 4.0, l.lit_y + 10.0, p.y); }
    // the giants throw long shadows through the fog: does the sun ray's
    // plan line pass through one of them?
    let sd = normalize(l.sun.xz + vec2f(1e-4));
    for (var i = 0; i < 3; i++) {
        let hh = hero(i);
        let rel = hh.xy - p.xz;
        let along = dot(rel, sd);
        let across = abs(rel.x * sd.y - rel.y * sd.x);
        if (along > 0.0) { v *= smoothstep(hh.z * 0.8, hh.z * 1.15, across); }
    }
    return v;
}

// fog density: patchy and drifting, thicker low down
fn mist_density(p: vec3f, l: Look, t: f32) -> f32 {
    let n = noise_value2(vec2f(p.x + t * 1.1, p.z - t * 0.4) * 0.045 + vec2f(p.y * 0.03, 0.0));
    return l.fog * (0.55 + 0.9 * n) * (0.6 + 0.4 * exp(-p.y / 25.0));
}

// march the fog along the ray: (in-scattered light, transmittance)
fn mist_march(ro: vec3f, rd: vec3f, t_end: f32, l: Look, ctx: Ctx) -> vec4f {
    let tmax = min(t_end, 170.0);
    let n = 32;
    let dt = tmax / f32(n);
    var t = dt * ctx.jitter;
    var tr = 1.0;
    var acc = vec3f(0.0);
    let mu = dot(rd, l.sun);
    // forward-scattering haze: bright looking into the light
    let ph = 0.25 + 2.2 * pow(saturate(mu), 6.0) + 0.5 * pow(saturate(mu), 2.0);
    for (var i = 0; i < 32; i++) {
        let p = ro + rd * t;
        let s = mist_density(p, l, ctx.t);
        let sun = sun_at(p, l, ctx.t);
        let li = l.fog_c + l.sun_c * sun * ph * l.shafts * 1.6;
        let st = exp(-s * dt);
        acc += tr * li * (1.0 - st);
        tr *= st;
        t += dt;
    }
    // beyond the march the fog is simply opaque in its own colour
    if (t_end > tmax) {
        let rest = 1.0 - exp(-l.fog * (min(t_end, 900.0) - tmax));
        // far off the fog glows toward the sun, lit by light we don't march
        let glow = l.fog_c + l.sun_c * l.shafts * 0.12 * pow(saturate(mu), 3.0);
        acc += tr * glow * rest;
        tr *= 1.0 - rest;
    }
    return vec4f(acc, tr);
}

// ------------------------------------------------------------ surfaces

fn bark(p: vec3f, c: vec2f, r: f32, rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let rel = p.xz - c;
    let ang = atan2(rel.y, rel.x);
    var n = normalize(vec3f(rel.x, 0.0, rel.y));
    // deep vertical furrows of fibrous bark
    // furrows that wander, merge and split up the trunk
    // (the ridges twist slowly up the trunk, and break into ropes)
    let u = ang * r * 1.6 + p.y * 0.03 + 0.8 * noise_value2(vec2f(ang * 3.0, p.y * 0.04));
    let fur = noise_value2(vec2f(u * 1.3, p.y * 0.05)) * 0.55 + noise_value2(vec2f(u * 3.7, p.y * 0.4)) * 0.45;
    let tg = vec3f(-n.z, 0.0, n.x);
    let n0 = n;
    n = normalize(n + tg * (fur - 0.5) * 0.7);
    var alb = mix(col_hex(0x3a2016u), col_hex(0x8e4c2eu), smoothstep(0.2, 0.8, fur));
    alb *= 0.8 + 0.3 * noise_value2(vec2f(u * 0.3, p.y * 0.02));
    // old bark weathers grey-brown in broad patches
    alb = mix(alb, col_hex(0x5e4e44u), smoothstep(0.55, 0.8, noise_value2(vec2f(u * 0.12 + 3.0, p.y * 0.015))) * 0.6);
    // moss and burn scars low down
    alb = mix(alb, col_hex(0x3c4a24u), smoothstep(0.55, 0.8, noise_value2(vec2f(u * 0.5, p.y * 0.3))) * exp(-p.y / 3.0) * 0.8);
    alb = mix(alb, col_hex(0x1c1411u), smoothstep(0.6, 0.75, noise_value2(vec2f(u * 0.2 + 7.0, p.y * 0.06))) * exp(-p.y / 8.0) * 0.8);
    let sun = sun_at(p, l, ctx.t);
    let dif = saturate(dot(n, l.sun));
    // the fog lights the trunk from all round, the floor bounces a little
    let occ = 0.55 + 0.45 * smoothstep(0.1, 0.9, fur);
    // backlit fog wraps light round the trunk's edges
    let rim = pow(1.0 - saturate(dot(n0, -rd)), 4.0) * pow(saturate(dot(rd, l.sun)), 2.0);
    return alb * (l.sun_c * dif * sun * 1.3 + (l.amb * 1.2 + l.fog_c * 0.6) * occ) + l.fog_c * rim * 0.2;
}

fn floor_col(p: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    // duff, sorrel and sword ferns
    let f1 = noise_fbm2(p.xz * 0.25, 4);
    let fern = smoothstep(0.48, 0.62, noise_fbm2(p.xz * 0.12 + 3.0, 4));
    let sorrel = smoothstep(0.5, 0.65, noise_fbm2(p.xz * 0.4 + 9.0, 3));
    var alb = mix(col_hex(0x33261cu), col_hex(0x4a3828u), f1);
    alb = mix(alb, col_hex(0x46602au), max(sorrel, 0.3) * 0.9);
    // fern fronds: striped, dark between
    let frond = 0.6 + 0.4 * noise_value2(p.xz * vec2f(3.0, 1.0) + f1 * 4.0);
    alb = mix(alb, col_hex(0x3f5f27u) * frond, fern);
    let sun = sun_at(p, l, ctx.t);
    // soft shadows of the giants near their feet
    var occ = 1.0;
    for (var i = 0; i < 3; i++) {
        let hh = hero(i);
        occ *= 1.0 - 0.5 * exp(-max(length(p.xz - hh.xy) - hh.z, 0.0) / 4.0);
    }
    return alb * (l.sun_c * l.sun.y * sun * 1.2 + (l.amb + l.fog_c * 0.4) * occ * mix(1.0, 0.7, fern));
}

// the canopy seen from below: foliage masses with gaps of bright sky.
// → (foliage colour, coverage)
fn canopy(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx) -> vec4f {
    var col = vec3f(0.0);
    var cov = 0.0;
    // two layers of foliage, 66 m and 48 m up, far one first
    for (var k = 1; k >= 0; k--) {
        let hgt = 48.0 + 18.0 * f32(k);
        let t = (hgt - ro.y) / rd.y;
        let q = (ro + rd * t).xz;
        let wob = vec2f(sin(ctx.t * 0.2), cos(ctx.t * 0.15)) * 0.03;
        let n = noise_fbm2(q * 0.05 + vec2f(f32(k) * 9.0, 2.0) + wob, 4);
        let m = smoothstep(0.42, 0.52, n);
        // thin edges glow where the sun comes through
        let edge = 1.0 - smoothstep(0.52, 0.62, n);
        let leaf = col_hex(0x1d2a17u) * (l.amb * 1.5 + l.fog_c * 0.4) + l.sun_c * edge * 0.05 * (0.3 + l.shafts);
        col = mix(col, leaf, m);
        cov = max(cov, m);
    }
    return vec4f(col, cov);
}

// Sword ferns on the floor near the camera, drawn in the frame (the camera
// never moves): clumps of arching fronds, each frond a tapering bent stroke
// with leaflets. → (colour, coverage, depth)
fn ferns(p: vec2f, cam: Cam, l: Look, ctx: Ctx) -> vec4f {
    var col = vec3f(0.0);
    var cov = 0.0;
    var depth = 1e6;
    // far rows first so near clumps paint over them
    for (var row = 6; row >= 0; row--) {
        let z = -3.6 - f32(row) * 2.2;
        for (var k = 0; k < 9; k++) {
            let h = hash_cell2(vec2i(k, row), 0xfe4u);
            if (h.w > 0.88) { continue; }
            let wx = (f32(k) - 4.0) * (1.25 + 0.4 * f32(row)) + (h.x - 0.5) * 1.4;
            let base = vec3f(wx, 0.0, z + (h.y - 0.5) * 1.6);
            let pr = cam_project(cam, base);
            if (pr.z < 0.5) { continue; }
            let sc = cam.zoom / pr.z;                // metres → frame units here
            let size = (1.0 + 0.6 * h.z) * sc;
            let d0 = p - pr.xy;
            if (dot(d0, d0) > size * size * 3.2) { continue; }
            // fronds fan out from the crown and droop at the tips
            for (var f = 0; f < 11; f++) {
                let hf = hash_cell2(vec2i(f, k + row * 9), 0x5a1u);
                let ang = (f32(f) / 10.0 - 0.5) * 2.9 + (hf.x - 0.5) * 0.35;
                let dir = vec2f(sin(ang), cos(ang));
                let len = size * (0.75 + 0.35 * hf.y) * (1.0 - 0.25 * abs(ang));
                // bent stroke: u along it, the tip drops as u^2
                let u = saturate(dot(d0, dir) / len);
                let droop = vec2f(0.0, -0.55 * len * u * u) + vec2f(dir.x, 0.0) * 0.15 * len * u * u;
                let c = dir * len * u + droop;
                let dd = length(d0 - c);
                let w = (0.13 * (1.0 - u * 0.8) + 0.02) * len * (0.7 + 0.3 * sin(u * 70.0 + hf.z * 6.0));
                let m = sstep(w + ctx.px, w - ctx.px * 0.5, dd) * step(0.0, dot(d0, dir) + len * 0.02);
                if (m > 0.0) {
                    let lit = 0.55 + 0.45 * u;
                    let alb = mix(col_hex(0x2c4a1fu), col_hex(0x4d7432u), hf.z * 0.6 + u * 0.4);
                    let sun = sun_at(base + vec3f(0.0, 0.6, 0.0), l, ctx.t);
                    let c3 = alb * ((l.amb * 1.3 + l.fog_c * 0.5) * lit + l.sun_c * l.sun.y * sun * 0.8 * lit);
                    col = mix(col, c3, m);
                    cov = max(cov, m);
                    depth = min(depth, pr.z);
                }
            }
        }
    }
    return vec4f(col, select(0.0, cov, depth < 1e5)) + vec4f(0.0, 0.0, 0.0, 0.0 * depth);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(0.0, 1.8, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.0, 0.13, -1.0), 0.0, 56.0);
    let rd = cam_ray(cam, p);
    // what the eye stops at: floor, trunk, or the canopy/sky above
    let t_floor = select(1e6, -ro.y / rd.y, rd.y < 0.0);
    let t_can = select(1e6, (48.0 - ro.y) / rd.y, rd.y > 0.0);
    let h = trace_trunks(ro, rd, min(min(t_floor, t_can), 400.0));
    var col: vec3f;
    var t_end = h.t;
    if (h.r > 0.0) {
        col = bark(ro + rd * h.t, h.c, h.r, rd, l, ctx);
    } else if (rd.y < 0.0 && t_floor < 400.0) {
        t_end = t_floor;
        col = floor_col(ro + rd * t_floor, t_floor, l, ctx);
    } else {
        // above: bright fog and sky through the canopy
        col = l.fog_c * 1.6 + l.sun_c * 0.25 * pow(saturate(dot(rd, l.sun)), 4.0) * l.shafts + l.amb * 0.5;
        if (rd.y > 0.02) {
            let cn = canopy(ro, rd, l, ctx);
            col = mix(col, cn.rgb, cn.a);
        }
        t_end = min(t_can, 400.0);
    }
    let fm = mist_march(ro, rd, t_end, l, ctx);
    col = col * fm.a + fm.rgb;
    // wisps of mist drifting between the trunks: thin sheets at a few
    // depths, low over the floor
    for (var k = 0; k < 3; k++) {
        let zk = -14.0 - f32(k) * 13.0;
        if (rd.z >= 0.0) { break; }
        let tk = (zk - ro.z) / rd.z;
        if (tk > t_end) { break; }
        let q = ro + rd * tk;
        let n = noise_fbm2(vec2f(q.x * 0.08 - ctx.t * (0.25 + 0.1 * f32(k)), q.y * 0.12 + f32(k) * 3.0), 3);
        let band = exp(-sq((q.y - 3.0 - 2.5 * f32(k)) / (4.0 + 2.0 * f32(k))));
        let w = smoothstep(0.45, 0.8, n) * band * 0.22;
        let lit = l.fog_c * 1.3 + l.sun_c * sun_at(q, l, ctx.t) * l.shafts * 0.25;
        col = mix(col, lit, w);
    }
    if (p.y < 0.0) {
        let fe = ferns(p, cam, l, ctx);
        // (the ferns are 3-17 m off: a touch of fog on them)
        col = mix(col, fe.rgb * 0.85 + l.fog_c * 0.15, fe.a);
    }
    return col * exp2(l.exposure);
}
