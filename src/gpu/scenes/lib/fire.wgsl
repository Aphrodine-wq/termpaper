// ---------------------------------------------------------------- fire
// Flames, embers and firelight.

// blackbody-ish ramp for flame temperature 0 (cool) .. 1 (white hot), linear
fn fire_temperature_color(temp: f32) -> vec3f {
    let x = saturate(temp);
    return col_ramp4(vec3f(0.0), vec3f(0.55, 0.05, 0.005), vec3f(1.6, 0.55, 0.08), vec3f(3.2, 2.6, 1.6), 0.35, 0.7, x) * (0.3 + 3.0 * x * x);
}
// Flame emission at p, flame base at origin, ~0.5 units tall.
// Returns (emitted rgb, opacity).
fn fire_flame(p: vec2f, t: f32, ctx: Ctx) -> vec4f {
    let q = vec2f(p.x, max(p.y, 0.0));
    let n = noise_fbm2(vec2f(q.x * 7.0, q.y * 5.0 - t * 2.4), 5);
    let n2 = noise_fbm2(vec2f(q.x * 13.0 + 3.0, q.y * 9.0 - t * 3.7), 4);
    let w = 0.16 * (1.0 - smoothstep(0.0, 0.62, q.y)) + 0.015;
    let x = abs(q.x + (n - 0.5) * 0.12 * q.y * 3.0) / w;
    let body = (1.0 - x) + (n2 - 0.5) * 0.9 - q.y * 1.1;
    let base = smoothstep(-0.03, 0.02, p.y);
    let temp = saturate(body * 1.3) * base;
    let a = smoothstep(0.02, 0.3, temp);
    return vec4f(fire_temperature_color(temp), a);
}
// firelight intensity multiplier ~[0.75, 1.15], slow enough for terminals
fn fire_light(t: f32) -> f32 {
    return 0.95 + 0.12 * noise_grad2(vec2f(t * 1.3, 0.0)) + 0.06 * noise_grad2(vec2f(t * 3.1, 7.0));
}
// rising embers: glowing specks above a fire at origin
fn fire_embers(p: vec2f, t: f32, density: f32, ctx: Ctx) -> vec3f {
    var c = vec3f(0.0);
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let s = 9.0 + fi * 6.0;
        var q = vec2f(p.x * s, p.y * s - t * (1.6 + fi * 0.5));
        q.x += 0.6 * sin(q.y * 0.7 + fi * 2.0 + t * 0.8);
        let cell = vec2i(floor(q));
        let h = hash_cell2(cell, 0xe3b0u + u32(i) * 13u);
        if (h.w < density) {
            let d = length(fract(q) - (0.25 + 0.5 * h.xy));
            let life = saturate(1.0 - p.y * (1.2 + h.z));
            let r = max(0.05, ctx.px * s * 0.8);
            c += fire_temperature_color(0.55 + 0.3 * h.z) * smoothstep(r, 0.0, d) * life * 0.8;
        }
    }
    return c * smoothstep(-0.02, 0.05, p.y);
}
