//! Check a Studio scene file without rebuilding termpaper or touching a GPU.
//!
//! ```text
//! cargo build --release --example shader_check
//! target/release/examples/shader_check src/gpu/scenes/<name>.wgsl [more.wgsl…]
//! ```
//!
//! Reads the scene (and the library) from disk, parses the header, composes
//! the shader the way the renderer does, runs naga's WGSL front end and
//! validator, translates to Metal and HLSL (what macOS and Windows compile),
//! and lints the author contract from CONTRIBUTING.md. Errors point at the
//! scene file's own line numbers. Exit status 1 when anything failed.
use std::path::Path;
use termpaper::scene::shader;

const RESERVED_PREFIXES: &[&str] = &[
    "hash_", "noise_", "col_", "tm_", "dither_", "cam_", "sdf_", "sdf2_", "op_", "rm_", "sky_", "star_", "cloud_", "vol_",
    "fog_", "water_", "wet_", "rain_", "snow_", "bokeh_", "light_", "fire_", "l2d_", "glyph_", "game_", "entry_",
];

fn translate(module: &naga::Module, info: &naga::valid::ModuleInfo) -> Result<(), String> {
    let (module, info) =
        naga::back::pipeline_constants::process_overrides(module, info, None, &naga::back::PipelineConstants::default())
            .map_err(|e| format!("pipeline constants: {e}"))?;
    let (module, info) = (module.as_ref(), info.as_ref());
    let msl = naga::back::msl::Options { lang_version: (2, 4), ..Default::default() };
    naga::back::msl::write_string(module, info, &msl, &naga::back::msl::PipelineOptions::default())
        .map_err(|e| format!("Metal (MSL): {e}"))?;
    let hlsl = naga::back::hlsl::Options { shader_model: naga::back::hlsl::ShaderModel::V5_1, ..Default::default() };
    let mut out = String::new();
    naga::back::hlsl::Writer::new(&mut out, &hlsl, &naga::back::hlsl::PipelineOptions::default())
        .write(module, info, None)
        .map_err(|e| format!("DX12 (HLSL SM 5.1): {e}"))?;
    Ok(())
}

fn validate(composed: &shader::Composed) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(&composed.source).map_err(|e| {
        let msg = e.emit_to_string(&composed.source);
        match e.location(&composed.source).map(|l| composed.locate(l.line_number as usize)) {
            Some((file, line)) => format!("{file}:{line}: {msg}"),
            None => msg,
        }
    })?;
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
        .validate(&module)
        .map_err(|e| {
            let msg = e.emit_to_string(&composed.source);
            // point the first span at the scene file
            let loc = e
                .spans()
                .next()
                .map(|(s, _)| s.location(&composed.source))
                .map(|l| composed.locate(l.line_number as usize));
            match loc {
                Some((file, line)) => format!("{file}:{line}: {msg}"),
                None => msg,
            }
        })?;
    if !module.entry_points.iter().any(|e| e.name == "main") {
        return Err("no `main` entry point".into());
    }
    translate(&module, &info)
}

fn lint(name: &str, text: &str, uses: &[&str]) -> Vec<String> {
    let code: String = text.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
    let mut problems = Vec::new();
    if code.matches("fn scene(").count() != 1 {
        problems.push("must define exactly one `fn scene(`".into());
    }
    for forbidden in ["@group", "@binding", "var<storage", "var<workgroup", "atomic", "loop {", "while "] {
        if code.contains(forbidden) {
            problems.push(format!("uses `{forbidden}` (scenes are pure, bounded functions)"));
        }
    }
    if code.contains("fract(sin") {
        problems.push("uses fract(sin()) hashing — use hash_* (identical on every GPU)".into());
    }
    for (i, line) in code.lines().enumerate() {
        let l = line.trim_start();
        if let Some(rest) = l.strip_prefix("fn ") {
            let fname = rest.split('(').next().unwrap_or("").trim();
            if RESERVED_PREFIXES.iter().any(|p| fname.starts_with(p)) {
                problems.push(format!("line {}: defines `{fname}`, which uses a reserved library prefix", i + 1));
            }
        }
        // a `for` without a bound: the GPU may never finish
        if l.starts_with("for (") && !["<", ">", "!="].iter().any(|op| l.contains(op)) {
            problems.push(format!("line {}: `for` loop without a bound", i + 1));
        }
    }
    if uses.contains(&"raymarch") && !code.contains("fn map(") {
        problems.push("uses raymarch but defines no `fn map(p: vec3f, ctx: Ctx) -> vec2f`".into());
    }
    if let Some(a) = code.find("const F:") {
        let _ = a;
        problems.push("`F` is the entry point's uniform: call your constant something else".into());
    }
    let _ = name;
    problems
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: shader_check src/gpu/scenes/<name>.wgsl [more…]");
        std::process::exit(2);
    }
    let mut failed = false;
    for arg in &args {
        let path = Path::new(arg);
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            eprintln!("{arg}: not a .wgsl path");
            failed = true;
            continue;
        };
        let dir = path.parent().unwrap_or(Path::new("."));
        let (composed, meta) = match shader::compose_from_dir(stem, dir) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("FAIL {e}");
                failed = true;
                continue;
            }
        };
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut problems = lint(stem, &text, &meta.uses);
        if let Err(e) = validate(&composed) {
            problems.push(e);
        }
        if problems.is_empty() {
            println!(
                "ok   {stem}: {} · {} · themes {} · params {} · uses {}",
                meta.category,
                meta.cost,
                meta.themes.join(","),
                if meta.params.is_empty() { "-".to_string() } else { meta.params.join(",") },
                meta.uses.join(",")
            );
        } else {
            failed = true;
            println!("FAIL {stem}:");
            for p in problems {
                println!("  {p}");
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
