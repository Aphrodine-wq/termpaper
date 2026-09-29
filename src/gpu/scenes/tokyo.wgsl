//! name: tokyo
//! title: Shinjuku Alley in the Rain
//! category: city
//! tags: tokyo, alley, rain, lanterns, signs, night, japan
//! desc: a narrow Shinjuku yokocho at night: stacked signs, red lanterns, wet asphalt mirroring it
//! themes: rain, clear, snow
//! uses: camera, raymarch, rain, snow, light, noise, water, wet
//! cost: medium
//! tonemap: aces
//! fallback: city
//! credits: original

// World units are metres. The alley runs down -z from the camera; it is cut
// into frontages ("segments") SEG metres long, each side hashed into its own
// little bar: facade setback, roof height, an awning, a vertical sign, a pair
// of paper lanterns, an air conditioner, an open door onto a lit counter. The
// far end opens onto a bright cross street. The floor, the sky and the empty
// core of the alley are analytic, so the march only runs where there is
// something to hit.
//
// Light is kept in physical proportion: emitters are radiances, surfaces get
// albedo/pi times irradiance from oriented emitters (signs shine along the
// alley, doors into it), so walls sit five to eight stops under the lamps as
// they do in a night photograph. The wet floor reflects each lamp as an
// anisotropic lobe, stretched vertically by 1/sin(grazing angle): the long
// streaks of a rainy street, with no noise.

const SEG: f32 = 3.4;
const W0: f32 = 1.5;
const NSEG: i32 = 22;
const Z_END: f32 = -74.8;     // -NSEG * SEG
const XST: f32 = 7.5;         // cross street width beyond the end
const CORE: f32 = 0.5;        // |x| < CORE holds nothing but air
const Y_TOP: f32 = 13.0;      // above every roof
const EAVE_Y: f32 = 2.45;
const VM_SEG: i32 = 2;        // the vending machine's frontage (right side)
const INV_PI: f32 = 0.318309886;

struct Look {
    wet: f32,        // 0 dry .. 1 soaked
    puddle: f32,     // puddle coverage
    rain: f32,       // rain streak density
    snow: f32,       // snowfall / snow cover
    fog: f32,        // extinction per metre
    scat: f32,       // in-scatter strength around lights
    sky_lo: vec3f,   // sky strip just above the roofs
    sky_hi: vec3f,
    amb: vec3f,      // skylight irradiance at the floor
    fogc: vec3f,     // haze radiance
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            // clear, dry night: crisp air, navy sky over a faint city glow
            return Look(0.1, 0.0, 0.0, 0.0, 0.004, 0.002,
                vec3f(0.016, 0.011, 0.016), vec3f(0.002, 0.003, 0.008),
                vec3f(0.008, 0.010, 0.016), vec3f(0.004, 0.004, 0.006), 0.2);
        }
        case 2u: {
            // snowfall: white ground, low cloud lit pink-orange by the city
            return Look(0.3, 0.0, 0.0, 1.0, 0.02, 0.005,
                vec3f(0.036, 0.030, 0.034), vec3f(0.012, 0.013, 0.018),
                vec3f(0.10, 0.11, 0.14), vec3f(0.016, 0.017, 0.022), -0.2);
        }
        default: {
            // rain: soaked asphalt, low cloud glowing with sodium and LED
            return Look(1.0, 0.62, 0.8, 0.0, 0.016, 0.006,
                vec3f(0.030, 0.020, 0.019), vec3f(0.007, 0.007, 0.010),
                vec3f(0.010, 0.011, 0.016), vec3f(0.010, 0.010, 0.013), 0.0);
        }
    }
}

// ------------------------------------------------------------ the frontages

struct Seg {
    wg: f32,     // ground-floor facade distance from the alley axis
    wu: f32,     // upper facade distance (upper floors may jut out)
    roof: f32,
    eave: f32,   // awning depth, 0 = none
    ey: f32,     // awning height
    sz: f32,     // sign centre z
    sy0: f32,    // sign bottom
    sy1: f32,    // sign top
    sd: f32,     // sign depth from the wall, 0 = none
    lant: f32,   // lanterns 0/1
    acz: f32,
    acy: f32,
    ac: f32,     // air conditioner 0/1
    shop: f32,   // < 0.8 open bar, else shuttered
    pal: f32,    // sign palette
    lamp: f32,   // lantern colour: < 0.9 red paper, else ivory
    stand: f32,  // standing signboard position along the frontage, 0 = none
}

fn seg_wall(i: i32, s: i32) -> vec3f {
    let h = hash_cell2(vec2i(i, s), 0x70c1u);
    return vec3f(W0 + 0.08 + 0.3 * h.x, W0 - 0.08 + 0.26 * h.y, 5.8 + 6.0 * h.z * h.z);
}

fn seg_info(i: i32, s: i32) -> Seg {
    let w = seg_wall(i, s);
    let g = hash_cell2(vec2i(i, s), 0x5a17u);
    let k = hash_cell2(vec2i(i, s), 0x3c6ef372u);
    let z0 = -f32(i) * SEG;
    var sg: Seg;
    sg.wg = w.x;
    sg.wu = w.y;
    sg.roof = w.z;
    sg.eave = select(0.0, 0.7 + 0.3 * g.x, g.y < 0.85);
    sg.ey = EAVE_Y + 0.12 * g.z;
    sg.sz = z0 - SEG * (0.28 + 0.44 * g.w);
    sg.sy0 = 2.95 + 0.5 * k.y;
    sg.sy1 = min(sg.sy0 + 1.6 + 2.8 * k.z, sg.roof - 0.25);
    sg.sd = select(0.0, 0.42 + 0.2 * k.w, k.x < 0.88);
    sg.lant = select(0.0, 1.0, fract(g.x * 13.7 + g.z * 3.1) < 0.7);
    sg.ac = select(0.0, 1.0, fract(k.y * 7.9) < 0.55);
    sg.acz = select(z0 - SEG * 0.22, z0 - SEG * 0.78, g.w < 0.5);
    sg.acy = select(4.4, 3.5, fract(k.z * 5.3) < 0.5) + 0.2 * k.w;
    sg.shop = fract(g.y * 17.3 + k.x * 5.7);
    sg.pal = fract(k.w * 11.3 + g.x * 2.9);
    sg.lamp = fract(k.z * 23.1);
    let st = fract(g.z * 31.7 + k.w * 3.9);
    sg.stand = select(0.0, select(0.14, 0.88, st < 0.3), st < 0.55 && sg.shop < 0.8);
    // the frontages nearest the camera are open bars under red lanterns
    if (i >= -2 && i <= 1) {
        sg.shop = select(0.2, 0.4, s == 1);
        sg.lant = 1.0;
        sg.lamp = 0.3;
        sg.eave = max(sg.eave, 0.8);
    }
    if (s == 1 && i == VM_SEG) { sg.lant = 0.0; sg.shop = 0.9; }
    return sg;
}

// a sign's panel and lettering colours
fn sign_cols(pal: f32) -> array<vec3f, 2> {
    if (pal < 0.34) { return array<vec3f, 2>(vec3f(1.0, 0.95, 0.86), vec3f(0.015, 0.012, 0.01)); }   // white, black
    if (pal < 0.50) { return array<vec3f, 2>(vec3f(1.0, 0.93, 0.85), vec3f(0.7, 0.015, 0.01)); }     // white, red
    if (pal < 0.64) { return array<vec3f, 2>(vec3f(1.0, 0.55, 0.1), vec3f(0.02, 0.012, 0.005)); }    // amber, black
    if (pal < 0.80) { return array<vec3f, 2>(vec3f(0.8, 0.045, 0.02), vec3f(1.0, 0.92, 0.8)); }      // red, white
    if (pal < 0.88) { return array<vec3f, 2>(vec3f(0.012, 0.012, 0.014), vec3f(1.0, 0.3, 0.5)); }    // black, pink neon
    return array<vec3f, 2>(vec3f(0.7, 0.85, 1.0), vec3f(0.015, 0.04, 0.2));                           // cool white, blue
}
// average emitted colour of a sign face (radiance)
fn sign_avg(pal: f32) -> vec3f {
    let c = sign_cols(pal);
    return mix(c[0], c[1], 0.3) * 2.6;
}
fn lamp_col(lamp: f32) -> vec3f {
    return select(vec3f(1.0, 0.12, 0.025), vec3f(1.0, 0.6, 0.28), lamp > 0.9);
}

// ------------------------------------------------------------ lights

// One emitter: position, radiant intensity along its axis (or all round when
// axis = 0), softening radius. `two` = shines both ways along the axis.
struct Lit { p: vec3f, c: vec3f, axis: vec3f, r: f32, two: f32 }

fn seg_lights(j: i32, s: i32) -> array<Lit, 5> {
    let sg = seg_info(j, s);
    let sx = select(-1.0, 1.0, s == 1);
    let z0 = -f32(j) * SEG;
    let none = Lit(vec3f(0.0, -50.0, 0.0), vec3f(0.0), vec3f(0.0), 1.0, 0.0);
    var o = array<Lit, 5>(none, none, none, none, none);
    if (sg.lant > 0.5) {
        let lc = lamp_col(sg.lamp) * 0.2;
        o[0] = Lit(vec3f(sx * (sg.wg - 0.42), sg.ey - 0.45, z0 - SEG * 0.27), lc, vec3f(0.0), 0.16, 0.0);
        o[1] = Lit(vec3f(sx * (sg.wg - 0.42), sg.ey - 0.45, z0 - SEG * 0.73), lc, vec3f(0.0), 0.16, 0.0);
    }
    if (sg.sd > 0.0) {
        let h = sg.sy1 - sg.sy0;
        o[2] = Lit(vec3f(sx * (sg.wu - sg.sd * 0.5), (sg.sy0 + sg.sy1) * 0.5, sg.sz),
                   sign_avg(sg.pal) * sg.sd * h * 0.35, vec3f(0.0, 0.0, 1.0), 0.3 + 0.25 * h, 1.0);
    }
    if (sg.shop < 0.8) {
        // the open door: warm light pouring out into the alley
        o[3] = Lit(vec3f(sx * (sg.wg - 0.05), 1.15, z0 - SEG * 0.5),
                   vec3f(1.0, 0.6, 0.3) * 2.2, vec3f(-sx, 0.0, 0.0), 1.0, 0.0);
    }
    if (sg.stand > 0.0) {
        let pal = fract(sg.pal + 0.61);
        o[4] = Lit(vec3f(sx * (sg.wg - 0.3), 0.55, z0 - SEG * sg.stand), sign_avg(pal) * 0.12, vec3f(0.0, 0.0, 1.0), 0.3, 1.0);
    }
    if (s == 1 && j == VM_SEG) {
        o[4] = Lit(vec3f(sg.wg - 0.52, 1.3, z0 - SEG * 0.55 + 0.45), vec3f(0.75, 0.88, 1.0) * 2.2, vec3f(0.0, 0.0, 1.0), 0.5, 0.0);
    }
    return o;
}

// intensity of emitter L toward unit direction `to` (from the light outward)
fn lit_i(L: Lit, to: vec3f) -> vec3f {
    if (dot(L.axis, L.axis) < 0.5) { return L.c; }
    let k = dot(L.axis, to);
    return L.c * select(max(k, 0.0), abs(k), L.two > 0.5);
}

// irradiance at p (normal n) from the emitters of the frontages around it
fn irradiance(p: vec3f, n: vec3f) -> vec3f {
    let i = i32(floor(-p.z / SEG));
    var e = vec3f(0.0);
    for (var j = i - 1; j <= i + 1; j++) {
        if (j < -2 || j >= NSEG) { continue; }
        for (var s = 0; s < 2; s++) {
            var ls = seg_lights(j, s);
            for (var m = 0; m < 5; m++) {
                let L = ls[m];
                let d = L.p - p;
                let d2 = dot(d, d);
                let dir = d * inverseSqrt(max(d2, 1e-6));
                let nl = saturate(dot(n, dir) * 0.9 + 0.1);
                e += lit_i(L, -dir) * nl / (d2 + L.r * L.r);
            }
        }
    }
    return e;
}

// halo of a lamp in damp air: a tight kernel around the closest approach of
// the ray (the long 1/d tail of true single scattering, summed over dozens of
// lamps, would only grey the whole alley)
fn halo_pt(ro: vec3f, rd: vec3f, tmax: f32, lp: vec3f, col: vec3f, r: f32) -> vec3f {
    let w = lp - ro;
    let tc = dot(w, rd);
    if (tc < 0.0 || tc > tmax + r) { return vec3f(0.0); }
    let d2 = max(dot(w, w) - tc * tc, 0.0);
    let r2 = r * r;
    return col * r2 / sq(d2 + r2);
}

// single scattering of a point emitter in the air along [0, tmax]: the
// closed form of the inverse-square line integral
fn scatter_pt(ro: vec3f, rd: vec3f, tmax: f32, lp: vec3f, col: vec3f, r: f32) -> vec3f {
    let w = lp - ro;
    let tc = dot(w, rd);
    let d = sqrt(max(dot(w, w) - tc * tc, 0.0) + r * r);
    let a = atan((tmax - tc) / d) + atan(tc / d);
    return col * a / d;
}

fn air_glow(ro: vec3f, rd: vec3f, tmax: f32, l: Look) -> vec3f {
    let end = ro + rd * tmax;
    let i0 = max(i32(floor(-ro.z / SEG)) - 1, -2);
    let i1 = min(i32(floor(-end.z / SEG)), NSEG - 1);
    var g = vec3f(0.0);
    for (var k = 0; k < 18; k++) {
        let j = i0 + k;
        if (j > i1 + 1 || j >= NSEG) { break; }
        for (var s = 0; s < 2; s++) {
            var ls = seg_lights(j, s);
            for (var m = 0; m < 5; m++) {
                let L = ls[m];
                if (L.c.x + L.c.y + L.c.z <= 0.0) { continue; }
                // signs scatter about a third of their peak all round; the
                // doorways sit low behind cloth and hardly light the air
                var w = select(0.35, 1.0, dot(L.axis, L.axis) < 0.5);
                if (m >= 3) { w = 0.03; }
                g += halo_pt(ro, rd, tmax, L.p, L.c * w, L.r * 2.2 + 0.25);
            }
        }
    }
    // the bright cross street at the end
    g *= l.scat * 12.0;
    g += scatter_pt(ro, rd, tmax, vec3f(0.0, 2.5, Z_END - 3.5), vec3f(1.0, 0.82, 0.66) * 6.0, 3.0) * l.scat;
    return g;
}

// ------------------------------------------------------------ geometry

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    let ax = abs(p.x);
    let s = select(0, 1, p.x > 0.0);
    let fi = -p.z / SEG;
    let i = i32(floor(fi));
    let zl = (fi - floor(fi)) * SEG;          // metres into the frontage
    // nothing on the far side can be nearer than this
    var res = vec2f(ax + CORE, 0.0);
    if (i >= NSEG) {
        return res;
    }
    let q = vec3f(ax, p.y, p.z);
    let z0 = -f32(i) * SEG;
    let sg = seg_info(i, s);
    // this building and the nearer neighbour: boxes along z
    let jn = select(i - 1, i + 1, zl > SEG * 0.5);
    for (var k = 0; k < 2; k++) {
        let j = select(i, jn, k == 1);
        if (j < -3 || j >= NSEG) { continue; }
        let w = select(seg_wall(j, s), vec3f(sg.wg, sg.wu, sg.roof), k == 0);
        let zc = -(f32(j) + 0.5) * SEG;
        let dz = abs(p.z - zc) - SEG * 0.5;
        let dg = max(max(w.x - ax, p.y - 2.62), dz);
        let du = max(max(max(w.y - ax, 2.62 - p.y), p.y - w.z), dz);
        res = op_umin(res, vec2f(dg, 2.0));
        res = op_umin(res, vec2f(du, 3.0));
    }
    // protrusions of this frontage; the neighbours' keep >= 0.15 m from
    // the boundary, so a step never jumps past one
    if (ax > W0 - 1.2 && p.y > 1.4 && p.y < 11.0) {
        if (sg.eave > 0.0) {
            let c = vec3f(sg.wg - sg.eave * 0.5, sg.ey - 0.07 * (sg.wg - ax), z0 - SEG * 0.5);
            let e = sdf_box(q - c, vec3f(sg.eave * 0.5, 0.03, SEG * 0.5 - 0.15));
            res = op_umin(res, vec2f(e, 4.0));
        }
        if (sg.sd > 0.0) {
            let c = vec3f(sg.wu - sg.sd * 0.5 - 0.06, (sg.sy0 + sg.sy1) * 0.5, sg.sz);
            let b = sdf_box(q - c, vec3f(sg.sd * 0.5, (sg.sy1 - sg.sy0) * 0.5, 0.07));
            // the bracket holding it off the wall
            let br = sdf_box(q - vec3f(sg.wu - 0.03, sg.sy1 - 0.1, sg.sz), vec3f(0.04, 0.03, 0.03));
            res = op_umin(res, vec2f(min(b, br), 5.0));
        }
        if (sg.lant > 0.5) {
            for (var k = 0; k < 2; k++) {
                let c = vec3f(sg.wg - 0.42, sg.ey - 0.45, z0 - SEG * (0.27 + 0.46 * f32(k)));
                let e = sdf_ellipsoid(q - c, vec3f(0.16, 0.23, 0.16));
                res = op_umin(res, vec2f(e, 6.0));
            }
        }
        if (sg.ac > 0.5) {
            let c = vec3f(sg.wu - 0.2, sg.acy, sg.acz);
            res = op_umin(res, vec2f(sdf_box(q - c, vec3f(0.2, 0.27, 0.37)) - 0.02, 7.0));
        }
        res.x = min(res.x, min(zl, SEG - zl) + 0.15);
    }
    // a lit signboard standing on the ground by the door
    if (sg.stand > 0.0 && ax > W0 - 0.9 && p.y < 1.2) {
        let c = vec3f(sg.wg - 0.3, 0.5, z0 - SEG * sg.stand);
        res = op_umin(res, vec2f(sdf_box(q - c, vec3f(0.2, 0.46, 0.11)) - 0.02, 9.0));
        res.x = min(res.x, min(zl, SEG - zl) + 0.15);
    }
    if (s == 1 && i == VM_SEG) {
        let c = vec3f(sg.wg - 0.52, 0.92, z0 - SEG * 0.55);
        res = op_umin(res, vec2f(sdf_box(q - c, vec3f(0.5, 0.92, 0.36)) - 0.02, 8.0));
    }
    return res;
}

// ------------------------------------------------------------ lettering

// abstract kanji-like glyph in the unit cell: radicals and strokes picked by
// the bits of id. Returns ink coverage; fades to average ink below a pixel.
fn glyph(uv: vec2f, id: u32, aa: f32) -> f32 {
    let h = hash_u(id);
    var d = 1e3;
    // horizontal strokes
    for (var k = 0u; k < 4u; k++) {
        if (((h >> k) & 1u) != 0u) {
            let y = 0.12 + 0.25 * f32(k);
            let x0 = select(0.1, 0.35, ((h >> (k + 8u)) & 1u) != 0u);
            let x1 = select(0.9, 0.62, ((h >> (k + 12u)) & 1u) != 0u);
            d = min(d, sdf2_segment(uv, vec2f(x0, y), vec2f(x1, y)));
        }
    }
    // verticals
    for (var k = 0u; k < 3u; k++) {
        if (((h >> (k + 4u)) & 1u) != 0u) {
            let x = 0.2 + 0.3 * f32(k);
            let y0 = select(0.08, 0.4, ((h >> (k + 16u)) & 1u) != 0u);
            let y1 = select(0.92, 0.6, ((h >> (k + 19u)) & 1u) != 0u);
            d = min(d, sdf2_segment(uv, vec2f(x, y0), vec2f(x, y1)));
        }
    }
    // a box radical (like 口) in one corner
    if (((h >> 22u) & 3u) == 0u) {
        let c = select(vec2f(0.28, 0.3), vec2f(0.7, 0.72), ((h >> 24u) & 1u) != 0u);
        d = min(d, abs(sdf2_box(uv - c, vec2f(0.16, 0.14))));
    }
    // sweeping legs (like 八)
    if (((h >> 7u) & 1u) != 0u) {
        d = min(d, sdf2_segment(uv, vec2f(0.45, 0.5), vec2f(0.12, 0.06)));
        d = min(d, sdf2_segment(uv, vec2f(0.55, 0.45), vec2f(0.9, 0.06)));
    }
    let w = 0.055;
    let fine = saturate(0.5 - (d - w) / max(aa, 1e-3));
    return mix(fine, 0.3, saturate(aa * 3.0 - 0.4));
}

// emitted light of a sign face; uv: x 0 (wall) .. 1 (tip), y metres up
fn sign_face(uv: vec2f, hgt: f32, depth: f32, pal: f32, id: u32, aa: f32, t: f32) -> vec3f {
    let c = sign_cols(pal);
    let bx = 0.1;
    let by = 0.08;
    let inner = uv.x > bx && uv.x < 1.0 - bx && uv.y > by && uv.y < hgt - by;
    let cell = depth * (1.0 - 2.0 * bx) * 1.05;
    let row = floor((uv.y - by) / cell);
    let gy = 1.0 - fract((uv.y - by) / cell);
    let gx = (uv.x - bx) / (1.0 - 2.0 * bx);
    let ink = glyph(vec2f(gx, gy), id * 31u + u32(max(row, 0.0)), aa / cell) * select(0.0, 1.0, inner);
    var e = mix(c[0], c[1], ink);
    // the frame is a darker rim; backlit panels are brightest in the middle
    e *= select(0.3, 1.0 - 0.25 * sq(uv.x * 2.0 - 1.0), inner);
    // one sign in eight is a tired tube, breathing slowly
    let fl = select(1.0, light_flicker(t, id, 0.5), (id & 7u) == 3u);
    return e * 2.8 * fl;
}

// ------------------------------------------------------------ shading

fn gap_sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let y = saturate(rd.y);
    var c = mix(l.sky_lo, l.sky_hi, sqrt(y));
    let uv = rd.xz / max(rd.y, 0.05);
    let n = noise_fbm2(uv * 0.35 + vec2f(ctx.t * 0.012, 0.0), 4);
    return c * (0.7 + 0.6 * n);
}

// the cross street across the end of the alley: lit shopfronts, passers-by
fn far_street(p: vec3f, aa: f32, l: Look, ctx: Ctx) -> vec3f {
    let x = p.x;
    let y = p.y;
    let bay = floor(x / 3.2);
    let h = hash_cell2(vec2i(i32(bay), 0), 0x6b43u);
    var c = vec3f(0.02, 0.018, 0.02);
    let fx = fract(x / 3.2);
    let win = smoothstep(0.04, 0.1, fx) * smoothstep(0.96, 0.9, fx) * smoothstep(3.2, 2.9, y);
    let wc = mix(vec3f(1.0, 0.82, 0.6), vec3f(0.85, 0.95, 1.0), step(0.55, h.x));
    c += wc * win * (1.5 + 2.5 * h.y);
    let sb = step(3.5, y) * step(y, 4.5) * step(0.12, fx) * step(fx, 0.88) * step(0.35, h.z);
    c += sign_avg(h.w) * sb;
    let uw = step(5.5, y) * step(fract(y / 3.0), 0.5) * step(0.3, fract(x / 1.6)) *
             step(0.55, hash_cell2(vec2i(i32(floor(x / 1.6)), i32(floor(y / 3.0))), 9u).x);
    c += vec3f(1.0, 0.8, 0.6) * uw * 0.5;
    // people walking along the cross street, dark against the light
    for (var k = 0; k < 4; k++) {
        let ev = hash_event(ctx.t + f32(k) * 7.3, 12.0, 0x9e1u + u32(k));
        let dir = select(-1.0, 1.0, ev.x > 0.5);
        let px = dir * (-7.0 + 14.0 * ev.y) + (ev.x - 0.5) * 2.0;
        let bob = abs(sin(ctx.t * 5.0 + f32(k))) * 0.03;
        let body = sdf2_round_box(vec2f(x - px, y - 1.05 - bob), vec2f(0.2, 0.55), 0.15);
        let head = length(vec2f(x - px, y - 1.72 - bob)) - 0.12;
        var fig = min(body, head);
        if (l.rain > 0.0 || l.snow > 0.0) {
            let u = vec2f(x - px, y - 2.05);
            fig = min(fig, max(length(u * vec2f(1.0, 2.2)) - 0.55, -u.y));
        }
        c *= mix(1.0, 0.06, saturate(0.5 - fig / max(aa, 0.02)));
    }
    return c;
}

// inside an open bar: the ray continues through the doorway into a shallow
// room (interior mapping). hp is on the door plane, side sx, frontage i.
fn bar_interior(hp: vec3f, rd: vec3f, sg: Seg, i: i32, sx: f32, aa: f32) -> vec3f {
    let z0 = -f32(i) * SEG;
    let depth = 2.4;
    let rdx = max(abs(rd.x), 1e-4);
    // distances to the back wall, ceiling / floor, side walls
    let tb = depth / rdx;
    var tc = 1e3;
    if (rd.y > 1e-4) { tc = (2.35 - hp.y) / rd.y; }
    if (rd.y < -1e-4) { tc = -hp.y / rd.y; }
    var ts = 1e3;
    if (rd.z < -1e-4) { ts = (z0 - SEG + 0.12 - hp.z) / rd.z; }
    if (rd.z > 1e-4) { ts = (z0 - 0.12 - hp.z) / rd.z; }
    let t = min(tb, min(tc, ts));
    let q = hp + rd * t;
    let warm = vec3f(1.0, 0.62, 0.32);
    var c: vec3f;
    let hs = hash_cell2(vec2i(i, i32(sx)), 0xba5u);
    if (t == tb) {
        // back wall: shelves of bottles over a counter, paper menu strips
        let u = q.z;
        c = warm * 0.16 * (0.8 + 0.4 * hs.x);
        let shelf = step(1.25, q.y) * step(q.y, 1.95);
        let row = fract((q.y - 1.25) / 0.35);
        let bot = smoothstep(0.4, 0.3, abs(fract(u * 9.0) - 0.5)) * step(0.15, row) * step(row, 0.85);
        let bh = hash_cell2(vec2i(i32(floor(u * 9.0)), i32(floor((q.y - 1.25) / 0.35))), 0xb07u);
        let bc = mix(vec3f(0.35, 0.18, 0.05), vec3f(0.15, 0.3, 0.12), step(0.6, bh.x)) * (1.0 + 2.0 * step(0.85, bh.y));
        c = mix(c, bc, bot * shelf);
        let menu = step(2.0, q.y) * step(q.y, 2.3) * step(0.3, fract(u * 4.0)) * step(fract(u * 4.0), 0.8);
        c = mix(c, vec3f(1.0, 0.9, 0.75) * 0.5, menu);
        c *= 0.35 + 0.8 * smoothstep(0.3, 2.3, q.y);
    } else if (t == tc && rd.y > 0.0) {
        // ceiling with a bare bulb in a paper shade
        let lamp = length(vec2f(q.x - sx * (sg.wg + 1.1), q.z - (z0 - SEG * 0.5)));
        c = warm * 0.08 + warm * 2.0 * smoothstep(0.22, 0.04, lamp);
    } else if (t == tc) {
        c = warm * 0.03;
    } else {
        c = warm * 0.09 * (0.7 + 0.3 * hs.y);
    }
    // the counter: a wooden band across the room
    let tcounter = 0.75 / rdx;
    let qc = hp + rd * tcounter;
    if (tcounter < t && qc.y < 1.0) {
        c = mix(vec3f(0.07, 0.035, 0.015), warm * 0.25, step(0.93, qc.y));
    }
    // customers on stools, backs to the alley
    let tp = 0.45 / rdx;
    let qp = hp + rd * tp;
    let u = (qp.z - (z0 - SEG)) / SEG;
    for (var k = 0; k < 3; k++) {
        let hk = hash_cell2(vec2i(i, k + 5 * i32(sx)), 0x5eedu);
        if (hk.x > 0.6) { continue; }
        let cz = 0.2 + 0.6 * (f32(k) + hk.y) / 3.0;
        let dx = (u - cz) * SEG;
        let yb = qp.y - 0.8;
        let body = sdf2_round_box(vec2f(dx, yb), vec2f(0.2 - 0.06 * saturate(-yb * 2.0), 0.42), 0.16);
        let head = length(vec2f(dx * 1.1, qp.y - 1.4 - 0.03 * hk.z)) - 0.1;
        let f = op_smin(body, head, 0.05);
        let shirt = select(vec3f(0.012, 0.012, 0.014), vec3f(0.05, 0.045, 0.04), hk.w > 0.6);
        c = mix(c, shirt + warm * 0.02 * saturate(1.0 + f * 12.0), saturate(0.5 - f / max(aa, 0.01)) * step(tp, t));
    }
    return c;
}

// the paper lantern: glowing red (or ivory) paper, ribs, black caps, a
// character brushed on the face toward the alley's mouth
fn lantern(q: vec3f, c: vec3f, lamp: f32, id: u32, aa: f32) -> vec3f {
    let d = q - c;
    let v = d.y / 0.23;
    let ang = atan2(d.x, d.z);          // 0 = facing +z, toward the camera
    let rib = 0.82 + 0.18 * cos(v * 30.0);
    let cap = smoothstep(0.8, 0.9, abs(v));
    // the bulb inside: a hot yellow-orange heart fading to deep red paper
    let rim = length(d.xz) / 0.16;
    let core = pow(saturate(1.0 - 0.8 * rim * rim - 0.35 * v * v), 1.5);
    var e = lamp_col(lamp) * (0.22 + 0.2 * rib) + vec3f(1.0, 0.36, 0.08) * core * 0.75;
    let g = glyph(vec2f(ang / 1.4 + 0.5, 0.5 - v * 0.62), id, aa * 4.0);
    e = mix(e, vec3f(0.02, 0.005, 0.0), g * 0.85 * step(abs(ang), 0.8) * step(abs(v), 0.7));
    return e * (1.0 - cap * 0.95);
}

fn shade(p: vec3f, n: vec3f, mat: f32, rd: vec3f, t: f32, aa: f32, l: Look, ctx: Ctx) -> vec3f {
    let s = select(0, 1, p.x > 0.0);
    let sx = select(-1.0, 1.0, s == 1);
    let ax = abs(p.x);
    let i = i32(floor(-p.z / SEG));
    let zl = fract(-p.z / SEG);
    let sg = seg_info(i, s);
    let id = u32(i + 1000) * 2u + u32(s);
    var alb = vec3f(0.2);
    var emi = vec3f(0.0);
    var gloss = 0.0;
    if (mat > 8.5) {
        // standing signboard: two lit faces, lettering in a column
        alb = vec3f(0.05);
        if (abs(n.z) > 0.6) {
            let u = ((sg.wg - 0.3) - ax) / 0.4 + 0.5;
            let pal = fract(sg.pal + 0.61);
            emi = sign_face(vec2f(u, p.y - 0.06), 0.88, 0.4, pal, id * 5u + 1u, aa, ctx.t) * 0.8;
        }
    } else if (mat > 7.5) {
        // vending machine: a lit product window over a white body
        let u = ((sg.wg - 0.52) - ax) / 0.5;   // -1..1 across the face
        let v = p.y;
        alb = vec3f(0.6, 0.62, 0.65);
        if (n.z > 0.5) {
            let win = step(abs(u), 0.86) * step(0.98, v) * step(v, 1.74);
            let rows = fract((v - 0.98) / 0.25);
            let cols = fract((u + 0.86) / 0.172);
            let can = smoothstep(0.34, 0.24, abs(cols - 0.5)) * smoothstep(0.12, 0.25, rows) * smoothstep(0.8, 0.62, rows);
            let hc = hash_cell2(vec2i(i32(floor((u + 0.86) / 0.172)), i32(floor((v - 0.98) / 0.25))), 0x77u);
            let cc = select(select(vec3f(0.9, 0.15, 0.1), vec3f(0.15, 0.4, 0.9), hc.x > 0.4), vec3f(0.95, 0.8, 0.2), hc.x > 0.8);
            emi = win * mix(vec3f(0.85, 0.92, 1.0) * 3.5, cc * 1.6, can);
            emi += step(abs(u), 0.86) * step(0.8, v) * step(v, 0.92) * vec3f(0.6, 0.8, 1.0) * 1.0;
        }
    } else if (mat > 6.5) {
        // air conditioner: grimy off-white box with a dark fan grille
        alb = vec3f(0.42, 0.42, 0.4) * (0.75 + 0.25 * noise_value2(p.xy * 9.0));
        if (n.z > 0.5) {
            let c = vec2f(ax - (sg.wu - 0.2), p.y - sg.acy);
            let r = length(c);
            alb *= mix(1.0, 0.2, smoothstep(0.2, 0.18, r) * (0.7 + 0.3 * sin(r * 120.0)));
        }
    } else if (mat > 5.5) {
        let z0 = -f32(i) * SEG;
        let k = select(0.0, 1.0, zl > 0.5);
        let c = vec3f(sg.wg - 0.42, sg.ey - 0.45, z0 - SEG * (0.27 + 0.46 * k));
        return lantern(vec3f(ax, p.y, p.z), c, sg.lamp, id * 3u + u32(k), aa / 0.3);
    } else if (mat > 4.5) {
        // vertical sign: lit faces, dark metal edges
        alb = vec3f(0.05);
        if (abs(n.z) > 0.6) {
            let u = (sg.wu - 0.06 - ax) / sg.sd;
            emi = sign_face(vec2f(u, p.y - sg.sy0), sg.sy1 - sg.sy0, sg.sd, sg.pal, id, aa, ctx.t);
        }
    } else if (mat > 3.5) {
        // corrugated awning, seen from below; the lip catches the light
        let ribs = 0.7 + 0.3 * cos(p.z * 42.0);
        let tint = select(vec3f(0.22, 0.2, 0.18), vec3f(0.3, 0.06, 0.04), sg.pal > 0.6);
        alb = tint * ribs;
        if (abs(n.z) > 0.5 || abs(n.x) > 0.5) { alb = tint * 1.4; }
    } else if (mat > 2.5) {
        // upper floors: tile or plaster, frosted windows, rain stains, a pipe
        let kind = fract(sg.pal * 7.1 + sg.shop * 3.3);
        var base = mix(col_hex(0x77716au), col_hex(0x4d4843u), kind);
        if (kind > 0.72) { base = col_hex(0x6a4a3au); }
        let stain = noise_fbm2(vec2f(p.z * 3.0, p.y * 0.35), 3);
        alb = base * (0.45 + 0.55 * stain);
        if (abs(n.x) > 0.5) {
            let fy = (p.y - 2.62) / 2.7;
            let fl = floor(fy);
            let wy = fract(fy);
            let wz = fract(zl * 2.0);
            let wh = hash_cell2(vec2i(i * 2 + i32(zl * 2.0), i32(fl) + s * 17), 0x1f3u);
            let win = step(0.22, wz) * step(wz, 0.78) * step(0.3, wy) * step(wy, 0.8) * step(0.3, wh.x);
            if (win > 0.5) {
                alb = vec3f(0.03);
                gloss = 0.5;
                let lit = step(0.45, wh.y);
                emi = mix(vec3f(1.0, 0.68, 0.4), vec3f(0.78, 0.88, 1.0), step(0.6, wh.z)) * lit * (0.25 + 0.5 * wh.w);
                let mull = step(abs(wz - 0.5), 0.015) + step(abs(wy - 0.55), 0.018);
                emi *= 1.0 - saturate(mull);
            }
            // a drain pipe down the corner of the frontage
            let pz = abs(zl - 0.04) * SEG;
            alb = mix(alb, vec3f(0.12, 0.12, 0.11) * (0.6 + 0.8 * saturate(1.0 - pz / 0.05)), step(pz, 0.05));
        }
    } else if (mat > 1.5) {
        // shopfront: sliding doors onto a lit counter, noren, a fascia board
        alb = vec3f(0.13, 0.085, 0.055) * (0.8 + 0.3 * noise_value2(vec2f(zl * 40.0, p.y * 2.0)));
        if (abs(n.x) > 0.5) {
            // a lit menu board on the pillar between doorways
            let mz = zl - 0.9;
            if (abs(mz) < 0.045 && p.y > 0.8 && p.y < 1.85) {
                let mb = glyph(vec2f(0.5 + mz / 0.09, fract((1.85 - p.y) / 0.26)), id * 13u + u32((1.85 - p.y) / 0.26), aa * 8.0);
                emi = mix(vec3f(1.0, 0.9, 0.7) * 1.1, vec3f(0.03, 0.02, 0.02), mb * 0.8);
                alb = vec3f(0.0);
            }
            let open = sg.shop < 0.8;
            let door = step(0.22, zl) * step(zl, 0.82) * step(p.y, 2.1);
            let frosted = i <= 1 || fract(sg.shop * 5.3) < 0.35;
            if (open && door > 0.5 && frosted) {
                // closed sliding doors of frosted glass in a wooden lattice,
                // glowing with the room behind; drinkers' shadows on the glass
                let dz = (zl - 0.22) / 0.6 * 4.0;
                let pz = fract(dz);
                let frame = step(pz, 0.05) + step(0.95, pz) + step(p.y, 0.38) + step(2.02, p.y);
                let kz = fract(pz * 3.0);
                let ky = fract(p.y * 3.4);
                let kumiko = step(0.94, kz) + step(0.95, ky);
                let hp = hash_cell2(vec2i(i * 4 + i32(dz), s), 0x9a5u);
                var glass = vec3f(1.0, 0.63, 0.33) * (0.05 + 0.07 * smoothstep(0.3, 2.0, p.y)) * (0.7 + 0.6 * hp.x);
                for (var k = 0; k < 3; k++) {
                    let hk = hash_cell2(vec2i(i, k + 5 * s), 0x5eedu);
                    if (hk.x > 0.7) { continue; }
                    let cz = (0.3 + 0.52 * (f32(k) + hk.y) / 3.0) * SEG;
                    let dx = zl * SEG - cz;
                    let body = sdf2_round_box(vec2f(dx, p.y - 0.95), vec2f(0.2, 0.42), 0.16);
                    let head = length(vec2f(dx, p.y - 1.45 - 0.04 * hk.z)) - 0.11;
                    let f = op_smin(body, head, 0.06);
                    glass *= 1.0 - 0.7 * smoothstep(0.1, -0.06, f);
                }
                emi = glass * (1.0 - 0.8 * saturate(kumiko)) * (1.0 - saturate(frame));
                alb = select(vec3f(0.0), vec3f(0.09, 0.05, 0.03), frame > 0.5);
            } else if (open && door > 0.5) {
                // lattice of the sliding doors (half open) and the glass
                let slide = step(0.5, zl);
                let lat = step(0.93, fract(zl * 14.0)) + step(0.94, fract(p.y * 3.3));
                emi = bar_interior(p, rd, sg, i, sx, aa) * (1.0 - 0.75 * saturate(lat) * slide);
                alb = vec3f(0.0);
            } else if (door > 0.5) {
                // closed: a roller shutter
                alb = vec3f(0.22, 0.23, 0.24) * (0.75 + 0.25 * cos(p.y * 70.0));
                gloss = 0.3;
            }
            if (open && door > 0.5) {
                // noren: indigo, red or white cloth over the upper doorway
                if (p.y > 1.55 && sg.shop < 0.55) {
                    let nz = fract((zl - 0.22) / 0.6 * 4.0);
                    let slit = step(0.95, nz) + step(nz, 0.02);
                    let nc = select(select(vec3f(0.012, 0.018, 0.05), vec3f(0.12, 0.012, 0.008), sg.shop < 0.3), vec3f(0.3, 0.29, 0.26), sg.shop < 0.12);
                    // backlit cloth: some of the room glows through
                    let cloth = nc * (0.5 + 0.8 * col_luma(emi));
                    let gl = glyph(vec2f((nz - 0.5) * 1.5 + 0.5, (2.0 - p.y) / 0.36), id + u32(zl * 7.0), aa * 3.0) * step(abs(nz - 0.5), 0.3) * step(1.64, p.y) * step(p.y, 2.0);
                    let ink = select(vec3f(0.9, 0.88, 0.8) * (0.02 + 0.5 * col_luma(emi)), vec3f(0.005), sg.shop < 0.12);
                    emi = mix(mix(cloth, ink, gl * 0.85), emi, slit);
                    alb = vec3f(0.0);
                }
            }
            // lit fascia board over the door
            if (p.y > 2.18 && p.y < 2.46 && zl > 0.2 && zl < 0.84 && open) {
                let fg = glyph(vec2f(fract(zl * 6.0), (2.46 - p.y) / 0.28), id * 7u + u32(zl * 6.0), aa * 3.5);
                let fc = sign_cols(fract(sg.pal + 0.37));
                emi = mix(fc[0], fc[1], fg * 0.9) * 0.9;
            }
        }
    }
    // lit by the lamps, signs and doorways around, and a little skylight
    var e = irradiance(p + n * 0.05, n);
    let open_sky = 0.25 + 0.75 * saturate((p.y - 2.0) / 9.0);
    e += l.amb * (0.4 + 0.6 * saturate(n.y + 0.3)) * open_sky;
    let ao = 0.5 + 0.5 * saturate(p.y / 0.6);
    var c = alb * INV_PI * e * ao + emi;
    // rain-soaked surfaces facing the camera pick up a faint sheen
    c += gloss * 0.04 * (l.wet * 0.5 + 0.5) * e * pow(1.0 - saturate(-dot(rd, n)), 3.0);
    // snow sticks to what faces up
    if (l.snow > 0.0 && n.y > 0.6) { c = mix(c, vec3f(0.85) * INV_PI * e + l.amb * 0.3, 0.8); }
    return c;
}

// march the mirror ray; shaded with softened detail
fn trace_reflection(ro: vec3f, rd: vec3f, blur: f32, l: Look, ctx: Ctx) -> vec3f {
    var t_hi = 200.0;
    if (rd.y > 1e-4) { t_hi = min(t_hi, (Y_TOP - ro.y) / rd.y); }
    let t_end = (Z_END - XST - ro.z) / min(rd.z, -1e-4);
    t_hi = min(t_hi, t_end);
    var t0 = 0.02;
    if (abs(ro.x) < CORE) {
        if (abs(rd.x) > 1e-5) {
            t0 = max(t0, (sign(rd.x) * CORE - ro.x) / rd.x);
        } else {
            t0 = t_hi;
        }
    }
    var c: vec3f;
    var th = t_hi;
    var hit = vec2f(-1.0);
    if (t0 < t_hi) {
        hit = rm_march(ro, rd, t0, t_hi, steps(64.0, ctx), ctx);
    }
    if (hit.x > 0.0) {
        th = hit.x;
        let hp = ro + rd * th;
        let n = rm_normal(hp, th, ctx);
        c = shade(hp, n, hit.y, rd, th, ctx.px * th + blur, l, ctx);
    } else if (abs(t_hi - t_end) < 1e-3 && (ro + rd * t_end).y < 24.0) {
        c = far_street(ro + rd * t_end, 0.1 + blur, l, ctx);
    } else {
        c = gap_sky(rd, l, ctx);
    }
    c = mix(l.fogc, c, exp(-min(th, 120.0) * l.fog));
    return c;
}

// wet asphalt, puddles, snow
fn shade_floor(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let ax = abs(p.x);
    let i = i32(floor(-p.z / SEG));
    let s = select(0, 1, p.x > 0.0);
    let wg = seg_wall(i, s).x;
    let edge = wg - ax;
    // patched asphalt, a line of drain covers along each side
    var alb = col_hex(0x3c3a38u) * (0.65 + 0.5 * noise_fbm2(p.xz * vec2f(1.5, 0.8), 3));
    let drain = step(0.08, edge) * step(edge, 0.42);
    alb = mix(alb, col_hex(0x2a2c2eu) * (0.7 + 0.3 * step(0.5, fract(p.z * 5.0))), drain);
    let pud = wet_puddles(p.xz * vec2f(0.8, 0.3) + vec2f(3.0, 0.0), l.puddle * (1.0 - 0.9 * drain), ctx);
    // snow: white along the walls, a trodden wet track down the middle
    let track = smoothstep(0.3, 0.75, abs(p.x + 0.12 * sin(p.z * 0.37)) + 0.25 * (noise_fbm2(p.xz * vec2f(2.0, 0.6), 3) - 0.5));
    let snowc = l.snow * mix(0.35, 1.0, track);
    // gentle undulation keeps reflections coherent; rain rings in puddles
    let wob = vec2f(noise_grad2(p.xz * vec2f(1.1, 0.5)), noise_grad2(p.xz * vec2f(1.1, 0.5) + 9.0)) * 0.02;
    var nrm = vec3f(wob.x, 1.0, wob.y);
    if (l.rain > 0.0) {
        let rip = water_rain_ripples(p.xz, ctx.t, 3.0);
        nrm += vec3f(rip.x, 0.0, rip.y) * 0.1 * pud;
    }
    let n = normalize(nrm);
    let wet = max(l.wet, pud);
    alb *= mix(1.0, 0.45, wet);
    alb = mix(alb, vec3f(0.75, 0.77, 0.8), snowc);
    let v = -rd;
    let sn = max(-rd.y, 0.02);          // sine of the grazing angle
    let cs = sqrt(1.0 - sn * sn);
    let r = reflect(rd, n);
    let fres = 0.02 + 0.98 * pow(1.0 - saturate(dot(v, n)), 5.0);
    // slope spread of the surface: rough dry asphalt, wet film, puddle
    let sigma = mix(mix(0.2, 0.035, l.wet), 0.006, pud);
    let spec_k = mix(0.25, 1.0, wet) * (1.0 - snowc);
    // walk the lamps down the alley: diffuse from the near ones, specular
    // streaks from all of them
    var e = vec3f(0.0);
    var spec = vec3f(0.0);
    for (var k = 0; k < 9; k++) {
        let j = i - 1 + k;
        if (j >= NSEG) { break; }
        for (var ss = 0; ss < 2; ss++) {
            var ls = seg_lights(j, ss);
            for (var m = 0; m < 5; m++) {
                let L = ls[m];
                let d = L.p - p;
                let d2 = dot(d, d);
                let dir = d * inverseSqrt(max(d2, 1e-6));
                let li = lit_i(L, -dir);
                if (k < 3) { e += li * saturate(dir.y) / (d2 + L.r * L.r); }
                // anisotropic lobe around the mirror direction
                let de = dir.y - r.y;
                let rh = normalize(r.xz + vec2f(1e-5));
                let da = dir.x * rh.y - dir.z * rh.x;
                let ang = L.r / sqrt(d2);
                let se = 2.0 * cs * sigma + ang;
                let sa = 2.0 * sn * sigma + ang * 0.7 + 0.002;
                let lobe = exp(-0.5 * (sq(de / se) + sq(da / sa))) / (TAU * se * sa);
                spec += li / (d2 + L.r * L.r) * lobe;
            }
        }
    }
    // skylight on the open floor; snow bounces it around
    e += l.amb * (0.5 + 2.5 * snowc);
    var c = alb * INV_PI * e * (0.55 + 0.45 * saturate(edge * 2.5));
    // dry asphalt: only the soft lobes; wet: lobes plus the mirror below
    c += spec * fres * spec_k * select(0.5, 1.2, sigma > 0.1);
    // the mirrored alley. The film on the asphalt scatters the mirror ray
    // along the view direction; a static per-sample jitter spreads it, so
    // supersampling turns it into the smeared streaks of a wet street.
    if (spec_k > 0.05 && sigma < 0.1) {
        let j1 = ctx.jitter - 0.5;
        let j2 = fract(ctx.jitter * 7.31 + 0.37) - 0.5;
        let rj = normalize(r + vec3f(0.0, j1 * 3.0 * cs * sigma, 0.0) + vec3f(-r.z, 0.0, r.x) * j2 * 2.0 * sn * sigma);
        let refl = trace_reflection(p, rj, sigma * 0.5, l, ctx);
        c += refl * fres * spec_k * mix(0.55, 1.0, pud);
    }
    if (l.rain > 0.0) {
        c += rain_splashes(p.xz * vec2f(1.0, 0.6), ctx.t, 2.2) * (e * 0.25 + spec * 0.05);
    }
    return c;
}

// overhead cables: bundles slung across the alley, and runs along it
fn cables(ro: vec3f, rd: vec3f, tmax: f32, ctx: Ctx) -> f32 {
    var cov = 0.0;
    for (var k = 0; k < 9; k++) {
        let h = hash_cell2(vec2i(k, 3), 0xcab1u);
        let zc = -2.5 - f32(k) * 7.3 - h.x * 3.0;
        let tc = (zc - ro.z) / min(rd.z, -1e-4);
        if (tc <= 0.0 || tc > tmax) { continue; }
        let q = ro + rd * tc;
        let span = W0 + 0.2;
        if (abs(q.x) > span) { continue; }
        let aa = ctx.px * tc;
        for (var m = 0; m < 3; m++) {
            let y0 = 5.2 + h.y * 2.0 + f32(m) * 0.14 + h.z * 0.3 * f32(m);
            let sag = 0.3 + 0.4 * fract(h.w * f32(m + 3) * 7.1);
            let yc = y0 - sag * (1.0 - sq(q.x / span)) + 0.2 * f32(m) * q.x / span;
            let d = abs(q.y - yc);
            cov = max(cov, saturate((0.016 + aa * 0.5 - d) / max(aa, 1e-4)) * min(1.0, 0.03 / max(aa, 0.03)));
        }
    }
    // cables along the alley, stapled to the walls
    for (var m = 0; m < 4; m++) {
        let sx = select(-1.0, 1.0, (m & 1) == 1);
        let lc = vec2f(sx * (W0 - 0.12 - 0.1 * f32(m / 2)), 5.7 + 0.35 * f32(m));
        let o = ro.xy - lc;
        let dxy = rd.xy;
        let tt = -dot(o, dxy) / max(dot(dxy, dxy), 1e-6);
        if (tt <= 0.0 || tt > tmax) { continue; }
        let d = length(o + dxy * tt);
        let aa = ctx.px * tt;
        cov = max(cov, saturate((0.014 + aa * 0.5 - d) / max(aa, 1e-4)) * min(1.0, 0.03 / max(aa, 0.03)));
    }
    return cov;
}

// yakitori smoke curling up from a grill under the lanterns
fn smoke(ro: vec3f, rd: vec3f, tmax: f32, ctx: Ctx) -> vec2f {
    let zs = -6.4;
    let ts = (zs - ro.z) / min(rd.z, -1e-4);
    if (ts <= 0.0 || ts > tmax) { return vec2f(0.0); }
    let q = ro + rd * ts;
    let u = q.x + 1.0;
    let v = q.y - 1.1;
    if (v < 0.0 || v > 5.0) { return vec2f(0.0); }
    let spread = 0.15 + v * 0.2;
    let bend = 0.03 * v * v;
    let shape = exp(-sq((u - bend) / spread)) * smoothstep(0.0, 0.3, v) * smoothstep(5.0, 1.2, v);
    let n = noise_fbm2(vec2f(u * 2.2, v * 1.3 - ctx.t * 0.45), 4);
    let n2 = noise_fbm2(vec2f(u * 4.0 + n, v * 2.0 - ctx.t * 0.7), 3);
    return vec2f(saturate(shape * (n * 1.8 + n2 * 0.7 - 0.7)), v);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(0.22, 1.55, 1.2);
    let cam = cam_look_at(ro, vec3f(-0.3, 3.0, -30.0), 0.0, 56.0);
    let rd = cam_ray(cam, p);

    // analytic bounds: floor, sky lid, the far facade, the empty core
    var t_floor = 1e5;
    if (rd.y < -1e-5) { t_floor = -ro.y / rd.y; }
    var t_sky = 1e5;
    if (rd.y > 1e-5) { t_sky = (Y_TOP - ro.y) / rd.y; }
    let t_end = (Z_END - XST - ro.z) / min(rd.z, -1e-4);
    let t_hi = min(min(t_floor, t_sky), t_end);
    var t0 = 0.05;
    if (abs(rd.x) > 1e-5) {
        t0 = max(t0, (sign(rd.x) * CORE - ro.x) / rd.x);
    } else {
        t0 = t_hi;
    }
    var hit = vec2f(-1.0);
    if (t0 < t_hi) {
        hit = rm_march(ro, rd, t0, t_hi, steps(110.0, ctx), ctx);
    }
    var col: vec3f;
    var th: f32;
    if (hit.x > 0.0) {
        th = hit.x;
        let hp = ro + rd * th;
        let n = rm_normal(hp, th, ctx);
        col = shade(hp, n, hit.y, rd, th, ctx.px * th, l, ctx);
    } else if (t_hi == t_floor) {
        th = t_floor;
        col = shade_floor(ro + rd * th, rd, th, l, ctx);
    } else if (t_hi == t_end && (ro + rd * t_end).y < 24.0) {
        th = t_end;
        col = far_street(ro + rd * th, ctx.px * th, l, ctx);
    } else {
        th = 400.0;
        col = gap_sky(rd, l, ctx);
    }
    // cables in front of whatever we hit
    let cab = cables(ro, rd, th, ctx);
    col = mix(col, vec3f(0.002), cab * 0.92);
    // haze, then the glow of every lamp in the damp air
    let tf = min(th, 120.0);
    col = mix(l.fogc, col, exp(-tf * l.fog));
    let glow = air_glow(ro, rd, tf, l);
    col += glow;
    // smoke from the grill, lit by the lanterns
    let sm = smoke(ro, rd, th, ctx);
    let smc = vec3f(0.07, 0.035, 0.022) * (1.0 - sm.y * 0.15) + l.amb * 0.8 + glow * 1.5;
    col = mix(col, smc, sm.x * 0.9);
    // weather in front of everything, lit by what shines behind it
    if (l.rain > 0.0) {
        let rs = rain_streaks(p, ctx, l.rain, 2.4, 0.07, 3);
        col += glow * 3.5 * rs * smoothstep(0.004, 0.03, col_luma(glow));
    }
    if (l.snow > 0.0) {
        let sn = snow_flakes(p * 2.6 + vec2f(0.0, 0.3), ctx, 0.3, 0.015, 3);
        col += (glow * 2.5 + vec3f(0.012, 0.013, 0.016)) * sn;
    }
    // natural lens falloff toward the corners
    let vig = mix(1.0, pow(saturate(dot(rd, cam.fw)), 2.5), 0.6);
    return col * vig * exp2(l.exposure);
}
