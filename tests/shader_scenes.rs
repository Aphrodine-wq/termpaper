//! Studio shader scenes, checked without a GPU.
//!
//! Every registered scene is composed exactly as the renderer composes it and
//! run through naga's WGSL front end and validator — the same checks wgpu
//! makes at pipeline creation — so a broken scene fails `cargo test` instead
//! of a user's wallpaper. Errors are reported against the scene file.
use termpaper::scene::{self, shader};

fn validate(composed: &shader::Composed) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(&composed.source).map_err(|e| {
        let msg = e.emit_to_string(&composed.source);
        let loc = e.location(&composed.source).map(|l| composed.locate(l.line_number as usize));
        match loc {
            Some((file, line)) => format!("{file}:{line}: {msg}"),
            None => msg,
        }
    })?;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
        .validate(&module)
        .map_err(|e| e.emit_to_string(&composed.source))?;
    if !module.entry_points.iter().any(|e| e.name == "main") {
        return Err("no `main` entry point".into());
    }
    Ok(())
}

#[test]
fn library_compiles_with_every_module() {
    let all: Vec<&str> = termpaper::scene::shader_meta::MODULES.iter().map(|m| m.0).collect();
    let text = format!(
        "//! name: probe\n//! title: Probe\n//! category: coast\n//! desc: probe\n//! themes: a, b\n\
         //! uses: {}\n//! fallback: ocean\n\
         fn map(p: vec3f, ctx: Ctx) -> vec2f {{ return vec2f(sdf_sphere(p, 1.0), 1.0); }}\n\
         fn scene(p: vec2f, ctx: Ctx) -> vec3f {{ return vec3f(p, 0.0); }}\n",
        all.join(", ")
    );
    let (c, _) = shader::compose_text("probe.wgsl", "probe", &text).unwrap();
    validate(&c).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn every_scene_compiles() {
    let mut failures = Vec::new();
    for spec in shader::SHADER_SCENES {
        if let Err(e) = validate(&shader::compose(spec)) {
            failures.push(format!("{}:\n{e}", spec.name));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}

#[test]
fn metadata_is_consistent_with_the_classic_registry() {
    for spec in shader::SHADER_SCENES {
        assert!(scene::find(spec.name).is_none(), "{} collides with a Classic scene", spec.name);
        assert!(
            scene::find(spec.fallback).is_some(),
            "{}: fallback '{}' is not a Classic scene",
            spec.name,
            spec.fallback
        );
        assert!(spec.category != scene::Category::Classic);
    }
    let names = scene::all_names();
    for (i, n) in names.iter().enumerate() {
        assert!(!names[..i].contains(n), "duplicate scene name {n}");
    }
    assert_eq!(names.len(), scene::names().len() + shader::SHADER_SCENES.len());
}

/// Names the library reserves; scenes defining them would collide as the
/// library grows.
const RESERVED_PREFIXES: &[&str] = &[
    "hash_", "noise_", "col_", "tm_", "dither_", "cam_", "sdf_", "sdf2_", "op_", "rm_", "sky_",
    "star_", "cloud_", "vol_", "fog_", "water_", "wet_", "rain_", "snow_", "bokeh_", "light_",
    "fire_", "l2d_", "entry_",
];

#[test]
fn scenes_follow_the_author_contract() {
    let mut problems = Vec::new();
    for spec in shader::SHADER_SCENES {
        let src = spec.source;
        let code: String = src
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        let mut bad = |what: &str| problems.push(format!("{}: {what}", spec.name));
        if code.matches("fn scene(").count() != 1 {
            bad("must define exactly one `fn scene(`");
        }
        for forbidden in ["@group", "@binding", "var<storage", "var<workgroup", "atomic", "loop {", "while "] {
            if code.contains(forbidden) {
                bad(&format!("uses `{forbidden}` (scenes are pure, bounded functions)"));
            }
        }
        if code.contains("sin(") && code.contains("fract(sin") {
            bad("uses fract(sin()) hashing — use hash_* (identical on every GPU)");
        }
        for line in code.lines() {
            let l = line.trim_start();
            if let Some(rest) = l.strip_prefix("fn ") {
                let name = rest.split('(').next().unwrap_or("").trim();
                if RESERVED_PREFIXES.iter().any(|p| name.starts_with(p)) {
                    bad(&format!("defines `{name}`, which uses a reserved library prefix"));
                }
            }
        }
        if spec.uses.contains(&"raymarch") && !code.contains("fn map(") {
            bad("uses raymarch but defines no `fn map(p: vec3f, ctx: Ctx) -> vec2f`");
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}
