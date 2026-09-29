// ---------------------------------------------------------------- game
// Closed-form motion for stateless games. A Studio scene keeps no state, so a
// ball, a paddle or a marching alien is a function of t alone: triangle waves
// bounce, sawtooth waves wrap, and "the ball will land there" comes from the
// same wave evaluated at the time of the next bounce.

// a coordinate bouncing between lo and hi at speed v (p units per second),
// `phase` in p units offsets where it starts
fn game_bounce(t: f32, lo: f32, hi: f32, v: f32, phase: f32) -> f32 {
    let span = max(hi - lo, 1e-5);
    let x = (t * v + phase) / (2.0 * span);
    return lo + span * (1.0 - abs(2.0 * fract(x) - 1.0));
}
// where the bounce is in its cycle: (cycle index, position in it 0..1);
// 0..0.5 rises lo → hi, 0.5..1 falls back
fn game_bounce_phase(t: f32, lo: f32, hi: f32, v: f32, phase: f32) -> vec2f {
    let span = max(hi - lo, 1e-5);
    let x = (t * v + phase) / (2.0 * span);
    return vec2f(floor(x), fract(x));
}
// the time the bounce next touches hi (if `at_hi`) or lo after t
fn game_next_bounce(t: f32, lo: f32, hi: f32, v: f32, phase: f32, at_hi: bool) -> f32 {
    let span = max(hi - lo, 1e-5);
    let period = 2.0 * span / max(v, 1e-5);
    let ph = game_bounce_phase(t, lo, hi, v, phase);
    let goal = select(1.0, 0.5, at_hi);
    var u = goal - ph.y;
    if (u <= 0.0) { u += 1.0; }
    return t + u * period;
}
// a coordinate wrapping from lo to hi at speed v (sawtooth), phase in p units
fn game_wrap(t: f32, lo: f32, hi: f32, v: f32, phase: f32) -> f32 {
    let span = max(hi - lo, 1e-5);
    return lo + fract((t * v + phase) / span) * span;
}
// eased move from a to b over [t0, t0 + dur]
fn game_ease_to(a: f32, b: f32, t: f32, t0: f32, dur: f32) -> f32 {
    let u = saturate((t - t0) / max(dur, 1e-5));
    return mix(a, b, u * u * (3.0 - 2.0 * u));
}
// discrete ticks: the tick index at t and the fraction into it
fn game_tick(t: f32, hz: f32) -> vec2f {
    let x = t * hz;
    return vec2f(floor(x), fract(x));
}
// square-wave blink, `duty` of the period on
fn game_blink(t: f32, hz: f32, duty: f32) -> f32 {
    return step(fract(t * hz), duty);
}
// a grid: cell index (x, y) and position inside the cell (0..1, y up) for a
// grid whose bottom-left corner is `origin` with `cell` p units per cell
fn game_grid(p: vec2f, origin: vec2f, cell: f32) -> vec4f {
    let q = (p - origin) / max(cell, 1e-6);
    let c = floor(q);
    return vec4f(c, q - c);
}
// a score that climbs through the session: hashed increments, never resets
fn game_score(t: f32, per_sec: f32, salt: u32) -> u32 {
    let k = floor(t * per_sec);
    return u32(k) * 10u + (hash_u(bitcast<u32>(i32(k)) ^ salt) % 10u);
}
// a crisp pixel-art rectangle: 1 inside, soft only within `aa` of the edge
fn game_rect(p: vec2f, c: vec2f, h: vec2f, aa: f32) -> f32 {
    let d = abs(p - c) - h;
    let m = max(d.x, d.y);
    return saturate(0.5 - m / max(aa, 1e-6));
}
// a CRT-style scanline darkening, gentle: rows of `rows` per unit
fn game_scanlines(p: vec2f, rows: f32, depth: f32) -> f32 {
    return 1.0 - depth * 0.5 * (1.0 + sin(p.y * rows * TAU));
}
