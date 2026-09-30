//! name: atoll
//! title: Atoll from Above
//! category: aerial
//! tags: drone, atoll, reef, lagoon, island, tropical, aerial
//! desc: a coral atoll from high above: surf on the ring reef, turquoise lagoon, palm islets
//! themes: noon, sunset, storm
//! params: surf, clouds
//! uses: camera, sky, water, noise, fog, clouds
//! cost: medium
//! fallback: ocean
//! credits: original

// World units are metres, y up. The drone hangs ~1000 m up looking down -z
// about 33 degrees below level, so the horizon sits near the top of a 16:9
// frame and the ring (radius ~850 m, centred 1.55 km ahead) fills the middle.
// The sea is one flat plane; everything on it is a depth map written in the
// ring's polar coordinates and coloured by Beer-Lambert absorption over coral
// sand plus a body-scatter term, so depth alone yields the palette: white
// surf on the crest, aquamarine flat, turquoise-to-teal lagoon, indigo ocean.

const CAM_H: f32 = 1000.0;
const CTR: vec2f = vec2f(0.0, -1550.0);
const R0: f32 = 850.0;
const SIG: vec3f = vec3f(0.42, 0.072, 0.034);   // absorption per metre, clear tropical water
const CLOUD_H: f32 = 2300.0;                     // cloud sheet height above the camera

struct Look {
    sun: vec3f,
    sun_c: vec3f,
    amb: vec3f,
    sand: vec3f,        // coral-sand floor
    deep: vec3f,        // open-ocean body colour
    lag: vec3f,         // lagoon body colour
    veg: vec3f,         // islet canopy
    cloud_cov: f32,
    cloud_lit: vec3f,
    cloud_dark: vec3f,
    overcast: f32,      // 0 = clear-sky model, 1 = flat grey lid
    haze: f32,          // fog density per metre
    glitter: f32,
    shin: f32,
    surf: f32,          // theme multiplier on the breaking surf
    rain: f32,
    sat: f32,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {  // sunset: sun low ahead, glitter path, lagoon toward teal-purple
            l.sun = sky_sun_dir(5.0, 12.0);
            l.sun_c = sky_sun_light(l.sun);
            l.amb = sky_ambient(l.sun) * 1.2 + vec3f(0.02, 0.01, 0.035);
            l.sand = vec3f(0.9, 0.78, 0.66);
            l.deep = vec3f(0.012, 0.014, 0.06);
            l.lag = vec3f(0.03, 0.1, 0.16);
            l.veg = vec3f(0.06, 0.11, 0.05);
            l.cloud_cov = 0.42;
            l.cloud_lit = vec3f(1.0, 0.5, 0.3);
            l.cloud_dark = vec3f(0.22, 0.14, 0.24);
            l.overcast = 0.0;
            l.haze = 0.00016;
            l.glitter = 1.6; l.shin = 70.0;
            l.surf = 1.0; l.rain = 0.0; l.sat = 1.1; l.exposure = 0.4;
        }
        case 2u: {  // storm: grey sea, heavy surf, low dark lid, rain veils
            l.sun = sky_sun_dir(30.0, 200.0);
            l.sun_c = sky_sun_light(l.sun) * 0.12;
            let a = sky_ambient(l.sun);
            l.amb = vec3f(col_luma(a)) * vec3f(0.9, 0.95, 1.0) * 2.2;
            l.sand = vec3f(0.5, 0.5, 0.46);
            l.deep = vec3f(0.035, 0.045, 0.052);
            l.lag = vec3f(0.05, 0.1, 0.11);
            l.veg = vec3f(0.05, 0.08, 0.04);
            l.cloud_cov = 0.96;
            l.cloud_lit = vec3f(0.42, 0.44, 0.47);
            l.cloud_dark = vec3f(0.12, 0.13, 0.15);
            l.overcast = 0.9;
            l.haze = 0.00032;
            l.glitter = 0.0; l.shin = 20.0;
            l.surf = 1.8; l.rain = 1.0; l.sat = 0.75; l.exposure = 0.6;
        }
        default: {  // noon: sun high behind the camera, full saturation, puffy clouds
            l.sun = sky_sun_dir(66.0, 165.0);
            l.sun_c = sky_sun_light(l.sun);
            l.amb = sky_ambient(l.sun);
            l.sand = vec3f(0.93, 0.89, 0.76);
            l.deep = vec3f(0.006, 0.022, 0.085);
            l.lag = vec3f(0.02, 0.16, 0.22);
            l.veg = vec3f(0.07, 0.19, 0.06);
            l.cloud_cov = 0.36;
            l.cloud_lit = vec3f(1.0, 1.0, 1.0);
            l.cloud_dark = vec3f(0.55, 0.6, 0.7);
            l.overcast = 0.0;
            l.haze = 0.00008;
            l.glitter = 0.6; l.shin = 300.0;
            l.surf = 1.0; l.rain = 0.0; l.sat = 1.15; l.exposure = -0.7;
        }
    }
    return l;
}

// ------------------------------------------------------------ the sky

fn firmament(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    var c = sky_atmosphere(rd, l.sun, ctx);
    // the model's horizon glow runs tan under a high sun: cool it toward white-blue
    let high = saturate(l.sun.y * 3.0 - 0.5);
    c = mix(c, col_saturation(c, 0.55) * vec3f(0.92, 0.97, 1.08), high * sstep(0.25, 0.0, rd.y));
    let lid = vec3f(col_luma(c)) * vec3f(0.85, 0.9, 1.0) * (0.45 + 0.25 * saturate(rd.y * 3.0));
    return mix(c, lid, l.overcast);
}

fn overhead(ro: vec3f, rd: vec3f, sky: vec3f, l: Look, cov: f32, zen: f32, ctx: Ctx) -> vec3f {
    if (rd.y <= 0.003) { return sky; }
    let q = (ro.xz + rd.xz * (CLOUD_H / rd.y)) * 0.00028 + ctx.seedf * 3.0;
    let cs = cloud_sheet(q, cov, ctx);
    let fade = sstep(0.0, 0.14, rd.y);                       // melt into the haze at the horizon
    let cc = mix(l.cloud_dark, l.cloud_lit, cs.y) * zen * 3.2;
    return mix(sky, cc, cs.x * fade);
}

// ------------------------------------------------------------ the atoll

struct Sea {
    depth: f32,     // metres of water
    d: f32,         // signed distance to the reef crest, positive outside
    inside: f32,    // 0 ocean .. 1 lagoon
    foam: f32,
    land: f32,      // soft field, > 0 on an islet
    coral: f32,
}

fn ang_diff(a: f32, b: f32) -> f32 {
    let d = a - b;
    return d - TAU * floor((d + PI) / TAU);
}

// ring radius by angle: continuous around the circle (noise sampled on the unit circle)
fn ring_r(ang: f32, ctx: Ctx) -> f32 {
    let c = vec2f(cos(ang), sin(ang));
    let s = ctx.seedf * 5.0;
    let n = noise_value2(c * 1.3 + s) - 0.5;
    let n2 = noise_value2(c * 2.9 + s.yx + 4.0) - 0.5;
    return R0 * (1.0 + 0.1 * cos(2.0 * ang + 0.8 + s.x) + 0.3 * n + 0.1 * n2);
}

// an islet: an ellipse along the ring (along, d) with a noisy coastline
fn islet(along: f32, d: f32, len: f32, wid: f32, coast: f32) -> f32 {
    return 1.0 - sq(along / len) - sq((d + 45.0) / wid) + coast;
}

fn sea_at(xz: vec2f, t: f32, surf: f32, ctx: Ctx) -> Sea {
    var s: Sea;
    let q = xz - CTR;
    let r = length(q);
    let ang = atan2(q.y, q.x);
    let rr = ring_r(ang, ctx);
    let d = r - rr;
    s.d = d;
    s.inside = sstep(40.0, -60.0, d);

    // depth: a shallow flat behind the crest, a bowl inside, a steep drop-off outside
    let bowl = pow(sstep(-140.0, -620.0, d), 1.3);
    let dep_in = mix(1.6, 24.0, bowl) * (0.85 + 0.3 * noise_fbm2(xz * 0.0025 + 3.0, 3));
    let dep_out = 1.2 + 6.0 * sstep(0.0, 70.0, d) + 2500.0 * sq(sstep(40.0, 420.0, d));
    var depth = mix(dep_out, dep_in, sstep(10.0, -20.0, d));
    depth = mix(depth, 0.5, exp(-sq(d / 18.0)));           // the crest itself nearly awash

    // coral heads: worley blotches in the lagoon, patchy
    let w = noise_worley2(xz * 0.011 + ctx.seedf * 3.0);
    s.coral = sstep(0.5, 0.15, w.x) * sstep(-80.0, -220.0, d) * sstep(0.38, 0.6, noise_fbm2(xz * 0.0018 + 9.0, 3));

    // islets on the ring: two or three, laid out per launch
    let sf = hash_seeded2(vec2i(3, 1), 17u, ctx);
    let coast = (noise_fbm2(xz * 0.004, 3) - 0.5) * 0.9;
    let a0 = 0.5 + sf.x * 1.2;
    let a1 = a0 + 2.1 + sf.y * 0.8;
    let a2 = a1 + 2.0 + sf.z * 0.6;
    var land = islet(ang_diff(ang, a0) * rr, d, 420.0, 80.0, coast);
    land = max(land, islet(ang_diff(ang, a1) * rr, d, 260.0, 65.0, coast));
    land = max(land, islet(ang_diff(ang, a2) * rr, d, 190.0, 55.0, coast) - sf.w * 0.7);
    s.land = land;
    depth = mix(depth, 0.6, sstep(-0.3, -0.02, land));    // bright sand shallows off the beaches
    s.depth = depth;

    // breaking surf on the outer crest: bands pulsing outward, broken up along
    // the ring by drifting noise, heavier on the windward side
    let env = exp(-sq((d - 22.0) / (24.0 + 32.0 * surf)));
    let bands = 0.55 + 0.45 * sin(d * 0.16 - t * 0.9 + ang * 3.0);
    let brk = noise_fbm2(xz * 0.009 + t * 0.03, 3);
    let wind = 0.75 + 0.45 * cos(ang - 2.4);
    var foam = env * (0.35 + 0.65 * bands) * (0.4 + 1.1 * brk) * wind;
    // back-reef wash fading into the flat
    foam += 0.3 * exp(-sq((d + 55.0) / 70.0)) * sstep(0.45, 0.75, brk);
    // faint swell crests approaching the reef: phase-warped so they wobble
    // rather than draw contour rings, and broken into segments
    let crest = sstep(0.75, 1.0, sin(d * 0.05 - t * 0.5 + 2.5 * brk));
    foam += 0.06 * crest * sstep(0.4, 0.7, noise_value2(xz * 0.006 + 40.0)) * exp(-d / 260.0) * sstep(30.0, 140.0, d);
    // a lace of wash just off the beaches
    foam += 0.45 * exp(-sq((land + 0.07) / 0.05));
    s.foam = saturate(foam * (0.45 + 0.55 * surf));
    return s;
}

// ------------------------------------------------------------ shading

fn shade_sea(s: Sea, xz: vec2f, rd: vec3f, dist: f32, shade: f32, l: Look, t: f32, ctx: Ctx) -> vec3f {
    // irradiance on a horizontal surface, under drifting cloud shadow
    let e = l.sun_c * max(l.sun.y, 0.0) * shade + l.amb;

    // water: floor light through the column plus body scatter
    let trans = exp(-SIG * s.depth * 2.0);
    let body_c = mix(l.deep, l.lag, s.inside);
    let scatter = body_c * (1.0 - exp(-s.depth * 0.09));
    var floor_c = l.sand * (0.9 + 0.2 * noise_value2(xz * 0.03));
    floor_c = mix(floor_c, vec3f(0.12, 0.1, 0.05), s.coral * 0.85);
    var col = (floor_c * trans + scatter) * e;

    // islets: dry sand beach, then canopy with palm dots and their shadows
    let land_m = sstep(-0.01, 0.02, s.land);
    let veg = sstep(0.14, 0.34, s.land);
    let fp = dist * ctx.px * 1.7 / max(-rd.y, 0.15);          // ground footprint of a pixel, metres
    let aa = max(fp * 0.7, 1.0);
    let cell = floor(xz / 13.0);
    let h = hash_cell2(vec2i(cell), 5u);
    let pc = (cell + 0.2 + 0.6 * h.xy) * 13.0;
    let pr = 4.5;
    let lod = sq(pr / (pr + aa));
    let alive = sstep(0.1, 0.25, h.z);
    let palm = sstep(pr + aa, pr - aa * 0.5, length(xz - pc)) * lod * alive;
    let psh = sstep(pr + aa, pr - aa * 0.5, length(xz - pc + l.sun.xz * 7.0)) * lod * alive;
    var canopy = l.veg * (0.8 + 0.4 * noise_value2(xz * 0.02));
    canopy = mix(canopy, canopy * 0.45, psh);
    canopy = mix(canopy, l.veg * 2.0 + vec3f(0.02, 0.08, 0.0), palm);
    let land_c = mix(l.sand * 1.05, canopy, veg) * e;
    col = mix(col, land_c, land_m);

    // surf foam
    let foam = s.foam * (1.0 - land_m);
    col = mix(col, vec3f(0.95, 0.96, 0.97) * e * 0.9, foam);

    // long swells: only the gentle slopes read from this height; far water flattens
    let nw = water_normal(xz * 0.035, t * 0.5, 0.6, dist * 0.035, ctx);
    var n = normalize(vec3f(nw.x, nw.y * 8.0, nw.z));
    n = normalize(mix(n, vec3f(0.0, 1.0, 0.0), sstep(1500.0, 4000.0, dist)));
    let fres = 0.02 + 0.98 * pow(1.0 - saturate(dot(-rd, n)), 5.0);
    let rr = reflect(rd, n);
    let skyref = firmament(normalize(vec3f(rr.x, max(rr.y, 0.02), rr.z)), l, ctx);
    let wet = (1.0 - land_m) * (1.0 - foam * 0.8);
    col = mix(col, skyref, fres * wet);
    let spec = pow(saturate(dot(rr, l.sun)), l.shin) * l.sun_c * l.glitter * (0.05 + fres) * wet * shade;
    return col + spec;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let k = ctx.params;
    let surf = (0.4 + 1.2 * k.x) * l.surf;                    // 1.0 at the default slider
    let cov = saturate(l.cloud_cov * (0.3 + 1.4 * k.y));

    // the drone only drifts
    let sway = cam_sway(t, 14.0, ctx.seed.x);
    let ro = vec3f(0.0, CAM_H, 0.0) + sway;
    let at = vec3f(0.0, 0.0, -CAM_H / tan(radians(33.0))) + sway * 0.6;
    let cam = cam_look_at(ro, at, 0.0, 78.0);
    let rd = cam_ray(cam, p);

    let zen = col_luma(sky_atmosphere(vec3f(0.0, 1.0, 0.0), l.sun, ctx));
    let hz = firmament(normalize(vec3f(rd.x, 0.012, rd.z)), l, ctx);

    let tw = water_intersect(ro, rd, 0.0);
    var col: vec3f;
    if (tw < 0.0) {
        col = overhead(ro, rd, firmament(rd, l, ctx), l, cov, zen, ctx);
        col += sky_sun_disk(rd, l.sun, 0.5) * (1.0 - l.overcast) * 0.5;
    } else {
        let dist = min(tw, 2.0e5);
        let xz = ro.xz + rd.xz * dist;
        // cloud shadows drifting over the water
        let cs = noise_fbm2(xz * 0.00075 + vec2f(t * 0.0055, t * 0.002) + ctx.seedf, 3);
        let thr = 1.0 - cov * 0.75;
        let shade = 1.0 - 0.7 * sstep(thr - 0.06, thr + 0.06, cs);
        let s = sea_at(xz, t, surf, ctx);
        col = shade_sea(s, xz, rd, dist, shade, l, t, ctx);
        col = fog_exp(col, hz, dist, l.haze);
    }

    // rain veils hanging from the lid near the horizon (storm)
    if (l.rain > 0.0) {
        let vn = noise_fbm2(vec2f(rd.x * 5.0 + t * 0.012, rd.y * 1.5 + rd.x), 3);
        let band = sstep(0.28, 0.06, rd.y) * sstep(-0.16, -0.05, rd.y);
        col = mix(col, hz, sstep(0.35, 0.75, vn) * band * 0.55 * l.rain);
    }

    col = col_saturation(col, l.sat) * exp2(l.exposure);
    return max(col, vec3f(0.0));
}
