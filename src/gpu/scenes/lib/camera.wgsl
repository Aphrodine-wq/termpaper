// ---------------------------------------------------------------- camera
// Pinhole camera whose field of view applies to the SHORT side of the frame,
// so a portrait monitor sees more sky and foreground instead of a squeezed
// slice. Right-handed, y up.

struct Cam { ro: vec3f, fw: vec3f, rt: vec3f, up: vec3f, zoom: f32 }

fn cam_look_at(ro: vec3f, at: vec3f, roll: f32, fov_short_deg: f32) -> Cam {
    let fw = normalize(at - ro);
    let wup = vec3f(sin(roll), cos(roll), 0.0);
    let rt = normalize(cross(fw, wup));
    let up = cross(rt, fw);
    let zoom = 0.5 / tan(radians(fov_short_deg) * 0.5);
    return Cam(ro, fw, rt, up, zoom);
}
// view ray through composition point p
fn cam_ray(cam: Cam, p: vec2f) -> vec3f {
    return normalize(cam.rt * p.x + cam.up * p.y + cam.fw * cam.zoom);
}
// world point → (p.xy, depth along the view axis); depth <= 0 is behind
fn cam_project(cam: Cam, w: vec3f) -> vec3f {
    let d = w - cam.ro;
    let z = dot(d, cam.fw);
    return vec3f(vec2f(dot(d, cam.rt), dot(d, cam.up)) * cam.zoom / max(z, 1e-4), z);
}
// very slow deterministic drift (a few cm): a moving camera repaints every
// terminal cell every frame, so keep amp tiny or zero
fn cam_sway(t: f32, amp: f32, salt: u32) -> vec3f {
    let s = f32(salt % 97u);
    return amp * vec3f(sin(t * 0.071 + s), sin(t * 0.053 + s * 1.7) * 0.6, sin(t * 0.037 + s * 2.3) * 0.4);
}
