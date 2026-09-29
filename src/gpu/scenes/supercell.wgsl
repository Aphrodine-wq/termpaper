//! name: supercell
//! title: Great Plains Supercell
//! category: weather
//! tags: storm, supercell, tornado alley, wheat, lightning
//! desc: a rotating supercell towers over golden wheat, rain shaft and wall cloud, a lone farm
//! themes: afternoon, dusk, night
//! uses: camera, sky, stars
//! cost: heavy
//! fallback: clouds
//! credits: original

// World units are metres. We stand in a wheat field on the Plains, the sun
// low behind our right shoulder. Twenty kilometres out a supercell's
// updraft climbs twelve kilometres: its rotating lower half is cut into
// hard, corkscrewing shelves; above, the tower boils into cauliflower and
// punches through the anvil, which streams off downwind over our heads.
// Under the flat, dark, rain-free base a wall cloud hangs low; beside it the
// forward-flank core drops a grey curtain of rain and hail. A line of
// younger towers steps down to the right. A farm and its windbreak sit in
// the middle distance for scale.

struct Look {
    sun: vec3f,
    sun_c: vec3f,     // precomputed sky-model light (with ozone)
    amb: vec3f,
    haze: f32,
    night: f32,
    flash_p: f32,     // chance of lightning per slot
    exposure: f32,
    tint: vec3f,      // storm light under the base (the green of a hail core)
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(sky_sun_dir(3.0, 112.0), vec3f(1.28, 0.38, 0.074), vec3f(0.12, 0.14, 0.21), 1.6, 0.0, 0.3, 0.45,
                        vec3f(0.55, 0.85, 0.62));
        }
        case 2u: {
            return Look(normalize(vec3f(0.3, 0.6, 0.5)), vec3f(0.009, 0.01, 0.014), vec3f(0.0018, 0.0022, 0.0036), 1.0, 1.0, 0.32, 1.0,
                        vec3f(0.8, 0.85, 1.0));
        }
        default: {
            return Look(sky_sun_dir(17.0, 105.0), vec3f(2.61, 1.93, 1.45), vec3f(0.256, 0.386, 0.55), 1.4, 0.0, 0.18, -0.75,
                        vec3f(0.8, 0.92, 0.88));
        }
    }
}

fn ozone_t(sun_y: f32) -> vec3f {
    let am = inverseSqrt(sun_y * sun_y + 0.00786);
    return exp(-vec3f(0.0195, 0.0564, 0.00256) * am);
}

// ------------------------------------------------------------------ storm

const UPD: vec2f = vec2f(2200.0, -16000.0);  // updraft centre at the base (xz)
const BASE: f32 = 1500.0;                    // the flat rain-free base
const ANVIL: f32 = 12200.0;                  // anvil top
const WIND: vec2f = vec2f(-0.76, 0.65);      // downwind: left and toward us
const SOFT: f32 = 200.0;                     // edge softness of the cloud

// the updraft leans downshear as it climbs
fn upd_axis(y: f32) -> vec2f {
    return UPD + vec2f(-0.13, -0.05) * max(y - BASE, 0.0);
}

// round heads, sharp creases: |gradient noise| summed over three scales
// (fine = false drops the smallest scale: for stepping toward the skin)
fn billow(p: vec3f, fine: bool) -> f32 {
    let a = abs(noise_grad3(p * (1.0 / 2300.0)));
    let b = abs(noise_grad3(p * (1.0 / 1050.0) + vec3f(5.1, 2.3, 7.7)));
    var s = a * 900.0 + b * 380.0;
    if (fine) { s += abs(noise_grad3(p * (1.0 / 480.0) + vec3f(1.9, 8.1, 3.3))) * 150.0; } else { s += 75.0; }
    return s;
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = saturate(0.5 + 0.5 * (b - a) / k);
    return mix(b, a, h) - k * h * (1.0 - h);
}

// radius of the updraft: a broad rotating bell over the base, narrowing
// into the tower, pinched in under the overshooting top
fn upd_r(y: f32) -> f32 {
    let h = saturate((y - BASE) / 5200.0);
    return 3700.0 + 2500.0 * (1.0 - smoothstep(0.05, 1.0, h)) + 300.0 * sin(y / 1500.0 + 1.0) - 1200.0 * smoothstep(9500.0, 11800.0, y);
}

// the anvil: flat on top, streaming off downwind and widening
fn anvil(p: vec3f) -> f32 {
    let at = p.xz - upd_axis(11000.0);
    let al = dot(at, WIND);
    let ac = dot(at, vec2f(WIND.y, -WIND.x));
    let thick = mix(2000.0, 600.0, saturate(al / 45000.0));
    let top = ANVIL - 0.01 * max(al, 0.0);
    return max(max(abs(ac) - (5000.0 + 0.5 * max(al, 0.0)), -al - 3600.0), max(p.y - top, top - thick - p.y));
}

// one tower of the flanking line: a tapering column with a domed head
fn flank(p: vec3f, i: i32) -> f32 {
    let fi = f32(i);
    let c = UPD + vec2f(7000.0 + 3000.0 * fi, 2600.0 + 1300.0 * fi);
    let ht = 6900.0 - 2300.0 * fi;
    let u = saturate((p.y - BASE) / (ht - BASE));
    let rr = (1800.0 - 300.0 * fi) * (1.0 - 0.3 * u);
    let yy = p.y - clamp(p.y, BASE + 200.0, ht - rr);
    return max(length(vec3f(p.x - c.x, yy, p.z - c.y)) - rr, BASE + 200.0 - p.y);
}

// the bare shapes, no detail: a cheap bound for skipping empty air and the
// long shadow probes (billows add at most ~1.5 km)
fn storm_lo(p: vec3f) -> f32 {
    let y = p.y;
    let a = p.xz - upd_axis(y);
    var d = max(length(a) - upd_r(y), max(BASE - y, y - 11600.0));
    d = min(d, length(vec3f(a.x, (y - 11800.0) * 1.7, a.y)) - 2400.0);
    d = min(d, anvil(p));
    d = min(d, min(flank(p, 0), flank(p, 1)));
    return d;
}

// a lower bound of storm(): each bare shape less the most its detail can
// push it out, the flat base exact (it carries no detail), for skipping
fn storm_bound(p: vec3f) -> f32 {
    let y = p.y;
    let a = p.xz - upd_axis(y);
    let r = length(a);
    let hi = smoothstep(5000.0, 7800.0, y);
    var d = max(r - upd_r(y) - mix(1100.0, 1450.0, hi), y - 11600.0 - 1450.0);
    d = min(d, length(vec3f(a.x, (y - 11800.0) * 1.7, a.y)) - 2400.0 - 1500.0);
    d = min(d, anvil(p) - 1000.0);
    d = min(d, min(flank(p, 0), flank(p, 1)) - 1750.0);
    d = max(d, BASE - y);
    // the wall cloud hangs below the base
    let wq = p.xz - (UPD + vec2f(-1700.0, 600.0));
    d = min(d, max(length(wq) - 2900.0, max(350.0 - y, y - BASE - 150.0)));
    return d;
}

// the full cloud field: signed distance (m), negative inside
fn storm(p: vec3f, t: f32, fine: bool) -> vec2f {
    let y = p.y;
    let a = p.xz - upd_axis(y);
    let r = length(a);
    let ang = atan2(a.y, a.x);
    let h = saturate((y - BASE) / 5200.0);
    let bil = billow(p + vec3f(0.0, -t * 4.0, 0.0), fine);
    // --- the updraft: its rotating lower half cut into hard shelves that
    // corkscrew up around it; higher up it boils into cauliflower
    // (angle-dependent terms use the direction, not the angle, so nothing
    // seams where atan2 wraps; the helix makes a whole number of turns)
    let striae = sstep(7000.0, 5600.0, y);
    // (lobed, not lathe-turned: whole multiples of the angle, so no seam)
    var d = r - upd_r(y) * (1.0 + 0.09 * sin(2.0 * ang + 0.7 + h * 2.0) + 0.05 * sin(3.0 * ang + 1.9 - h * 3.0));
    if (striae > 0.0) {
        let cs = a / max(r, 1.0);
        let wv = noise_value3(vec3f(cs * 1.4, y / 2200.0)) - 0.5;
        let ph = y / 720.0 + ang * (7.0 / TAU) - t * 0.02 + wv * 0.8;
        let f = fract(ph);
        // each band: a sharp lip on top, its face sloping back underneath;
        // most bands bold, a few fading out around the column
        let bold = 0.3 + 0.7 * smoothstep(0.1, 0.55, noise_value3(vec3f(cs * 1.1, floor(ph) * 0.71)));
        d += (300.0 - (f * f) * sstep(1.0, 0.86, f) * 850.0 * bold) * striae;
    }
    // turbulence tearing at the shelves
    if (fine) { d += (noise_value3(p * (1.0 / 800.0) + vec3f(0.0, -t * 0.002, 0.0)) - 0.5) * 380.0; }
    d -= bil * mix(0.22, 1.0, smoothstep(5000.0, 7800.0, y));
    d = max(d, BASE - y);
    let top = max(d, y - 11600.0);
    d = smin(top, length(vec3f(a.x, (y - 11800.0) * 1.7, a.y)) - 2400.0 - bil, 500.0);
    // --- the anvil, fraying at its edges
    let dav = anvil(p);
    if (dav < 2500.0) {
        let at = p.xz - upd_axis(11000.0);
        let fib = noise_value2(vec2f(dot(at, WIND) * 0.00012, dot(at, vec2f(WIND.y, -WIND.x)) * 0.0006)) - 0.5;
        d = smin(d, dav + fib * 900.0 - bil * 0.35, 900.0);
    }
    // --- the flanking line
    let fk = min(flank(p, 0), flank(p, 1));
    d = smin(d, fk - bil * 1.2, 700.0);
    // --- the flat base, broken only by the wall cloud under the updraft
    d = max(d, BASE - y);
    // (it turns slowly, its ragged edge drifting round)
    if (y < BASE + 600.0) {
        let wq = p.xz - (UPD + vec2f(-1700.0, 600.0));
        let wl = length(wq);
        let rag = noise_value3(vec3f(rot2(t * 0.02) * (wq / max(wl, 1.0)) * 1.6, y / 280.0));
        let wall = max(wl - 2500.0 - 600.0 * (rag - 0.5), max(500.0 + 300.0 * rag - y, y - BASE - 150.0));
        d = min(d, wall);
    }
    // (x: distance; y: how far the billows push out here, for crease shading)
    return vec2f(d, bil);
}

struct Flash { pos: vec3f, w: f32, cg: f32, seed: f32 }

// lightning: rare strokes in the storm, some reaching the ground in a
// visible channel under the base. Closed-form in time.
fn lightning(t: f32, l: Look) -> Flash {
    var fl: Flash;
    fl.pos = vec3f(0.0);
    fl.w = 0.0;
    fl.cg = 0.0;
    fl.seed = 0.0;
    let ev = hash_event(t, 7.0, 0x51a7u);
    if (ev.x > l.flash_p) { return fl; }
    let k = u32(i32(ev.z));
    let t_in = ev.y * 7.0 - 2.5 * hash_f(k * 13u + 1u);
    if (t_in < 0.0) { return fl; }
    // a stroke and its re-strokes, dying away in under a second
    fl.w = exp(-t_in * 9.0) + 0.7 * exp(-max(t_in - 0.14, 0.0) * 12.0) * step(0.14, t_in)
         + 0.45 * exp(-max(t_in - 0.33, 0.0) * 11.0) * step(0.33, t_in);
    let hx = hash_f(k * 7u + 3u);
    fl.pos = vec3f(UPD.x - 5500.0 + 7500.0 * hx, 2800.0 + 5000.0 * hash_f(k * 5u + 2u), UPD.y + (hash_f(k * 11u + 4u) - 0.5) * 4000.0);
    fl.cg = step(0.45, hash_f(k * 17u + 9u));
    fl.seed = f32(k % 997u);
    return fl;
}

// march the storm: sphere-trace to its skin, then integrate a thin, dense
// volume. Light: the sun on the side of each billow that faces it (the SDF's
// change toward the sun), long shadows from the bare shapes, skylight that
// dies away under the base, and lightning from inside. (rgb, transmittance)
struct Hit { c: vec3f, tr: f32, t: f32 }

fn storm_march(ro: vec3f, rd: vec3f, l: Look, t: f32, fl: Flash, jit: f32, n: i32) -> Hit {
    var hit: Hit;
    hit.c = vec3f(0.0);
    hit.tr = 1.0;
    hit.t = 1e7;
    // bound: a big vertical cylinder around the storm and its anvil
    let oc = ro.xz - (UPD + vec2f(-4000.0, 6000.0));
    let d2 = rd.xz;
    let a = max(dot(d2, d2), 1e-8);
    let hb = dot(oc, d2);
    let disc = hb * hb - a * (dot(oc, oc) - 24000.0 * 24000.0);
    if (disc <= 0.0) { return hit; }
    let sq_ = sqrt(disc);
    var tt = max((-hb - sq_) / a, 0.0);
    var t1 = (-hb + sq_) / a;
    if (rd.y > 0.0) { t1 = min(t1, (ANVIL + 2500.0 - ro.y) / rd.y); }
    if (rd.y < 0.0) { t1 = min(t1, (600.0 - ro.y) / rd.y); }
    if (rd.y > 0.0) { tt = max(tt, (500.0 - ro.y) / rd.y); }
    var tr = 1.0;
    var acc = vec3f(0.0);
    let fwd = pow(saturate(dot(rd, l.sun) * 0.5 + 0.5), 5.0);
    let fcol = vec3f(0.75, 0.8, 1.0) * fl.w;
    var first = true;
    for (var i = 0; i < 110; i++) {
        if (i >= n || tt > t1 || tr < 0.02) { break; }
        let p = ro + rd * tt;
        // far from the bare shapes nothing can be there
        let dl = storm_bound(p);
        if (dl > SOFT) {
            tt += dl;
            continue;
        }
        // approach on the coarse field (fine detail moves the skin < 300 m)
        let dc = storm(p, t, false).x;
        if (dc > SOFT + 300.0) {
            tt += max((dc - SOFT * 0.5 - 300.0) * 0.6, 50.0);
            continue;
        }
        let sf = storm(p, t, true);
        let d = sf.x;
        if (d > SOFT) {
            tt += max((d - SOFT * 0.5) * 0.6, 50.0);
            continue;
        }
        if (first) {
            // entering the skin: dither the first sample so the edge does not band
            first = false;
            hit.t = tt;
            tt += jit * 60.0;
        }
        let dens = saturate(0.5 - d / (2.0 * SOFT));
        // sun on this face: how the cloud recedes toward the light
        // (on the coarse field: the finest billows are below a pixel's reach)
        let ds = (storm(p + l.sun * 320.0, t, false).x - dc) / 320.0;
        let face = smoothstep(-0.25, 0.85, ds);
        // long shadows: the bare storm between here and the sun
        let occ = saturate(0.4 - storm_lo(p + l.sun * 1300.0) / 1500.0) + saturate(0.4 - storm_lo(p + l.sun * 4500.0) / 2000.0);
        // deep in the creases between billows little light reaches
        let crease = mix(0.45, 1.0, smoothstep(200.0, 800.0, sf.y));
        let sunv = face * exp(-occ * 2.0) * mix(1.0, crease, 0.5);
        // skylight: open above, closed in under the base and the anvil
        let above = storm_lo(p + vec3f(0.0, 1800.0, 0.0));
        let open = saturate(0.3 + above / 3000.0) * smoothstep(BASE - 300.0, BASE + 3000.0, p.y);
        let under = sstep(BASE + 400.0, BASE - 200.0, p.y);
        var c = l.sun_c * sunv * (1.05 + 0.6 * fwd) * (1.0 - 0.85 * under);
        c += l.amb * mix(0.04, 0.34, open) * crease * mix(vec3f(1.0), l.tint, under * 0.8);
        if (fl.w > 0.0) {
            let fd = length(p - fl.pos);
            // the stroke lights the cloud from inside: a glow around it, and
            // the billows facing it
            let fdir = (fl.pos - p) / max(fd, 1.0);
            let ff = smoothstep(-0.5, 0.8, (storm(p + fdir * 320.0, t, false).x - dc) / 320.0);
            c += fcol * (2.2 * exp(-fd / 1800.0) + 0.9 * ff / (1.0 + sq(fd / 4000.0)));
        }
        let step_len = select(200.0, 320.0, d < -SOFT);
        let st = exp(-dens * step_len * 0.006);
        acc += tr * c * (1.0 - st);
        tr *= st;
        tt += step_len;
    }
    hit.c = acc;
    hit.tr = tr;
    return hit;
}

// the forward-flank core: a grey wall of rain and hail from the ground up
// into the storm, streaked and slanting with the outflow; (rgb, transmittance)
const RAIN_TOP: f32 = 10500.0;
fn precip_shaft(ro: vec3f, rd: vec3f, l: Look, t: f32, fl: Flash, tmax: f32) -> vec4f {
    let c = UPD + vec2f(-8600.0, 1500.0);
    let oc = ro.xz - c;
    let d2 = rd.xz;
    let a = max(dot(d2, d2), 1e-8);
    let hb = dot(oc, d2);
    let rr = 5400.0;
    let disc = hb * hb - a * (dot(oc, oc) - rr * rr);
    if (disc <= 0.0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let sq_ = sqrt(disc);
    let t0 = max((-hb - sq_) / a, 0.0);
    var t1 = min((-hb + sq_) / a, tmax);
    if (rd.y > 0.0) { t1 = min(t1, (RAIN_TOP - ro.y) / rd.y); }
    if (t1 <= t0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    var tr = 1.0;
    var acc = vec3f(0.0);
    let dt = (t1 - t0) / 10.0;
    for (var i = 0; i < 10; i++) {
        let p = ro + rd * (t0 + (f32(i) + 0.5) * dt);
        if (p.y > RAIN_TOP || p.y < 0.0) { continue; }
        let q = p.xz - c + vec2f(p.y * 0.35, 0.0);
        let r = length(q * vec2f(1.0, 1.3)) / rr;
        // streaky curtain, slanting with the wind, ragged at its edges
        let sx = q.x + p.y * 0.3;
        let streak = noise_value2(vec2f(sx * 0.0016, t * 0.03)) * 0.65 + 0.35 * noise_value2(vec2f(sx * 0.0045, 3.0 + p.y * 0.0003));
        // dense low down, thinning upward into the dark mass under the anvil
        let hfade = mix(1.0, 0.45, smoothstep(1500.0, 6000.0, p.y)) * sstep(RAIN_TOP, RAIN_TOP - 2500.0, p.y);
        let rag = noise_value2(vec2f(sx * 0.0005, p.y * 0.00025 + 7.0)) - 0.5;
        let dens = sstep(1.0, 0.3, r + (streak - 0.5) * 0.5 + rag * 0.5) * hfade * (0.45 + 0.55 * streak);
        // in the storm's shadow: grey-blue, the hail core faintly green;
        // the sun catches its flank toward the updraft
        let sunlit = smoothstep(-0.2, 0.9, q.x / rr) * 0.08;
        let lc = (l.amb * (0.05 + 0.06 * hfade) + l.sun_c * (0.01 + sunlit)) * l.tint + vec3f(0.6, 0.65, 0.8) * fl.w * 0.15;
        let st = exp(-dens * dt * 0.0021);
        acc += tr * lc * (1.0 - st);
        tr *= st;
    }
    return vec4f(acc, tr);
}

// ------------------------------------------------------------------ land

fn sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    if (l.night > 0.5) {
        var c = sky_night(rd) * 1.2;
        // the glow of a distant town low on the horizon
        c += vec3f(0.02, 0.011, 0.005) * exp(-max(rd.y, 0.0) * 40.0) * exp(-sq((rd.x + 0.55) * 2.2));
        c += star_field(rd, 0.6, ctx) * 0.6 * smoothstep(0.02, 0.12, rd.y);
        return c;
    }
    return air_col(rd, l) + sky_sun_disk(rd, l.sun, 0.53);
}

// the sky without stars or sun: the colour of the air, for haze
fn air_col(rd: vec3f, l: Look) -> vec3f {
    if (l.night > 0.5) { return sky_night(rd) * 1.2; }
    var c = sky_atmosphere_haze(rd, l.sun, l.haze) * ozone_t(l.sun.y);
    c += vec3f(0.06, 0.07, 0.08) * exp(-max(rd.y, 0.0) * 18.0) * saturate(l.sun.y * 6.0);
    return c;
}

// the farm: house, barn and a windbreak of trees, 460 m out
const FARM_Z: f32 = -460.0;
fn farm(x: f32, y: f32) -> vec2f {
    // (coverage-ready signed distance, material: 0 trees, 1 walls, 2 roof)
    var d = 1e5;
    var m = 0.0;
    // cottonwood windbreak: a row of rounded crowns
    let tx = x + 250.0;
    let cell = floor(tx / 9.0);
    let h = hash_f(u32(i32(cell) + 1000) * 2654435761u);
    let cx = (cell + 0.5) * 9.0;
    if (tx > -40.0 && tx < 60.0) {
        let crown = length(vec2f((tx - cx) / 1.1, y - 11.0 - 4.0 * h)) - 6.0 - 2.0 * h;
        let trunk = max(abs(tx - cx) - 0.5, y - 9.0);
        d = min(crown, trunk);
    }
    // the house: gable roof
    let hx = x + 196.0;
    let walls = max(abs(hx) - 6.5, max(-y, y - 5.5));
    let roof = max(abs(hx) * 0.55 + (y - 8.0), max(5.2 - y, abs(hx) - 7.2));
    if (walls < d) { d = walls; m = 1.0; }
    if (roof < d) { d = roof; m = 2.0; }
    // the barn and its silo
    let bx = x + 170.0;
    let barn = max(abs(bx) - 9.0, max(-y, y - 7.0 - 3.0 * saturate(1.0 - abs(bx) / 9.0)));
    if (barn < d) { d = barn; m = 2.0; }
    let silo = max(abs(x + 156.0) - 2.2, max(-y, y - 15.0));
    if (silo < d) { d = silo; m = 1.0; }
    return vec2f(d, m);
}

fn wheat(p: vec3f, rd: vec3f, t_hit: f32, pxa: f32, l: Look, ctx: Ctx, shade_k: f32) -> vec3f {
    // wind gusts sweep across the field as travelling waves of sheen
    let gust = noise_value2(vec2f(p.x * 0.008 + ctx.t * 0.14, p.z * 0.014 + ctx.t * 0.05)) * 0.7 + 0.3 * noise_value2(vec2f(p.x * 0.03 + ctx.t * 0.3, p.z * 0.05));
    // heads and gaps of the canopy, faded with the pixel footprint
    let fp = t_hit * pxa;
    let fine = saturate(1.0 - fp / 0.06);
    let tex = mix(0.5, noise_value2(p.xz * vec2f(14.0, 9.0)), fine);
    // looking down into the near crop we see shaded stems; further out only
    // the sunlit heads
    let near = exp(-t_hit / 25.0);
    var alb = mix(col_hex(0xa8843fu), col_hex(0xe0c285u), saturate(0.25 + 0.3 * (tex - 0.5) + 0.7 * (gust - 0.3)));
    alb *= 1.0 - 0.35 * near;
    // tramlines: the tractor's wheel tracks run out to the horizon
    let tl = abs(fmod_pos(p.x + 3.0, 24.0) - 12.0);
    let track = sstep(0.35 + fp, 0.15, abs(tl - 0.9)) * saturate(1.0 - fp / 1.5);
    alb *= 1.0 - 0.45 * track;
    // stubble and a darker strip of fallow further out
    alb = mix(alb, col_hex(0x6d5a3cu), smoothstep(0.62, 0.7, noise_value2(p.xz * vec2f(0.004, 0.0015))) * 0.6);
    let sheen = 0.85 + 0.35 * smoothstep(0.4, 0.8, gust);
    let dif = saturate(l.sun.y * 0.8 + 0.2);
    return alb * (l.sun_c * dif * shade_k * sheen * 0.8 + l.amb * 0.9);
}

// a cloud-to-ground channel, jagged, drawn in screen space between the
// projected ends of the stroke; returns its brightness at p
fn bolt(p: vec2f, cam: Cam, fl: Flash, ctx: Ctx) -> f32 {
    let top = cam_project(cam, vec3f(fl.pos.x, BASE - 50.0, fl.pos.z));
    let bot = cam_project(cam, vec3f(fl.pos.x + 900.0 * (fract(fl.seed * 0.173) - 0.5), 0.0, fl.pos.z + 600.0));
    if (top.z <= 0.0 || bot.z <= 0.0) { return 0.0; }
    let s = (p.y - bot.y) / max(top.y - bot.y, 1e-4);
    if (s < -0.05 || s > 1.02) { return 0.0; }
    let sc = saturate(s);
    let span = top.y - bot.y;
    var x = mix(bot.x, top.x, sc);
    x += span * (0.16 * noise_grad2(vec2f(sc * 5.0, fl.seed)) + 0.07 * noise_grad2(vec2f(sc * 13.0, fl.seed + 3.0)) + 0.03 * noise_grad2(vec2f(sc * 31.0, fl.seed + 7.0)));
    let dx = abs(p.x - x);
    let w = ctx.px * 0.7;
    return exp(-dx * dx / (w * w)) + 0.25 * exp(-dx / (ctx.px * 5.0));
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(0.0, 1.8, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.05, 0.2, -1.0), 0.0, 58.0);
    let rd = cam_ray(cam, p);
    let pxa = ctx.px / cam.zoom;
    var fl = lightning(ctx.t, l);
    // by day a flash is a faint pulse inside the sunlit cloud
    fl.w *= mix(0.25, 1.0, l.night);
    // the colour of the humid air toward the horizon
    let air = air_col(normalize(vec3f(rd.x, max(rd.y, 0.0) * 0.5 + 0.03, rd.z)), l);
    var col: vec3f;
    var tg = -1.0;
    if (rd.y < 0.0) { tg = (0.9 - ro.y) / rd.y; }
    if (tg > 0.0) {
        let hp = ro + rd * tg;
        // the far fields lie in the shadow of the storm and its anvil
        var occ = 0.0;
        if (tg > 2000.0) {
            for (var j = 1; j < 4; j++) {
                let fj = f32(j);
                occ += saturate(0.5 - storm_lo(hp + l.sun * (fj * 4000.0)) / 2500.0);
            }
        }
        let shade = exp(-occ * 1.5);
        col = wheat(hp, rd, tg, pxa, l, ctx, shade);
        // a flash lights the whole field for an instant
        col += col_hex(0xc8b68au) * vec3f(0.5, 0.55, 0.7) * fl.w * 0.06 / (1.0 + tg / 6000.0);
        col = mix(col, air * mix(vec3f(1.0), l.tint * 0.7, (1.0 - shade) * 0.6), 1.0 - exp(-tg * 0.00012));
    } else {
        col = sky(rd, l, ctx);
        // the air beneath the anvil lies in its shadow
        let az = atan2(rd.x, -rd.z);
        let under_anvil = smoothstep(-0.9, -0.4, az) * sstep(0.55, 0.2, az) * sstep(0.5, 0.15, rd.y);
        col *= 1.0 - 0.45 * under_anvil * (1.0 - l.night);
    }
    // the storm in front of the sky and the far fields, and the rain in
    // front of whatever of the storm lies behind it
    let tmax = select(1e7, tg, tg > 0.0);
    let st = storm_march(ro, rd, l, ctx.t, fl, ctx.jitter, steps(60.0, ctx));
    // aerial perspective over twenty kilometres of humid air
    col = col * st.tr + mix(st.c, air * (1.0 - st.tr), 0.2);
    let rs = precip_shaft(ro, rd, l, ctx.t, fl, min(tmax, st.t + 800.0));
    col = col * rs.w + mix(rs.rgb, air * (1.0 - rs.w), 0.15);
    // a cloud-to-ground stroke under the base
    if (fl.w > 0.02 && fl.cg > 0.5) {
        col += vec3f(0.85, 0.85, 1.0) * bolt(p, cam, fl, ctx) * fl.w * 8.0 * rs.w;
    }
    // the farm, silhouetted or sunlit
    if (rd.z < -0.01) {
        let tf = (FARM_Z - ro.z) / rd.z;
        let fp = ro + rd * tf;
        if (fp.y > -1.0 && fp.y < 30.0 && (tg < 0.0 || tg > tf)) {
            let f = farm(fp.x, fp.y);
            let cov = saturate(0.5 - f.x / (pxa * tf));
            if (cov > 0.0) {
                var fc = col_hex(0x2a3320u) * (l.sun_c * 0.25 + l.amb * 0.5);
                if (f.y > 0.5) { fc = col_hex(0xc8c0b0u) * (l.sun_c * 0.45 + l.amb * 0.6); }
                if (f.y > 1.5) { fc = col_hex(0x5a3a30u) * (l.sun_c * 0.35 + l.amb * 0.5); }
                fc += vec3f(0.3, 0.32, 0.4) * fl.w * 0.05;
                fc = mix(fc, air, 1.0 - exp(-tf * 0.00012));
                // a lit window at night
                if (l.night > 0.5 && f.y > 0.5 && f.y < 1.5 && abs(fp.x + 194.0) < 1.0 && abs(fp.y - 2.6) < 0.7) {
                    fc = vec3f(1.6, 0.9, 0.35);
                }
                col = mix(col, fc, cov);
            }
        }
    }
    // a fence line running out across the field
    let fn_ = vec3f(1.0, 0.0, 0.16);
    let den = dot(rd, fn_);
    if (abs(den) > 1e-4) {
        let tfe = -(dot(ro, fn_) - 7.0) / den;
        if (tfe > 1.0 && tfe < 400.0) {
            let q = ro + rd * tfe;
            let w = pxa * tfe;
            let post = abs(fmod_pos(q.z, 4.5) - 2.25) - 0.06;
            var fcov = saturate(0.5 - post / w) * step(0.0, q.y) * step(q.y, 1.25);
            for (var k = 0; k < 3; k++) {
                let wy = 0.55 + 0.3 * f32(k);
                fcov = max(fcov, saturate(0.5 - (abs(q.y - wy) - 0.006) / w) * 0.8);
            }
            if (fcov > 0.0) {
                let fcol = col_hex(0x4a4036u) * (l.sun_c * 0.35 + l.amb * 0.6) + vec3f(0.5, 0.55, 0.7) * fl.w * 0.02;
                col = mix(col, fcol, fcov);
            }
        }
    }
    return col * exp2(l.exposure);
}
