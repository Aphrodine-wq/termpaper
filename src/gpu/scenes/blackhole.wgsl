//! name: blackhole
//! title: Black Hole
//! category: space
//! tags: black hole, accretion disk, lensing, relativity, space
//! desc: a black hole's accretion disk bent around its shadow, one side Doppler-bright
//! themes: amber, blue, edge-on
//! uses: camera, noise, stars
//! cost: medium
//! fallback: nebula
//! credits: original; photon paths from the Schwarzschild orbit equation

// Units are the Schwarzschild radius (the event horizon sits at r = 1).
// Every pixel's ray is followed backwards from the camera through the
// hole's field, bending by the exact orbit equation for light, written in
// Cartesian form: a = -3/2 h² x / r⁵, with h the ray's angular momentum.
// Each time the ray crosses the disk plane it picks up the disk's light
// there, so the lensed images come for free: the far side of the disk
// arched over the shadow, its underside curled beneath, and the thin
// photon ring where rays wound round the hole before escaping. The disk
// is a thin, turbulent Keplerian flow: hotter inward, Doppler-brightened
// where it comes toward us, redshifted near the horizon. What escapes
// shows the stars behind, lensed into arcs near the edge of the shadow.

const R_IN: f32 = 3.0;       // innermost stable orbit
const R_OUT: f32 = 11.0;
const DIST: f32 = 28.0;      // camera distance

struct Look {
    elev: f32,        // camera above the disk plane, degrees
    t_in: f32,        // disk temperature at its inner edge, kelvin
    tint: vec3f,      // what the colour film makes of it
    density: f32,     // disk opacity
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(16.0, 16000.0, vec3f(0.8, 0.95, 1.3), 0.9, 0.2);
        }
        case 2u: {
            return Look(1.2, 6200.0, vec3f(1.1, 0.98, 0.86), 2.2, 0.1);
        }
        default: {
            return Look(7.0, 5400.0, vec3f(1.2, 0.98, 0.78), 1.8, 0.0);
        }
    }
}

// the disk's light where a ray crosses it at `x` travelling along `dir`:
// (radiance, opacity)
fn disk(x: vec3f, dir: vec3f, l: Look, t: f32, ctx: Ctx) -> vec4f {
    let r = length(x.xz);
    if (r < R_IN * 0.92 || r > R_OUT * 1.25) {
        return vec4f(0.0);
    }
    // the orbit: slower outward (Kepler), turned down to a crawl so the
    // disk drifts rather than spins
    let omega = 0.55 * pow(r, -1.5);
    let phi = atan2(x.z, x.x) - t * omega;
    // turbulence: streaks along the orbits, clumps between them
    let q = vec3f(cos(phi) * 2.4, sin(phi) * 2.4, log(r) * 7.0);
    let n = noise_fbm3(q, select(4, 5, ctx.detail > 0u));
    let streak = noise_value3(vec3f(cos(phi) * 1.2, sin(phi) * 1.2, log(r) * 26.0));
    var dens = 0.15 + 1.4 * n * n * n + 0.45 * streak * streak;
    // soft edges: the inner edge falls off fast, the outer one fades out
    dens *= sstep(R_IN * 0.92, R_IN * 1.08, r) * sstep(R_OUT * 1.25, R_OUT * 0.7, r);
    // temperature: hotter inward (a thin disk's T ~ r^-3/4)
    let fall = pow(R_IN / r, 0.75) * pow(max(1.0 - sqrt(R_IN * 0.92 / r), 0.0), 0.25) * 1.45;
    // orbital speed (rs = 1: v = sqrt(1/2 / (r - 1))) and the Doppler factor
    // for light leaving toward the camera (opposite the traced ray)
    let v = sqrt(0.5 / max(r - 1.0, 0.5));
    let vdir = normalize(vec3f(-x.z, 0.0, x.x));
    let k = -normalize(dir);
    let gamma = 1.0 / sqrt(max(1.0 - v * v, 0.05));
    let doppler = 1.0 / (gamma * (1.0 - v * dot(vdir, k)));
    // climbing out of the well reddens it too
    let grav = sqrt(max(1.0 - 1.0 / r, 0.0));
    let g = doppler * grav;
    let temp = l.t_in * fall * g;
    let col = col_kelvin(temp) * l.tint;
    // beaming: brightness goes with g to the fourth power (kept in bounds),
    // so the side coming toward us outshines the side going away
    let bright = min(pow(g, 4.0), 16.0) * fall * fall * fall;
    let a = 1.0 - exp(-dens * l.density);
    return vec4f(col * bright * (0.5 + dens) * 1.3, a);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let el = radians(l.elev);
    let ro = vec3f(0.0, sin(el), cos(el)) * DIST;
    let cam = cam_look_at(ro, vec3f(0.0, 0.0, 0.0), radians(-3.0), 36.0);
    var dir = cam_ray(cam, p);
    var pos = ro;
    let hvec = cross(pos, dir);
    let h2 = dot(hvec, hvec);

    var col = vec3f(0.0);
    var alpha = 0.0;
    var captured = false;
    var rmin = 1e9;
    let n = select(160, 240, ctx.detail > 1u);
    for (var i = 0; i < 240; i++) {
        if (i >= n) { break; }
        let r2 = dot(pos, pos);
        let r = sqrt(r2);
        rmin = min(rmin, r);
        if (r < 1.0) {
            captured = true;
            break;
        }
        // leaving for good: the rest of the way is a straight line
        if (r > DIST * 1.6 && dot(pos, dir) > 0.0) { break; }
        // steps shrink near the hole, and near the disk plane so thin
        // crossings are not skipped
        let dt = clamp(0.06 * r * min(1.0, 0.2 + abs(pos.y) * 0.9), 0.012, 1.4);
        let prev = pos;
        let acc = -1.5 * h2 * pos / (r2 * r2 * r);
        dir += acc * dt;
        pos += dir * dt;
        if (prev.y * pos.y < 0.0 && alpha < 0.995) {
            let f = prev.y / (prev.y - pos.y);
            let x = mix(prev, pos, f);
            let d = disk(x, dir, l, ctx.t, ctx);
            col += (1.0 - alpha) * d.rgb * d.a;
            alpha += (1.0 - alpha) * d.a;
        }
    }
    if (!captured && alpha < 0.999) {
        // what lies behind, seen along the bent ray
        let rd = normalize(dir);
        var sky = star_field(rd, 0.35, ctx) * 0.5;
        sky += star_milky_way(rd, normalize(vec3f(0.3, 1.0, -0.2)), normalize(vec3f(-1.0, -0.3, -0.4)), ctx) * 0.3;
        // the photon ring: light that skimmed the photon sphere (r = 1.5)
        let ring = exp(-sq((rmin - 1.52) / 0.07));
        sky += l.tint * col_kelvin(l.t_in * 1.3) * ring * 0.1;
        col += (1.0 - alpha) * sky;
    }
    return col * exp2(l.exposure);
}
