//! The theme gallery: themes people share on termpaper's website, which
//! `termpaper theme install / browse / publish` and the Themes page's
//! Gallery shelf talk to.
//!
//! termpaper never calls the gallery on its own: only these commands and
//! `g` on the Themes page do, and a request carries nothing but what it
//! asks for (an install is counted by the site, once per address, by a
//! salted hash of it).
//!
//! Parsing and the helpers are plain functions, tested without a network;
//! the requests themselves need the `net` feature (on by default).

use std::path::PathBuf;

use crate::theme::Theme;

/// Where the gallery lives unless `gallery_url` in the config or
/// `$TERMPAPER_GALLERY` says otherwise.
pub const DEFAULT_URL: &str = "https://termpaper-site.vercel.app";

/// Seconds a request may take, all in.
pub const TIMEOUT_SECS: u64 = 5;

/// The gallery to use: `$TERMPAPER_GALLERY`, else the config's
/// `gallery_url`, else [`DEFAULT_URL`]; without a trailing slash.
pub fn base_url(configured: Option<&str>) -> String {
    let env = std::env::var("TERMPAPER_GALLERY").ok().filter(|s| !s.trim().is_empty());
    let url = env.as_deref().or(configured.filter(|s| !s.trim().is_empty())).unwrap_or(DEFAULT_URL);
    url.trim().trim_end_matches('/').to_string()
}

/// The host of a gallery URL, for messages ("termpaper-site.vercel.app").
pub fn host(base: &str) -> &str {
    let rest = base.split_once("://").map_or(base, |(_, r)| r);
    rest.split('/').next().unwrap_or(rest)
}

/// A theme as the gallery lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub tags: Vec<String>,
    /// its share code: the theme itself
    pub code: String,
    pub installs: u64,
    pub likes: u64,
    /// one of termpaper's own
    pub builtin: bool,
}

impl Listed {
    /// The theme, from its share code (validated, as any code is).
    pub fn theme(&self) -> Result<Theme, String> {
        Theme::from_code(&self.code).map(|(t, _)| t)
    }
}

fn listed(v: &serde_json::Value) -> Result<Listed, String> {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let n = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
    let id = s("id");
    let code = s("code");
    if id.is_empty() || !code.starts_with("tp1:") {
        return Err("the gallery sent something that is not a theme".into());
    }
    Ok(Listed {
        id,
        name: s("name"),
        author: s("author"),
        description: s("description"),
        tags: v
            .get("tags")
            .and_then(|t| t.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default(),
        code,
        installs: n("installs"),
        likes: n("likes"),
        builtin: v.get("builtin").and_then(|b| b.as_bool()).unwrap_or(false),
    })
}

fn json(text: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(text).map_err(|_| "the gallery's answer was not what termpaper expects (is gallery_url right?)".to_string())
}

/// A page of the list: its themes, and the offset of the next page.
pub fn parse_list(text: &str) -> Result<(Vec<Listed>, Option<u64>), String> {
    let v = json(text)?;
    let items = v.get("themes").and_then(|t| t.as_array()).ok_or("the gallery's answer has no themes in it")?;
    let themes = items.iter().filter_map(|t| listed(t).ok()).collect();
    Ok((themes, v.get("next").and_then(|n| n.as_u64())))
}

/// One theme.
pub fn parse_item(text: &str) -> Result<Listed, String> {
    listed(&json(text)?)
}

/// The message in an error answer (`{"error": "..."}`), else the status.
pub fn parse_error(text: &str, status: u16) -> String {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
        .unwrap_or_else(|| format!("the gallery answered {status}"))
}

/// What `theme install` was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// a share code: no gallery needed
    Code(String),
    /// a theme in a gallery: its id, and the gallery when the link named
    /// one
    Id { base: Option<String>, id: String },
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Read what someone pasted: a `tp1:` code, a link to a theme's page
/// (`https://…/t/<id>`, `…/themes/view/?id=<id>`, an API link), or an id.
pub fn target(s: &str) -> Result<Target, String> {
    let s = s.trim();
    if s.starts_with("tp1:") {
        return Ok(Target::Code(s.to_string()));
    }
    if let Some((scheme, rest)) = s.split_once("://") {
        if scheme != "https" && scheme != "http" {
            return Err(format!("not a gallery link: {s}"));
        }
        let (hostpart, path) = rest.split_once('/').unwrap_or((rest, ""));
        let base = format!("{scheme}://{hostpart}");
        let (path, query) = path.split_once('?').unwrap_or((path, ""));
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        let id = match parts.as_slice() {
            ["t", id, ..] => Some(id.to_string()),
            ["api", "themes", id, ..] => Some(id.to_string()),
            ["themes", "view"] | ["themes", "studio"] => query
                .split('&')
                .find_map(|kv| kv.strip_prefix("id=").map(str::to_string)),
            _ => None,
        };
        return match id {
            Some(id) if valid_id(&id) => Ok(Target::Id { base: Some(base), id }),
            _ => Err(format!("that link is not to a theme: {s}")),
        };
    }
    if valid_id(s) {
        Ok(Target::Id { base: None, id: s.to_string() })
    } else {
        Err(format!("not a theme id, link or tp1: code: {s}"))
    }
}

/// What publishing answered.
#[derive(Clone, Debug, PartialEq)]
pub struct Published {
    pub id: String,
    /// lets you delete it later; kept in the state folder
    pub token: String,
    /// the theme's page, absolute
    pub url: String,
    pub warnings: Vec<String>,
}

pub fn parse_published(text: &str, base: &str) -> Result<Published, String> {
    let v = json(text)?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let (id, token, url) = (s("id"), s("token"), s("url"));
    if id.is_empty() || token.is_empty() {
        return Err("the gallery did not say where the theme went".into());
    }
    let url = if url.starts_with('/') { format!("{base}{url}") } else { url };
    let warnings = v
        .get("warnings")
        .and_then(|w| w.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    Ok(Published { id, token, url, warnings })
}

/// The body `POST /api/themes` takes: `{"theme": {...}}`, serialized
/// straight from the struct so every f32 goes out as written (0.7, not
/// the 0.699999988079071 a serde_json::Value would widen it to).
pub fn publish_body(theme: &Theme) -> String {
    #[derive(serde::Serialize)]
    struct Body<'a> {
        theme: &'a Theme,
    }
    serde_json::to_string(&Body { theme }).unwrap_or_default()
}

// ------------------------------------------------------------------ edit tokens

fn tokens_path() -> Option<PathBuf> {
    crate::platform::state_dir().map(|d| d.join("gallery-tokens.json"))
}

fn read_tokens() -> serde_json::Map<String, serde_json::Value> {
    tokens_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn write_tokens(map: &serde_json::Map<String, serde_json::Value>) -> std::io::Result<PathBuf> {
    let p = tokens_path().ok_or_else(|| std::io::Error::other("no state folder"))?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(map).unwrap_or_default())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(p)
}

/// Keep the edit token for a theme you published (`host/id` → token).
pub fn save_token(base: &str, id: &str, token: &str) -> std::io::Result<PathBuf> {
    let mut map = read_tokens();
    map.insert(format!("{}/{id}", host(base)), serde_json::Value::String(token.to_string()));
    write_tokens(&map)
}

pub fn token(base: &str, id: &str) -> Option<String> {
    read_tokens().get(&format!("{}/{id}", host(base))).and_then(|v| v.as_str()).map(str::to_string)
}

pub fn forget_token(base: &str, id: &str) {
    let mut map = read_tokens();
    if map.remove(&format!("{}/{id}", host(base))).is_some() {
        let _ = write_tokens(&map);
    }
}

// ------------------------------------------------------------------ requests

#[cfg(feature = "net")]
mod net {
    use std::time::Duration;

    use super::*;

    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(TIMEOUT_SECS)))
            .user_agent(concat!("termpaper/", env!("CARGO_PKG_VERSION")))
            .http_status_as_error(false)
            .build()
            .into()
    }

    fn unreachable(base: &str, e: ureq::Error) -> String {
        match e {
            ureq::Error::Timeout(_) => format!("{} did not answer within {TIMEOUT_SECS} s", host(base)),
            e => format!("could not reach {}: {e}", host(base)),
        }
    }

    /// Status and body of an answer.
    fn read(base: &str, r: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<(u16, String), String> {
        let mut r = r.map_err(|e| unreachable(base, e))?;
        let status = r.status().as_u16();
        let body = r.body_mut().with_config().limit(1 << 20).read_to_string().map_err(|e| unreachable(base, e))?;
        Ok((status, body))
    }

    fn ok(base: &str, r: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<String, String> {
        let (status, body) = read(base, r)?;
        if status >= 400 {
            return Err(parse_error(&body, status));
        }
        Ok(body)
    }

    fn enc(s: &str) -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                b' ' => "+".to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    }

    /// Themes people shared: matching `query` (every word), with `tag`,
    /// most installed first when `popular`.
    pub fn list(base: &str, query: &str, tag: Option<&str>, popular: bool, offset: u64) -> Result<(Vec<Listed>, Option<u64>), String> {
        let mut q = vec![format!("sort={}", if popular { "popular" } else { "new" })];
        if !query.trim().is_empty() {
            q.push(format!("q={}", enc(query.trim())));
        }
        if let Some(t) = tag {
            q.push(format!("tag={}", enc(t)));
        }
        if offset > 0 {
            q.push(format!("offset={offset}"));
        }
        let url = format!("{base}/api/themes?{}", q.join("&"));
        parse_list(&ok(base, agent().get(&url).call())?)
    }

    /// One theme by id (a shared one, or a built-in's slug).
    pub fn fetch(base: &str, id: &str) -> Result<Listed, String> {
        let url = format!("{base}/api/themes/{}", enc(id));
        match read(base, agent().get(&url).call())? {
            (404, _) => Err(format!("no theme {id} in the gallery at {} (deleted, or hidden after reports)", host(base))),
            (s, body) if s >= 400 => Err(parse_error(&body, s)),
            (_, body) => parse_item(&body),
        }
    }

    /// Count an install (best effort: an error changes nothing for you).
    pub fn count_install(base: &str, id: &str) {
        let url = format!("{base}/api/themes/{}/install", enc(id));
        let _ = agent().post(&url).send_empty();
    }

    /// Share a theme in the gallery.
    pub fn publish(base: &str, theme: &Theme) -> Result<Published, String> {
        let url = format!("{base}/api/themes");
        let body = ok(base, agent().post(&url).header("content-type", "application/json").send(publish_body(theme)))?;
        parse_published(&body, base)
    }

    /// Take a theme you published down again.
    pub fn unpublish(base: &str, id: &str, token: &str) -> Result<(), String> {
        let url = format!("{base}/api/themes/{}", enc(id));
        ok(base, agent().delete(&url).header("authorization", format!("Bearer {token}")).call()).map(|_| ())
    }
}

#[cfg(feature = "net")]
pub use net::{count_install, fetch, list, publish, unpublish};

/// Said when a gallery command runs in a build without the `net` feature.
pub const NO_NET: &str = "this termpaper was built without the gallery (the `net` feature); share codes still work: termpaper theme import tp1:…";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gallery_url_comes_from_the_environment_the_config_or_the_default() {
        // (TERMPAPER_GALLERY is not set in the test environment)
        if std::env::var_os("TERMPAPER_GALLERY").is_none() {
            assert_eq!(base_url(None), DEFAULT_URL);
            assert_eq!(base_url(Some("https://example.org/")), "https://example.org");
            assert_eq!(base_url(Some("  ")), DEFAULT_URL);
        }
        assert_eq!(host("https://termpaper-site.vercel.app"), "termpaper-site.vercel.app");
        assert_eq!(host("http://localhost:5173/x"), "localhost:5173");
    }

    #[test]
    fn install_takes_codes_links_and_ids() {
        assert_eq!(target(" tp1:abc ").unwrap(), Target::Code("tp1:abc".into()));
        assert_eq!(target("8aV7gsQH").unwrap(), Target::Id { base: None, id: "8aV7gsQH".into() });
        assert_eq!(target("tokyo-night").unwrap(), Target::Id { base: None, id: "tokyo-night".into() });
        let link = |s: &str, base: &str, id: &str| {
            assert_eq!(target(s).unwrap(), Target::Id { base: Some(base.into()), id: id.into() }, "{s}");
        };
        link("https://g.example/t/8aV7gsQH", "https://g.example", "8aV7gsQH");
        link("https://g.example/t/8aV7gsQH/", "https://g.example", "8aV7gsQH");
        link("http://localhost:5173/t/abc", "http://localhost:5173", "abc");
        link("https://g.example/themes/view/?id=nord", "https://g.example", "nord");
        link("https://g.example/themes/studio/?x=1&id=nord", "https://g.example", "nord");
        link("https://g.example/api/themes/abc123?format=toml", "https://g.example", "abc123");
        assert!(target("https://g.example/themes/").is_err());
        assert!(target("ftp://g.example/t/abc").is_err());
        assert!(target("https://g.example/t/../../etc").is_err());
        assert!(target("two words").is_err());
        assert!(target("").is_err());
    }

    #[test]
    fn answers_parse_and_bad_ones_say_why() {
        let t = crate::theme::Store::load_from(None).get("nord").unwrap().theme.clone();
        let code = t.to_code();
        let item = format!(
            r#"{{"id":"abc","name":"Nord","author":"me","description":"d","tags":["cool","dark"],"code":"{code}","installs":3,"likes":1,"created_at":"2026-09-29T00:00:00Z"}}"#
        );
        let l = parse_item(&item).unwrap();
        assert_eq!(l.id, "abc");
        assert_eq!(l.tags, vec!["cool", "dark"]);
        assert_eq!((l.installs, l.likes, l.builtin), (3, 1, false));
        assert_eq!(l.theme().unwrap().look, t.look);
        let list = format!(r#"{{"themes":[{item},{{"id":"x","code":"nope"}}],"next":24}}"#);
        let (themes, next) = parse_list(&list).unwrap();
        assert_eq!(themes.len(), 1, "entries that are not themes are skipped");
        assert_eq!(next, Some(24));
        assert_eq!(parse_list(r#"{"themes":[]}"#).unwrap(), (vec![], None));
        assert!(parse_list("<!doctype html>").unwrap_err().contains("gallery_url"));
        assert!(parse_item(r#"{"id":"x"}"#).is_err());
        assert_eq!(parse_error(r#"{"error":"5 themes an hour is the limit"}"#, 429), "5 themes an hour is the limit");
        assert_eq!(parse_error("oops", 502), "the gallery answered 502");
    }

    #[test]
    fn publishing_sends_the_theme_and_reads_back_where_it_went() {
        let t = crate::theme::Store::load_from(None).get("nord").unwrap().theme.clone();
        let body: serde_json::Value = serde_json::from_str(&publish_body(&t)).unwrap();
        // the file format, flattened: what the site's validator reads
        assert_eq!(body["theme"]["name"], "Nord");
        assert!(body["theme"]["palette"]["colors"].is_array());
        assert!(body["theme"].get("look").is_none());
        // amounts go out as written, not widened from f32
        let mut t2 = t.clone();
        t2.look.effects.set_amount("grain", 0.3);
        assert!(publish_body(&t2).contains("\"grain\":0.3"), "{}", publish_body(&t2));
        let p = parse_published(r#"{"id":"k3","token":"tok","url":"/t/k3","warnings":["clamped"]}"#, "https://g.example").unwrap();
        assert_eq!(p.url, "https://g.example/t/k3");
        assert_eq!(p.warnings, vec!["clamped"]);
        assert!(parse_published(r#"{"id":"k3"}"#, "https://g.example").is_err());
    }
}
