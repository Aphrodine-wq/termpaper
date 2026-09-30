//! name: braided
//! title: Braided River
//! category: aerial
//! tags: drone, river, iceland, glacial, delta, aerial, top-down
//! desc: straight down at a glacial river braiding over black sand, milky channels split and rejoin
//! themes: day, dusk, winter
//! params: flow, altitude
//! uses: noise
//! cost: light
//! fallback: ripple
//! credits: original

// A nadir drone view of an Icelandic glacial outwash plain. A belt of milky
// channels wanders across black sand: each channel is a sinuous ribbon, and
// the ribbons cross, merge and part again, which is what braiding looks like
// from straight above. Grey gravel bars sit between them, moss (or snow) on
// the higher banks either side. Silt slides slowly along every channel, a
// cloud shadow crosses the plain now and then, and foam glitters faintly
// where channels meet. Everything else holds still.

const NCH: i32 = 7;                 // channels in the belt; the first three are the big ones
const FLOW_ANGLE: f32 = -0.22;      // the belt runs across the frame, tilted a little

struct Look {
    shallow: vec3f,     // milky water along the banks
    deep: vec3f,        // water in mid-channel
    sand_wet: vec3f,    // black sand at the water's edge
    sand: vec3f,        // dry black sand
    gravel: vec3f,      // the bars between channels
    bank: vec3f,        // moss (or snow) on the higher banks
    foam: vec3f,
    glint: vec3f,       // sun glints on the water (zero under overcast)
    shade: vec3f,       // what the shaded side of a bank falls toward
    cloud: vec3f,       // multiplier under the cloud shadow
    light_dir: vec2f,   // where the light comes from, in belt space
    relief: f32,        // strength of the contour shading
    snow: f32,          // snow cover on bars and low banks
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            // dusk: long warm side light, orange glints, purple-grey sand, warm bars
            return Look(col_hex(0xa89088u), col_hex(0x5a6478u),
                        col_hex(0x2a2432u), col_hex(0x44394eu), col_hex(0x8c7a6cu), col_hex(0x6a5a30u),
                        col_hex(0xffe0c0u), col_hex(0xff9a40u) * 2.0,
                        vec3f(0.2, 0.15, 0.3), vec3f(0.45, 0.4, 0.55),
                        normalize(vec2f(1.0, 0.25)), 0.9, 0.0, 0.8);
        }
        case 2u: {
            // winter: snow on the bars and banks, dark navy water, pale sky in it, blue shadows
            return Look(col_hex(0x5a7090u), col_hex(0x1c2a44u),
                        col_hex(0x24272cu), col_hex(0x363a40u), col_hex(0xd6dbe2u), col_hex(0xe9edf3u),
                        col_hex(0xffffffu), vec3f(0.5, 0.55, 0.65),
                        vec3f(0.45, 0.55, 0.8), vec3f(0.6, 0.66, 0.8),
                        normalize(vec2f(0.5, 0.8)), 0.5, 1.0, 0.95);
        }
        default: {
            // day: flat overcast light, turquoise water, black sand, ochre moss
            return Look(col_hex(0xa8c9c6u), col_hex(0x6fb0b0u),
                        col_hex(0x17181au), col_hex(0x2b2c2fu), col_hex(0x74746eu), col_hex(0x7b7434u),
                        col_hex(0xeef4f2u), vec3f(0.0),
                        vec3f(0.35, 0.37, 0.4), vec3f(0.55, 0.57, 0.62),
                        normalize(vec2f(0.4, 0.9)), 0.3, 0.0, 1.0);
        }
    }
}

// ------------------------------------------------------------------ the channels
// The channel field at one point of the belt. x runs along the flow, v across
// it (already relative to the belt's centreline). Each channel is a ribbon
// around a wandering centreline; where two ribbons cross they merge.
struct Braid {
    d1: f32,    // signed distance to the nearest channel (negative = water)
    d2: f32,    // to the second nearest
    w1: f32,    // half width of the nearest
}

fn braid(x: f32, v: f32, ctx: Ctx) -> Braid {
    var b = Braid(1e3, 1e3, 0.03);
    for (var i = 0; i < NCH; i++) {
        let fi = f32(i);
        let h = hash_seeded2(vec2i(i, 0), 0xb7a1u, ctx);
        let big = i < 3;
        let lane = (h.x - 0.5) * 0.56;                          // resting position across the belt
        let amp = select(0.07, 0.13, big) + 0.07 * h.y;          // how far it wanders
        let fx = select(1.6, 1.0, big) + 0.6 * h.z;              // how often
        let wander = noise_fbm2(vec2f(x * fx + h.w * 40.0, 7.3 * fi + 2.1), 2) - 0.5;
        let cy = lane + amp * 2.2 * wander;
        // widening and narrowing along the channel
        let wm = 0.55 + 0.9 * noise_value2(vec2f(x * 2.2 + fi * 13.0, 0.5));
        let w = select(0.016, 0.04, big) * wm;
        let d = abs(v - cy) - w;
        if (d < b.d1) {
            b.d2 = b.d1;
            b.d1 = d;
            b.w1 = w;
        } else if (d < b.d2) {
            b.d2 = d;
        }
    }
    return b;
}

// ------------------------------------------------------------------ the land
// The plain's height and its two material noises: (height, bars, hummocks).
// The plain rises away from the belt; gravel bars are lumps inside it, moss
// hummocks sit on the higher ground.
fn land(q: vec2f, v: f32) -> vec3f {
    let bank = sstep(0.28, 0.6, abs(v));
    let bars = noise_fbm2(q * 6.0 + 11.0, 3);
    let hum = noise_fbm2(q * 4.0 + 31.0, 3);
    let h = bank * 0.6 + bars * 0.25 + hum * 0.2 * bank;
    return vec3f(h, bars, hum);
}

// ------------------------------------------------------------------ the scene
fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let k = ctx.params;
    let flow = k.x * 2.0;                               // 0.5 -> as designed, 0 -> still
    let zoom = pow(2.0, (k.y - 0.5) * 2.0);            // altitude: higher = further, smaller braids
    let q = rot2(FLOW_ANGLE) * p * zoom;               // belt space: x along the flow, y across
    let px = ctx.px * zoom;                             // one output pixel in belt units

    // the belt itself wanders slowly across the plain
    let belt = 0.18 * (noise_fbm2(vec2f(q.x * 0.35 + ctx.seedf.x * 10.0, 3.3), 2) - 0.5);
    let v = q.y - belt;

    // the channels, with a soft union of the two nearest so confluences widen
    let b = braid(q.x, v, ctx);
    let kk = 0.02;
    let hh = saturate(0.5 + 0.5 * (b.d2 - b.d1) / kk);
    let d = mix(b.d2, b.d1, hh) - kk * hh * (1.0 - hh);

    // the land: height twice (for the contour shading), materials once
    let ld = l.light_dir;
    let eps = max(0.015, px * 2.0);
    let g0 = land(q, v);
    let g1 = land(q + ld * eps, v + ld.y * eps);
    let slope = (g1.x - g0.x) / eps;                    // rising toward the light = lit
    let bank = sstep(0.28, 0.6, abs(v));

    var land_col = l.sand;
    // gravel bars: the lumps inside the belt
    let gravel_m = sstep(0.5, 0.6, g0.y) * (1.0 - bank * 0.7);
    land_col = mix(land_col, l.gravel, gravel_m);
    // moss on the higher ground, patchy where the bank begins
    let moss_m = bank * sstep(0.35, 0.6, g0.z + bank * 0.3);
    land_col = mix(land_col, l.bank, moss_m);
    // snow lies on anything that stands up; the black sand shows through where the wind took it
    let snow_m = l.snow * sstep(0.3, 0.55, g0.y * 0.6 + bank * 0.5 + g0.z * 0.3);
    land_col = mix(land_col, l.bank, snow_m);
    // wet sand darkens toward the water
    let wet = sstep(0.03, 0.0, d);
    land_col = mix(land_col, l.sand_wet, wet * 0.6 * (1.0 - snow_m * 0.6));

    // the water: milky at the banks, deeper colour mid-channel, silt sliding along
    let depth = saturate(-d / (b.w1 + 0.008));
    let sx = q.x - t * 0.03 * flow;
    let silt = noise_fbm2(vec2f(sx * 14.0, v * 55.0) + 5.0, 2);
    var wcol = mix(l.shallow, l.deep, 0.8 * sstep(0.0, 1.0, depth));
    wcol *= 0.88 + 0.24 * silt;
    // glints where the low light catches the ripples; they slide with the silt
    wcol += l.glint * pow(saturate(silt * 1.15), 7.0) * depth;
    // foam where two channels meet, glittering slowly
    let seam = sstep(0.012, 0.0, abs(b.d2 - b.d1)) * sstep(0.0, -0.006, b.d2);
    let fn_ = noise_value2(vec2f(sx * 30.0, v * 60.0));
    let glitter = 0.6 + 0.4 * sin(t * 1.1 + fn_ * 6.0);
    wcol = mix(wcol, l.foam, seam * sstep(0.35, 0.7, fn_) * 0.8 * glitter);

    // compose: land, then water over it with soft banks
    let aa = max(0.006, px * 1.2);
    let water = sstep(aa, -aa, d);
    var col = mix(land_col, wcol, water);

    // contour shading on the land (a touch on the water's shaded bank)
    let sh = slope * l.relief;
    col = mix(col, col * l.shade, saturate(-sh) * (1.0 - water * 0.7));
    col *= 1.0 + 0.3 * saturate(sh) * (1.0 - water);

    // a big soft cloud shadow drifting across the whole plain
    let cl = noise_fbm2(p * 0.6 + vec2f(-t * 0.011, t * 0.003) + ctx.seedf * 7.0, 3);
    let shadow = sstep(0.5, 0.62, cl);
    col = mix(col, col * l.cloud, shadow);

    return max(col * l.exposure, vec3f(0.0));
}
