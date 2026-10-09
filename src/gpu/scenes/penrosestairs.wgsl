//! name: penrosestairs
//! title: Penrose Stairs
//! category: abstract
//! tags: impossible, staircase, escher, architecture, lantern
//! desc: Four flights keep climbing around an impossible square as one lantern traces the loop
//! themes: sandstone, moonlit, blueprint
//! params: steps, lantern, depth
//! uses: sdf
//! cost: medium
//! fallback: orbits
//! credits: original

struct Look {
    sky: vec3f,
    stone: vec3f,
    side: vec3f,
    edge: vec3f,
    lantern: vec3f,
}

fn look(theme: u32) -> Look {
    if (theme == 1u) {
        return Look(vec3f(0.012, 0.019, 0.045), vec3f(0.28, 0.35, 0.53),
                    vec3f(0.065, 0.105, 0.20), vec3f(0.41, 0.63, 0.82), vec3f(1.25, 0.70, 0.25));
    }
    if (theme == 2u) {
        return Look(vec3f(0.006, 0.035, 0.065), vec3f(0.035, 0.16, 0.23),
                    vec3f(0.009, 0.065, 0.11), vec3f(0.18, 0.84, 1.03), vec3f(0.47, 1.1, 0.80));
    }
    return Look(vec3f(0.075, 0.045, 0.035), vec3f(0.51, 0.39, 0.24),
                vec3f(0.20, 0.14, 0.10), vec3f(0.80, 0.59, 0.36), vec3f(1.6, 0.73, 0.19));
}

fn corner(id: i32) -> vec2f {
    switch (id) {
        case 0: { return vec2f(0.0, -0.29); }
        case 1: { return vec2f(0.43, 0.02); }
        case 2: { return vec2f(0.0, 0.31); }
        default: { return vec2f(-0.43, -0.01); }
    }
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    // The projecting camera is fixed. Different portal eyes shift near
    // columns more than the distant wall, so the room has real parallax.
    let far = p + ctx.portal * select(0.0, 0.10, ctx.portal_mode);
    let near = p + ctx.portal * select(0.0, 0.32, ctx.portal_mode);
    var col = l.sky * (0.76 + 0.24 * sstep(-0.7, 0.7, far.y));
    let wall_grid = min(abs(fract(far.x * 8.0) - 0.5), abs(fract(far.y * 8.0) - 0.5));
    col += l.edge * exp(-wall_grid * 85.0) * 0.012;
    // Nested doorway and back wall establish a still, deep chamber.
    let arch = abs(sdf2_round_box(far - vec2f(0.0, 0.11), vec2f(0.64, 0.46), 0.19));
    col += l.edge * exp(-arch * 47.0) * 0.11;
    let inset = sdf2_round_box(far - vec2f(0.0, 0.08), vec2f(0.50, 0.35), 0.11);
    col = mix(col, l.sky * 0.45, aa_fill(inset, ctx) * 0.36);
    // Four flights use cyclic overdraw: each appears above its predecessor
    // at one corner and below it at the next. No physical global height can
    // satisfy that loop, which is the Penrose illusion.
    for (var side = 0; side < 4; side++) {
        let a = corner(side);
        let b = corner((side + 1) % 4);
        let v = b - a;
        let t = clamp(dot(near - a, v) / dot(v, v), 0.0, 1.0);
        let nearest = mix(a, b, t);
        let d = length(near - nearest);
        let side_depth = 0.070 + ctx.params.z * 0.045;
        let shadow = sdf2_segment(near + vec2f(0.0, side_depth), a, b);
        col = mix(col, l.side * (0.85 - f32(side) * 0.07), (1.0 - sstep(0.060, 0.067, shadow)) * 0.85);
        let width = 0.069;
        let cover = 1.0 - sstep(width - ctx.px, width + ctx.px, d);
        let stair_count = floor(5.0 + ctx.params.x * 8.0);
        let tread = fract(t * stair_count);
        let riser = 1.0 - sstep(0.04, 0.17, tread);
        let surface = l.stone * (0.82 + 0.16 * t) + l.edge * riser * 0.19;
        col = mix(col, surface, cover);
        let rail_left = abs(d - width * 0.83);
        col += l.edge * exp(-rail_left * 110.0) * cover * 0.12;
        let pillar = sdf2_round_box(near - b + vec2f(0.0, side_depth * 0.35), vec2f(0.04, side_depth * 0.55), 0.012);
        col = mix(col, l.side, aa_fill(pillar, ctx) * 0.88);
    }
    // A single walker makes the loop's impossible direction legible without
    // animating the architecture or camera.
    let cycle = fract(ctx.t * 0.012);
    let index = min(i32(floor(cycle * 4.0)), 3);
    let u = fract(cycle * 4.0);
    let walker = mix(corner(index), corner((index + 1) % 4), u) + vec2f(0.0, 0.040);
    let lamp_d = length(near - walker);
    col += l.lantern * exp(-lamp_d * 31.0) * (0.20 + ctx.params.y * 0.25);
    col = mix(col, l.lantern * 1.5, 1.0 - sstep(0.007, 0.007 + ctx.px, lamp_d));
    let halo = exp(-length(near - ctx.focus) * 26.0) * ctx.focus_pulse * ctx.reaction;
    col += l.edge * halo * 0.11;
    return col;
}
