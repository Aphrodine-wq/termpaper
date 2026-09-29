//! name: bamboo
//! title: Arashiyama Bamboo
//! category: wilds
//! tags: bamboo, forest, kyoto, lanterns, rain
//! desc: the path through Kyoto's bamboo grove, tall culms swaying and meeting overhead
//! themes: day, rain, lantern
//! uses: camera, sky, rain, light
//! cost: medium
//! fallback: canopy
//! credits: original

// World units are metres, y up, the path at y = 0 running away from the
// camera (-z) between low brushwood fences. Culms stand in depth layers:
// every 0.9 m of depth a vertical sheet of jittered culms that lean in over
// the path as they rise. Each sheet is hit exactly by the view ray and
// each culm contributes its coverage of the pixel footprint, so the far
// culms blend into the green haze instead of aliasing.

struct Look {
    sun: vec3f,
    sun_c: vec3f,
    amb: vec3f,       // green-tinted skylight inside the grove
    sky_c: vec3f,     // light through the leaves overhead
    haze: f32,
    wet: f32,
    dusk: f32,        // lanterns lit
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            l.sun = sky_sun_dir(50.0, 10.0);
            l.sun_c = vec3f(0.0);
            l.amb = vec3f(0.07, 0.095, 0.08);
            l.sky_c = vec3f(0.5, 0.54, 0.54);
            l.haze = 0.028;
            l.wet = 1.0;
            l.dusk = 0.0;
            l.exposure = 0.2;
        }
        case 2u: {
            l.sun = sky_sun_dir(-6.0, 30.0);
            l.sun_c = vec3f(0.0);
            l.amb = vec3f(0.01, 0.014, 0.028);
            l.sky_c = vec3f(0.06, 0.085, 0.16);
            l.haze = 0.03;
            l.wet = 0.0;
            l.dusk = 1.0;
            l.exposure = 0.3;
        }
        default: {
            l.sun = sky_sun_dir(55.0, 25.0);
            l.sun_c = sky_sun_light(sky_sun_dir(55.0, 25.0)) * 1.1;
            l.amb = vec3f(0.13, 0.17, 0.11);
            l.sky_c = vec3f(0.85, 0.95, 0.88);
            l.haze = 0.02;
            l.wet = 0.0;
            l.dusk = 0.0;
            l.exposure = -0.1;
        }
    }
    return l;
}

// ------------------------------------------------------------ layout

// the path's centre line wanders gently
fn path_x(z: f32) -> f32 { return 1.2 * sin(z * 0.045 + 0.5) - 0.9 * sin(z * 0.013); }
const FENCE: f32 = 1.9;       // fence line, from the path centre
const GROVE: f32 = 2.4;       // first culms, from the path centre

// how far a culm leans in toward the path at height y (m), with sway
fn lean(y: f32, amt: f32) -> f32 {
    let k = saturate(y / 18.0);
    return amt * k * k;
}

// ------------------------------------------------------------ culms

struct Culms { col: vec3f, a: f32 }

// one depth sheet at z = zk: coverage and colour of the culms the ray
// crosses there. p = ray point on the sheet, fp = pixel footprint (m).
fn sheet(p: vec3f, zk: f32, k: i32, fp: f32, l: Look, ctx: Ctx) -> vec4f {
    let pc = path_x(zk);
    let w = 0.85;
    var best = vec4f(0.0);
    // left side leans +x, right side -x
    for (var side = 0; side < 2; side++) {
        let sgn = select(1.0, -1.0, side == 1);
        // undo the lean at this height to find the culm's foot
        let sw = 0.35 * sin(ctx.t * 0.55 + zk * 0.21 + f32(side) * 2.0) + 0.2 * sin(ctx.t * 0.9 + zk * 0.5);
        let reach = 2.6 + 0.6 * sin(zk * 0.7);
        let shift = sgn * lean(p.y, reach + sw);
        let u = p.x - shift;
        let c0 = floor(u / w);
        for (var j = 0; j <= 1; j++) {
            let c = c0 + f32(j) - select(0.0, 1.0, fract(u / w) < 0.5);
            let h = hash_cell2(vec2i(i32(c), k * 2 + side), 0xba3u);
            let x0 = (c + 0.2 + 0.6 * h.x) * w;
            // only on this side of the path, behind the fence
            if (sgn * (pc - x0) < GROVE) { continue; }
            let tall = 14.0 + 7.0 * h.y;
            if (p.y > tall) { continue; }
            let r = 0.045 + 0.05 * h.z;
            let d = abs(u - x0) - r;
            let a = saturate(0.5 - d / max(fp, 1e-4));
            if (a > best.x) {
                // across the culm: -1..1, for shading the round stem
                let across = clamp((u - x0) / r, -1.0, 1.0);
                best = vec4f(a, across, h.w, h.y);
            }
        }
    }
    return best;
}

fn culm_col(p: vec3f, cov: vec4f, l: Look, ctx: Ctx) -> vec3f {
    let across = cov.y;
    let age = cov.z;
    // young culms green, old ones yellowed and greyer
    var alb = mix(col_hex(0x557f2eu), col_hex(0x9a9e50u), smoothstep(0.55, 0.95, age));
    alb = mix(alb, col_hex(0x7f8c6au), smoothstep(0.85, 1.0, age) * 0.5);
    // each culm its own shade
    alb *= 0.7 + 0.5 * fract(age * 7.13);
    // rain darkens them
    alb *= mix(1.0, 0.6, l.wet);
    // nodes every ~40 cm: a dark ring with a pale bloom just below
    let ny = p.y / (0.34 + 0.12 * cov.w);
    let f = fract(ny);
    alb *= 1.0 - 0.45 * smoothstep(0.93, 0.98, f) * (1.0 - smoothstep(0.98, 1.0, f));
    alb = mix(alb, col_hex(0xc9cdb8u), smoothstep(0.75, 0.93, f) * (1.0 - smoothstep(0.93, 0.96, f)) * 0.35);
    // round stem: lit from the sunward side, a gloss line
    let nrm = vec3f(across, 0.0, sqrt(max(1.0 - across * across, 0.0)));
    let dif = saturate(dot(nrm, normalize(vec3f(l.sun.x, 0.0, -l.sun.z) + vec3f(0.3, 0.0, 0.0))));
    // (sun reaches a culm only in thin slivers through the leaves)
    let dap = dapple(p, l, ctx) * smoothstep(0.62, 0.85, noise_value2(vec2f(p.y * 0.9, p.x * 3.0 + p.z))) * 0.6;
    // skylight falls down the grove from the slot overhead: culms are lit
    // brighter higher up
    let sky_k = 1.0 + 0.8 * smoothstep(2.0, 14.0, p.y);
    var c = alb * (l.amb * (0.8 + 0.2 * nrm.z) * 1.5 * sky_k + l.sun_c * dif * dap * 0.6);
    // waxy skin: a gloss line and a sheen at the grazing edges
    c += l.sky_c * 0.06 * pow(saturate(1.0 - abs(across - 0.35)), 12.0) * (1.0 - l.dusk);
    c += (l.amb * 0.8 + l.sky_c * 0.05) * pow(1.0 - nrm.z, 3.0) * 0.5;
    if (l.wet > 0.0) { c += l.sky_c * 0.1 * pow(saturate(1.0 - abs(across + 0.2)), 20.0); }
    c += lantern_light(p, vec3f(nrm.x, 0.0, nrm.z), l, ctx) * alb;
    return c;
}

// ------------------------------------------------------------ light

// sun through the leaves: soft moving patches
fn dapple(p: vec3f, l: Look, ctx: Ctx) -> f32 {
    if (l.sun_c.x <= 0.0) { return 0.0; }
    let q = p.xz - l.sun.xz / l.sun.y * (16.0 - p.y) + vec2f(sin(ctx.t * 0.5), cos(ctx.t * 0.4)) * 0.15;
    let n = noise_value2(q * 2.6) * 0.6 + noise_value2(q * 6.1 + 5.0) * 0.4;
    return smoothstep(0.58, 0.78, n) * 0.55 + 0.12;
}

// paper lanterns along both fences, every 7 m
fn lantern_pos(i: i32, side: f32) -> vec3f {
    let z = -3.0 - f32(i) * 7.0 - select(0.0, 3.5, side > 0.0);
    return vec3f(path_x(z) + side * (FENCE - 0.15), 0.45, z);
}
fn lantern_light(p: vec3f, n: vec3f, l: Look, ctx: Ctx) -> vec3f {
    if (l.dusk <= 0.0) { return vec3f(0.0); }
    var c = vec3f(0.0);
    // the two nearest lanterns on each side
    let base = max(floor((-p.z - 3.0) / 7.0), 0.0);
    for (var s = 0; s < 2; s++) {
        let side = select(-1.0, 1.0, s == 1);
        for (var j = 0; j < 2; j++) {
            let lp = lantern_pos(i32(base) + j, side);
            let d = lp - p;
            let d2 = dot(d, d);
            let fl = light_flicker(ctx.t, u32(i32(base) + j) * 2u + u32(s), 0.5);
            c += col_kelvin(2300.0) * 2.2 * fl * saturate(dot(n, d * inverseSqrt(d2)) * 0.7 + 0.3) / (d2 + 0.4);
        }
    }
    return c;
}

// ------------------------------------------------------------ pieces

// brushwood fence: vertical bundles of twigs, ~1.1 m high
fn fence_col(p: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let u = p.z * 26.0 + noise_value2(vec2f(p.z * 2.0, p.y * 3.0)) * 3.0;
    let twig = noise_value2(vec2f(u, p.y * 1.2)) * 0.6 + noise_value2(vec2f(u * 3.0, p.y * 3.0)) * 0.4;
    var alb = mix(col_hex(0x2a1d12u), col_hex(0x7e6242u), smoothstep(0.2, 0.8, twig));
    // the lashing bands
    alb *= 1.0 - 0.5 * sstep(0.03, 0.0, abs(p.y - 0.35)) - 0.5 * sstep(0.03, 0.0, abs(p.y - 0.85));
    alb = mix(alb, alb * 0.5, l.wet * 0.5);
    let top = smoothstep(0.9, 1.1, p.y);
    return alb * (l.amb * (0.7 + 0.3 * top) * 1.3 + l.sun_c * dapple(p, l, ctx) * 0.35) + lantern_light(p, vec3f(-sign(p.x - path_x(p.z)), 0.0, 0.0), l, ctx) * alb;
}

fn path_col(p: vec3f, rd: vec3f, t: f32, l: Look, ctx: Ctx) -> vec3f {
    let dx = p.x - path_x(p.z);
    let g = noise_value2(p.xz * 6.0) * 0.5 + noise_value2(p.xz * 17.0) * 0.5;
    var alb = mix(col_hex(0x6d6250u), col_hex(0x9a8e76u), g);
    // worn centre, darker verges with moss
    alb = mix(alb, col_hex(0x4d5a34u), smoothstep(1.2, 1.8, abs(dx)) * 0.7);
    alb *= mix(1.0, 0.45, l.wet);
    var c = alb * (l.amb * 1.3 + l.sun_c * l.sun.y * dapple(p, l, ctx) * 0.9);
    c += lantern_light(p, vec3f(0.0, 1.0, 0.0), l, ctx) * alb;
    if (l.wet > 0.0) {
        // puddles mirror the bright gap of sky between the culms overhead
        let pud = smoothstep(0.55, 0.62, noise_value2(p.xz * 0.9 + 3.0)) * sstep(1.5, 0.6, abs(dx));
        let rip = drip_rings(p.xz, ctx.t);
        let r = reflect(rd, normalize(vec3f(rip.x, 1.0, rip.y)));
        let refl = l.sky_c * 0.6 * smoothstep(0.2, 0.9, r.y) * (0.3 + 0.7 * sstep(1.0, 0.0, abs(r.x / max(r.y, 0.1)) * 0.6));
        let fres = 0.05 + 0.6 * pow(1.0 - saturate(-rd.y), 5.0);
        c = mix(c, refl, saturate(pud * 0.8 + 0.2) * fres * 1.4 + pud * 0.3);
    }
    return c;
}

// raindrop rings on the puddles (normal xy perturbation)
fn drip_rings(xz: vec2f, t: f32) -> vec2f {
    let q = xz * 4.0;
    let c = floor(q);
    let h = hash_cell2(vec2i(c), 0x71fu);
    let ph = fract(t * (0.8 + 0.5 * h.z) + h.w);
    let d = q - c - h.xy;
    let r = length(d);
    let ring = sin((r - ph * 0.6) * 40.0) * exp(-sq((r - ph * 0.6) * 8.0)) * (1.0 - ph);
    return d / max(r, 1e-3) * ring * 0.3;
}

// leaves overhead: two layers of lace with the sky behind
fn canopy(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    var c = l.sky_c * (1.0 + 0.5 * pow(saturate(dot(rd, l.sun)), 8.0) * (1.0 - l.dusk));
    for (var k = 1; k >= 0; k--) {
        let hgt = 13.0 + 5.0 * f32(k);
        let t = (hgt - ro.y) / max(rd.y, 0.02);
        let q = (ro + rd * t).xz;
        let pc = path_x(q.y);
        // leaves crowd in from the sides; a slot of sky stays open above the path
        let slot = smoothstep(0.8, 3.5, abs(q.x - pc) + 1.5 * f32(k));
        let sway = vec2f(sin(ctx.t * 0.5 + q.y * 0.1), 0.0) * 0.2;
        let n = noise_value2((q + sway) * 1.1 + f32(k) * 7.0) * 0.6 + noise_value2((q + sway) * 2.6) * 0.4;
        let m = smoothstep(0.62 - 0.35 * slot, 0.7 - 0.35 * slot, n);
        let leaf = mix(col_hex(0x3e5a22u), col_hex(0x7a9a3eu), n) * (l.amb * 2.0 + l.sky_c * 0.35);
        c = mix(c, leaf, m);
    }
    return c;
}

// ------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(path_x(0.0), 1.6, 0.0);
    let cam = cam_look_at(ro, ro + vec3f(0.02, 0.26, -1.0), 0.0, 60.0);
    let rd = cam_ray(cam, p);
    let px_ang = ctx.px / cam.zoom;
    // background: the path / fences / grove floor / sky through the leaves
    var bg: vec3f;
    var t_bg = 1e6;
    let t_ground = select(1e6, -ro.y / rd.y, rd.y < 0.0);
    if (t_ground < 1e5) {
        let gp = ro + rd * t_ground;
        let dx = gp.x - path_x(gp.z);
        if (abs(dx) < FENCE) {
            bg = path_col(gp, rd, t_ground, l, ctx);
            t_bg = t_ground;
        } else {
            // floor of the grove, dim leaf litter
            bg = col_hex(0x4a3f2au) * l.amb * 0.9;
            t_bg = t_ground;
        }
    } else {
        bg = canopy(ro, rd, l, ctx);
        t_bg = 60.0;
    }
    // the fences: two sheets along the path, 1.1 m high (local plane)
    for (var s = 0; s < 2; s++) {
        let side = select(-1.0, 1.0, s == 1);
        // intersect x = path_x(z) + side*FENCE, refined twice for the curve
        var t = 1e6;
        var zz = -6.0;
        for (var it = 0; it < 3; it++) {
            let fx = path_x(zz) + side * FENCE;
            if (abs(rd.x) > 1e-4) { t = (fx - ro.x) / rd.x; }
            zz = ro.z + rd.z * max(t, 0.0);
        }
        if (t > 0.1 && t < t_bg) {
            let fp = ro + rd * t;
            // the top is ragged with twig ends
            if (fp.y < 1.08 + 0.05 * noise_value2(vec2f(fp.z * 7.0, 1.0)) + 0.06 * noise_value2(vec2f(fp.z * 40.0, 3.0))) {
                bg = fence_col(fp, l, ctx);
                t_bg = t;
            }
        }
    }
    // culm sheets, front to back
    var col = vec3f(0.0);
    var tr = 1.0;
    for (var k = 0; k < 40; k++) {
        let zk = -1.2 - f32(k) * 0.9;
        if (rd.z >= -1e-4) { break; }
        let t = (zk - ro.z) / rd.z;
        if (t > t_bg || tr < 0.02) { break; }
        let sp = ro + rd * t;
        if (sp.y < 0.0) { break; }
        let cv = sheet(sp, zk, k, t * px_ang, l, ctx);
        if (cv.x > 0.001) {
            var cc = culm_col(sp, cv, l, ctx);
            // green haze of the grove with depth
            let fogk = 1.0 - exp(-t * l.haze);
            cc = mix(cc, l.amb * 1.1 + l.sky_c * 0.07, fogk);
            col += tr * cv.x * cc;
            tr *= 1.0 - cv.x;
        }
    }
    let fogb = 1.0 - exp(-min(t_bg, 120.0) * l.haze);
    bg = mix(bg, l.amb * 1.1 + l.sky_c * 0.07, fogb * select(1.0, 0.4, t_bg >= 59.0));
    // a ray that runs out of sheets inside the grove looks into its dim
    // depths, not at the sky
    let far_z = -1.2 - 39.0 * 0.9;
    if (rd.z < 0.0) {
        let tf = (far_z - ro.z) / rd.z;
        let fp = ro + rd * tf;
        if (tf < t_bg && abs(fp.x - path_x(fp.z)) > GROVE && fp.y < 17.0) {
            bg = mix(l.amb * 0.55, l.amb * 1.2 + l.sky_c * 0.06, smoothstep(2.0, 16.0, fp.y));
        }
    }
    col += tr * bg;
    // lanterns themselves
    if (l.dusk > 0.0) {
        for (var i = 0; i < 6; i++) {
            for (var s = 0; s < 2; s++) {
                let side = select(-1.0, 1.0, s == 1);
                let lp = lantern_pos(i, side);
                let pr = cam_project(cam, lp);
                if (pr.z < 0.3) { continue; }
                let sz = cam.zoom / pr.z;
                let q = (p - pr.xy) / sz;
                let body = sstep(0.14, 0.12, abs(q.x)) * sstep(0.2, 0.18, abs(q.y));
                let fl = light_flicker(ctx.t, u32(i) * 2u + u32(s), 0.5);
                col = mix(col, col_kelvin(2500.0) * 3.5 * fl, body);
                col += col_kelvin(2300.0) * 0.06 * fl * exp(-dot(q, q) * 2.0);
            }
        }
    }
    if (l.wet > 0.0) {
        // fine, sparse streaks: rain everywhere would repaint every cell
        let rs = rain_streaks(p, ctx, 0.3, 2.2, 0.08, 2);
        col = mix(col, l.sky_c * 0.9, saturate(rs * 0.3));
    }
    return col * exp2(l.exposure);
}
