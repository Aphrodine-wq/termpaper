//! Studio shader scenes: the registry and WGSL composition.
//!
//! A Studio scene is one stateless WGSL file under `src/gpu/scenes/`. It
//! implements `fn scene(p: vec2f, ctx: Ctx) -> vec3f`, returning linear HDR
//! radiance for a point `p` in composition space. The frame is a pure function
//! of `(p, time, seed, theme)`, so every terminal of a wall renders exactly its
//! own crop of the same picture from the shared clock — no replay, no drift.
//!
//! This module has no GPU dependency: the registry, composition and quality
//! tables are shared by the renderer (`gpu::shader_scene`), the no-GPU test
//! suite and the settings menu.
use super::Detail;
use std::path::Path;

/// Where a scene is filed in the browser. `Classic` holds the original CPU
/// scenes; Studio scenes use the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Coast,
    Wilds,
    Weather,
    City,
    Cozy,
    Space,
    Classic,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Category::Coast,
        Category::Wilds,
        Category::Weather,
        Category::City,
        Category::Cozy,
        Category::Space,
        Category::Classic,
    ];

    /// Browser heading.
    pub fn label(self) -> &'static str {
        match self {
            Category::Coast => "Coast & Water",
            Category::Wilds => "Mountains & Wild",
            Category::Weather => "Weather & Sky",
            Category::City => "Cities & Streets",
            Category::Cozy => "Cozy & Interiors",
            Category::Space => "Space",
            Category::Classic => "Classic",
        }
    }

    /// Lowercase key used on the command line and in headers.
    pub fn slug(self) -> &'static str {
        match self {
            Category::Coast => "coast",
            Category::Wilds => "wilds",
            Category::Weather => "weather",
            Category::City => "city",
            Category::Cozy => "cozy",
            Category::Space => "space",
            Category::Classic => "classic",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.slug() == s)
    }
}

/// Rough GPU cost class, declared by the author. Sets the supersampling
/// ceiling (`max_spp`); the governor picks the actual count at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cost {
    Light,
    Medium,
    Heavy,
}

pub struct LibModule {
    pub name: &'static str,
    pub source: &'static str,
}

pub struct ShaderSpec {
    pub name: &'static str,
    pub title: &'static str,
    pub category: Category,
    pub tags: &'static [&'static str],
    pub desc: &'static str,
    /// Two to five themes; the first is the default.
    pub themes: &'static [&'static str],
    /// Resolved library modules in composition order.
    pub uses: &'static [&'static str],
    pub cost: Cost,
    /// `TM_MODE` for the tonemapper: 0 AgX, 1 AgX punchy, 2 ACES, 3 neutral.
    pub tonemap: u32,
    /// Exposure offset in stops.
    pub exposure: f32,
    /// The scene supersamples itself (see `ss_offset` in the core module).
    pub ss_manual: bool,
    /// Classic scene shown when there is no GPU.
    pub fallback: &'static str,
    pub credits: &'static str,
    pub source: &'static str,
    /// Repo-relative path, for error messages and hot reload.
    pub path: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/shader_scenes.rs"));

impl PartialEq for ShaderSpec {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl std::fmt::Debug for ShaderSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ShaderSpec({})", self.name)
    }
}

/// Look a Studio scene up by name.
pub fn find(name: &str) -> Option<&'static ShaderSpec> {
    SHADER_SCENES.iter().find(|s| s.name == name)
}

/// A composed shader: one WGSL module (header constants, library, scene,
/// entry point), plus where each part starts so errors can be reported
/// against the file the author actually edits.
pub struct Composed {
    pub source: String,
    /// (file label, first line in `source`), in order.
    pub sections: Vec<(String, usize)>,
}

impl Composed {
    /// Map a 1-based line of the composed source to (file, 1-based line).
    pub fn locate(&self, line: usize) -> (String, usize) {
        let mut best = ("<prelude>".to_string(), line);
        for (label, start) in &self.sections {
            if *start <= line {
                best = (label.clone(), line - start + 1);
            }
        }
        best
    }
}

fn lib_source(name: &str) -> &'static str {
    LIB.iter()
        .find(|m| m.name == name)
        .map(|m| m.source)
        .unwrap_or("")
}

fn assemble(
    spec_consts: (u32, f32, bool),
    modules: &[(String, String)],
    scene: (&str, &str),
    entry: &str,
) -> Composed {
    let (tm, exposure, ss_manual) = spec_consts;
    let mut source = format!(
        "// composed by termpaper — see src/scene/shader.rs\n\
         const TM_MODE: u32 = {tm}u;\n\
         const SCENE_EXPOSURE: f32 = {exposure:?};\n\
         const SS_MANUAL: bool = {ss_manual};\n"
    );
    let mut sections = Vec::new();
    let mut push = |label: String, text: &str, source: &mut String| {
        sections.push((label, source.lines().count() + 1));
        source.push_str(text);
        if !text.ends_with('\n') {
            source.push('\n');
        }
    };
    for (label, text) in modules {
        push(label.clone(), text, &mut source);
    }
    push(scene.0.to_string(), scene.1, &mut source);
    push("src/gpu/shaders/scene_entry.wgsl".into(), entry, &mut source);
    Composed { source, sections }
}

/// Compose the embedded sources for a scene.
pub fn compose(spec: &ShaderSpec) -> Composed {
    let modules: Vec<(String, String)> = spec
        .uses
        .iter()
        .map(|m| (format!("src/gpu/scenes/lib/{m}.wgsl"), lib_source(m).to_string()))
        .collect();
    assemble(
        (spec.tonemap, spec.exposure, spec.ss_manual),
        &modules,
        (spec.path, spec.source),
        ENTRY,
    )
}

/// Compose a scene from files on disk instead of the embedded copies, for
/// iterating on a scene without rebuilding (`TERMPAPER_SHADER_DIR`, and the
/// `shader_review` example). `dir` is the scenes directory; its `lib/`
/// subdirectory supplies the modules and `../shaders/scene_entry.wgsl` the
/// entry point when present. Missing files fall back to the embedded ones.
pub fn compose_from_dir(name: &str, dir: &Path) -> Result<(Composed, super::shader_meta::Meta), String> {
    let path = dir.join(format!("{name}.wgsl"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let meta = super::shader_meta::parse(&text, name)
        .map_err(|(l, m)| format!("{}:{l}: {m}", path.display()))?;
    let modules: Vec<(String, String)> = meta
        .uses
        .iter()
        .map(|m| {
            let p = dir.join("lib").join(format!("{m}.wgsl"));
            let src = std::fs::read_to_string(&p).unwrap_or_else(|_| lib_source(m).to_string());
            (p.display().to_string(), src)
        })
        .collect();
    let entry_path = dir.join("../shaders/scene_entry.wgsl");
    let entry = std::fs::read_to_string(&entry_path).unwrap_or_else(|_| ENTRY.to_string());
    let composed = assemble(
        (meta.tonemap_mode(), meta.exposure, meta.ss_manual),
        &modules,
        (&path.display().to_string(), &text),
        &entry,
    );
    Ok((composed, meta))
}

/// Compose a scene given as text (header included) against the embedded
/// library — used by tests and tooling for scenes that are not registered.
pub fn compose_text(label: &str, stem: &str, text: &str) -> Result<(Composed, super::shader_meta::Meta), String> {
    let meta = super::shader_meta::parse(text, stem).map_err(|(l, m)| format!("{label}:{l}: {m}"))?;
    let modules: Vec<(String, String)> = meta
        .uses
        .iter()
        .map(|m| (format!("src/gpu/scenes/lib/{m}.wgsl"), lib_source(m).to_string()))
        .collect();
    let composed = assemble((meta.tonemap_mode(), meta.exposure, meta.ss_manual), &modules, (label, text), ENTRY);
    Ok((composed, meta))
}

/// Supersampling ceiling for a cost class at a detail level. The governor
/// works below this; it never exceeds it.
pub fn max_spp(cost: Cost, detail: Detail) -> u32 {
    match (cost, detail) {
        (Cost::Light, Detail::Low) => 4,
        (Cost::Light, Detail::Medium) => 9,
        (Cost::Light, Detail::High) => 16,
        (Cost::Medium, Detail::Low) => 2,
        (Cost::Medium, Detail::Medium) => 6,
        (Cost::Medium, Detail::High) => 12,
        (Cost::Heavy, Detail::Low) => 1,
        (Cost::Heavy, Detail::Medium) => 4,
        (Cost::Heavy, Detail::High) => 8,
    }
}

/// Structural quality (march steps, octaves) by detail. Shared by every pane
/// of a wall, unlike spp, so seams never show a quality step.
pub fn march_scale(detail: Detail) -> f32 {
    match detail {
        Detail::Low => 0.6,
        Detail::Medium => 1.0,
        Detail::High => 1.4,
    }
}

pub fn detail_index(detail: Detail) -> u32 {
    match detail {
        Detail::Low => 0,
        Detail::Medium => 1,
        Detail::High => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locate_maps_back_to_the_scene_file() {
        let c = assemble(
            (0, 0.0, false),
            &[("lib/a.wgsl".into(), "a1\na2\n".into())],
            ("scene.wgsl", "s1\ns2\ns3\n"),
            "e1\n",
        );
        let lines: Vec<&str> = c.source.lines().collect();
        let s2 = lines.iter().position(|l| *l == "s2").unwrap() + 1;
        assert_eq!(c.locate(s2), ("scene.wgsl".to_string(), 2));
        let a1 = lines.iter().position(|l| *l == "a1").unwrap() + 1;
        assert_eq!(c.locate(a1), ("lib/a.wgsl".to_string(), 1));
    }

    #[test]
    fn spp_ceilings_grow_with_detail() {
        for cost in [Cost::Light, Cost::Medium, Cost::Heavy] {
            assert!(max_spp(cost, Detail::Low) < max_spp(cost, Detail::Medium));
            assert!(max_spp(cost, Detail::Medium) < max_spp(cost, Detail::High));
        }
    }
}
