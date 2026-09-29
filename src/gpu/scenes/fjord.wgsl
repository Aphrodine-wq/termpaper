//! name: fjord
//! title: Norwegian Fjord
//! category: wilds
//! tags: fjord, waterfall, mountains, snow, aurora, norway
//! desc: sheer walls dropping into a still green fjord as a ferry draws its wake past the falls
//! themes: summer, winter, overcast
//! uses: camera, sdf, sky, fog, stars
//! cost: heavy
//! fallback: alpine
//! credits: original

// World units are metres, y up, the fjord's surface at y = 0. The camera
// hangs 330 m up over the basin at the head of the fjord (Geiranger)
// looking down its winding length: gneiss walls, scored by gullies and
// broken into crags, climb past wooded benches to a snowy plateau
// 1100-1400 m up, the Seven Sisters thread down the left wall, and a ferry
// heads out, its Kelvin wake spreading across the mirror. The walls are a
// heightfield; at normal-estimation detail the steep faces also carry 3D
// crag noise, so they shade as broken rock without slowing the march.

struct Look {
    sun: vec3f,
    night: f32,      // 1 = polar twilight with aurora
    overcast: f32,
    haze: f32,
    ceil: f32,       // cloud base hanging on the walls (0: none)
    snow: f32,       // snow line lowered to the water
    mist: f32,       // mist lying on the far water
    exposure: f32,
    sun_c: vec3f,
    amb: vec3f,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: { l = Look(sky_sun_dir(-7.0, -40.0), 1.0, 0.0, 1.0, 0.0, 1.0, 0.25, 1.1, vec3f(0.0), vec3f(0.0)); }
        case 2u: { l = Look(sky_sun_dir(22.0, 40.0), 0.0, 0.8, 1.05, 860.0, 0.0, 0.8, -0.45, vec3f(0.0), vec3f(0.0)); }
        default: { l = Look(sky_sun_dir(32.0, -125.0), 0.0, 0.1, 0.9, 0.0, 0.0, 0.2, -0.25, vec3f(0.0), vec3f(0.0)); }
    }
    if (l.night > 0.5) {
        l.sun_c = vec3f(0.0);
        l.amb = vec3f(0.014, 0.02, 0.04);
    } else {
        let sl = sky_sun_light(l.sun);
        let sa = sky_ambient(l.sun);
        l.sun_c = sl * (1.0 - 0.85 * l.overcast);
        l.amb = mix(sa, vec3f(col_luma(sa + sl * 0.12)) * vec3f(0.94, 0.98, 1.04), l.overcast) * (1.0 + 0.5 * l.overcast);
    }
    return l;
}

// ------------------------------------------------------------ geometry

fn snoise(p: vec2f) -> f32 { return noise_value2(p) * 2.0 - 1.0; }
// signed 1D value noise: two integer hashes (for things that vary along z)
fn n1(x: f32) -> f32 {
    let i = floor(x);
    let f = x - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash_f(bitcast<u32>(i32(i)));
    let b = hash_f(bitcast<u32>(i32(i) + 1));
    return mix(a, b, u) * 2.0 - 1.0;
}

// the fjord winds: centre line and half width along its length
fn fj_x(z: f32) -> f32 { return 520.0 * sin(z * 0.00038 + 0.2) + 160.0 * sin(z * 0.0011 + 1.0) - 150.0; }
// (it opens out into a wide basin at its head, under the camera)
fn fj_w(z: f32) -> f32 { return 380.0 + 110.0 * sin(z * 0.0007 + 2.0) + 480.0 * smoothstep(-1800.0, 700.0, z); }

// plateau height on either side
fn plateau(z: f32, side: f32) -> f32 {
    return 1150.0 + 220.0 * sin(z * 0.0004 + side * 1.7) + 140.0 * n1(z * 0.0009 + side * 31.0);
}

// terrain height and its steepness bound: (height, |gradient| estimate)
fn terrain(xz: vec2f, lod: i32) -> vec2f {
    let dx = xz.x - fj_x(xz.y);
    let side = sign(dx);
    // spurs and bays along the shore
    let d0 = abs(dx) - fj_w(xz.y) + 130.0 * n1(xz.y * 0.0022 + side * 57.0) + 40.0 * n1(xz.y * 0.008 + side * 91.0);
    // under the water the walls just keep plunging
    if (d0 < 0.0) { return vec2f(d0 * 3.0, 3.0); }
    let top = plateau(xz.y, side);
    let L = 205.0 + 55.0 * n1(xz.y * 0.0012 + side * 13.0);
    let wall = smoothstep(0.0, 90.0, d0) * (1.0 - smoothstep(800.0, 1300.0, d0));
    // the smooth profile's height: the wall's vertical coordinate
    let hv = top * (1.0 - exp(-max(d0, 0.0) / L));
    // crags and buttresses: the face pushed in and out over (along, up)
    let fu = vec2f(xz.y * 0.0042 + side * 7.0, hv * 0.0042);
    var cr = snoise(fu);
    if (lod > 0) { cr += 0.45 * snoise(fu * 2.4 + 5.0); }
    if (lod > 1) { cr += 0.22 * snoise(fu * 5.5 + 11.0); }
    // arêtes and gullies down the fall line: sharp crests between broad
    // gullies, cut into the face (a horizontal push: the walls are steep)
    var gz = xz.y;
    if (lod > 0) { gz += 160.0 * snoise(xz * 0.0021); }
    var gul = abs(snoise(vec2f(gz * 0.0062, hv * 0.0011 + side * 5.0)));
    var gdep = 0.7;
    if (lod > 0) {
        gul += 0.35 * abs(snoise(vec2f(gz * 0.017 + 3.0, hv * 0.003 + side * 2.0)));
        gdep = 0.45 + 0.55 * noise_value2(vec2f(gz * 0.002, side));
    }
    var d = d0 + (34.0 * cr - 90.0 * gul * gdep) * wall;
    // cliff bands between wooded benches, in some reaches of the wall
    let band = smoothstep(0.1, 0.6, n1(xz.y * 0.0011 + side * 23.0));
    let toff = 0.7 * n1(xz.y * 0.0016 + side * 71.0);
    let tx = max(d, 0.0) / 140.0 + toff;
    let de = mix(max(d, 0.0), (floor(tx) + smoothstep(0.35, 0.75, fract(tx)) - toff) * 140.0, 0.6 * band * wall);
    let e = exp(-de / L);
    var h = top * (1.0 - e);
    let g = top / L * e * (1.0 + 1.5 * band * wall) * (1.0 + 2.4 * wall);
    // below the waterline the walls keep plunging
    h = select(h, d * 3.0, d < 0.0);
    // knobs and peaks on the plateau
    if (d0 > 150.0) {
        h += 180.0 * snoise(xz * 0.0011 + side * 9.0) * smoothstep(300.0, 1200.0, d0);
        // the gullies notch the brow, and crags break the skyline
        h -= 110.0 * gul * gul * gdep * smoothstep(150.0, 450.0, d0) * (1.0 - smoothstep(900.0, 1400.0, d0));
        let pk = 1.0 - abs(snoise(xz * 0.0032 + side * 4.0));
        h += 110.0 * pk * pk * pk * smoothstep(200.0, 600.0, d0);
    }
    if (lod > 0) { h += 16.0 * snoise(xz * 0.006); }
    if (lod > 1) { h += 5.0 * snoise(xz * 0.05 + 9.0); }
    return vec2f(h, g + 2.2);
}

// the ferry: position, and heading along the fjord
fn ferry_pos(t: f32) -> vec4f {
    let period = 960.0;
    let u = fmod_pos(t + 150.0, period) / period;
    let z = 150.0 - u * 4300.0;
    // fade in as it sails into view, out as it vanishes down the fjord
    let vis = smoothstep(0.0, 0.03, u) * (1.0 - smoothstep(0.9, 1.0, u));
    return vec4f(fj_x(z) + 70.0, 0.0, z, vis);
}
// unit heading (x, z) of the ferry: down the fjord's centre line
fn ferry_dir(f: vec4f) -> vec2f {
    return normalize(vec2f(fj_x(f.z - 60.0) - fj_x(f.z + 60.0), -120.0));
}
// ferry-local coordinates: x across, z toward the stern
fn ferry_local(p: vec3f, f: vec4f) -> vec3f {
    let dir = ferry_dir(f);
    let r = p.xz - f.xz;
    return vec3f(dot(r, vec2f(-dir.y, dir.x)), p.y, -dot(r, dir));
}
fn ferry(p: vec3f, f: vec4f) -> f32 {
    let q = ferry_local(p, f);
    let hull = sdf_round_box(q - vec3f(0.0, 2.5, 0.0), vec3f(10.0, 4.5, 52.0), 3.5);
    let deck = sdf_box(q - vec3f(0.0, 10.0, 8.0), vec3f(8.0, 3.5, 28.0));
    let bridge = sdf_box(q - vec3f(0.0, 15.0, -10.0), vec3f(6.0, 2.2, 6.0));
    return min(min(hull, deck), bridge);
}

// crags on the steep faces: 3D blocks and creases (0 mean); a heightfield
// alone can only score a cliff up and down
fn crag3(p: vec3f) -> f32 {
    let a = abs(noise_value3(p * vec3f(0.011, 0.016, 0.011)) - 0.5);
    let b = abs(noise_value3(p * 0.034 + 7.0) - 0.5);
    return (a + 0.45 * b) * 2.0 - 0.7;
}

// → (distance, material): 1 terrain, 3 ferry. At lod 2 (normals only) the
// faces carry the crags, so they shade as broken rock.
fn sdf(p: vec3f, f: vec4f, lod: i32) -> vec2f {
    if (p.y > 1650.0) { return vec2f(p.y - 1600.0, 0.0); }
    let tr = terrain(p.xz, lod);
    var res = vec2f((p.y - tr.x) / sqrt(1.0 + tr.y * tr.y) * 0.9, 1.0);
    if (lod > 1) { res.x -= 22.0 * crag3(p) * smoothstep(1.5, 5.0, tr.y) * step(1.0, p.y); }
    let fq = p - f.xyz;
    if (f.w > 0.01) {
        if (dot(fq, fq) < 6400.0) {
            let fd = ferry(p, f);
            if (fd < res.x) { res = vec2f(fd, 3.0); }
        } else {
            res.x = min(res.x, length(fq) - 70.0);
        }
    }
    return res;
}

fn trace(ro: vec3f, rd: vec3f, tmax: f32, n: i32, f: vec4f, lod: i32, ctx: Ctx) -> vec2f {
    var t = 2.0;
    let foot = ctx.px * 0.4;
    var h = vec2f(1e9, 0.0);
    var w = 1.3;
    var last_r = 0.0;
    var last_step = 0.0;
    for (var i = 0; i < 200; i++) {
        if (i >= n) { break; }
        let q = ro + rd * t;
        if (q.y > 1650.0 && rd.y > 0.0) { return vec2f(-1.0, 0.0); }
        h = sdf(q, f, lod);
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
        if (t > tmax) { break; }
    }
    if (h.x < 0.02 * t) { return vec2f(t, h.y); }
    return vec2f(-1.0, 0.0);
}

fn nrm(p: vec3f, t: f32, f: vec4f, lod: i32, ctx: Ctx) -> vec3f {
    let e = max(0.05, t * ctx.px * 0.5);
    let k = vec2f(1.0, -1.0);
    return normalize(
        k.xyy * sdf(p + k.xyy * e, f, lod).x + k.yyx * sdf(p + k.yyx * e, f, lod).x +
        k.yxy * sdf(p + k.yxy * e, f, lod).x + k.xxx * sdf(p + k.xxx * e, f, lod).x);
}

fn shadow(ro: vec3f, l: vec3f, f: vec4f) -> f32 {
    var res = 1.0;
    var t = 8.0;
    for (var i = 0; i < 10; i++) {
        let p = ro + l * t;
        if (p.y > 1600.0) { break; }
        let h = sdf(p, f, 0).x;
        res = min(res, 8.0 * h / t);
        if (res < 0.01) { break; }
        t += clamp(h, 20.0 + t * 0.12, 700.0);
    }
    return saturate(res);
}

// ------------------------------------------------------------ sky

fn aurora(rd: vec3f, ctx: Ctx) -> vec3f {
    if (rd.y < 0.0) { return vec3f(0.0); }
    // curtains hanging along an arc across the northern sky
    let a = atan2(rd.x, -rd.z);
    let el = rd.y;
    let arc = 0.2 + 0.08 * sin(a * 1.7 + ctx.t * 0.02) + 0.04 * sin(a * 4.3 - ctx.t * 0.035);
    let band = exp(-sq((el - arc) / 0.06)) + 0.5 * exp(-sq((el - arc - 0.1) / 0.12));
    let rays = 0.55 + 0.45 * noise_value2(vec2f(a * 40.0 + ctx.t * 0.05, 0.5));
    let fold = 0.6 + 0.4 * sin(a * 9.0 + ctx.t * 0.04 + 3.0 * noise_value2(vec2f(a * 3.0, ctx.t * 0.01)));
    let top = sstep(arc + 0.25, arc, el);
    let c = mix(vec3f(0.1, 1.0, 0.45), vec3f(0.5, 0.2, 0.8), smoothstep(arc + 0.05, arc + 0.2, el));
    return c * band * rays * fold * top * 0.05 * smoothstep(-0.6, 0.4, -a);
}

fn sky(rd: vec3f, l: Look, ctx: Ctx, full: bool) -> vec3f {
    if (l.night > 0.5) {
        let y = max(rd.y, 0.0);
        // polar twilight: a deep blue sky, paler toward the hidden sun
        var c = mix(vec3f(0.03, 0.05, 0.11), vec3f(0.006, 0.012, 0.035), pow(y, 0.5));
        c += vec3f(0.04, 0.035, 0.05) * pow(saturate(dot(normalize(vec3f(rd.x, 0.0, rd.z)), normalize(vec3f(l.sun.x, 0.0, l.sun.z))) * 0.5 + 0.5), 4.0) * exp(-y * 6.0);
        if (full) {
            c += star_field(rd, 0.6, ctx) * smoothstep(0.05, 0.3, rd.y) * 0.6;
        }
        c += aurora(rd, ctx);
        return c;
    }
    var c = sky_atmosphere_haze(rd, l.sun, l.haze);
    let grey = vec3f(col_luma(c)) * vec3f(0.95, 0.98, 1.03);
    c = mix(c, grey * 1.15 + l.amb * 0.2, l.overcast * 0.9);
    // (no disk, and so no glint on the water, through an overcast)
    if (full && l.overcast < 0.5) { c += sky_sun_disk(rd, l.sun, 0.5) * (1.0 - l.overcast); }
    // cumulus, or the overcast's lumpy base (not in the haze colour: at a
    // fixed elevation it would streak)
    if (full && rd.y > -0.02) {
        let hp = rd.xz / (rd.y + 0.07) * 1.5 + vec2f(ctx.t * 0.002, ctx.t * 0.0007);
        let n = noise_fbm2(hp * vec2f(0.8, 1.3) + 5.0, 5);
        let covk = mix(0.6, 0.3, l.overcast);
        let cov = saturate((n - covk) * 3.0) * smoothstep(-0.02, 0.1, rd.y);
        let n2 = noise_fbm2(hp * vec2f(0.8, 1.3) + 5.0 + normalize(l.sun.xz + vec2f(1e-4)) * 0.05, 5);
        let lit = l.amb * mix(1.0, 0.7, cov) + l.sun_c * (0.3 + 0.6 * saturate(0.5 + (n - n2) * 5.0)) * 0.8;
        c = mix(c, lit, cov * 0.9);
    }
    return c;
}

fn aerial(col: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    // cool, blue-grey distance (the model's khaki horizon is its lack of ozone)
    let hz = sky(normalize(vec3f(rd.x, 0.07, rd.z)), l, ctx, false) * select(vec3f(0.92, 0.97, 1.08), vec3f(1.0), l.night > 0.5);
    var c = mix(col, hz * 0.95, 1.0 - exp(-t * 0.00006 * l.haze));
    if (l.night < 0.5) { c += fog_sun(rd, l.sun, l.sun_c, 0.15 * (1.0 - exp(-t * 0.0002))); }
    return c;
}

// the cloud base hanging on the walls: lumpy, drifting, the walls fading
// up into it
fn ceiling(col: vec3f, p: vec3f, l: Look, ctx: Ctx) -> vec3f {
    if (l.ceil <= 0.0) { return col; }
    let n = noise_fbm2(p.xz * 0.0017 + vec2f(ctx.t * 0.0012, ctx.t * 0.0005), 4);
    let base = l.ceil + 520.0 * (n - 0.5);
    let a = smoothstep(base - 140.0, base + 40.0, p.y);
    let cc = l.amb * (0.95 + 0.25 * n);
    return mix(col, cc, a);
}

// mist lying on the far water and the foot of the walls, in soft patches
fn lake_mist(col: vec3f, p: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let patchy = noise_fbm2(p.xz * vec2f(0.0016, 0.0009) + vec2f(ctx.t * 0.001, 0.0), 3);
    let a = l.mist * smoothstep(2500.0, 7000.0, t) * exp(-max(p.y, 0.0) / 20.0) * smoothstep(0.35, 0.75, patchy);
    let mc = select(l.amb * 1.15 + l.sun_c * 0.3, l.amb * 1.6, l.night > 0.5);
    return mix(col, mc, saturate(a));
}

// ------------------------------------------------------------ the falls

// The Seven Sisters: threads of water down a cliff band of the left wall,
// painted on the rock so they follow its every rib and ledge. The wall is
// seen obliquely down the fjord, so the threads are spaced across the line
// of sight, not along the wall, or they would pile into one sheet.
const FALLS_Z: f32 = -1500.0;
fn eye_pos() -> vec3f { return vec3f(fj_x(900.0) + 60.0, 330.0, 900.0); }
// (coverage, spray) at a point of the wall; fp = one pixel in metres there
fn sisters(p: vec3f, fp: f32, ctx: Ctx) -> vec2f {
    if (abs(p.z - FALLS_Z) > 450.0 || p.x > fj_x(p.z) || p.y > 480.0) { return vec2f(0.0); }
    let c = vec2f(fj_x(FALLS_Z) - fj_w(FALLS_Z), FALLS_Z);
    let v = normalize(c - eye_pos().xz);
    let s = dot(p.xz - c, vec2f(-v.y, v.x));
    if (abs(s) > 130.0) { return vec2f(0.0); }
    var cov = 0.0;
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let h = hash_cell2(vec2i(i, 7), 0x5157u);
        let top = 280.0 + 190.0 * h.x;
        let drop = max(top - p.y, 0.0);
        let sc = (fi - 2.0) * 36.0 + 14.0 * (h.y - 0.5) + 7.0 * snoise(vec2f(fi * 3.7, p.y * 0.012));
        let w = (1.6 + drop * 0.011) * (0.6 + 0.7 * h.z);
        let cv = saturate((w + 0.5 * fp - abs(s - sc)) / fp) * min(1.0, 2.0 * w / fp);
        // they break into spray and gather again down the ledges
        let pulse = (0.55 + 0.45 * noise_value2(vec2f(fi * 5.0, p.y * 0.03 + ctx.t * 1.3))) * (0.45 + 0.55 * smoothstep(0.25, 0.6, noise_value2(vec2f(fi * 9.0 + 2.0, p.y * 0.012))));
        cov = max(cov, cv * pulse * smoothstep(top + 2.0, top - 14.0, p.y));
    }
    // spray where they land (and only there, not all down the shore)
    let a = dot(p.xz - c, v);
    let spray = exp(-max(p.y, 0.0) / 22.0) * exp(-sq(s / 80.0)) * exp(-sq(a / 220.0));
    return vec2f(cov, spray);
}

// ------------------------------------------------------------ surfaces

fn rock_col(p: vec3f, n_in: vec3f, t: f32, l: Look, f: vec4f, shadows: bool, fp: f32, ctx: Ctx) -> vec3f {
    var n = n_in;
    // broken rock: facets bumped into the steep faces, sharp-edged
    let hl = length(n.xz);
    if (hl > 0.3 && fp < 20.0) {
        let t1 = vec3f(-n.z, 0.0, n.x) / hl;
        let t2 = cross(n, t1);
        let nd = noise_value2_d(vec2f(dot(p.xz, t1.xz) * 0.03, p.y * 0.04));
        let bp = nd.yz * 0.22 * smoothstep(24.0, 8.0, fp) * smoothstep(0.3, 0.7, hl);
        n = normalize(n + t1 * bp.x + t2 * bp.y);
    }
    let slope = 1.0 - n.y;
    let n1v = noise_fbm2(p.xz * 0.004 + p.y * 0.001, 4);
    let u = p.x + p.z;
    // dark gneiss, banded and blotched; cavities of the crags dark and wet,
    // the jutting blocks paler
    var alb = mix(col_hex(0x28292bu), col_hex(0x585653u), n1v);
    if (hl > 0.3 && fp < 40.0) { alb *= 0.75 + 0.5 * saturate(crag3(p) + 0.5); }
    alb *= 0.75 + 0.45 * noise_value2(vec2f(u * 0.02, p.y * 0.012));
    // snowmelt: dark wet seeps and pale water-washed streaks down the fall line
    alb *= 1.0 - 0.5 * smoothstep(0.6, 0.85, noise_value2(vec2f(u * 0.035, p.y * 0.0028)));
    alb = mix(alb, col_hex(0x7c7f82u), smoothstep(0.74, 0.92, noise_value2(vec2f(u * 0.05 + 7.0, p.y * 0.0022))) * 0.35 * saturate(slope * 1.5));
    // pine and birch on every bench and gully floor up to ~700 m
    let patchy = noise_value2(p.xz * 0.012 + p.y * 0.01);
    // forest up the lower walls on all but the sheerest rock, thinning to
    // wooded benches higher up, none above ~750 m
    let low = smoothstep(480.0, 120.0, p.y + 120.0 * (n1v - 0.5));
    let vs = slope + 0.3 * (patchy - 0.5);
    // (on the sheer faces the crags would speckle it: there it gathers in
    // broad patches, on the benches and in the gullies)
    let sheer = smoothstep(2.5, 6.0, terrain(p.xz, 0).y);
    let broad = mix(1.0, smoothstep(0.45, 0.65, noise_fbm2(vec2f(u * 0.006, p.y * 0.009), 2)), sheer);
    let veg = sstep(0.66 + 0.26 * low, 0.56 + 0.26 * low, vs) * (1.0 - smoothstep(550.0, 800.0, p.y + 200.0 * n1v)) * mix(broad, 1.0, low * 0.6);
    let wood = mix(col_hex(0x132016u), col_hex(0x34441fu), noise_value2(p.xz * 0.03) * 0.6 + n1v * 0.4);
    alb = mix(alb, wood, veg * (1.0 - l.snow * 0.7) * smoothstep(6.0, 30.0, p.y));
    // bare fell above the trees: grey-olive tundra and rock
    let fell = smoothstep(650.0, 950.0, p.y + 150.0 * n1v) * sstep(0.6, 0.3, slope);
    alb = mix(alb, mix(col_hex(0x4a4c40u), col_hex(0x6a6a5au), n1v), fell * 0.8);
    // snow: on the plateau, and on every ledge in winter
    let line = mix(1000.0, -50.0, l.snow);
    let snow = smoothstep(line - 80.0, line + 80.0, p.y + 180.0 * (n1v - 0.5)) * sstep(0.75, 0.35, slope + 0.2 * l.snow * (1.0 - n1v)) * mix(1.0, broad, l.snow);
    alb = mix(alb, vec3f(0.8, 0.83, 0.88), snow);
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    if (shadows && dif > 0.0 && l.night < 0.5 && l.overcast < 0.9) { sh = shadow(p + n * (3.0 + t * ctx.px * 1.5), l.sun, f); }
    let occ = 0.7 + 0.3 * saturate(n.y + 0.4);
    var c = alb * (l.sun_c * dif * sh * 1.2 + l.amb * (0.6 + 0.4 * n.y) * 1.2 * occ);
    if (l.night > 0.5) { c += alb * vec3f(0.0, 0.02, 0.01) * snow; }
    // the falls, white threads catching the light, and their spray
    let fs = sisters(p, fp, ctx);
    if (fs.x + fs.y > 0.0) {
        let wl = vec3f(0.85, 0.88, 0.9) * (l.sun_c * max(dif * sh, 0.25) * 0.9 + l.amb * 1.5);
        c = mix(c, wl, fs.x);
        c = mix(c, wl * 0.9, fs.y * 0.4);
    }
    return c;
}

fn ferry_col(p: vec3f, n: vec3f, l: Look, f: vec4f, ctx: Ctx) -> vec3f {
    let q = ferry_local(p, f);
    // white superstructure over a dark blue hull, a band of windows
    var alb = vec3f(0.8);
    alb = mix(alb, col_hex(0x1c2438u), step(q.y, 1.6));
    let win = step(8.8, q.y) * step(q.y, 10.8) * step(0.45, fract(q.z * 0.2));
    alb = mix(alb, vec3f(0.06), win * 0.8);
    var c = alb * (l.sun_c * saturate(dot(n, l.sun)) * 1.1 + l.amb * 1.3);
    if (l.night > 0.5) { c += col_kelvin(3000.0) * 1.5 * win; }
    return c;
}

// the ferry's wake on the water: (normal tilt across, along, foam)
fn wake(xz: vec2f, f: vec4f) -> vec3f {
    if (f.w < 0.01) { return vec3f(0.0); }
    let q = ferry_local(vec3f(xz.x, 0.0, xz.y), f);
    let behind = q.z - 50.0;
    if (behind < -110.0 || behind > 2600.0) { return vec3f(0.0); }
    let lat = q.x;
    let s = max(behind, 1.0);
    // Kelvin arms at 19.5 degrees, fading with distance
    let arm = abs(lat) - 0.354 * s - 8.0;
    let aw = 7.0 + 0.018 * s;
    let fade = exp(-s / 320.0) * f.w;
    let arms = exp(-sq(arm / aw)) * fade * (0.5 + 0.5 * noise_value2(vec2f(s * 0.05, sign(lat) * 3.0)));
    // transverse ripples between the arms
    let inside = sstep(0.0, -20.0, arm) * fade * 0.4;
    let trans = inside * sin(behind * 0.12);
    // churned white track right behind the stern, and the bow wave
    let track = exp(-sq(lat / (7.0 + 0.025 * s))) * exp(-s / 170.0) * f.w * step(-60.0, behind);
    let bow = exp(-sq(lat / 14.0)) * exp(-sq((behind + 100.0) / 12.0)) * f.w;
    return vec3f(sign(lat) * arms * 0.04, trans * 0.02, track * 0.6 + arms * 0.24 + bow * 0.6);
}

fn fjord_water(p: vec3f, rd: vec3f, t: f32, l: Look, f: vec4f, cam: Cam, ctx: Ctx) -> vec3f {
    // nearly still: faint cat's-paws only
    let cat = smoothstep(0.55, 0.8, noise_value2(p.xz * 0.004 + vec2f(ctx.t * 0.003, 0.0)));
    let wn = vec2f(snoise(p.xz * 0.12 + vec2f(ctx.t * 0.08, 0.0)), snoise(p.xz * 0.12 + vec2f(3.0, ctx.t * 0.07))) * (0.002 + 0.01 * cat);
    let wk = wake(p.xz, f);
    let n = normalize(vec3f(wn.x + wk.x, 1.0, wn.y + wk.y));
    let r = reflect(rd, n);
    var refl: vec3f;
    // the reflected world: a cheaper trace of the walls
    let h = trace(p + vec3f(0.0, 0.5, 0.0), r, 16000.0, steps(28.0, ctx), f, 0, ctx);
    if (h.x > 0.0) {
        let hp = p + r * h.x;
        let hn = nrm(hp, h.x + t, f, 1, ctx);
        var c: vec3f;
        if (h.y > 2.5) {
            c = ferry_col(hp, hn, l, f, ctx);
        } else {
            c = rock_col(hp, hn, h.x + t, l, f, false, (h.x + t) * ctx.px / cam.zoom, ctx);
        }
        refl = ceiling(aerial(c, r, h.x, l, ctx), hp, l, ctx);
    } else {
        refl = sky(r, l, ctx, true);
    }
    let fres = 0.02 + 0.98 * pow(1.0 - saturate(dot(n, -rd)), 5.0);
    // deep glacial green below
    var body = col_hex(0x0f3a36u) * (l.amb * 0.9 + l.sun_c * 0.05);
    if (l.night > 0.5) { body = vec3f(0.002, 0.006, 0.008); }
    var c = mix(body, refl, saturate(fres * 1.3 + 0.25));
    let foam = (l.amb * 1.2 + l.sun_c * 0.55) * 0.9;
    c = mix(c, foam, saturate(wk.z));
    // spray from the falls drifting over the water at their foot
    let fq = p.xz - vec2f(fj_x(FALLS_Z) - fj_w(FALLS_Z) + 40.0, FALLS_Z);
    let spray = exp(-dot(fq, fq) / 9000.0) * (0.6 + 0.4 * noise_value2(p.xz * 0.05 + vec2f(0.0, ctx.t * 0.2)));
    c = mix(c, foam, spray * 0.35);
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let f = ferry_pos(ctx.t);
    let ro = eye_pos();
    let cam = cam_look_at(ro, ro + vec3f(-0.3, -0.1, -1.0), 0.0, 42.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    let tw = select(1e6, -ro.y / rd.y, rd.y < 0.0);
    let hit = trace(ro, rd, min(tw, 30000.0), steps(92.0, ctx), f, 1, ctx);
    if (hit.x > 0.0) {
        let hp = ro + rd * hit.x;
        let n = nrm(hp, hit.x, f, 2, ctx);
        var c: vec3f;
        if (hit.y > 2.5) {
            c = ferry_col(hp, n, l, f, ctx);
        } else {
            c = rock_col(hp, n, hit.x, l, f, true, hit.x * ctx.px / cam.zoom, ctx);
        }
        col = aerial(c, rd, hit.x, l, ctx);
        col = lake_mist(col, hp, hit.x, l, ctx);
        col = ceiling(col, hp, l, ctx);
    } else if (tw < 1e5) {
        let hp = ro + rd * tw;
        col = aerial(fjord_water(hp, rd, tw, l, f, cam, ctx), rd, tw, l, ctx);
        col = lake_mist(col, hp, tw, l, ctx);
    } else {
        col = sky(rd, l, ctx, true);
    }
    return col * exp2(l.exposure);
}
