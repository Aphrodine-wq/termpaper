//! name: porch
//! title: Summer Storm Porch
//! category: cozy
//! tags: porch, south, storm, rain, fireflies, lightning, dusk
//! desc: a screened Southern porch at dusk: rain off the eaves, a rocking chair, fireflies after
//! themes: storm, fireflies, night
//! uses: sdf, rain, stars, noise
//! cost: light
//! fallback: fireflies
//! credits: original

// Inside a screened porch in the American South, looking out over the lawn to
// a line of live oaks hung with Spanish moss. White posts frame the view
// under a haint-blue beadboard ceiling; a lantern on the left post draws the
// moths. In the storm, water sheets off the eave and lightning flickers far
// off; once it has passed, fireflies rise out of the wet grass.

const HOR: f32 = 0.0;          // horizon (base of the tree line)
const POST: f32 = 1.24;        // post spacing
const RAIL: f32 = -0.17;       // top of the railing
const DECK: f32 = -0.4;        // porch floor line at the railing
const LAMP: vec2f = vec2f(-0.62, 0.14);

struct Look {
    mode: u32,        // 0 storm, 1 fireflies (after the storm), 2 night
    sky_top: vec3f,
    sky_low: vec3f,
    trees: vec3f,
    lawn: vec3f,
    lamp: f32,
    haze: f32,        // rain haze over the yard
    exposure: f32,
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            return Look(1u, col_hex(0x243456u) * 0.35, col_hex(0xd89a7au) * 0.55, col_hex(0x0a0e0cu) * 0.25,
                        col_hex(0x1e3018u) * 0.12, 1.0, 0.12, 0.3);
        }
        case 2u: {
            return Look(2u, col_hex(0x060a18u) * 0.35, col_hex(0x16203au) * 0.4, col_hex(0x03050au) * 0.3,
                        col_hex(0x0c160eu) * 0.1, 1.0, 0.0, 0.4);
        }
        default: {
            return Look(0u, col_hex(0x1c2230u) * 0.4, col_hex(0x4a5260u) * 0.45, col_hex(0x10141au) * 0.5,
                        col_hex(0x18241cu) * 0.14, 1.0, 0.55, 0.3);
        }
    }
}

// distant lightning: (sky flash, bolt visible)
fn lightning(t: f32) -> vec2f {
    let ev = hash_event(t, 11.0, 0x5708u);
    if (ev.x > 0.45) { return vec2f(0.0); }
    let lt = ev.y * 11.0 - 2.0 - ev.x * 10.0;
    if (lt < 0.0 || lt > 0.9) { return vec2f(0.0); }
    let f = exp(-lt * 10.0) + 0.7 * exp(-abs(lt - 0.22) * 25.0) + 0.3 * exp(-abs(lt - 0.45) * 30.0);
    return vec2f(f, step(lt, 0.3));
}

// light from the porch lantern at a pseudo-3D point
fn lantern(pos: vec3f, n: vec3f, l: Look) -> vec3f {
    let lp = vec3f(LAMP.x + 0.03, LAMP.y, -0.12);
    let d = lp - pos;
    let dist2 = dot(d, d);
    return col_kelvin(2500.0) * l.lamp * (max(dot(n, d * inverseSqrt(dist2)), 0.0) * 0.03 / (dist2 + 0.01) + 0.01 / (1.0 + dist2 * 4.0));
}

// ------------------------------------------------------------------ the yard
// live oaks along the far edge of the lawn: (crown top, crown underside)
fn oaks(x: f32) -> vec2f {
    let cs = 0.26;
    let c = floor(x / cs);
    var top = 0.02;
    var under = 1.0;
    for (var k = -1; k <= 1; k++) {
        let ck = c + f32(k);
        let h = hash_cell2(vec2i(i32(ck), 5), 0x0a6u);
        let cx = (ck + 0.5 + (h.x - 0.5) * 0.5) * cs;
        let w = cs * (0.7 + 0.5 * h.y);
        let dx = (x - cx) / w;
        let dome = sqrt(saturate(1.0 - dx * dx));
        let lump = 0.025 * noise_value2(vec2f(x * 22.0, h.w * 9.0)) + 0.01 * noise_value2(vec2f(x * 60.0, h.z * 7.0));
        let ht = (0.12 + 0.1 * h.z) * pow(dome, 0.45) + lump * dome;
        if (ht > top) { top = ht; }
        let low = 0.035 + 0.02 * noise_value2(vec2f(x * 18.0, h.x * 5.0));
        under = min(under, low + (1.0 - dome) * 0.12);
    }
    return vec2f(top, under);
}

fn yard(p: vec2f, l: Look, ctx: Ctx, flash: vec2f) -> vec3f {
    let t = ctx.t;
    let e = p.y - HOR;
    var c = mix(l.sky_low, l.sky_top, smoothstep(0.0, 0.45, e));
    // storm clouds / clearing clouds
    let cn = noise_fbm2(vec2f(p.x * 2.2 + t * 0.004, e * 5.0), 5);
    if (l.mode == 0u) {
        c *= 0.7 + 0.5 * cn;
        c += col_hex(0xc8d0ff) * flash.x * (0.4 + 0.8 * cn) * 0.6;
        // the bolt itself, far off to the right
        if (flash.y > 0.0) {
            let bx = 0.35 + 0.05 * noise_sfbm2(vec2f(e * 12.0, 3.0), 4) + 0.1 * e;
            let bolt = exp(-sq((p.x - bx) / 0.0025)) * step(0.14, e) * step(e, 0.45);
            c += vec3f(1.2, 1.25, 1.5) * bolt * flash.x * 2.0;
        }
    } else if (l.mode == 1u) {
        // the storm moving off: broken clouds lit pink from below
        let cl = smoothstep(0.45, 0.75, cn);
        c = mix(c, mix(col_hex(0x3a3a50u) * 0.3, col_hex(0xe8a088u) * 0.5, smoothstep(0.3, 0.0, e)), cl * 0.8);
        let sd = normalize(vec3f(p.x, e + 0.1, 1.0));
        c += star_field(sd, 0.3, ctx) * smoothstep(0.25, 0.45, e) * (1.0 - cl);
    } else {
        let sd = normalize(vec3f(p.x, e + 0.1, 1.0));
        c += star_field(sd, 0.25, ctx) * smoothstep(0.1, 0.3, e) * 0.7;
        let md = length(p - vec2f(0.32, 0.36));
        c += vec3f(1.0, 0.96, 0.88) * (smoothstep(0.022, 0.018, md) * 1.2 + 0.05 * exp(-md * 10.0));
    }
    // tree line: broad oak crowns on dark trunks, grey moss hanging in strands
    let oak = oaks(p.x);
    let woods = 0.035 + 0.01 * noise_value2(vec2f(p.x * 20.0, 2.0));
    if (e < woods) { c = mix(c, l.trees * 1.4, 0.8); }
    let strands = smoothstep(0.6, 0.85, noise_value2(vec2f(p.x * 140.0, 0.5))) * smoothstep(oak.y - 0.05, oak.y, e) * step(e, oak.y);
    let tq = fract(p.x / 0.26 + 0.3 + 0.1 * noise_value2(vec2f(floor(p.x / 0.26 + 0.3), 3.0)));
    let tw = 0.012 + 0.012 * smoothstep(0.02, 0.06, e);  // trunks flare a little into limbs
    let trunk = smoothstep(tw, tw * 0.6, abs(tq - 0.5) * 0.26) * step(e, oak.y + 0.01);
    if ((e < oak.x && e > oak.y) || trunk > 0.5) {
        var tc = l.trees;
        if (l.mode == 0u) { tc += col_hex(0x8890b0u) * flash.x * 0.05; }
        c = tc;
    }
    c = mix(c, l.trees * 2.0 + l.sky_low * 0.1, strands * 0.5);
    // the lawn, wet, lit near the porch by the lantern
    if (e < 0.0) {
        let z = 0.25 / max(-e, 0.004);
        var g = l.lawn * (0.8 + 0.3 * noise_value2(vec2f(p.x * z * 4.0, z * 0.5)));
        g += lantern(vec3f(p.x, -0.45, -0.2 - 1.0 / z), vec3f(0.0, 1.0, 0.0), l) * 0.12 * smoothstep(0.2, 0.9, 1.0 / z);
        if (l.mode == 0u) { g += col_hex(0xa8b0d0u) * flash.x * 0.03; }
        c = g;
    }
    // haze of rain over the yard
    c = mix(c, l.sky_low * 0.8, l.haze * smoothstep(-0.3, 0.15, e) * 0.7);
    if (l.mode == 0u) {
        let r = rain_streaks(p, ctx, 0.6, 2.2, 0.05, 3);
        c += (l.sky_low * 0.35 + vec3f(0.01)) * r;
    }
    // fireflies over the grass and against the trees
    if (l.mode != 0u) {
        let amount = select(0.35, 1.0, l.mode == 1u);
        let q = vec2f(p.x * 11.0, (p.y + 0.1) * 11.0);
        let cq = floor(q);
        var ff = 0.0;
        for (var n = 0; n < 6; n++) {
            let cell = vec2i(cq) + vec2i(n % 3 - 1, n / 3 - 1);
            let h = hash_cell2(cell, 0xf1f1u);
            if (h.w > 0.35 * amount || cell.y < -2 || cell.y > 2) { continue; }
            let life = 9.0 + 5.0 * h.z;
            let ph = fract(t / life + h.x);
            let pos = vec2f(cell) + vec2f(0.5 + 0.35 * sin(t * 0.3 + h.y * 20.0), 0.2 + ph * 1.4 + 0.1 * sin(t * 0.5 + h.x * 9.0));
            let d = length((q - pos) / 11.0);
            // a slow flash every few seconds
            let blink = smoothstep(0.0, 0.15, fract(t / (2.5 + 2.5 * h.y) + h.z)) * smoothstep(0.45, 0.2, fract(t / (2.5 + 2.5 * h.y) + h.z));
            let r = max(ctx.px * 0.8, 0.0025);
            ff += blink * (exp(-sq(d / r)) * 2.0 + exp(-d / 0.012) * 0.12) * smoothstep(0.0, 0.1, ph) * smoothstep(1.0, 0.8, ph);
        }
        c += vec3f(0.75, 1.0, 0.25) * ff * 0.8;
    }
    return c;
}

// ------------------------------------------------------------------ the porch
// rocking chair outline, local coords (seat front at origin), rocking angle a
fn chair_d(q0: vec2f, a: f32) -> f32 {
    // rock about the rockers' contact point
    let piv = vec2f(0.0, -0.23);
    let q = rot2(a) * (q0 - piv) + piv;
    // rockers: a shallow arc under everything
    let rk = abs(length(q - vec2f(0.05, 0.6)) - 0.83) - 0.006;
    var d = max(rk, abs(q.x - 0.05) - 0.22);
    // front and back legs
    d = min(d, sdf2_box(q - vec2f(-0.1, -0.12), vec2f(0.007, 0.1)));
    d = min(d, sdf2_box(q - vec2f(0.17, -0.12), vec2f(0.007, 0.1)));
    // seat
    d = min(d, sdf2_box(q - vec2f(0.035, -0.01), vec2f(0.15, 0.012)));
    // back: seen from the side it is a leaning panel of slats under a crest rail
    let lean = q.y * 0.2;
    let bx = q.x - 0.19 - lean;
    let panel = max(abs(bx) - 0.028, max(q.y - 0.3, 0.02 - q.y));
    d = min(d, panel);
    d = min(d, sdf2_round_box(q - vec2f(0.19 + 0.3 * 0.2, 0.315), vec2f(0.042, 0.016), 0.008));
    // woven seat
    d = min(d, sdf2_box(q - vec2f(0.035, 0.0), vec2f(0.15, 0.02)));
    // arm and its support
    d = min(d, sdf2_round_box(q - vec2f(0.03, 0.09), vec2f(0.14, 0.008), 0.006));
    d = min(d, sdf2_box(q - vec2f(-0.08, 0.04), vec2f(0.006, 0.05)));
    return d;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let aa = ctx.px;
    var flash = vec2f(0.0);
    if (l.mode == 0u) { flash = lightning(t); }

    // ---- through the screen: the yard, the eave's curtain of water
    var col = yard(p, l, ctx, flash);
    if (l.mode == 0u) {
        // water sheets off the eave in drip lines just beyond the screen
        let dx = p.x * 60.0;
        let cx = floor(dx);
        let h = hash_cell2(vec2i(i32(cx), 3), 0xd419u);
        let lx = abs(fract(dx) - 0.5 - (h.x - 0.5) * 0.4);
        let flow = fract(p.y * (3.0 + h.y) + t * (2.8 + h.z));
        let stream = smoothstep(0.18, 0.02, lx) * (0.4 + 0.6 * smoothstep(0.2, 0.9, flow)) * step(0.55, h.w);
        let near_lamp = exp(-length(p - LAMP) * 3.0);
        col += (l.sky_low * 0.25 + col_kelvin(2500.0) * near_lamp * 0.35 + vec3f(0.6, 0.65, 0.8) * flash.x * 0.2) * stream * 0.6 * step(RAIL, p.y);
    }
    // the screen mesh: a faint veil
    col = mix(col, vec3f(0.01) + lantern(vec3f(p, -0.02), vec3f(0.0, 0.0, -1.0), l) * 0.2, 0.18);

    // ---- railing: top rail, balusters, bottom rail
    if (p.y < RAIL + 0.012 && p.y > DECK) {
        let bx = abs(fract(p.x / 0.055) - 0.5) * 0.055;
        let bal = bx - 0.009;
        let rail = abs(p.y - RAIL) - 0.012;
        let bottom = abs(p.y - (DECK + 0.02)) - 0.01;
        let d = min(min(bal, rail), bottom);
        let n = normalize(vec3f(0.0, select(0.0, 0.8, rail < 0.0), -1.0));
        let wc = vec3f(0.78, 0.76, 0.72) * (lantern(vec3f(p, -0.06), n, l) * 1.2 + l.sky_low * 0.08 + vec3f(0.6, 0.65, 0.8) * flash.x * 0.05);
        col = mix(col, wc, aa_fill(d, ctx));
    }
    // ---- the floor: painted boards receding
    if (p.y < DECK) {
        let vp = vec2f(0.0, 0.05);
        let z = 0.12 / max(vp.y - p.y, 0.01);
        let wx = (p.x - vp.x) * z;
        let seam = smoothstep(0.03, 0.0, abs(fract(wx / 0.09) - 0.5) - 0.47) * saturate(1.0 - aa * z * 25.0);
        var fc = col_hex(0x6a7074u) * 0.5 * (1.0 - 0.4 * seam) * (lantern(vec3f(p.x, DECK, -0.1 - (z - 0.3) * 0.3), vec3f(0.0, 1.0, 0.0), l) * 1.3 + l.sky_low * 0.05);
        // damp boards near the screen catch the sky
        fc += l.sky_low * 0.06 * smoothstep(DECK - 0.05, DECK, p.y) * select(0.5, 1.0, l.mode == 0u);
        col = fc;
    }
    // ---- header beam and the haint-blue beadboard ceiling
    if (p.y > 0.36) {
        if (p.y < 0.42) {
            let n = normalize(vec3f(0.0, select(-0.8, 0.0, p.y > 0.37), -1.0));
            col = vec3f(0.8, 0.78, 0.74) * (lantern(vec3f(p, -0.08), n, l) * 1.1 + l.sky_low * 0.04);
        } else {
            let z = 0.1 / max(p.y - 0.36, 0.01);
            let wx = p.x * z;
            let bead = smoothstep(0.03, 0.0, abs(fract(wx / 0.06) - 0.5) - 0.46) * saturate(1.0 - aa * z * 25.0);
            col = col_hex(0x9ccfd0u) * 0.5 * (1.0 - 0.3 * bead) * (lantern(vec3f(p.x, 0.45, -0.1 - z * 0.2), vec3f(0.0, -1.0, 0.0), l) * 0.8 + l.sky_low * 0.25 + vec3f(0.004, 0.008, 0.01));
        }
    }
    // ---- posts: square, painted white, repeated along the porch
    let px = p.x - POST * round((p.x - 0.62) / POST) - 0.62;
    let pd = abs(px) - 0.028;
    if (pd < aa && p.y > DECK - 0.02) {
        let side = smoothstep(-0.028, 0.028, px);
        let pc = vec3f(0.82, 0.8, 0.76) * (lantern(vec3f(p, -0.05), normalize(vec3f(mix(-1.0, 1.0, side), 0.0, -1.0)), l) * 1.1 + l.sky_low * 0.06);
        col = mix(col, pc, aa_fill(pd, ctx));
    }
    let px2 = p.x - POST * round((p.x + 0.62) / POST) + 0.62;
    let pd2 = abs(px2) - 0.028;
    if (pd2 < aa && p.y > DECK - 0.02) {
        let side = smoothstep(-0.028, 0.028, px2);
        let lit = lantern(vec3f(p, -0.05), normalize(vec3f(mix(-1.0, 1.0, side), 0.0, -1.0)), l);
        let pc = vec3f(0.82, 0.8, 0.76) * (lit * 1.1 + l.sky_low * 0.06);
        col = mix(col, pc, aa_fill(pd2, ctx));
    }

    // ---- the lantern on the left post
    {
        let q = p - LAMP - vec2f(0.045, 0.0);
        let cage = sdf2_round_box(q, vec2f(0.026, 0.045), 0.006);
        let cap = sdf2_box(q - vec2f(0.0, 0.055), vec2f(0.032, 0.008));
        let arm = sdf2_box(p - vec2f(LAMP.x + 0.02, LAMP.y + 0.03), vec2f(0.02, 0.004));
        let frame = min(min(abs(cage) - 0.004, cap), min(arm, max(abs(q.x) - 0.003, abs(q.y) - 0.045)));
        let glass = smoothstep(0.002, -0.002, cage);
        col = mix(col, col_kelvin(2400.0) * l.lamp * (2.2 + 1.0 * exp(-dot(q, q) * 900.0)), glass);
        col = mix(col, vec3f(0.01), aa_fill(frame, ctx));
        // glow in the damp air
        let gd = length(q);
        col += col_kelvin(2500.0) * l.lamp * (0.12 * exp(-gd * 18.0) + 0.04 * exp(-gd * 5.0)) * (1.0 + l.haze);
        // moths circling it
        for (var k = 0; k < 5; k++) {
            let fk = f32(k);
            let h = hash_cell2(vec2i(k, 1), 0x3074u);
            let sp = 1.5 + 1.5 * h.x;
            let a = t * sp + h.y * TAU;
            let r = 0.05 + 0.03 * sin(t * (0.7 + h.z) + fk);
            let mp = vec2f(cos(a) * r * 1.3, sin(a * 1.3) * r * 0.8 + 0.01 * sin(t * 3.0 + fk));
            let md = length(q - mp);
            let mr = max(aa * 0.7, 0.003);
            col += vec3f(0.9, 0.85, 0.7) * l.lamp * 0.5 * exp(-sq(md / mr));
        }
    }

    // ---- the rocking chair, right foreground, rocking slowly
    {
        let a = 0.035 * sin(t * 1.4);
        let sc = 1.25;
        let q = (p - vec2f(0.5, -0.2)) / sc;
        let d = chair_d(q, a) * sc;
        if (d < aa) {
            // a white-painted rocker, lit by the lantern from across the porch,
            // its far edges catching the grey light from the yard
            let rim = smoothstep(-0.006, 0.0, d);
            var wc = vec3f(0.78, 0.76, 0.72) * (lantern(vec3f(p, -0.3), normalize(vec3f(-0.8, 0.2, -0.6)), l) * 1.1 + l.sky_low * 0.12 + vec3f(0.004)) + l.sky_low * 0.1 * rim;
            // slat gaps in the back, a cane weave on the seat
            let qq = (p - vec2f(0.5, -0.2)) / sc;
            let slat = step(0.5, fract((qq.y - 0.02) / 0.045)) * step(0.025, qq.y) * step(qq.y, 0.29) * step(0.12, qq.x);
            wc *= 1.0 - 0.45 * slat;
            let cane = step(abs(qq.y), 0.02) * step(abs(qq.x - 0.035), 0.15) * (0.8 + 0.2 * step(0.5, fract(qq.x * 90.0)));
            wc = mix(wc, wc * vec3f(0.85, 0.72, 0.5), cane * 0.8);
            col = mix(col, wc, aa_fill(d, ctx));
        }
    }
    return col * exp2(l.exposure);
}
