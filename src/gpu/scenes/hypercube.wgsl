//! name: hypercube
//! title: Hypercube Chamber
//! category: abstract
//! tags: tesseract, four dimensional, geometry, gallery, glass, light
//! desc: a tesseract of glowing glass rods turning in four dimensions in a concrete gallery
//! themes: violet, ice, ember
//! params: rotation, glow, skylight
//! uses: camera, raymarch, sdf, noise, light
//! cost: medium
//! fallback: orbits
//! credits: original

// World units are metres. A bare concrete gallery: a polished floor that
// mirrors the room, panelled walls with their shadow gaps, a skylight in the
// ceiling and a slot window in the back wall. On a low plinth hangs a
// tesseract: its 32 edges are glass rods lit from inside, and the whole
// figure turns slowly in two planes of four-dimensional space, so the rods
// stretch, shrink and pass through each other as the inner cube becomes the
// outer. The rods light the room, and the floor reflects them. The camera
// is fixed; only the figure and the dust it lights move.

const ROOM: vec3f = vec3f(4.5, 2.6, 5.5);   // half extents; the floor is y = 0
const CENTRE: vec3f = vec3f(0.0, 2.1, -0.6);
const SCALE: f32 = 0.95;
const ROD: f32 = 0.022;

struct Look {
    rod: vec3f,       // the rods' light
    sky: vec3f,       // skylight colour
    amb: vec3f,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: { return Look(vec3f(0.5, 0.85, 1.4), vec3f(0.9, 0.95, 1.0) * 1.1, vec3f(0.08, 0.09, 0.11), -0.1); }
        case 2u: { return Look(vec3f(1.5, 0.5, 0.12), vec3f(0.3, 0.2, 0.14) * 0.5, vec3f(0.04, 0.025, 0.02), 0.2); }
        default: { return Look(vec3f(0.75, 0.35, 1.5), vec3f(0.35, 0.4, 0.6) * 0.35, vec3f(0.02, 0.02, 0.035), 0.3); }
    }
}

// ------------------------------------------------------------------ the figure

struct Spin { a: vec2f, b: vec2f, c: vec2f }

fn spin(ctx: Ctx) -> Spin {
    let rate = 0.03 + ctx.params.x * 0.07;
    let a = ctx.t * rate;
    let b = ctx.t * rate * 0.61 + 1.0;
    let c = ctx.t * rate * 0.37 + 2.0;
    return Spin(vec2f(cos(a), sin(a)), vec2f(cos(b), sin(b)), vec2f(cos(c), sin(c)));
}

// vertex `bits` of the unit tesseract, turned in the xw, yz and zw planes
// and projected from 4D to 3D (w toward the eye: the inner cube is the far one)
fn vertex(bits: u32, s: Spin) -> vec3f {
    var v = vec4f(
        select(-1.0, 1.0, (bits & 1u) != 0u),
        select(-1.0, 1.0, (bits & 2u) != 0u),
        select(-1.0, 1.0, (bits & 4u) != 0u),
        select(-1.0, 1.0, (bits & 8u) != 0u));
    v = vec4f(v.x * s.a.x - v.w * s.a.y, v.y, v.z, v.x * s.a.y + v.w * s.a.x);
    v = vec4f(v.x, v.y * s.b.x - v.z * s.b.y, v.y * s.b.y + v.z * s.b.x, v.w);
    v = vec4f(v.x, v.y, v.z * s.c.x - v.w * s.c.y, v.z * s.c.y + v.w * s.c.x);
    let depth = 2.0 / (3.4 - v.w);
    return CENTRE + v.xyz * depth * SCALE;
}

// distance to the nearest rod
fn rods(p: vec3f, s: Spin) -> f32 {
    var d = 1e5;
    for (var i = 0u; i < 16u; i++) {
        let a = vertex(i, s);
        for (var j = 0u; j < 4u; j++) {
            if ((i & (1u << j)) != 0u) { continue; }
            let b = vertex(i | (1u << j), s);
            d = min(d, sdf_capsule(p, a, b, ROD));
        }
    }
    return d;
}

// a cheap bound on the figure: everything lies in this sphere
fn figure_bound(p: vec3f) -> f32 {
    return length(p - CENTRE) - 1.95 * SCALE;
}

// ------------------------------------------------------------------ the room

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    // the room, inside out: floor, walls, ceiling
    let q = p - vec3f(0.0, ROOM.y, 0.0);
    var d = -(sdf_box(q, ROOM));
    var m = 1.0;
    if (p.y < 0.02) { m = 2.0; }
    // the plinth
    let pl = sdf_round_box(p - vec3f(CENTRE.x, 0.12, CENTRE.z), vec3f(1.15, 0.12, 1.15), 0.03);
    if (pl < d) { d = pl; m = 3.0; }
    // the rods, only near the figure (the march bounds its own steps by
    // the figure's sphere; shadows and occlusion must not see a sphere)
    if (figure_bound(p) < 0.15) {
        let fr = rods(p, spin(ctx));
        if (fr < d) { d = fr; m = 4.0; }
    }
    return vec2f(d, m);
}

// the skylight: a lit rectangle in the ceiling; the slot: a tall strip in
// the back wall. Both are emitters and both throw light.
fn skylight_mask(p: vec3f) -> f32 {
    let s = abs(p.xz - vec2f(0.0, -0.6)) - vec2f(1.3, 1.6);
    return 1.0 - smoothstep(-0.05, 0.05, max(s.x, s.y));
}

fn slot_mask(p: vec3f) -> f32 {
    let s = abs(p.xy - vec2f(2.6, 1.6)) - vec2f(0.12, 1.4);
    return 1.0 - smoothstep(-0.03, 0.03, max(s.x, s.y));
}

// how much of the skylight a point sees, as a soft area light from above
fn skylight(p: vec3f, n: vec3f, ctx: Ctx) -> f32 {
    let to = vec3f(0.0, 2.0 * ROOM.y, -0.6) - p;
    let dist = length(to);
    let l = to / dist;
    let dif = saturate(dot(n, l));
    if (dif <= 0.0) { return 0.0; }
    // a short shadow march: the plinth and the figure block it
    var sh = 1.0;
    var t = 0.05;
    for (var i = 0; i < 12; i++) {
        let h = map(p + l * t, ctx).x;
        sh = min(sh, 4.0 * h / t);
        if (sh < 0.02) { break; }
        t += clamp(h, 0.05, 0.6);
        if (t > dist - 0.1) { break; }
    }
    // the skylight is broad: its light falls off gently with distance
    let spread = 1.0 / (1.0 + dist * dist * 0.08);
    return dif * saturate(sh) * spread * 6.0;
}

// light from the rods: the figure as a point source at its centre, plus a
// cheap sense of which side faces it
fn figure_light(p: vec3f, n: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let to = CENTRE - p;
    let d2 = dot(to, to);
    let ld = to * inverseSqrt(max(d2, 1e-4));
    let dif = saturate(dot(n, ld) * 0.7 + 0.3);
    return l.rod * dif * (1.2 + ctx.params.y * 1.6) / (1.0 + d2 * 1.1);
}

fn surface_color(p: vec3f, n: vec3f, mat: f32) -> vec3f {
    if (mat < 1.5) {
        // concrete panels with shadow gaps
        let along = select(p.x, p.z, abs(n.x) > 0.5);
        let gap = min(smoothstep(0.0, 0.03, abs(fract(along / 1.5) - 0.5) * 1.5 - 0.72),
                      smoothstep(0.0, 0.03, abs(fract(p.y / 1.3 + 0.5) - 0.5) * 1.3 - 0.63));
        let tone = 0.85 + 0.16 * noise_fbm2(p.xy * 1.7 + p.z * 0.9, 3) + 0.08 * noise_value3(p * 9.0) + 0.04 * noise_value3(p * 60.0);
        var c = col_hex(0x8d8a86u) * tone;
        if (abs(n.y) > 0.5) { c = col_hex(0x7f7c78u) * tone; }
        return c * mix(0.35, 1.0, gap);
    }
    if (mat < 2.5) {
        // the polished floor: dark, with a faint aggregate pattern
        return col_hex(0x34343au) * (0.9 + 0.12 * noise_fbm2(p.xz * 3.0, 3) + 0.06 * noise_value2(p.xz * 45.0));
    }
    // the plinth: pale stone
    return col_hex(0xa8a39au) * (0.85 + 0.25 * noise_value3(p * 12.0));
}

// everything but the rods: the room lit by the skylight, the slot, the figure
fn shade(p: vec3f, rd: vec3f, n: vec3f, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    let alb = surface_color(p, n, mat);
    var c = alb * l.sky * skylight(p, n, ctx) * (0.5 + ctx.params.z);
    c += alb * figure_light(p, n, l, ctx);
    // the slot window, as a dim side light
    let to = vec3f(ROOM.x, 1.6, -2.6) - p;
    let sl = normalize(to);
    c += alb * l.sky * 0.6 * saturate(dot(n, sl)) / (1.0 + dot(to, to) * 0.3);
    c += alb * l.amb * (0.6 + 0.4 * saturate(n.y)) * rm_ao(p, n, ctx);
    return c;
}

// glass rod: light from inside, the skylight glinting on it
fn shade_rod(p: vec3f, rd: vec3f, n: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let fres = light_fresnel(saturate(dot(n, -rd)), 0.04);
    let glint = light_ggx(n, -rd, normalize(vec3f(0.0, 1.0, 0.2)), 0.2) * l.sky * 2.0;
    return l.rod * (2.2 + ctx.params.y * 2.0) * (0.75 + 0.5 * fres) + glint;
}

// march that also gathers the rods' glow in the air along the way
struct Hit { t: f32, m: f32, glow: f32 }

fn march(ro: vec3f, rd: vec3f, tmax: f32, n: i32, ctx: Ctx) -> Hit {
    var h: Hit;
    h.t = -1.0;
    h.m = -1.0;
    h.glow = 0.0;
    let s = spin(ctx);
    var t = 0.02;
    for (var i = 0; i < 160; i++) {
        if (i >= n) { break; }
        let p = ro + rd * t;
        let d = map(p, ctx);
        let fb = figure_bound(p);
        if (fb < 0.4) {
            // near the figure the rod distance is what map found
            var r: f32;
            if (d.y > 3.5 && fb < 0.15) { r = d.x; } else { r = rods(p, s); }
            h.glow += exp(-r * 9.0) * min(d.x, 0.1) * 1.2;
        }
        let eps = max(0.0003, 0.3 * ctx.px * t);
        if (d.x < eps) { h.t = t; h.m = d.y; break; }
        // never step across the figure's sphere from outside it
        var step_ = d.x;
        if (fb >= 0.15) { step_ = min(step_, fb); }
        t += step_ * 0.9;
        if (t > tmax) { break; }
    }
    return h;
}

// dust in the air near the figure, lit by it
fn dust(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx) -> vec3f {
    var c = vec3f(0.0);
    for (var i = 0; i < 3; i++) {
        let sc = 7.0 + f32(i) * 5.0;
        // a slow drift in the still air, each layer its own
        let q = (ro + rd * (3.0 + f32(i) * 1.2)) * sc + vec3f(ctx.t * 0.06, -ctx.t * 0.04, ctx.t * 0.02) * (1.0 + f32(i));
        let cell = floor(q);
        let hh = hash_cell3(vec3i(cell), 0xd05u + u32(i));
        if (hh.w > 0.12) { continue; }
        let d = length(fract(q) - 0.3 - 0.4 * hh.xyz);
        let m = smoothstep(0.12, 0.03, d);
        let pw = ro + rd * (3.0 + f32(i) * 1.2);
        let near = exp(-length(pw - CENTRE) * 0.9);
        c += l.rod * m * near * 0.5;
    }
    return c;
}

// ------------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let shift = cam_depth_offset(ctx) * 1.5;
    let ro = vec3f(0.0, 2.3, 5.4) + shift;
    var cam = cam_look_at(ro, vec3f(0.0, 1.45, -0.6), 0.0, 50.0);
    cam = cam_portal(cam, ctx, 5.8);
    let rd = cam_ray(cam, p);
    let h = march(ro, rd, 30.0, steps(110.0, ctx), ctx);
    var col = vec3f(0.0);
    if (h.t > 0.0) {
        let hp = ro + rd * h.t;
        if (h.m > 3.5) {
            let n = rm_normal(hp, h.t, ctx);
            col = shade_rod(hp, rd, n, l, ctx);
        } else {
            let n = rm_normal(hp, h.t, ctx);
            col = shade(hp, rd, n, h.m, l, ctx);
            // the emitters themselves
            if (hp.y > 2.0 * ROOM.y - 0.02) { col += l.sky * 2.5 * skylight_mask(hp) * (0.5 + ctx.params.z); }
            if (hp.x > ROOM.x - 0.02) { col += l.sky * 1.5 * slot_mask(hp); }
            // the polished floor mirrors the room: the figure and the skylight
            if (h.m > 1.5 && h.m < 2.5) {
                let rn = normalize(n + (noise_value2(hp.xz * 30.0) - 0.5) * 0.02);
                let rr = reflect(rd, rn);
                let rh = march(hp + rn * 0.01, rr, 14.0, steps(48.0, ctx), ctx);
                var refl = vec3f(0.0);
                if (rh.t > 0.0) {
                    let rp = hp + rn * 0.01 + rr * rh.t;
                    if (rh.m > 3.5) {
                        refl = l.rod * 2.5;
                    } else {
                        let rnn = rm_normal(rp, rh.t, ctx);
                        refl = surface_color(rp, rnn, rh.m) * (l.amb * 1.5 + figure_light(rp, rnn, l, ctx) * 0.6);
                        if (rp.y > 2.0 * ROOM.y - 0.02) { refl += l.sky * 2.5 * skylight_mask(rp) * (0.5 + ctx.params.z); }
                        if (rp.x > ROOM.x - 0.02) { refl += l.sky * 1.5 * slot_mask(rp); }
                    }
                }
                refl += l.rod * rh.glow * 0.8;
                let f = light_fresnel(saturate(dot(rn, -rd)), 0.04);
                col += refl * (0.14 + 0.8 * f);
            }
        }
    }
    col += l.rod * h.glow * (0.6 + ctx.params.y * 0.8);
    col += dust(ro, rd, select(30.0, h.t, h.t > 0.0), l, ctx);
    // a focused monitor: the figure pulses a touch brighter
    col *= 1.0 + 0.08 * ctx.focus_pulse * ctx.reaction;
    return col * exp2(l.exposure);
}
