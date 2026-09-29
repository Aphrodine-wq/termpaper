//! name: venice
//! title: Venice Canal
//! category: city
//! tags: venice, canal, gondola, bridge, reflections, italy
//! desc: a narrow Venetian rio between weathered palazzi, a gondola drifting under a stone bridge
//! themes: morning, sunset, night
//! uses: camera, raymarch, sky, water, light, noise
//! cost: light
//! tonemap: aces
//! fallback: ripple
//! credits: original

// World units are metres. The camera floats a couple of metres above a
// narrow rio, looking along it (-z). Palazzo walls rise straight from the
// water at x = +-CW; a stone bridge arches across; a campanile rises over
// the roofs at the far end. Walls, bridge, poles and laundry are analytic;
// the gondola is ray-marched in its own bounding box. The water mirrors the
// same scene along a wave-tilted reflection ray. The sun's shadow line on the
// walls is closed form, as in a street canyon.

const CW: f32 = 4.6;          // canal half width (wall planes)
const BZ: f32 = -25.0;        // bridge front face
const BT: f32 = 3.2;          // bridge thickness
const ZEND: f32 = -118.0;     // the facade closing the vista

struct Look {
    sun: vec3f,
    sun_c: vec3f,
    amb: vec3f,
    sky_hi: vec3f,
    sky_lo: vec3f,
    lamps: f32,
    haze: f32,
    hazec: vec3f,
    exposure: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    if (theme == 1u) {
        // sunset: the last warm light high on the left-hand walls
        l.sun = sky_sun_dir(10.0, 38.0);
        l.sun_c = vec3f(1.0, 0.52, 0.28) * 3.6;
        l.amb = vec3f(0.3, 0.24, 0.3);
        l.sky_hi = vec3f(0.18, 0.2, 0.36);
        l.sky_lo = vec3f(1.0, 0.55, 0.38);
        l.lamps = 0.25;
        l.haze = 2.0;
        l.hazec = vec3f(0.55, 0.36, 0.3);
        l.exposure = -0.2;
    } else if (theme == 2u) {
        // night: iron lanterns on the walls, windows, the moonless blue
        l.sun = sky_sun_dir(-20.0, 0.0);
        l.sun_c = vec3f(0.0);
        l.amb = vec3f(0.006, 0.008, 0.015);
        l.sky_hi = vec3f(0.002, 0.004, 0.012);
        l.sky_lo = vec3f(0.018, 0.02, 0.035);
        l.lamps = 1.0;
        l.haze = 1.0;
        l.hazec = vec3f(0.012, 0.014, 0.022);
        l.exposure = 0.2;
    } else {
        // morning: soft sun from the right, lagoon haze
        l.sun = sky_sun_dir(30.0, 50.0);
        l.sun_c = vec3f(1.0, 0.88, 0.72) * 2.8;
        l.amb = vec3f(0.5, 0.55, 0.62);
        l.sky_hi = vec3f(0.28, 0.42, 0.65);
        l.sky_lo = vec3f(0.85, 0.82, 0.78);
        l.lamps = 0.0;
        l.haze = 1.6;
        l.hazec = vec3f(0.62, 0.64, 0.66);
        l.exposure = -0.45;
    }
    return l;
}

// ------------------------------------------------------------ palazzi

struct Pal {
    z0: f32,      // nearer end
    len: f32,
    h: f32,       // eave height
    base: vec3f,  // plaster colour
    id: u32,
}

fn palazzo(z: f32, side: i32) -> Pal {
    // frontages 9-16 m, cut at irregular points
    let c0 = floor(-z / 12.0);
    let h = hash_cell2(vec2i(i32(c0), side), 0x7e4u);
    var p: Pal;
    p.z0 = -c0 * 12.0;
    p.len = 12.0;
    // the left bank runs to four and five floors; the right is lower, so the
    // low sun reaches across onto the left-hand facades
    if (side == 0) {
        p.h = select(12.0 + 4.5 * h.x, 8.5 + 1.5 * h.x, h.w < 0.15);
    } else {
        p.h = select(8.6 + 2.8 * h.x, 13.0 + 2.0 * h.x, h.w < 0.25);
    }
    let k = h.y;
    var col = col_hex(0xd8a45cu);                    // ochre
    if (k > 0.2) { col = col_hex(0xc8704cu); }       // terracotta
    if (k > 0.38) { col = col_hex(0xdc9c8cu); }      // rose
    if (k > 0.55) { col = col_hex(0xe6ceacu); }      // pale cream
    if (k > 0.7) { col = col_hex(0xb4523eu); }       // Venetian red
    if (k > 0.85) { col = col_hex(0xe8b89cu); }      // faded pink
    p.base = col * (0.85 + 0.2 * h.z);
    p.id = hash_u2(vec2u(bitcast<u32>(i32(c0)), u32(side) + 3u));
    return p;
}

// lit by the sun? the opposite wall's eave casts the shadow (closed form)
fn sunlit(q: vec3f, l: Look) -> f32 {
    if (l.sun_c.x <= 0.0) { return 0.0; }
    let s = l.sun;
    if (abs(s.x) < 0.01) { return 1.0; }
    let other = select(0, 1, s.x > 0.0);
    let xo = select(-CW, CW, other == 1);
    let dist = (xo - q.x) / s.x;
    if (dist <= 0.0) { return 1.0; }
    let r = q + s * dist;
    let pp = palazzo(r.z, other);
    return smoothstep(-0.3, 0.3, r.y - pp.h - 0.6);
}

fn wall_lamp(i: i32) -> vec3f {
    // iron lanterns on brackets, alternating sides every 9 m
    let side = select(-1.0, 1.0, (i & 1) == 1);
    return vec3f(side * (CW - 0.55), 4.3, -f32(i) * 9.0 - 3.0);
}

fn lamp_light(p: vec3f, n: vec3f, l: Look) -> vec3f {
    if (l.lamps <= 0.1) { return vec3f(0.0); }
    let i0 = i32(floor((-p.z - 3.0) / 9.0));
    var e = vec3f(0.0);
    for (var i = i0 - 1; i <= i0 + 2; i++) {
        if (i < 0) { continue; }
        let lp = wall_lamp(i);
        let d = lp - p;
        let d2 = dot(d, d);
        let nl = saturate(dot(n, d * inverseSqrt(d2)) * 0.9 + 0.1);
        e += vec3f(1.0, 0.68, 0.36) * 3.5 * nl / (d2 + 0.4);
    }
    return e * l.lamps;
}

// plaster wall of palazzo pl at p; u metres along it, facing n
fn shade_wall(p: vec3f, n: vec3f, rd: vec3f, pl: Pal, l: Look, ctx: Ctx, lod: f32) -> vec3f {
    let zl = pl.z0 - p.z;
    let y = p.y;
    let hb = hash_cell2(vec2i(i32(pl.id & 1023u), 5), 0x33u);
    var alb = pl.base;
    // weathering: stains running down, blotches, bare brick where the
    // plaster has fallen away
    let st = noise_fbm2(vec2f(zl * 0.6, y * 0.12), 4);
    alb *= 0.78 + 0.35 * st;
    let blotch = noise_fbm2(vec2f(zl * 0.4, y * 0.45) + f32(pl.id & 255u), 4);
    let brick_m = smoothstep(0.7, 0.73, blotch + 0.08 * smoothstep(3.0, 0.8, y));
    let bq = vec2f(zl / 0.25 + 0.5 * floor(y / 0.07), y / 0.07);
    let mortar = step(0.85, fract(bq.x)) + step(0.8, fract(bq.y));
    let brick = mix(col_hex(0x9a4a34u), col_hex(0xb8a890u), saturate(mortar) * (1.0 - saturate(lod * 12.0)) + 0.2 * saturate(lod * 12.0));
    alb = mix(alb, brick, brick_m);
    // green-black tide band at the waterline, a white stone course above
    let tide = smoothstep(0.9, 0.2, y + 0.15 * noise_value2(vec2f(zl * 2.0, 1.0)));
    alb = mix(alb, vec3f(0.05, 0.07, 0.045), tide);
    let course = step(abs(y - 1.3), 0.12);
    alb = mix(alb, vec3f(0.78, 0.76, 0.7), course);
    // party joint between palazzi
    let joint = step(zl, 0.08) + step(pl.len - 0.08, zl);
    alb *= 1.0 - 0.4 * joint;
    // windows: Gothic arches on bays of 2.4 m; floors of 3.1 m above a
    // ground floor of water gates
    var emi = vec3f(0.0);
    var glass = 0.0;
    let bay = zl / 2.4;
    let u = fract(bay) - 0.5;
    let fl = floor((y - 2.6) / 3.1);
    let fy = (y - 2.6) - fl * 3.1;
    let wh = hash_cell2(vec2i(i32(floor(bay)) + i32(pl.id & 511u), i32(fl)), 0x5e1u);
    if (y > 2.6 && y < pl.h - 0.8 && wh.x > 0.15) {
        // lancet: a rectangle topped by a pointed arch
        let hw = 0.42;
        let top = 1.9;
        let ax = abs(u * 2.4);
        let arch = top + 0.45 * sqrt(saturate(1.0 - sq(ax / hw))) * select(1.0, 1.25, hb.x > 0.5);
        let inw = step(ax, hw) * step(0.45, fy) * step(fy, arch);
        let frame = step(ax, hw + 0.12) * step(0.33, fy) * step(fy, arch + 0.12) * (1.0 - inw);
        alb = mix(alb, vec3f(0.8, 0.78, 0.72), frame);
        // a projecting sill with its shadow beneath
        let sill = step(ax, hw + 0.22) * step(0.3, fy) * step(fy, 0.42);
        alb = mix(alb, vec3f(0.85, 0.83, 0.78), sill);
        alb *= 1.0 - 0.45 * step(ax, hw + 0.2) * step(0.1, fy) * step(fy, 0.3);
        if (inw > 0.5) {
            // green shutters (often closed), or dark glass with a lit room
            // the reveal: stone depth shading the top and one side
            let reveal = 1.0 - 0.6 * smoothstep(hw - 0.1, hw, ax + select(0.0, 0.08, u < 0.0)) - 0.4 * smoothstep(arch - 0.15, arch, fy);
            if (wh.y < 0.4 && l.lamps < 0.9) {
                alb = vec3f(0.08, 0.2, 0.12) * (0.8 + 0.2 * step(0.2, fract(fy * 8.0))) * reveal;
            } else {
                glass = 1.0;
                let lit = step(0.55, wh.z) * l.lamps + step(0.93, wh.z) * 0.3;
                emi = mix(col_kelvin(2400.0), col_kelvin(3000.0), wh.w) * 0.55 * lit;
                if (wh.y < 0.4) { emi *= 0.3; }   // light through closed shutter slats
            }
        }
        // a flower box of geraniums under some windows
        let box = step(ax, hw + 0.05) * step(0.18, fy) * step(fy, 0.45) * step(0.7, wh.w);
        let fl_n = noise_value2(vec2f(zl * 14.0, y * 14.0));
        alb = mix(alb, mix(vec3f(0.12, 0.25, 0.06), vec3f(0.7, 0.05, 0.04), step(0.55, fl_n)), box);
    } else if (y < 2.4) {
        // water gate: a stone-framed doorway with steps into the canal
        let gz = fract(zl / pl.len) - 0.5;
        let gate = step(abs(gz * pl.len), 0.8) * step(y, 2.3) * step(0.2, fract(f32(pl.id) * 0.013));
        alb = mix(alb, vec3f(0.12, 0.08, 0.05), gate * step(0.1, y));
    }
    // roof: terracotta eave overhang
    let eave = step(pl.h - 0.35, y);
    alb = mix(alb, vec3f(0.5, 0.25, 0.15), eave);
    let sh = sunlit(p + n * 0.05, l);
    var e = l.sun_c * saturate(dot(n, l.sun)) * sh + l.amb * (0.5 + 0.2 * n.y);
    // light bounced off the water and the sunlit wall opposite
    e += l.sun_c * 0.05 + l.amb * 0.15 * smoothstep(3.0, 0.0, y);
    e += lamp_light(p, n, l);
    var c = alb * 0.318 * e * (1.0 - 0.5 * eave * step(y, pl.h - 0.1));
    if (glass > 0.5) {
        // old glass mirrors the facade across the canal, or the sky
        let r = reflect(rd, n);
        let fr = 0.06 + 0.6 * pow(1.0 - saturate(-dot(rd, n)), 4.0);
        let sk = mix(l.sky_lo, l.sky_hi, saturate(r.y * 2.0));
        let across = vec3f(0.75, 0.55, 0.4) * 0.318 * (l.amb * 0.8 + l.sun_c * 0.25) + vec3f(1.0, 0.7, 0.4) * 0.05 * l.lamps;
        let refl = mix(across, sk, smoothstep(0.15, 0.5, r.y));
        c = alb * 0.01 * e + refl * fr + emi;
    }
    return c;
}

// ------------------------------------------------------------ the bridge

// bridge silhouette in its face plane: x across, y up. < 0 inside stone
fn bridge_d(x: f32, y: f32) -> f32 {
    // a segmental arch springing from the walls, a stepped deck, parapets
    let span = CW;
    let crown = 2.9;
    let arch = 0.4 + (crown - 0.4) * sqrt(saturate(1.0 - sq(x / span)));
    let deck = 3.5 + 0.55 * (1.0 - sq(x / (span + 2.0)));
    let parapet = deck + 1.0;
    var d = max(y - parapet, arch - y);
    d = max(d, abs(x) - span - 3.0);
    return d;
}

// ------------------------------------------------------------ the gondola

fn gondola_pos(t: f32) -> vec4f {
    // drifting along the canal and back over four minutes: (x, z, heading, oar phase)
    let ph = fract(t / 240.0);
    let s = select(ph * 2.0, 2.0 - ph * 2.0, ph > 0.5);
    let e = s * s * (3.0 - 2.0 * s);
    let z = mix(-58.0, -12.0, e);
    let x = 1.2 + 0.4 * sin(t * 0.05);
    let dir = select(1.0, -1.0, ph > 0.5);
    return vec4f(x, z, dir, t * 0.9);
}

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    let g = gondola_pos(ctx.t);
    var q = p - vec3f(g.x, 0.0, g.y);
    q.x = q.x + q.z * 0.12;                    // a slight angle to the canal
    // hull: long, narrow, both ends rising (and asymmetric: lean to port)
    let lz = q.z / 5.6;
    let rise = 0.55 * lz * lz * lz * lz + 0.1 * lz * lz;
    let hq = vec3f(q.x + 0.12 * (1.0 - lz * lz), q.y - rise - 0.2, q.z);
    let hull = sdf_ellipsoid(hq, vec3f(0.7, 0.42, 5.6));
    var d = max(hull, -(q.y - rise - 0.34));   // open top
    d = min(d, max(hull, abs(q.y - rise - 0.3) - 0.03));
    // the ferro at the bow and the stern post
    let bow = sdf_box(q - vec3f(0.0, 1.25 + rise * 0.0, -5.2 * g.z), vec3f(0.02, 0.45, 0.28));
    d = min(d, bow);
    var r = vec2f(d, 1.0);
    // the gondolier standing at the stern, rowing
    let st = vec3f(0.2, 0.55, 4.0 * g.z);
    let bq = q - st;
    let body = sdf_capsule(bq, vec3f(0.0, 0.0, 0.0), vec3f(0.0, 1.1, 0.0), 0.2);
    let head = sdf_sphere(bq - vec3f(0.0, 1.42, 0.0), 0.12);
    let hat = sdf_cyl_y(bq - vec3f(0.0, 1.55, 0.0), 0.02, 0.2);
    let legs = sdf_capsule(bq, vec3f(0.0, -0.5, 0.0), vec3f(0.0, 0.1, 0.0), 0.13);
    let osw = sin(g.w) * 0.35;
    let oar = sdf_capsule(bq, vec3f(0.0, 1.0, 0.0), vec3f(-0.9, -0.6, osw * 2.0), 0.035);
    r = op_umin(r, vec2f(min(min(body, legs), min(head, hat)), 2.0));
    r = op_umin(r, vec2f(oar, 3.0));
    return r;
}

fn gondola_bounds(ro: vec3f, rd: vec3f, ctx: Ctx) -> vec2f {
    let g = gondola_pos(ctx.t);
    let c = vec3f(g.x, 1.0, g.y);
    let hb = vec3f(1.8, 1.7, 6.2);
    let inv = 1.0 / select(rd, vec3f(1e-6), abs(rd) < vec3f(1e-6));
    let t0 = (c - hb - ro) * inv;
    let t1 = (c + hb - ro) * inv;
    let tmin = min(t0, t1);
    let tmax = max(t0, t1);
    return vec2f(max(max(tmin.x, tmin.y), max(tmin.z, 0.0)), min(min(tmax.x, tmax.y), tmax.z));
}

// ------------------------------------------------------------ tracing

struct Hit { t: f32, kind: i32, p: vec3f, n: vec3f, side: i32 }

fn trace(ro: vec3f, rd: vec3f) -> Hit {
    var h = Hit(1e9, 0, vec3f(0.0), vec3f(0.0, 1.0, 0.0), 0);
    if (rd.y < 0.0) {
        let t = -ro.y / rd.y;
        h = Hit(t, 1, ro + rd * t, vec3f(0.0, 1.0, 0.0), 0);
    }
    // walls
    if (abs(rd.x) > 1e-5) {
        let side = select(0, 1, rd.x > 0.0);
        let sx = select(-1.0, 1.0, side == 1);
        let t = (sx * CW - ro.x) / rd.x;
        if (t > 0.0 && t < h.t) {
            let p = ro + rd * t;
            if (p.z > ZEND) {
                let pl = palazzo(p.z, side);
                if (p.y < pl.h) {
                    h = Hit(t, 2, p, vec3f(-sx, 0.0, 0.0), side);
                } else {
                    // chimneys: bell-topped Venetian flues standing on the roofs
                    let tc = (sx * (CW + 2.5) - ro.x) / rd.x;
                    let pc = ro + rd * tc;
                    let cz = fract(-pc.z / 7.0) - 0.5;
                    let ch = hash_cell2(vec2i(i32(floor(-pc.z / 7.0)), side), 0xc41u);
                    let ph = palazzo(pc.z, side).h;
                    let yy = pc.y - ph;
                    let stack = step(abs(cz * 7.0), 0.22) * step(yy, 1.6 + ch.y);
                    let bell = step(abs(cz * 7.0), 0.22 + 0.3 * smoothstep(1.2 + ch.y, 2.0 + ch.y, yy)) * step(1.2 + ch.y, yy) * step(yy, 2.1 + ch.y);
                    if (max(stack, bell) > 0.5 && ch.x < 0.6 && yy > -0.5 && tc < h.t && pc.z > ZEND) {
                        h = Hit(tc, 7, pc, vec3f(-sx, 0.0, 0.0), side);
                    }
                }
            }
        }
    }
    // stone balconies with colonnettes in front of some upper windows
    if (abs(rd.x) > 1e-5) {
        let side = select(0, 1, rd.x > 0.0);
        let sx = select(-1.0, 1.0, side == 1);
        let tb = (sx * (CW - 0.5) - ro.x) / rd.x;
        if (tb > 0.0 && tb < h.t) {
            let p = ro + rd * tb;
            let pl = palazzo(p.z, side);
            let zl = pl.z0 - p.z;
            let bay = zl / 2.4;
            let u = (fract(bay) - 0.5) * 2.4;
            let fl = floor((p.y - 2.6) / 3.1);
            let fy = (p.y - 2.6) - fl * 3.1;
            let hb = hash_cell2(vec2i(i32(floor(bay)) + i32(pl.id & 511u), i32(fl)), 0xba1cu);
            if (fl >= 0.0 && fl <= 1.0 && hb.x < 0.28 && abs(u) < 0.75 && fy < 1.0 && p.y < pl.h - 1.0 && p.z > ZEND) {
                // bulging colonnettes: fat in the middle, slim at the ends
                let cf = fract((u + 0.75) / 0.19) - 0.5;
                let bulge = 0.16 + 0.12 * sin(PI * saturate((fy - 0.12) / 0.73));
                let col = step(abs(cf), bulge);
                let rail = step(0.85, fy);
                let slab = step(fy, 0.12);
                if (max(col, max(rail, slab)) > 0.5) {
                    h = Hit(tb, 8, p, vec3f(-sx, 0.0, 0.0), side);
                }
            }
        }
    }
    // bridge: front face, the stone seen through under the arch (intrados)
    if (rd.z < 0.0) {
        let tf = (BZ - ro.z) / rd.z;
        if (tf > 0.0 && tf < h.t) {
            let p = ro + rd * tf;
            if (abs(p.x) < CW && bridge_d(p.x, p.y) < 0.0) {
                h = Hit(tf, 3, p, vec3f(0.0, 0.0, 1.0), 0);
            } else if (abs(p.x) < CW && p.y < 3.0 && p.y > 0.0) {
                // under the arch: the ray may meet the curved soffit before
                // leaving by the back face
                let tb = (BZ - BT - ro.z) / rd.z;
                let pb = ro + rd * tb;
                if (bridge_d(pb.x, pb.y) < 0.0 && pb.y > 0.3) {
                    h = Hit(tf + (tb - tf) * 0.5, 4, pb, vec3f(0.0, -1.0, 0.0), 0);
                }
            }
        }
        // the facade closing the vista, the campanile beyond
        let te = (ZEND - ro.z) / rd.z;
        if (te > 0.0 && te < h.t) {
            let p = ro + rd * te;
            if (p.y < 15.5) {
                h = Hit(te, 5, p, vec3f(0.0, 0.0, 1.0), 0);
            }
        }
    }
    return h;
}

fn campanile(ro: vec3f, rd: vec3f, l: Look, ctx: Ctx, zoom: f32) -> vec4f {
    if (rd.z >= 0.0) { return vec4f(0.0); }
    let t = (-260.0 - ro.z) / rd.z;
    let p = ro + rd * t;
    let lod = ctx.px / zoom * t;
    let x = p.x - 6.0;
    let y = p.y;
    // brick shaft, the open belfry, a drum and a pointed cone
    var d = max(abs(x) - 3.6, y - 44.0);
    d = min(d, max(abs(x) - 4.1, abs(y - 47.0) - 3.0));
    d = min(d, max(abs(x) - 3.2, abs(y - 52.5) - 2.5));
    d = min(d, max(abs(x) - 2.6 * saturate((64.0 - y) / 9.0), abs(y - 59.5) - 4.5));
    d = max(d, -y);
    let a = saturate(0.5 - d / max(lod, 0.05));
    if (a <= 0.0) { return vec4f(0.0); }
    let belfry = step(abs(y - 47.0), 2.0) * step(0.5, fract(x / 1.8 + 0.25));
    var alb = mix(col_hex(0xa0553eu), vec3f(0.02), belfry);
    let sh = select(0.0, 1.0, l.sun.x < 0.0);
    var c = alb * 0.318 * (l.sun_c * (0.3 + 0.4 * sh) + l.amb);
    c += vec3f(1.0, 0.75, 0.45) * 0.4 * l.lamps * step(abs(y - 47.0), 2.0) * belfry;
    c = mix(c, l.hazec, 1.0 - exp(-t * 0.001 * l.haze));
    return vec4f(c * a, a);
}

fn venice_sky(rd: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let y = max(rd.y, 0.0);
    var c = mix(l.sky_lo, l.sky_hi, pow(saturate(y * 1.8), 0.6));
    let mu = saturate(dot(rd, l.sun));
    c += l.sun_c * 0.15 * pow(mu, 12.0);
    let uv = rd.xz / (rd.y + 0.12);
    let n = noise_fbm2(uv * 0.5 + vec2f(ctx.t * 0.003, 0.0), 5);
    let cl = smoothstep(0.5, 0.8, n) * smoothstep(0.02, 0.2, rd.y);
    let lit = mix(l.sky_lo * 1.1, l.amb * 2.0 + l.sun_c * 0.1, 0.5);
    c = mix(c, lit, cl * 0.5);
    return c;
}

// laundry strung across the canal: lines with sheets and shirts, swaying
fn laundry(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx, zoom: f32) -> vec4f {
    var res = vec4f(0.0);
    for (var k = 0; k < 3; k++) {
        let zl = -9.0 - f32(k) * 19.0 - 4.0 * f32(k & 1);
        let t = (zl - ro.z) / min(rd.z, -1e-4);
        if (t <= 0.0 || t > tmax || res.w > 0.99) { continue; }
        let p = ro + rd * t;
        if (abs(p.x) > CW) { continue; }
        let lod = ctx.px / zoom * t;
        let y0 = 6.2 + 1.8 * f32(k);
        let yl = y0 - 0.6 * (1.0 - sq(p.x / CW));
        let line = saturate(0.5 - (abs(p.y - yl) - 0.01) / max(lod, 1e-3)) * 0.8;
        // garments: pegged at intervals, hanging below, lifting in the breeze
        let gx = (p.x + CW) / 0.95;
        let gi = floor(gx);
        let gh = hash_cell2(vec2i(i32(gi), k), 0x1a7u);
        let fx = fract(gx);
        let sway = 0.05 * sin(ctx.t * 0.7 + gi * 1.3);
        let len = 0.55 + 0.6 * gh.y;
        let dy = yl - p.y;
        let cloth = step(0.12, fx + sway) * step(fx + sway, 0.88) * step(0.0, dy) * step(dy, len) * step(0.35, gh.x);
        var cc = mix(vec3f(0.85, 0.83, 0.8), vec3f(0.3, 0.45, 0.7), step(0.6, gh.z));
        if (gh.z > 0.85) { cc = vec3f(0.7, 0.2, 0.15); }
        if (gh.z < 0.2) { cc = vec3f(0.85, 0.7, 0.3); }
        let sh = sunlit(p, l);
        let e = l.sun_c * (0.3 + 0.5 * sh) + l.amb + lamp_light(p, vec3f(0.0, 0.0, 1.0), l) * 0.5;
        // cloth glows when the sun is behind it
        let c = cc * 0.318 * e * (1.0 + 0.5 * saturate(dot(rd, l.sun)));
        let a = max(line, cloth * 0.95);
        let col = mix(vec3f(0.02) * (l.amb + 0.05), c, cloth);
        res = vec4f(res.xyz + (1.0 - res.w) * col * a, res.w + (1.0 - res.w) * a);
    }
    return res;
}

// striped mooring poles near the walls
fn poles(ro: vec3f, rd: vec3f, tmax: f32, l: Look, ctx: Ctx, zoom: f32) -> vec4f {
    var best = vec4f(0.0, 0.0, 0.0, 1e9);
    for (var k = 0; k < 8; k++) {
        let h = hash_cell2(vec2i(k, 2), 0x9a1eu);
        let side = select(-1.0, 1.0, h.x > 0.5);
        let c = vec2f(side * (CW - 0.7 - 0.3 * h.y), -8.0 - f32(k) * 11.0 - 4.0 * h.z);
        for (var m = 0; m < 2; m++) {
            let cc = c + vec2f(0.0, f32(m) * 0.9);
            let o = ro.xz - cc;
            let a = dot(rd.xz, rd.xz);
            let b = dot(o, rd.xz);
            let q = dot(o, o) - 0.11 * 0.11;
            let disc = b * b - a * q;
            if (disc < 0.0) { continue; }
            let t = (-b - sqrt(disc)) / a;
            if (t <= 0.0 || t > min(tmax, best.w)) { continue; }
            let p = ro + rd * t;
            let top = 3.2 + 0.4 * h.w;
            if (p.y < 0.0 || p.y > top) { continue; }
            let n = normalize(vec3f(p.x - cc.x, 0.0, p.z - cc.y));
            // spiral stripes, a painted cap, a green slime foot
            let ang = atan2(n.z, n.x);
            let stripe = step(0.5, fract(p.y * 1.6 + ang / TAU));
            var alb = mix(vec3f(0.85, 0.83, 0.78), select(vec3f(0.55, 0.05, 0.04), vec3f(0.08, 0.15, 0.45), h.z > 0.6), stripe);
            if (p.y > top - 0.25) { alb = vec3f(0.6, 0.45, 0.1); }
            alb = mix(alb, vec3f(0.05, 0.08, 0.04), smoothstep(0.6, 0.1, p.y));
            let sh = sunlit(p, l);
            let e = l.sun_c * saturate(dot(n, l.sun)) * sh + l.amb * 0.8 + lamp_light(p, n, l);
            best = vec4f(alb * 0.318 * e, t);
        }
    }
    return best;
}

fn shade_hit(h: Hit, rd: vec3f, l: Look, ctx: Ctx, lod: f32) -> vec3f {
    if (h.kind == 2) {
        return shade_wall(h.p, h.n, rd, palazzo(h.p.z, h.side), l, ctx, lod);
    }
    if (h.kind == 3 || h.kind == 4) {
        // Istrian stone, darker under the soffit
        let alb = vec3f(0.72, 0.7, 0.65) * (0.8 + 0.3 * noise_value2(h.p.xy * 3.0));
        let sh = sunlit(h.p, l);
        var e = l.sun_c * saturate(dot(h.n, l.sun)) * sh + l.amb * select(0.8, 0.35, h.kind == 4);
        e += lamp_light(h.p, h.n, l) + l.amb * 0.4 * select(0.0, 1.0, h.kind == 4);
        let courses = 0.9 + 0.1 * step(0.1, fract(h.p.y / 0.35));
        return alb * 0.318 * e * courses;
    }
    if (h.kind == 5) {
        // the palazzo across the end of the rio
        let pl = Pal(ZEND, 30.0, 15.5, col_hex(0xdcb088u), 991u);
        let q = vec3f(h.p.z, h.p.y, -h.p.x + 20.0);
        var c = shade_wall(vec3f(-CW, q.y, -q.z), vec3f(1.0, 0.0, 0.0), rd, Pal(20.0, 40.0, 15.5, col_hex(0xdcb088u), 991u), l, ctx, lod);
        let _u = pl.len;
        return c;
    }
    if (h.kind == 8) {
        // Istrian stone balustrade
        var alb = vec3f(0.78, 0.76, 0.7) * (0.85 + 0.2 * noise_value2(h.p.zy * 6.0));
        // round the colonnettes: darker toward their edges
        let pl = palazzo(h.p.z, h.side);
        let cf = fract(((fract((pl.z0 - h.p.z) / 2.4) - 0.5) * 2.4 + 0.75) / 0.19) - 0.5;
        alb *= 0.7 + 0.3 * sqrt(saturate(1.0 - sq(cf / 0.28)));
        let sh = sunlit(h.p, l);
        let e = l.sun_c * saturate(dot(h.n, l.sun)) * sh + l.amb * 0.7 + lamp_light(h.p, h.n, l) + l.sun_c * 0.04;
        return alb * 0.318 * e;
    }
    if (h.kind == 7) {
        let alb = vec3f(0.45, 0.3, 0.22);
        return alb * 0.318 * (l.sun_c * 0.4 + l.amb);
    }
    return vec3f(0.0);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let ro = vec3f(1.5, 3.0, 6.0);
    let cam = cam_look_at(ro, vec3f(-2.2, 6.0, -60.0), 0.0, 55.0);
    let rd = cam_ray(cam, p);
    let zoom = cam.zoom;
    let h = trace(ro, rd);
    var th = h.t;
    // the gondola
    var gh = vec2f(-1.0);
    let gb = gondola_bounds(ro, rd, ctx);
    if (gb.y > gb.x && gb.x < th) {
        gh = rm_march(ro, rd, gb.x, min(gb.y, th), steps(48.0, ctx), ctx);
    }
    var col: vec3f;
    if (gh.x > 0.0) {
        th = gh.x;
        let gp = ro + rd * th;
        let n = rm_normal(gp, th, ctx);
        var alb = vec3f(0.012);
        if (gh.y > 1.5 && gh.y < 2.5) {
            // striped jersey, straw hat
            alb = mix(vec3f(0.02), vec3f(0.6, 0.58, 0.55), step(0.5, fract(gp.y * 7.0)));
            if (gp.y > 2.0) { alb = vec3f(0.6, 0.5, 0.3); }
        }
        let sh = sunlit(gp, l);
        let e = l.sun_c * saturate(dot(n, l.sun)) * sh + l.amb * (0.5 + 0.5 * n.y) + lamp_light(gp, n, l);
        col = alb * 0.318 * e;
        // lacquer gloss on the hull
        col += l.amb * 0.25 * pow(1.0 - saturate(-dot(rd, n)), 4.0) * select(0.0, 1.0, gh.y < 1.5);
    } else if (h.kind == 1) {
        // the canal: green water mirroring everything
        let wp = h.p;
        let n = water_normal(wp.xz * 2.2, ctx.t * 0.7, 0.18, h.t * 2.2, ctx);
        var r = reflect(rd, n);
        r.y = abs(r.y);
        let hr = trace(wp + vec3f(0.0, 0.01, 0.0), r);
        var rc = venice_sky(r, l, ctx);
        let cp = campanile(wp, r, l, ctx, zoom);
        rc = rc * (1.0 - cp.w) + cp.xyz;
        if (hr.t < 1e8) {
            rc = shade_hit(hr, r, l, ctx, 0.05 + ctx.px * hr.t);
            rc = mix(rc, l.hazec, 1.0 - exp(-hr.t * 0.001 * l.haze));
        }
        // the gondola's dark reflection
        let grb = gondola_bounds(wp, r, ctx);
        if (grb.y > grb.x && grb.x < hr.t) {
            let gr = rm_march(wp, r, grb.x, min(grb.y, hr.t), steps(24.0, ctx), ctx);
            if (gr.x > 0.0) { rc = vec3f(0.01) * (l.amb * 3.0 + 0.1); }
        }
        let pr = poles(wp, r, min(hr.t, 200.0), l, ctx, zoom);
        if (pr.w < 1e8) { rc = pr.xyz; }
        rc += lamp_glow(wp, r, min(hr.t, 300.0), l, 1.8);
        let fres = water_fresnel(dot(-rd, n));
        let body = vec3f(0.03, 0.06, 0.045) * (l.amb * 2.0 + l.sun_c * 0.1) + vec3f(0.004, 0.008, 0.006);
        col = mix(body, rc, max(fres, 0.35));
    } else if (h.t < 1e8) {
        col = shade_hit(h, rd, l, ctx, ctx.px / zoom * h.t);
    } else {
        col = venice_sky(rd, l, ctx);
        let cp = campanile(ro, rd, l, ctx, zoom);
        col = col * (1.0 - cp.w) + cp.xyz;
        th = 5000.0;
    }
    let pl = poles(ro, rd, th, l, ctx, zoom);
    if (pl.w < th) { col = pl.xyz; th = pl.w; }
    if (th < 4000.0) { col = mix(col, l.hazec, 1.0 - exp(-th * 0.001 * l.haze)); }
    let ld = laundry(ro, rd, th, l, ctx, zoom);
    col = col * (1.0 - ld.w) + ld.xyz;
    col += lamp_glow(ro, rd, th, l, 1.0);
    return col * exp2(l.exposure);
}

// the lanterns themselves and their halos over the water
fn lamp_glow(ro: vec3f, rd: vec3f, tmax: f32, l: Look, spread: f32) -> vec3f {
    if (l.lamps <= 0.1) { return vec3f(0.0); }
    var g = vec3f(0.0);
    for (var i = 0; i < 14; i++) {
        let lp = wall_lamp(i);
        let w = lp - ro;
        let tc = dot(w, rd);
        if (tc < 0.0 || tc > tmax + 1.0) { continue; }
        let d2 = max(dot(w, w) - tc * tc, 0.0);
        let r = 0.14 * spread;
        g += vec3f(1.0, 0.72, 0.4) * (3.0 * exp(-d2 / (r * r)) + 0.06 / (1.0 + d2 / (0.5 * spread)));
    }
    return g * l.lamps;
}
