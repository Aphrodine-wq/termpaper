//! name: paris
//! title: Paris Café Street
//! category: city
//! tags: paris, cafe, eiffel tower, haussmann, cobblestones, rain, autumn
//! desc: a Haussmann street running to the Eiffel Tower, a café glowing under its awning
//! themes: rain, autumn, night
//! uses: camera, raymarch, sky, rain, light, noise, water
//! cost: light
//! tonemap: aces
//! fallback: lanterns
//! credits: original

// World units are metres. The camera stands in a narrow Haussmann street in
// the 7th arrondissement, looking down it (-z) to the Eiffel Tower rising
// above the rooflines at its end. Facades, balcony railings, cornices and
// zinc mansards are analytic planes, so reflections in the wet cobbles are
// just the same intersections from the mirrored ray. The café's tables and
// chairs are the only ray-marched geometry. The low autumn sun's shadow line
// across the facades is closed form: a point is lit if the ray toward the
// sun clears the opposite roofline. The tower sparkles for the first five
// minutes of every scene hour.

const HW: f32 = 7.2;          // facade line, metres from the street axis
const CURB: f32 = 4.8;        // kerb line
const ZEND: f32 = -300.0;     // the street ends at the Champ de Mars
const TZ: f32 = -520.0;       // the tower
const TX: f32 = 2.0;
const LAMP_DZ: f32 = 24.0;

struct Look {
    sun: vec3f,
    sun_c: vec3f,     // direct sun (0 when overcast / night)
    amb: vec3f,       // skylight
    wet: f32,
    rain: f32,
    lamps: f32,       // street lamps and windows on
    tower: f32,       // golden illumination of the tower
    leaves: f32,
    haze: f32,
    hazec: vec3f,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    if (theme == 1u) {
        // autumn, late afternoon: low sun ahead-left, golden light high on
        // the right-hand facades, the street in cool shade
        l.sun = sky_sun_dir(14.0, -16.0);
        l.sun_c = vec3f(1.0, 0.66, 0.36) * 2.6;
        l.amb = vec3f(0.24, 0.25, 0.31);
        l.wet = 0.0;
        l.rain = 0.0;
        l.lamps = 0.15;
        l.tower = 0.0;
        l.leaves = 1.0;
        l.haze = 2.0;
        l.hazec = vec3f(0.6, 0.42, 0.26);
        l.exposure = -0.15;
    } else if (theme == 2u) {
        // night: lamps, the café, the tower in gold
        l.sun = sky_sun_dir(-20.0, 0.0);
        l.sun_c = vec3f(0.0);
        l.amb = vec3f(0.006, 0.008, 0.014);
        l.wet = 0.25;
        l.rain = 0.0;
        l.lamps = 1.0;
        l.tower = 1.0;
        l.leaves = 0.3;
        l.haze = 0.6;
        l.hazec = vec3f(0.012, 0.012, 0.018);
        l.exposure = 0.1;
    } else {
        // rain at blue hour
        l.sun = sky_sun_dir(-5.0, -30.0);
        l.sun_c = vec3f(0.0);
        l.amb = vec3f(0.035, 0.045, 0.07);
        l.wet = 1.0;
        l.rain = 0.7;
        l.lamps = 1.0;
        l.tower = 1.0;
        l.leaves = 0.25;
        l.haze = 2.2;
        l.hazec = vec3f(0.05, 0.06, 0.085);
        l.exposure = 0.0;
    }
    return l;
}

// ------------------------------------------------------------ buildings

struct Bay {
    z0: f32,      // building start (nearer end, larger z)
    len: f32,
    cor: f32,     // cornice height
    tone: vec3f,  // limestone tint
    id: u32,
    shop: f32,    // ground-floor kind
}

// buildings along one side: lots 14-22 m long
fn building(z: f32, side: i32) -> Bay {
    let cell = floor(-z / 18.0);
    let h = hash_cell2(vec2i(i32(cell), side), 0xba1u);
    var b: Bay;
    b.len = 18.0;
    b.z0 = -cell * 18.0;
    b.cor = 19.2 + 1.8 * h.x;
    b.tone = mix(col_hex(0xd9ccb2u), col_hex(0xcfc3ae), h.y) * (0.92 + 0.12 * h.z);
    b.id = hash_u2(vec2u(bitcast<u32>(i32(cell)), u32(side) + 7u));
    b.shop = h.w;
    return b;
}

fn floor_y(i: i32) -> f32 {
    // ground 4.4, entresol 2.6, then 3.3, 3.1, 2.95, 2.85
    switch (i) {
        case 0: { return 0.0; }
        case 1: { return 4.4; }
        case 2: { return 7.0; }
        case 3: { return 10.3; }
        case 4: { return 13.4; }
        case 5: { return 16.35; }
        default: { return 19.2; }
    }
}

// ------------------------------------------------------------ the tower

// the Eiffel Tower seen face on, in its own plane: (coverage, girder mask)
fn eiffel(q: vec2f, lod: f32) -> vec2f {
    let y = q.y;
    let ax = abs(q.x);
    if (y < 0.0 || y > 330.0 || ax > 66.0) { return vec2f(0.0); }
    // outer profile: exponential taper from 62 m half-width at the feet
    let hw = 62.0 * exp(-y / 95.0) + 1.2;
    var d = ax - hw;
    // the great arch under the first floor
    let arch = 37.0 * sqrt(saturate(1.0 - sq(y / 40.0)));
    d = max(d, arch - ax);
    // between the first and second floors the legs still stand apart
    if (y > 60.0 && y < 113.0) {
        let k = (y - 60.0) / 53.0;
        let gap = hw * 0.3 * sqrt(saturate(k * 3.0)) * (1.0 - k);
        d = max(d, gap - ax);
    }
    // antenna above the top floor
    if (y > 276.0) { d = ax - select(4.0, 0.9, y > 282.0); }
    // platforms stick out a little
    let plat = min(abs(y - 58.5) - 2.2, abs(y - 116.0) - 1.6);
    if (plat < 0.0) { d = min(d, ax - hw - 3.0); }
    let a = saturate(0.5 - d / max(lod, 0.5));
    if (a <= 0.0) { return vec2f(0.0); }
    // lattice: outer girders solid, the inside open-work of crosses
    let edge = saturate(1.0 - abs(ax - hw + 1.5) / max(lod, 2.2));
    let cell = 7.0 + y * 0.02;
    let g = fract(vec2f(q.x, y) / cell);
    let cross = min(abs(g.x - g.y), abs(g.x + g.y - 1.0)) * cell;
    let lattice = saturate(1.0 - cross / max(lod * 0.8, 0.9));
    let dens = max(max(edge, lattice * 0.8), mix(0.35, 0.75, saturate(lod / 6.0)));
    let plat_m = select(0.0, 1.0, plat < 0.0);
    return vec2f(a * max(dens, plat_m), max(edge, plat_m));
}

// ------------------------------------------------------------ shading

fn paris_sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let y = max(rd.y, 0.0);
    var c: vec3f;
    if (l.sun_c.x > 0.0) {
        // late afternoon: gold low toward the sun, clear blue overhead
        let mu = saturate(dot(rd, l.sun));
        c = mix(vec3f(1.1, 0.72, 0.4), vec3f(0.22, 0.33, 0.55), pow(saturate(y * 2.2), 0.55));
        c += vec3f(1.4, 0.8, 0.4) * (0.5 * pow(mu, 6.0) + 2.0 * pow(mu, 60.0));
        c += sky_sun_disk(rd, l.sun, 0.6) * 0.3;
    } else if (l.rain > 0.0) {
        // low rain cloud at blue hour, a little of the city in it
        c = mix(vec3f(0.07, 0.075, 0.1), vec3f(0.03, 0.04, 0.07), pow(saturate(y * 2.0), 0.6));
        let n = noise_fbm2(rd.xz / (rd.y + 0.1) * 0.8 + vec2f(ctx.t * 0.004, 0.0), 4);
        c *= 0.8 + 0.4 * n;
    } else {
        c = mix(vec3f(0.02, 0.018, 0.026), vec3f(0.002, 0.003, 0.009), pow(saturate(y * 2.5), 0.5));
    }
    return c;
}

// is this point in the sun? The low sun comes from the left side of the
// street, over the left roofline (closed form)
fn sunlit(p: vec3f, l: Look) -> f32 {
    if (l.sun_c.x <= 0.0) { return 0.0; }
    let s = l.sun;
    if (s.x >= -0.01) { return 1.0; }
    let dist = (p.x + HW) / -s.x;                 // metres to the left facade line
    let q = p + s * dist;
    let b = building(q.z, 0);
    let roof = b.cor + 3.2;
    return smoothstep(-0.4, 0.4, q.y - roof);
}

// warm or dark window of a facade bay; u in bays, v in floors
fn window_light(bx: f32, fl: i32, b: Bay, l: Look, t: f32) -> vec3f {
    let h = hash_cell2(vec2i(i32(floor(bx)) + i32(b.id & 4095u), fl), 0x3e1u);
    let on = h.x < 0.45 * l.lamps + 0.05;
    let c = mix(col_kelvin(2500.0), col_kelvin(3200.0), h.y) * (0.35 + 0.4 * h.z);
    return select(vec3f(0.0), c, on);
}

// the stone facade at p (on the plane |x| = HW), facing normal n
fn shade_facade(p: vec3f, n: vec3f, rd: vec3f, lod: f32, l: Look, ctx: Ctx, side: i32) -> vec3f {
    let b = building(p.z, side);
    let zl = b.z0 - p.z;                           // metres along the building
    let bw = b.len / 6.0;                          // six bays
    let bx = zl / bw;
    let u = fract(bx);
    var fl = 0;
    for (var i = 1; i < 7; i++) { if (p.y >= floor_y(i)) { fl = i; } }
    let y0 = floor_y(fl);
    let y1 = select(floor_y(fl + 1), b.cor, fl >= 5);
    let v = (p.y - y0) / max(y1 - y0, 0.1);
    var alb = b.tone;
    // rusticated ground floor, string courses, carved window surrounds
    alb *= 0.9 + 0.1 * noise_value2(p.zy * vec2f(1.5, 4.0));
    let course = step(abs(p.y - y0 - 0.1), 0.12) * step(1.0, f32(fl));
    alb *= 1.0 - 0.25 * course;
    let party = step(zl, 0.25) + step(b.len - 0.25, zl);
    var emi = vec3f(0.0);
    var glass = 0.0;
    if (fl == 0) {
        // shopfronts: glazed bays with warm interiors, or carriage doors
        let isw = step(0.12, u) * step(u, 0.88) * step(0.35, p.y) * step(p.y, 3.2);
        let fascia = step(3.35, p.y) * step(p.y, 3.95) * step(0.05, u) * step(u, 0.95);
        let open = b.shop < 0.6;
        let cafe = side == 1 && p.z < -1.5 && p.z > -14.5;
        if (isw > 0.5 && cafe) {
            // the café: warm room, a zinc bar, people at the window
            glass = 1.0;
            let warm = vec3f(1.0, 0.66, 0.36);
            var room = warm * (0.25 + 0.2 * smoothstep(0.4, 2.8, p.y));
            let bar = step(p.y, 1.1) * step(0.9, p.y);
            room = mix(room, vec3f(0.5, 0.5, 0.48) * 0.4, bar);
            let fx = fract(-p.z / 1.3);
            let hp = hash_f(u32(i32(floor(-p.z / 1.3))) + 700u);
            let px = (fx - 0.5) * 1.3;
            let person = sdf2_round_box(vec2f(px, p.y - 1.0), vec2f(0.21 - 0.05 * saturate(1.15 - p.y), 0.36), 0.15);
            let head = length(vec2f(px + 0.02, p.y - 1.5)) - 0.105;
            let fig = step(op_smin(person, head, 0.06), 0.0) * step(0.45, hp);
            room = mix(room, vec3f(0.03, 0.02, 0.015), fig);
            emi = room * (0.6 + 0.4 * l.lamps);
        } else if (isw > 0.5) {
            glass = 1.0;
            let warm = mix(vec3f(1.0, 0.72, 0.45), vec3f(1.0, 0.85, 0.65), b.shop);
            let hb = hash_f(b.id + u32(floor(bx)) * 7u);
            emi = select(vec3f(0.003, 0.003, 0.004), warm * (0.02 + 0.07 * l.lamps) * (0.4 + hb), open && hb > 0.3);
            // the nearest shop on the left is shut behind green shutters
            if (side == 0 && p.z > -12.0 && p.z < 1.0) {
                emi = vec3f(0.0);
                glass = 0.0;
                alb = vec3f(0.09, 0.2, 0.13) * (0.7 + 0.3 * step(0.15, fract(p.y / 0.12)));
            }
            // mullions and the display inside
            emi *= (0.6 + 0.4 * step(0.5, fract(u * 3.0))) * (0.6 + 0.4 * smoothstep(0.35, 1.5, p.y));
        } else if (fascia > 0.5) {
            // painted shop fascia: bottle green or oxblood, gilt letters
            let fc = select(vec3f(0.03, 0.09, 0.06), vec3f(0.12, 0.03, 0.03), b.shop > 0.3);
            let letters = step(0.3, fract(bx * 3.0)) * step(abs(p.y - 3.65), 0.12) * step(0.5, fract(zl * 4.0));
            alb = mix(fc, vec3f(0.6, 0.45, 0.15), letters * 0.8);
        } else {
            alb *= 0.75 + 0.25 * step(0.08, fract(p.y / 0.55));   // rustication
        }
    } else if (fl < 6) {
        // tall French windows
        let isw = step(0.3, u) * step(u, 0.7) * step(0.12, v) * step(v, 0.86);
        if (isw > 0.5) {
            glass = 1.0;
            emi = window_light(bx, fl, b, l, ctx.t);
            // curtains and the mullion
            emi *= (0.6 + 0.4 * smoothstep(0.3, 0.42, u) * smoothstep(0.7, 0.58, u)) * (1.0 - step(abs(u - 0.5), 0.012));
        }
        // stone surround
        let sur = step(0.26, u) * step(u, 0.74) * step(0.08, v) * step(v, 0.9) * (1.0 - isw);
        alb *= 1.0 + 0.12 * sur;
    }
    alb *= 1.0 - 0.3 * party;
    // light: sun where the opposite roofline lets it through, sky, lamps
    let sh = sunlit(p + n * 0.1, l);
    let dif = saturate(dot(n, l.sun));
    var e = l.sun_c * dif * sh + l.amb * (0.55 + 0.1 * n.y);
    // warm light bounced off the sunlit stone across the street
    e += l.sun_c * vec3f(0.9, 0.75, 0.55) * 0.06 * smoothstep(4.0, 20.0, p.y);
    e += lamp_light(p, n, l);
    // window glass mirrors the far side of the street, the sky only when
    // it tilts up to it
    var c = alb * 0.318 * e;
    if (glass > 0.5) {
        let r = reflect(rd, n);
        let fr = 0.04 + 0.5 * pow(1.0 - saturate(-dot(rd, n)), 4.0);
        let street = l.amb * 0.12 + l.sun_c * 0.02 + vec3f(1.0, 0.7, 0.4) * 0.02 * l.lamps;
        let refl = mix(street, paris_sky(r, l, ctx), smoothstep(0.1, 0.5, r.y));
        c = alb * 0.02 * e + refl * fr + emi;
    }
    return c;
}

// zinc mansard: blue-grey, dormers with a few lit windows
fn shade_mansard(p: vec3f, n: vec3f, l: Look, ctx: Ctx, side: i32) -> vec3f {
    let b = building(p.z, side);
    let zl = b.z0 - p.z;
    let bx = zl / (b.len / 6.0);
    let u = fract(bx);
    var alb = vec3f(0.3, 0.33, 0.37) * (0.85 + 0.15 * step(0.5, fract(zl * 2.0)));
    let dy = p.y - b.cor;
    let dormer = step(0.33, u) * step(u, 0.67) * step(0.5, dy) * step(dy, 2.1);
    var emi = vec3f(0.0);
    if (dormer > 0.5) {
        alb = b.tone * 0.9;
        let win = step(0.4, u) * step(u, 0.6) * step(0.7, dy) * step(dy, 1.8);
        emi = window_light(bx, 6, b, l, ctx.t) * win;
        alb *= 1.0 - win * 0.9;
    }
    let sh = sunlit(p + n * 0.1, l);
    var e = l.sun_c * saturate(dot(n, l.sun)) * sh + l.amb * (0.6 + 0.4 * n.y);
    e += lamp_light(p, n, l) * 0.5;
    var c = alb * 0.318 * e + emi;
    // zinc sheen toward the bright sky
    c += l.amb * 0.1 * saturate(n.y);
    return c;
}

// street lamps: posts along both kerbs every LAMP_DZ metres
fn lamp_pos(i: i32, side: i32) -> vec3f {
    let sx = select(-1.0, 1.0, side == 1);
    let z = -f32(i) * LAMP_DZ - select(4.0, 16.0, side == 1);
    return vec3f(sx * (CURB + 0.35), 4.3, z);
}

fn lamp_light(p: vec3f, n: vec3f, l: Look) -> vec3f {
    if (l.lamps <= 0.2) { return vec3f(0.0); }
    let i0 = i32(floor(-p.z / LAMP_DZ));
    var e = vec3f(0.0);
    for (var i = i0 - 1; i <= i0 + 1; i++) {
        for (var s = 0; s < 2; s++) {
            let lp = lamp_pos(i, s);
            let d = lp - p;
            let d2 = dot(d, d);
            let nl = saturate(dot(n, d * inverseSqrt(d2)) * 0.9 + 0.1);
            e += vec3f(1.0, 0.72, 0.42) * 4.0 * nl / (d2 + 0.5);
        }
    }
    // the café's glow on the near right
    let cd = vec3f(HW - 1.0, 2.0, -8.0) - p;
    e += vec3f(1.0, 0.68, 0.38) * 6.0 * l.lamps * saturate(dot(n, normalize(cd)) * 0.8 + 0.2) / (dot(cd, cd) + 4.0);
    return e * l.lamps;
}

// ------------------------------------------------------------ the café terrace

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    // tables in two rows under the awning, chairs facing the street
    let zc = clamp(round((p.z + 3.0) / 1.9), -5.0, 0.0);
    let zz = p.z + 3.0 - zc * 1.9;
    var r = vec2f(1e5, 0.0);
    for (var row = 0; row < 2; row++) {
        let tx = HW - 1.9 + f32(row) * 1.1 + 0.2 * f32((i32(zc) + row) & 1);
        let q = vec3f(p.x - tx, p.y, zz + f32(row) * 0.5 - 0.25);
        // round marble top on a cast-iron pedestal
        let top = sdf_cyl_y(q - vec3f(0.0, 0.73, 0.0), 0.015, 0.3);
        let ped = sdf_cyl_y(q - vec3f(0.0, 0.37, 0.0), 0.36, 0.025);
        let foot = sdf_cyl_y(q - vec3f(0.0, 0.01, 0.0), 0.01, 0.2);
        r = op_umin(r, vec2f(min(top, min(ped, foot)), 1.0));
        // a rattan bistro chair on the street side, facing out
        let cq = q - vec3f(-0.55, 0.0, 0.0);
        let seat = sdf_round_box(cq - vec3f(0.0, 0.46, 0.0), vec3f(0.2, 0.025, 0.2), 0.02);
        let back = sdf_round_box(cq - vec3f(0.2, 0.72, 0.0), vec3f(0.025, 0.24, 0.19), 0.03);
        let lg = vec2f(abs(cq.x) - 0.17, abs(cq.z) - 0.17);
        let legs = length(vec2f(max(lg.x, 0.0), max(lg.y, 0.0))) + min(max(lg.x, lg.y), 0.0) - 0.015;
        let legs_c = max(legs - 0.0, cq.y - 0.46);
        r = op_umin(r, vec2f(min(min(seat, back), max(legs_c, -cq.y)), 2.0));
    }
    return r;
}

// the awning: a sloped red canvas from the facade over the terrace, with a
// scalloped valance lettered in white. Returns (t, u along, v down slope).
fn awning_hit(ro: vec3f, rd: vec3f) -> vec3f {
    // plane through (HW, 3.3) and (HW - 2.7, 2.55), along z in [-14.5, -1.5]
    let a = vec2f(HW, 3.3);
    let b = vec2f(HW - 2.7, 2.55);
    let dir = normalize(b - a);
    let nrm = vec2f(-dir.y, dir.x);
    let den = dot(rd.xy, nrm);
    if (abs(den) < 1e-5) { return vec3f(-1.0); }
    let t = dot(a - ro.xy, nrm) / den;
    if (t <= 0.0) { return vec3f(-1.0); }
    let p = ro + rd * t;
    let s = dot(p.xy - a, dir) / length(b - a);
    if (s < 0.0 || s > 1.0 || p.z < -14.5 || p.z > -1.5) { return vec3f(-1.0); }
    return vec3f(t, -p.z, s);
}

fn valance_hit(ro: vec3f, rd: vec3f) -> vec3f {
    // the hanging front edge: a vertical strip at x = HW - 2.7, y in [2.3, 2.55]
    let xv = HW - 2.7;
    let t = (xv - ro.x) / rd.x;
    if (t <= 0.0 || abs(rd.x) < 1e-5) { return vec3f(-1.0); }
    let p = ro + rd * t;
    if (p.z < -14.5 || p.z > -1.5) { return vec3f(-1.0); }
    let sc = 0.3 * abs(fract(-p.z / 0.5) - 0.5);    // scallops
    if (p.y > 2.55 || p.y < 2.28 + sc) { return vec3f(-1.0); }
    return vec3f(t, -p.z, p.y);
}

// ------------------------------------------------------------ tracing

struct Hit { t: f32, kind: i32, p: vec3f, n: vec3f, side: i32 }

// facades, balconies, cornices, mansards, the ground, the far end. The café
// furniture is traced separately.
fn trace(ro: vec3f, rd: vec3f, prim: bool) -> Hit {
    var h = Hit(1e9, 0, vec3f(0.0), vec3f(0.0, 1.0, 0.0), 0);
    // ground
    if (rd.y < 0.0) {
        let t = -ro.y / rd.y;
        h = Hit(t, 1, ro + rd * t, vec3f(0.0, 1.0, 0.0), 0);
    }
    // the facade planes on the side the ray is heading
    if (abs(rd.x) > 1e-5) {
        let side = select(0, 1, rd.x > 0.0);
        let sx = select(-1.0, 1.0, side == 1);
        let n = vec3f(-sx, 0.0, 0.0);
        let t = (sx * HW - ro.x) / rd.x;
        if (t > 0.0 && t < h.t) {
            let p = ro + rd * t;
            if (p.z > ZEND) {
                let b = building(p.z, side);
                if (p.y < b.cor) {
                    h = Hit(t, 2, p, n, side);
                } else {
                    // steep zinc mansard rising back from the cornice
                    let a = vec2f(HW, b.cor + 0.35);
                    let m = vec2f(0.34, 0.94);                 // along the slope
                    let mn = vec2f(-0.94, 0.34);               // its normal (toward the street)
                    let o = vec2f(sx * ro.x, ro.y);
                    let dxy = vec2f(sx * rd.x, rd.y);
                    let den = dot(dxy, mn);
                    if (abs(den) > 1e-5) {
                        let tm = dot(a - o, mn) / den;
                        let pm = ro + rd * tm;
                        let s = dot(vec2f(sx * pm.x, pm.y) - a, m);
                        if (tm > 0.0 && tm < h.t && s >= 0.0 && s < 3.6 && pm.z > ZEND) {
                            h = Hit(tm, 3, pm, normalize(vec3f(-sx * 0.94, 0.34, 0.0)), side);
                        }
                    }
                }
            }
        }
        // cornice ledge and balcony slabs, seen from below, on this side
        if (rd.y > 0.0) {
            for (var k = 0; k < 3; k++) {
                var yk = 7.0;
                var dk = 0.75;
                if (k == 1) { yk = 16.35; }
                if (k == 2) { yk = 19.2; dk = 0.55; }
                let ts = (yk - ro.y) / rd.y;
                if (ts > 0.0 && ts < h.t) {
                    let ps = ro + rd * ts;
                    let inx = sx * ps.x;
                    let b = building(ps.z, side);
                    let ycor = select(yk, b.cor, k == 2);
                    let ts2 = (ycor - ro.y) / rd.y;
                    let ps2 = ro + rd * ts2;
                    if (sx * ps2.x > HW - dk && sx * ps2.x < HW && ps2.z > ZEND && ts2 < h.t) {
                        h = Hit(ts2, 4, ps2, vec3f(0.0, -1.0, 0.0), side);
                    }
                }
            }
        }
        // balcony railings: wrought iron in front of the 2nd and 5th floors
        let tr = (sx * (HW - 0.75) - ro.x) / rd.x;
        if (tr > 0.0 && tr < h.t) {
            let p = ro + rd * tr;
            let y2 = p.y - 7.0;
            let y5 = p.y - 16.35;
            var yy = -1.0;
            if (y2 > 0.0 && y2 < 1.0) { yy = y2; }
            if (y5 > 0.0 && y5 < 1.0) { yy = y5; }
            if (yy >= 0.0 && p.z > ZEND) {
                // bars, a top rail, a band of scrolls
                let bars = step(0.8, fract(p.z * 8.0));
                let rail = step(0.9, yy) + step(yy, 0.06);
                let band = step(0.62, yy) * step(yy, 0.8) * step(0.5, fract(p.z * 4.0 + sin(yy * 30.0) * 0.2));
                if (max(max(bars, rail), band) > 0.5) {
                    h = Hit(tr, 5, p, vec3f(-sx, 0.0, 0.0), side);
                }
            }
        }
    }
    // the far end: the tower stands beyond, above the trees of the Champ de Mars
    if (h.t > 1e8 || h.p.z < ZEND) {
        if (rd.z < 0.0) {
            let tt = (ZEND - 60.0 - ro.z) / rd.z;
            let pt = ro + rd * tt;
            let trees = 18.0 + 5.0 * noise_fbm2(vec2f(pt.x * 0.08, 1.0), 3);
            if (pt.y < trees && tt < h.t) {
                h = Hit(tt, 6, pt, vec3f(0.0, 0.0, 1.0), 0);
            }
        }
    }
    return h;
}

// reflections off wet stone: the same trace from the mirror ray, shaded
// simply (no terrace, no glass reflections)
fn shade_simple(h: Hit, rd: vec3f, l: Look, ctx: Ctx, lod: f32) -> vec3f {
    if (h.kind == 2) { return shade_facade(h.p, h.n, rd, lod, l, ctx, h.side); }
    if (h.kind == 3) { return shade_mansard(h.p, h.n, l, ctx, h.side); }
    if (h.kind == 4) {
        let b = building(h.p.z, h.side);
        return b.tone * 0.318 * (l.amb * 0.35 + lamp_light(h.p, h.n, l) * 0.6);
    }
    if (h.kind == 5) { return vec3f(0.01) * (l.amb.x * 10.0 + 0.3); }
    if (h.kind == 6) {
        return vec3f(0.02, 0.03, 0.02) * (l.amb * 4.0 + l.sun_c * 0.1) + l.hazec * 0.6;
    }
    return vec3f(0.0);
}

fn far_things(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx, zoom: f32) -> vec4f {
    // the tower: rgb premultiplied, coverage
    if (rd.z >= 0.0) { return vec4f(0.0); }
    let t = (TZ - ro.z) / rd.z;
    let p = ro + rd * t;
    let lod = ctx.px / zoom * t;
    let e = eiffel(vec2f(p.x - TX, p.y), lod);
    if (e.x <= 0.0) { return vec4f(0.0); }
    // puddled-iron brown in daylight, backlit; gold under its lamps at night
    var c = vec3f(0.12, 0.08, 0.055) * 0.318 * (l.amb * 0.8 + l.sun_c * 0.08);
    c += vec3f(1.0, 0.58, 0.22) * 1.1 * l.tower * (0.55 + 0.45 * e.y);
    // the sparkle: the first five minutes of every hour
    let tw = fmod_pos(ctx.t, 3600.0);
    if (tw < 300.0 && l.tower > 0.5) {
        let cell = vec2i(floor(vec2f(p.x - TX, p.y) / 5.0));
        let hc = hash_cell2(cell, 0x5a4cu);
        let ph = fract(ctx.t * (0.8 + 0.5 * hc.x) + hc.y);
        let flash = smoothstep(0.85, 0.95, ph) * smoothstep(1.0, 0.95, ph);
        c += vec3f(1.0, 0.97, 0.92) * 9.0 * flash * step(hc.z, 0.7) / max(e.x, 0.3);
    }
    // aerial haze across 700 m
    let fk = 1.0 - exp(-t * 0.001 * l.haze);
    c = mix(c, l.hazec, fk);
    return vec4f(c * e.x, e.x);
}

// the rotating beacon on the summit: two beams sweeping the night haze
fn beacon(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    if (l.tower < 0.5) { return vec3f(0.0); }
    let top = vec3f(TX, 318.0, TZ);
    var g = vec3f(0.0);
    for (var k = 0; k < 2; k++) {
        let a = ctx.t * 0.11 + f32(k) * PI;
        let bd = normalize(vec3f(cos(a), 0.02, sin(a)));
        // closest approach between the view ray and the beam (a half line)
        let w = ro - top;
        let b = dot(rd, bd);
        let dd = dot(rd, w);
        let e = dot(bd, w);
        let den = 1.0 - b * b;
        if (den < 1e-5) { continue; }
        let s = (b * dd - e) / den;       // along the beam
        let tr = (dd - b * e) / den * -1.0;
        if (s <= 0.0) { continue; }
        let pa = ro + rd * max(-tr, 0.0);
        let pb = top + bd * s;
        let dist = length(pa - pb);
        g += vec3f(1.0, 0.85, 0.6) * 0.04 * exp(-dist / (4.0 + s * 0.004)) * exp(-s / 3500.0);
    }
    return g * (0.5 + l.haze * 0.3);
}

// plane-tree leaves drifting down past the camera
fn leaves(p: vec2f, l: Look, ctx: Ctx) -> vec4f {
    var acc = vec4f(0.0);
    for (var k = 0; k < 2; k++) {
        let sc = 3.0 + f32(k) * 2.5;
        var q = p * sc + vec2f(ctx.t * 0.12 * sc * 0.2, ctx.t * 0.22 * sc * 0.25);
        let c = vec2i(floor(q));
        let h = hash_cell2(c, 0x1eafu + u32(k));
        if (h.w > 0.2 * l.leaves) { continue; }
        let f = fract(q) - 0.5 - (h.xy - 0.5) * 0.5;
        let rot = rot2(ctx.t * (0.8 + h.z) + h.x * 6.0) * f;
        let d = length(rot * vec2f(1.0, 2.4 + 1.5 * sin(ctx.t * 1.3 + h.y * 6.0))) - 0.07;
        let a = saturate(0.5 - d / max(ctx.px * sc * 1.5, 0.01));
        let col = mix(vec3f(0.35, 0.18, 0.05), vec3f(0.55, 0.36, 0.08), h.z);
        let lit = l.amb * 0.6 + l.sun_c * 0.08 + vec3f(1.0, 0.7, 0.4) * 0.03 * l.lamps;
        acc = vec4f(mix(acc.xyz, col * lit, a), max(acc.w, a));
    }
    return acc;
}

fn shade_ground(p: vec3f, rd: vec3f, l: Look, ctx: Ctx, zoom: f32, t: f32) -> vec3f {
    let ax = abs(p.x);
    let road = ax < CURB;
    var alb: vec3f;
    var n = vec3f(0.0, 1.0, 0.0);
    var rough = 0.5;
    if (road) {
        // granite setts in arcs of rows; each stone bulges a little
        let q = vec2f(p.x / 0.12, p.z / 0.16 + 0.5 * floor(p.x / 0.12));
        let f = fract(q) - 0.5;
        let hs = hash_cell2(vec2i(floor(q)), 0xc0b1u);
        let bump = (1.0 - smoothstep(0.3, 0.5, max(abs(f.x), abs(f.y))));
        let lodf = saturate(ctx.px / zoom * t / 0.05);
        n = normalize(vec3f(f.x * 0.8 * bump * (1.0 - lodf), 1.0, f.y * 0.6 * bump * (1.0 - lodf)));
        alb = mix(vec3f(0.16, 0.15, 0.15), vec3f(0.24, 0.22, 0.2), hs.x) * mix(0.55, 1.0, bump + lodf);
        rough = mix(0.25, 0.05, l.wet);
    } else {
        // pavement slabs, the kerb stone
        let kerb = step(ax, CURB + 0.25);
        alb = mix(vec3f(0.3, 0.29, 0.27), vec3f(0.42, 0.4, 0.37), kerb) * (0.85 + 0.15 * noise_value2(p.xz * 2.0));
        alb *= 1.0 - 0.3 * step(0.95, fract(p.z / 1.2));
        rough = mix(0.35, 0.1, l.wet);
    }
    let wet = l.wet;
    alb *= mix(1.0, 0.5, wet);
    let sh = sunlit(p + vec3f(0.0, 0.05, 0.0), l);
    var e = l.sun_c * saturate(l.sun.y) * sh + l.amb * 0.9 + lamp_light(p, n, l);
    var c = alb * 0.318 * e;
    // mirror: facades, lamps and the tower in the wet stone
    let r = reflect(rd, n);
    let fres = 0.02 + 0.98 * pow(1.0 - saturate(dot(-rd, n)), 5.0);
    let spec = mix(0.1, 0.9, wet);
    if (spec * fres > 0.01) {
        let j = (ctx.jitter - 0.5) * rough * 0.3;
        let rr = normalize(r + vec3f(0.0, j, 0.0));
        let hr = trace(p + n * 0.01, rr, false);
        var rc = paris_sky(rr, l, ctx);
        let tw = far_things(p, rr, l, ctx, zoom);
        rc = rc * (1.0 - tw.w) + tw.xyz;
        if (hr.t < 1e8) {
            rc = shade_simple(hr, rr, l, ctx, 0.1 + rough);
            rc = mix(rc, l.hazec, 1.0 - exp(-hr.t * 0.001 * l.haze));
        }
        rc += lamp_glow(p, rr, min(hr.t, 400.0), l, 1.0 + rough * 6.0);
        c += rc * fres * spec;
    }
    if (l.rain > 0.0) {
        let rip = water_rain_ripples(p.xz * 1.5, ctx.t, 2.0);
        c += rip.z * 0.02 * e;
    }
    return c;
}

// lamp heads and their halos in the damp air along a ray
fn lamp_glow(ro: vec3f, rd: vec3f, tmax: f32, l: Look, spread: f32) -> vec3f {
    if (l.lamps <= 0.2) { return vec3f(0.0); }
    var g = vec3f(0.0);
    let i0 = max(i32(floor(-ro.z / LAMP_DZ)) - 1, -1);
    for (var k = 0; k < 14; k++) {
        let i = i0 + k;
        for (var s = 0; s < 2; s++) {
            let lp = lamp_pos(i, s);
            let w = lp - ro;
            let tc = dot(w, rd);
            if (tc < 0.0 || tc > tmax + 1.0) { continue; }
            let d2 = max(dot(w, w) - tc * tc, 0.0);
            let r = 0.18 * spread;
            // the glass lantern itself, then a soft halo
            g += vec3f(1.0, 0.75, 0.45) * (3.0 * exp(-d2 / (r * r)) + 0.12 * (0.3 + l.haze * 0.2) / (1.0 + d2 / (0.8 * spread)));
        }
    }
    return g * l.lamps;
}

// lamp posts: dark cast iron, thin verticals at the kerbs
fn posts(ro: vec3f, rd: vec3f, tmax: f32, ctx: Ctx, zoom: f32) -> f32 {
    var cov = 0.0;
    let i0 = max(i32(floor(-ro.z / LAMP_DZ)) - 1, -1);
    for (var k = 0; k < 10; k++) {
        let i = i0 + k;
        for (var s = 0; s < 2; s++) {
            let lp = lamp_pos(i, s);
            // closest approach of the ray to the vertical line through the post
            let o = ro.xz - lp.xz;
            let dxz = rd.xz;
            let tt = -dot(o, dxz) / max(dot(dxz, dxz), 1e-6);
            if (tt <= 0.0 || tt > tmax) { continue; }
            let y = ro.y + rd.y * tt;
            if (y > 4.1 || y < 0.0) { continue; }
            let d = length(o + dxz * tt);
            let lod = ctx.px / zoom * tt;
            let r = 0.06 + 0.05 * smoothstep(1.2, 0.0, y);
            cov = max(cov, saturate((r - d) / max(lod, 1e-3) + 0.5));
        }
    }
    return cov;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(-2.6, 1.62, 6.0);
    let cam = cam_look_at(ro, vec3f(3.0, 52.0, -400.0), 0.0, 50.0);
    let rd = cam_ray(cam, p);
    let zoom = cam.zoom;
    var h = trace(ro, rd, true);
    // café terrace furniture (bounded march)
    var furn = vec2f(-1.0);
    if (abs(rd.x) > 1e-4) {
        let t0 = (HW - 2.4 - ro.x) / rd.x;
        let t1 = (HW - ro.x) / rd.x;
        let ta = max(min(t0, t1), 0.0);
        let tb = min(max(t0, t1), h.t);
        if (tb > ta && rd.x > 0.0) {
            furn = rm_march(ro, rd, ta, tb, steps(40.0, ctx), ctx);
        }
    }
    let aw = awning_hit(ro, rd);
    let va = valance_hit(ro, rd);
    var col: vec3f;
    var th = h.t;
    let sky = paris_sky(rd, l, ctx);
    let tower = far_things(ro, rd, l, ctx, zoom);
    if (va.x > 0.0 && va.x < th && (furn.x < 0.0 || va.x < furn.x)) {
        // the lettered valance
        th = va.x;
        let u = fract(va.y / 1.4);
        let txt = step(0.25, u) * step(u, 0.75) * step(2.36, va.z) * step(va.z, 2.5) * step(0.4, fract(va.y * 5.0));
        let base = vec3f(0.34, 0.04, 0.03);
        col = mix(base, vec3f(0.85, 0.82, 0.75), txt * 0.8) * 0.318 * (l.amb + l.sun_c * 0.1 + lamp_light(ro + rd * th, vec3f(-1.0, 0.0, 0.0), l) * 0.8);
    } else if (aw.x > 0.0 && aw.x < th && (furn.x < 0.0 || aw.x < furn.x)) {
        // under the awning: the canvas glowing with the bulbs beneath
        th = aw.x;
        let stripe = step(0.5, fract(aw.y / 0.5));
        let canvas = mix(vec3f(0.3, 0.035, 0.025), vec3f(0.24, 0.028, 0.02), stripe);
        col = canvas * (0.12 + 0.45 * l.lamps) + canvas * l.amb * 2.0;
        // a row of little bulbs along the edge
        let bz = fract(aw.y / 1.2) - 0.5;
        col += vec3f(1.0, 0.75, 0.45) * 3.0 * l.lamps * exp(-(sq(bz * 1.2) + sq((aw.z - 0.9) * 2.4)) / 0.004);
    } else if (furn.x > 0.0 && furn.x < th) {
        th = furn.x;
        let fp = ro + rd * th;
        let n = rm_normal(fp, th, ctx);
        var alb = select(vec3f(0.55, 0.36, 0.2), vec3f(0.75, 0.73, 0.7), furn.y < 1.5);
        if (furn.y < 1.5 && fp.y < 0.7) { alb = vec3f(0.03); }
        // rattan weave
        if (furn.y > 1.5) { alb *= 0.75 + 0.25 * step(0.5, fract((fp.y + fp.z) * 30.0)); }
        let e = lamp_light(fp, n, l) * 1.3 + l.amb * (0.5 + 0.5 * n.y) + l.sun_c * 0.05;
        col = alb * 0.318 * e;
    } else if (h.t < 1e8) {
        if (h.kind == 1) {
            col = shade_ground(h.p, rd, l, ctx, zoom, h.t);
        } else if (h.kind == 6) {
            col = shade_simple(h, rd, l, ctx, 0.0);
            col = col * (1.0 - tower.w) + tower.xyz;
        } else {
            col = shade_simple(h, rd, l, ctx, ctx.px / zoom * h.t);
        }
    } else {
        col = sky * (1.0 - tower.w) + tower.xyz;
        th = 5000.0;
    }
    if (h.kind == 2 && h.t < th + 0.01) {
        col = shade_facade(h.p, h.n, rd, ctx.px / zoom * h.t, l, ctx, h.side);
    }
    if (th < 4000.0) { col = mix(col, l.hazec, 1.0 - exp(-th * 0.001 * l.haze)); }
    // posts, lamp heads and halos
    col = mix(col, vec3f(0.006) + l.amb * 0.05, posts(ro, rd, th, ctx, zoom) * 0.95);
    col += lamp_glow(ro, rd, th, l, 1.0);
    col += beacon(ro, rd, l, ctx) * step(1000.0, th);
    // weather in front
    if (l.rain > 0.0) {
        let rs = rain_streaks(p, ctx, l.rain, 2.0, 0.05, 3);
        let lum = col_luma(lamp_glow(ro, rd, th, l, 3.0));
        col += vec3f(0.6, 0.62, 0.7) * rs * (0.015 + lum * 0.4);
    }
    let lv = leaves(p, l, ctx);
    col = mix(col, lv.xyz, lv.w);
    // gentle lens falloff
    col *= mix(1.0, pow(saturate(dot(rd, cam.fw)), 2.0), 0.5);
    return col * exp2(l.exposure);
}
