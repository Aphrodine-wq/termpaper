//! name: skyislands
//! title: Sky Islands
//! category: aerial
//! tags: floating islands, clouds, fantasy, bridges, ruins
//! desc: Stone islands and tiny towers hang above a still sea of cloud under changing skies
//! themes: sunrise, daylight, twilight
//! params: islands, clouds, sunlight
//! uses: camera, sdf, raymarch, noise
//! cost: heavy
//! fallback: clouds
//! credits: original

fn island_center(id: vec2f) -> vec3f {
    let h = hash_cell2(vec2i(id), 27u).x;
    return vec3f(id.x * 5.0 + (h - 0.5) * 1.6, h * 1.6 - 0.7, id.y * 7.0);
}

fn map(p: vec3f, ctx: Ctx) -> vec2f {
    let id = round(p.xz / vec2f(5.0, 7.0));
    let character = hash_cell2(vec2i(id), 48u);
    if (character.x < 0.27) { return vec2f(1.0, -1.0); }
    let center = island_center(id);
    let q = p - center;
    let size = 0.85 + ctx.params.x * 0.7 + hash_cell2(vec2i(id), 1u).x * 0.8;
    var hit = vec2f(sdf_ellipsoid(q - vec3f(0.0, -0.22, 0.0), vec3f(size, 0.44, size * 0.72)), 1.0);
    let root = sdf_cone_y(q - vec3f(0.0, -0.9, 0.0), 0.75, 0.05, size * 0.85);
    hit = op_umin(hit, vec2f(root, 2.0));
    if (character.y > 0.62) {
        let tower = sdf_box(q - vec3f(0.13, 0.48, 0.0), vec3f(0.19, 0.5, 0.2));
        hit = op_umin(hit, vec2f(tower, 3.0));
        let roof = sdf_cone_y(q - vec3f(0.13, 1.02, 0.0), 0.19, 0.34, 0.04);
        hit = op_umin(hit, vec2f(roof, 4.0));
    } else {
        let trunk = sdf_cyl_y(q - vec3f(-0.10, 0.22, 0.0), 0.24, 0.055);
        let crown = sdf_cone_y(q - vec3f(-0.10, 0.68, 0.0), 0.45, 0.40, 0.02);
        hit = op_umin(hit, vec2f(trunk, 5.0));
        hit = op_umin(hit, vec2f(crown, 6.0));
    }
    let shelf = sdf_box(q - vec3f(-0.16, -0.43, 0.08), vec3f(size * 0.66, 0.08, size * 0.43));
    hit = op_umin(hit, vec2f(shelf, 2.0));
    return hit;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let shift = cam_depth_offset(ctx) * 2.2;
    let ro = vec3f(0.6 + shift.x, 1.9 + shift.y, 11.5 + shift.z);
    let cam = cam_portal(cam_look_at(ro, vec3f(0.0, -0.15, -7.0), 0.0, 59.0), ctx, 15.0);
    let rd = cam_ray(cam, p);
    var sky = mix(vec3f(0.12, 0.30, 0.64), vec3f(0.38, 0.58, 0.74), sstep(0.6, -0.3, rd.y));
    var sun = vec3f(1.1, 0.64, 0.31);
    if (ctx.theme == 1u) { sky = mix(vec3f(0.16, 0.43, 0.80), vec3f(0.48, 0.75, 0.93), sstep(0.6, -0.3, rd.y)); sun = vec3f(1.0, 0.93, 0.67); }
    if (ctx.theme == 2u) { sky = mix(vec3f(0.035, 0.055, 0.19), vec3f(0.23, 0.14, 0.34), sstep(0.6, -0.3, rd.y)); sun = vec3f(0.63, 0.33, 0.77); }
    let sun_dir = normalize(vec3f(-0.6, 0.35, -1.0));
    sky += sun * pow(max(dot(rd, sun_dir), 0.0), 80.0) * (0.5 + ctx.params.z);
    let cloud = noise_fbm2(rd.xz * 5.0 / max(abs(rd.y + 0.15), 0.08) + vec2f(1.7, 4.3), 3);
    sky = mix(sky, sky + vec3f(0.14, 0.17, 0.20), sstep(0.38, 0.7, cloud) * sstep(0.0, -0.3, rd.y) * ctx.params.y);
    let hit = rm_march(ro, rd, 0.1, 55.0, steps(64.0, ctx), ctx);
    if (hit.x < 0.0) {
        // Two distant birds move inside a few pixels; the sky itself is still.
        let bird_x = -0.50 + fract(ctx.t * 0.010) * 1.0;
        let bird = min(sdf2_segment(p, vec2f(bird_x - 0.016, 0.16), vec2f(bird_x, 0.153)),
                       sdf2_segment(p, vec2f(bird_x, 0.153), vec2f(bird_x + 0.016, 0.16)));
        return sky * (1.0 - (1.0 - sstep(0.001, 0.001 + ctx.px, bird)) * 0.55);
    }
    let q = ro + rd * hit.x;
    let n = rm_normal(q, hit.x, ctx);
    let diffuse = max(dot(n, sun_dir), 0.0);
    var stone = vec3f(0.31, 0.30, 0.27);
    if (hit.y < 1.5) { stone = vec3f(0.21, 0.41, 0.23); }
    if (hit.y > 2.5) { stone = vec3f(0.49, 0.45, 0.36); }
    if (hit.y > 3.5) { stone = vec3f(0.36, 0.20, 0.15); }
    if (hit.y > 4.5) { stone = vec3f(0.15, 0.28, 0.15); }
    let strata = 0.86 + 0.14 * sin(q.y * 23.0 + sin(q.x * 4.0));
    let rim = pow(max(dot(n, -rd), 0.0), 3.0);
    let lit = stone * strata * (0.38 + diffuse * 0.85) + sun * (diffuse * 0.13 + rim * 0.06);
    return mix(lit, sky, 1.0 - exp(-hit.x * 0.024));
}
