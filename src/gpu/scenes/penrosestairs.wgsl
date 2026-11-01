//! name: penrosestairs
//! title: Penrose Stairs
//! category: abstract
//! tags: impossible, staircase, escher, sandstone, courtyard, lantern
//! desc: sandstone stairs that climb forever around a courtyard, one lantern walking the loop
//! themes: noon, evening, moonlit, rain
//! params: steps, lantern, haze
//! uses: camera, raymarch, sdf, sky, noise, light, wet, stars, clouds, rain
//! cost: medium
//! fallback: orbits
//! credits: original; the construction follows Penrose & Penrose (1958)

// World units are metres, y up. The figure is built the way the drawing
// is: four real flights of stairs, two of eight steps and two of four, laid
// end to end and each climbing. Their last corner lands 4 treads along the
// vector (1, h, 1), and the camera is a parallel projection looking exactly
// along that vector, so the corner falls on the first step and the loop
// closes on screen. Nothing in the picture is a trick of shading: every
// riser and tread is solid masonry lit by the sky, and the walker with the
// lantern climbs real steps the whole way round.

const W: f32 = 0.4;             // tread
const R: f32 = 0.1;             // riser: h = 6 R / W makes the loop close
const H: f32 = 6.0 * R / W;
const WALK: f32 = 0.9;          // width of a flight
const DROP: f32 = 7.0;          // the masonry falls this far to the plaza
const PLAZA: f32 = -7.0;

struct Look {
    sun: vec3f,
    sun_c: vec3f,
    amb: vec3f,
    night: f32,
    wet: f32,
    lamp: f32,
    fogc: vec3f,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            l.sun = normalize(vec3f(0.9, 0.55, 0.7)); l.night = 0.0; l.wet = 0.0; l.lamp = 0.35; l.exposure = 0.2;
            l.sun_c = vec3f(1.5, 0.78, 0.42); l.amb = vec3f(0.18, 0.17, 0.22);
        }
        case 2u: {
            l.sun = normalize(vec3f(0.3, 1.0, 0.25)); l.night = 1.0; l.wet = 0.0; l.lamp = 1.0; l.exposure = 0.9;
            l.sun_c = vec3f(0.06, 0.08, 0.13); l.amb = vec3f(0.01, 0.013, 0.025);
        }
        case 3u: {
            l.sun = normalize(vec3f(0.15, 1.0, 0.2)); l.night = 0.0; l.wet = 1.0; l.lamp = 0.5; l.exposure = 0.45;
            l.sun_c = vec3f(0.3, 0.32, 0.36); l.amb = vec3f(0.22, 0.24, 0.27);
        }
        default: {
            l.sun = normalize(vec3f(0.25, 1.0, 0.2)); l.night = 0.0; l.wet = 0.0; l.lamp = 0.0; l.exposure = -0.1;
            l.sun_c = sky_sun_light(l.sun) * 1.1; l.amb = sky_ambient(l.sun) * 0.9;
        }
    }
    l.fogc = l.amb * 1.2 + l.sun_c * 0.08;
    return l;
}

// ------------------------------------------------------------------ the figure

// flight k: start corner, direction along it, across direction, steps
struct Flight { p: vec3f, dir: vec3f, across: vec3f, n: f32 }

fn flight(k: i32) -> Flight {
    switch (k) {
        case 0: { return Flight(vec3f(0.0, 0.0, 0.0), vec3f(1.0, 0.0, 0.0), vec3f(0.0, 0.0, 1.0), 8.0); }
        case 1: { return Flight(vec3f(8.0 * W, 8.0 * R, 0.0), vec3f(0.0, 0.0, 1.0), vec3f(1.0, 0.0, 0.0), 8.0); }
        case 2: { return Flight(vec3f(8.0 * W, 16.0 * R, 8.0 * W), vec3f(-1.0, 0.0, 0.0), vec3f(0.0, 0.0, 1.0), 4.0); }
        default: { return Flight(vec3f(4.0 * W, 20.0 * R, 8.0 * W), vec3f(0.0, 0.0, -1.0), vec3f(1.0, 0.0, 0.0), 4.0); }
    }
}

// one flight as solid masonry: the union of boxes, box i rising to step i's
// tread and running from step i to the flight's end, all the way down
fn flight_sdf(p: vec3f, f: Flight) -> f32 {
    let q = p - f.p;
    let u = dot(q, f.dir);
    let v = dot(q, f.across);
    let len = f.n * W;
    // the walkway is wider than the path by half the width on each side,
    // and the corners are shared, so each flight overruns its start
    let i = clamp(floor(u / W), 0.0, f.n - 1.0);
    var d = 1e5;
    for (var k = -1; k <= 1; k++) {
        let j = clamp(i + f32(k), 0.0, f.n - 1.0);
        let top = (j + 1.0) * R;
        let u0 = j * W - select(0.0, WALK * 0.5, j < 0.5);
        let cu = 0.5 * (u0 + len + WALK * 0.5);
        let hu = 0.5 * (len + WALK * 0.5 - u0);
        let b = vec3f(u - cu, q.y - (top - DROP) * 0.5, v);
        let hb = vec3f(hu, (top + DROP) * 0.5, WALK * 0.5);
        let dd = abs(b) - hb;
        d = min(d, min(max(dd.x, max(dd.y, dd.z)), 0.0) + length(max(dd, vec3f(0.0))));
    }
    return d;
}

// where the walker is: (position of the feet, flight index), climbing one
// step a second, around and around
fn walker_at(t: f32) -> vec4f {
    let total = 24.0;
    let s = fmod_pos(t * 0.85, total);
    var k = 0;
    var base = 0.0;
    if (s >= 8.0) { k = 1; base = 8.0; }
    if (s >= 16.0) { k = 2; base = 16.0; }
    if (s >= 20.0) { k = 3; base = 20.0; }
    let f = flight(k);
    let u = s - base;
    let step_i = floor(u);
    let frac_ = u - step_i;
    // a step: the foot rises on the first half and glides on the second
    let rise = smoothstep(0.0, 0.5, frac_);
    let pos = f.p + f.dir * ((u + 0.5) * W) + vec3f(0.0, (step_i + 1.0) * R + rise * R * 0.0, 0.0) + f.across * 0.2;
    let y = f.p.y + (step_i + 1.0) * R;
    return vec4f(pos.x, y + 0.02 * abs(sin(frac_ * PI)), pos.z, f32(k));
}

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    // the plaza far below
    var d = p.y - PLAZA;
    var m = 1.0;
    for (var k = 0; k < 4; k++) {
        let dk = flight_sdf(p, flight(k));
        if (dk < d) { d = dk; m = 2.0; }
    }
    // the walker: a cloaked figure, and the lantern on its arm
    let wk = walker_at(ctx.t);
    let feet = wk.xyz;
    // a cloak that flares to the hem, the hood a little forward
    let f0 = flight(i32(wk.w));
    let body = sdf_cone_y(p - feet - vec3f(0.0, 0.34, 0.0), 0.34, 0.15, 0.07);
    let head = length(p - feet - vec3f(0.0, 0.76, 0.0) - f0.dir * 0.03) - 0.075;
    let fig = op_smin(body, head, 0.06);
    if (fig < d) { d = fig; m = 3.0; }
    let f = flight(i32(wk.w));
    let lamp = length(p - feet - vec3f(0.0, 0.45, 0.0) - f.dir * 0.28) - 0.06;
    if (lamp < d) { d = lamp; m = 4.0; }
    return vec2f(d, m);
}

fn lamp_pos(ctx: Ctx) -> vec3f {
    let wk = walker_at(ctx.t);
    let f = flight(i32(wk.w));
    return wk.xyz + vec3f(0.0, 0.45, 0.0) + f.dir * 0.28;
}

// ------------------------------------------------------------------ shading

fn stone(p: vec3f, n: vec3f, l: Look) -> vec3f {
    // sandstone blocks: courses of 0.6 x 0.26 m with recessed joints,
    // iron staining, moss where the rain gathers
    let along = select(p.x, p.z, abs(n.x) > 0.5);
    let course = floor(p.y / 0.26);
    let jx = abs(fract((along + course * 0.3) / 0.6) - 0.5) * 0.6;
    let jy = abs(fract(p.y / 0.26) - 0.5) * 0.26;
    let joint = smoothstep(0.0, 0.012, min(0.3 - jx, 0.13 - jy) - 0.006);
    let block = hash_cell2(vec2i(i32(floor((along + course * 0.3) / 0.6)), i32(course)), 0x5a9du);
    var c = mix(col_hex(0xc9a877u), col_hex(0xa8865cu), block.x * 0.8);
    c *= 0.85 + 0.3 * noise_fbm2(vec2f(along, p.y) * 7.0, 3);
    // tread tops: worn paler, the joints across the tread
    if (n.y > 0.7) {
        c = mix(c, col_hex(0xd6bb90u), 0.4);
        c *= 0.9 + 0.2 * noise_value2(p.xz * 14.0);
    }
    c = mix(c, col_hex(0x8a5a30u), smoothstep(0.6, 0.85, noise_fbm2(vec2f(along * 1.3, p.y * 2.0) + 11.0, 3)) * 0.35);
    c = mix(c, col_hex(0x4d6a2au), (1.0 - joint) * 0.5 + smoothstep(0.65, 0.9, noise_value3(p * 5.0)) * 0.25 * (1.0 - n.y));
    return c * mix(0.45, 1.0, joint);
}

fn shade(p: vec3f, rd: vec3f, t: f32, mat: f32, l: Look, ctx: Ctx) -> vec3f {
    let n = rm_normal(p, t, ctx);
    var alb: vec3f;
    var rough = 0.8;
    if (mat < 1.5) {
        // the plaza: big flagstones, lost in haze
        let g = abs(fract(p.xz / 1.4) - 0.5) * 1.4;
        let joint = smoothstep(0.0, 0.03, min(g.x, g.y) - 0.02);
        alb = col_hex(0x6a6358u) * (0.8 + 0.3 * noise_value2(p.xz * 2.0)) * mix(0.6, 1.0, joint);
    } else if (mat < 2.5) {
        alb = stone(p, n, l);
    } else if (mat < 3.5) {
        // a wool cloak, worn
        alb = col_hex(0x4a2e22u) * (0.8 + 0.3 * noise_value3(p * 25.0));
        rough = 0.95;
    } else {
        alb = vec3f(1.0, 0.8, 0.5);
    }
    // rain: dark wet stone, puddles standing on the treads
    var wet = 0.0;
    if (l.wet > 0.5) {
        wet = 0.75;
        if (n.y > 0.8 && mat > 1.5) { wet = max(wet, wet_puddles(p.xz * 3.0, 0.35, ctx)); }
        let ws = wet_surface(alb, rough, wet);
        alb = ws.rgb;
        rough = ws.a;
    }
    let dif = saturate(dot(n, l.sun));
    var sh = 1.0;
    if (dif > 0.0) {
        sh = rm_shadow(p + n * (0.01 + t * ctx.px * 1.5), l.sun, 0.02, 12.0, 12.0, ctx);
    }
    let occ = rm_ao(p, n, ctx);
    var c = alb * (l.sun_c * dif * sh + l.amb * occ * (0.65 + 0.35 * saturate(n.y)));
    c += l.sun_c * light_ggx(n, -rd, l.sun, rough) * sh * (0.15 + wet * 0.6);
    // the lantern: a warm point light with a slow flicker
    if (l.lamp > 0.0) {
        let lp = lamp_pos(ctx);
        let flick = light_flicker(ctx.t, 7u, 1.0);
        let lc = col_kelvin(2100.0) * l.lamp * (0.35 + ctx.params.y * 0.5) * flick;
        // its own shadow: the walker's body stands between it and the steps
        let to = lp - p;
        let dist = length(to);
        let ld = to / max(dist, 1e-4);
        var lsh = 1.0;
        if (dist < 4.0) { lsh = rm_shadow(p + n * 0.01, ld, 0.02, dist - 0.08, 16.0, ctx); }
        c += alb * light_point(p, n, lp, lc, 0.12) * lsh;
        c += lc * light_ggx(n, -rd, ld, rough) * lsh / (1.0 + dist * dist) * (0.2 + wet);
    }
    if (mat > 3.5) {
        // the lantern glass glows
        c += col_kelvin(2100.0) * (0.5 + 4.0 * l.lamp * light_flicker(ctx.t, 7u, 1.0));
    }
    return c;
}

fn sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    if (l.night > 0.5) {
        var c = sky_night(rd) * 1.3;
        c += star_field(rd, 0.7, ctx) * smoothstep(0.0, 0.2, rd.y);
        c += sky_moon(rd, l.sun, 0.5, 1.2) * 0.8;
        return c;
    }
    if (l.wet > 0.5) {
        // overcast, moving
        let hp = rd.xz / (max(rd.y, 0.05) + 0.02);
        let cs = cloud_sheet(hp * 0.3 + vec2f(4.0, 1.0), 0.92, ctx);
        return mix(l.amb * 1.6, l.amb * (1.0 + cs.y), cs.x);
    }
    return sky_atmosphere_haze(rd, l.sun, 1.6 + ctx.params.z * 2.0) * 0.9 + sky_sun_disk(rd, l.sun, 0.55) * 0.3;
}

// ------------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    // the parallel projection along (1, h, 1): rt and up span the picture
    let v = normalize(vec3f(1.0, H, 1.0));
    let rt = normalize(cross(vec3f(0.0, 1.0, 0.0), v));
    let up = cross(v, rt);
    let scale = 3.4;
    // centre the figure: its visible middle, shifted up a little
    let centre = vec3f(4.5 * W, 10.0 * R, 4.5 * W);
    let drift = cam_depth_offset(ctx) * 0.6;
    let ro = centre + v * 30.0 + rt * (p.x * scale + drift.x) + up * (p.y * scale + drift.y);
    let rd = -v;
    // march: parallel rays, so the hit tolerance is one pixel of the picture
    let eps = max(0.0008, ctx.px * scale * 0.4);
    var t = 0.0;
    var hit = vec2f(-1.0, -1.0);
    let n = steps(90.0, ctx);
    for (var i = 0; i < 180; i++) {
        if (i >= n) { break; }
        let d = map(ro + rd * t, ctx);
        if (d.x < eps) { hit = vec2f(t, d.y); break; }
        t += d.x * 0.9;
        if (t > 80.0) { break; }
    }
    var col: vec3f;
    if (hit.x > 0.0) {
        let hp = ro + rd * hit.x;
        col = shade(hp, rd, 1.0, hit.y, l, ctx);
        // haze fills the drop to the plaza
        let depth = saturate((centre.y - hp.y) / DROP);
        col = mix(col, l.fogc, depth * depth * 0.7);
    } else {
        col = sky(rd, l, ctx);
    }
    if (l.wet > 0.5) {
        // rain, fine and slanting, gated by the sky's light
        col += l.amb * 1.5 * rain_streaks(p, ctx, 0.5, 1.6, 0.12, 2) * 0.25;
    }
    col *= 1.0 + 0.06 * ctx.focus_pulse * ctx.reaction * exp(-length(p - ctx.focus) * 3.0);
    return col * exp2(l.exposure);
}
