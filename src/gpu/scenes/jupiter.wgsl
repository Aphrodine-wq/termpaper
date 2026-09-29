//! name: jupiter
//! title: Jupiter
//! category: space
//! tags: jupiter, planet, storms, great red spot, io, aurora, juno, space
//! desc: Jupiter's banded storms and the Great Red Spot, Io's shadow crossing the clouds
//! themes: voyager, juno, aurora
//! uses: camera, noise, stars
//! cost: medium
//! fallback: orbits
//! credits: original; band layout after Voyager, Cassini and Juno imagery

// Units are Jupiter's equatorial radius; the globe sits at the origin, its
// spin axis along +y, flattened by 6.5%. Clouds are a function of latitude
// and longitude: a band profile of pale zones and brown belts, turbulence
// dragged along each band by its own jet (so neighbouring bands shear past
// each other), the Great Red Spot turning inside its pale collar, blue-grey
// festoons trailing off the equator, and at the poles the ring of cyclones
// Juno found. Voyager's view has Io and its shadow crossing the disc; Juno
// skims low over the south pole; the aurora view looks at the night side
// from above the north pole, where the auroral oval glows.

const FLAT: f32 = 0.935;
const GRS_LAT: f32 = -0.39;    // 22 degrees south

struct Look {
    mode: u32,
    ro: vec3f,         // camera position
    at: vec3f,         // looking at
    fov: f32,
    sun: vec3f,        // direction toward the sun
    colour: f32,       // 0 natural, 1 Juno's enhanced colour
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            // Juno: under the south pole, looking up at its ring of cyclones
            return Look(1u, vec3f(0.35, -1.95, 0.45), vec3f(0.1, 0.0, -0.1), 60.0,
                        normalize(vec3f(0.62, -0.42, 0.66)), 1.0, -0.1);
        }
        case 2u: {
            // above the north pole, on the night side: the auroral oval
            return Look(2u, vec3f(-0.6, 2.7, -1.5), vec3f(0.05, 0.25, 0.0), 44.0,
                        normalize(vec3f(0.5, -0.1, 0.86)), 0.0, 0.6);
        }
        default: {
            return Look(0u, vec3f(0.6, 0.3, 3.9), vec3f(0.5, 0.02, 0.0), 38.0,
                        normalize(vec3f(-0.45, 0.12, 0.88)), 0.0, -0.4);
        }
    }
}

// ray against the oblate globe (radius scale `s`): t or -1
fn hit_globe_s(ro: vec3f, rd: vec3f, s: f32) -> f32 {
    let k = vec3f(1.0, 1.0 / FLAT, 1.0) / s;
    let o = ro * k;
    let d = rd * k;
    let a = dot(d, d);
    let b = dot(o, d);
    let c = dot(o, o) - 1.0;
    let disc = b * b - a * c;
    if (disc < 0.0) { return -1.0; }
    let t = (-b - sqrt(disc)) / a;
    return select(-1.0, t, t > 0.0);
}

// the zonal wind at a latitude: which way (and how fast) its band drifts
fn jet(lat: f32) -> f32 {
    return 0.012 * sin(lat * 13.0) + 0.006 * sin(lat * 29.0 + 0.7) + 0.01 * exp(-sq(lat / 0.12));
}

// the band profile: 0 = pale zone, 1 = dark belt
fn belt(lat: f32) -> f32 {
    let neb = exp(-sq((lat - 0.2) / 0.075));             // north equatorial belt
    let seb = exp(-sq((lat + 0.24) / 0.07));             // south equatorial belt
    let ntb = exp(-sq((lat - 0.47) / 0.05)) * 0.7;
    let stb = exp(-sq((lat + 0.5) / 0.05)) * 0.6;
    let nntb = exp(-sq((lat - 0.68) / 0.05)) * 0.5;
    let sstb = exp(-sq((lat + 0.7) / 0.05)) * 0.5;
    return saturate(neb + seb + ntb + stb + nntb + sstb);
}

struct Cloud {
    albedo: vec3f,
    glow: f32,         // aurora emission
}

fn clouds(n: vec3f, l: Look, t: f32, ctx: Ctx) -> Cloud {
    let lat = asin(clamp(n.y, -1.0, 1.0));
    let lon0 = atan2(n.z, n.x);
    // each band slides at its own speed: the texture is advected by the jet
    let drift = t * jet(lat);
    let lon = lon0 + drift + t * 0.004;
    // turbulence, sheared along the bands (stretched in longitude)
    let q = vec2f(cos(lon) * 3.0, sin(lon) * 3.0);
    let w = noise_fbm3(vec3f(q, lat * 9.0), 4);
    let w2 = noise_fbm3(vec3f(q * 2.3 + 5.0, lat * 21.0 + w * 2.5), select(3, 4, ctx.detail > 0u));
    let wl = lat + (w - 0.5) * 0.06 + (w2 - 0.5) * 0.035;
    let b = belt(wl);
    // palette: creamy zones, rusty belts, bluish polar haze
    let zone = vec3f(0.88, 0.82, 0.7);
    let beltc = mix(vec3f(0.45, 0.27, 0.16), vec3f(0.62, 0.42, 0.26), w2);
    var c = mix(zone, beltc, b);
    // fine bright and dark eddies inside every band
    c *= 0.82 + 0.36 * w2;
    c = col_saturation(c, 1.25);
    // the equatorial zone: ochre haze
    c = mix(c, vec3f(0.9, 0.78, 0.58), exp(-sq(lat / 0.1)) * 0.45);
    // festoons: dark blue-grey plumes off the north equatorial belt's edge
    let fx = fract((lon0 + t * 0.012) * 1.6 + 0.5);
    let festoon = exp(-sq((lat - 0.12 + 0.05 * sin(fx * TAU)) / 0.018)) * sstep(0.2, 0.5, fx) * sstep(0.9, 0.6, fx);
    c = mix(c, vec3f(0.36, 0.42, 0.5), festoon * 0.6);
    // the poles: no bands, just turbulence drawn into filaments, and the
    // ring of cyclones Juno found round each pole (eight, one in the middle)
    let polar = sstep(0.95, 1.2, abs(lat));
    if (polar > 0.0) {
        let pole = select(-1.0, 1.0, lat > 0.0);
        let pp = vec2f(n.x, n.z) / max(abs(n.y), 0.2);
        let fil = noise_warp2(pp * 7.0 + vec2f(t * 0.002, 0.0), 1.4, 4);
        let fine = noise_fbm2(pp * 26.0 + vec2f(fil * 3.0), 3);
        var pc = mix(vec3f(0.28, 0.3, 0.38), vec3f(0.8, 0.76, 0.7), sstep(0.35, 0.72, fil * 0.8 + fine * 0.35));
        for (var i = 0; i < 9; i++) {
            let fi = f32(i);
            let ring = select(0.34, 0.0, i == 8);
            let a = fi * TAU / 8.0 + t * 0.0008 * pole;
            let cc = vec2f(cos(a), sin(a)) * ring;
            let rr = select(0.13, 0.11, i == 8) * (0.9 + 0.2 * hash_f(u32(i) + 7u));
            let dv = pp - cc;
            let d = length(dv) / rr;
            if (d < 1.4) {
                // storm bands wound into a spiral, turning slowly
                let ang = atan2(dv.y, dv.x) * pole + 5.5 * d - t * 0.03;
                let lanes = 0.5 + 0.5 * cos(ang * 2.0 + fine * 2.0);
                let body = sstep(1.05, 0.7, d);
                let storm = mix(vec3f(0.95, 0.9, 0.8), vec3f(0.35, 0.36, 0.44), lanes * sstep(0.12, 0.45, d) * 0.8);
                pc = mix(pc, storm, body);
                // a dark moat round each storm
                pc *= 1.0 - 0.35 * exp(-sq((d - 1.12) / 0.12));
            }
        }
        c = mix(c, pc, polar);
    }
    // the Great Red Spot: an oval swirling in its pale collar, drifting west
    // (it starts on the face turned to the Voyager camera)
    let gl = lon0 + t * 0.002 - 1.15;
    let dl = atan2(sin(gl), cos(gl));
    let e = vec2f(dl / 0.21, (lat - GRS_LAT) / 0.075);
    let er = length(e);
    if (er < 2.0) {
        let swirl = atan2(e.y, e.x) + 2.5 * (1.0 - er) + t * 0.03;
        let inner = noise_fbm3(vec3f(cos(swirl) * er * 2.0, sin(swirl) * er * 2.0, 7.0), 4);
        let spot = mix(vec3f(0.62, 0.24, 0.13), vec3f(0.8, 0.42, 0.26), inner);
        let collar = exp(-sq((er - 1.12) / 0.2));
        c = mix(c, spot, sstep(1.05, 0.9, er));
        c = mix(c, vec3f(0.96, 0.92, 0.84), collar * 0.55);
    }
    // a few white ovals in the south temperate belt
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let ol = lon0 + t * 0.003 + fi * 2.1 + 0.4;
        let od = atan2(sin(ol), cos(ol));
        let oe = length(vec2f(od / 0.06, (lat + 0.58) / 0.028));
        c = mix(c, vec3f(0.97, 0.95, 0.9), exp(-oe * oe) * 0.8);
    }
    if (l.colour > 0.5) {
        // Juno's enhanced colour: deep blue shadows, cream storm tops, browns
        // between
        let k = pow(saturate(col_luma(c) * 1.15), 1.8);
        c = col_ramp4(vec3f(0.05, 0.1, 0.26), vec3f(0.22, 0.32, 0.5), vec3f(0.62, 0.52, 0.42), vec3f(0.98, 0.92, 0.8), 0.33, 0.66, k);
    }
    // the auroral ovals (seen on the night side)
    let colat = acos(clamp(abs(n.y) * 1.03, -1.0, 1.0));
    let oval = exp(-sq((colat - 0.28 - 0.03 * sin(lon0 * 3.0 + t * 0.05)) / 0.035));
    let flick = 0.6 + 0.4 * noise_value2(vec2f(lon0 * 12.0 + t * 0.08, t * 0.05));
    return Cloud(c, oval * flick * select(0.0, 1.0, n.y > 0.0));
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    // tall frames: step back a little so the planet still fits
    let fit = max(1.0, 1.0 / ctx.aspect);
    let ro = l.at + (l.ro - l.at) * mix(1.0, fit, select(0.6, 0.0, l.mode == 1u));
    // (Juno looks straight up the axis: a sideways up vector)
    let cam = cam_look_at(ro, l.at, radians(select(-8.0, 80.0, l.mode == 1u)), l.fov);
    let rd = cam_ray(cam, p);
    var col = star_field(rd, 0.25, ctx) * 0.4;
    let sun_i = 0.62;

    // Io, on its orbit (sped up), and its shadow
    let ia = ctx.t * 0.004 + 0.9;
    let io = vec3f(cos(ia) * 1.9, 0.02, sin(ia) * 1.9) * select(1.0, 0.0, l.mode != 0u);
    let io_r = 0.045;

    let tg = hit_globe_s(ro, rd, 1.0);
    if (tg > 0.0) {
        let hp = ro + rd * tg;
        let n = normalize(hp * vec3f(1.0, 1.0 / (FLAT * FLAT), 1.0));
        let cl = clouds(n, l, ctx.t, ctx);
        let ndl = dot(n, l.sun);
        let ndv = saturate(dot(n, -rd));
        // Minnaert limb darkening and a soft terminator
        var lit = pow(saturate(ndl), 0.9) * pow(max(ndv, 0.03), -0.1) * sstep(-0.02, 0.12, ndl);
        // Io's shadow: a hard black dot where Io blocks the sun
        if (l.mode == 0u) {
            let to_io = io - hp;
            let along = dot(to_io, l.sun);
            if (along > 0.0) {
                let miss = length(to_io - l.sun * along);
                lit *= sstep(io_r * 0.8, io_r * 1.05, miss);
            }
        }
        col = cl.albedo * lit * sun_i;
        // the night side: faint, and the aurora
        col += cl.albedo * 0.004;
        col += vec3f(0.65, 0.35, 1.0) * cl.glow * (1.0 - sstep(-0.1, 0.2, ndl)) * 0.9;
        // a thin haze of scattered light at the limb
        col += vec3f(0.55, 0.6, 0.75) * pow(1.0 - ndv, 6.0) * saturate(ndl + 0.2) * 0.25;
    } else {
        // the atmosphere's rim, lit where the sun reaches it
        let k = vec3f(1.0, 1.0 / FLAT, 1.0);
        let o = ro * k;
        let d = normalize(rd * k);
        let tc = -dot(o, d);
        let closest = o + d * max(tc, 0.0);
        let gz = length(closest) - 1.0;
        let nrm = normalize(closest / k);
        let rim = exp(-max(gz, 0.0) / 0.012) * sstep(-0.004, 0.002, gz);
        col += vec3f(0.6, 0.66, 0.8) * rim * saturate(dot(nrm, l.sun) + 0.15) * 0.35;
        // the aurora standing above the northern limb
        if (l.mode == 2u && nrm.y > 0.6) {
            let colat = acos(clamp(nrm.y, -1.0, 1.0));
            let curtain = exp(-sq((colat - 0.28) / 0.05)) * exp(-max(gz, 0.0) / 0.03);
            col += vec3f(0.6, 0.35, 1.0) * curtain * 0.5;
        }
    }
    // Io itself
    if (l.mode == 0u) {
        let oc = ro - io;
        let bq = dot(oc, rd);
        let cq = dot(oc, oc) - io_r * io_r;
        let dq = bq * bq - cq;
        if (dq > 0.0) {
            let ti = -bq - sqrt(dq);
            if (ti > 0.0 && (tg < 0.0 || ti < tg)) {
                let mn = normalize(ro + rd * ti - io);
                let io_col = mix(vec3f(0.95, 0.85, 0.45), vec3f(0.8, 0.5, 0.25), noise_value3(mn * 6.0));
                let ml = saturate(dot(mn, l.sun));
                col = io_col * ml * sun_i;
            }
        }
    }
    return col * exp2(l.exposure);
}
