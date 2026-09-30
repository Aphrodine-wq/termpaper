//! name: coastline
//! title: Drone over the Shore
//! category: aerial
//! tags: drone, beach, surf, waves, aerial
//! desc: a drone hanging over a beach, lines of surf rolling onto the sand from above
//! themes: golden, noon, storm
//! params: swell, altitude
//! uses: camera, sky, water, noise, fog, clouds, rain
//! cost: medium
//! fallback: ocean
//! credits: original

// World units are metres, y up, sea level at y = 0. The shoreline runs along
// z: dry sand at x < shore_x(z), sea at x > shore_x(z). The drone hangs a
// little seaward of the water line and looks along the beach toward -z,
// pitched down so the horizon sits high in the frame. Motion lives in the
// surf; the camera only drifts.

struct Look {
    sun: vec3f,
    haze: f32,          // sky haze
    fog: f32,           // aerial perspective density, 1/m
    exposure: f32,      // stops
    sun_gain: f32,      // direct light multiplier (the storm sheet eats it)
    cloud: f32,         // cloud sheet coverage
    dark: f32,          // 1 = storm: grey sheet, rain, muted sea
    swell: f32,         // theme swell multiplier
    sand: vec3f,
    shallow: vec3f,
    deep: vec3f,
    grass: vec3f,
    sun_c: vec3f,       // derived: direct light colour
    amb: vec3f,         // derived: sky ambient
    shadow_len: f32,    // derived: ground shadow length per metre of height
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            // noon: high white sun, saturated turquoise, crisp foam
            l.sun = sky_sun_dir(72.0, 55.0);
            l.haze = 1.0; l.fog = 0.00055; l.exposure = -2.3; l.sun_gain = 1.0;
            l.cloud = 0.18; l.dark = 0.0; l.swell = 1.0;
            l.sand = col_hex(0xf0e4c6u); l.shallow = col_hex(0x22d0c2u);
            l.deep = col_hex(0x06377eu); l.grass = col_hex(0x7f8b48u);
        }
        case 2u: {
            // storm: grey-green sea, bigger swell, dark low sheet, rain far out
            l.sun = sky_sun_dir(26.0, -35.0);
            l.haze = 1.7; l.fog = 0.0022; l.exposure = -1.3; l.sun_gain = 0.1;
            l.cloud = 0.92; l.dark = 1.0; l.swell = 1.9;
            l.sand = col_hex(0xa39680u); l.shallow = col_hex(0x4d7d72u);
            l.deep = col_hex(0x1b3a42u); l.grass = col_hex(0x5b6a3eu);
        }
        default: {
            // golden: low warm sun behind the left shoulder, long soft shadows
            l.sun = sky_sun_dir(9.0, -140.0);
            l.haze = 2.2; l.fog = 0.0009; l.exposure = -1.1; l.sun_gain = 1.0;
            l.cloud = 0.32; l.dark = 0.0; l.swell = 1.0;
            l.sand = col_hex(0xe9d2a6u); l.shallow = col_hex(0x3cb3acu);
            l.deep = col_hex(0x0f3d6cu); l.grass = col_hex(0x8a8a4eu);
        }
    }
    l.sun_c = sky_sun_light(l.sun) * l.sun_gain;
    let amb = sky_ambient(l.sun);
    // under the storm sheet the ambient goes grey and drops
    l.amb = mix(amb, vec3f(col_luma(amb)) * 0.75, l.dark * 0.7);
    let horiz = sqrt(max(1.0 - l.sun.y * l.sun.y, 0.0));
    l.shadow_len = min(horiz / max(l.sun.y, 0.05), 7.0);
    return l;
}

// Water line: sand at x < shore_x(z). A gently curving beach.
fn shore_x(z: f32) -> f32 {
    return 34.0 * sin(z * 0.0031 + 0.8) + 12.0 * sin(z * 0.0093 + 2.4)
        + 6.0 * noise_grad2(vec2f(z * 0.02, 5.5));
}

// distance from a point to the segment a-b (umbrella shadows)
fn seg_dist(p: vec2f, a: vec2f, b: vec2f) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = saturate(dot(pa, ba) / max(dot(ba, ba), 1e-4));
    return length(pa - ba * h);
}

// Surf at d metres seaward of the water line, z along the shore.
// Returns (foam 0..1, surface slope along the shore normal, breaking 0..1, phase 0..1).
fn surf(d: f32, z: f32, t: f32, sw: f32) -> vec4f {
    let wl = 18.0 + 8.0 * sw;
    let wob = 0.22 * noise_grad2(vec2f(z * 0.017, 3.7))
        + 0.08 * noise_grad2(vec2f(z * 0.05, d * 0.01 + 9.0));
    let ph = (d + t * 2.3) / wl + wob;          // crests travel shoreward
    let f = fract(ph);                          // 0 at the crest, foam trails seaward
    let d_break = 20.0 + 50.0 * sw;
    let brk = sstep(d_break + 30.0, d_break - 12.0, d);
    let shape = exp(-f * 3.2) * (1.0 - sstep(0.62, 1.0, f));
    // the swell steepens as it shoals and collapses once broken
    let amp = 0.9 * sw * (1.0 + 0.7 * sstep(160.0, 30.0, d)) * (1.0 - 0.55 * brk * shape)
        * mix(0.4, 1.0, sstep(240.0, 80.0, d));       // open-water swell is gentler
    let slope = -amp * TAU / wl * sin(TAU * ph);
    return vec4f(shape * brk, slope, brk, f);
}

// the cheap horizon colour both the sky and the aerial perspective share
fn horizon_col(hz: vec3f, l: Look) -> vec3f {
    return sky_fast(hz, l.sun) * (1.0 - 0.45 * l.dark);
}

fn shore_sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let hz = normalize(vec3f(rd.x, 0.006, rd.z));
    let hf = sstep(0.0, 0.12, rd.y);
    var col = mix(horizon_col(hz, l), sky_atmosphere_haze(rd, l.sun, l.haze), hf);
    col += sky_sun_disk(rd, l.sun, 0.6) * (1.0 - l.dark);
    // a cloud sheet on a plane above the camera, fading into the haze
    let up = max(rd.y, 0.03);
    let q = rd.xz / up * 0.45 + vec2f(3.0, 1.0);
    let cs = cloud_sheet(q, l.cloud, ctx);
    let bright = mix(l.amb * 1.2, l.sun_c * 0.25 + l.amb * 0.5, cs.y);
    let storm = l.amb * 0.55 * (0.6 + 0.4 * cs.y);
    let ccol = mix(bright, storm, l.dark);
    col = mix(col, ccol, cs.x * sstep(0.0, 0.10, rd.y));
    return col;
}

fn shore_ground(g: vec3f, rd: vec3f, dist: f32, l: Look, sw: f32, t: f32, ctx: Ctx) -> vec3f {
    let sx = shore_x(g.z);
    let d = g.x - sx;                                   // metres seaward of the water line
    let dz = (shore_x(g.z + 2.0) - shore_x(g.z - 2.0)) * 0.25;
    let sn = normalize(vec2f(1.0, -dz));                // shore normal in xz, seaward
    let pxm = max(dist * ctx.px * 1.4 / sqrt(max(-rd.y, 0.05)), 0.05); // metres per pixel here
    let lod = sstep(7.0, 1.5, pxm);                     // 1 near, 0 far
    let ndl = max(l.sun.y, 0.0);
    let key = l.sun_c * ndl;
    let shd_dir = -normalize(vec2f(l.sun.x, l.sun.z) + vec2f(1e-4, 0.0));
    let flat_r = reflect(rd, vec3f(0.0, 1.0, 0.0));
    let refl_flat = horizon_col(normalize(vec3f(flat_r.x, max(flat_r.y, 0.02), flat_r.z)), l);
    let fr_flat = water_fresnel(max(-rd.y, 0.0));

    // ---- surf, shared by sea and swash -----------------------------------
    let s = surf(d, g.z, t, sw);
    var nz = noise_fbm2(vec2f(g.x * 0.11, g.z * 0.075) + vec2f(0.0, t * 0.12), 3);
    nz = mix(0.5, nz, lod);
    let foam = sstep(0.55, 0.9, s.x + 0.5 * nz);
    let foam_c = vec3f(0.92) * (l.amb * 1.4 + l.sun_c * (0.55 + 0.6 * ndl));

    // ---- sand --------------------------------------------------------------
    let mott = (noise_value2(g.xz * 0.05) * 0.16 + noise_value2(g.xz * 0.35) * 0.08 - 0.12) * lod;
    var sand = l.sand * (1.0 + mott);
    // dunes with grass clumps beyond ~70 m from the water
    let dune = sstep(-62.0, -84.0, d);
    let clump = sstep(0.42, 0.7, noise_fbm2(g.xz * 0.045, 3));
    sand = mix(sand, l.grass * (0.8 + 0.4 * noise_value2(g.xz * 0.2)), dune * clump);
    let crest = pulse(-78.0, 5.0, d + 4.0 * noise_grad2(vec2f(g.z * 0.03, 1.0)));
    sand *= 1.0 + 0.12 * crest * dune;
    // wet band mirrors the sky; the swash film beyond the run-up edge is a mirror
    let wet = sstep(-15.0, -6.0, d);
    sand *= mix(1.0, 0.58, wet);
    let ph0 = t * 2.3 / (18.0 + 8.0 * sw) + 0.22 * noise_grad2(vec2f(g.z * 0.017, 3.7));
    let run = 0.5 + 0.5 * cos(TAU * (ph0 - 0.15));
    let edge = -2.0 - 9.0 * sw * run + 1.5 * noise_grad2(vec2f(g.z * 0.08, 7.0));
    let film = sstep(edge - 1.0, edge + 1.0, d);
    let lace = pulse(edge, max(1.6, pxm * 1.2), d) * sstep(0.3, 0.6, nz + 0.1 * (1.0 - lod));

    // shadows: the dune line throws a long soft band seaward at a low sun
    let reach = l.shadow_len * max(dot(shd_dir, sn), 0.0);
    let dh = 4.5 + 1.5 * noise_grad2(vec2f(g.z * 0.02, 2.0));
    let dune_sh = sstep(-79.0, -74.0, d) * sstep(-76.0 + dh * reach + 3.0, -76.0 + dh * reach - 3.0, d);
    var shadow = 1.0 - 0.8 * dune_sh;

    // towels and umbrellas on the dry sand, with their shadows
    var dot_c = vec3f(0.0);
    var dot_a = 0.0;
    if (d > -66.0 && d < -8.0 && pxm < 4.0) {
        var pal = array<vec3f, 5>(col_hex(0xe0453au), col_hex(0xf2b532u), col_hex(0x2f7fd8u),
            col_hex(0xf6f1e6u), col_hex(0x3aa86bu));
        let cell = floor(g.xz / 7.0);
        for (var i = -1; i <= 1; i++) {
            for (var j = -1; j <= 1; j++) {
                let c = cell + vec2f(f32(i), f32(j));
                let h = hash_seeded2(vec2i(c), 0xbea7u, ctx);
                if (h.w > 0.16) { continue; }
                let pos = (c + 0.5) * 7.0 + (h.xy - 0.5) * 4.5;
                let cd = pos.x - shore_x(pos.y);
                if (cd > -13.0 || cd < -60.0) { continue; }
                let umb = h.z > 0.5;
                let r = select(0.9, 1.5, umb);
                if (umb) {
                    let sd = seg_dist(g.xz, pos, pos + shd_dir * l.shadow_len * 1.9) - r * 0.8;
                    shadow *= 1.0 - 0.55 * saturate(0.5 - sd / pxm);
                }
                let dd = length(g.xz - pos) - r;
                let a = saturate(0.5 - dd / pxm);
                let pc = pal[i32(h.x * 4.99)];
                dot_c = mix(dot_c, pc, a);
                dot_a = max(dot_a, a);
            }
        }
    }
    var sand_lit = sand * (key * shadow + l.amb);
    sand_lit = mix(sand_lit, dot_c * (key * 0.9 + l.amb), dot_a);
    let swash = mix(sand_lit * 0.85, refl_flat, saturate(fr_flat * 1.5 + 0.15));
    sand_lit = mix(sand_lit, swash, film * 0.75);
    sand_lit = mix(sand_lit, foam_c, lace * 0.9);

    // ---- sea ---------------------------------------------------------------
    let depth = max(d, 0.0) * (0.045 + 0.02 * noise_grad2(vec2f(g.z * 0.012, d * 0.004))) + 0.1;
    let wn = water_normal(g.xz, t, 0.35 + 0.5 * l.dark, dist, ctx);
    let swl = s.y * mix(0.2, 1.0, lod);                 // sub-pixel swell would alias into stripes
    let n = normalize(vec3f(wn.x / wn.y - swl * sn.x, 1.0, wn.z / wn.y - swl * sn.y));
    let bottom = mix(l.sand * 0.7, l.shallow, expease(depth, 1.3));
    let body = mix(bottom, l.deep, expease(depth, 0.28)) * (l.amb + key * 0.6);
    let rr = reflect(rd, n);
    let refl = horizon_col(normalize(vec3f(rr.x, max(rr.y, 0.02), rr.z)), l);
    let fr = water_fresnel(max(dot(-rd, n), 0.0));
    var sea = mix(body, refl, saturate(fr * 1.4));
    // sun glitter on the chop
    let glit = pow(max(dot(rr, l.sun), 0.0), 380.0) * (1.0 - 0.8 * l.dark);
    sea += l.sun_c * glit * 0.35;
    // the translucent lip just shoreward of a breaking crest
    let lip = pulse(0.93, 0.07, s.w) * s.z * (1.0 - foam) * lod;
    sea += l.shallow * (l.amb + key) * 0.5 * lip;
    sea = mix(sea, foam_c, foam * 0.9);

    let shore = sstep(-0.8, 0.8, d);
    return mix(sand_lit, sea, shore);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let k = ctx.params;
    let sw = (0.35 + 1.3 * k.x) * l.swell;              // swell: wave height and foam
    let alt = 60.0 * exp2((k.y - 0.5) * 2.0);           // altitude: 1.0 is twice as high

    // the drone hangs over the wet sand, drifting a metre or two, never flying
    let sway = cam_sway(t, 1.6, 11u);
    let ro = vec3f(shore_x(0.0) + 48.0 * alt / 60.0, alt, 0.0) + sway;
    let fwd = vec3f(0.0, -0.5, -0.866);                 // pitched 30 degrees down along the shore
    let cam = cam_look_at(ro, ro + fwd * 100.0, 0.0, 70.0);
    let rd = cam_ray(cam, p);

    var col: vec3f;
    if (rd.y > -1e-4) {
        col = shore_sky(rd, l, ctx);
    } else {
        let dist = -ro.y / rd.y;
        let g = ro + rd * dist;
        let hz = normalize(vec3f(rd.x, 0.006, rd.z));
        let fogc = horizon_col(hz, l);
        col = shore_ground(g, rd, dist, l, sw, t, ctx);
        col = fog_exp(col, fogc, dist, l.fog);
    }
    // faint rain far out under the storm sheet
    if (l.dark > 0.5) {
        let rs = rain_streaks(p, ctx, 0.5, 0.7, 0.15, 2);
        let hz2 = normalize(vec3f(rd.x, 0.006, rd.z));
        col += horizon_col(hz2, l) * rs * 0.3 * sstep(-0.1, 0.25, p.y);
    }
    return max(col * exp2(l.exposure), vec3f(0.0));
}
