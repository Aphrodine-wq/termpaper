//! name: icebergs
//! title: Jökulsárlón
//! category: coast
//! tags: iceland, glacier, icebergs, lagoon, aurora, black sand
//! desc: blue icebergs drift in a glacier lagoon below Vatnajökull, ice glinting on black sand
//! themes: day, twilight, aurora
//! uses: camera, sky, stars
//! cost: heavy
//! fallback: frost
//! credits: original

// World units are metres; y up, the lagoon is y = 0. The camera crouches on
// the black-sand shore 1.6 m up, looking north across the lagoon to the
// glacier tongue and the mountains flanking it. Icebergs are convex
// polyhedra (a set of random planes each) with a little melt noise, shaded
// as translucent ice: white where snow and air bubbles scatter, deepening to
// glacier blue toward the waterline, banded by old ash layers.

const CAM: vec3f = vec3f(0.0, 1.6, 0.0);
const NBERG: i32 = 7;

struct Look {
    sun: vec3f,
    sun_c: vec3f,
    amb: vec3f,
    hor: vec3f,
    zen: vec3f,
    glow: vec3f,     // alpenglow on the high snow
    night: f32,
    aurora: f32,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    l.glow = vec3f(0.0);
    l.aurora = 0.0;
    switch (theme) {
        case 1u: {
            // the sun just set: blue shadows, pink light on the high snow
            l.sun = sky_sun_dir(-1.5, -125.0);
            l.sun_c = vec3f(0.0);
            l.amb = vec3f(0.12, 0.13, 0.2);
            l.zen = vec3f(0.07, 0.1, 0.22);
            l.hor = vec3f(0.34, 0.24, 0.3);
            l.glow = vec3f(1.1, 0.42, 0.45);
            l.night = 0.0;
            l.exposure = 0.5;
        }
        case 2u: {
            l.sun = normalize(vec3f(0.2, 0.9, -0.4));
            l.sun_c = vec3f(0.0);
            l.amb = vec3f(0.006, 0.012, 0.012);
            l.zen = vec3f(0.002, 0.004, 0.009);
            l.hor = vec3f(0.006, 0.012, 0.014);
            l.night = 1.0;
            l.aurora = 1.0;
            l.exposure = 1.1;
        }
        default: {
            // soft overcast: the light comes from the whole sky
            l.sun = normalize(vec3f(-0.3, 0.8, 0.5));
            l.sun_c = vec3f(0.18, 0.18, 0.19);
            l.amb = vec3f(0.6, 0.66, 0.74);
            l.zen = vec3f(0.5, 0.56, 0.64);
            l.hor = vec3f(0.76, 0.79, 0.83);
            l.night = 0.0;
            l.exposure = -0.9;
        }
    }
    return l;
}

// ------------------------------------------------------------ icebergs

struct Berg { c: vec3f, r: f32, seed: u32 }

fn berg_get(i: i32) -> Berg {
    switch (i) {
        case 1: { return Berg(vec3f(24.0, 0.0, -88.0), 13.0, 31u); }
        case 2: { return Berg(vec3f(2.0, 0.0, -170.0), 12.0, 57u); }
        case 3: { return Berg(vec3f(-62.0, 0.0, -130.0), 13.0, 92u); }
        case 4: { return Berg(vec3f(46.0, 0.0, -42.0), 4.0, 13u); }
        case 5: { return Berg(vec3f(-3.0, 0.0, -24.0), 1.8, 77u); }
        case 6: { return Berg(vec3f(105.0, 0.0, -215.0), 15.0, 43u); }
        default: { return Berg(vec3f(-15.0, 0.0, -50.0), 9.5, 18u); }
    }
}

// An iceberg: sheer calved walls (an irregular polygon of leaning vertical
// planes), a melt notch at the waterline, and a tilted top broken by a ridge
// or spire, the way blocks look after they roll and melt in the lagoon.
fn berg_sdf(q: vec3f, b: Berg) -> f32 {
    let s = b.seed;
    var tall = hash_f(s * 11u);
    if (s == 18u) { tall = 0.95; }
    let hgt = b.r * mix(0.35, 1.05, tall);
    let notch = 0.12 * b.r * exp(-q.y * q.y / (0.02 * b.r * b.r + 0.05));
    // per-plane variety from one hash per berg (a hash per plane per step
    // was the scene's biggest cost)
    let hb = vec3f(hash_pcg3(vec3u(s, 1u, 3u)) >> vec3u(8u)) * (1.0 / 16777216.0);
    var side = -1e5;
    for (var k = 0; k < 7; k++) {
        let fk = f32(k);
        let j1 = fract(hb.x * 7.31 + fk * 0.618034);
        let j2 = fract(hb.y * 5.17 + fk * 0.754878);
        let j3 = fract(hb.z * 3.73 + fk * 0.569840);
        let a = (fk + 0.6 * j1) * TAU / 7.0;
        let n = vec2f(cos(a), sin(a));
        let lean = (j2 - 0.35) * 0.35;
        let r = b.r * (0.62 + 0.4 * j3);
        side = max(side, dot(q.xz, n) + q.y * lean - r + notch);
    }
    let tilt = (vec2f(hash_f(s * 3u), hash_f(s * 5u)) - 0.5) * mix(0.9, 0.4, tall);
    var top = hgt * 0.75 + dot(q.xz, tilt);
    let ang = hash_f(s * 7u) * TAU;
    let rdir = vec2f(cos(ang), sin(ang));
    let pk = q.xz - rdir * b.r * 0.3 * (hash_f(s * 13u) - 0.5);
    top = min(top, hgt * 1.15 - abs(dot(pk, rdir)) * 0.9 - abs(dot(pk, vec2f(-rdir.y, rdir.x))) * 0.35);
    return max(max(side, q.y - top), -q.y - hgt * 2.0);
}

fn bergs(p: vec3f) -> vec2f {
    var d = vec2f(1e5, -1.0);
    for (var i = 0; i < NBERG; i++) {
        let b = berg_get(i);
        let q = p - b.c;
        let bound = length(q) - b.r * 1.3;
        if (bound > 2.0) { d.x = min(d.x, bound); continue; }
        var bd = berg_sdf(q, b);
        if (bd < 1.5) { bd += 0.035 * b.r * noise_value3(q * (1.4 / b.r)); }
        if (bd < d.x) { d = vec2f(bd, f32(i)); }
    }
    return d;
}

// small clear ice chunks stranded on the black sand
fn chunks(p: vec3f) -> vec2f {
    let cs = 1.1;
    let c = floor(p.xz / cs);
    let h = hash_cell2(vec2i(c), 0xd1a0u);
    if (h.w > 0.3) {
        return vec2f(max(cs * 0.4, 0.2), -1.0);
    }
    let r = 0.05 + 0.2 * h.z * h.z * h.z;
    let ctr = vec3f((c.x + 0.25 + 0.5 * h.x) * cs, r * 0.4 + beach_h(vec2f(c.x * cs, c.y * cs)), (c.y + 0.25 + 0.5 * h.y) * cs);
    var q = p - ctr;
    let ang = h.w * 40.0;
    let xz = rot2(ang) * q.xz;
    q = vec3f(xz.x, q.y, xz.y);
    let d = max(max(abs(q.x) * 0.9 + abs(q.y) * 0.5, abs(q.z) * 0.8 + abs(q.y) * 0.6), abs(q.y) * 1.2 + abs(q.x) * 0.3) - r;
    return vec2f(d * 0.7, 10.0 + h.z);
}

// the black-sand shore sloping into the lagoon
fn beach_h(xz: vec2f) -> f32 {
    return 0.25 - (-xz.y - 4.0) * 0.07 + 0.08 * noise_value2(xz * 0.4) - 0.05 * xz.x * 0.02;
}

// glacier tongue and the mountains flanking it
fn land_h(xz: vec2f) -> f32 { return land_ho(xz, 5); }

fn land_ho(xz: vec2f, oct: i32) -> f32 {
    let z = -xz.y;
    // the glacier: a calving front ~25 m high, rising gently to the ice cap
    let gx = (xz.x - 150.0) / 1700.0;
    let tongue = smoothstep(1.0, 0.7, abs(gx));
    let front = 1450.0 + 120.0 * noise_value2(vec2f(xz.x * 0.004, 1.0));
    let ice = (25.0 + (z - front) * 0.09) * smoothstep(front - 10.0, front + 20.0, z) + 6.0 * noise_value2(xz * 0.02);
    // mountains: dark, ridged, snow-streaked
    let m = noise_ridged2(xz * 0.00045 + vec2f(3.1, 0.0), oct);
    let side = smoothstep(0.55, 1.1, abs(gx));
    let mtn = (180.0 + 1050.0 * m * m) * smoothstep(1800.0, 4200.0, z) * (0.35 + 0.65 * side) + 300.0 * side * smoothstep(1500.0, 2600.0, z);
    return max(ice * tongue - 5.0 * (1.0 - tongue), mtn) - 2.0;
}

fn world(p: vec3f) -> vec2f {
    var d = bergs(p);
    if (p.z > -14.0) {
        if (p.y < 1.2) {
            let bh = (p.y - beach_h(p.xz)) * 0.9;
            if (bh < d.x) { d = vec2f(bh, 20.0); }
            let ch = chunks(p);
            if (ch.x < d.x) { d = ch; }
        } else {
            d.x = min(d.x, p.y - 1.15);
        }
    }
    return d;
}

fn march(ro: vec3f, rd: vec3f, tmax: f32, n: i32) -> vec2f {
    var t = 0.02;
    for (var i = 0; i < 128; i++) {
        if (i >= n) { break; }
        let p = ro + rd * t;
        if (p.y > 45.0 && rd.y > 0.0) { break; }
        let h = world(p);
        if (h.x < 0.0008 * t + 0.001) { return vec2f(t, h.y); }
        t += h.x;
        if (t > tmax) { break; }
    }
    return vec2f(-1.0);
}

fn land_march(ro: vec3f, rd: vec3f, n: i32, oct: i32) -> f32 {
    if (rd.y > 0.3) { return -1.0; }
    if (rd.z > -0.05) { return -1.0; }
    var t = 1300.0 / -rd.z;
    // Nothing out here stands above the line y = 60 + 0.6 (d - 1400), d the
    // distance north: jump straight to where the ray meets it.
    let den = rd.y + 0.6 * rd.z;
    if (den < -1e-4) {
        let tc = (60.0 + 0.6 * (-ro.z - 1400.0) - ro.y) / den;
        t = max(t, tc);
    } else {
        return -1.0;
    }
    for (var i = 0; i < 96; i++) {
        if (i >= n) { break; }
        let p = ro + rd * t;
        if (p.y > 1560.0 && rd.y >= 0.0) { break; }
        let dy = p.y - land_ho(p.xz, oct);
        if (dy < 0.004 * t) { return t; }
        t += max(dy * 0.45, 3.0 + t * 0.003);
        if (t > 12000.0) { break; }
    }
    return -1.0;
}

fn world_normal(p: vec3f, t: f32) -> vec3f {
    let e = max(0.003, t * 0.0012);
    let k = vec2f(1.0, -1.0);
    return normalize(k.xyy * world(p + k.xyy * e).x + k.yyx * world(p + k.yyx * e).x +
                     k.yxy * world(p + k.yxy * e).x + k.xxx * world(p + k.xxx * e).x);
}

// ------------------------------------------------------------ sky

fn aurora(rd: vec3f, ctx: Ctx) -> vec3f {
    if (rd.y < 0.01) { return vec3f(0.0); }
    var acc = vec3f(0.0);
    // sample the curtain through altitude shells: green low, red-violet high
    for (var i = 0; i < 12; i++) {
        let fi = f32(i);
        let hgt = 1.0 + fi * 0.22;
        let q = rd.xz / rd.y * hgt;
        let wav = q.x * 0.6 + 0.8 * noise_value2(vec2f(q.x * 0.35 + ctx.t * 0.035, 2.0)) + 0.25 * sin(q.x * 1.7 + ctx.t * 0.13);
        let band = exp(-sq((q.y + 3.0 + wav * 0.5) * 2.6)) + 0.5 * exp(-sq((q.y + 4.8 + wav * 0.7) * 3.2));
        let rays = 0.45 + 0.9 * noise_value2(vec2f(q.x * 7.0 + wav * 3.0, ctx.t * 0.22));
        let lower = exp(-fi * 0.35);
        let col = mix(vec3f(0.6, 0.12, 0.35), vec3f(0.12, 1.0, 0.35), lower);
        acc += col * band * rays * (0.4 + lower);
    }
    return acc * 0.035 * smoothstep(0.01, 0.12, rd.y);
}

fn backdrop(rd: vec3f, l: Look, ctx: Ctx, full: bool) -> vec3f {
    let y = max(rd.y, 0.0);
    var c = mix(l.hor, l.zen, pow(y, 0.45));
    if (l.night > 0.5) {
        c = sky_night(rd) * 1.2;
        if (full) { c += star_field(rd, 0.5, ctx) * smoothstep(0.02, 0.2, rd.y) * 0.7; }
        c += aurora(rd, ctx) * l.aurora;
    } else if (l.glow.x > 0.0) {
        // twilight: the Belt of Venus over the glacier, earth shadow below it
        let belt = exp(-sq((y - 0.12) * 9.0));
        c += vec3f(0.25, 0.1, 0.14) * belt;
    } else {
        // overcast: soft structure in a low cloud deck
        if (rd.y > 0.0) {
            let q = rd.xz / (rd.y + 0.1) * 2.0;
            let n = noise_fbm2(q * 0.45 + vec2f(ctx.t * 0.012, 0.0), 5);
            c *= 0.74 + 0.42 * n;
        }
    }
    return c;
}

// ------------------------------------------------------------ shading

fn shade_berg(p: vec3f, rd: vec3f, t: f32, id: f32, l: Look, ctx: Ctx) -> vec3f {
    let b = berg_get(i32(id + 0.5));
    let q = p - b.c;
    let sc = 5.0 / b.r;
    let e = 0.2;
    let g = vec3f(noise_value3((q + vec3f(e, 0.0, 0.0)) * sc), noise_value3((q + vec3f(0.0, e, 0.0)) * sc), noise_value3((q + vec3f(0.0, 0.0, e)) * sc)) - noise_value3(q * sc);
    let n = normalize(world_normal(p, t) - g * 1.2);
    // translucent glacier ice: white where snow and bubbles scatter the
    // light (tops, fresh fractures), deep blue lower down and in shade
    let hgt = saturate(p.y / (b.r * 0.9));
    let top = smoothstep(0.5, 0.85, n.y);
    // patches of clear, bubble-free ice glow an intense blue; the rest is
    // white with a cold blue-grey cast
    let clear = smoothstep(0.55, 0.72, noise_value3(q * (1.8 / b.r) + vec3f(f32(b.seed))) + 0.3 * (1.0 - hgt) - 0.12);
    var ice = mix(vec3f(0.86, 0.9, 0.93), col_hex(0x9cc6d8u), 0.35 + 0.3 * (1.0 - n.y));
    ice = mix(ice, col_hex(0x2a86c0u), clear * 0.85);
    ice = mix(ice, col_hex(0x125a94u), clear * smoothstep(0.5, 0.0, hgt) * 0.6);
    ice = mix(ice, vec3f(0.95, 0.96, 0.98), top * 0.8);
    // compressed old ice and ash bands
    let bd = normalize(vec3f(0.3, 1.0, 0.2 + 0.4 * hash_f(b.seed)));
    let bnd = sin(dot(q, bd) * (1.1 + 0.6 * hash_f(b.seed * 3u)) + noise_value3(q * 0.3) * 3.0);
    let ash = smoothstep(0.7, 0.97, bnd) * (0.6 + 0.4 * noise_value3(q * 0.8));
    ice = mix(ice, col_hex(0x3a3d40u), ash * step(0.4, hash_f(b.seed * 5u)) * 0.7);
    // melt scallops
    ice *= 0.9 + 0.15 * noise_value3(q * 1.3);
    let sky = l.amb * (0.6 + 0.4 * n.y);
    var c = ice * (sky + l.sun_c * saturate(dot(n, l.sun)));
    // light soaking through the ice glows from within
    c += col_hex(0x3fa9d8u) * l.amb * 0.25 * (1.0 - top) * (0.5 + 0.5 * hgt);
    // alpenglow catches only the high tops of the tall bergs
    c += ice * l.glow * top * 0.3;
    if (l.aurora > 0.0) { c += ice * vec3f(0.004, 0.014, 0.009) * (0.3 + 0.7 * n.y); }
    // glossy wet ice
    let r = reflect(rd, n);
    let f = 0.02 + 0.98 * pow(1.0 - saturate(dot(-rd, n)), 5.0);
    c += backdrop(r, l, ctx, false) * f * 0.6;
    return c;
}

fn shade_beach(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = world_normal(p, t);
    let wet = smoothstep(0.35, 0.02, p.y);
    var alb = col_hex(0x1a1a1bu) * (0.7 + 0.5 * noise_value2(p.xz * 18.0)) * mix(1.0, 0.55, wet);
    var c = alb * (l.amb * (0.6 + 0.4 * n.y) + l.sun_c * saturate(dot(n, l.sun)));
    // wet sand mirrors the sky
    let r = reflect(rd, n);
    let f = 0.02 + 0.98 * pow(1.0 - saturate(dot(-rd, n)), 5.0);
    c += backdrop(r, l, ctx, false) * f * (0.15 + 0.75 * wet);
    return c;
}

fn shade_chunk(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = world_normal(p, t);
    // clear ice: mostly refraction of the dark sand, bright rims and glints
    let f = 0.06 + 0.94 * pow(1.0 - saturate(dot(-rd, n)), 3.0);
    let r = reflect(rd, n);
    // some shards are milky with trapped air, some glass-clear
    let cid = hash_f(u32(i32(floor(p.x / 1.1) * 7.0 + floor(p.z / 1.1) * 131.0) + 7));
    let milky = step(0.45, cid);
    let body = mix(col_hex(0x4d7f95u) * 0.22, col_hex(0xd8e6ecu) * (0.55 + 0.45 * n.y), milky) * l.amb;
    var c = mix(body, backdrop(r, l, ctx, false) * 1.1, f * (1.0 - milky * 0.5));
    c += vec3f(0.9, 0.95, 1.0) * pow(saturate(dot(r, normalize(vec3f(-0.2, 0.9, -0.4)))), 40.0) * (l.amb.b * 2.0 + l.aurora * 0.15);
    return c;
}

fn shade_land(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let e = max(1.0, t * 0.002);
    let hx = land_h(p.xz + vec2f(e, 0.0)) - land_h(p.xz - vec2f(e, 0.0));
    let hz = land_h(p.xz + vec2f(0.0, e)) - land_h(p.xz - vec2f(0.0, e));
    let n = normalize(vec3f(-hx, 2.0 * e, -hz));
    let gx = (p.x - 150.0) / 1700.0;
    let on_ice = smoothstep(1.0, 0.75, abs(gx)) * smoothstep(3500.0, 2500.0, -p.z + 400.0 * abs(gx));
    // dark basalt with snow on every ledge; the glacier grey-white with
    // blue crevasses and dark moraine stripes
    let snow = smoothstep(0.55, 0.8, n.y + 0.2 * noise_value2(p.xz * 0.01)) * smoothstep(250.0, 500.0, p.y);
    var alb = mix(col_hex(0x2a2929u), vec3f(0.9, 0.92, 0.95), snow);
    let crev = smoothstep(0.6, 0.8, noise_value2(vec2f(p.x * 0.05, p.z * 0.012)));
    let moraine = smoothstep(0.75, 0.85, noise_value2(vec2f(p.x * 0.004, 5.0)));
    var gice = mix(vec3f(0.8, 0.85, 0.88), col_hex(0x5a9cc0u), crev * 0.6);
    gice = mix(gice, col_hex(0x2a2826u), moraine * 0.8);
    alb = mix(alb, gice, on_ice);
    var c = alb * (l.amb * (0.55 + 0.45 * n.y) + l.sun_c * saturate(dot(n, l.sun)));
    // alpenglow on the high snow
    c += alb * l.glow * smoothstep(350.0, 900.0, p.y) * saturate(n.y + 0.2) * (snow + on_ice * 0.6);
    if (l.aurora > 0.0) { c += alb * vec3f(0.004, 0.02, 0.01) * n.y; }
    let fog = 1.0 - exp(-t * 0.0001);
    return mix(c, l.hor * 0.95, fog);
}

fn trace(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx, n: i32, full: bool) -> vec4f {
    let tw = select(1e5, -ro.y / rd.y, rd.y < -1e-4);
    let h = march(ro, rd, min(tw, 600.0), n);
    if (h.x > 0.0) {
        let p = ro + rd * h.x;
        var c: vec3f;
        if (h.y > 19.5) { c = shade_beach(p, rd, h.x, l, ctx); }
        else if (h.y > 9.5) { c = shade_chunk(p, rd, h.x, l, ctx); }
        else { c = shade_berg(p, rd, h.x, h.y, l, ctx); }
        return vec4f(c, h.x);
    }
    if (tw < 1e5) { return vec4f(-1.0, 0.0, 0.0, tw); }
    let tl = land_march(ro, rd, select(28, 64, full), select(3, 4, full));
    if (tl > 0.0) { return vec4f(shade_land(ro + rd * tl, rd, tl, l, ctx), tl); }
    return vec4f(backdrop(rd, l, ctx, full), 1e5);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let cam = cam_look_at(CAM, CAM + vec3f(0.0, 0.02, -1.0), 0.0, 50.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    let v = trace(CAM, rd, l, ctx, steps(96.0, ctx), true);
    if (v.x >= 0.0) {
        col = v.rgb;
    } else {
        // the lagoon: milky glacial water, nearly still
        let tw = v.w;
        let wp = CAM + rd * tw;
        // cat's-paws of breeze drifting across the still lagoon
        let g = vec2f(noise_value2(wp.xz * 0.6 + vec2f(ctx.t * 0.35, ctx.t * 0.1)), noise_value2(wp.xz * 0.6 - vec2f(ctx.t * 0.28, -ctx.t * 0.15) + 7.0)) - 0.5;
        let g2 = vec2f(noise_value2(wp.xz * 2.1 + vec2f(-ctx.t * 0.6, ctx.t * 0.4)), noise_value2(wp.xz * 2.1 + vec2f(ctx.t * 0.5, ctx.t * 0.55) + 3.0)) - 0.5;
        let gust = smoothstep(0.35, 0.75, noise_value2(wp.xz * 0.02 + vec2f(ctx.t * 0.02, 0.0)));
        let n = normalize(vec3f(g.x * 0.035 + g2.x * 0.03 * gust, 1.0, g.y * 0.035 + g2.y * 0.03 * gust));
        var r = reflect(rd, n);
        r.y = abs(r.y) + 0.001;
        let rv = trace(wp + vec3f(0.0, 0.01, 0.0), normalize(r), l, ctx, 48, false);
        var rc = rv.rgb;
        if (rv.x < 0.0) { rc = backdrop(r, l, ctx, false); }
        let f = 0.02 + 0.98 * pow(1.0 - saturate(dot(-rd, n)), 5.0);
        let body = col_hex(0x3f6468u) * l.amb * 0.5;
        col = mix(body, rc, max(f, 0.12));
        let fog = 1.0 - exp(-tw * 0.00014);
        col = mix(col, l.hor * 0.95, fog);
    }
    return col * exp2(l.exposure);
}
