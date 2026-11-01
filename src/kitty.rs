//! Kitty graphics output: a frame as one image under the text layer.
//!
//! The pixels go through POSIX shared memory (`t=s`), which kitty maps,
//! reads and unlinks, so a frame costs the terminal one short escape instead
//! of a screenful of colour codes. The image sits below cells with a
//! non-default background (`z` past INT32_MIN/2), so the clock, the menu and
//! every other overlay still draw on top.
//!
//! Only a local kitty qualifies: shared memory needs the terminal on the
//! same machine, and a multiplexer would swallow the escapes.

use std::io::{self, Write};

/// Under text and under cells with a non-default background colour.
const Z_UNDER_BACKGROUNDS: i32 = -1_073_741_825;

/// Whether this environment shows kitty graphics through shared memory.
pub fn supported() -> bool {
    supported_with(|k| std::env::var(k).ok().filter(|v| !v.is_empty()), std::path::Path::new("/dev/shm").is_dir())
}

pub fn supported_with(get: impl Fn(&str) -> Option<String>, have_shm: bool) -> bool {
    // TERMPAPER_KITTY=1 / 0 overrides the guess (testing, or a terminal that
    // speaks the protocol without saying so)
    match get("TERMPAPER_KITTY").as_deref() {
        Some("1") => return have_shm,
        Some("0") => return false,
        _ => {}
    }
    let kitty = get("KITTY_WINDOW_ID").is_some() || get("TERM").as_deref() == Some("xterm-kitty");
    let muxed = get("TMUX").is_some() || get("STY").is_some() || get("ZELLIJ").is_some();
    let remote = get("SSH_CONNECTION").is_some() || get("SSH_TTY").is_some();
    kitty && !muxed && !remote && have_shm
}

/// Remove frames that processes no longer running left in /dev/shm (a
/// termpaper killed before its own cleanup ran).
pub fn sweep() {
    let Ok(dir) = std::fs::read_dir("/dev/shm") else { return };
    for e in dir.flatten() {
        let name = e.file_name();
        let Some(rest) = name.to_str().and_then(|n| n.strip_prefix("termpaper-")) else { continue };
        let Some(pid) = rest.split('-').next().and_then(|p| p.parse::<u32>().ok()) else { continue };
        if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// One image id, replaced frame by frame.
pub struct Image {
    id: u32,
    serial: u64,
    shown: bool,
}

/// This process's image id: unlikely to collide with anything else in the
/// window.
fn own_id() -> u32 {
    0x7e00_0000 | (std::process::id() & 0x00ff_ffff)
}

/// Remove this process's image, whatever state the program is in (the
/// panic and exit paths that restore the terminal).
pub fn forget(out: &mut impl Write) -> io::Result<()> {
    write!(out, "\x1b_Ga=d,d=I,i={},q=2\x1b\\", own_id())
}

impl Default for Image {
    fn default() -> Self {
        Image { id: own_id(), serial: 0, shown: false }
    }
}

impl Image {
    /// Show `rgba` (`w` x `h`, 4 bytes a pixel) scaled over the cell
    /// rectangle at (`col`, `row`), `cols` x `rows` cells.
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        out: &mut impl Write,
        rgba: &[u8],
        w: usize,
        h: usize,
        col: u16,
        row: u16,
        cols: u16,
        rows: u16,
    ) -> io::Result<()> {
        debug_assert_eq!(rgba.len(), w * h * 4);
        self.serial += 1;
        // kitty unlinks a frame once it has read it; one it has not read two
        // frames later never will be (a terminal that ignored the escape),
        // and would sit in RAM
        if self.serial > 2 {
            let stale = format!("termpaper-{}-{}", std::process::id(), self.serial - 2);
            let _ = std::fs::remove_file(std::path::Path::new("/dev/shm").join(stale));
        }
        let name = format!("termpaper-{}-{}", std::process::id(), self.serial);
        let path = std::path::Path::new("/dev/shm").join(&name);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path)?;
            f.write_all(rgba)?;
        }
        #[cfg(not(unix))]
        return Err(io::Error::new(io::ErrorKind::Unsupported, format!("no shared memory for {}", path.display())));
        // the placement starts at the cursor; C=1 leaves the cursor there
        write!(out, "\x1b[{};{}H", row + 1, col + 1)?;
        write!(
            out,
            "\x1b_Ga=T,f=32,s={w},v={h},S={},t=s,i={},p=1,c={cols},r={rows},C=1,z={Z_UNDER_BACKGROUNDS},q=2;{}\x1b\\",
            rgba.len(),
            self.id,
            base64(name.as_bytes()),
        )?;
        self.shown = true;
        Ok(())
    }

    /// Remove the image (leaving kitty mode, or quitting).
    pub fn clear(&mut self, out: &mut impl Write) -> io::Result<()> {
        if self.shown {
            write!(out, "\x1b_Ga=d,d=I,i={},q=2\x1b\\", self.id)?;
            self.shown = false;
        }
        Ok(())
    }
}

impl Drop for Image {
    /// Kitty unlinks each frame once it has read it; anything it never got to
    /// (a frame written as the program exits) is ours to remove.
    fn drop(&mut self) {
        let prefix = format!("termpaper-{}-", std::process::id());
        if let Ok(dir) = std::fs::read_dir("/dev/shm") {
            for e in dir.flatten() {
                if e.file_name().to_string_lossy().starts_with(&prefix) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_rfc_vectors() {
        for (i, o) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foobar", "Zm9vYmFy")] {
            assert_eq!(base64(i.as_bytes()), o);
        }
    }

    #[test]
    fn only_a_local_unmultiplexed_kitty() {
        let env = |vars: &'static [(&'static str, &'static str)]| {
            move |k: &str| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
        };
        assert!(supported_with(env(&[("KITTY_WINDOW_ID", "1")]), true));
        assert!(supported_with(env(&[("TERM", "xterm-kitty")]), true));
        assert!(!supported_with(env(&[("KITTY_WINDOW_ID", "1")]), false));
        assert!(!supported_with(env(&[("KITTY_WINDOW_ID", "1"), ("TMUX", "/tmp/x")]), true));
        assert!(!supported_with(env(&[("KITTY_WINDOW_ID", "1"), ("SSH_CONNECTION", "a b c d")]), true));
        assert!(!supported_with(env(&[("TERM", "xterm-ghostty")]), true));
    }

    /// One frame: the pixels land in /dev/shm under the name the escape
    /// carries, and the escape places them over the requested cells.
    #[test]
    fn a_frame_is_one_escape_and_one_shm_object() {
        let mut img = Image { id: 7, serial: 1000, shown: false };
        let mut out = Vec::new();
        let rgba = vec![9u8; 4 * 8 * 4];
        img.show(&mut out, &rgba, 4, 8, 2, 3, 1, 1).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("\x1b[4;3H\x1b_Ga=T,f=32,s=4,v=8,S=128,t=s,i=7,p=1,c=1,r=1,C=1,"), "{s:?}");
        let name = format!("termpaper-{}-1001", std::process::id());
        assert!(s.ends_with(&format!(";{}\x1b\\", base64(name.as_bytes()))));
        let path = std::path::Path::new("/dev/shm").join(&name);
        assert_eq!(std::fs::read(&path).unwrap(), rgba);
        std::fs::remove_file(path).unwrap();
    }
}
