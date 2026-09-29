//! name: iss
//! title: Earth from the ISS
//! category: space
//! tags: earth, orbit, iss, city lights, aurora, sunrise
//! desc: the curved limb of the Earth from orbit: city lights, clouds, airglow, an orbital sunrise
//! themes: night, sunrise, day
//! uses: camera, noise, stars, sdf
//! cost: medium
//! fallback: starfield
//! credits: original

// World units are kilometres; the Earth's centre is the origin and the
// station flies 420 km up, looking forward and down at the limb. The ground
// (continents, oceans, a cloud deck, city lights) is procedural on the
// sphere and turns slowly underneath. The air is a single-scattering shell
// (Rayleigh + Mie) marched along every ray; its scale heights are about
// three times the real ones so the thin blue limb survives a terminal's
// resolution. At night a green airglow line rides above the limb and an
// aurora ring glows toward the pole. In the sunrise theme the sun climbs
// over the limb and sets again on a 90 s loop.

struct Look {
    sun_el: f32,      // sun elevation above the limb plane, degrees
    sun_az: f32,
    loop_amp: f32,    // sunrise loop amplitude (degrees)
    lights: f32,      // city lights
    aurora: f32,
    stars: f32,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: { return Look(-21.0, 8.0, 5.0, 1.0, 0.25, 0.22, 0.0); }
        case 2u: { return Look(35.0, 160.0, 0.0, 0.0, 0.0, 0.0, -0.9); }
        default: { return Look(-70.0, 0.0, 0.0, 1.0, 1.0, 0.28, 0.8); }
    }
}

const RE: f32 = 6371.0;
const RA: f32 = 6671.0;          // top of the (exaggerated) air
const HR: f32 = 24.0;            // Rayleigh scale height (x3)
const HM: f32 = 3.6;             // Mie scale height (x3)
const BR: vec3f = vec3f(5.8e-3, 13.5e-3, 33.1e-3) / 3.0;   // per km
const BM: f32 = 21e-3 / 3.0;
const ALT: f32 = 420.0;
const SPIN: f32 = 0.00004;       // how fast the ground turns under us (rad/s)

fn ray_sphere(ro: vec3f, rd: vec3f, r: f32) -> vec2f {
    let b = dot(ro, rd);
    let c = dot(ro, ro) - r * r;
    let d = b * b - c;
    if (d < 0.0) { return vec2f(-1.0); }
    let s = sqrt(d);
    return vec2f(-b - s, -b + s);
}

fn sun_dir(l: Look, t: f32) -> vec3f {
    // elevation measured from the local horizontal at the station; the limb
    // itself sits about 20 degrees below it
    var el = l.sun_el;
    if (l.loop_amp > 0.0) {
        el += l.loop_amp * (0.5 - 0.5 * cos(TAU * t / 90.0)) - l.loop_amp * 0.35;
    }
    let e = radians(el);
    let a = radians(l.sun_az);
    return normalize(vec3f(sin(a) * cos(e), sin(e), -cos(a) * cos(e)));
}

// the ground turns about an axis tilted like an orbit's
fn spin(p: vec3f, t: f32) -> vec3f {
    let axis = normalize(vec3f(1.0, 0.25, 0.1));
    let a = t * SPIN + 0.7;
    let c = cos(a);
    let s = sin(a);
    return p * c + cross(axis, p) * s + axis * dot(axis, p) * (1.0 - c);
}

struct Ground {
    alb: vec3f,
    land: f32,
    city: f32,
    cloud: f32,
}

fn ground(n: vec3f, t: f32, px_km: f32) -> Ground {
    let q = spin(n, t);
    // continents: warped fbm on the sphere
    let w = vec3f(noise_fbm3(q * 4.0 + 3.0, 4), noise_fbm3(q * 4.0 + 9.0, 4), noise_fbm3(q * 4.0 + 17.0, 4));
    let c = noise_fbm3(q * 7.0 + w * 1.6, 7);
    let land = sstep(0.535, 0.555, c);
    let dry = noise_fbm3(q * 12.0 + 5.0, 5);
    let green = vec3f(0.035, 0.06, 0.022);
    let desert = vec3f(0.2, 0.15, 0.08);
    let rock = vec3f(0.08, 0.065, 0.05);
    var lc = mix(green, desert, sstep(0.5, 0.66, dry));
    lc = mix(lc, rock, sstep(0.55, 0.75, noise_fbm3(q * 40.0, 4)) * 0.6);
    // shallow shelves are lighter
    let shelf = sstep(0.47, 0.515, c) * (1.0 - land);
    let ocean = mix(vec3f(0.002, 0.008, 0.03), vec3f(0.006, 0.035, 0.06), shelf);
    let alb = mix(ocean, lc, land);
    // cities: where land is lush and near coasts, as clusters of points
    let pop = sstep(0.4, 0.72, noise_fbm3(q * 9.0 + 2.0, 4)) * land * (0.5 + 0.8 * sstep(0.58, 0.54, c) + 0.3);
    let cq = q * 900.0;
    let cc = vec3i(floor(cq));
    let h = hash_cell3(cc, 0xc17u);
    let f = cq - floor(cq) - (0.2 + 0.6 * h.xyz);
    let pt = exp(-dot(f, f) * 10.0) * step(h.w, pop * 0.9);
    let glow = pop * 0.45 * sstep(0.5, 0.8, noise_fbm3(q * 60.0, 3)) + pop * pop * 0.3 * sstep(0.62, 0.85, noise_fbm3(q * 22.0 + 5.0, 3));
    let city = pt * 0.8 + glow;
    // cloud deck, drifting on its own
    // fronts and cloud streets, broken into small cumulus
    let cqd = spin(n, t * 1.3) * 9.0;
    let cw = noise_fbm3(cqd * 0.8 + 7.0, 3);
    let big = noise_fbm3(cqd * 0.6 + vec3f(cw * 1.5), 5);
    let small = noise_fbm3(cqd * 7.0 + 3.0, 4);
    var cl = sstep(0.5, 0.64, big) * sstep(0.4, 0.62, small + big * 0.3);
    cl = max(cl, sstep(0.64, 0.74, big));
    // storm swirls near the equator band of the texture
    return Ground(alb, land, city, cl);
}

// optical depth from p toward the sun, a short march
fn od_sun(p: vec3f, s: vec3f) -> vec2f {
    let tt = ray_sphere(p, s, RA).y;
    let n = 4;
    let ds = tt / f32(n);
    var od = vec2f(0.0);
    for (var i = 0; i < 4; i++) {
        let q = p + s * (f32(i) + 0.5) * ds;
        let h = length(q) - RE;
        if (h < 0.0) { return vec2f(1e4); }
        od += vec2f(exp(-h / HR), exp(-h / HM)) * ds;
    }
    return od;
}

// single scattering along ro + rd*[t0, t1]; returns (in-scattered light, transmittance)
struct Air {
    light: vec3f,
    tr: vec3f,
    glow: vec3f,      // airglow and aurora emission
}

// Oxygen airglow: a thin shell ~100 km up. Its emission along a ray is the
// ray's path length through the shell, in closed form (a march would need
// hundreds of steps to resolve 5 km at the limb). `hits` = the ray ends on
// the ground (so it crosses the shell once).
fn airglow(ro: vec3f, rd: vec3f, s: vec3f, hits: bool) -> vec3f {
    let rg = RE + 100.0;
    let w = 6.0;
    let tc = -dot(ro, rd);
    let rmin2 = dot(ro, ro) - tc * tc;
    let cross = rg * rg - rmin2;
    // path through a Gaussian shell of width w, grazing-safe
    let len = w * 1.77 * rg / sqrt(max(cross, 0.0) + 2.0 * rg * w);
    let reach = sstep(-2.0 * rg * w, 0.0, cross);
    var k = select(2.0, 0.08, hits);
    // night side only, judged where the ray grazes the shell
    let tp = normalize(ro + rd * max(tc, 0.0));
    let night = sstep(0.1, -0.12, dot(tp, s));
    return vec3f(0.35, 1.0, 0.4) * len * reach * k * night * 0.00016;
}

fn air(ro: vec3f, rd: vec3f, t0: f32, t1: f32, s: vec3f, l: Look, ctx: Ctx) -> Air {
    let n = 18;
    let ds = (t1 - t0) / f32(n);
    let mu = dot(rd, s);
    let pr = 3.0 / (16.0 * PI) * (1.0 + mu * mu);
    let g = 0.76;
    let pm = 3.0 / (8.0 * PI) * ((1.0 - g * g) * (1.0 + mu * mu)) / ((2.0 + g * g) * pow(1.0 + g * g - 2.0 * g * mu, 1.5));
    var od = vec2f(0.0);
    var sr = vec3f(0.0);
    var sm = vec3f(0.0);
    var em = vec3f(0.0);
    for (var i = 0; i < 18; i++) {
        let p = ro + rd * (t0 + (f32(i) + ctx.jitter) * ds);
        let r = length(p);
        let h = r - RE;
        let dr = exp(-h / HR) * ds;
        let dm = exp(-h / HM) * ds;
        od += vec2f(dr, dm);
        let ol = od_sun(p, s);
        let tau = BR * (od.x + ol.x) + BM * 1.1 * (od.y + ol.y);
        let tr = exp(-tau);
        sr += tr * dr;
        sm += tr * dm;
        // night emissions
        let nightside = sstep(0.05, -0.15, dot(p / r, s));
        if (nightside > 0.0) {
            // aurora: a ring around the magnetic pole, rays standing up in it
            if (l.aurora > 0.0) {
                let pole = normalize(vec3f(-0.3, 0.78, -0.55));
                let lat = dot(p / r, pole);
                let ring = exp(-sq((lat - 0.93) / 0.02));
                if (ring > 0.01 && h > 90.0 && h < 400.0) {
                    let lon = atan2(dot(p / r, vec3f(0.0, 0.0, 1.0)), dot(p / r, vec3f(1.0, 0.0, 0.0)));
                    let rays = 0.3 + 0.7 * noise_value2(vec2f(lon * 60.0 + ctx.t * 0.05, lat * 30.0));
                    let prof = sstep(95.0, 115.0, h) * exp(-(h - 115.0) / 70.0);
                    let red = sstep(200.0, 300.0, h) * 0.2;
                    em += (vec3f(0.1, 1.0, 0.35) * prof + vec3f(0.8, 0.1, 0.3) * red) * ring * rays * ds * 0.0018 * l.aurora * nightside;
                }
            }
        }
    }
    let sun = 20.0;
    let light = sun * (sr * BR * pr + sm * BM * pm);
    let tr = exp(-(BR * od.x + BM * 1.1 * od.y));
    return Air(light, tr, em);
}

// a piece of the station: a solar array wing reaching in from the frame's
// top-right corner, and a strut
fn station(p: vec2f, ctx: Ctx, s: vec3f, bright: f32) -> vec4f {
    let corner = ctx.half;
    let q = p - corner;
    // the wing: a long panel, tilted, with a cell grid
    let ang = -0.42;
    let r = rot2(ang) * (q - vec2f(-0.06, -0.08));
    let along = r.x + 0.55;
    let half_w = 0.075;
    let inside = sstep(ctx.px, -ctx.px, abs(r.y) - half_w) * sstep(-ctx.px, ctx.px, along);
    // the mast down its middle
    let mast = sstep(ctx.px * 1.5, 0.0, abs(r.y) - 0.004);
    // cells: amber-gold film with dark gaps
    let cell = vec2f(fract(along * 26.0), fract(r.y * 26.0 + 0.5));
    let gap = max(sstep(0.9, 0.97, cell.x), sstep(0.88, 0.96, cell.y)) * 0.8;
    let blanket = 0.35 + 0.1 * noise_value2(vec2f(along * 8.0, r.y * 8.0));
    var c = vec3f(0.35, 0.22, 0.08) * blanket * (1.0 - gap);
    // the sun glances off it in daylight, it is a shadow at night
    c = c * (0.02 + 0.9 * bright);
    c = mix(c, vec3f(0.6, 0.6, 0.62) * (0.03 + 0.5 * bright), mast);
    // a strut from the corner
    let sd = sdf2_segment(q, vec2f(0.0, -0.02), vec2f(-0.2, -0.16)) - 0.012;
    let strut = sstep(ctx.px, -ctx.px, sd);
    let sc = vec3f(0.55, 0.55, 0.58) * (0.02 + 0.6 * bright) * (0.7 + 0.3 * sstep(-0.012, 0.012, sd + 0.006));
    var a = max(inside, strut);
    c = mix(c, sc, strut);
    return vec4f(c, a);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let s = sun_dir(l, ctx.t);
    let ro = vec3f(0.0, RE + ALT, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.12, -0.62, -1.0), 0.14, 70.0);
    let rd = cam_ray(cam, p);
    var col = vec3f(0.0);
    let te = ray_sphere(ro, rd, RE);
    let ta = ray_sphere(ro, rd, RA);
    // background: stars (only where the air and the Earth leave them)
    var bg = star_field(rd, l.stars, ctx) * 0.5;
    // the sun itself, with a glare that the lens spreads
    let mu = dot(rd, s);
    bg += vec3f(1.0, 0.95, 0.85) * (sstep(0.99996, 0.99999, mu) * 60.0 + pow(saturate(mu), 2000.0) * 6.0 + pow(saturate(mu), 60.0) * 0.08);
    if (te.x > 0.0) {
        // the ground
        let hp = ro + rd * te.x;
        let n = hp / RE;
        let g = ground(n, ctx.t, te.x * ctx.px);
        let ndl = dot(n, s);
        let day = saturate(ndl * 4.0 + 0.1);
        // sunlight reaching the ground, reddened near the terminator
        let od = od_sun(hp + n * 0.5, s);
        let sun_tr = exp(-(BR * od.x + BM * 1.1 * od.y));
        let lit = 20.0 * 0.16 * sun_tr * saturate(ndl);
        var gc = g.alb * lit;
        // ocean sunglint
        let hv = normalize(s - rd);
        gc += (1.0 - g.land) * (1.0 - g.cloud) * sun_tr * pow(saturate(dot(n, hv)), 300.0) * 2.0 * step(0.0, ndl);
        // clouds above it all
        gc = mix(gc, vec3f(0.85) * lit * 1.1 + vec3f(0.01, 0.015, 0.03) * day, g.cloud * 0.9);
        // city lights on the night side, dimmed under cloud; a gibbous moon
        // silvers the cloud tops
        let night = sstep(0.05, -0.1, ndl);
        gc += (g.alb * 0.4 + vec3f(0.5) * g.cloud) * vec3f(0.006, 0.008, 0.012) * night * l.lights;
        gc += vec3f(1.0, 0.62, 0.28) * g.city * night * l.lights * (1.0 - 0.8 * g.cloud) * 0.2;
        // lightning, now and then, inside a storm on the night side
        let ev = hash_event(ctx.t, 7.0, 0x11e7u);
        if (ev.x < 0.5 && ev.y < 0.06 && l.lights > 0.0) {
            let hc = hash_cell2(vec2i(i32(ev.z), 1), 0x11e8u);
            let lp = vec2f(-0.4 + 0.8 * hc.x, -0.35 + 0.3 * hc.y);
            let fl = exp(-length(p - lp) / 0.03) * g.cloud * night;
            gc += vec3f(0.6, 0.7, 1.0) * fl * 0.4 * (1.0 - ev.y / 0.06);
        }
        let a = air(ro, rd, max(ta.x, 0.0), te.x, s, l, ctx);
        col = gc * a.tr + a.light * 0.55 + a.glow + airglow(ro, rd, s, true);
    } else if (ta.y > 0.0) {
        // the limb: sky seen edge-on against space
        let a = air(ro, rd, max(ta.x, 0.0), ta.y, s, l, ctx);
        col = bg * a.tr + a.light + a.glow + airglow(ro, rd, s, false);
    } else {
        col = bg;
    }
    // the station in the corner
    let lit_st = saturate(dot(normalize(vec3f(0.3, 0.8, 0.5)), s) * 2.0 + 0.1) * step(-0.2, s.y);
    let st = station(p, ctx, s, lit_st);
    col = mix(col, st.rgb, st.a);
    return col * exp2(l.exposure);
}
