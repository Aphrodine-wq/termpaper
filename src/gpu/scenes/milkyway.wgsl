//! name: milkyway
//! title: Milky Way over Joshua Tree
//! category: weather
//! tags: stars, milky way, desert, night, meteors, astrophotography
//! desc: the galactic core rising over Joshua trees and granite boulders in the Mojave
//! themes: summer, winter, moonrise
//! uses: camera, raymarch, noise, stars
//! cost: heavy
//! fallback: starfield
//! credits: original; star positions from public catalogues

// World units are metres, y up; the camera faces south (-z), east is to the
// left. The sky is a long exposure: the Milky Way is modelled in galactic
// coordinates (bulge, star clouds, the Great Rift and dust lanes, a few
// emission nebulae), bright stars sit at their real positions, and the whole
// sky turns about the north celestial pole at twice real time. The ground is
// raymarched: a Joshua tree against the band, granite boulders, the desert
// floor lit by starlight, faint low-level light, or the rising moon.

struct Look {
    gc: vec3f,        // toward the galactic centre (t = 0)
    gt: vec3f,        // along the band, toward Cygnus
    gn: vec3f,        // galactic north
    mw: f32,          // Milky Way brightness
    core: f32,        // bulge brightness
    stars: f32,       // star density
    moon: vec3f,      // moon direction (below the horizon when unused)
    moon_up: f32,     // 0 no moon .. 1 up
    glow: vec3f,      // airglow / horizon colour
    lll: f32,         // low-level foreground light
    orion: f32,       // show the winter constellations
    exposure: f32,
}

fn altaz(alt_deg: f32, az_deg: f32) -> vec3f {
    let a = radians(alt_deg);
    let z = radians(az_deg);
    return vec3f(sin(z) * cos(a), sin(a), -cos(z) * cos(a));
}

fn band(gc: vec3f, up: vec3f) -> mat3x3f {
    let t = normalize(up - gc * dot(up, gc));
    return mat3x3f(gc, t, normalize(cross(gc, t)));
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            // winter: the faint outer arm beside Orion, no core
            let b = band(altaz(-30.0, -60.0), altaz(60.0, -35.0));
            l = Look(b[0], b[1], b[2], 0.35, 0.0, 0.42, altaz(-20.0, -70.0), 0.0,
                vec3f(0.006, 0.009, 0.006), 1.0, 1.0, 0.0);
        }
        case 2u: {
            // moonrise: the core fading as the moon clears the eastern ridge
            let b = band(altaz(13.0, -8.0), altaz(62.0, -58.0));
            l = Look(b[0], b[1], b[2], 0.45, 0.45, 0.3, altaz(6.0, -34.0), 1.0,
                vec3f(0.01, 0.011, 0.012), 0.0, 0.0, -0.1);
        }
        default: {
            // summer: the core low in the south-southeast, band arching up
            let b = band(altaz(13.0, -8.0), altaz(62.0, -58.0));
            l = Look(b[0], b[1], b[2], 1.0, 1.0, 0.4, altaz(-20.0, -70.0), 0.0,
                vec3f(0.006, 0.01, 0.006), 1.0, 0.0, 0.0);
        }
    }
    return l;
}

const RO: vec3f = vec3f(0.0, 1.5, 0.0);
const LAT: f32 = 34.0;
const SKY_RATE: f32 = 2.0;

// the sky turns about the north celestial pole (behind the camera)
fn celestial_turn(rd: vec3f, t: f32) -> vec3f {
    let a = radians(LAT);
    let axis = vec3f(0.0, sin(a), cos(a));
    let ang = -t * SKY_RATE * TAU / 86164.0;
    let c = cos(ang);
    let s = sin(ang);
    return rd * c + cross(axis, rd) * s + axis * dot(axis, rd) * (1.0 - c);
}

// ---------------------------------------------------------------- galaxy

fn galaxy(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let x = dot(rd, l.gc);
    let y = dot(rd, l.gt);
    let z = dot(rd, l.gn);
    let gl = atan2(y, x);                    // galactic longitude, 0 at the core
    let gb = asin(clamp(z, -1.0, 1.0));     // latitude
    let al = abs(gl);
    let q = vec2f(gl, gb);
    // the disc thickens toward the core
    let thick = 0.09 + 0.13 * exp(-al * al * 2.0);
    let disc = exp(-sq(gb / thick)) * (0.2 + 0.8 * exp(-al * 1.2));
    let halo = exp(-sq(gb / (thick * 3.2))) * (0.1 + 0.25 * exp(-al * 1.5));
    let bulge = exp(-(sq(gl / 0.26) + sq(gb / 0.19))) * l.core;
    // star clouds: mottled at every scale
    let cloud = noise_fbm2(q * vec2f(7.0, 12.0) + vec2f(2.0, 5.0), 5);
    let fine = noise_fbm2(q * vec2f(38.0, 52.0) + vec2f(7.0, 1.0), 3);
    var lum = disc * (0.35 + 1.1 * cloud * cloud) * (0.75 + 0.5 * fine) + halo * 0.5;
    lum += bulge * (0.9 + 0.6 * cloud) * 1.6;
    // dust: the Great Rift runs up the band from the core toward Cygnus,
    // with lanes and knots everywhere near the plane
    let rift_b = 0.03 + 0.025 * sin(gl * 2.3 + 0.6) + 0.02 * (noise_value2(vec2f(gl * 6.0, 3.0)) - 0.5);
    let rift_w = 0.018 + 0.022 * saturate(gl);
    let rift = exp(-sq((gb - rift_b) / rift_w)) * sstep(-0.25, 0.1, gl) * sstep(2.2, 1.3, gl);
    let dn = noise_fbm2(q * vec2f(4.5, 30.0) + vec2f(11.0, 4.0), 5);
    let lanes = sstep(0.48, 0.66, dn) * exp(-sq(gb / (thick * 0.8)));
    let absorb = saturate(rift * (0.55 + 0.6 * dn) + lanes * 0.75);
    let warm = saturate(bulge * 1.4 + exp(-al * 2.0) * 0.35);
    var c = mix(vec3f(0.62, 0.67, 0.82), vec3f(1.0, 0.8, 0.58), warm) * lum;
    // dust reddens what it does not block
    c *= (1.0 - absorb) * mix(vec3f(1.0), vec3f(1.0, 0.86, 0.72), saturate(absorb * 1.5));
    // emission nebulae near the core: Lagoon, Trifid, Eagle, Swan
    var neb = 0.0;
    neb += exp(-(sq(gl - 0.105) + sq(gb + 0.021)) / 0.00012);
    neb += 0.6 * exp(-(sq(gl - 0.122) + sq(gb + 0.005)) / 0.00006);
    neb += 0.5 * exp(-(sq(gl - 0.297) + sq(gb - 0.014)) / 0.00008);
    neb += 0.5 * exp(-(sq(gl - 0.264) + sq(gb + 0.012)) / 0.00008);
    c += vec3f(0.9, 0.28, 0.42) * neb * 0.6 * l.core;
    // the band is full of faint unresolved stars
    c += star_layer(rd, 500.0, 0.9 * saturate(disc * 2.0 + bulge), ctx, 0x6a1au) * 0.5;
    return c * l.mw * 0.16;
}

// A bright star: tight core plus a small soft glow, so it survives the
// terminal's resolution without turning into a blob.
fn bright_star(rd: vec3f, dir: vec3f, mag: f32, kelvin: f32, ctx: Ctx) -> vec3f {
    let d = length(rd - dir);
    let w = max(ctx.px * 0.8, 0.0006);
    let flux = pow(2.512, -mag);
    let core = exp(-sq(d / w));
    let glow = exp(-d / (w * 3.0)) * 0.08;
    return mix(vec3f(1.0), col_kelvin(kelvin), 0.7) * flux * (core + glow) * 1.4;
}

// star at tangent-plane offset (x east-positive-left, y north), degrees
fn cat_dir(c: vec3f, e: vec3f, n: vec3f, x: f32, y: f32) -> vec3f {
    return normalize(c + e * radians(x) + n * radians(y));
}

fn winter_stars(rd: vec3f, ctx: Ctx) -> vec3f {
    // Orion due south-southwest, Sirius below-left, Aldebaran and the
    // Pleiades up to the right (sky frame at t = 0)
    let c = altaz(34.0, 12.0);
    let e2 = -normalize(cross(c, vec3f(0.0, 1.0, 0.0)));
    let n = normalize(cross(c, e2));
    let east = -e2;                                      // east = left
    var s = vec3f(0.0);
    s += bright_star(rd, cat_dir(c, east, n, -4.8, 8.4), 0.5, 3500.0, ctx);   // Betelgeuse
    s += bright_star(rd, cat_dir(c, east, n, 2.7, 7.3), 1.6, 20000.0, ctx);   // Bellatrix
    s += bright_star(rd, cat_dir(c, east, n, -1.2, -0.9), 1.8, 25000.0, ctx); // Alnitak
    s += bright_star(rd, cat_dir(c, east, n, 0.0, -0.2), 1.7, 25000.0, ctx);  // Alnilam
    s += bright_star(rd, cat_dir(c, east, n, 1.0, 0.7), 2.2, 25000.0, ctx);   // Mintaka
    s += bright_star(rd, cat_dir(c, east, n, -3.0, -8.7), 2.1, 22000.0, ctx); // Saiph
    s += bright_star(rd, cat_dir(c, east, n, 5.4, -7.2), 0.1, 12000.0, ctx);  // Rigel
    s += bright_star(rd, cat_dir(c, east, n, -16.6, -15.7), -1.4, 9900.0, ctx); // Sirius
    s += bright_star(rd, cat_dir(c, east, n, 14.4, 17.5), 0.9, 3900.0, ctx);  // Aldebaran
    // the Orion Nebula, pink under the belt
    let m42 = cat_dir(c, east, n, 0.2, -4.4);
    s += vec3f(0.8, 0.35, 0.5) * exp(-sq(length(rd - m42) / 0.009)) * 0.05;
    // the Pleiades: a little knot of blue stars
    let m45 = cat_dir(c, east, n, 24.8, 25.0);
    let pd = length(rd - m45);
    if (pd < 0.04) {
        for (var i = 0; i < 6; i++) {
            let h = hash_cell2(vec2i(i, 45), 0x45u);
            let o = cat_dir(c, east, n, 24.8 + (h.x - 0.5) * 1.6, 25.0 + (h.y - 0.5) * 1.1);
            s += bright_star(rd, o, 3.0 + h.z * 1.5, 14000.0, ctx);
        }
        s += vec3f(0.5, 0.6, 1.0) * exp(-sq(pd / 0.015)) * 0.012;
    }
    return s;
}

fn summer_stars(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    // Antares, the red heart of Scorpius, right of the core; and the
    // teapot of Sagittarius just left of it (galactic offsets, degrees)
    var s = vec3f(0.0);
    s += bright_star(rd, cat_dir(l.gc, l.gt, l.gn, -8.0, 15.0), 1.0, 3400.0, ctx);  // Antares
    s += bright_star(rd, cat_dir(l.gc, l.gt, l.gn, 2.5, -8.5), 1.8, 11000.0, ctx);  // Kaus Australis
    s += bright_star(rd, cat_dir(l.gc, l.gt, l.gn, 8.0, -10.5), 2.0, 18000.0, ctx); // Nunki
    s += bright_star(rd, cat_dir(l.gc, l.gt, l.gn, 5.0, -5.0), 2.7, 4500.0, ctx);   // Kaus Borealis
    s += bright_star(rd, cat_dir(l.gc, l.gt, l.gn, 7.5, -15.5), 2.6, 9500.0, ctx);  // Ascella
    s += bright_star(rd, cat_dir(l.gc, l.gt, l.gn, -14.0, -3.0), 1.6, 25000.0, ctx); // Shaula
    return s;
}

fn sky(rd_world: vec3f, l: Look, ctx: Ctx, with_stars: bool) -> vec3f {
    let alt = max(rd_world.y, 0.0);
    let rd = celestial_turn(rd_world, ctx.t);
    // the night sky itself: faint blue-grey, brighter toward the horizon
    var c = mix(vec3f(0.012, 0.014, 0.022), vec3f(0.006, 0.007, 0.013), sqrt(alt));
    // airglow: a greenish band ten degrees up
    c += l.glow * exp(-sq((alt - 0.12) / 0.12)) * 1.0;
    // light pollution dome from the towns to the west (right)
    let az = atan2(rd_world.x, -rd_world.z);
    c += vec3f(0.035, 0.018, 0.008) * exp(-alt / 0.07) * exp(-sq((az - 1.1) / 0.5));
    // the moon brightens the whole sky around it, washing the stars out
    var wash = 0.0;
    if (l.moon_up > 0.0) {
        let mu = saturate(dot(rd_world, l.moon));
        let sky_m = vec3f(0.03, 0.045, 0.08) * (0.35 + 0.65 * exp(-alt * 2.5)) + vec3f(0.12, 0.07, 0.035) * pow(mu, 10.0);
        c += sky_m * l.moon_up;
        wash = saturate(0.5 + 0.5 * pow(mu, 3.0)) * l.moon_up;
    }
    let stars_vis = 1.0 - 0.7 * wash;
    // the band and the stars, dimmed by the atmosphere near the horizon
    let ext = exp(-0.12 / max(alt + 0.02, 0.02));
    if (with_stars) {
        c += galaxy(rd, l, ctx) * ext * (1.0 - 0.75 * wash);
        var st = star_field(rd, l.stars, ctx) * 0.6;
        if (l.orion > 0.0) {
            st += winter_stars(rd, ctx);
        } else {
            st += summer_stars(rd, l, ctx);
        }
        c += st * ext * stars_vis;
    }
    return c;
}

// meteors: now and then a streak burns across the sky (screen space)
fn meteor(p: vec2f, rd: vec3f, ctx: Ctx) -> vec3f {
    let ev = hash_event(ctx.t, 19.0, 0x3e7eu);
    if (ev.x > 0.55 || rd.y < 0.02) { return vec3f(0.0); }
    let dur = 0.12 + 0.06 * ev.x;
    if (ev.y > dur) { return vec3f(0.0); }
    let h = hash_cell2(vec2i(i32(ev.z), 3), 0x3e7fu);
    let a = vec2f((h.x - 0.5) * 1.4, 0.05 + 0.4 * h.y);
    let ang = -0.4 - 1.2 * h.z + select(0.0, PI, h.w > 0.5) * 0.0;
    let dir = vec2f(cos(ang), sin(ang)) * select(1.0, -1.0, h.w > 0.5);
    let len = 0.18 + 0.2 * h.z;
    let k = ev.y / dur;                     // 0..1 through its life
    let head = a + dir * len * k;
    let tail = a + dir * len * max(k - 0.35, 0.0);
    let pa = p - tail;
    let ba = head - tail;
    let s = saturate(dot(pa, ba) / max(dot(ba, ba), 1e-6));
    let d = length(pa - ba * s);
    let w = max(ctx.px * 0.7, 0.0015);
    let bright = sin(k * PI) * (0.3 + 0.7 * s);
    return vec3f(0.75, 1.0, 0.85) * exp(-sq(d / w)) * bright * 0.8;
}

// a satellite crossing slowly, once every couple of minutes
fn satellite(p: vec2f, ctx: Ctx) -> vec3f {
    let ev = hash_event(ctx.t, 140.0, 0x5a7u);
    if (ev.x > 0.7) { return vec3f(0.0); }
    let k = ev.y * 140.0 / 45.0;            // crossing takes 45 s
    if (k > 1.0) { return vec3f(0.0); }
    let h = hash_cell2(vec2i(i32(ev.z), 9), 0x5a8u);
    let a = vec2f(-1.1, 0.05 + 0.3 * h.x);
    let b = vec2f(1.1, 0.2 + 0.3 * h.y);
    let pos = mix(a, b, k);
    let d = length(p - pos);
    let w = max(ctx.px * 0.6, 0.0012);
    // it fades as it slides into Earth's shadow
    let fade = sstep(1.0, 0.7, k) * sstep(0.0, 0.05, k);
    return vec3f(1.0) * exp(-sq(d / w)) * 0.25 * fade;
}

// ---------------------------------------------------------------- ground

fn ground_h(xz: vec2f) -> f32 {
    return 0.6 * noise_grad2(xz * 0.018) + 0.15 * noise_grad2(xz * 0.09 + 5.0) + 0.04 * noise_grad2(xz * 0.5);
}

// a granite boulder pile: rounded, weathered blocks stacked on each other,
// with dark cracks between them
fn boulders(p: vec3f) -> f32 {
    let c = vec3f(10.5, 0.0, -24.0);
    let q = p - c;
    let bc = length(q - vec3f(0.0, 2.5, 0.0));
    if (bc > 12.0) { return bc - 10.0; }
    var d = 1e5;
    for (var i = 0; i < 11; i++) {
        let h = hash_cell2(vec2i(i, 1), 0xb01du);
        let tier = select(select(0.0, 1.0, i >= 5), 2.0, i >= 9);
        let spread = 12.0 - tier * 4.0;
        // the pile leans: higher blocks sit off to the left
        let o = vec3f((h.x - 0.5) * spread - tier * 1.6, 0.0, (h.y - 0.5) * spread * 0.6);
        let r = vec3f(1.7 + 0.8 * h.w, 1.2 + 0.5 * h.x, 1.6) * (0.8 + 0.8 * h.z) * (1.0 - tier * 0.15);
        let lift = r.y * 0.72 + tier * 2.2;
        d = op_smin(d, sdf_ellipsoid(q - o - vec3f(0.0, lift, 0.0), r), 0.12);
    }
    return d + 0.1 * noise_grad3(p * 0.8) + 0.04 * noise_grad3(p * 2.7);
}

fn capsule_min(d: f32, p: vec3f, a: vec3f, b: vec3f, r: f32) -> f32 {
    return min(d, sdf_capsule(p, a, b, r));
}

// a leaf rosette: a starburst of stiff spikes, pointing along `ax`
fn rosette(p: vec3f, c: vec3f, ax: vec3f, near: bool) -> f32 {
    let q = p - c - ax * 0.15;
    let r = length(q);
    if (r > 0.95) { return r - 0.8; }
    let dn = q / max(r, 1e-4);
    // spikes: sharp peaks at scattered directions (cells on the sphere)
    var spike = 0.5;
    if (near) {
        let w = noise_worley3(dn * 3.2 + c * 7.0);
        spike = pow(saturate(1.0 - w.x * 1.7), 2.5);
    }
    let reach = 0.2 + 0.55 * spike * (0.55 + 0.45 * saturate(dot(dn, ax) + 0.6));
    return (r - reach) * 0.45;
}

// A Joshua tree: a shaggy trunk forking into crooked arms, each arm
// forking again and ending in a spiky rosette. q in tree space (base at
// the origin, metres); returns (distance, material)
fn joshua(q: vec3f, s: f32, near: bool) -> vec2f {
    let p = q / s;
    let bc = length(p - vec3f(0.2, 3.0, 0.0));
    if (bc > 3.9) { return vec2f((bc - 3.6) * s, 3.0); }
    let f = vec3f(0.18, 2.0, 0.05);
    let a1 = vec3f(-0.8, 2.75, 0.25);
    let a2 = vec3f(1.05, 2.55, -0.2);
    let a3 = vec3f(0.35, 3.25, 0.5);
    let b1 = vec3f(-1.5, 3.05, 0.1);
    let t1 = vec3f(-2.05, 3.75, 0.0);
    let t2 = vec3f(-1.2, 3.95, 0.3);
    let t3 = vec3f(-0.55, 4.1, -0.1);
    let t4 = vec3f(1.95, 3.35, -0.3);
    let t5 = vec3f(1.25, 3.7, 0.1);
    let t6 = vec3f(0.55, 4.55, 0.6);
    let t7 = vec3f(-0.95, 1.95, 0.6);
    let t8 = vec3f(1.55, 2.2, 0.55);
    // the trunk wears a skirt of dead leaves: thick and rough
    var d = sdf_capsule(p, vec3f(0.0, 0.0, 0.0), f, 0.24) + 0.03 * sin(p.y * 23.0 + atan2(p.z, p.x) * 5.0);
    d = capsule_min(d, p, f, a1, 0.15);
    d = capsule_min(d, p, f, a2, 0.15);
    d = capsule_min(d, p, f, a3, 0.14);
    d = capsule_min(d, p, a1, b1, 0.12);
    d = capsule_min(d, p, b1, t1, 0.1);
    d = capsule_min(d, p, b1, t2, 0.1);
    d = capsule_min(d, p, a1, t3, 0.1);
    d = capsule_min(d, p, a2, t4, 0.1);
    d = capsule_min(d, p, a2, t5, 0.1);
    d = capsule_min(d, p, a3, t6, 0.09);
    d = capsule_min(d, p, vec3f(0.08, 1.2, 0.03), t7, 0.09);
    d = capsule_min(d, p, vec3f(0.12, 1.5, 0.04), t8, 0.09);
    var lv = rosette(p, t1, normalize(t1 - b1), near);
    lv = min(lv, rosette(p, t2, normalize(t2 - b1), near));
    lv = min(lv, rosette(p, t3, normalize(t3 - a1), near));
    lv = min(lv, rosette(p, t4, normalize(t4 - a2), near));
    lv = min(lv, rosette(p, t5, normalize(t5 - a2), near));
    lv = min(lv, rosette(p, t6, normalize(t6 - a3), near));
    lv = min(lv, rosette(p, t7, normalize(t7 - vec3f(0.08, 1.2, 0.03)), near));
    lv = min(lv, rosette(p, t8, normalize(t8 - vec3f(0.12, 1.5, 0.04)), near));
    return select(vec2f(d * s, 3.0), vec2f(lv * s, 4.0), lv < d);
}

fn trees(p: vec3f) -> vec2f {
    // the hero tree left of centre, two smaller ones further out
    var r = joshua(p - vec3f(-3.3, -0.1, -10.5), 1.35, true);
    let q2 = p - vec3f(6.5, 0.0, -38.0);
    r = op_umin(r, joshua(vec3f(-q2.x, q2.y, q2.z), 0.9, false));
    let q3 = p - vec3f(-22.0, 0.0, -55.0);
    r = op_umin(r, joshua(vec3f(q3.z, q3.y, -q3.x), 1.0, false));
    return r;
}

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    var r = vec2f((p.y - ground_h(p.xz)) * 0.8, 1.0);
    if (p.y < 12.0) {
        r = op_umin(r, vec2f(boulders(p) * 0.8, 2.0));
        r = op_umin(r, trees(p));
    }
    return r;
}

// the distant range along the southern horizon, as an angular profile
fn ridge_alt(az: f32) -> f32 {
    return 0.035 + 0.028 * noise_fbm2(vec2f(az * 2.4, 1.0), 5) + 0.02 * noise_ridged2(vec2f(az * 5.0, 3.0), 4)
        - 0.012 * sstep(-0.2, 0.6, az);
}

fn soft_shadow(ro: vec3f, rd: vec3f, ctx: Ctx) -> f32 {
    var res = 1.0;
    var t = 0.1;
    for (var i = 0; i < 40; i++) {
        let h = map(ro + rd * t, ctx).x;
        res = min(res, 8.0 * h / t);
        if (res < 0.02 || t > 60.0) { break; }
        t += clamp(h, 0.1, 2.5);
    }
    return saturate(res);
}

fn shade(p: vec3f, rd: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = rm_normal(p, t, ctx);
    var alb = vec3f(0.3, 0.24, 0.17);                         // decomposed granite sand
    if (mat > 1.5 && mat < 2.5) { alb = vec3f(0.36, 0.3, 0.24) * (0.8 + 0.4 * noise_value2(p.xz * 2.0 + p.y)); }
    if (mat > 2.5 && mat < 3.5) { alb = vec3f(0.09, 0.075, 0.06); }  // shaggy bark
    if (mat > 3.5) { alb = vec3f(0.1, 0.13, 0.06); }                // yucca leaves
    // skylight: starlight and airglow from above
    let sky_amb = vec3f(0.0095, 0.011, 0.017) * (0.6 + 0.4 * n.y);
    var c = alb * sky_amb * 2.0;
    // soft low-level light from a lamp off to the camera's left
    if (l.lll > 0.0) {
        let ld = normalize(vec3f(-0.7, 0.3, 0.65));
        c += alb * col_kelvin(3600.0) * saturate(dot(n, ld)) * l.lll * 0.02;
    }
    // the rising moon rakes the desert from the east
    if (l.moon_up > 0.0) {
        let ml = normalize(l.moon + vec3f(0.0, 0.04, 0.0));
        let ndl = saturate(dot(n, ml));
        if (ndl > 0.0) {
            c += alb * vec3f(0.5, 0.36, 0.22) * ndl * soft_shadow(p + n * 0.05, ml, ctx) * l.moon_up * 0.5;
        }
        c += alb * vec3f(0.02, 0.028, 0.045) * (0.6 + 0.4 * n.y) * l.moon_up;
    }
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let cam = cam_look_at(RO, RO + vec3f(0.0, 0.26, -1.0), 0.0, 62.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    var hit = vec2f(-1.0);
    if (rd.y < 0.5) {
        // nothing stands taller than 12 m: rays climbing past that are sky
        let tmax = select(400.0, min(400.0, (12.0 - RO.y) / max(rd.y, 1e-3)), rd.y > 0.0);
        hit = rm_march(RO, rd, 0.1, tmax, steps(120.0, ctx), ctx);
    }
    // haze: faint, the colour of the horizon sky
    let hz = sky(normalize(vec3f(rd.x, 0.03, rd.z)), l, ctx, false);
    if (hit.x < 0.0 && rd.y < -0.005) {
        // a grazing ray that ran out of steps: it ends on the desert floor
        hit = vec2f((RO.y - 0.2) / -rd.y, 1.0);
    }
    if (hit.x > 0.0) {
        col = shade(RO + rd * hit.x, rd, hit.x, hit.y, l, ctx);
        col = mix(col, hz, 1.0 - exp(-hit.x * 0.004));
    } else {
        let az = atan2(rd.x, -rd.z);
        let ra = ridge_alt(az);
        col = sky(rd, l, ctx, true);
        col += meteor(p, rd, ctx) + satellite(p, ctx);
        if (l.moon_up > 0.0) {
            // the moon itself, just clearing the ridge: big, orange, dimmed
            let md = rd - l.moon;
            let mr = 0.011;
            let disk = sstep(mr + ctx.px * 0.7, mr - ctx.px * 0.7, length(md));
            let maria = 0.8 + 0.2 * noise_fbm2(md.xy / mr * 1.5 + 4.0, 3);
            col = mix(col, vec3f(3.2, 1.8, 0.85) * maria, disk);
            col += vec3f(0.35, 0.2, 0.1) * exp(-length(md) / 0.02) * 0.4;
        }
        // the far range: a hazy silhouette against the sky
        let rm = sstep(ctx.px, -ctx.px, rd.y - ra);
        let range_c = mix(hz * 0.55, hz, 0.3) + vec3f(0.002, 0.002, 0.003);
        col = mix(col, range_c, rm);
    }
    return col * exp2(l.exposure);
}
