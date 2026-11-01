//! name: wormhole
//! title: Wormhole Mouth
//! category: space
//! tags: wormhole, lensing, nebula, plasma, relativity, space
//! desc: a wormhole from outside: another universe through the throat, our own sky bent round its r
//! themes: aurora, ember, ghost
//! params: spin, plasma, haze
//! uses: camera, noise, stars
//! cost: heavy
//! fallback: starfield
//! credits: original; light paths after the Ellis wormhole

// Units are the throat radius. Every pixel's ray is followed backwards
// through the wormhole's field: it bends toward the throat, and a ray that
// reaches the throat passes through to the far side and sees that sky
// instead of ours. Rays that just miss wrap most of the way round, so our
// own sky stretches into a ring at the rim, the way it does round a black
// hole, except that the centre is not dark: another universe looks out of
// it. A thin ring of plasma orbits the mouth, lit from inside, and drifts
// slowly. The camera is fixed; the sky never moves.

const DIST: f32 = 13.0;

struct Look {
    nebula: vec3f,    // the far universe's nebula colour
    nebula2: vec3f,
    plasma: vec3f,    // the orbiting ring
    haze: vec3f,      // dust glow round the mouth
    stars: f32,       // our side's star density
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: { return Look(vec3f(1.3, 0.45, 0.12), vec3f(0.6, 0.12, 0.08), vec3f(1.6, 0.7, 0.3), vec3f(0.5, 0.2, 0.08), 0.6, 0.1); }
        case 2u: { return Look(vec3f(0.8, 0.82, 1.0), vec3f(0.55, 0.5, 0.8), vec3f(0.9, 0.95, 1.1), vec3f(0.3, 0.32, 0.4), 0.5, 0.15); }
        default: { return Look(vec3f(0.15, 0.9, 0.75), vec3f(0.1, 0.35, 0.8), vec3f(0.35, 1.1, 0.9), vec3f(0.08, 0.35, 0.3), 0.7, 0.0); }
    }
}

// our sky: stars and the Milky Way, slightly rotated so the two skies
// never line up
fn near_sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    var c = star_field(rd, l.stars, ctx) * 1.6;
    c += star_milky_way(rd, normalize(vec3f(0.25, 0.9, -0.35)), normalize(vec3f(-0.7, -0.2, -0.68)), ctx) * 1.1;
    return c;
}

// the far universe: denser stars, a bright nebula, a different galaxy
fn far_sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let r = vec3f(rd.z, rd.x, -rd.y);
    var c = star_field(r, 1.1, ctx) * 1.2;
    let n1 = noise_fbm3(r * 2.6 + vec3f(5.0, 1.0, 9.0), 5);
    let n2 = noise_fbm3(r * 5.0 + vec3f(1.0, 7.0, 2.0), 4);
    let cloud = smoothstep(0.35, 0.8, n1) * (0.4 + 0.8 * n2);
    let lanes = smoothstep(0.5, 0.65, noise_fbm3(r * 9.0 + 3.0, 4));
    c += mix(l.nebula, l.nebula2, n2) * cloud * (1.0 - 0.6 * lanes) * 1.4;
    // a bright young cluster inside the nebula
    let cl = pow(saturate(dot(r, normalize(vec3f(0.3, 0.2, -0.93)))), 60.0);
    c += vec3f(1.0, 0.95, 0.9) * cl * 2.0 * (0.5 + 0.5 * n2);
    c += star_milky_way(r, normalize(vec3f(-0.6, 0.3, 0.74)), normalize(vec3f(0.3, 0.9, 0.3)), ctx) * 0.35;
    return c;
}

// the plasma ring in the mouth's plane: (density) at x, turning slowly
fn plasma(x: vec3f, t: f32, ctx: Ctx) -> f32 {
    let rho = length(x.xz);
    let band = exp(-sq((rho - 1.5) / 0.32)) * exp(-sq(x.y / 0.1));
    if (band < 0.002) { return 0.0; }
    // a Keplerian swirl: faster inside, in clumps and filaments
    let phi = atan2(x.z, x.x) - t * (0.45 + ctx.params.x * 0.8) / max(rho * rho, 0.5);
    let q = vec3f(cos(phi) * 3.0, sin(phi) * 3.0, rho * 6.0);
    let n = noise_fbm3(q, 4);
    let streak = noise_value3(vec3f(cos(phi) * 1.5, sin(phi) * 1.5, rho * 22.0));
    let gaps = smoothstep(0.3, 0.6, n);
    return band * gaps * (0.1 + 2.2 * n * n + 0.8 * streak * streak);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let shift = cam_depth_offset(ctx) * 3.0;
    let ro = vec3f(0.0, 3.2, DIST) + shift;
    var cam = cam_look_at(ro, vec3f(0.0, 0.0, 0.0), 0.0, 25.0);
    cam = cam_portal(cam, ctx, DIST);
    var dir = cam_ray(cam, p);
    var pos = ro;
    // angular momentum sets how hard the ray is bent
    let hvec = cross(pos, dir);
    let h2 = dot(hvec, hvec);
    var col = vec3f(0.0);
    var tr = 1.0;
    var through = false;
    var rmin = 1e9;
    let n = select(150, 220, ctx.detail > 1u);
    for (var i = 0; i < 220; i++) {
        if (i >= n) { break; }
        let r2 = dot(pos, pos);
        let r = sqrt(r2);
        rmin = min(rmin, r);
        if (r < 1.0) { through = true; break; }
        if (r > DIST * 1.5 && dot(pos, dir) > 0.0) { break; }
        // the steps shrink near the throat and near the ring's plane
        let dt = clamp(0.07 * r * min(1.0, 0.25 + abs(pos.y) * 0.8), 0.015, 1.2);
        // bending: a softened inverse-cube pull toward the throat
        let acc = -0.75 * h2 * pos / pow(r2 + 0.3, 2.5);
        dir += acc * dt;
        pos += dir * dt;
        // the plasma ring and the dust it lights
        if (abs(pos.y) < 0.6 && r < 2.6 && r > 1.0) {
            let d = plasma(pos, ctx.t, ctx) * (0.5 + ctx.params.y);
            if (d > 0.001) {
                // the ring breathes: hot spots come and go
                let pulse = 0.85 + 0.3 * sin(ctx.t * 0.7 + atan2(pos.z, pos.x) * 2.0);
                let lit = l.plasma * (0.6 + 2.0 * exp(-(r - 1.0) * 1.5)) * pulse;
                let a = 1.0 - exp(-d * dt * 2.0);
                col += tr * lit * a;
                tr *= 1.0 - a * 0.7;
            }
        }
        // a faint halo of dust round the mouth
        col += tr * l.haze * exp(-(r - 1.0) * 2.5) * dt * 0.03 * (0.5 + ctx.params.z);
        if (tr < 0.01) { break; }
    }
    let rd = normalize(dir);
    var sky: vec3f;
    if (through) {
        // where the ray entered the throat decides what it sees beyond:
        // the throat is a window, not a pinhole
        let fd = normalize(rd * 0.55 + normalize(pos) * 0.45);
        sky = far_sky(fd, l, ctx) * 1.3;
    } else {
        sky = near_sky(rd, l, ctx);
        // the Einstein ring: rays that wound round the throat before leaving
        let ring = exp(-sq((rmin - 1.05) / 0.07));
        sky += (l.nebula * 0.5 + vec3f(0.35)) * ring * 1.2;
    }
    col += tr * sky;
    // focus: the mouth brightens a touch
    col *= 1.0 + 0.06 * ctx.focus_pulse * ctx.reaction;
    return col * exp2(l.exposure);
}
