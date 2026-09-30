//! name: terraces
//! title: Rice Terraces
//! category: aerial
//! tags: drone, rice, terraces, paddies, valley, mist, aerial
//! desc: a drone over stepped rice terraces, each paddy a mirror of the sky, mist in the valley
//! themes: dawn, green, mist
//! params: mist, wind
//! uses: camera, raymarch, sky, noise, fog
//! cost: medium
//! fallback: meadow
//! credits: original

// World units are metres, y up. The drone hangs some 60 m over a hillside
// of flooded rice terraces and looks across a valley: the near slope falls
// away under the camera to the valley floor around z = -450, the far slope
// climbs to a ridge around z = -1500. The land is a smooth heightfield
// quantised into steps. Every step is a flat pool of still water held by a
// low mud bund, the risers between the pools are bare earth, and a rim of
// seedlings runs along each lip. Mist pools in the valley floor and thins
// upward; two farmhouses sit high on the far slope, each with a lit window.

struct Look {
    sun: vec3f,
    mist: f32,       // valley fog density (the mist slider scales it)
    mist_fall: f32,  // how fast the fog thins with height
    haze: f32,       // aerial perspective density
    overcast: f32,   // 0 clear sky .. 1 flat grey light
    green: f32,      // how far along, and how saturated, the seedlings are
    sun_c: vec3f,
    amb: vec3f,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        // mid-morning: blue sky, high sun from the right, thin mist
        case 1u: { l = Look(sky_sun_dir(46.0, 118.0), 0.22, 0.05, 0.00028, 0.0, 1.0, vec3f(0.0), vec3f(0.0)); }
        // fog: a white valley, flat grey light, only the near pools clear
        case 2u: { l = Look(sky_sun_dir(30.0, 150.0), 1.2, 0.02, 0.0011, 0.85, 0.45, vec3f(0.0), vec3f(0.0)); }
        // dawn: the sun just over the far ridge, ahead and a little left
        default: { l = Look(sky_sun_dir(13.0, -18.0), 0.5, 0.035, 0.00045, 0.0, 0.7, vec3f(0.0), vec3f(0.0)); }
    }
    l.sun_c = sky_sun_light(l.sun);
    l.amb = sky_ambient(l.sun);
    if (l.sun.y < 0.25) {
        // low sun: warm light, cold blue shadow
        l.sun_c *= vec3f(1.5, 0.95, 0.8) * 1.5;
        l.amb = vec3f(col_luma(l.amb)) * vec3f(0.7, 0.82, 1.3) * 1.3;
    }
    // overcast flattens the sun into a bright grey ambient
    let grey = vec3f(col_luma(l.amb) * 1.5 + col_luma(l.sun_c) * 0.1) * vec3f(0.98, 1.0, 1.02);
    l.amb = mix(l.amb, grey, l.overcast);
    l.sun_c *= 1.0 - 0.9 * l.overcast;
    return l;
}

// ------------------------------------------------------------ terrain

const S0: f32 = 4.0;                          // one terrace step, metres
const SF: f32 = 2.5;                          // far steps are this many times taller
const H0: vec2f = vec2f(-140.0, -720.0);      // farmhouses (x, z), on the facing slope
const H1: vec2f = vec2f(120.0, -790.0);

// the smooth hill before it was cut into terraces
fn base_h(xz: vec2f, ctx: Ctx) -> f32 {
    let d = -xz.y;
    // a short slope drops under the drone to the valley floor, then the far
    // side climbs, facing the drone, gently at first and steeper to the ridge
    var h = -20.0 + 70.0 * pow(sstep(230.0, -80.0, d), 1.4);
    h += 300.0 * pow(smoothstep(230.0, 1150.0, d), 1.25);
    h += 160.0 * smoothstep(1100.0, 2200.0, d);
    // big undulations bend the contour lines round spurs and hollows
    let oct = select(2i, 3i, ctx.detail > 0u);
    let u = noise_fbm2(xz * 0.0016 + ctx.seedf * 5.0 + vec2f(3.1, 7.7), oct);
    h += (u - 0.5) * 80.0 * (0.4 + 0.6 * smoothstep(100.0, 700.0, d));
    // a spur on the far slope, so the contours bulge toward the drone
    h += 30.0 * exp(-sq((xz.x + 60.0) / 220.0)) * smoothstep(250.0, 600.0, d) * sstep(1200.0, 700.0, d);
    return h;
}

// the step size the pattern reads at: 4 m near the drone, 10 m across the
// valley where 4 m bands would be a pixel wide
fn step_w(xz: vec2f) -> f32 { return smoothstep(350.0, 800.0, -xz.y); }
fn step_s(xz: vec2f) -> f32 { return select(S0, S0 * SF, step_w(xz) > 0.5); }

// the hill quantised into pools. Blending two step functions keeps every
// pool flat (a distance-dependent step size would tilt them).
fn terrace_h(xz: vec2f, h: f32) -> f32 {
    let w = step_w(xz);
    let a = floor(h / S0) * S0;
    let b = floor(h / (S0 * SF)) * (S0 * SF);
    return mix(a, b, w);
}

// the surface the drone sees: still water 0.35 m deep in each pool and a
// low mud bund along the downhill lip
fn ground_h(xz: vec2f, ctx: Ctx) -> f32 {
    let h = base_h(xz, ctx);
    let f = fract(h / step_s(xz));
    let bund = 1.0 - smoothstep(0.02, 0.1, f);
    return terrace_h(xz, h) + 0.35 + 0.3 * bund;
}

// a farmhouse: whitewashed walls under a pitched roof, standing on its pool
fn house_d(p: vec3f, c: vec2f, ctx: Ctx) -> f32 {
    let r = length(p.xz - c);
    if (r > 40.0) { return r - 30.0; }
    let base = ground_h(c, ctx);
    let q = vec3f(p.x - c.x, p.y - base, p.z - c.y);
    let bq = abs(q - vec3f(0.0, 1.6, 0.0)) - vec3f(4.6, 1.6, 3.2);
    let body = length(max(bq, vec3f(0.0))) + min(max(bq.x, max(bq.y, bq.z)), 0.0);
    let roof = max(max((abs(q.x) * 0.7 + q.y - 5.0) * 0.82, 3.1 - q.y), abs(q.z) - 3.7);
    return min(body, roof);
}

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    var d = p.y - ground_h(p.xz, ctx);
    var m = 1.0;
    let h0 = house_d(p, H0, ctx);
    if (h0 < d) { d = h0; m = 2.0; }
    let h1 = house_d(p, H1, ctx);
    if (h1 < d) { d = h1; m = 2.0; }
    return vec2f(d, m);
}

// ------------------------------------------------------------ sky and shading

// the sky, or a flat overcast lid; disk = 1 draws the sun itself
fn skydome(rd: vec3f, l: Look, ctx: Ctx, disk: f32) -> vec3f {
    let d = normalize(vec3f(rd.x, max(rd.y, 0.012), rd.z));
    var c = sky_atmosphere(d, l.sun, ctx);
    c += sky_sun_disk(d, l.sun, 1.1) * disk * (1.0 - l.overcast);
    let grey = l.amb * (0.8 + 0.5 * d.y) * vec3f(0.97, 0.985, 1.0);
    return mix(c, grey, l.overcast);
}

// one flooded terrace: mirror water, mud bund, earth riser, seedling rim
fn shade_paddy(q: vec3f, n: vec3f, rd: vec3f, dist: f32, wind: f32, l: Look, ctx: Ctx) -> vec3f {
    let t = ctx.t;
    let h = base_h(q.xz, ctx);
    let s = step_s(q.xz);
    let f = fract(h / s);
    // the smooth hill's gradient: which way is uphill, and how steep
    let e = 3.0;
    let g = vec2f(base_h(q.xz + vec2f(e, 0.0), ctx) - h, base_h(q.xz + vec2f(0.0, e), ctx) - h) / e;
    let slope = length(g);
    let uphill = g / max(slope, 1e-4);

    // a riser: the surface is steep, or the march sank through the wall face
    // and stopped below the pool it landed in (robust to relaxed marching)
    let sink = terrace_h(q.xz, h) + 0.35 - q.y;
    let wall = max(1.0 - smoothstep(0.5, 0.85, n.y), smoothstep(0.25, 0.9, sink));
    let n_wall = normalize(vec3f(-uphill.x, 0.3, -uphill.y));
    let nn = normalize(mix(n, n_wall, wall));
    let bund = 1.0 - smoothstep(0.03, 0.12, f);
    let water = (1.0 - wall) * (1.0 - bund);
    let rim = bund * (1.0 - wall) * smoothstep(0.6, 0.9, n.y) * 0.75;

    // the riser above each pool shades the pool's uphill edge when the sun
    // stands low on the uphill side: a long blue band at dawn, a sliver at noon
    let sun_flat = length(l.sun.xz);
    let sun_up = saturate(dot(l.sun.xz / max(sun_flat, 1e-3), uphill));
    let cover = slope * sun_up * sun_flat / max(l.sun.y, 0.03);
    let band = sstep(cover + 0.06, cover - 0.1, 1.0 - f);
    let sh = 1.0 - 0.85 * band * (1.0 - l.overcast);

    let dif = saturate(dot(nn, l.sun));
    let lit = l.sun_c * dif * sh + l.amb * (0.55 + 0.45 * nn.y) * 1.2;

    // palette
    let earth = col_hex(0x4a3524u);
    let moss = col_hex(0x3f4a26u);
    let rice = mix(col_hex(0x7a8f3eu), col_hex(0x5db02eu), l.green);
    let mud = col_hex(0x2c2e1cu);
    // how far along the seedlings are, varying pool by pool
    var grow = 0.5;
    if (ctx.detail > 0u) { grow = noise_value2(vec2f(q.x * 0.006, floor(h / s) * 0.61 + 11.0)); }
    let under = mix(mud, rice * 0.55, grow * l.green * 0.9);

    let earthc = mix(earth, moss, 0.35 * (1.0 - wall)) * lit;
    let rimc = rice * lit;

    // still water: wind ripples, two travelling trains dying away with distance
    let amp = 0.035 * wind * exp(-dist / 400.0);
    let k1 = vec2f(0.95, 0.4);
    let k2 = vec2f(-0.5, 1.15);
    let ph1 = dot(q.xz, k1) + t * 1.7;
    let ph2 = dot(q.xz, k2) * 0.8 + t * 1.25 + 2.0;
    let grad = amp * (cos(ph1) * k1 + 0.6 * cos(ph2) * k2);
    let nw = normalize(vec3f(-grad.x, 1.0, -grad.y));
    let refl = reflect(rd, nw);
    let fr = 0.04 + 0.96 * pow(1.0 - saturate(dot(-rd, nw)), 5.0);
    let skyr = skydome(refl, l, ctx, 0.0);
    let glint = pow(saturate(dot(refl, l.sun)), 120.0) * l.sun_c * 0.25 * (1.0 - l.overcast);
    let underlit = under * (l.sun_c * saturate(l.sun.y) * sh + l.amb * 1.1);
    let wcol = mix(underlit, skyr + glint, fr);

    var c = mix(earthc, wcol, water);
    c = mix(c, rimc, rim);
    return c;
}

fn shade_house(q: vec3f, n: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let c = select(H0, H1, length(q.xz - H1) < length(q.xz - H0));
    let base = ground_h(c, ctx);
    let lq = vec3f(q.x - c.x, q.y - base, q.z - c.y);
    let roof = smoothstep(0.3, 0.6, n.y);
    let alb = mix(col_hex(0xb9ab8eu), col_hex(0x4a3b30u), roof);
    let dif = saturate(dot(n, l.sun));
    var col = alb * (l.sun_c * dif * 0.9 + l.amb * (0.6 + 0.4 * n.y) * 1.1);
    // one lit window in the wall facing the valley
    let win = (1.0 - smoothstep(0.9, 1.3, abs(lq.x)))
        * (1.0 - smoothstep(0.5, 0.75, abs(lq.y - 1.4)))
        * (1.0 - smoothstep(0.2, 0.5, n.y));
    col += col_hex(0xffb45au) * 6.0 * win * (1.3 - saturate(l.sun.y));
    return col;
}

// ------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let k = ctx.params;
    let mist_k = l.mist * pow(k.x * 2.0, 1.5);
    let wind = k.y * 2.0;

    // the drone: a slow drift of a metre or so, nothing more
    let ro = vec3f(0.0, 90.0, 0.0) + cam_sway(t, 1.2, ctx.seed.x);
    let cam = cam_look_at(ro, vec3f(20.0, 80.0, -650.0), 0.0, 50.0);
    let rd = cam_ray(cam, p);

    // haze is the sky at the horizon in this direction; the mist's own colour
    // is the lit sky toward the sun softened by the ambient
    let hazec = skydome(normalize(vec3f(rd.x, 0.06, rd.z)), l, ctx, 0.0);
    let sunw = skydome(normalize(vec3f(l.sun.x, 0.08, l.sun.z)), l, ctx, 0.0);
    let fogc = mix(sunw * 0.55 + l.amb * 0.6, l.amb * 1.2, l.overcast);

    var hit = vec2f(-1.0, -1.0);
    if (rd.y < 0.42) {
        hit = rm_march_k(ro, rd, 1.0, 2800.0, steps(96.0, ctx), 0.6, ctx);
    }

    var col: vec3f;
    var dist = 2800.0;
    var q = ro + rd * 600.0;   // where the mist is sampled for sky rays
    if (hit.x > 0.0) {
        dist = hit.x;
        q = ro + rd * dist;
        let n = rm_normal(q, dist, ctx);
        if (hit.y > 1.5) {
            col = shade_house(q, n, l, ctx);
        } else {
            col = shade_paddy(q, n, rd, dist, wind, l, ctx);
        }
        // aerial perspective
        col = fog_exp(col, hazec, dist, l.haze);
    } else {
        col = skydome(rd, l, ctx, 1.0);
        // a ray that runs out under the horizon: far terraces lost in the haze
        col = mix(col, hazec, sstep(0.03, -0.02, rd.y));
    }

    // mist lying in the valley, drifting slowly, thinning with height
    var drift = 1.0;
    if (ctx.detail > 0u) {
        drift = 0.7 + 0.6 * noise_fbm2(q.xz * 0.0022 + vec2f(t * 0.011, t * 0.006), 2);
    }
    col = fog_height(col, fogc, ro, rd, dist, mist_k * drift * 0.003, l.mist_fall);
    // the glow the mist picks up looking toward a low sun
    let glow = 0.06 * (1.0 - exp(-dist * 0.0015)) * (1.0 - l.overcast) * (0.3 + 0.7 * saturate(mist_k));
    col += fog_sun(rd, l.sun, l.sun_c, glow);
    return max(col, vec3f(0.0));
}
