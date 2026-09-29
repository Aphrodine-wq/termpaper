// ---------------------------------------------------------------- sky
// Physically based sky: single scattering through a Rayleigh + Mie
// atmosphere (Nishita-style, 12 view / 4 light samples). Returns linear
// radiance scaled so a clear noon zenith is ~0.3-0.6 and the sun disk is
// ~50+ before exposure — AgX handles the range. Directions are y-up unit
// vectors; below the horizon the sky is evaluated at the horizon.

const SKY_RE: f32 = 6360e3;
const SKY_RA: f32 = 6420e3;
const SKY_HR: f32 = 7994.0;
const SKY_HM: f32 = 1200.0;
const SKY_BR: vec3f = vec3f(5.8e-6, 13.5e-6, 33.1e-6);
const SKY_BM: f32 = 21e-6;
const SKY_SUN: f32 = 20.0;

// direction to the sun from elevation / azimuth in degrees (azimuth 0 = -z,
// i.e. straight ahead of a camera looking down -z; positive to the right)
fn sky_sun_dir(elev_deg: f32, azim_deg: f32) -> vec3f {
    let e = radians(elev_deg);
    let a = radians(azim_deg);
    return normalize(vec3f(sin(a) * cos(e), sin(e), -cos(a) * cos(e)));
}
fn sky_ray_sphere(ro: vec3f, rd: vec3f, r: f32) -> vec2f {
    let b = dot(ro, rd);
    let c = dot(ro, ro) - r * r;
    let d = b * b - c;
    if (d < 0.0) { return vec2f(-1.0); }
    let s = sqrt(d);
    return vec2f(-b - s, -b + s);
}
fn sky_phase_r(mu: f32) -> f32 { return 3.0 / (16.0 * PI) * (1.0 + mu * mu); }
fn sky_phase_m(mu: f32, g: f32) -> f32 {
    let g2 = g * g;
    return 3.0 / (8.0 * PI) * ((1.0 - g2) * (1.0 + mu * mu)) / ((2.0 + g2) * pow(1.0 + g2 - 2.0 * g * mu, 1.5));
}
// optical depth (rayleigh, mie) from a point along rd to the top of the sky
fn sky_depth_to_space(p: vec3f, rd: vec3f) -> vec2f {
    let t = sky_ray_sphere(p, rd, SKY_RA).y;
    let n = 4;
    let ds = t / f32(n);
    var od = vec2f(0.0);
    for (var i = 0; i < 4; i++) {
        let q = p + rd * (f32(i) + 0.5) * ds;
        let h = length(q) - SKY_RE;
        if (h < 0.0) { return vec2f(1e9); }
        od += vec2f(exp(-h / SKY_HR), exp(-h / SKY_HM)) * ds;
    }
    return od;
}
fn sky_scatter(rd_in: vec3f, sun: vec3f, haze: f32) -> vec3f {
    var rd = rd_in;
    rd.y = max(rd.y, 0.002);
    rd = normalize(rd);
    let ro = vec3f(0.0, SKY_RE + 50.0, 0.0);
    let tmax = sky_ray_sphere(ro, rd, SKY_RA).y;
    let n = 12;
    let ds = tmax / f32(n);
    let mu = dot(rd, sun);
    let bm = SKY_BM * haze;
    var od = vec2f(0.0);
    var sum_r = vec3f(0.0);
    var sum_m = vec3f(0.0);
    for (var i = 0; i < 12; i++) {
        let p = ro + rd * (f32(i) + 0.5) * ds;
        let h = length(p) - SKY_RE;
        let dr = exp(-h / SKY_HR) * ds;
        let dm = exp(-h / SKY_HM) * ds;
        od += vec2f(dr, dm);
        let ol = sky_depth_to_space(p, sun);
        let tau = SKY_BR * (od.x + ol.x) + bm * 1.1 * (od.y + ol.y);
        let tr = exp(-tau);
        sum_r += tr * dr;
        sum_m += tr * dm;
    }
    return SKY_SUN * (sum_r * SKY_BR * sky_phase_r(mu) + sum_m * bm * sky_phase_m(mu, 0.76));
}
// full sky (no sun disk); haze 1 = clear, 3-8 = hazy/smoggy
fn sky_atmosphere(rd: vec3f, sun: vec3f, ctx: Ctx) -> vec3f { return sky_scatter(rd, sun, 1.0); }
fn sky_atmosphere_haze(rd: vec3f, sun: vec3f, haze: f32) -> vec3f { return sky_scatter(rd, sun, haze); }
// sunlight colour/intensity reaching the ground (for direct lighting)
fn sky_sun_light(sun: vec3f) -> vec3f {
    let ro = vec3f(0.0, SKY_RE + 50.0, 0.0);
    let s = normalize(vec3f(sun.x, max(sun.y, -0.02), sun.z));
    let od = sky_depth_to_space(ro, s);
    let fade = smoothstep(-0.04, 0.02, sun.y);
    return SKY_SUN * 0.16 * exp(-(SKY_BR * od.x + SKY_BM * 1.1 * od.y)) * fade;
}
// rough sky irradiance on an upward surface (for ambient light)
fn sky_ambient(sun: vec3f) -> vec3f {
    let z = sky_scatter(vec3f(0.0, 1.0, 0.0), sun, 1.0);
    let h = sky_scatter(normalize(vec3f(-sun.x, 0.15, -sun.z)), sun, 1.0);
    return (z * 0.6 + h * 0.4) * 1.3 + vec3f(0.002, 0.003, 0.006);
}
// Cheap analytic sky gradient for stylised or distant use.
fn sky_fast(rd: vec3f, sun: vec3f) -> vec3f {
    let y = max(rd.y, 0.0);
    let s = saturate(sun.y * 2.0 + 0.3);
    let zen = mix(vec3f(0.012, 0.018, 0.05), vec3f(0.08, 0.2, 0.55), s);
    let hor = mix(vec3f(0.35, 0.16, 0.08), vec3f(0.55, 0.7, 0.9), s);
    var c = mix(hor, zen, pow(y, 0.45));
    let mu = saturate(dot(rd, sun));
    c += col_kelvin(mix(2200.0, 5800.0, s)) * (0.25 * pow(mu, 8.0) + 0.9 * pow(mu, 180.0)) * (0.3 + s);
    return c;
}
// sun disk with limb darkening; tinted by the atmosphere's transmittance
fn sky_sun_disk(rd: vec3f, sun: vec3f, radius_deg: f32) -> vec3f {
    let c = dot(rd, sun);
    let r = radians(radius_deg);
    let d = acos(clamp(c, -1.0, 1.0));
    let x = saturate(d / r);
    let limb = select(0.0, 0.4 + 0.6 * sqrt(max(1.0 - x * x, 0.0)), d < r);
    let edge = smoothstep(r * 1.15, r * 0.85, d);
    return sky_sun_light(sun) * 60.0 * limb * edge;
}
// Moon disk: phase 0 = new, 0.5 = full, 1 = new; lit from the side the
// phase implies, with faint maria. `light` scales brightness.
fn sky_moon(rd: vec3f, moon: vec3f, phase: f32, radius_deg: f32) -> vec3f {
    let r = radians(radius_deg);
    let right = normalize(cross(moon, vec3f(0.0, 1.0, 0.0)));
    let up = cross(right, moon);
    let q = vec2f(dot(rd, right), dot(rd, up)) / r;
    let d2 = dot(q, q);
    if (d2 > 1.3 || dot(rd, moon) < 0.0) { return vec3f(0.0); }
    let z = sqrt(max(1.0 - d2, 0.0));
    let n = vec3f(q, z);
    let a = (phase - 0.5) * TAU;
    let l = normalize(vec3f(sin(a), 0.1, cos(a)));
    let lit = smoothstep(-0.05, 0.12, dot(n, l));
    let maria = 0.72 + 0.28 * smoothstep(0.35, 0.65, noise_fbm2(q * 2.2 + 3.0, 4));
    let disk = smoothstep(1.0, 0.94, sqrt(d2));
    let earthshine = 0.012;
    return vec3f(1.0, 0.97, 0.9) * (lit * maria + earthshine) * disk * 2.2;
}
// night sky base: deep blue gradient plus faint airglow at the horizon
fn sky_night(rd: vec3f) -> vec3f {
    let y = max(rd.y, 0.0);
    let base = mix(vec3f(0.006, 0.009, 0.02), vec3f(0.0012, 0.002, 0.006), pow(y, 0.5));
    let glow = vec3f(0.01, 0.014, 0.008) * exp(-y * 14.0);
    return base + glow;
}
