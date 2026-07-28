//! Scene Marketplace — browse, install, and rebuild community scenes from GitHub.
//!
//! Catalog: `marketplace/index.json` in the termpaper repo.
//! Installed packages live under the XDG data dir; `build.rs` compiles them in
//! on the next `cargo install`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const DEFAULT_INDEX_URL: &str =
    "https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/marketplace/index.json";
pub const MARKETPLACE_DOCS: &str =
    "https://github.com/Aphrodine-wq/termpaper/tree/main/marketplace";
pub const INDEX_PR_URL: &str =
    "https://github.com/Aphrodine-wq/termpaper/edit/main/marketplace/index.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceConfig {
    #[serde(default = "default_index_url")]
    pub index_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_dir: Option<String>,
    #[serde(default)]
    pub installed: Vec<InstalledEntry>,
}

fn default_index_url() -> String {
    DEFAULT_INDEX_URL.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledEntry {
    pub id: String,
    pub scene_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub struct_name: Option<String>,
    pub description: String,
    #[serde(default)]
    pub themes: Vec<String>,
    pub repo: String,
    pub install_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Index {
    pub version: u32,
    pub scenes: Vec<IndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub repo: String,
    pub scene_name: String,
    #[serde(default)]
    pub struct_name: Option<String>,
    #[serde(default)]
    pub themes: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub subdir: Option<String>,
    #[serde(default)]
    pub min_termpaper: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SceneManifest {
    id: String,
    scene_name: String,
    #[serde(default)]
    struct_name: Option<String>,
    description: String,
    #[serde(default)]
    themes: Vec<String>,
    #[serde(default)]
    repo: Option<String>,
}

pub fn config_path() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("termpaper/marketplace.toml"));
        }
    }
    dirs_home().map(|h| h.join(".config/termpaper/marketplace.toml"))
}

pub fn data_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("termpaper/marketplace"));
        }
    }
    dirs_home().map(|h| h.join(".local/share/termpaper/marketplace"))
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

pub fn load_config() -> MarketplaceConfig {
    let Some(path) = config_path() else {
        return MarketplaceConfig {
            index_url: DEFAULT_INDEX_URL.to_string(),
            source_dir: None,
            installed: Vec::new(),
        };
    };
    match fs::read_to_string(&path) {
        Ok(raw) => toml::from_str(&raw).unwrap_or_else(|e| {
            eprintln!("termpaper: warning: {path:?}: {e}");
            MarketplaceConfig {
                index_url: DEFAULT_INDEX_URL.to_string(),
                source_dir: None,
                installed: Vec::new(),
            }
        }),
        Err(_) => MarketplaceConfig {
            index_url: DEFAULT_INDEX_URL.to_string(),
            source_dir: None,
            installed: Vec::new(),
        },
    }
}

pub fn save_config(cfg: &MarketplaceConfig) -> io::Result<()> {
    let path = config_path().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "no config directory")
    })?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, toml::to_string_pretty(cfg).map_err(|e| io::Error::other(e))?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn set_source_dir(dir: &Path) -> io::Result<()> {
    let mut cfg = load_config();
    cfg.source_dir = Some(dir.to_string_lossy().into_owned());
    save_config(&cfg)
}

pub fn fetch_index(url: &str) -> io::Result<Index> {
    let body = ureq::get(url)
        .call()
        .map_err(|e| io::Error::other(e.to_string()))?
        .into_body()
        .read_to_string()
        .map_err(|e| io::Error::other(e.to_string()))?;
    serde_json::from_str(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn install_dir(id: &str) -> Option<PathBuf> {
    data_dir().map(|d| d.join(sanitize_id(id)))
}

fn run_git(args: &[&str], cwd: Option<&Path>) -> io::Result<()> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(c) = cwd {
        cmd.current_dir(c);
    }
    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "git {} failed (exit {})",
            args.join(" "),
            status.code().unwrap_or(-1)
        )))
    }
}

fn copy_dir_files(src: &Path, dest: &Path) -> io::Result<()> {
    fs::create_dir_all(dest)?;
    for name in ["scene.rs", "scene.toml", "README.md", "LICENSE"] {
        let from = src.join(name);
        if from.is_file() {
            fs::copy(&from, dest.join(name))?;
        }
    }
    Ok(())
}

fn read_manifest(dir: &Path) -> io::Result<SceneManifest> {
    let path = dir.join("scene.toml");
    let raw = fs::read_to_string(&path)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "missing scene.toml"))?;
    toml::from_str(&raw).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn install(entry: &IndexEntry) -> io::Result<InstalledEntry> {
    let dest = install_dir(&entry.id).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "no data directory (HOME unset?)")
    })?;
    if dest.exists() {
        fs::remove_dir_all(&dest)?;
    }
    fs::create_dir_all(&dest)?;

    let tmp = dest.with_extension("clone");
    if tmp.exists() {
        fs::remove_dir_all(&tmp)?;
    }

    run_git(
        &[
            "clone",
            "--depth",
            "1",
            &entry.repo,
            tmp.to_str().unwrap_or("clone"),
        ],
        None,
    )?;

    let src = match &entry.subdir {
        Some(sub) => tmp.join(sub),
        None => tmp.clone(),
    };
    if !src.join("scene.rs").is_file() {
        let _ = fs::remove_dir_all(&tmp);
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "repo has no scene.rs (check subdir in index)",
        ));
    }

    copy_dir_files(&src, &dest)?;
    let _ = fs::remove_dir_all(&tmp);

    let manifest = read_manifest(&dest)?;
    if manifest.scene_name != entry.scene_name {
        eprintln!(
            "termpaper: warning: index scene_name '{}' != manifest '{}'",
            entry.scene_name, manifest.scene_name
        );
    }

    let installed = InstalledEntry {
        id: entry.id.clone(),
        scene_name: manifest.scene_name,
        struct_name: manifest.struct_name.or(entry.struct_name.clone()),
        description: manifest.description,
        themes: if manifest.themes.is_empty() {
            entry.themes.clone()
        } else {
            manifest.themes
        },
        repo: entry.repo.clone(),
        install_path: dest.to_string_lossy().into_owned(),
    };

    let mut cfg = load_config();
    cfg.installed.retain(|e| e.id != entry.id);
    cfg.installed.push(installed.clone());
    save_config(&cfg)?;
    Ok(installed)
}

pub fn remove(id: &str) -> io::Result<()> {
    let mut cfg = load_config();
    let Some(pos) = cfg.installed.iter().position(|e| e.id == id) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("not installed: {id}"),
        ));
    };
    let path = PathBuf::from(&cfg.installed[pos].install_path);
    cfg.installed.remove(pos);
    save_config(&cfg)?;
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    println!("removed {id}");
    Ok(())
}

pub fn find_index_entry<'a>(index: &'a Index, query: &str) -> Option<&'a IndexEntry> {
    let q = query.to_lowercase();
    index
        .scenes
        .iter()
        .find(|e| e.id.eq_ignore_ascii_case(query) || e.scene_name.eq_ignore_ascii_case(query))
        .or_else(|| {
            index.scenes.iter().find(|e| {
                e.id.to_lowercase().contains(&q)
                    || e.name.to_lowercase().contains(&q)
                    || e.scene_name.to_lowercase().contains(&q)
            })
        })
}

pub fn default_source_dir() -> Option<PathBuf> {
    let cfg = load_config();
    if let Some(d) = cfg.source_dir {
        let p = PathBuf::from(d);
        if p.join("Cargo.toml").is_file() {
            return Some(p);
        }
    }
    if let Ok(src) = std::env::var("TERMPAPER_SRC") {
        let p = PathBuf::from(src);
        if p.join("Cargo.toml").is_file() {
            return Some(p);
        }
    }
    dirs_home().map(|h| h.join(".local/share/termpaper/src"))
        .filter(|p| p.join("Cargo.toml").is_file())
}

pub fn rebuild() -> io::Result<()> {
    let src = default_source_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "termpaper source not found — reinstall with ./install.sh from a clone, \
             or set TERMPAPER_SRC=/path/to/termpaper",
        )
    })?;
    let status = Command::new("cargo")
        .args(["install", "--path", src.to_str().unwrap_or("."), "--locked", "--force"])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("cargo install failed"))
    }
}

pub fn cmd_list(json: bool) -> io::Result<()> {
    let cfg = load_config();
    let index = fetch_index(&cfg.index_url)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&index.scenes).unwrap());
        return Ok(());
    }
    println!("Scene Marketplace ({} scenes)\n", index.scenes.len());
    for e in &index.scenes {
        let tag = if cfg.installed.iter().any(|i| i.id == e.id) {
            " [installed]"
        } else {
            ""
        };
        println!(
            "  {:<14}  {:<24}  {}{tag}",
            e.scene_name, e.author, e.description
        );
    }
    println!("\n  termpaper marketplace install <id>     install a scene");
    println!("  termpaper marketplace info <id>        details + repo URL");
    println!("  Docs: {MARKETPLACE_DOCS}");
    Ok(())
}

pub fn cmd_search(query: &str) -> io::Result<()> {
    let cfg = load_config();
    let index = fetch_index(&cfg.index_url)?;
    let q = query.to_lowercase();
    let hits: Vec<_> = index
        .scenes
        .iter()
        .filter(|e| {
            e.id.to_lowercase().contains(&q)
                || e.name.to_lowercase().contains(&q)
                || e.description.to_lowercase().contains(&q)
                || e.author.to_lowercase().contains(&q)
                || e.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .collect();
    if hits.is_empty() {
        println!("no matches for '{query}'");
        return Ok(());
    }
    for e in hits {
        println!("  {}  {} — {}", e.id, e.scene_name, e.description);
    }
    Ok(())
}

pub fn cmd_info(query: &str) -> io::Result<()> {
    let cfg = load_config();
    let index = fetch_index(&cfg.index_url)?;
    let Some(e) = find_index_entry(&index, query) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("unknown marketplace id or scene: {query}"),
        ));
    };
    println!("id:          {}", e.id);
    println!("scene:       {}", e.scene_name);
    println!("author:      {}", e.author);
    println!("description: {}", e.description);
    println!("repo:        {}", e.repo);
    if let Some(sub) = &e.subdir {
        println!("subdir:      {sub}");
    }
    if !e.themes.is_empty() {
        println!("themes:      {}", e.themes.join(", "));
    }
    if !e.tags.is_empty() {
        println!("tags:        {}", e.tags.join(", "));
    }
    if cfg.installed.iter().any(|i| i.id == e.id) {
        println!("\nstatus:      installed");
        println!("run:         termpaper {}", e.scene_name);
    } else {
        println!("\ninstall:     termpaper marketplace install {}", e.id);
    }
    Ok(())
}

pub fn cmd_install(query: &str, no_rebuild: bool) -> io::Result<()> {
    let cfg = load_config();
    let index = fetch_index(&cfg.index_url)?;
    let Some(entry) = find_index_entry(&index, query) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("unknown marketplace id or scene: {query}"),
        ));
    };
    if !Command::new("git").arg("--version").status().map(|s| s.success()).unwrap_or(false) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "git is required to install marketplace scenes",
        ));
    }
    let installed = install(entry)?;
    println!(
        "installed {} → {}",
        installed.id, installed.install_path
    );
    if no_rebuild {
        println!("\nrebuild skipped. Run:  termpaper marketplace rebuild");
        return Ok(());
    }
    print!("rebuilding termpaper with community scenes… ");
    io::stdout().flush()?;
    match rebuild() {
        Ok(()) => {
            println!("done.");
            println!("\n  termpaper {}", installed.scene_name);
        }
        Err(e) => {
            println!("failed.");
            eprintln!("{e}");
            eprintln!("\nInstall Rust source, then run:  termpaper marketplace rebuild");
        }
    }
    Ok(())
}

pub fn cmd_installed() -> io::Result<()> {
    let cfg = load_config();
    if cfg.installed.is_empty() {
        println!("no marketplace scenes installed");
        println!("  termpaper marketplace list");
        return Ok(());
    }
    for e in &cfg.installed {
        println!("  {:<14}  {}  {}", e.scene_name, e.id, e.description);
    }
    println!("\n  termpaper marketplace rebuild   recompile with installed scenes");
    Ok(())
}

pub fn cmd_update() -> io::Result<()> {
    let cfg = load_config();
    if cfg.installed.is_empty() {
        println!("nothing installed");
        return Ok(());
    }
    let index = fetch_index(&cfg.index_url)?;
    for id in cfg.installed.iter().map(|e| e.id.clone()).collect::<Vec<_>>() {
        let Some(entry) = find_index_entry(&index, &id) else {
            eprintln!("warning: {id} no longer in catalog, skipping");
            continue;
        };
        print!("updating {}… ", entry.id);
        io::stdout().flush()?;
        match install(entry) {
            Ok(_) => println!("ok"),
            Err(e) => println!("failed: {e}"),
        }
    }
    println!("\nrun:  termpaper marketplace rebuild");
    Ok(())
}

pub fn cmd_publish() -> io::Result<()> {
    println!("Publish a scene to the Scene Marketplace\n");
    println!("1. Copy the template:");
    println!("   {MARKETPLACE_DOCS}/template/");
    println!("\n2. Implement scene.rs (see CONTRIBUTING.md) and fill scene.toml");
    println!("\n3. Push to your GitHub repo (public)");
    println!("\n4. Open a PR adding your entry to marketplace/index.json:");
    println!("   {INDEX_PR_URL}");
    println!("\n5. After merge, anyone can install:");
    println!("   termpaper marketplace install your-user/your-scene-id");
    Ok(())
}

/// Short lines for the in-app menu (installed + hint).
pub fn menu_lines() -> Vec<String> {
    let cfg = load_config();
    let mut lines = Vec::new();
    if cfg.installed.is_empty() {
        lines.push("no community scenes installed".into());
    } else {
        for e in &cfg.installed {
            lines.push(format!("● {}  {}", e.scene_name, e.description));
        }
    }
    lines.push(String::new());
    lines.push("termpaper marketplace list".into());
    lines.push("termpaper marketplace install <id>".into());
    lines.push("termpaper marketplace publish".into());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_id_replaces_slashes() {
        assert_eq!(sanitize_id("user/cool-scene"), "user-cool-scene");
    }

    #[test]
    fn parse_index_json() {
        let raw = r#"{"version":1,"scenes":[{"id":"a/b","name":"x","description":"d","author":"u","repo":"https://github.com/u/r","scene_name":"x"}]}"#;
        let index: Index = serde_json::from_str(raw).unwrap();
        assert_eq!(index.scenes[0].scene_name, "x");
    }
}
