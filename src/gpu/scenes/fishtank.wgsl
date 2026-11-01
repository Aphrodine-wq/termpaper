//! name: fishtank
//! title: Fish Tank Wall
//! category: coast
//! tags: aquarium, reef fish, caustics, glass, underwater, shared world
//! desc: a long reef tank at the glass: caustics on the sand, swaying plants, fish crossing panes
//! themes: daylight, evening, moonlit
//! params: fish, plants, light
//! uses: camera, raymarch, sdf, noise, water, light, fog
//! cost: heavy
//! fallback: aquarium
//! credits: original

// World units are metres. The camera stands at the front glass of a long
// aquarium looking in: sand ripples on the floor, rocks, driftwood, clumps
// of vallisneria swaying in the filter's current, a column of bubbles, and
// a strip light over the surface throwing caustics and shafts down through
// the water. The tank is one world: fish follow closed-form loops in it, so
// a wall of monitors shows one tank and the fish swim from pane to pane.
// The camera never moves; the water, the plants and the fish do.

const SAND: f32 = -0.55;
const SURFACE: f32 = 0.66;
const BACK: f32 = -2.7;
const BUBBLE_XZ: vec2f = vec2f(-0.86, -1.05);

struct Look {
    light: vec3f,     // the strip light, just under the surface
    ldir: vec3f,      // its direction (toward the light)
    amb: vec3f,       // diffuse light in the water
    sig: vec3f,       // extinction per metre
    water: vec3f,     // in-scatter colour of the water
    shafts: f32,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            l.light = vec3f(1.5, 0.95, 0.55) * 1.6;
            l.amb = vec3f(0.07, 0.07, 0.08);
            l.sig = vec3f(0.42, 0.2, 0.16);
            l.water = vec3f(0.05, 0.045, 0.045);
            l.shafts = 0.9; l.exposure = 0.3;
        }
        case 2u: {
            l.light = vec3f(0.35, 0.5, 1.0) * 1.3;
            l.amb = vec3f(0.015, 0.03, 0.07);
            l.sig = vec3f(0.55, 0.3, 0.14);
            l.water = vec3f(0.01, 0.025, 0.06);
            l.shafts = 1.1; l.exposure = 0.55;
        }
        default: {
            l.light = vec3f(1.0, 1.0, 0.92) * 2.4;
            l.amb = vec3f(0.11, 0.19, 0.27);
            l.sig = vec3f(0.3, 0.13, 0.09);
            l.water = vec3f(0.035, 0.1, 0.17);
            l.shafts = 0.7; l.exposure = 0.1;
        }
    }
    l.ldir = normalize(vec3f(0.18, 1.0, 0.3));
    return l;
}

// ------------------------------------------------------------------ the tank

fn sand_h(xz: vec2f) -> f32 {
    // ripples the current combed into the sand, and a few low dunes
    let r = sin(xz.x * 21.0 + noise_value2(xz * 3.0) * 4.0 + xz.y * 2.0);
    return SAND + 0.008 * r + 0.03 * (noise_value2(xz * 1.6 + 3.0) - 0.5) + 0.05 * (noise_value2(xz * 0.5) - 0.5);
}

struct Rock { c: vec3f, r: vec3f }

fn rock_at(i: i32) -> Rock {
    switch (i) {
        case 0: { return Rock(vec3f(0.62, SAND + 0.05, -1.25), vec3f(0.34, 0.24, 0.26)); }
        case 1: { return Rock(vec3f(0.88, SAND + 0.02, -0.95), vec3f(0.2, 0.16, 0.17)); }
        case 2: { return Rock(vec3f(-1.35, SAND + 0.06, -1.4), vec3f(0.42, 0.3, 0.3)); }
        case 3: { return Rock(vec3f(-0.2, SAND + 0.0, -1.9), vec3f(0.26, 0.19, 0.22)); }
        case 4: { return Rock(vec3f(1.9, SAND + 0.03, -1.6), vec3f(0.3, 0.22, 0.24)); }
        default: { return Rock(vec3f(-2.4, SAND + 0.04, -1.1), vec3f(0.28, 0.2, 0.22)); }
    }
}

// a clump of vallisneria: (base x, base z, height, blades)
fn clump_at(i: i32) -> vec4f {
    switch (i) {
        case 0: { return vec4f(-0.55, -1.55, 1.05, 5.0); }
        case 1: { return vec4f(1.25, -1.75, 0.9, 4.0); }
        case 2: { return vec4f(-1.75, -0.85, 0.75, 4.0); }
        case 3: { return vec4f(0.25, -2.1, 1.1, 5.0); }
        case 4: { return vec4f(2.5, -1.2, 0.85, 4.0); }
        default: { return vec4f(-2.7, -1.9, 1.0, 4.0); }
    }
}

// one clump's blades: thin ribbons leaning with the current
fn clump(p: vec3f, c: vec4f, t: f32, ctx: Ctx) -> f32 {
    let q = p - vec3f(c.x, SAND, c.y);
    // bounding
    let bb = length(vec3f(q.x, (q.y - c.z * 0.5) * 0.6, q.z)) - c.z * 0.5 - 0.2;
    if (bb > 0.05) { return bb; }
    var d = 1e5;
    let n = i32(c.w);
    for (var j = 0; j < 5; j++) {
        if (j >= n) { break; }
        let h = hash_cell2(vec2i(j, i32(c.x * 10.0)), 0x5eedu);
        let bx = (h.x - 0.5) * 0.16;
        let bz = (h.y - 0.5) * 0.16;
        let hgt = c.z * (0.7 + 0.5 * h.z);
        let y = clamp(q.y, 0.0, hgt);
        let f = y / hgt;
        // the current bends the top over and ripples along the blade
        let lean = (0.12 + 0.08 * h.w) * f * f + 0.035 * f * sin(t * 1.1 + h.w * 6.0 + y * 4.0);
        let dx = abs(q.x - bx - lean) - 0.014 * (1.0 - f * 0.6);
        let dz = abs(q.z - bz - lean * 0.3) - 0.003;
        var b = max(dx, dz);
        b = max(b, q.y - hgt);
        b = max(b, -q.y);
        d = min(d, b);
    }
    return d;
}

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    if (p.y > SURFACE + 0.5) { return vec2f(p.y - SURFACE - 0.3, 1.0); }
    if (p.z < BACK) { return vec2f(BACK - p.z + 0.02, 6.0); }
    var d = p.y - sand_h(p.xz);
    var m = 1.0;
    let back = p.z - BACK;
    if (back < d) { d = back; m = 6.0; }
    for (var i = 0; i < 6; i++) {
        let r = rock_at(i);
        let q = p - r.c;
        var dr = sdf_ellipsoid(q, r.r);
        if (dr > 0.09) { if (dr < d) { d = dr; m = 2.0; } continue; }
        dr -= (noise_value3(q * 9.0 + f32(i)) - 0.5) * 0.06 + (noise_value3(q * 25.0) - 0.5) * 0.015;
        if (dr < d) { d = dr; m = 2.0; }
    }
    // driftwood: a bent branch resting on the rocks
    let w1 = sdf_capsule(p, vec3f(0.25, SAND + 0.02, -1.0), vec3f(0.75, SAND + 0.38, -1.3), 0.035);
    let w2 = sdf_capsule(p, vec3f(0.75, SAND + 0.38, -1.3), vec3f(1.15, SAND + 0.5, -1.65), 0.028);
    let w3 = sdf_capsule(p, vec3f(0.7, SAND + 0.34, -1.28), vec3f(0.9, SAND + 0.62, -1.15), 0.016);
    var wood = min(w1, min(w2, w3));
    if (wood < 0.04) { wood += (noise_value3(p * 30.0) - 0.5) * 0.008; }
    if (wood < d) { d = wood; m = 3.0; }
    let nclumps = i32(3.0 + ctx.params.y * 3.0);
    for (var i = 0; i < 6; i++) {
        if (i >= nclumps) { break; }
        let dc = clump(p, clump_at(i), ctx.t, ctx);
        if (dc < d) { d = dc; m = 4.0; }
    }
    // a bushy anubias on the big rock, and a mound of moss by the wood
    var b1 = op_smin(length(p - vec3f(-1.3, SAND + 0.36, -1.3)) - 0.17, length(p - vec3f(-1.12, SAND + 0.3, -1.2)) - 0.13, 0.08);
    if (b1 < 0.06) { b1 += (noise_value3(p * 18.0) - 0.5) * 0.05; }
    var b2 = length(p - vec3f(0.45, SAND + 0.08, -0.9)) - 0.13;
    if (b2 < 0.05) { b2 += (noise_value3(p * 28.0) - 0.5) * 0.04; }
    let bush = min(b1, b2);
    if (bush < d) { d = bush; m = 5.0; }
    return vec2f(d, m);
}

// ------------------------------------------------------------------ fish

struct Fish { c: vec3f, fw: vec3f, rt: vec3f, up: vec3f, len: f32, kind: u32, wag: f32 }

fn fish_get(i: i32, t: f32) -> Fish {
    var f: Fish;
    let h = hash_cell2(vec2i(i, 4), 0xf15eu);
    f.kind = u32(i) % 4u;
    // the tetras school: one loop, small offsets
    let school = f.kind == 3u;
    var centre = vec3f(0.0, -0.05, -1.3);
    var ax = 1.6;
    var az = 0.55;
    var w = 0.11;
    var ph = h.x * TAU;
    switch (f.kind) {
        case 0u: {
            f.len = 0.1; centre = vec3f(0.3 + (h.y - 0.5) * 0.6, -0.12 + h.z * 0.25, -1.0); ax = 0.9 + h.y; az = 0.35; w = 0.16 + h.w * 0.05;
            // the first one patrols right behind the glass
            if (i == 0) { f.len = 0.13; centre = vec3f(0.1, -0.08, -0.5); ax = 1.4; az = 0.18; w = 0.13; }
        }
        case 1u: { f.len = 0.16; centre = vec3f(-0.4 + (h.y - 0.5) * 0.8, 0.05 + h.z * 0.25, -1.45); ax = 1.5 + h.y * 0.7; az = 0.5; w = -0.09 - h.w * 0.04; }
        case 2u: { f.len = 0.13; centre = vec3f((h.y - 0.5) * 1.2, -0.25 + h.z * 0.3, -0.85); ax = 1.9; az = 0.3; w = 0.12 + h.w * 0.03; }
        default: { f.len = 0.045; centre = vec3f(0.6, 0.18, -1.7); ax = 1.1; az = 0.4; w = 0.19; ph = 0.3 + f32(i) * 0.09; centre.y += f32(i % 3) * 0.04; }
    }
    let a = w * t + ph;
    let bob = 0.03 * sin(t * 0.7 + ph * 3.0);
    f.c = centre + vec3f(ax * cos(a), bob, az * sin(a));
    var v = vec3f(-ax * sin(a), 0.6 * 0.03 * cos(t * 0.7 + ph * 3.0), az * cos(a)) * sign(w);
    f.fw = normalize(v);
    f.rt = normalize(cross(f.fw, vec3f(0.0, 1.0, 0.0)));
    f.up = cross(f.rt, f.fw);
    f.wag = sin(t * (7.0 + 2.0 * h.z) + ph * 4.0) * 0.35;
    if (school) { f.wag = sin(t * 11.0 + f32(i)) * 0.3; }
    return f;
}

// species colouring at a point of the body, local coordinates in body lengths
fn fish_albedo(kind: u32, lp: vec3f, l: Look) -> vec3f {
    switch (kind) {
        case 0u: {
            // clownfish: orange, three white bars edged black
            let bar = smoothstep(0.06, 0.02, abs(abs(lp.x - 0.02) - 0.2)) + smoothstep(0.06, 0.02, abs(lp.x + 0.36));
            let edge = smoothstep(0.1, 0.06, abs(abs(lp.x - 0.02) - 0.2)) - bar;
            var c = mix(col_hex(0xf07a18u), vec3f(0.95), saturate(bar));
            return mix(c, vec3f(0.05), saturate(edge) * 0.8);
        }
        case 1u: {
            // blue tang: royal blue, the black palette mark, a yellow tail
            let mark = smoothstep(0.08, 0.04, abs(lp.y - 0.04 + lp.x * 0.3)) * smoothstep(-0.3, -0.1, lp.x) * smoothstep(0.42, 0.3, lp.x);
            var c = mix(col_hex(0x1a4de0u), vec3f(0.03), mark);
            return mix(c, col_hex(0xf2d21bu), smoothstep(-0.38, -0.46, lp.x));
        }
        case 2u: {
            return col_hex(0xf5d216u) * (0.85 + 0.3 * smoothstep(0.0, 0.2, lp.y));
        }
        default: {
            // neon tetra: silver, a blue line, red below the tail
            let line = smoothstep(0.05, 0.015, abs(lp.y - 0.03));
            let red = smoothstep(0.0, -0.15, lp.x) * smoothstep(0.1, -0.05, lp.y);
            var c = mix(vec3f(0.6, 0.62, 0.6), col_hex(0x3fd6ffu) * 1.5, line);
            return mix(c, col_hex(0xe0202au), red * (1.0 - line));
        }
    }
}

// the ray against one fish: (t, nx, ny, nz) in world space; t < 0 for a miss
fn fish_hit(ro: vec3f, rd: vec3f, f: Fish) -> vec4f {
    let o = ro - f.c;
    let lo = vec3f(dot(o, f.fw), dot(o, f.up), dot(o, f.rt));
    let ld = vec3f(dot(rd, f.fw), dot(rd, f.up), dot(rd, f.rt));
    // the body: an ellipsoid, a little deeper than wide
    let r = vec3f(f.len * 0.5, f.len * 0.2, f.len * 0.1);
    let so = lo / r;
    let sd = ld / r;
    let a = dot(sd, sd);
    let b = dot(so, sd);
    let c = dot(so, so) - 1.0;
    let disc = b * b - a * c;
    var best = -1.0;
    var n = vec3f(0.0);
    if (disc > 0.0) {
        let tb = (-b - sqrt(disc)) / a;
        if (tb > 0.0) {
            best = tb;
            let lp = lo + ld * tb;
            n = normalize(lp / (r * r));
        }
    }
    // the tail fin: a triangle on a plane that swings with the wag
    let s = f.wag;
    let denom = ld.z + s * ld.x;
    if (abs(denom) > 1e-5) {
        let tt = -(lo.z + s * lo.x + s * f.len * 0.4) / denom;
        if (tt > 0.0 && (best < 0.0 || tt < best)) {
            let lp = lo + ld * tt;
            let q = vec2f(lp.y, lp.x + f.len * 0.78);
            let d = sdf2_tri(q, f.len * 0.2, f.len * 0.34);
            if (d < 0.0) {
                best = tt;
                n = normalize(vec3f(s, 0.0, 1.0)) * -sign(denom);
            }
        }
    }
    // the dorsal fin, in the body's own plane
    if (abs(ld.z) > 1e-5) {
        let tt = -lo.z / ld.z;
        if (tt > 0.0 && (best < 0.0 || tt < best)) {
            let lp = lo + ld * tt;
            let q = vec2f(lp.x - f.len * 0.02, lp.y - f.len * 0.17);
            let d = sdf2_tri(q, f.len * 0.2, f.len * 0.17);
            if (d < 0.0) {
                best = tt;
                n = vec3f(0.0, 0.0, -sign(ld.z));
            }
        }
    }
    if (best < 0.0) { return vec4f(-1.0); }
    let wn = f.fw * n.x + f.up * n.y + f.rt * n.z;
    return vec4f(best, wn);
}

// ------------------------------------------------------------------ light

fn caustic(p: vec3f, t: f32) -> f32 {
    // the pattern projects down from the surface, drifting with the ripples
    return water_caustics(p.xz * 2.8 + vec2f(p.y * 0.15, 0.0), t * 0.9);
}

// light arriving at a surface point from the strip light, with the caustic
// network and a short shadow march
fn direct(p: vec3f, n: vec3f, l: Look, ctx: Ctx, near: bool) -> vec3f {
    let dif = saturate(dot(n, l.ldir));
    if (dif <= 0.0) { return vec3f(0.0); }
    var sh = 1.0;
    if (near) {
        var t = 0.03;
        for (var i = 0; i < 8; i++) {
            let h = map(p + l.ldir * t, ctx).x;
            sh = min(sh, 8.0 * h / t);
            if (sh < 0.03) { break; }
            t += clamp(h, 0.02, 0.2);
            if (t > 1.2) { break; }
        }
        sh = saturate(sh);
    }
    let ca = 0.45 + 0.8 * caustic(p, ctx.t) * (0.5 + ctx.params.z);
    // the light falls off below the surface
    let depth = saturate((SURFACE - p.y) / 1.3);
    let reach = exp(-l.sig * depth * 1.6);
    return l.light * reach * dif * sh * ca;
}

fn shade(p: vec3f, rd: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = rm_normal(p, t, ctx);
    var alb: vec3f;
    var rough = 0.6;
    if (mat < 1.5) {
        // sand: pale grains, darker in the ripple troughs, a little algae
        let grain = noise_value2(p.xz * 60.0);
        alb = mix(col_hex(0xd9c49au), col_hex(0xa89270u), grain * 0.7);
        alb = mix(alb, col_hex(0x5a6e3au), smoothstep(0.6, 0.8, noise_value2(p.xz * 4.0 + 9.0)) * 0.35);
    } else if (mat < 2.5) {
        // rock: dark basalt with lighter veins and green algae up top
        let vein = smoothstep(0.55, 0.7, noise_value3(p * 14.0));
        alb = mix(col_hex(0x45413eu), col_hex(0x5e5750u), vein);
        alb = mix(alb, col_hex(0x3c6b2cu), smoothstep(0.3, 0.8, n.y) * smoothstep(0.4, 0.7, noise_value3(p * 6.0)) * 0.8);
        rough = 0.5;
    } else if (mat < 3.5) {
        // driftwood
        let rings = 0.75 + 0.25 * sin(p.x * 90.0 + p.y * 40.0 + noise_value3(p * 20.0) * 6.0);
        alb = col_hex(0x5e4630u) * rings;
        rough = 0.7;
    } else if (mat < 4.5) {
        // vallisneria: bright green, translucent toward the light
        alb = mix(col_hex(0x4f9a2eu), col_hex(0x8cc84au), noise_value3(p * 30.0));
        rough = 0.35;
    } else if (mat < 5.5) {
        alb = mix(col_hex(0x1f4f22u), col_hex(0x3f7a2eu), noise_value3(p * 40.0));
    } else {
        // the painted back: deep blue, almost lost in the water anyway
        alb = col_hex(0x0a1c33u);
    }
    // the backdrop is a print behind the back glass: no caustics on it
    if (mat > 5.5) { return alb * l.amb * 1.5; }
    // shadows and occlusion where they show: on the sand and the rocks
    let near = t < 2.2 && mat < 2.5;
    var c = alb * direct(p, n, l, ctx, near);
    var occ = 1.0;
    if (near) { occ = rm_ao(p, n, ctx); }
    c += alb * l.amb * occ * (0.6 + 0.4 * saturate(n.y));
    // leaves lit from behind glow
    if (mat > 3.5 && mat < 4.5) { c += alb * l.light * 0.08 * saturate(-dot(n, l.ldir)) * exp(-(SURFACE - p.y) * 1.2); }
    c += l.light * light_ggx(n, -rd, l.ldir, rough) * 0.08 * exp(-(SURFACE - p.y) * 1.5);
    return c;
}

fn shade_fish(p: vec3f, rd: vec3f, n: vec3f, f: Fish, l: Look, ctx: Ctx) -> vec3f {
    let o = p - f.c;
    let lp = vec3f(dot(o, f.fw), dot(o, f.up), dot(o, f.rt)) / f.len;
    var alb = fish_albedo(f.kind, lp, l);
    // the eye
    let eye = smoothstep(0.05, 0.03, length(vec2f(lp.x - 0.34, lp.y - 0.05)));
    alb = mix(alb, vec3f(0.02), eye);
    alb = mix(alb, vec3f(0.9), smoothstep(0.02, 0.01, length(vec2f(lp.x - 0.335, lp.y - 0.055))) * 0.8);
    // belly paler
    alb = mix(alb, alb * 1.3 + 0.1, smoothstep(0.0, -0.15, lp.y) * 0.5);
    let depth = saturate((SURFACE - p.y) / 1.3);
    let reach = exp(-l.sig * depth * 1.6);
    let ca = 0.5 + 0.7 * caustic(p, ctx.t);
    var c = alb * (l.light * reach * saturate(dot(n, l.ldir)) * ca * 0.9 + l.amb * (0.6 + 0.4 * n.y));
    // scales: a tight iridescent highlight
    c += l.light * reach * light_ggx(n, -rd, l.ldir, 0.25) * 0.3 * mix(vec3f(1.0), vec3f(0.6, 0.9, 1.3), 0.5);
    return c;
}

// the strip light's shafts: how much lit water the ray passes through
fn shafts(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx) -> f32 {
    var acc = 0.0;
    let n = 4;
    let dt = min(tmax, 2.8) / f32(n);
    for (var i = 0; i < 4; i++) {
        let t = dt * (f32(i) + ctx.jitter);
        let p = ro + rd * t;
        if (p.y > SURFACE) { continue; }
        let ca = caustic(p, ctx.t);
        acc += ca * exp(-(SURFACE - p.y) * 1.1) * exp(-t * 0.5) * dt;
    }
    return acc;
}

// the bubble column: (brightness, alpha, t)
fn bubbles(ro: vec3f, rd: vec3f, tmax: f32, ctx: Ctx) -> vec3f {
    let oc = ro.xz - BUBBLE_XZ;
    let a = dot(rd.xz, rd.xz);
    if (a < 1e-6) { return vec3f(0.0, 0.0, tmax); }
    let t = -dot(oc, rd.xz) / a;
    if (t < 0.0 || t > tmax) { return vec3f(0.0, 0.0, tmax); }
    let q = ro + rd * t;
    if (q.y < SAND || q.y > SURFACE) { return vec3f(0.0, 0.0, tmax); }
    let r = length(q.xz - BUBBLE_XZ);
    if (r > 0.08) { return vec3f(0.0, 0.0, tmax); }
    var m = 0.0;
    for (var k = 0; k < 3; k++) {
        let rise = 0.22 + 0.06 * f32(k);
        let y = q.y - SAND - ctx.t * rise;
        let cell = floor(y * 14.0 + f32(k) * 0.37);
        let h = hash_cell2(vec2i(i32(cell), k), 0xb0bu);
        if (h.w > 0.68) { continue; }
        let cy = (cell + 0.3 + 0.4 * h.x) / 14.0;
        let wob = sin((q.y - SAND) * 9.0 + h.y * 7.0 + ctx.t * 2.0) * 0.02;
        let dx = (q.x - BUBBLE_XZ.x) - (h.z - 0.5) * 0.06 - wob;
        let dy = y - cy;
        let size = 0.0035 + 0.005 * h.y * (q.y - SAND);
        let d = length(vec2f(dx, dy));
        // a ring: the bubble's edge catches the light, its middle shows through
        m = max(m, smoothstep(size + ctx.px * 1.5, size, d) * (0.35 + 0.65 * smoothstep(size * 0.4, size * 0.9, d)));
    }
    return vec3f(m, m * 0.6, t);
}

// the surface seen from below: Snell's window with the light bar, total
// internal reflection outside it
fn surface(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let t = (SURFACE - ro.y) / rd.y;
    let p = ro + rd * t;
    let e = 0.02;
    let hx = noise_value2(p.xz * 7.0 + vec2f(ctx.t * 0.5, 0.0) + vec2f(e, 0.0)) - noise_value2(p.xz * 7.0 + vec2f(ctx.t * 0.5, 0.0) - vec2f(e, 0.0));
    let hz = noise_value2(p.xz * 7.0 + vec2f(ctx.t * 0.5, 0.0) + vec2f(0.0, e)) - noise_value2(p.xz * 7.0 + vec2f(ctx.t * 0.5, 0.0) - vec2f(0.0, e));
    let n = normalize(vec3f(-hx * 0.6, 1.0, -hz * 0.6));
    let cos_t = dot(rd, n);
    // the window: through it, the strip light and the dim room
    let window = smoothstep(0.62, 0.72, cos_t);
    let bar = smoothstep(0.35, 0.2, abs(p.z + 0.9)) * smoothstep(3.5, 3.2, abs(p.x));
    var above = l.light * (0.08 + bar * 0.9) + vec3f(0.04, 0.045, 0.05);
    // outside it the surface mirrors the tank: the backdrop, a hint of sand
    let below = col_hex(0x0a1c33u) * l.amb * 1.5 + col_hex(0xc8b48cu) * l.light * 0.015 * saturate(1.0 - cos_t * 2.0);
    // (the water between is added by the scene, as for everything else)
    return mix(below, above, window);
}

// ------------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let shift = cam_depth_offset(ctx) * 0.4;
    let ro = vec3f(0.0, 0.04, 0.05) + shift;
    var cam = cam_look_at(ro, ro + vec3f(0.0, -0.03, -1.0), 0.0, 58.0);
    cam = cam_portal(cam, ctx, 1.3);
    let rd = cam_ray(cam, p);
    var tmax = 6.0;
    if (rd.y > 0.0) { tmax = min(tmax, (SURFACE - ro.y) / rd.y); }
    let hit = rm_march_k(ro, rd, 0.02, tmax, steps(52.0, ctx), 0.8, ctx);
    var t_scene = tmax;
    var col: vec3f;
    if (hit.x > 0.0) {
        t_scene = hit.x;
        col = shade(ro + rd * hit.x, rd, hit.x, hit.y, l, ctx);
    } else if (rd.y > 0.0) {
        col = surface(ro, rd, l, ctx);
    } else {
        col = l.water * 1.5;
    }
    // the fish, nearest first
    let nfish = i32(4.0 + ctx.params.x * 8.0);
    var t_fish = t_scene;
    var fish_col = vec3f(0.0);
    var got = false;
    for (var i = 0; i < 12; i++) {
        if (i >= nfish) { break; }
        let f = fish_get(i, ctx.t);
        // a cheap reject: the fish's bounding sphere
        let oc = ro - f.c;
        let b = dot(oc, rd);
        if (b * b - dot(oc, oc) + sq(f.len * 0.85) < 0.0) { continue; }
        let h = fish_hit(ro, rd, f);
        if (h.x > 0.0 && h.x < t_fish) {
            t_fish = h.x;
            fish_col = shade_fish(ro + rd * h.x, rd, h.yzw, f, l, ctx);
            got = true;
        }
    }
    if (got) {
        col = fish_col;
        t_scene = t_fish;
    } else if (hit.x > 0.0 && hit.y < 1.5) {
        // the fish's shadows drift over the sand
        let sp = ro + rd * hit.x;
        var sh = 1.0;
        for (var i = 0; i < 12; i++) {
            if (i >= nfish) { break; }
            let f = fish_get(i, ctx.t);
            let dxz = length((sp.xz - f.c.xz + l.ldir.xz * (f.c.y - sp.y)) / vec2f(1.0, 0.45));
            sh *= 1.0 - 0.45 * smoothstep(f.len * 0.7, f.len * 0.3, dxz) * smoothstep(0.0, 0.3, f.c.y - sp.y);
        }
        col *= sh;
    }
    // bubbles
    let bub = bubbles(ro, rd, t_scene, ctx);
    col = mix(col, l.light * 0.5 + l.amb * 2.0, bub.y);
    // the water between: extinction and in-scatter, and the light's shafts
    let tr = exp(-l.sig * t_scene);
    col = col * tr + l.water * (1.0 - tr) * 1.4;
    col += l.light * shafts(ro, rd, t_scene, l, ctx) * 0.09 * l.shafts;
    // suspended specks near the glass
    let sp = p * 40.0 + vec2f(ctx.t * 0.15, -ctx.t * 0.25);
    let sc = hash_cell2(vec2i(floor(sp)), 0x5e3cu);
    if (sc.w < 0.05) {
        let dd = length(fract(sp) - sc.xy);
        col += l.light * 0.08 * smoothstep(0.12, 0.04, dd) * (1.0 - exp(-0.3));
    }
    // a touch of the room's reflection in the front glass, strongest where
    // the water is darkest
    col += vec3f(0.012, 0.013, 0.015) * exp(-length(p * vec2f(0.6, 1.0) - vec2f(0.0, 0.3)) * 1.5);
    col *= 1.0 + 0.05 * ctx.focus_pulse * ctx.reaction * exp(-length(p - ctx.focus) * 3.0);
    return col * exp2(l.exposure);
}
