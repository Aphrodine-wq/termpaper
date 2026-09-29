// ---------------------------------------------------------------- core
// Studio shader library: the context every scene receives, and small math.
//
// Coordinates: `p` is composition space, y up, centred on the whole wall
// frame; the frame's SHORT side spans [-0.5, 0.5]. `ctx.half` is the frame's
// half extent, so the visible area is |p.x| <= half.x, |p.y| <= half.y. On a
// multi-monitor wall a pane only renders its crop of that frame.

const PI: f32 = 3.14159265358979;
const TAU: f32 = 6.28318530717959;

struct Ctx {
    // scene seconds, speed-scaled; continuous across the hourly wrap
    t: f32,
    // one output pixel in p units — use for anti-aliasing widths and LOD
    px: f32,
    // half extent of the whole wall frame in p units
    half: vec2f,
    // half.x / half.y
    aspect: f32,
    // index into the scene's `themes:` list
    theme: u32,
    // 0 low, 1 medium, 2 high
    detail: u32,
    // march-step multiplier from detail (0.6 / 1.0 / 1.4); see `steps`
    march: f32,
    // per-launch seed shared by every pane of a wall
    seed: vec2u,
    // the seed as two floats in [0, 1)
    seedf: vec2f,
    // per-sample static noise in [0, 1): dither ray starts with it
    jitter: f32,
    // sample index inside the pixel, and the pixel's sample count
    sample: u32,
    spp: u32,
    params: vec4f,
}

fn saturate(x: f32) -> f32 { return clamp(x, 0.0, 1.0); }
fn saturate3(v: vec3f) -> vec3f { return clamp(v, vec3f(0.0), vec3f(1.0)); }
fn remap(x: f32, a: f32, b: f32, c: f32, d: f32) -> f32 { return c + (d - c) * (x - a) / (b - a); }
// linear 0..1 ramp between a and b (works for a > b too)
fn linstep(a: f32, b: f32, x: f32) -> f32 { return saturate((x - a) / (b - a)); }
// smoothstep that is defined for a > b (WGSL's is not)
fn sstep(a: f32, b: f32, x: f32) -> f32 { let t = linstep(a, b, x); return t * t * (3.0 - 2.0 * t); }
// GLSL-style mod: result has the sign of y
fn fmod_pos(x: f32, y: f32) -> f32 { return x - y * floor(x / y); }
fn fmod_pos2(x: vec2f, y: f32) -> vec2f { return x - y * floor(x / y); }
// rotation by a radians (counter-clockwise): rot2(a) * v
fn rot2(a: f32) -> mat2x2f { let c = cos(a); let s = sin(a); return mat2x2f(c, s, -s, c); }
// the frame as [0,1]², y up
fn frame_uv(p: vec2f, ctx: Ctx) -> vec2f { return p / (2.0 * ctx.half) + 0.5; }
// coverage of a signed distance (negative inside), one pixel wide
fn aa_fill(d: f32, ctx: Ctx) -> f32 { return saturate(0.5 - d / ctx.px); }
fn aa_fill_w(d: f32, w: f32) -> f32 { return saturate(0.5 - d / max(w, 1e-6)); }
fn aa_stroke(d: f32, w: f32, ctx: Ctx) -> f32 { return aa_fill(abs(d) - w, ctx); }
fn over(dst: vec3f, src: vec3f, a: f32) -> vec3f { return mix(dst, src, saturate(a)); }
// march-step budget: base scaled by detail, clamped to [4, 512]
fn steps(base: f32, ctx: Ctx) -> i32 { return clamp(i32(base * ctx.march), 4, 512); }
fn max3(v: vec3f) -> f32 { return max(v.x, max(v.y, v.z)); }
fn min3(v: vec3f) -> f32 { return min(v.x, min(v.y, v.z)); }
fn sq(x: f32) -> f32 { return x * x; }
// cheap soft pulse centred on c with half-width w
fn pulse(c: f32, w: f32, x: f32) -> f32 { let d = abs(x - c) / w; return saturate(1.0 - d * d * (3.0 - 2.0 * d)); }
// exponential ease toward 1: 1 - e^(-k x)
fn expease(x: f32, k: f32) -> f32 { return 1.0 - exp(-k * max(x, 0.0)); }
// loop-safe triangle wave 0..1..0 with period 1
fn tri(x: f32) -> f32 { return 1.0 - abs(2.0 * fract(x) - 1.0); }
// sub-pixel offset of sample i, in p units, for `ss: manual` scenes
fn ss_offset(i: u32, ctx: Ctx) -> vec2f {
    let r = fract(vec2f(0.5) + vec2f(0.7548776662, 0.5698402910) * f32(i)) - 0.5;
    return r * ctx.px;
}
