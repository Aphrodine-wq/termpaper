// ---------------------------------------------------------------- color
// Work in LINEAR light. Colours picked from photos/palettes are sRGB: convert
// them with col_hex / col_srgb_to_linear.

fn col_srgb_to_linear(c: vec3f) -> vec3f {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3f(2.4));
    return select(hi, lo, c <= vec3f(0.04045));
}
fn col_linear_to_srgb(c: vec3f) -> vec3f {
    let x = max(c, vec3f(0.0));
    let lo = x * 12.92;
    let hi = 1.055 * pow(x, vec3f(1.0 / 2.4)) - 0.055;
    return select(hi, lo, x <= vec3f(0.0031308));
}
// 0xRRGGBB sRGB → linear
fn col_hex(rgb: u32) -> vec3f {
    let c = vec3f(f32((rgb >> 16u) & 0xffu), f32((rgb >> 8u) & 0xffu), f32(rgb & 0xffu)) / 255.0;
    return col_srgb_to_linear(c);
}
fn col_luma(c: vec3f) -> f32 { return dot(c, vec3f(0.2126, 0.7152, 0.0722)); }
// Blackbody-ish colour for a temperature in kelvin (1000..40000), linear,
// normalised so max channel is 1. Candles ~1900K, tungsten 2700K, noon 5500K,
// overcast 6500K, blue hour 9000K+.
fn col_kelvin(k: f32) -> vec3f {
    let t = clamp(k, 1000.0, 40000.0) / 100.0;
    var r: f32;
    var g: f32;
    var b: f32;
    if (t <= 66.0) {
        r = 1.0;
        g = saturate(0.39008157876 * log(t) - 0.63184144378);
        if (t <= 19.0) { b = 0.0; } else { b = saturate(0.54320678911 * log(t - 10.0) - 1.19625408914); }
    } else {
        r = saturate(1.29293618606 * pow(t - 60.0, -0.1332047592));
        g = saturate(1.12989086089 * pow(t - 60.0, -0.0755148492));
        b = 1.0;
    }
    let c = col_srgb_to_linear(vec3f(r, g, b));
    return c / max(max3(c), 1e-4);
}
fn col_hsv(h: f32, s: f32, v: f32) -> vec3f {
    let k = vec3f(1.0, 2.0 / 3.0, 1.0 / 3.0);
    let p = abs(fract(vec3f(h) + k) * 6.0 - 3.0);
    return v * mix(vec3f(1.0), saturate3(p - 1.0), s);
}
fn col_saturation(c: vec3f, s: f32) -> vec3f {
    let l = col_luma(c);
    return max(vec3f(0.0), vec3f(l) + (c - vec3f(l)) * s);
}
fn col_to_oklab(c: vec3f) -> vec3f {
    let l = 0.4122214708 * c.r + 0.5363325363 * c.g + 0.0514459929 * c.b;
    let m = 0.2119034982 * c.r + 0.6806995451 * c.g + 0.1073969566 * c.b;
    let s = 0.0883024619 * c.r + 0.2817188376 * c.g + 0.6299787005 * c.b;
    let lms = pow(max(vec3f(l, m, s), vec3f(0.0)), vec3f(1.0 / 3.0));
    return vec3f(
        0.2104542553 * lms.x + 0.7936177850 * lms.y - 0.0040720468 * lms.z,
        1.9779984951 * lms.x - 2.4285922050 * lms.y + 0.4505937099 * lms.z,
        0.0259040371 * lms.x + 0.7827717662 * lms.y - 0.8086757660 * lms.z);
}
fn col_from_oklab(c: vec3f) -> vec3f {
    let l = c.x + 0.3963377774 * c.y + 0.2158037573 * c.z;
    let m = c.x - 0.1055613458 * c.y - 0.0638541728 * c.z;
    let s = c.x - 0.0894841775 * c.y - 1.2914855480 * c.z;
    let lms = vec3f(l, m, s);
    let lms3 = lms * lms * lms;
    return vec3f(
        4.0767416621 * lms3.x - 3.3077115913 * lms3.y + 0.2309699292 * lms3.z,
        -1.2684380046 * lms3.x + 2.6097574011 * lms3.y - 0.3413193965 * lms3.z,
        -0.0041960863 * lms3.x - 0.7034186147 * lms3.y + 1.7076147010 * lms3.z);
}
// perceptual blend — gradients between very different hues stay clean
fn col_mix_oklab(a: vec3f, b: vec3f, t: f32) -> vec3f {
    return max(col_from_oklab(mix(col_to_oklab(a), col_to_oklab(b), t)), vec3f(0.0));
}
// piecewise-linear ramp through 4 stops at positions (0, k1, k2, 1)
fn col_ramp4(c0: vec3f, c1: vec3f, c2: vec3f, c3: vec3f, k1: f32, k2: f32, t: f32) -> vec3f {
    let x = saturate(t);
    if (x < k1) { return mix(c0, c1, x / k1); }
    if (x < k2) { return mix(c1, c2, (x - k1) / (k2 - k1)); }
    return mix(c2, c3, (x - k2) / (1.0 - k2));
}
