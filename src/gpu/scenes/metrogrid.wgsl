//! name: metrogrid
//! title: City Grid at Night
//! category: aerial
//! tags: drone, city, night, traffic, grid, top-down, aerial
//! desc: straight down on a city at night: dark blocks, lamp-dotted streets, a river, headlights
//! themes: night, rain, dusk
//! params: traffic, lights
//! uses: noise, light, game
//! cost: medium
//! fallback: traffic
//! credits: original

// A nadir drone shot drawn flat in p units. The grid repeats every CELL:
// streets on the lines, every third line a wider avenue. Blocks are dark
// rooftop slabs cut into 2-4 lots, one block in nine a park. Street lamps
// are small warm pools along the street centre, coloured per district;
// between them the asphalt stays black. A river crosses the frame
// diagonally; only the avenues bridge it. Cars run in closed form
// (game_wrap) along the streets, a signal at every intersection.

struct Look {
    asphalt: vec3f,
    roof: vec3f,
    park: vec3f,
    path: vec3f,
    water: vec3f,
    sodium: vec3f,
    cool: vec3f,
    lamp_r: f32,
    lamp_i: f32,   // peak pool radiance
    lit: f32,      // fraction of lamps switched on
    smear: f32,    // rain: vertical reflection smear of the lamp pools
    head: f32,
    tail: f32,
    haze: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            // rain: wet asphalt smears the lamps into vertical streaks, everything cooler and softer
            return Look(vec3f(0.004, 0.005, 0.008), vec3f(0.038, 0.042, 0.050), vec3f(0.010, 0.024, 0.014),
                        vec3f(0.030, 0.034, 0.034), vec3f(0.005, 0.007, 0.011),
                        col_hex(0xffc080u) * 0.8, col_hex(0xc8dcffu),
                        0.014, 0.4, 0.95, 0.5, 1.5, 0.8, 1.0);
        }
        case 2u: {
            // dusk: the sky still lights the rooftops blue-grey, only half the lamps are on
            return Look(vec3f(0.022, 0.026, 0.036), vec3f(0.115, 0.125, 0.155), vec3f(0.030, 0.065, 0.035),
                        vec3f(0.110, 0.115, 0.110), vec3f(0.075, 0.095, 0.135),
                        col_hex(0xffa23cu) * 0.9, col_hex(0xd6e6ffu) * 0.8,
                        0.010, 0.55, 0.5, 0.0, 1.5, 0.8, 0.0);
        }
        default: {
            // night: near-black asphalt, sodium orange pools
            return Look(col_hex(0x0b0c10u) * 0.6, vec3f(0.050, 0.051, 0.056), vec3f(0.010, 0.028, 0.012),
                        vec3f(0.035, 0.038, 0.030), vec3f(0.004, 0.007, 0.012),
                        col_hex(0xffa23cu), col_hex(0xd6e6ffu),
                        0.012, 0.7, 1.0, 0.0, 2.0, 1.0, 0.0);
        }
    }
}

const CELL: f32 = 0.20;
const SW: f32 = 0.015;          // street half width
const AW: f32 = 0.022;          // avenue half width (every third line)
const LAMP_SP: f32 = 0.05;      // lamp spacing along a street
const RW: f32 = 0.075;          // river half width
const PERIOD: f32 = 30.0;       // traffic-light cycle
const RIVER_DIR: vec2f = vec2f(0.8503, 0.5263);   // normalised (1, 0.62)

fn is_avenue(k: i32) -> bool { return fmod_pos(f32(k), 3.0) < 0.5; }
fn avenue_f(k: i32) -> f32 { return select(0.0, 1.0, is_avenue(k)); }
fn half_w(k: i32) -> f32 { return select(SW, AW, is_avenue(k)); }

// river in frame space: (signed distance across, coordinate along)
fn river(p: vec2f) -> vec2f {
    let n = vec2f(-RIVER_DIR.y, RIVER_DIR.x);
    let along = dot(p, RIVER_DIR);
    let across = dot(p, n);
    let c = 0.12 + 0.10 * sin(along * 2.2 + 1.0) + 0.03 * sin(along * 5.1 + 2.5);
    return vec2f(across - c, along);
}

// ------------------------------------------------------------- rooftops
fn rooftops(q: vec2f, cell: vec2i, l: Look, lights: f32, t: f32, ctx: Ctx) -> vec3f {
    let hb = hash_seeded2(cell, 0xB10Cu, ctx);
    let o = vec2f(cell) * CELL;
    let u = (q - o) / CELL;
    if (hb.x < 0.111) {
        // a park: dark green with a lighter diagonal path
        var col = l.park * (0.8 + 0.4 * noise_value2(q * 40.0));
        let dd = select(abs(u.x - u.y), abs(u.x + u.y - 1.0), hb.y < 0.5) * CELL * 0.7071;
        col = mix(col, l.path, aa_fill(dd - max(0.005, 0.6 * ctx.px), ctx));
        return col;
    }
    // 2-4 lots: split in x, in y, or both
    let nx = select(1.0, 2.0, hb.y < 0.7);
    let ny = select(1.0, 2.0, hb.y > 0.3);
    let nn = vec2f(nx, ny);
    let lot = floor(u * nn);
    let lu = u * nn - lot;
    let lot_size = CELL / nn;
    let hs = hash_cell2(cell * 4 + vec2i(lot), 0x5AB0u);
    var col = l.roof * mix(0.45, 1.0, hs.x);
    // a thin darker gap between the lots
    let da = min(min(lu.x, 1.0 - lu.x) * lot_size.x, min(lu.y, 1.0 - lu.y) * lot_size.y);
    col *= 1.0 - 0.5 * aa_fill(da - max(0.003, 0.5 * ctx.px), ctx);
    // one lot in six carries a lit feature
    if (hs.w < 0.1667 && lights > 0.05) {
        let fc = o + (lot + 0.3 + 0.4 * vec2f(hs.z, fract(hs.z * 7.3))) * lot_size;
        if (hs.y < 0.4) {
            // stairwell: a small warm rectangle
            let hr = vec2f(max(0.008, 0.6 * ctx.px), max(0.005, 0.5 * ctx.px));
            let d = max(abs(q.x - fc.x) - hr.x, abs(q.y - fc.y) - hr.y);
            col = mix(col, col_hex(0xffd699u) * 0.45, aa_fill(d, ctx));
        } else if (hs.y < 0.75) {
            // rooftop pool: a blue glow
            let d = length(q - fc);
            let r = max(0.010, 0.7 * ctx.px);
            col += col_hex(0x3a8cffu) * (0.5 * light_glow(d, r) * sstep(3.0 * r, 0.5 * r, d));
        } else {
            // aircraft warning light: tiny red, blinking at 0.5 Hz
            let d = length(q - fc);
            let r = max(0.004, 0.6 * ctx.px);
            let bl = game_blink(t + hs.z * 2.0, 0.5, 0.5);
            col += col_hex(0xff2020u) * (1.2 * bl * light_glow(d, r) * sstep(3.0 * r, 0.5 * r, d));
        }
    }
    return col;
}

// ---------------------------------------------------------------- lamps
// pools of the five nearest lamps on one grid line; `dir`/`k` squash the
// pool perpendicular to dir (rain smear, river reflection), rmul scales it
fn lamps_line(q: vec2f, horiz: bool, jl: i32, line_pos: f32, l: Look, lights: f32, t: f32,
              off: vec2f, dir: vec2f, k: f32, rmul: f32, ctx: Ctx) -> vec3f {
    let along = select(q.y, q.x, horiz);
    let axis = select(0, 1, horiz);
    let m0 = i32(floor(along / LAMP_SP));
    let r = max(l.lamp_r * rmul, 0.7 * ctx.px);
    let bridge = is_avenue(jl);
    var acc = vec3f(0.0);
    for (var i = 0; i < 5; i++) {
        let m = m0 - 2 + i;
        let am = (f32(m) + 0.5) * LAMP_SP;
        let lp = select(vec2f(line_pos, am), vec2f(am, line_pos), horiz);
        // lamps do not stand in the river unless the line is a bridge
        let rv = river(lp - off);
        if (abs(rv.x) < RW + 0.01 && !bridge) { continue; }
        let h = hash_cell2(vec2i(m, jl * 2 + axis), 0x1A3Bu);
        if (h.x >= l.lit * lights) { continue; }
        // district (3x3 blocks) decides sodium orange or cool white
        let dcell = vec2i(floor(lp / (CELL * 3.0)));
        let sodium = hash_seeded2(dcell, 0xD157u, ctx).x < 0.55;
        let lc = select(l.cool, l.sodium, sodium);
        let fl = light_flicker(t, bitcast<u32>(m * 131 + jl * 17 + 7919) & 0x3ffu, 0.5);
        var fo = q - lp;
        fo -= dir * (dot(fo, dir) * k);
        let d = length(fo);
        let g = light_glow(d, r) * sstep(3.0 * r, 0.6 * r, d);
        acc += lc * (l.lamp_i * min(lights, 1.5) * g * fl);
    }
    return acc;
}

// -------------------------------------------------------------- traffic
// 1-3 cars per segment and lane, moving along the street; two white dots
// ahead, two red behind
fn cars_line(q: vec2f, horiz: bool, jl: i32, line_pos: f32, hw: f32, l: Look, traffic: f32, t: f32,
             off: vec2f, ctx: Ctx) -> vec3f {
    let along = select(q.y, q.x, horiz);
    let across = select(q.x, q.y, horiz) - line_pos;
    let k0 = i32(floor(along / CELL));
    let r = max(0.004, 0.6 * ctx.px);
    let lane = hw * 0.45;
    let sep = max(0.0025, 0.35 * ctx.px);
    let bridge = is_avenue(jl);
    var acc = vec3f(0.0);
    for (var li = 0; li < 2; li++) {
        let s = select(-1.0, 1.0, li == 0);
        // right-hand traffic: eastbound on the south side, northbound on the east side
        let lane_across = select(s * lane, -s * lane, horiz);
        let dl = across - lane_across;
        if (abs(dl) > 4.0 * r) { continue; }
        for (var j = 0; j < 3; j++) {
            let k = k0 - 1 + j;
            let hseg = hash_cell2(vec2i(k, jl * 4 + li), 0xCA75u);
            let n = min(3, i32(floor(traffic * (1.0 + 2.0 * hseg.x) + 0.5)));
            let seg0 = f32(k) * CELL;
            for (var c = 0; c < 3; c++) {
                if (c >= n) { break; }
                let hc = hash_cell2(vec2i(k * 3 + c, jl * 4 + li), 0xC0FFu);
                let v = 0.025 + 0.02 * hc.x;
                let pos = seg0 + game_wrap(t, 0.0, CELL, s * v, hc.y * CELL);
                let lp = select(vec2f(line_pos, pos), vec2f(pos, line_pos), horiz);
                if (abs(river(lp - off).x) < RW + 0.01 && !bridge) { continue; }
                let da = along - pos;
                let fx = da - s * 0.006;
                let bx = da + s * 0.006;
                let dh = min(length(vec2f(fx, dl - sep)), length(vec2f(fx, dl + sep)));
                let dt = min(length(vec2f(bx, dl - sep)), length(vec2f(bx, dl + sep)));
                acc += vec3f(1.0, 0.97, 0.9) * (l.head * light_glow(dh, r) * sstep(3.0 * r, 0.6 * r, dh));
                acc += col_hex(0xff2a1cu) * (l.tail * light_glow(dt, r) * sstep(3.0 * r, 0.6 * r, dt));
            }
        }
    }
    return acc;
}

// traffic signals at the nearest intersection: one red, one green, 30 s cycle
fn signals(q: vec2f, jx: i32, jy: i32, xl: f32, yl: f32, wv: f32, wh: f32, t: f32, ctx: Ctx) -> vec3f {
    let ph = hash_cell2(vec2i(jx, jy), 0xC1C1u).x;
    let f = fract(t / PERIOD + ph);
    let red = col_hex(0xff2020u);
    let green = col_hex(0x22ff66u);
    let ch = select(red, green, f < 0.5);
    let cv = select(green, red, f < 0.5);
    let r = max(0.004, 0.55 * ctx.px);
    let p1 = vec2f(xl - wv - 0.006, yl - wh - 0.006);
    let p2 = vec2f(xl + wv + 0.006, yl + wh + 0.006);
    let d1 = length(q - p1);
    let d2 = length(q - p2);
    return ch * (1.2 * light_glow(d1, r) * sstep(3.0 * r, 0.6 * r, d1))
         + cv * (1.2 * light_glow(d2, r) * sstep(3.0 * r, 0.6 * r, d2));
}

// ----------------------------------------------------------------- scene
fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let traffic = ctx.params.x * 2.0;
    let lights = ctx.params.y * 2.0;

    // per-launch offset of the grid against the river
    let off = (ctx.seedf - 0.5) * CELL * 3.0;
    let q = p + off;

    // grid cell and the nearest lines
    let ix = i32(floor(q.x / CELL));
    let iy = i32(floor(q.y / CELL));
    let fx = q.x / CELL - f32(ix);
    let fy = q.y / CELL - f32(iy);
    let jx = select(ix, ix + 1, fx > 0.5);
    let jy = select(iy, iy + 1, fy > 0.5);
    let xl = f32(jx) * CELL;
    let yl = f32(jy) * CELL;
    let wv = half_w(jx);
    let wh = half_w(jy);
    let mv = aa_fill(abs(q.x - xl) - wv, ctx);
    let mh = aa_fill(abs(q.y - yl) - wh, ctx);
    let street = max(mv, mh);

    // the river lives in frame space so it always crosses the picture
    let rv = river(p);
    let water_cov = sstep(RW + 0.02, RW - 0.005, abs(rv.x));
    let bridge = max(mv * avenue_f(jx), mh * avenue_f(jy));
    let on_water = water_cov * (1.0 - bridge);

    // rooftops, then asphalt, then water
    var col = rooftops(q, vec2i(ix, iy), l, lights, t, ctx);
    let asph = l.asphalt * (0.8 + 0.4 * noise_value2(q * 30.0));
    col = mix(col, asph, street);
    let ripple = noise_fbm2(vec2f(rv.y * 10.0 - t * 0.03, rv.x * 30.0), 3);
    let water = l.water * (0.8 + 0.4 * ripple);
    col = mix(col, water, on_water);

    // lamp pools on the two nearest streets, a little spill onto the kerb-side roofs
    let up = vec2f(0.0, 1.0);
    let lamp = lamps_line(q, true, jy, yl, l, lights, t, off, up, l.smear, 1.0, ctx)
             + lamps_line(q, false, jx, xl, l, lights, t, off, up, l.smear, 1.0, ctx);
    col += lamp * mix(0.25, 1.0, street) * (1.0 - on_water);

    // reflections smeared along the river
    if (on_water > 0.001) {
        let refl = lamps_line(q, true, jy, yl, l, lights, t, off, RIVER_DIR, 0.6, 1.3, ctx)
                 + lamps_line(q, false, jx, xl, l, lights, t, off, RIVER_DIR, 0.6, 1.3, ctx);
        let streak = 0.6 + 0.8 * noise_value2(vec2f(rv.y * 30.0 + t * 0.05, rv.x * 120.0));
        col += refl * (0.3 * streak * on_water);
    }

    // cars on the two nearest streets, the signals at their crossing
    col += cars_line(q, true, jy, yl, wh, l, traffic, t, off, ctx);
    col += cars_line(q, false, jx, xl, wv, l, traffic, t, off, ctx);
    let xing = vec2f(xl, yl) - off;
    let xing_dry = select(1.0, avenue_f(jx) * avenue_f(jy), abs(river(xing).x) < RW + 0.01);
    col += signals(q, jx, jy, xl, yl, wv, wh, t, ctx) * xing_dry;

    // rain: a cool, soft lift over everything
    col = col * (1.0 - 0.2 * l.haze) + vec3f(0.004, 0.005, 0.008) * l.haze;
    return max(col, vec3f(0.0));
}
