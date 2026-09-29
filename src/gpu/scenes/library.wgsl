//! name: library
//! title: Candlelit Library
//! category: cozy
//! tags: library, books, candle, lamp, dust, rain, interior
//! desc: an old reading room: towering shelves, a green banker's lamp, a candle, dust in the air
//! themes: candle, dawn, storm
//! uses: sdf, rain, noise
//! cost: light
//! fallback: den
//! credits: original

// A panelled reading room seen from a chair at the desk. The back wall is
// shelving floor to ceiling around a tall arched window; the desk fills the
// foreground. Surfaces carry a pseudo-3D position (x, y on screen, z toward
// the viewer) and are lit by up to three lights: the banker's lamp, the
// candle, and the window (moonlight, a dawn shaft full of dust, or storm
// light with rare lightning).
//
// Past the frame the room goes on: the desk ends and more bays of books run
// down to a panelled plinth and a Persian carpet; a second arched window
// between a standing lamp and a wall sconce (the far side's own warm light,
// windowed so it never reaches back into the frame), a globe on its stand; a
// coffered ceiling above the cornice. Far left, a twin sconce.

const WIN_C: vec2f = vec2f(0.0, 0.14);
const WIN_H: vec2f = vec2f(0.125, 0.25);    // straight part half size; arch on top
const WIN2_C: vec2f = vec2f(2.24, 0.14);    // second window, beyond the frame
const SHELF_H: f32 = 0.13;                  // shelf pitch
const DESK_BACK: f32 = -0.25;
const DESK_FRONT: f32 = -0.46;
const DESK_R: f32 = 1.15;                   // right end of the desk's front edge
const LAMP: vec2f = vec2f(-0.46, -0.1);     // shade centre
const CANDLE: vec2f = vec2f(0.43, -0.2);    // wick
const FLAMP: vec2f = vec2f(1.84, 0.08);     // standing lamp: shade centre
const SCONCE: vec2f = vec2f(2.55, 0.24);    // wall sconce on the upright right of it: shade centre
const SCONCE_L: vec2f = vec2f(-1.53, 0.24); // and its twin far left
const GLOBE: vec2f = vec2f(2.63, -0.42);    // globe centre
const FLOOR_Y: f32 = -0.6;                  // the shelves meet the floor
const CORN_TOP: f32 = 0.56;                 // the cornice meets the ceiling

// The entry point continues past the frame's edge as a cylinder, which suits
// a 3D camera; this room is painted flat, so undo it and keep the desk's own
// scale there (a window keeps its shape on a portrait monitor beside the
// row). Identity inside the frame.
fn flat_p(p: vec2f, ctx: Ctx) -> vec2f {
    let hx = ctx.half.x;
    let a = abs(p.x);
    if (a <= hx) { return p; }
    return vec2f(sign(p.x) * (hx + (atan(a) - atan(hx)) * (1.0 + hx * hx)), p.y);
}

struct Look {
    mode: u32,        // 0 candle, 1 dawn, 2 storm
    lamp: f32,
    candle: f32,
    win_c: vec3f,     // light through the window
    sky_top: vec3f,
    sky_low: vec3f,
    amb: vec3f,
    shaft: f32,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, 0.0, 0.6, col_hex(0xcfe0ffu) * 1.0, col_hex(0x6a86b8u) * 0.6, col_hex(0xf0c8b8u) * 0.9,
                        col_hex(0x8aa0c8u) * 0.09, 1.0, 0.8);
        }
        case 2u: {
            return Look(2u, 0.8, 0.0, col_hex(0x8a9ab8u) * 0.22, col_hex(0x1c2230u) * 0.5, col_hex(0x4a5260u) * 0.45,
                        col_hex(0x607090u) * 0.03, 0.0, 0.7);
        }
        default: {
            return Look(0u, 1.0, 1.0, col_hex(0x6a80b0u) * 0.05, col_hex(0x0a1226u) * 0.4, col_hex(0x1c2848u) * 0.4,
                        col_hex(0x506080u) * 0.01, 0.0, 0.7);
        }
    }
}

// lightning: (flash brightness 0..1) — rare double flicker, closed-form
fn lightning(t: f32) -> f32 {
    let ev = hash_event(t, 19.0, 0x11ceu);
    if (ev.x > 0.55) { return 0.0; }
    let lt = ev.y * 19.0 - 3.0 - ev.x * 8.0;
    if (lt < 0.0 || lt > 0.8) { return 0.0; }
    return exp(-lt * 14.0) + 0.6 * exp(-abs(lt - 0.25) * 22.0);
}

// Light arriving at pseudo-3D point pos with normal n.
fn room_light(pos: vec3f, n: vec3f, l: Look, t: f32, flash: f32) -> vec3f {
    var e = l.amb;
    // banker's lamp: the shade sends light down and forward
    if (l.lamp > 0.0) {
        let lp = vec3f(LAMP.x, LAMP.y - 0.03, -0.55);
        let d = lp - pos;
        let dist2 = dot(d, d);
        let ld = d * inverseSqrt(dist2);
        let down = smoothstep(-0.35, 0.25, ld.y);    // the shade blocks light going up
        e += col_kelvin(2700.0) * l.lamp * (max(dot(n, ld), 0.0) * 0.1 * (0.2 + 0.8 * down) / (dist2 + 0.012)
             + 0.13 / (1.0 + dist2 * 3.0));
    }
    // candle: small, low, warm; breathing slowly
    if (l.candle > 0.0) {
        let fl = 0.85 + 0.15 * (0.6 * sin(t * 1.3) * sin(t * 0.8 + 1.0) + 0.4 * sin(t * 0.5));
        let lp = vec3f(CANDLE.x, CANDLE.y + 0.03, -0.6);
        let d = lp - pos;
        let dist2 = dot(d, d);
        let ld = d * inverseSqrt(dist2);
        e += col_kelvin(1850.0) * l.candle * fl * (max(dot(n, ld), 0.0) * 0.03 / (dist2 + 0.006) + 0.04 / (1.0 + dist2 * 5.0));
    }
    // standing lamp beyond the frame: the shade glows and throws light
    // down and (less) up
    if (l.lamp > 0.0) {
        let lp = vec3f(FLAMP.x, FLAMP.y - 0.02, -0.16);
        let d = lp - pos;
        let dist2 = dot(d, d);
        let ld = d * inverseSqrt(dist2);
        let down = 0.45 + 0.55 * smoothstep(-0.3, 0.3, ld.y);
        // (windowed so it fades out before reaching the frame)
        let toward = exp(-sq(max(FLAMP.x - 0.4 - pos.x, 0.0)) * 10.0);
        e += col_kelvin(2800.0) * l.lamp * (max(dot(n, ld), 0.0) * 0.08 * down / (dist2 + 0.02) + 0.08 / (1.0 + dist2 * 3.0)) * exp(-dist2 * 1.2) * toward;
        // the sconce: smaller, higher, on the far side of the window
        let sp = vec3f(SCONCE.x, SCONCE.y - 0.02, -0.08);
        let sd = sp - pos;
        let sdist2 = dot(sd, sd);
        let sl = sd * inverseSqrt(sdist2);
        let sdown = 0.5 + 0.5 * smoothstep(-0.3, 0.3, sl.y);
        e += col_kelvin(2600.0) * l.lamp * (max(dot(n, sl), 0.0) * 0.045 * sdown / (sdist2 + 0.015) + 0.05 / (1.0 + sdist2 * 3.0)) * exp(-sdist2 * 1.0) * toward;
        // the twin far left, fading out toward the frame the same way
        let lpl = vec3f(SCONCE_L.x, SCONCE_L.y - 0.02, -0.08);
        let dl = lpl - pos;
        let ldist2 = dot(dl, dl);
        let ll = dl * inverseSqrt(ldist2);
        let ldown = 0.5 + 0.5 * smoothstep(-0.3, 0.3, ll.y);
        let toward_l = exp(-sq(max(pos.x - SCONCE_L.x - 0.25, 0.0)) * 30.0);
        e += col_kelvin(2600.0) * l.lamp * (max(dot(n, ll), 0.0) * 0.045 * ldown / (ldist2 + 0.015) + 0.05 / (1.0 + ldist2 * 3.0)) * exp(-ldist2 * 1.0) * toward_l;
    }
    // windows: soft light from the back wall
    let wd = vec3f(WIN_C, 0.05) - pos;
    let wdist2 = dot(wd, wd);
    e += l.win_c * (0.25 * max(dot(n, normalize(wd)), 0.0) / (1.0 + wdist2 * 4.0) + 0.12 / (1.0 + wdist2 * 3.0));
    let wd2 = vec3f(WIN2_C, 0.05) - pos;
    let wdist22 = dot(wd2, wd2);
    e += l.win_c * (0.25 * max(dot(n, normalize(wd2)), 0.0) / (1.0 + wdist22 * 4.0) + 0.12 / (1.0 + wdist22 * 3.0)) * exp(-wdist22 * 0.8)
         * exp(-sq(max(WIN2_C.x - 0.75 - pos.x, 0.0)) * 6.0);
    e += col_hex(0xdce8ffu) * flash * 1.2;
    return e;
}

// ------------------------------------------------------------------ window
fn win_d(p: vec2f, c: vec2f) -> f32 {
    let q = p - c;
    let rect = sdf2_box(q, WIN_H);
    let arch = length(q - vec2f(0.0, WIN_H.y)) - WIN_H.x;
    return select(rect, min(rect, arch), q.y > 0.0);
}

// leaded panes: 1 on the lead lines
fn leads(p: vec2f, c: vec2f, aa: f32) -> f32 {
    let q = p - c;
    // diamond lattice
    let a = (q.x + q.y) / 0.05;
    let b = (q.x - q.y) / 0.05;
    let da = abs(fract(a) - 0.5);
    let db = abs(fract(b) - 0.5);
    let w = max(0.06, aa / 0.05 * 0.7);
    var m = max(smoothstep(w, w * 0.4, 0.5 - da), smoothstep(w, w * 0.4, 0.5 - db)) * 0.8;
    // stone mullion and transom
    m = max(m, smoothstep(0.009, 0.006, abs(q.x)));
    m = max(m, smoothstep(0.009, 0.006, abs(q.y - 0.06)));
    return m;
}

fn outside(p: vec2f, wc: vec2f, l: Look, t: f32, flash: f32) -> vec3f {
    let q = p - wc;
    var c = mix(l.sky_low, l.sky_top, smoothstep(-0.2, 0.35, q.y));
    // rooftops and a spire across the quad
    let roof = -0.14 + 0.03 * step(0.5, fract(p.x * 9.0 + 0.3)) + 0.12 * saturate(1.0 - abs(p.x - 0.06) / 0.015) * step(abs(p.x - 0.06), 0.015);
    if (q.y < roof) { c = l.sky_low * select(0.15, 0.35, l.mode == 1u); }
    if (l.mode == 0u) {
        // the moon, high in the left pane
        let md = length(p - vec2f(-0.06, 0.26));
        c += vec3f(1.0, 0.97, 0.9) * (smoothstep(0.02, 0.017, md) * 1.2 + 0.1 * exp(-md * 20.0));
    }
    // clouds drifting slowly across the glass: moonlit edges, pink at dawn
    if (l.mode != 2u && q.y > roof) {
        let cq = vec2f(p.x * 5.0 - t * 0.05, p.y * 11.0);
        let cn = noise_fbm2(cq + vec2f(3.0, 7.0), 5);
        let cloud = smoothstep(0.5, 0.72, cn) * smoothstep(-0.05, 0.12, q.y);
        let md = length(p - vec2f(-0.06, 0.26));
        var cc = select(col_hex(0x1a2438u) * 0.35 + vec3f(0.35, 0.36, 0.4) * exp(-md * 9.0), col_hex(0xf0b8a8u) * 0.9, l.mode == 1u);
        c = mix(c, cc, cloud * 0.85);
    }
    c += col_hex(0xe0e8ffu) * flash * 1.5;
    return c;
}

// ------------------------------------------------------------------ shelves
fn book_col(h: f32) -> vec3f {
    let i = u32(h * 11.0);
    switch (i) {
        case 0u: { return col_hex(0x5a1a14u); }  // oxblood
        case 1u: { return col_hex(0x1e3a28u); }  // bottle green
        case 2u: { return col_hex(0x1c2640u); }  // navy
        case 3u: { return col_hex(0x8a6a44u); }  // tan calf
        case 4u: { return col_hex(0x4a2e1cu); }  // brown
        case 5u: { return col_hex(0x141210u); }  // black
        case 6u: { return col_hex(0x7a5a3au); }  // faded
        case 7u: { return col_hex(0x6a2a1eu); }  // rust
        case 8u: { return col_hex(0xa89a78u); }  // vellum
        case 9u: { return col_hex(0x2a3a3au); }  // slate
        default: { return col_hex(0x5a4430u); }
    }
}

// the back wall: shelving. Returns colour (lit).
fn shelves(p: vec2f, l: Look, t: f32, flash: f32, aa: f32) -> vec3f {
    let wood = col_hex(0x3a2414u) * 0.5;
    // bays: uprights every 0.34
    let bay = fract((p.x + 0.17) / 0.34);
    let upr = min(bay, 1.0 - bay) * 0.34;
    let row = floor((p.y + 0.5) / SHELF_H);
    let ry = fract((p.y + 0.5) / SHELF_H);
    let n_face = vec3f(0.0, 0.0, -1.0);
    var c: vec3f;
    // shelf boards and uprights
    let board = step(ry, 0.14);
    if (upr < 0.012 || board > 0.5) {
        var bn = n_face;
        if (board > 0.5) { bn = normalize(vec3f(0.0, select(-0.3, 0.6, ry > 0.1), -1.0)); }
        c = wood * (0.85 + 0.2 * noise_value2(p * vec2f(80.0, 6.0))) * room_light(vec3f(p, -0.02), bn, l, t, flash);
        return c;
    }
    // books: jittered-width slots
    let x = p.x * 1.0;
    let w = 0.024;
    let k = floor(x / w);
    let hb = hash_cell2(vec2i(i32(k), i32(row)), 0xb00cu);
    let hbn = hash_cell2(vec2i(i32(k) + 1, i32(row)), 0xb00cu);
    let x0 = (k + (hb.w - 0.5) * 0.35) * w;
    let x1 = (k + 1.0 + (hbn.w - 0.5) * 0.35) * w;
    var bid = k;
    var h = hb;
    var bx0 = x0;
    var bx1 = x1;
    if (x < x0) {
        bid = k - 1.0;
        h = hash_cell2(vec2i(i32(bid), i32(row)), 0xb00cu);
        bx1 = x0;
        bx0 = (bid + (h.w - 0.5) * 0.35) * w;
    } else if (x > x1) {
        bid = k + 1.0;
        h = hbn;
        bx0 = x1;
        let h2 = hash_cell2(vec2i(i32(bid) + 1, i32(row)), 0xb00cu);
        bx1 = (bid + 1.0 + (h2.w - 0.5) * 0.35) * w;
    }
    let clear = 1.0 - 0.14;
    let top = 0.14 + clear * (0.72 + 0.24 * h.y);
    let u = (x - bx0) / max(bx1 - bx0, 1e-4);
    // back of the case, in deep shadow
    var back = wood * 0.3 * room_light(vec3f(p, 0.05), n_face, l, t, flash);
    // gaps, and now and then a stack lying flat
    let group = hash_cell2(vec2i(i32(floor(x / (w * 5.0))), i32(row)), 0x57acu);
    if (group.x < 0.07) {
        let sy = (ry - 0.14) / clear;
        let layer = floor(sy / 0.13);
        let hs = hash_cell2(vec2i(i32(layer), i32(row) * 31 + i32(floor(x / (w * 5.0)))), 0x5a5au);
        let edge = abs(fract(x / (w * 5.0)) - 0.5);
        if (layer < 3.0 && edge < 0.38 + 0.08 * hs.x) {
            let bc = book_col(hs.y) * 0.6 * (0.8 + 0.2 * smoothstep(0.0, 0.3, fract(sy / 0.13)));
            return bc * room_light(vec3f(p, -0.04), normalize(vec3f(0.0, 0.3, -1.0)), l, t, flash);
        }
        return back;
    }
    if (h.x < 0.05 || ry > top) { return back; }
    var bc = book_col(h.z);
    bc *= 0.75 + 0.35 * hash_f(u32(bid) * 13u + u32(row));
    // gilt bands and a label on some spines
    let sy = (ry - 0.14) / (top - 0.14);
    let gilt = step(0.5, h.y) * (step(abs(sy - 0.9), 0.012) + step(abs(sy - 0.84), 0.008) + step(abs(sy - 0.1), 0.01));
    bc = mix(bc, col_hex(0xb89040u) * 0.9, saturate(gilt) * saturate(1.5 - aa * 200.0));
    let label = step(0.75, h.x) * step(abs(sy - 0.7), 0.06) * step(abs(u - 0.5), 0.3);
    bc = mix(bc, col_hex(0xc8b890u) * 0.6, label);
    // rounded spine
    let across = (u - 0.5) * 2.0;
    let n = normalize(vec3f(across * 0.8, 0.0, -1.0));
    bc *= 0.7 + 0.3 * (1.0 - across * across);
    // the shelf above shades the tops of the books
    let shade = 0.55 + 0.45 * smoothstep(1.0, 0.55, ry);
    c = bc * 0.55 * room_light(vec3f(p, -0.03), n, l, t, flash) * shade;
    // cracks between spines
    c *= smoothstep(0.0, 0.08, min(u, 1.0 - u)) * 0.6 + 0.4;
    return c;
}

// ------------------------------------------------------------------ desk items
fn candle_flame(p: vec2f, t: f32) -> vec4f {
    let sway = 0.003 * sin(t * 1.1) + 0.0015 * sin(t * 2.3 + 1.0);
    let q = p - CANDLE - vec2f(sway * clamp(p.y - CANDLE.y, 0.0, 0.05) * 40.0, 0.0);
    // teardrop: round bottom, tapering to a point that stretches and shrinks
    let tip = 0.032 * (0.88 + 0.12 * sin(t * 1.7) * sin(t * 0.9 + 2.0));
    let yy = q.y - 0.012;
    let r = 0.0058 * sqrt(saturate(1.0 - yy / tip));
    var d = length(vec2f(q.x, min(yy, 0.0) * 1.3)) - 0.0058;
    if (yy > 0.0) { d = abs(q.x) - r; }
    if (yy > tip) { d = length(vec2f(q.x, yy - tip)); }
    let core = smoothstep(0.002, -0.004, d);
    let blue = smoothstep(0.008, 0.0, q.y) * core;
    var c = mix(vec3f(3.2, 2.2, 1.0), vec3f(2.4, 0.9, 0.25), smoothstep(-0.006, 0.0, d));
    c = mix(c, vec3f(0.2, 0.3, 1.0), blue * 0.5);
    return vec4f(c, smoothstep(0.003, -0.002, d));
}

// ------------------------------------------------------------------ beyond the frame
// Coffered ceiling in perspective, above the cornice.
fn ceiling(p: vec2f, l: Look, t: f32, flash: f32, aa: f32) -> vec3f {
    let z = CORN_TOP * 0.3 / max(p.y, 0.01);        // 0.3 at the wall, smaller toward us
    let wx = p.x * z;
    // deep beams parallel to the wall, light ribs between them: coffers.
    // (Far from the centre the ribs run nearly flat, so they stay quiet.)
    let cu = vec2f(wx / 0.1, (0.3 - z) / 0.045);
    let f = abs(fract(cu) - 0.5);
    let lod = saturate(1.0 - aa * z * 18.0);
    let beam = sstep(0.33, 0.38, f.y);
    let rib = sstep(0.4, 0.44, f.x) * (1.0 - beam) * lod;
    let mould = sstep(0.25, 0.29, f.y) * (1.0 - beam);
    var alb = col_hex(0x5a3a24u) * 0.45;
    alb = mix(alb, col_hex(0x3a2414u) * 0.55, beam);
    alb = mix(alb, col_hex(0x4a2e1au) * 0.5, rib * 0.6);
    alb = mix(alb, col_hex(0xb89040u) * 0.4, mould * 0.45 * lod);
    // beam soffits face down; the moulding faces tilt toward the room
    let side = sign(fract(cu.y) - 0.5);
    let n = normalize(vec3f(0.0, -1.0, -0.8 * mould * side));
    let pos = vec3f(p.x, CORN_TOP + 0.02, -(0.3 - z) * 2.0);
    return alb * room_light(pos, n, l, t, flash) * 1.6;
}

// The floor beyond the desk: parquet, and a Persian carpet on it.
fn floor_c(p: vec2f, l: Look, t: f32, flash: f32, aa: f32) -> vec3f {
    let z = -FLOOR_Y * 0.3 / max(-p.y, 0.01);       // 0.3 at the wall
    let wx = p.x * z;
    let lod = saturate(1.0 - aa * z * 25.0);
    // oak parquet in a herringbone of short boards
    let b = vec2f(wx / 0.05 + floor((0.3 - z) / 0.025) * 0.5, (0.3 - z) / 0.025);
    let hb = hash_cell2(vec2i(floor(b)), 0x0a4u);
    var alb = mix(col_hex(0x5a3a20u), col_hex(0x74502cu), hb.x) * 0.45;
    alb *= 1.0 - 0.35 * lod * max(sstep(0.44, 0.5, abs(fract(b.x) - 0.5)), sstep(0.42, 0.5, abs(fract(b.y) - 0.5)));
    // carpet: deep red field, a navy border with gold lines, a lattice of lozenges
    let cq = vec2f(wx - 0.02, z - 0.19);
    let cd = sdf2_box(cq, vec2f(0.62, 0.085));
    if (cd < 0.0) {
        let bd = -cd;                                        // distance in from the edge
        var cc = col_hex(0x8a2620u);
        let lz = abs(fract(cq.x / 0.07) - 0.5) + abs(fract(cq.y / 0.05) - 0.5);
        cc = mix(cc, col_hex(0x3a1830u), sstep(0.05, 0.02, abs(lz - 0.5)) * 0.6 * lod);
        cc = mix(cc, col_hex(0xb08040u), sstep(0.1, 0.06, lz) * 0.5 * lod);
        cc = mix(cc, col_hex(0x5a1410u), 0.3 * sstep(0.3, 0.1, lz));
        let border = sstep(0.016, 0.014, bd);
        cc = mix(cc, col_hex(0x1c2440u), border);
        cc = mix(cc, col_hex(0xc8a060u), (sstep(0.004, 0.002, abs(bd - 0.004)) + sstep(0.003, 0.0015, abs(bd - 0.0165))) * lod);
        // pile: a soft sheen, fringe at the ends
        alb = cc * 0.5 * (0.9 + 0.1 * noise_value2(vec2f(wx * 200.0, z * 400.0)));
    }
    let pos = vec3f(p.x, FLOOR_Y - 0.02, -(0.3 - z) * 3.0);
    var c = alb * room_light(pos, vec3f(0.0, 1.0, 0.0), l, t, flash);
    // a dark skirting line where the floor meets the plinth
    c *= 0.4 + 0.6 * sstep(FLOOR_Y, FLOOR_Y - 0.02, p.y);
    return c;
}

// the standing lamp: fabric drum shade, brass column, round foot. (colour, coverage)
fn floor_lamp(p: vec2f, l: Look, t: f32, flash: f32, ctx: Ctx) -> vec4f {
    let q = p - FLAMP;
    // shade: a truncated cone, wider at the bottom
    let hw = mix(0.075, 0.055, saturate((q.y + 0.055) / 0.11));
    let shade = max(abs(q.x) - hw, abs(q.y) - 0.055);
    let column = sdf2_box(p - vec2f(FLAMP.x, (FLAMP.y - 0.055 + FLOOR_Y - 0.07) * 0.5), vec2f(0.006, (FLAMP.y - 0.055 - FLOOR_Y + 0.07) * 0.5));
    let foot = sdf2_round_box(p - vec2f(FLAMP.x, FLOOR_Y - 0.075), vec2f(0.05, 0.01), 0.006);
    let brass = min(column, foot);
    var c = vec3f(0.0);
    var a = 0.0;
    let ba = aa_fill(brass, ctx);
    if (ba > 0.0) {
        let bn = normalize(vec3f(clamp((p.x - FLAMP.x) / 0.006, -1.0, 1.0) * 0.8, 0.2, -1.0));
        c = col_hex(0xb8903cu) * 0.5 * room_light(vec3f(p, -0.16), bn, l, t, flash);
        a = ba;
    }
    let sa = aa_fill(shade, ctx);
    if (sa > 0.0) {
        // parchment glowing from within, brighter toward the rims, seams
        let across = q.x / hw;
        let down = saturate((0.055 - q.y) / 0.11);
        var sc = col_hex(0xffc47au) * l.lamp * (0.07 + 0.1 * (1.0 - across * across)) * (0.7 + 0.6 * down);
        sc += col_hex(0xe8d4a8u) * 0.3 * room_light(vec3f(p, -0.2), normalize(vec3f(across * 0.7, 0.0, -1.0)), l, t, flash);
        sc *= 0.9 + 0.1 * sstep(0.02, 0.0, abs(fract(across * 2.5) - 0.5) - 0.45);
        let rim = sstep(0.008, 0.0, 0.055 - abs(q.y));
        sc = mix(sc, col_hex(0xb07a3au) * (0.03 + l.lamp * 0.3), rim);
        c = mix(c, sc, sa);
        a = max(a, sa);
    }
    return vec4f(c, a);
}

// a terrestrial globe on a turned wooden stand, a brass meridian ring
fn globe(p: vec2f, l: Look, t: f32, flash: f32, ctx: Ctx) -> vec4f {
    let q = p - GLOBE;
    let r = 0.07;
    let sph = length(q) - r;
    let ring = abs(length(q * vec2f(1.0, 0.97)) - r - 0.01) - 0.004;
    let stand = min(min(sdf2_box(p - vec2f(GLOBE.x, GLOBE.y - 0.13), vec2f(0.008, 0.055)),
                        sdf2_round_box(p - vec2f(GLOBE.x, GLOBE.y - 0.09), vec2f(0.03, 0.008), 0.004)),
                    min(sdf2_segment(p, vec2f(GLOBE.x, GLOBE.y - 0.18), vec2f(GLOBE.x - 0.06, FLOOR_Y - 0.13)) - 0.006,
                        sdf2_segment(p, vec2f(GLOBE.x, GLOBE.y - 0.18), vec2f(GLOBE.x + 0.06, FLOOR_Y - 0.13)) - 0.006));
    var c = vec3f(0.0);
    var a = 0.0;
    let sa = aa_fill(stand, ctx);
    if (sa > 0.0) {
        c = col_hex(0x4a2a16u) * 0.5 * room_light(vec3f(p, -0.2), normalize(vec3f(0.2, 0.3, -1.0)), l, t, flash);
        a = sa;
    }
    let ga = aa_fill(sph, ctx);
    if (ga > 0.0) {
        let u = q / r;
        let nz = sqrt(saturate(1.0 - dot(u, u)));
        let n = normalize(vec3f(u.x, u.y, -nz));
        // antique map: parchment land on sea-green, tilted axis
        let lon = atan2(u.x, nz) + 0.6;
        let lat = u.y;
        let land = sstep(0.52, 0.56, noise_fbm2(vec2f(lon * 2.2, lat * 3.0) + vec2f(3.0, 1.0), 3));
        var alb = mix(col_hex(0x4a6a58u), col_hex(0xc8b080u), land) * 0.5;
        alb *= 0.85 + 0.15 * sstep(0.02, 0.0, abs(fract(lat * 4.0) - 0.5) - 0.46);
        var gc = alb * room_light(vec3f(p, -0.2 - nz * 0.05), n, l, t, flash);
        // varnish: a warm highlight from the lamp side
        gc += col_kelvin(2800.0) * l.lamp * 0.05 * pow(saturate(dot(n, normalize(vec3f(-0.6, 0.5, -0.6)))), 12.0);
        c = mix(c, gc, ga);
        a = max(a, ga);
    }
    let ra = aa_fill(ring, ctx) * select(1.0, 0.0, q.y < -r * 0.9);
    if (ra > 0.0) {
        c = mix(c, col_hex(0xb8903cu) * 0.55 * room_light(vec3f(p, -0.22), normalize(vec3f(sign(q.x) * 0.6, 0.3, -1.0)), l, t, flash), ra);
        a = max(a, ra);
    }
    return vec4f(c, a);
}

// ------------------------------------------------------------------ scene
fn scene(pw: vec2f, ctx: Ctx) -> vec3f {
    let p = flat_p(pw, ctx);
    let l = look(ctx.theme);
    let t = ctx.t;
    let aa = ctx.px;
    var flash = 0.0;
    if (l.mode == 2u) { flash = lightning(t); }
    var col: vec3f;

    // ---- back wall: cornice, shelves, window
    col = shelves(p, l, t, flash, aa);
    if (p.y > 0.44) {
        // carved cornice
        let v = (p.y - 0.44) / 0.1;
        let bn = normalize(vec3f(0.0, -0.6 + 0.4 * sin(v * 9.0), -1.0));
        col = col_hex(0x3a2414u) * 0.45 * room_light(vec3f(p, -0.03), bn, l, t, flash) * (0.85 + 0.15 * sin(p.x * 120.0) * step(0.3, fract(v * 3.0)));
        if (p.y > CORN_TOP) { col = ceiling(p, l, t, flash, aa); }
    }
    if (p.y < -0.5) {
        // panelled plinth under the lowest shelf, then the floor
        let bx = (fract((p.x + 0.17) / 0.34) - 0.5) * 0.34;
        let pd = sdf2_box(vec2f(bx, p.y - (FLOOR_Y - 0.5) * 0.5), vec2f(0.13, (-0.5 - FLOOR_Y) * 0.5 - 0.02));
        // raised panels: a bevel catches the light along their top edges
        let bevel = sstep(0.007, 0.0, abs(pd));
        let pn = normalize(vec3f(0.0, bevel * sign(p.y - (FLOOR_Y - 0.5) * 0.5) * 0.8, -1.0));
        col = col_hex(0x3a2414u) * (0.45 + 0.1 * sstep(0.0, -0.004, pd)) * room_light(vec3f(p, -0.02), pn, l, t, flash) * (0.9 + 0.1 * noise_value2(p * vec2f(60.0, 8.0)));
        if (p.y < FLOOR_Y) { col = floor_c(p, l, t, flash, aa); }
    }
    let second = p.x > 1.12;
    let wc = select(WIN_C, WIN2_C, second);
    let wd = win_d(p, wc);
    if (wd < 0.03) {
        // stone surround
        let st = col_hex(0x8a8074u) * 0.4 * room_light(vec3f(p, -0.01), vec3f(0.0, 0.0, -1.0), l, t, flash);
        col = mix(col, st, aa_fill(wd - 0.03, ctx));
    }
    if (wd < 0.0) {
        var o = outside(p, wc, l, t, flash);
        if (l.mode == 2u) {
            let rg = rain_glass(p * 2.0, t, ctx);
            o = outside(p + rg.xy * 0.3, wc, l, t, flash) * (1.0 + rg.z * 0.5);
        }
        let ld = leads(p, wc, aa);
        o = mix(o, col_hex(0x1a1612u) * 0.2 * (l.amb * 10.0 + l.win_c * 0.3), ld);
        col = mix(col, o, aa_fill(wd, ctx));
    }

    // ---- rolling ladder leaning on the right-hand shelves
    {
        let lx = 0.66 - (p.y - 0.45) * 0.14;
        let rail = min(abs(p.x - lx + 0.045), abs(p.x - lx - 0.045)) - 0.006;
        let rung = max(abs(fract((p.y + 0.5) / 0.07) - 0.5) * 0.07 - 0.004, abs(p.x - lx) - 0.045);
        let ld = min(rail, rung);
        if (ld < aa && p.y < 0.46) {
            let lc = col_hex(0x4a2e18u) * 0.5 * room_light(vec3f(p, -0.15), normalize(vec3f(-0.3, 0.2, -1.0)), l, t, flash);
            col = mix(col, lc, aa_fill(ld, ctx));
        }
    }

    // ---- beyond the frame: the standing lamp and the globe
    if (abs(p.x - FLAMP.x) < 0.09 && p.y < FLAMP.y + 0.06 && p.y > FLOOR_Y - 0.09) {
        let fl = floor_lamp(p, l, t, flash, ctx);
        col = mix(col, fl.rgb, fl.a);
    }
    let sc_c = select(SCONCE_L, SCONCE, p.x > 0.0);
    if (abs(p.x - sc_c.x) < 0.06 && abs(p.y - sc_c.y + 0.03) < 0.07) {
        let q = (p - sc_c) * vec2f(sign(p.x), 1.0);    // the left one is a mirror image
        let hw = mix(0.042, 0.03, saturate((q.y + 0.025) / 0.05));
        let shade = max(abs(q.x) - hw, abs(q.y) - 0.025);
        let plate = sdf2_round_box(q - vec2f(0.0, -0.075), vec2f(0.011, 0.025), 0.005);
        let arm = abs(length((q - vec2f(0.03, -0.055)) * vec2f(1.0, 1.4)) - 0.03) - 0.003;
        let brass = min(plate, max(arm, max(q.x - 0.03, -q.y - 0.075)));
        let ba = aa_fill(min(brass, sdf2_box(q - vec2f(0.0, -0.035), vec2f(0.003, 0.012))), ctx);
        col = mix(col, col_hex(0xb8903cu) * 0.55 * room_light(vec3f(p, -0.05), normalize(vec3f(0.3, 0.3, -1.0)), l, t, flash) + col_kelvin(2600.0) * l.lamp * 0.03 * sstep(-0.02, -0.04, q.y), ba);
        let sa = aa_fill(shade, ctx);
        let across = q.x / hw;
        var sc = col_hex(0xffc47au) * l.lamp * (0.06 + 0.08 * (1.0 - across * across)) * (0.7 + 0.6 * saturate((0.025 - q.y) / 0.05));
        sc += col_hex(0xe8d4a8u) * 0.3 * room_light(vec3f(p, -0.1), normalize(vec3f(across * 0.7, 0.0, -1.0)), l, t, flash);
        sc = mix(sc, col_hex(0xb07a3au) * (0.03 + l.lamp * 0.3), sstep(0.006, 0.0, 0.025 - abs(q.y)));
        col = mix(col, sc, sa);
    }
    if (abs(p.x - GLOBE.x) < 0.1 && p.y < GLOBE.y + 0.09 && p.y > FLOOR_Y - 0.15) {
        let gl = globe(p, l, t, flash, ctx);
        col = mix(col, gl.rgb, gl.a);
    }
    if (l.lamp > 0.0 && p.x > 0.95) {
        // the lamp's glow in the air (faded out before the frame), the cone
        // of light under its shade
        let g = length((p - FLAMP) * vec2f(1.0, 1.4));
        col += col_kelvin(2800.0) * l.lamp * (0.04 * exp(-g * 14.0) + 0.012 * exp(-g * 3.5)) * sstep(0.95, 1.3, p.x);
        let below = FLAMP.y - 0.055 - p.y;
        let cone = exp(-sq((p.x - FLAMP.x) / (0.07 + 0.35 * max(below, 0.0)))) * sstep(0.0, 0.02, below) * exp(-max(below, 0.0) * 2.5);
        col += col_kelvin(2800.0) * l.lamp * 0.012 * cone;
        let gs = length((p - SCONCE) * vec2f(1.0, 1.3));
        col += col_kelvin(2600.0) * l.lamp * (0.03 * exp(-gs * 18.0) + 0.008 * exp(-gs * 4.0));
    }
    if (l.lamp > 0.0 && p.x < -0.95) {
        let gs = length((p - SCONCE_L) * vec2f(1.0, 1.3));
        col += col_kelvin(2600.0) * l.lamp * (0.03 * exp(-gs * 18.0) + 0.008 * exp(-gs * 4.0)) * sstep(-0.95, -1.2, p.x);
    }

    // ---- dawn: a shaft of light full of dust (from whichever window is near)
    var shaft = 0.0;
    if (l.shaft > 0.0) {
        let dir = normalize(vec2f(0.42, -0.6));
        let sc = select(WIN_C, WIN2_C, p.x > 1.5);
        for (var i = 0; i < 10; i++) {
            let s = (f32(i) + 0.5) * 0.055;
            let q = p - dir * s;
            shaft += aa_fill_w(win_d(q, sc), 0.03) * (1.0 - leads(q, sc, 0.01) * 0.7) * exp(-s * 1.5);
        }
        shaft /= 10.0;
        col += l.win_c * shaft * 0.16 * smoothstep(-0.02, 0.05, win_d(p, sc));
    }

    // ---- the desk (its right end lies beyond the frame)
    let dv = (p.y - DESK_FRONT) / (DESK_BACK - DESK_FRONT);
    let desk_x = DESK_R / mix(1.0, 1.25, saturate(dv));
    if (p.y < DESK_BACK && abs(p.x) < desk_x + aa) {
        // top surface in perspective: z from far (back edge) to near
        let v = dv;
        let pz = mix(-0.95, -0.35, v);
        let pos = vec3f(p.x, -0.3, pz);
        let dn = vec3f(0.0, 1.0, 0.0);
        let z = 1.0 / (0.6 + v * 1.4);
        var alb = col_hex(0x4a2a16u) * 0.55 * (0.85 + 0.2 * noise_value2(vec2f(p.x * 14.0 / z, v * 3.0)));
        // green leather writing inset with a gilt border
        let inset = sdf2_box(vec2f(p.x * mix(1.0, 1.25, v), p.y - (DESK_FRONT + DESK_BACK) * 0.5), vec2f(0.34, 0.08));
        if (inset < 0.0) {
            alb = col_hex(0x2e5a3cu) * 0.7 * (0.9 + 0.1 * noise_value2(p * 90.0));
            alb = mix(alb, col_hex(0xb89040u) * 0.7, smoothstep(0.004, 0.001, abs(inset + 0.008)));
        }
        var dc = alb * room_light(pos, dn, l, t, flash);
        // polished wood reflects the lamp and the candle
        dc += col_kelvin(2700.0) * l.lamp * 0.05 * exp(-sq((p.x - LAMP.x) / 0.05) - sq((p.y + 0.36) / 0.04)) * step(0.0, inset);
        dc += col_kelvin(1850.0) * l.candle * 0.06 * exp(-sq((p.x - CANDLE.x) / 0.02) - sq((p.y + 0.3) / 0.05));
        // dawn patch on the desk
        dc += l.win_c * shaft * 0.5 * alb * 4.0;
        // the end of the top catches the light along its edge
        dc += col_kelvin(2700.0) * 0.004 * sstep(0.012, 0.0, desk_x - abs(p.x)) * l.lamp;
        // front edge of the desk: moulding and apron
        if (p.y < DESK_FRONT) {
            let fy = (DESK_FRONT - p.y);
            let an = normalize(vec3f(0.0, select(0.8, -0.1, fy > 0.012), -1.0));
            var ac = col_hex(0x3a2010u) * 0.5 * room_light(vec3f(p.x, p.y, -0.3), an, l, t, flash);
            ac *= 1.0 - 0.5 * smoothstep(0.01, 0.2, fy);
            // brass drawer pull
            let pull = length((p - vec2f(0.0, DESK_FRONT - 0.03)) * vec2f(0.5, 1.0)) - 0.008;
            ac = mix(ac, col_hex(0xb8903cu) * 0.6 * room_light(vec3f(p, -0.3), vec3f(0.0, 0.3, -1.0), l, t, flash), aa_fill(pull, ctx));
            // the corner post at the desk's end
            ac *= 1.0 + 0.5 * sstep(0.02, 0.0, abs(abs(p.x) - DESK_R + 0.02) - 0.01) * sstep(0.3, 0.0, fy);
            col = mix(col, ac, aa_fill(abs(p.x) - DESK_R, ctx));
        } else {
            col = mix(col, dc, aa_fill(abs(p.x) - desk_x, ctx));
        }
    }

    // ---- open book on the leather
    {
        let bq = p - vec2f(-0.01, -0.34);
        let pw = 0.13;
        let ph = 0.05;
        // slight perspective: narrower at the top edge
        let sx = bq.x * (1.0 + bq.y * 1.5);
        let page = sdf2_round_box(vec2f(sx, bq.y), vec2f(pw * 2.0 * 0.5 + pw * 0.0, ph), 0.006);
        let bpage = max(page, -0.0);
        if (bpage < aa) {
            let side = sign(sx);
            let u = abs(sx) / pw;
            // pages curl up toward the spine
            let bulge = 1.0 - 0.35 * exp(-u * 6.0) + 0.08 * (1.0 - u);
            let pn = normalize(vec3f(-side * 0.5 * exp(-u * 4.0), 1.0, -0.2));
            var pc = col_hex(0xe8dcc0u) * 0.8 * room_light(vec3f(p.x, -0.29, -0.7), pn, l, t, flash) * bulge;
            // lines of text: soft grey rows
            let lines = step(0.35, fract((bq.y + ph) / 0.0085)) * step(0.1, u) * step(u, 0.9) * step(abs(bq.y), ph - 0.008);
            pc *= 1.0 - 0.25 * lines * saturate(1.5 - aa * 150.0) - 0.1 * lines;
            // gutter shadow
            pc *= 0.6 + 0.4 * smoothstep(0.0, 0.08, u);
            col = mix(col, pc, aa_fill(bpage, ctx));
        }
    }

    // ---- small stack of books by the candle
    {
        let sq0 = sdf2_box(p - vec2f(0.28, -0.3), vec2f(0.07, 0.012));
        let sq1 = sdf2_box(p - vec2f(0.285, -0.276), vec2f(0.062, 0.012));
        let sq2 = sdf2_box(p - vec2f(0.275, -0.254), vec2f(0.055, 0.01));
        let sd = min(sq0, min(sq1, sq2));
        if (sd < aa) {
            var bc = book_col(0.05);
            if (sq1 < sq0 && sq1 < sq2) { bc = book_col(0.15); }
            if (sq2 < sq0 && sq2 < sq1) { bc = book_col(0.3); }
            let lit = bc * 0.6 * room_light(vec3f(p, -0.62), normalize(vec3f(0.3, 0.4, -1.0)), l, t, flash);
            let pages = step(0.066, abs(p.x - 0.28)) ;
            col = mix(col, lit, aa_fill(sd, ctx));
        }
    }

    // ---- candle in a brass stick
    {
        let wax = sdf2_box(p - vec2f(CANDLE.x, CANDLE.y - 0.045), vec2f(0.009, 0.045));
        let stick = min(sdf2_box(p - vec2f(CANDLE.x, CANDLE.y - 0.1), vec2f(0.005, 0.015)),
                        sdf2_round_box(p - vec2f(CANDLE.x, CANDLE.y - 0.115), vec2f(0.028, 0.006), 0.004));
        if (min(wax, stick) < aa) {
            let wc = col_hex(0xe8dcc0u) * 0.7 * (room_light(vec3f(p, -0.6), vec3f(0.3, 0.2, -1.0), l, t, flash) + col_kelvin(1850.0) * l.candle * 0.12 * smoothstep(CANDLE.y - 0.04, CANDLE.y, p.y));
            let bc = col_hex(0xb8903cu) * 0.6 * room_light(vec3f(p, -0.6), vec3f(0.4, 0.4, -1.0), l, t, flash);
            col = mix(col, bc, aa_fill(stick, ctx));
            col = mix(col, wc, aa_fill(wax, ctx));
        }
        if (l.candle > 0.0) {
            let f = candle_flame(p, t);
            col = mix(col, f.rgb * l.candle, f.a);
            let g = length(p - CANDLE - vec2f(0.0, 0.015));
            col += col_kelvin(1850.0) * l.candle * 0.08 * exp(-g * 30.0) * (0.9 + 0.1 * sin(t * 1.3) * sin(t * 0.8 + 1.0));
        }
    }

    // ---- banker's lamp
    {
        let base = sdf2_round_box(p - vec2f(LAMP.x, -0.305), vec2f(0.06, 0.012), 0.008);
        let stem = sdf2_box(p - vec2f(LAMP.x, -0.21), vec2f(0.006, 0.09));
        let q = p - LAMP;
        // shade: a half cylinder seen from the front
        let shade = max(sdf2_round_box(q, vec2f(0.11, 0.03), 0.02), -q.y - 0.024);
        let brass = min(base, stem);
        if (brass < aa) {
            let bc = col_hex(0xb8903cu) * 0.55 * room_light(vec3f(p, -0.55), normalize(vec3f(0.2, 0.3, -1.0)), l, t, flash);
            col = mix(col, bc + col_kelvin(2700.0) * l.lamp * 0.03 * smoothstep(-0.28, -0.32, p.y), aa_fill(brass, ctx));
        }
        if (shade < aa) {
            // green glass lit from inside, brighter at the rim, a brass trim
            let gy = saturate((q.y + 0.024) / 0.06);
            var sc = col_hex(0x0e6a3au) * (0.08 + l.lamp * (0.35 + 0.6 * exp(-gy * 3.0)));
            sc += col_hex(0x7ad8a0u) * l.lamp * 0.25 * exp(-sq((q.y - 0.02) / 0.01)) * (1.0 - abs(q.x) / 0.13);
            sc += col_hex(0x0e6a3au) * room_light(vec3f(p, -0.55), vec3f(0.0, 0.5, -1.0), l, t, flash) * 0.4;
            let trim = smoothstep(0.006, 0.002, abs(q.y + 0.022));
            sc = mix(sc, col_hex(0xb8903cu) * (0.2 + l.lamp * 0.6), trim);
            col = mix(col, sc, aa_fill(shade, ctx));
        }
        // bright opening under the shade
        let under = exp(-sq(q.x / 0.09) - sq((q.y + 0.03) / 0.008)) * step(q.y, -0.02);
        col += col_kelvin(2900.0) * l.lamp * 0.6 * under;
    }

    // ---- dust motes: drifting in the shaft (and faintly in the lamplight)
    {
        let mq = p * 30.0 + vec2f(t * 0.55, -t * 0.12);
        let c = vec2i(floor(mq));
        let h = hash_cell2(c, 0xd057u);
        if (h.w < 0.2) {
            let wob = vec2f(sin(t * 0.3 + h.x * 20.0), cos(t * 0.23 + h.y * 20.0)) * 0.2;
            let d = length(fract(mq) - 0.5 - (h.xy - 0.5) * 0.5 - wob);
            let r = max(0.06, aa * 30.0 * 0.7);
            let mote = smoothstep(r, r * 0.3, d) * (0.4 + 0.6 * h.z) * (0.06 / r);
            let lampzone = exp(-sq((p.x - LAMP.x) / 0.16) - sq((p.y + 0.18) / 0.12)) * l.lamp;
            let candlezone = exp(-sq((p.x - CANDLE.x) / 0.1) - sq((p.y - CANDLE.y - 0.05) / 0.1)) * l.candle;
            let floorzone = exp(-sq((p.x - FLAMP.x) / 0.18) - sq((p.y - FLAMP.y + 0.2) / 0.2)) * l.lamp;
            col += (l.win_c * shaft * 2.0 + col_kelvin(2700.0) * (lampzone * 1.4 + floorzone) + col_kelvin(1900.0) * candlezone * 0.8) * mote
                   * max(smoothstep(DESK_BACK - 0.01, DESK_BACK + 0.03, p.y), step(DESK_R, abs(p.x)));
        }
    }
    return col * exp2(l.exposure);
}
