//! name: shibuya
//! title: Shibuya Scramble
//! category: city
//! tags: tokyo, crossing, crowd, neon, screens, rain, night
//! desc: the Shibuya scramble from above: crowds surge across every stripe when the lights change
//! themes: rain, night, day
//! uses: camera, sdf, rain, wet, noise
//! cost: light
//! fallback: city
//! credits: original

// World units are metres, y up. We look down from a window high in the
// south-east corner building, across the crossing toward the north-west. The
// ground is the plane y = 0; the city blocks are boxes carrying window grids,
// vertical signs and big video screens. The signal cycle is 40 s: 20 s of
// traffic (the east-west road, then the north-south road), then 20 s when
// every crossing — four sides and both diagonals — turns green at once and
// the waiting crowds pour out. Every pedestrian and car is a closed-form
// trajectory, looked up per pixel through its lane.

const CYCLE: f32 = 40.0;
const WALK0: f32 = 20.0;      // walk phase: 20 .. 40 s
const A_G0: f32 = 0.5;        // east-west road green
const A_G1: f32 = 9.0;
const B_G0: f32 = 10.5;       // north-south road green
const B_G1: f32 = 19.0;
const ROAD_A: f32 = 11.0;     // half width of the east-west road (|z| < 11)
const ROAD_B: f32 = 10.0;     // half width of the north-south road (|x| < 10)
const BLK: f32 = 22.0;        // building line

struct Look {
    mode: u32,        // 0 rain, 1 night, 2 day
    amb: vec3f,       // ambient light on the ground
    sky: vec3f,
    lamp: vec3f,      // street lamp colour * intensity
    facade: vec3f,    // light on facades (day)
    wet: f32,
    screen: f32,      // screen brightness
    win: f32,         // lit-window brightness
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, vec3f(0.010, 0.010, 0.013), col_hex(0x1c1626u) * 0.25, col_kelvin(4600.0) * 1.1,
                        vec3f(0.0), 0.15, 1.6, 1.0, 0.0);
        }
        case 2u: {
            return Look(2u, col_hex(0xd8dce4u) * 0.42, col_hex(0xc8d2e0u) * 0.9, vec3f(0.0),
                        col_hex(0xe0e4ecu) * 0.45, 0.0, 1.1, 0.1, 0.0);
        }
        default: {
            return Look(0u, vec3f(0.012, 0.012, 0.016), col_hex(0x3a2f36u) * 0.3, col_kelvin(4600.0) * 1.0,
                        vec3f(0.0), 1.0, 1.5, 0.9, 0.2);
        }
    }
}

// ------------------------------------------------------------------ city blocks
// building k: min corner (x, z), max corner (x, z); height in block_h
const NB: i32 = 13;
fn block(k: i32) -> vec4f {
    switch (k) {
        case 0: { return vec4f(-46.0, -50.0, -BLK, -BLK); }    // NW corner: the big screen
        case 1: { return vec4f(-74.0, -46.0, -48.0, -BLK); }
        case 2: { return vec4f(-46.0, -96.0, -24.0, -53.0); }
        case 3: { return vec4f(-112.0, -52.0, -77.0, -BLK); }
        case 4: { return vec4f(BLK, -44.0, 44.0, -BLK); }      // NE corner
        case 5: { return vec4f(46.0, -48.0, 76.0, -BLK); }
        case 6: { return vec4f(24.0, -96.0, 50.0, -47.0); }
        case 7: { return vec4f(-48.0, BLK, -BLK, 46.0); }      // SW corner
        case 8: { return vec4f(-50.0, 49.0, -24.0, 84.0); }
        case 9: { return vec4f(30.0, -172.0, 72.0, -124.0); }  // the tall tower
        case 11: { return vec4f(25.0, -33.0, 41.0, -32.4); }   // rooftop billboard on the NE corner
        case 12: { return vec4f(-72.0, -30.0, -52.0, -29.4); } // rooftop billboard, NW
        default: { return vec4f(-62.0, -150.0, -14.0, -100.0); }
    }
}
fn block_h(k: i32) -> f32 {
    switch (k) {
        case 0: { return 40.0; }
        case 1: { return 27.0; }
        case 2: { return 50.0; }
        case 3: { return 33.0; }
        case 4: { return 31.0; }
        case 5: { return 45.0; }
        case 6: { return 58.0; }
        case 7: { return 25.0; }
        case 8: { return 37.0; }
        case 9: { return 210.0; }
        case 11: { return 40.0; }
        case 12: { return 35.0; }
        default: { return 64.0; }
    }
}

// base height of box k (billboards stand on roofs)
fn block_y0(k: i32) -> f32 {
    if (k == 11) { return 31.5; }
    if (k == 12) { return 27.5; }
    return 0.0;
}

// ray/box: (t, face) with face 0 -x, 1 +x, 2 -z, 3 +z, 4 roof; t < 0 on a miss
fn box_hit(ro: vec3f, rd: vec3f, k: i32) -> vec2f {
    let b = block(k);
    let bmin = vec3f(b.x, block_y0(k), b.y);
    let bmax = vec3f(b.z, block_h(k), b.w);
    let r = select(rd, vec3f(1e-6), abs(rd) < vec3f(1e-6));
    let inv = 1.0 / r;
    let t0 = (bmin - ro) * inv;
    let t1 = (bmax - ro) * inv;
    let tn = min(t0, t1);
    let tf = max(t0, t1);
    let tnear = max(max(tn.x, tn.y), tn.z);
    let tfar = min(min(tf.x, tf.y), tf.z);
    if (tnear > tfar || tfar < 0.0 || tnear < 0.0) { return vec2f(-1.0, 0.0); }
    var face = 4.0;
    if (tnear == tn.x) { face = select(1.0, 0.0, rd.x > 0.0); }
    else if (tnear == tn.z) { face = select(3.0, 2.0, rd.z > 0.0); }
    return vec2f(tnear, face);
}

fn scene_hit(ro: vec3f, rd: vec3f) -> vec3f {
    var best = vec3f(1e9, 0.0, -1.0);
    for (var k = 0; k < NB; k++) {
        let h = box_hit(ro, rd, k);
        if (h.x > 0.0 && h.x < best.x) { best = vec3f(h.x, h.y, f32(k)); }
    }
    return best;
}

// ------------------------------------------------------------------ screens
// screen s: (building, face, centre along the face, centre height), size (w, h)
fn screen_def(s: i32) -> vec4f {
    switch (s) {
        case 0: { return vec4f(0.0, 3.0, -34.0, 25.0); }  // NW corner, south face: the big one
        case 1: { return vec4f(0.0, 1.0, -36.0, 21.0); }  // NW corner, east face
        case 2: { return vec4f(4.0, 3.0, 33.0, 21.0); }   // NE corner, south face
        case 3: { return vec4f(7.0, 1.0, 34.0, 15.0); }   // SW corner, east face: a tall banner
        case 4: { return vec4f(5.0, 3.0, 61.0, 34.0); }   // NE, second building
        case 5: { return vec4f(11.0, 3.0, 33.0, 35.75); } // rooftop billboard
        case 6: { return vec4f(12.0, 3.0, -62.0, 31.25); } // rooftop billboard
        default: { return vec4f(-1.0); }
    }
}
fn screen_size(s: i32) -> vec2f {
    switch (s) {
        case 0: { return vec2f(17.0, 11.0); }
        case 1: { return vec2f(11.0, 9.0); }
        case 2: { return vec2f(13.0, 8.0); }
        case 3: { return vec2f(5.0, 13.0); }
        case 4: { return vec2f(13.0, 8.0); }
        case 5: { return vec2f(15.4, 8.0); }
        case 6: { return vec2f(19.4, 7.0); }
        default: { return vec2f(0.0); }
    }
}

fn ad_color(i: u32) -> vec3f {
    switch (i % 8u) {
        case 0u: { return col_hex(0xfff2dcu); }
        case 1u: { return col_hex(0x36c8ffu); }
        case 2u: { return col_hex(0xff4f96u); }
        case 3u: { return col_hex(0xff8a2eu); }
        case 4u: { return col_hex(0x9cf05au); }
        case 5u: { return col_hex(0x3050ffu); }
        case 6u: { return col_hex(0xff3a3au); }
        default: { return col_hex(0xffd64au); }
    }
}

// one ad "slide": two colours, a soft gradient and a drifting blob
fn ad_slide(uv: vec2f, slot: f32, id: i32, t: f32) -> vec3f {
    let h = hash_cell2(vec2i(i32(slot), id), 0xad5u);
    let a = ad_color(u32(h.x * 8.0));
    let b = ad_color(u32(h.y * 8.0) + 3u);
    let ang = h.z * TAU;
    let g = saturate(0.5 + 0.7 * dot(uv - 0.5, vec2f(cos(ang), sin(ang))));
    var c = mix(a, b * 0.35, smoothstep(0.1, 0.9, g));
    let bc = vec2f(0.3 + 0.4 * h.w + 0.1 * sin(t * 0.3 + h.x * 6.0), 0.5 + 0.15 * cos(t * 0.23 + h.y * 6.0));
    let blob = exp(-dot(uv - bc, uv - bc) * 14.0);
    c = mix(c, mix(a, vec3f(1.0), 0.5), blob * (0.25 + 0.3 * h.z));
    return c;
}

// screen content at uv (0..1), crossfading to the next slide every ~9 s
fn screen_col(uv: vec2f, id: i32, t: f32) -> vec3f {
    let per = 8.0 + f32(id) * 0.9;
    let x = t / per + f32(id) * 0.37;
    let slot = floor(x);
    let f = fract(x);
    let fade = smoothstep(0.86, 1.0, f);
    var c = ad_slide(uv, slot, id, t);
    if (fade > 0.0) { c = mix(c, ad_slide(uv, slot + 1.0, id, t), fade); }
    return c;
}
fn screen_avg(id: i32, t: f32) -> vec3f { return screen_col(vec2f(0.5), id, t) * 0.8; }

// ------------------------------------------------------------------ facades
fn facade(hp: vec3f, face: u32, k: i32, l: Look, t: f32) -> vec3f {
    let along = select(hp.x, hp.z, face < 2u);
    let y = hp.y;
    let bh = block_h(k);
    if (face == 4u) {
        return vec3f(0.015) * (l.amb * 20.0 + 0.2) + l.facade * 0.25;
    }
    if (k >= 11) {
        var bc = vec3f(0.01) + l.facade * 0.1;
        let sdf = screen_def(k - 6);
        let sz = screen_size(k - 6);
        let q = vec2f(along - sdf.z, y - sdf.w);
        if (face == 3u && abs(q.x) < sz.x * 0.5 && abs(q.y) < sz.y * 0.5) {
            bc = screen_col(q / sz + 0.5, k - 6, t) * l.screen;
        }
        return bc;
    }
    let hb = hash_cell2(vec2i(k, i32(face)), 0xfa0u);
    // concrete, tile or glass
    let alb = mix(col_hex(0x3e4046u), col_hex(0x6a665eu), hb.x) * 0.3;
    var c = alb * (l.amb * 3.0 + l.facade * (0.8 + 0.4 * hb.y));
    // storeys: offices have ribbon glazing, others punched windows
    let ribbon = hb.y > 0.5;
    let bay = select(1.6, 3.0, ribbon);
    let wuv = vec2f(along / bay, y / 3.6);
    let cell = vec2i(floor(wuv));
    let f = fract(wuv);
    let wh = hash_cell2(cell + vec2i(k * 57, i32(face) * 13), 0x3a7u);
    var frame = step(0.2, f.x) * step(f.x, 0.8) * step(0.3, f.y) * step(f.y, 0.8);
    if (ribbon) { frame = step(0.05, f.x) * step(0.28, f.y) * step(f.y, 0.86); }
    let band = floor(y / 3.6);
    let fl = hash_cell2(vec2i(i32(band), k * 3 + i32(face)), 0x71au);
    let on = select(step(wh.x, 0.2 + 0.5 * fl.x), step(fl.x, 0.45) * step(0.3, wh.x) * (0.2 + 1.2 * wh.y * wh.y), ribbon);
    let tint = mix(col_kelvin(3200.0), col_kelvin(select(4200.0, 6200.0, fl.z > 0.4)), select(0.0, 1.0, ribbon));
    let upper = step(4.6, y) * step(y, bh - 1.0);
    c += tint * frame * on * l.win * select(0.14 + 0.12 * wh.z, 0.07 + 0.06 * wh.z, ribbon) * upper;
    // ribbon glass mirrors the glow of the street a little
    c += frame * select(0.0, 0.012, ribbon) * l.win * vec3f(0.9, 0.7, 0.8) * upper;
    if (l.mode == 2u) { c = mix(c, l.sky * 0.16, frame * 0.6 * upper); }
    // lit sign bands across some floors
    if (fl.y < 0.22 && y > 5.0 && y < bh - 2.0) {
        let inband = smoothstep(0.1, 0.18, fract(y / 3.5)) * smoothstep(0.9, 0.82, fract(y / 3.5));
        let sc = select(ad_color(u32(fl.z * 8.0)), col_hex(0xfff2dcu), fl.w < 0.5);
        c = mix(c, sc * l.screen * 0.14, inband * 0.9);
    }
    // street level: shopfronts
    // the street's glow climbs the lower floors
    c += col_kelvin(3000.0) * 0.035 * exp(-y / 7.0) * select(1.0, 0.2, l.mode == 2u) * step(4.6, y);
    // street level: dark ground floor, lit shop windows between pillars,
    // a canopy line above
    if (y < 4.6) {
        let sf = fract(along / 6.0 + hb.x);
        let sh = hash_cell2(vec2i(i32(floor(along / 6.0 + hb.x)), k * 5 + i32(face)), 0x5b0u);
        let glass = step(0.12, sf) * step(sf, 0.9) * smoothstep(3.4, 3.2, y) * smoothstep(0.3, 0.5, y);
        c = alb * 0.5 * (l.amb * 3.0 + l.facade);
        c += col_kelvin(2900.0 + 2600.0 * sh.x) * glass * (0.1 + 0.25 * sh.y) * select(1.0, 0.25, l.mode == 2u);
        c = mix(c, vec3f(0.01), smoothstep(3.7, 3.8, y) * smoothstep(4.3, 4.2, y));
    }
    // vertical signboards stacked down one corner
    let b = block(k);
    let e0 = select(b.x, b.y, face < 2u);
    let e1 = select(b.z, b.w, face < 2u);
    let ec = select(e1 - 1.6, e0 + 1.6, hb.w > 0.5);
    let sd = abs(along - ec);
    if (sd < 1.0 && y > 5.0 && y < bh - 2.0 && k != 9) {
        let seg = floor(y / 2.4);
        let sh = hash_cell2(vec2i(k * 7 + i32(face), i32(seg)), 0x516u);
        let panel = step(sd, 0.85) * step(0.1, fract(y / 2.4));
        let sc = mix(select(ad_color(u32(sh.x * 8.0)), col_hex(0xfff2dcu), sh.y < 0.35), vec3f(0.9), 0.3);
        c = mix(c, sc * l.screen * (0.08 + 0.1 * sh.z), panel);
    }
    // video screens
    for (var s = 0; s < 7; s++) {
        let sdf = screen_def(s);
        if (i32(sdf.x) != k || u32(sdf.y) != face) { continue; }
        let sz = screen_size(s);
        let q = vec2f(along - sdf.z, y - sdf.w);
        if (abs(q.x) < sz.x * 0.5 + 0.5 && abs(q.y) < sz.y * 0.5 + 0.5) {
            let inner = step(abs(q.x), sz.x * 0.5) * step(abs(q.y), sz.y * 0.5);
            var uv = q / sz + 0.5;
            if (face == 1u || face == 2u) { uv.x = 1.0 - uv.x; }
            c = mix(vec3f(0.008), screen_col(uv, s, t) * l.screen, inner);
        }
    }
    return c;
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

// person colour: mostly dark coats, some light, a few colours
fn person_col(h: f32, rainy: bool) -> vec3f {
    if (rainy) {
        // umbrellas: Tokyo's clear vinyl ones, black, navy, a few colours
        if (h < 0.5) { return vec3f(0.55, 0.57, 0.6); }
        if (h < 0.75) { return vec3f(0.02); }
        if (h < 0.85) { return col_hex(0x1e2a4au); }
        return mix(ad_color(u32(h * 97.0)), vec3f(0.3), 0.5) * 0.3;
    }
    if (h < 0.45) { return vec3f(0.018, 0.018, 0.02); }
    if (h < 0.65) { return vec3f(0.07, 0.065, 0.06); }
    if (h < 0.88) { return vec3f(0.5, 0.48, 0.45); }
    return mix(ad_color(u32(h * 97.0)), vec3f(0.2), 0.55) * 0.25;
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
    let r = select(0.26, 0.52, rainy);
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
            // queue position, a little jitter; arrivals trickle in while waiting
            let u0 = -0.6 - f32(k) * sp - ha.x * sp * 0.5;
            let arrive = f32(k) / 10.0 * (WALK0 - 3.0) + ha.y * 2.0;
            var vis = 1.0;
            var uk = u0;
            if (walking) {
                uk = u0 + shift * (0.95 + 0.1 * ha.z);
            } else {
                vis = smoothstep(arrive, arrive + 1.0, cyc);
            }
            // fade out on reaching the far pavement
            vis *= smoothstep(len + 4.0, len + 1.5, uk);
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
    let r = select(0.26, 0.5, rainy);
    for (var layer = 0; layer < 2; layer++) {
        let dir = select(-1.0, 1.0, layer == 0);
        var q = select(g.yx, g, along_x);
        q.x += dir * 1.2 * t + f32(layer) * 5.0;
        let cs = select(2.4, 3.0, rainy);
        let c = vec2i(floor(q / cs));
        let h = hash_cell2(c + vec2i(layer * 1000, 0), 0x9a5u);
        let busy = 0.05 + 0.33 * smoothstep(42.0, 16.0, length(g));
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

struct Car { a: f32, c: vec3f, beam: f32 }

fn cars(ro: vec3f, rd: vec3f, t: f32, l: Look, fp: f32) -> Car {
    var out = Car(0.0, vec3f(0.0), 0.0);
    // body test on the roof plane, beams on the road
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
                            var col = body * (l.amb * 4.0 + l.facade * 0.9 + 0.02);
                            // glass band and roof
                            let roof = smoothstep(0.9, 0.5, abs(q.x + 0.2)) * smoothstep(0.75, 0.5, abs(q.y));
                            col = mix(col, vec3f(0.01), smoothstep(1.5, 1.2, abs(q.x + 0.2)) * (1.0 - roof) * 0.7);
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

// light falling on the ground: lamps, screens
fn ground_light(g: vec2f, l: Look, t: f32) -> vec3f {
    var e = l.amb + l.facade;
    if (l.mode != 2u) {
        // the whole crossing sits in a wash of light from signs and screens
        e += l.lamp * (0.05 + 0.04 * l.wet) * smoothstep(60.0, 20.0, length(g));
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
            e += l.lamp * 0.35 * 100.0 / (100.0 + d2) * 100.0 / (100.0 + d2 * 0.3);
        }
    }
    // screens: small area lights facing the crossing
    for (var s = 0; s < 7; s++) {
        let sdf = screen_def(s);
        let k = i32(sdf.x);
        let face = u32(sdf.y);
        let b = block(k);
        var c3: vec3f;
        var nrm: vec2f;
        if (face == 1u) { c3 = vec3f(b.z, sdf.w, sdf.z); nrm = vec2f(1.0, 0.0); }
        else { c3 = vec3f(sdf.z, sdf.w, b.w); nrm = vec2f(0.0, 1.0); }
        let dv = vec3f(g.x, 0.0, g.y) - c3;
        let dist2 = dot(dv, dv);
        let dir = dv * inverseSqrt(dist2);
        let cosn = max(dot(dir.xz, nrm), 0.0) * length(dir.xz);
        let cosg = max(-dir.y, 0.0);
        let sz = screen_size(s);
        e += screen_avg(s, t) * l.screen * sz.x * sz.y * cosn * cosg / (PI * dist2) * 0.9;
    }
    return e;
}

fn ground(g: vec2f, ro: vec3f, rd: vec3f, tg: f32, l: Look, t: f32, fp: f32, ctx: Ctx) -> vec3f {
    let ax = abs(g.x);
    let az = abs(g.y);
    let road = ax < ROAD_B || az < ROAD_A;
    var alb: vec3f;
    var paint = 0.0;
    if (road) {
        alb = vec3f(0.05, 0.05, 0.055) * (0.85 + 0.3 * noise_value2(g * 0.7));
        paint = markings(g, fp, rd);
        alb = mix(alb, vec3f(0.62), paint);
    } else {
        // pavement: light pavers, yellow tactile strips at the kerbs
        let tile = step(0.06, fract(g.x / 0.6)) * step(0.06, fract(g.y / 0.6));
        alb = col_hex(0x8e8a84u) * 0.45 * mix(0.85, 1.0, mix(tile, 0.94, saturate(fp * 4.0)));
        let kerb = min(abs(ax - ROAD_B), abs(az - ROAD_A));
        let near_cross = (az < 19.0 && ax < 19.0);
        if (kerb < 1.0 && kerb > 0.4 && near_cross) { alb = col_hex(0xd8b83au) * 0.5; }
        alb *= 1.0 - 0.5 * smoothstep(0.4, 0.0, kerb);
        // warm pools thrown out of the shopfronts
        let front = min(abs(ax - BLK), abs(az - BLK));
        paint = -exp(-front / 2.5) * select(0.9, 0.15, l.mode == 2u);
    }
    let wet = l.wet;
    let pud = wet_puddles(g * 0.9, 0.35 * wet, ctx) * select(0.0, 1.0, road);
    let wsurf = wet_surface(alb, 0.6, wet * 0.8 + pud * 0.2);
    var c = wsurf.rgb * (ground_light(g, l, t) + col_kelvin(3100.0) * max(-paint, 0.0) * 0.12);
    paint = max(paint, 0.0);
    if (wet > 0.0) {
        // wet mirror: facades and screens, smeared vertically by the film of water
        let gp = vec3f(g.x, 0.0, g.y);
        let rr = vec3f(rd.x, -rd.y, rd.z);
        let fres = 0.04 + 0.5 * pow(1.0 - saturate(-rd.y), 5.0);
        var refl = vec3f(0.0);
        for (var j = 0; j < 3; j++) {
            let jit = (f32(j) - 1.0) * 0.035 * (1.0 - pud);
            let r2 = normalize(rr + vec3f(0.0, jit, 0.0));
            let h = scene_hit(gp + vec3f(0.0, 0.01, 0.0), r2);
            if (h.z >= 0.0) {
                refl += facade(gp + r2 * h.x, u32(h.y), i32(h.z), l, t);
            } else {
                refl += l.sky;
            }
        }
        refl /= 3.0;
        c += refl * fres * (wet * 0.6 + pud * 1.4) * (1.0 - paint * 0.6);
    }
    return c;
}

// ------------------------------------------------------------------ scene
fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let cyc = fmod_pos(t, CYCLE);
    let ro = vec3f(26.0, 50.0, 84.0);
    let cam = cam_look_at(ro, vec3f(-3.0, 4.0, -10.0), 0.0, 36.0);
    let rd = cam_ray(cam, p);
    let rainy = l.mode == 0u;

    let hit = scene_hit(ro, rd);
    let tg = select(1e9, -ro.y / rd.y, rd.y < -1e-4);
    var col: vec3f;
    if (hit.z >= 0.0 && hit.x < tg) {
        let hp = ro + rd * hit.x;
        col = facade(hp, u32(hit.y), i32(hit.z), l, t);
        col = mix(col, l.sky * 0.8, 1.0 - exp(-hit.x * select(select(0.0016, 0.0006, l.mode == 2u), 0.0028, rainy)));
    } else if (tg < 1e8) {
        let g = (ro + rd * tg).xz;
        let fp = ctx.px * tg / cam.zoom;
        col = ground(g, ro, rd, tg, l, t, fp, ctx);
        // car head-lamp pools and bodies
        let cr = cars(ro, rd, t, l, fp);
        col += col_kelvin(5000.0) * cr.beam * select(0.0, 0.06, l.mode != 2u) * (1.0 + 1.5 * l.wet);
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
            var pc = person_col(best.y, rainy) * (pl * 1.2 + 0.01);
            if (rainy && best.y < 0.5) {
                // clear umbrellas glow with the light around them
                pc = mix(col, pl * 0.35 + vec3f(0.012), 0.5);
            }
            col = mix(col, pc, best.x);
        }
        col = mix(col, cr.c, cr.a);
        col = mix(col, l.sky * 0.8, 1.0 - exp(-tg * select(select(0.0016, 0.0006, l.mode == 2u), 0.0028, rainy)));
    } else {
        col = l.sky;
    }
    // screens glow in the damp air
    for (var s = 0; s < 7; s++) {
        let sdf = screen_def(s);
        let b = block(i32(sdf.x));
        var c3: vec3f;
        if (u32(sdf.y) == 1u) { c3 = vec3f(b.z + 1.0, sdf.w, sdf.z); } else { c3 = vec3f(sdf.z, sdf.w, b.w + 1.0); }
        let oc = c3 - ro;
        let tc = max(dot(oc, rd), 0.0);
        let d2 = dot(oc, oc) - tc * tc;
        let sz = screen_size(s);
        let r2 = sz.x * sz.y * 0.25;
        col += screen_avg(s, t) * l.screen * select(0.02, 0.035, rainy) * sq(r2 / (r2 + d2)) * select(1.0, 0.25, l.mode == 2u);
    }
    if (rainy) {
        let r = rain_streaks(p, ctx, 0.5, 2.0, 0.08, 3);
        col += r * (vec3f(0.01) + col * 0.3);
    }
    return col * exp2(l.exposure);
}
