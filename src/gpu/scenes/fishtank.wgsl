//! name: fishtank
//! title: Fish Tank Wall
//! category: coast
//! tags: aquarium, fish, glass, reef, underwater, shared world
//! desc: A single glass aquarium spans the desk while fish cross between its monitor windows
//! themes: reef, twilight, moonlit
//! params: fish, plants, glints
//! uses: noise, camera, sdf
//! cost: medium
//! fallback: aquarium
//! credits: original

struct Look {
    top: vec3f,
    deep: vec3f,
    sand: vec3f,
    plant: vec3f,
    coral: vec3f,
    fish: vec3f,
    gleam: vec3f,
}

fn look(theme: u32) -> Look {
    if (theme == 1u) {
        return Look(vec3f(0.035, 0.23, 0.29), vec3f(0.013, 0.065, 0.13),
                    vec3f(0.20, 0.19, 0.17), vec3f(0.026, 0.20, 0.16),
                    vec3f(0.31, 0.15, 0.21), vec3f(1.0, 0.49, 0.28), vec3f(0.24, 0.75, 0.79));
    }
    if (theme == 2u) {
        return Look(vec3f(0.017, 0.055, 0.14), vec3f(0.006, 0.020, 0.062),
                    vec3f(0.11, 0.14, 0.22), vec3f(0.016, 0.12, 0.19),
                    vec3f(0.14, 0.19, 0.35), vec3f(0.34, 0.86, 1.15), vec3f(0.32, 0.62, 1.3));
    }
    return Look(vec3f(0.10, 0.48, 0.60), vec3f(0.035, 0.19, 0.28),
                vec3f(0.45, 0.38, 0.26), vec3f(0.04, 0.29, 0.19),
                vec3f(0.62, 0.28, 0.18), vec3f(1.3, 0.75, 0.24), vec3f(0.58, 1.0, 0.91));
}

fn plant(q: vec2f, root: vec2f, height: f32, bend: f32, ctx: Ctx) -> f32 {
    let y = q.y - root.y;
    if (y < 0.0 || y > height) { return 0.0; }
    let center = root.x + bend * y * y;
    let stem = 1.0 - sstep(0.004, 0.004 + ctx.px, abs(q.x - center));
    let leaf_y = fract(y * 13.0);
    let leaf = exp(-abs(leaf_y - 0.5) * 12.0)
        * (1.0 - sstep(0.0, 0.045, abs(q.x - center - (select(-1.0, 1.0, fract(y * 6.5) > 0.5)) * 0.024)));
    return max(stem, leaf * 0.75);
}

fn fish_color(q: vec2f, center: vec2f, size: f32, dir: f32, tint: vec3f, ctx: Ctx) -> vec4f {
    let f = vec2f((q.x - center.x) * dir, q.y - center.y) / size;
    let body = length(f / vec2f(1.0, 0.58)) - 1.0;
    let tail = sdf2_tri(rot2(-1.5707963) * (f - vec2f(-1.13, 0.0)), 0.72, 0.72);
    let fin = sdf2_tri(f - vec2f(0.05, 0.31), 0.25, 0.38);
    let silhouette = min(body, min(tail, fin));
    let alpha = aa_fill_w(silhouette, ctx.px / size);
    let stripe = 0.5 + 0.5 * sin(f.x * 8.0);
    let shade = 0.60 + 0.35 * max(f.y, 0.0) + 0.12 * stripe;
    let eye = 1.0 - sstep(0.045, 0.10, length(f - vec2f(0.54, 0.12)));
    let c = mix(tint * shade, vec3f(0.005), eye);
    return vec4f(c, alpha);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let q = p - cam_depth_offset(ctx).xy * 0.25;
    let far = q + ctx.portal * select(0.0, 0.12, ctx.portal_mode);
    let near = q + ctx.portal * select(0.0, 0.34, ctx.portal_mode);
    let floor_y = -0.38;
    var col = mix(l.deep, l.top, sstep(-0.55, 0.65, q.y));
    // Fixed light shafts, a distant reef and caustics give the glass depth.
    let shafts = pow(max(sin(far.x * 8.0 - far.y * 2.0 + 0.8), 0.0), 15.0);
    col += l.gleam * shafts * sstep(-0.15, 0.60, far.y) * 0.075;
    let distant_reef = -0.26 + noise_value2(vec2f(far.x * 6.0, 2.0)) * 0.10;
    col = mix(col, l.deep * 0.72, (1.0 - sstep(distant_reef - 0.02, distant_reef + 0.02, far.y)) * 0.28);
    let caustic = pow(max(sin(q.x * 31.0 + sin(q.y * 11.0) * 1.7), 0.0), 16.0);
    col += l.gleam * caustic * (0.025 + ctx.params.z * 0.030) * sstep(0.40, -0.25, q.y);
    let seabed = floor_y + noise_value2(vec2f(q.x * 5.0, 3.0)) * 0.035;
    let sand_mask = 1.0 - sstep(seabed - ctx.px, seabed + ctx.px, q.y);
    let sand_grain = noise_value2(q * 32.0) * 0.17;
    col = mix(col, l.sand * (0.80 + sand_grain), sand_mask);
    for (var i = 0; i < 12; i++) {
        let id = i32(i);
        let seed = hash_cell2(vec2i(id, 3), 11u);
        if (seed.w < 0.19) { continue; }
        let x = (f32(i) - 5.5) * 0.27 + (seed.x - 0.5) * 0.17;
        let root = vec2f(x, floor_y + 0.005);
        let h = (0.13 + seed.y * 0.28) * (0.45 + ctx.params.y);
        let green = plant(near, root, h, (seed.z - 0.5) * 1.2, ctx);
        col = mix(col, mix(l.plant, l.coral, seed.w * 0.27), green * 0.85);
        let rock = sdf2_round_box(near - root - vec2f(0.04, 0.015), vec2f(0.050 + seed.y * 0.035, 0.027), 0.018);
        col = mix(col, l.coral * (0.66 + seed.z * 0.25), aa_fill(rock, ctx) * 0.9);
    }
    // Individual fish occupy little of the frame. Their paths cross monitor
    // seams because positions are in the one shared composition space.
    for (var i = 0; i < 12; i++) {
        if (f32(i) >= 3.0 + ctx.params.x * 9.0) { break; }
        let seed = hash_cell2(vec2i(i, 9), 31u);
        let size = 0.024 + seed.x * 0.034;
        let direction = select(-1.0, 1.0, seed.y > 0.5);
        let span = 2.0 * (ctx.half.x + 0.22);
        let travel = ctx.t * (0.015 + seed.z * 0.025) * direction;
        let x = -ctx.half.x - 0.11 + fmod_pos(seed.w * span + travel, span);
        let y = -0.17 + seed.y * 0.48 + sin(ctx.t * 0.3 + f32(i) * 2.1) * 0.012;
        let tint = mix(l.fish, l.gleam, seed.x * 0.48);
        let fish = fish_color(q, vec2f(x, y), size, direction, tint, ctx);
        col = mix(col, fish.xyz, fish.w);
    }
    // One foreground coral head, with branches that occlude passing fish.
    let coral_base = near - vec2f(0.62, -0.34);
    let coral_stem = sdf2_segment(coral_base, vec2f(0.0), vec2f(0.0, 0.23));
    let coral_left = sdf2_segment(coral_base, vec2f(0.0, 0.10), vec2f(-0.10, 0.20));
    let coral_right = sdf2_segment(coral_base, vec2f(0.0, 0.12), vec2f(0.12, 0.26));
    let coral = min(coral_stem, min(coral_left, coral_right));
    col = mix(col, l.coral * 1.4, 1.0 - sstep(0.010, 0.010 + ctx.px, coral));
    let glint = exp(-length(q - ctx.focus) * 34.0) * ctx.focus_pulse * ctx.reaction;
    col += l.gleam * glint * 0.16;
    // The front glass has stationary narrow highlights.
    col += l.gleam * (exp(-abs(q.x + ctx.half.x - 0.025) * 95.0)
        + exp(-abs(q.x - ctx.half.x + 0.028) * 110.0)) * 0.015;
    return col;
}
