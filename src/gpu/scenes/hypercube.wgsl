//! name: hypercube
//! title: Hypercube Chamber
//! category: abstract
//! tags: tesseract, four dimensional, geometry, wireframe, portal
//! desc: A four-dimensional cube turns above a still geometric chamber and its luminous floor
//! themes: violet, ice, ember
//! params: rotation, lattice, glow
//! uses: camera, sdf
//! cost: medium
//! fallback: orbits
//! credits: original

fn vertex(bits: u32, rot: vec4f) -> vec3f {
    var v = vec4f(
        select(-1.0, 1.0, (bits & 1u) != 0u),
        select(-1.0, 1.0, (bits & 2u) != 0u),
        select(-1.0, 1.0, (bits & 4u) != 0u),
        select(-1.0, 1.0, (bits & 8u) != 0u));
    v = vec4f(v.x * rot.x - v.w * rot.y, v.y * rot.z - v.z * rot.w,
              v.y * rot.w + v.z * rot.z, v.x * rot.y + v.w * rot.x);
    let depth = 2.7 / (3.1 - v.w);
    return v.xyz * depth;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let shift = cam_depth_offset(ctx);
    let cam = cam_portal(cam_look_at(vec3f(shift.xy * 2.2, 5.2 + shift.z), vec3f(0.0), 0.0, 58.0), ctx, 5.2);
    let a = ctx.t * (0.017 + ctx.params.x * 0.045);
    let b = ctx.t * 0.013;
    let rot = vec4f(cos(a), sin(a), cos(b), sin(b));
    var vertices: array<vec3f, 16>;
    for (var i = 0u; i < 16u; i++) { vertices[i] = cam_project(cam, vertex(i, rot)); }
    var bg = mix(vec3f(0.004, 0.004, 0.014), vec3f(0.024, 0.017, 0.059), sstep(0.7, 0.0, length(p)));
    let palette = select(select(vec3f(0.23, 0.72, 1.2), vec3f(1.4, 0.30, 0.08), ctx.theme == 2u),
                         vec3f(0.72, 0.27, 1.3), ctx.theme == 0u);
    // Static chamber: receding joints, an octagonal dais and dark pillars.
    let floor_mask = sstep(-0.16, -0.40, p.y);
    let floor_scale = 1.0 / max(-p.y + 0.15, 0.08);
    let floor_joint = min(abs(fract(p.x * floor_scale * 1.2) - 0.5), abs(fract(floor_scale * 0.8) - 0.5));
    bg += palette * floor_mask * exp(-floor_joint * 70.0) * 0.035;
    let dais = abs(length(vec2f(p.x, p.y + 0.23) * vec2f(1.0, 2.7)) - 0.39);
    bg += palette * exp(-dais * 80.0) * 0.13;
    let arch = min(abs(abs(p.x) - 0.66), abs(p.y - 0.51));
    bg += palette * exp(-arch * 50.0) * 0.018;
    var glow = 0.0;
    var core = 0.0;
    for (var i = 0u; i < 16u; i++) {
        let pa = vertices[i];
        for (var j = 0u; j < 4u; j++) {
            if ((i & (1u << j)) != 0u) { continue; }
            let pb = vertices[i | (1u << j)];
            let d = sdf2_segment(p, pa.xy, pb.xy);
            let w = mix(0.0018, 0.004, saturate((pa.z + pb.z) * 0.09));
            let foreground = select(0.6, 1.3, pa.z + pb.z < 10.0);
            core += foreground * (1.0 - sstep(w, w + ctx.px * 1.5, d));
            glow += foreground * exp(-d * 55.0) * 0.025;
        }
    }
    let ring = abs(length(p) - 0.43);
    bg += palette * exp(-ring * 80.0) * ctx.params.y * 0.10;
    bg += palette * (core * 0.75 + glow * (1.0 + ctx.params.z * 2.0));
    bg += palette * exp(-length(p) * 13.0) * ctx.focus_pulse * ctx.reaction * 0.12;
    return bg;
}
