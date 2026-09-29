// ---------------------------------------------------------------- stars
// Stars on the celestial sphere. Brightness follows a steep power law (a few
// bright stars, many faint); colours span blackbody temperatures. No
// twinkle: flicker costs terminal bandwidth for little gain.

fn star_layer(rd: vec3f, scale: f32, density: f32, ctx: Ctx, salt: u32) -> vec3f {
    let q = rd * scale;
    let c = vec3i(floor(q));
    let f = q - floor(q);
    var col = vec3f(0.0);
    let h = hash_cell3(c, salt);
    // density 1 ≈ one star in ten cells: a real sky is mostly black. Coarse
    // (terminal) resolutions keep only the brighter ones, or the sky fills
    // with pixel-sized blobs.
    let res_keep = clamp(0.0035 / max(ctx.px, 1e-5), 0.25, 1.0);
    if (h.w < density * 0.1 * res_keep) {
        let pos = vec3f(0.2) + 0.6 * h.xyz;
        let d = length(f - pos) / scale;
        let size = max(ctx.px * 0.9, 0.0004);
        let b = pow(hash_unorm(hash_u3(bitcast<vec3u>(c) ^ vec3u(salt, 0x9e3779b9u, 7u))), 12.0);
        let temp = mix(3200.0, 11000.0, hash_f(hash_u(bitcast<u32>(c.z)) ^ salt ^ 0x51ed27u));
        let tint = mix(vec3f(1.0), col_kelvin(temp), 0.55);
        col = tint * (0.015 + 3.0 * b) * exp(-d * d / (size * size));
    }
    return col;
}
// point stars; density ~0.3-1.0
fn star_field(rd: vec3f, density: f32, ctx: Ctx) -> vec3f {
    var c = star_layer(rd, 90.0, density * 0.9, ctx, 0x1234567u);
    c += star_layer(rd, 180.0, density * 0.6, ctx, 0x7654321u);
    c += star_layer(rd, 320.0, density * 0.4, ctx, 0x0badf00du);
    return c;
}
// the Milky Way: a band around the great circle perpendicular to `pole`,
// with a bright core toward `core` and dark dust lanes
fn star_milky_way(rd: vec3f, pole: vec3f, core: vec3f, ctx: Ctx) -> vec3f {
    let lat = dot(rd, pole);
    let band = exp(-lat * lat * 38.0);
    let wide = exp(-lat * lat * 9.0);
    let toward = saturate(dot(rd, core) * 0.5 + 0.5);
    let bulge = pow(toward, 6.0) * exp(-lat * lat * 14.0);
    let n = noise_fbm3(rd * 7.0, 6);
    let dust = smoothstep(0.42, 0.62, noise_fbm3(rd * 13.0 + vec3f(3.0), 6)) * exp(-lat * lat * 120.0);
    let clumps = pow(noise_fbm3(rd * 30.0 + vec3f(9.0), 4), 3.0);
    var c = vec3f(0.55, 0.6, 0.75) * (band * (0.4 + 0.8 * n) + wide * 0.12) * (0.4 + 0.8 * toward);
    c += vec3f(0.95, 0.78, 0.58) * bulge * (0.6 + n);
    c += vec3f(0.8, 0.8, 0.9) * clumps * band * 0.9;
    c *= 1.0 - 0.85 * dust;
    c += star_layer(rd, 600.0, 0.55 * band, ctx, 0x3141592u) * 0.6;
    return c * 0.035;
}
// rotate a direction about the celestial pole (latitude in degrees) by t
// seconds of sidereal time × `rate` (1 = real time; 60 for visible motion)
fn star_rotate(rd: vec3f, lat_deg: f32, t: f32) -> vec3f {
    let lat = radians(lat_deg);
    let axis = normalize(vec3f(0.0, sin(lat), -cos(lat)));
    let a = t * TAU / 86164.0;
    let c = cos(a);
    let s = sin(a);
    return rd * c + cross(axis, rd) * s + axis * dot(axis, rd) * (1.0 - c);
}
