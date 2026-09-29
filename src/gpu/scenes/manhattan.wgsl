//! name: manhattan
//! title: Manhattan Rooftops
//! category: city
//! tags: new york, rooftops, water towers, skyline, dusk, empire state
//! desc: wooden water towers over Chelsea rooftops as dusk settles on the Midtown skyline
//! themes: dusk, night, snow
//! uses: camera, raymarch, sky, clouds, snow, light, noise
//! cost: light
//! tonemap: aces
//! fallback: city
//! credits: original

// World units are metres. The camera stands on a Chelsea roof, 45 m up,
// looking north-east (-z) toward Midtown; the sun has just set behind it.
// The city is a sequence of block rows, each a front plane of contiguous
// brick buildings with roofs, stair bulkheads and water towers behind it:
// intersected analytically row by row, so the near roofs are seen from
// above and the far rows stack into a skyline. The Empire State and the
// Chrysler Building stand in the far rows. One cedar water tower on the
// neighbouring roof is ray-marched in full 3D. On the dusk theme the
// twilight deepens over the scene hour and windows come on one by one.

const EYE: f32 = 45.0;
const DEPTH: f32 = 46.0;     // block depth behind each front
// the hero water tower: base on the first roof
const WT: vec3f = vec3f(-13.0, 38.0, -47.0);

struct Look {
    dusk: f32,       // 0 = just after sunset .. 1 = night
    snow: f32,
    sun: vec3f,      // direction of the (set) sun, for the sky model
    key: vec3f,      // afterglow on faces turned to the west
    amb: vec3f,      // skylight from above
    lit: f32,        // fraction of windows on
    hazec: vec3f,
    haze: f32,       // per km
    exposure: f32,
}

fn look(theme: u32, t: f32) -> Look {
    var l: Look;
    if (theme == 1u) {
        l.dusk = 1.0;
        l.snow = 0.0;
    } else if (theme == 2u) {
        l.dusk = 0.85;
        l.snow = 1.0;
    } else {
        // twilight deepens across the scene hour: quickly at first
        let x = saturate(t / 2400.0);
        l.dusk = 1.0 - sq(1.0 - x);
        l.dusk = l.dusk * 0.92;
        l.snow = 0.0;
    }
    let el = -1.2 - 8.0 * l.dusk;
    l.sun = sky_sun_dir(el, 180.0);
    let g = 1.0 - l.dusk;
    l.key = vec3f(1.0, 0.6, 0.45) * 1.1 * g * g;
    // at night the sky is lit from below by the city: a sodium-tinted glow
    l.amb = mix(vec3f(0.034, 0.026, 0.026), vec3f(0.12, 0.13, 0.2), g * g);
    l.lit = 0.18 + 0.62 * smoothstep(0.1, 0.9, l.dusk);
    l.hazec = mix(vec3f(0.018, 0.016, 0.022), vec3f(0.2, 0.16, 0.2), g * g);
    l.haze = 0.5;
    l.exposure = 0.0;
    if (l.snow > 0.0) {
        l.key = vec3f(0.0);
        l.amb = vec3f(0.1, 0.092, 0.1);
        l.hazec = vec3f(0.07, 0.062, 0.07);
        l.haze = 1.6;
        l.lit = 0.7;
        l.exposure = -0.2;
    }
    return l;
}

// ------------------------------------------------------------ the blocks

fn row_d(k: i32) -> f32 {
    switch (k) {
        case 0: { return 40.0; }
        case 1: { return 100.0; }
        case 2: { return 165.0; }
        case 3: { return 235.0; }
        case 4: { return 320.0; }
        case 5: { return 415.0; }
        case 6: { return 530.0; }
        case 7: { return 670.0; }
        case 8: { return 840.0; }
        case 9: { return 1040.0; }
        case 10: { return 1260.0; }
        case 11: { return 1560.0; }
        case 12: { return 1900.0; }
        default: { return 2400.0; }
    }
}

struct Bld {
    h: f32,      // roof height
    x0: f32,     // left edge
    w: f32,
    id: u32,
    kind: f32,   // facade: brick red / tan brick / limestone / glass
    bs: f32,     // bulkhead setback behind the front
    bx: f32,     // bulkhead centre x
    bw: f32,
    bh: f32,
    wt: f32,     // a water tower on the bulkhead roof 0/1
}

// Midtown rises in the distance around the Empire State
fn zoning(k: i32, x: f32) -> f32 {
    let far = saturate((f32(k) - 6.0) / 6.0);
    let mid = exp(-sq((x - 300.0) / 900.0));
    return far * (0.4 + 0.6 * mid);
}

fn lot_w(k: i32) -> f32 { return select(select(18.0, 24.0, k >= 6), 34.0, k >= 10); }

fn bld(k: i32, c: f32) -> Bld {
    let cw = lot_w(k);
    let h = hash_cell2(vec2i(i32(c), k), 0x3a7u);
    let g = hash_cell2(vec2i(i32(c), k), 0x91fu);
    var b: Bld;
    b.x0 = c * cw;
    b.w = cw;
    b.id = hash_u2(vec2u(bitcast<u32>(i32(c)), u32(k)));
    // walk-ups, lofts and the odd tower; the first rows stay below the eye
    let zn = zoning(k, (c + 0.5) * cw);
    var hgt = 15.0 + 11.0 * h.x + 18.0 * pow(h.y, 3.0);
    if (k <= 6) { hgt = min(hgt, 34.0); }
    hgt += zn * (40.0 + 230.0 * pow(h.z, 2.2));
    b.h = hgt;
    b.kind = g.x;
    b.bs = 4.0 + 12.0 * g.y;
    b.bw = cw * (0.25 + 0.25 * g.z);
    b.bx = b.x0 + b.bw * 0.5 + (cw - b.bw) * g.w;
    b.bh = 3.2 + 1.5 * h.w;
    b.wt = select(0.0, 1.0, fract(g.x * 7.3 + h.w * 3.1) < 0.45 && hgt < 80.0);
    // the loft under the hero water tower
    if (k == 0 && abs(b.x0 + cw * 0.5 - WT.x) < cw) {
        b.h = WT.y;
        b.wt = 0.0;
        b.bs = 30.0;
        b.bx = WT.x + 12.0;
    }
    return b;
}

// ------------------------------------------------------------ landmarks

// Empire State Building, front silhouette at x relative to its centre
fn esb_d(q: vec2f) -> f32 {
    var d = sdf2_box(q - vec2f(0.0, 14.0), vec2f(64.0, 14.0));
    d = min(d, sdf2_box(q - vec2f(0.0, 40.0), vec2f(44.0, 40.0)));
    d = min(d, sdf2_box(q - vec2f(0.0, 140.0), vec2f(29.0, 128.0)));
    d = min(d, sdf2_box(q - vec2f(0.0, 280.0), vec2f(22.0, 14.0)));
    d = min(d, sdf2_box(q - vec2f(0.0, 305.0), vec2f(15.0, 12.0)));
    // the mooring mast, tapering, and the antenna
    let tw = 9.0 - 4.0 * saturate((q.y - 317.0) / 50.0);
    d = min(d, sdf2_box(q - vec2f(0.0, 342.0), vec2f(tw, 25.0)));
    d = min(d, sdf2_box(q - vec2f(0.0, 405.0), vec2f(1.2, 38.0)));
    return d;
}
// Chrysler Building: shaft, stacked arches, needle
fn chrysler_d(q: vec2f) -> f32 {
    var d = sdf2_box(q - vec2f(0.0, 120.0), vec2f(21.0, 120.0));
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let w = 18.0 - fi * 3.2;
        let y0 = 240.0 + fi * 9.0;
        // a rounded arch tier
        let a = length(vec2f(q.x, max(q.y - y0, 0.0) * 1.3)) - w;
        d = min(d, max(a, y0 - 4.0 - q.y));
    }
    let needle = max(abs(q.x) - 2.2 * saturate((320.0 - q.y) / 36.0), abs(q.y - 302.0) - 18.0);
    return min(d, needle);
}

// ------------------------------------------------------------ the hero tower

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    let q = p - WT;
    // tank: cedar staves bound with steel hoops
    var r = vec2f(sdf_cyl_y(q - vec3f(0.0, 9.1, 0.0), 2.9, 3.3), 1.0);
    // conical roof with a finial
    r = op_umin(r, vec2f(sdf_cone_y(q - vec3f(0.0, 12.95, 0.0), 0.95, 3.5, 0.15), 2.0));
    r = op_umin(r, vec2f(sdf_sphere(q - vec3f(0.0, 14.05, 0.0), 0.18), 3.0));
    // floor beams under the tank
    r = op_umin(r, vec2f(sdf_box(q - vec3f(0.0, 6.1, 0.0), vec3f(3.0, 0.1, 3.0)), 3.0));
    // steel stand: four splayed legs and cross bracing
    var st = 1e5;
    for (var i = 0; i < 4; i++) {
        let a = f32(i) * 1.5708 + 0.785;
        let top = vec3f(cos(a) * 2.5, 6.0, sin(a) * 2.5);
        let bot = vec3f(cos(a) * 2.9, 0.0, sin(a) * 2.9);
        st = min(st, sdf_capsule(q, bot, top, 0.11));
        let a2 = a + 1.5708;
        let top2 = vec3f(cos(a2) * 2.5, 6.0, sin(a2) * 2.5);
        let bot2 = vec3f(cos(a2) * 2.9, 0.0, sin(a2) * 2.9);
        st = min(st, sdf_capsule(q, bot, top2, 0.045));
        st = min(st, sdf_capsule(q, top, bot2, 0.045));
    }
    r = op_umin(r, vec2f(st, 3.0));
    // ladder up the side toward the camera
    let lq = q - vec3f(0.6, 0.0, 3.35);
    let rails = min(sdf_box(lq - vec3f(-0.25, 7.0, 0.0), vec3f(0.03, 7.0, 0.03)), sdf_box(lq - vec3f(0.25, 7.0, 0.0), vec3f(0.03, 7.0, 0.03)));
    r = op_umin(r, vec2f(rails, 3.0));
    return r;
}

// bounding cylinder of the hero tower: (t0, t1) or t1 < t0
fn tower_bounds(ro: vec3f, rd: vec3f) -> vec2f {
    let o = ro.xz - WT.xz;
    let d = rd.xz;
    let a = dot(d, d);
    let b = dot(o, d);
    let c = dot(o, o) - 3.8 * 3.8;
    let h = b * b - a * c;
    if (h < 0.0 || a < 1e-8) { return vec2f(1.0, 0.0); }
    let s = sqrt(h);
    var t0 = (-b - s) / a;
    var t1 = (-b + s) / a;
    // clip to the tower's height
    if (abs(rd.y) > 1e-5) {
        let ya = (WT.y - ro.y) / rd.y;
        let yb = (WT.y + 14.3 - ro.y) / rd.y;
        t0 = max(t0, min(ya, yb));
        t1 = min(t1, max(ya, yb));
    }
    return vec2f(max(t0, 0.0), t1);
}

// ------------------------------------------------------------ shading

// window light; each window comes on at its own darkness, and a few change
// later. Tall sash windows: blinds, curtains, the odd television.
fn windows(u: f32, v: f32, cols: f32, rows: f32, id: u32, l: Look, lod: f32, t: f32) -> vec3f {
    let fx = u * cols;
    let fy = v * rows;
    let c = vec2i(i32(floor(fx)), i32(floor(fy)));
    let h = hash_cell2(c + vec2i(i32(id & 1023u), i32(id >> 22u)), 0x77e1u);
    let slot = floor(t / (150.0 + 200.0 * h.w) + h.z);
    let late = hash_f(bitcast<u32>(i32(slot)) ^ id ^ bitcast<u32>(c.x * 7919 + c.y * 104729));
    let on = h.x < l.lit && late > 0.1;
    let wx = fract(fx);
    let wy = fract(fy);
    let fill = step(0.3, wx) * step(wx, 0.7) * step(0.22, wy) * step(wy, 0.78);
    var wc = mix(col_kelvin(2600.0), col_kelvin(3300.0), h.y);
    if (h.y > 0.78) { wc = col_kelvin(4600.0); }
    // blinds and the sash bar, a curtain edge
    var pat = 1.0 - 0.5 * step(abs(wy - 0.5), 0.02);
    if (h.w > 0.6) { pat *= 0.75 + 0.25 * step(0.5, fract(wy * 14.0)); }
    pat *= mix(1.0, smoothstep(0.3, 0.45, wx), step(0.8, h.z));
    var e = wc;
    if (h.y > 0.93) {
        // a television: cool and slowly changing
        e = vec3f(0.35, 0.5, 1.0) * (0.5 + 0.5 * light_flicker(t, id ^ u32(c.x * 31 + c.y), 0.6));
    }
    let sharp = select(0.0, 1.0, on) * fill * pat * (0.6 + 0.6 * h.z);
    let k = saturate(lod * rows - 0.6);
    return e * mix(sharp, l.lit * 0.3, k * 0.7) * 0.6;
}

fn facade_col(kind: f32) -> vec3f {
    if (kind < 0.45) { return col_hex(0x7a3e2cu); }       // red brick
    if (kind < 0.7) { return col_hex(0x9c7a5bu); }        // tan brick
    if (kind < 0.88) { return col_hex(0xa89f90u); }       // limestone
    return col_hex(0x3a4450u);                           // glass
}

// a front face at (x, y) of building b in row k; t = distance
fn shade_front(b: Bld, x: f32, y: f32, t: f32, k: i32, l: Look, ctx: Ctx, lod: f32) -> vec3f {
    let u = (x - b.x0) / b.w;
    let alb = facade_col(b.kind) * (0.8 + 0.4 * hash_f(b.id));
    let fl = select(3.4, 3.9, b.kind > 0.88);
    let rows = max(floor(b.h / fl), 1.0);
    let cols = max(floor(b.w / 2.6), 2.0);
    let v = y / b.h;
    // cornice: a lighter band under the roofline
    let cornice = step(b.h - 1.2, y);
    // light: afterglow from behind the camera on these west-facing fronts,
    // skylight, and the street glow from below at night
    let street = vec3f(1.0, 0.6, 0.3) * 0.06 * l.dusk * exp(-y / 6.0);
    var e = l.key + l.amb * 0.5 + street;
    var c = alb * INV_PI_M * e * (1.0 + cornice * 0.4);
    // windows
    let w = windows(u, v, cols, rows, b.id, l, lod / b.h, ctx.t);
    let glass = step(0.3, fract(u * cols)) * step(fract(u * cols), 0.7) * step(0.22, fract(v * rows)) * step(fract(v * rows), 0.78);
    // dark glass mirrors the sunset glow behind the camera
    let mirror = vec3f(1.0, 0.55, 0.4) * 0.22 * (1.0 - l.dusk) * (1.0 - l.dusk) + l.amb * 0.08;
    let gl = glass * (1.0 - saturate(lod * rows / b.h * 0.7));
    c = mix(c, mirror * (0.6 + 0.8 * hash_f(b.id + u32(u * cols) * 13u + u32(v * rows) * 131u)), gl * 0.85);
    c += w * (1.0 - cornice);
    // a lit shopfront at street level
    if (y < 4.5) { c += vec3f(1.0, 0.75, 0.45) * 0.25 * step(0.2, fract(u * 2.0)) * l.dusk; }
    // black iron fire escape
    if (k <= 2) {
        let fe = iron_stairs(u, y, b, lod);
        c = mix(c, vec3f(0.012, 0.011, 0.01) + l.key * 0.01, fe);
    }
    return c;
}

fn shade_roof(b: Bld, p: vec3f, d: f32, l: Look) -> vec3f {
    let back = -p.z - d;                  // metres behind the front edge
    let u = p.x - b.x0;
    let n = noise_value2(p.xz * 0.6);
    // tar paper laid in strips, patched
    var alb = vec3f(0.11, 0.105, 0.1) * (0.75 + 0.45 * n) * (0.9 + 0.1 * step(0.08, fract(back / 0.95)));
    // many roofs are painted with silver coating
    let silver = step(0.6, hash_f(b.id * 7u + 5u));
    alb = mix(alb, vec3f(0.5, 0.5, 0.52) * (0.8 + 0.3 * n), silver);
    // parapet coping along the front and the party walls
    let edge = min(back, min(u, b.w - u));
    let coping = step(edge, 0.45);
    alb = mix(alb, vec3f(0.35, 0.3, 0.26), coping);
    // a skylight or two, glass that holds the sky or the room light
    let sk = vec2f(fract(u / 7.0), fract(back / 9.0));
    let hs = hash_cell2(vec2i(i32(floor(u / 7.0)) + i32(b.id & 255u), i32(floor(back / 9.0))), 0x5c1u);
    let sky_glass = step(abs(sk.x - 0.5), 0.12) * step(abs(sk.y - 0.5), 0.1) * step(0.7, hs.x) * step(back, DEPTH - 2.0) * step(1.0, back);
    alb = mix(alb, vec3f(0.02), sky_glass);
    alb = mix(alb, vec3f(0.8, 0.82, 0.86), l.snow * 0.9 * (1.0 - sky_glass));
    // horizontal roofs see the whole sky dome, and at a grazing angle they
    // mirror a little of the bright band low in the sky
    let e = l.amb * 2.6 + l.key * 0.3;
    var c = alb * INV_PI_M * e;
    c += (l.amb * 0.35 + l.key * 0.05) * mix(0.25, 0.6, silver) * (1.0 - l.snow);
    // skylights: sky reflection at dusk, warm rooms at night
    c += sky_glass * (l.amb * 0.25 + vec3f(1.0, 0.7, 0.4) * 0.35 * step(0.4, hs.y) * l.dusk);
    // the coping lip catches the afterglow
    c += coping * step(back, 0.45) * alb * INV_PI_M * l.key * 1.5;
    return c;
}

// fire escape on a front: platforms at every floor, ladders zig-zagging.
// Returns iron coverage.
fn iron_stairs(u: f32, y: f32, b: Bld, lod: f32) -> f32 {
    var hs = hash_f(b.id * 3u + 1u);
    if (abs(b.h - WT.y) < 0.01) { hs = 0.1; }
    if (hs > 0.6 || b.h > 60.0) { return 0.0; }
    let x = (u - 0.3 - 0.2 * hs) * b.w;          // metres from its left end
    let wd = 4.2;
    if (x < 0.0 || x > wd || y < 4.0 || y > b.h - 1.5) { return 0.0; }
    let fl = 3.4;
    let fy = fract(y / fl) * fl;
    let lw = max(lod * 0.7, 0.04);
    var d = abs(fy - 0.3) - 0.06;                // platform
    d = min(d, abs(fy - 1.2) - 0.03);            // railing
    let dir = select(1.0, -1.0, fract(floor(y / fl) * 0.5) > 0.25);
    let lx = select(x, wd - x, dir < 0.0);
    d = min(d, abs(fy - 0.3 - lx * 0.78) * 0.8 - 0.04);   // ladder
    d = min(d, min(abs(x) - 0.03, abs(x - wd) - 0.03));
    return saturate(0.5 - d / lw) * 0.9;
}

// small water tower on a bulkhead roof, as a flat shape in the bulkhead plane
fn little_tower(q: vec2f, lod: f32) -> vec2f {
    // q: x from the tower axis, y from the bulkhead roof. (coverage, shade)
    let tank = sdf2_box(q - vec2f(0.0, 5.6), vec2f(2.3, 2.1));
    let roof = sdf2_tri(q - vec2f(0.0, 7.6), 2.5, 1.5);
    let legs = min(abs(abs(q.x) - 1.8) - 0.1, abs(q.x) - 0.08);
    let legd = max(legs, max(q.y - 3.6, -q.y));
    let d = min(min(tank, roof), legd);
    let a = saturate(0.5 - d / max(lod, 0.02));
    // cylinder shading across the tank
    let s = select(0.6, 0.5 + 0.5 * sqrt(saturate(1.0 - sq(q.x / 2.3))), tank < 0.0);
    return vec2f(a, s);
}

fn dusk_sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let y = max(rd.y, 0.0);
    var c: vec3f;
    if (l.snow > 0.0) {
        // low snow cloud glowing with the city
        return mix(vec3f(0.075, 0.064, 0.07), vec3f(0.032, 0.03, 0.038), pow(saturate(y * 2.5), 0.6));
    }
    let g = 1.0 - l.dusk;
    let g2 = g * g;
    // looking away from the set sun: Earth's shadow, a blue-grey band on
    // the horizon rising as the sun sinks, under the pink Belt of Venus,
    // under a deepening blue zenith
    let hs = 0.025 + 0.12 * l.dusk;
    let shadow = vec3f(0.10, 0.11, 0.18) * g2 + vec3f(0.02, 0.016, 0.02);
    let belt = vec3f(0.5, 0.27, 0.3) * g2 + vec3f(0.018, 0.014, 0.018);
    let zen = mix(vec3f(0.003, 0.004, 0.011), vec3f(0.05, 0.085, 0.2), g2);
    c = mix(shadow, belt, smoothstep(hs - 0.015, hs + 0.035, y));
    c = mix(c, mix(belt, zen, 0.5), smoothstep(hs + 0.04, hs + 0.14, y));
    c = mix(c, zen, smoothstep(hs + 0.1, hs + 0.5, y));
    // the city's own glow on the haze, as night takes over
    c += vec3f(0.04, 0.026, 0.022) * l.dusk * exp(-y * 9.0);
    // a few clouds catching the last light
    if (rd.y > 0.0) {
        let uv = rd.xz / (rd.y + 0.1) * 0.9;
        let cs = cloud_sheet(uv + vec2f(ctx.t * 0.002, 3.0), 0.38, ctx);
        let lit = vec3f(0.95, 0.5, 0.45) * 0.4 * g2 + vec3f(0.025, 0.02, 0.025) * (0.4 + l.dusk);
        c = mix(c, lit * (0.7 + 0.6 * cs.y), cs.x * 0.6 * smoothstep(0.02, 0.15, rd.y));
    }
    return c;
}

const INV_PI_M: f32 = 0.318309886;

// a party wall seen past a lower neighbour: blind brick, sometimes the
// ghost of a painted advertisement
fn shade_side(b: Bld, z: f32, y: f32, l: Look) -> vec3f {
    let alb = facade_col(b.kind) * 0.8 * (0.8 + 0.4 * hash_f(b.id));
    var c = alb * INV_PI_M * (l.amb * 0.45 + l.key * 0.12);
    let ad = step(0.7, hash_f(b.id * 5u + 3u)) * step(b.h - 12.0, y) * step(y, b.h - 2.0) * step(abs(fract(z / DEPTH) - 0.5), 0.3);
    c = mix(c, vec3f(0.5, 0.45, 0.32) * INV_PI_M * (l.amb * 0.45 + l.key * 0.12), ad * 0.5);
    return c;
}

// the city along a ray: rgb and coverage, front to back. Each block row is
// a slab of contiguous lots; the ray walks the lots it crosses inside the
// slab and enters through a front, a party wall, or down onto a roof.
fn city(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx, zoom: f32, tmax: f32) -> vec4f {
    var col = vec3f(0.0);
    var acc = 0.0;
    if (rd.z > -1e-4) { return vec4f(0.0); }
    let sx = select(-1.0, 1.0, rd.x > 0.0);
    for (var k = 0; k < 14; k++) {
        let d = row_d(k);
        let tf = d / -rd.z;
        if (tf > tmax) { break; }
        let tback = (d + DEPTH) / -rd.z;
        let p = ro + rd * tf;
        let lod = ctx.px / zoom * tf;
        if (p.y < 0.0) {
            // the ray reached the street in front of this row
            let tg = -ro.y / rd.y;
            let g = ro + rd * tg;
            var c = vec3f(0.02, 0.018, 0.017) * l.amb * 4.0;
            c += vec3f(1.0, 0.62, 0.3) * 0.05 * l.dusk * (0.6 + 0.4 * noise_value2(g.xz * 0.2));
            return vec4f(col + (1.0 - acc) * c, 1.0);
        }
        // landmarks in the far rows
        if (k == 11) {
            let q = vec2f(p.x - 330.0, p.y);
            let ed = esb_d(q);
            let ae = saturate(0.5 - ed / lod);
            if (ae > 0.0) {
                let u = q.x / 58.0 + 0.5;
                var e = windows(u, p.y / 262.0, 18.0, 72.0, 9001u, l, lod / 262.0, ctx.t) * step(q.y, 262.0) * 0.9;
                e += vec3f(0.62, 0.58, 0.52) * INV_PI_M * (l.key * 0.8 + l.amb * 0.5);
                // the crown floodlit gold and white from the 72nd floor up
                let crown = smoothstep(258.0, 270.0, q.y) * step(q.y, 368.0);
                e += mix(vec3f(1.0, 0.82, 0.5), vec3f(1.0, 0.95, 0.85), smoothstep(290.0, 340.0, q.y)) * crown * mix(0.35, 1.5, smoothstep(0.1, 0.5, l.dusk));
                let ek = 1.0 - exp(-tf * 0.001 * l.haze);
                col += (1.0 - acc) * ae * mix(e, l.hazec, ek * 0.7);
                acc += (1.0 - acc) * ae;
            }
            let tip = length(vec2f(q.x, q.y - 441.0)) / max(lod * 0.6, 1.2);
            let blink = step(0.5, fract(ctx.t * 0.45));
            col += (1.0 - acc) * vec3f(1.0, 0.05, 0.02) * 2.5 * exp(-tip * tip) * blink;
        }
        if (k == 13) {
            let q = vec2f(p.x - 780.0, p.y);
            let cd = chrysler_d(q);
            let ac = saturate(0.5 - cd / lod);
            if (ac > 0.0) {
                var e = windows(q.x / 42.0 + 0.5, p.y / 240.0, 12.0, 60.0, 9002u, l, lod / 240.0, ctx.t) * step(q.y, 238.0) * 0.8;
                let ch = step(238.0, q.y) * step(0.5, fract(q.y / 4.0 + abs(q.x) / 8.0));
                e += vec3f(0.9, 0.93, 1.0) * ch * mix(0.2, 1.4, smoothstep(0.1, 0.5, l.dusk));
                e += vec3f(0.55, 0.57, 0.6) * INV_PI_M * (l.key + l.amb * 0.5) * step(238.0, q.y);
                let ek = 1.0 - exp(-tf * 0.001 * l.haze);
                col += (1.0 - acc) * ac * mix(e, l.hazec, ek);
                acc += (1.0 - acc) * ac;
            }
        }
        if (acc > 0.995) { break; }
        let cw = lot_w(k);
        var c = floor(p.x / cw);
        for (var j = 0; j < 4; j++) {
            let b = bld(k, c);
            // skip generic lots where a landmark stands
            let cx = b.x0 + cw * 0.5;
            if ((k == 11 && abs(cx - 330.0) < 80.0) || (k == 13 && abs(cx - 780.0) < 40.0)) {
                c += sx;
                continue;
            }
            // the ray's span over this lot, inside the slab
            var ta = tf;
            var tb = tback;
            if (abs(rd.x) > 1e-6) {
                let t0 = (b.x0 - ro.x) / rd.x;
                let t1 = (b.x0 + cw - ro.x) / rd.x;
                ta = max(ta, min(t0, t1));
                tb = min(tb, max(t0, t1));
            }
            if (ta >= tb) { c += sx; continue; }
            let pa = ro + rd * ta;
            let lodj = ctx.px / zoom * ta;
            var sh = vec3f(0.0);
            var a = 0.0;
            var th = 1e9;
            // entering below the roofline: the front, or a party wall
            a = saturate(0.5 - (pa.y - b.h) / lodj);
            if (a > 0.0) {
                if (ta <= tf + 1e-3) {
                    sh = shade_front(b, pa.x, pa.y, ta, k, l, ctx, lodj);
                } else {
                    sh = shade_side(b, pa.z, pa.y, l);
                }
                th = ta;
            }
            if (a < 1.0) {
                // over the roof: the stair bulkhead, a water tower, the roof
                var s2 = vec3f(0.0);
                var a2 = 0.0;
                let tbk = (d + b.bs) / -rd.z;
                if (tbk > ta && tbk < tb) {
                    let pb = ro + rd * tbk;
                    let lodb = ctx.px / zoom * tbk;
                    if (abs(pb.x - b.bx) < b.bw * 0.5 && pb.y < b.h + b.bh && pb.y > b.h) {
                        s2 = facade_col(b.kind) * 0.6 * INV_PI_M * (l.key + l.amb * 0.5);
                        a2 = 1.0;
                    } else if (b.wt > 0.5 && pb.y > b.h) {
                        let lt = little_tower(vec2f(pb.x - b.bx, pb.y - b.h - b.bh), lodb);
                        if (lt.x > 0.0) {
                            s2 = vec3f(0.3, 0.22, 0.16) * INV_PI_M * (l.key * lt.y + l.amb * 0.5);
                            s2 = mix(s2, vec3f(0.8) * INV_PI_M * l.amb * 2.0, l.snow * step(b.h + b.bh + 7.4, pb.y));
                            a2 = lt.x;
                        }
                    }
                }
                if (a2 < 1.0 && rd.y < 0.0) {
                    let tr = (b.h - ro.y) / rd.y;
                    if (tr > ta && tr < tb && (a2 == 0.0 || tr < tbk)) {
                        let rc = shade_roof(b, ro + rd * tr, d, l);
                        s2 = mix(rc, s2, a2);
                        a2 = 1.0;
                    }
                }
                // premultiplied: the front's edge over whatever lies behind it
                let at = a + (1.0 - a) * a2;
                sh = (sh * a + s2 * (1.0 - a) * a2) / max(at, 1e-4);
                if (a2 > 0.0 && th > 1e8) { th = tbk; }
                a = at;
            }
            if (a > 0.0) {
                let fk = 1.0 - exp(-min(th, tf) * 0.001 * l.haze);
                let cc = mix(sh, l.hazec, fk);
                col += (1.0 - acc) * a * cc;
                acc += (1.0 - acc) * a;
            }
            // warning lights on the tall ones
            if (b.h > 200.0 && hash_f(b.id * 11u) < 0.45) {
                let q = vec2f(pa.x - cx, pa.y - b.h - 2.0);
                let r = length(q) / max(lodj * 0.6, 1.2);
                let blink = step(0.5, fract(ctx.t * 0.4 + hash_f(b.id) * 0.8));
                col += (1.0 - acc * 0.5) * vec3f(1.0, 0.05, 0.02) * 1.8 * exp(-r * r) * blink;
            }
            if (acc > 0.995 || tb >= tback - 1e-3) { break; }
            c += sx;
        }
        if (acc > 0.995) { break; }
    }
    return vec4f(col, acc);
}

fn shade_tower(p: vec3f, n: vec3f, mat: f32, rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let q = p - WT;
    var alb: vec3f;
    if (mat < 1.5) {
        // weathered cedar staves, steel hoops
        let ang = atan2(q.z, q.x);
        let stave = 0.8 + 0.2 * hash_f(u32(floor((ang + PI) * 30.0)));
        alb = col_hex(0x6e5a48u) * stave * (0.85 + 0.3 * noise_value2(vec2f(ang * 20.0, q.y * 2.0)));
        let hoop = step(0.82, fract((q.y - 6.2) / 0.55 + 0.5 * step(q.y, 9.0)));
        alb = mix(alb, vec3f(0.08, 0.07, 0.065), hoop);
        // water stains running down
        alb *= 0.75 + 0.25 * noise_value2(vec2f(ang * 8.0, q.y * 0.3));
    } else if (mat < 2.5) {
        alb = vec3f(0.12, 0.11, 0.1);
        alb = mix(alb, vec3f(0.85, 0.86, 0.9), l.snow * smoothstep(0.3, 0.6, n.y));
    } else {
        alb = vec3f(0.06, 0.055, 0.05);
    }
    // afterglow from behind the camera (+z, low), sky from above
    let kd = normalize(vec3f(0.35, 0.25, 1.0));
    let dif = saturate(dot(n, kd));
    let sky = 0.55 + 0.45 * n.y;
    var e = l.key * dif * 2.0 + l.amb * sky;
    // city glow from below at night
    e += vec3f(1.0, 0.6, 0.35) * 0.02 * l.dusk * saturate(-n.y * 0.5 + 0.5);
    var c = alb * INV_PI_M * e;
    // rim of sky light where the tank turns away
    c += l.amb * 0.08 * pow(1.0 - saturate(-dot(rd, n)), 4.0);
    return c;
}

// exhaust steam rising from a vent on a roof two blocks away, drifting east
fn steam(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx) -> vec4f {
    let zs = -182.0;
    let ts = (zs - ro.z) / min(rd.z, -1e-4);
    if (ts <= 0.0 || ts > tmax) { return vec4f(0.0); }
    let q = ro + rd * ts;
    let base = vec2f(38.0, 30.0);
    let v = q.y - base.y;
    if (v < 0.0 || v > 45.0) { return vec4f(0.0); }
    let drift = v * v * 0.012 + v * 0.25;
    let u = q.x - base.x - drift;
    let spread = 1.2 + v * 0.35;
    let shape = exp(-sq(u / spread)) * smoothstep(0.0, 2.0, v) * smoothstep(45.0, 12.0, v);
    let n = noise_fbm2(vec2f(u * 0.18, v * 0.12 - ctx.t * 0.35), 4);
    let n2 = noise_fbm2(vec2f(u * 0.4 + n * 2.0, v * 0.25 - ctx.t * 0.6), 3);
    let dens = saturate(shape * (n * 1.7 + n2 * 0.6 - 0.75)) * 0.75;
    // lit by the western afterglow and the sky; faintly by the city at night
    let c = l.key * 0.45 + l.amb * 0.9 + vec3f(1.0, 0.65, 0.4) * 0.05 * l.dusk;
    return vec4f(c, dens);
}

// an airliner on its way into LaGuardia every few minutes: a strobe and
// the red and green navigation lights
fn airliner(rd: vec3f, ctx: Ctx) -> vec3f {
    let ev = hash_event(ctx.t + 20.0, 150.0, 0xa17u);
    if (ev.y > 0.6) { return vec3f(0.0); }
    let s = ev.y / 0.6;
    let az = mix(-0.9, 0.7, s) + (ev.x - 0.5) * 0.2;
    let el = 0.1 + 0.08 * ev.x - 0.04 * s;
    let dir = normalize(vec3f(sin(az), el, -cos(az)));
    let d = acos(clamp(dot(rd, dir), -1.0, 1.0));
    let r = 0.0012;
    let strobe = step(0.9, fract(ctx.t * 1.1));
    let side = normalize(cross(dir, vec3f(0.0, 1.0, 0.0)));
    let dl = acos(clamp(dot(rd, normalize(dir + side * 0.0025)), -1.0, 1.0));
    let dr = acos(clamp(dot(rd, normalize(dir - side * 0.0025)), -1.0, 1.0));
    var c = vec3f(1.0) * 4.0 * strobe * exp(-sq(d / r));
    c += vec3f(1.0, 0.1, 0.05) * 1.5 * exp(-sq(dl / r));
    c += vec3f(0.1, 1.0, 0.3) * 1.2 * exp(-sq(dr / r));
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme, ctx.t);
    let ro = vec3f(0.0, EYE, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.06, 0.02, -1.0), 0.0, 48.0);
    let rd = cam_ray(cam, p);
    let zoom = cam.zoom;
    // the hero water tower
    let tb = tower_bounds(ro, rd);
    var hit = vec2f(-1.0);
    if (tb.y > tb.x) {
        hit = rm_march(ro, rd, tb.x, tb.y, steps(48.0, ctx), ctx);
    }
    let tmax = select(1e9, hit.x, hit.x > 0.0);
    let cty = city(ro, rd, l, ctx, zoom, tmax);
    var col = (dusk_sky(rd, l, ctx) + airliner(rd, ctx)) * (1.0 - cty.w) + cty.xyz;
    let sm = steam(ro, rd, tmax, l, ctx);
    col = mix(col, sm.xyz, sm.w);
    if (hit.x > 0.0) {
        let hp = ro + rd * hit.x;
        let n = rm_normal(hp, hit.x, ctx);
        col = shade_tower(hp, n, hit.y, rd, l, ctx);
        col = mix(col, l.hazec, 1.0 - exp(-hit.x * 0.001 * l.haze));
    }
    if (l.snow > 0.0) {
        let sn = snow_flakes(p * 2.8 + vec2f(0.0, 0.4), ctx, 0.35, 0.02, 3);
        col = mix(col, vec3f(0.1, 0.1, 0.11), sn * 0.7);
    }
    return col * exp2(l.exposure);
}
