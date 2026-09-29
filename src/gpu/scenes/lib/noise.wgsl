// ---------------------------------------------------------------- noise
// Lattice noises on integer hashes. Value noise returns [0,1], gradient
// noise [-1,1]. fbm loops are literally bounded at 8 octaves.

fn noise_h2(c: vec2i) -> f32 { return hash_unorm(hash_pcg3(vec3u(bitcast<vec2u>(c), 0x68e31da4u)).x); }
fn noise_h3(c: vec3i) -> f32 { return hash_unorm(hash_pcg3(bitcast<vec3u>(c)).x); }
fn noise_g2(c: vec2i) -> vec2f {
    let a = noise_h2(c) * TAU;
    return vec2f(cos(a), sin(a));
}
fn noise_g3(c: vec3i) -> vec3f {
    let h = hash_pcg3(bitcast<vec3u>(c) ^ vec3u(0x1b873593u));
    let u = hash_unorm(h.x) * 2.0 - 1.0;
    let a = hash_unorm(h.y) * TAU;
    let r = sqrt(max(1.0 - u * u, 0.0));
    return vec3f(r * cos(a), r * sin(a), u);
}
fn noise_fade2(f: vec2f) -> vec2f { return f * f * f * (f * (f * 6.0 - 15.0) + 10.0); }
fn noise_fade3(f: vec3f) -> vec3f { return f * f * f * (f * (f * 6.0 - 15.0) + 10.0); }

fn noise_value2(p: vec2f) -> f32 {
    let i = vec2i(floor(p));
    let u = noise_fade2(fract(p));
    let a = noise_h2(i);
    let b = noise_h2(i + vec2i(1, 0));
    let c = noise_h2(i + vec2i(0, 1));
    let d = noise_h2(i + vec2i(1, 1));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
// value noise with analytic derivatives: (value in [0,1], d/dx, d/dy)
fn noise_value2_d(p: vec2f) -> vec3f {
    let i = vec2i(floor(p));
    let f = fract(p);
    let u = noise_fade2(f);
    let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);
    let a = noise_h2(i);
    let b = noise_h2(i + vec2i(1, 0));
    let c = noise_h2(i + vec2i(0, 1));
    let d = noise_h2(i + vec2i(1, 1));
    let k1 = b - a;
    let k2 = c - a;
    let k4 = a - b - c + d;
    return vec3f(a + k1 * u.x + k2 * u.y + k4 * u.x * u.y,
                 du.x * (k1 + k4 * u.y),
                 du.y * (k2 + k4 * u.x));
}
fn noise_value3(p: vec3f) -> f32 {
    let i = vec3i(floor(p));
    let u = noise_fade3(fract(p));
    let n000 = noise_h3(i);
    let n100 = noise_h3(i + vec3i(1, 0, 0));
    let n010 = noise_h3(i + vec3i(0, 1, 0));
    let n110 = noise_h3(i + vec3i(1, 1, 0));
    let n001 = noise_h3(i + vec3i(0, 0, 1));
    let n101 = noise_h3(i + vec3i(1, 0, 1));
    let n011 = noise_h3(i + vec3i(0, 1, 1));
    let n111 = noise_h3(i + vec3i(1, 1, 1));
    let x0 = mix(mix(n000, n100, u.x), mix(n010, n110, u.x), u.y);
    let x1 = mix(mix(n001, n101, u.x), mix(n011, n111, u.x), u.y);
    return mix(x0, x1, u.z);
}
fn noise_grad2(p: vec2f) -> f32 {
    let i = vec2i(floor(p));
    let f = fract(p);
    let u = noise_fade2(f);
    let a = dot(noise_g2(i), f);
    let b = dot(noise_g2(i + vec2i(1, 0)), f - vec2f(1.0, 0.0));
    let c = dot(noise_g2(i + vec2i(0, 1)), f - vec2f(0.0, 1.0));
    let d = dot(noise_g2(i + vec2i(1, 1)), f - vec2f(1.0, 1.0));
    return 1.41 * mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
fn noise_grad3(p: vec3f) -> f32 {
    let i = vec3i(floor(p));
    let f = fract(p);
    let u = noise_fade3(f);
    let n000 = dot(noise_g3(i), f);
    let n100 = dot(noise_g3(i + vec3i(1, 0, 0)), f - vec3f(1.0, 0.0, 0.0));
    let n010 = dot(noise_g3(i + vec3i(0, 1, 0)), f - vec3f(0.0, 1.0, 0.0));
    let n110 = dot(noise_g3(i + vec3i(1, 1, 0)), f - vec3f(1.0, 1.0, 0.0));
    let n001 = dot(noise_g3(i + vec3i(0, 0, 1)), f - vec3f(0.0, 0.0, 1.0));
    let n101 = dot(noise_g3(i + vec3i(1, 0, 1)), f - vec3f(1.0, 0.0, 1.0));
    let n011 = dot(noise_g3(i + vec3i(0, 1, 1)), f - vec3f(0.0, 1.0, 1.0));
    let n111 = dot(noise_g3(i + vec3i(1, 1, 1)), f - vec3f(1.0, 1.0, 1.0));
    let x0 = mix(mix(n000, n100, u.x), mix(n010, n110, u.x), u.y);
    let x1 = mix(mix(n001, n101, u.x), mix(n011, n111, u.x), u.y);
    return 1.15 * mix(x0, x1, u.z);
}
// Worley / cellular: (F1, F2) distances to the nearest feature points
fn noise_worley2(p: vec2f) -> vec2f {
    let i = vec2i(floor(p));
    let f = fract(p);
    var d = vec2f(8.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2i(x, y);
            let h = hash_cell2(i + o, 0x2c1b3c6du).xy;
            let r = vec2f(o) + h - f;
            let dd = dot(r, r);
            if (dd < d.x) { d = vec2f(dd, d.x); } else if (dd < d.y) { d.y = dd; }
        }
    }
    return sqrt(d);
}
fn noise_worley3(p: vec3f) -> vec2f {
    let i = vec3i(floor(p));
    let f = fract(p);
    var d = vec2f(8.0);
    for (var z = -1; z <= 1; z++) {
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                let o = vec3i(x, y, z);
                let h = hash_cell3(i + o, 0x297a2d39u).xyz;
                let r = vec3f(o) + h - f;
                let dd = dot(r, r);
                if (dd < d.x) { d = vec2f(dd, d.x); } else if (dd < d.y) { d.y = dd; }
            }
        }
    }
    return sqrt(d);
}
// fractal sums; octaves are clamped to 8
const NOISE_ROT: mat2x2f = mat2x2f(0.80, 0.60, -0.60, 0.80);
fn noise_fbm2(p: vec2f, oct: i32) -> f32 {
    var q = p;
    var a = 0.5;
    var s = 0.0;
    var n = 0.0;
    for (var i = 0; i < 8; i++) {
        if (i >= oct) { break; }
        s += a * noise_value2(q);
        n += a;
        q = NOISE_ROT * q * 2.03 + vec2f(17.1, 3.7);
        a *= 0.5;
    }
    return s / max(n, 1e-6);
}
fn noise_fbm3(p: vec3f, oct: i32) -> f32 {
    var q = p;
    var a = 0.5;
    var s = 0.0;
    var n = 0.0;
    for (var i = 0; i < 8; i++) {
        if (i >= oct) { break; }
        s += a * noise_value3(q);
        n += a;
        q = q * 2.02 + vec3f(11.3, 5.9, 7.7);
        a *= 0.5;
    }
    return s / max(n, 1e-6);
}
// signed gradient-noise fbm in about [-1, 1]
fn noise_sfbm2(p: vec2f, oct: i32) -> f32 {
    var q = p;
    var a = 0.5;
    var s = 0.0;
    for (var i = 0; i < 8; i++) {
        if (i >= oct) { break; }
        s += a * noise_grad2(q);
        q = NOISE_ROT * q * 2.01 + vec2f(3.1, 9.2);
        a *= 0.5;
    }
    return s;
}
fn noise_sfbm3(p: vec3f, oct: i32) -> f32 {
    var q = p;
    var a = 0.5;
    var s = 0.0;
    for (var i = 0; i < 8; i++) {
        if (i >= oct) { break; }
        s += a * noise_grad3(q);
        q = q * 2.01 + vec3f(3.1, 9.2, 1.7);
        a *= 0.5;
    }
    return s;
}
// sharp-crested ridges in [0,1] (mountain crests, lightning, cracks)
fn noise_ridged2(p: vec2f, oct: i32) -> f32 {
    var q = p;
    var a = 0.5;
    var s = 0.0;
    var n = 0.0;
    var prev = 1.0;
    for (var i = 0; i < 8; i++) {
        if (i >= oct) { break; }
        var r = 1.0 - abs(noise_grad2(q));
        r = r * r;
        s += a * r * prev;
        n += a;
        prev = r;
        q = NOISE_ROT * q * 2.07 + vec2f(1.7, 9.2);
        a *= 0.5;
    }
    return s / max(n, 1e-6);
}
// domain-warped fbm: organic, flowing shapes (smoke, marble, cloud decks)
fn noise_warp2(p: vec2f, amt: f32, oct: i32) -> f32 {
    let w = vec2f(noise_fbm2(p + vec2f(0.0, 0.0), 4), noise_fbm2(p + vec2f(5.2, 1.3), 4));
    return noise_fbm2(p + amt * (w - 0.5) * 2.0, oct);
}
// divergence-free 2D flow from the curl of gradient noise
fn noise_curl2(p: vec2f) -> vec2f {
    let e = 0.01;
    let n1 = noise_grad2(p + vec2f(0.0, e));
    let n2 = noise_grad2(p - vec2f(0.0, e));
    let n3 = noise_grad2(p + vec2f(e, 0.0));
    let n4 = noise_grad2(p - vec2f(e, 0.0));
    return vec2f(n1 - n2, n4 - n3) / (2.0 * e);
}
// Eroded terrain height with derivatives: (height ~[0,1], dh/dx, dh/dz).
// Octaves are damped where the slope is steep, which carves valleys and keeps
// ridges crisp (the "derivative fbm" trick).
fn noise_terrain(p: vec2f, oct: i32) -> vec3f {
    var q = p;
    var a = 0.5;
    var h = 0.0;
    var d = vec2f(0.0);
    var m = mat2x2f(1.0, 0.0, 0.0, 1.0);
    for (var i = 0; i < 8; i++) {
        if (i >= oct) { break; }
        let n = noise_value2_d(q);
        d += m * n.yz;
        h += a * n.x / (1.0 + dot(d, d));
        a *= 0.5;
        m = NOISE_ROT * m * 2.0;
        q = NOISE_ROT * q * 2.0 + vec2f(13.7, 5.1);
    }
    return vec3f(h, d);
}
