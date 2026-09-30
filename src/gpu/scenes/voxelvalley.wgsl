//! name: voxelvalley
//! title: Voxel Valley
//! category: games
//! tags: voxel, blocks, game, valley, cubes, sandbox
//! desc: a blocky voxel valley: cube terrain, square trees, a flat lake, a square sun over it all
//! themes: day, sunset, night, snow
//! params: trees, clouds
//! uses: camera, sky, stars, noise, fog
//! cost: heavy
//! fallback: sand
//! credits: original

// World units are cubes, y up. A smooth heightfield is quantised to whole
// cubes so the ground is stepped columns; the lake is a flat plane; trees are
// a 1x1 trunk with a 3x3x3 leaf cube; clouds are flat slabs high up. The
// ground is traversed cell by cell (a DDA), which gives exact axis-aligned
// faces, so every cube reads as flat-shaded quads with a dark seam.

struct Look {
    sun_elev: f32,
    sun_azim: f32,
    sun_c: vec3f,
    amb: vec3f,
    sky_top: vec3f,
    sky_hor: vec3f,
    glow: vec3f,
    disk: vec3f,
    disk_deg: f32,
    grass: vec3f,
    grass2: vec3f,
    dirt: vec3f,
    stone: vec3f,
    sand: vec3f,
    leaf: vec3f,
    trunk: vec3f,
    deep: vec3f,
    shallow: vec3f,
    refl: f32,
    wave: f32,
    cloud: vec3f,
    fog_d: f32,
    night: f32,
    snowy: f32,
    bare: f32,
}

fn look(theme: u32) -> Look {
    var l: Look;
    // day: noon sun, blue sky, saturated greens
    l.sun_elev = 58.0; l.sun_azim = -40.0;
    l.sun_c = vec3f(2.3, 2.1, 1.75);
    l.amb = vec3f(0.30, 0.40, 0.58);
    l.sky_top = vec3f(0.18, 0.42, 0.95);
    l.sky_hor = vec3f(0.62, 0.78, 0.96);
    l.glow = vec3f(1.0, 0.95, 0.8);
    l.disk = vec3f(9.0, 8.5, 6.5); l.disk_deg = 2.6;
    l.grass = col_hex(0x4fb52au); l.grass2 = col_hex(0x63c93cu);
    l.dirt = col_hex(0x7a5230u);
    l.stone = col_hex(0x8a8f96u);
    l.sand = col_hex(0xdcc98cu);
    l.leaf = col_hex(0x2f8f27u);
    l.trunk = col_hex(0x5a3b1fu);
    l.deep = vec3f(0.03, 0.12, 0.28); l.shallow = vec3f(0.18, 0.45, 0.50);
    l.refl = 1.0; l.wave = 1.0;
    l.cloud = vec3f(1.0, 1.0, 1.0);
    l.fog_d = 0.009;
    l.night = 0.0; l.snowy = 0.0; l.bare = 0.0;
    switch (theme) {
        case 1u: {
            // sunset: low orange sun from the left, purple sky, orange lake
            l.sun_elev = 7.0; l.sun_azim = -105.0;
            l.sun_c = vec3f(2.4, 1.15, 0.45);
            l.amb = vec3f(0.16, 0.12, 0.22);
            l.sky_top = vec3f(0.22, 0.10, 0.36);
            l.sky_hor = vec3f(0.95, 0.42, 0.18);
            l.glow = vec3f(1.2, 0.5, 0.15);
            l.disk = vec3f(8.0, 3.2, 0.9); l.disk_deg = 3.0;
            l.grass = col_hex(0x4a9a2au); l.grass2 = col_hex(0x5aac36u);
            l.deep = vec3f(0.06, 0.05, 0.10); l.shallow = vec3f(0.30, 0.20, 0.16);
            l.fog_d = 0.011;
        }
        case 2u: {
            // night: square moon, stars, black lake, torches on the shore
            l.sun_elev = 42.0; l.sun_azim = 30.0;
            l.sun_c = vec3f(0.10, 0.13, 0.24);
            l.amb = vec3f(0.018, 0.024, 0.05);
            l.sky_top = vec3f(0.006, 0.009, 0.03);
            l.sky_hor = vec3f(0.02, 0.03, 0.07);
            l.glow = vec3f(0.10, 0.12, 0.18);
            l.disk = vec3f(1.6, 1.7, 1.9); l.disk_deg = 2.2;
            l.deep = vec3f(0.002, 0.003, 0.006); l.shallow = vec3f(0.01, 0.015, 0.02);
            l.refl = 0.7; l.wave = 0.7;
            l.cloud = vec3f(0.06, 0.07, 0.10);
            l.fog_d = 0.008;
            l.night = 1.0;
        }
        case 3u: {
            // snow: white tops, grey sky, frozen pale lake, bare dark trees
            l.sun_elev = 30.0; l.sun_azim = -60.0;
            l.sun_c = vec3f(0.55, 0.56, 0.60);
            l.amb = vec3f(0.42, 0.45, 0.52);
            l.sky_top = vec3f(0.40, 0.44, 0.50);
            l.sky_hor = vec3f(0.62, 0.64, 0.68);
            l.glow = vec3f(0.3, 0.3, 0.3);
            l.disk = vec3f(1.4, 1.4, 1.4); l.disk_deg = 2.4;
            l.grass = col_hex(0xf2f5fau); l.grass2 = col_hex(0xe3e9f2u);
            l.dirt = col_hex(0x5a4536u);
            l.stone = col_hex(0x6e737au);
            l.sand = col_hex(0xd9d4c4u);
            l.trunk = col_hex(0x2c211au);
            l.deep = vec3f(0.62, 0.68, 0.74); l.shallow = vec3f(0.74, 0.78, 0.82);
            l.refl = 0.12; l.wave = 0.0;
            l.cloud = vec3f(0.55, 0.57, 0.60);
            l.fog_d = 0.012;
            l.snowy = 1.0; l.bare = 1.0;
        }
        default: {}
    }
    return l;
}

const WATER_Y: f32 = 0.45;
const CLOUD_Y: f32 = 30.0;

// ------------------------------------------------------------ world

// the smooth valley: a basin with the lake in it, hills rising all round
fn ground_h(xz: vec2f) -> f32 {
    let n = noise_fbm2(xz * 0.032 + vec2f(5.3, 2.7), 3);
    let z = xz.y;
    let basin = 22.0 * (1.0 - exp(-z * z / 1600.0)) - 6.0;
    // the low hill the camera stands on, at the south rim
    let d = xz - vec2f(0.0, 44.0);
    let hill = 9.0 * exp(-dot(d, d) / 260.0);
    return basin + hill + (n - 0.5) * 15.0 + 0.05 * abs(xz.x);
}

fn column_top(c: vec2f) -> f32 { return floor(ground_h(c + 0.5)); }

struct Tree { on: f32, cell: vec2f, base: f32 }

// one tree at most per 6x6 block, on grass cubes only
fn tree_of(block: vec2f, dens: f32) -> Tree {
    let h = hash_cell2(vec2i(block), 31u);
    let cell = block * 6.0 + 2.0 + floor(h.xy * 3.0);
    let base = column_top(cell);
    let grass = step(2.5, base) * step(base, 9.5);
    // keep the camera's hilltop clear so the view is not blocked by a canopy
    let clear = step(12.0, length(cell - vec2f(0.0, 42.0)));
    return Tree(step(h.z, dens) * grass * clear, cell, base);
}

// what stands in one cell: the ground column (to top), and a leaf interval
// kind: 0 plain column, 1 trunk above gnd, 2 torch cube above gnd
struct Cell { top: f32, gnd: f32, la: f32, lb: f32, kind: f32 }

fn cell_info(c: vec2f, dens: f32, l: Look) -> Cell {
    let gnd = column_top(c);
    var ci = Cell(gnd, gnd, 0.0, -1.0, 0.0);
    // Most columns contain no tree. Reject them before evaluating the
    // tree's terrain height (three noise octaves per column otherwise).
    let block = floor(c / 6.0);
    let hash = hash_cell2(vec2i(block), 31u);
    let tree_cell = block * 6.0 + 2.0 + floor(hash.xy * 3.0);
    let delta = abs(c - tree_cell);
    var tr = Tree(0.0, tree_cell, 0.0);
    if (hash.z <= dens && max(delta.x, delta.y) < 1.5) {
        tr = tree_of(block, dens);
    }
    if (tr.on > 0.5) {
        let d = c - tr.cell;
        if (max(abs(d.x), abs(d.y)) < 1.5 && l.bare < 0.5) {
            ci.la = tr.base + 3.0; ci.lb = tr.base + 6.0;
        }
        if (abs(d.x) + abs(d.y) < 0.5) {
            ci.top = tr.base + 3.0 + l.bare * 2.0;
            ci.kind = 1.0;
        }
    }
    if (l.night > 0.5 && gnd >= 1.0 && gnd <= 2.0) {
        let tb = floor(c / 9.0);
        let th = hash_cell2(vec2i(tb), 57u);
        let tc = tb * 9.0 + floor(th.xy * 9.0);
        if (abs(tc.x - c.x) + abs(tc.y - c.y) < 0.5) { ci.top = gnd + 1.0; ci.kind = 2.0; }
    }
    return ci;
}

struct Hit { t: f32, n: vec3f, what: f32, c: vec2f, ci: Cell }   // what: 0 miss, 1 column, 2 leaves

// cell-by-cell traversal of the stepped ground: exact faces, no acne
fn march(ro: vec3f, rd_in: vec3f, tmax: f32, n_iter: i32, dens: f32, l: Look) -> Hit {
    var rd = rd_in;
    rd.x = select(rd.x, 1e-4, abs(rd.x) < 1e-4);
    rd.z = select(rd.z, 1e-4, abs(rd.z) < 1e-4);
    var c = floor(ro.xz);
    let stp = sign(rd.xz);
    let inv = 1.0 / rd.xz;
    var tnext = ((c + max(stp, vec2f(0.0))) - ro.xz) * inv;
    let dt = abs(inv);
    var t_in = 0.0;
    var wall = vec3f(0.0, 1.0, 0.0);
    var miss = Hit(-1.0, vec3f(0.0, 1.0, 0.0), 0.0, c, Cell(0.0, 0.0, 0.0, -1.0, 0.0));
    for (var i = 0; i < 260; i++) {
        if (i >= n_iter) { break; }
        let t_out = min(tnext.x, tnext.y);
        let y_in = ro.y + rd.y * t_in;
        if (y_in > 31.0 && rd.y >= 0.0) { break; }
        let ci = cell_info(c, dens, l);
        let y_out = ro.y + rd.y * t_out;
        if (y_in <= ci.top) {
            if (i == 0) { break; }
            return Hit(t_in, wall, 1.0, c, ci);
        }
        if (y_out < ci.top) {
            return Hit((ci.top - ro.y) / rd.y, vec3f(0.0, 1.0, 0.0), 1.0, c, ci);
        }
        if (ci.lb > ci.la) {
            if (y_in >= ci.la && y_in <= ci.lb) {
                if (i > 0) { return Hit(t_in, wall, 2.0, c, ci); }
            } else if (rd.y < 0.0 && y_in > ci.lb && y_out < ci.lb) {
                return Hit((ci.lb - ro.y) / rd.y, vec3f(0.0, 1.0, 0.0), 2.0, c, ci);
            } else if (rd.y > 0.0 && y_in < ci.la && y_out > ci.la) {
                return Hit((ci.la - ro.y) / rd.y, vec3f(0.0, -1.0, 0.0), 2.0, c, ci);
            }
        }
        if (tnext.x < tnext.y) {
            c.x += stp.x; tnext.x += dt.x; wall = vec3f(-stp.x, 0.0, 0.0);
        } else {
            c.y += stp.y; tnext.y += dt.y; wall = vec3f(0.0, 0.0, -stp.y);
        }
        t_in = t_out;
        if (t_in > tmax) { break; }
    }
    miss.c = c;
    return miss;
}

// flat cuboid clouds drifting on a coarse grid: coverage at a point of the slab
fn puff_cover(xz: vec2f, t: f32, dens: f32) -> f32 {
    let q = xz - vec2f(0.3, 0.09) * t;
    let cs = 28.0;
    let cell = floor(q / cs);
    let h = hash_cell2(vec2i(cell), 91u);
    if (h.x >= dens) { return 0.0; }
    let ctr = cell * cs + cs * 0.5 + (h.yz - 0.5) * 8.0;
    let d = abs(q - ctr);
    let a = step(d.x, 5.0 + 7.0 * h.w) * step(d.y, 3.0 + 3.0 * h.x);
    let d2 = abs(q - ctr - vec2f(4.0 * (h.z - 0.5) * 2.0, 2.0 * (h.y - 0.5) * 2.0));
    let b = step(d2.x, 3.0 + 3.0 * h.y) * step(d2.y, 4.0 + 2.0 * h.w);
    return max(a, b);
}

// ------------------------------------------------------------ sky

// a square disc (the sun, or the moon) in the direction `s`
fn square_disc(rd: vec3f, s: vec3f, deg: f32, ctx: Ctx) -> f32 {
    let cs = dot(rd, s);
    if (cs < 0.5) { return 0.0; }
    let rt = normalize(cross(vec3f(0.0, 1.0, 0.0), s));
    let up = cross(s, rt);
    let u = dot(rd, rt) / cs;
    let v = dot(rd, up) / cs;
    let half = tan(radians(deg));
    let aa = ctx.px * 1.2;
    return 1.0 - sstep(half - aa, half + aa, max(abs(u), abs(v)));
}

fn dome(rd: vec3f, sun: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let up = saturate(rd.y);
    var col = mix(l.sky_hor, l.sky_top, pow(up, 0.6));
    let cs = saturate(dot(rd, sun));
    col += l.glow * (0.35 * pow(cs, 6.0) + 0.12 * pow(cs, 2.0)) * (1.0 - 0.6 * up);
    col += l.disk * square_disc(rd, sun, l.disk_deg, ctx);
    if (l.night > 0.5) {
        col += star_field(rd, 0.7, ctx) * sstep(-0.02, 0.15, rd.y);
    }
    return col;
}

// ------------------------------------------------------------ shading

// warm light from the torch cubes near the shore (night only)
fn torch_glow(pos: vec3f, n: vec3f, l: Look) -> vec3f {
    if (l.night < 0.5) { return vec3f(0.0); }
    let b0 = floor(pos.xz / 9.0);
    var acc = vec3f(0.0);
    for (var i = -1; i <= 1; i++) {
        for (var j = -1; j <= 1; j++) {
            let tb = b0 + vec2f(f32(i), f32(j));
            let th = hash_cell2(vec2i(tb), 57u);
            let tc = tb * 9.0 + floor(th.xy * 9.0);
            let g = column_top(tc);
            if (g < 1.0 || g > 2.0) { continue; }
            let lp = vec3f(tc.x + 0.5, g + 1.5, tc.y + 0.5);
            let dv = lp - pos;
            let d2 = dot(dv, dv);
            let nd = saturate(dot(n, dv * inverseSqrt(d2 + 1e-4)) + 0.3);
            acc += vec3f(1.0, 0.45, 0.12) * 6.0 * nd / (1.0 + d2);
        }
    }
    return acc;
}

fn shade(h: Hit, pos: vec3f, sun: vec3f, l: Look, k: vec4f, ctx: Ctx, t: f32) -> vec3f {
    let n = h.n;
    let ci = h.ci;
    var alb = l.grass;
    var emis = vec3f(0.0);
    let two = step(0.5, hash_cell2(vec2i(h.c), 5u).x);
    if (h.what > 1.5) {
        alb = mix(l.leaf, l.leaf * 1.18, two);
    } else if (ci.kind > 1.5 && pos.y > ci.gnd + 0.001) {
        alb = vec3f(0.35, 0.2, 0.08);
        emis = vec3f(3.0, 1.3, 0.3);
    } else if (ci.kind > 0.5 && pos.y > ci.gnd + 0.001) {
        alb = l.trunk;
    } else {
        let g = ci.gnd;
        let jitter = (hash_cell2(vec2i(h.c), 9u).y - 0.5) * 2.0;
        if (g < 2.5) {
            alb = mix(l.sand, l.sand * 0.92, two);
        } else if (g + jitter > 8.0) {
            alb = mix(l.stone, l.stone * 0.85, two);
            if (l.snowy > 0.5 && n.y > 0.5) { alb = mix(l.grass, l.grass2, two); }
        } else if (n.y > 0.5) {
            alb = mix(l.grass, l.grass2, two);
        } else {
            // dirt sides with the grass strip along the top row
            let strip = step(g - 0.22, pos.y);
            alb = mix(l.dirt * mix(1.0, 0.9, two), mix(l.grass, l.grass2, two) * 0.9, strip);
        }
    }
    // cube seams: a dark line where the face-local coordinates cross 0 / 1
    let f = fract(pos);
    var e = 1.0;
    if (abs(n.y) > 0.5) { e = min(min(f.x, 1.0 - f.x), min(f.z, 1.0 - f.z)); }
    else if (abs(n.x) > 0.5) { e = min(min(f.y, 1.0 - f.y), min(f.z, 1.0 - f.z)); }
    else { e = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y)); }
    let pw = t * ctx.px * 1.05;                       // one pixel in world units here
    let seam = (1.0 - sstep(0.0, 0.03 + 0.6 * pw, e)) * saturate(0.22 / (pw + 0.02));
    alb *= 1.0 - 0.28 * seam;

    // flat shading: face tint, sun with a cast shadow, sky ambient
    let tint = select(select(0.72, 0.84, abs(n.x) > 0.5), 1.0, n.y > 0.5);
    let tint2 = select(tint, 0.45, n.y < -0.5);
    var sh = 1.0;
    if (dot(n, sun) > 0.0) {
        let sro = pos + n * 0.003;
        let shadow_steps = select(select(10, 20, ctx.detail > 0u), 40, ctx.detail > 1u);
        let s = march(sro, sun, f32(shadow_steps), shadow_steps, k.x * 1.2, l);
        sh = select(1.0, 0.0, s.what > 0.5);
        // cloud shadows
        if (sun.y > 0.05) {
            let cp = sro + sun * ((CLOUD_Y - sro.y) / sun.y);
            sh *= 1.0 - 0.55 * puff_cover(cp.xz, ctx.t, k.y * 1.4);
        }
    }
    let dif = l.sun_c * saturate(dot(n, sun)) * sh;
    let amb = l.amb * (0.65 + 0.35 * n.y);
    var col = alb * (dif + amb) * tint2 + emis;
    col += alb * torch_glow(pos, n, l);
    return col;
}

fn lake(ro: vec3f, rd: vec3f, tw: f32, sun: vec3f, l: Look, ctx: Ctx) -> vec3f {
    let wp = ro + rd * tw;
    let wc = floor(wp.xz);
    let under = column_top(wc);
    let depth = WATER_Y - under;
    let hw = hash_cell2(vec2i(wc), 77u);
    let two = step(0.5, hw.w);
    let stripe = sin(wc.y * 0.35 + ctx.t * 0.45 + hw.x * 2.0);
    let n = normalize(vec3f((hw.x - 0.5) * 0.09 * l.wave + 0.04 * l.wave * stripe, 1.0, (hw.y - 0.5) * 0.09 * l.wave));
    let rr = reflect(rd, n);
    let body = mix(l.shallow, l.deep, saturate((depth - 0.45) / 3.5)) * mix(1.0, 0.9, two);
    let fr = 0.04 + 0.96 * pow(1.0 - saturate(-dot(rd, n)), 5.0);
    var refl = dome(vec3f(rr.x, abs(rr.y), rr.z), sun, l, ctx);
    var col = mix(body * (l.amb * 2.5 + l.sun_c * 0.25 * saturate(sun.y)), refl, saturate(fr * 1.4) * l.refl);
    col += body * torch_glow(wp, vec3f(0.0, 1.0, 0.0), l);
    return col;
}

// ------------------------------------------------------------ scene

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let k = ctx.params;
    let dens = k.x * 1.2;

    // the sun slowly climbs and wheels; the moon does the same at night
    let sun = sky_sun_dir(l.sun_elev + 4.0 * sin(t * 0.011), l.sun_azim + t * 0.04);

    // static camera on a hill at the south rim, looking north across the lake
    let base = vec3f(0.0, 0.0, 40.0);
    let hill = column_top(floor(base.xz));
    let ro = vec3f(0.0, max(18.0, hill + 4.0), 40.0) + cam_sway(t, 0.12, 3u);
    let cam = cam_look_at(ro, vec3f(0.0, 3.0, -60.0), 0.0, 54.0);
    let rd = cam_ray(cam, p);

    // the lake plane bounds the ground march
    var tw = -1.0;
    if (rd.y < -1e-4) { tw = (WATER_Y - ro.y) / rd.y; }
    var tmax = 175.0;
    if (tw > 0.0) { tmax = min(tmax, tw); }

    let h = march(ro, rd, tmax, 255, dens, l);
    var col = vec3f(0.0);
    var dist = 1e4;
    if (h.what > 0.5 && (tw < 0.0 || h.t < tw)) {
        let pos = ro + rd * h.t;
        col = shade(h, pos, sun, l, k, ctx, h.t);
        dist = h.t;
    } else if (tw > 0.0) {
        col = lake(ro, rd, tw, sun, l, ctx);
        dist = tw;
    } else {
        col = dome(rd, sun, l, ctx);
        // flat clouds overhead
        if (rd.y > 0.01) {
            let tc = (CLOUD_Y - ro.y) / rd.y;
            let cp = ro + rd * tc;
            let cov = puff_cover(cp.xz, t, k.y * 1.4);
            if (cov > 0.5) {
                let ccol = l.cloud * (l.amb * 1.6 + l.sun_c * 0.22 * saturate(sun.y + 0.2) + l.glow * 0.05);
                let fade = 1.0 - sstep(180.0, 420.0, tc);
                col = mix(col, ccol, fade);
                dist = tc * 0.5;
            }
        }
    }
    // distance haze into the horizon colour; far ground fades fully into it
    if (dist < 9e3) {
        let fogc = mix(l.sky_hor, l.glow * 0.5 + l.sky_hor * 0.5, 0.35 * pow(saturate(dot(rd, sun)), 4.0));
        var fc = fog_exp(col, fogc, dist, l.fog_d);
        fc = mix(fc, fogc, sstep(125.0, 172.0, dist));
        col = fc;
    }
    return max(col, vec3f(0.0));
}
