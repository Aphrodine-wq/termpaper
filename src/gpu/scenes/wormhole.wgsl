//! name: wormhole
//! title: Wormhole Mouth
//! category: space
//! tags: wormhole, nebula, dimensional, portal, cosmic
//! desc: A fixed view of a luminous aperture, with a small vortex turning far inside its throat
//! themes: aurora, ember, ghost
//! params: spin, aperture, haze
//! uses: camera, noise, sdf
//! cost: medium
//! fallback: starfield
//! credits: original

struct Look {
    void: vec3f,
    sky: vec3f,
    rim: vec3f,
    core: vec3f,
    accent: vec3f,
}

fn look(theme: u32) -> Look {
    if (theme == 1u) {
        return Look(vec3f(0.011, 0.003, 0.007), vec3f(0.050, 0.012, 0.026),
                    vec3f(1.9, 0.51, 0.11), vec3f(0.96, 0.14, 0.08), vec3f(0.36, 0.045, 0.11));
    }
    if (theme == 2u) {
        return Look(vec3f(0.004, 0.009, 0.018), vec3f(0.016, 0.028, 0.043),
                    vec3f(0.76, 0.93, 1.15), vec3f(0.38, 0.67, 0.92), vec3f(0.13, 0.21, 0.30));
    }
    return Look(vec3f(0.005, 0.005, 0.021), vec3f(0.028, 0.017, 0.070),
                vec3f(0.45, 1.12, 1.64), vec3f(0.24, 0.70, 1.31), vec3f(0.29, 0.13, 0.67));
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    // The mouth, star field and outer nebula remain still. Only a small disc
    // well inside the mouth turns, so most terminal cells are unchanged.
    let offset = cam_depth_offset(ctx).xy * 0.3;
    let center = vec2f(0.03, 0.015) + offset;
    let q = p - center;
    let r = length(q);
    let angle = atan2(q.y, q.x);
    let radius = 0.27 + 0.09 * ctx.params.y;
    let outer = abs(r - radius);
    let inner = abs(r - radius * 0.72);
    let stars_cell = vec2i(floor(p * 84.0));
    let stars = hash_cell2(stars_cell, 19u);
    let star = step(0.992, stars.x) * exp(-length(fract(p * 84.0) - stars.yz) * 18.0);
    let fixed_cloud = noise_fbm2(p * 2.2 + vec2f(4.0, 8.0), 3);
    var col = mix(l.void, l.sky, sstep(0.0, 1.5, length(p)))
        + l.accent * (fixed_cloud * fixed_cloud) * (0.08 + 0.14 * ctx.params.z);
    col += vec3f(0.74, 0.83, 1.0) * star * 0.75;
    // A dark silhouette and several luminous static rims make a deep frame.
    let mask = 1.0 - sstep(radius - ctx.px, radius + ctx.px, r);
    col = mix(col, l.void * 0.16, mask);
    col += l.rim * exp(-outer * 70.0) * 0.53;
    col += l.accent * exp(-inner * 85.0) * 0.33;
    col += l.rim * exp(-outer * 17.0) * 0.10;
    let lens = exp(-abs(r - radius * 1.21) * 120.0);
    col += l.core * lens * 0.20;
    // The moving material is confined to the innermost ~8% of the frame.
    let throat_r = radius * 0.50;
    if (r < throat_r + ctx.px * 2.0) {
        let depth = 0.034 / max(r, 0.006);
        let turn = ctx.t * (0.18 + ctx.params.x * 0.42);
        let spiral = angle * 5.0 + depth * 3.6 - turn;
        let threads = pow(max(sin(spiral), 0.0), 9.0);
        let rings = pow(max(cos(depth * 13.0), 0.0), 12.0);
        let core = l.core * (0.14 + threads * 0.65 + rings * 0.21);
        let fade = 1.0 - sstep(throat_r - ctx.px, throat_r + ctx.px, r);
        col = mix(col, core, fade);
        col += l.rim * exp(-r * 40.0) * 0.22 * fade;
    }
    // Focus glints on the ring only; there is no generic full-frame flash.
    let focus_angle = atan2(ctx.focus.y - center.y, ctx.focus.x - center.x);
    col += l.rim * exp(-outer * 58.0) * pow(max(cos(angle - focus_angle), 0.0), 24.0)
        * ctx.focus_pulse * ctx.reaction * 0.21;
    return col;
}
