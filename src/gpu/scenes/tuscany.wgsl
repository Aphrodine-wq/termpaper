//! name: tuscany
//! title: Tuscan Hills
//! category: wilds
//! tags: hills, cypress, farmhouse, fog, italy
//! desc: rolling Val d'Orcia hills, a cypress-lined road up to a farmhouse, fog in the valleys
//! themes: dawn, summer, autumn
//! uses: camera, sdf, sky, fog
//! cost: heavy
//! fallback: meadow
//! credits: original

// World units are metres, y up. The camera stands on a rise above the Val
// d'Orcia looking across folds of farmland: a white road climbs in bends to
// a stone farmhouse on the next crest between two rows of cypresses, and
// further hills fade into the haze. Fields are parcels of wheat, pasture
// and ploughed clay; at dawn fog lies in the hollows between the hills.

struct Look {
    sun: vec3f,
    haze: f32,
    fog: f32,         // valley fog density
    season: f32,      // 0 green spring, 1 golden summer, 2 ploughed autumn
    exposure: f32,
    sun_c: vec3f,
    amb: vec3f,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: { l = Look(sky_sun_dir(24.0, -100.0), 2.0, 0.0, 1.0, -0.3, vec3f(0.0), vec3f(0.0)); }
        case 2u: { l = Look(sky_sun_dir(9.0, 100.0), 1.6, 0.12, 2.0, 0.0, vec3f(0.0), vec3f(0.0)); }
        default: { l = Look(sky_sun_dir(4.5, -75.0), 1.4, 0.55, 0.0, 0.25, vec3f(0.0), vec3f(0.0)); }
    }
    l.sun_c = sky_sun_light(l.sun);
    l.amb = sky_ambient(l.sun);
    if (l.sun.y < 0.12) { l.sun_c *= 1.5; l.amb *= 1.3; }
    return l;
}

// ------------------------------------------------------------ geometry

fn snoise(p: vec2f) -> f32 { return noise_value2(p) * 2.0 - 1.0; }

const FARM: vec2f = vec2f(200.0, -1450.0);

fn terrain_h(xz: vec2f, oct: i32) -> f32 {
    // broad rolling folds, and smaller swells on them
    var h = 110.0 * noise_value2(xz * 0.0015 + vec2f(3.0, 1.0));
    h += 55.0 * noise_value2(xz * 0.0038 + 7.0);
    if (oct > 0) { h += 14.0 * noise_value2(xz * 0.011 + 2.0); } else { h += 7.0; }
    // our hill (the town walls of Pienza, say), the valley below it, the
    // farm's crest beyond
    h += 150.0 * exp(-dot(xz, xz) / (520.0 * 520.0));
    h -= 60.0 * exp(-sq((xz.y + 850.0) / 320.0));
    let fq = xz - FARM;
    h += 55.0 * exp(-dot(fq, fq) / (420.0 * 420.0));
    if (oct > 1) { h += 1.2 * snoise(xz * 0.03); }
    return h;
}

// the white road climbing to the farm in bends: its x at depth z
fn road_x(z: f32) -> f32 {
    let u = saturate((-z - 900.0) / (-FARM.y - 900.0));
    return mix(40.0, FARM.x - 12.0, u) + 150.0 * sin(u * 7.0 + 0.4) * (1.0 - u * 0.7);
}
fn on_road(z: f32) -> bool { return z < -900.0 && z > FARM.y + 20.0; }

// a cypress: a tall narrow flame, 15 m high
fn cypress(q: vec3f, tall: f32) -> f32 {
    let rr = tall * 0.1;
    let body = sdf_ellipsoid(q - vec3f(0.0, tall * 0.42, 0.0), vec3f(rr, tall * 0.5, rr));
    // a cone trims the crown to a point
    let cone = (length(q.xz) - rr * 1.9 * (1.0 - q.y / tall)) * 0.95;
    return max(body, cone);
}

// cypresses both sides of the road, one every 13 m of depth
fn road_trees(p: vec3f) -> f32 {
    let sp = 21.0;
    let c0 = round(p.z / sp);
    var d = 1e5;
    for (var i = -1; i <= 1; i++) {
        let zc = (c0 + f32(i)) * sp;
        if (!on_road(zc)) { continue; }
        let rx = road_x(zc);
        // the road runs at a slant: set the trees off perpendicular to it
        let slope = road_x(zc - 1.0) - rx;
        let side_off = 5.5 * sqrt(1.0 + slope * slope);
        let g = terrain_h(vec2f(rx, zc), 1);
        for (var s = 0; s < 2; s++) {
            let sx = rx + select(-side_off, side_off, s == 1);
            let h = hash_cell2(vec2i(i32(c0) + i, s), 0xc1au);
            let tall = 13.0 + 5.0 * h.x;
            d = min(d, cypress(p - vec3f(sx, g - 0.5, zc), tall));
        }
    }
    return d;
}

// the farmhouse: two stone blocks, a squat tower, low hipped roofs
fn farm(p: vec3f, gy: f32) -> vec2f {
    let q0 = (p - vec3f(FARM.x, gy, FARM.y)) / 1.3;
    let q = vec3f(q0.x * 0.87 + q0.z * 0.5, q0.y, -q0.x * 0.5 + q0.z * 0.87);
    var walls = sdf_box(q - vec3f(0.0, 4.0, 0.0), vec3f(11.0, 5.5, 6.0));
    walls = min(walls, sdf_box(q - vec3f(-13.0, 3.0, 3.0), vec3f(5.0, 4.0, 5.0)));
    walls = min(walls, sdf_box(q - vec3f(6.0, 6.0, -2.0), vec3f(3.2, 8.0, 3.2)));
    // hipped roofs: boxes cut by sloping planes
    let r1 = max(sdf_box(q - vec3f(0.0, 10.5, 0.0), vec3f(11.8, 2.0, 6.8)), (q.y - 9.5) * 0.9 + abs(q.z) * 0.45 - 3.0);
    let r2 = max(sdf_box(q - vec3f(6.0, 15.0, -2.0), vec3f(3.8, 1.5, 3.8)), (q.y - 14.0) * 0.9 + max(abs(q.x - 6.0), abs(q.z + 2.0)) * 0.5 - 1.6);
    let roof = min(r1, r2);
    return select(vec2f(walls, 4.0), vec2f(roof, 5.0), roof < walls) * vec2f(1.3, 1.0);
}

// a row of cypresses along a field edge on our own hill, bottom left
fn near_row(p: vec3f) -> f32 {
    let sp = 9.0;
    let c = clamp(round((p.x + 95.0) / sp), 0.0, 5.0);
    let h = hash_cell2(vec2i(i32(c), 3), 0x2e1u);
    let x = -95.0 + c * sp + (h.y - 0.5) * 4.0;
    let z = -330.0 + c * 4.0 + (h.z - 0.5) * 3.0;
    let g = terrain_h(vec2f(x, z), 1);
    return cypress(p - vec3f(x, g - 0.5, z), 11.0 + 8.0 * h.x);
}

// a few cypresses round the farm
fn farm_trees(p: vec3f) -> f32 {
    var d = 1e5;
    for (var i = 0; i < 5; i++) {
        let h = hash_cell2(vec2i(i, 9), 0x7f1u);
        let c = FARM + vec2f(select(-28.0, 30.0, i > 2) + 10.0 * h.x, -40.0 + 45.0 * h.y);
        let g = terrain_h(c, 1);
        d = min(d, cypress(p - vec3f(c.x, g - 0.5, c.y), 12.0 + 6.0 * h.z));
    }
    return d;
}

// → (distance, material): 1 fields, 3 cypress, 4 stone, 5 roof tiles
fn sdf(p: vec3f, lod: i32) -> vec2f {
    if (p.y > 420.0) { return vec2f(p.y - 410.0, 0.0); }
    let th = terrain_h(p.xz, lod);
    var res = vec2f((p.y - th) * 0.8, 1.0);
    if (p.y - th < 22.0) {
        // along the road
        if (on_road(p.z) && abs(p.x - road_x(p.z)) < 40.0) {
            let rt = road_trees(p);
            if (rt < res.x) { res = vec2f(rt, 3.0); }
        }
        if (abs(p.x + 72.0) < 40.0 && abs(p.z + 320.0) < 25.0) {
            let nr = near_row(p);
            if (nr < res.x) { res = vec2f(nr, 3.0); }
        }
        let fq = p.xz - FARM;
        if (dot(fq, fq) < 3600.0) {
            let fb = farm(p, terrain_h(FARM, 2) - 1.0);
            if (fb.x < res.x) { res = fb; }
            let ft = farm_trees(p);
            if (ft < res.x) { res = vec2f(ft, 3.0); }
        }
    }
    return res;
}

fn trace(ro: vec3f, rd: vec3f, n: i32, ctx: Ctx) -> vec2f {
    var t = 1.0;
    let foot = ctx.px * 0.4;
    var h = vec2f(1e9, 0.0);
    var w = 1.3;
    var last_r = 0.0;
    var last_step = 0.0;
    for (var i = 0; i < 200; i++) {
        if (i >= n) { break; }
        let q = ro + rd * t;
        if (q.y > 420.0 && rd.y > 0.0) { return vec2f(-1.0, 0.0); }
        h = sdf(q, 1);
        if (w > 1.0 && h.x + last_r < last_step) {
            t += last_r - last_step;
            last_step = last_r;
            w = 1.0;
            continue;
        }
        if (h.x < foot * t) { return vec2f(t, h.y); }
        last_r = h.x;
        last_step = h.x * w;
        t += last_step;
        if (t > 9000.0) { return vec2f(-1.0, 0.0); }
    }
    if (h.x < 0.03 * t) { return vec2f(t, h.y); }
    return vec2f(-1.0, 0.0);
}

fn nrm(p: vec3f, t: f32, ctx: Ctx) -> vec3f {
    let e = max(0.02, t * ctx.px * 0.5);
    let k = vec2f(1.0, -1.0);
    return normalize(
        k.xyy * sdf(p + k.xyy * e, 2).x + k.yyx * sdf(p + k.yyx * e, 2).x +
        k.yxy * sdf(p + k.yxy * e, 2).x + k.xxx * sdf(p + k.xxx * e, 2).x);
}

// long soft shadows: hills and the cypresses both cast them
fn shadow(ro: vec3f, l: vec3f) -> f32 {
    var res = 1.0;
    var t = 0.5;
    for (var i = 0; i < 14; i++) {
        let p = ro + l * t;
        if (p.y > 410.0) { break; }
        // far along the ray a coarser terrain will do (it sits 7 m low
        // on average: bias it back up)
        var h: f32;
        if (t < 60.0) { h = sdf(p, 1).x; } else { h = sdf(p, 0).x + 7.5; }
        res = min(res, 10.0 * h / t);
        if (res < 0.01) { break; }
        t += clamp(h, 0.8 + t * 0.04, 150.0);
    }
    return saturate(res);
}

// ------------------------------------------------------------ atmosphere

fn sky(rd: vec3f, l: Look, full: bool) -> vec3f {
    // (a hazy horizon stays bright: don't let the model darken right at it)
    // brightness from the physical model; hue from a palette read off
    // photographs of the valley (the model turns khaki this close to the
    // horizon: it has no ozone and a single aerosol)
    let d = normalize(vec3f(rd.x, max(rd.y, 0.25), rd.z));
    let y = max(rd.y, 0.0);
    // a hazy sky is brightest at the horizon
    let lum = col_luma(sky_atmosphere_haze(d, l.sun, 1.1)) * (1.0 + 0.35 * exp(-y * 12.0));
    var hor: vec3f;
    var top: vec3f;
    var glow: vec3f;
    if (l.season < 0.5) {
        hor = col_hex(0xf2cfb2u); top = col_hex(0x93aad6u); glow = col_hex(0xf7bd84u);
    } else if (l.season < 1.5) {
        hor = col_hex(0xe2e6e6u); top = col_hex(0x7aa2d6u); glow = col_hex(0xf2ead8u);
    } else {
        hor = col_hex(0xefcb98u); top = col_hex(0x98b0d6u); glow = col_hex(0xf6c07cu);
    }
    var pal = mix(hor, top, smoothstep(0.0, 0.12, y));
    let toward = pow(saturate(dot(normalize(vec3f(rd.x, 0.0, rd.z) + vec3f(1e-4)), normalize(vec3f(l.sun.x, 0.0, l.sun.z)))), 3.0);
    pal = mix(pal, glow, toward * exp(-y * 10.0) * 0.7);
    var c = pal * lum / max(col_luma(pal), 1e-4) * 1.25;
    if (full) { c += sky_sun_disk(rd, l.sun, 0.5); }
    return c;
}

fn aerial(col: vec3f, ro: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let hz = sky(normalize(vec3f(rd.x, 0.0, rd.z)), l, false);
    var c = mix(col, hz, 1.0 - exp(-t * 0.0001 * l.haze));
    // fog pooled in the hollows: thick below ~30 m, patchy, drifting
    if (l.fog > 0.0) {
        let hp = ro + rd * t;
        let patch_n = noise_fbm2(hp.xz * 0.004 + vec2f(ctx.t * 0.012, ctx.t * 0.004), 3);
        let dens = 0.02 * l.fog * (0.4 + 1.2 * patch_n);
        let fogc = l.amb * 1.1 + l.sun_c * 0.25 * (0.4 + 0.6 * pow(saturate(dot(rd, l.sun) * 0.5 + 0.5), 3.0));
        c = fog_height(c, fogc, ro - vec3f(0.0, 85.0, 0.0), rd, t, dens, 0.08);
    }
    c += fog_sun(rd, l.sun, l.sun_c, 0.25 * (1.0 - exp(-t * 0.0004)));
    return c;
}

// ------------------------------------------------------------ surfaces

// a field parcel: crop colour, row direction, distance to its edge
struct Parcel { crop: vec3f, ang: f32, edge: f32 }
fn parcel(xz: vec2f, l: Look) -> Parcel {
    // parcels on a skewed grid, edges wobbling with the contours
    let q = vec2f(xz.x * 0.9 + xz.y * 0.35, xz.y * 0.95 - xz.x * 0.2) / 170.0;
    let w = q + 0.25 * vec2f(snoise(xz * 0.004), snoise(xz * 0.004 + 9.0));
    let c = floor(w);
    let h = hash_cell2(vec2i(c), 0x7a5u);
    let f = fract(w);
    let edge = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
    // crops by season
    let wheat_g = col_hex(0x7d8c3cu);
    let wheat_y = col_hex(0xc9a64eu);
    let pasture = col_hex(0x6f7f3au);
    let clay = col_hex(0x8a6a4eu);
    let stubble = col_hex(0xb09a68u);
    var crop: vec3f;
    let r = h.x;
    if (l.season < 0.5) {
        crop = select(select(wheat_g, pasture, r > 0.55), clay * 1.1, r > 0.88);
    } else if (l.season < 1.5) {
        crop = select(select(wheat_y, stubble, r > 0.5), pasture, r > 0.85);
    } else {
        crop = select(select(clay, clay * vec3f(0.85, 0.8, 0.75), r > 0.45), select(stubble, pasture, r > 0.93), r > 0.75);
    }
    crop *= 0.85 + 0.3 * h.y;
    return Parcel(crop, h.z * PI, edge);
}

fn field_col(p: vec3f, n: vec3f, t: f32, rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let pc = parcel(p.xz, l);
    var alb = pc.crop;
    // rows / furrows, fading out with distance
    let dir = vec2f(cos(pc.ang), sin(pc.ang));
    let rows = sin(dot(p.xz, dir) * 2.2) * 0.5 + 0.5;
    let fade = 1.0 - saturate(t * ctx.px * 0.35);
    alb *= 1.0 - 0.18 * rows * fade;
    // hedges and grassy margins between parcels
    alb = mix(alb, col_hex(0x5a6434u), sstep(0.008, 0.0, pc.edge) * 0.35);
    // erosion: pale bare clay (crete senesi) on steep banks
    alb = mix(alb, col_hex(0xb8a488u), smoothstep(0.25, 0.45, 1.0 - n.y) * 0.8);
    // the white road
    if (on_road(p.z)) {
        let rd_ = abs(p.x - road_x(p.z));
        alb = mix(alb, col_hex(0xd8ccb4u), sstep(3.0, 2.0, rd_));
    }
    // wind running across the standing crops: slow bright waves
    let crop = 1.0 - sstep(0.012, 0.0, pc.edge);
    let wv = sin(dot(p.xz, vec2f(0.045, 0.02)) - ctx.t * 1.3 + 2.0 * noise_value2(p.xz * 0.01));
    alb *= 1.0 + 0.16 * wv * crop * (1.0 - saturate(t / 2500.0)) * noise_value2(p.xz * 0.006 - vec2f(ctx.t * 0.02, 0.0));
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    if (dif > 0.0 && t < 5000.0) { sh = shadow(p + n * 0.3, l.sun); }
    sh *= drift_shade(p, l, ctx.t);
    return alb * (l.sun_c * dif * sh * 1.1 + l.amb * (0.6 + 0.4 * n.y));
}

// shadows of fair-weather clouds drifting over the hills
fn drift_shade(p: vec3f, l: Look, t: f32) -> f32 {
    if (l.fog > 0.5) { return 1.0; }
    let q = p.xz - l.sun.xz / max(l.sun.y, 0.15) * (1200.0 - p.y) + vec2f(t * 4.0, t * 1.5);
    let n = noise_fbm2(q * 0.0011 + 5.0, 3);
    return mix(1.0, 0.35, smoothstep(0.58, 0.68, n));
}

fn shade(p: vec3f, rd: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = nrm(p, t, ctx);
    if (mat < 1.5) { return field_col(p, n, t, rd, l, ctx); }
    var alb: vec3f;
    if (mat < 3.5) {
        // cypress: near-black green, dense
        alb = mix(col_hex(0x142012u), col_hex(0x24361cu), noise_value2(p.xz * 2.0 + p.y));
    } else if (mat < 4.5) {
        // warm stone, rough
        alb = col_hex(0xe0cdb0u) * (0.85 + 0.2 * noise_value2(vec2f(p.x + p.z, p.y) * 0.8));
        // small dark windows
        let hq = p - vec3f(FARM.x, 0.0, FARM.y);
        let u = hq.x * 0.87 + hq.z * 0.5 + hq.z * 0.0;
        let win = step(0.7, fract(u * 0.3)) * step(0.55, fract(p.y * 0.3));
        alb = mix(alb, vec3f(0.03), win * 0.8);
    } else {
        alb = col_hex(0xa35a3cu) * (0.85 + 0.2 * noise_value2(p.xz * 2.0));
    }
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    if (dif > 0.0) { sh = shadow(p + n * 0.3, l.sun); }
    return alb * (l.sun_c * dif * sh * 1.1 + l.amb * (0.6 + 0.4 * n.y));
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(-10.0, terrain_h(vec2f(-10.0, 0.0), 2) + 25.0, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.12, -0.115, -1.0), 0.0, 24.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    let hit = trace(ro, rd, steps(88.0, ctx), ctx);
    if (hit.x > 0.0) {
        col = aerial(shade(ro + rd * hit.x, rd, hit.x, hit.y, l, ctx), ro, rd, hit.x, l, ctx);
    } else {
        col = sky(rd, l, true);
        // beyond 9 km: ranges of hills by direction, Monte Amiata's cone
        let a = atan2(rd.x, -rd.z);
        let hz = sky(normalize(vec3f(rd.x, 0.0, rd.z)), l, false);
        for (var k = 0; k < 2; k++) {
            let fk = f32(k);
            var ridge = -0.012 + 0.012 * noise_fbm2(vec2f(a * (9.0 - fk * 3.0) + fk * 5.0, fk), 3) + 0.004 * fk;
            if (k == 1) { ridge = max(ridge, 0.036 * exp(-sq((a - 0.42) / 0.3)) - 0.004); }
            if (rd.y < ridge) {
                let shade_c = mix(hz * vec3f(0.84, 0.88, 0.97), hz * vec3f(0.92, 0.94, 0.99), fk) + l.sun_c * 0.01 * saturate(dot(normalize(vec3f(rd.x, 0.0, rd.z)), l.sun));
                col = mix(col, shade_c, 0.9);
                break;
            }
        }
        // a few soft banks of high cloud catching the low light
        if (rd.y > 0.0) {
            let hp = rd.xz / (rd.y + 0.12) * 0.7 + vec2f(ctx.t * 0.0015, 0.0);
            let n = noise_fbm2(hp * vec2f(0.6, 1.6) + 3.0, 4);
            let cov = saturate((n - 0.58) * 2.5) * smoothstep(0.02, 0.12, rd.y);
            let under = sky(normalize(vec3f(rd.x, 0.0, rd.z)), l, false);
            col = mix(col, under * 1.05 + l.sun_c * 0.05, cov * 0.45);
        }
        if (l.fog > 0.0) {
            let band = exp(-max(rd.y, 0.0) * 40.0) * l.fog * 0.3;
            col = mix(col, l.amb * 1.2 + l.sun_c * 0.1, saturate(band));
        }
    }
    return col * exp2(l.exposure);
}
