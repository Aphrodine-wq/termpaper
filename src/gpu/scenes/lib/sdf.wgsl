// ---------------------------------------------------------------- sdf
// Signed distance primitives (standard formulas; see Quilez's catalogue).
fn sdf_sphere(p: vec3f, r: f32) -> f32 { return length(p) - r; }
fn sdf_box(p: vec3f, b: vec3f) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}
fn sdf_round_box(p: vec3f, b: vec3f, r: f32) -> f32 { return sdf_box(p, b - vec3f(r)) - r; }
fn sdf_capsule(p: vec3f, a: vec3f, b: vec3f, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = saturate(dot(pa, ba) / dot(ba, ba));
    return length(pa - ba * h) - r;
}
// vertical cylinder centred at origin, half-height h, radius r
fn sdf_cyl_y(p: vec3f, h: f32, r: f32) -> f32 {
    let d = abs(vec2f(length(p.xz), p.y)) - vec2f(r, h);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2f(0.0)));
}
// vertical capped cone centred at origin: half-height h, radius r1 at
// y = -h and r2 at y = +h
fn sdf_cone_y(p: vec3f, h: f32, r1: f32, r2: f32) -> f32 {
    let q = vec2f(length(p.xz), p.y);
    let k1 = vec2f(r2, h);
    let k2 = vec2f(r2 - r1, 2.0 * h);
    let ca = vec2f(q.x - min(q.x, select(r2, r1, q.y < 0.0)), abs(q.y) - h);
    let cb = q - k1 + k2 * saturate(dot(k1 - q, k2) / dot(k2, k2));
    let s = select(1.0, -1.0, cb.x < 0.0 && ca.y < 0.0);
    return s * sqrt(min(dot(ca, ca), dot(cb, cb)));
}
fn sdf_torus(p: vec3f, big_r: f32, r: f32) -> f32 {
    let q = vec2f(length(p.xz) - big_r, p.y);
    return length(q) - r;
}
// approximate ellipsoid (bound-correct near the surface)
fn sdf_ellipsoid(p: vec3f, r: vec3f) -> f32 {
    let k0 = length(p / r);
    let k1 = length(p / (r * r));
    return k0 * (k0 - 1.0) / max(k1, 1e-6);
}
fn sdf_plane_y(p: vec3f, h: f32) -> f32 { return p.y - h; }
fn sdf2_circle(p: vec2f, r: f32) -> f32 { return length(p) - r; }
fn sdf2_box(p: vec2f, b: vec2f) -> f32 {
    let d = abs(p) - b;
    return length(max(d, vec2f(0.0))) + min(max(d.x, d.y), 0.0);
}
fn sdf2_round_box(p: vec2f, b: vec2f, r: f32) -> f32 { return sdf2_box(p, b - vec2f(r)) - r; }
fn sdf2_segment(p: vec2f, a: vec2f, b: vec2f) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = saturate(dot(pa, ba) / dot(ba, ba));
    return length(pa - ba * h);
}
// isoceles triangle pointing up: half-width w at the base (y=0), height h
fn sdf2_tri(p: vec2f, w: f32, h: f32) -> f32 {
    let q = vec2f(abs(p.x), p.y);
    let e = vec2f(-w, h);
    let t = saturate(dot(q - vec2f(w, 0.0), e) / dot(e, e));
    let d1 = length(q - vec2f(w, 0.0) - e * t);
    let d2 = select(1e9, abs(q.y), q.x <= w);
    let inside = q.y >= 0.0 && q.y <= h * (1.0 - q.x / w);
    return select(min(d1, d2), -min(d1, d2), inside);
}
fn op_smin(a: f32, b: f32, k: f32) -> f32 {
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}
fn op_smax(a: f32, b: f32, k: f32) -> f32 { return -op_smin(-a, -b, k); }
// union of (distance, material) pairs
fn op_umin(a: vec2f, b: vec2f) -> vec2f { return select(b, a, a.x < b.x); }
fn op_rep(p: vec3f, s: vec3f) -> vec3f { return p - s * round(p / s); }
// cell index of op_rep
fn op_rep_id(p: vec3f, s: vec3f) -> vec3f { return round(p / s); }
fn op_rep_lim(p: vec3f, s: f32, lim: vec3f) -> vec3f { return p - s * clamp(round(p / s), -lim, lim); }
fn op_rep2(p: vec2f, s: vec2f) -> vec2f { return p - s * round(p / s); }
