// ---------------------------------------------------------------- dither
// Interleaved gradient noise (Jimenez 2014): static per pixel, so it hides
// banding without animating (moving noise costs terminal bandwidth).
fn dither_ign(px: vec2f) -> f32 {
    return fract(52.9829189 * fract(dot(px, vec2f(0.06711056, 0.00583715))));
}
