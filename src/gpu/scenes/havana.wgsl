//! name: havana
//! title: Havana Malecón
//! category: city
//! tags: havana, cuba, seawall, waves, sunset, vintage cars
//! desc: waves bursting over Havana's seawall at sunset, 1950s cars under faded colonial arcades
//! themes: sunset, day, storm
//! uses: camera, raymarch, sky, clouds, water, light, noise
//! cost: light
//! tonemap: aces
//! fallback: city
//! credits: original

// World units are metres. The camera stands on the Malecón promenade by the
// seawall, looking west (-z) along the curving shore toward Vedado; the sea
// is on the right. The shore bends right with distance: every surface
// parallel to it lives at a constant u = x - K z^2, so seawall faces, the
// arcade and the facades are intersected exactly by solving a quadratic,
// no marching. The parked cars are ray-marched in their bounding boxes.
// Waves break against the wall in slow bursts of spray, each a closed-form
// event, lit from behind by the sun.

const K: f32 = 0.0009;        // shore curvature
const SEA_Y: f32 = -3.2;      // sea level below the promenade
const WALL_H: f32 = 0.85;     // seawall height above the promenade
const U_ROAD: f32 = -6.5;     // road edge (sea side)
const U_KERB: f32 = -25.0;    // road edge (city side)
const U_ARC: f32 = -29.0;     // arcade column line
const U_FAC: f32 = -32.5;     // facade wall inside the arcade

struct Look {
    sun: vec3f,
    sun_c: vec3f,
    amb: vec3f,
    sky_lo: vec3f,
    sky_hi: vec3f,
    cloud: f32,
    swell: f32,       // wave height
    spray: f32,       // how often and how big the bursts are
    wet: f32,
    haze: f32,
    hazec: vec3f,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    if (theme == 1u) {
        // day: high sun behind the camera, turquoise sea, trade-wind clouds
        l.sun = sky_sun_dir(52.0, 160.0);
        l.sun_c = sky_sun_light(l.sun);
        l.amb = sky_ambient(l.sun);
        l.sky_lo = vec3f(0.0);
        l.sky_hi = vec3f(0.0);
        l.cloud = 0.45;
        l.swell = 0.9;
        l.spray = 0.8;
        l.wet = 0.15;
        l.haze = 0.9;
        l.hazec = l.amb * 1.1;
        l.exposure = -0.7;
    } else if (theme == 2u) {
        // storm: slate sky, heavy swell, the sea going over the wall
        l.sun = sky_sun_dir(8.0, 25.0);
        l.sun_c = vec3f(0.12, 0.11, 0.1);
        l.amb = vec3f(0.12, 0.13, 0.15);
        l.sky_lo = vec3f(0.16, 0.17, 0.19);
        l.sky_hi = vec3f(0.06, 0.07, 0.085);
        l.cloud = 0.9;
        l.swell = 2.2;
        l.spray = 2.0;
        l.wet = 1.0;
        l.haze = 3.5;
        l.hazec = vec3f(0.14, 0.15, 0.17);
        l.exposure = -0.2;
    } else {
        // sunset: the sun going down into the Florida Straits, ahead-right
        l.sun = sky_sun_dir(2.8, 24.0);
        l.sun_c = vec3f(1.0, 0.45, 0.18) * 3.0;
        l.amb = vec3f(0.2, 0.13, 0.15);
        l.sky_lo = vec3f(1.05, 0.36, 0.12);
        l.sky_hi = vec3f(0.1, 0.1, 0.26);
        l.cloud = 0.5;
        l.swell = 1.1;
        l.spray = 1.0;
        l.wet = 0.35;
        l.haze = 1.4;
        l.hazec = vec3f(0.8, 0.45, 0.3);
        l.exposure = -0.35;
    }
    return l;
}

// ------------------------------------------------------------ the curved shore

fn u_of(p: vec3f) -> f32 { return p.x - K * p.z * p.z; }
fn x_of(u: f32, z: f32) -> f32 { return u + K * z * z; }
fn n_u(z: f32) -> vec3f { return normalize(vec3f(1.0, 0.0, -2.0 * K * z)); }

// first t > tmin where the ray crosses the bent surface u = c
fn hit_u(ro: vec3f, rd: vec3f, c: f32, tmin: f32) -> f32 {
    let a = -K * rd.z * rd.z;
    let b = rd.x - 2.0 * K * ro.z * rd.z;
    let cc = ro.x - K * ro.z * ro.z - c;
    if (abs(a) < 1e-9) {
        if (abs(b) < 1e-9) { return -1.0; }
        let t = -cc / b;
        return select(-1.0, t, t > tmin);
    }
    let disc = b * b - 4.0 * a * cc;
    if (disc < 0.0) { return -1.0; }
    let s = sqrt(disc);
    let t1 = (-b - s) / (2.0 * a);
    let t2 = (-b + s) / (2.0 * a);
    let lo = min(t1, t2);
    let hi = max(t1, t2);
    if (lo > tmin) { return lo; }
    if (hi > tmin) { return hi; }
    return -1.0;
}

// ------------------------------------------------------------ sky and sea

fn hav_sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let y = max(rd.y, 0.0);
    var c: vec3f;
    if (l.sky_lo.x + l.sky_hi.x <= 0.0) {
        c = sky_atmosphere_haze(rd, l.sun, 1.3) * 1.2;
        // sea haze on the horizon: pale, not the model's brown limb
        c = mix(c, vec3f(0.5, 0.58, 0.68) * 0.9, exp(-y * 14.0) * 0.85);
    } else {
        c = mix(l.sky_lo, mix(l.sky_lo * 0.5 + l.sky_hi * 0.6, l.sky_hi, saturate(y * 3.0)), pow(saturate(y * 5.0), 0.6));
        let mu = saturate(dot(rd, l.sun));
        c += l.sun_c * (0.1 * pow(mu, 4.0) + 0.5 * pow(mu, 60.0));
    }
    c += sky_sun_disk(rd, l.sun, 0.6) * select(1.0, 0.6, l.sky_lo.x > 1.0) * step(0.5, l.sun_c.x + l.sun_c.y);
    // clouds: trade-wind cumulus by day, a lit deck at sunset, slate in storm
    if (rd.y > 0.0) {
        let uv = rd.xz / (rd.y + 0.08) * 0.6;
        let cs = cloud_sheet(uv + vec2f(ctx.t * 0.004, 0.0), l.cloud, ctx);
        let mu = saturate(dot(rd, l.sun));
        var lit = l.amb * 1.4 + l.sun_c * (0.25 + 0.8 * pow(mu, 3.0)) * cs.y;
        if (l.cloud > 0.8) { lit = l.sky_lo * (0.55 + 0.3 * cs.y); }
        c = mix(c, lit, cs.x * smoothstep(0.0, 0.08, rd.y) * 0.85);
    }
    return c;
}

// Vedado far along the shore: tower blocks in the haze
fn skyline(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx, zoom: f32) -> vec4f {
    if (rd.z >= 0.0) { return vec4f(0.0); }
    let t = (-1700.0 - ro.z) / rd.z;
    let p = ro + rd * t;
    let lod = ctx.px / zoom * t;
    let xs = p.x - x_of(U_ARC - 40.0, -1700.0);
    // a jumble of blocks, the FOCSA slab and the twin-towered hotel
    let cw = 40.0;
    let c = floor(xs / cw);
    let h = hash_cell2(vec2i(i32(c), 3), 0x4a7u);
    var hgt = 15.0 + 40.0 * h.x * h.x;
    if (xs > 200.0 || xs < -900.0) { hgt = 0.0; }
    var d = p.y - hgt - SEA_Y;
    let focsa = max(abs(xs + 120.0) - 22.0, p.y - SEA_Y - 121.0);
    let hotel = max(abs(abs(xs + 420.0) - 14.0) - 8.0, p.y - SEA_Y - 62.0);
    let hotel_b = max(abs(xs + 420.0) - 40.0, p.y - SEA_Y - 40.0);
    d = min(d, min(focsa, min(hotel, hotel_b)));
    let a = saturate(0.5 - d / max(lod, 0.1));
    let c0 = mix(l.amb * 0.25 + l.sun_c * 0.02, l.hazec, 0.55);
    return vec4f(c0 * a, a);
}

fn sea(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = water_normal(p.xz * 0.6, ctx.t * 0.8, l.swell * 0.7, t * 0.6, ctx);
    var r = reflect(rd, n);
    r.y = abs(r.y) + 0.002;
    var refl = hav_sky(r, l, ctx);
    // the sun's glitter path across the swell
    let hv = normalize(l.sun - rd);
    let g = pow(saturate(dot(n, hv)), 600.0) * 40.0 + pow(saturate(dot(n, hv)), 60.0) * 0.8;
    refl += l.sun_c * g * step(0.0, l.sun.y) * step(l.cloud, 0.8);
    let fres = water_fresnel(dot(-rd, n));
    // turquoise over the reef by day, deep blue beyond; grey in storm
    var deep = vec3f(0.01, 0.05, 0.08);
    var shal = vec3f(0.03, 0.18, 0.2);
    if (l.cloud > 0.8) { deep = vec3f(0.03, 0.045, 0.05); shal = vec3f(0.05, 0.08, 0.08); }
    let dist = u_of(p);
    let body = mix(shal, deep, smoothstep(5.0, 120.0, dist)) * (l.amb * 1.5 + l.sun_c * 0.1);
    var c = mix(body, refl, fres);
    // foam where the swell breaks against the wall's foot
    let foam = smoothstep(10.0, 0.0, dist) * (0.5 + 0.5 * sin(dist * 0.8 - ctx.t * 1.3 + noise_value2(p.xz * 0.2) * 5.0));
    let fn2 = noise_fbm2(p.xz * vec2f(0.2, 0.08) + vec2f(0.0, ctx.t * 0.1), 4);
    let fo = saturate(foam * smoothstep(0.45, 0.7, fn2) * l.swell);
    c = mix(c, (l.amb * 1.2 + l.sun_c * 0.5) * 0.8, fo * 0.8);
    return c;
}

// ------------------------------------------------------------ facades

struct Bld { z0: f32, len: f32, floors: f32, col: vec3f, id: u32, ruin: f32 }

fn building(z: f32) -> Bld {
    let c = floor(-z / 14.0);
    let h = hash_cell2(vec2i(i32(c), 0), 0x6ab1u);
    var b: Bld;
    b.z0 = -c * 14.0;
    b.len = 14.0;
    b.floors = 2.0 + floor(h.x * 2.99);
    // faded tropical pastels
    var col = col_hex(0x8fc7c0u);                     // sea green
    if (h.y > 0.18) { col = col_hex(0xe6a9a0u); }     // salmon pink
    if (h.y > 0.34) { col = col_hex(0xe8d08au); }     // yellow ochre
    if (h.y > 0.5) { col = col_hex(0x9bb8d8u); }      // faded blue
    if (h.y > 0.66) { col = col_hex(0xdcd2c0u); }     // bleached cream
    if (h.y > 0.82) { col = col_hex(0xc9867au); }     // terracotta
    b.col = col * (0.8 + 0.2 * h.z);
    b.id = hash_u(u32(i32(c) + 5000));
    b.ruin = h.w;
    return b;
}

fn shade_facade(p: vec3f, n: vec3f, rd: vec3f, inner: bool, l: Look, ctx: Ctx) -> vec3f {
    let b = building(p.z);
    let zl = b.z0 - p.z;
    let y = p.y;
    let top = 4.6 + (b.floors - 1.0) * 3.8;
    var alb = b.col;
    // salt-worn paint: blotches of bare plaster and grime from the rain
    let n1 = noise_fbm2(vec2f(zl * 1.1, y * 0.9) + f32(b.id & 255u), 4);
    alb = mix(alb, vec3f(0.66, 0.62, 0.56), smoothstep(0.62, 0.72, n1) * 0.7);
    alb *= 0.82 + 0.25 * noise_fbm2(vec2f(zl * 2.0, y * 0.2), 3);
    alb *= 1.0 - 0.3 * step(zl, 0.1);
    var glass = 0.0;
    var dark = 0.0;
    if (inner) {
        // back wall of the arcade: doorways into dim shops and stairwells
        let dz = fract(zl / 3.5);
        let door = step(0.25, dz) * step(dz, 0.75) * step(y, 3.2);
        dark = door * 0.9;
    } else if (y > 4.6 && y < top) {
        // upper floors: tall louvred shutters behind iron balconies
        let fy = fract((y - 4.6) / 3.8);
        let bz = fract(zl / 3.5);
        let win = step(0.28, bz) * step(bz, 0.72) * step(0.12, fy) * step(fy, 0.82);
        if (win > 0.5) {
            let wh = hash_cell2(vec2i(i32(floor(zl / 3.5)) + i32(b.id & 1023u), i32(floor((y - 4.6) / 3.8))), 0x2b1u);
            if (wh.x < 0.55) {
                // louvres, some hanging open
                alb = mix(alb * 0.7, vec3f(0.3, 0.45, 0.4) * (0.8 + 0.4 * wh.y), 0.6) * (0.8 + 0.2 * step(0.4, fract(fy * 22.0)));
            } else {
                dark = 0.85;
            }
        }
        let bal = step(0.0, fy) * step(fy, 0.3) * step(0.22, bz) * step(bz, 0.78) * step(0.3, fract((y - 4.6) / 3.8 * 1.0 + 0.9));
        let rail = step(0.8, fract(zl * 7.0)) * step(0.02, fy) * step(fy, 0.26) + step(abs(fy - 0.26), 0.012);
        alb = mix(alb, vec3f(0.04), saturate(rail) * bal);
        // a cornice line under each floor
        alb *= 1.0 - 0.25 * step(abs(fy - 0.97), 0.03);
    }
    // parapet with a moulded cornice at the top
    let cornice = step(top - 0.5, y) * step(y, top);
    alb *= 1.0 + 0.2 * cornice;
    alb = mix(alb, alb * 0.3, dark);
    // light: low sun from ahead-right, the sky, warm bounce off the road
    let dif = saturate(dot(n, l.sun));
    let shade = select(1.0, 0.25, inner);
    var e = l.sun_c * dif * shade + l.amb * (0.55 + 0.2 * n.y) * select(1.0, 0.6, inner) + l.sun_c * 0.05;
    var c = alb * 0.318 * e;
    // a couple of lit windows as the light goes
    if (dark > 0.5 && l.sky_lo.x > 1.0) {
        let lh = hash_f(b.id + u32(zl) * 31u + u32(y) * 7u);
        c += vec3f(1.0, 0.7, 0.4) * 0.12 * step(0.75, lh);
    }
    return c;
}

// ------------------------------------------------------------ the cars

struct Car { pos: vec3f, col: vec3f, len: f32 }

fn car(i: i32) -> Car {
    // parked along both kerbs
    var z = -14.0 - f32(i) * 11.0;
    var u = U_KERB + 1.3;
    if (i == 1) { u = U_ROAD - 1.2; z = -21.0; }
    let h = hash_cell2(vec2i(i, 9), 0xca5u);
    var col = col_hex(0xe7a2b5u);                   // flamingo pink
    if (h.x > 0.25) { col = col_hex(0x3fb3b0u); }  // turquoise
    if (h.x > 0.5) { col = col_hex(0xb3202fu); }   // cherry red
    if (h.x > 0.75) { col = col_hex(0xe9e1c9u); }  // cream
    if (i == 0) { col = col_hex(0x3fb3b0u); }
    if (i == 1) { col = col_hex(0xe7a2b5u); }
    return Car(vec3f(x_of(u, z), 0.0, z), col, 5.3);
}

// the cruising car: along the far lane, every 50 s, westbound
fn cruiser(t: f32) -> vec4f {
    let ev = hash_event(t, 50.0, 0x55u);
    let s = ev.y;
    let z = mix(8.0, -140.0, s);
    let u = -15.0;
    return vec4f(x_of(u, z), 0.0, z, select(0.0, 1.0, s < 0.95));
}

fn car_sdf(q: vec3f) -> vec2f {
    // q: car frame, x across (width), z along (length, +z = rear)
    let lz = q.z;
    // body: long, low, slab-sided
    var body = sdf_round_box(q - vec3f(0.0, 0.62, 0.0), vec3f(0.95, 0.32, 2.62), 0.2);
    // tail fins rising at the back
    let fin = sdf_round_box(vec3f(abs(q.x) - 0.82, q.y - 0.95 - 0.12 * saturate((lz - 1.2) / 1.3), lz - 1.9), vec3f(0.06, 0.12, 0.7), 0.05);
    body = min(body, fin);
    // greenhouse: cabin with raked screens
    var cab = sdf_round_box(q - vec3f(0.0, 1.18, 0.2), vec3f(0.82, 0.28, 1.05), 0.14);
    cab = max(cab, (q.y - 1.18) * 0.9 + (lz - 0.2 + 0.3) * 0.45 - 0.6);
    cab = max(cab, (q.y - 1.18) * 0.9 - (lz - 0.2 - 0.3) * 0.45 - 0.55);
    var r = vec2f(body, 1.0);
    r = op_umin(r, vec2f(cab, 2.0));
    // chrome bumpers
    let bump = sdf_round_box(vec3f(q.x, q.y - 0.42, abs(lz) - 2.68), vec3f(0.98, 0.07, 0.06), 0.04);
    r = op_umin(r, vec2f(bump, 3.0));
    // wheels in their arches
    let wq = vec3f(abs(q.x) - 0.82, q.y - 0.36, abs(lz) - 1.55);
    let wheel = max(length(wq.yz) - 0.36, abs(wq.x) - 0.12);
    r = op_umin(r, vec2f(wheel, 4.0));
    return r;
}

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    var r = vec2f(1e5, 0.0);
    for (var i = 0; i < 3; i++) {
        let c = car(i);
        let q = p - c.pos;
        if (length(q) > 6.0) { continue; }
        let cr = car_sdf(vec3f(q.x, q.y, q.z));
        r = op_umin(r, vec2f(cr.x, cr.y + f32(i) * 10.0));
    }
    let cz = cruiser(ctx.t);
    if (cz.w > 0.5) {
        let q = p - cz.xyz;
        if (length(q) < 6.0) {
            let cr = car_sdf(vec3f(q.x, q.y, q.z));
            r = op_umin(r, vec2f(cr.x, cr.y + 30.0));
        }
    }
    return r;
}

fn car_bounds(ro: vec3f, rd: vec3f, c: vec3f) -> vec2f {
    let hb = vec3f(1.2, 1.1, 3.0);
    let cc = c + vec3f(0.0, 0.75, 0.0);
    let inv = 1.0 / select(rd, vec3f(1e-6), abs(rd) < vec3f(1e-6));
    let t0 = (cc - hb - ro) * inv;
    let t1 = (cc + hb - ro) * inv;
    let a = min(t0, t1);
    let b = max(t0, t1);
    return vec2f(max(max(a.x, a.y), max(a.z, 0.0)), min(min(b.x, b.y), b.z));
}

fn shade_car(p: vec3f, n: vec3f, rd: vec3f, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    let idx = i32(floor(mat / 10.0));
    let m = mat - f32(idx) * 10.0;
    var paint = vec3f(0.5);
    if (idx < 3) { paint = car(idx).col; } else { paint = col_hex(0xe9d27eu); }
    let r = reflect(rd, n);
    let fres = 0.04 + 0.96 * pow(1.0 - saturate(-dot(rd, n)), 5.0);
    let env = hav_sky(normalize(vec3f(r.x, abs(r.y), r.z)), l, ctx);
    let dif = saturate(dot(n, l.sun));
    let e = l.sun_c * dif + l.amb * (0.5 + 0.5 * n.y);
    let spec = l.sun_c * pow(saturate(dot(r, l.sun)), 120.0) * 6.0;
    if (m < 1.5) {
        // glossy lacquer, a little faded
        return paint * 0.318 * e + (env * 0.8 + spec) * fres * 0.9;
    }
    if (m < 2.5) {
        // glass: dark interior, sky in the screens; a painted roof above
        if (n.y > 0.75) { return paint * 0.318 * e + env * fres * 0.8 + spec * fres; }
        return vec3f(0.01) + env * (0.15 + 0.6 * fres) + spec * 0.5;
    }
    if (m < 3.5) {
        // chrome
        return env * 0.75 + spec * 1.5 + l.amb * 0.05;
    }
    return vec3f(0.015) * e;
}

// ------------------------------------------------------------ spray

// a wave bursting against the wall: a sheet of white water thrown up from
// the sea side, billowing over the promenade and falling back as mist, in
// slow motion. Returns (rgb premultiplied, alpha) for bursts before tmax.
fn spray(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx) -> vec4f {
    var acc = vec4f(0.0);
    for (var k = 0; k < 4; k++) {
        let period = mix(9.0, 5.5, saturate(l.spray - 1.0));
        let ev = hash_event(ctx.t + f32(k) * 2.9, period, 0x5b7u + u32(k) * 13u);
        let dur = 0.85;
        let ph = ev.y / dur;
        if (ph > 1.0 || ev.x > 0.35 + 0.3 * l.spray) { continue; }
        let h2 = hash_f(u32(i32(ev.z) * 7 + k) ^ 0xabcu);
        let z = -12.0 - 75.0 * h2;
        let t = (z - ro.z) / min(rd.z, -1e-4);
        if (t <= 0.0 || t > tmax) { continue; }
        let p = ro + rd * t;
        let u = u_of(p);
        let y = p.y - WALL_H;
        // rises fast, hangs, falls slowly; leans landward with the wind
        let rise = 1.0 - pow(1.0 - saturate(ph * 1.8), 3.0);
        let height = (3.5 + 4.5 * h2) * l.spray * rise * (1.0 - 0.4 * smoothstep(0.5, 1.0, ph));
        let lean = y * (0.35 + 0.4 * ph);
        let width = (1.2 + 0.25 * y + 3.0 * ph) * (0.7 + 0.5 * saturate(l.spray - 1.0) + 0.3 * h2);
        let uc = 0.8 - lean;
        let shape = exp(-sq((u - uc) / width)) * smoothstep(-1.5, 0.3, y) * smoothstep(height, height * 0.3, y);
        // a dense sheet in the core, lumpy billows, fine mist around
        let n = noise_fbm2(vec2f((u - uc) * 0.7 + h2 * 9.0, y * 0.8 - ph * 2.5), 5);
        let fine = noise_fbm2(vec2f(u * 2.2, y * 2.4 + ph * 3.0) + n * 2.0, 3);
        let core = saturate((n * 1.8 + fine * 0.5 - 0.85) * 2.0);
        let mist = saturate(n * 1.2 - 0.35) * 0.35;
        let fade = 1.0 - smoothstep(0.6, 1.0, ph);
        let dens = saturate(shape * (core + mist)) * fade;
        // white water: bright in the sun, glowing when the sun is behind it
        let mu = saturate(dot(rd, l.sun));
        let c = (l.amb * 1.8 + l.sun_c * (0.45 + 2.2 * pow(mu, 5.0))) * 0.85;
        let a = dens * (1.0 - acc.w);
        acc = vec4f(acc.xyz + c * a, acc.w + a);
    }
    return acc;
}

// people sitting on the seawall watching the sun go down, and a fisherman
// standing with his rod: silhouettes in planes across the view
fn people(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx, zoom: f32) -> vec4f {
    // nobody sits on the wall while the sea is coming over it
    if (l.spray > 1.5) { return vec4f(0.0); }
    // figures are ordered near to far, so the first one hit is the nearest
    for (var k = 0; k < 6; k++) {
        let h = hash_cell2(vec2i(k, 1), 0x9e0u);
        let z = -5.5 - f32(k) * 7.0 - 3.0 * h.x;
        let t = (z - ro.z) / min(rd.z, -1e-4);
        if (t <= 0.0 || t > tmax + 0.5) { continue; }
        let p = ro + rd * t;
        let u = u_of(p);
        let y = p.y;
        let lod = ctx.px / zoom * t;
        var d: f32;
        if (k == 3) {
            // the fisherman on the promenade, casting over the wall
            let uc = -1.0;
            let body = sdf2_segment(vec2f(u, y), vec2f(uc, 0.15), vec2f(uc + 0.05, 1.35)) - 0.17;
            let head = length(vec2f(u - uc - 0.08, y - 1.6)) - 0.12;
            let arm = sdf2_segment(vec2f(u, y), vec2f(uc + 0.05, 1.25), vec2f(uc + 0.5, 1.1)) - 0.05;
            // the rod: a thin arc out over the sea
            let s = saturate((u - uc - 0.5) / 4.0);
            let ry = 1.1 + s * 2.4 - s * s * 1.2;
            let rod = max(abs(y - ry) - 0.015, max(uc + 0.5 - u, u - uc - 4.5));
            d = min(min(body, head), min(arm, rod));
        } else {
            // sitting on the wall facing the sea, legs over the edge
            let uc = -0.28;
            let y0 = WALL_H;
            let lean = (h.y - 0.35) * 0.18;
            let torso = sdf2_segment(vec2f(u, y), vec2f(uc - 0.02, y0 + 0.12), vec2f(uc - 0.04 + lean, y0 + 0.6)) - 0.15;
            let head = length(vec2f(u - uc + 0.02 - lean * 1.3, y - y0 - 0.8)) - 0.1;
            let thigh = sdf2_segment(vec2f(u, y), vec2f(uc, y0 + 0.1), vec2f(uc + 0.42, y0 + 0.12)) - 0.08;
            let shin = sdf2_segment(vec2f(u, y), vec2f(uc + 0.42, y0 + 0.1), vec2f(uc + 0.47 + 0.1 * h.z, y0 - 0.36)) - 0.065;
            d = min(min(torso, head), min(thigh, shin));
        }
        let a = saturate(0.5 - d / max(lod, 0.004));
        if (a > 0.01) {
            // backlit: dark shapes with a thin warm rim toward the sun
            let mu = saturate(dot(rd, l.sun));
            let rim = saturate(1.0 + d / max(lod * 1.5, 0.01)) * pow(mu, 3.0);
            let c = vec3f(0.02, 0.015, 0.014) * (l.amb * 3.0 + 0.2) + l.sun_c * 0.25 * rim + l.amb * 0.05;
            return vec4f(c * a, a);
        }
    }
    return vec4f(0.0);
}

// streetlights along the promenade: slim posts with a lamp on a curved arm,
// glowing sodium at dusk and in the storm. Returns (rgb premultiplied, alpha).
fn streetlights(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx, zoom: f32) -> vec4f {
    var res = vec4f(0.0);
    let on = select(0.0, 1.0, l.sky_lo.x > 0.0);
    for (var k = 0; k < 7; k++) {
        let z = -13.0 - f32(k) * 24.0;
        let pos = vec3f(x_of(U_ROAD + 0.4, z), 0.0, z);
        let o = ro.xz - pos.xz;
        let dxz = rd.xz;
        let tt = -dot(o, dxz) / max(dot(dxz, dxz), 1e-6);
        if (tt <= 0.0 || tt > tmax) { continue; }
        let y = ro.y + rd.y * tt;
        let d = length(o + dxz * tt);
        let lod = ctx.px / zoom * tt;
        var a = 0.0;
        if (y > 0.0 && y < 6.2) {
            a = saturate((0.06 + 0.04 * smoothstep(1.0, 0.0, y) - d) / max(lod, 1e-3) + 0.5);
        }
        // lamp head out over the promenade, and its glow
        let head = pos + vec3f(1.1, 6.15, 0.0);
        let w = head - ro;
        let tc = dot(w, rd);
        let dh2 = max(dot(w, w) - tc * tc, 0.0);
        // the arm curves out over the road
        let ax = -(o.x + dxz.x * tt);
        let arm = step(abs(y - 6.05 - 0.25 * sqrt(saturate(1.0 - sq((d - 0.55) / 0.6)))), 0.035 + lod * 0.5) * step(d, 1.15) * step(0.0, -ax);
        a = max(a, arm * 0.9);
        let glow = on * select(0.0, 1.0, tc > 0.0 && tc < tmax + 1.0) * vec3f(1.0, 0.62, 0.3) * (2.5 * exp(-dh2 / 0.02) + 0.04 / (1.0 + dh2 * 2.0)) * select(0.6, 1.0, l.cloud > 0.8);
        let c = vec3f(0.02) * (l.amb + 0.1) + l.sun_c * 0.01;
        res = vec4f(res.xyz * (1.0 - a) + c * a + glow, max(res.w, a));
    }
    return res;
}

// ------------------------------------------------------------ the scene

fn shade_ground(p: vec3f, rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let u = u_of(p);
    var alb: vec3f;
    var gloss = 0.0;
    if (u > U_ROAD) {
        // the promenade: worn concrete, wet near the wall
        alb = vec3f(0.42, 0.4, 0.37) * (0.8 + 0.3 * noise_value2(p.xz * 0.8));
        alb *= 1.0 - 0.15 * step(0.96, fract(p.z / 3.0));
        gloss = max(l.wet, smoothstep(-3.0, -0.6, u) * 0.7 * l.spray);
    } else if (u > U_KERB) {
        // six lanes of patched asphalt, faded lines
        alb = vec3f(0.12, 0.115, 0.11) * (0.8 + 0.4 * noise_fbm2(p.xz * 0.3, 3));
        let lane = fract((u - U_KERB) / 6.2);
        let dash = step(abs(lane - 0.5), 0.012) * step(0.5, fract(p.z / 6.0));
        alb = mix(alb, vec3f(0.55, 0.53, 0.48), dash * 0.6);
        gloss = l.wet * 0.9;
    } else {
        alb = vec3f(0.38, 0.35, 0.32) * (0.85 + 0.2 * noise_value2(p.xz));
        gloss = l.wet * 0.5;
    }
    let e = l.sun_c * saturate(l.sun.y + 0.05) + l.amb;
    var c = alb * mix(1.0, 0.5, gloss) * 0.318 * e;
    // wet sheen: the sky and the low sun smeared along the road
    let fres = 0.02 + 0.98 * pow(1.0 - saturate(-rd.y), 5.0);
    let r = vec3f(rd.x, -rd.y, rd.z);
    let refl = hav_sky(r, l, ctx) * 0.6 + l.sun_c * pow(saturate(dot(r, l.sun)), 30.0) * 2.0;
    c += refl * fres * gloss;
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(-3.0, 1.65, 0.0);
    let cam = cam_look_at(ro, vec3f(10.0, 3.5, -100.0), 0.0, 55.0);
    let rd = cam_ray(cam, p);
    let zoom = cam.zoom;

    // candidate surfaces
    var t = 1e9;
    var kind = 0;
    var nn = vec3f(0.0, 1.0, 0.0);
    // ground (promenade, road) and the sea below the wall
    if (rd.y < 0.0) {
        let tg = -ro.y / rd.y;
        let pg = ro + rd * tg;
        if (u_of(pg) < -0.6) { t = tg; kind = 1; }
        let ts = (SEA_Y - ro.y) / rd.y;
        let ps = ro + rd * ts;
        if (u_of(ps) > 0.0 && ts < t) { t = ts; kind = 2; }
    }
    // the seawall: land face, top, sea face
    let tl = hit_u(ro, rd, -0.6, 0.0);
    if (tl > 0.0 && tl < t) {
        let pl = ro + rd * tl;
        if (pl.y < WALL_H && pl.y > 0.0) { t = tl; kind = 3; nn = -n_u(pl.z); }
    }
    if (rd.y < 0.0) {
        let tt = (WALL_H - ro.y) / rd.y;
        let pt = ro + rd * tt;
        let ut = u_of(pt);
        if (ut > -0.6 && ut < 0.0 && tt < t) { t = tt; kind = 4; nn = vec3f(0.0, 1.0, 0.0); }
    }
    let tsf = hit_u(ro, rd, 0.0, 0.0);
    if (tsf > 0.0 && tsf < t) {
        let ps = ro + rd * tsf;
        if (ps.y < WALL_H && ps.y > SEA_Y) { t = tsf; kind = 5; nn = n_u(ps.z); }
    }
    // the arcade: columns at U_ARC with the upper floors resting on them,
    // the shop wall behind
    let ta = hit_u(ro, rd, U_ARC, 0.0);
    if (ta > 0.0 && ta < t) {
        let pa = ro + rd * ta;
        let b = building(pa.z);
        let top = 4.6 + (b.floors - 1.0) * 3.8;
        if (pa.y < top) {
            let zl = b.z0 - pa.z;
            let col = abs(fract(zl / 3.5) - 0.5) * 3.5;
            let arch = 3.6 + 0.6 * sqrt(saturate(1.0 - sq(col / 1.45)));
            if (pa.y > 4.1 || col < 0.28 || pa.y > arch) {
                t = ta; kind = 6; nn = n_u(pa.z);
            } else {
                let tf = hit_u(ro, rd, U_FAC, ta);
                if (tf > 0.0 && tf < t) {
                    t = tf; kind = 7; nn = n_u((ro + rd * tf).z);
                }
            }
        }
    }
    // cars
    var ch = vec2f(-1.0);
    for (var i = 0; i < 4; i++) {
        var cpos: vec3f;
        if (i < 3) { cpos = car(i).pos; } else {
            let cz = cruiser(ctx.t);
            if (cz.w < 0.5) { continue; }
            cpos = cz.xyz;
        }
        let bb = car_bounds(ro, rd, cpos);
        if (bb.y > bb.x && bb.x < t) {
            let hh = rm_march(ro, rd, bb.x, min(bb.y, t), steps(40.0, ctx), ctx);
            if (hh.x > 0.0 && hh.x < t) { t = hh.x; ch = hh; kind = 8; }
        }
    }

    var col: vec3f;
    let hp = ro + rd * t;
    if (kind == 1) {
        col = shade_ground(hp, rd, l, ctx);
    } else if (kind == 2) {
        col = sea(hp, rd, t, l, ctx);
    } else if (kind >= 3 && kind <= 5) {
        // painted concrete of the seawall, stained dark by the sea
        var alb = vec3f(0.55, 0.53, 0.5) * (0.8 + 0.3 * noise_value2(vec2f(hp.z * 0.6, hp.y * 2.0)));
        if (kind == 5) { alb *= mix(0.35, 1.0, smoothstep(-2.5, 0.5, hp.y)); }
        var e = l.sun_c * saturate(dot(nn, l.sun)) + l.amb * (0.6 + 0.3 * nn.y);
        // the sunlit promenade bounces light back onto the wall
        e += (l.sun_c * saturate(l.sun.y + 0.1) * 0.35 + l.amb * 0.4) * select(0.0, 1.0, kind == 3);
        col = alb * 0.318 * e;
        col = mix(col, col * 0.6 + l.amb * 0.02, l.wet * 0.5);
    } else if (kind == 6 || kind == 7) {
        col = shade_facade(hp, nn, rd, kind == 7, l, ctx);
    } else if (kind == 8) {
        let n = rm_normal(hp, t, ctx);
        col = shade_car(hp, n, rd, ch.y, l, ctx);
    } else {
        col = hav_sky(rd, l, ctx);
        let sk = skyline(ro, rd, l, ctx, zoom);
        col = col * (1.0 - sk.w) + sk.xyz;
        t = 5000.0;
    }
    // haze along the shore, glowing toward the sun
    if (t < 4000.0) {
        let fk = 1.0 - exp(-t * 0.001 * l.haze);
        let mu = saturate(dot(rd, l.sun));
        col = mix(col, l.hazec * (1.0 + 1.5 * pow(mu, 8.0)), fk);
    }
    // streetlights, people on the wall, then spray over it
    let sl = streetlights(ro, rd, t, l, ctx, zoom);
    col = col * (1.0 - sl.w) + sl.xyz;
    let pp = people(ro, rd, t, l, ctx, zoom);
    col = col * (1.0 - pp.w) + pp.xyz;
    let sp = spray(ro, rd, t, l, ctx);
    col = col * (1.0 - sp.w) + sp.xyz;
    return col * exp2(l.exposure);
}
