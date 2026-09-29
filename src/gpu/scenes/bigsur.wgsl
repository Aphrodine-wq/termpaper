//! name: bigsur
//! title: Big Sur Coast
//! category: coast
//! tags: ocean, cliffs, fog, sunset, california
//! desc: headlands of the Big Sur coast dropping into Pacific swells as marine fog rolls in
//! themes: sunset, fog, noon, moonlight
//! uses: camera, raymarch, sky, stars, clouds, fog, water
//! cost: heavy
//! fallback: ocean
//! credits: original

// World units are metres. The camera stands on a cliff top looking south
// along the coast: land on the left, the Pacific on the right and ahead, the
// sun setting over the water.

struct Look {
    sun: vec3f,       // direction toward the key light (sun or moon)
    night: f32,       // 1 = moonlit
    fog: f32,         // marine layer density multiplier
    haze: f32,        // atmospheric haze for the sky model
    exposure: f32,
    // derived once per pixel (the sky model is not free)
    sun_c: vec3f,     // direct light colour
    amb: vec3f,       // sky ambient
    fogc: vec3f,      // marine layer colour
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: { l = Look(sky_sun_dir(16.0, 25.0), 0.0, 2.4, 2.6, 0.15, vec3f(0.0), vec3f(0.0), vec3f(0.0)); }
        case 2u: { l = Look(sky_sun_dir(58.0, 150.0), 0.0, 0.35, 1.2, -0.4, vec3f(0.0), vec3f(0.0), vec3f(0.0)); }
        case 3u: { l = Look(sky_sun_dir(9.0, -14.0), 1.0, 0.8, 1.0, 0.6, vec3f(0.0), vec3f(0.0), vec3f(0.0)); }
        default: { l = Look(sky_sun_dir(2.4, 6.0), 0.0, 1.0, 1.6, 0.0, vec3f(0.0), vec3f(0.0), vec3f(0.0)); }
    }
    if (l.night > 0.5) {
        l.sun_c = vec3f(0.07, 0.085, 0.12);
        l.amb = vec3f(0.008, 0.011, 0.02);
        l.fogc = vec3f(0.012, 0.016, 0.026);
    } else {
        l.sun_c = sky_sun_light(l.sun);
        l.amb = sky_ambient(l.sun);
        l.fogc = l.amb * 0.9 + l.sun_c * 0.18;
    }
    return l;
}

// Coastline: land lies at x < coast_x(z). Headlands jut out as z recedes.
fn coast_x(z: f32) -> f32 {
    let s = -z;
    return -150.0 + 170.0 * sin(s * 0.0024 + 0.9) + 90.0 * sin(s * 0.0061 + 2.0)
        + 45.0 * noise_grad2(vec2f(s * 0.004, 3.0)) + 18.0 * noise_grad2(vec2f(s * 0.011, 7.0)) - s * 0.035;
}

// Sea stacks: one candidate rock every 330 m of coast, some just offshore.
fn stacks(p: vec3f) -> f32 {
    let cell = round(p.z / 330.0);
    var d = 1e5;
    for (var k = -1; k <= 1; k++) {
        let c = cell + f32(k);
        let h = hash_cell2(vec2i(i32(c), 3), 0x5eau);
        if (h.w > 0.65) { continue; }
        let z = c * 330.0 + (h.x - 0.5) * 160.0;
        let x = coast_x(z) + 30.0 + 90.0 * h.y;
        let r = 9.0 + 16.0 * h.z;
        let tall = 20.0 + 55.0 * h.x;
        let q = p - vec3f(x, 0.0, z);
        let body = sdf_cone_y(q - vec3f(0.0, tall * 0.5 - 8.0, 0.0), tall * 0.5 + 8.0, r * 1.25, r * 0.55);
        if (body > 12.0) {
            d = min(d, body - 5.0);
        } else {
            let rough = noise_grad3(q * 0.12) * 3.5 + noise_grad3(q * 0.4) * 1.0;
            d = min(d, body + rough);
        }
    }
    return d;
}

fn land_h(xz: vec2f, oct: i32) -> f32 {
    return land_hc(xz, coast_x(xz.y), oct);
}

fn land_hc(xz: vec2f, cx: f32, oct: i32) -> f32 {
    let d = cx - xz.x;
    let n = noise_terrain(xz * 0.0045, oct);
    let rough = (noise_value2(xz * 0.03) - 0.5) * 2.0;
    // cliffs: near-vertical for the first ~30 m inland, carved by gullies,
    // then rolling ridges
    let gully = noise_ridged2(vec2f(xz.y * 0.018, xz.x * 0.004), 3);
    let cliff = 175.0 * (1.0 - exp(-max(d, 0.0) / 28.0)) * (0.8 + 0.28 * gully);
    let ridge = 260.0 * n.x * smoothstep(0.0, 400.0, d);
    let below = min(d, 0.0) * 0.6 - 6.0 + rough * 3.0;
    let above = cliff + ridge + rough * 7.0 - 6.0;
    return mix(below, above, smoothstep(-4.0, 4.0, d));
}

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    // cheap bounds first: above every peak, or far out to sea
    if (p.y > 470.0) { return vec2f(p.y - 460.0, 1.0); }
    let cx = coast_x(p.z);
    let off = p.x - cx;
    if (off > 170.0) { return vec2f((off - 150.0) * 0.6, 1.0); }
    var r = vec2f((p.y - land_hc(p.xz, cx, 3)) * 0.5, 1.0);
    if (off > -30.0 && p.y < 85.0) { r = op_umin(r, vec2f(stacks(p) * 0.7, 2.0)); }
    return r;
}

// soft terrain shadow toward the light: a short march over a 2-octave
// heightfield (the full map with sea stacks is ten times the cost)
fn land_shadow(ro: vec3f, l: vec3f, ctx: Ctx) -> f32 {
    var res = 1.0;
    var t = 10.0;
    for (var i = 0; i < 20; i++) {
        let p = ro + l * t;
        if (p.y > 470.0) { break; }
        // the coarse heightfield can sit above the detailed surface: bias it
        let h = p.y - land_h(p.xz, 2) + 9.0;
        res = min(res, 10.0 * h / t);
        if (res < 0.01) { break; }
        t += max(8.0, t * 0.18);
    }
    return saturate(res);
}

fn land_normal(xz: vec2f, t: f32, ctx: Ctx) -> vec3f {
    let e = max(0.6, t * ctx.px * 0.8);
    let oct = select(8, 5, t > 1500.0);
    let hx = land_h(xz + vec2f(e, 0.0), oct) - land_h(xz - vec2f(e, 0.0), oct);
    let hz = land_h(xz + vec2f(0.0, e), oct) - land_h(xz - vec2f(0.0, e), oct);
    return normalize(vec3f(-hx, 2.0 * e, -hz));
}

fn backdrop(rd: vec3f, l: Look, ctx: Ctx, with_sun: bool) -> vec3f {
    if (l.night > 0.5) {
        var c = sky_night(rd) * 1.6;
        if (with_sun) {
            let sr = star_rotate(rd, 36.0, ctx.t * 20.0);
            c += star_field(sr, 0.8, ctx) * smoothstep(0.0, 0.25, rd.y);
        }
        c += vec3f(0.02, 0.028, 0.045) * pow(saturate(dot(rd, l.sun)), 6.0);
        if (with_sun) { c += sky_moon(rd, l.sun, 0.5, 1.3); }
        return c;
    }
    var c = sky_atmosphere_haze(rd, l.sun, l.haze);
    if (with_sun) { c += sky_sun_disk(rd, l.sun, 0.55); }
    // high cirrus deck
    if (rd.y > 0.0) {
        let hp = rd.xz / (rd.y + 0.04) * 1.6;
        let cs = cloud_sheet(hp * 0.9 + vec2f(3.0, 1.0), 0.42, ctx);
        let lit = l.sun_c * (0.25 + 0.9 * pow(saturate(dot(rd, l.sun) * 0.5 + 0.5), 4.0)) + l.amb * 0.6;
        c = mix(c, lit * mix(0.6, 1.2, cs.y), cs.x * 0.55 * smoothstep(0.0, 0.12, rd.y));
    }
    return c;
}

// marine layer: dense at the sea surface, patchy, drifting inland
fn marine_fog(col: vec3f, ro: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let hit = ro + rd * t;
    let blot = noise_fbm2(hit.xz * 0.0016 + vec2f(ctx.t * 0.006, ctx.t * 0.002), 4);
    let dens = l.fog * (0.0009 + 0.003 * smoothstep(0.35, 0.75, blot));
    let fogc = l.fogc;
    // far away, the air takes the colour of the sky at the horizon — the
    // sea then meets the sky without a seam
    let horizon = backdrop(normalize(vec3f(rd.x, 0.025, rd.z)), l, ctx, false);
    var c = fog_height(col, mix(fogc, horizon, 0.45), ro, rd, t, dens, 0.022);
    c = mix(c, horizon, 1.0 - exp(-t * 0.00016 * (0.6 + l.haze * 0.4)));
    if (l.night < 0.5) { c += fog_sun(rd, l.sun, l.sun_c, 0.25 * (1.0 - exp(-t * 0.0004))); }
    return c;
}

fn shade_land(p: vec3f, rd: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    var n = land_normal(p.xz, t, ctx);
    if (mat > 1.5) { n = rm_normal(p, t, ctx); }
    let slope = 1.0 - n.y;
    let d = coast_x(p.z) - p.x;
    // dry golden grass on the tops, chaparral mid, bare rock on the cliffs
    let grass = col_hex(0xb09a5eu);
    let scrub = col_hex(0x4d5334u);
    let rock = col_hex(0x7a6a5cu);
    let sand = col_hex(0xb8a88au);
    let n1 = noise_fbm2(p.xz * 0.02, 4);
    let chap = col_hex(0x2f3b24u);
    var alb = mix(grass, scrub, smoothstep(0.35, 0.7, n1 + slope * 0.6));
    alb = mix(alb, chap, smoothstep(0.55, 0.75, noise_fbm2(p.xz * 0.008 + 7.0, 4)) * 0.8);
    // layered sandstone on the cliff faces
    let strata = 0.86 + 0.14 * sin(p.y * 0.37 + noise_fbm2(p.xz * 0.02 + p.y * 0.01, 3) * 9.0) * noise_value2(vec2f(p.y * 0.1, p.z * 0.01));
    alb = mix(alb, rock * strata, smoothstep(0.28, 0.55, slope + (n1 - 0.5) * 0.3));
    if (mat > 1.5) { alb = rock * 0.7 * strata; }
    alb = mix(sand, alb, smoothstep(2.0, 9.0, p.y));
    alb *= 0.75 + 0.5 * noise_value2(p.xz * 0.25);
    let sun_c = l.sun_c;
    let amb = l.amb;
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    if (dif > 0.0 && t < 2500.0) {
        // start past the march's hit tolerance, which grows with distance
        sh = land_shadow(p + n * (1.5 + t * ctx.px * 1.5), l.sun, ctx);
    }
    let occ = 0.6 + 0.4 * saturate(n.y);
    var c = alb * (sun_c * dif * sh * 1.1 + amb * occ * 1.5);
    // wet dark rock at the waterline
    c *= mix(0.45, 1.0, smoothstep(0.0, 6.0, p.y));
    return c;
}

fn shade_water(p: vec3f, rd: vec3f, t: f32, ro: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let n = water_normal(p.xz, ctx.t, 0.9, t, ctx);
    let r = reflect(rd, n);
    var refl = backdrop(normalize(vec3f(r.x, abs(r.y) + 0.01, r.z)), l, ctx, false);
    // sun / moon glitter
    let spec_col = select(l.sun_c * 9.0, vec3f(0.5, 0.55, 0.65), l.night > 0.5);
    let hv = normalize(l.sun - rd);
    let glit = pow(saturate(dot(n, hv)), 900.0) * 60.0 + pow(saturate(dot(n, hv)), 80.0) * 1.2;
    refl += spec_col * glit;
    let d = p.x - coast_x(p.z); // metres offshore
    let shallow = exp(-max(d, 0.0) / 40.0);
    var deep = col_hex(0x0a2a3au) * 0.18;
    var shal = col_hex(0x2f7f86u) * 0.28;
    let amb = l.amb;
    deep *= amb * 3.0 + 0.02;
    shal *= amb * 3.0 + 0.02;
    var c = water_color(rd, n, refl, deep, shal, 1.0 - shallow);
    // surf: foam lines that roll toward the rocks
    let wave = sin(d * 0.09 + ctx.t * 0.9 + noise_value2(p.xz * 0.02) * 4.0);
    let foam_n = noise_fbm2(p.xz * vec2f(0.15, 0.08) + vec2f(ctx.t * 0.05, 0.0), 4);
    let near = exp(-max(d, 0.0) / 22.0);
    let foam = saturate(near * (0.55 + 0.45 * wave) * 1.6 + smoothstep(0.62, 0.85, foam_n) * near) *
               smoothstep(-2.0, 2.0, d);
    let foam_lit = select(l.sun_c * 0.6 + amb * 1.2, vec3f(0.02, 0.025, 0.04), l.night > 0.5);
    c = mix(c, foam_lit * 0.9, foam * 0.85);
    return c;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(330.0, 200.0, 320.0);
    let cam = cam_look_at(ro, ro + vec3f(-0.40, -0.135, -1.0), 0.0, 48.0);
    let rd = cam_ray(cam, p);
    var col: vec3f;
    let tw = water_intersect(ro, rd, 0.0);
    var tl = -1.0;
    var mat = 1.0;
    if (rd.y < 0.08) {
        let tmax = select(14000.0, tw + 30.0, tw > 0.0);
        let hit = rm_march_k(ro, rd, 5.0, tmax, steps(110.0, ctx), 1.0, ctx);
        tl = hit.x;
        mat = hit.y;
    }
    // a grazing ray can slip past a thin gully ridge and "hit" the sea plane
    // underneath the land; re-march that span carefully
    if (tl < 0.0 && tw > 0.0) {
        let wp = ro + rd * tw;
        if (coast_x(wp.z) - wp.x > -2.0) {
            let hit = rm_march_k(ro, rd, max(tw * 0.6, 5.0), tw + 5.0, 96, 0.25, ctx);
            if (hit.x > 0.0) {
                tl = hit.x;
                mat = hit.y;
            } else {
                tl = tw;
            }
        }
    }
    if (tl > 0.0 && (tw < 0.0 || tl <= tw)) {
        let hp = ro + rd * tl;
        col = marine_fog(shade_land(hp, rd, tl, mat, l, ctx), ro, rd, tl, l, ctx);
    } else if (tw > 0.0) {
        let hp = ro + rd * tw;
        col = marine_fog(shade_water(hp, rd, tw, ro, l, ctx), ro, rd, tw, l, ctx);
    } else {
        col = backdrop(rd, l, ctx, true);
        // the marine layer hugging the horizon
        let band = exp(-max(rd.y, 0.0) * 60.0) * l.fog * 0.35;
        col = mix(col, l.fogc * 1.1, saturate(band));
    }
    return col * exp2(l.exposure);
}
