//! theme.toml: description of a theme (one per theme directory of
//! ~/.config/<desktop>/themes; on Openbox next to its polybar/ and rofi/
//! layouts).

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// Name shown in the menus (default: the directory name, capitalized)
    pub name: Option<String>,
    /// Wallpaper, absolute or relative to the theme directory
    #[serde(default = "default_wallpaper")]
    pub wallpaper: String,
    /// Named colors or expressions (see color.rs). Required: background,
    /// foreground, color0..color15 (terminal palette)
    pub colors: BTreeMap<String, String>,
    #[serde(default)]
    pub roles: Roles,
    pub fonts: Fonts,
    pub appearance: Appearance,
    pub openbox: Openbox,
    pub dunst: Dunst,
    /// Ignored: plank was removed, old themes copied to ~/.config still have it
    #[serde(default, rename = "plank")]
    _plank: Option<toml::Table>,
    pub picom: Picom,
    #[serde(default)]
    pub polybar: Polybar,
    /// Settings of the XFCE desktop only
    #[serde(default)]
    pub xfce: Xfce,
    /// Settings of the Sway desktop only
    #[serde(default)]
    pub sway: Sway,
}

fn default_wallpaper() -> String {
    "wallpaper".into()
}

/// Colors of the terminal, the bar, rofi and dunst: color expressions
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields, default)]
pub struct Roles {
    pub terminal_background: String,
    pub terminal_foreground: String,
    pub bar_background: String,
    pub bar_foreground: String,
    pub bar_alt_background: String,
    pub bar_alt_foreground: String,
    pub bar_accent: String,
    pub rofi_background: String,
    pub rofi_alt_background: String,
    pub rofi_foreground: String,
    pub rofi_selected: String,
    pub rofi_active: String,
    pub rofi_urgent: String,
    pub dunst_background: String,
    pub dunst_foreground: String,
    pub dunst_frame: String,
    pub dunst_urgent: String,
    /// Border of the area selection of the screenshots
    pub screenshot_border: String,
    /// Logo, title and keys of fastfetch (none: its own colors)
    pub fastfetch: Option<String>,
    /// Inactive windows of the window list script polywins.sh (none:
    /// bar_foreground)
    pub polywins_inactive: Option<String>,
    /// Cursor of the terminal (none: terminal_foreground in kitty,
    /// bar_accent in xfce4-terminal)
    pub terminal_cursor: Option<String>,
    /// Selected text of kitty (none: terminal_foreground on
    /// terminal_background, reversed)
    pub terminal_selection_background: Option<String>,
    pub terminal_selection_foreground: Option<String>,
    /// Windows of Sway: background of the frames and text of the unfocused
    /// title bars (none: bar_background, bar_foreground), focused window
    /// (bar_accent), focused window of an unfocused container
    /// (bar_alt_foreground), unfocused windows (bar_alt_background), urgent
    /// window (color1), split indicator (color2)
    pub window_background: Option<String>,
    pub window_foreground: Option<String>,
    pub window_focused: Option<String>,
    pub window_focused_inactive: Option<String>,
    pub window_unfocused: Option<String>,
    pub window_urgent: Option<String>,
    pub window_indicator: Option<String>,
    /// Lock screen of Sway (hyprlock): accent (none: bar_accent), text
    /// (bar_foreground), password field (bar_alt_background, with the
    /// lock_ring_opacity of [sway]), right and wrong password (color2,
    /// color1)
    pub lock_accent: Option<String>,
    pub lock_text: Option<String>,
    pub lock_field: Option<String>,
    pub lock_check: Option<String>,
    pub lock_fail: Option<String>,
}

impl Default for Roles {
    fn default() -> Roles {
        let s = |x: &str| x.to_string();
        Roles {
            terminal_background: s("background"),
            terminal_foreground: s("foreground"),
            bar_background: s("background"),
            bar_foreground: s("foreground"),
            bar_alt_background: s("alt_background"),
            bar_alt_foreground: s("alt_foreground"),
            bar_accent: s("accent"),
            rofi_background: s("background"),
            rofi_alt_background: s("alt_background"),
            rofi_foreground: s("foreground"),
            rofi_selected: s("accent"),
            rofi_active: s("color2"),
            rofi_urgent: s("color1"),
            dunst_background: s("background"),
            dunst_foreground: s("foreground"),
            dunst_frame: s("alt_background"),
            dunst_urgent: s("color1"),
            screenshot_border: s("accent"),
            fastfetch: None,
            polywins_inactive: None,
            terminal_cursor: None,
            terminal_selection_background: None,
            terminal_selection_foreground: None,
            window_background: None,
            window_foreground: None,
            window_focused: None,
            window_focused_inactive: None,
            window_unfocused: None,
            window_urgent: None,
            window_indicator: None,
            lock_accent: None,
            lock_text: None,
            lock_field: None,
            lock_check: None,
            lock_fail: None,
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Fonts {
    /// polybar font-0 ("Family:size=N;offset")
    pub polybar: String,
    pub rofi: String,
    pub terminal: String,
    pub terminal_size: u32,
    pub geany: String,
    pub gtk: String,
    pub openbox: String,
    pub openbox_size: u32,
    pub dunst: String,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Appearance {
    pub gtk_theme: String,
    pub icon_theme: String,
    pub cursor_theme: String,
    /// GNOME color scheme: prefer-dark, prefer-light or default
    pub color_scheme: String,
    /// Icon theme of the rofi menus
    pub rofi_icons: String,
    /// Geany color scheme (file of /usr/share/geany/colorschemes, from
    /// packarch-geany-config, or of ~/.config/geany/colorschemes)
    pub geany_colors: String,
    /// Folder color of the icon theme, "#rrggbb": in the session,
    /// packarch-folder-color makes the icon theme with these folders (none:
    /// the icon theme as it is)
    #[serde(default)]
    pub folder_color: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Openbox {
    /// Window theme (OB-*)
    pub theme: String,
    /// Title bar buttons (D: desktops, L: label, I: iconify, M: maximize, C: close)
    pub layout: String,
    /// Menu file of rc.xml (menu-icons.xml, menu-glyphs.xml...)
    pub menu: String,
    /// Screen margins left free by windows: top, bottom, left, right
    pub margins: [u32; 4],
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Dunst {
    pub width: u32,
    pub height: u32,
    pub offset: String,
    pub origin: String,
    pub border: u32,
    pub separator: u32,
    /// Seconds before the notifications close, by urgency (0: never), for
    /// dunst and mako (Sway)
    #[serde(default = "timeout_low")]
    pub timeout_low: u32,
    #[serde(default = "timeout_normal")]
    pub timeout_normal: u32,
    #[serde(default)]
    pub timeout_critical: u32,
}

fn timeout_low() -> u32 {
    2
}

fn timeout_normal() -> u32 {
    5
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Picom {
    pub backend: String,
    pub corner_radius: u32,
    pub shadow_radius: u32,
    pub shadow_opacity: String,
    pub shadow_offset_x: i32,
    pub shadow_offset_y: i32,
    pub blur_method: String,
    pub blur_strength: u32,
    /// Color of the shadows (color expression; none: black), with
    /// shadow_opacity (picom shadow-color, SwayFX shadow_color)
    #[serde(default)]
    pub shadow_color: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields, default)]
pub struct Polybar {
    /// Bars of config.ini to launch on the primary monitor (on each one
    /// when none is primary)
    pub bars: Vec<String>,
    /// BACKGROUND1..BACKGROUND6 in colors.ini (and gradient1..6 colors): a
    /// gradient from bar_background to bar_alt_background
    pub gradient: bool,
    /// Colors of the window list script (polybar/scripts/polywins.sh)
    pub polywins: bool,
    /// Additional entries of colors.ini: NAME = color expression
    pub extra: BTreeMap<String, String>,
}

impl Default for Polybar {
    fn default() -> Polybar {
        Polybar {
            bars: vec!["main".into()],
            gradient: false,
            polywins: false,
            extra: BTreeMap::new(),
        }
    }
}

/// XFCE desktop: the same themes, without what is specific to Openbox
#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Xfce {
    /// Window border theme of xfwm4 (none: the GTK theme, which must have
    /// an xfwm4 directory)
    pub xfwm4_theme: Option<String>,
}

/// Sway desktop: what the other desktops do not have
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields, default)]
pub struct Sway {
    /// Opacity of the password field of the lock screen (0 to 1)
    pub lock_ring_opacity: f64,
}

impl Default for Sway {
    fn default() -> Sway {
        Sway { lock_ring_opacity: 0.5 }
    }
}

/// A theme of the themes directory
pub struct Entry {
    /// Directory name (identifier used by the commands)
    pub id: String,
    pub dir: PathBuf,
}

impl Entry {
    pub fn load(&self) -> Result<Theme, String> {
        let file = self.dir.join("theme.toml");
        let text = fs::read_to_string(&file)
            .map_err(|e| format!("{} : {e}", file.display()))?;
        toml::from_str(&text).map_err(|e| format!("{} : {e}", file.display()))
    }

    /// Thumbnail of the theme chooser: thumbnail.png of the theme directory,
    /// else ICONS/<id>.png
    pub fn thumbnail(&self, icons: Option<&Path>) -> Option<PathBuf> {
        let own = self.dir.join("thumbnail.png");
        if own.is_file() {
            return Some(own);
        }
        icons.map(|d| d.join(format!("{}.png", self.id))).filter(|p| p.is_file())
    }

    pub fn display_name(&self, theme: Option<&Theme>) -> String {
        if let Some(n) = theme.and_then(|t| t.name.clone()) {
            return n;
        }
        let mut c = self.id.chars();
        match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => String::new(),
        }
    }
}

/// Themes of DIR (directories with a theme.toml), sorted by identifier
pub fn list(dir: &Path) -> Vec<Entry> {
    let mut v: Vec<Entry> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("theme.toml").is_file())
        .map(|e| Entry { id: e.file_name().to_string_lossy().into_owned(), dir: e.path() })
        .collect();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    v
}
