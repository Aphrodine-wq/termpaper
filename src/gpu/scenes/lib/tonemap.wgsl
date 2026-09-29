// ---------------------------------------------------------------- tonemap
// Scene-referred HDR in, display-linear [0,1] out. The entry point applies
// this per sample, then averages, so a single blazing sun/neon sample can't
// alias. TM_MODE comes from the scene header (`tonemap:`).
//
// AgX: Troy Sobotka's AgX via the Filament/three.js polynomial fit (Apache-2.0
// / MIT). ACES: Stephen Hill's RRT+ODT fit (MIT). Constants only.

fn tm_agx_contrast(x: vec3f) -> vec3f {
    let x2 = x * x;
    let x4 = x2 * x2;
    return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232;
}
fn tm_agx_core(c: vec3f, punchy: bool) -> vec3f {
    let srgb_to_2020 = mat3x3f(
        vec3f(0.6274, 0.0691, 0.0164), vec3f(0.3293, 0.9195, 0.0880), vec3f(0.0433, 0.0113, 0.8956));
    let inset = mat3x3f(
        vec3f(0.856627153315983, 0.137318972929847, 0.11189821299995),
        vec3f(0.0951212405381588, 0.761241990602591, 0.0767994186031903),
        vec3f(0.0482516061458583, 0.101439036467562, 0.811302368396859));
    let outset = mat3x3f(
        vec3f(1.1271005818144368, -0.1413297634984383, -0.14132976349843826),
        vec3f(-0.11060664309660323, 1.157823702216272, -0.11060664309660294),
        vec3f(-0.016493938717834573, -0.016493938717834257, 1.2519364065950405));
    let r2020_to_srgb = mat3x3f(
        vec3f(1.6605, -0.1246, -0.0182), vec3f(-0.5876, 1.1329, -0.1006), vec3f(-0.0728, -0.0083, 1.1187));
    let min_ev = -12.47393;
    let max_ev = 4.026069;
    var v = inset * (srgb_to_2020 * c);
    v = log2(max(v, vec3f(1e-10)));
    v = saturate3((v - min_ev) / (max_ev - min_ev));
    v = tm_agx_contrast(v);
    if (punchy) {
        let l = dot(v, vec3f(0.2126, 0.7152, 0.0722));
        v = pow(max(v, vec3f(0.0)), vec3f(1.35));
        v = vec3f(l) + 1.4 * (v - vec3f(l));
    }
    v = outset * v;
    v = pow(max(v, vec3f(0.0)), vec3f(2.2));
    return saturate3(r2020_to_srgb * v);
}
fn tm_agx(c: vec3f) -> vec3f { return tm_agx_core(c, false); }
fn tm_agx_punchy(c: vec3f) -> vec3f { return tm_agx_core(c, true); }
fn tm_aces(c: vec3f) -> vec3f {
    let aces_in = mat3x3f(
        vec3f(0.59719, 0.07600, 0.02840), vec3f(0.35458, 0.90834, 0.13383), vec3f(0.04823, 0.01566, 0.83777));
    let aces_out = mat3x3f(
        vec3f(1.60475, -0.10208, -0.00327), vec3f(-0.53108, 1.10813, -0.07276), vec3f(-0.07367, -0.00605, 1.07602));
    let v = aces_in * (c * 1.6);
    let a = v * (v + 0.0245786) - 0.000090537;
    let b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return saturate3(aces_out * (a / b));
}
// hue-preserving soft rolloff, for scenes that want their colours literal
fn tm_neutral(c: vec3f) -> vec3f {
    let m = max3(c);
    let k = m / (1.0 + m);
    return saturate3(c * (k / max(m, 1e-6)));
}
fn tm_apply(c: vec3f) -> vec3f {
    switch (TM_MODE) {
        case 1u: { return tm_agx_punchy(c); }
        case 2u: { return tm_aces(c); }
        case 3u: { return tm_neutral(c); }
        default: { return tm_agx(c); }
    }
}
