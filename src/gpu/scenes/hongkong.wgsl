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
// 5.5 m above the water, looking south across the harbour (+z; east, +x, is
// on the left). The island is a stack of vertical planes, front to back: the
// waterfront with its piers, promenade lamps and observation wheel; six rows
// of towers; two rows of Mid-Levels flats climbing the slope; the Peak
// ridge. Each plane holds procedural towers of several kinds (dark glass
// edged with LED strips, offices lit floor by floor, flats, floodlit towers,
// LED facades, towers gone dark) plus hand-built landmarks; a ray is
// intersected with the planes analytically and composited front to back
// with coverage, so silhouettes stay anti-aliased at any size, and every
// plane sits deeper in the harbour haze than the one before. The water
// mirrors the same island along chop-tilted rays: far off the facets
// average into long smooth columns of colour, close in they break into
// bands.

const EYE: f32 = 5.5;
const NL: i32 = 10;

struct Look {
    night: f32,      // 1 night, 0 blue hour
    fog: f32,        // low cloud 0..1
    cloud_base: f32, // metres
    haze: f32,       // aerial extinction per km
    sky_lo: vec3f,   // sky at the horizon
    sky_hi: vec3f,   // sky overhead
    glow: vec3f,     // city light on the cloud base
    amb: vec3f,      // skylight on the facades
    hazec: vec3f,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            // blue hour: the sky still deep blue, every tower already lit,
            // glass still holding the sky
            return Look(0.0, 0.0, 2000.0, 0.3,
                vec3f(0.13, 0.16, 0.30), vec3f(0.012, 0.026, 0.09), vec3f(0.07, 0.06, 0.08),
                vec3f(0.028, 0.040, 0.080), vec3f(0.09, 0.11, 0.2), -0.4);
        }
        case 2u: {
            // low cloud swallowing the towers, glowing with their light
            return Look(1.0, 1.0, 175.0, 0.7,
                vec3f(0.1, 0.088, 0.084), vec3f(0.05, 0.047, 0.05), vec3f(0.11, 0.09, 0.078),
                vec3f(0.008, 0.008, 0.009), vec3f(0.07, 0.064, 0.064), -0.3);
        }
        default: {
            // night: a hazy sky glowing with the city, cloud lit from below
            return Look(1.0, 0.0, 2000.0, 0.2,
                vec3f(0.05, 0.034, 0.042), vec3f(0.003, 0.004, 0.01), vec3f(0.085, 0.052, 0.05),
                vec3f(0.004, 0.005, 0.009), vec3f(0.014, 0.011, 0.015), 0.0);
        }
    }
}

// ------------------------------------------------------------ the island

fn layer_d(k: i32) -> f32 {
    switch (k) {
        case 0: { return 1250.0; }   // waterfront: piers, promenade, wheel
        case 1: { return 1320.0; }   // front row
        case 2: { return 1400.0; }   // IFC, Central Plaza
        case 3: { return 1500.0; }
        case 4: { return 1620.0; }   // Bank of China
        case 5: { return 1760.0; }   // The Center
        case 6: { return 1930.0; }
        case 7: { return 2250.0; }   // Mid-Levels, lower slope
        case 8: { return 2650.0; }   // Mid-Levels, upper slope
        default: { return 3600.0; }  // the Peak ridge
    }
}
fn layer_cw(k: i32) -> f32 {
    switch (k) {
        case 1: { return 34.0; }
        case 2: { return 46.0; }
        case 3: { return 50.0; }
        case 4: { return 42.0; }
        case 5: { return 38.0; }
        case 6: { return 32.0; }
        case 7: { return 23.0; }
        default: { return 21.0; }
    }
}

// ridge line behind the city
fn ridge(x: f32) -> f32 {
    let peak = 330.0 * exp(-sq((x + 700.0) / 900.0)) + 260.0 * exp(-sq((x - 1100.0) / 1000.0))
             + 200.0 * exp(-sq((x + 2600.0) / 1100.0)) + 300.0;
    return peak + 70.0 * (noise_fbm2(vec2f(x * 0.0016, 3.7), 4) - 0.5) + 25.0 * (noise_value2(vec2f(x * 0.01, 1.0)) - 0.5);
}

// how tall the towers of a district run: Central and Admiralty, then Wan Chai
fn envelope(x: f32) -> f32 {
    let central = exp(-sq((x + 260.0) / 560.0));
    let wanchai = exp(-sq((x - 560.0) / 420.0)) * 0.85;
    let rest = 0.36 * (1.0 - sstep(1800.0, 4500.0, abs(x)));
    // neighbourhoods run taller and lower
    let n = noise_value2(vec2f(x * 0.005, 7.0));
    return max(max(central, wanchai), rest) * (0.5 + 0.75 * n);
}

struct Twr {
    d: f32,       // signed distance to the silhouette (m), < 0 inside
    u: f32,       // across the front face 0..1; outside that, the flank
    h: f32,       // roof height (above its base)
    w: f32,       // width
    id: f32,      // hash seed; negative ids are landmarks
    base: f32,    // ground under it
    kind: u32,    // facade kind
}

// landmark silhouettes in their layer's plane (x, y in metres)
fn landmark(k: i32, x: f32, y: f32) -> Twr {
    var t = Twr(1e5, 0.0, 0.0, 1.0, 0.0, 0.0, 0u);
    if (k == 2 && x < 0.0) {
        // IFC: a tall shaft that narrows in steps to a crown of fins
        let cx = -420.0;
        let q = vec2f(x - cx, y);
        let hw = 29.0 - 4.0 * saturate(y / 300.0);
        var d = sdf2_box(q - vec2f(0.0, 170.0), vec2f(hw, 170.0));
        d = min(d, sdf2_box(q - vec2f(0.0, 352.0), vec2f(23.0, 12.0)));
        d = min(d, sdf2_box(q - vec2f(0.0, 372.0), vec2f(18.0, 8.0)));
        d = min(d, sdf2_box(q - vec2f(0.0, 386.0), vec2f(13.0, 6.0)));
        let fin = min(sdf2_box(vec2f(abs(q.x) - 10.0, q.y - 400.0), vec2f(2.4, 14.0)),
                      sdf2_box(vec2f(abs(q.x) - 3.5, q.y - 397.0), vec2f(1.8, 10.0)));
        d = min(d, fin);
        t = Twr(d, (x - cx) / 58.0 + 0.5, 390.0, 58.0, -1.0, 0.0, 0u);
    } else if (k == 4) {
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
        t = Twr(d, (x - cx) / 52.0 + 0.5, 310.0, 52.0, -2.0, 0.0, 0u);
    } else if (k == 2) {
        // Central Plaza: a triangular prism under a pyramid and a mast
        let cx = 360.0;
        let q = vec2f(x - cx, y);
        var d = sdf2_box(q - vec2f(0.0, 150.0), vec2f(23.0, 150.0));
        d = min(d, max(sdf2_box(q - vec2f(0.0, 318.0), vec2f(19.0, 18.0)), abs(q.x) * 1.4 + (q.y - 300.0) - 38.0));
        d = min(d, sdf2_box(q - vec2f(0.0, 352.0), vec2f(1.1, 22.0)));
        t = Twr(d, (x - cx) / 46.0 + 0.5, 300.0, 46.0, -3.0, 0.0, 0u);
    } else if (k == 5) {
        // The Center: a stepped crown banded with light
        let cx = -270.0;
        let q = vec2f(x - cx, y);
        var d = sdf2_box(q - vec2f(0.0, 140.0), vec2f(21.0, 140.0));
        d = min(d, sdf2_box(q - vec2f(0.0, 290.0), vec2f(16.0, 11.0)));
        d = min(d, sdf2_box(q - vec2f(0.0, 309.0), vec2f(10.0, 9.0)));
        d = min(d, sdf2_box(q - vec2f(0.0, 330.0), vec2f(1.0, 14.0)));
        t = Twr(d, (x - cx) / 42.0 + 0.5, 280.0, 42.0, -4.0, 0.0, 0u);
    } else if (k == 0) {
        // the convention centre's wing roof on the waterfront
        let cx = 250.0;
        let q = vec2f(x - cx, y);
        let roof = 20.0 + 9.0 * (1.0 - sq(q.x / 150.0));
        let d = max(abs(q.x) - 150.0, q.y - roof);
        t = Twr(d, (x - cx) / 300.0 + 0.5, roof, 300.0, -5.0, 0.0, 0u);
    }
    return t;
}

fn landmark_x(k: i32) -> vec4f {
    // (centre, half width incl. margin) of up to two landmarks in layer k
    switch (k) {
        case 0: { return vec4f(250.0, 160.0, -300.0, 38.0); }
        case 2: { return vec4f(360.0, 34.0, -420.0, 38.0); }
        case 4: { return vec4f(-110.0, 36.0, 0.0, -1.0); }
        case 5: { return vec4f(-270.0, 30.0, 0.0, -1.0); }
        default: { return vec4f(0.0, -1.0, 0.0, -1.0); }
    }
}

// procedural tower of layer k at x (the one in its cell)
fn tower(k: i32, x: f32, y: f32, d: f32) -> Twr {
    let none = Twr(1e5, 0.0, 0.0, 1.0, 0.0, 0.0, 0u);
    let cw = select(layer_cw(k), 26.0, k == 0);
    let c = floor(x / cw);
    let h = hash_cell2(vec2i(i32(c), k), 0x4b1du);
    let g = hash_cell2(vec2i(i32(c), k), 0x2c9u);
    // empty lots let the rows behind show through
    if (h.w < select(0.2, 0.08, k >= 7)) { return none; }
    var w = cw * (0.42 + 0.5 * h.x);
    let pencil = g.w < 0.18 && k >= 1 && k <= 6;
    if (pencil) { w = min(w, 13.0 + 5.0 * h.x); }
    let cx = (c + 0.5) * cw + (h.y - 0.5) * (cw - w) * 0.9;
    var base = 0.0;
    var hgt: f32;
    var kind = 0u;
    if (k >= 7) {
        // Mid-Levels: slim residential towers stacked up the slope
        let r = ridge(cx);
        base = r * select(0.3, 0.5, k == 8) * (0.8 + 0.3 * g.x);
        hgt = 50.0 + 95.0 * h.z;
        hgt = min(hgt, r - 35.0 - base);
        kind = select(2u, 1u, g.z < 0.15);
    } else if (k == 0) {
        hgt = 9.0 + 24.0 * h.z * h.z;
        kind = select(1u, 2u, g.z < 0.4);
    } else {
        let env = envelope(cx);
        var tall = 1.0;
        switch (k) {
            case 1: { tall = 0.5; }
            case 2: { tall = 0.9; }
            case 3: { tall = 1.12; }
            case 4: { tall = 1.05; }
            case 5: { tall = 0.95; }
            default: { tall = 0.8; }
        }
        hgt = (40.0 + (70.0 + 210.0 * pow(h.z, 1.6)) * env * tall) * (0.85 + 0.3 * g.x);
        if (pencil) { hgt *= 1.25; }
        // facade kinds: the waterfront rows show off, the back rows are workaday
        let kr = g.z;
        if (k <= 4) {
            if (kr < 0.24) { kind = 0u; }        // dark glass, LED edges
            else if (kr < 0.52) { kind = 1u; }   // offices lit floor by floor
            else if (kr < 0.64) { kind = 3u; }   // floodlit
            else if (kr < 0.73) { kind = 4u; }   // LED facade
            else if (kr < 0.86) { kind = 2u; }   // flats and hotels
            else { kind = 5u; }                  // dark
        } else {
            if (kr < 0.15) { kind = 0u; }
            else if (kr < 0.5) { kind = 1u; }
            else if (kr < 0.82) { kind = 2u; }
            else { kind = 5u; }
        }
    }
    if (hgt <= 6.0) { return none; }
    // the flank facing the camera shows as a strip beside the front face
    let side = clamp(-cx / d * (18.0 + 20.0 * g.w), -14.0, 14.0) * select(1.0, 0.4, k >= 7);
    let q = vec2f(x - cx - side * 0.5, y - base);
    var dd = sdf2_box(q - vec2f(0.0, hgt * 0.5), vec2f(w * 0.5 + abs(side) * 0.5, hgt * 0.5));
    // roof forms: setback tiers, pitched top, wedge, mast, rooftop plant
    let form = fract(g.y * 7.3);
    if (form < 0.2 && hgt > 70.0) {
        dd = min(dd, sdf2_box(q - vec2f((g.w - 0.5) * w * 0.2, hgt + 9.0), vec2f(w * 0.36, 9.0)));
        dd = min(dd, sdf2_box(q - vec2f((g.w - 0.5) * w * 0.2, hgt + 22.0), vec2f(w * 0.2, 5.0)));
    } else if (form < 0.34 && hgt > 90.0) {
        dd = min(dd, max(sdf2_box(q - vec2f(0.0, hgt + 14.0), vec2f(w * 0.5, 14.0)), abs(q.x) * 1.0 + (q.y - hgt) - w * 0.5));
    } else if (form < 0.44 && hgt > 80.0) {
        dd = max(dd, (q.y - hgt) + q.x * select(0.5, -0.5, g.x < 0.5));
        dd = min(dd, sdf2_box(q - vec2f(0.0, hgt * 0.5 - w * 0.25), vec2f(w * 0.5 + abs(side) * 0.5, hgt * 0.5 - w * 0.25)));
    } else if (form < 0.56 && hgt > 130.0) {
        dd = min(dd, sdf2_box(q - vec2f(0.0, hgt + 18.0), vec2f(0.7, 18.0)));
    } else {
        dd = min(dd, sdf2_box(q - vec2f((g.w - 0.5) * w * 0.5, hgt + 2.5), vec2f(w * 0.2, 2.5)));
    }
    // keep clear of the landmarks sharing this plane
    let lm = landmark_x(k);
    if (lm.y > 0.0 && abs(cx - lm.x) < lm.y + w * 0.5) { return none; }
    if (lm.w > 0.0 && abs(cx - lm.z) < lm.w + w * 0.5) { return none; }
    // u > 1 or < 0 marks the flank
    return Twr(dd, (x - (cx - w * 0.5)) / w, hgt, w, h.w * 1000.0 + f32(k) * 7.0 + 1.0, base, kind);
}

// ------------------------------------------------------------ facades

fn led_col(h: f32) -> vec3f {
    switch (u32(h * 7.0)) {
        case 0u: { return vec3f(0.95, 0.97, 1.0); }
        case 1u: { return vec3f(0.3, 0.75, 1.0); }
        case 2u: { return vec3f(0.25, 0.4, 1.0); }
        case 3u: { return vec3f(1.0, 0.3, 0.75); }
        case 4u: { return vec3f(0.3, 1.0, 0.55); }
        case 5u: { return vec3f(1.0, 0.72, 0.3); }
        default: { return vec3f(0.7, 0.4, 1.0); }
    }
}

// box-filtered periodic band: coverage of [lo, hi] (cell units) at x, over w cells
fn pband(x: f32, lo: f32, hi: f32, w: f32) -> f32 {
    let duty = hi - lo;
    if (w >= 1.0) { return duty; }
    let f = fract(x);
    let a = f - w * 0.5;
    let b = f + w * 0.5;
    var c = max(0.0, min(b, hi) - max(a, lo));
    c += max(0.0, min(b, hi - 1.0) - max(a, lo - 1.0));
    c += max(0.0, min(b, hi + 1.0) - max(a, lo + 1.0));
    return mix(c / max(w, 1e-4), duty, saturate(w * 2.0 - 1.0));
}

// light of a tower facade. lod = metres per pixel. Below a pixel every
// pattern falls back to its average so distant towers do not sparkle.
fn facade(u: f32, y: f32, tw: Twr, lod: f32, l: Look, t: f32) -> vec3f {
    let hs = hash_cell2(vec2i(i32(abs(tw.id) * 13.0), 91), 0x7ae1u);
    let wid = u32(abs(tw.id) * 17.0);
    let xm = u * tw.w;
    let lw = max(lod, 0.3);
    var e = vec3f(0.0);
    // glass holds the sky: dark at night, blue at blue hour
    var body = l.amb * (0.7 + 0.6 * hs.y);
    switch (tw.kind) {
        case 0u: {
            // dark glass curtain wall: faint floor lines, a few lit floors,
            // LED strips down the edges (and on some, down the mullions)
            body = l.amb * (1.3 + 0.6 * hs.y) + vec3f(0.002, 0.003, 0.005);
            let fh = 4.0;
            let fl = floor(y / fh);
            let lit = step(hash_f(u32(i32(fl) + 7000) * 2246822519u ^ wid), 0.14);
            e = col_kelvin(6000.0) * (0.02 + 0.12 * lit) * pband(y / fh, 0.3, 0.8, lw / fh);
            let edge = select(min(xm, tw.w - xm), tw.w - xm, hs.y < 0.35);
            var led = saturate(1.0 - (edge - 0.8) / lw);
            if (hs.z > 0.55) {
                let pitch = tw.w / (2.0 + floor(hs.w * 3.0));
                led = max(led, pband(xm / pitch, 0.0, 0.8 / pitch, lw / pitch) * 0.7);
            }
            e += led_col(hs.x) * led * 0.6 * step(6.0, y);
        }
        case 1u: {
            // offices lit floor by floor; tenants take blocks of floors
            // offices lit in blocks of floors, each wing of a wide tower its own
            let fh = 4.0;
            let fl = floor(y / fh);
            let zone = select(0u, u32(u * 2.0), tw.w > 30.0 && hs.x > 0.4);
            let bh = 3.0 + floor(hs.y * 6.0);
            let blk = floor(fl / bh);
            let zid = wid ^ (zone * 0x51u);
            let hb = hash_f(u32(i32(blk) + 4000) * 2654435761u ^ zid);
            let slot = floor(t / (90.0 + 60.0 * hb) + hb * 7.0);
            let occ = hash_f(u32(i32(slot) + 9000) * 747796405u ^ u32(i32(blk)) * 7919u ^ zid);
            let pl = 0.45 + 0.35 * hs.z;
            let bright = 0.5 + 0.8 * hash_f(u32(i32(blk) + 300) * 0x68e31da4u ^ zid);
            let lit = step(1.0 - pl, occ) * bright * (0.75 + 0.25 * hash_f(u32(i32(fl) + 7000) * 2246822519u ^ wid));
            let strip = pband(y / fh, 0.25, 0.86, lw / fh);
            let mull = 1.0 - 0.3 * pband(xm / 1.5, 0.0, 0.15, lw / 1.5);
            let tint = mix(col_kelvin(4800.0), col_kelvin(7500.0), fract(hs.y + hb * 0.3)) * vec3f(0.96, 1.0, 0.97);
            let k = saturate(lw / fh - 1.0);
            e = tint * lit * mix(strip * mull, 0.52, k) * (0.2 + 0.16 * hs.w);
        }
        case 2u: {
            // flats and hotels: punched windows, fluorescent white and warm
            let fh = 2.9;
            let fx = xm / 2.6;
            let fy = y / fh;
            let cell = vec2i(i32(floor(fx)), i32(floor(fy)));
            let hc = hash_cell2(cell + vec2i(i32(wid), 0), 0x51edu);
            let slot = floor(t / (80.0 + 90.0 * hc.z) + hc.w);
            let pl = 0.3 + 0.25 * hs.z;
            let on = hash_f(bitcast<u32>(i32(slot)) ^ bitcast<u32>(cell.x * 7919 + cell.y * 104729) ^ wid) < pl;
            let fill = pband(fx, 0.2, 0.8, lw / 2.6) * pband(fy, 0.25, 0.8, lw / fh);
            let wc = select(mix(col_kelvin(5600.0), col_kelvin(7500.0), hc.y), mix(col_kelvin(3000.0), col_kelvin(3800.0), hc.y), hc.x < 0.35);
            let k = saturate(lw / fh - 1.0);
            let bc = vec2i(i32(floor(fx / 3.0)), i32(floor(fy / 3.0)));
            let hbk = hash_cell2(bc + vec2i(i32(wid), 3), 0x3b1du);
            let avg = mix(col_kelvin(3600.0), col_kelvin(7000.0), hbk.y) * pl * 0.33 * (0.3 + 1.4 * hbk.x);
            e = mix(wc * select(0.0, 1.0, on) * fill * (0.7 + 0.5 * hc.z), avg, k) * 0.36;
        }
        case 3u: {
            // floodlit from the base: warm white, pale gold or cool white fins
            let fin = 0.6 + 0.4 * pband(xm / 3.0, 0.0, 0.5, lw / 3.0);
            let grad = 0.35 + 0.65 * exp(-y / max(tw.h * 0.45, 20.0)) + 0.3 * sstep(tw.h - 25.0, tw.h, y);
            var fc = mix(vec3f(1.0, 0.84, 0.62), vec3f(0.92, 0.95, 1.0), hs.y);
            if (hs.z > 0.75) { fc = vec3f(1.0, 0.7, 0.35); }
            e = fc * fin * grad * 0.3;
        }
        case 4u: {
            // LED facade: a field of colour that drifts very slowly
            let hue = fract(hs.x + t / 240.0 + y / max(tw.h, 1.0) * 0.12);
            let dots = mix(pband(xm / 1.6, 0.0, 0.6, lw / 1.6) * pband(y / 1.6, 0.0, 0.6, lw / 1.6), 0.36, saturate(lw / 1.6 - 0.5));
            e = col_hsv(hue, 0.75, 1.0) * (0.25 + 0.75 * dots) * (0.25 + 0.12 * sin(y * 0.05 - t * 0.2));
        }
        default: {
            // dark for the night: a few stragglers
            let cell = vec2i(i32(floor(xm / 3.0)), i32(floor(y / 3.8)));
            let on = hash_cell2(cell + vec2i(i32(wid), 7), 0x77u).x < 0.06;
            e = col_kelvin(4500.0) * mix(select(0.0, 0.8, on), 0.05, saturate(lw / 3.8 - 0.5)) * 0.3;
        }
    }
    // a mottle a few pixels across: rooms lit and dark, blinds, fittings
    if (tw.kind == 1u || tw.kind == 2u || tw.kind == 3u) {
        let rc = vec2i(i32(floor(xm / 6.0)), i32(floor(y / 8.0)));
        let n = hash_cell2(rc + vec2i(i32(wid) * 3, 11), 0x6a09u).x;
        let k = saturate(lw / 12.0 - 0.5);
        e *= mix(0.55 + 0.9 * n, 1.0, k);
    }
    e += body;
    // lit crowns and roof bands
    let crown = step(tw.h - 7.0, y) * step(y, tw.h + 1.0) * step(0.74, hs.w);
    e += crown * mix(vec3f(0.85, 0.92, 1.0), led_col(fract(hs.w * 5.1)), step(0.88, hs.w)) * 0.7;
    // the company name high on the harbour side
    if (tw.h > 110.0 && hs.w < 0.28 && tw.kind != 2u) {
        let sy = tw.h - 16.0;
        let sgn = step(abs(y - sy), 3.5) * step(abs(u - 0.5), 0.34);
        let lc = select(select(vec3f(1.0), vec3f(1.0, 0.12, 0.08), hs.x < 0.35), vec3f(0.2, 0.5, 1.0), hs.x > 0.75);
        let letters = mix(0.6 + 0.4 * step(0.4, fract(u * 11.0 + hs.z * 5.0)), 0.8, saturate(lw / 2.0));
        e = mix(e, lc * 0.85 * letters, sgn);
    }
    return e;
}

fn landmark_light(u: f32, y: f32, tw: Twr, lod: f32, l: Look, t: f32) -> vec3f {
    let id = -tw.id;
    var e = vec3f(0.0);
    let aa = max(lod, 1.0);
    let edge = saturate(1.0 - abs(tw.d) / max(aa * 0.8, 1.2));
    if (id < 1.5) {
        // IFC: pale glass striped by its piers, the crown stepping up in light
        let fy = y / 4.2;
        let rise = 0.6 + 0.6 * sstep(40.0, 340.0, y);
        e = col_kelvin(6400.0) * (0.05 + 0.34 * pband(u * 7.0 + 0.15, 0.0, 0.3, aa / 58.0 * 7.0)) * (0.75 + 0.25 * pband(fy, 0.3, 1.0, aa / 4.2)) * rise;
        e += l.amb * 1.5;
        let cr = sstep(338.0, 350.0, y);
        e += vec3f(0.9, 0.95, 1.0) * (0.5 * cr + 0.9 * step(380.0, y) + 0.4 * pband(y / 12.0, 0.0, 0.2, aa / 12.0) * cr);
        e += vec3f(0.8, 0.9, 1.0) * edge * 0.3;
    } else if (id < 2.5) {
        // Bank of China: white bracing, crossed in every square bay of each
        // prism, and the edges outlined, over dark blue glass
        e = vec3f(0.015, 0.03, 0.06) + l.amb * 1.4;
        let sy = fract(y / 52.0);
        let x = u - 0.5;
        let dd = min(abs(sy - 0.5 - x), abs(sy - 0.5 + x));
        let line = saturate(1.0 - dd * 52.0 / max(aa * 0.8, 1.5));
        let bay = saturate(1.0 - abs(fract(y / 52.0 + 0.5) - 0.5) * 52.0 / max(aa * 0.8, 1.5));
        e += vec3f(0.85, 0.92, 1.0) * max(max(line, bay * 0.7), edge) * 1.1;
    } else if (id < 3.5) {
        // Central Plaza: gold bands, and the crown that tells the time in
        // colour, changing every quarter hour
        e = vec3f(1.0, 0.72, 0.38) * 0.2 * (0.55 + 0.45 * pband(y / 8.0, 0.5, 1.0, aa / 8.0));
        e += vec3f(1.0, 0.55, 0.2) * edge * 0.5;
        let q = floor(fmod_pos(t / 900.0, 4.0));
        let cc = select(select(select(vec3f(1.0, 0.3, 0.25), vec3f(1.0, 0.95, 0.9), q > 0.5), vec3f(1.0, 0.8, 0.3), q > 1.5), vec3f(0.65, 0.4, 1.0), q > 2.5);
        e = mix(e, cc * 1.1, step(300.0, y));
    } else if (id < 4.5) {
        // The Center: neon along every floor, slowly cycling through colours
        e = col_kelvin(5200.0) * 0.05 + l.amb;
        let band = pband(y / 4.0, 0.0, 0.25, aa / 4.0);
        let hue = fract(t / 150.0 + floor(y / 36.0) * 0.09);
        e += col_hsv(hue, 0.6, 1.0) * band * 0.9;
        e += col_hsv(hue, 0.4, 1.0) * edge * 0.6;
    } else {
        // convention centre: glass halls glowing warm under the wing
        e = vec3f(1.0, 0.82, 0.6) * 0.16 * step(y, tw.h - 4.0) * step(3.0, y) * (0.7 + 0.3 * pband(u * 40.0, 0.2, 1.0, aa / 300.0 * 40.0));
        e += vec3f(0.9, 0.95, 1.0) * 0.5 * step(tw.h - 1.5, y);
    }
    return e;
}

// ------------------------------------------------------------ the waterfront

// the observation wheel at Central: a lit rim, spokes, and gondolas,
// turning slowly. Returns (rgb emission, coverage)
fn wheel(x: f32, y: f32, lod: f32, t: f32) -> vec4f {
    let q = vec2f(x + 300.0, y - 40.0);
    let r = length(q);
    let aa = max(lod, 0.6);
    // the A-frame legs from the hub to the ground
    let legs = saturate(1.0 - (abs(abs(q.x) + q.y * 0.35) - 0.8) / aa) * step(q.y, 0.0) * step(-40.0, q.y);
    if (r > 36.0) {
        return vec4f(vec3f(0.04, 0.05, 0.07) * legs, legs * 0.8);
    }
    let rim = saturate(1.0 - (abs(r - 30.0) - 0.8) / aa);
    let ang = atan2(q.y, q.x) + t * 0.006;
    // spokes thin out to their average where they crowd closer than a pixel
    let spacing = TAU * r / 21.0;
    let spoke = saturate(1.0 - (abs(fract(ang / TAU * 21.0 + 0.5) - 0.5) * spacing - 0.25) / aa) * step(r, 30.0) * step(3.0, r)
              * saturate(spacing / (aa * 4.0));
    let gond = saturate(1.0 - (length(vec2f((fract(ang / TAU * 42.0) - 0.5) * TAU / 42.0 * 31.5, r - 31.5)) - 1.1) / aa);
    let hub = saturate(1.0 - (r - 2.5) / aa);
    let rimc = mix(vec3f(0.2, 0.55, 1.0), vec3f(1.0, 0.3, 0.7), 0.5 + 0.5 * sin(ang * 3.0 + t * 0.05));
    var e = rimc * rim * 0.3 + vec3f(0.5, 0.6, 1.0) * spoke * 0.08 + vec3f(1.0, 0.9, 0.75) * gond * 0.2 + vec3f(0.6) * hub;
    let a = max(max(rim, spoke * 0.25), max(max(gond, hub), legs * 0.8));
    e += vec3f(0.04, 0.05, 0.07) * legs * (1.0 - a);
    return vec4f(e, a);
}

// lights on the Peak: roads winding along the slope, houses, the tower in
// the gap, masts on the summit. hr = ridge height at x
fn peak_lights(x: f32, y: f32, hr: f32, lod: f32, l: Look, t: f32) -> vec3f {
    var e = vec3f(0.0);
    let aa = max(lod, 1.0);
    // scattered houses and lamps, thinning upward
    let cell = vec2i(i32(floor(x / 11.0)), i32(floor(y / 9.0)));
    let hc = hash_cell2(cell, 0x9eu);
    let dens = 0.3 * sstep(hr - 15.0, hr - 120.0, y) * (0.6 + 0.4 * sstep(0.0, 200.0, hr - y));
    if (hc.x < dens) {
        let p = (vec2f(cell) + vec2f(0.2) + 0.6 * hc.yz) * vec2f(11.0, 9.0);
        let dd = length(vec2f(x, y) - p);
        e += mix(col_kelvin(3000.0), col_kelvin(6500.0), hc.w) * exp(-dd * dd / (aa * aa * 0.5)) * 0.9;
    }
    // two roads traversing the slope, lamps every 30 m
    for (var i = 0; i < 2; i++) {
        let fi = f32(i);
        let ry = hr * (0.45 + 0.2 * fi) + 25.0 * sin(x * (0.004 + 0.002 * fi) + fi * 2.0);
        let lx = fract(x / 30.0 + fi * 0.5) - 0.5;
        let dd = length(vec2f(lx * 30.0, y - ry));
        e += col_kelvin(2400.0 + 1800.0 * fi) * exp(-dd * dd / (aa * aa * 0.6)) * 0.7;
    }
    // the tower in the gap: a lit bowl on the saddle
    let pt = vec2f(x + 380.0, y - (ridge(-380.0) - 30.0));
    let bowl = step(abs(pt.x), 28.0 - 0.5 * pt.y) * step(abs(pt.y - 8.0), 8.0);
    e += vec3f(1.0, 0.85, 0.6) * bowl * 0.45;
    // summit masts, their red lights slow
    let mx = abs(x + 700.0);
    let mt = ridge(-700.0);
    let mast = saturate(1.0 - (mx - 0.6) / aa) * step(y, mt + 45.0) * step(mt - 10.0, y);
    e += vec3f(0.05) * mast;
    let top = length(vec2f(mx, y - mt - 46.0)) / max(aa * 0.7, 1.0);
    e += vec3f(1.0, 0.05, 0.02) * exp(-top * top) * 1.8 * step(0.5, fract(t * 0.5));
    return e * l.night + e * 0.6 * (1.0 - l.night);
}

// ------------------------------------------------------------ compositing

fn hk_sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let y = max(rd.y, 0.0);
    // the glow of the city lies in a broad band over the ridge
    var c = mix(l.sky_lo, l.sky_hi, pow(saturate(y * 2.2), 0.75));
    c += vec3f(0.12, 0.07, 0.07) * (1.0 - l.night) * (1.0 - l.fog) * sstep(0.3, -0.9, rd.x) * exp(-y * 7.0);
    // broken cloud lit from below by the city, brightest low over the towers
    if (rd.y > 0.0) {
        let uv = rd.xz / (rd.y + 0.05) * 0.6;
        let cs = cloud_sheet(uv + vec2f(ctx.t * 0.0015, 0.0), 0.55 + 0.3 * l.fog, ctx);
        let lit = l.glow * (0.35 + 1.4 * exp(-y * 9.0)) * (0.8 + 0.4 * cs.y);
        c = mix(c, lit, cs.x * 0.85 * sstep(0.0, 0.04, rd.y));
    }
    if (l.fog > 0.0) {
        // the underside of the cloud deck, brightest just over the towers and
        // where the city below is densest
        let n = noise_fbm2(vec2f(rd.x * 5.0 + ctx.t * 0.002, rd.y * 12.0), 4);
        let cx = -rd.x / max(rd.z, 0.1) * 1500.0;
        let city = envelope(cx) * (0.7 + 0.6 * noise_value2(vec2f(cx * 0.008, 3.0)));
        let base = 170.0 / 1500.0 - 0.094;
        let above = max(rd.y - 0.06, 0.0);
        c = mix(c, l.hazec * (0.8 + 0.7 * n) * (0.65 + 0.9 * exp(-y * 8.0)), 0.9);
        c += l.glow * city * exp(-above * 6.0) * sstep(0.0, 0.07, rd.y) * 1.8;
    }
    return c;
}

// the island along one ray: premultiplied rgb and coverage
fn island(ro: vec3f, rd: vec3f, blur: f32, l: Look, ctx: Ctx, zoom: f32) -> vec4f {
    var col = vec3f(0.0);
    var acc = 0.0;
    if (rd.z <= 1e-4) { return vec4f(0.0); }
    for (var k = 0; k < NL; k++) {
        let d = layer_d(k);
        let t = (d - ro.z) / rd.z;
        let p = ro + rd * t;
        if (p.y < -2.0) { continue; }
        let lod = ctx.px / zoom * t * (1.0 + blur);
        var c = vec3f(0.0);
        var a = 0.0;
        var tw_d_last = 1e5;
        if (k == 0) {
            // the harbour wall and promenade: a string of lamps at the
            // water's edge, a road of sodium light behind
            let cell = floor(p.x / 17.0);
            let hl = hash_f(u32(i32(cell) + 90000));
            let lx = (fract(p.x / 17.0) - 0.3 - 0.4 * hl) * 17.0;
            let lamp = exp(-(sq(lx) + sq(p.y - 6.0 - 3.0 * fract(hl * 7.0))) / max(sq(lod * 0.5), 1.0));
            let lc = select(vec3f(1.0, 0.62, 0.3), vec3f(0.9, 0.95, 1.0), hl > 0.5);
            col += (1.0 - acc) * lc * lamp * 0.8 * step(0.35, fract(hl * 13.0)) * step(0.3, noise_value2(vec2f(p.x * 0.006, 1.0)));
            let road = exp(-sq((p.y - 11.0) / max(lod * 0.6, 0.8))) * (0.5 + 0.5 * step(0.5, fract(p.x / 9.0))) * step(380.0, abs(p.x - 60.0));
            col += (1.0 - acc) * vec3f(1.0, 0.6, 0.25) * road * 0.35 * mix(1.0, 0.6, saturate(lod / 9.0));
            let wh = wheel(p.x, p.y, lod, ctx.t);
            col += (1.0 - acc) * wh.xyz;
            acc += (1.0 - acc) * wh.w;
        }
        if (k == 9) {
            // the ridge: dark wooded slope, lights along the roads low down
            let hr = ridge(p.x);
            let dd = p.y - hr;
            a = saturate(0.5 - dd / lod);
            if (a > 0.0) {
                let n = noise_value2(vec2f(p.x * 0.012, p.y * 0.02));
                c = l.amb * (0.5 + 0.35 * n) + vec3f(0.004, 0.004, 0.005);
                c += peak_lights(p.x, p.y, hr, lod, l, ctx.t);
            }
        } else {
            var tw = tower(k, p.x, p.y, d);
            tw_d_last = 1e5;
            let lm = landmark(k, p.x, p.y);
            if (lm.d < tw.d) { tw = lm; }
            a = saturate(0.5 - tw.d / lod);
            tw_d_last = tw.d;
            if (a > 0.0) {
                let yy = p.y - tw.base;
                if (tw.id < 0.0) {
                    c = landmark_light(tw.u, yy, tw, lod, l, ctx.t);
                } else if (tw.u < 0.0 || tw.u > 1.0) {
                    // the flank: same building, seen obliquely and a shade darker
                    let fu = select(-tw.u, tw.u - 1.0, tw.u > 1.0);
                    c = facade(fract(fu * 2.3), yy, tw, lod * 2.0, l, ctx.t) * 0.5;
                    // the corner catches a little light
                    c += l.amb * 1.5 * saturate(1.0 - fu * tw.w / max(lod, 1.0));
                } else {
                    c = facade(tw.u, yy, tw, lod, l, ctx.t);
                }
                // the lowest floors: shops and lobbies glowing warm
                if (k >= 1 && k <= 4) {
                    c += vec3f(1.0, 0.75, 0.45) * 0.1 * (1.0 - sstep(4.0, 14.0, yy));
                }
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
        // aerial perspective across the harbour, and mist lying low on the water
        let low = exp(-max(p.y, 0.0) / 120.0);
        let fk = 1.0 - exp(-t * 0.001 * l.haze * (1.0 + 0.8 * low));
        c = mix(c, l.hazec * a * (1.0 + 0.5 * low), fk);
        // low cloud: towers dissolve upward into the deck, their light
        // smeared into it
        var ae = a;
        if (l.fog > 0.0) {
            let n = noise_fbm2(vec2f(p.x * 0.004 + ctx.t * 0.006, p.y * 0.012), 4);
            let base = l.cloud_base + 60.0 * (n - 0.5) + f32(k) * 6.0;
            let inside = sstep(base - 40.0, base + 40.0, p.y);
            let deep = sstep(base, base + 130.0, p.y);
            col += (1.0 - acc) * a * inside * (1.0 - deep) * c * 0.28;
            ae = a * (1.0 - inside);
            // lit towers make a halo in the cloud around them
            if (k != 9 && k != 0 && tw_d_last > 0.0) {
                let cw = layer_cw(k);
                let fx = fract(p.x / cw);
                let fade = sstep(0.0, 0.3, min(fx, 1.0 - fx));
                col += (1.0 - acc) * inside * l.glow * 0.06 * exp(-tw_d_last / 22.0) * (1.0 - a) * fade;
            }
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
    let hull = max(sdf2_box(q - vec2f(0.0, 1.6), vec2f(14.0, 1.6)), q.y - 3.0 - 2.5 * sstep(6.0, 14.0, -q.x));
    // three sails, each a fan widening upward between its battens
    var sail = 1e5;
    for (var k = 0; k < 3; k++) {
        var cx = 0.0;
        let h0 = 4.5;
        var h1 = 22.0;
        var w = 8.5;
        if (k == 1) { cx = 10.0; h1 = 16.0; w = 6.0; }
        if (k == 2) { cx = -11.5; h1 = 13.0; w = 5.0; }
        let v = saturate((q.y - h0) / (h1 - h0));
        let hw = w * (0.55 + 0.45 * v) * 0.5;
        let lean = v * w * 0.25;
        let sd = max(abs(q.x - cx - lean) - hw, max(h0 - q.y, q.y - h1 + 1.5 * sq((q.x - cx - lean) / max(hw, 0.1))));
        sail = min(sail, sd);
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

// harbour chop: slope of the surface (d/dx, d/dz). Crests run across the
// view, so the slope along z dominates; far away the facets average out
// and kf -> 1.
fn chop(xz: vec2f, t: f32) -> vec2f {
    var s = vec2f(0.0);
    var amp = 0.05;
    var fr = 1.3;
    var ang = 0.3;
    for (var i = 0; i < 4; i++) {
        let dir = vec2f(sin(ang), cos(ang));
        let wob = noise_value2(xz * fr * 0.15 + vec2f(f32(i) * 7.0, 0.0)) * 5.0;
        let ph = dot(xz, dir) * fr + t * sqrt(9.81 * fr) * 0.6 + wob;
        s += dir * cos(ph) * amp;
        amp *= 0.75;
        fr *= 1.8;
        ang = -ang * 1.7 + 0.5;
    }
    let n = noise_grad2(xz * vec2f(0.6, 2.2) + vec2f(0.0, t * 0.4));
    s += vec2f(0.02, 0.07) * n;
    return s;
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
        // how many metres of water along z one pixel spans: far off, many waves
        let fz = ctx.px / zoom * tw / max(-rd.y, 1e-3);
        let kf = saturate(fz / 6.0 - 0.3);
        let sl = chop(wp.xz, ctx.t) * (1.0 - kf);
        let n = normalize(vec3f(-sl.x, 1.0, -sl.y));
        var r = reflect(rd, n);
        r.y = abs(r.y) + 0.0005;
        // unresolved facets spread the mirror ray up and down
        let spread = 0.1 * kf + 0.004;
        var refl = vec3f(0.0);
        for (var k = 0; k < 3; k++) {
            let o = (f32(k) - 1.0 + (ctx.jitter - 0.5)) * spread * 0.5;
            let rk = normalize(r + vec3f(0.0, o * (0.4 + r.y * 6.0), 0.0));
            let isl = island(wp, rk, 1.0 + kf * 2.0, l, ctx, zoom);
            let fr = ferry(wp, rk, l, ctx, zoom);
            let jk = junk(wp, rk, l, ctx, zoom);
            var c = hk_sky(rk, l, ctx) * (1.0 - isl.w) + isl.xyz;
            c = c * (1.0 - jk.w) + jk.xyz;
            c = c * (1.0 - fr.w) + fr.xyz;
            refl += c;
        }
        refl /= 3.0;
        let fres = mix(water_fresnel(dot(-rd, n)), 0.35, kf * 0.5);
        let deep = vec3f(0.002, 0.005, 0.007) + l.amb * 0.25;
        col = mix(deep, refl, fres);
        let jk = junk(ro, rd, l, ctx, zoom);
        col = col * (1.0 - jk.w) + jk.xyz;
        let fr = ferry(ro, rd, l, ctx, zoom);
        col = col * (1.0 - fr.w) + fr.xyz;
        col = mix(col, l.hazec, 1.0 - exp(-tw * 0.001 * l.haze * 1.6));
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
