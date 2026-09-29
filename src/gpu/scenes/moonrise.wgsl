//! name: moonrise
//! title: Moonrise over the Ocean
//! category: space
//! tags: moon, ocean, night, glitter, clouds, pine
//! desc: a huge orange moon lifting off the sea, its glittering path running in to a dark shore
//! themes: harvest, full, crescent
//! uses: camera, noise, stars, water
//! cost: light
//! fallback: ocean
//! credits: original

// The camera stands on a headland 14 m above the sea, looking east. The sky,
// the moon and its clouds live in direction space; the ocean is the sea
// plane under a sum of swells, lit by the moon (a glitter path of facets
// that tilt toward it) and by the sky it reflects. The shore rocks and the
// windswept pine are silhouettes in the frame, rimmed by moonlight. The
// moon is drawn at about four times its real size, as a long lens sees it,
// flattened and reddened by the air when it sits on the horizon.

struct Look {
    moon_alt: f32,    // degrees at t = 0
    rise: f32,        // degrees per second (slowed for the wallpaper)
    phase: f32,       // 0.5 full .. 0.12 thin crescent
    moon_c: vec3f,    // moonlight at the top of the air
    sky_hi: vec3f,
    sky_lo: vec3f,
    stars: f32,
    clouds: f32,
    dusk: f32,        // Earth's shadow and the Belt of Venus (moonrise at dusk)
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(13.0, 0.0006, 0.5, vec3f(0.95, 0.95, 0.9), vec3f(0.008, 0.014, 0.03),
                vec3f(0.03, 0.042, 0.068), 0.35, 0.6, 0.0, -0.3);
        }
        case 2u: {
            return Look(7.0, 0.0006, 0.13, vec3f(0.9, 0.9, 0.85), vec3f(0.004, 0.006, 0.014),
                vec3f(0.012, 0.016, 0.03), 1.0, 0.35, 0.0, 0.3);
        }
        default: {
            return Look(0.35, 0.0014, 0.5, vec3f(1.0, 0.97, 0.9), vec3f(0.012, 0.022, 0.06),
                vec3f(0.035, 0.045, 0.075), 0.25, 0.45, 1.0, -0.4);
        }
    }
}

const RO: vec3f = vec3f(0.0, 14.0, 0.0);
const MOON_AZ: f32 = 9.0;
const MOON_R: f32 = 0.023;      // ~1.3 degrees: a long-lens moon

fn dir_of(alt_deg: f32, az_deg: f32) -> vec3f {
    let a = radians(alt_deg);
    let z = radians(az_deg);
    return vec3f(sin(z) * cos(a), sin(a), -cos(z) * cos(a));
}

// transmittance of the air toward elevation `e` (radians): the moon (and
// its light) goes orange, then red, as it sinks
fn air_tr(e: f32) -> vec3f {
    let m = 1.0 / (sin(max(e, 0.0)) + 0.15 * pow(max(e * 57.3, 0.0) + 3.885, -1.253));
    return exp(-vec3f(0.035, 0.075, 0.17) * m * 1.25);
}

struct Moon {
    dir: vec3f,
    col: vec3f,   // light it casts, after the air
}

fn moon_of(l: Look, t: f32) -> Moon {
    let alt = l.moon_alt + t * l.rise;
    let d = dir_of(alt, MOON_AZ + t * l.rise * 0.6);
    let lit = select(1.0, 0.16, l.phase < 0.3);
    return Moon(d, l.moon_c * air_tr(radians(alt)) * lit);
}

// the moon's disc: oblate near the horizon, limb-darkened, with maria,
// lit by the phase (earthshine on the dark part)
fn moon_disc(rd: vec3f, m: Moon, l: Look, ctx: Ctx) -> vec3f {
    let right = normalize(cross(m.dir, vec3f(0.0, 1.0, 0.0)));
    let up = cross(right, m.dir);
    var q = vec2f(dot(rd - m.dir, right), dot(rd - m.dir, up)) / MOON_R;
    // refraction squashes it on the horizon
    let squash = 1.0 + 0.16 * exp(-max(m.dir.y, 0.0) * 40.0);
    q.y *= squash;
    let r2 = dot(q, q);
    let aa = ctx.px / MOON_R * 1.2;
    let disk = sstep(1.0 + aa, 1.0 - aa, sqrt(r2));
    if (disk <= 0.0) { return vec3f(0.0); }
    let z = sqrt(max(1.0 - r2, 0.0));
    // maria: dark basalt seas, placed roughly as on the real near side
    var mar = 0.0;
    mar += sstep(0.33, 0.12, length(q - vec2f(-0.25, 0.38)));    // Imbrium
    mar += sstep(0.24, 0.08, length(q - vec2f(0.28, 0.35)));     // Serenitatis
    mar += sstep(0.26, 0.08, length(q - vec2f(0.42, 0.08)));     // Tranquillitatis
    mar += sstep(0.4, 0.1, length(q - vec2f(-0.5, -0.05))) * 0.8;   // Procellarum
    mar += sstep(0.16, 0.05, length(q - vec2f(0.62, 0.3)));      // Crisium
    mar += sstep(0.2, 0.06, length(q - vec2f(-0.15, -0.45))) * 0.6; // Nubium
    let tex = noise_fbm2(q * 4.0 + 3.0, 4);
    let alb = mix(1.0, 0.62, saturate(mar * (0.7 + 0.5 * tex))) * (0.9 + 0.2 * noise_value2(q * 14.0));
    // Tycho's bright spot
    let ty = exp(-dot(q - vec2f(-0.1, -0.72), q - vec2f(-0.1, -0.72)) * 300.0) * 0.4;
    let limb = 0.55 + 0.45 * pow(z, 0.5);
    // phase: the sun lights it from the right (waxing)
    let a = (l.phase - 0.5) * TAU;
    let sdir = normalize(vec3f(sin(-a), 0.05, cos(a)));
    let n = vec3f(q.x, q.y / squash, z);
    let lit = sstep(-0.04, 0.08, dot(n, sdir));
    let earthshine = 0.02;
    let bright = (alb + ty) * limb * (lit + earthshine);
    return m.col * bright * disk * 5.5;
}

fn clouds(rd: vec3f, m: Moon, l: Look, ctx: Ctx) -> vec4f {
    if (rd.y < -0.01) { return vec4f(0.0); }
    let y = max(rd.y, 0.0);
    // a thin stratus band hugging the horizon, and scattered higher cloud
    let hp = rd.xz / (y + 0.06);
    let drift = vec2f(ctx.t * 0.0025, ctx.t * 0.0008);
    let n1 = noise_fbm2(hp * vec2f(1.1, 3.5) + drift + vec2f(2.0, 7.0), 5);
    let band = exp(-sq((y - 0.022) / 0.012)) * sstep(0.45, 0.65, noise_fbm2(vec2f(rd.x * 9.0 + ctx.t * 0.004, y * 60.0), 4));
    let high = sstep(0.62 - l.clouds * 0.12, 0.8, n1) * sstep(0.03, 0.12, y) * l.clouds;
    let dens = saturate(max(band * 0.9, high));
    // moonlit rims: thin edges toward the moon glow silver
    let mu = saturate(dot(rd, m.dir));
    let edge = saturate(1.0 - dens * 1.6);
    let rim = pow(mu, 12.0) * edge * 3.0 + pow(mu, 3.0) * 0.4;
    let base = l.sky_lo * 0.5 + m.col * 0.01;
    let c = base + m.col * (0.018 + 0.08 * rim);
    return vec4f(c, dens);
}

fn sky(rd: vec3f, m: Moon, l: Look, ctx: Ctx, with_moon: bool) -> vec3f {
    let y = max(rd.y, 0.0);
    var c = mix(l.sky_lo, l.sky_hi, pow(y, 0.4));
    if (l.dusk > 0.0) {
        // opposite the set sun: the blue-grey shadow of the Earth on the
        // horizon, the pink Belt of Venus above it, then the evening blue
        let shadow = vec3f(0.05, 0.065, 0.1);
        let belt = vec3f(0.16, 0.085, 0.1);
        let blue = vec3f(0.035, 0.06, 0.14);
        let e = y * 57.3;
        var dc = mix(shadow, belt, sstep(3.0, 9.0, e));
        dc = mix(dc, blue, sstep(10.0, 24.0, e));
        dc = mix(dc, l.sky_hi, sstep(24.0, 60.0, e));
        c = mix(c, dc, l.dusk);
    }
    // scattered moonlight: a broad glow around the moon, warm when it is low
    let mu = saturate(dot(rd, m.dir));
    c += m.col * (0.012 * pow(mu, 6.0) + 0.05 * pow(mu, 60.0) + 0.3 * pow(mu, 900.0));
    // stars, fewer near the bright moon
    let wash = saturate(pow(mu, 4.0) * 1.5);
    c += star_field(rd, l.stars, ctx) * 0.6 * (1.0 - wash) * exp(-0.12 / (y + 0.03));
    if (with_moon) {
        c += moon_disc(rd, m, l, ctx);
    }
    let cl = clouds(rd, m, l, ctx);
    c = mix(c, cl.rgb, cl.a);
    return c;
}

// the shore: a rocky headland in the lower left corner, dropping steeply
// into the sea; a few rocks awash further out
fn rocks(p: vec2f) -> f32 {
    let x = p.x;
    // the headland: a knob at x = -0.62 where the pine grows, falling away
    // stepped ledges: the top, a shoulder, then the fall into the surf
    var head = -0.16 + 0.03 * exp(-sq((x + 0.62) / 0.1)) - 0.08 * sstep(-0.62, -0.95, x);
    head -= 0.12 * sstep(-0.47, -0.42, x) + 0.1 * sstep(-0.36, -0.3, x) + 0.4 * sstep(-0.28, -0.16, x);
    let lump = 0.05 * (noise_fbm2(vec2f(x * 6.0, 1.0), 5) - 0.5) + 0.02 * (noise_ridged2(vec2f(x * 22.0, 2.0), 3) - 0.5);
    return head + lump;
}

// signed distance-ish field of the pine (negative inside), p in frame units
fn pine(p: vec2f) -> f32 {
    let root = vec2f(-0.62, -0.135);
    // a leaning trunk that bends toward the sea
    let t = saturate((p.y - root.y) / 0.36);
    let cx = root.x + 0.07 * t * t + 0.02 * sin(t * 5.0);
    var d = abs(p.x - cx) - (0.011 - 0.006 * t);
    d = max(d, max(root.y - 0.02 - p.y, p.y - root.y - 0.34));
    // flat foliage pads, blown to the right
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let h = hash_cell2(vec2i(i, 3), 0x9153u);
        let c = vec2f(cx + (-0.05 + 0.12 * h.x) + fi * 0.012, root.y + 0.2 + fi * 0.035 + 0.02 * h.y);
        let q = (p - c) / vec2f(0.075 + 0.04 * h.z, 0.022 + 0.008 * h.w);
        let ragged = 0.25 * (noise_value2(p * 90.0 + fi * 7.0) - 0.5) + 0.2 * (noise_value2(p * 30.0 + fi) - 0.5);
        let pad = (length(q) - 1.0 + ragged) * 0.03;
        d = min(d, pad);
        // branch to the pad
        let bd = abs(p.y - mix(root.y + 0.1 + fi * 0.04, c.y, saturate((p.x - cx) / max(c.x - cx, 1e-3)))) - 0.004;
        let bx = step(min(cx, c.x), p.x) * step(p.x, max(cx, c.x));
        d = min(d, select(1.0, bd, bx > 0.5));
    }
    return d;
}

// boulders awash off the point: rounded shapes sitting in the sea, so the
// water shows around and below them (negative inside, frame units)
fn boulders(p: vec2f) -> f32 {
    let q1 = (p - vec2f(-0.1, -0.455)) / vec2f(0.065, 0.028);
    let q2 = (p - vec2f(0.07, -0.47)) / vec2f(0.04, 0.02);
    let wob = 0.12 * (noise_value2(p * 60.0) - 0.5);
    let d1 = (length(vec2f(q1.x, max(q1.y, q1.y * 2.2))) - 1.0 + wob) * 0.028;
    let d2 = (length(vec2f(q2.x, max(q2.y, q2.y * 2.2))) - 1.0 + wob) * 0.02;
    return min(d1, d2);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let m = moon_of(l, ctx.t);
    let cam = cam_look_at(RO, RO + vec3f(0.16, 0.035, -1.0), 0.0, 34.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    let tw = water_intersect(RO, rd, 0.0);
    if (tw > 0.0) {
        let hp = RO + rd * tw;
        var n = water_normal(hp.xz, ctx.t, 0.8, tw, ctx);
        // wind chop: small facets that break the moon's path into glints
        let ch = vec2f(noise_grad2(hp.xz * 0.9 + vec2f(ctx.t * 0.4, 0.0)), noise_grad2(hp.xz * 0.9 + vec2f(3.0, ctx.t * 0.35)));
        n = normalize(n + vec3f(ch.x, 0.0, ch.y) * 0.1 * exp(-tw * 0.004));
        var r = reflect(rd, n);
        r.y = abs(r.y) + 0.002;
        var refl = sky(r, m, l, ctx, false);
        // the glitter path: facets tilted toward the moon flash
        let h = normalize(m.dir - rd);
        let nh = saturate(dot(n, h));
        let far = saturate(tw / 3000.0);
        let spec = pow(nh, mix(2200.0, 6000.0, far)) * 700.0 + pow(nh, 350.0) * 0.8;
        refl += m.col * spec * (0.02 + 0.98 * saturate(m.dir.y * 30.0 + 0.3));
        let f = water_fresnel(dot(-rd, n));
        let body = l.sky_lo * 0.12 + m.col * 0.003;
        col = mix(body, refl, f);
        // the air between us and the far sea
        let hz = sky(normalize(vec3f(rd.x, 0.004, rd.z)), m, l, ctx, false);
        col = mix(col, hz, 1.0 - exp(-tw * 0.00012));
    } else {
        col = sky(rd, m, l, ctx, true);
    }
    // ---- the shore, in the frame
    let rk = rocks(p);
    let rock_d = min(p.y - rk, boulders(p));
    let pn = pine(p);
    let rim_dir = normalize(vec2f(0.55, 0.75));
    if (rock_d < 0.02 || pn < 0.02) {
        // rocks: dark, with moonlight catching their upper edges
        let slope = (rocks(p + vec2f(0.004, 0.0)) - rocks(p - vec2f(0.004, 0.0))) / 0.008;
        let edge = exp(rock_d / 0.012);
        let facing = saturate(0.5 - slope * 0.6);
        let crag = noise_fbm2(p * vec2f(26.0, 34.0), 5);
        var rc = l.sky_lo * 0.06 + m.col * 0.012 * (0.4 + 0.6 * crag);
        rc += m.col * 0.12 * edge * facing * (0.5 + crag);
        // surf: foam bursting white at the foot of the rocks, now and then
        let surge = 0.5 + 0.5 * sin(ctx.t * 0.75 + p.x * 3.0);
        let foot = rk - p.y;
        let foam_band = exp(-sq((p.y - (rk - 0.01)) / 0.014)) * sstep(-0.62, -0.3, p.x) * surge;
        let fo = foam_band * sstep(0.45, 0.7, noise_fbm2(p * vec2f(60.0, 90.0) + vec2f(ctx.t * 0.3, 0.0), 4));
        let rocky = sstep(ctx.px, -ctx.px, rock_d);
        col = mix(col, rc, rocky);
        col += m.col * 0.25 * fo * (1.0 - rocky) * sstep(-0.1, 0.0, foot);
        // the pine: a silhouette, faintly rimmed on its sea side
        let pin = sstep(ctx.px, -ctx.px, pn);
        let prim = exp(-abs(pn) / 0.004) * 0.5;
        let pc = l.sky_lo * 0.03 + m.col * 0.012 + m.col * 0.05 * prim * saturate(dot(rim_dir, vec2f(1.0, 0.3)));
        col = mix(col, pc, pin);
    }
    return col * exp2(l.exposure);
}
