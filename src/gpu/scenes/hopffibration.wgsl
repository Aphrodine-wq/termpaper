//! name: hopffibration
//! title: Hopf Fibration
//! category: abstract
//! tags: hopf, four dimensional, topology, glass, sculpture, gallery
//! desc: linked rings of lit glass, fibres of the three-sphere, turning on a plinth in a gallery
//! themes: ultraviolet, pearl, gold
//! params: fibres, rotation, glow
//! uses: camera, sdf, noise, light
//! cost: heavy
//! fallback: orbits
//! credits: original

// World units are metres. The Hopf map sends every point of the 3-sphere to
// a point of the ordinary sphere, and the points that land together form a
// circle: a fibre. Stereographic projection brings those circles into our
// space as real circles, every pair of them linked once. Twelve of them,
// over three rings of latitude on the base sphere, are cast here as glass
// tubes lit from inside, on a plinth under a spotlight in a dark gallery.
// The base points drift slowly round the sphere, so the tubes swell and
// pass through one another without ever touching. The camera is fixed.

const CENTRE: vec3f = vec3f(0.0, 1.55, 0.0);
const TUBE: f32 = 0.032;
const NF: i32 = 12;

struct Look {
    glass: vec3f,     // the tubes' inner light
    glass2: vec3f,
    spot: vec3f,      // the spotlight
    amb: vec3f,
    wall: vec3f,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: { return Look(vec3f(1.0, 0.85, 0.95), vec3f(0.7, 0.9, 1.0), vec3f(1.0, 0.97, 0.92) * 6.0, vec3f(0.14, 0.14, 0.15), col_hex(0xbdb7afu), -0.15); }
        case 2u: { return Look(vec3f(1.5, 0.9, 0.3), vec3f(1.2, 0.4, 0.1), vec3f(1.0, 0.85, 0.6) * 5.0, vec3f(0.035, 0.025, 0.02), col_hex(0x4a3b2cu), 0.25); }
        default: { return Look(vec3f(0.5, 0.25, 1.4), vec3f(0.2, 0.7, 1.3), vec3f(0.7, 0.75, 1.0) * 4.0, vec3f(0.02, 0.02, 0.035), col_hex(0x2a2a36u), 0.3); }
    }
}

// a fibre as a circle in space: centre, radius, unit normal
struct Circle { c: vec3f, r: f32, n: vec3f }

// the fibre over base point (polar angle `alpha`, azimuth `beta`), projected
// stereographically from (0, 0, 0, 1): three points on it fix the circle
fn fibre_point(alpha: f32, beta: f32, psi: f32) -> vec3f {
    let a = cos(alpha * 0.5);
    let b = sin(alpha * 0.5);
    let z1 = vec2f(cos(psi), sin(psi)) * a;
    let z2 = vec2f(cos(psi + beta), sin(psi + beta)) * b;
    let w = z2.y;
    return vec3f(z1.x, z1.y, z2.x) / max(1.0 - w, 0.04);
}

fn fibre(i: i32, ctx: Ctx) -> Circle {
    // three latitudes, four fibres on each, all drifting round the base
    let ring = i / 4;
    let k = f32(i % 4);
    let drift = ctx.t * (0.012 + ctx.params.y * 0.03);
    let alpha = 0.5 + f32(ring) * 0.55 + 0.18 * sin(drift * 0.7 + f32(ring));
    let beta = k * (TAU / 4.0) + drift + f32(ring) * 0.5;
    let p0 = fibre_point(alpha, beta, 0.0);
    let p1 = fibre_point(alpha, beta, TAU / 3.0);
    let p2 = fibre_point(alpha, beta, 2.0 * TAU / 3.0);
    // circumcircle of three points
    let a = p1 - p0;
    let b = p2 - p0;
    let axb = cross(a, b);
    let denom = 2.0 * dot(axb, axb);
    let to_c = (dot(b, b) * cross(axb, a) + dot(a, a) * cross(b, axb)) / max(denom, 1e-6);
    var c: Circle;
    c.c = CENTRE + (p0 + to_c) * 0.42;
    c.r = length(to_c) * 0.42;
    c.n = normalize(axb);
    return c;
}

fn circle_sdf(p: vec3f, c: Circle) -> f32 {
    let q = p - c.c;
    let qn = dot(q, c.n);
    let qr = length(q - c.n * qn);
    return length(vec2f(qr - c.r, qn)) - TUBE;
}

// the sculpture: (distance, fibre index)
fn fibres(p: vec3f, cs: array<Circle, 12>, count: i32) -> vec2f {
    var cc = cs;
    var d = 1e5;
    var id = 0.0;
    for (var i = 0; i < NF; i++) {
        if (i >= count) { break; }
        let di = circle_sdf(p, cc[i]);
        if (di < d) { d = di; id = f32(i); }
    }
    return vec2f(d, id);
}

// everything but the glass: a square plinth and the gallery's floor and walls
fn room(p: vec3f) -> vec2f {
    var d = p.y;
    var m = 1.0;
    let pl = sdf_round_box(p - vec3f(0.0, 0.3, 0.0), vec3f(0.7, 0.3, 0.7), 0.02);
    if (pl < d) { d = pl; m = 2.0; }
    let walls = -(sdf_box(p - vec3f(0.0, 2.2, 0.0), vec3f(4.2, 2.2, 4.8)));
    if (walls < d) { d = walls; m = 3.0; }
    return vec2f(d, m);
}

fn bound(p: vec3f) -> f32 {
    return length(p - CENTRE) - 1.45;
}

struct Hit { t: f32, m: f32, id: f32, glow: f32 }

// march the room and the glass, gathering the tubes' glow in the air
fn march(ro: vec3f, rd: vec3f, tmax: f32, n: i32, cs: array<Circle, 12>, count: i32, ctx: Ctx) -> Hit {
    var h: Hit;
    h.t = -1.0;
    h.m = -1.0;
    h.id = 0.0;
    h.glow = 0.0;
    var t = 0.02;
    for (var i = 0; i < 140; i++) {
        if (i >= n) { break; }
        let p = ro + rd * t;
        var d = room(p);
        let b = bound(p);
        if (b < 0.25) {
            let f = fibres(p, cs, count);
            h.glow += exp(-f.x * 14.0) * min(d.x, min(f.x, 0.08)) * 1.3;
            if (f.x < d.x) { d = vec2f(f.x, 4.0); h.id = f.y; }
        } else {
            d.x = min(d.x, b);
        }
        let eps = max(0.0003, 0.3 * ctx.px * t);
        if (d.x < eps) { h.t = t; h.m = d.y; break; }
        t += d.x * 0.9;
        if (t > tmax) { break; }
    }
    return h;
}

fn normal_at(p: vec3f, glass: bool, cs: array<Circle, 12>, count: i32, t: f32, ctx: Ctx) -> vec3f {
    let e = max(0.0005, 0.5 * ctx.px * t);
    let k = vec2f(1.0, -1.0);
    if (glass) {
        return normalize(
            k.xyy * fibres(p + k.xyy * e, cs, count).x +
            k.yyx * fibres(p + k.yyx * e, cs, count).x +
            k.yxy * fibres(p + k.yxy * e, cs, count).x +
            k.xxx * fibres(p + k.xxx * e, cs, count).x);
    }
    return normalize(
        k.xyy * room(p + k.xyy * e).x +
        k.yyx * room(p + k.yyx * e).x +
        k.yxy * room(p + k.yxy * e).x +
        k.xxx * room(p + k.xxx * e).x);
}

// the spotlight above the plinth: a cone of light, with the glass and the
// plinth casting soft shadows
const SPOT: vec3f = vec3f(0.4, 4.3, 0.9);

fn spot_light(p: vec3f, n: vec3f, l: Look, cs: array<Circle, 12>, count: i32, ctx: Ctx) -> vec3f {
    let to = SPOT - p;
    let dist = length(to);
    let ld = to / dist;
    let dif = saturate(dot(n, ld));
    if (dif <= 0.0) { return vec3f(0.0); }
    // the cone: aimed at the sculpture, soft-edged
    let aim = normalize(CENTRE - SPOT);
    let cone = smoothstep(0.89, 0.95, dot(-ld, aim));
    if (cone <= 0.0) { return vec3f(0.0); }
    var sh = 1.0;
    var t = 0.03;
    for (var i = 0; i < 16; i++) {
        let q = p + ld * t;
        var hh = room(q).x;
        if (bound(q) < 0.25) { hh = min(hh, fibres(q, cs, count).x); } else { hh = min(hh, bound(q)); }
        sh = min(sh, 10.0 * hh / t);
        if (sh < 0.02) { break; }
        t += clamp(hh, 0.02, 0.4);
        if (t > dist - 0.2) { break; }
    }
    return l.spot * dif * cone * saturate(sh) / (dist * dist);
}

fn glass_light(p: vec3f, n: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let to = CENTRE - p;
    let d2 = dot(to, to);
    let ld = to * inverseSqrt(max(d2, 1e-4));
    return mix(l.glass, l.glass2, 0.5) * saturate(dot(n, ld) * 0.7 + 0.3) * (0.5 + ctx.params.z) / (0.6 + d2 * 1.6);
}

fn shade_room(p: vec3f, rd: vec3f, n: vec3f, m: f32, l: Look, cs: array<Circle, 12>, count: i32, ctx: Ctx) -> vec3f {
    var alb: vec3f;
    var rough = 0.7;
    if (m < 1.5) {
        // dark terrazzo floor, polished
        alb = col_hex(0x2c2c30u) * (0.92 + 0.1 * noise_fbm2(p.xz * 3.0, 2));
        rough = 0.25;
    } else if (m < 2.5) {
        alb = col_hex(0xd8d4ccu) * (0.9 + 0.15 * noise_value3(p * 14.0));
        rough = 0.5;
    } else {
        alb = l.wall * (0.85 + 0.2 * noise_fbm2(p.xy * 2.0 + p.z, 3));
    }
    var c = alb * spot_light(p, n, l, cs, count, ctx);
    c += alb * glass_light(p, n, l, ctx);
    c += alb * l.amb * (0.6 + 0.4 * saturate(n.y));
    c += l.spot * light_ggx(n, -rd, normalize(SPOT - p), rough) * 0.01;
    return c;
}

fn shade_glass(p: vec3f, rd: vec3f, n: vec3f, id: f32, l: Look, ctx: Ctx) -> vec3f {
    let hue = fract(id / 4.0 + floor(id / 4.0) * 0.17);
    let inner = mix(l.glass, l.glass2, hue);
    let fres = light_fresnel(saturate(dot(n, -rd)), 0.04);
    var c = inner * (1.6 + ctx.params.z * 1.6) * (0.6 + 0.6 * fres);
    // the spotlight's highlight on the glass
    let ld = normalize(SPOT - p);
    c += l.spot * light_ggx(n, -rd, ld, 0.12) * 0.05;
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let count = clamp(i32(6.0 + ctx.params.x * 7.0), 4, NF);
    var cs: array<Circle, 12>;
    for (var i = 0; i < NF; i++) { cs[i] = fibre(i, ctx); }
    let shift = cam_depth_offset(ctx) * 1.2;
    let ro = vec3f(1.9, 2.0, 3.6) + shift;
    var cam = cam_look_at(ro, CENTRE - vec3f(0.0, 0.08, 0.0), 0.0, 44.0);
    cam = cam_portal(cam, ctx, 4.2);
    let rd = cam_ray(cam, p);
    let h = march(ro, rd, 24.0, steps(100.0, ctx), cs, count, ctx);
    var col = vec3f(0.0);
    if (h.t > 0.0) {
        let hp = ro + rd * h.t;
        if (h.m > 3.5) {
            let n = normal_at(hp, true, cs, count, h.t, ctx);
            col = shade_glass(hp, rd, n, h.id, l, ctx);
        } else {
            let n = normal_at(hp, false, cs, count, h.t, ctx);
            col = shade_room(hp, rd, n, h.m, l, cs, count, ctx);
            if (h.m < 1.5) {
                // the polished floor mirrors the sculpture
                let rr = reflect(rd, n);
                let rh = march(hp + n * 0.01, rr, 8.0, steps(40.0, ctx), cs, count, ctx);
                var refl = vec3f(0.0);
                if (rh.t > 0.0 && rh.m > 3.5) {
                    refl = mix(l.glass, l.glass2, fract(rh.id / 4.0)) * 1.8;
                } else if (rh.t > 0.0) {
                    refl = l.amb * 0.8;
                }
                refl += mix(l.glass, l.glass2, 0.5) * rh.glow * 0.8;
                let f = light_fresnel(saturate(dot(n, -rd)), 0.04);
                col += refl * (0.1 + 0.7 * f);
            }
        }
    }
    col += mix(l.glass, l.glass2, 0.5) * h.glow * (0.5 + ctx.params.z * 0.8);
    col *= 1.0 + 0.08 * ctx.focus_pulse * ctx.reaction;
    return col * exp2(l.exposure);
}
