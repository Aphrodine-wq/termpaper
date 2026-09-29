//! name: harbor
//! title: Fishing Harbor at Dawn
//! category: coast
//! tags: harbor, boats, nova scotia, maine, mist, reflections, dawn
//! desc: lobster boats riding at their moorings in a misty cove of fish shacks and a wooden pier
//! themes: dawn, dusk, night
//! uses: camera, sdf, sky, stars, clouds
//! cost: heavy
//! fallback: ocean
//! credits: original

// World units are metres; y up, the water is y = 0 and the camera stands on
// a float 2.8 m up, looking down -z across the cove. A wooden pier enters
// from the left; four lobster boats ride at their moorings; weathered fish
// shacks on stilts line the granite far shore under a spruce ridge. The
// water is a near mirror: reflections are real re-marches of the scene,
// broken by slow ripples, and a low mist lies on the water.

const CAM: vec3f = vec3f(0.0, 2.8, 0.0);
const P0: vec2f = vec2f(-8.5, -9.0);      // pier: near end ...
const P1: vec2f = vec2f(-21.0, -72.0);    // ... far end
const SHORE: f32 = -88.0;                 // far shoreline (z)

struct Look {
    sun: vec3f,
    night: f32,    // 0 day, 1 blue hour, 2 night
    lamps: f32,    // how much the lamps are on
    exposure: f32,
    haze: f32,
    mist: f32,
    sun_c: vec3f,
    amb: vec3f,
    hor: vec3f,    // horizon air colour
    lamp_c: vec3f,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            l.sun = sky_sun_dir(-3.5, 62.0);
            l.night = 1.0; l.lamps = 1.0; l.exposure = 1.1; l.haze = 1.2; l.mist = 0.7;
        }
        case 2u: {
            l.sun = sky_sun_dir(34.0, 28.0);
            l.night = 2.0; l.lamps = 1.0; l.exposure = 1.0; l.haze = 1.0; l.mist = 0.8;
        }
        default: {
            l.sun = sky_sun_dir(2.5, -140.0);
            l.night = 0.0; l.lamps = 0.0; l.exposure = 0.9; l.haze = 1.3; l.mist = 1.5;
        }
    }
    l.lamp_c = col_kelvin(2500.0) * 2.2;
    if (l.night > 1.5) {
        l.sun_c = vec3f(0.035, 0.045, 0.07);
        l.amb = vec3f(0.004, 0.006, 0.012);
        l.hor = vec3f(0.006, 0.009, 0.016);
    } else if (l.night > 0.5) {
        // blue hour: no direct light, a deep blue dome, afterglow on the right
        l.sun_c = vec3f(0.0);
        l.amb = vec3f(0.008, 0.013, 0.03);
        l.hor = vec3f(0.028, 0.034, 0.066);
    } else {
        l.sun_c = sky_sun_light(l.sun) * 1.3;
        l.amb = sky_ambient(l.sun) * select(1.0, 1.6, l.night > 0.5);
        l.hor = sky_atmosphere_haze(normalize(vec3f(0.0, 0.03, -1.0)), l.sun, l.haze) * ozone(l.sun);
        l.amb *= ozone(l.sun);
    }
    return l;
}

// The sky model has no ozone. Its Chappuis band absorbs orange and green
// along the long twilight path, which is what keeps dawn and blue-hour
// skies blue instead of olive-brown.
fn ozone(sun: vec3f) -> vec3f {
    let path = saturate(1.0 - sun.y * 6.0) * 1.3;
    return exp(-vec3f(0.62, 0.52, 0.04) * path);
}

// ------------------------------------------------------------ geometry

fn gable_roof(q: vec3f, half_len: f32, half_w: f32, rise: f32) -> f32 {
    // ridge along x; base at y = 0
    let s = rise / half_w;
    let slope = (abs(q.z) * s + q.y - rise) / sqrt(1.0 + s * s);
    return max(max(slope, -q.y), abs(q.x) - half_len);
}

// fish shacks on stilts along the far shore: (distance, material, hash)
fn shacks(p: vec3f) -> vec3f {
    let cw = 16.0;
    let ci = clamp(round(p.x / cw), -7.0, 7.0);
    let h = hash_cell2(vec2i(i32(ci), 3), 0x5ac4u);
    if (h.w > 0.85) { return vec3f(max(abs(p.z - SHORE) - 10.0, 1.0), 0.0, 0.0); }
    let zc = SHORE + 1.5 + 4.0 * noise_value2(vec2f(ci * 0.7, 1.0)) - 2.0;
    let c = vec3f(ci * cw + (h.x - 0.5) * 5.0, 1.8, zc);
    let q = p - c;
    let hl = 3.8 + 2.2 * h.y;         // half length along x
    let hw = 3.2 + 1.0 * h.z;         // half depth along z
    let wh = 3.6 + 1.8 * h.x;         // wall height
    let walls = sdf_box(q - vec3f(0.0, wh * 0.5, 0.0), vec3f(hl, wh * 0.5, hw));
    var roof: f32;
    if (h.y > 0.45) {
        roof = gable_roof(q - vec3f(0.0, wh, 0.0), hl + 0.35, hw + 0.35, 2.4 + h.z);
    } else {
        // gable end facing the water
        let qr = vec3f(q.z, q.y - wh, q.x);
        roof = gable_roof(qr, hw + 0.35, hl + 0.35, 2.6 + h.z);
    }
    var d = vec3f(walls, 1.0, h.x * 0.5 + h.z * 0.5);
    if (roof < d.x) { d = vec3f(roof, 2.0, h.y); }
    // stilts under the seaward half, a small wharf in front
    let g = vec2f(q.x, q.z - hw * 0.2);
    let cg = g - vec2f(2.4, 2.2) * clamp(round(g / vec2f(2.4, 2.2)), vec2f(-2.0, -1.0), vec2f(2.0, 1.0));
    let post = max(length(cg) - 0.14, abs(q.y + 1.8) - 1.8);
    let wharf = sdf_box(q - vec3f(0.0, -0.1, hw + 1.6), vec3f(hl * 0.7, 0.12, 1.6));
    let s = min(post, wharf);
    if (s < d.x) { d = vec3f(s, 3.0, 0.0); }
    return d;
}

fn pier_frame(p: vec3f) -> vec3f {
    let dir = normalize(P1 - P0);
    let d = p.xz - P0;
    return vec3f(dot(d, vec2f(-dir.y, dir.x)), p.y, dot(d, dir));
}

fn pier(p: vec3f) -> vec2f {
    let q = pier_frame(p);
    let len = length(P1 - P0);
    // deck and its stringers
    let zc = clamp(q.z, -2.0, len);
    var d = vec2f(sdf_box(vec3f(q.x, q.y - 2.1, q.z - zc), vec3f(1.7, 0.13, 0.4)), 4.0);
    // pilings in pairs every 3.5 m, thick with weed below the tide line
    let zi = clamp(round(q.z / 3.5), 0.0, floor(len / 3.5));
    let pz = q.z - zi * 3.5;
    let px = abs(q.x) - 1.55;
    let pil = max(length(vec2f(px, pz)) - 0.15, abs(q.y - 0.6) - 1.5);
    d = op_umin(d, vec2f(pil, 5.0));
    // lamp posts on the seaward side every 21 m
    let li = clamp(round((q.z - 8.0) / 21.0), 0.0, 2.0);
    let lz = q.z - 8.0 - li * 21.0;
    let post = max(length(vec2f(q.x - 1.5, lz)) - 0.06, abs(q.y - 3.9) - 1.7);
    let head = sdf_box(vec3f(q.x - 1.32, q.y - 5.58, lz), vec3f(0.22, 0.07, 0.1));
    d = op_umin(d, vec2f(min(post, head), 6.0));
    // stacked lobster traps
    let tz = q.z - 14.0 - 22.0 * clamp(round((q.z - 14.0) / 22.0), 0.0, 1.0);
    let tq = vec3f(q.x + 0.7, q.y - 2.23, tz);
    let tg = vec3f(tq.x, tq.y - 0.28 * clamp(round(tq.y / 0.56), 0.0, 2.0) * 2.0, tq.z - 1.0 * clamp(round(tq.z / 1.0), -1.0, 1.0));
    let trap = max(sdf_box(tg - vec3f(0.0, 0.25, 0.0), vec3f(0.42, 0.25, 0.46)), sdf_box(tq - vec3f(0.0, 0.8, 0.0), vec3f(0.5, 0.82, 1.6)));
    d = op_umin(d, vec2f(trap, 7.0));
    return d;
}

struct Boat { c: vec3f, head: f32, hull: vec3f, stripe: vec3f }

fn boat_get(i: i32) -> Boat {
    switch (i) {
        case 1: { return Boat(vec3f(-11.0, 0.0, -37.0), -1.77, col_hex(0x2f4a3au), col_hex(0xd9d4c8u)); }
        case 2: { return Boat(vec3f(25.0, 0.0, -55.0), -0.5, col_hex(0xe4e0d6u), col_hex(0x2e4f7au)); }
        case 3: { return Boat(vec3f(-4.0, 0.0, -72.0), 2.6, col_hex(0xb8892eu), col_hex(0x2a2522u)); }
        default: { return Boat(vec3f(5.0, 0.0, -23.0), 0.55, col_hex(0xe6e3dcu), col_hex(0x8e2a22u)); }
    }
}

// boat-local point: heave, roll and pitch are closed-form swells in t
fn boat_local(p: vec3f, b: Boat, i: i32, t: f32) -> vec3f {
    let fi = f32(i);
    let heave = 0.07 * sin(t * 0.85 + fi * 1.9) + 0.03 * sin(t * 1.7 + fi);
    let roll = 0.035 * sin(t * 0.72 + fi * 2.7);
    let pitch = 0.015 * sin(t * 0.61 + fi * 0.8);
    var q = p - b.c - vec3f(0.0, heave, 0.0);
    let xz = rot2(b.head) * q.xz;
    q = vec3f(xz.x, q.y, xz.y);
    let yz = rot2(roll) * vec2f(q.y, q.z);
    q = vec3f(q.x, yz.x, yz.y);
    let xy = rot2(pitch) * vec2f(q.x, q.y);
    return vec3f(xy.x, xy.y, q.z);
}

// lobster boat, bow toward +x: (distance, material)
fn boat_sdf(q: vec3f) -> vec2f {
    // Downeast hull: high flared bow with a raked stem, sheer sweeping down
    // to a low open cockpit aft, flat transom, soft V bottom
    let u = saturate((q.x + 5.3) / 10.8);
    let taper = mix(1.0, 0.06, pow(smoothstep(0.42, 1.0, u), 1.3));
    let flare = 0.82 + 0.18 * saturate(q.y / 1.4);
    let hw = 1.85 * taper * flare;
    let top = 1.0 + 0.75 * pow(smoothstep(0.35, 1.0, u), 1.6);
    let stem = 5.3 + 0.55 * saturate(q.y / 1.6);
    let keel = -0.7 + 0.45 * saturate(abs(q.z) / 1.8) + 0.25 * smoothstep(0.8, 1.0, u);
    let side = (abs(q.z) - hw) * 0.8;
    let hull = max(max(side, max(q.x - stem, -q.x - 5.3)), max(q.y - top, keel - q.y));
    var d = vec2f(hull, 10.0);
    // open cockpit aft: carve the inside of the gunwale
    let pit = sdf_box(q - vec3f(-2.8, 1.2, 0.0), vec3f(2.35, 0.45, max(1.85 * flare - 0.14, 0.1)));
    d.x = max(d.x, -pit);
    // wheelhouse forward of midships, raked windshield, overhanging roof
    var cab = sdf_box(q - vec3f(1.0, 1.95, 0.0), vec3f(1.3, 0.95, 1.25));
    cab = max(cab, (q.x - 2.3 + (q.y - 1.0) * 0.35) * 0.94);
    let cabroof = sdf_box(q - vec3f(0.85, 2.95, 0.0), vec3f(1.65, 0.06, 1.35));
    d = op_umin(d, vec2f(min(cab, cabroof), 11.0));
    // mast and antenna
    let mast = max(length(q.xz - vec2f(0.6, 0.0)) - 0.05, abs(q.y - 4.0) - 1.1);
    d = op_umin(d, vec2f(mast, 12.0));
    return d;
}

fn boats(p: vec3f, t: f32) -> vec3f {
    var d = vec3f(1e4, 0.0, 0.0);
    for (var i = 0; i < 4; i++) {
        let b = boat_get(i);
        let r = length(p - b.c - vec3f(0.0, 1.0, 0.0));
        if (r > 8.0) { d.x = min(d.x, r - 7.0); continue; }
        let bd = boat_sdf(boat_local(p, b, i, t));
        if (bd.x < d.x) { d = vec3f(bd.x, bd.y, f32(i)); }
    }
    return d;
}

// granite far shore under the spruce ridge
fn shore_h(xz: vec2f) -> f32 {
    let s = -xz.y + SHORE - 4.0 * noise_value2(vec2f(xz.x * 0.03, 5.0));
    let n = noise_fbm2(xz * 0.08, 4);
    let rock = 7.0 * smoothstep(-3.0, 9.0, s) + 2.5 * (n - 0.5) * smoothstep(-2.0, 6.0, s);
    return rock - 2.2 + 0.06 * max(s - 9.0, 0.0) + 1.6 * noise_value2(xz * 0.03);
}

// (distance, material, extra) for the whole world except water and sky
fn world(p: vec3f, t: f32) -> vec3f {
    var d = vec3f(1e4, 0.0, 0.0);
    if (p.z < SHORE + 21.0) {
        d = vec3f((p.y - shore_h(p.xz)) * 0.55, 8.0, 0.0);
        if (p.y < 16.0 && p.z > SHORE - 12.0) {
            let s = shacks(p);
            if (s.x < d.x) { d = s; }
        }
    } else {
        d.x = p.z - SHORE - 21.0 + 0.5;
    }
    let pb = pier_frame(p);
    if (abs(pb.x) < 12.0 && pb.z > -12.0 && pb.z < 75.0 && p.y < 8.0) {
        let pd = pier(p);
        if (pd.x < d.x) { d = vec3f(pd.x, pd.y, 0.0); }
    } else {
        d.x = min(d.x, max(abs(pb.x) - 10.0, 0.5));
    }
    if (p.z > SHORE + 8.0 && p.y < 7.0) {
        let bd = boats(p, t);
        if (bd.x < d.x) { d = bd; }
    }
    return d;
}

fn march(ro: vec3f, rd: vec3f, tmax: f32, n: i32, t: f32) -> vec4f {
    var s = 0.05;
    for (var i = 0; i < 160; i++) {
        if (i >= n) { break; }
        let p = ro + rd * s;
        if (p.y > 17.0 && rd.y > 0.0) { break; }
        if (p.z < SHORE - 60.0) { break; }
        let h = world(p, t);
        if (h.x < 0.0015 * s + 0.002) { return vec4f(s, h.y, h.z, 1.0); }
        s += h.x;
        if (s > tmax) { break; }
    }
    return vec4f(-1.0);
}

fn world_normal(p: vec3f, s: f32, t: f32) -> vec3f {
    let e = max(0.004, s * 0.0012);
    let k = vec2f(1.0, -1.0);
    return normalize(k.xyy * world(p + k.xyy * e, t).x + k.yyx * world(p + k.yyx * e, t).x +
                     k.yxy * world(p + k.yxy * e, t).x + k.xxx * world(p + k.xxx * e, t).x);
}

fn soft_shadow(p: vec3f, l: vec3f, t: f32) -> f32 {
    var res = 1.0;
    var s = 0.08;
    for (var i = 0; i < 28; i++) {
        let q = p + l * s;
        if (q.y > 17.0) { break; }
        let h = world(q, t).x;
        res = min(res, 10.0 * h / s);
        if (res < 0.02) { break; }
        s += clamp(h, 0.1, 6.0);
        if (s > 80.0) { break; }
    }
    return saturate(res);
}

// ------------------------------------------------------------ sky

fn backdrop(rd: vec3f, l: Look, ctx: Ctx, full: bool) -> vec3f {
    var c: vec3f;
    if (l.night > 1.5) {
        c = sky_night(rd) * 1.3;
        c += vec3f(0.015, 0.02, 0.035) * pow(saturate(dot(rd, l.sun)), 8.0);
        if (full) {
            c += star_field(rd, 0.4, ctx) * smoothstep(0.03, 0.25, rd.y) * 0.6;
            c += sky_moon(rd, l.sun, 0.55, 1.0);
        }
    } else if (l.night > 0.5) {
        let y = max(rd.y, 0.0);
        let toward = pow(saturate(dot(normalize(vec3f(rd.x, 0.0, rd.z) + vec3f(1e-4, 0.0, 0.0)), normalize(vec3f(l.sun.x, 0.0, l.sun.z))) * 0.5 + 0.5), 4.0);
        let hor = mix(vec3f(0.03, 0.036, 0.07), vec3f(0.2, 0.085, 0.045), toward);
        c = mix(hor, vec3f(0.003, 0.009, 0.036), pow(y, 0.4));
        c += vec3f(0.08, 0.035, 0.03) * toward * exp(-y * 14.0);
        if (full) { c += star_field(rd, 0.3, ctx) * smoothstep(0.2, 0.6, y) * 0.35; }
    } else {
        c = sky_atmosphere_haze(rd, l.sun, l.haze) * ozone(l.sun);
        if (full && l.night < 0.5) { c += sky_sun_disk(rd, l.sun, 0.55); }
    }
    // an altocumulus deck catching the low light
    if (rd.y > 0.0) {
        let hp = rd.xz / (rd.y + 0.05) * 2.2;
        let cs = cloud_sheet(hp * 0.7 + vec2f(1.0, 4.0), 0.5, ctx);
        let toward = pow(saturate(dot(normalize(vec3f(rd.x, 0.0, rd.z)), normalize(vec3f(l.sun.x, 0.0, l.sun.z))) * 0.5 + 0.5), 3.0);
        var lit = l.sun_c * (0.75 + 1.2 * toward) * mix(0.5, 1.1, cs.y) + l.amb * 0.7;
        if (l.night > 0.5 && l.night < 1.5) {
            lit = vec3f(0.05, 0.022, 0.03) * (0.25 + 1.6 * toward) + l.amb * 0.45;
        }
        if (l.night > 1.5) { lit = l.amb * 1.5 + l.sun_c * 0.25 * cs.y; }
        c = mix(c, lit, cs.x * 0.7 * smoothstep(0.0, 0.1, rd.y));
    }
    return c;
}

// the spruce ridge behind the cove, drawn in angle space (it is far enough
// that parallax between camera and water reflections is negligible)
fn ridge(rd: vec3f, l: Look) -> vec4f {
    let az = atan2(rd.x, -rd.z);
    let el = rd.y;
    let hill = 0.045 + 0.035 * noise_fbm2(vec2f(az * 2.0, 1.0), 4) + 0.015 * sin(az * 1.3 + 0.8);
    // spruce: irregular narrow spires in clumps, gaps here and there
    let k = az * 150.0;
    let cell = floor(k);
    var spire = 0.0;
    for (var j = -1; j <= 1; j++) {
        let c = cell + f32(j);
        let hh = hash_cell2(vec2i(i32(c), 9), 0x51ceu);
        let clump = noise_value2(vec2f(c * 0.08, 3.0));
        let hgt = (0.004 + 0.018 * hh.y * hh.y) * smoothstep(0.2, 0.6, clump + 0.3 * hh.z);
        let x = k - (c + 0.2 + 0.6 * hh.x);
        spire = max(spire, hgt * saturate(1.0 - abs(x) / (0.9 + 0.8 * hh.z)));
    }
    let top = hill + spire;
    if (el > top) { return vec4f(0.0); }
    let depth = saturate((top - el) / 0.03);
    var c = col_hex(0x16241au) * (l.amb * 0.9 + l.sun_c * 0.15 * saturate(l.sun.y * 4.0 + 0.3));
    c = mix(c, l.hor * 0.8, 0.28 + 0.2 * (1.0 - depth));
    return vec4f(c, 1.0);
}

// small gulls wheeling over the cove
fn gulls(p: vec2f, l: Look, ctx: Ctx) -> f32 {
    var a = 0.0;
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let ph = ctx.t * (0.05 + 0.02 * fi) + fi * 2.1;
        let c = vec2f(-0.2 + 0.35 * fi + 0.25 * sin(ph), 0.2 + 0.06 * fi + 0.05 * sin(ph * 1.7 + 1.0));
        let s = 0.012 - fi * 0.002;
        let q = (p - c) / s;
        let flap = 0.35 * sin(ctx.t * (5.0 + fi) + fi * 3.0);
        let wing = abs(q.x);
        let y = q.y - (0.5 * wing - 0.25 * wing * wing) * (1.0 + flap * 2.0) + flap * 0.2;
        let d = max(abs(y) - 0.12, wing - 1.0);
        a = max(a, saturate(0.5 - d * s / ctx.px));
    }
    return a;
}

// ------------------------------------------------------------ shading

fn shade(p: vec3f, rd: vec3f, s: f32, hit: vec4f, l: Look, ctx: Ctx, primary: bool) -> vec3f {
    let n = world_normal(p, s, ctx.t);
    let mat = hit.y;
    var alb = vec3f(0.5);
    var emit = vec3f(0.0);
    var rough = 0.8;
    if (mat < 1.5) {
        // shingled walls in a Nova Scotia palette, weathered and streaked
        let pal = hit.z;
        var wc = col_hex(0x8e3a2au);
        if (pal > 0.8) { wc = col_hex(0xd8d2c2u); } else if (pal > 0.62) { wc = col_hex(0x4e6676u); }
        else if (pal > 0.45) { wc = col_hex(0xc39a3cu); } else if (pal > 0.3) { wc = col_hex(0x3f5a47u); }
        let sh = 0.85 + 0.15 * smoothstep(0.2, 0.5, fract(p.y * 3.2)) * (0.8 + 0.4 * noise_value2(vec2f(p.x * 4.0 + p.z * 4.0, floor(p.y * 3.2))));
        alb = wc * sh * (0.8 + 0.3 * noise_value2(p.xz * 0.7 + p.y));
        // windows and doors on the faces
        let u = select(p.x, p.z, abs(n.x) > 0.5);
        let wu = fract(u * 0.33 + pal * 3.0);
        let win = step(0.42, wu) * step(wu, 0.62) * step(abs(p.y - 4.1 - pal), 0.55) * step(abs(n.y), 0.3);
        if (win > 0.5) {
            alb = vec3f(0.03, 0.035, 0.04);
            let on = step(0.35, fract(pal * 13.7 + floor(u * 0.33) * 0.37));
            emit = l.lamp_c * 1.2 * on * l.lamps;
        }
    } else if (mat < 2.5) {
        alb = col_hex(0x3a3634u) * (0.8 + 0.3 * noise_value2(vec2f(p.x * 2.0, p.y * 6.0)));
    } else if (mat < 3.5) {
        alb = col_hex(0x4a3f35u);
    } else if (mat < 4.5) {
        alb = col_hex(0x6d6153u) * (0.8 + 0.35 * noise_value2(vec2f(pier_frame(p).z * 4.0, 1.0)));
    } else if (mat < 5.5) {
        // pilings: grey above, black weed and barnacles below the tide line
        alb = mix(col_hex(0x1b1f17u), col_hex(0x5a5048u), smoothstep(0.4, 1.0, p.y));
    } else if (mat < 6.5) {
        alb = vec3f(0.05);
        let pb = pier_frame(p);
        if (pb.y > 5.4 && pb.x < 1.5 && n.y < -0.5) { emit = l.lamp_c * 6.0 * l.lamps; }
    } else if (mat < 7.5) {
        alb = mix(col_hex(0x5c6b4au), col_hex(0x7a3b2cu), step(0.5, fract(floor(p.y * 1.8) * 0.37 + floor(p.z) * 0.21)));
    } else if (mat < 8.5) {
        // granite with rockweed at the waterline, grass and scrub above
        let n1 = noise_fbm2(p.xz * 0.3, 4);
        alb = col_hex(0x6c6660u) * (0.7 + 0.4 * n1);
        alb = mix(alb, col_hex(0x3a3a1eu), smoothstep(0.6, 0.85, n.y) * smoothstep(3.5, 6.0, p.y));
        alb = mix(col_hex(0x2a2616u), alb, smoothstep(0.2, 1.2, p.y));
    } else {
        let b = boat_get(i32(hit.z + 0.5));
        let q = boat_local(p, b, i32(hit.z + 0.5), ctx.t);
        if (mat < 10.5) {
            alb = b.hull;
            rough = 0.35;
            if (q.y > 0.85) { alb = b.stripe; }
            if (q.y < 0.18) { alb = col_hex(0x7a2520u); }
        } else if (mat < 11.5) {
            alb = col_hex(0xe2ded4u);
            if (q.y > 2.15 && q.y < 2.7) {
                alb = vec3f(0.03, 0.04, 0.05);
                rough = 0.1;
                emit = l.lamp_c * 0.8 * l.lamps * step(0.5, fract(hit.z * 0.37 + 0.2));
            }
        } else {
            alb = vec3f(0.1);
        }
    }
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    if (primary && dif > 0.0 && l.night < 0.5) { sh = soft_shadow(p + n * 0.05, l.sun, ctx.t); }
    let sky = l.amb * (0.6 + 0.4 * n.y);
    // light off the water onto undersides
    let bounce = (l.amb * 0.3 + l.sun_c * max(l.sun.y, 0.0) * 0.1) * saturate(-n.y);
    var c = alb * (l.sun_c * dif * sh + sky + bounce);
    // glossy hulls pick up the sky
    if (rough < 0.5) {
        let r = reflect(rd, n);
        c += backdrop(r, l, ctx, false) * (0.04 + 0.2 * pow(1.0 - saturate(dot(-rd, n)), 5.0));
    }
    // lamp light pools at dusk and night
    if (l.lamps > 0.0) {
        let pb = pier_frame(p);
        let li = clamp(round((pb.z - 8.0) / 21.0), 0.0, 2.0);
        let lz = pb.z - 8.0 - li * 21.0;
        let lv = vec3f(1.25 - pb.x, 5.4 - pb.y, -lz);
        let d2 = dot(lv, lv);
        c += alb * l.lamp_c * 7.0 * l.lamps / (d2 + 2.0) * saturate(0.3 + 0.7 * max(n.y, 0.0));
    }
    return c + emit;
}

// ------------------------------------------------------------ water

// slow ripples with analytic slopes; unresolved ones become roughness
fn ripples(xz: vec2f, t: f32, foot: f32) -> vec4f {
    var g = vec2f(0.0);
    var vu = 0.0;
    let q = xz + vec2f(noise_value2(xz * 0.05), noise_value2(xz * 0.05 + 3.1)) * 6.0;
    var ang = 1.1;
    var k = 0.6;
    var a = 0.02;
    for (var i = 0; i < 6; i++) {
        let dir = vec2f(cos(ang), sin(ang));
        let ph = dot(q, dir) * k - sqrt(9.81 * k) * t * 0.35 + f32(i) * 2.3;
        let s = a * k;
        let res = saturate(1.6 - foot * k * 0.5);
        g += dir * (s * cos(ph) * res);
        vu += 0.5 * s * s * (1.0 - res * res);
        ang += 2.2;
        k *= 1.6;
        a *= 0.6;
    }
    return vec4f(normalize(vec3f(-g.x, 1.0, -g.y)), sqrt(vu));
}

fn mist_patch(xz: vec2f, t: f32) -> f32 {
    return smoothstep(0.3, 0.75, noise_fbm2(xz * vec2f(0.02, 0.05) + vec2f(t * 0.012, 0.0), 4));
}

// low mist on the water: exponential in height, patchy, integrated in closed
// form along the ray; returns (in-scatter rgb, transmittance)
fn mist(ro: vec3f, rd: vec3f, s: f32, l: Look, ctx: Ctx) -> vec4f {
    let hsc = 1.3;
    let dist = min(s, 400.0);
    var amount: f32;
    if (abs(rd.y) < 1e-4) {
        amount = exp(-ro.y / hsc) * dist;
    } else {
        amount = exp(-ro.y / hsc) * (1.0 - exp(-rd.y * dist / hsc)) * hsc / rd.y;
    }
    // patchiness where the ray runs lowest over the water
    let tlow = select(dist, clamp((ro.y - 1.0) / max(-rd.y, 1e-3), 0.0, dist), rd.y < 0.0);
    let pw = ro + rd * min(tlow, dist);
    let patchy = 0.15 + 1.4 * mist_patch(pw.xz, ctx.t) * smoothstep(15.0, 55.0, -pw.z);
    let tr = exp(-amount * 0.016 * l.mist * patchy);
    let mu = dot(rd, l.sun);
    let fwd = 0.25 + 1.8 * pow(saturate(mu), 6.0);
    var mc = l.amb * 1.1 + l.sun_c * fwd * 0.5;
    if (l.night > 0.5) { mc = l.amb * 1.3 + l.lamp_c * 0.02 * l.lamps; }
    return vec4f(mc * (1.0 - tr), tr);
}

fn trace_view(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx, primary: bool) -> vec4f {
    let n = select(56, 96, primary);
    let h = march(ro, rd, 400.0, n, ctx.t);
    if (h.w > 0.0) {
        return vec4f(shade(ro + rd * h.x, rd, h.x, h, l, ctx, primary), h.x);
    }
    var c = backdrop(rd, l, ctx, primary);
    let rg = ridge(rd, l);
    c = mix(c, rg.rgb, rg.a);
    return vec4f(c, 1e4);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let cam = cam_look_at(CAM, CAM + vec3f(0.0, 0.03, -1.0), 0.0, 50.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    var s = 1e4;
    let tw = select(-1.0, -CAM.y / rd.y, rd.y < -1e-4);
    let prim = march(CAM, rd, select(400.0, tw, tw > 0.0), steps(110.0, ctx), ctx.t);
    if (prim.w > 0.0) {
        s = prim.x;
        col = shade(CAM + rd * s, rd, s, prim, l, ctx, true);
    } else if (tw > 0.0) {
        s = tw;
        let wp = CAM + rd * tw;
        let foot = tw * ctx.px / sqrt(max(-rd.y, 0.02));
        let rn = ripples(wp.xz, ctx.t, foot);
        var r = reflect(rd, rn.xyz);
        r.y = abs(r.y) + rn.w * 0.4 + 0.001;
        r = normalize(r);
        let refl = trace_view(wp + vec3f(0.0, 0.02, 0.0), r, l, ctx, false);
        var rc = refl.rgb;
        // mist seen in the reflection
        let rm = mist(wp, r, refl.w, l, ctx);
        rc = rc * rm.w + rm.rgb;
        let f = saturate(0.02 + 0.98 * pow(1.0 - saturate(dot(-rd, rn.xyz) + rn.w), 5.0));
        let body = vec3f(0.012, 0.022, 0.02) * (l.amb * 4.0 + l.sun_c * 0.1);
        col = mix(body, rc, max(f, 0.35));
        // lamp and window streaks come through the ripples on their own
    } else {
        let v = trace_view(CAM, rd, l, ctx, true);
        col = v.rgb;
    }
    let m = mist(CAM, rd, s, l, ctx);
    col = col * m.w + m.rgb;
    let g = gulls(p, l, ctx);
    var gc = vec3f(0.85, 0.85, 0.8) * (l.amb * 1.4 + l.sun_c * 0.5);
    if (l.night > 0.5) { gc = l.amb * 0.4; }
    if (l.night < 1.5) { col = mix(col, gc, g); }
    return col * exp2(l.exposure);
}
