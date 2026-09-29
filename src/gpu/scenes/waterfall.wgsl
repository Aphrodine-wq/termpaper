//! name: waterfall
//! title: Skógafoss
//! category: coast
//! tags: iceland, waterfall, rainbow, mist, cliff, river
//! desc: a 60 m curtain of water pours off a mossy basalt cliff into mist, a rainbow in the spray
//! themes: summer, winter, midnight
//! uses: camera, sky, clouds
//! cost: medium
//! fallback: ocean
//! credits: original

// World units are metres; y up, the camera stands on the gravel bank of the
// river 2 m up, ~130 m south of the falls, looking north. The old sea cliff
// runs across the frame; the fall drops 60 m from its lip along a ballistic
// curve. The curtain's streaks are keyed to each parcel's time of flight,
// sqrt(2 (60 - y) / g) - t, so they fall and stretch exactly like water.
// The rainbow sits 42 degrees from the antisolar point, weighted by how
// much sunlit spray the ray crosses.

const CAM: vec3f = vec3f(0.0, 2.2, 0.0);
const FALL_X: f32 = -4.0;
const LIP_Y: f32 = 60.0;
const LIP_Z: f32 = -141.0;
const V0: f32 = 2.6;          // horizontal speed off the lip, m/s
const HALF_W: f32 = 12.5;

struct Look {
    sun: vec3f,
    sun_c: vec3f,
    amb: vec3f,
    hor: vec3f,
    snow: f32,
    flow: f32,     // winter: less water, edges frozen
    bow: f32,      // rainbow strength
    exposure: f32,
    haze: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            l.sun = sky_sun_dir(3.5, -160.0);
            l.snow = 1.0; l.flow = 0.62; l.bow = 0.25; l.exposure = 0.7; l.haze = 1.6;
        }
        case 2u: {
            l.sun = sky_sun_dir(4.5, 136.0);
            l.snow = 0.0; l.flow = 1.0; l.bow = 1.3; l.exposure = 0.35; l.haze = 1.3;
        }
        default: {
            l.sun = sky_sun_dir(32.0, 153.0);
            l.snow = 0.0; l.flow = 1.0; l.bow = 1.0; l.exposure = -0.2; l.haze = 1.0;
        }
    }
    l.sun_c = sky_sun_light(l.sun);
    l.amb = sky_ambient(l.sun);
    if (l.snow > 0.5) {
        // winter: a weak pink sun skims the plateau, the cove lies in blue shade
        l.sun_c *= vec3f(0.8, 0.75, 0.85) * 0.035;
        l.amb = vec3f(0.1, 0.15, 0.26) * 1.3;
    }
    if (theme == 2u) {
        l.sun_c *= vec3f(1.25, 1.0, 0.8);
        l.amb *= 0.75;
    }
    l.hor = sky_atmosphere_haze(normalize(vec3f(0.0, 0.05, -1.0)), l.sun, l.haze) * ozone(l.sun);
    if (l.snow > 0.5) { l.hor = vec3f(0.16, 0.19, 0.25); }
    return l;
}

fn ozone(sun: vec3f) -> vec3f {
    let path = saturate(1.0 - sun.y * 6.0) * 1.1;
    return exp(-vec3f(0.62, 0.52, 0.04) * path);
}

// ------------------------------------------------------------ the cliff

// z of the cliff face at (x, y): the old coastline, an alcove carved behind
// the fall, grassy talus fans at the foot away from the plunge pool
fn face_z(x: f32, y: f32) -> f32 {
    let dx = abs(x - FALL_X);
    var z = -134.0 + 7.0 * noise_value2(vec2f(x * 0.015, 3.0)) - 0.02 * x;
    z -= 9.0 * smoothstep(30.0, 13.0, dx);
    // the old sea cliff leans back and is grassed over; beside the fall the
    // spray has kept a near-vertical wall of bare basalt
    let lean = mix(0.08, 0.36, smoothstep(14.0, 40.0, dx));
    z -= max(y, 0.0) * lean;
    // vertical ribs and gullies
    z += 3.2 * noise_value2(vec2f(x * 0.11, y * 0.012)) + 1.3 * noise_value2(vec2f(x * 0.45, y * 0.04));
    let talus = smoothstep(16.0, 34.0, dx) * clamp(16.0 - y, 0.0, 17.0) * 1.3 * (0.8 + 0.4 * noise_value2(vec2f(x * 0.04, 7.0)));
    return z + talus;
}

fn cliff_top(x: f32, z: f32) -> f32 {
    return LIP_Y + 1.0 - 16.0 * smoothstep(35.0, 170.0, x) - 6.0 * smoothstep(-60.0, -190.0, x) + 7.0 * noise_value2(vec2f(x * 0.018, z * 0.03)) + 2.0 * noise_value2(vec2f(x * 0.09, 3.0)) + 0.06 * (-z - 140.0);
}

fn cliff_sdf(p: vec3f) -> f32 {
    let dz = p.z - face_z(p.x, p.y);
    let dy = p.y - cliff_top(p.x, p.z);
    // rounded lip where the face meets the plateau
    let k = 4.0;
    let h = max(k - abs(dz * 0.55 - dy * 0.8), 0.0) / k;
    return max(dz * 0.55, dy * 0.8) + h * h * k * 0.25;
}

fn cliff_march(ro: vec3f, rd: vec3f, tmax: f32, n: i32) -> f32 {
    if (rd.z > -0.05) { return -1.0; }
    var t = max((-95.0 - ro.z) / rd.z, 1.0);
    if (t > tmax) { return -1.0; }
    for (var i = 0; i < 128; i++) {
        if (i >= n) { break; }
        let p = ro + rd * t;
        let d = cliff_sdf(p);
        if (d < 0.002 * t) { return t; }
        t += max(d, 0.02 * t * 0.05);
        if (t > tmax || p.y > 80.0) { break; }
    }
    return -1.0;
}

fn cliff_shadow(p: vec3f, l: vec3f) -> f32 {
    var res = 1.0;
    var t = 0.6;
    for (var i = 0; i < 22; i++) {
        let q = p + l * t;
        if (q.y > 85.0) { break; }
        let h = cliff_sdf(q);
        res = min(res, 7.0 * h / t);
        if (res < 0.02) { break; }
        t += clamp(h, 0.4, 12.0);
    }
    return saturate(res);
}

fn cliff_normal(p: vec3f, t: f32) -> vec3f {
    let e = max(0.05, t * 0.0015);
    let k = vec2f(1.0, -1.0);
    return normalize(k.xyy * cliff_sdf(p + k.xyy * e) + k.yyx * cliff_sdf(p + k.yyx * e) +
                     k.yxy * cliff_sdf(p + k.yxy * e) + k.xxx * cliff_sdf(p + k.xxx * e));
}

// ------------------------------------------------------------ the fall

fn fall_tau(y: f32) -> f32 { return sqrt(max(2.0 * (LIP_Y - y) / 9.81, 0.0)); }
fn fall_z(y: f32) -> f32 { return LIP_Z + V0 * fall_tau(y); }
fn fall_hw(y: f32, l: Look) -> f32 { return (HALF_W + 0.05 * (LIP_Y - y)) * mix(0.7, 1.0, l.flow); }

// intersect the ballistic curtain: returns (t, x-offset, y), t < 0 on a miss
fn fall_hit(ro: vec3f, rd: vec3f, l: Look) -> vec3f {
    if (rd.z > -1e-3) { return vec3f(-1.0); }
    var t = (LIP_Z + 5.0 - ro.z) / rd.z;
    for (var i = 0; i < 4; i++) {
        let y = clamp(ro.y + rd.y * t, -2.0, LIP_Y);
        t = (fall_z(y) - ro.z) / rd.z;
    }
    let p = ro + rd * t;
    let u = p.x - FALL_X;
    if (p.y > LIP_Y + 0.3 || p.y < -1.0 || abs(u) > fall_hw(p.y, l) + 2.0) { return vec3f(-1.0); }
    return vec3f(t, u, p.y);
}

// the falling water: (colour, opacity)
fn fall_shade(u: f32, y: f32, t: f32, rd: vec3f, l: Look, ctx: Ctx) -> vec4f {
    let tau = fall_tau(y);
    let hw = fall_hw(y, l);
    // pattern space moves with the water: streaks fall and stretch
    let s = tau - ctx.t * 0.9;
    let q = vec2f(u * 0.9, s * 3.2);
    let streak = noise_fbm2(vec2f(q.x, q.y * 0.35), 5);
    let fine = noise_value2(vec2f(u * 3.5, s * 9.0));
    let clump = smoothstep(0.35, 0.7, noise_value2(vec2f(u * 0.35, s * 1.1)));
    // glassy sheet at the lip that breaks into white water as it falls
    let breakup = smoothstep(0.1, 0.7, tau);
    let edge = smoothstep(hw + 1.5, hw - 2.5, abs(u) + (streak - 0.5) * 4.0 * breakup) * smoothstep(LIP_Y + 0.3, LIP_Y - 0.4, y);
    var a = edge * mix(0.95, 0.4 + 0.5 * clump + 0.6 * (streak - 0.5), breakup);
    a = saturate(a * (0.6 + 0.6 * fine));
    // winter: frozen curtain edges
    let foam = mix(0.35, 1.0, breakup) * (0.6 + 0.6 * streak) * (0.8 + 0.3 * fine);
    let lit = saturate(0.35 + 0.65 * dot(vec3f(0.0, 0.25, 1.0), l.sun) * 1.2);
    var c = vec3f(0.82, 0.86, 0.88) * foam * (l.sun_c * lit * 0.9 + l.amb * 1.2);
    c = mix(col_hex(0x2a3a36u) * (l.amb * 1.5 + l.sun_c * 0.1), c, breakup);
    return vec4f(c, a);
}

// icicles hanging from the lip beside the fall, and along ledges (winter)
fn icicles(u: f32, y: f32, l: Look) -> f32 {
    if (l.snow < 0.5) { return 0.0; }
    let k = u * 1.6;
    let c = floor(k);
    let h = hash_f(u32(i32(c) + 3000));
    let len = 3.0 + 14.0 * h * h;
    let w = 0.3 * (1.0 - saturate((LIP_Y - y) / len));
    return step(abs(fract(k) - 0.5), w) * step(LIP_Y - len, y) * step(y, LIP_Y + 0.5);
}

// ------------------------------------------------------------ ground & river

// river centreline x at distance z; the Skógá runs from the pool past us on the left
fn river_x(z: f32) -> f32 { return FALL_X - 6.0 * smoothstep(-120.0, 0.0, z) + 3.5 * sin(z * 0.035 + 0.5); }
fn river_hw(z: f32) -> f32 { return 9.0 + 4.0 * smoothstep(-30.0, -90.0, z) + 9.0 * smoothstep(-100.0, -135.0, z); }

fn ground_h(xz: vec2f) -> f32 {
    let rx = abs(xz.x - river_x(xz.y));
    let bank = smoothstep(river_hw(xz.y) - 2.0, river_hw(xz.y) + 14.0, rx);
    return -0.35 + 0.9 * bank + 0.5 * noise_value2(xz * 0.08) * bank + 0.15 * noise_value2(xz * 0.6);
}

fn shade_ground(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let e = max(0.03, t * ctx.px);
    let hx = ground_h(p.xz + vec2f(e, 0.0)) - ground_h(p.xz - vec2f(e, 0.0));
    let hz = ground_h(p.xz + vec2f(0.0, e)) - ground_h(p.xz - vec2f(0.0, e));
    let n = normalize(vec3f(-hx * 0.4, 2.0 * e, -hz * 0.4));
    let rx = abs(p.x - river_x(p.z));
    // black volcanic gravel by the water, grass on the banks
    let pebble = 0.6 + 0.6 * noise_value2(p.xz * 5.0) * noise_value2(p.xz * 1.3 + 3.0);
    var alb = col_hex(0x5e5a55u) * pebble;
    let grass = smoothstep(0.45, 0.7, noise_fbm2(p.xz * 0.04, 4) + 0.4 * smoothstep(river_hw(p.z) + 10.0, river_hw(p.z) + 45.0, rx) - 0.2);
    let tuft = 0.7 + 0.6 * noise_value2(p.xz * 1.5) * noise_value2(p.xz * 0.37);
    alb = mix(alb, col_hex(0x4f6a2au) * tuft, grass);
    // pebbles at several scales, wet and dark nearer the water
    alb *= 0.75 + 0.5 * noise_value2(p.xz * 13.0) * (0.6 + 0.4 * noise_value2(p.xz * 0.6));
    alb *= mix(0.6, 1.0, smoothstep(river_hw(p.z), river_hw(p.z) + 3.0, rx));
    if (l.snow > 0.5) {
        let bank = smoothstep(river_hw(p.z) + 0.5, river_hw(p.z) + 4.0 + 3.0 * noise_value2(p.xz * 0.2), rx);
        alb = mix(alb, vec3f(0.85, 0.88, 0.92) * (0.9 + 0.1 * noise_value2(p.xz * 2.0)), bank);
    }
    let dif = saturate(dot(n, l.sun));
    return alb * (l.sun_c * dif + l.amb * (0.6 + 0.4 * n.y));
}

fn shade_river(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    // riffles stretched along the current and carried toward us
    let fl = vec2f(p.x * 2.2, p.z * 0.6 - ctx.t * 3.0);
    let far = smoothstep(15.0, 110.0, t);
    let g = vec2f(noise_value2(fl) - 0.5, noise_value2(fl * 1.7 + 9.0) - 0.5) * mix(0.12, 0.04, far);
    let n = normalize(vec3f(g.x, 1.0, g.y));
    var r = reflect(rd, n);
    r.y = abs(r.y) + 0.01;
    let f = 0.02 + 0.98 * pow(1.0 - saturate(dot(-rd, n)), 5.0);
    var refl = sky_atmosphere_haze(normalize(r), l.sun, l.haze) * ozone(l.sun);
    if (l.snow > 0.5) { refl = l.hor * 1.1; }
    // the valley walls darken the low reflections
    refl *= mix(0.55, 1.0, smoothstep(0.02, 0.3, r.y));
    // shallow: the dark pebble bed shows through
    let bed = col_hex(0x35383au) * (0.7 + 0.5 * noise_value2(p.xz * 2.0)) * (l.sun_c * max(l.sun.y, 0.05) * 0.5 + l.amb * 0.8);
    var c = mix(bed, refl, f);
    // white water: the plunge pool boils, riffles break over stones
    let dpool = length((p.xz - vec2f(FALL_X, LIP_Z + 12.0)) / vec2f(28.0, 20.0));
    let boil = noise_fbm2(p.xz * 0.15 + vec2f(0.0, ctx.t * 0.4) + normalize(p.xz - vec2f(FALL_X, LIP_Z) + vec2f(1e-3)) * ctx.t * 0.7, 4);
    var foam = smoothstep(1.3, 0.3, dpool) * smoothstep(0.3, 0.6, boil + 0.35 * (1.0 - dpool));
    let rif = noise_fbm2(vec2f(p.x * 0.7, p.z * 0.1 - ctx.t * 0.8), 4);
    foam = max(foam, smoothstep(0.6, 0.8, rif) * 0.6 * (1.0 - far * 0.4));
    if (l.snow > 0.5) { foam *= 0.7; }
    let fc = vec3f(0.85, 0.88, 0.9) * (l.sun_c * max(l.sun.y, 0.1) * 0.9 + l.amb * 1.1);
    return mix(c, fc, foam);
}

// ------------------------------------------------------------ sky, mist, bow

fn dome(rd: vec3f, l: Look, ctx: Ctx, full: bool) -> vec3f {
    var c = sky_atmosphere_haze(rd, l.sun, l.haze) * ozone(l.sun);
    if (l.snow > 0.5) { c = mix(vec3f(0.18, 0.22, 0.3), vec3f(0.1, 0.14, 0.22), saturate(rd.y * 2.0)); }
    if (full && l.snow < 0.5) { c += sky_sun_disk(rd, l.sun, 0.55); }
    if (rd.y > 0.0) {
        let hp = rd.xz / (rd.y + 0.06) * 1.8;
        let cs = cloud_sheet(hp * 0.8 + vec2f(2.0, 7.0), select(0.45, 0.85, l.snow > 0.5), ctx);
        let toward = pow(saturate(dot(rd, l.sun) * 0.5 + 0.5), 3.0);
        var lit = l.sun_c * (0.35 + 0.9 * toward) * mix(0.55, 1.1, cs.y) + l.amb * 0.9;
        if (l.snow > 0.5) { lit = l.amb * mix(0.75, 1.05, cs.y); }
        c = mix(c, lit, cs.x * select(0.75, 0.95, l.snow > 0.5) * smoothstep(0.0, 0.12, rd.y));
    }
    return c;
}

fn mist_density(p: vec3f, ctx: Ctx) -> f32 {
    let c = vec3f(FALL_X, 6.0, LIP_Z + 12.0);
    let q = (p - c) / vec3f(40.0, 30.0, 30.0);
    let r = length(q);
    if (r > 1.0) { return 0.0; }
    // billows boiling up and out from the plunge
    let adv = vec3f(0.0, -ctx.t * 1.1, -ctx.t * 0.7);
    let n = noise_value3((p + adv) * 0.09) * 0.6 + noise_value3((p + adv * 1.6) * 0.21) * 0.4;
    let core = smoothstep(1.0, 0.2, r) * (1.0 - smoothstep(0.0, 1.0, q.y + 0.2) * 0.6);
    return saturate(core * (n * 1.8 - 0.25));
}

// (in-scattered rgb, transmittance, sunlit droplet depth for the rainbow)
fn mist_march(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx) -> vec4f {
    // bounding sphere of the spray cloud
    let c = vec3f(FALL_X, 6.0, LIP_Z + 12.0);
    let oc = ro - c;
    let b = dot(oc, rd);
    let h = b * b - (dot(oc, oc) - 42.0 * 42.0);
    if (h < 0.0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let sh = sqrt(h);
    let t0 = max(-b - sh, 0.0);
    let t1 = min(-b + sh, tmax);
    if (t1 <= t0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let n = 10;
    let dt = (t1 - t0) / f32(n);
    var tr = 1.0;
    var acc = vec3f(0.0);
    let ph = 0.5 + 1.2 * pow(saturate(dot(rd, l.sun)), 6.0);
    for (var i = 0; i < 10; i++) {
        let tt = t0 + (f32(i) + ctx.jitter) * dt;
        let d = mist_density(ro + rd * tt, ctx);
        if (d > 0.001) {
            let ext = d * 0.12;
            let st = exp(-ext * dt);
            acc += tr * (l.sun_c * ph * 0.8 + l.amb * 1.1) * (1.0 - st);
            tr *= st;
        }
    }
    return vec4f(acc, tr);
}

// rainbow colour for an angle (degrees) from the antisolar point
fn bow_color(th: f32) -> vec3f {
    // primary 40.6 (violet) .. 42.4 (red); secondary 50.2 (red) .. 53.3 (violet)
    let x1 = (th - 40.4) / 2.2;
    let x2 = (53.4 - th) / 3.3;
    var c = vec3f(0.0);
    c += vec3f(exp(-sq((x1 - 0.85) / 0.17)), exp(-sq((x1 - 0.52) / 0.17)), exp(-sq((x1 - 0.18) / 0.16)));
    c += 0.42 * vec3f(exp(-sq((x2 - 0.85) / 0.17)), exp(-sq((x2 - 0.52) / 0.17)), exp(-sq((x2 - 0.18) / 0.16)));
    // brighter sky inside the primary, Alexander's dark band between the bows
    c += vec3f(0.12) * smoothstep(41.0, 30.0, th);
    return c;
}

// ------------------------------------------------------------ scene

fn shade_cliff(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = cliff_normal(p, t);
    let dx = abs(p.x - FALL_X);
    // black basalt, vivid moss where spray and ledges let it hold, grass on top
    let ledge = smoothstep(0.12, 0.4, n.y);
    let wet = smoothstep(40.0, 14.0, dx);
    let outcrop = smoothstep(0.55, 0.75, noise_fbm2(vec2f(p.x * 0.06, p.y * 0.025), 4));
    let mossy = saturate(ledge * 1.3 - outcrop * 0.9 + 0.25 * noise_value2(vec2f(p.x * 0.3, p.y * 0.06)) + wet * 0.25 - 0.1);
    var alb = col_hex(0x2c2b2au) * (0.7 + 0.5 * noise_value2(vec2f(p.x * 0.5, p.y * 0.08)));
    let mv = noise_fbm2(vec2f(p.x * 0.05, p.y * 0.04), 3);
    let moss = mix(col_hex(0x3a5a20u), col_hex(0x5a7c2cu), mv) * (0.85 + 0.3 * noise_value2(vec2f(p.x * 0.7, p.y * 0.35)));
    // a band of bare basalt just under the lip
    let band = smoothstep(9.0, 4.0, cliff_top(p.x, p.z) - p.y) * smoothstep(0.5, 2.0, cliff_top(p.x, p.z) - p.y);
    alb = mix(alb, moss, mossy * (1.0 - band * 0.8));
    alb *= mix(1.0, 0.55, wet * (1.0 - mossy));
    let top = smoothstep(cliff_top(p.x, p.z) - 3.0, cliff_top(p.x, p.z) - 0.5, p.y);
    alb = mix(alb, col_hex(0x5a7a30u), top * ledge);
    if (l.snow > 0.5) {
        alb = mix(alb, vec3f(0.86, 0.9, 0.95), saturate(smoothstep(0.18, 0.45, n.y + 0.15 * noise_value2(vec2f(p.x * 0.3, p.y * 0.2))) + top * 0.8));
        // icicle curtains streak the wet walls beside the fall
        alb = mix(alb, vec3f(0.62, 0.72, 0.8), step(0.5, n.z) * wet * smoothstep(0.55, 0.75, noise_value2(vec2f(p.x * 2.0, p.y * 0.05))));
    }
    var dif = saturate(dot(n, l.sun));
    if (dif > 0.0 && l.snow < 0.5) { dif *= cliff_shadow(p + n * 0.3, l.sun); }
    // gullies are darker: occlusion from the rib noise
    let gully = noise_value2(vec2f(p.x * 0.11, p.y * 0.012));
    let occ = (0.6 + 0.4 * saturate(n.y * 0.5 + 0.5)) * (0.65 + 0.5 * gully);
    var c = alb * (l.sun_c * dif * (0.75 + 0.35 * gully) + l.amb * occ);
    // wet rock glistens
    let r = reflect(rd, n);
    c += dome(r, l, ctx, false) * wet * 0.06 * (1.0 - mossy);
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let cam = cam_look_at(CAM, CAM + vec3f(0.0, 0.19, -1.0), 0.0, 55.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    var tmax = 1e4;
    // ground plane first (it bounds everything else)
    var tg = -1.0;
    if (rd.y < 0.0) { tg = CAM.y / -rd.y; }
    let tc = cliff_march(CAM, rd, select(3000.0, tg, tg > 0.0), steps(96.0, ctx));
    if (tc > 0.0) {
        tmax = tc;
        col = shade_cliff(CAM + rd * tc, rd, tc, l, ctx);
    } else if (tg > 0.0) {
        tmax = tg;
        let gp = CAM + rd * tg;
        let rx = abs(gp.x - river_x(gp.z));
        if (rx < river_hw(gp.z) + 1.0 * noise_value2(gp.xz * 0.3)) {
            col = shade_river(gp, rd, tg, l, ctx);
        } else {
            col = shade_ground(gp, rd, tg, l, ctx);
        }
    } else {
        col = dome(rd, l, ctx, true);
    }
    // aerial perspective (Icelandic air is clear)
    if (tmax < 5000.0) { col = mix(col, l.hor, 1.0 - exp(-tmax * 0.0006 * l.haze)); }
    // the falling curtain in front of the cliff
    let fh = fall_hit(CAM, rd, l);
    if (fh.x > 0.0 && fh.x < tmax) {
        let fs = fall_shade(fh.y, fh.z, fh.x, rd, l, ctx);
        col = mix(col, fs.rgb, fs.a);
        let ic = icicles(fh.y, fh.z, l) * (1.0 - fs.a);
        col = mix(col, vec3f(0.7, 0.8, 0.88) * (l.amb * 1.4 + l.sun_c * 0.4), ic);
    }
    // spray cloud and its rainbow
    let m = mist_march(CAM, rd, tmax, l, ctx);
    col = col * m.w + m.rgb;
    let th = degrees(acos(clamp(dot(rd, -l.sun), -1.0, 1.0)));
    if (th > 36.0 && th < 56.0) {
        col += bow_color(th) * l.sun_c * (1.0 - m.w) * l.bow * 0.55;
    }
    return col * exp2(l.exposure);
}
