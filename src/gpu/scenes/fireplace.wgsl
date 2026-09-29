//! name: fireplace
//! title: Cabin Fireplace
//! category: cozy
//! tags: fire, cabin, stone, snow, winter, warm, interior
//! desc: a crackling fire in a fieldstone hearth, snow falling past the cabin window
//! themes: snow, rain, autumn
//! uses: sdf, fire, snow, rain, noise
//! cost: light
//! fallback: campfire
//! credits: original

// A log cabin at night. The room is drawn in screen space but every surface
// carries a pseudo-3D position (x, y on screen, z toward the viewer) and a
// normal, lit by one point light: the fire. Its flicker is weighted by
// distance, so the hearth breathes while the far walls hold steady (a whole
// room flickering would repaint every terminal cell every frame).

const FIRE: vec2f = vec2f(0.12, -0.215);       // base of the flames
const OPEN_C: vec2f = vec2f(0.12, -0.16);      // firebox opening centre
const OPEN_H: vec2f = vec2f(0.19, 0.14);       // opening half size
const FLOOR_Y: f32 = -0.36;
const WIN_C: vec2f = vec2f(-0.6, 0.1);
const WIN_H: vec2f = vec2f(0.17, 0.2);

struct Look {
    mode: u32,        // 0 snow, 1 rain, 2 autumn
    fill: vec3f,      // cool/warm light from the window side
    amb: vec3f,       // room ambient
    sky_top: vec3f,
    sky_low: vec3f,
    land: vec3f,      // ground outside
    trees: vec3f,
    fire: f32,        // fire intensity
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, col_hex(0x6a7a96u) * 0.014, vec3f(0.008, 0.0085, 0.01),
                        col_hex(0x1a2230u) * 0.35, col_hex(0x3a4452u) * 0.3, col_hex(0x1c2420u) * 0.12,
                        col_hex(0x0c1210u) * 0.1, 1.0, 1.0);
        }
        case 2u: {
            return Look(2u, col_hex(0xffb070u) * 0.03, vec3f(0.012, 0.009, 0.007),
                        col_hex(0x5a6a98u) * 0.5, col_hex(0xffa060u) * 1.2, col_hex(0x6a5a30u) * 0.3,
                        col_hex(0x8a3a14u) * 0.35, 0.85, 0.9);
        }
        default: {
            return Look(0u, col_hex(0x6d8ac8u) * 0.035, vec3f(0.008, 0.009, 0.012),
                        col_hex(0x0c1630u) * 0.5, col_hex(0x2a3c64u) * 0.5, col_hex(0x9eb2d8u) * 0.28,
                        col_hex(0x08101cu) * 0.4, 1.0, 1.0);
        }
    }
}

// fire colour and slow breathing; `near` weights the flicker toward the hearth
fn hearth_c() -> vec3f { return vec3f(1.0, 0.58, 0.28); }

// Light at a pseudo-3D point with normal n. flick: fire_light(t) - 1.
fn hearth_lit(pos: vec3f, n: vec3f, l: Look, flick: f32) -> vec3f {
    // the fire's light spills out of the opening into the room
    let lp = vec3f(FIRE.x + 0.01, FIRE.y + 0.08, -0.22);
    let d = lp - pos;
    let dist2 = dot(d, d);
    let ld = d * inverseSqrt(dist2);
    let near = exp(-sqrt(dist2) * 3.0);
    let k = l.fire * (1.0 + flick * (0.25 + 0.75 * near));
    let direct = max(dot(n, ld), 0.0) * 0.05 / (dist2 + 0.03);
    // warm bounce off the floor and walls fills the room
    let bounce = 0.045 / (1.0 + dist2 * 3.0);
    return hearth_c() * k * (direct + bounce);
}

// ------------------------------------------------------------------ materials
// cellular stones: (F1, F2, cell id hash) at p
fn stones(p: vec2f) -> vec3f {
    let i = vec2i(floor(p));
    let f = fract(p);
    var d1 = 8.0;
    var d2 = 8.0;
    var id = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2i(x, y);
            let h = hash_cell2(i + o, 0x570eu);
            let r = vec2f(o) + 0.15 + 0.7 * h.xy - f;
            // stones are wider than tall
            let dd = length(r * vec2f(0.8, 1.25));
            if (dd < d1) { d2 = d1; d1 = dd; id = h.z; } else if (dd < d2) { d2 = dd; }
        }
    }
    return vec3f(d1, d2, id);
}

// fieldstone wall: (albedo rgb, height) — rounded stones in deep mortar
fn stone_wall(p: vec2f) -> vec4f {
    let s = stones(p * vec2f(9.0, 9.0));
    let edge = s.y - s.x;
    let h = sqrt(saturate(edge * 2.2));
    let tone = s.z;
    var alb = mix(col_hex(0x7a6e62u), col_hex(0x9a9286u), tone);
    alb = mix(alb, col_hex(0x6a5a48u), step(0.8, tone));
    alb = mix(alb, col_hex(0x8c8a84u), step(tone, 0.15));
    alb *= 0.75 + 0.35 * noise_value2(p * 60.0);
    let mortar = smoothstep(0.07, 0.02, edge);
    alb = mix(alb, col_hex(0x5a5248u) * 0.8, mortar);
    return vec4f(alb * 0.55, h);
}

// horizontal logs of the cabin wall: (albedo, log-local v, chinking)
fn log_wall(p: vec2f) -> vec3f {
    let lh = 0.085;
    let v = fract(p.y / lh);
    let idx = floor(p.y / lh);
    let hl = hash_f(u32(idx + 100.0));
    let chink = smoothstep(0.1, 0.04, v) + smoothstep(0.9, 0.96, v);
    return vec3f(v, chink, hl);
}

// ------------------------------------------------------------------ outside
fn outside(p: vec2f, l: Look, ctx: Ctx) -> vec3f {
    let q = p - WIN_C;
    var c = mix(l.sky_low, l.sky_top, smoothstep(-0.05, 0.2, q.y));
    if (l.mode == 0u) {
        // moonlit clearing: firs at two distances on bright snow
        var sc = c;
        let gnd = -0.1 + 0.012 * sin(p.x * 11.0);
        for (var layer = 0; layer < 2; layer++) {
            let fl = f32(layer);
            let w = 0.05 - fl * 0.02;
            let cx = (floor(p.x / w) + 0.5) * w;
            let h = hash_cell2(vec2i(i32(floor(p.x / w)), layer), 0xf12u);
            let tall = (0.09 + 0.08 * h.x) * (1.0 - fl * 0.45);
            let base = gnd + 0.005 + fl * 0.02;
            let yy = q.y - base;
            let half = w * (0.35 + 0.15 * h.y) * saturate(1.0 - yy / tall);
            let tier = 1.0 - 0.25 * fract(yy / tall * 4.0);
            if (yy > 0.0 && yy < tall && abs(p.x - cx - (h.z - 0.5) * w * 0.3) < half * tier && h.w < 0.8) {
                sc = mix(col_hex(0x0a1426u) * 0.3, l.sky_low * 0.6, fl * 0.5);
            }
        }
        if (q.y < gnd) { sc = l.land * (0.85 + 0.15 * noise_value2(p * 30.0)) * (0.8 + 0.4 * smoothstep(gnd - 0.1, gnd, q.y)); }
        let s = snow_flakes(p * 1.8, ctx, 0.5, 0.01, 3);
        return mix(sc, col_hex(0xdce6f8u) * 0.45, s * 0.9);
    }
    // distant tree line, snowy/wet/autumn ground
    let tl = -0.07 + 0.05 * noise_fbm2(vec2f(p.x * 14.0, 0.5), 3) + 0.04 * saturate(sin(p.x * 60.0) * 0.5 + 0.5) * select(1.0, 0.3, l.mode == 2u);
    let gnd = -0.12 + 0.01 * sin(p.x * 9.0);
    if (q.y < tl) { c = l.trees; }
    if (l.mode == 2u && q.y < tl + 0.03 && q.y > gnd) {
        // autumn crowns: patches of orange, rust and yellow
        let n = noise_fbm2(p * 40.0, 3);
        let crown = mix(col_hex(0xb4541cu), col_hex(0xd8a032u), n) * 0.35;
        c = mix(c, crown, smoothstep(0.45, 0.55, n + 0.1 * (tl - q.y) * 20.0) * step(q.y, tl + 0.02));
    }
    if (q.y < gnd) { c = l.land * (0.8 + 0.2 * noise_value2(p * 30.0)); }
    // weather
    if (l.mode == 0u) {
        let s = snow_flakes(p, ctx, 0.55, 0.02, 3);
        c = mix(c, col_hex(0xdce6f8u) * 0.5, s);
    } else if (l.mode == 2u) {
        // a few leaves drifting down
        let lq = vec2f(p.x * 12.0 + sin(ctx.t * 0.7 + p.y * 9.0) * 0.4, p.y * 12.0 + ctx.t * 0.9);
        let lc = vec2i(floor(lq));
        let lh = hash_cell2(lc, 0x1eafu);
        if (lh.w < 0.12) {
            let d = length((fract(lq) - 0.3 - 0.4 * lh.xy) * vec2f(1.0, 1.8));
            c = mix(c, mix(col_hex(0xc0501cu), col_hex(0xe0a030u), lh.z) * 0.5, smoothstep(0.12, 0.07, d));
        }
    }
    return c;
}

// ------------------------------------------------------------------ scene
fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let flick = (fire_light(t * 0.7) - 0.95) * 1.4;
    let aa = ctx.px;
    var col = vec3f(0.0);

    // ---- walls: log cabin wall everywhere, stone chimney breast in the middle
    let breast = p.x > -0.28 && p.x < 0.52 && p.y > FLOOR_Y;
    if (p.y > FLOOR_Y) {
        let lw = log_wall(p);
        let v = lw.x;
        // round log: normal bulges toward the viewer across its height
        let ny = (v - 0.5) * 1.6;
        let n = normalize(vec3f(0.0, ny, -1.0));
        let grain = 0.8 + 0.25 * noise_value2(vec2f(p.x * 18.0 + lw.z * 40.0, p.y * 90.0)) + 0.1 * sin(p.x * 70.0 + lw.z * 9.0);
        var alb = mix(col_hex(0x7a5634u), col_hex(0x9c7448u), lw.z) * 0.45 * grain;
        alb = mix(alb, col_hex(0xb0a080u) * 0.35, lw.y);
        let pos = vec3f(p, 0.02);
        col = alb * (hearth_lit(pos, n, l, flick) + l.amb * (1.0 + ny * 0.5) + l.fill * smoothstep(0.1, -0.7, p.x) * 0.6);
        // ceiling beam
        if (p.y > 0.43) {
            let bv = saturate((p.y - 0.43) / 0.08);
            let bn = normalize(vec3f(0.0, -1.0 + bv, -1.0));
            let ba = col_hex(0x3a2616u) * 0.4 * (0.85 + 0.2 * noise_value2(vec2f(p.x * 12.0, p.y * 60.0)));
            col = ba * (hearth_lit(vec3f(p, -0.05), bn, l, flick) * 0.8 + l.amb);
            if (p.y > 0.51) { col = col_hex(0x2a1c10u) * 0.3 * (hearth_lit(vec3f(p, 0.0), vec3f(0.0, -1.0, 0.0), l, flick) * 0.5 + l.amb); }
        }
    }
    if (breast && p.y < 0.43) {
        let sw = stone_wall(p);
        // dome normal from the stone height field
        let e = 0.004;
        let hx = stone_wall(p + vec2f(e, 0.0)).w - sw.w;
        let hy = stone_wall(p + vec2f(0.0, e)).w - sw.w;
        let n = normalize(vec3f(-hx * 1.6, -hy * 1.6, -e * 3.0));
        let pos = vec3f(p, -0.04 - 0.02 * sw.w);
        col = sw.rgb * (hearth_lit(pos, n, l, flick) + l.amb * 1.1 + l.fill * 0.2);
        // soot above the opening
        let soot = exp(-sq((p.x - OPEN_C.x) / 0.16)) * smoothstep(OPEN_C.y + OPEN_H.y - 0.02, OPEN_C.y + OPEN_H.y + 0.1, p.y) * smoothstep(0.06, 0.0, p.y);
        col *= 1.0 - 0.6 * soot;
        // the breast's side edges are a little darker (it stands proud of the wall)
        col *= 0.75 + 0.25 * smoothstep(0.0, 0.02, min(p.x + 0.28, 0.52 - p.x));
    }

    // ---- window with its view out
    let wq = p - WIN_C;
    let wd = sdf2_box(wq, WIN_H);
    if (wd < 0.035) {
        // plank trim around the window, lit a little by the fire
        let trim = col_hex(0x5a3c22u) * 0.45 * (hearth_lit(vec3f(p, -0.02), vec3f(0.3, 0.0, -1.0), l, flick) + l.amb + l.fill * 0.8);
        col = mix(col, trim, aa_fill(wd - 0.035, ctx));
    }
    if (wd < 0.0) {
        var o = outside(p, l, ctx);
        if (l.mode == 1u) {
            let rg = rain_glass(p * 1.4, t, ctx);
            o = outside(p + rg.xy * 0.6, l, ctx) * (1.0 + rg.z * 0.4);
        }
        // the glass holds a faint reflection of the fire
        let rf = exp(-sq((p.x - (WIN_C.x + 0.07)) / 0.035) - sq((p.y - (WIN_C.y - 0.12)) / 0.04));
        o += hearth_c() * rf * 0.05 * l.fire * (1.0 + flick * 0.5);
        // muntins: 2 x 3 panes
        let mx = abs(fract((wq.x + WIN_H.x) / (WIN_H.x * 2.0 / 2.0)) - 0.5);
        let my = abs(fract((wq.y + WIN_H.y) / (WIN_H.y * 2.0 / 3.0)) - 0.5);
        let mun = max(1.0 - smoothstep(0.018, 0.03, mx * WIN_H.x * 2.0 / 2.0 * 2.0 * 0.5), 1.0 - smoothstep(0.012, 0.02, my * WIN_H.y * 2.0 / 3.0 * 2.0 * 0.5));
        let munc = col_hex(0x4a3220u) * 0.4 * (l.amb * 2.0 + l.fill * 1.5 + hearth_lit(vec3f(p, -0.01), vec3f(0.3, 0.0, -1.0), l, flick) * 0.6);
        o = mix(o, munc, mun);
        // snow banked in the pane corners
        if (l.mode == 0u) {
            let py = fract((wq.y + WIN_H.y) / (WIN_H.y * 2.0 / 3.0));
            let px = fract((wq.x + WIN_H.x) / WIN_H.x);
            let bank = 0.03 + 0.05 * sq(abs(px - 0.5) * 2.0) + 0.012 * noise_value2(p * 90.0);
            let bottom_row = step(wq.y, -WIN_H.y + WIN_H.y * 2.0 / 3.0);
            o = mix(o, col_hex(0xd8e4f4u) * 0.35, smoothstep(0.02, -0.02, py - bank) * (1.0 - mun) * bottom_row);
        }
        col = mix(col, o, aa_fill(wd, ctx));
    }

    // ---- mantel beam and what stands on it
    let md = sdf2_box(p - vec2f(0.12, 0.048), vec2f(0.36, 0.028));
    if (md < aa) {
        let v = (p.y - 0.02) / 0.056;
        let n = normalize(vec3f(0.0, mix(-1.2, 0.3, smoothstep(0.0, 0.35, v)), -1.0));
        let ma = col_hex(0x4a2e18u) * 0.5 * (0.85 + 0.25 * noise_value2(vec2f(p.x * 14.0, p.y * 140.0)));
        let mc = ma * (hearth_lit(vec3f(p, -0.09), n, l, flick) + l.amb);
        col = mix(col, mc, aa_fill(md, ctx));
    }
    {
        // a lantern on the left, a stack of books right, a small clock
        let lant = min(sdf2_round_box(p - vec2f(-0.14, 0.115), vec2f(0.022, 0.04), 0.008), sdf2_box(p - vec2f(-0.14, 0.165), vec2f(0.01, 0.01)));
        let books = min(sdf2_box(p - vec2f(0.33, 0.1), vec2f(0.05, 0.026)), sdf2_box(p - vec2f(0.34, 0.135), vec2f(0.04, 0.009)));
        let clock = sdf2_circle(p - vec2f(0.12, 0.12), 0.04);
        let obj = min(min(lant, books), clock);
        if (obj < aa) {
            var oc = col_hex(0x2a2018u) * 0.3;
            if (books < aa) { oc = mix(col_hex(0x5a1e18u), col_hex(0x1e3a2au), step(0.126, p.y)) * 0.35; }
            if (clock < aa) { oc = mix(col_hex(0x6a4a24u) * 0.4, col_hex(0xd8ccb0u) * 0.4, smoothstep(0.0, -0.01, clock + 0.01)); }
            let bottom = smoothstep(0.16, 0.08, p.y);
            let lit = oc * (hearth_lit(vec3f(p, -0.1), normalize(vec3f(0.0, -0.4, -1.0)), l, flick) * (0.4 + 0.6 * bottom) + l.amb);
            col = mix(col, lit, aa_fill(obj, ctx));
        }
    }

    // ---- the firebox opening
    let oq = p - OPEN_C;
    let arch = OPEN_H.y - 0.03 * (1.0 - sq(oq.x / OPEN_H.x));
    let od = max(abs(oq.x) - OPEN_H.x, max(-oq.y - OPEN_H.y, oq.y - arch));
    if (od < aa * 2.0) {
        // sooty back wall and splayed sides, glowing where the fire licks them
        let depth = saturate(1.0 - abs(oq.x) / OPEN_H.x);
        let sw = stone_wall(p * 1.6 + 3.0);
        var inner = sw.rgb * 0.35;
        let heat = exp(-sq((p.x - FIRE.x) / 0.12) - sq((p.y - FIRE.y - 0.05) / 0.1));
        inner *= 0.2 + (hearth_c() * l.fire * (1.0 + flick * 0.8)) * (0.6 * heat + 0.15 * depth) * 3.0;
        inner *= smoothstep(OPEN_C.y + arch + 0.02, OPEN_C.y - 0.02, p.y) * 0.8 + 0.2;
        col = mix(col, inner, aa_fill(od, ctx));
        // coal bed
        let bed = sdf2_round_box(p - vec2f(FIRE.x, FIRE.y - 0.07), vec2f(0.15, 0.018), 0.012);
        if (bed < aa) {
            let n = noise_value2(vec2f(p.x * 90.0, p.y * 160.0) + vec2f(0.0, floor(t * 0.5) * 0.0));
            let pulse = 0.75 + 0.25 * sin(t * 0.9 + n * 12.0);
            let glow = fire_temperature_color(0.25 + 0.25 * n * pulse) * l.fire * 0.8;
            col = mix(col, glow, aa_fill(bed, ctx));
        }
        // andirons
        let andi = min(sdf2_box(p - vec2f(FIRE.x - 0.13, FIRE.y - 0.03), vec2f(0.007, 0.05)), sdf2_box(p - vec2f(FIRE.x + 0.13, FIRE.y - 0.03), vec2f(0.007, 0.05)));
        col = mix(col, vec3f(0.004) + hearth_c() * 0.02, aa_fill(andi, ctx));
        // three logs: charred, glowing in their cracks
        for (var k = 0; k < 3; k++) {
            var a: vec2f;
            var b: vec2f;
            var r: f32;
            if (k == 0) { a = vec2f(0.0, -0.262); b = vec2f(0.24, -0.25); r = 0.026; }
            else if (k == 1) { a = vec2f(0.02, -0.21); b = vec2f(0.16, -0.25); r = 0.02; }
            else { a = vec2f(0.1, -0.245); b = vec2f(0.25, -0.2); r = 0.021; }
            let d = sdf2_segment(p - vec2f(FIRE.x - 0.12, 0.0), a, b) - r;
            if (d < aa) {
                let ba = b - a;
                let along = dot(p - vec2f(FIRE.x - 0.12, 0.0) - a, ba) / dot(ba, ba);
                let across = saturate(0.5 - d / (2.0 * r));
                let crack = smoothstep(0.7, 0.86, noise_value2(vec2f(along * 30.0 + f32(k) * 7.0, across * 4.0)));
                let pulse = 0.7 + 0.3 * sin(t * 0.8 + along * 9.0 + f32(k));
                var lc = col_hex(0x1a120cu) * 0.08 * (0.4 + across);
                lc += fire_temperature_color(0.28 + 0.12 * pulse) * crack * (0.3 + 0.7 * across) * l.fire * 0.8;
                // flames light the logs' upper sides; bark texture
                let bark = 0.7 + 0.3 * noise_value2(vec2f(along * 60.0, across * 12.0 + f32(k)));
                lc += hearth_c() * 0.05 * l.fire * smoothstep(0.35, 0.95, across) * bark;
                // sawn ends show pale wood rings where they face us
                let endd = length(p - vec2f(FIRE.x - 0.12, 0.0) - a) - r * 0.85;
                if (k == 0 && endd < 0.0) {
                    lc = col_hex(0x8a6a44u) * 0.25 * (0.8 + 0.2 * sin(length(p - vec2f(FIRE.x - 0.12, 0.0) - a) * 700.0)) * (0.3 + hearth_c() * 0.5 * l.fire);
                }
                col = mix(col, lc, aa_fill(d, ctx));
            }
        }
        // flames: a few overlapping tongues
        if (od < 0.0 || p.y > OPEN_C.y + arch - 0.02) {
            var fsum = vec3f(0.0);
            var asum = 0.0;
            for (var k = 0; k < 4; k++) {
                var off: vec2f;
                var sc: f32;
                if (k == 0) { off = vec2f(0.0, 0.0); sc = 0.36; }
                else if (k == 1) { off = vec2f(-0.07, -0.005); sc = 0.27; }
                else if (k == 2) { off = vec2f(0.075, -0.004); sc = 0.29; }
                else { off = vec2f(-0.11, -0.012); sc = 0.16; }
                let fq = (p - FIRE - off) / sc;
                if (abs(fq.x) < 0.3 && fq.y > -0.05 && fq.y < 0.75) {
                    let f = fire_flame(fq, t * 0.8 + f32(k) * 3.7, ctx);
                    fsum += f.rgb * f.a;
                    asum = max(asum, f.a);
                }
            }
            let inside = aa_fill(od, ctx);
            col = mix(col, col * 0.4 + fsum * l.fire, asum * inside) + fsum * l.fire * 0.15 * inside;
        }
    }

    // ---- hearth slab
    let hd = sdf2_box(p - vec2f(0.12, FLOOR_Y + 0.03), vec2f(0.42, 0.032));
    if (hd < aa) {
        let top = smoothstep(FLOOR_Y + 0.045, FLOOR_Y + 0.06, p.y);
        let n = normalize(vec3f(0.0, mix(-0.2, 1.0, top), -1.0));
        let ha = col_hex(0x6e665cu) * 0.5 * (0.8 + 0.3 * noise_value2(p * vec2f(20.0, 80.0))) * (0.85 + 0.15 * step(0.5, fract((p.x + 0.3) / 0.14)));
        let hc = ha * (hearth_lit(vec3f(p.x, FLOOR_Y + 0.05, -0.12 + (p.y - FLOOR_Y) * 0.5), n, l, flick) + l.amb);
        col = mix(col, hc, aa_fill(hd, ctx));
    }

    // ---- firewood stacked left of the hearth: log ends
    let wood_box = sdf2_box(p - vec2f(-0.38, -0.25), vec2f(0.085, 0.11));
    if (wood_box < 0.02 && p.y > FLOOR_Y) {
        let r = 0.024;
        let row = floor((p.y - FLOOR_Y) / (r * 1.75));
        let offx = select(0.0, r, (i32(row) & 1) == 1);
        let cx = floor((p.x + 0.5 - offx) / (r * 2.05));
        let c = vec2f((cx + 0.5) * r * 2.05 - 0.5 + offx, FLOOR_Y + (row + 0.5) * r * 1.75);
        let hw = hash_cell2(vec2i(i32(cx), i32(row)), 0xf00du);
        let rr = r * (0.82 + 0.18 * hw.x);
        let d = length(p - c) - rr;
        let inbox = sdf2_round_box(p - vec2f(-0.38, -0.25), vec2f(0.085, 0.11), 0.03) < 0.0;
        if (d < aa && inbox) {
            let rings = 0.85 + 0.15 * sin(length(p - c) * 420.0 + hw.y * 6.0);
            let bark = smoothstep(-0.005, 0.0, d + 0.002);
            var wc = mix(col_hex(0xb08858u) * rings, col_hex(0x3a2818u), bark) * 0.45;
            wc *= hearth_lit(vec3f(p, -0.1), vec3f(0.3, 0.1, -1.0), l, flick) + l.amb;
            col = mix(col, wc, aa_fill(d, ctx));
        } else if (inbox) {
            col *= 0.3;
        }
    }

    // ---- floor: wide planks receding to a vanishing point, a braided rug
    if (p.y < FLOOR_Y) {
        let vp = vec2f(0.1, 0.05);
        let z = 0.12 / max(vp.y - p.y, 0.01);
        let wx = (p.x - vp.x) * z;
        let plank = fract(wx / 0.2);
        let pid = floor(wx / 0.2);
        let hp = hash_f(u32(pid + 500.0));
        let seam = smoothstep(0.02, 0.0, min(plank, 1.0 - plank)) * saturate(1.0 - aa * z * 30.0);
        var fa = mix(col_hex(0x7a4e2cu), col_hex(0x9a6a3cu), hp) * 0.45 * (0.85 + 0.2 * noise_value2(vec2f(wx * 3.0, z * 0.6)));
        fa *= 1.0 - 0.5 * seam;
        let fpos = vec3f(p.x, FLOOR_Y, -(z - 0.3) * 0.25);
        var fc = fa * (hearth_lit(fpos, vec3f(0.0, 1.0, 0.0), l, flick) * 1.4 + l.amb + l.fill * 0.5 * smoothstep(0.0, -0.8, p.x));
        // glossy floor catches the firelight
        fc += hearth_c() * 0.02 * l.fire * exp(-sq((p.x - FIRE.x) / 0.12)) * smoothstep(FLOOR_Y - 0.12, FLOOR_Y, p.y) * (1.0 + flick * 0.5);
        // the rug: concentric braided rings
        let rq = vec2f(wx - 0.02, (z - 0.55) * 0.9);
        let rd = length(rq / vec2f(0.95, 0.42));
        if (rd < 1.0) {
            let ring = fract(rd * 7.0);
            let band = u32(floor(rd * 7.0));
            var rc = col_hex(0x7a3a2au);
            if (band % 4u == 1u) { rc = col_hex(0x9a8058u); }
            if (band % 4u == 2u) { rc = col_hex(0x3a4450u); }
            if (band % 4u == 3u) { rc = col_hex(0xb8ac90u); }
            let braid = 0.85 + 0.15 * sin(atan2(rq.y, rq.x) * 60.0 + ring * 6.0) * saturate(1.0 - aa * z * 20.0);
            let rugc = mix(rc, vec3f(col_luma(rc)), 0.25) * 0.4 * braid * (hearth_lit(fpos, vec3f(0.0, 1.0, 0.0), l, flick) * 1.4 + l.amb);
            fc = mix(fc, rugc, smoothstep(1.0, 0.97, rd));
        }
        col = fc;
    }

    // ---- leather armchair, right foreground, rim-lit by the fire
    let ch = min(sdf2_round_box(p - vec2f(0.78, -0.2), vec2f(0.2, 0.3), 0.12), sdf2_round_box(p - vec2f(0.6, -0.36), vec2f(0.07, 0.16), 0.06));
    if (ch < aa * 2.0) {
        let e = 0.006;
        let rim = smoothstep(-0.03, 0.0, ch) * smoothstep(0.5, 0.2, p.x - 0.4) ;
        let leather = col_hex(0x6a3418u) * 0.9 * (0.8 + 0.2 * noise_value2(p * 40.0));
        // cylindrical form across the back, a seam where back meets arm
        let across = clamp((p.x - 0.78) / 0.2, -1.0, 1.0);
        let up = clamp((p.y + 0.2) / 0.3, -1.0, 1.0);
        let cn = normalize(vec3f(across * 0.9, 0.15 + 0.5 * smoothstep(0.6, 1.0, up), -0.6));
        var cc = leather * (l.amb * 1.5 + hearth_lit(vec3f(p, -0.35), cn, l, flick) * 1.2);
        cc *= 1.0 - 0.4 * exp(-sq((p.x - 0.665) / 0.01)) * step(p.y, -0.2);
        cc += hearth_c() * l.fire * 0.08 * rim * (1.0 + flick * 0.6);
        // the fire catches the curve of the top
        cc += hearth_c() * l.fire * 0.05 * smoothstep(-0.025, 0.0, ch) * smoothstep(0.0, 0.08, p.y) * smoothstep(0.95, 0.65, p.x);
        // a sheen along the arm roll
        cc += hearth_c() * 0.03 * l.fire * exp(-sq((p.y + 0.21) / 0.012)) * step(p.x, 0.66);
        col = mix(col, cc, aa_fill(ch, ctx));
    }

    // ---- glow in the air around the fire, embers rising into the flue
    let gl = exp(-sq((p.x - FIRE.x) / 0.18) - sq((p.y - FIRE.y - 0.05) / 0.14));
    col += hearth_c() * gl * 0.06 * l.fire * (1.0 + flick * 0.5);
    if (od < 0.0) {
        col += fire_embers((p - FIRE) * 2.2, t, 0.1, ctx) * 0.3 * l.fire * smoothstep(FIRE.y + 0.02, FIRE.y + 0.1, p.y);
    }
    return col * exp2(l.exposure);
}
