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

// World units are metres. We stand in a wheat field on the Plains; 18 km
// away a supercell's rotating updraft rises ten kilometres, its sides cut
// into stacked, spiralling plates. A wall cloud hangs under the flat base,
// the rain core falls to its left, and a farm with its windbreak sits in
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
            return Look(normalize(vec3f(0.3, 0.6, 0.5)), vec3f(0.012, 0.014, 0.02), vec3f(0.0015, 0.0018, 0.003), 1.0, 1.0, 0.75, 1.0,
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

const AXIS: vec2f = vec2f(3200.0, -19000.0);  // updraft centre (xz)
const BASE: f32 = 1400.0;                     // flat cloud base
const TOPY: f32 = 12000.0;

// signed distance to the storm (approximate), with its rotation at time t.
// fine adds the billows; the laminar mesocyclone stays smooth.
fn storm(p: vec3f, t: f32, fine: bool) -> f32 {
    let q = p.xz - AXIS;
    let r = length(q);
    let ang = atan2(q.y, q.x);
    let y = p.y;
    // the mesocyclone: an inverted bell of stacked, spiralling plates,
    // widening from the base up to ~6 km
    let h = (y - BASE) / 5000.0;
    // a wedding cake, not an onion: widest just above the flat base,
    // stepping in as it climbs into the tower
    var rm = 5600.0 + 500.0 * smoothstep(-0.05, 0.12, h) - 1900.0 * smoothstep(0.15, 1.05, h);
    // not a lathe-turned bell: lopsided, lumpier on one flank
    rm *= 1.0 + 0.1 * sin(ang * 2.0 + 1.3) + 0.07 * sin(ang * 3.0 + y / 1900.0);
    let warp = noise_value2(vec2f(ang * 1.4, y / 2300.0)) - 0.5;
    let ph = y / 700.0 + ang * 0.28 - t * 0.012 + warp * 0.9;
    let f = fract(ph);
    // sharp upper lip, sloping underside: each plate is a shelf; some
    // plates are bold, others barely there
    let groove = smoothstep(0.0, 0.12, f) * sstep(1.0, 0.35, f);
    let lean = 0.5 + 0.5 * cos(ang - 0.6);
    let bold = smoothstep(0.25, 0.75, noise_value2(vec2f(floor(ph) * 0.83, 0.5 + ang * 0.3)));
    var d = r - rm + (90.0 + 520.0 * lean * bold) * (1.0 - groove);
    d = max(d, y - (BASE + 5400.0) - 600.0 * cos(ang * 2.0 + 1.0));
    // the updraft tower above it, cumuliform, sheared downwind
    let sh = (y - 5000.0) * vec2f(-0.18, 0.1);
    let tq = q - sh;
    let tr_ = 3200.0 + 500.0 * sin(y / 1300.0 + 1.0) - 1200.0 * smoothstep(9000.0, 11800.0, y) + 900.0 * (noise_value3(p * 0.00022) - 0.5);
    var tower = max(length(tq) - tr_, max(4200.0 - y, y - 11800.0));
    tower = min(tower, length(vec3f(tq.x, (y - 11300.0) * 1.8, tq.y)) - 2600.0);
    // the anvil: a thin wedge fanning out downwind, flat underneath
    let aq = q + vec2f(7000.0, -2500.0);
    let ar = length(aq * vec2f(0.5, 0.85)) - 8500.0;
    let anvil = max(ar, max(10600.0 - y + 0.08 * max(-ar, 0.0) * 0.0, y - (11600.0 - 0.1 * max(ar + 8500.0 - 3000.0, 0.0))));
    var dt = min(tower, anvil * 0.8);
    if (fine) {
        // cumulus billows on the tower and anvil edge
        let n = noise_fbm3(p * 0.0006 + vec3f(0.0, -t * 0.002, 0.0), 3);
        dt += (n - 0.5) * 1300.0;
        d += (noise_value3(p * 0.00045) - 0.5) * 350.0;
    }
    d = min(d, dt);
    // flat base, broken only by the wall cloud hanging under the updraft
    d = max(d, BASE - y);
    let wq = vec2f(q.x + 700.0, q.y + 900.0);
    let wall = max(length(wq) - 1900.0 - 220.0 * sin(atan2(wq.y, wq.x) * 3.0 - t * 0.05), max(650.0 - y, y - BASE - 250.0));
    return min(d, wall);
}

// the bare shape (bell, tower, anvil) for the long light march: plates and
// billows are too small to matter for the large-scale shadowing
fn storm_lo(p: vec3f) -> f32 {
    let q = p.xz - AXIS;
    let y = p.y;
    let h = (y - BASE) / 5000.0;
    let rm = 5600.0 + 500.0 * smoothstep(-0.05, 0.12, h) - 1900.0 * smoothstep(0.15, 1.05, h);
    var d = max(length(q) - rm, y - (BASE + 5400.0));
    let tq = q - (y - 5000.0) * vec2f(-0.18, 0.1);
    d = min(d, max(length(tq) - 3200.0, max(4200.0 - y, y - 11800.0)));
    let aq = q + vec2f(7000.0, -2500.0);
    d = min(d, max(length(aq * vec2f(0.5, 0.85)) - 8500.0, abs(y - 11100.0) - 500.0) * 0.8);
    return max(d, BASE - y);
}

// soft storm radiance along a ray: sphere-trace to the shell, then integrate
// a thin volume. Light: a short march toward the sun through the coarse
// shape (the tower shades the plates and the base), a skylight term that
// darkens with depth under the storm, and the lightning when it fires.
// Returns (rgb, transmittance).
fn storm_march(ro: vec3f, rd: vec3f, l: Look, t: f32, flash: vec4f, n: i32) -> vec4f {
    // bound: the storm's cylinder
    let oc = ro.xz - (AXIS + vec2f(-3500.0, 1000.0));
    let d2 = rd.xz;
    let a = max(dot(d2, d2), 1e-8);
    let hb = dot(oc, d2);
    let disc = hb * hb - a * (dot(oc, oc) - 15500.0 * 15500.0);
    if (disc <= 0.0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let sq_ = sqrt(disc);
    var tt = max((-hb - sq_) / a, 0.0);
    var t1 = (-hb + sq_) / a;
    if (rd.y > 0.0) { t1 = min(t1, (TOPY + 800.0 - ro.y) / rd.y); }
    if (rd.y < 0.0) { t1 = min(t1, (550.0 - ro.y) / rd.y); }
    var tr = 1.0;
    var acc = vec3f(0.0);
    let shell = 850.0;
    let fwd = pow(saturate(dot(rd, l.sun) * 0.5 + 0.5), 6.0);
    for (var i = 0; i < 96; i++) {
        if (i >= n || tt > t1 || tr < 0.03) { break; }
        let p = ro + rd * tt;
        // the coarse shape first: billows only matter within ~1.3 km of it
        var d = storm(p, t, false);
        if (d < shell + 1400.0) { d = storm(p, t, true); }
        if (d < shell) {
            let dens = saturate((shell - d) / (2.0 * shell));
            // sun visibility: how deep the coarse storm lies toward the sun
            var occ = 0.0;
            for (var j = 1; j < 4; j++) {
                let sd = storm_lo(p + l.sun * (f32(j) * f32(j) * 600.0));
                occ += saturate(0.5 - sd / 1000.0) * (1.5 - f32(j) * 0.3);
            }
            // and a short look for the shelves: each lip shades the plate below
            let dn = storm(p + l.sun * 280.0, t, false) - storm(p, t, false);
            let sunv = exp(-occ * 1.4) * mix(0.15, 1.0, smoothstep(0.1, 0.9, 0.45 + dn / 280.0));
            let under = sstep(BASE + 3500.0, BASE + 200.0, p.y);
            // below the base (the wall cloud) only a little light gets in
            let belly = sstep(BASE + 250.0, BASE - 350.0, p.y);
            var c = l.sun_c * sunv * (0.35 + 0.9 * fwd) * 1.3 * (1.0 - 0.8 * belly);
            // skylight: open sky above, the dark belly below
            c += l.amb * mix(0.13, 0.04, under) * mix(vec3f(1.0), l.tint, under);
            if (flash.w > 0.0) {
                let fd = length(p - flash.xyz);
                c += vec3f(0.75, 0.8, 1.0) * flash.w * 1.4 * exp(-fd / 1700.0) / (1.0 + sq(fd / 2500.0));
            }
            let step_len = max(d * 0.45, 160.0);
            let st = exp(-dens * step_len * 0.0024);
            acc += tr * c * (1.0 - st);
            tr *= st;
            tt += step_len;
        } else {
            tt += (d - shell * 0.5) * 0.75;
        }
    }
    return vec4f(acc, tr);
}

// the forward-flank rain shaft: a grey curtain falling from the base
fn precip_shaft(ro: vec3f, rd: vec3f, l: Look, t: f32, flash: vec4f, tmax: f32) -> vec4f {
    let c = AXIS + vec2f(-8800.0, 900.0);
    let oc = ro.xz - c;
    let d2 = rd.xz;
    let a = max(dot(d2, d2), 1e-8);
    let hb = dot(oc, d2);
    let rr = 3900.0;
    let disc = hb * hb - a * (dot(oc, oc) - rr * rr);
    if (disc <= 0.0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    let sq_ = sqrt(disc);
    let t0 = max((-hb - sq_) / a, 0.0);
    let t1 = min((-hb + sq_) / a, tmax);
    if (t1 <= t0) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    var tr = 1.0;
    var acc = vec3f(0.0);
    let dt = (t1 - t0) / 8.0;
    for (var i = 0; i < 8; i++) {
        let p = ro + rd * (t0 + (f32(i) + 0.5) * dt);
        if (p.y > BASE + 100.0 || p.y < 0.0) { continue; }
        let r = length(p.xz - c) / rr;
        // streaky curtain, slanting with the wind, thinning at its edges
        let streak = noise_value2(vec2f((p.x + p.y * 0.35) * 0.0035, t * 0.02)) * 0.7 + 0.3 * noise_value2(vec2f((p.x + p.y * 0.35) * 0.012, 3.0));
        let dens = sstep(1.0, 0.3, r + (streak - 0.5) * 0.5) * sstep(BASE + 100.0, BASE - 400.0, p.y);
        let lc = (l.amb * 0.22 + l.sun_c * 0.03) * l.tint + vec3f(0.6, 0.65, 0.8) * flash.w * 0.06;
        let st = exp(-dens * dt * 0.0013);
        acc += tr * lc * (1.0 - st);
        tr *= st;
    }
    return vec4f(acc, tr);
}

// lightning: rare flashes deep in the tower; (position, intensity)
fn lightning(t: f32, l: Look) -> vec4f {
    let ev = hash_event(t, 5.0, 0x51a7u);
    if (ev.x > l.flash_p) { return vec4f(0.0); }
    let k = u32(i32(ev.z));
    let t_in = ev.y * 5.0 - 1.5 * hash_f(k * 13u + 1u);
    if (t_in < 0.0) { return vec4f(0.0); }
    // a stroke and its re-strokes, dying away in under a second
    let flick = exp(-t_in * 9.0) + 0.6 * exp(-max(t_in - 0.12, 0.0) * 14.0) * step(0.12, t_in)
              + 0.4 * exp(-max(t_in - 0.3, 0.0) * 12.0) * step(0.3, t_in);
    let pos = vec3f(AXIS.x + (hash_f(k * 7u + 3u) - 0.5) * 7000.0 - 1500.0, 2500.0 + 5000.0 * hash_f(k * 5u + 2u),
                    AXIS.y + (hash_f(k * 11u + 4u) - 0.5) * 3000.0);
    return vec4f(pos, flick);
}

// ------------------------------------------------------------------ land

fn sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    if (l.night > 0.5) {
        var c = sky_night(rd) * 1.2;
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

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(0.0, 1.8, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.06, 0.14, -1.0), 0.0, 58.0);
    let rd = cam_ray(cam, p);
    let pxa = ctx.px / cam.zoom;
    var flash = lightning(ctx.t, l);
    // by day a flash is a faint pulse inside the sunlit cloud
    flash.w *= mix(0.2, 1.0, l.night);
    var col: vec3f;
    var tg = -1.0;
    if (rd.y < 0.0) { tg = (0.9 - ro.y) / rd.y; }
    if (tg > 0.0) {
        let hp = ro + rd * tg;
        // the storm's shadow reaches the far fields
        let under = sstep(-6000.0, -13000.0, hp.z);
        col = wheat(hp, rd, tg, pxa, l, ctx, 1.0 - 0.8 * under);
        col += vec3f(0.5, 0.55, 0.7) * flash.w * 0.04 * under;
        let hz = air_col(normalize(vec3f(rd.x, 0.01, rd.z)), l);
        col = mix(col, hz * mix(vec3f(1.0), l.tint * 0.8, under * 0.6), 1.0 - exp(-tg * 0.00012));
    } else {
        col = sky(rd, l, ctx);
        // the air beneath the anvil lies in its shadow
        let az = atan2(rd.x, -rd.z);
        let under_anvil = smoothstep(-0.9, -0.4, az) * sstep(0.55, 0.2, az) * sstep(0.5, 0.15, rd.y);
        col *= 1.0 - 0.45 * under_anvil * (1.0 - l.night);
    }
    // storm and rain in front of the sky and the far fields
    let tmax = select(1e7, tg, tg > 0.0);
    let rs = precip_shaft(ro, rd, l, ctx.t, flash, tmax);
    col = col * rs.w + rs.rgb;
    let st = storm_march(ro, rd, l, ctx.t, flash, steps(64.0, ctx));
    // aerial perspective over 18 km of humid air
    let air = air_col(normalize(vec3f(rd.x, 0.05, rd.z)), l);
    col = col * st.w + mix(st.rgb, air * (1.0 - st.w), 0.16);
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
                let fcol = col_hex(0x4a4036u) * (l.sun_c * 0.35 + l.amb * 0.6) + vec3f(0.5, 0.55, 0.7) * flash.w * 0.02;
                col = mix(col, fcol, fcov);
            }
        }
    }
    return col * exp2(l.exposure);
}
