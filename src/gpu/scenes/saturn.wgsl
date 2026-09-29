//! name: saturn
//! title: Cassini at Saturn
//! category: space
//! tags: saturn, rings, planet, cassini, moon, space
//! desc: Saturn and its rings from Cassini: ring shadows on the clouds, a moon on its orbit
//! themes: sunlit, backlit, equinox
//! uses: camera, noise, stars
//! cost: light
//! fallback: orbits
//! credits: original; ring radii from public Cassini data

// Units are Saturn's equatorial radius; the planet sits at the origin with
// its spin axis along +y, flattened by 10%. Everything is ray-traced in
// closed form: the oblate globe, the ring plane (a radial profile of the D,
// C, B, A and F rings with the Cassini division and the Encke gap, plus the
// faint G and E rings that only show when backlit), the rings' shadow on the
// clouds, the planet's shadow across the rings, lit versus unlit ring
// faces, and a moon on its orbit.

struct Look {
    sun: vec3f,       // direction toward the sun
    cam_el: f32,      // camera elevation above the ring plane (degrees)
    cam_az: f32,
    dist: f32,
    backlit: f32,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            // "In Saturn's Shadow": the sun hidden behind the globe, a little
            // below the ring plane, so we look at the rings' unlit face
            return Look(normalize(vec3f(0.02, -0.07, -1.0)), 12.0, 0.0, 8.0, 1.0, 1.2);
        }
        case 2u: {
            // equinox: the sun in the ring plane, the rings a dark sliver
            // and their shadow a hairline on the equator
            return Look(normalize(vec3f(-0.75, 0.006, 0.66)), 9.0, 18.0, 6.6, 0.0, -1.1);
        }
        default: {
            return Look(normalize(vec3f(-0.62, 0.42, 0.66)), 21.0, 12.0, 6.8, 0.0, -1.3);
        }
    }
}

const FLAT: f32 = 0.902;       // polar / equatorial radius
const SPIN: f32 = 0.02;        // rad/s: the planet and its spokes, time-lapsed

// radial optical depth of the main rings at radius r (planet radii); fw is
// the pixel's footprint across the rings, so hairline features (the F ring,
// the gaps) widen and fade instead of aliasing
fn ring_tau(r: f32) -> f32 { return ring_tau_f(r, 0.0); }

fn ring_tau_f(r: f32, fw: f32) -> f32 {
    var t = 0.0;
    // D ring: faint
    t += 0.02 * sstep(1.11, 1.13, r) * sstep(1.24, 1.22, r);
    // C ring: thin, banded with plateaus
    let c = sstep(1.235, 1.245, r) * sstep(1.527, 1.52, r);
    t += c * (0.08 + 0.06 * sstep(0.55, 0.8, noise_value2(vec2f(r * 90.0, 1.0))));
    // B ring: dense, brightest, with lots of structure
    let b = sstep(1.525, 1.53, r) * sstep(1.95, 1.945, r);
    t += b * (1.1 + 1.2 * sstep(1.6, 1.75, r) + 0.5 * (noise_value2(vec2f(r * 160.0, 2.0)) - 0.5));
    // Cassini division: mostly empty, a few faint ringlets
    let cd = sstep(1.95, 1.955, r) * sstep(2.025, 2.02, r);
    t += cd * 0.06 * noise_value2(vec2f(r * 300.0, 3.0));
    // A ring, with the Encke gap near its outer edge and the Keeler gap
    let a = sstep(2.02, 2.028, r) * sstep(2.27, 2.265, r);
    let ew = max(0.005, fw);
    let kw = max(0.002, fw);
    let encke = 1.0 - (1.0 - sstep(ew * 0.8, ew * 1.3, abs(r - 2.214))) * (0.005 / ew);
    let keeler = 1.0 - (1.0 - sstep(kw * 0.75, kw * 1.5, abs(r - 2.2605))) * (0.002 / kw);
    t += a * (0.55 + 0.15 * (noise_value2(vec2f(r * 220.0, 4.0)) - 0.5)) * encke * keeler;
    // F ring: a thin braided strand
    let fwid = max(0.0035, fw);
    t += 0.35 * (0.0035 / fwid) * exp(-sq((r - 2.326) / fwid));
    return t;
}

// the dusty rings (visible only in forward scattering): G and the broad E
fn dust_tau(r: f32) -> f32 {
    return 0.035 * exp(-sq((r - 2.78) / 0.05)) + 0.02 * exp(-sq((r - 3.95) / 0.9)) * sstep(3.0, 3.4, r);
}

fn ring_colour(r: f32) -> vec3f {
    let c = vec3f(0.36, 0.33, 0.3);          // C ring: dusky grey
    let b = vec3f(0.9, 0.76, 0.56);          // B ring: warm cream
    let a = vec3f(0.68, 0.62, 0.54);         // A ring: greyer
    var col = mix(c, b, sstep(1.5, 1.6, r));
    col = mix(col, a, sstep(1.96, 2.04, r));
    return col * (0.92 + 0.16 * noise_value2(vec2f(r * 70.0, 9.0)));
}

// ray against the oblate globe: t or -1
fn hit_globe(ro: vec3f, rd: vec3f) -> f32 {
    let s = vec3f(1.0, 1.0 / FLAT, 1.0);
    let o = ro * s;
    let d = rd * s;
    let a = dot(d, d);
    let b = dot(o, d);
    let c = dot(o, o) - 1.0;
    let disc = b * b - a * c;
    if (disc < 0.0) { return -1.0; }
    let t = (-b - sqrt(disc)) / a;
    return select(-1.0, t, t > 0.0);
}

// closest the ray passes to the globe, in radii above it (for the limb glow)
fn graze(ro: vec3f, rd: vec3f) -> f32 {
    let s = vec3f(1.0, 1.0 / FLAT, 1.0);
    let o = ro * s;
    let d = normalize(rd * s);
    let tc = -dot(o, d);
    return length(o + d * max(tc, 0.0)) - 1.0;
}

fn globe_colour(n: vec3f, t: f32) -> vec3f {
    let lat = asin(clamp(n.y, -1.0, 1.0));
    // the planet turns (a time-lapse: a turn every five minutes), and the
    // bands slip against each other on their zonal jets
    let lon = atan2(n.z, n.x) + t * SPIN;
    let jet = 0.004 * sin(lat * 7.0) + 0.002 * sin(lat * 17.0);
    let q = vec2f(lon + t * jet, lat);
    let turb = noise_fbm2(vec2f(q.x * 9.0, q.y * 40.0), 4) - 0.5 + 0.5 * (noise_fbm2(vec2f(q.x * 30.0, q.y * 90.0), 3) - 0.5);
    let bl = lat + turb * 0.045;
    let bands = 0.5 + 0.5 * sin(bl * 26.0) * 0.6 + 0.5 * sin(bl * 9.0 + 1.0) * 0.4;
    var c = mix(vec3f(0.72, 0.56, 0.34), vec3f(0.9, 0.78, 0.55), bands);
    // butterscotch equatorial zone, pale blue-grey toward the north pole
    c = mix(c, vec3f(0.95, 0.78, 0.46), exp(-sq(lat / 0.12)) * 0.6);
    c = mix(c, vec3f(0.55, 0.62, 0.68), sstep(0.75, 1.2, lat) * 0.8);
    c = mix(c, vec3f(0.66, 0.6, 0.52), sstep(-0.8, -1.2, lat) * 0.5);
    // the ribbon: a wavy dark jet near 47 degrees north
    let rib = 0.82 + 0.025 * sin(q.x * 11.0) + 0.012 * sin(q.x * 27.0 + 1.0);
    c *= 1.0 - 0.18 * exp(-sq((lat - rib) / 0.018));
    // storms: pale ovals and a dark vortex riding the jets
    for (var i = 0; i < 7; i++) {
        let h = hash_cell2(vec2i(i, 5), 0x5707u);
        let slat = 0.2 + 0.55 * h.x;
        let slon = h.y * TAU;
        let dl = atan2(sin(q.x - slon), cos(q.x - slon));
        let e = sq(dl / (0.12 + 0.12 * h.z)) + sq((lat - slat) / (0.045 + 0.03 * h.z));
        let tone = select(vec3f(0.97, 0.93, 0.84), vec3f(0.55, 0.42, 0.28), h.w > 0.8);
        c = mix(c, tone, exp(-e * 1.5) * 0.6);
    }
    return c;
}

struct Moon {
    c: vec3f,
    r: f32,
}

fn moon_pos(t: f32) -> Moon {
    // Tethys (drawn twice its size), on an orbit sped up forty times
    let a = t * 0.003 + 2.2;
    let rr = 4.9;
    return Moon(vec3f(cos(a) * rr, 0.0, sin(a) * rr), 0.045);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    var l = look(ctx.theme);
    let el = radians(l.cam_el);
    let az = radians(l.cam_az);
    // narrow (portrait) frames: step back so the whole ring system fits
    let fit = max(1.0, 1.05 / ctx.aspect);
    let ro = vec3f(sin(az) * cos(el), sin(el), cos(az) * cos(el)) * l.dist * fit;
    if (l.backlit > 0.0) {
        // the sun exactly behind the globe: we float in Saturn's shadow
        l.sun = normalize(-ro + vec3f(0.0, -0.15, 0.0));
    }
    let cam = cam_look_at(ro, vec3f(0.25 / fit, -0.05, 0.0), radians(-6.0), 34.0);
    let rd = cam_ray(cam, p);
    let sun_i = 3.2;
    // background stars
    var col = star_field(rd, 0.2, ctx) * 0.35;
    // the sun, when it is in view, blazing behind everything
    let mu = dot(rd, l.sun);
    // (hidden behind the globe, it leaves no glare)
    let sun_seen = select(1.0, 0.0, hit_globe(ro, l.sun) > 0.0);
    col += vec3f(1.0, 0.96, 0.9) * (pow(saturate(mu), 3000.0) * 30.0 + pow(saturate(mu), 200.0) * 0.4) * l.backlit * sun_seen;
    // the E ring and G ring, glowing only toward the sun (forward scatter)
    let tr = -ro.y / rd.y;
    let tg = hit_globe(ro, rd);
    let fwd = pow(saturate(mu * 0.5 + 0.5), 12.0);
    if (tr > 0.0) {
        let rp = ro + rd * tr;
        let rr = length(rp.xz);
        let dust = dust_tau(rr) * (0.08 + 3.0 * fwd * l.backlit);
        if (tg < 0.0 || tr < tg) {
            col += vec3f(0.55, 0.65, 0.9) * dust * 0.6 / max(abs(rd.y), 0.05) * 0.1;
        }
    }
    // the globe
    var gc = vec3f(0.0);
    var globe_a = 0.0;
    if (tg > 0.0) {
        let hp = ro + rd * tg;
        let n = normalize(hp * vec3f(1.0, 1.0 / (FLAT * FLAT), 1.0));
        let alb = globe_colour(n, ctx.t);
        let ndl = dot(n, l.sun);
        let ndv = saturate(dot(n, -rd));
        // Minnaert limb darkening
        let k = 0.85;
        var lit = pow(saturate(ndl), k) * pow(max(ndv, 0.02), k - 1.0) * saturate(ndl * 8.0);
        // the rings' shadow on the clouds
        let ts = -hp.y / l.sun.y;
        if (ts > 0.0) {
            let sp = hp + l.sun * ts;
            let sr = length(sp.xz);
            let tau = ring_tau(sr);
            lit *= mix(1.0, exp(-tau * 1.2 / max(abs(l.sun.y), 0.02)), sstep(1.1, 1.12, sr) * sstep(2.35, 2.33, sr));
        }
        gc = alb * lit * sun_i * 0.35;
        // ringshine: the sunlit rings light the night side faintly
        let ring_side = sign(l.sun.y) * n.y;
        gc += alb * (0.004 + 0.02 * l.backlit) * saturate(ring_side * 0.8 + 0.3 + l.backlit * 0.5) * (1.0 - saturate(ndl * 4.0 + 0.5));
        globe_a = 1.0;
    }
    // limb: sunlight scattering through the upper atmosphere, a bright ring
    // when the sun is behind the planet
    let gz = graze(ro, rd);
    if (l.backlit > 0.0) {
        let halo = exp(-max(gz, 0.0) / 0.005) * sstep(-0.003, 0.001, gz) * (0.3 + 2.0 * fwd);
        col += vec3f(1.0, 0.86, 0.68) * halo * 0.3;
        gc += vec3f(1.0, 0.8, 0.6) * exp(max(-gz, 0.0) / -0.004) * globe_a * 0.12 * (0.3 + 2.0 * fwd);
    }
    col = mix(col, gc, globe_a);
    // the rings, in front of the globe or on their own
    if (tr > 0.0 && (tg < 0.0 || tr < tg)) {
        let rp = ro + rd * tr;
        let rr = length(rp.xz);
        let fw = ctx.px * tr / max(abs(rd.y), 0.05);
        let tau = ring_tau_f(rr, fw);
        if (tau > 0.0005) {
            let mu_v = max(abs(rd.y), 0.02);
            let mu_s = max(abs(l.sun.y), 0.004);
            let opac = 1.0 - exp(-tau / mu_v);
            let rc = ring_colour(rr);
            // in the planet's shadow?
            var shad = 1.0;
            if (hit_globe(rp + l.sun * 0.001, l.sun) > 0.0) { shad = 0.0; }
            let same_side = sign(rd.y) != sign(l.sun.y);
            // lit face: reflected sunlight; unlit face: light diffusing through
            let refl = rc * (1.0 - exp(-tau / mu_s)) * 0.55;
            let trans = rc * exp(-tau / mu_s) * (1.0 - exp(-tau / mu_s)) * (0.2 + 1.6 * fwd);
            // (a lit face gathers sunlight in proportion to the sun's height)
            var ring_l = select(trans, refl * saturate(mu_s * 3.0), same_side) * sun_i * shad;
            // Saturnshine on the rings near the planet
            ring_l += rc * 0.004 / (rr * rr);
            // spokes in the B ring: faint dark wedges riding around
            // (spokes ride the magnetic field, so they turn with the planet)
            let ang = atan2(rp.z, rp.x) + ctx.t * SPIN;
            let spoke = sstep(0.7, 0.85, noise_value2(vec2f(ang * 40.0, rr * 2.5))) * sstep(1.62, 1.72, rr) * sstep(1.92, 1.84, rr);
            ring_l *= 1.0 - 0.15 * spoke;
            col = mix(col, ring_l, opac);
        }
    }
    // the moon
    let m = moon_pos(ctx.t);
    let oc = ro - m.c;
    let bq = dot(oc, rd);
    let cq = dot(oc, oc) - m.r * m.r;
    let dq = bq * bq - cq;
    if (dq > 0.0) {
        let tm = -bq - sqrt(dq);
        let occluded = (tg > 0.0 && tg < tm) || (tr > 0.0 && tr < tm && ring_tau(length((ro + rd * tr).xz)) > 0.3);
        if (tm > 0.0 && !occluded) {
            let mn = normalize(ro + rd * tm - m.c);
            var ml = saturate(dot(mn, l.sun));
            if (hit_globe(m.c, l.sun) > 0.0) { ml = 0.0; }
            let mc = vec3f(0.85, 0.85, 0.83) * (ml * sun_i * 0.3 + 0.002);
            let edge = sstep(0.0, ctx.px * 0.8 / max(tm, 1.0) * 1.5, dq / (m.r * 2.0));
            col = mix(col, mc, saturate(edge + 0.5));
        }
    }
    return col * exp2(l.exposure);
}
