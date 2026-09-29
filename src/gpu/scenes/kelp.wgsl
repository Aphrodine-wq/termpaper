//! name: kelp
//! title: Monterey Kelp Forest
//! category: coast
//! tags: underwater, kelp, god rays, fish, california, ocean
//! desc: looking up through swaying giant kelp as sun shafts pour down and a school of fish mills
//! themes: sunlit, deep, twilight
//! uses: camera, water
//! cost: medium
//! fallback: abyss
//! credits: original

// World units are metres; y up, the sea surface is y = 0, the sand y = -17.
// The camera hovers 10 m down with a wide dome-port lens, tilted up. Giant
// kelp stands on a jittered grid: a 2D DDA walks the cells a ray crosses,
// front to back, and each plant is hit analytically as a swaying column of
// ragged blades (closest approach of the ray to its axis, re-solved after
// the sway at that height). Sun shafts, Snell's window, the canopy, a milling
// school of fish and drifting particles complete it.

const CAM: vec3f = vec3f(0.0, -12.8, 0.0);
const FLOOR: f32 = -17.0;
const CELL: f32 = 6.5;

struct Look {
    ls: vec3f,       // direction toward the (refracted) sun, underwater
    sun_c: vec3f,    // sunlight just under the surface
    sky_c: vec3f,    // diffuse daylight just under the surface
    sig: vec3f,      // extinction per metre
    water: vec3f,    // single-scatter colour of the water
    shafts: f32,
    exposure: f32,
    kelp: vec3f,     // kelp albedo
}

fn look(theme: u32) -> Look {
    var l: Look;
    var elev = 38.0;
    var az = 28.0;
    switch (theme) {
        case 1u: {
            elev = 55.0; az = 15.0;
            l.sun_c = vec3f(0.5, 0.62, 0.7);
            l.sky_c = vec3f(0.22, 0.34, 0.45);
            l.sig = vec3f(0.5, 0.16, 0.12);
            l.water = vec3f(0.02, 0.07, 0.13);
            l.shafts = 0.25; l.exposure = 0.6;
            l.kelp = col_hex(0x5a4a26u);
        }
        case 2u: {
            elev = 12.0; az = 35.0;
            l.sun_c = vec3f(2.6, 1.25, 0.4);
            l.sky_c = vec3f(0.34, 0.3, 0.3);
            l.sig = vec3f(0.18, 0.1, 0.15);
            l.water = vec3f(0.075, 0.07, 0.055);
            l.shafts = 1.1; l.exposure = 0.3;
            l.kelp = col_hex(0x7a5a26u);
        }
        default: {
            l.sun_c = vec3f(1.9, 1.85, 1.55);
            l.sky_c = vec3f(0.45, 0.62, 0.7);
            l.sig = vec3f(0.26, 0.075, 0.09);
            l.water = vec3f(0.035, 0.12, 0.14);
            l.shafts = 1.0; l.exposure = -0.2;
            l.kelp = col_hex(0x6e5628u);
        }
    }
    // refraction at the surface steepens the sun: cos(e_w) = cos(e) / 1.333
    let ce = cos(radians(elev)) / 1.333;
    let ew = acos(ce);
    let a = radians(az);
    l.ls = normalize(vec3f(sin(a) * cos(ew), sin(ew), -cos(a) * cos(ew)));
    return l;
}

// downwelling light at depth y (y <= 0), before the view path
fn downwell(y: f32, l: Look) -> vec3f {
    let d = max(-y, 0.0);
    return l.sun_c * exp(-l.sig * d / l.ls.y) * l.ls.y + l.sky_c * exp(-l.sig * d * 1.2);
}

// the blue-green haze in direction rd seen from depth y
fn sea_haze(rd: vec3f, y: f32, l: Look) -> vec3f {
    let up = saturate(rd.y * 0.5 + 0.5);
    let e = downwell(y, l);
    let fw = pow(saturate(dot(rd, l.ls)), 4.0);
    return l.water * (e * (0.25 + 1.2 * up * up) + l.sun_c * exp(-l.sig * max(-y, 0.0)) * fw * 0.4);
}

// surface canopy: the floating tops of the kelp mat the surface
fn canopy(xz: vec2f, t: f32) -> f32 {
    let n = noise_fbm2(xz * 0.12 + vec2f(t * 0.004, 0.0), 4);
    return smoothstep(0.56, 0.7, n) * 0.85;
}

// light pattern at the surface: wave focusing, drifting slowly
fn shaft_pat(xz: vec2f, t: f32) -> f32 {
    let a = noise_value2(xz * 0.9 + vec2f(t * 0.15, t * 0.06));
    let b = noise_value2(xz * 2.1 - vec2f(t * 0.11, -t * 0.13) + 7.0);
    return pow(a * 0.6 + b * 0.4, 4.0) * 6.0;
}

// ------------------------------------------------------------ kelp

// the lens zoom (cam_look_at with 80 degrees on the short side)
fn lens_zoom() -> f32 { return 0.5 / tan(radians(40.0)); }

fn plant_sway(y: f32, t: f32, h: vec4f) -> vec2f {
    let s = saturate((y - FLOOR) / -FLOOR);
    let k = pow(s, 1.5);
    let ph = h.z * TAU;
    return vec2f(0.95 * k * sin(t * 0.62 + ph + y * 0.1), 0.35 * k * sin(t * 0.43 + ph * 1.7));
}

// A plant seen as a cutout facing the camera (the camera never moves, so a
// facing cutout placed in 3D is exact): a wavy stipe with leaf-shaped
// blades angled up by their gas floats and streaming with the surge, which
// spread into a flat canopy near the surface. (u, y) in metres; returns
// coverage in [0, 1] given the pixel footprint.
fn stipe_cut(u: f32, y: f32, seed: f32, t: f32, top: f32, stream: f32) -> f32 {
    let st = u - 0.07 * sin(y * 1.7 + seed * 9.0);
    var d = abs(st) - 0.02;
    let sp = 0.26;
    let yk = floor(y / sp);
    for (var j = -3; j <= 0; j++) {
        let k = yk + f32(j);
        let hb = hash_cell2(vec2i(i32(k), i32(seed * 997.0)), 0xb1adu);
        let side = select(-1.0, 1.0, hb.x > 0.5);
        let y0 = (k + hb.y * 0.8) * sp;
        let near_top = smoothstep(top - 2.5, top - 0.3, y0);
        // blades flatten and tangle into the canopy near the surface
        let beta = mix(0.95 + 0.35 * hb.z, -0.25 + 0.7 * hb.z, near_top) + stream * side * 0.35;
        let dir = vec2f(side * cos(beta), sin(beta));
        let len = mix(0.4 + 0.45 * hb.w, 0.7 + 1.1 * hb.w * hb.w, near_top) * smoothstep(FLOOR + 1.0, FLOOR + 3.5, y0);
        let q = vec2f(st, y - y0);
        let along = dot(q, dir);
        let s = saturate(along / max(len, 1e-3));
        let across = abs(q.x * dir.y - q.y * dir.x);
        // leaf outline: narrow at the float, widest past the middle, pointed
        // tip, ruffled edge
        let w = (0.03 + 0.075 * sin(PI * pow(s, 0.7))) * (0.85 + 0.3 * sin(along * 30.0 + hb.z * 9.0 + t * 1.5));
        d = min(d, max(across - w, max(-along, along - len)));
    }
    return d;
}

// A plant seen as a cutout facing the camera (the camera never moves, so a
// facing cutout placed in 3D is exact): two wavy stipes with leaf-shaped
// blades angled up by their gas floats and streaming with the surge.
// (u, y) in metres; returns coverage given the pixel footprint.
fn plant_cut(u: f32, y: f32, h: vec4f, t: f32, top: f32, foot: f32, stream: f32) -> f32 {
    let d1 = stipe_cut(u + 0.14, y, h.x, t, top, stream);
    let d2 = stipe_cut(u - 0.16 - 0.05 * sin(y * 0.9), y + 0.13, h.y + 0.5, t, top - 0.4 * h.z, stream * 0.9);
    return saturate(0.5 - min(d1, d2) / foot);
}

// front-to-back walk through the forest: (colour, alpha, first depth)
fn kelp_trace(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx) -> vec4f {
    var acc = vec3f(0.0);
    var alpha = 0.0;
    let dxz = rd.xz;
    let len2 = max(dot(dxz, dxz), 1e-8);
    let lxz = sqrt(len2);
    var cell = floor(ro.xz / CELL);
    let st = sign(dxz);
    let inv = 1.0 / select(dxz, vec2f(1e-6), abs(dxz) < vec2f(1e-6));
    let tdel = abs(CELL * inv);
    var tnext = ((cell + max(st, vec2f(0.0))) * CELL - ro.xz) * inv;
    var tcell = 0.0;
    for (var i = 0; i < 30; i++) {
        if (tcell > tmax || alpha > 0.97) { break; }
        var h = hash_cell2(vec2i(cell), 0x6e1fu);
        // a hero plant framing the right foreground
        if (cell.x == 0.0 && cell.y == -1.0) { h = vec4f(0.72, 0.62, 0.35, 0.0); }
        if (h.w < 0.44) {
            let base = (cell + 0.5 + (h.xy - 0.5) * 0.34) * CELL;
            let top = -0.1 - 9.0 * max(h.z - 0.72, 0.0);
            var ax = base;
            var ts = 0.0;
            var ys = 0.0;
            for (var it = 0; it < 3; it++) {
                ts = dot(ax - ro.xz, dxz) / len2;
                ys = ro.y + rd.y * ts;
                ax = base + plant_sway(clamp(ys, FLOOR, top), ctx.t, h);
            }
            let cp = ro.xz + dxz * ts - ax;
            let dn = dxz / lxz;
            let u = cp.x * dn.y - cp.y * dn.x;
            if (ys > FLOOR && ys < top + 0.5 && ts > 0.3 && abs(u) < 3.0) {
                let foot = max(ts * ctx.px / lens_zoom(), 0.004);
                let stream = sin(ctx.t * 0.62 + h.z * TAU + ys * 0.1 + 1.2);
                let a = plant_cut(u, ys, h, ctx.t, top, foot, stream) * 0.97;
                if (a > 0.003 && ts < tmax) {
                    let th = ts;
                    let p = ro + rd * th;
                    let ang = u * 3.0;
                    // blades: wrinkled golden-brown, translucent against the light
                    let n = normalize(vec3f(-dn.x + 0.4 * sign(u) * dn.y, 0.3, -dn.y - 0.4 * sign(u) * dn.x));
                    let e = downwell(p.y, l);
                    let dapple = 0.55 + 0.9 * shaft_pat(p.xz + l.ls.xz / l.ls.y * (-p.y), ctx.t) * (1.0 - canopy(p.xz + l.ls.xz / l.ls.y * (-p.y), ctx.t) * 0.8);
                    let wrap = saturate(dot(n, l.ls) * 0.6 + 0.45);
                    let trans = pow(saturate(dot(rd, l.ls)), 2.0) * 2.0 + saturate(rd.y + 0.2) * 0.9;
                    let vein = 0.8 + 0.35 * noise_value2(vec2f(ang * 5.0, p.y * 9.0));
                    var c = l.kelp * vein * (e * wrap * dapple + l.sun_c * exp(-l.sig * max(-p.y, 0.0)) * trans * 0.5 * dapple);
                    // a few gas bladders catch a glint
                    let fog = exp(-l.sig * th);
                    c = c * fog + sea_haze(rd, CAM.y, l) * (1.0 - fog);
                    acc += (1.0 - alpha) * a * c;
                    alpha += (1.0 - alpha) * a;
                }
            }
        }
        // step to the next cell
        if (tnext.x < tnext.y) {
            tcell = tnext.x;
            tnext.x += tdel.x;
            cell.x += st.x;
        } else {
            tcell = tnext.y;
            tnext.y += tdel.y;
            cell.y += st.y;
        }
        if (lxz < 1e-3) { break; }
    }
    return vec4f(acc, alpha);
}

// ------------------------------------------------------------ floor & surface

fn shade_floor(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let n1 = noise_fbm2(p.xz * 0.35, 4);
    var alb = col_hex(0xb3a88fu) * (0.8 + 0.35 * noise_value2(p.xz * 2.5));
    // boulders crusted with pink coralline algae, dark holdfasts
    let rock = smoothstep(0.55, 0.68, n1);
    alb = mix(alb, mix(col_hex(0x4a3c38u), col_hex(0x9a5a5au), smoothstep(0.3, 0.7, noise_value2(p.xz * 1.7))), rock);
    let rip = 0.85 + 0.15 * sin(dot(p.xz, vec2f(0.8, 0.6)) * 7.0 + n1 * 6.0);
    alb *= mix(rip, 1.0, rock);
    let sp = p.xz + l.ls.xz / l.ls.y * (-p.y);
    let foot = t * ctx.px;
    let cau = water_caustics(sp * 0.8, ctx.t * 0.9) * (1.0 - smoothstep(0.03, 0.15, foot));
    let shade = 1.0 - 0.75 * canopy(sp, ctx.t);
    let e = (l.sun_c * exp(-l.sig * max(-p.y, 0.0) / l.ls.y) * l.ls.y * (0.4 + 1.8 * cau) * shade + l.sky_c * exp(-l.sig * max(-p.y, 0.0) * 1.2)) * 1.6;
    let c = alb * e;
    let fog = exp(-l.sig * t);
    return c * fog + sea_haze(rd, CAM.y, l) * (1.0 - fog);
}

// the surface seen from below: Snell's window (the whole sky squeezed into
// a 97 degree cone), total internal reflection outside it, canopy on top
fn shade_surface(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let g = vec2f(noise_value2(p.xz * 0.9 + ctx.t * 0.3), noise_value2(p.xz * 0.9 - ctx.t * 0.27 + 5.0)) - 0.5;
    let n = normalize(vec3f(-g.x * 0.35, -1.0, -g.y * 0.35));
    let ct = saturate(dot(-rd, -n));
    let crit = 0.6615; // cos(48.6 deg)
    let inside = smoothstep(crit - 0.03, crit + 0.03, ct);
    var sky = l.sky_c * 1.4 + l.sun_c * 0.25;
    // the sun's disc and glare inside the window
    let tr = refract(rd, n, 1.333);
    let mu = saturate(dot(normalize(tr + vec3f(0.0, 1e-3, 0.0)), normalize(vec3f(l.ls.x * 1.333, 0.0, l.ls.z * 1.333) + vec3f(0.0, sqrt(max(1.0 - dot(l.ls.xz, l.ls.xz) * 1.777, 0.05)), 0.0))));
    sky += l.sun_c * (pow(mu, 400.0) * 30.0 + pow(mu, 20.0) * 1.5);
    let tir = sea_haze(vec3f(rd.x, -rd.y, rd.z), 0.0, l) * 0.8;
    var c = mix(tir, sky, inside);
    // floating canopy: dark fronds with a golden fringe where light soaks through
    let cn = canopy(p.xz, ctx.t);
    c = mix(c, l.kelp * (l.sun_c * 0.25 + l.sky_c * 0.2) * (0.6 + 0.8 * inside), cn * 0.92);
    let fog = exp(-l.sig * t);
    return c * fog + sea_haze(rd, CAM.y, l) * (1.0 - fog);
}

// ------------------------------------------------------------ volumes

// sun shafts: in-scattering along the ray, lit through canopy gaps by the
// focusing pattern of the waves, which the shafts carry down
fn shafts(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = 18;
    let tm = min(tmax, 36.0);
    let dt = tm / f32(n);
    var acc = vec3f(0.0);
    let ph = 0.15 + 2.2 * pow(saturate(dot(rd, l.ls)), 5.0) + 0.3 * saturate(dot(rd, l.ls));
    for (var i = 0; i < 18; i++) {
        let s = (f32(i) + fract(ctx.jitter + 0.5)) * dt;
        let q = ro + rd * s;
        if (q.y > 0.0) { break; }
        let sp = q.xz + l.ls.xz / l.ls.y * (-q.y);
        let lit = shaft_pat(sp * 0.5, ctx.t * 0.5) * (1.0 - canopy(sp, ctx.t) * 0.9);
        acc += l.sun_c * exp(-l.sig * (-q.y / l.ls.y + s)) * lit * dt;
    }
    return acc * l.water * ph * l.shafts * 1.4;
}

// marine snow: sparse specks at several depths, drifting slowly
fn particles(p: vec2f, l: Look, ctx: Ctx) -> f32 {
    var a = 0.0;
    for (var k = 0; k < 3; k++) {
        let fk = f32(k);
        let sc = 14.0 + fk * 11.0;
        let q = p * sc + vec2f(ctx.t * (0.05 + fk * 0.03), ctx.t * (0.09 + fk * 0.04));
        let c = floor(q);
        let h = hash_cell2(vec2i(c), 0x5a0u + u32(k));
        if (h.w < 0.09) {
            let d = length(fract(q) - 0.2 - 0.6 * h.xy);
            let r = max(0.05 + 0.04 * h.z, ctx.px * sc * 0.7);
            a += smoothstep(r, r * 0.3, d) * (0.5 - fk * 0.12);
        }
    }
    return a;
}

// a milling school of silvery fish: a torus of 64 fish circling
fn school(p: vec2f, cam: Cam, tscene: f32, l: Look, ctx: Ctx) -> vec4f {
    let c0 = vec3f(3.5 + 1.5 * sin(ctx.t * 0.031), -7.5 + 0.8 * sin(ctx.t * 0.023), -13.0);
    let pc = cam_project(cam, c0);
    let rad = 5.5 * cam.zoom / pc.z;
    if (length(p - pc.xy) > rad || pc.z <= 0.0) { return vec4f(0.0); }
    var best = vec4f(0.0);
    var bz = 1e5;
    for (var i = 0; i < 64; i++) {
        let hh = hash_pcg3(vec3u(u32(i), 91u, 7u));
        let hx = hash_unorm(hh.x);
        let hy = hash_unorm(hh.y);
        let hz = hash_unorm(hh.z);
        let r = 2.2 + 1.8 * hx;
        let w = 0.32 + 0.08 * hy;
        let a = hz * TAU + ctx.t * w;
        let wob = 0.25 * sin(ctx.t * 1.3 + hx * 20.0);
        let fp = c0 + vec3f(cos(a) * (r + wob), (hy - 0.5) * 2.2 + 0.3 * sin(a * 2.0 + hx * 5.0), sin(a) * (r + wob));
        let vel = vec3f(-sin(a), 0.0, cos(a));
        let pr = cam_project(cam, fp);
        if (pr.z <= 0.5 || pr.z > tscene) { continue; }
        let pv = cam_project(cam, fp + vel * 0.3);
        var dir = pv.xy - pr.xy;
        let dl = length(dir);
        dir = select(vec2f(1.0, 0.0), dir / max(dl, 1e-6), dl > 1e-6);
        let s = cam.zoom / pr.z;
        let len = 0.26 * s;
        let q = p - pr.xy;
        let u = dot(q, dir);
        let v = dot(q, vec2f(-dir.y, dir.x));
        let body = length(vec2f(u / max(len, 1e-5), v / max(len * 0.28, 1e-5))) - 1.0;
        let cov = saturate(0.5 - body * min(len * 0.28, len) / max(ctx.px, 1e-5) * 1.5);
        if (cov > 0.0 && pr.z < bz) {
            // silver flank: flashes as a fish turns its side to the light
            let side = abs(dot(normalize(vec3f(vel.z, 0.0, -vel.x)), normalize(CAM - fp)));
            let e = downwell(fp.y, l);
            var col = vec3f(0.55, 0.6, 0.62) * e * (0.35 + 0.9 * side * side) + l.sun_c * exp(-l.sig * max(-fp.y, 0.0)) * 0.15 * side;
            let fog = exp(-l.sig * pr.z);
            col = col * fog + sea_haze(normalize(fp - CAM), fp.y, l) * (1.0 - fog);
            best = vec4f(col, cov);
            bz = pr.z;
        }
    }
    return best;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let cam = cam_look_at(CAM, CAM + vec3f(0.0, 0.27, -1.0), 0.0, 80.0);
    let rd = cam_ray(cam, p);
    var tmax = 80.0;
    var col: vec3f;
    let tf = select(1e5, (FLOOR - CAM.y) / rd.y, rd.y < -1e-4);
    let tsu = select(1e5, -CAM.y / rd.y, rd.y > 1e-4);
    if (tf < tmax) {
        tmax = tf;
        col = shade_floor(CAM + rd * tf, rd, tf, l, ctx);
    } else if (tsu < tmax) {
        tmax = tsu;
        col = shade_surface(CAM + rd * tsu, rd, tsu, l, ctx);
    } else {
        col = sea_haze(rd, CAM.y, l);
    }
    col += shafts(CAM, rd, tmax, l, ctx);
    let k = kelp_trace(CAM, rd, tmax, l, ctx);
    col = col * (1.0 - k.a) + k.rgb;
    // shafts in front of the nearest kelp still glow
    let sc = school(p, cam, 1e4, l, ctx);
    col = mix(col, sc.rgb, sc.a);
    col += l.water * downwell(CAM.y, l) * particles(p, l, ctx) * 2.5;
    return col * exp2(l.exposure);
}
