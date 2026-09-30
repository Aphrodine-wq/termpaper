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

// World units are metres, y up. The drone hangs 60 m over a terraced
// hillside and looks across it, pitched about a third of the way down. The
// hill rises to the right and away, rounds over into a ridge 300-400 m
// ahead and falls into the next valley behind it; to the left the slope
// runs down into a low valley floor where the mist lies. The smooth hill
// is quantised into 5 m steps: every step is a flat pool of still water,
// a bright rim of seedlings runs along its front lip, and a dark earth
// riser climbs to the next pool. Two farmhouses and a few palms sit on the
// mid slope, and a footpath zigzags up the risers.

struct Look {
    sun: vec3f,
    mist: f32,       // valley fog density (the mist slider scales it)
    haze: f32,       // aerial perspective density
    overcast: f32,   // 0 clear sky .. 1 flat grey light
    green: f32,      // how far along, and how saturated, the seedlings are
    expo: f32,       // exposure compensation
    sun_c: vec3f,
    amb: vec3f,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        // mid-morning: blue sky, high sun from the upper left, pools reflect blue
        case 1u: { l = Look(sky_sun_dir(52.0, -70.0), 0.004, 0.00030, 0.0, 1.0, 0.85, vec3f(0.0), vec3f(0.0)); }
        // mist: soft light from the right, fog pooled in the valley floor only
        case 2u: { l = Look(sky_sun_dir(22.0, 120.0), 0.07, 0.00050, 0.55, 0.6, 0.9, vec3f(0.0), vec3f(0.0)); }
        // dawn: the sun just over the ridge ahead-right, the slope in cool shadow
        default: { l = Look(sky_sun_dir(6.0, 38.0), 0.014, 0.00040, 0.0, 0.7, 0.7, vec3f(0.0), vec3f(0.0)); }
    }
    l.sun_c = sky_sun_light(l.sun);
    l.amb = sky_ambient(l.sun);
    if (l.sun.y < 0.25) {
        // low sun: warm light, cold blue shadow
        l.sun_c *= vec3f(1.4, 0.9, 0.7);
        l.amb = vec3f(col_luma(l.amb)) * vec3f(0.75, 0.85, 1.25);
    }
    let grey = vec3f(col_luma(l.amb) * 1.4 + col_luma(l.sun_c) * 0.08);
    l.amb = mix(l.amb, grey, l.overcast);
    l.sun_c *= 1.0 - 0.85 * l.overcast;
    return l;
}

// ------------------------------------------------------------ terrain

const STEP: f32 = 5.0;                        // one terrace step, metres
const UP: vec2f = vec2f(0.8, -0.6);           // uphill direction in xz: right and away
const FOOT: f32 = 50.0;                       // ground height under the drone
const H0: vec2f = vec2f(55.0, -175.0);        // farmhouses (x, z)
const H1: vec2f = vec2f(-60.0, -235.0);
const P0: vec2f = vec2f(-20.0, -105.0);       // palms
const P1: vec2f = vec2f(95.0, -150.0);
const P2: vec2f = vec2f(-110.0, -190.0);

// the smooth hill before it was cut into terraces
fn base_h(xz: vec2f, ctx: Ctx) -> f32 {
    let s = dot(xz, UP);
    // a 1:5 slope up to the ridge, steeper down into the valley on the left
    let hs = select(0.2 * s, 0.32 * s, s < 0.0);
    // round the crest over at 34 m
    let x = clamp((hs - 34.0) / 8.0, -30.0, 30.0);
    var h = hs - 8.0 * log(1.0 + exp(x));
    // behind the ridge the land falls into the next valley
    h -= 280.0 * smoothstep(250.0, 480.0, s);
    // the whole hillside rises gently to the right
    h += 0.03 * xz.x;
    // spurs and hollows bend the contour lines
    let u = noise_fbm2(xz * 0.0042 + ctx.seedf * 3.0 + vec2f(2.3, 5.1), 4);
    h += (u - 0.5) * 30.0;
    return FOOT + h;
}

// the surface the drone sees: flat pools 0.3 m deep, a low bund along the
// front lip, and a short steep ramp up to the next pool (a ramp rather than
// a vertical wall keeps the march robust and gives the riser a normal)
fn ground_h(xz: vec2f, ctx: Ctx) -> f32 {
    let h = base_h(xz, ctx);
    let f = fract(h / STEP);
    let riser = STEP * smoothstep(0.86, 1.0, f);
    let bund = 0.45 * (1.0 - smoothstep(0.0, 0.07, f));
    return floor(h / STEP) * STEP + riser + bund + 0.3;
}

// a farmhouse: dark timber walls under a pitched roof, standing on its pool
fn house_d(p: vec3f, c: vec2f, ctx: Ctx) -> f32 {
    let r = length(p.xz - c);
    if (r > 30.0) { return r - 22.0; }
    let base = ground_h(c, ctx);
    let q = vec3f(p.x - c.x, p.y - base, p.z - c.y);
    let bq = abs(q - vec3f(0.0, 1.5, 0.0)) - vec3f(4.5, 1.5, 3.2);
    let body = length(max(bq, vec3f(0.0))) + min(max(bq.x, max(bq.y, bq.z)), 0.0);
    let roof = max(max((abs(q.x) * 0.75 + q.y - 5.2) * 0.8, 2.9 - q.y), abs(q.z) - 3.8);
    return min(body, roof);
}

// a palm: a thin trunk and a round crown
fn palm_d(p: vec3f, c: vec2f, ctx: Ctx) -> f32 {
    let r = length(p.xz - c);
    if (r > 20.0) { return r - 14.0; }
    let base = ground_h(c, ctx);
    let q = vec3f(p.x - c.x, p.y - base, p.z - c.y);
    let trunk = length(vec2f(length(q.xz), max(abs(q.y - 3.5) - 3.5, 0.0))) - 0.35;
    let crown = length(q - vec3f(0.0, 7.5, 0.0)) - 2.6;
    return min(trunk, crown);
}

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    // nothing stands above the highest pool plus a palm
    let top = FOOT + 62.0 + 0.03 * p.x;
    if (p.y > top) { return vec2f(p.y - top, 1.0); }
    var d = p.y - ground_h(p.xz, ctx);
    var m = 1.0;
    let h0 = house_d(p, H0, ctx);
    if (h0 < d) { d = h0; m = 2.0; }
    let h1 = house_d(p, H1, ctx);
    if (h1 < d) { d = h1; m = 2.0; }
    let t0 = palm_d(p, P0, ctx);
    if (t0 < d) { d = t0; m = 3.0; }
    let t1 = palm_d(p, P1, ctx);
    if (t1 < d) { d = t1; m = 3.0; }
    let t2 = palm_d(p, P2, ctx);
    if (t2 < d) { d = t2; m = 3.0; }
    return vec2f(d, m);
}

// ------------------------------------------------------------ sky and shading

// the sky, or a flat overcast lid; disk = 1 draws the sun itself
fn skydome(rd: vec3f, l: Look, ctx: Ctx, disk: f32) -> vec3f {
    let d = normalize(vec3f(rd.x, max(rd.y, 0.012), rd.z));
    var c = sky_atmosphere(d, l.sun, ctx);
    c += sky_sun_disk(d, l.sun, 1.2) * disk * (1.0 - l.overcast);
    let grey = l.amb * (0.9 + 0.5 * d.y);
    return mix(c, grey, l.overcast);
}

// one flooded terrace: mirror water, seedling rim, earth riser, footpath
fn shade_paddy(q: vec3f, n: vec3f, rd: vec3f, dist: f32, wind: f32, l: Look, ctx: Ctx) -> vec3f {
    let t = ctx.t;
    let h = base_h(q.xz, ctx);
    let f = fract(h / STEP);
    let k = floor(h / STEP);
    // the smooth hill's gradient: which way is uphill, and how steep
    let e = 2.0;
    let g = vec2f(base_h(q.xz + vec2f(e, 0.0), ctx) - h, base_h(q.xz + vec2f(0.0, e), ctx) - h) / e;
    let slope = max(length(g), 1e-4);
    let uphill = g / slope;
    // position along the contour, for the path
    let c = dot(q.xz, vec2f(-UP.y, UP.x));

    let wall = max(smoothstep(0.85, 0.9, f), 1.0 - smoothstep(0.55, 0.8, n.y));
    let rim = (1.0 - smoothstep(0.015, 0.07, f)) * (1.0 - wall);
    let water = (1.0 - wall) * (1.0 - rim);

    // the footpath: along the lip of each pool between two crossings, then up the riser
    let c0 = -120.0 + 38.0 * sin(k * 2.1 + 0.7);
    let c1 = -120.0 + 38.0 * sin((k + 1.0) * 2.1 + 0.7);
    let along = step(min(c0, c1) - 1.0, c) * step(c, max(c0, c1) + 1.0);
    let cross = 1.0 - smoothstep(0.9, 1.6, abs(c - c1));
    let path = max(rim * along, wall * cross);

    let n_wall = normalize(vec3f(-uphill.x, 0.35, -uphill.y));
    let nn = normalize(mix(n, n_wall, wall));

    // the riser above each pool shades the pool's uphill edge when the sun
    // stands low on the uphill side: the whole pool at dawn, a sliver at noon
    let sun_flat = max(length(l.sun.xz), 1e-3);
    let sun_up = saturate(dot(l.sun.xz / sun_flat, uphill));
    let cover = sun_up * sun_flat / max(l.sun.y, 0.03) * slope;
    let band = sstep(cover + 0.05, cover - 0.08, 1.0 - f);
    let sh = 1.0 - 0.85 * band * (1.0 - l.overcast);

    let dif = saturate(dot(nn, l.sun));
    let lit = l.sun_c * dif * sh + l.amb * (0.55 + 0.45 * nn.y);

    // palette
    let earth = col_hex(0x4a3320u);
    let rice = mix(col_hex(0x86a83cu), col_hex(0x62c22cu), l.green);
    let mud = col_hex(0x1d2114u);
    let pathc = col_hex(0xa89a78u);
    var grow = 0.5;
    if (ctx.detail > 0u) { grow = noise_value2(vec2f(q.x * 0.008, k * 0.61 + 11.0)); }
    let under = mix(mud, rice * 0.45, grow * l.green * 0.8);

    // still water mirrors the sky; the wind roughens it with two slow trains
    let amp = 0.03 * wind * exp(-dist / 350.0);
    let k1 = vec2f(0.9, 0.45);
    let k2 = vec2f(-0.55, 1.1);
    let ph1 = dot(q.xz, k1) + t * 1.6;
    let ph2 = dot(q.xz, k2) * 0.8 + t * 1.2 + 2.0;
    let grad = amp * (cos(ph1) * k1 + 0.6 * cos(ph2) * k2);
    let nw = normalize(vec3f(-grad.x, 1.0, -grad.y));
    var refl = reflect(rd, nw);
    refl = normalize(vec3f(refl.x, max(refl.y, 0.02), refl.z));
    let fr = 0.12 + 0.88 * pow(1.0 - saturate(dot(-rd, nw)), 4.0);
    let skyr = skydome(refl, l, ctx, 0.0);
    let glint = pow(saturate(dot(refl, l.sun)), 200.0) * l.sun_c * 0.35 * (1.0 - l.overcast);
    let underlit = under * (l.sun_c * saturate(l.sun.y) * sh + l.amb);
    let wcol = mix(underlit, skyr + glint, fr);

    var col = mix(earth * lit, wcol, water);
    col = mix(col, rice * lit, rim);
    col = mix(col, pathc * lit, path);
    return col;
}

fn shade_house(q: vec3f, n: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let c = select(H0, H1, length(q.xz - H1) < length(q.xz - H0));
    let base = ground_h(c, ctx);
    let lq = vec3f(q.x - c.x, q.y - base, q.z - c.y);
    let roof = smoothstep(0.3, 0.6, n.y);
    let alb = mix(col_hex(0x3a2e26u), col_hex(0x2a221cu), roof);
    let dif = saturate(dot(n, l.sun));
    var col = alb * (l.sun_c * dif * 0.9 + l.amb * (0.6 + 0.4 * n.y));
    // one warm lit window in each end wall
    let win = (1.0 - smoothstep(0.7, 1.1, abs(lq.x)))
        * (1.0 - smoothstep(0.45, 0.7, abs(lq.y - 1.5)))
        * (1.0 - smoothstep(0.2, 0.5, n.y));
    col += col_hex(0xffb45au) * 5.0 * win * (1.2 - saturate(l.sun.y));
    return col;
}

fn shade_palm(n: vec3f, l: Look) -> vec3f {
    let dif = saturate(dot(n, l.sun));
    return col_hex(0x24421cu) * (l.sun_c * dif * 0.8 + l.amb * 0.8);
}

// ------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let k = ctx.params;
    let mist_k = l.mist * pow(k.x * 2.0, 1.5);
    let wind = k.y * 2.0;

    // the drone: 60 m up, pitched 33 degrees down, a slow drift of under a metre
    let ro = vec3f(0.0, FOOT + 60.0, 0.0) + cam_sway(t, 0.8, ctx.seed.x);
    let cam = cam_look_at(ro, ro + vec3f(0.0, -0.545, -0.839) * 100.0, 0.0, 70.0);
    let rd = cam_ray(cam, p);

    // haze is the sky at the horizon in this direction; the mist takes the
    // same tone softened by the ambient
    let hazec = skydome(normalize(vec3f(rd.x, 0.05, rd.z)), l, ctx, 0.0);
    let fogc = mix(hazec * 0.85 + l.amb * 0.4, l.amb * 1.3, l.overcast);

    var hit = vec2f(-1.0, -1.0);
    if (rd.y < 0.02) {
        hit = rm_march_k(ro, rd, 1.0, 1500.0, steps(96.0, ctx), 0.5, ctx);
    }

    var col: vec3f;
    var dist = 1500.0;
    var q = ro + rd * 500.0;   // where the mist drift is sampled for sky rays
    if (hit.x > 0.0) {
        dist = hit.x;
        q = ro + rd * dist;
        let n = rm_normal(q, dist, ctx);
        if (hit.y > 2.5) {
            col = shade_palm(n, l);
        } else if (hit.y > 1.5) {
            col = shade_house(q, n, l, ctx);
        } else {
            col = shade_paddy(q, n, rd, dist, wind, l, ctx);
        }
        // aerial perspective: the ridge stays visible, just softened
        col = fog_exp(col, hazec, dist, l.haze);
    } else {
        col = skydome(rd, l, ctx, 1.0);
        // a ray over the ridge that runs out: the next valley lost in haze
        col = mix(col, hazec, sstep(0.04, -0.01, rd.y));
    }

    // mist lying in the valley floor, drifting slowly, thinning fast with height
    var drift = 1.0;
    if (ctx.detail > 0u) {
        drift = 0.7 + 0.6 * noise_fbm2(q.xz * 0.003 + vec2f(t * 0.01, t * 0.006), 2);
    }
    col = fog_height(col, fogc, ro, rd, dist, mist_k * drift, 0.09);
    // the glow the haze picks up looking toward a low sun
    let glow = 0.04 * (1.0 - exp(-dist * 0.002)) * (1.0 - l.overcast);
    col += fog_sun(rd, l.sun, l.sun_c, glow);
    return max(col * l.expo, vec3f(0.0));
}
