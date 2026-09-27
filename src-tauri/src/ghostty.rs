//! Reads the user's Ghostty config so tiles match their real terminal:
//! font family, font size, and the resolved colour theme.
//!
//! Ghostty semantics: a `theme` is applied as a base, then explicit colour
//! keys in the config override it regardless of order.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub cursor: Option<String>,
    pub selection_background: Option<String>,
    pub selection_foreground: Option<String>,
    /// Sixteen entries, `#rrggbb` or null.
    pub palette: Vec<Option<String>>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: None,
            foreground: None,
            cursor: None,
            selection_background: None,
            selection_foreground: None,
            palette: vec![None; 16],
        }
    }
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    pub font_family: Vec<String>,
    pub font_size: Option<f32>,
    pub theme_name: Option<String>,
    pub theme: Option<Theme>,
    /// The config file that was read, for the UI to show.
    pub source: Option<String>,
}

const MAX_INCLUDE_DEPTH: usize = 3;

pub fn load() -> Appearance {
    match config_path() {
        Some(path) => load_from(&path),
        None => Appearance::default(),
    }
}

pub fn load_from(path: &Path) -> Appearance {
    let entries = read_entries(path, 0);
    let mut app = Appearance {
        source: Some(path.display().to_string()),
        ..Default::default()
    };
    let mut theme = Theme::default();
    let mut has_colors = false;

    if let Some((_, value)) = entries.iter().rev().find(|(k, _)| k == "theme") {
        let name = pick_dark(value);
        app.theme_name = Some(name.clone());
        if let Some(theme_file) = theme_path(&name, path) {
            for (k, v) in read_entries(&theme_file, 0) {
                has_colors |= apply_color(&mut theme, &k, &v);
            }
        }
    }

    for (k, v) in &entries {
        match k.as_str() {
            "font-family" => {
                if v.is_empty() {
                    app.font_family.clear();
                } else {
                    app.font_family.push(v.clone());
                }
            }
            "font-size" => app.font_size = v.parse().ok().filter(|n: &f32| *n > 0.0),
            _ => has_colors |= apply_color(&mut theme, k, v),
        }
    }
    if has_colors {
        app.theme = Some(theme);
    }
    app
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn config_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        candidates.push(PathBuf::from(xdg).join("ghostty").join("config"));
    }
    if let Some(home) = home() {
        candidates.push(home.join(".config").join("ghostty").join("config"));
        candidates.push(
            home.join("Library")
                .join("Application Support")
                .join("com.mitchellh.ghostty")
                .join("config"),
        );
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// `key = value` lines, quotes stripped, `config-file` includes expanded.
fn read_entries(path: &Path, depth: usize) -> Vec<(String, String)> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut out = Vec::new();
    for (key, value) in parse_entries(&text) {
        if key == "config-file" {
            if depth >= MAX_INCLUDE_DEPTH {
                continue;
            }
            let optional = value.starts_with('?');
            let target = value.trim_start_matches('?');
            let target = if Path::new(target).is_absolute() {
                PathBuf::from(target)
            } else {
                dir.join(target)
            };
            if target.is_file() || !optional {
                out.extend(read_entries(&target, depth + 1));
            }
        } else {
            out.push((key, value));
        }
    }
    out
}

pub fn parse_entries(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (k, v) = line.split_once('=')?;
            let key = k.trim().to_string();
            let mut value = v.trim();
            if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\'')))
            {
                value = &value[1..value.len() - 1];
            }
            Some((key, value.to_string()))
        })
        .collect()
}

/// `theme = light:X,dark:Y` picks the dark half; a plain name is itself.
pub fn pick_dark(value: &str) -> String {
    let mut light = None;
    let mut dark = None;
    for part in value.split(',') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("dark:") {
            dark = Some(rest.trim());
        } else if let Some(rest) = part.strip_prefix("light:") {
            light = Some(rest.trim());
        }
    }
    dark.or(light).unwrap_or(value.trim()).to_string()
}

fn theme_path(name: &str, config: &Path) -> Option<PathBuf> {
    let direct = Path::new(name);
    if direct.is_absolute() && direct.is_file() {
        return Some(direct.to_path_buf());
    }
    let mut candidates = Vec::new();
    if let Some(dir) = config.parent() {
        candidates.push(dir.join("themes").join(name));
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        candidates.push(PathBuf::from(xdg).join("ghostty").join("themes").join(name));
    }
    if let Some(home) = home() {
        candidates.push(home.join(".config").join("ghostty").join("themes").join(name));
    }
    if let Some(res) = std::env::var_os("GHOSTTY_RESOURCES_DIR") {
        candidates.push(PathBuf::from(res).join("themes").join(name));
    }
    candidates.push(
        PathBuf::from("/Applications/Ghostty.app/Contents/Resources/ghostty/themes").join(name),
    );
    candidates.into_iter().find(|p| p.is_file())
}

/// Applies one colour key. Returns true when the key was a colour key with
/// a parseable value. Named X11 colours are not supported and are skipped.
pub fn apply_color(theme: &mut Theme, key: &str, value: &str) -> bool {
    match key {
        "background" => set(&mut theme.background, value),
        "foreground" => set(&mut theme.foreground, value),
        "cursor-color" => set(&mut theme.cursor, value),
        "selection-background" => set(&mut theme.selection_background, value),
        "selection-foreground" => set(&mut theme.selection_foreground, value),
        "palette" => {
            let Some((idx, color)) = value.split_once('=') else {
                return false;
            };
            let Ok(idx) = idx.trim().parse::<usize>() else {
                return false;
            };
            if idx >= 16 {
                return false;
            }
            match normalize(color) {
                Some(c) => {
                    theme.palette[idx] = Some(c);
                    true
                }
                None => false,
            }
        }
        _ => false,
    }
}

fn set(slot: &mut Option<String>, value: &str) -> bool {
    match normalize(value) {
        Some(c) => {
            *slot = Some(c);
            true
        }
        None => false,
    }
}

/// `#RRGGBB`, `RRGGBB`, or `#RGB` → `#rrggbb`.
pub fn normalize(value: &str) -> Option<String> {
    let hex = value.trim().trim_start_matches('#');
    let expanded: String = match hex.len() {
        6 => hex.to_string(),
        3 => hex.chars().flat_map(|c| [c, c]).collect(),
        _ => return None,
    };
    if !expanded.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{}", expanded.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_and_strips_quotes() {
        let e = parse_entries(
            "# comment\nfont-size=20\nfont-family = \"JetBrains Mono Nerd Font\"\ntheme   = night-owl\n\nbad line\n",
        );
        assert_eq!(
            e,
            vec![
                ("font-size".into(), "20".into()),
                ("font-family".into(), "JetBrains Mono Nerd Font".into()),
                ("theme".into(), "night-owl".into()),
            ]
        );
    }

    #[test]
    fn picks_the_dark_half() {
        assert_eq!(pick_dark("night-owl"), "night-owl");
        assert_eq!(pick_dark("light:Solarized Light,dark:Solarized Dark"), "Solarized Dark");
        assert_eq!(pick_dark("light:Only Light"), "Only Light");
    }

    #[test]
    fn normalizes_colors() {
        assert_eq!(normalize("011627").as_deref(), Some("#011627"));
        assert_eq!(normalize("#ABCDEF").as_deref(), Some("#abcdef"));
        assert_eq!(normalize("#fff").as_deref(), Some("#ffffff"));
        assert_eq!(normalize("red"), None);
        assert_eq!(normalize("#12345"), None);
    }

    #[test]
    fn applies_palette_and_named_keys() {
        let mut t = Theme::default();
        assert!(apply_color(&mut t, "palette", "4=#82aaff"));
        assert!(apply_color(&mut t, "background", "011627"));
        assert!(!apply_color(&mut t, "palette", "16=#000000"));
        assert!(!apply_color(&mut t, "palette", "nonsense"));
        assert!(!apply_color(&mut t, "font-size", "20"));
        assert_eq!(t.palette[4].as_deref(), Some("#82aaff"));
        assert_eq!(t.background.as_deref(), Some("#011627"));
    }

    #[test]
    fn config_overrides_theme_and_includes_expand() {
        let dir = std::env::temp_dir().join(format!("wg-ghostty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("themes")).unwrap();
        fs::write(
            dir.join("themes").join("night-owl"),
            "palette = 0=#011627\npalette = 4=#82aaff\nbackground = 011627\nforeground = d6deeb\ncursor-color = 7e57c2\n",
        )
        .unwrap();
        fs::write(dir.join("extra"), "font-family = \"Fallback Mono\"\n").unwrap();
        fs::write(
            dir.join("config"),
            "font-size=20\nfont-family = \"JetBrains Mono Nerd Font\"\ntheme = night-owl\nforeground = 94a4b6\nconfig-file = extra\nconfig-file = ?missing\n",
        )
        .unwrap();

        let a = load_from(&dir.join("config"));
        assert_eq!(a.font_size, Some(20.0));
        assert_eq!(a.font_family, vec!["JetBrains Mono Nerd Font", "Fallback Mono"]);
        assert_eq!(a.theme_name.as_deref(), Some("night-owl"));
        let t = a.theme.expect("theme");
        assert_eq!(t.background.as_deref(), Some("#011627"));
        assert_eq!(t.foreground.as_deref(), Some("#94a4b6"), "config foreground wins over theme");
        assert_eq!(t.cursor.as_deref(), Some("#7e57c2"));
        assert_eq!(t.palette[4].as_deref(), Some("#82aaff"));
        assert!(t.palette[5].is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_config_yields_defaults() {
        let a = load_from(Path::new("/definitely/not/here/config"));
        assert_eq!(a.font_family, Vec::<String>::new());
        assert_eq!(a.font_size, None);
        assert_eq!(a.theme, None);
    }
}
