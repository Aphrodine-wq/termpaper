//! name: hongkong
//! title: Victoria Harbour
//! category: city
//! tags: hong kong, skyline, harbour, night, reflections, ferry
//! desc: Hong Kong Island's towers across Victoria Harbour, lights streaking on the water
//! themes: night, bluehour, fog
//! uses: camera, sdf, sky, clouds, water, light, noise
//! cost: medium
//! tonemap: aces
//! fallback: city
//! credits: original

// World units are metres. The camera stands on the Tsim Sha Tsui promenade,
// 5.5 m above the water, looking south across the harbour (+z). The island
// is a stack of vertical planes, front to back: the waterfront, rows of
// towers, the Mid-Levels climbing the hill, the Peak ridge. Each plane holds
// procedural towers plus a few hand-built landmarks; a ray is intersected
// with the planes analytically and composited front to back with coverage,
// so silhouettes stay anti-aliased at any size. The harbour reflects the
// same skyline along wave-tilted mirror rays, spread vertically the way
// chop at a grazing angle stretches city lights into long columns.

const EYE: f32 = 5.5;

struct Look {
    night: f32,      // 1 night, 0 blue hour
    fog: f32,        // low cloud 0..1
    cloud_base: f32, // metres
    haze: f32,       // aerial extinction per km
    sky_lo: vec3f,
    sky_hi: vec3f,
    amb: vec3f,      // light on the facades from the sky
    hazec: vec3f,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            // blue hour: the sky still deep blue, every tower already lit
            return Look(0.0, 0.0, 2000.0, 0.24,
                vec3f(0.11, 0.13, 0.22), vec3f(0.010, 0.022, 0.075),
                vec3f(0.022, 0.030, 0.055), vec3f(0.075, 0.09, 0.15), -0.35);
        }
        case 2u: {
            // low cloud swallowing the towers, glowing with their light
            return Look(1.0, 1.0, 205.0, 0.4,
                vec3f(0.11, 0.085, 0.075), vec3f(0.05, 0.043, 0.045),
                vec3f(0.006, 0.006, 0.008), vec3f(0.085, 0.068, 0.062), -0.3);
        }
        default: {
            // night: a hazy sky glowing with the city, cloud lit from below
            return Look(1.0, 0.0, 2000.0, 0.22,
                vec3f(0.085, 0.055, 0.060), vec3f(0.004, 0.005, 0.012),
                vec3f(0.004, 0.005, 0.009), vec3f(0.028, 0.022, 0.028), 0.0);
        }
    }
}

// ------------------------------------------------------------ the island

fn layer_d(k: i32) -> f32 {
    switch (k) {
        case 0: { return 1250.0; }   // waterfront: piers, convention centre
        case 1: { return 1400.0; }   // front row
        case 2: { return 1520.0; }   // the tall ones
        case 3: { return 1650.0; }
        case 4: { return 1820.0; }
        case 5: { return 2300.0; }   // Mid-Levels on the slope
        default: { return 3600.0; }  // the Peak ridge
    }
}

// ridge line behind the city, and the slope height under the Mid-Levels
fn ridge(x: f32) -> f32 {
    let peak = 330.0 * exp(-sq((x + 700.0) / 900.0)) + 260.0 * exp(-sq((x - 1100.0) / 1000.0))
             + 200.0 * exp(-sq((x + 2600.0) / 1100.0)) + 300.0;
    return peak + 70.0 * (noise_fbm2(vec2f(x * 0.0016, 3.7), 4) - 0.5) + 25.0 * (noise_value2(vec2f(x * 0.01, 1.0)) - 0.5);
}

// how tall the towers of a district run
fn envelope(x: f32) -> f32 {
    let central = exp(-sq((x + 250.0) / 700.0));
    let wanchai = exp(-sq((x - 500.0) / 500.0)) * 0.85;
    let rest = 0.4 * (1.0 - smoothstep(1800.0, 4500.0, abs(x)));
    return max(max(central, wanchai), rest);
}

struct Twr {
    d: f32,       // signed distance to the silhouette (m), < 0 inside
    u: f32,       // across the tower 0..1
    h: f32,       // roof height (above its base)
    w: f32,       // width
    id: f32,      // hash seed; negative ids are landmarks
    base: f32,    // ground under it
}

// landmark silhouettes in their layer's plane (x, y in metres)
fn landmark(k: i32, x: f32, y: f32) -> Twr {
    var t = Twr(1e5, 0.0, 0.0, 1.0, 0.0, 0.0);
    if (k == 2) {
        // IFC: a tall tapering shaft, setbacks, a crown of fins
        let cx = -420.0;
        let q = vec2f(x - cx, y);
        let hw = 29.0 - 5.0 * saturate(y / 400.0);
        var d = sdf2_box(q - vec2f(0.0, 175.0), vec2f(hw, 175.0));
        d = min(d, sdf2_box(q - vec2f(0.0, 362.0), vec2f(21.0, 12.0)));
        d = min(d, sdf2_box(q - vec2f(0.0, 382.0), vec2f(15.0, 8.0)));
        let fin = min(sdf2_box(vec2f(abs(q.x) - 11.0, q.y - 398.0), vec2f(2.5, 14.0)),
                      sdf2_box(vec2f(abs(q.x) - 4.0, q.y - 394.0), vec2f(1.8, 9.0)));
        d = min(d, fin);
        t = Twr(d, (x - cx) / 58.0 + 0.5, 390.0, 58.0, -1.0, 0.0);
    } else if (k == 3) {
        // Bank of China: prisms cut on the diagonal, twin masts
        let cx = -110.0;
        let q = vec2f(x - cx, y);
        let hw = 26.0;
        var d = sdf2_box(q - vec2f(0.0, 115.0), vec2f(hw, 115.0));
        let t1 = max(sdf2_box(q - vec2f(0.0, 200.0), vec2f(hw, 85.0)), (q.y - 200.0) + q.x * 1.25 - 55.0);
        let t2 = max(sdf2_box(q - vec2f(-7.0, 262.0), vec2f(hw - 7.0, 45.0)), (q.y - 262.0) - (q.x + 7.0) * 1.3 - 30.0);
        let t3 = max(sdf2_box(q - vec2f(-13.0, 300.0), vec2f(13.0, 16.0)), (q.y - 292.0) + (q.x + 13.0) * 1.7 - 12.0);
        d = min(d, min(t1, min(t2, t3)));
        d = min(d, sdf2_box(vec2f(abs(q.x + 13.0) - 5.0, q.y - 330.0), vec2f(0.9, 36.0)));
        t = Twr(d, (x - cx) / 52.0 + 0.5, 310.0, 52.0, -2.0, 0.0);
    } else if (k == 1) {
        // Central Plaza: a triangular prism under a pyramid and a mast
        let cx = 360.0;
        let q = vec2f(x - cx, y);
        var d = sdf2_box(q - vec2f(0.0, 150.0), vec2f(23.0, 150.0));
        d = min(d, max(sdf2_box(q - vec2f(0.0, 318.0), vec2f(19.0, 18.0)), abs(q.x) * 1.4 + (q.y - 300.0) - 38.0));
        d = min(d, sdf2_box(q - vec2f(0.0, 352.0), vec2f(1.1, 22.0)));
        t = Twr(d, (x - cx) / 46.0 + 0.5, 300.0, 46.0, -3.0, 0.0);
    } else if (k == 4) {
        // The Center: a stepped crown banded with light
        let cx = -270.0;
        let q = vec2f(x - cx, y);
        var d = sdf2_box(q - vec2f(0.0, 140.0), vec2f(21.0, 140.0));
        d = min(d, sdf2_box(q - vec2f(0.0, 290.0), vec2f(16.0, 11.0)));
        d = min(d, sdf2_box(q - vec2f(0.0, 309.0), vec2f(10.0, 9.0)));
        d = min(d, sdf2_box(q - vec2f(0.0, 330.0), vec2f(1.0, 14.0)));
        t = Twr(d, (x - cx) / 42.0 + 0.5, 280.0, 42.0, -4.0, 0.0);
    } else if (k == 0) {
        // the convention centre's low wing roof on the waterfront
        let cx = 250.0;
        let q = vec2f(x - cx, y);
        let roof = 20.0 + 9.0 * (1.0 - sq(q.x / 150.0));
        let d = max(abs(q.x) - 150.0, q.y - roof);
        t = Twr(d, (x - cx) / 300.0 + 0.5, roof, 300.0, -5.0, 0.0);
    }
    return t;
}

fn landmark_x(k: i32) -> vec2f {
    // (centre, half width incl. margin) of the landmark in layer k
    switch (k) {
        case 0: { return vec2f(250.0, 160.0); }
        case 1: { return vec2f(360.0, 34.0); }
        case 2: { return vec2f(-420.0, 38.0); }
        case 3: { return vec2f(-110.0, 36.0); }
        case 4: { return vec2f(-270.0, 30.0); }
        default: { return vec2f(0.0, -1.0); }
    }
}

// procedural tower of layer k at x (the one in its cell)
fn tower(k: i32, x: f32, y: f32, d: f32) -> Twr {
    let cw = select(select(40.0, 30.0, k == 5), 26.0, k == 0);
    let c = floor(x / cw);
    let h = hash_cell2(vec2i(i32(c), k), 0x4b1du);
    let g = hash_cell2(vec2i(i32(c), k), 0x2c9u);
    let w = cw * (0.55 + 0.4 * h.x);
    let cx = (c + 0.5) * cw + (h.y - 0.5) * (cw - w) * 0.9;
    var base = 0.0;
    var hgt: f32;
    if (k == 5) {
        // Mid-Levels: slim residential towers stacked up the slope
        let r = ridge(cx);
        base = r * 0.42 * (0.7 + 0.3 * g.x);
        hgt = 60.0 + 100.0 * h.z;
        hgt = min(hgt, r - 40.0 - base);
    } else if (k == 0) {
        hgt = 10.0 + 26.0 * h.z * h.z;
    } else {
        let env = envelope(cx);
        var tall = 1.0;
        if (k == 1) { tall = 0.62; }
        if (k == 2) { tall = 1.18; }
        if (k == 3) { tall = 1.1; }
        if (k == 4) { tall = 0.95; }
        hgt = (50.0 + (80.0 + 190.0 * pow(h.z, 1.4)) * env * tall) * (0.8 + 0.2 * g.x);
        if (g.y < 0.06) { hgt = 0.0; }
    }
    // the flank facing the camera shows as a strip beside the front face
    let side = clamp(-cx / d * (18.0 + 20.0 * g.w), -14.0, 14.0);
    let q = vec2f(x - cx - side * 0.5, y - base);
    var dd = sdf2_box(q - vec2f(0.0, hgt * 0.5), vec2f(w * 0.5 + abs(side) * 0.5, hgt * 0.5));
    // roof forms: setback tier, pitched top, mast, rooftop plant
    let form = g.z;
    if (form < 0.25 && hgt > 70.0) {
        dd = min(dd, sdf2_box(q - vec2f((g.w - 0.5) * w * 0.25, hgt + 8.0), vec2f(w * 0.33, 8.0)));
    } else if (form < 0.4 && hgt > 90.0) {
        dd = min(dd, max(sdf2_box(q - vec2f(0.0, hgt + 12.0), vec2f(w * 0.5, 12.0)), abs(q.x) * 1.1 + (q.y - hgt) - w * 0.5));
    } else if (form < 0.5 && hgt > 140.0) {
        dd = min(dd, sdf2_box(q - vec2f(0.0, hgt + 16.0), vec2f(0.7, 16.0)));
    } else {
        dd = min(dd, sdf2_box(q - vec2f((g.w - 0.5) * w * 0.5, hgt + 2.5), vec2f(w * 0.18, 2.5)));
    }
    if (hgt <= 5.0) { dd = 1e5; }
    // keep clear of the landmark sharing this plane
    let lm = landmark_x(k);
    if (lm.y > 0.0 && abs(cx - lm.x) < lm.y + w * 0.5) { dd = 1e5; }
    // u > 1 or < 0 marks the flank
    return Twr(dd, (x - (cx - w * 0.5)) / w, hgt, w, h.w * 1000.0 + f32(k) * 7.0, base);
}

// ------------------------------------------------------------ facades

// window light of a tower facade. lod = metres per pixel. Four kinds of
// tower: offices with strip windows lit floor by floor (tenants take blocks
// of floors), flats with punched windows, floodlit feature towers, and towers
// gone dark for the night. Below a pixel every pattern falls back to its
// average so distant towers do not sparkle.
fn facade(u: f32, y: f32, tw: Twr, lod: f32, l: Look, t: f32) -> vec3f {
    let hs = hash_cell2(vec2i(i32(abs(tw.id) * 13.0), 91), 0x7ae1u);
    let kind = hs.x;
    let wid = u32(abs(tw.id) * 17.0);
    var e = vec3f(0.0);
    if (kind < 0.42) {
        // office: strip windows, cool white
        let fh = 4.0;
        let fl = floor(y / fh);
        let blk = floor(fl / (1.0 + floor(hs.y * 4.0)));
        let hb = hash_f(u32(i32(blk) + 4000) * 2654435761u ^ wid);
        let slot = floor(t / (90.0 + 60.0 * hb) + hb * 7.0);
        let occ = hash_f(u32(i32(slot) + 9000) * 747796405u ^ u32(i32(blk)) * 7919u ^ wid);
        let lit = step(0.7 - 0.4 * hs.z, occ) * (0.5 + 0.5 * hash_f(u32(i32(fl) + 7000) * 2246822519u ^ wid));
        let strip = step(0.25, fract(y / fh)) * step(fract(y / fh), 0.85);
        let mull = 0.8 + 0.2 * step(0.1, fract(u * tw.w / 1.5));
        let sharp = lit * strip * mull;
        let avg = lit * 0.6;
        let k = saturate(lod / fh - 3.0);
        e = mix(col_kelvin(5200.0), col_kelvin(8000.0), hs.y) * mix(sharp, avg, k) * (0.55 + 0.5 * hs.z);
    } else if (kind < 0.78) {
        // flats and hotels: punched windows, warm, a third lit
        let fh = 3.0;
        let fx = u * tw.w / 2.8;
        let fy = y / fh;
        let cell = vec2i(i32(floor(fx)), i32(floor(fy)));
        let hc = hash_cell2(cell + vec2i(i32(wid), 0), 0x51edu);
        let slot = floor(t / (80.0 + 90.0 * hc.z) + hc.w);
        let on = hash_f(bitcast<u32>(i32(slot)) ^ bitcast<u32>(cell.x * 7919 + cell.y * 104729) ^ wid) < 0.22 + 0.25 * hs.z;
        let fill = step(0.2, fract(fx)) * step(fract(fx), 0.8) * step(0.25, fract(fy)) * step(fract(fy), 0.8);
        let sharp = select(0.0, 1.0, on) * fill;
        let avg = (0.22 + 0.25 * hs.z) * 0.36;
        let k = saturate(lod / fh - 3.0);
        // most flats burn cool fluorescent tubes, some warm lamps
        let wc = select(mix(col_kelvin(5200.0), col_kelvin(7000.0), hc.y), mix(col_kelvin(2900.0), col_kelvin(3600.0), hc.y), hc.x < 0.35);
        e = wc * mix(sharp, avg, k) * 0.8;
    } else if (kind < 0.9) {
        // feature tower: floodlit fins, warm white or pale gold
        let fin = 0.55 + 0.45 * step(0.5, fract(u * tw.w / 3.0));
        let k = saturate(lod / 3.0 - 0.5);
        let grad = 0.6 + 0.4 * smoothstep(0.0, tw.h, y);
        var fc = mix(vec3f(1.0, 0.86, 0.66), vec3f(0.9, 0.95, 1.0), hs.y);
        if (hs.z > 0.6) { fc = select(vec3f(0.25, 0.5, 1.0), vec3f(0.6, 0.35, 1.0), hs.z > 0.8); }
        e = fc * mix(fin, 0.78, k) * grad * 0.32;
    } else {
        // dark for the night: a few stragglers
        let cell = vec2i(i32(floor(u * tw.w / 3.0)), i32(floor(y / 3.8)));
        let on = hash_cell2(cell + vec2i(i32(wid), 7), 0x77u).x < 0.06;
        e = col_kelvin(4500.0) * mix(select(0.0, 0.8, on), 0.05, saturate(lod / 3.8 - 0.5)) * 0.4;
    }
    // lit crowns and roof bands
    let crown = step(tw.h - 6.0, y) * step(y, tw.h + 1.0) * step(0.86, hs.w);
    e += crown * mix(vec3f(0.85, 0.92, 1.0), vec3f(0.45, 0.65, 1.0), step(0.94, hs.w)) * 0.9;
    // LED strips down the edges of a few
    let edge = min(u, 1.0 - u) * tw.w;
    if (hs.z > 0.8 && kind < 0.9) {
        let led = saturate(1.0 - edge / max(lod * 0.7, 1.0));
        e += led * mix(vec3f(0.35, 0.6, 1.0), vec3f(0.95, 0.95, 1.0), step(0.92, hs.z)) * 0.8;
    }
    return e;
}

fn landmark_light(u: f32, y: f32, tw: Twr, lod: f32, l: Look, t: f32) -> vec3f {
    let id = -tw.id;
    var e = vec3f(0.0);
    let aa = max(lod, 1.0);
    if (id < 1.5) {
        // IFC: pale striped glass, bright crown
        let fy = y / 4.2;
        e = col_kelvin(6000.0) * (0.16 + 0.1 * step(0.5, fract(u * 9.0))) * (0.8 + 0.2 * step(0.3, fract(fy)));
        e += vec3f(0.9, 0.95, 1.0) * 1.3 * step(350.0, y);
    } else if (id < 2.5) {
        // Bank of China: white diagonal cross bracing over dark blue glass
        e = vec3f(0.02, 0.035, 0.07);
        let sy = fract(y / 52.0);
        let x = u - 0.5;
        let dd = min(abs(sy - 0.5 - x), abs(sy - 0.5 + x));
        let line = saturate(1.0 - dd * 52.0 / max(aa * 0.8, 1.5));
        let edge = saturate(1.0 - min(u, 1.0 - u) * 52.0 / max(aa * 0.8, 1.5));
        e += vec3f(0.85, 0.92, 1.0) * max(line, edge) * 1.2;
    } else if (id < 3.5) {
        // Central Plaza: gold bands, and the crown that tells the time in
        // colour, changing every quarter hour
        e = vec3f(1.0, 0.72, 0.38) * 0.22 * (0.6 + 0.4 * step(0.5, fract(y / 8.0)));
        let q = floor(fmod_pos(t / 900.0, 4.0));
        let cc = select(select(select(vec3f(1.0, 0.3, 0.25), vec3f(1.0, 0.95, 0.9), q > 0.5), vec3f(1.0, 0.8, 0.3), q > 1.5), vec3f(0.65, 0.4, 1.0), q > 2.5);
        e = mix(e, cc * 1.2, step(300.0, y));
    } else if (id < 4.5) {
        // The Center: bands of light round the stepped crown, slowly
        // cycling through a few colours
        e = col_kelvin(5200.0) * 0.12 * step(0.4, fract(y / 4.0));
        let band = step(0.5, fract(y / 9.0)) * step(250.0, y);
        let hue = fract(t / 150.0 + floor(y / 9.0) * 0.07);
        e += col_hsv(hue, 0.5, 1.0) * band * 1.0;
    } else {
        // convention centre: glass halls glowing warm under the wing
        e = vec3f(1.0, 0.82, 0.6) * 0.18 * step(y, tw.h - 4.0) * step(3.0, y);
        e += vec3f(0.9, 0.95, 1.0) * 0.5 * step(tw.h - 1.5, y);
    }
    return e;
}

// ------------------------------------------------------------ compositing

fn hk_sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let y = max(rd.y, 0.0);
    var c = mix(l.sky_lo, l.sky_hi, pow(saturate(y * 4.0), 0.55));
    // broken cloud lit from below by the city, drifting east
    if (rd.y > 0.0) {
        let uv = rd.xz / (rd.y + 0.06) * 0.7;
        let cs = cloud_sheet(uv + vec2f(ctx.t * 0.002, 0.0), 0.5 + 0.25 * l.fog, ctx);
        let under = mix(l.sky_lo * 1.8, l.sky_lo * 0.7 + l.sky_hi * 1.5, saturate(y * 3.0));
        c = mix(c, under * (0.75 + 0.5 * cs.y), cs.x * 0.8 * smoothstep(0.0, 0.05, rd.y));
    }
    if (l.fog > 0.0) {
        // the underside of the cloud deck, brightest just over the towers
        let n = noise_fbm2(vec2f(rd.x * 6.0 + ctx.t * 0.003, rd.y * 14.0), 4);
        c = mix(c, l.hazec * (0.9 + 0.9 * n) * (0.6 + 0.9 * exp(-y * 9.0)), 0.85);
    }
    return c;
}

// the island along one ray: premultiplied rgb and coverage
fn island(ro: vec3f, rd: vec3f, blur: f32, l: Look, ctx: Ctx, zoom: f32) -> vec4f {
    var col = vec3f(0.0);
    var acc = 0.0;
    if (rd.z <= 1e-4) { return vec4f(0.0); }
    for (var k = 0; k < 7; k++) {
        let d = layer_d(k);
        let t = (d - ro.z) / rd.z;
        let p = ro + rd * t;
        if (p.y < -2.0) { continue; }
        let lod = ctx.px / zoom * t * (1.0 + blur);
        var c = vec3f(0.0);
        var a = 0.0;
        if (k == 0) {
            // the waterfront road and promenade: a string of lamps at the
            // water's edge
            let cell = floor(p.x / 17.0);
            let hl = hash_f(u32(i32(cell) + 90000));
            let lx = (fract(p.x / 17.0) - 0.3 - 0.4 * hl) * 17.0;
            let lamp = exp(-(sq(lx) + sq(p.y - 6.0 - 3.0 * fract(hl * 7.0))) / max(sq(lod * 0.5), 1.0));
            let lc = select(vec3f(1.0, 0.62, 0.3), vec3f(0.9, 0.95, 1.0), hl > 0.5);
            col += (1.0 - acc) * lc * lamp * 1.1 * step(0.3, fract(hl * 13.0));
        }
        if (k == 6) {
            // the ridge: dark wooded slope, lights along the roads low down
            let hr = ridge(p.x);
            let dd = p.y - hr;
            a = saturate(0.5 - dd / lod);
            let n = noise_value2(vec2f(p.x * 0.015, p.y * 0.02));
            c = l.amb * 0.5 * (0.6 + 0.4 * n);
            let cell = vec2i(i32(floor(p.x / 9.0)), i32(floor(p.y / 8.0)));
            let lights = step(0.9, hash_cell2(cell, 0x9eu).x) * smoothstep(hr - 40.0, hr - 140.0, p.y);
            c += col_kelvin(3400.0) * lights * 0.25 * l.night;
        } else {
            var tw = tower(k, p.x, p.y, d);
            let lm = landmark(k, p.x, p.y);
            if (lm.d < tw.d) { tw = lm; }
            a = saturate(0.5 - tw.d / lod);
            if (a > 0.0) {
                let yy = p.y - tw.base;
                if (tw.id < 0.0) {
                    c = landmark_light(tw.u, yy, tw, lod, l, ctx.t);
                } else if (tw.u < 0.0 || tw.u > 1.0) {
                    // the flank: same building, seen obliquely and a shade darker
                    let fu = select(-tw.u, tw.u - 1.0, tw.u > 1.0);
                    c = facade(fract(fu * 2.3), yy, tw, lod * 2.0, l, ctx.t) * 0.55;
                } else {
                    c = facade(tw.u, yy, tw, lod, l, ctx.t);
                }
                // facade body: dark glass holding a little sky
                c += l.amb * (0.6 + 0.5 * fract(tw.id * 0.137)) * (0.7 + 0.3 * step(0.5, fract(tw.u * 3.0)));
            }
            // aircraft warning lights on the tallest roofs, blinking slowly
            if (tw.id < -0.5 && tw.id > -4.5 && tw.d < 60.0) {
                let mast = select(select(14.0, 22.0, tw.id < -2.5), 36.0, tw.id < -1.5 && tw.id > -2.5);
                let top = vec2f((tw.u - 0.5) * tw.w, p.y - tw.base - tw.h - mast - 3.0);
                let r = length(top) / max(lod * 0.5, 1.0);
                let blink = step(0.55, fract(ctx.t * 0.5 + fract(abs(tw.id) * 0.31)));
                let bl = exp(-r * r) * blink;
                c = c * (1.0 - bl) + vec3f(1.0, 0.06, 0.02) * 2.0 * bl;
                a = max(a, bl);
            }
        }
        // aerial perspective across the harbour
        let fk = 1.0 - exp(-t * 0.001 * l.haze);
        c = mix(c, l.hazec * a, fk);
        // low cloud: towers dissolve upward into the deck, their light
        // smeared into it
        var ae = a;
        if (l.fog > 0.0) {
            let n = noise_fbm2(vec2f(p.x * 0.004 + ctx.t * 0.006, p.y * 0.012), 4);
            let base = l.cloud_base + 60.0 * (n - 0.5);
            let inside = smoothstep(base - 30.0, base + 50.0, p.y);
            col += (1.0 - acc) * a * inside * c * 0.12;
            ae = a * (1.0 - inside);
        }
        col += (1.0 - acc) * ae * c;
        acc += (1.0 - acc) * ae;
        if (acc > 0.995) { break; }
    }
    return vec4f(col, acc);
}

// the Star Ferry on its crossing from Tsim Sha Tsui to Central and back:
// green hull, white upper deck, lit saloon windows
fn ferry(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx, zoom: f32) -> vec4f {
    let ev = hash_event(ctx.t + 70.0, 300.0, 0x5ea7u);
    let outward = fract(ev.z * 0.5) < 0.25;
    let s = select(1.0 - ev.y, ev.y, outward);
    let e = s * s * (3.0 - 2.0 * s);
    let fz = mix(330.0, 1080.0, e);
    let fx = mix(240.0, -380.0, e);
    let t = (fz - ro.z) / max(rd.z, 1e-4);
    let p = ro + rd * t;
    let q = vec2f(p.x - fx, p.y);
    let lod = ctx.px / zoom * t;
    let hull = sdf2_round_box(q - vec2f(0.0, 2.0), vec2f(20.0, 2.0), 1.4);
    let deck = sdf2_box(q - vec2f(0.0, 5.3), vec2f(16.5, 1.3));
    let upper = sdf2_box(q - vec2f(0.0, 7.6), vec2f(14.0, 1.0));
    let roof = sdf2_box(q - vec2f(0.0, 8.8), vec2f(11.5, 0.25));
    let mast = sdf2_box(q - vec2f(6.0, 10.6), vec2f(0.3, 1.8));
    let d = min(min(hull, deck), min(min(upper, roof), mast));
    let a = saturate(0.5 - d / lod);
    var c = vec3f(0.01, 0.035, 0.02) + l.amb * 0.8;
    if (deck < 0.0 || upper < 0.0) {
        let win = step(0.3, fract(q.x / 2.2)) * step(abs(fract(q.y / 2.3) - 0.5), 0.3);
        c = vec3f(0.3) * l.amb * 4.0 + vec3f(1.0, 0.8, 0.55) * 0.7 * mix(win, 0.55, saturate(lod / 2.0));
    }
    // masthead light and running lights, glowing past the silhouette
    let mh = exp(-sq(length(q - vec2f(6.0, 12.6)) / max(lod * 0.8, 0.6)));
    c = c * a + vec3f(1.0, 0.95, 0.85) * 3.0 * mh;
    return vec4f(c, max(a, mh));
}

// the Aqua Luna: a junk under three red battened sails, crossing slowly
// the other way, sails floodlit from the deck after dark
fn junk(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx, zoom: f32) -> vec4f {
    let ev = hash_event(ctx.t + 150.0, 420.0, 0x1u);
    let dir = select(-1.0, 1.0, fract(ev.z * 0.5) < 0.25);
    let fx = dir * (-560.0 + 1120.0 * ev.y);
    let zj = 470.0;
    let t = (zj - ro.z) / max(rd.z, 1e-4);
    let p = ro + rd * t;
    let q = vec2f((p.x - fx) * dir, p.y);
    let lod = ctx.px / zoom * t;
    // hull with a raised stern
    let hull = max(sdf2_box(q - vec2f(0.0, 1.6), vec2f(14.0, 1.6)), q.y - 3.0 - 2.5 * smoothstep(6.0, 14.0, -q.x));
    // three sails, each a fan widening upward between its battens
    var sail = 1e5;
    var id = 0.0;
    for (var k = 0; k < 3; k++) {
        var cx = 0.0;
        var h0 = 4.5;
        var h1 = 22.0;
        var w = 8.5;
        if (k == 1) { cx = 10.0; h1 = 16.0; w = 6.0; }
        if (k == 2) { cx = -11.5; h1 = 13.0; w = 5.0; }
        let v = saturate((q.y - h0) / (h1 - h0));
        let hw = w * (0.55 + 0.45 * v) * 0.5;
        let lean = v * w * 0.25;
        let sd = max(abs(q.x - cx - lean) - hw, max(h0 - q.y, q.y - h1 + 1.5 * sq((q.x - cx - lean) / max(hw, 0.1))));
        if (sd < sail) { sail = sd; id = f32(k); }
    }
    let d = min(hull, sail);
    let a = saturate(0.5 - d / max(lod, 0.3));
    if (a <= 0.0) { return vec4f(0.0); }
    var c = vec3f(0.02, 0.012, 0.008) + l.amb * 0.5;
    if (sail < hull) {
        let batten = step(0.85, fract(q.y / 2.2));
        let red = vec3f(0.55, 0.06, 0.03);
        c = red * (0.25 + 0.6 * l.night) * (1.0 - 0.5 * batten) + red * l.amb * 3.0;
    } else {
        // lights along the deck
        c += vec3f(1.0, 0.8, 0.5) * 0.8 * step(2.2, q.y) * step(0.5, fract(q.x / 2.0)) * l.night;
    }
    return vec4f(c * a, a);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(0.0, EYE, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.0, 0.094, 1.0), 0.0, 34.0);
    let rd = cam_ray(cam, p);
    let zoom = cam.zoom;
    var col: vec3f;
    let tw = water_intersect(ro, rd, 0.0);
    let shore = 1250.0 / max(rd.z, 1e-4);
    if (tw > 0.0 && tw < shore) {
        // the harbour
        let wp = ro + rd * tw;
        let n = water_normal(wp.xz * 3.5, ctx.t * 1.6, 0.3, tw * 3.5, ctx);
        var r = reflect(rd, n);
        r.y = abs(r.y) + 0.0005;
        // chop at a grazing angle spreads the mirror ray up and down
        let sn = max(-rd.y, 0.003);
        let spread = 0.012 + 0.35 * sn;
        var refl = vec3f(0.0);
        for (var k = 0; k < 4; k++) {
            let o = (f32(k) - 1.5 + (ctx.jitter - 0.5)) * spread * 0.5;
            let rk = normalize(r + vec3f(0.0, o, 0.0));
            let isl = island(wp, rk, 1.0, l, ctx, zoom);
            let fr = ferry(wp, rk, l, ctx, zoom);
            let jk = junk(wp, rk, l, ctx, zoom);
            var c = hk_sky(rk, l, ctx) * (1.0 - isl.w) + isl.xyz;
            c = c * (1.0 - jk.w) + jk.xyz;
            c = c * (1.0 - fr.w) + fr.xyz;
            refl += c;
        }
        refl *= 0.25;
        let fres = water_fresnel(dot(-rd, n));
        let deep = vec3f(0.002, 0.005, 0.007) + l.amb * 0.2;
        col = mix(deep, refl, fres);
        let jk = junk(ro, rd, l, ctx, zoom);
        col = col * (1.0 - jk.w) + jk.xyz;
        let fr = ferry(ro, rd, l, ctx, zoom);
        col = col * (1.0 - fr.w) + fr.xyz;
        col = mix(col, l.hazec, 1.0 - exp(-tw * 0.001 * l.haze));
    } else {
        let isl = island(ro, rd, 0.0, l, ctx, zoom);
        col = hk_sky(rd, l, ctx) * (1.0 - isl.w) + isl.xyz;
        let jk = junk(ro, rd, l, ctx, zoom);
        col = col * (1.0 - jk.w) + jk.xyz;
        let fr = ferry(ro, rd, l, ctx, zoom);
        col = col * (1.0 - fr.w) + fr.xyz;
    }
    return col * exp2(l.exposure);
}
