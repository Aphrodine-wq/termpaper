//! name: shibuya
//! title: Shibuya Scramble
//! category: city
//! tags: tokyo, crossing, crowd, neon, screens, rain, night
//! desc: the Shibuya scramble from above: crowds surge across every stripe when the lights change
//! themes: rain, night, day
//! uses: camera, sdf, rain, wet, noise
//! cost: light
//! tonemap: aces
//! fallback: city
//! credits: original

// World units are metres, y up. We look down from a window high in the
// south-east corner building, across the crossing toward the north-west. The
// ground is the plane y = 0. Near the crossing every building is a box from a
// hand-placed list, each with a rooftop element (a stair house or a sign
// facing the crossing); beyond them a grid of lots is walked with a DDA so
// the city runs on to the horizon. Facades are procedural by kind: office
// ribbons lit tenant by tenant, "zakkyo" bar buildings banded with signs and
// hung with vertical signboards, flats, dark glass towers edged with LEDs,
// department stores, and the corner building with the giant screen. The
// signal cycle is 40 s: 20 s of traffic (the east-west road, then the
// north-south road), then 20 s when every crossing — four sides and both
// diagonals — turns green at once and the waiting crowds pour out. Every
// pedestrian and car is a closed-form trajectory, looked up per pixel
// through its lane.

const CYCLE: f32 = 40.0;
const WALK0: f32 = 20.0;      // walk phase: 20 .. 40 s
const A_G0: f32 = 0.5;        // east-west road green
const A_G1: f32 = 9.0;
const B_G0: f32 = 10.5;       // north-south road green
const B_G1: f32 = 19.0;
const ROAD_A: f32 = 11.0;     // half width of the east-west road (|z| < 11)
const ROAD_B: f32 = 10.0;     // half width of the north-south road (|x| < 10)
const BLK: f32 = 22.0;        // building line
// the hand-built district; the lot grid owns everything outside it
const NX0: f32 = -112.0;
const NX1: f32 = 96.0;
const NZ0: f32 = -128.0;
const NZ1: f32 = 96.0;
const LOT: f32 = 16.0;

struct Look {
    mode: u32,        // 0 rain, 1 night, 2 day
    amb: vec3f,       // skylight on the ground and roofs
    sky: vec3f,       // sky and haze colour
    lamp: vec3f,      // street light colour * intensity
    day: vec3f,       // daylight on facades (0 at night)
    wet: f32,
    screen: f32,      // screen brightness
    win: f32,         // lit-window brightness
    sign: f32,        // sign brightness
    haze: f32,        // extinction per metre
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            // clear night after a shower: the asphalt still damp and glossy
            return Look(1u, vec3f(0.006, 0.006, 0.009), vec3f(0.010, 0.009, 0.015), col_kelvin(5400.0),
                        vec3f(0.0), 0.45, 0.85, 1.0, 1.0, 0.0012, 0.15);
        }
        case 2u: {
            // overcast afternoon
            return Look(2u, col_hex(0xd8dce4u) * 0.30, col_hex(0xc4ccd8u) * 0.62, vec3f(0.0),
                        col_hex(0xe4e6ecu) * 0.46, 0.0, 0.75, 0.10, 0.22, 0.0009, -0.3);
        }
        default: {
            // rain: soaked asphalt, a lit mist over everything
            return Look(0u, vec3f(0.009, 0.009, 0.012), vec3f(0.030, 0.027, 0.036), col_kelvin(5200.0),
                        vec3f(0.0), 1.0, 0.8, 0.95, 0.95, 0.0042, 0.15);
        }
    }
}

// ------------------------------------------------------------------ buildings
// footprint min/max corner (x, z), base and roof height, facade kind:
// 0 office, 1 zakkyo, 2 flats, 3 glass tower, 4 the screen corner,
// 5 rooftop billboard, 6 department store, 7 stair house, 8 roof sign
struct Bld { mn: vec2f, mx: vec2f, y0: f32, h: f32, kind: u32 }

const NB: i32 = 25;
fn bld(k: i32) -> Bld {
    switch (k) {
        // north-west: the screen corner and the row along both roads
        case 0: { return Bld(vec2f(-46.0, -46.0), vec2f(-22.0, -22.0), 0.0, 44.0, 4u); }
        case 1: { return Bld(vec2f(-60.0, -40.0), vec2f(-46.0, -22.0), 0.0, 30.0, 1u); }
        case 2: { return Bld(vec2f(-73.0, -44.0), vec2f(-60.0, -22.0), 0.0, 38.0, 0u); }
        case 3: { return Bld(vec2f(-88.0, -36.0), vec2f(-73.0, -22.0), 0.0, 24.0, 1u); }
        case 4: { return Bld(vec2f(-110.0, -48.0), vec2f(-88.0, -22.0), 0.0, 46.0, 6u); }
        case 5: { return Bld(vec2f(-40.0, -62.0), vec2f(-22.0, -48.0), 0.0, 28.0, 1u); }
        case 6: { return Bld(vec2f(-44.0, -80.0), vec2f(-22.0, -62.0), 0.0, 50.0, 3u); }
        case 7: { return Bld(vec2f(-38.0, -98.0), vec2f(-22.0, -80.0), 0.0, 32.0, 1u); }
        case 8: { return Bld(vec2f(-46.0, -126.0), vec2f(-22.0, -98.0), 0.0, 58.0, 0u); }
        case 9: { return Bld(vec2f(-108.0, -120.0), vec2f(-48.0, -50.0), 0.0, 33.0, 2u); }
        // north-east
        case 10: { return Bld(vec2f(22.0, -44.0), vec2f(44.0, -22.0), 0.0, 34.0, 6u); }
        case 11: { return Bld(vec2f(44.0, -38.0), vec2f(58.0, -22.0), 0.0, 26.0, 1u); }
        case 12: { return Bld(vec2f(58.0, -46.0), vec2f(76.0, -22.0), 0.0, 46.0, 0u); }
        case 13: { return Bld(vec2f(76.0, -36.0), vec2f(94.0, -22.0), 0.0, 30.0, 1u); }
        case 14: { return Bld(vec2f(22.0, -66.0), vec2f(40.0, -46.0), 0.0, 40.0, 1u); }
        case 15: { return Bld(vec2f(22.0, -86.0), vec2f(38.0, -68.0), 0.0, 28.0, 2u); }
        case 16: { return Bld(vec2f(22.0, -124.0), vec2f(46.0, -88.0), 0.0, 64.0, 3u); }
        case 17: { return Bld(vec2f(42.0, -120.0), vec2f(94.0, -50.0), 0.0, 36.0, 2u); }
        // south-west, in the foreground
        case 18: { return Bld(vec2f(-46.0, 22.0), vec2f(-22.0, 44.0), 0.0, 26.0, 0u); }
        case 19: { return Bld(vec2f(-44.0, 44.0), vec2f(-22.0, 62.0), 0.0, 34.0, 1u); }
        case 20: { return Bld(vec2f(-50.0, 62.0), vec2f(-22.0, 92.0), 0.0, 40.0, 6u); }
        case 21: { return Bld(vec2f(-110.0, 24.0), vec2f(-52.0, 94.0), 0.0, 30.0, 2u); }
        // rooftop billboards
        case 22: { return Bld(vec2f(25.0, -33.0), vec2f(41.0, -32.4), 34.0, 43.0, 5u); }
        case 23: { return Bld(vec2f(-72.0, -31.0), vec2f(-61.0, -30.4), 38.0, 45.0, 5u); }
        // the tall tower behind the north-east blocks
        default: { return Bld(vec2f(28.0, -196.0), vec2f(66.0, -156.0), 0.0, 200.0, 3u); }
    }
}

// the rooftop element of building k: a stair and lift house, or a sign
// turned toward the crossing; empty when h <= y0
fn roof_el(k: i32, b: Bld) -> Bld {
    let h = hash_cell2(vec2i(k, 11), 0x700fu);
    let w = b.mx - b.mn;
    var o = Bld(b.mn, b.mx, b.h, b.h, 7u);
    if (b.kind == 5u || min(w.x, w.y) < 8.0) { return o; }
    if (h.x < 0.5 && b.kind != 3u && !(b.mn.x < -20.0 && b.mn.y > 20.0)) {
        let sh = 3.5 + 3.5 * h.y;
        if (b.mn.x < -20.0 && b.mn.y > 20.0) {
            // south-west blocks turn their signs east
            let zc = mix(b.mn.y, b.mx.y, 0.3 + 0.4 * h.z);
            let hw = w.y * (0.22 + 0.14 * h.w);
            o.mn = vec2f(b.mx.x - 2.2, zc - hw);
            o.mx = vec2f(b.mx.x - 1.6, zc + hw);
        } else {
            let xc = mix(b.mn.x, b.mx.x, 0.3 + 0.4 * h.z);
            let hw = w.x * (0.22 + 0.14 * h.w);
            o.mn = vec2f(xc - hw, b.mx.y - 2.2);
            o.mx = vec2f(xc + hw, b.mx.y - 1.6);
        }
        o.h = b.h + sh;
        o.kind = 8u;
    } else {
        let sz = w * vec2f(0.2 + 0.18 * h.y, 0.2 + 0.18 * h.z);
        let c0 = b.mn + vec2f(1.5) + (w - sz - vec2f(3.0)) * vec2f(h.z, h.w);
        o.mn = c0;
        o.mx = c0 + sz;
        o.h = b.h + 2.8 + 2.2 * h.y;
    }
    return o;
}

// a lot of the far city at grid cell c; empty when h <= 0
fn far_lot(c: vec2i) -> Bld {
    let mn = vec2f(c) * LOT;
    let mx = mn + vec2f(LOT);
    let ctr = mn + vec2f(LOT * 0.5);
    var o = Bld(mn, mx, 0.0, 0.0, 0u);
    if (ctr.x > NX0 && ctr.x < NX1 && ctr.y > NZ0 && ctr.y < NZ1) { return o; }
    // the two avenues run on
    if (mx.x > -12.0 && mn.x < 12.0) { return o; }
    if (mx.y > -13.0 && mn.y < 13.0) { return o; }
    // the tower's plaza
    if (mx.x > 20.0 && mn.x < 74.0 && mx.y > -204.0 && mn.y < -148.0) { return o; }
    // side streets: whole rows and columns of lots left open
    let sx = hash_f(u32(c.x + 4096) * 0x9e37u);
    let sz = hash_f(u32(c.y + 4096) * 0x7f4bu + 17u);
    if (sx < 0.16 || sz < 0.14) { return o; }
    let h = hash_cell2(c, 0xf00du);
    let g = hash_cell2(c, 0x1ea5u);
    o.mn = mn + vec2f(0.6 + 2.4 * h.x, 0.6 + 2.4 * h.y);
    o.mx = mx - vec2f(0.6 + 2.4 * h.z, 0.6 + 2.4 * g.x);
    var ht = 14.0 + 36.0 * pow(h.w, 1.5);
    if (g.y < 0.07) { ht = 62.0 + 60.0 * g.z; }
    o.h = ht;
    o.kind = u32(g.w * 3.99);
    return o;
}

// ray/box with precomputed 1/rd: (t, face) with face 0 -x, 1 +x, 2 -z,
// 3 +z, 4 roof; t < 0 on a miss or when starting inside
fn box_t(ro: vec3f, inv: vec3f, rd: vec3f, mn: vec2f, y0: f32, mx: vec2f, y1: f32) -> vec2f {
    let t0 = (vec3f(mn.x, y0, mn.y) - ro) * inv;
    let t1 = (vec3f(mx.x, y1, mx.y) - ro) * inv;
    let tn = min(t0, t1);
    let tf = max(t0, t1);
    let tnear = max(max(tn.x, tn.y), tn.z);
    let tfar = min(min(tf.x, tf.y), tf.z);
    if (tnear > tfar || tnear < 0.0) { return vec2f(-1.0, 0.0); }
    var face = 4.0;
    if (tnear == tn.x) { face = select(1.0, 0.0, rd.x > 0.0); }
    else if (tnear == tn.z) { face = select(3.0, 2.0, rd.z > 0.0); }
    return vec2f(tnear, face);
}

fn safe_inv(rd: vec3f) -> vec3f { return 1.0 / select(rd, vec3f(1e-6), abs(rd) < vec3f(1e-6)); }

// the blocks in groups, for culling: (first, end) of the index range and bounds
fn group_range(gi: i32) -> vec2i {
    switch (gi) {
        case 0: { return vec2i(0, 10); }
        case 1: { return vec2i(10, 18); }
        case 2: { return vec2i(18, 22); }
        default: { return vec2i(22, 25); }
    }
}
fn group_box(gi: i32) -> Bld {
    switch (gi) {
        case 0: { return Bld(vec2f(-110.0, -126.0), vec2f(-22.0, -22.0), 0.0, 66.0, 0u); }
        case 1: { return Bld(vec2f(22.0, -124.0), vec2f(94.0, -22.0), 0.0, 72.0, 0u); }
        case 2: { return Bld(vec2f(-110.0, 22.0), vec2f(-22.0, 94.0), 0.0, 48.0, 0u); }
        default: { return Bld(vec2f(-72.0, -196.0), vec2f(66.0, -30.0), 0.0, 208.0, 0u); }
    }
}
// entry and exit of a box along the ray (entry clamped at 0 when inside)
fn box_span(ro: vec3f, inv: vec3f, b: Bld) -> vec2f {
    let t0 = (vec3f(b.mn.x, b.y0, b.mn.y) - ro) * inv;
    let t1 = (vec3f(b.mx.x, b.h, b.mx.y) - ro) * inv;
    let tn = min(t0, t1);
    let tf = max(t0, t1);
    return vec2f(max(max(max(tn.x, tn.y), tn.z), 0.0), min(min(tf.x, tf.y), tf.z));
}

// nearest hand-built box: (t, face, k); k >= 100 is the rooftop element of k - 100
fn scene_hit(ro: vec3f, rd: vec3f) -> vec3f {
    var best = vec3f(1e9, 0.0, -1.0);
    let inv = safe_inv(rd);
    for (var gi = 0; gi < 4; gi++) {
    let gs = box_span(ro, inv, group_box(gi));
    if (gs.x > gs.y || gs.x >= best.x) { continue; }
    let gr = group_range(gi);
    for (var k = gr.x; k < gr.y; k++) {
        let b = bld(k);
        let e = box_t(ro, inv, rd, b.mn, b.y0, b.mx, b.h + 7.5);
        if (e.x < 0.0 || e.x >= best.x) { continue; }
        let h0 = box_t(ro, inv, rd, b.mn, b.y0, b.mx, b.h);
        if (h0.x > 0.0 && h0.x < best.x) { best = vec3f(h0.x, h0.y, f32(k)); }
        let r = roof_el(k, b);
        if (r.h > r.y0) {
            let h1 = box_t(ro, inv, rd, r.mn, r.y0, r.mx, r.h);
            if (h1.x > 0.0 && h1.x < best.x) { best = vec3f(h1.x, h1.y, f32(k + 100)); }
        }
    }
    }
    return best;
}

// the far city along a ray, up to tmax: (t, face, cell x, cell z); t < 0 on a miss
fn far_hit(ro: vec3f, rd: vec3f, tmax: f32) -> vec4f {
    let miss = vec4f(-1.0, 0.0, 0.0, 0.0);
    let inv = safe_inv(rd);
    let d2 = rd.xz;
    let i2 = inv.xz;
    let ta = (vec2f(NX0, NZ0) - ro.xz) * i2;
    let tb = (vec2f(NX1, NZ1) - ro.xz) * i2;
    let tex = min(max(ta.x, tb.x), max(ta.y, tb.y));
    var t = max(tex, 0.0) + 0.01;
    if (t > tmax) { return miss; }
    let p = ro.xz + d2 * t;
    var c = vec2i(floor(p / LOT));
    let stp = vec2i(select(vec2i(-1), vec2i(1), d2 > vec2f(0.0)));
    let tdel = abs(vec2f(LOT) * i2);
    var tn = ((vec2f(c) + select(vec2f(0.0), vec2f(1.0), d2 > vec2f(0.0))) * LOT - ro.xz) * i2;
    for (var i = 0; i < 40; i++) {
        let b = far_lot(c);
        if (b.h > 0.0) {
            let hh = box_t(ro, inv, rd, b.mn, 0.0, b.mx, b.h);
            if (hh.x > 0.0 && hh.x < tmax) { return vec4f(hh.x, hh.y, f32(c.x), f32(c.y)); }
        }
        if (tn.x < tn.y) { t = tn.x; tn.x += tdel.x; c.x += stp.x; }
        else { t = tn.y; tn.y += tdel.y; c.y += stp.y; }
        if (t > tmax || t > 900.0) { break; }
        if (rd.y > 0.0 && ro.y + rd.y * t > 125.0) { break; }
    }
    return miss;
}

// ------------------------------------------------------------------ screens
// screen s: (building, face, centre along the face, centre height), size (w, h)
const NS: i32 = 8;
fn screen_def(s: i32) -> vec4f {
    switch (s) {
        case 0: { return vec4f(0.0, 3.0, -34.5, 26.5); }   // the corner's giant screen
        case 1: { return vec4f(0.0, 1.0, -37.0, 31.0); }   // the corner, east face
        case 2: { return vec4f(10.0, 3.0, 33.0, 20.0); }   // north-east corner
        case 3: { return vec4f(18.0, 1.0, 33.0, 15.0); }   // south-west corner: a tall banner
        case 4: { return vec4f(12.0, 3.0, 67.0, 35.0); }   // north-east, the office block
        case 5: { return vec4f(22.0, 3.0, 33.0, 38.5); }   // rooftop billboard
        case 6: { return vec4f(23.0, 3.0, -66.5, 41.5); }  // rooftop billboard
        default: { return vec4f(6.0, 1.0, -71.0, 40.0); }  // the glass tower, east face
    }
}
fn screen_size(s: i32) -> vec2f {
    switch (s) {
        case 0: { return vec2f(20.0, 15.0); }
        case 1: { return vec2f(12.0, 9.0); }
        case 2: { return vec2f(14.0, 9.0); }
        case 3: { return vec2f(6.0, 14.0); }
        case 4: { return vec2f(13.0, 9.0); }
        case 5: { return vec2f(15.4, 8.0); }
        case 6: { return vec2f(10.4, 6.4); }
        default: { return vec2f(13.0, 9.0); }
    }
}

// saturated broadcast palette
fn pal(i: u32) -> vec3f {
    switch (i % 12u) {
        case 0u: { return col_hex(0xe8106au); }   // magenta
        case 1u: { return col_hex(0x0a8cffu); }   // electric blue
        case 2u: { return col_hex(0xff3a10u); }   // red-orange
        case 3u: { return col_hex(0x00c8d8u); }   // cyan
        case 4u: { return col_hex(0xffb400u); }   // amber
        case 5u: { return col_hex(0x5a18e0u); }   // violet
        case 6u: { return col_hex(0x18d060u); }   // green
        case 7u: { return col_hex(0xff2440u); }   // red
        case 8u: { return col_hex(0x1840c8u); }   // deep blue
        case 9u: { return col_hex(0xff6ab0u); }   // pink
        case 10u: { return col_hex(0xf0e000u); }  // yellow
        default: { return col_hex(0x0a6a5au); }   // teal
    }
}
fn slide_cols(slot: f32, id: i32) -> vec4f {
    let h = hash_cell2(vec2i(i32(slot), id), 0xad5u);
    return h;
}

// soft rectangle mask, edges softened by w
fn rect(q: vec2f, c: vec2f, hs: vec2f, w: f32) -> f32 {
    let d = sdf2_box(q - c, hs);
    return saturate(0.5 - d / max(w, 1e-3));
}

// one advertisement: a programme picked per slot, two saturated colours.
// lt = seconds into the slot, aa = uv units per pixel
fn ad(uv: vec2f, slot: f32, id: i32, lt: f32, aa: f32) -> vec3f {
    let h = slide_cols(slot, id);
    let g = hash_cell2(vec2i(i32(slot), id), 0x9e1u);
    let a = pal(u32(h.y * 12.0));
    let b = pal(u32(h.z * 12.0) + 5u);
    let prog = (u32(i32(slot) + 4096) * 3u + u32(id) * 2u) % 5u;
    let w = max(aa, 0.02);
    var c = vec3f(0.0);
    if (prog == 0u) {
        // fashion: a close-up portrait in soft side light, a column of copy
        let sx = select(0.33, 0.67, g.x < 0.5);
        let side = select(1.0, -1.0, g.x < 0.5);
        c = mix(b * 0.06, a * 0.75, sstep(-0.2, 1.1, uv.y + 0.25 * (uv.x - 0.5) * side));
        let z = 1.0 + 0.03 * lt;
        let q = (uv - vec2f(sx, 0.47)) / z;
        let hair = sstep(1.0, 0.8, length((q - vec2f(0.0, 0.07)) / vec2f(0.25, 0.42)));
        let face = sstep(1.0, 0.7, length((q - vec2f(0.015 * side, 0.0)) / vec2f(0.17, 0.3)));
        let neck = sstep(0.09, 0.05, abs(q.x)) * step(q.y, -0.2);
        let shoulder = sstep(0.0, -0.06, q.y + 0.33 - 0.9 * q.x * q.x);
        let lit = 0.3 + 0.7 * saturate(0.5 + q.x * side * 3.5);
        let skin = mix(col_hex(0xb86a48u), col_hex(0xffcfb0u), lit) * (0.8 + 0.2 * sstep(-0.3, 0.2, q.y));
        let eyes = 1.0 - 0.35 * exp(-sq((q.y - 0.06) / 0.035)) * step(abs(q.x), 0.13);
        c = mix(c, mix(vec3f(0.012, 0.008, 0.006), a * 0.12, 0.4), hair);
        c = mix(c, skin * 0.85, max(neck * 0.9, 0.0));
        c = mix(c, mix(vec3f(0.03), b * 0.35, 0.6) * (0.5 + 0.5 * lit), shoulder);
        c = mix(c, skin * eyes, face);
        let tx = 1.0 - sx;
        var copy = 0.0;
        for (var i = 0; i < 3; i++) {
            let yy = 0.64 - f32(i) * 0.1;
            copy = max(copy, rect(uv, vec2f(tx, yy), vec2f(0.16 - 0.04 * f32(i), 0.022), w));
        }
        c = mix(c, vec3f(1.0), copy * 0.9);
        c = mix(c, b, rect(uv, vec2f(tx, 0.2), vec2f(0.12, 0.04), w));
    } else if (prog == 1u) {
        // live concert: coloured beams fanning from the stage, haze, a crowd
        c = mix(b * 0.03, a * 0.1, uv.y);
        for (var i = 0; i < 4; i++) {
            let fi = f32(i);
            let ox = 0.2 + 0.2 * fi;
            let ang = 0.5 * sin(lt * (0.35 + 0.1 * fi) + fi * 1.9 + g.y * 6.0);
            let d = uv - vec2f(ox, 0.12);
            let dir = vec2f(sin(ang), cos(ang));
            let along = dot(d, dir);
            let across = abs(d.x * dir.y - d.y * dir.x);
            let beam = exp(-sq(across / (0.012 + 0.07 * max(along, 0.0)))) * step(0.0, along) * exp(-along * 1.2);
            c += select(a, b, (i & 1) == 1) * beam * 0.8;
        }
        c += mix(a, vec3f(1.0), 0.4) * exp(-sq((uv.y - 0.14) / 0.06)) * 0.6;
        let heads = 0.1 + 0.03 * sin(uv.x * 60.0 + g.z * 9.0) + 0.02 * sin(uv.x * 23.0);
        c = mix(c, vec3f(0.005), saturate(0.5 - (uv.y - heads) / w));
    } else if (prog == 2u) {
        // drink: split field, a can with its label and a splash of light
        let split = step(uv.x + (uv.y - 0.5) * 0.4, 0.5);
        c = mix(a * 0.85, b * 0.6, split) * (0.5 + 0.5 * uv.y);
        let sx = select(0.36, 0.64, g.x < 0.5);
        c += vec3f(1.0) * exp(-sq((uv.x - sx) / 0.2) - sq((uv.y - 0.55) / 0.3)) * 0.25;
        let bob = 0.015 * sin(lt * 0.9);
        let can = rect(uv, vec2f(sx, 0.5 + bob), vec2f(0.13, 0.3), w * 1.5);
        let cx = (uv.x - (sx - 0.13)) / 0.26;
        let shade = 0.35 + 0.65 * exp(-sq((cx - 0.35) * 3.5));
        let label = b * 0.9;
        let metal = mix(vec3f(0.8), label, step(abs(uv.y - 0.47 - bob), 0.16)) * shade;
        c = mix(c, metal, can);
        let tx = 1.0 - sx;
        c = mix(c, vec3f(1.0), rect(uv, vec2f(tx, 0.62), vec2f(0.16, 0.035), w) * 0.95);
        c = mix(c, vec3f(0.9), rect(uv, vec2f(tx, 0.5), vec2f(0.12, 0.022), w) * 0.85);
    } else if (prog == 3u) {
        // news: headline bars, a picture, a ticker along the bottom
        c = mix(col_hex(0x061a50u), col_hex(0x0a3aa0u), uv.y) * 0.8;
        c = mix(c, vec3f(1.0), rect(uv, vec2f(0.3, 0.8), vec2f(0.24, 0.05), w) * 0.95);
        c = mix(c, vec3f(0.85), rect(uv, vec2f(0.24, 0.66), vec2f(0.18, 0.035), w) * 0.9);
        let pic = rect(uv, vec2f(0.76, 0.55), vec2f(0.2, 0.2), w);
        let ph = mix(a * 0.7, b * 0.5, sstep(0.35, 0.75, uv.y)) + vec3f(0.3) * exp(-sq((uv.x - 0.72) * 12.0) - sq((uv.y - 0.5) * 10.0));
        c = mix(c, ph, pic);
        let tk = rect(uv, vec2f(0.5, 0.12), vec2f(0.5, 0.07), w);
        let tick = step(0.45, fract(uv.x * 7.0 + lt * 0.35)) * step(abs(uv.y - 0.12), 0.03);
        c = mix(c, mix(vec3f(0.95), vec3f(0.1), tick * 0.8), tk);
        c = mix(c, col_hex(0xff1010u), rect(uv, vec2f(0.07, 0.12), vec2f(0.07, 0.07), w));
    } else {
        // brand: a radial burst of colour and a white mark
        let q = uv - vec2f(0.5, 0.55);
        let r = length(q * vec2f(1.6, 1.0));
        c = mix(a, b * 0.15, sstep(0.0, 0.8, r));
        let ang = atan2(q.y, q.x);
        c *= 0.8 + 0.25 * sstep(0.2, 0.9, sin(ang * 6.0 + lt * 0.15));
        let ring = abs(r - 0.18) - 0.035;
        c = mix(c, vec3f(1.0), saturate(0.5 - ring / (w * 1.4)) * 0.95);
        c = mix(c, vec3f(1.0), rect(uv, vec2f(0.5, 0.18), vec2f(0.22, 0.035), w) * 0.9);
    }
    return c;
}

// screen content at uv (0..1); every ~10 s the next ad wipes in from the left
fn screen_col(uv: vec2f, id: i32, t: f32, aa: f32) -> vec3f {
    let per = 9.0 + f32(id) * 1.3;
    let x = t / per + f32(id) * 0.37;
    let slot = floor(x);
    let lt = fract(x) * per;
    let wipe = saturate((lt - (per - 0.6)) / 0.6);
    var c: vec3f;
    if (uv.x < wipe) { c = ad(uv, slot + 1.0, id, lt - per, aa); } else { c = ad(uv, slot, id, lt, aa); }
    c += vec3f(1.0) * exp(-sq((uv.x - wipe) / 0.02)) * step(0.001, wipe) * step(wipe, 0.999) * 0.5;
    return c;
}
// the average colour a screen throws on its surroundings
fn screen_avg(id: i32, t: f32) -> vec3f {
    let per = 9.0 + f32(id) * 1.3;
    let slot = floor(t / per + f32(id) * 0.37);
    let h = slide_cols(slot, id);
    return (pal(u32(h.y * 12.0)) * 0.6 + pal(u32(h.z * 12.0) + 5u) * 0.3 + vec3f(0.08)) * 0.45;
}

// ------------------------------------------------------------------ facades
// sign colours: backlit boxes and bands
fn sign_col(h: f32) -> vec3f {
    switch (u32(h * 11.0)) {
        case 0u: { return col_hex(0xff2a1au); }
        case 1u: { return col_hex(0xffcc10u); }
        case 2u: { return col_hex(0xf6f4eau); }
        case 3u: { return col_hex(0x1a64ffu); }
        case 4u: { return col_hex(0x10c860u); }
        case 5u: { return col_hex(0xff7010u); }
        case 6u: { return col_hex(0xff2a8au); }
        case 7u: { return col_hex(0x10c0f0u); }
        case 8u: { return col_hex(0xf6f4eau); }
        case 9u: { return col_hex(0xe01020u); }
        default: { return col_hex(0xffe4a8u); }
    }
}

// lettering on a sign: q in character units (x along the line, y 0..1
// across it), lod in character units; 1 on a stroke
fn glyphs(q: vec2f, seed: u32, lod: f32) -> f32 {
    let avg = 0.3;
    let k = saturate(lod * 2.5 - 0.5);
    if (k >= 1.0) { return avg; }
    let f = vec2f(fract(q.x), q.y);
    var on = 0.0;
    if (f.x > 0.12 && f.x < 0.88 && f.y > 0.14 && f.y < 0.86) {
        let gc = vec2i(floor((f - vec2f(0.12, 0.14)) / vec2f(0.76, 0.72) * 3.0));
        let hh = hash_u3(vec3u(u32(i32(floor(q.x)) + 50000), u32(gc.x + gc.y * 3), seed));
        on = select(0.0, 1.0, hash_unorm(hh) < 0.5);
    }
    return mix(on, avg, k);
}

// box-filtered periodic band: coverage of [lo, hi] (cell units) at x,
// filtered over w cells
fn pband(x: f32, lo: f32, hi: f32, w: f32) -> f32 {
    let duty = hi - lo;
    if (w >= 1.0) { return duty; }
    let f = fract(x);
    let a = f - w * 0.5;
    let b = f + w * 0.5;
    var c = max(0.0, min(b, hi) - max(a, lo));
    c += max(0.0, min(b, hi - 1.0) - max(a, lo - 1.0));
    c += max(0.0, min(b, hi + 1.0) - max(a, lo + 1.0));
    return mix(c / max(w, 1e-4), duty, saturate(w * 2.0 - 1.0));
}

// daylight by orientation: soft overcast from the south-west
fn face_shade(face: u32) -> f32 {
    switch (face) {
        case 0u: { return 0.78; }
        case 1u: { return 0.62; }
        case 2u: { return 0.5; }
        case 3u: { return 0.92; }
        default: { return 1.12; }
    }
}

fn roof_col(hp: vec3f, b: Bld, id: u32, l: Look, lod: f32) -> vec3f {
    let hr = hash_cell2(vec2i(i32(id), 17), 0x2f0fu);
    var alb = mix(col_hex(0x6a6a66u), col_hex(0x8a8478u), hr.x) * 0.42;
    if (hr.y < 0.25) { alb = col_hex(0x4a6a5eu) * 0.4; }    // green waterproofing
    alb *= 0.72 + 0.45 * noise_value2(hp.xz * 0.3 + hr.zw * 40.0);
    let de = min(min(hp.x - b.mn.x, b.mx.x - hp.x), min(hp.z - b.mn.y, b.mx.y - hp.z));
    let lw = max(lod, 0.05);
    // drainage stains and patched membrane
    alb *= 1.0 - 0.3 * sstep(0.55, 0.8, noise_value2(hp.xz * 0.12 + hr.yx * 17.0));
    // pipe runs and cable trays across the roof
    let pipe = pband((hp.x + hr.z * 30.0) / 7.0, 0.0, 0.06, lw / 7.0) * step(0.5, hr.w)
             + pband((hp.z + hr.x * 30.0) / 9.0, 0.0, 0.05, lw / 9.0) * step(hr.w, 0.7);
    alb = mix(alb, vec3f(0.3, 0.3, 0.29), saturate(pipe) * 0.7 * step(1.2, de));
    // air-conditioning units in rows, each with a shadow
    let q = hp.xz / 2.4 + hr.xy * 9.0;
    let cell = vec2i(floor(q));
    let hu = hash_cell2(cell + vec2i(i32(id) * 31, 0), 0xacu);
    let f = fract(q);
    if (hu.x < 0.42 && de > 1.6) {
        let sz = vec2f(0.24 + 0.14 * hu.y, 0.18 + 0.12 * hu.z);
        let du = sdf2_box(f - vec2f(0.5), sz) * 2.4;
        let ds = sdf2_box(f - vec2f(0.5) - vec2f(0.1, 0.13), sz) * 2.4;
        let unit = saturate(0.5 - du / lw);
        let sh = saturate(0.5 - ds / lw) * (1.0 - unit);
        alb = mix(alb, vec3f(0.4, 0.4, 0.38) * (0.8 + 0.4 * hu.w), unit * 0.85);
        alb *= 1.0 - 0.55 * sh;
    }
    // parapet: a pale lip with a dark gutter inside it
    let rim = saturate(1.0 - (de - 0.45) / lw);
    let gut = saturate(1.0 - abs(de - 0.8) / max(lw, 0.3)) * (1.0 - rim);
    alb *= 1.0 - 0.4 * gut;
    alb = mix(alb, vec3f(0.42, 0.41, 0.39), rim);
    var c = alb * (l.amb * 3.0 + l.day * face_shade(4u));
    if (l.mode != 2u) {
        // spill from the signs and screens below, a little warm
        c += alb * vec3f(0.09, 0.065, 0.075) * (0.5 + hr.z);
        // a warm service lamp on some roofs
        let lp = mix(b.mn, b.mx, hr.zw);
        let d = length(hp.xz - lp);
        c += col_kelvin(3000.0) * 0.5 * alb * exp(-d * d / 6.0) * step(0.55, hr.w);
    }
    return c;
}

// video screens on face `face` of box kfix; alb = the wall's albedo, for spill
fn screens_on(c0: vec3f, hp: vec3f, face: u32, kfix: i32, along: f32, alb: vec3f, l: Look, t: f32, lw: f32) -> vec3f {
    var c = c0;
    for (var s = 0; s < NS; s++) {
        let sd = screen_def(s);
        if (i32(sd.x) != kfix || u32(sd.y) != face) { continue; }
        let sz = screen_size(s);
        let q = vec2f(along - sd.z, hp.y - sd.w);
        if (abs(q.x) < sz.x * 0.5 + 0.6 && abs(q.y) < sz.y * 0.5 + 0.6) {
            let inner = rect(q, vec2f(0.0), sz * 0.5, lw);
            let bez = rect(q, vec2f(0.0), sz * 0.5 + 0.5, lw);
            var uv = q / sz + 0.5;
            if (face == 1u || face == 2u) { uv.x = 1.0 - uv.x; }
            let sc = screen_col(uv, s, t, lw / sz.x);
            c = mix(c, vec3f(0.006), bez);
            c = mix(c, sc * l.screen, inner);
        } else if (l.mode != 2u) {
            // the screen's colour spills on the wall around it
            let dd = length(max(abs(q) - sz * 0.5, vec2f(0.0)));
            c += alb * screen_avg(s, t) * l.screen * 0.8 * exp(-dd / 4.0);
        }
    }
    return c;
}

// facade of building b (id for hashing), face 0-3, at world point hp; lod =
// metres per pixel. Returns radiance.
fn facade(hp: vec3f, face: u32, b: Bld, id: u32, kfix: i32, l: Look, t: f32, lod: f32) -> vec3f {
    if (face == 4u) { return roof_col(hp, b, id, l, lod); }
    let xface = face < 2u;
    let along = select(hp.x, hp.z, xface);
    let e0 = select(b.mn.x, b.mn.y, xface);
    let e1 = select(b.mx.x, b.mx.y, xface);
    let wdt = e1 - e0;
    let u = along - e0;
    let y = hp.y - b.y0;
    let bh = b.h - b.y0;
    let hb = hash_cell2(vec2i(i32(id), 3), 0xb1du);
    let hf = hash_cell2(vec2i(i32(id), i32(face)), 0xfacu);
    let night = l.mode != 2u;
    let fs = face_shade(face);
    let lw = max(lod, 0.03);
    let kind = b.kind;

    // wall material
    var alb: vec3f;
    switch (kind) {
        case 0u: {
            alb = mix(col_hex(0x7a7e86u), col_hex(0xb0a898u), hb.x) * 0.5;
            if (hb.y < 0.3) { alb = col_hex(0x3a4a5eu) * 0.45; }
        }
        case 1u: {
            let m = u32(hb.x * 4.0);
            alb = select(select(select(col_hex(0x9a9690u), col_hex(0x7a5a48u), m == 1u), col_hex(0xd8d4ccu), m == 2u), col_hex(0x5a5c62u), m == 3u) * 0.5;
        }
        case 2u: {
            alb = mix(col_hex(0xd8d0c0u), col_hex(0xa89c8cu), hb.x) * 0.5;
            if (hb.y < 0.3) { alb = col_hex(0x8a5e46u) * 0.5; }
        }
        case 3u: { alb = col_hex(0x1c2230u) * 0.4; }
        case 4u: { alb = col_hex(0x20242cu) * 0.4; }
        case 6u: { alb = mix(col_hex(0xd0c8b8u), col_hex(0xe8e4dcu), hb.x) * 0.5; }
        case 7u: { alb = col_hex(0x8a8a86u) * 0.45; }
        default: { alb = col_hex(0x2a2a2eu) * 0.4; }
    }
    // the street's glow climbs the lower floors; screens and signs light the rest faintly
    let glow = select(vec3f(0.0), col_kelvin(3600.0) * (0.06 * exp(-y / 7.0) + 0.012) + vec3f(0.012, 0.011, 0.018), night);
    // weathering: rain streaks down the concrete, the street canyon darker low down
    alb *= 0.82 + 0.3 * noise_value2(vec2f(u * 0.9 + hb.z * 50.0, y * 0.06));
    let canyon = 0.55 + 0.45 * saturate(y / 30.0);
    var c = alb * (l.amb * 2.0 + l.day * fs * canyon + glow);
    // daylight: glass holds the sky, brighter up high
    let glass_day = l.sky * (0.1 + 0.16 * saturate(y / 45.0)) * fs * canyon + l.day * 0.015;
    let glass_night = vec3f(0.004, 0.005, 0.008) + glow * 0.1;
    let glass = select(glass_day, glass_night, night);

    let ground_fl = 4.6;
    if (kind == 7u) {
        // stair house: blank wall, a lit door
        let door = rect(vec2f(u, y), vec2f(wdt * 0.3, 1.1), vec2f(0.5, 1.1), lw);
        c = mix(c, col_kelvin(3400.0) * 0.25 * select(0.1, 1.0, night), door * step(0.5, hf.x));
        return c;
    }
    if (kind == 8u || kind == 5u) {
        // roof signs: the face turned to the crossing is a lit panel, the rest
        // a dark steel frame
        let front = select(face == 3u, face == 1u, xface);
        if (kind == 8u && front && wdt > 2.0) {
            let sc = sign_col(hf.y);
            let inner = rect(vec2f(u, y), vec2f(wdt * 0.5, bh * 0.5), vec2f(wdt * 0.5 - 0.4, bh * 0.5 - 0.4), lw);
            let ch = bh * 0.62;
            let gl = glyphs(vec2f((u - wdt * 0.5) / ch, (y - bh * 0.19) / ch), id, lw / ch);
            let tc = select(vec3f(0.02), sign_col(hf.z), hf.w < 0.5);
            let lit = mix(sc, tc, gl * step(abs(y - bh * 0.5), ch * 0.5) * 0.85);
            let e = lit * 0.42;
            return mix(c * 0.5, select(lit * l.day * fs * 0.9, e, night), inner);
        }
        let strut = pband(u / 1.5, 0.0, 0.2, lw / 1.5) + pband(y / 1.5, 0.0, 0.2, lw / 1.5);
        return screens_on(c * 0.5 * (0.6 + 0.8 * saturate(strut)), hp, face, kfix, along, alb, l, t, lw);
    }

    let fh = select(3.4 + 0.5 * hb.y, 3.9 + 0.3 * hb.y, kind == 0u || kind == 3u);
    let fl = floor((y - ground_fl) / fh);
    let fy = fract((y - ground_fl) / fh);
    let upper = step(ground_fl, y) * step(y, bh - 1.2);
    let fw = lw / fh;

    if (kind == 0u || kind == 3u || kind == 4u) {
        // ribbon glazing lit tenant by tenant: cool fluorescent offices, the
        // odd warm floor, dark floors gone home
        let segw = 5.0 + 7.0 * hf.x;
        let seg = floor(u / segw);
        let hs = hash_cell2(vec2i(i32(fl), i32(seg) + i32(id) * 37 + i32(face) * 11), 0x0ff1u);
        let occ = select(select(0.5 + 0.3 * hb.z, 0.3 + 0.25 * hb.z, kind == 3u), 0.3, kind == 4u);
        let lit = select(0.0, 1.0, hs.x < occ);
        let temp = select(mix(4400.0, 6200.0, hs.y), mix(2900.0, 3700.0, hs.y), hs.z < 0.35 && kind != 4u);
        // bay by bay: blinds half down, desks, a lamp left on
        let hbay = hash_cell2(vec2i(i32(floor(u / 1.8)), i32(fl) + i32(id) * 131), 0xba1u);
        let blind = select(1.0, sstep(0.55 + 0.3 * hbay.y, 0.45 + 0.3 * hbay.y, fy), hbay.x < 0.3);
        let bayk = mix(0.55 + 0.6 * hbay.z, 0.85, saturate(lw / 1.8 - 0.3)) * mix(blind, 0.8, saturate(fw * 2.0 - 0.5));
        let br = (0.08 + 0.17 * hs.w) * (0.75 + 0.35 * sstep(0.35, 0.85, fy)) * bayk;
        var win = col_kelvin(temp) * vec3f(0.96, 1.0, 0.94) * br * l.win;
        // far away the floors average out
        let avg = col_kelvin(4600.0) * occ * 0.12 * l.win;
        win = mix(win, avg, saturate(fw * 1.5 - 0.5));
        let gy = pband((y - ground_fl) / fh, 0.3, select(0.88, 0.94, kind == 3u), fw);
        let mull = 1.0 - 0.45 * pband(u / 1.8, 0.0, 0.1, lw / 1.8);
        var wc = glass + select(vec3f(0.0), win * lit, night) + select(win * lit * 0.4, vec3f(0.0), night);
        wc = mix(wc, glass + avg * select(0.0, 1.0, night), saturate(fw * 1.5 - 0.5));
        c = mix(c, wc * mull, gy * upper);
        if (kind == 3u) {
            // LED strips on the corners and a lit crown
            let edge = min(u, wdt - u);
            let led = saturate(1.0 - (edge - 0.25) / lw) * sstep(bh * 0.45, bh * 0.75, y);
            let lc = select(vec3f(0.6, 0.85, 1.0), vec3f(1.0, 0.95, 0.9), hb.w < 0.5);
            c += lc * led * select(0.05, 0.25, night);
            let crown = step(bh - 3.0, y) * step(y, bh - 0.8);
            c = mix(c, lc * select(0.25, 0.6, night), crown);
        }
    } else if (kind == 1u) {
        // zakkyo: every floor a tenant's lit sign band over punched windows
        let ns = 1.0 + floor(hf.x * 2.0 + 0.5);
        let sw = wdt / ns;
        let si = floor(u / sw);
        let hs = hash_cell2(vec2i(i32(fl), i32(si) + i32(id) * 17 + i32(face) * 5), 0x2a1au);
        let band = pband((y - ground_fl) / fh, 0.6, 0.93, fw);
        let bandm = band * step(0.2, fract(u / sw)) * step(fract(u / sw), 0.97) * step(hs.w, 0.62);
        var sc = sign_col(hs.x);
        let chh = fh * 0.28;
        let gl = glyphs(vec2f(u / chh + hs.y * 7.0, (fy - 0.62) / 0.3), u32(id) * 13u + u32(fl), lw / chh);
        let white = hs.z < 0.45;
        sc = select(mix(sc, vec3f(0.04), gl * 0.75), mix(vec3f(0.95, 0.94, 0.9), sign_col(hs.y) * 0.6, gl), white);
        let se = sc * select(0.12, 0.2 + 0.24 * hs.y, night) * select(l.sign, 1.0, night);
        let sday = sc * l.day * fs * 0.85;
        // punched windows below the band
        let bay = 1.7;
        let wcell = vec2i(i32(floor(u / bay)), i32(fl));
        let hw = hash_cell2(wcell + vec2i(i32(id) * 7, i32(face)), 0x3a7u);
        let wx = pband(u / bay, 0.2, 0.8, lw / bay);
        let wy = pband((y - ground_fl) / fh, 0.14, 0.52, fw);
        let on = select(0.0, 1.0, hw.x < 0.5);
        let wt = col_kelvin(select(3000.0, 5600.0, hw.y < 0.55)) * (0.1 + 0.16 * hw.z) * l.win;
        let wavg = col_kelvin(4200.0) * 0.07 * l.win;
        let wcol = glass + select(vec3f(0.0), mix(wt * on, wavg, saturate(fw * 1.5 - 0.5)), night);
        c = mix(c, wcol, wx * wy * upper);
        c = mix(c, select(sday, se, night), bandm * upper);
    } else if (kind == 2u) {
        // flats and hotels: balconies, warm windows, a few lit
        let bay = 3.2;
        let wcell = vec2i(i32(floor(u / bay)), i32(fl));
        let hw = hash_cell2(wcell + vec2i(i32(id) * 7, i32(face)), 0x51edu);
        let wx = pband(u / bay, 0.18, 0.82, lw / bay);
        let wy = pband((y - ground_fl) / fh, 0.22, 0.8, fw);
        let on = select(0.0, 1.0, hw.x < 0.3);
        let wt = col_kelvin(select(2800.0, 4800.0, hw.y < 0.3)) * (0.08 + 0.12 * hw.z) * l.win;
        let wavg = col_kelvin(3300.0) * 0.035 * l.win;
        let wcol = glass + select(vec3f(0.0), mix(wt * on, wavg, saturate(fw * 1.5 - 0.5)), night);
        c = mix(c, wcol, wx * wy * upper);
        // balcony slabs catch the light
        let slab = pband((y - ground_fl) / fh, 0.0, 0.12, fw);
        c = mix(c, alb * 1.4 * (l.amb * 2.0 + l.day * fs + glow), slab * upper * 0.7);
    } else if (kind == 6u) {
        // department store: stone with vertical fins, uplit; a brand band on top
        let fin = pband(u / 3.0, 0.0, 0.18, lw / 3.0);
        c *= 1.0 + 0.3 * fin;
        if (night) { c += alb * col_kelvin(3200.0) * 0.12 * exp(-(y - ground_fl) / 12.0) * step(ground_fl, y); }
        let slot = pband(u / 3.0, 0.4, 0.62, lw / 3.0) * pband((y - ground_fl) / fh, 0.2, 0.8, fw);
        let hw = hash_cell2(vec2i(i32(floor(u / 3.0)), i32(fl)) + vec2i(i32(id) * 3, 0), 0x6d1u);
        c = mix(c, glass + select(vec3f(0.0), col_kelvin(3600.0) * 0.12 * step(hw.x, 0.45) * l.win, night), slot * upper * 0.85);
        let top = step(bh - 5.5, y) * step(y, bh - 1.5);
        let chh = 3.4;
        let gl = glyphs(vec2f((u - wdt * 0.2) / chh, (y - (bh - 5.2)) / chh), id * 7u, lw / chh);
        let inword = step(wdt * 0.2, u) * step(u, wdt * 0.8);
        let bc = select(sign_col(hb.w), vec3f(0.95), hb.y < 0.5);
        let bandc = mix(select(vec3f(0.03), c, hb.y < 0.5), bc, gl * inword);
        let be = bandc * select(0.15, 0.7, night) * select(l.sign * 2.0, 1.0, night);
        c = mix(c, select(bandc * l.day * fs, be, night), top);
    }

    // stacked vertical signboards down one or two columns
    if (kind == 1u || kind == 6u || (kind == 0u && hf.w < 0.4)) {
        let ncol = select(1.0, 2.0, wdt > 16.0 && hf.z < 0.6);
        for (var ci = 0; ci < 2; ci++) {
            if (f32(ci) >= ncol) { break; }
            let side = select(hf.y < 0.5, !(hf.y < 0.5), ci == 1);
            let cx = select(wdt - 1.4, 1.4, side);
            let sw = select(0.6, 0.85, kind == 6u);
            let du = abs(u - cx);
            let top = bh - select(1.5, 3.0, kind == 6u);
            let colm = saturate(0.5 - (du - sw) / lw) * step(ground_fl + 0.4, y) * step(y, top);
            if (colm > 0.0) {
                let pan = 2.6 + 1.8 * hf.x;
                let pi = floor((y - ground_fl) / pan);
                let hp2 = hash_cell2(vec2i(i32(pi), i32(id) * 5 + ci + i32(face) * 3), 0x516u);
                let gap = pband((y - ground_fl) / pan, 0.14, 1.0, lw / pan) * select(1.0, 0.0, hp2.w > 0.82);
                var sc = sign_col(hp2.x);
                let chh = sw * 1.5;
                let gl = glyphs(vec2f((y - ground_fl) / chh, (u - cx + sw) / (2.0 * sw)), id * 31u + u32(ci), lw / chh);
                sc = select(mix(sc, vec3f(0.03), gl * 0.8), mix(vec3f(0.95), sign_col(hp2.y) * 0.5, gl), hp2.z < 0.35);
                let e = sc * select(0.15, 0.3 + 0.3 * hp2.w, night) * select(l.sign * 2.2, 1.0, night);
                let dc = sc * l.day * fs * 0.8;
                c = mix(c, select(dc, e, night), colm * gap);
            }
        }
    }

    // street level: a canopy, lit shop windows between pillars
    if (y < ground_fl) {
        let bw = 4.5 + 2.5 * hb.z;
        let sf = fract(u / bw + hb.x);
        let sh = hash_cell2(vec2i(i32(floor(u / bw + hb.x)), i32(id) * 5 + i32(face)), 0x5b0u);
        let gx = pband(u / bw + hb.x, 0.1, 0.92, lw / bw);
        let gy = saturate((3.3 - y) / lw) * saturate((y - 0.3) / lw);
        var shop = col_kelvin(select(3000.0 + 800.0 * sh.x, 5200.0 + 1200.0 * sh.x, sh.y < 0.45));
        if (sh.z < 0.12) { shop = sign_col(sh.w) * 0.8 + vec3f(0.1); }
        let shut = select(1.0, 0.08, sh.w < 0.18);
        let se = shop * (0.22 + 0.36 * sh.y) * select(0.06, 1.0, night) * shut * (0.65 + 0.35 * sstep(0.4, 3.0, y));
        c = alb * 0.4 * (l.amb * 2.0 + l.day * fs);
        c = mix(c, glass * 0.7 + se, gx * gy);
        // the fascia over the shops: a lit name board
        let fas = saturate((y - 3.5) / lw) * saturate((4.3 - y) / lw);
        let fc = sign_col(sh.z);
        c = mix(c, fc * select(l.day * fs * 0.8, vec3f(0.22 + 0.2 * sh.x), night), fas * step(0.3, sf) * step(sf, 0.9));
    }
    if (kind == 4u) {
        // the corner's two lowest floors: a café in a glass box over the crossing
        let cafe = step(y, 8.6);
        let bays = pband(u / 2.4, 0.1, 1.0, lw / 2.4);
        let slab = 1.0 - saturate(1.0 - abs(y - 4.3) / max(lw, 0.3)) * 0.85;
        let fig = step(0.5, noise_value2(vec2f(u * 1.3, floor(y / 4.3) * 7.0))) * step(fract(y / 4.3), 0.45);
        let lamp = 0.6 + 0.4 * sstep(0.5, 0.95, fract(y / 4.3));
        let interior = col_kelvin(3300.0) * select(0.04, 0.2, night) * (1.0 - 0.7 * fig) * lamp;
        c = mix(c, (glass + interior) * bays * slab, cafe);
        // a lattice of light over the curtain wall above the screen
        if (night) {
            let lat = max(pband(u / 3.0, 0.0, 0.06, lw / 3.0), pband(y / 3.0, 0.0, 0.06, lw / 3.0));
            c += vec3f(0.5, 0.65, 0.9) * lat * 0.1 * step(9.5, y);
            // the name along the top
            c += vec3f(0.95) * 0.6 * saturate(1.0 - abs(y - (bh - 1.8)) / max(lw, 0.4)) * step(2.0, u) * step(u, wdt - 2.0);
        }
    }

    return screens_on(c, hp, face, kfix, along, alb, l, t, lw);
}

// shade a hit on hand-built box k (or its rooftop element)
fn shade_near(hp: vec3f, face: u32, kk: i32, l: Look, t: f32, lod: f32) -> vec3f {
    if (kk >= 100) {
        let k = kk - 100;
        let r = roof_el(k, bld(k));
        return facade(hp, face, r, u32(kk) * 7u + 3u, -1, l, t, lod);
    }
    return facade(hp, face, bld(kk), u32(kk), kk, l, t, lod);
}

// ------------------------------------------------------------------ crowd
// crosswalk w: start (x, z), end (x, z); width in crosswalk_w
fn crosswalk(w: i32) -> vec4f {
    switch (w) {
        case 0: { return vec4f(-ROAD_B, -14.5, ROAD_B, -14.5); }  // N
        case 1: { return vec4f(ROAD_B, 14.5, -ROAD_B, 14.5); }    // S
        case 2: { return vec4f(-13.5, ROAD_A, -13.5, -ROAD_A); }  // W
        case 3: { return vec4f(13.5, -ROAD_A, 13.5, ROAD_A); }    // E
        case 4: { return vec4f(-ROAD_B, -ROAD_A, ROAD_B, ROAD_A); } // NW-SE
        default: { return vec4f(ROAD_B, -ROAD_A, -ROAD_B, ROAD_A); } // NE-SW
    }
}
fn crosswalk_w(w: i32) -> f32 { return select(7.0, 5.0, w >= 4); }

// person colour: mostly dark coats, some light, a few colours; in the rain,
// umbrellas: Tokyo's clear vinyl ones, black, navy, and a scatter of colour
fn person_col(h: f32, rainy: bool) -> vec3f {
    if (rainy) {
        if (h < 0.34) { return vec3f(0.55, 0.57, 0.6); }
        if (h < 0.66) { return vec3f(0.012); }
        if (h < 0.78) { return col_hex(0x1a2648u); }
        return umbrella_col(h) * 0.6;
    }
    if (h < 0.45) { return vec3f(0.018, 0.018, 0.02); }
    if (h < 0.65) { return vec3f(0.07, 0.065, 0.06); }
    if (h < 0.88) { return vec3f(0.5, 0.48, 0.45); }
    return mix(sign_col(fract(h * 97.0)), vec3f(0.2), 0.45) * 0.3;
}

fn umbrella_col(h: f32) -> vec3f {
    switch (u32(h * 997.0) % 8u) {
        case 0u: { return col_hex(0xe02030u); }
        case 1u: { return col_hex(0xffc020u); }
        case 2u: { return col_hex(0xff5aa0u); }
        case 3u: { return col_hex(0x2a90ffu); }
        case 4u: { return col_hex(0x20b060u); }
        case 5u: { return col_hex(0xff7020u); }
        case 6u: { return col_hex(0x8040e0u); }
        default: { return col_hex(0xf0f0f0u); }
    }
}

// crowd on crosswalk w at point g (heads plane): (coverage, colour key)
fn crowd(g: vec2f, w: i32, cyc: f32, fp: f32, rainy: bool) -> vec2f {
    let cw = crosswalk(w);
    let a = cw.xy;
    let bpt = cw.zw;
    let len = length(bpt - a);
    let d = (bpt - a) / len;
    let n = vec2f(-d.y, d.x);
    let rel = g - a;
    let u = dot(rel, d);
    let v = dot(rel, n);
    let hw = crosswalk_w(w) * 0.5;
    let wq = 6.0;
    if (abs(v) > hw + 0.6 || u < -wq - 1.0 || u > len + wq + 1.0) { return vec2f(0.0); }
    let walking = cyc >= WALK0;
    let tau = cyc - WALK0;
    let tw = CYCLE - WALK0;
    var best = vec2f(0.0);
    let lane_w = 0.7;
    let j0 = round(v / lane_w);
    let r = select(0.3, 0.52, rainy);
    for (var dj = -1; dj <= 1; dj++) {
        let j = j0 + f32(dj);
        if (abs(j * lane_w) > hw) { continue; }
        let hl = hash_cell2(vec2i(i32(j) + 40, w), 0xc0du);
        let dir_pos = hl.x < 0.5;
        let uu = select(len - u, u, dir_pos);
        let sp = select(1.05, 1.5, rainy) + 0.9 * hl.z;
        // everyone clears the crossing before the lights change
        let speed = (len + wq + 3.0) / (tw - 1.5) * (0.92 + 0.12 * hl.y);
        var shift = 0.0;
        if (walking) { shift = speed * tau; }
        // index of the agent nearest this point
        let kf = (shift - uu - 0.6) / sp;
        let kc = i32(floor(kf));
        for (var dk = -1; dk <= 2; dk++) {
            let k = kc + dk;
            if (k < 0 || k > 9) { continue; }
            let ha = hash_cell2(vec2i(k, i32(j) * 7 + w * 131), 0xa9e7u);
            if (ha.w > 0.82) { continue; }
            // queue position, a little jitter; arrivals trickle in until the lights change
            let u0 = -0.6 - f32(k) * sp - ha.x * sp * 0.5;
            let arrive = f32(k) / 10.0 * (WALK0 - 3.0) + ha.y * 2.0;
            var vis = 1.0;
            var uk = u0;
            if (walking) {
                uk = u0 + shift * (0.95 + 0.1 * ha.z);
            } else {
                vis = sstep(arrive, arrive + 1.0, cyc);
            }
            // fade out on reaching the far pavement
            vis *= sstep(len + 4.0, len + 1.5, uk);
            let vj = j * lane_w + (ha.z - 0.5) * 0.35;
            let dd = length(vec2f(uu - uk, v - vj));
            let cov = saturate(0.5 - (dd - r) / max(fp, 0.05)) * vis;
            if (cov > best.x) { best = vec2f(cov, ha.y * 0.7 + ha.x * 0.3); }
        }
    }
    return best;
}

// people milling along the pavements: drifting in two directions
fn pavement_crowd(g: vec2f, t: f32, fp: f32, rainy: bool) -> vec2f {
    // pavements: outside both roads, in front of the building line
    let ax = abs(g.x);
    let az = abs(g.y);
    let on_pav = (ax > ROAD_B + 0.3 || az > ROAD_A + 0.3) && !(ax > BLK - 0.8 && az > BLK - 0.8);
    if (!on_pav) { return vec2f(0.0); }
    let along_x = az > ax;   // pavements beside the east-west road run along x
    var best = vec2f(0.0);
    let r = select(0.3, 0.5, rainy);
    for (var layer = 0; layer < 2; layer++) {
        let dir = select(-1.0, 1.0, layer == 0);
        var q = select(g.yx, g, along_x);
        q.x += dir * 1.2 * t + f32(layer) * 5.0;
        let cs = select(2.4, 3.0, rainy);
        let c = vec2i(floor(q / cs));
        let h = hash_cell2(c + vec2i(layer * 1000, 0), 0x9a5u);
        let busy = 0.03 + 0.3 * sstep(42.0, 16.0, length(g));
        if (h.w < busy) {
            let pc = (vec2f(c) + 0.2 + 0.6 * h.xy) * cs;
            let dd = length(q - pc);
            let cov = saturate(0.5 - (dd - r) / max(fp, 0.05));
            if (cov > best.x) { best = vec2f(cov, h.z); }
        }
    }
    return best;
}

// ------------------------------------------------------------------ traffic
// approach i: stop point (x, z), direction (dx, dz); lanes at lateral offsets
fn approach(i: i32) -> vec4f {
    switch (i) {
        case 0: { return vec4f(19.0, 0.0, -1.0, 0.0); }   // westbound
        case 1: { return vec4f(-19.0, 0.0, 1.0, 0.0); }   // eastbound
        case 2: { return vec4f(0.0, -20.0, 0.0, 1.0); }   // southbound
        default: { return vec4f(0.0, 20.0, 0.0, -1.0); }  // northbound
    }
}

// car k of cycle c on a lane: position along the lane (stop line at 0)
fn car_u(k: i32, c: f32, g0: f32, g1: f32, t: f32) -> f32 {
    let fk = f32(k);
    let q = -3.0 - fk * 6.8;
    let arrive = (c - 1.0) * CYCLE + g1 + 2.0 + fk * 3.2;
    let depart = c * CYCLE + g0 + 0.6 + fk * 1.5;
    if (t < arrive) { return q - 9.0 * (arrive - t); }
    if (t < depart) { return q; }
    let dt = t - depart;
    let acc = 2.8;
    let vmax = 12.0;
    let t1 = vmax / acc;
    if (dt < t1) { return q + 0.5 * acc * dt * dt; }
    return q + 0.5 * acc * t1 * t1 + vmax * (dt - t1);
}

struct Car { a: f32, c: vec3f, beam: f32, glint: vec3f }

fn cars(ro: vec3f, rd: vec3f, t: f32, l: Look, fp: f32) -> Car {
    var out = Car(0.0, vec3f(0.0), 0.0, vec3f(0.0));
    // body test on the roof plane, beams and lamp reflections on the road
    let tr = (1.3 - ro.y) / rd.y;
    let rp = (ro + rd * tr).xz;
    let gp = (ro + rd * (-ro.y / rd.y)).xz;
    let cyc_i = floor(t / CYCLE);
    let night = l.mode != 2u;
    for (var i = 0; i < 4; i++) {
        let ap = approach(i);
        let s = ap.xy;
        let d = ap.zw;
        let n = vec2f(-d.y, d.x);
        let is_a = i < 2;
        let g0 = select(B_G0, A_G0, is_a);
        let g1 = select(B_G1, A_G1, is_a);
        let u = dot(rp - s, d);
        let v = dot(rp - s, n);
        let ug = dot(gp - s, d);
        let vg = dot(gp - s, n);
        // keep left: lanes at v = -3.2 and -6.6
        for (var ln = 0; ln < 2; ln++) {
            let vc = -3.2 - f32(ln) * 3.4;
            let near_body = abs(v - vc) < 1.3;
            let near_beam = night && abs(vg - vc) < 3.0;
            if (!near_body && !near_beam) { continue; }
            for (var cc = 0; cc < 2; cc++) {
                let c = cyc_i + f32(cc);
                for (var k = 0; k < 4; k++) {
                    let uk = car_u(k, c, g0, g1, t) - f32(ln) * 2.0;
                    let hk = hash_cell2(vec2i(k + ln * 8 + i * 32, i32(c)), 0xca7u);
                    if (hk.w > 0.8) { continue; }
                    if (near_body) {
                        let q = vec2f(u - uk, v - vc);
                        let dd = sdf2_round_box(q, vec2f(2.25, 0.85), 0.35);
                        let a = saturate(0.5 - dd / max(fp, 0.05));
                        if (a > out.a) {
                            // taxis and private cars
                            var body = vec3f(0.0);
                            if (hk.x < 0.3) { body = col_hex(0x1c2140u); }
                            else if (hk.x < 0.42) { body = col_hex(0x0e0e10u); }
                            else if (hk.x < 0.52) { body = col_hex(0xc8a23au); }
                            else if (hk.x < 0.75) { body = col_hex(0xd8d8d6u); }
                            else { body = col_hex(0x8a8c90u); }
                            var col = body * (l.amb * 4.0 + l.day * 0.9 + vec3f(0.02));
                            // glass band and roof
                            let roof = sstep(0.9, 0.5, abs(q.x + 0.2)) * sstep(0.75, 0.5, abs(q.y));
                            col = mix(col, vec3f(0.01), sstep(1.5, 1.2, abs(q.x + 0.2)) * (1.0 - roof) * 0.7);
                            if (night) {
                                // head lamps forward, tail lamps back, taxi roof sign
                                let hl = exp(-dot(q - vec2f(2.15, 0.6), q - vec2f(2.15, 0.6)) * 12.0)
                                       + exp(-dot(q - vec2f(2.15, -0.6), q - vec2f(2.15, -0.6)) * 12.0);
                                let tl = exp(-dot(q - vec2f(-2.2, 0.62), q - vec2f(-2.2, 0.62)) * 14.0)
                                       + exp(-dot(q - vec2f(-2.2, -0.62), q - vec2f(-2.2, -0.62)) * 14.0);
                                col += col_kelvin(5200.0) * hl * 6.0 + vec3f(1.0, 0.05, 0.02) * tl * 3.0;
                                if (hk.x < 0.52) { col += col_kelvin(3000.0) * exp(-dot(q, q) * 6.0) * 1.2; }
                            }
                            out.a = a;
                            out.c = col;
                        }
                    }
                    if (near_beam) {
                        let along = ug - uk - 2.3;
                        if (along > 0.0 && along < 12.0) {
                            let spread = 0.8 + along * 0.22;
                            out.beam += exp(-along / 4.5) * exp(-sq((vg - vc) / spread)) * 0.5;
                        }
                        // lamps mirrored in the wet road: smeared toward the camera
                        let qg = vec2f(ug - uk, vg - vc);
                        let tail = exp(-sq((abs(qg.y) - 0.62) / 0.35) - sq((qg.x + 2.4 + 1.2) / 1.6));
                        let head = exp(-sq((abs(qg.y) - 0.6) / 0.35) - sq((qg.x - 2.2 + 1.2) / 1.6));
                        out.glint += vec3f(1.0, 0.06, 0.02) * tail * 0.5 + col_kelvin(5200.0) * head * 0.8;
                    }
                }
            }
        }
    }
    return out;
}

// ------------------------------------------------------------------ ground
// box-filtered square wave: coverage of bars (duty 0.5) of pitch p over width w
fn bars(x: f32, p: f32, w: f32) -> f32 {
    let ww = max(w, 1e-3);
    let a = x - ww * 0.5;
    let b = x + ww * 0.5;
    let fa = floor(a / p) * p * 0.5 + min(fract(a / p) * p, p * 0.5);
    let fb = floor(b / p) * p * 0.5 + min(fract(b / p) * p, p * 0.5);
    return (fb - fa) / ww;
}

// white paint coverage at ground point g
fn markings(g: vec2f, fp: f32, rd: vec3f) -> f32 {
    var m = 0.0;
    // the footprint stretches along the view direction by 1/sin(elevation)
    let vh = normalize(rd.xz);
    let stretch = 1.0 / max(abs(rd.y), 0.05) - 1.0;
    for (var w = 0; w < 6; w++) {
        let cw = crosswalk(w);
        let a = cw.xy;
        let len = length(cw.zw - a);
        let d = (cw.zw - a) / len;
        let n = vec2f(-d.y, d.x);
        let u = dot(g - a, d);
        let v = dot(g - a, n);
        let hw = crosswalk_w(w) * 0.5;
        if (abs(v) < hw && u > 0.3 && u < len - 0.3) {
            let wu = fp * (1.0 + abs(dot(d, vh)) * stretch) * 1.3;
            m = max(m, bars(u, 0.9, wu));
        }
    }
    // stop lines behind the crossings
    let sl_a = step(abs(abs(g.x) - 19.2), 0.3) * step(abs(g.y), ROAD_A) * step(0.0, -g.y * sign(g.x));
    let sl_b = step(abs(abs(g.y) - 20.2), 0.3) * step(abs(g.x), ROAD_B) * step(0.0, g.x * sign(g.y));
    m = max(m, max(sl_a, sl_b) * 0.9);
    // lane lines on the approaches
    let la = step(ROAD_B + 9.0, abs(g.x)) * step(abs(abs(g.y) - 4.9), 0.08) * step(0.5, fract(g.x / 8.0));
    let lb = step(ROAD_A + 10.0, abs(g.y)) * step(abs(abs(g.x) - 4.8), 0.08) * step(0.5, fract(g.y / 8.0));
    let cl = step(ROAD_B + 9.0, abs(g.x)) * step(abs(g.y), 0.12) + step(ROAD_A + 10.0, abs(g.y)) * step(abs(g.x), 0.12);
    m = max(m, max(max(la, lb), cl) * saturate(1.0 - fp * 3.0 + 0.3));
    return m;
}

// light falling on the ground: the wash of signs, lamps, screens
fn ground_light(g: vec2f, l: Look, t: f32) -> vec3f {
    var e = l.amb + l.day;
    if (l.mode != 2u) {
        // the whole crossing sits in a wash of cool LED and warm shop light
        let r = length(g);
        e += (col_kelvin(6500.0) * 0.17 + col_kelvin(3200.0) * 0.035) * sstep(80.0, 16.0, r);
        e += col_kelvin(6000.0) * 0.12 * sstep(34.0, 10.0, r);
        for (var i = 0; i < 8; i++) {
            var lp: vec2f;
            switch (i) {
                case 0: { lp = vec2f(-18.0, -18.0); }
                case 1: { lp = vec2f(18.0, -18.0); }
                case 2: { lp = vec2f(-18.0, 18.0); }
                case 3: { lp = vec2f(18.0, 18.0); }
                case 4: { lp = vec2f(-45.0, 13.0); }
                case 5: { lp = vec2f(45.0, -13.0); }
                case 6: { lp = vec2f(12.0, -48.0); }
                default: { lp = vec2f(-12.0, 48.0); }
            }
            let d2 = dot(g - lp, g - lp);
            e += l.lamp * 0.3 * 100.0 / (100.0 + d2) * 100.0 / (100.0 + d2 * 0.3);
        }
    }
    // screens: area lights facing the crossing
    for (var s = 0; s < NS; s++) {
        let sd = screen_def(s);
        let b = bld(i32(sd.x));
        let face = u32(sd.y);
        var c3: vec3f;
        var nrm: vec2f;
        if (face == 1u) { c3 = vec3f(b.mx.x, sd.w, sd.z); nrm = vec2f(1.0, 0.0); }
        else { c3 = vec3f(sd.z, sd.w, b.mx.y); nrm = vec2f(0.0, 1.0); }
        let dv = vec3f(g.x, 0.0, g.y) - c3;
        let dist2 = dot(dv, dv);
        let dir = dv * inverseSqrt(dist2);
        let cosn = max(dot(dir.xz, nrm), 0.0);
        let cosg = max(-dir.y, 0.0);
        let sz = screen_size(s);
        e += screen_avg(s, t) * l.screen * sz.x * sz.y * cosn * cosg / (PI * dist2) * 2.2;
    }
    return e;
}

fn city_sky(rd: vec3f, l: Look) -> vec3f {
    // city glow under the cloud: brightest at the horizon
    let y = max(rd.y, 0.0);
    return l.sky * (0.5 + 0.9 * exp(-y * 6.0));
}

fn ground(g: vec2f, rd: vec3f, l: Look, t: f32, fp: f32, ctx: Ctx) -> vec3f {
    let ax = abs(g.x);
    let az = abs(g.y);
    let road = ax < ROAD_B || az < ROAD_A;
    var alb: vec3f;
    var paint = 0.0;
    var kr = 0.0;
    var front = 1e3;
    if (road) {
        let n1 = noise_value2(g * 0.35);
        let n2 = noise_value2(g * 2.1 + vec2f(17.0));
        alb = vec3f(0.05, 0.05, 0.055) * (0.72 + 0.36 * n1 + 0.14 * n2);
        paint = markings(g, fp, rd) * (0.8 + 0.2 * n2);
        alb = mix(alb, vec3f(0.66, 0.66, 0.64), paint);
        kr = mix(0.3, 0.07, paint);
    } else {
        // pavement: light pavers, yellow tactile strips at the kerbs
        let tile = pband(g.x / 0.6, 0.06, 1.0, fp / 0.6) * pband(g.y / 0.6, 0.06, 1.0, fp / 0.6);
        alb = col_hex(0x8e8a84u) * 0.42 * mix(0.85, 1.0, tile);
        let kerb = min(abs(ax - ROAD_B), abs(az - ROAD_A));
        let near_cross = (az < 19.0 && ax < 19.0);
        if (kerb < 1.0 && kerb > 0.4 && near_cross) { alb = col_hex(0xd8b83au) * 0.5; }
        alb *= 1.0 - 0.5 * sstep(0.4, 0.0, kerb);
        front = min(abs(ax - BLK), abs(az - BLK));
        kr = 0.12;
    }
    let wet = l.wet;
    let pud = wet_puddles(g * 0.9, 0.4 * wet, ctx) * select(0.3, 1.0, road) * step(0.5, wet);
    let wsurf = wet_surface(alb, 0.6, (wet * 0.8 + pud * 0.2) * (1.0 - 0.6 * paint));
    // warm pools thrown out of the shopfronts
    let shopl = col_kelvin(3000.0) * exp(-front / 2.8) * select(0.35, 0.03, l.mode == 2u);
    var c = wsurf.rgb * (ground_light(g, l, t) + shopl);
    // the avenues running on past the district: shop light along their
    // edges, a slow stream of head and tail lights down the middle
    let north = ax < 16.0 && g.y < -112.0;
    let west = az < 16.0 && g.x < -100.0;
    if ((north || west) && l.mode != 2u) {
        let acr = select(g.y, g.x, north);
        let alo = select(-g.x, -g.y, north);
        let edge = exp(-max(15.0 - abs(acr), 0.0) / 1.6);
        c += wsurf.rgb * col_kelvin(3300.0) * edge * 3.0;
        let away = acr < 0.0;
        let dir = select(-1.0, 1.0, away);
        let q = alo - dir * 4.0 * t;
        let cell = floor(q / 7.0);
        let hc = hash_cell2(vec2i(i32(cell), select(0, 1, away)), 0x7aefu);
        if (hc.x < 0.65) {
            let pos = (cell + 0.3 + 0.4 * hc.y) * 7.0;
            let lane = select(3.2, 6.6, hc.z < 0.5);
            let dd = vec2f(q - pos, abs(acr) - lane);
            let pair = exp(-(sq(dd.x) + sq(abs(dd.y) - 0.6)) / max(sq(fp * 0.8), 0.2));
            let lc = select(col_kelvin(5200.0) * 1.6, vec3f(1.0, 0.06, 0.02) * 1.2, away);
            c += lc * pair;
        }
    }
    if (wet > 0.0) {
        // wet mirror: facades and screens, smeared into streaks toward the camera
        let gp = vec3f(g.x, 0.02, g.y);
        let rr = vec3f(rd.x, -rd.y, rd.z);
        let vh = normalize(rd.xz);
        let lat = dot(g, vec2f(-vh.y, vh.x));
        let lon = dot(g, vh);
        let streak = noise_value2(vec2f(lat * 1.7, lon * 0.12)) - 0.5;
        let spread = mix(0.05, 0.015, pud);
        var refl = vec3f(0.0);
        for (var j = 0; j < 2; j++) {
            let off = ((f32(j) + ctx.jitter) - 1.0) * 1.4 * spread + streak * 0.07 * (1.0 - pud * 0.6);
            let r2 = normalize(rr + vec3f(0.0, off, 0.0));
            let h = scene_hit(gp, r2);
            if (h.z >= 0.0) {
                let hp = gp + r2 * h.x;
                refl += shade_near(hp, u32(h.y), i32(h.z), l, t, 0.6 + h.x * 0.02);
            } else {
                refl += city_sky(r2, l);
            }
        }
        refl *= 0.5;
        c += refl * wet * (kr + pud * 0.45);
    }
    return c;
}

// ------------------------------------------------------------------ scene
fn haze_of(c: vec3f, d: f32, l: Look) -> vec3f {
    return mix(c, l.sky * 0.9, 1.0 - exp(-d * l.haze));
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let cyc = fmod_pos(t, CYCLE);
    let ro = vec3f(26.0, 50.0, 84.0);
    let cam = cam_look_at(ro, vec3f(-3.0, 9.0, -13.0), 0.0, 36.0);
    let rd = cam_ray(cam, p);
    let rainy = l.mode == 0u;
    let mpp = ctx.px / cam.zoom;   // metres per pixel per metre of distance

    let hit = scene_hit(ro, rd);
    let tg = select(1e9, -ro.y / rd.y, rd.y < -1e-4);
    let tnear = min(hit.x, tg);
    let far = far_hit(ro, rd, min(tnear, 1e4));
    var col: vec3f;
    if (far.x > 0.0) {
        let hp = ro + rd * far.x;
        let cell = vec2i(i32(far.z), i32(far.w));
        let b = far_lot(cell);
        let id = hash_u2(bitcast<vec2u>(cell)) % 100000u + 1000u;
        col = facade(hp, u32(far.y), b, id, -1, l, t, mpp * far.x);
        col = haze_of(col, far.x, l);
    } else if (hit.z >= 0.0 && hit.x < tg) {
        let hp = ro + rd * hit.x;
        col = shade_near(hp, u32(hit.y), i32(hit.z), l, t, mpp * hit.x);
        col = haze_of(col, hit.x, l);
    } else if (tg < 1e8) {
        let g = (ro + rd * tg).xz;
        let fp = mpp * tg;
        col = ground(g, rd, l, t, fp, ctx);
        // car head-lamp pools, their reflections, and the cars
        let cr = cars(ro, rd, t, l, fp);
        col += col_kelvin(5000.0) * cr.beam * select(0.0, 0.06, l.mode != 2u) * (1.0 + 1.5 * l.wet);
        col += cr.glint * l.wet * 0.35;
        // heads (or umbrellas) on the plane 1.5 m up
        let th = (1.5 - ro.y) / rd.y;
        let hp = (ro + rd * th).xz;
        var best = vec2f(0.0);
        for (var w = 0; w < 6; w++) {
            let cw = crowd(hp, w, cyc, fp, rainy);
            if (cw.x > best.x) { best = cw; }
        }
        let pv = pavement_crowd(hp, t, fp, rainy);
        if (pv.x > best.x) { best = pv; }
        if (best.x > 0.0) {
            let pl = ground_light(hp, l, t);
            var pc = person_col(best.y, rainy) * (pl * 1.4 + vec3f(0.012));
            if (rainy && best.y < 0.34) {
                // clear umbrellas glow with the light around them
                pc = mix(col, pl * 0.38 + vec3f(0.012), 0.5);
            }
            col = mix(col, pc, best.x);
        }
        col = mix(col, cr.c, cr.a);
        col = haze_of(col, tg, l);
    } else {
        col = city_sky(rd, l);
    }
    // screens glow in the damp air
    for (var s = 0; s < NS; s++) {
        let sd = screen_def(s);
        let b = bld(i32(sd.x));
        var c3: vec3f;
        if (u32(sd.y) == 1u) { c3 = vec3f(b.mx.x + 1.0, sd.w, sd.z); } else { c3 = vec3f(sd.z, sd.w, b.mx.y + 1.0); }
        let oc = c3 - ro;
        let tc = max(dot(oc, rd), 0.0);
        let d2 = dot(oc, oc) - tc * tc;
        let sz = screen_size(s);
        let r2 = sz.x * sz.y * 0.3;
        col += screen_avg(s, t) * l.screen * select(0.03, 0.07, rainy) * sq(r2 / (r2 + d2)) * select(1.0, 0.2, l.mode == 2u);
    }
    if (rainy) {
        let r = rain_streaks(p, ctx, 0.5, 2.0, 0.08, 3);
        col += r * (vec3f(0.01) + col * 0.3);
    }
    return col * exp2(l.exposure);
}
