//! name: earthrise
//! title: Earthrise
//! category: space
//! tags: earth, moon, earthrise, apollo, craters, eclipse, space
//! desc: the Earth hanging over the lunar horizon, craters stretching long shadows below
//! themes: apollo, crescent, eclipse
//! uses: camera, noise, stars
//! cost: medium
//! fallback: starfield
//! credits: original; after the Apollo 8 photograph

// Units are the Moon's radius; the Moon sits at the origin. The camera is
// in lunar orbit, about a hundred kilometres up, looking along the horizon
// through a long lens, so the Earth (really two degrees across) fills a
// good part of the frame, as in the photograph. The lunar surface is the
// sphere itself, dressed in craters: a few sizes of bowls on a jittered
// grid, each with a raised rim, shaded for the low sun and given the
// crescent of shadow its own rim throws into the bowl. The Earth is a
// sphere of oceans, land and weather with a thin blue limb; in the eclipse
// the Sun is behind it and it is a black disc ringed with every sunset on
// the planet, the only light on the grey ground below.

const ALT: f32 = 0.06;          // orbit height, Moon radii (~100 km)
const EARTH_DIST: f32 = 221.0;  // Earth's distance, Moon radii
const EARTH_R: f32 = 3.67;      // Earth's radius, Moon radii

struct Look {
    mode: u32,
    sun: vec3f,        // direction toward the sun (world)
    earth_rot: f32,    // which face of the Earth is turned to us
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            // the Sun low behind and beside the Earth: a crescent, long shadows
            return Look(1u, normalize(vec3f(-0.82, 0.12, -0.56)), 2.1, 0.2);
        }
        case 2u: {
            // the Earth eclipsing the Sun
            return Look(2u, normalize(vec3f(0.0, 0.3, -1.0)), 0.8, 1.2);
        }
        default: {
            // Apollo 8: the Earth half lit, the sun low on the right
            return Look(0u, normalize(vec3f(0.97, 0.14, -0.2)), 0.2, 0.0);
        }
    }
}

// the camera, where the Earth hangs, and the sun, shared by every pane
struct Setup {
    ro: vec3f,
    fw: vec3f,
    earth: vec3f,
    sun: vec3f,
}

fn setup(l: Look) -> Setup {
    let ro = vec3f(0.0, 1.0 + ALT, 0.0);
    // the horizon dips below level by acos(1 / (1 + ALT)); look just above it
    let dip = acos(1.0 / (1.0 + ALT));
    let pitch = -dip + radians(1.3);
    let fw = normalize(vec3f(0.0, sin(pitch), -cos(pitch)));
    // the Earth a little above the horizon, a touch left of centre
    let ed = normalize(vec3f(-0.01, sin(-dip + radians(2.05)), -cos(-dip + radians(2.05))));
    let earth = ro + ed * EARTH_DIST;
    // eclipse: the Sun exactly behind the Earth
    let sun = select(l.sun, ed, l.mode == 2u);
    return Setup(ro, fw, earth, sun);
}

// a sparse sky of point stars on the frame itself (the long lens magnifies
// a sky model's stars into blocks)
fn frame_stars(p: vec2f, density: f32, ctx: Ctx) -> vec3f {
    let g = p / 0.022;
    let c = floor(g);
    let h = hash_cell2(vec2i(c), 0x57a45u);
    if (h.w > density) {
        return vec3f(0.0);
    }
    let d = length((g - c - 0.15 - 0.7 * h.xy) * 0.022);
    let b = pow(h.z, 3.0);
    return mix(vec3f(0.8, 0.85, 1.0), vec3f(1.0, 0.9, 0.75), h.x) * b * sstep(ctx.px * 1.2, 0.0, d);
}

// ---------------------------------------------------------------- the Moon
// craters on the tangent-plane coordinates `q` (Moon radii): (shade
// change, shadow, rim) — the bowl darkens away from the sun, the rim's
// shadow falls across the bowl, and the rim itself catches the light
fn craters(q: vec2f, sun2: vec2f, sun_el: f32, ctx: Ctx) -> vec3f {
    var shade = 0.0;
    var shadow = 0.0;
    var rim = 0.0;
    // the sun's height sets how far each rim's shadow reaches into its bowl
    let reach = clamp(0.18 / max(tan(sun_el), 0.05), 0.15, 1.3);
    for (var o = 0; o < 3; o++) {
        let scale = 0.012 * pow(0.42, f32(o));
        let g = q / scale;
        let cell = floor(g);
        for (var j = -1; j <= 1; j++) {
            for (var i = -1; i <= 1; i++) {
                let c = cell + vec2f(f32(i), f32(j));
                let h = hash_cell2(vec2i(c), 0x0c7a7e0u + u32(o));
                if (h.w > 0.32) { continue; }
                let r = 0.18 + 0.3 * h.z;
                let center = c + 0.2 + 0.6 * h.xy;
                let d = g - center;
                let dist = length(d) / r;
                if (dist > 1.35) { continue; }
                // the bowl: its far wall (away from the sun) faces the light,
                // the wall nearest the sun is turned away
                let into = sstep(1.0, 0.4, dist);
                let side = dot(normalize(d + vec2f(1e-4)), sun2);
                shade += into * side * 0.7;
                // the rim: a raised ring, bright on its far side
                rim += exp(-sq((dist - 1.0) / 0.09)) * max(side, 0.0) * 0.9;
                // the crescent of shadow cast by the rim nearest the sun,
                // softening toward its edge
                let up = center - sun2 * r;
                let along = dot(g - up, -sun2) / r;
                shadow = max(shadow, sstep(1.0, 0.9, dist) * sstep(reach + 0.15, reach - 0.1, along) * 0.85);
            }
        }
    }
    return vec3f(shade, saturate(shadow), rim);
}

fn moon_surface(h: vec3f, rd: vec3f, l: Look, s: Setup, sun_col: vec3f, ctx: Ctx) -> vec3f {
    let n = normalize(h);
    // tangent frame at the sub-camera point: east and north
    let e = vec3f(1.0, 0.0, 0.0);
    let nn = vec3f(0.0, 0.0, -1.0);
    let q = vec2f(dot(h, e), dot(h, nn));
    let sun = s.sun;
    let sun2 = normalize(vec2f(dot(sun, e), dot(sun, nn)) + vec2f(1e-4));
    let sun_el = asin(clamp(dot(sun, n), -1.0, 1.0));
    let cr = craters(q, sun2, sun_el, ctx);
    // regolith: grey, a little mottled, maria darker
    let mare = sstep(0.45, 0.62, noise_fbm2(q * 9.0 + 3.0, 4));
    var alb = mix(vec3f(0.42, 0.41, 0.4), vec3f(0.25, 0.25, 0.26), mare);
    alb *= 0.85 + 0.3 * noise_fbm2(q * 140.0, 3);
    // lighting: the low sun, raked across the craters
    let ndl = saturate(dot(n, sun));
    var lit = ndl * (1.0 + cr.x + cr.z * 0.8) * (1.0 - 0.85 * cr.y);
    // the ground rolls a little: gentle hummocks
    let roll = noise_fbm2(q * 30.0, 4) - 0.5;
    lit *= saturate(1.0 + roll * 0.9);
    var c = alb * sun_col * max(lit, 0.0);
    // earthshine on the unlit ground (faint, blue)
    c += alb * vec3f(0.3, 0.38, 0.55) * 0.012;
    return c;
}

// ---------------------------------------------------------------- the Earth
fn earth_colour(n: vec3f, l: Look, t: f32) -> vec3f {
    // turn the planet slowly
    let a = l.earth_rot + t * 0.004;
    let rn = vec3f(n.x * cos(a) - n.z * sin(a), n.y, n.x * sin(a) + n.z * cos(a));
    let lat = asin(clamp(rn.y, -1.0, 1.0));
    // land: noise continents, desert toward the tropics, green elsewhere
    let land = noise_fbm3(rn * 2.2 + vec3f(1.7, 0.0, 3.1), 5);
    let is_land = sstep(0.52, 0.56, land);
    let arid = sstep(0.1, 0.45, 1.0 - abs(abs(lat) - 0.4) * 3.0) * noise_fbm3(rn * 5.0, 3);
    var c = mix(vec3f(0.01, 0.04, 0.12), vec3f(0.02, 0.07, 0.18), noise_fbm3(rn * 6.0, 3));
    let ground = mix(vec3f(0.08, 0.13, 0.05), vec3f(0.36, 0.28, 0.17), arid);
    c = mix(c, ground, is_land);
    // ice at the poles
    c = mix(c, vec3f(0.85, 0.88, 0.92), sstep(1.15, 1.3, abs(lat)));
    // weather: swirled cloud bands, drifting
    let wq = rn * 3.3 + vec3f(t * 0.003, 0.0, 0.0);
    let w = noise_fbm3(wq + vec3f(noise_fbm3(wq * 1.7, 3) * 1.6), 5);
    let belts = 0.6 + 0.4 * cos(lat * 6.0);
    let cloud = sstep(0.5, 0.72, w * belts + 0.1);
    c = mix(c, vec3f(0.95), cloud);
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let s = setup(l);
    // a long lens: a few degrees across the short side
    let cam = cam_look_at(s.ro, s.ro + s.fw, radians(2.5), 6.0);
    let rd = cam_ray(cam, p);
    let sun_col = select(vec3f(1.0, 0.98, 0.95) * 1.6, vec3f(0.0), l.mode == 2u);
    let lsun = s.sun;

    // black sky: few stars beside the sunlit ground, many in the eclipse
    var col = frame_stars(p, select(0.025, 0.12, l.mode == 2u), ctx) * select(0.25, 0.5, l.mode == 2u);

    // the Earth
    let oc = s.ro - s.earth;
    let b = dot(oc, rd);
    let cq = dot(oc, oc) - EARTH_R * EARTH_R;
    let disc = b * b - cq;
    // how close the ray passes the Earth, in Earth radii from its centre
    let tc = -b;
    let miss = length(oc + rd * tc) / EARTH_R;
    if (disc > 0.0) {
        let te = -b - sqrt(disc);
        let hp = s.ro + rd * te;
        let n = normalize(hp - s.earth);
        let alb = earth_colour(n, l, ctx.t);
        let ndl = dot(n, lsun);
        let ndv = saturate(dot(n, -rd));
        var ec = alb * saturate(ndl) * 1.3;
        // oceans glint where the sun reflects
        let hv = normalize(lsun - rd);
        ec += vec3f(1.0, 0.95, 0.85) * pow(saturate(dot(n, hv)), 60.0) * saturate(ndl) * 0.25 * (1.0 - sstep(0.0, 0.4, col_luma(alb) - 0.1));
        // a blue veil of air, thicker toward the limb
        ec += vec3f(0.25, 0.45, 1.0) * pow(1.0 - ndv, 3.0) * saturate(ndl + 0.1) * 0.5;
        if (l.mode == 2u) {
            // the night side faces us: black, but for lightning and cities
            ec = alb * 0.002 + vec3f(1.0, 0.7, 0.35) * sstep(0.6, 0.8, noise_fbm3(n * 14.0, 3)) * sstep(0.54, 0.6, noise_fbm3(n * 2.2 + vec3f(1.7, 0.0, 3.1), 5)) * 0.01;
        }
        col = ec;
    }
    // the atmosphere's thin rim, and in the eclipse the ring of sunsets
    let above = miss - 1.0;
    if (above > -0.02) {
        let rim = exp(-max(above, 0.0) / 0.025) * sstep(-0.02, 0.0, above);
        // the limb's outward normal: is the sun on that side?
        let toward = saturate(dot(normalize(oc + rd * tc), lsun) * 0.5 + 0.5);
        if (l.mode == 2u) {
            // refracted sunlight: red nearest the ground, orange, then blue
            let red = exp(-sq(above / 0.014)) * sstep(-0.012, 0.0, above);
            col += vec3f(1.0, 0.12, 0.02) * red * 0.55 + vec3f(1.0, 0.4, 0.1) * exp(-sq((above - 0.016) / 0.012)) * 0.18 + vec3f(0.3, 0.5, 1.0) * rim * 0.05;
            // the Sun's corona, faint, spreading past the ring
            col += vec3f(1.0, 0.9, 0.78) * exp(-max(above, 0.0) / 0.18) * 0.02 * sstep(0.0, 0.03, above);
        } else {
            // only where the sun reaches the air
            col += vec3f(0.3, 0.5, 1.0) * rim * pow(toward, 3.0) * 0.5;
        }
    }

    // the Moon, in front of everything below the horizon
    let bq = dot(s.ro, rd);
    let cm = dot(s.ro, s.ro) - 1.0;
    let dm = bq * bq - cm;
    if (dm > 0.0) {
        let tm = -bq - sqrt(dm);
        if (tm > 0.0) {
            let h = s.ro + rd * tm;
            var light = sun_col;
            if (l.mode == 2u) {
                // the eclipse: only the red ring lights the ground
                light = vec3f(1.0, 0.28, 0.1) * 0.35;
            }
            var mc = moon_surface(h, rd, l, s, light, ctx);
            if (l.mode == 2u) {
                // the ring's light comes from the Earth, low over the horizon
                var s2 = s;
                s2.sun = normalize(s.earth - h);
                mc = moon_surface(h, rd, l, s2, light, ctx);
                // and the whole red sky of the ring, faintly, from everywhere
                mc += vec3f(0.42, 0.1, 0.05) * 0.02 * (0.8 + 0.4 * noise_fbm2(h.xz * 60.0, 3));
            }
            // the horizon is soft: a pixel's worth of blend
            let edge = sstep(0.0, ctx.px * 2.0, sqrt(dm) * 0.1);
            col = mix(col, mc, saturate(edge * 4.0));
        }
    }
    return col * exp2(l.exposure);
}
