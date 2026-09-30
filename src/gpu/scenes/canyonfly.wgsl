//! name: canyonfly
//! title: Canyon Flight
//! category: aerial
//! tags: drone, canyon, sandstone, river, flight, aerial
//! desc: a slow drone flight up a sandstone canyon, the river winding below, one wall in sun
//! themes: noon, sunset, dawn
//! params: speed, haze
//! uses: camera, raymarch, sky, noise, fog, water
//! cost: heavy
//! fallback: alpine
//! credits: original

// World units are metres. The canyon runs along +z and the drone flies up it
// (toward +z) at about 6 m/s, hanging a little below mid-height between the
// walls and turning gently with the bends. The centre line meanders slowly
// in x. Each wall is a 45-degree scree apron under a sheer, horizontally
// bedded face, capped by a flat slickrock plateau; a shallow green river
// fills the trough in the floor and runs back down-canyon past the camera.
// The sun lights one wall and leaves the other to the sky; the far bends
// dissolve into haze.

struct Look {
    sun: vec3f,        // toward the sun
    sun_c: vec3f,      // direct sunlight reaching the canyon
    amb: vec3f,        // sky light: what the shadow wall sees
    haze_c: vec3f,     // aerial-perspective colour
    fogd: f32,         // haze density per metre (as designed, before the Tune slider)
    mist: f32,         // river mist density at the floor, falling off with height
    mist_c: vec3f,
    sky_haze: f32,     // sky-model haze: 1 clear .. 6 thick
    glow: f32,         // forward-scatter glow in the haze toward the sun
    exposure: f32,
    deep: vec3f,       // river body colour where it is deep
    shallow: vec3f,    // ... and over the gravel bars
}

// sun direction from elevation and an azimuth measured from straight ahead
// (+z) toward the right (+x), so the lit wall is easy to reason about
fn sun_from(elev_deg: f32, az_deg: f32) -> vec3f {
    let e = radians(elev_deg);
    let a = radians(az_deg);
    return normalize(vec3f(cos(e) * sin(a), sin(e), cos(e) * cos(a)));
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            // sunset: the sun low at the far end of the canyon, grazing the
            // left wall, orange haze, purple shadows
            l = Look(sun_from(7.0, 14.0), vec3f(1.7, 0.78, 0.22), vec3f(0.22, 0.16, 0.28),
                     vec3f(0.95, 0.46, 0.20), 0.0021, 0.004, vec3f(0.55, 0.30, 0.25),
                     3.0, 1.0, 1.3, vec3f(0.018, 0.06, 0.05), vec3f(0.14, 0.24, 0.17));
        }
        case 2u: {
            // dawn: soft light from the left onto the right wall, pale
            // blue-pink sky, cool mist lying on the river
            l = Look(sun_from(11.0, -58.0), vec3f(1.55, 1.05, 0.62), vec3f(0.24, 0.29, 0.40),
                     vec3f(0.62, 0.60, 0.68), 0.0016, 0.025, vec3f(0.50, 0.56, 0.66),
                     2.0, 0.35, 1.0, vec3f(0.02, 0.07, 0.07), vec3f(0.12, 0.28, 0.24));
        }
        default: {
            // noon: high sun from the right, the left wall blazing, the right
            // wall in blue shadow, light haze
            l = Look(sun_from(60.0, 72.0), vec3f(2.9, 2.6, 2.3), vec3f(0.22, 0.34, 0.60),
                     vec3f(0.66, 0.70, 0.80), 0.0009, 0.0, vec3f(0.60, 0.65, 0.75),
                     1.2, 0.15, 0.6, vec3f(0.02, 0.10, 0.09), vec3f(0.12, 0.34, 0.26));
        }
    }
    return l;
}

// ------------------------------------------------------------------ canyon

const RIM: f32 = 130.0;      // plateau height
const WATER_Y: f32 = 1.5;    // river surface
const AMP: f32 = 5.0;        // wall displacement amplitude (the cheap bound subtracts it)

// the canyon centre line: x at distance z
fn cx_at(z: f32) -> f32 {
    return 30.0 * sin(z * 0.0085) + 9.0 * sin(z * 0.027 + 1.7);
}

// half width of the slot at height y: gravel floor, 45-degree scree to 30 m,
// then a sheer face flaring a little toward the rim
fn half_w(y: f32) -> f32 {
    return 20.0 + 0.9 * clamp(y, 0.0, 30.0) + 0.10 * max(y - 30.0, 0.0);
}

// floor height across the slot: a shallow trough holding the river
fn floor_h(dx: f32) -> f32 {
    return 0.35 * max(abs(dx) - 10.0, 0.0);
}

// horizontal bedding: stratum index at height y (beds ~8 m, slightly uneven)
fn stratum(y: f32) -> f32 {
    return floor(y * 0.125 + 0.5 * sin(y * 0.041));
}

// Distance to rock (positive in air) and material: 1 wall, 2 floor, 3
// plateau; 0 marks a conservative bound, never a surface. The slot air is
// min(wall, floor); the air above the plateau joins it with max. Slopes in
// the profile, the meander and the noise are paid for by the 0.62 factor.
fn map(p: vec3f, ctx: Ctx) -> vec2f {
    let top = RIM + 5.0 * noise_grad2(p.xz * 0.011 + vec2f(3.0, 1.0));
    let d_top = p.y - top;
    if (p.y > RIM + 12.0) { return vec2f(d_top * 0.9, 3.0); }

    let dx = p.x - cx_at(p.z);
    let adx = abs(dx);
    let hw0 = half_w(p.y);
    let slot0 = hw0 - adx;
    let d_fl0 = p.y - floor_h(dx);
    // well away from both walls and the floor nothing detailed can matter
    let bound = min(slot0 - AMP - 3.0, d_fl0 - 1.0) * 0.66;
    if (bound > 8.0 && p.y < RIM - 2.0) { return vec2f(bound, 0.0); }

    // the face: harder beds stand out as ledges (each bed a smooth bump that
    // returns to the base at its boundaries, so the field stays continuous),
    // a coarse displacement adds buttresses and alcoves; the same noise
    // roughens the floor
    let side = sign(dx);
    let sy = p.y * 0.125 + 0.5 * sin(p.y * 0.041);
    let s = floor(sy);
    let f = sy - s;
    let bump = sstep(0.0, 0.25, f) * (1.0 - sstep(0.75, 1.0, f));
    let hs = hash_cell2(vec2i(i32(s), i32(side)), 11u);
    let ledge = (hs.x - 0.5) * 2.0 * bump * sstep(24.0, 34.0, p.y);
    let nz = noise_fbm2(vec2f(p.z * 0.02 + side * 37.0, p.y * 0.035), 3);
    let hw = hw0 + ledge + AMP * 2.0 * (nz - 0.5);
    let d_slot = (hw - adx) * 0.6;
    let d_fl = (d_fl0 - 0.6 * (nz - 0.5)) * 0.9;

    var res = vec2f(d_slot, 1.0);
    if (d_fl < d_slot) { res = vec2f(d_fl, 2.0); }
    if (d_top * 0.9 > res.x) { res = vec2f(d_top * 0.9, 3.0); }
    return res;
}

// ------------------------------------------------------------------ surfaces

// colour bands by height: red at the base, orange, cream, purple-brown at the rim
fn strata_col(y: f32) -> vec3f {
    var c = mix(col_hex(0x8e3b22u), col_hex(0xc9702eu), sstep(30.0, 46.0, y));
    c = mix(c, col_hex(0xd8b78au), sstep(76.0, 90.0, y));
    c = mix(c, col_hex(0x6a4045u), sstep(106.0, 120.0, y));
    return c;
}

fn rock_albedo(pos: vec3f, n: vec3f, mat: f32, dx: f32) -> vec3f {
    if (mat > 2.5) {
        // plateau: pale slickrock with sparse dark scrub
        let sc = noise_fbm2(pos.xz * 0.05 + vec2f(9.0, 2.0), 3);
        let scrub = sstep(0.60, 0.70, sc);
        return mix(col_hex(0xb9946cu), col_hex(0x3a4a2cu), scrub * 0.7);
    }
    if (mat > 1.5) {
        // gravel bars, wet and dark beside the water
        let g = noise_fbm2(pos.xz * 0.15, 3);
        let wet = 1.0 - sstep(WATER_Y, WATER_Y + 1.2, pos.y);
        return col_hex(0x8c7a62u) * (0.75 + 0.5 * g) * (1.0 - 0.45 * wet);
    }
    // the wall: bands by height, alternating light and dark beds, desert
    // varnish streaking down from the rim, dusty scree at the base
    let s = stratum(pos.y);
    let side = sign(dx);
    let hs = hash_cell2(vec2i(i32(s), i32(side)), 11u);
    var c = strata_col(pos.y) * (0.80 + 0.40 * hs.y);
    let vz = noise_fbm2(vec2f(pos.z * 0.04 + side * 71.0, pos.y * 0.004), 3);
    let varnish = sstep(0.52, 0.72, vz) * sstep(20.0, 60.0, pos.y);
    c = mix(c, col_hex(0x3c2a26u), varnish * 0.6);
    let under = sstep(-0.1, 0.5, -n.y);          // undersides of ledges
    c *= 1.0 - 0.35 * under;
    let scree = 1.0 - sstep(22.0, 34.0, pos.y);
    return mix(c, col_hex(0x9a6d4eu), scree * 0.55);
}

fn shade_rock(pos: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = rm_normal(pos, t, ctx);
    let dx = pos.x - cx_at(pos.z);
    let alb = rock_albedo(pos, n, mat, dx);
    let ndl = dot(n, l.sun);
    // the plateau is always in the sun; walls and floor ask the other wall
    var sh = select(0.0, 1.0, mat > 2.5);
    if (ndl > 0.02 && mat < 2.5) {
        sh = rm_shadow(pos + n * (0.4 + 0.003 * t), l.sun, 0.5, 320.0, 10.0, ctx);
    }
    // sky light: the deeper in the slot, the less sky a point sees
    let depthk = saturate(pos.y / RIM);
    let skyv = (0.30 + 0.70 * depthk) * (0.62 + 0.38 * n.y);
    // warm bounce off the sunlit wall onto the shadow wall and the floor
    let sx = sign(l.sun.x + 1e-4);
    let facing = saturate(-n.x * sx) + 0.5 * saturate(n.y);
    let bounce = l.sun_c * vec3f(0.6, 0.36, 0.2) * 0.15 * facing * saturate(l.sun.y * 1.6) * (1.0 - 0.5 * depthk);
    return alb * (l.sun_c * max(ndl, 0.0) * sh + l.amb * skyv + bounce);
}

fn shade_water(pw: vec3f, rd: vec3f, tw: f32, l: Look, t: f32, ctx: Ctx) -> vec3f {
    let dx = pw.x - cx_at(pw.z);
    // the current runs down-canyon (toward -z): drift the ripples with it
    let flow = vec2f(pw.x, pw.z + t * 2.5);
    let n = water_normal(flow, t, 0.12, tw, ctx);
    let rr = reflect(rd, n);
    let sh = rm_shadow(pw + vec3f(0.0, 0.6 + 0.003 * tw, 0.0), l.sun, 0.5, 320.0, 10.0, ctx);
    let lightf = l.amb * 2.2 + l.sun_c * 0.25 * sh;
    // deep in the slot the surface mostly mirrors the walls; only steep
    // reflections reach the sky
    let wall_refl = strata_col(38.0) * (l.amb * 0.9 + l.sun_c * 0.12 * saturate(l.sun.y * 1.5));
    let refl = mix(wall_refl, sky_fast(rr, l.sun), sstep(0.30, 0.70, rr.y));
    let depthk = saturate((WATER_Y - floor_h(dx)) / 1.4);
    var col = water_color(rd, n, refl, l.deep * lightf, l.shallow * lightf, depthk);
    // riffles: white water over the shallows, a few streaks mid-stream
    let shallow_k = 1.0 - depthk;
    let rif = noise_fbm2(vec2f(dx * 0.22, (pw.z + t * 2.5) * 0.05), 3);
    let foam = sstep(0.64 - 0.24 * shallow_k, 0.76 - 0.24 * shallow_k, rif);
    let foamc = vec3f(0.85) * (l.amb * 1.4 + l.sun_c * 0.5 * sh);
    col = mix(col, foamc, foam * 0.85);
    col += l.sun_c * 1.5 * sh * pow(max(dot(rr, l.sun), 0.0), 180.0);
    return col;
}

// ------------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let k = ctx.params;
    let spd = 12.0 * k.x;                       // 6 m/s as designed, 0 = hovering
    let hazek = 2.0 * k.y;                      // haze density multiplier
    let fogd = l.fogd * hazek;
    let mistd = l.mist * hazek;

    // the drone: mid-height in the slot, following the centre line, looking
    // a little down at the next bend and banking slightly into it
    let zc = t * spd;
    let ro = vec3f(cx_at(zc) + 6.0 * sin(t * 0.13), 56.0 + 2.5 * sin(t * 0.21), zc);
    let za = zc + 130.0;
    let at = vec3f(cx_at(za), 44.0, za);
    let bank = (cx_at(zc + 40.0) - cx_at(zc - 40.0)) / 80.0;
    let cam = cam_look_at(ro, at, -0.16 * bank, 62.0);
    let rd = cam_ray(cam, p);

    // haze colour along this ray: a touch brighter looking up, glowing
    // toward the sun
    let mu = max(dot(rd, l.sun), 0.0);
    let hc = l.haze_c * (0.8 + 0.2 * saturate(rd.y * 3.0 + 0.5)) + l.sun_c * l.glow * 0.12 * pow(mu, 6.0);

    let hit = rm_march(ro, rd, 0.5, 1200.0, steps(96.0, ctx), ctx);
    let tw = water_intersect(ro, rd, WATER_Y);
    var col: vec3f;
    if (hit.x > 0.0 && (tw < 0.0 || hit.x < tw)) {
        let pos = ro + rd * hit.x;
        col = shade_rock(pos, hit.x, hit.y, l, ctx);
        col = fog_exp(col, hc, hit.x, fogd);
        col = fog_height(col, l.mist_c, ro, rd, hit.x, mistd, 0.09);
    } else if (tw > 0.0) {
        let pw = ro + rd * tw;
        col = shade_water(pw, rd, tw, l, t, ctx);
        col = fog_exp(col, hc, tw, fogd);
        col = fog_height(col, l.mist_c, ro, rd, tw, mistd, 0.09);
    } else {
        // the sky band above the rims; near the horizon it is seen through
        // the whole length of the canyon's haze
        col = sky_atmosphere_haze(rd, l.sun, l.sky_haze) + min(sky_sun_disk(rd, l.sun, 0.53), vec3f(40.0));
        let far = 1400.0 * (1.0 - sstep(0.0, 0.25, rd.y));
        col = fog_exp(col, hc, far, fogd);
    }
    return max(col * l.exposure, vec3f(0.0));
}
