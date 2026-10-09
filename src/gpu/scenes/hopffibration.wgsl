//! name: hopffibration
//! title: Hopf Fibration
//! category: abstract
//! tags: hopf, four dimensional, geometry, fibres, stereographic
//! desc: Linked circles from the three-sphere cast a slowly turning four-dimensional sculpture
//! themes: ultraviolet, pearl, gold
//! params: fibres, rotation, bloom
//! uses: camera, sdf
//! cost: heavy
//! fallback: orbits
//! credits: original

// A Hopf fibre is (z1,z2) = (a exp(i theta), b exp(i theta)) on S3,
// where a = cos(alpha/2) and b = sin(alpha/2) exp(i beta). The two rotations
// below mix x with w and y with z in R4. Stereographic projection from the
// north pole w=1 sends the fibre to a circle in R3; linked fibres remain
// linked through the projection.
fn fibre_point(id: u32, theta: f32, ca: f32, sa: f32, cb: f32, sb: f32) -> vec3f {
    let alpha = 0.36 + f32(id) * 0.17;
    let beta = f32(id) * 2.3999632;
    let a = cos(alpha * 0.5);
    let b = sin(alpha * 0.5);
    let z1 = vec2f(cos(theta), sin(theta)) * a;
    let z2 = vec2f(cos(theta + beta), sin(theta + beta)) * b;
    let x = z1.x * ca - z2.y * sa;
    let w = z1.x * sa + z2.y * ca;
    let y = z1.y * cb - z2.x * sb;
    let z = z1.y * sb + z2.x * cb;
    return vec3f(x, y, z) * (1.12 / max(1.0 - w, 0.06));
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let offset = cam_depth_offset(ctx);
    let cam = cam_portal(cam_look_at(vec3f(1.5 + offset.x, 0.85 + offset.y, 5.5 + offset.z), vec3f(0.0), 0.0, 62.0), ctx, 5.5);
    var base = vec3f(0.016, 0.011, 0.043);
    var a = vec3f(0.55, 0.27, 1.40);
    var b = vec3f(0.14, 0.76, 1.15);
    if (ctx.theme == 1u) {
        base = vec3f(0.015, 0.025, 0.038); a = vec3f(0.71, 0.90, 1.12); b = vec3f(0.26, 0.62, 0.81);
    }
    if (ctx.theme == 2u) {
        base = vec3f(0.039, 0.019, 0.011); a = vec3f(1.6, 0.81, 0.18); b = vec3f(0.83, 0.31, 0.12);
    }
    // Stationary framing and ground shadow; only the narrow fibres move.
    var col = base * (0.55 + 0.45 * sstep(0.8, 0.0, length(p)));
    let plinth = abs(length(vec2f(p.x, (p.y + 0.40) * 2.2)) - 0.43);
    col += b * exp(-plinth * 65.0) * 0.065;
    let sphere = abs(length(p) - 0.49);
    col += a * exp(-sphere * 75.0) * 0.025;
    let angle_a = sin(ctx.t * (0.006 + ctx.params.y * 0.012)) * 0.18;
    let angle_b = ctx.t * (0.004 + ctx.params.y * 0.010);
    let ca = cos(angle_a);
    let sa = sin(angle_a);
    let cb = cos(angle_b);
    let sb = sin(angle_b);
    let count = i32(4.0 + ctx.params.x * 4.0);
    for (var i = 0; i < 8; i++) {
        if (i >= count) { break; }
        let hue = f32(i) / f32(max(count - 1, 1));
        let ink = mix(a, b, hue);
        var min_d = 10.0;
        var nearest_depth = 10.0;
        let first = cam_project(cam, fibre_point(u32(i), 0.0, ca, sa, cb, sb));
        var prev = first;
        for (var j = 1; j <= 24; j++) {
            let theta = f32(j) * (TAU / 24.0);
            let next = cam_project(cam, fibre_point(u32(i), theta, ca, sa, cb, sb));
            let d = sdf2_segment(p, prev.xy, next.xy);
            if (d < min_d) { min_d = d; nearest_depth = min(prev.z, next.z); }
            prev = next;
        }
        let front = 0.62 + 0.75 * sstep(8.0, 4.0, nearest_depth);
        let core = 1.0 - sstep(0.0018, 0.0018 + ctx.px, min_d);
        let halo = exp(-min_d * 50.0) * (0.030 + ctx.params.z * 0.038);
        col += ink * front * (core * 0.77 + halo);
    }
    col += a * exp(-length(p) * 26.0) * ctx.focus_pulse * ctx.reaction * 0.09;
    return col;
}
