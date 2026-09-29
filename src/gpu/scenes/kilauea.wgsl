//! name: kilauea
//! title: Kilauea Lava
//! category: wilds
//! tags: volcano, lava, ocean, steam, hawaii, night
//! desc: a lava channel crossing black pahoehoe to the sea, the steam plume glowing above the surf
//! themes: night, dusk, eruption
//! uses: camera, sky, stars, water, fire
//! cost: medium
//! fallback: lava
//! credits: original

// World units are metres, sea level y = 0. We stand on a tumulus of older
// pahoehoe a few hundred metres inland, looking down the coast: the Pacific
// on the left, the flow field on the right. A channel of molten rock comes
// past us on the right and winds away to the new lava delta, where it pours
// over the sea cliff into the surf and a column of steam boils up, lit
// orange from below. Fresh toes of lava break out of the crust nearby.

struct Look {
    moon: vec3f,
    moon_c: vec3f,    // direct moon / twilight light
    amb: vec3f,       // sky ambient
    kind: u32,        // 0 night, 1 dusk, 2 eruption
    lava: f32,        // glow intensity
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(normalize(vec3f(-0.75, 0.16, -0.64)), vec3f(0.05, 0.06, 0.1), vec3f(0.018, 0.028, 0.06), 1u, 0.8, 0.1);
        }
        case 2u: {
            return Look(normalize(vec3f(0.5, 0.45, -0.6)), vec3f(0.012, 0.014, 0.02), vec3f(0.003, 0.003, 0.005), 2u, 1.25, -1.0);
        }
        default: {
            return Look(normalize(vec3f(0.5, 0.45, -0.6)), vec3f(0.014, 0.017, 0.026), vec3f(0.0025, 0.003, 0.006), 0u, 1.0, -0.8);
        }
    }
}

// ------------------------------------------------------------------ layout

const EYE: vec3f = vec3f(0.0, 20.0, 0.0);
const ENTRY: vec3f = vec3f(-150.0, 0.0, -430.0);
const LAND_TOP: f32 = 12.0;
// the way the older flows ran to the sea (their ropes fold across it)
const FLOW: vec2f = vec2f(-0.371, -0.928);

// the coastline: land for x > coast_x(z); the delta bulges out at the entry
fn coast_x(z: f32) -> f32 {
    let u = z - ENTRY.z;
    let far = 1.0 - exp(-u * u / 6400.0);
    return ENTRY.x + 0.2 * u + (25.0 + 7.0 * sin(z * 0.021 + 0.7) + 4.0 * sin(z * 0.063 + 2.0)) * far;
}

// the channel's centreline x(z): it comes past us on the right and runs
// away, meandering a little, to the entry
fn chan_x(z: f32) -> f32 {
    let u = z - ENTRY.z;
    // (its last reach swings round to pour straight off the delta)
    return ENTRY.x + 0.46 * u + 12.0 * sin(u * 0.021) + 4.0 * sin(u * 0.052) + 16.0 * (1.0 - exp(-max(u, 0.0) / 22.0));
}
fn chan_slope(z: f32) -> f32 {
    let u = z - ENTRY.z;
    return 0.46 + 0.252 * cos(u * 0.021) + 0.208 * cos(u * 0.052) + 16.0 / 22.0 * exp(-max(u, 0.0) / 22.0);
}

// (distance from the centreline, signed offset across it)
fn chan(xz: vec2f) -> vec2f {
    if (xz.y < ENTRY.z - 4.0) { return vec2f(1e4, 1e4); }
    let s = chan_slope(xz.y);
    let d = (xz.x - chan_x(xz.y)) * inverseSqrt(1.0 + s * s);
    return vec2f(abs(d), d);
}

fn land_h(xz: vec2f) -> f32 {
    let off = xz.x - coast_x(xz.y);
    var h = 5.5 + 3.5 * (1.0 - exp(-max(off, 0.0) / 220.0));
    // inflated sheets and tumuli of older pahoehoe
    h += 1.8 * noise_value2(xz * 0.021 + 1.7) + 0.7 * noise_value2(xz * 0.075 + 3.0) - 1.2;
    // the channel runs between its levees, the lava a metre down
    let c = chan(xz).x;
    h += 1.1 * exp(-sq((c - 7.5) / 3.5)) - 1.5 * sstep(6.5, 4.0, c);
    // a low sea cliff
    return min(h, off * 1.3 - 0.8);
}

// ------------------------------------------------------------------ lava

// nearest two points of a jittered grid: (F1, F2, random id of the nearest)
fn toe_cells(p: vec2f) -> vec3f {
    let i = vec2i(floor(p));
    let f = fract(p);
    var d1 = 8.0;
    var d2 = 8.0;
    var id = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2i(x, y);
            let h = hash_cell2(i + o, 0x51f3u);
            let r = vec2f(o) + 0.15 + 0.7 * h.xy - f;
            let dd = dot(r, r);
            if (dd < d1) { d2 = d1; d1 = dd; id = h.z; } else if (dd < d2) { d2 = dd; }
        }
    }
    return vec3f(sqrt(d1), sqrt(d2), id);
}

// pahoehoe lobes: every cell of a jittered grid inflates a dome (a
// paraboloid of random size); the ground is the highest dome, so lobes are
// rounded on top and meet in sharp cracks. Returns the winning dome's height
// gradient (cell units), the gap to the runner-up (0 in a crack), the vector
// to its centre (for ropes) and a random id (for colour)
struct Lobe { f1: f32, e: f32, g: vec2f, r1: vec2f, id: f32 }

fn lobes(p: vec2f) -> Lobe {
    let i = vec2i(floor(p));
    let f = fract(p);
    var v1 = -8.0;
    var v2 = -8.0;
    var r1 = vec2f(0.0);
    var id = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2i(x, y);
            let h = hash_cell2(i + o, 0x9a17u);
            let r = vec2f(o) + 0.1 + 0.8 * h.xy - f;
            let rr = 0.55 + 0.35 * h.w;
            let v = rr * rr - dot(r, r);
            if (v > v1) { v2 = v1; v1 = v; r1 = r; id = h.z; } else if (v > v2) { v2 = v; }
        }
    }
    var lb: Lobe;
    lb.f1 = length(r1);
    lb.e = v1 - v2;
    lb.g = 2.0 * r1;
    lb.r1 = r1;
    lb.id = id;
    return lb;
}

struct Lava { e: vec3f, heat: f32 }

// breakouts: fresh pahoehoe toes budding from the crust (xz centre, radius)
const BRK0: vec3f = vec3f(-15.0, -58.0, 10.0);
const BRK1: vec3f = vec3f(38.0, -190.0, 18.0);
const BRK2: vec3f = vec3f(-64.0, -128.0, 13.0);

fn breakout(xz: vec2f, b: vec3f, t: f32, fp: f32) -> f32 {
    let rel = (xz - b.xy) / b.z;
    let d = length(rel) + (noise_value2(xz * 0.09 + b.xy) - 0.5) * 0.7;
    if (d > 1.1) { return 0.0; }
    let inside = sstep(1.05, 0.8, d);
    // toes about two metres across; each one swells, glows and skins over
    // on its own slow clock. Most live ones push out on the downhill front.
    let front = saturate(dot(rel, vec2f(-0.6, -0.8)) / max(length(rel), 1e-3) * 0.5 + 0.5);
    let tc = toe_cells(xz * 0.26 + b.xy);
    let edge = tc.y - tc.x;
    let busy = step(0.62 - 0.5 * front * sstep(0.2, 0.9, d), tc.z);
    let age = fract(t / 64.0 + tc.z * 5.37);
    let glow = busy * sstep(0.0, 0.05, age) * sstep(0.95, 0.35, age);
    // a rounded toe: bright molten middle, its margin chilled to a dark rind
    let blob = sstep(0.72, 0.36, tc.x) * sstep(0.0, 0.08, edge);
    let core = sstep(0.5, 0.1, tc.x);
    var heat = glow * blob * mix(0.36, 0.73, core);
    // as it ages a skin wrinkles over the toe, split by glowing cracks
    let cr = noise_worley2(xz * 1.3 + vec2f(tc.z * 17.0, 0.0));
    let cw = max(0.07, fp * 0.8);
    let crack = sstep(cw, 0.0, cr.y - cr.x) * min(1.0, 0.1 / cw);
    heat *= mix(1.0, 0.3 + 0.55 * crack, sstep(0.3, 0.75, age));
    // the whole lobe is still hot under its skin: dull red in the seams
    // between the toes and through the cracks
    let warm = (0.14 + 0.16 * crack + 0.1 * sstep(0.12, 0.0, edge)) * sstep(1.0, 0.35, d) * smoothstep(0.25, 0.65, noise_value2(xz * 0.3 + b.xy));
    heat = max(heat, warm);
    // the advancing margin: an incandescent seam where the new lobe spills
    // out of its crust, broken where it has skinned over
    let seam = exp(-sq((d - 0.93) / 0.07)) * front * smoothstep(0.3, 0.6, noise_value2(xz * 0.35 + vec2f(t * 0.01, b.x)));
    heat = max(heat, 0.66 * seam);
    return heat * inside;
}

fn lava_at(xz: vec2f, t: f32, fp: f32, l: Look) -> Lava {
    var r: Lava;
    r.e = vec3f(0.0);
    r.heat = 0.0;
    let off = xz.x - coast_x(xz.y);
    let c = chan(xz);
    // the channel: molten rock running to the sea under drifting plates of
    // crust; the banks are frozen crust, cracked and dull red
    if (c.x < 8.0 && off > -2.0) {
        let across = c.x / 5.8;
        // plates of crust ride the current, torn apart along hot seams;
        // some reaches run open, others are nearly crusted over
        let sflow = xz.y + t * 1.3;
        let tc = toe_cells(vec2f(sflow * 0.15, c.y * 0.45));
        let seam = sstep(0.2, 0.02, tc.y - tc.x);
        let open = 0.55 + 0.4 * smoothstep(0.3, 0.7, noise_value2(vec2f(xz.y * 0.02, 5.0)));
        let plate = step(open, tc.z) * (1.0 - seam) * sstep(0.8, 0.5, across);
        let streak = noise_value2(vec2f(sflow * 0.06, c.y * 1.1));
        // molten, hottest mid-stream; plates and torn scraps of darker
        // crust drift on it, drawn out into streaks by the shear
        let scrap = smoothstep(0.58, 0.72, noise_value2(vec2f(sflow * 0.35, c.y * 1.6))) * sstep(0.2, 0.7, across);
        var heat = 0.76 + 0.06 * streak - 0.12 * across * across;
        heat = mix(heat, 0.5 + 0.06 * streak, max(plate, scrap * 0.8));
        // the banks: crust frozen to the levees, dull red, a few hot cracks
        let crk = sstep(0.1, 0.0, abs(noise_grad2(vec2f(xz.y * 0.22, c.y * 0.6))));
        heat = mix(heat, 0.26 + 0.24 * crk, sstep(0.72, 0.98, across));
        heat *= sstep(1.2, 0.98, across);
        r.heat = heat;
        r.e = fire_temperature_color(heat);
    }
    let bk = max(breakout(xz, BRK0, t, fp), max(breakout(xz, BRK1, t, fp), breakout(xz, BRK2, t, fp)));
    if (bk > 0.0) {
        r.heat = max(r.heat, bk);
        r.e = max(r.e, fire_temperature_color(bk));
    }
    // scattered breakouts glowing far out across the flow field
    let cell = floor(xz / 70.0);
    let h = hash_cell2(vec2i(cell), 0x77a1u);
    if (h.w < 0.2 && off > 25.0 && xz.y < -120.0) {
        let cp = (cell + 0.2 + 0.6 * h.xy) * 70.0;
        let dd = length((xz - cp) * vec2f(1.0, 1.6)) / (2.0 + 5.0 * h.z) + (noise_value2(xz * 0.4) - 0.5) * 0.8;
        let s = sstep(1.0, 0.2, dd) * (0.55 + 0.35 * h.z);
        r.heat = max(r.heat, s);
        r.e = max(r.e, fire_temperature_color(s));
    }
    // where the lava pours over the cliff into the sea
    let de = length(xz - ENTRY.xz);
    let pour = exp(-de * de / 160.0) * smoothstep(-5.0, 2.0, off);
    r.heat = max(r.heat, pour);
    r.e += fire_temperature_color(0.85) * pour;
    r.e *= l.lava;
    return r;
}

// light the lava throws on a surface point: the channel's nearest reach,
// the breakouts, and the entry with the lit foot of the plume
fn lava_light(p: vec3f, n: vec3f, l: Look) -> vec3f {
    var e = vec3f(0.0);
    let hot = fire_temperature_color(0.7);
    let c = chan(p.xz);
    if (c.x < 400.0) {
        let s = chan_slope(p.z);
        let across = vec2f(1.0, -s) * inverseSqrt(1.0 + s * s);
        let to = vec3f(-across.x * c.y, 2.5, -across.y * c.y);
        let dif = 0.3 + 0.7 * saturate(dot(n, normalize(to)));
        e += hot * 1.3 * dif / (1.0 + sq(c.x / 10.0));
    }
    for (var i = 0; i < 3; i++) {
        var b = BRK0;
        if (i == 1) { b = BRK1; }
        if (i == 2) { b = BRK2; }
        let to = vec3f(b.x - p.x, 1.5, b.y - p.z);
        let d = max(length(to.xz) - b.z * 0.5, 0.0);
        let dif = 0.3 + 0.7 * saturate(dot(n, normalize(to)));
        e += hot * 0.8 * dif / (1.0 + sq(d / (b.z * 0.8)));
    }
    let toe = ENTRY + vec3f(0.0, 25.0, 0.0) - p;
    let de2 = dot(toe, toe);
    e += hot * (0.2 + 0.8 * saturate(dot(n, normalize(toe)))) * 3.0 / (1.0 + de2 / 1600.0);
    return e * l.lava;
}

// ------------------------------------------------------------------ steam

const PLUME_TOP: f32 = 360.0;

// the plume axis bends away downwind with height and sways slowly
fn plume_axis(y: f32, t: f32) -> vec2f {
    let lean = vec2f(-0.3, 0.1) * y * (0.5 + 0.5 * smoothstep(0.0, 200.0, y));
    let sway = vec2f(10.0 * sin(y * 0.014 - t * 0.05), 7.0 * sin(y * 0.011 + 1.0 - t * 0.04)) * smoothstep(0.0, 80.0, y);
    return ENTRY.xz + lean + sway;
}

// (x: density; y: the billows' reach here, low in the creases between them)
fn plume_dens(p: vec3f, t: f32) -> vec2f {
    if (p.y < -2.0 || p.y > PLUME_TOP) { return vec2f(0.0); }
    let y = max(p.y, 0.0);
    let rad = 18.0 + 0.38 * y;
    let d = p.xz - plume_axis(y, t);
    let r = length(d) / rad;
    if (r > 1.7) { return vec2f(0.0); }
    // self-similar cauliflower: the lobes grow as they climb (log height)
    // and roll slowly upward; |noise| gives round heads with sharp creases
    let q = vec3f(d.x / rad * 1.25, log(rad) * 3.3 - t * 0.04, d.y / rad * 1.25);
    let b0 = noise_grad3(q * 0.45 + vec3f(7.7, 0.0, 2.2));
    let b1 = abs(noise_grad3(q));
    let b2 = abs(noise_grad3(q * 2.3 + vec3f(3.1, 1.7, 5.3)));
    let bil = b1 * 0.72 + b2 * 0.28 + b0 * 0.55;
    // it thins and frays as it climbs, evaporating into the dry air
    let fade = sstep(PLUME_TOP, PLUME_TOP * 0.3, y) * smoothstep(-2.0, 4.0, p.y);
    let body = sstep(1.0, 0.82, r - 0.62 * bil + 0.1 + (1.0 - fade) * 0.5);
    return vec2f(body * fade, b1 * 0.72 + b2 * 0.28);
}

// march the plume; (radiance, transmittance)
fn steam(ro: vec3f, rd: vec3f, tmax: f32, t: f32, jit: f32, l: Look, n: i32) -> vec4f {
    // bound: a vertical cylinder around the leaning column
    let cen = plume_axis(PLUME_TOP * 0.45, t);
    let oc = ro.xz - cen;
    let d2 = rd.xz;
    let a = max(dot(d2, d2), 1e-6);
    let hb = dot(oc, d2);
    let rb = 150.0;
    let disc = hb * hb - a * (dot(oc, oc) - rb * rb);
    if (disc <= 0.0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let sq_ = sqrt(disc);
    let t0 = max((-hb - sq_) / a, 0.0);
    var t1 = min((-hb + sq_) / a, tmax);
    if (rd.y > 0.0) { t1 = min(t1, (PLUME_TOP - ro.y) / rd.y); }
    if (t1 <= t0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let dt = (t1 - t0) / f32(n);
    var tt = t0 + dt * jit;
    var tr = 1.0;
    var acc = vec3f(0.0);
    let glow_lo = fire_temperature_color(0.74) * l.lava;
    let glow_hi = fire_temperature_color(0.6) * 1.4 * l.lava;
    for (var i = 0; i < 48; i++) {
        if (i >= n || tr < 0.02) { break; }
        let p = ro + rd * tt;
        let pd = plume_dens(p, t);
        let d = pd.x;
        if (d > 0.01) {
            let y = max(p.y, 0.0);
            // lit from below by the entry: a lobe glows on the side facing
            // down toward it and falls dark on its crown
            let ve = ENTRY + vec3f(0.0, -4.0, 0.0) - p;
            let de2 = dot(ve, ve);
            let dl = plume_dens(p + ve * inverseSqrt(de2) * (5.0 + 0.1 * y), t).x;
            let facing = saturate((d - dl) * 2.0 + 0.15 + 0.6 * exp(-y / 18.0));
            let reach = 1.2 / (1.0 + de2 / 700.0) + 0.012 * exp(-y / 200.0);
            // the light reddens as it climbs through the steam; the creases
            // between billows stay dark
            let crease = mix(0.35, 1.0, smoothstep(0.08, 0.45, pd.y));
            let lit = mix(glow_lo, glow_hi, smoothstep(8.0, 110.0, y)) * reach * facing * crease;
            let top = smoothstep(30.0, 300.0, y);
            let sky = (l.amb * (1.2 + 1.5 * top) + l.moon_c * 0.4 * top * (1.2 - facing)) * crease;
            let ext = d * 0.07;
            let st = exp(-ext * dt);
            acc += tr * (lit + sky) * (1.0 - st);
            tr *= st;
        }
        tt += dt;
    }
    // stopped because the steam went opaque: no stars through its core
    if (tr < 0.04) {
        acc /= max(1.0 - tr, 1e-3);
        tr = 0.0;
    }
    return vec4f(acc, tr);
}

// ------------------------------------------------------------------ sky

// the shield of Kilauea behind the flow field: elevation of its skyline
fn shield(az: f32) -> f32 {
    let rise = smoothstep(-0.3, 0.9, az);
    return -0.002 + 0.05 * rise + 0.01 * smoothstep(0.12, 0.22, az) + 0.003 * noise_value2(vec2f(az * 30.0, 2.0)) * rise;
}

fn sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let y = max(rd.y, 0.0);
    var c: vec3f;
    if (l.kind == 1u) {
        // blue hour: deep blue overhead, a last violet-rose band low in the west
        let west = saturate(dot(normalize(rd.xz + vec2f(1e-4)), normalize(l.moon.xz)) * 0.5 + 0.5);
        c = mix(col_hex(0x4a6aa8u) * 0.11, col_hex(0x0e2250u) * 0.06, pow(y, 0.35));
        c += col_hex(0xd08a7au) * 0.05 * exp(-y * 10.0) * west * west;
        c += star_field(rd, 0.4, ctx) * 0.35 * smoothstep(0.08, 0.4, y);
    } else {
        c = sky_night(rd) * 1.2;
        c += star_field(rd, 0.5, ctx) * 0.7 * smoothstep(0.02, 0.15, y);
        c += star_milky_way(rd, normalize(vec3f(-0.35, 0.5, 0.79)), normalize(vec3f(0.5, 0.25, -0.83)), ctx);
    }
    // the sky over the volcano glows with the lava and the vog
    let az = atan2(rd.x, -rd.z);
    let lava_sky = fire_temperature_color(0.62) * 0.01 * l.lava * exp(-y * 7.0) * (0.4 + smoothstep(-0.4, 0.8, az));
    c += lava_sky * select(select(1.0, 0.15, l.kind == 1u), 2.5, l.kind == 2u);
    return c;
}

// the far shield with lava rivulets down its flanks; an eruption adds a
// fountain and its ash column. Returns (colour, coverage)
fn backdrop(rd: vec3f, l: Look, pxa: f32, t: f32) -> vec4f {
    let az = atan2(rd.x, -rd.z);
    let top = shield(az);
    var cov = saturate((top - rd.y) / max(pxa, 1e-5) + 0.5);
    var col = vec3f(0.0012, 0.0013, 0.0017) + l.amb * 0.2;
    if (l.kind == 1u) { col += vec3f(0.003, 0.005, 0.011) * sstep(0.0, top, rd.y); }
    // a few rivulets of lava slanting down from the rim of the pali
    let w = max(pxa * 0.6, 0.0006);
    var riv = 0.0;
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let h = hash_f(u32(i) * 747u + 11u);
        let ai = 0.05 + 0.09 * fi + 0.05 * h;
        if (hash_f(u32(i) * 91u + 5u) < 0.3) { continue; }
        let ti = shield(ai) - 0.002;
        let dy = ti - rd.y;
        if (dy < 0.0 || dy > 0.04) { continue; }
        let x = az - ai + dy * 1.4 - 0.001 * sin(dy * 260.0 + fi * 2.0);
        let seg = smoothstep(0.35, 0.6, noise_value2(vec2f(dy * 140.0, fi * 7.0)));
        riv += (0.2 + 0.5 * h) * seg * exp(-x * x / (w * w)) * sstep(0.04, 0.028, dy);
    }
    col += fire_temperature_color(0.62) * riv * 0.9 * l.lava * select(1.0, 1.6, l.kind == 2u);
    var out = col * cov;
    if (l.kind == 2u) {
        // lava fountain on the flank
        let fa = 0.3;
        let fx = az - fa;
        let fy = rd.y - (shield(fa) - 0.002);
        let ww = max(pxa * 0.9, 0.0018);
        let jet = exp(-fx * fx / (ww * ww * (1.0 + max(fy, 0.0) * 500.0))) * sstep(0.05, 0.01, fy) * smoothstep(-0.003, 0.0, fy);
        out += fire_temperature_color(0.72 + 0.12 * sstep(0.05, 0.0, fy)) * jet * 4.0;
        // its glow on the flank and in the air around it
        out += fire_temperature_color(0.6) * 0.06 * exp(-(fx * fx + fy * fy) / 0.0009);
        // ash and gas column leaning away, lit red from below
        let hgt = max(fy, 0.0);
        let cx = fx - hgt * 0.4 - 0.01 * sin(hgt * 25.0 - t * 0.04);
        let wid = 0.004 + hgt * 0.4;
        let nz = noise_fbm2(vec2f(cx * 60.0, hgt * 40.0 - t * 0.03), 4);
        let ash = sstep(1.0, 0.35, abs(cx) / wid + 0.8 * (nz - 0.5));
        let ashd = ash * smoothstep(0.0, 0.008, fy) * sstep(0.45, 0.15, hgt);
        let ashc = mix(vec3f(0.0025, 0.0022, 0.0022), fire_temperature_color(0.6) * 0.06, exp(-hgt * 18.0));
        out = mix(out, ashc, ashd * 0.95);
        cov = max(cov, ashd);
    }
    return vec4f(out, cov);
}

// ------------------------------------------------------------------ surfaces

// march the land; (hit distance or -1, glow of the fume over the channel
// gathered on the way: seen from low down the levees hide the far channel,
// but not the orange haze of fume and steam that hangs over it)
fn trace_land(ro: vec3f, rd: vec3f, tmax: f32) -> vec2f {
    if (rd.y > -1e-4) { return vec2f(-1.0, 0.0); }
    // start where the ray drops below the highest land
    var t = max((LAND_TOP - ro.y) / rd.y, 0.5);
    var tp = t;
    var hp = 1.0;
    var fume = 0.0;
    for (var i = 0; i < 120; i++) {
        let p = ro + rd * t;
        let h = p.y - land_h(p.xz);
        let cd = chan(p.xz).x;
        fume += exp(-cd * cd / 45.0 - max(h, 0.0) / 3.0) * (t - tp);
        if (h < 0.0) {
            if (i == 0) { return vec2f(t, fume); }
            // refine the crossing (regula falsi)
            var a = tp;
            var b = t;
            var ha = hp;
            var hb = h;
            for (var k = 0; k < 4; k++) {
                let m = a + (b - a) * ha / (ha - hb);
                let q = ro + rd * m;
                let hm = q.y - land_h(q.xz);
                if (hm < 0.0) { b = m; hb = hm; } else { a = m; ha = hm; }
            }
            return vec2f(a + (b - a) * ha / (ha - hb), fume);
        }
        // past the sea surface: nothing more to find
        if (t >= tmax) { break; }
        tp = t;
        hp = h;
        // the ground is gentle (slopes under ~0.5): the vertical gap converts
        // to a safe step along the ray; far away, grow with distance. The
        // last step lands exactly on the sea plane so no shore is skipped.
        t = min(t + max(h / (0.45 - rd.y) * 0.9, 0.02 + t * 0.01), tmax);
    }
    return vec2f(-1.0, fume);
}

// what the glassy crust mirrors: the sky, the lit plume, the channel
fn sheen_env(r: vec3f, p: vec3f, l: Look) -> vec3f {
    let hot = fire_temperature_color(0.7) * l.lava;
    var e = l.amb * (0.6 + 0.6 * saturate(r.y));
    let to_s = normalize(ENTRY + vec3f(-12.0, 60.0, 4.0) - p);
    e += hot * (0.06 * pow(saturate(dot(r, to_s)), 30.0) + 0.5 * pow(saturate(dot(r, to_s)), 200.0));
    let c = chan(p.xz);
    if (c.x < 60.0) {
        let s = chan_slope(p.z);
        let across = vec2f(1.0, -s) * inverseSqrt(1.0 + s * s);
        let to = normalize(vec3f(-across.x * c.y, 1.0, -across.y * c.y));
        e += hot * 0.6 * pow(saturate(dot(r, to)), 16.0) / (1.0 + sq(c.x / 8.0));
    }
    e += l.moon_c * 6.0 * pow(saturate(dot(r, l.moon)), 60.0);
    return e;
}

fn shade_land(p: vec3f, rd: vec3f, t: f32, pxa: f32, l: Look, ctx: Ctx) -> vec3f {
    let fp = t * pxa;
    let e = max(0.06, fp);
    let hx = land_h(p.xz + vec2f(e, 0.0)) - land_h(p.xz - vec2f(e, 0.0));
    let hz = land_h(p.xz + vec2f(0.0, e)) - land_h(p.xz - vec2f(0.0, e));
    var n = normalize(vec3f(-hx, 2.0 * e, -hz));
    // pahoehoe: domed lobes a few metres across split by dark cracks, ropy
    // folds wrinkling each one in arcs around where it budded
    var crack = 1.0;
    var shine = 1.0;
    var tone = 0.5;
    let lk = saturate(1.0 - fp / 1.8);
    if (lk > 0.0) {
        let s1 = 0.2;
        let wq = vec2f(noise_value2(p.xz * 0.06), noise_value2(p.xz * 0.06 + 5.3)) - 0.5;
        let lb = lobes(p.xz * s1 + wq * 2.2 + vec2f(3.7, 1.1));
        // each lobe inflates into a dome, rounded on top, steep at its margin
        // smooth sheets in places, lumpy toes and lobes in others
        let lumpy = smoothstep(0.2, 0.65, noise_value2(p.xz * 0.035 + 2.0));
        let hgt = 1.3 * (0.3 + lb.id) * (0.25 + 0.75 * lumpy);
        var g = lb.g * s1 * hgt;
        let rk = saturate(1.0 - fp / 0.22);
        if (rk > 0.0) {
            let rp = lb.f1 * 42.0 + noise_value2(p.xz * 0.9) * 2.5;
            g += -lb.r1 / max(lb.f1, 1e-3) * s1 * cos(rp) * 42.0 * 0.014 * rk * sstep(0.02, 0.12, lb.e);
        }
        n = normalize(n - vec3f(g.x, 0.0, g.y) * lk);
        crack = mix(1.0, 0.45 + 0.55 * sstep(0.0, 0.04, lb.e), lk * saturate(1.3 - fp / 1.2) * (0.3 + 0.7 * lumpy));
        shine = mix(1.0, 0.2 + 0.65 * lb.id, lk);
        tone = mix(0.5, lb.id, lk);
    }
    // older, larger inflation lobes and tumuli further out
    let bk2 = saturate(1.0 - fp / 6.0) * (1.0 - lk * 0.5);
    if (bk2 > 0.0) {
        let lb2 = lobes(p.xz * 0.07 + vec2f(9.1, 4.3));
        let g2 = lb2.g * 0.07 * 2.6;
        n = normalize(n - vec3f(g2.x, 0.0, g2.y) * bk2);
        crack *= mix(1.0, sstep(0.0, 0.06, lb2.e), bk2 * 0.7);
    }
    let lv = lava_at(p.xz, ctx.t, fp, l);
    // glassy black basalt: dark diffuse, a silvery sheen at grazing angles
    let alb = mix(col_hex(0x1c1c1eu), col_hex(0x3a3633u), tone * 0.7 + 0.3 * noise_value2(p.xz * 0.07)) * crack;
    let ll = lava_light(p, n, l);
    let dif = saturate(dot(n, l.moon));
    var c = alb * (l.moon_c * dif + l.amb * (0.6 + 0.4 * n.y) + ll);
    // glassy crust, but lumpy: at grazing angles and from afar the bumps
    // mask and average the mirror away
    let fres = 0.04 + 0.4 * pow(1.0 - saturate(dot(n, -rd)), 5.0);
    let gloss = 1.0 / (1.0 + fp * 1.5);
    let r = reflect(rd, n);
    let env = mix(l.amb * 0.8, sheen_env(r, p, l), gloss);
    c += fres * env * shine * crack * (1.0 - saturate(lv.heat * 2.0));
    return c + lv.e;
}

fn shade_sea(p: vec3f, rd: vec3f, t: f32, pxa: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = water_normal(p.xz, ctx.t, 0.7, t, ctx);
    let r = reflect(rd, n);
    let fres = water_fresnel(dot(-rd, n));
    let rr = normalize(vec3f(r.x, abs(r.y) + 0.005, r.z));
    var refl = sky(rr, l, ctx) * 0.8;
    // the glow of the entry and the lit column of steam, reflected as a
    // broken path of light running toward us
    let to_e = normalize(ENTRY + vec3f(0.0, 8.0, 0.0) - p);
    let glow = fire_temperature_color(0.72) * l.lava;
    let de_ = ENTRY.xz - p.xz;
    let daz = atan2(r.x, -r.z) - atan2(de_.x, -de_.y);
    let path = exp(-sq((daz + 0.05) / 0.1)) * saturate(r.y * 30.0 + 0.3) * exp(-max(r.y, 0.0) * 4.0);
    refl += glow * (pow(saturate(dot(r, to_e)), 200.0) * 12.0 + path * 0.6);
    refl += l.moon_c * pow(saturate(dot(r, l.moon)), 300.0) * 25.0;
    var c = mix(vec3f(0.001, 0.0022, 0.003), refl, fres);
    // surf: swells steepen and break as they reach the cliff, each crest
    // a sharp line of white water trailing broken foam seaward
    let off = p.x - coast_x(p.z);
    let dsh = max(-off, 0.0);
    let de = length(p.xz - ENTRY.xz);
    let wob = noise_value2(p.xz * vec2f(0.03, 0.012)) * 0.8;
    let ph = fract(dsh / 16.0 + ctx.t * 0.09 + wob);
    let brk = sstep(40.0, 8.0, dsh);
    let tex = smoothstep(0.25, 0.75, noise_value2(p.xz * vec2f(0.3, 0.12) + vec2f(ctx.t * 0.08, 0.0)));
    var foam = sstep(0.0, 0.015, ph) * exp(-ph * 9.0) * brk * (0.35 + 0.65 * tex);
    // churning white water at the foot of the cliff
    let swash = sstep(6.0, 0.5, dsh) * (0.4 + 0.6 * tex) * (0.6 + 0.4 * sin(ctx.t * 0.5 + p.z * 0.05 + wob * 6.0));
    foam = max(foam, swash * 0.8);
    let lit_e = lava_light(vec3f(p.x, 1.0, p.z), vec3f(0.0, 1.0, 0.0), l);
    let fl = l.amb * 2.2 + l.moon_c * 0.5 + lit_e * 0.6;
    c = mix(c, fl * 0.8, saturate(foam));
    // where a breaker hits hot lava it bursts into steam, lit orange from
    // the lava it quenches
    let hitk = max(exp(-de / 50.0), exp(-max(length(p.xz - BRK2.xy) - BRK2.z * 0.5, 0.0) / 12.0));
    let arrive = sstep(7.0, 0.0, dsh) * exp(-ph * 7.0) * sstep(0.0, 0.02, ph);
    c += fire_temperature_color(0.72) * l.lava * hitk * (arrive * 2.0 + swash * 0.3) * (0.3 + 0.7 * tex);
    // water at the entry boils and glows
    c += fire_temperature_color(0.8) * exp(-de * de / 500.0) * 0.8 * l.lava;
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = EYE;
    let cam = cam_look_at(ro, ro + vec3f(-0.132, -0.073, -1.0), 0.0, 40.0);
    let rd = cam_ray(cam, p);
    let pxa = ctx.px / cam.zoom;
    let tw = water_intersect(ro, rd, 0.0);
    let tr_ = trace_land(ro, rd, select(9000.0, tw, tw > 0.0));
    let tl = tr_.x;
    // the colour of the haze: the low sky, dimmed by the vog
    let hz = sky(normalize(vec3f(rd.x, 0.01, rd.z)), l, ctx) * 0.6;
    var col: vec3f;
    var tt = 1e5;
    if (tl > 0.0) {
        let hp = ro + rd * tl;
        col = shade_land(hp, rd, tl, pxa, l, ctx);
        tt = tl;
    } else if (tw > 0.0) {
        let hp = ro + rd * tw;
        tt = tw;
        if (hp.x > coast_x(hp.z)) {
            // beyond the land march's reach: the far flow field, lost in haze
            col = hz;
        } else {
            col = shade_sea(hp, rd, tw, pxa, l, ctx);
        }
    } else {
        col = sky(rd, l, ctx);
        let b = backdrop(rd, l, pxa, ctx.t);
        // the shield stands kilometres off: its foot sinks into the same
        // haze as the far flow field and the sea horizon
        let bc = mix(b.rgb, hz, 0.85 * exp(-max(rd.y, 0.0) * 45.0));
        col = mix(col, bc, b.w);
    }
    // vog and sea haze toward the horizon
    if (tt < 9e4) {
        let haze = 1.0 - exp(-tt * 0.00035);
        col = mix(col, hz, haze);
    }
    col += fire_temperature_color(0.66) * l.lava * tr_.y * 0.004;
    // the steam column in front of whatever lies behind it
    let st = steam(ro, rd, tt, ctx.t, ctx.jitter, l, steps(34.0, ctx));
    col = col * st.w + st.rgb;
    return col * exp2(l.exposure);
}
