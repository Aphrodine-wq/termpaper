// ---------------------------------------------------------------- hash
// Integer hashes (PCG family, Jarzynski & Olano 2020). Never fract(sin()):
// that varies between GPUs and drivers, and panes of a wall must agree.

fn hash_u(x: u32) -> u32 {
    let v = x * 747796405u + 2891336453u;
    let w = ((v >> ((v >> 28u) + 4u)) ^ v) * 277803737u;
    return (w >> 22u) ^ w;
}
fn hash_pcg3(a: vec3u) -> vec3u {
    var v = a * 1664525u + 1013904223u;
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    v = v ^ (v >> vec3u(16u));
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    return v;
}
fn hash_pcg4(a: vec4u) -> vec4u {
    var v = a * 1664525u + 1013904223u;
    v.x += v.y * v.w; v.y += v.z * v.x; v.z += v.x * v.y; v.w += v.y * v.z;
    v = v ^ (v >> vec4u(16u));
    v.x += v.y * v.w; v.y += v.z * v.x; v.z += v.x * v.y; v.w += v.y * v.z;
    return v;
}
fn hash_u2(v: vec2u) -> u32 { return hash_pcg3(vec3u(v, 0x9e3779b9u)).x; }
fn hash_u3(v: vec3u) -> u32 { return hash_pcg3(v).x; }
// u32 → [0, 1)
fn hash_unorm(h: u32) -> f32 { return f32(h >> 8u) * (1.0 / 16777216.0); }
fn hash_f(x: u32) -> f32 { return hash_unorm(hash_u(x)); }
// four independent [0,1) values for a 2D lattice cell
fn hash_cell2(c: vec2i, salt: u32) -> vec4f {
    let h = hash_pcg4(vec4u(bitcast<vec2u>(c), salt, 0x85ebca6bu));
    return vec4f(h >> vec4u(8u)) * (1.0 / 16777216.0);
}
fn hash_cell3(c: vec3i, salt: u32) -> vec4f {
    let h = hash_pcg4(vec4u(bitcast<vec3u>(c), salt));
    return vec4f(h >> vec4u(8u)) * (1.0 / 16777216.0);
}
// like hash_cell2, but different for each launch seed (still the same on
// every pane of a wall)
fn hash_seeded2(c: vec2i, salt: u32, ctx: Ctx) -> vec4f {
    return hash_cell2(c, salt ^ hash_u(ctx.seed.x ^ (ctx.seed.y * 0x27d4eb2du)));
}
// Closed-form random events: time is cut into `period`-second slots; returns
// (random [0,1) for this slot, phase [0,1) inside it, slot index). Use it for
// lightning, meteors, passing cars — anything that must look random yet be
// identical on every pane at the same time.
fn hash_event(t: f32, period: f32, salt: u32) -> vec3f {
    let k = floor(t / period);
    let r = hash_f((bitcast<u32>(i32(k)) * 0x9e3779b9u) ^ salt);
    return vec3f(r, t / period - k, k);
}
