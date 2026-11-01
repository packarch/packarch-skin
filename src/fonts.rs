//! Global fonts: ~/.config/openbox/themes/fonts.toml replaces the [fonts]
//! section of every theme, so that the fonts chosen by the user (menu
//! Préférences › Changer de police, "packarch-skin font") do not change with
//! the theme.

use crate::apply::write_fonts;
use crate::theme::Fonts;
use crate::{bar, current, find, run, spawn, Dirs};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const HEADER: &str = "\
# Polices globales, choisies avec « packarch-skin font » (menu Préférences ›
# Changer de police) : elles remplacent la section [fonts] de chaque thème.
# polybar n'a pas de décalage « ;N » : celui du thème est gardé.

";

/// fonts.toml: the keys of the [fonts] section of theme.toml, all optional
#[derive(Deserialize, Serialize, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Global {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polybar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rofi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geany: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gtk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openbox: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openbox_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dunst: Option<String>,
}

pub fn file(dirs: &Dirs) -> PathBuf {
    dirs.themes.join("fonts.toml")
}

impl Global {
    /// Reads FILE (empty when it does not exist)
    pub fn load(file: &Path) -> Result<Global, String> {
        match fs::read_to_string(file) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{} : {e}", file.display())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Global::default()),
            Err(e) => Err(format!("{} : {e}", file.display())),
        }
    }

    pub fn save(&self, file: &Path) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|e| format!("{} : {e}", file.display()))?;
        fs::write(file, format!("{HEADER}{text}")).map_err(|e| format!("{} : {e}", file.display()))
    }

    /// The fonts of a theme, with these ones in place
    pub fn over(&self, theme: &Fonts) -> Fonts {
        let pick = |g: &Option<String>, t: &String| g.clone().unwrap_or_else(|| t.clone());
        let polybar = match (&self.polybar, theme.polybar.split_once(';')) {
            (Some(p), Some((_, offset))) if !p.contains(';') => format!("{p};{offset}"),
            (Some(p), _) => p.clone(),
            (None, _) => theme.polybar.clone(),
        };
        Fonts {
            polybar,
            rofi: pick(&self.rofi, &theme.rofi),
            terminal: pick(&self.terminal, &theme.terminal),
            terminal_size: self.terminal_size.unwrap_or(theme.terminal_size),
            geany: pick(&self.geany, &theme.geany),
            gtk: pick(&self.gtk, &theme.gtk),
            openbox: pick(&self.openbox, &theme.openbox),
            openbox_size: self.openbox_size.unwrap_or(theme.openbox_size),
            dunst: pick(&self.dunst, &theme.dunst),
        }
    }

    /// Stores the font FAMILY STYLE SIZE (as the font selector gives it,
    /// STYLE may be empty) for PART
    pub fn set(&mut self, part: Part, family: &str, style: &str, size: &str) -> Result<(), String> {
        let (family, style, size) = (family.trim(), style.trim(), size.trim());
        let n: f64 = size.parse().map_err(|_| format!("taille invalide : {size}"))?;
        if family.is_empty() || !(n > 0.0) {
            return Err(format!("police invalide : « {family} {style} {size} »"));
        }
        let pango = [family, style, size].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ");
        let whole = (n.round() as u32).max(1);
        match part {
            Part::Polybar => {
                self.polybar = Some(if style.is_empty() {
                    format!("{family}:size={size}")
                } else {
                    format!("{family}:style={style}:size={size}")
                })
            }
            Part::Rofi => self.rofi = Some(pango),
            Part::Terminal => {
                self.terminal = Some(family.into());
                self.terminal_size = Some(whole);
            }
            Part::Geany => self.geany = Some(pango),
            Part::Gtk => self.gtk = Some(pango),
            Part::Openbox => {
                self.openbox = Some(family.into());
                self.openbox_size = Some(whole);
            }
            Part::Dunst => self.dunst = Some(pango),
        }
        Ok(())
    }
}

/// A program whose font can be chosen
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Part {
    Polybar,
    Rofi,
    Terminal,
    Geany,
    Gtk,
    Openbox,
    Dunst,
}

pub const PARTS: [(&str, Part); 7] = [
    ("polybar", Part::Polybar),
    ("rofi", Part::Rofi),
    ("terminal", Part::Terminal),
    ("geany", Part::Geany),
    ("gtk", Part::Gtk),
    ("openbox", Part::Openbox),
    ("dunst", Part::Dunst),
];

impl Part {
    fn parse(name: &str) -> Result<Part, String> {
        PARTS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, p)| *p)
            .ok_or_else(|| format!("cible inconnue : {name} (polybar, rofi, terminal, geany, gtk, openbox, dunst)"))
    }
}

/// Font of PART as a Pango description "FAMILY [STYLE] SIZE" (the font
/// selector opens on it)
pub fn describe(f: &Fonts, part: Part) -> String {
    match part {
        Part::Polybar => polybar_to_pango(&f.polybar),
        Part::Rofi => f.rofi.clone(),
        Part::Terminal => format!("{} {}", f.terminal, f.terminal_size),
        Part::Geany => f.geany.clone(),
        Part::Gtk => f.gtk.clone(),
        Part::Openbox => format!("{} {}", f.openbox, f.openbox_size),
        Part::Dunst => f.dunst.clone(),
    }
}

/// "Family[:style=S|:S]:size=N[;offset]" -> "Family [S] N"
fn polybar_to_pango(value: &str) -> String {
    let base = value.split(';').next().unwrap_or("");
    let mut fields = base.split(':');
    let family = fields.next().unwrap_or("");
    let (mut style, mut size) = ("", "");
    for field in fields {
        if let Some(s) = field.strip_prefix("size=") {
            size = s;
        } else if let Some(s) = field.strip_prefix("style=") {
            style = s;
        } else if !field.contains('=') {
            style = field;
        }
    }
    [family, style, size].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ")
}

/// packarch-skin font [CIBLE [FAMILLE STYLE TAILLE]]
pub fn command(dirs: &Dirs, args: &[String]) -> Result<(), String> {
    let entry = find(dirs, &current(dirs))?;
    let path = file(dirs);
    let mut global = Global::load(&path)?;
    let fonts = global.over(&entry.load()?.fonts);
    match args {
        [] => {
            for (name, part) in PARTS {
                println!("{name}\t{}", describe(&fonts, part));
            }
            Ok(())
        }
        [name] => {
            println!("{}", describe(&fonts, Part::parse(name)?));
            Ok(())
        }
        [name, family, style, size] => {
            let part = Part::parse(name)?;
            global.set(part, family, style, size)?;
            global.save(&path)?;
            let fonts = global.over(&entry.load()?.fonts);
            write_fonts(dirs, &entry.dir, &fonts, crate::apply::Target::Session, Some(part));
            match part {
                Part::Polybar => bar::launch(dirs)?,
                Part::Terminal => {
                    run("pkill", &["-USR1", "-x", "kitty"]);
                }
                Part::Openbox => {
                    run("openbox", &["--reconfigure"]);
                }
                Part::Dunst => {
                    if run("pkill", &["-x", "dunst"]) {
                        spawn("dunst", &[], &[]);
                    }
                }
                // GTK: xfconf and Thunar in write_fonts; rofi and geany read
                // their font at each launch
                Part::Rofi | Part::Geany | Part::Gtk => {}
            }
            Ok(())
        }
        _ => Err("usage : packarch-skin font [CIBLE [FAMILLE STYLE TAILLE]]".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme_fonts() -> Fonts {
        Fonts {
            polybar: "Iosevka:size=10;3".into(),
            rofi: "Iosevka 10".into(),
            terminal: "JetBrainsMono Nerd Font".into(),
            terminal_size: 10,
            geany: "JetBrainsMono Nerd Font 10".into(),
            gtk: "Noto Sans 10".into(),
            openbox: "Sans".into(),
            openbox_size: 12,
            dunst: "Iosevka Custom 9".into(),
        }
    }

    #[test]
    fn describe_parts() {
        let f = theme_fonts();
        assert_eq!(describe(&f, Part::Polybar), "Iosevka 10");
        assert_eq!(describe(&f, Part::Terminal), "JetBrainsMono Nerd Font 10");
        assert_eq!(describe(&f, Part::Openbox), "Sans 12");
        assert_eq!(polybar_to_pango("Iosevka:style=Bold:size=11;2"), "Iosevka Bold 11");
        assert_eq!(polybar_to_pango("Iosevka:Medium:size=9;2"), "Iosevka Medium 9");
    }

    #[test]
    fn global_over_theme() {
        let mut g = Global::default();
        assert_eq!(g.over(&theme_fonts()).gtk, "Noto Sans 10");
        g.set(Part::Gtk, "Sans", "Regular", "11").unwrap();
        g.set(Part::Openbox, "Sans", "Regular", "13").unwrap();
        g.set(Part::Polybar, "Iosevka", "Bold", "11").unwrap();
        g.set(Part::Terminal, "Hack", "", "11.5").unwrap();
        let f = g.over(&theme_fonts());
        assert_eq!(f.gtk, "Sans Regular 11");
        assert_eq!((f.openbox.as_str(), f.openbox_size), ("Sans", 13));
        // The offset of the theme is kept
        assert_eq!(f.polybar, "Iosevka:style=Bold:size=11;3");
        assert_eq!((f.terminal.as_str(), f.terminal_size), ("Hack", 12));
        assert_eq!(f.rofi, "Iosevka 10");
        assert!(g.set(Part::Gtk, "Sans", "Regular", "x").is_err());
        assert!(g.set(Part::Gtk, "", "Regular", "10").is_err());
    }

    #[test]
    fn save_load() {
        let dir = std::env::temp_dir().join(format!("packarch-skin-fonts-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fonts.toml");
        assert_eq!(Global::load(&path).unwrap(), Global::default());
        let mut g = Global::default();
        g.set(Part::Gtk, "Sans", "Regular", "10").unwrap();
        g.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("gtk = \"Sans Regular 10\""));
        assert!(!text.contains("rofi"));
        assert_eq!(Global::load(&path).unwrap(), g);
        fs::remove_dir_all(&dir).unwrap();
    }
}
