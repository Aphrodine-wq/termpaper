//! name: freeway
//! title: LA Freeway Timelapse
//! category: city
//! tags: los-angeles, freeway, light-trails, long-exposure, skyline, palms
//! desc: long-exposure light trails on a curving LA freeway, downtown towers glowing in the haze
//! themes: dusk, night, rain
//! uses: camera, sdf, light, rain, noise
//! cost: light
//! fallback: traffic
//! credits: original

// World units are metres; y up, the camera looks toward -z. We stand on an
// overpass 13 m above the median with a long lens, looking north up a ten-lane
// freeway that swings right and then runs straight at downtown Los Angeles,
// 3.5 km away, with the San Gabriel Mountains behind it. Everything is
// analytic: the road is the plane y = 0, each lane's lamps are thin lines on
// the planes y = h (their wet reflections on y = -h), the roadside is a few
// vertical "curtains" that follow the curve (sound wall, trees, buildings),
// and gantries, palms and towers are flat drawings on vertical planes.

struct Look {
    mode: u32,        // 0 dusk, 1 night, 2 rain
    hor: vec3f,       // sky at the horizon (also the haze colour)
    low: vec3f,       // sky a few degrees up
    top: vec3f,       // sky at the top of the frame
    warm: vec3f,      // afterglow toward the set sun (west, frame left)
    amb: vec3f,       // ambient light on surfaces
    haze: f32,        // aerial perspective density (per metre)
    wet: f32,         // road wetness 0..1
    trail: f32,       // light-trail brightness
    city: f32,        // city light level
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            // night: navy sky over a grey-orange light-pollution dome
            l = Look(1u, col_hex(0x544a55u) * 0.42, col_hex(0x272b40u) * 0.3, col_hex(0x0b1124u) * 0.25,
                     col_hex(0x3a2c2au) * 0.0, vec3f(0.005, 0.0055, 0.008), 0.00030, 0.0, 1.2, 1.0, 0.2);
        }
        case 2u: {
            // rain: low overcast lit orange from below by the city, soaked road
            l = Look(2u, col_hex(0x6f6360u) * 0.42, col_hex(0x4a4650u) * 0.3, col_hex(0x25252fu) * 0.22,
                     col_hex(0x5a4438u) * 0.0, vec3f(0.008, 0.008, 0.011), 0.00085, 1.0, 1.05, 0.8, 0.35);
        }
        default: {
            // dusk: sun just down, peach smog band under a lavender-blue sky
            l = Look(0u, col_hex(0xeaa585u) * 0.95, col_hex(0xb78c9fu) * 0.55, col_hex(0x4a6598u) * 0.3,
                     col_hex(0xff9150u) * 0.5, vec3f(0.034, 0.03, 0.04), 0.00040, 0.0, 1.0, 0.5, 0.0);
        }
    }
    return l;
}

// ------------------------------------------------------------------ road
const CAM_H: f32 = 17.0;
const LANE_W: f32 = 3.7;
const LANE0: f32 = 1.7;          // inner edge of the first lane from the median
const EDGE: f32 = 20.2;          // outer edge of the fifth lane
const WALL_U: f32 = 23.6;        // sound wall line
const WALL_H: f32 = 4.2;

// centreline x at distance s (= -z): swings right, then straight at downtown
fn road_cx(s: f32) -> f32 {
    return 34.0 * sin(s * 0.0046) * exp(-s * 0.001) + 0.055 * s;
}
fn road_slope(s: f32) -> f32 { return (road_cx(s + 2.0) - road_cx(s - 2.0)) * 0.25; }
// (s, u): distance along the road, signed lateral offset (+ = right)
fn road_su(x: f32, z: f32) -> vec2f {
    let s = -z;
    let k = road_slope(s);
    return vec2f(s, (x - road_cx(s)) * inverseSqrt(1.0 + k * k));
}
fn road_u(ro: vec3f, rd: vec3f, t: f32) -> f32 {
    let p = ro + rd * t;
    return road_su(p.x, p.z).y;
}
// distance along the ray at which |u| first reaches `uu`, between ta (inside)
// and tb (outside)
fn cross_u(ro: vec3f, rd: vec3f, ta0: f32, tb0: f32, uu: f32) -> f32 {
    var ta = ta0;
    var tb = tb0;
    for (var i = 0; i < 12; i++) {
        let tm = 0.5 * (ta + tb);
        if (abs(road_u(ro, rd, tm)) < uu) { ta = tm; } else { tb = tm; }
    }
    return tb;
}

// Long-exposure streaks of the cars in one lane, at road position xi in the
// lane's moving frame. Each car in a 26 m cell leaves a streak `len` long.
fn fw_streaks(xi: f32, salt: u32, dens: f32, len: f32) -> f32 {
    let cell = 26.0;
    let k1 = floor(xi / cell);
    var acc = 0.0;
    for (var k = 0; k < 4; k++) {
        let c = k1 - f32(k);
        let h = hash_cell2(vec2i(i32(c), 0), hash_u(salt * 0x9e3779b9u + 0xf1eeu));
        if (h.x < dens) {
            let d = xi - (c + h.y) * cell;
            acc += smoothstep(0.0, 4.0, d) * smoothstep(len, len - 14.0, d) * (0.6 + 0.8 * h.z);
        }
    }
    return min(acc, 1.8);
}

// Light trails of one carriageway seen at road point (s, u).
// side = -1 oncoming (headlights, toward the camera), +1 outbound (tail lights).
// fp = pixel footprint across the road (m); fs = footprint along it.
// Returns (thin lamp lines, broad glow).
fn fw_trails(s: f32, u: f32, side: f32, fp: f32, fs: f32, t: f32, glow_w: f32) -> vec2f {
    var line = 0.0;
    var glow = 0.0;
    let au = u * side;
    if (au < -2.0 * (fp + glow_w)) { return vec2f(0.0); }
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let uc = LANE0 + LANE_W * (fi + 0.5);
        // outbound (toward downtown) is rush hour: slow and dense
        // timelapse: traffic runs ~2x real speed, 1.6 s effective exposure
        var v = (24.0 + fi * 2.5) * 2.0;
        var dens = 0.78 + 0.04 * fi;
        var len = v * 1.6;
        if (side > 0.0) { v = (9.0 + fi * 1.4) * 2.0; dens = 0.9; len = v * 2.4; }
        let xi = select(s + v * t, v * t - s, side > 0.0);
        var b = fw_streaks(xi, u32(i) + select(0u, 16u, side > 0.0), dens, len);
        let mean = min(dens * len / 26.0, 1.4);
        b = mix(b, mean, saturate(fs / 80.0));
        // a car's two lamps trace two lines 1.5 m apart; lane changes wander
        let wob = 0.35 * sin(s * 0.013 + fi * 2.1);
        let dd = abs(abs(au - uc - wob) - 0.75);
        let sig = max(0.06, fp * 0.5);
        line += b * (0.06 / sig) * exp(-0.5 * sq(dd / sig));
        glow += b * exp(-0.5 * sq((au - uc) / (glow_w + fp)));
    }
    return vec2f(line, glow);
}

fn headlight_col() -> vec3f { return col_kelvin(4300.0); }
fn taillight_col() -> vec3f { return vec3f(1.0, 0.05, 0.014); }

// ------------------------------------------------------------------ sky
fn fw_sky(rd: vec3f, l: Look) -> vec3f {
    let e = rd.y;
    var c = mix(l.hor, l.low, smoothstep(0.0, 0.055, e));
    c = mix(c, l.top, smoothstep(0.05, 0.24, e));
    // the west (left) keeps the afterglow
    let west = saturate(-rd.x * 1.3 + 0.2);
    c += l.warm * west * west * exp(-max(e, 0.0) * 16.0);
    // long high cloud bands: lit peach at dusk, dark against the glow otherwise
    if (e > 0.0) {
        let q = vec2f(rd.x / (e + 0.1), e * 9.0);
        let n = noise_fbm2(vec2f(q.x * 1.3, q.y * 5.0) + vec2f(4.0, 1.0), 5);
        let band = smoothstep(0.55, 0.78, n) * smoothstep(0.03, 0.08, e);
        if (l.mode == 0u) {
            c = mix(c, col_hex(0xeea17fu) * 0.75 * (0.5 + west), band * 0.5);
        } else {
            c = mix(c, c * 1.25 + vec3f(0.004, 0.003, 0.002), band * 0.5);
        }
    }
    return c;
}

// San Gabriel Mountains, ~35 km out
fn mountain_h(x: f32) -> f32 {
    let q = x / 2600.0;
    return 900.0 + 700.0 * noise_fbm2(vec2f(q + 3.1, 0.5), 5) + 420.0 * (noise_ridged2(vec2f(q * 1.7, 2.3), 4) - 0.5);
}

// Downtown towers on a plane 3.5 km out: (x centre, half width, roof, style).
// Styles: 0 box, 1 cylinder with glass crown, 2 sail top + spire, 3 bevel,
// 4 twin dark slab.
fn tower(i: i32) -> vec4f {
    switch (i) {
        case 0: { return vec4f(40.0, 30.0, 290.0, 2.0); }
        case 1: { return vec4f(128.0, 27.0, 296.0, 1.0); }
        case 2: { return vec4f(-52.0, 31.0, 262.0, 0.0); }
        case 3: { return vec4f(236.0, 25.0, 228.0, 3.0); }
        case 4: { return vec4f(182.0, 23.0, 226.0, 3.0); }
        case 5: { return vec4f(-128.0, 25.0, 221.0, 0.0); }
        case 6: { return vec4f(300.0, 27.0, 222.0, 3.0); }
        case 7: { return vec4f(-214.0, 21.0, 213.0, 4.0); }
        case 8: { return vec4f(-176.0, 21.0, 213.0, 4.0); }
        case 9: { return vec4f(88.0, 24.0, 180.0, 0.0); }
        case 10: { return vec4f(-6.0, 28.0, 158.0, 0.0); }
        case 11: { return vec4f(-296.0, 32.0, 140.0, 0.0); }
        case 12: { return vec4f(362.0, 30.0, 150.0, 0.0); }
        case 13: { return vec4f(-92.0, 36.0, 120.0, 0.0); }
        case 14: { return vec4f(-384.0, 44.0, 96.0, 0.0); }
        case 15: { return vec4f(426.0, 40.0, 92.0, 0.0); }
        default: { return vec4f(0.0); }
    }
}

fn tower_d(q: vec2f, tw: vec4f) -> f32 {
    let w = tw.y;
    let h = tw.z;
    var d = sdf2_box(q - vec2f(0.0, h * 0.5), vec2f(w, h * 0.5));
    let style = u32(tw.w);
    if (style == 1u) {
        d = min(d, sdf2_box(q - vec2f(0.0, h + 8.0), vec2f(w * 0.72, 8.0)));
        d = min(d, sdf2_box(q - vec2f(0.0, h + 18.0), vec2f(w * 0.45, 4.0)));
    } else if (style == 2u) {
        let rise = saturate((q.x + w) / (2.0 * w));
        let roof = h + 24.0 * rise * rise;
        d = max(sdf2_box(q - vec2f(0.0, (h + 24.0) * 0.5), vec2f(w, (h + 24.0) * 0.5)), q.y - roof);
        d = min(d, sdf2_box(q - vec2f(w * 0.72, h + 36.0), vec2f(1.3, 14.0)));
    } else if (style == 3u) {
        d = max(d, (q.y - h) + abs(q.x) * 0.9 - w * 0.25);
    }
    return d;
}

fn tower_top(tw: vec4f) -> f32 {
    let style = u32(tw.w);
    if (style == 1u) { return tw.z + 22.0; }
    if (style == 2u) { return tw.z + 50.0; }
    return tw.z;
}

// Distant layers: sky, mountains, downtown.
fn fw_far(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx, zoom: f32) -> vec3f {
    var c = fw_sky(rd, l);
    // mountains, drowned in haze toward their feet
    let tm = (-35000.0 - ro.z) / rd.z;
    let mp = ro + rd * tm;
    let mcov = aa_fill_w(mp.y - mountain_h(mp.x), ctx.px * tm / zoom);
    var mcol = mix(l.hor, l.top, 0.35) * 0.85;
    if (l.mode == 0u) { mcol = col_hex(0x9b7c90u) * 0.5; }
    mcol = mix(mcol, l.hor, exp(-max(mp.y, 0.0) / 700.0) * 0.8);
    c = mix(c, mcol, mcov * select(0.9, 0.0, l.mode == 2u));
    // downtown
    let tt = (-3500.0 - ro.z) / rd.z;
    let wp = ro + rd * tt;
    let aa = ctx.px * tt / zoom;
    let lx = wp.x - (road_cx(3500.0) - 30.0);
    if (abs(lx) < 560.0 && wp.y < 360.0) {
        var cov = 0.0;
        var face = vec3f(0.0);
        var em = vec3f(0.0);
        let glass = mix(l.low, l.top, 0.6) * 0.1 + vec3f(0.002, 0.0025, 0.004);
        // low- and mid-rise mass the towers stand in
        let cb = floor(lx / 34.0);
        let hb = hash_cell2(vec2i(i32(cb), 1), 0xd0du);
        let bh = (28.0 + 62.0 * hb.x) * smoothstep(560.0, 300.0, abs(lx));
        let ab = aa_fill_w(wp.y - bh, aa);
        if (ab > 0.0) {
            let occ = noise_value2(vec2f(lx / 12.0, wp.y / 9.0) + hb.yz * 30.0);
            face = glass * 0.6 + col_kelvin(3000.0 + 2000.0 * hb.z) * smoothstep(0.4, 0.9, occ) * 0.3 * l.city;
            cov = ab;
        }
        for (var i = 0; i < 16; i++) {
            let tw = tower(i);
            let q = vec2f(lx - tw.x, wp.y);
            if (abs(q.x) > tw.y + 12.0 || q.y > tw.z + 60.0) { continue; }
            let a = aa_fill_w(tower_d(q, tw), aa);
            if (a <= 0.0) { continue; }
            let hi = hash_cell2(vec2i(i, 3), 0x70eu);
            // lit office floors: each floor on or off, each tower its own
            // occupancy; sub-pixel floors average to a steady glow
            let fl = floor(wp.y / 4.2);
            let fh = hash_f(u32(fl) * 97u + u32(i) * 7919u);
            let occ = (0.25 + 0.6 * hi.x) * select(0.5, 1.0, l.mode == 0u);
            let floor_on = select(0.12, 1.0, fh < occ);
            let fine = saturate(1.6 - aa / 4.2);
            var lit = mix(occ * 0.9 + 0.1, floor_on, fine) * (0.85 + 0.15 * noise_value2(vec2f(q.x / 20.0, wp.y / 30.0) + hi.yz * 9.0));
            lit *= mix(1.0, step(0.18, fract((q.x + tw.y) / 4.5)), fine * 0.4);
            if (u32(tw.w) == 4u) { lit *= 0.35; }
            var e = col_kelvin(mix(3100.0, 5600.0, hi.z)) * lit * 0.32 * l.city;
            if (u32(tw.w) == 1u && q.y > tw.z - 3.0) { e += col_kelvin(6000.0) * 1.1 * l.city; }
            if (u32(tw.w) == 2u && q.y > tw.z - 12.0) { e += col_hex(0xa9c8ffu) * 0.9 * l.city; }
            // glass holds a little sky, brighter toward the top; the west
            // edge catches the afterglow
            var body = glass * (0.7 + 0.6 * saturate(q.y / tw.z));
            body += l.warm * 0.06 * smoothstep(-tw.y * 0.4, -tw.y, q.x);
            face = mix(face, body + e, a);
            cov = max(cov, a);
            if (i < 4) {
                let r = max(aa * 0.7, 1.5);
                let al = length(vec2f(q.x, q.y - tower_top(tw) - 1.0));
                em += vec3f(1.0, 0.06, 0.02) * 2.5 * exp(-sq(al / r)) * sq(1.5 / r);
            }
        }
        var fogk = 1.0 - exp(-tt * l.haze * 0.5);
        if (l.mode == 2u) { fogk = mix(fogk, 1.0, smoothstep(150.0, 300.0, wp.y) * 0.85); }
        c = mix(c, mix(face, l.hor * 0.9, fogk * 0.7), cov);
        c += em * (1.0 - fogk * 0.6);
    }
    return c;
}

// ------------------------------------------------------------------ roadside
// the flats far beyond the freeway: dark roofs and trees, scattered lamps
fn fw_flats(x: f32, z: f32, fp: f32, l: Look) -> vec3f {
    let q = vec2f(x, z);
    var c = vec3f(0.012, 0.013, 0.014) * (0.6 + 0.8 * noise_value2(q * 0.02)) + l.amb * 0.25;
    let cell = vec2i(floor(q / 18.0));
    let h = hash_cell2(cell, 0xc17u);
    let near_k = saturate(1.0 - fp / 40.0);
    if (h.w < 0.35) {
        let d = length(fmod_pos2(q, 18.0) - 9.0 - (h.xy - 0.5) * 12.0);
        let rr = max(1.2, fp * 0.6);
        let kel = select(2200.0 + 3000.0 * h.z, 3000.0 + 1500.0 * h.z, h.z > 0.5);
        c += col_kelvin(kel) * l.city * 1.2 * exp(-sq(d / rr)) * sq(1.2 / rr) * near_k;
    }
    c += col_kelvin(2600.0) * l.city * 0.018 * (1.0 - near_k);
    return c;
}

// canopy / roofline height of roadside curtain `layer` at distance s
fn curtain_h(s: f32, layer: i32, side: f32) -> vec2f {
    let salt = u32(layer) * 2u + select(0u, 1u, side > 0.0);
    if (layer == 0) {
        // street trees: a lumpy canopy mass 5-15 m with the odd gap
        let o = f32(salt) * 17.0;
        let big = noise_fbm2(vec2f(s / 60.0 + o, 0.5), 3);
        let lump = noise_value2(vec2f(s / 7.0 + o, 1.5));
        let fine = noise_value2(vec2f(s / 2.2 + o, 2.5));
        let h = 4.0 + 11.0 * smoothstep(0.3, 0.7, big) + 2.5 * lump + 0.8 * fine;
        return vec2f(h, 0.0);
    }
    // apartment blocks and warehouses: flat roofs
    let cs = select(26.0, 38.0, layer == 2);
    let c = floor(s / cs);
    let h = hash_cell2(vec2i(i32(c), i32(salt)), 0xb1du);
    let f = fract(s / cs);
    var top = 6.0 + 18.0 * h.x * h.x + select(0.0, 10.0, layer == 2) * h.y;
    top *= step(0.06, f) * step(f, 0.94 - 0.2 * h.z);
    return vec2f(max(top, 4.0), 1.0 + c);
}

// shade a curtain hit at world point wp
fn curtain_col(wp: vec3f, s: f32, u: f32, layer: i32, bid: f32, fp: f32, l: Look) -> vec3f {
    let y = wp.y;
    if (layer == 0) {
        // dark foliage, clumped, sky-lit toward the top
        let n = noise_value2(vec2f(s * 0.35, y * 0.45)) * noise_value2(vec2f(s * 0.9, y * 1.1) + 7.0);
        return vec3f(0.004, 0.006, 0.004) * (0.6 + 1.2 * n) + l.amb * (0.08 + 0.12 * saturate((y - 4.0) / 10.0)) * (0.7 + 0.6 * n);
    }
    let hb = hash_cell2(vec2i(i32(bid), layer), 0x9a1u);
    var c = vec3f(0.006, 0.006, 0.007) * (0.7 + 0.6 * hb.x) + l.amb * (0.16 + 0.1 * hb.y);
    // windows: 3.2 m floors, 3 m bays; a few lit warm, the odd TV-blue
    let cell = vec2i(i32(floor(s / 3.0)), i32(floor(y / 3.2)));
    let h = hash_cell2(cell + vec2i(0, i32(bid) * 13), 0x3e1u);
    let f = vec2f(fract(s / 3.0), fract(y / 3.2));
    let win = step(0.3, f.x) * step(f.x, 0.7) * step(0.35, f.y) * step(f.y, 0.75);
    let near_k = saturate(1.5 - fp / 2.0);
    let tint = select(col_kelvin(2400.0 + 1600.0 * h.y), vec3f(0.45, 0.6, 1.0), h.z > 0.93);
    let on = step(h.x, 0.1 + 0.18 * hb.z);
    c += tint * l.city * 0.5 * on * mix(0.16, win, near_k) * step(1.5, y);
    return c;
}

// ------------------------------------------------------------------ props
// palm (Washingtonia robusta) outline in local metres: base at origin
fn palm_d(q: vec2f, h: f32, lean: f32, hv: vec4f) -> f32 {
    let yb = saturate(q.y / h);
    let cx = lean * yb * yb;
    let r = mix(0.4, 0.22, yb);
    var d = max(abs(q.x - cx) - r, max(-q.y, q.y - h));
    let c = vec2f(lean, h);
    // shaggy skirt of dead fronds under the crown
    let sk = q - c + vec2f(0.0, 1.5);
    let shag = 0.12 * sin(q.x * 9.0 + q.y * 3.0);
    d = min(d, length(sk / vec2f(0.8 + 0.12 * (c.y - q.y), 1.8)) - 1.0 + shag);
    // live fronds: thin stalks ending in fan blades
    for (var k = 0; k < 16; k++) {
        let fk = f32(k);
        let hk = hash_f(u32(k) * 7u + u32(hv.y * 1000.0));
        let a = -0.55 + fk * 0.28 + (hv.x - 0.5) * 0.3 + (hk - 0.5) * 0.2;
        let dir = vec2f(cos(a), sin(a));
        let len = 2.6 + 1.3 * hk;
        let droop = (0.5 + 0.9 * saturate(0.6 - dir.y)) * len * 0.35;
        let p1 = c + dir * len * 0.5 + vec2f(0.0, 0.15);
        let p2 = c + dir * len - vec2f(0.0, droop);
        d = min(d, sdf2_segment(q, c, p1) - 0.1);
        d = min(d, sdf2_segment(q, p1, p2) - 0.08 - 0.42 * saturate(dot(q - p1, p2 - p1) / dot(p2 - p1, p2 - p1)));
    }
    return d;
}

// palm k: (s, u, height, lean); pairs alternate sides, receding
fn palm(k: i32) -> vec4f {
    let h = hash_cell2(vec2i(k, 11), 0xa1au);
    let side = select(-1.0, 1.0, (k & 1) == 1);
    let grp = f32(k / 2);
    let s = 110.0 + grp * grp * 36.0 + grp * 80.0 + h.x * 30.0;
    let u = side * (WALL_U + 3.0 + 10.0 * h.y);
    return vec4f(s, u, 21.0 + 8.0 * h.z, (h.w - 0.5) * 3.0);
}

struct Over { a: f32, t: f32, c: vec3f }

// sign gantry across the road at distance sg, drawn on its vertical plane
fn gantry(ro: vec3f, rd: vec3f, sg: f32, half_only: bool, l: Look, ctx: Ctx, zoom: f32, glow: vec2f) -> Over {
    var o = Over(0.0, 1e9, vec3f(0.0));
    let k = road_slope(sg);
    let inv = inverseSqrt(1.0 + k * k);
    let tg = vec3f(k, 0.0, -1.0) * inv;
    let nl = vec3f(1.0, 0.0, k) * inv;
    let org = vec3f(road_cx(sg), 0.0, -sg);
    let den = dot(rd, tg);
    if (abs(den) < 1e-4) { return o; }
    let t = dot(org - ro, tg) / den;
    if (t <= 0.0) { return o; }
    let hp = ro + rd * t;
    let q = vec2f(dot(hp - org, nl), hp.y);
    if (abs(q.x) > 26.0 || q.y > 11.5 || q.y < 0.0) { return o; }
    if (half_only && q.x < -1.0) { return o; }
    let aa = ctx.px * t / zoom;
    // truss: two chords with lacing, posts at the shoulders and the median
    var ds = min(abs(q.y - 7.4) - 0.2, abs(q.y - 8.9) - 0.2);
    ds = max(ds, abs(q.x) - 24.0);
    let lace = abs(fract(q.x / 1.5) - 0.5) * 1.5;
    let dl = max(abs(lace - (q.y - 7.4) * 0.5) - 0.06, max(abs(q.y - 8.15) - 0.75, abs(q.x) - 24.0));
    ds = min(ds, dl);
    let post = min(abs(abs(q.x) - 24.0), abs(q.x)) - 0.35;
    ds = min(ds, max(post, q.y - 9.2));
    // signs face the outbound drivers (us) on the right; backs on the left
    let hs = hash_cell2(vec2i(i32(sg), 5), 0x5a9u);
    let w1 = 3.0 + 1.4 * hs.x;
    let c1 = vec2f(5.6 + w1, 8.6);
    let c2 = vec2f(17.0, 8.4);
    let w2 = 2.4 + 0.9 * hs.y;
    let face = min(sdf2_box(q - c1, vec2f(w1, 1.9)), sdf2_box(q - c2, vec2f(w2, 1.6)));
    var back = sdf2_box(q - vec2f(-9.0, 8.5), vec2f(3.6, 1.8));
    if (hs.z > 0.4) { back = min(back, sdf2_box(q - vec2f(-17.5, 8.4), vec2f(2.8, 1.6))); }
    let a_s = aa_fill_w(ds, aa);
    let a_f = aa_fill_w(face, aa);
    let a_b = aa_fill_w(back, aa);
    // steel: dark, a little sky from above, lamp glow from below
    let under = headlight_col() * glow.x * 0.03 + taillight_col() * glow.y * 0.05;
    let steel = vec3f(0.02, 0.021, 0.024) * (l.amb * 14.0 + 0.15) + under;
    var c = steel;
    if (a_f > 0.0) {
        // highway green, white border and legend lines; the sheeting catches
        // the outbound cars' headlamps
        let inside1 = sdf2_box(q - c1, vec2f(w1, 1.9)) < sdf2_box(q - c2, vec2f(w2, 1.6));
        let local = select(q - c2, q - c1, inside1);
        let hw = select(vec2f(w2, 1.6), vec2f(w1, 1.9), inside1);
        let light = 0.05 + l.amb.g * 3.5 + glow.y * 0.03 + 0.04 * l.city;
        var g = col_hex(0x0e5a34u) * light;
        let border = abs(sdf2_box(local, hw - vec2f(0.2)));
        var ink = 1.0 - smoothstep(0.06, 0.06 + aa, border);
        let row = floor((local.y + hw.y) / 0.85);
        let rows = abs(fract((local.y + hw.y) / 0.85) - 0.5);
        let lenr = hash_f(u32(row) + u32(sg) * 3u + select(0u, 9u, inside1));
        let rowmask = step(0.45, local.y + hw.y) * step(local.y, hw.y - 0.45);
        ink = max(ink, (1.0 - smoothstep(0.14, 0.14 + aa, rows)) * rowmask * step(abs(local.x + hw.x * 0.1), hw.x * (0.3 + 0.45 * lenr)));
        g = mix(g, vec3f(0.62, 0.63, 0.6) * light, saturate(ink) * 0.9);
        c = mix(c, g, a_f);
    }
    c = mix(c, steel * 0.9, a_b * (1.0 - a_f));
    o.a = max(a_s, max(a_f, a_b));
    o.t = t;
    o.c = c;
    return o;
}

// ------------------------------------------------------------------ scene
fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(road_cx(0.0), CAM_H, 0.0);
    let cam = cam_look_at(ro, vec3f(158.0, CAM_H - 3000.0 * 0.0445, -3000.0), 0.0, 22.0);
    let rd = cam_ray(cam, p);
    let zoom = cam.zoom;
    let t = ctx.t;

    var col = vec3f(0.0);
    var tbase = 1e9;
    var glow_under = vec2f(0.0);

    let tg = select(-1.0, -ro.y / rd.y, rd.y < -1e-5);
    if (tg > 0.0) {
        let gp = ro + rd * tg;
        let su = road_su(gp.x, gp.z);
        let fp = ctx.px * tg / zoom;
        let fs = fp * tg / CAM_H;
        let au = abs(su.y);
        if (au < WALL_U) {
            // ---- the road
            let lane_pos = (au - LANE0) / LANE_W;
            var alb = 0.05 + 0.02 * noise_value2(gp.xz * vec2f(0.4, 0.05));
            alb *= 1.0 + 0.15 * (1.0 - abs(fract(lane_pos) - 0.5) * 2.0) * step(0.0, lane_pos) * step(lane_pos, 5.0);
            if (au < 0.45) { alb = 0.2; }
            let pw = max(0.08, fp * 0.5);
            let paint_k = 0.075 / pw;
            var paint = exp(-sq((au - LANE0) / pw)) * paint_k * vec3f(0.9, 0.7, 0.2);
            paint += exp(-sq((au - EDGE) / pw)) * paint_k * vec3f(0.8);
            let lanes_f = fract(lane_pos + 0.5) - 0.5;
            let dash = mix(step(fract(su.x / 12.0), 0.3), 0.3, saturate(fs / 10.0));
            paint += exp(-sq(lanes_f * LANE_W / pw)) * paint_k * dash * vec3f(0.75) * step(0.5, lane_pos) * step(lane_pos, 4.5);
            let tr_in = fw_trails(su.x, su.y, -1.0, fp, fs, t, 1.0);
            let tr_out = fw_trails(su.x, su.y, 1.0, fp, fs, t, 1.0);
            // headlamp pools ahead of the oncoming cars, red wash behind the others
            let spill = headlight_col() * tr_in.y * 0.3 + taillight_col() * tr_out.y * 0.2;
            let wet = l.wet;
            let alb3 = vec3f(alb) * mix(1.0, 0.4, wet);
            col = alb3 * (l.amb * 2.2 + spill * l.trail) + paint * (l.amb * 3.0 + spill * l.trail * 0.8) * (1.0 - 0.5 * wet);
            if (wet > 0.0) {
                // mirror: the far city and sky along the reflected ray, and the
                // trails doubled on the plane y = -h, blurred sideways by ripples
                let rr = vec3f(rd.x, -rd.y, rd.z);
                let fres = 0.1 + 0.6 * pow(1.0 - saturate(-rd.y * 3.0), 4.0);
                col = mix(col, fw_far(gp, rr, l, ctx, zoom) * 0.75, fres * wet * 0.8);
                let th = (-0.8 - ro.y) / rd.y;
                let rp = ro + rd * th;
                let rs = road_su(rp.x, rp.z);
                let fpr = ctx.px * th / zoom + 0.45;
                let m_in = fw_trails(rs.x, rs.y, -1.0, fpr, fs + 10.0, t, 1.2);
                let m_out = fw_trails(rs.x, rs.y, 1.0, fpr, fs + 10.0, t, 1.2);
                col += (headlight_col() * (m_in.x * 2.5 + m_in.y * 0.5) + taillight_col() * (m_out.x * 3.0 + m_out.y * 0.6))
                       * fres * wet * l.trail * 1.3;
            }
            tbase = tg;
        } else {
            // ---- roadside: sound wall, then trees, then buildings, then flats
            let tw = cross_u(ro, rd, 0.0, tg, WALL_U);
            let wy = ro.y + rd.y * tw;
            if (wy < WALL_H) {
                let wp = ro + rd * tw;
                let ws = road_su(wp.x, wp.z);
                let fpw = ctx.px * tw / zoom;
                // block wall, pilasters every 6 m, a lighter cap, rain-streaked
                let streak = noise_value2(vec2f(ws.x * 0.7, 0.5)) * 0.25 + noise_value2(vec2f(ws.x * 0.05, wy * 0.2)) * 0.2;
                var alb = col_hex(0x8a8580u) * (0.8 - streak * 0.6);
                let detail_k = saturate(1.0 - fpw * 2.0);
                alb *= 1.0 - 0.12 * detail_k * (step(fract(wy / 0.4), 0.12) + step(fract(ws.x / 6.0), 0.06));
                alb *= mix(1.0, 1.25, step(WALL_H - 0.3, wy));
                let low = exp(-wy * 0.45);
                col = alb * (l.amb * 2.0 * (0.55 + 0.1 * wy) + (headlight_col() * 0.06 + taillight_col() * 0.06) * low * l.trail);
                col *= mix(1.0, 0.6, l.wet);
                tbase = tw;
            } else {
                var ta = tw;
                var hit = false;
                for (var layer = 0; layer < 3; layer++) {
                    let uu = select(select(WALL_U + 14.0, 75.0, layer == 1), 190.0, layer == 2);
                    if (au < uu) { break; }
                    let tc = cross_u(ro, rd, ta, tg, uu);
                    let cp = ro + rd * tc;
                    let cs = road_su(cp.x, cp.z);
                    let ch = curtain_h(cs.x, layer, cs.y);
                    if (cp.y < ch.x) {
                        col = curtain_col(cp, cs.x, cs.y, layer, ch.y, ctx.px * tc / zoom, l);
                        tbase = tc;
                        hit = true;
                        break;
                    }
                    ta = tc;
                }
                if (!hit) {
                    col = fw_flats(gp.x, gp.z, fp, l);
                    tbase = tg;
                }
            }
        }
        // haze: ground haze sits a little darker than the sky behind it
        let fogk = 1.0 - exp(-tbase * l.haze);
        col = mix(col, l.hor * mix(0.7, 0.95, saturate(tbase / 4000.0)), fogk);
    } else {
        col = fw_far(ro, rd, l, ctx, zoom);
    }

    // ---- lamp trails in the air, in front of whatever the ray hit
    if (rd.y < 0.0) {
        let th_h = (0.7 - ro.y) / rd.y;
        if (th_h < tbase + 1.0) {
            let hp = ro + rd * th_h;
            let hs = road_su(hp.x, hp.z);
            let fp = ctx.px * th_h / zoom;
            let tr = fw_trails(hs.x, hs.y, -1.0, fp, fp * th_h / CAM_H, t, 1.0);
            col += headlight_col() * (tr.x * 8.0 + tr.y * (0.08 + 0.22 * l.wet)) * l.trail * exp(-th_h * l.haze * 0.8);
            glow_under.x = tr.y;
        }
        let th_t = (0.95 - ro.y) / rd.y;
        if (th_t < tbase + 1.0) {
            let hp = ro + rd * th_t;
            let hs = road_su(hp.x, hp.z);
            let fp = ctx.px * th_t / zoom;
            let tr = fw_trails(hs.x, hs.y, 1.0, fp, fp * th_t / CAM_H, t, 1.0);
            col += taillight_col() * (tr.x * 5.5 + tr.y * (0.12 + 0.3 * l.wet)) * l.trail * exp(-th_t * l.haze * 0.8);
            glow_under.y = tr.y;
        }
    }

    // ---- palms, far to near
    for (var k = 11; k >= 0; k--) {
        let pk = palm(k);
        let k2 = road_slope(pk.x);
        let base = vec3f(road_cx(pk.x) + pk.y * sqrt(1.0 + k2 * k2), 0.0, -pk.x);
        let toc = base - ro;
        let nrm = normalize(vec3f(toc.x, 0.0, toc.z));
        let den = dot(rd, nrm);
        if (den <= 0.0) { continue; }
        let tp = dot(toc, nrm) / den;
        if (tp >= tbase) { continue; }
        let hp = ro + rd * tp;
        let q = vec2f(dot(hp - base, vec3f(-nrm.z, 0.0, nrm.x)), hp.y);
        if (abs(q.x) > 7.0 || q.y > pk.z + 5.0 || q.y < -1.0) { continue; }
        let a = aa_fill_w(palm_d(q, pk.z, pk.w, hash_cell2(vec2i(k, 2), 0xbeefu)), ctx.px * tp / zoom);
        if (a <= 0.0) { continue; }
        var pc = vec3f(0.005, 0.005, 0.006) + l.amb * 0.1;
        pc += l.warm * 0.025 * saturate(-q.x * 0.5);
        let fogk = 1.0 - exp(-tp * l.haze);
        pc = mix(pc, mix(l.hor, l.low, saturate((hp.y - ro.y) / 60.0)) * 0.85, fogk);
        col = mix(col, pc, a);
    }

    // ---- sign gantries, far to near
    for (var g = 2; g >= 0; g--) {
        let sg = select(select(105.0, 250.0, g == 1), 570.0, g == 2);
        let o = gantry(ro, rd, sg, g == 0, l, ctx, zoom, glow_under);
        if (o.a > 0.0 && o.t < tbase) {
            let fogk = 1.0 - exp(-o.t * l.haze);
            col = mix(col, mix(o.c, l.hor * 0.8, fogk), o.a);
        }
    }

    // ---- rain: streaks show against the lights, barely against the sky
    if (l.mode == 2u) {
        let r = rain_streaks(p, ctx, 0.5, 2.2, 0.1, 3);
        col += r * (vec3f(0.006) + col * 0.35);
    }
    return col * exp2(l.exposure);
}
