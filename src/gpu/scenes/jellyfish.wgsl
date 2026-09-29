//! name: jellyfish
//! title: Moon Jelly Gallery
//! category: coast
//! tags: aquarium, jellyfish, monterey, underwater, calm
//! desc: translucent moon jellies pulse and drift through the blue of an aquarium gallery tank
//! themes: blue, sunset, deep
//! uses: camera, noise
//! cost: medium
//! fallback: aquarium
//! credits: original

// World units are metres; the camera looks into a jelly tank from the glass.
// Every jelly is analytic: the bell is the difference of two clipped
// ellipsoids (the thin living tissue glows by the length of the path the
// view ray takes through it, which is what makes the rims shine), the four
// gonad rings sit on a plane inside it, the marginal tentacles are a fine
// skirt of lines and the oral arms a frilled column below. Bells pulse with
// a quick contraction and a slow relaxation, and drift on closed-form paths.

const NJ: i32 = 9;

struct Look {
    top: vec3f,      // backdrop at the top of the tank
    bot: vec3f,      // backdrop at the bottom
    tissue: vec3f,   // light scattered by the bell
    gonad: vec3f,
    rim: f32,        // rim light emphasis (deep theme)
    glow: f32,       // body glow
    fog: vec3f,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    switch (theme) {
        case 1u: {
            // amber backlight, as in the sea-nettle kreisel
            l.top = vec3f(0.42, 0.095, 0.011);
            l.bot = vec3f(0.02, 0.004, 0.001);
            l.tissue = vec3f(1.0, 0.72, 0.42);
            l.gonad = vec3f(1.0, 0.45, 0.15);
            l.rim = 2.0; l.glow = 0.75;
            l.fog = vec3f(0.3, 0.08, 0.015);
            l.exposure = -0.5;
        }
        case 2u: {
            l.top = vec3f(0.004, 0.009, 0.02);
            l.bot = vec3f(0.0005, 0.001, 0.003);
            l.tissue = vec3f(0.55, 0.72, 0.95);
            l.gonad = vec3f(0.45, 0.3, 0.6);
            l.rim = 3.2; l.glow = 0.07;
            l.fog = vec3f(0.002, 0.004, 0.01);
            l.exposure = 0.5;
        }
        default: {
            l.top = vec3f(0.03, 0.2, 0.62);
            l.bot = vec3f(0.0, 0.025, 0.12);
            l.tissue = vec3f(0.72, 0.86, 1.0);
            l.gonad = vec3f(0.85, 0.5, 0.85);
            l.rim = 1.0; l.glow = 0.55;
            l.fog = vec3f(0.01, 0.08, 0.3);
            l.exposure = -0.1;
        }
    }
    return l;
}

fn backdrop(p: vec2f, l: Look, ctx: Ctx) -> vec3f {
    let y = saturate(p.y * 0.9 + 0.55);
    var c = mix(l.bot, l.top, y * y * (3.0 - 2.0 * y));
    // the soft pool of light the tank is lit by, from above
    c += l.top * 0.35 * exp(-sq(p.x * 1.2)) * smoothstep(-0.1, 0.6, p.y);
    // faint, static shafts from the light at the top of the tank
    let sh = noise_value2(vec2f(p.x * 7.0 + p.y * 1.5, 3.0));
    c *= 0.92 + 0.14 * sh * smoothstep(-0.3, 0.5, p.y);
    // the dark gallery closes in around the lit tank
    c *= 1.0 - 0.45 * smoothstep(0.35, 1.3, length(p * vec2f(0.75, 1.0)));
    return c;
}

struct Jelly { c: vec3f, up: vec3f, r: f32, h: f32, pulse: f32, id: f32 }

fn jelly_get(i: i32, ctx: Ctx) -> Jelly {
    let fi = f32(i);
    let hh = hash_pcg3(vec3u(u32(i), 17u, 5u));
    let hx = hash_unorm(hh.x);
    let hy = hash_unorm(hh.y);
    let hz = hash_unorm(hh.z);
    // spread through the tank at several depths; one hero close to the glass
    var base = vec3f((hx - 0.5) * 3.6, (hy - 0.5) * 2.0, -2.2 - 4.0 * hz);
    var r = 0.13 + 0.07 * hash_f(u32(i) * 7u + 1u);
    if (i == 0) { base = vec3f(-0.12, 0.02, -1.15); r = 0.19; }
    if (i == 1) { base = vec3f(0.62, 0.3, -2.1); }
    if (i == 2) { base = vec3f(-0.95, -0.25, -2.6); }
    if (i == 3) { base = vec3f(0.78, -0.42, -0.85); r = 0.12; }
    if (i == 4) { base = vec3f(-0.55, 0.55, -1.9); }
    let t = ctx.t;
    let w = 0.03 + 0.02 * hx;
    let drift = vec3f(0.35 * sin(t * w + fi * 2.1), 0.22 * sin(t * w * 1.3 + fi * 1.3), 0.3 * sin(t * w * 0.7 + fi));
    // pulse: a quick contraction and a slow relaxation, ~0.6 Hz
    let ph = fract(t * (0.55 + 0.12 * hy) + hz);
    let pulse = smoothstep(0.0, 0.18, ph) * (1.0 - smoothstep(0.18, 0.95, ph));
    // each stroke lifts the bell a little; it sinks back while relaxing
    let bob = 0.018 * (pulse - 0.4);
    let c = base + drift + vec3f(0.0, bob, 0.0);
    let vel = vec3f(0.35 * w * cos(t * w + fi * 2.1), 0.22 * w * 1.3 * cos(t * w * 1.3 + fi * 1.3) + 0.02, 0.3 * w * 0.7 * cos(t * w * 0.7 + fi));
    // tilt toward the direction of travel, plus a slow personal wobble
    // most swim tipped toward the glass (their gonad rings show through the
    // bell); a few tip away and show the oral arms
    let lean = select(0.55 + 0.25 * hx, -0.45, hash_f(u32(i) * 13u + 5u) < 0.3);
    let up = normalize(vec3f(0.0, 1.0, lean) + vel * 6.0 + vec3f(0.15 * sin(t * 0.21 + fi), 0.0, 0.12 * cos(t * 0.17 + fi * 2.0)));
    return Jelly(c, up, r, r * 0.44, pulse, fi);
}

// clipped interval inside ellipsoid radii (a, b, a) centred at yc, above ymin
fn ell_span(ro: vec3f, rd: vec3f, a: f32, b: f32, yc: f32, ymin: f32) -> vec2f {
    let s = vec3f(1.0 / a, 1.0 / b, 1.0 / a);
    let o = (ro - vec3f(0.0, yc, 0.0)) * s;
    let d = rd * s;
    let qa = dot(d, d);
    let qb = dot(o, d);
    let qc = dot(o, o) - 1.0;
    let disc = qb * qb - qa * qc;
    if (disc <= 0.0) { return vec2f(1.0, 0.0); }
    let sq_ = sqrt(disc);
    var t0 = (-qb - sq_) / qa;
    var t1 = (-qb + sq_) / qa;
    // the bell is a dome: keep y >= ymin
    if (abs(rd.y) > 1e-5) {
        let ty = (ymin - ro.y) / rd.y;
        if (rd.y > 0.0) { t0 = max(t0, ty); } else { t1 = min(t1, ty); }
    } else if (ro.y < ymin) {
        return vec2f(1.0, 0.0);
    }
    return vec2f(t0, t1);
}

// (colour, alpha) of one jelly along the ray
fn jelly_shade(ro: vec3f, rd: vec3f, j: Jelly, l: Look, ctx: Ctx) -> vec4f {
    // local frame: y along the bell's axis
    let up = j.up;
    let rt = normalize(cross(up, vec3f(0.0, 0.0, 1.0)));
    let fw = cross(rt, up);
    let o = ro - j.c;
    let lro = vec3f(dot(o, rt), dot(o, up), dot(o, fw));
    let lrd = vec3f(dot(rd, rt), dot(rd, up), dot(rd, fw));
    // contracted bells are narrower and taller
    let a = j.r * (1.0 - 0.14 * j.pulse);
    let b = j.h * (1.0 + 0.12 * j.pulse);
    let ymin = -0.12 * b;
    let dist = length(o);
    var col = vec3f(0.0);
    var alpha = 0.0;
    // bell: outer dome minus the subumbrellar cavity (thicker at the apex)
    let so = ell_span(lro, lrd, a, b, 0.0, ymin);
    if (so.y > so.x && so.y > 0.0) {
        let si = ell_span(lro, lrd, a * 0.9, b * 0.8, -0.06 * b, ymin);
        var path = so.y - max(so.x, 0.0);
        if (si.y > si.x) { path -= max(si.y - max(si.x, 0.0), 0.0); }
        let tissue = 1.0 - exp(-path / (1.6 * a));
        // rim: the thin margin seen edge-on
        let lp = lro + lrd * max(so.x, 0.0);
        let edge = saturate(length(lp.xz) / a);
        let rim = pow(edge, 5.0) * l.rim;
        var c = l.tissue * (tissue * l.glow * 0.6 + rim * 0.8 * (0.25 + tissue) + 0.025 * l.glow);
        // subtle radial canals
        let ang = atan2(lp.z, lp.x);
        c *= 0.9 + 0.15 * smoothstep(0.7, 1.0, cos(ang * 16.0)) * edge;
        // four gonad rings on a plane inside the bell
        if (abs(lrd.y) > 1e-4) {
            let tg = (0.28 * b - lro.y) / lrd.y;
            let g = lro + lrd * tg;
            let gr = length(g.xz);
            if (tg > 0.0 && gr < 0.7 * a) {
                // four horseshoes, open toward the centre, soft and a little
                // irregular (they are tissue, not paint)
                var ring = 0.0;
                for (var k = 0; k < 4; k++) {
                    let an = 0.785398 + f32(k) * 1.570796 + 0.1 * sin(j.id * 3.0 + f32(k));
                    let dir = vec2f(cos(an), sin(an));
                    let cc = dir * 0.26 * a;
                    let v = g.xz - cc;
                    let dd = length(v);
                    let open = mix(0.35, 1.0, smoothstep(-0.95, -0.7, dot(v / max(dd, 1e-5), dir)));
                    let wob = 1.0 + 0.12 * sin(atan2(v.y, v.x) * 3.0 + j.id);
                    ring = max(ring, exp(-sq((dd - 0.155 * a * wob) / (0.055 * a))) * open);
                }
                let tint = mix(l.gonad, l.tissue, 0.35 * hash_f(u32(j.id) * 3u + 1u));
                c += tint * ring * (0.22 + 0.6 * l.glow) * (1.0 - 0.35 * j.pulse);
            }
        }
        col = c;
        alpha = saturate(tissue * 0.35 + rim * 0.25);
    }
    // marginal tentacles: a skirt of fine lines trailing below the margin
    let tr = a * (0.96 + 0.1 * j.pulse);
    let ca = lrd.x * lrd.x + lrd.z * lrd.z;
    if (ca > 1e-6) {
        let cb = lro.x * lrd.x + lro.z * lrd.z;
        let cc = lro.x * lro.x + lro.z * lro.z - tr * tr;
        let disc = cb * cb - ca * cc;
        if (disc > 0.0) {
            let sq_ = sqrt(disc);
            let tl = 0.3 * a + 0.25 * a * (1.0 - j.pulse);
            for (var s = 0; s < 2; s++) {
                let tt = (-cb + select(-sq_, sq_, s == 1)) / ca;
                let q = lro + lrd * tt;
                let dy = ymin - q.y;
                if (tt > 0.0 && dy > 0.0 && dy < tl) {
                    let an = atan2(q.z, q.x);
                    let sway = 0.25 * sin(ctx.t * 0.8 + dy / a * 3.0 + j.id);
                    let lines = fract(an * 90.0 / TAU + sway + dy / a * 0.6);
                    let wline = max(0.2, ctx.px * dist * 110.0);
                    let m = smoothstep(wline, 0.0, abs(lines - 0.5) - 0.01) * sq(1.0 - dy / tl) * select(0.5, 1.0, s == 0);
                    col += l.tissue * m * 0.18 * (l.glow + 0.3 * l.rim);
                    alpha = max(alpha, m * 0.12);
                }
            }
        }
    }
    // oral arms: a frilled column hanging from the centre
    let orr = 0.16 * a;
    if (ca > 1e-6) {
        let cb = lro.x * lrd.x + lro.z * lrd.z;
        let cc = lro.x * lro.x + lro.z * lro.z - orr * orr;
        let disc = cb * cb - ca * cc;
        if (disc > 0.0) {
            let tt = (-cb - sqrt(disc)) / ca;
            let q = lro + lrd * tt;
            let dy = -0.05 * b - q.y;
            let al = 1.1 * a;
            if (tt > 0.0 && dy > 0.0 && dy < al) {
                let an = atan2(q.z, q.x);
                let rib = pow(0.5 + 0.5 * cos(an * 4.0 + sin(dy / a * 3.0 + ctx.t * 0.4 + j.id) * 0.6), 3.0);
                let frill = 0.6 + 0.4 * sin(dy / a * 22.0 + an * 3.0);
                let m = rib * frill * (1.0 - dy / al) * 0.45;
                col += mix(l.tissue, l.gonad, 0.3) * m * (0.6 * l.glow + 0.25 * l.rim);
                alpha = max(alpha, m * 0.35);
            }
        }
    }
    return vec4f(col, alpha);
}

// suspended particles: fine specks and big soft motes near the glass
fn motes(p: vec2f, l: Look, ctx: Ctx) -> f32 {
    var a = 0.0;
    for (var k = 0; k < 3; k++) {
        let fk = f32(k);
        let sc = 9.0 + fk * 9.0;
        let q = p * sc + vec2f(ctx.t * (0.03 + fk * 0.015), ctx.t * (0.02 - fk * 0.01));
        let c = floor(q);
        let h = hash_cell2(vec2i(c), 0x7e11u + u32(k));
        if (h.w < 0.07) {
            let d = length(fract(q) - 0.2 - 0.6 * h.xy);
            let r = select(max(0.04, ctx.px * sc * 0.7), 0.18 + 0.1 * h.z, k == 0);
            let soft = select(r * 0.4, r * 0.9, k == 0);
            a += smoothstep(r, r - soft, d) * select(0.5, 0.25, k == 0);
        }
    }
    return a;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let cam = cam_look_at(vec3f(0.0), vec3f(0.0, 0.0, -1.0), 0.0, 50.0);
    let rd = cam_ray(cam, p);
    let bg = backdrop(p, l, ctx);
    // jellies, sorted back to front once per pixel
    var js_: array<Jelly, 9>;
    var order = array<i32, 9>(0, 1, 2, 3, 4, 5, 6, 7, 8);
    var depth = array<f32, 9>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (var i = 0; i < NJ; i++) {
        js_[i] = jelly_get(i, ctx);
        depth[i] = -js_[i].c.z;
    }
    for (var i = 0; i < NJ; i++) {
        for (var k = 0; k < NJ - 1; k++) {
            if (depth[order[k]] < depth[order[k + 1]]) {
                let tmp = order[k];
                order[k] = order[k + 1];
                order[k + 1] = tmp;
            }
        }
    }
    // A real lens: 8 fixed samples over the aperture, focused on the hero.
    // Jellies nearer or farther than 1.2 m blur like they do through glass.
    let focus = 1.2;
    let aperture = 0.03;
    var acc = vec3f(0.0);
    for (var s = 0; s < 8; s++) {
        let ang = (f32(s) + ctx.jitter) * 2.399963;
        let rr = sqrt((f32(s) + 0.5) / 8.0) * aperture;
        let lo = cam.rt * (cos(ang) * rr) + cam.up * (sin(ang) * rr);
        let fp = rd * (focus / dot(rd, cam.fw));
        let rds = normalize(fp - lo);
        var col = bg;
        for (var n = 0; n < NJ; n++) {
            let j = js_[order[n]];
            let pr = cam_project(cam, j.c);
            if (pr.z <= 0.1) { continue; }
            let coc = aperture * abs(pr.z - focus) / (pr.z * focus) * cam.zoom;
            let rad = (j.r * 2.4) * cam.zoom / pr.z + coc;
            if (length(p - pr.xy - vec2f(0.0, -rad * 0.3)) > rad * 1.25) { continue; }
            var sh = jelly_shade(lo, rds, j, l, ctx);
            // water haze toward the back of the tank
            let fog = exp(-pr.z * 0.18);
            sh = vec4f(sh.rgb * fog + l.fog * (1.0 - fog) * sh.a, sh.a);
            col = col * (1.0 - sh.a * 0.35) + sh.rgb;
        }
        acc += col;
    }
    var col = acc / 8.0;
    col += (l.top * 0.6 + l.tissue * 0.05) * motes(p, l, ctx);
    return col * exp2(l.exposure);
}
