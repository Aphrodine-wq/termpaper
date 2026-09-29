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

// World units are metres, sea level y = 0. We stand on an older flow 28 m up,
// looking along the coast: the Pacific on the left, the new lava delta on the
// right. A channel of molten rock winds down from the shield of Kilauea to
// the ocean entry, where the lava meets the surf in a column of steam lit
// orange from below.

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
            return Look(normalize(vec3f(0.5, 0.18, -0.85)), vec3f(0.05, 0.06, 0.1), vec3f(0.018, 0.028, 0.06), 1u, 0.8, 0.2);
        }
        case 2u: {
            return Look(normalize(vec3f(-0.6, 0.45, 0.4)), vec3f(0.012, 0.014, 0.02), vec3f(0.003, 0.003, 0.005), 2u, 1.25, -1.0);
        }
        default: {
            return Look(normalize(vec3f(-0.6, 0.45, 0.4)), vec3f(0.014, 0.017, 0.026), vec3f(0.0025, 0.003, 0.006), 0u, 1.0, -0.8);
        }
    }
}

// ------------------------------------------------------------------ layout

const ENTRY: vec3f = vec3f(-38.0, 0.0, -240.0);

// the coastline: land for x > coast_x(z)
fn coast_x(z: f32) -> f32 {
    let s = -z;
    return -48.0 + 22.0 * sin(s * 0.0052 + 0.4) + 8.0 * sin(s * 0.019 + 1.3) - s * 0.03;
}

// the channel: centre z as a function of x; it comes across the delta from
// the right and pours into the sea at the entry
fn chan_z(x: f32) -> f32 {
    return ENTRY.z + (x - ENTRY.x) * 0.9 + 24.0 * (sin(x * 0.013 + 0.6) - sin(ENTRY.x * 0.013 + 0.6));
}

// distance to the channel centreline (horizontal, approximate) and the
// channel coordinate along it
fn chan(xz: vec2f) -> vec2f {
    if (xz.x < ENTRY.x - 6.0) { return vec2f(1e4, xz.x); }
    let dz = xz.y - chan_z(xz.x);
    // correct for the channel's slant so the width reads true
    let slope = 0.9 + 0.312 * cos(xz.x * 0.013 + 0.6);
    return vec2f(abs(dz) * inverseSqrt(1.0 + slope * slope), xz.x);
}

fn land_h(xz: vec2f) -> f32 {
    let off = xz.x - coast_x(xz.y);
    // older flows rise gently inland; lobes and tumuli of pahoehoe
    var h = 5.0 + off * 0.012 + max(-xz.y - 900.0, 0.0) * 0.03;
    h += 1.6 * noise_value2(xz * 0.03) + 0.7 * noise_value2(xz * 0.11 + 3.0);
    // the channel sits between levees
    let c = chan(xz);
    h += 1.4 * exp(-sq((c.x - 13.0) / 5.0)) - 1.8 * sstep(12.0, 5.0, c.x);
    // a low sea cliff at the coast
    return min(h, off * 1.6 - 1.0);
}

// ------------------------------------------------------------------ lava

// heat of the ground 0..1 (emission and the light it throws)
struct Lava { e: vec3f, heat: f32 }

// breakouts: fresh pahoehoe toes oozing from the crust, hot at their seams
const BRK0: vec3f = vec3f(34.0, -12.0, 26.0);   // centre xz, radius
const BRK1: vec3f = vec3f(128.0, -64.0, 34.0);

fn breakout(xz: vec2f, b: vec3f, t: f32) -> f32 {
    let d = length(xz - b.xy) / b.z + (noise_value2(xz * 0.07 + b.xy) - 0.5) * 0.55 + (noise_value2(xz * 0.3) - 0.5) * 0.12;
    if (d > 1.2) { return 0.0; }
    let breathe = 0.85 + 0.15 * sin(t * 0.3 + b.x);
    // the advancing front: bulging incandescent toes
    // (only on its downhill, seaward side: behind, it is fed by a tube)
    let dir = normalize(xz - b.xy + vec2f(1e-3));
    let down = smoothstep(-0.3, 0.5, dot(dir, vec2f(-0.75, 0.66)));
    let front = exp(-sq((d - 1.0) / 0.05)) * smoothstep(0.3, 0.7, noise_value2(xz * 0.25 + vec2f(t * 0.01, 0.0))) * down;
    // behind it a silvery crust, split by a few still-glowing cracks
    let w = noise_worley2(xz * 0.32);
    let crack = sstep(0.1, 0.0, w.y - w.x) * smoothstep(0.52, 0.8, noise_value2(xz * 0.18 + b.xy * 0.1));
    let inside = sstep(1.0, 0.75, d);
    return breathe * max(front * 0.9, crack * inside * 0.7);
}
fn lava_at(xz: vec2f, t: f32, fp: f32, l: Look) -> Lava {
    var r: Lava;
    r.e = vec3f(0.0);
    r.heat = 0.0;
    let c = chan(xz);
    // the channel: molten rock under drifting plates of crust; some reaches
    // run hot and open, others are nearly crusted over
    if (c.x < 14.0) {
        let across = c.x / 10.0;
        let uv = vec2f(c.y * 0.22 + t * 0.33, (xz.y - chan_z(c.y)) * 0.3);
        let w = noise_worley2(uv);
        let seam = sstep(0.35, 0.02, w.y - w.x);
        let open = 0.55 + 0.45 * smoothstep(0.3, 0.7, noise_value2(vec2f(c.y * 0.018 + t * 0.004, 3.0)));
        let core = sstep(1.05, 0.4, across);
        var heat = mix(0.45, 0.92, seam * open + (1.0 - open) * 0.2) * core * (0.75 + 0.25 * open);
        heat = max(heat, 0.35 * sstep(1.3, 0.8, across));
        r.heat = heat;
        r.e = fire_temperature_color(heat) * 1.4;
    }
    // cooling crust along the channel: a web of glowing cracks
    let fresh = exp(-max(c.x - 12.0, 0.0) / 40.0);
    if (fresh > 0.05 && c.x >= 12.0) {
        let n = noise_value2(xz * 0.05 + vec2f(t * 0.01, 0.0));
        let hot = fresh * smoothstep(0.35, 0.8, n);
        if (hot > 0.02) {
            let w = noise_worley2(xz * 0.45);
            // cracks thinner than a pixel still glow, just dimmer
            let cw = max(0.06, fp * 0.45);
            let crack = sstep(cw, 0.0, w.y - w.x) * min(1.0, 0.1 / cw) * smoothstep(0.45, 0.75, noise_value2(xz * 0.16 + 3.0));
            let heat = hot * (0.25 + 0.55 * crack);
            r.heat = max(r.heat, heat);
            r.e += fire_temperature_color(heat * crack * 0.9) * crack * hot * 1.2;
        }
    }
    let bk = max(breakout(xz, BRK0, t), breakout(xz, BRK1, t));
    if (bk > 0.0) {
        r.heat = max(r.heat, bk);
        r.e += fire_temperature_color(bk) * 1.3;
    }
    // where the lava pours into the sea
    let off = xz.x - coast_x(xz.y);
    let de = length(xz - ENTRY.xz);
    let pour = exp(-de * de / 180.0) * smoothstep(-4.0, 3.0, off);
    r.heat = max(r.heat, pour);
    r.e += fire_temperature_color(0.9) * pour * 1.6;
    r.e *= l.lava;
    return r;
}

// light thrown by the lava onto a point (channel, entry and the breakouts)
fn lava_light(p: vec3f, t: f32, l: Look) -> vec3f {
    let c = chan(p.xz);
    let hy = max(p.y - land_h(p.xz), 0.0) + 2.0;
    var e = 2.6 * exp(-max(c.x - 8.0, 0.0) / 34.0);
    let de = length(p - ENTRY);
    e += 60.0 / (1.0 + de * de / 150.0) + 1.5 * exp(-de / 70.0);
    e += 1.1 * exp(-max(length(p.xz - BRK0.xy) - BRK0.z * 0.6, 0.0) / 14.0);
    e += 1.1 * exp(-max(length(p.xz - BRK1.xy) - BRK1.z * 0.6, 0.0) / 16.0);
    return fire_temperature_color(0.72) * e * l.lava * (1.0 / hy);
}

// ------------------------------------------------------------------ steam

// the plume axis leans downwind with height and sways slowly
fn plume_axis(y: f32, t: f32) -> vec2f {
    let lean = vec2f(0.28, -0.2) * y + vec2f(12.0 * sin(y * 0.016 - t * 0.04), 8.0 * sin(y * 0.012 + 1.0 - t * 0.03));
    return ENTRY.xz + lean;
}

fn plume_dens(p: vec3f, t: f32) -> f32 {
    if (p.y < 0.0 || p.y > 380.0) { return 0.0; }
    let rad = 9.0 + 0.42 * p.y;
    // large, slow turbulence bends the column into billows
    let wq = vec3f(p.x, p.y, p.z) * 0.012 - vec3f(0.0, t * 0.03, 0.0);
    let warp = vec2f(noise_value3(wq) - 0.5, noise_value3(wq + vec3f(7.1, 3.3, 1.9)) - 0.5) * rad * 1.1;
    let r = length(p.xz + warp - plume_axis(p.y, t)) / rad;
    if (r > 1.35) { return 0.0; }
    // billows: rising, rolling, eating into the edge of the column
    let q = vec3f(p.x, p.y * 0.8, p.z) * (0.9 / rad + 0.012) + vec3f(0.0, -t * 0.06, t * 0.01);
    let n = noise_fbm3(q, 4);
    let body = sstep(1.05, 0.15, r + (n - 0.5) * 1.3);
    let holes = smoothstep(0.12, 0.48, n);
    let fade = sstep(380.0, 120.0, p.y) * smoothstep(0.0, 5.0, p.y);
    return body * holes * fade;
}

// march the plume; (radiance, transmittance)
fn steam(ro: vec3f, rd: vec3f, tmax: f32, t: f32, jit: f32, l: Look, n: i32) -> vec4f {
    // bound: a vertical cylinder around the leaning column
    let cen = plume_axis(170.0, t);
    let oc = ro.xz - cen;
    let d2 = rd.xz;
    let a = max(dot(d2, d2), 1e-6);
    let hb = dot(oc, d2);
    let disc = hb * hb - a * (dot(oc, oc) - 175.0 * 175.0);
    if (disc <= 0.0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let sq_ = sqrt(disc);
    let t0 = max((-hb - sq_) / a, 0.0);
    var t1 = min((-hb + sq_) / a, tmax);
    if (rd.y > 0.0) { t1 = min(t1, (380.0 - ro.y) / rd.y); }
    if (t1 <= t0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let dt = (t1 - t0) / f32(n);
    var tt = t0 + dt * jit;
    var tr = 1.0;
    var acc = vec3f(0.0);
    let glow = fire_temperature_color(0.7) * l.lava;
    for (var i = 0; i < 40; i++) {
        if (i >= n || tr < 0.03) { break; }
        let p = ro + rd * tt;
        let d = plume_dens(p, t);
        if (d > 0.01) {
            // lit from below by the entry: bright at the foot of the column,
            // fading as the steam climbs out of reach of the glow; the upper
            // plume only catches the sky and the moon
            let lit = glow * (1.3 * exp(-p.y / 22.0) + 0.003 * exp(-p.y / 120.0));
            let sky = l.amb * 2.5 + l.moon_c * 0.35 * smoothstep(20.0, 250.0, p.y);
            let ext = d * 0.03;
            let st = exp(-ext * dt);
            acc += tr * (lit + sky) * (1.0 - st);
            tr *= st;
        }
        tt += dt;
    }
    return vec4f(acc, tr);
}

// ------------------------------------------------------------------ sky

// the shield of Kilauea behind the delta: elevation of its skyline
fn shield(az: f32) -> f32 {
    let rise = smoothstep(-0.35, 1.1, az);
    return -0.003 + 0.082 * rise - 0.02 * smoothstep(0.9, 1.6, az) + 0.004 * noise_value2(vec2f(az * 30.0, 2.0)) * rise;
}

fn sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let y = max(rd.y, 0.0);
    var c: vec3f;
    if (l.kind == 1u) {
        // blue hour: deep blue overhead, a last violet-rose band low in the west
        let west = saturate(dot(normalize(rd.xz + vec2f(1e-4)), normalize(l.moon.xz)) * 0.5 + 0.5);
        c = mix(col_hex(0x4a6aa8u) * 0.11, col_hex(0x0e2250u) * 0.06, pow(y, 0.35));
        c += col_hex(0xd08a7au) * 0.045 * exp(-y * 10.0) * west * west;
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
        let ai = 0.03 + 0.07 * fi + 0.05 * h;
        if (hash_f(u32(i) * 91u + 5u) < 0.35) { continue; }
        let ti = shield(ai) - 0.003;
        let dy = ti - rd.y;
        if (dy < 0.0 || dy > 0.05) { continue; }
        let x = az - ai - dy * 0.9 - 0.0012 * sin(dy * 260.0 + fi * 2.0);
        let seg = smoothstep(0.35, 0.6, noise_value2(vec2f(dy * 140.0, fi * 7.0)));
        riv += (0.2 + 0.5 * h) * seg * exp(-x * x / (w * w)) * sstep(0.05, 0.035, dy);
    }
    col += fire_temperature_color(0.62) * riv * 0.9 * l.lava * select(1.0, 1.6, l.kind == 2u);
    var out = col * cov;
    if (l.kind == 2u) {
        // lava fountain on the flank
        let fa = 0.2;
        let fx = az - fa;
        let fy = rd.y - (shield(fa) - 0.002);
        let ww = max(pxa * 0.9, 0.0018);
        let jet = exp(-fx * fx / (ww * ww * (1.0 + max(fy, 0.0) * 500.0))) * sstep(0.055, 0.01, fy) * smoothstep(-0.003, 0.0, fy);
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

fn trace_land(ro: vec3f, rd: vec3f) -> f32 {
    if (rd.y > 0.02) { return -1.0; }
    var t = 1.0;
    var tp = t;
    var hp = 1.0;
    for (var i = 0; i < 110; i++) {
        let p = ro + rd * t;
        let h = p.y - land_h(p.xz);
        if (h < 0.0) {
            if (i == 0) { return t; }
            return tp + (t - tp) * hp / (hp - h);
        }
        tp = t;
        hp = h;
        t += max(h * 0.7, 0.06 + t * 0.004);
        if (t > 5000.0 || p.y < -3.0) { break; }
    }
    return -1.0;
}

fn shade_land(p: vec3f, rd: vec3f, t: f32, pxa: f32, l: Look, ctx: Ctx) -> vec3f {
    let e = max(0.08, t * pxa);
    let hx = land_h(p.xz + vec2f(e, 0.0)) - land_h(p.xz - vec2f(e, 0.0));
    let hz = land_h(p.xz + vec2f(0.0, e)) - land_h(p.xz - vec2f(0.0, e));
    var n = normalize(vec3f(-hx, 2.0 * e, -hz));
    // ropy pahoehoe folds, faded with the pixel footprint
    // lumpy pahoehoe toes, faded with the pixel footprint
    let lump = saturate(1.0 - t * pxa / 0.6);
    if (lump > 0.0) {
        let w = noise_worley2(p.xz * 0.35);
        let g = vec2f(noise_value2(p.xz * 0.35 + 5.0) - 0.5, noise_value2(p.xz * 0.35 + 9.0) - 0.5);
        n = normalize(n + vec3f(g.x, 0.0, g.y) * 0.5 * lump * smoothstep(0.1, 0.4, w.y - w.x));
    }
    let lv = lava_at(p.xz, ctx.t, t * pxa, l);
    // glassy black basalt: dark diffuse, a silvery sheen at grazing angles
    let alb = mix(col_hex(0x141416u), col_hex(0x201f1eu), noise_value2(p.xz * 0.07)) * 0.5;
    let ll = lava_light(p, ctx.t, l);
    let dif = saturate(dot(n, l.moon));
    var c = alb * (l.moon_c * dif + l.amb + ll * 0.6);
    let fres = 0.03 + 0.25 * pow(1.0 - saturate(dot(n, -rd)), 5.0);
    let r = reflect(rd, n);
    c += fres * (l.amb * 0.8 + ll * 0.08 * saturate(r.y + 0.3));
    c += fres * 0.15 * l.moon_c * pow(saturate(dot(r, l.moon)), 24.0) * 8.0;
    return c + lv.e;
}

fn shade_sea(p: vec3f, rd: vec3f, t: f32, pxa: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = water_normal(p.xz, ctx.t, 0.8, t, ctx);
    let r = reflect(rd, n);
    let fres = water_fresnel(dot(-rd, n));
    let rr = normalize(vec3f(r.x, abs(r.y) + 0.005, r.z));
    var refl = sky(rr, l, ctx) * 0.8;
    // the glow of the entry and the lit steam, reflected as a broken path
    let to_e = normalize(ENTRY + vec3f(0.0, 30.0, 0.0) - p);
    let to_s = normalize(ENTRY + vec3f(25.0, 110.0, -30.0) - p);
    let glow = fire_temperature_color(0.72) * l.lava;
    refl += glow * (pow(saturate(dot(r, to_e)), 40.0) * 8.0 + pow(saturate(dot(r, to_s)), 10.0) * 0.8);
    refl += l.moon_c * pow(saturate(dot(r, l.moon)), 300.0) * 25.0;
    var c = mix(vec3f(0.001, 0.0022, 0.003), refl, fres);
    // surf along the delta, lit by the lava
    let off = p.x - coast_x(p.z);
    let near = exp(-max(-off, 0.0) / 14.0);
    let wave = sin(-off * 0.16 + ctx.t * 1.1 + noise_value2(p.xz * 0.03) * 5.0);
    let foam = near * smoothstep(0.2, 0.9, wave) * (0.5 + 0.5 * noise_value2(p.xz * vec2f(0.2, 0.08) + ctx.t * 0.1));
    let fl = l.amb * 2.0 + l.moon_c * 0.4 + lava_light(vec3f(p.x, 0.5, p.z), ctx.t, l) * 0.5;
    c = mix(c, fl * 0.8, saturate(foam));
    // water at the entry boils and glows
    let de = length(p.xz - ENTRY.xz);
    c += fire_temperature_color(0.8) * exp(-de * de / 300.0) * 0.8 * l.lava;
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(62.0, 34.0, 60.0);
    let cam = cam_look_at(ro, ro + vec3f(-0.16, -0.1, -1.0), 0.0, 40.0);
    let rd = cam_ray(cam, p);
    let pxa = ctx.px / cam.zoom;
    let tl = trace_land(ro, rd);
    let tw = water_intersect(ro, rd, 0.0);
    var col: vec3f;
    var tt = 1e5;
    if (tl > 0.0 && (tw < 0.0 || tl < tw)) {
        let hp = ro + rd * tl;
        col = shade_land(hp, rd, tl, pxa, l, ctx);
        tt = tl;
    } else if (tw > 0.0) {
        let hp = ro + rd * tw;
        col = shade_sea(hp, rd, tw, pxa, l, ctx);
        tt = tw;
    } else {
        col = sky(rd, l, ctx);
        let b = backdrop(rd, l, pxa, ctx.t);
        col = mix(col, b.rgb, b.w);
    }
    // vog and sea haze toward the horizon
    if (tt < 9e4 || tw > 0.0) {
        let haze = 1.0 - exp(-min(tt, 1e5) * 0.00035);
        col = mix(col, sky(normalize(vec3f(rd.x, 0.01, rd.z)), l, ctx), haze);
    }
    // the steam column in front of whatever lies behind it
    let st = steam(ro, rd, tt, ctx.t, ctx.jitter, l, steps(22.0, ctx));
    col = col * st.w + st.rgb;
    return col * exp2(l.exposure);
}
