//! Application of a theme (what the apply.sh script of each theme did), to
//! the running session or, at package build time, to the files of /etc/skel.
//! On XFCE, only what this desktop has: wallpaper, GTK, xfwm4, icon and
//! cursor themes, xfce4-terminal and Geany colors, theme button of the panel.
//! On Sway: see sway.rs.

use crate::color::{Color, Palette};
use crate::edit::{set_block, set_line, set_xml_text, truncate_at};
use crate::fonts::{self, Global, Part};
use crate::theme::{Appearance, Entry, Fonts, Theme};
use crate::xfce::{self, xfconf_new};
use crate::{bar, dry, run, spawn, xml_escape, Desktop, Dirs};
use std::fs;
use std::process::{Command, Stdio};
use std::path::Path;

/// Where a theme is applied
#[derive(Clone, Copy)]
pub enum Target<'a> {
    /// The running session: the files, then xfconf, gsettings, nitrogen and
    /// the reload of Openbox, dunst, kitty and the bar
    Session,
    /// The files only, under the package root ROOT (packarch-openbox-settings build):
    /// HOME is ROOT/etc/skel, the xfconf settings go to xsettings.xml, the
    /// wallpaper to nitrogen/bg-saved.cfg and the color scheme to a GSettings
    /// override in ROOT/usr/share/glib-2.0/schemas. Nothing is run.
    Package(&'a Path),
}

/// First line of the fastfetch configuration written for the "fastfetch"
/// role: a file without it belongs to the user and is not touched
const FASTFETCH_MARK: &str = "// Couleurs du thème, écrites par packarch-skin (supprimer cette ligne pour garder vos réglages)";

/// Logo of /etc/xdg/fastfetch/config.jsonc (packarch-system-settings), which
/// the file of the role replaces: packarch.png in kitty, the text logo of the
/// distribution, in the color of the role, elsewhere
const FASTFETCH_LOGO: &str = "\"type\": \"kitty\", \"source\": \"/usr/share/pixmaps/packarch.png\", \"width\": 20, \"padding\": { \"top\": 2, \"left\": 2, \"right\": 4 }";

/// Modules of fastfetch (its default list: a configuration file without
/// "modules" shows none)
const FASTFETCH_MODULES: [&str; 26] = [
    "title", "separator", "os", "host", "kernel", "shell", "display", "de", "wm",
    "wmtheme", "theme", "icons", "font", "cursor", "terminal", "terminalfont", "cpu", "gpu", "memory",
    "swap", "disk", "localip", "battery", "poweradapter", "break", "colors",
];

/// Colors of the roles, resolved
pub(crate) struct Colors {
    pub(crate) term_bg: Color,
    pub(crate) term_fg: Color,
    pub(crate) palette: Vec<Color>,
    pub(crate) bar_bg: Color,
    pub(crate) bar_fg: Color,
    pub(crate) bar_alt_bg: Color,
    pub(crate) bar_alt_fg: Color,
    pub(crate) bar_accent: Color,
    pub(crate) gradient: Vec<Color>,
    pub(crate) extra: Vec<(String, Color)>,
    pub(crate) rofi: [Color; 6],
    pub(crate) dunst: [Color; 4],
    pub(crate) border: Color,
    pub(crate) fastfetch: Option<Color>,
    pub(crate) polywins_inactive: Color,
}

fn resolve(theme: &Theme) -> Result<Colors, String> {
    if let Some(c) = &theme.appearance.folder_color
        && !is_hex_color(c)
    {
        return Err(format!("folder_color : couleur #rrggbb attendue : {c}"));
    }
    let mut defs = theme.colors.clone();
    // Defaults of the derived colors (as apply.sh computed them)
    defs.entry("alt_background".into()).or_insert("lighten(background, 0.05)".into());
    defs.entry("alt_foreground".into()).or_insert("darken(foreground, 0.30)".into());
    let mut p = Palette::new(&defs);
    let r = &theme.roles;

    let bar_bg = p.eval(&r.bar_background)?;
    let bar_alt_bg = p.eval(&r.bar_alt_background)?;
    if theme.polybar.gradient {
        p.gradient = bar_bg
            .rgb
            .gradient(bar_alt_bg.rgb, 7)
            .into_iter()
            .map(|rgb| Color { text: rgb.hex(), rgb })
            .collect();
    }
    let mut palette = Vec::with_capacity(16);
    for i in 0..16 {
        palette.push(p.get(&format!("color{i}"))?);
    }
    let mut extra = Vec::new();
    for (k, v) in &theme.polybar.extra {
        extra.push((k.clone(), p.eval(v)?));
    }
    Ok(Colors {
        term_bg: p.eval(&r.terminal_background)?,
        term_fg: p.eval(&r.terminal_foreground)?,
        palette,
        bar_fg: p.eval(&r.bar_foreground)?,
        bar_alt_fg: p.eval(&r.bar_alt_foreground)?,
        bar_accent: p.eval(&r.bar_accent)?,
        gradient: p.gradient.clone(),
        extra,
        rofi: [
            p.eval(&r.rofi_background)?,
            p.eval(&r.rofi_alt_background)?,
            p.eval(&r.rofi_foreground)?,
            p.eval(&r.rofi_selected)?,
            p.eval(&r.rofi_active)?,
            p.eval(&r.rofi_urgent)?,
        ],
        dunst: [
            p.eval(&r.dunst_background)?,
            p.eval(&r.dunst_foreground)?,
            p.eval(&r.dunst_frame)?,
            p.eval(&r.dunst_urgent)?,
        ],
        border: p.eval(&r.screenshot_border)?,
        fastfetch: r.fastfetch.as_deref().map(|e| p.eval(e)).transpose()?,
        polywins_inactive: match &r.polywins_inactive {
            Some(e) => p.eval(e)?,
            None => p.eval(&r.bar_foreground)?,
        },
        bar_bg,
        bar_alt_bg,
    })
}

/// Checks that every color of THEME resolves
pub fn check(theme: &Theme) -> Result<(), String> {
    resolve(theme).map(|_| ())
}

/// "#rrggbb"
fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Icon theme to use: the icon theme of A with the folders of its
/// folder_color. In the session, packarch-folder-color makes it (or gives
/// back the icon theme itself when it is already of that color) and removes
/// the ones it made before; without folder_color, it only removes them. The
/// package keeps the icon theme as it is (no generated theme in /etc/skel).
pub(crate) fn icon_theme_for(a: &Appearance, target: Target) -> String {
    let keep = a.icon_theme.clone();
    if !matches!(target, Target::Session) {
        return keep;
    }
    let args: Vec<&str> = match &a.folder_color {
        Some(c) => vec!["--prepare", &a.icon_theme, c],
        None => vec!["--clean"],
    };
    if dry() {
        println!("[dry] packarch-folder-color {}", args.join(" "));
        return keep;
    }
    match Command::new("packarch-folder-color").args(&args).stderr(Stdio::null()).output() {
        Ok(o) if o.status.success() => {
            let id = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if a.folder_color.is_some() && !id.is_empty() { id } else { keep }
        }
        Ok(_) => {
            eprintln!("packarch-skin : packarch-folder-color {} a échoué", args.join(" "));
            keep
        }
        Err(_) => {
            if a.folder_color.is_some() {
                eprintln!("packarch-skin : commande introuvable : packarch-folder-color");
            }
            keep
        }
    }
}

/// Reads FILE, applies F, writes it back if it changed. A missing file is
/// skipped (the user may have removed it).
pub(crate) fn edit_file(file: &Path, f: impl FnOnce(&mut String)) {
    let Ok(old) = fs::read_to_string(file) else { return };
    let mut text = old.clone();
    f(&mut text);
    if text != old {
        if let Err(e) = fs::write(file, text) {
            eprintln!("{} : {e}", file.display());
        }
    }
}

pub(crate) fn write_file(file: &Path, text: &str) {
    if let Some(d) = file.parent() {
        let _ = fs::create_dir_all(d);
    }
    if let Err(e) = fs::write(file, text) {
        eprintln!("{} : {e}", file.display());
    }
}

pub(crate) fn xfconf(channel: &str, prop: &str, value: &str) {
    run("xfconf-query", &["-c", channel, "-p", prop, "-s", value]);
}

pub(crate) fn xfconf_get(channel: &str, prop: &str) -> Option<String> {
    if dry() {
        return None;
    }
    let out = Command::new("xfconf-query").args(["-c", channel, "-p", prop]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Writes the fonts F (of all the programs, or only of ONLY) into their
/// configuration. The theme directory TDIR holds the polybar and rofi
/// layouts. Nothing is reloaded but the Thunar daemon, which only reads the
/// GTK font at startup.
pub fn write_fonts(dirs: &Dirs, tdir: &Path, f: &Fonts, target: Target, only: Option<Part>) {
    let want = |p: Part| only.is_none_or(|o| o == p);
    let conf = &dirs.config;

    if want(Part::Polybar) {
        edit_file(&tdir.join("polybar/config.ini"), |t| {
            set_line(t, "font-0 = ", &format!("font-0 = \"{}\"", f.polybar));
        });
    }
    if want(Part::Rofi) {
        edit_file(&tdir.join("rofi/shared/fonts.rasi"), |t| {
            set_line(t, "font:", &format!("font: \"{}\";", f.rofi));
        });
    }
    if want(Part::Terminal) {
        edit_file(&conf.join("kitty/fonts.conf"), |t| {
            let n = &f.terminal;
            set_block(
                t,
                "#-fn-start",
                "#-fn-end",
                &format!("font_family {n}\nbold_font {n}\nitalic_font {n}\nbold_italic_font {n}"),
            );
            set_line(t, "font_size ", &format!("font_size {}", f.terminal_size));
        });
    }
    if want(Part::Geany) {
        edit_file(&conf.join("geany/geany.conf"), |t| {
            set_line(t, "editor_font=", &format!("editor_font={}", f.geany));
        });
    }
    if want(Part::Gtk) {
        match target {
            Target::Session => {
                if xfconf_get("xsettings", "/Gtk/FontName").as_deref() != Some(f.gtk.as_str()) {
                    xfconf("xsettings", "/Gtk/FontName", &f.gtk);
                    // Its windows are closed
                    if run("pgrep", &["-x", "thunar"]) {
                        run("thunar", &["-q"]);
                        spawn("thunar", &["--daemon"], &[]);
                    }
                }
            }
            Target::Package(_) => {
                edit_file(&conf.join("xfce4/xfconf/xfce-perchannel-xml/xsettings.xml"), |t| {
                    set_xfconf_file(t, "FontName", &f.gtk);
                });
            }
        }
        // Read by the GTK applications started without xfsettingsd
        edit_file(&conf.join("gtk-3.0/settings.ini"), |t| {
            set_line(t, "gtk-font-name=", &format!("gtk-font-name={}", f.gtk));
        });
        edit_file(&dirs.home.join(".gtkrc-2.0"), |t| {
            set_line(t, "gtk-font-name=", &format!("gtk-font-name=\"{}\"", f.gtk));
        });
    }
    if want(Part::Openbox) {
        edit_file(&conf.join("openbox/rc.xml"), |x| {
            let th = ("theme", None);
            // Menu items: feather first for the glyphs of menu-glyphs.xml (their
            // codepoints are also in other icon fonts, with other widths)
            let menu_font = format!("{},feather", f.openbox);
            for (place, weight) in [("ActiveWindow", "Bold"), ("InactiveWindow", "Normal"), ("MenuHeader", "Bold"), ("MenuItem", "Normal"), ("ActiveOnScreenDisplay", "Bold"), ("InactiveOnScreenDisplay", "Normal")] {
                let font = ("font", Some(("place", place)));
                let name = if place == "MenuItem" { &menu_font } else { &f.openbox };
                set_xml_text(x, &[th, font, ("name", None)], name);
                set_xml_text(x, &[th, font, ("size", None)], &f.openbox_size.to_string());
                set_xml_text(x, &[th, font, ("weight", None)], weight);
                set_xml_text(x, &[th, font, ("slant", None)], "Normal");
            }
        });
    }
    if want(Part::Dunst) {
        edit_file(&conf.join("dunst/dunstrc"), |t| {
            set_line(t, "font = ", &format!("font = {}", f.dunst));
        });
    }
}

/// Sets the value of the xfconf property NAME (string) in the channel file
/// TEXT (xfce-perchannel-xml)
fn set_xfconf_file(text: &mut String, name: &str, value: &str) {
    set_line(
        text,
        &format!("<property name=\"{name}\" "),
        &format!("<property name=\"{name}\" type=\"string\" value=\"{}\"/>", xml_escape(value)),
    );
}

/// GTK, icon and cursor themes and color scheme of A (ICONS: the icon theme
/// to use, with its folder color), for every GTK version
pub(crate) fn write_gtk(dirs: &Dirs, id: &str, a: &Appearance, icons: &str, target: Target) {
    let conf = &dirs.config;
    let scheme = format!("'{}'", a.color_scheme);
    match target {
        // Sway: no xfsettingsd, GTK reads GSettings (sway-theme sets them
        // again at each start of Sway, also for the package)
        Target::Session if dirs.desktop == Desktop::Sway => {
            let gs = |key: &str, value: &str| {
                run("gsettings", &["set", "org.gnome.desktop.interface", key, value]);
            };
            gs("gtk-theme", &a.gtk_theme);
            gs("icon-theme", icons);
            gs("cursor-theme", &a.cursor_theme);
            gs("color-scheme", &scheme);
            run("gsettings", &["set", "org.x.apps.portal", "color-scheme", &scheme]);
        }
        Target::Package(_) if dirs.desktop == Desktop::Sway => {}
        Target::Session => {
            xfconf("xsettings", "/Net/ThemeName", &a.gtk_theme);
            xfconf("xsettings", "/Net/IconThemeName", icons);
            xfconf("xsettings", "/Gtk/CursorThemeName", &a.cursor_theme);
            run("gsettings", &["set", "org.gnome.desktop.interface", "color-scheme", &scheme]);
            run("gsettings", &["set", "org.x.apps.portal", "color-scheme", &scheme]);
        }
        Target::Package(root) => {
            edit_file(&conf.join("xfce4/xfconf/xfce-perchannel-xml/xsettings.xml"), |t| {
                set_xfconf_file(t, "ThemeName", &a.gtk_theme);
                set_xfconf_file(t, "IconThemeName", icons);
                set_xfconf_file(t, "CursorThemeName", &a.cursor_theme);
            });
            // A schema that is not installed (org.x.apps.portal without
            // xapp) is ignored by glib-compile-schemas. XFCE: the file of
            // packarch-xfce-settings, with the themes (used by GTK
            // applications started without xfsettingsd)
            let (name, what, themes) = match dirs.desktop {
                Desktop::Openbox | Desktop::Sway => ("25_packarch-openbox", "Color scheme", String::new()),
                Desktop::Xfce => (
                    "25_packarch-xfce-settings",
                    "Themes and color scheme",
                    format!(
                        "gtk-theme='{}'\nicon-theme='{icons}'\ncursor-theme='{}'\n",
                        a.gtk_theme, a.cursor_theme
                    ),
                ),
            };
            write_file(
                &root.join(format!("usr/share/glib-2.0/schemas/{name}.gschema.override")),
                &format!(
                    "# {what} of the default theme ({id}), written by packarch-skin render\n\
                     [org.gnome.desktop.interface]\n{themes}color-scheme={scheme}\n\n[org.x.apps.portal]\ncolor-scheme={scheme}\n"
                ),
            );
        }
    }
    // Read by the GTK applications started without xfsettingsd
    for v in ["gtk-3.0", "gtk-4.0"] {
        edit_file(&conf.join(v).join("settings.ini"), |t| {
            set_line(t, "gtk-theme-name=", &format!("gtk-theme-name={}", a.gtk_theme));
            set_line(t, "gtk-icon-theme-name=", &format!("gtk-icon-theme-name={icons}"));
            set_line(t, "gtk-cursor-theme-name=", &format!("gtk-cursor-theme-name={}", a.cursor_theme));
        });
    }
    edit_file(&dirs.home.join(".gtkrc-2.0"), |t| {
        set_line(t, "gtk-theme-name=", &format!("gtk-theme-name=\"{}\"", a.gtk_theme));
        set_line(t, "gtk-icon-theme-name=", &format!("gtk-icon-theme-name=\"{icons}\""));
        set_line(t, "gtk-cursor-theme-name=", &format!("gtk-cursor-theme-name=\"{}\"", a.cursor_theme));
    });
    edit_file(&dirs.home.join(".icons/default/index.theme"), |t| {
        set_line(t, "Inherits=", &format!("Inherits={}", a.cursor_theme));
    });
}

pub fn apply(dirs: &Dirs, entry: &Entry, target: Target) -> Result<(), String> {
    let theme = entry.load()?;
    let c = resolve(&theme)?;
    let id = &entry.id;
    let tdir = &entry.dir;
    let conf = &dirs.config;
    let obox = conf.join("openbox");
    let session = matches!(target, Target::Session);

    if session && dirs.desktop == Desktop::Openbox {
        run(
            "dunstify",
            &[
                "-u", "normal", "-h", "string:x-dunst-stack-tag:applytheme",
                "-i", "/usr/share/packarch/icons/dunst/themes.png",
                &format!("Application du style : {}", entry.display_name(Some(&theme))),
            ],
        );
    }
    write_file(&dirs.state, &format!("{id}\n"));
    match dirs.desktop {
        Desktop::Xfce => return apply_xfce(dirs, entry, &theme, &c, target),
        Desktop::Sway => return crate::sway::apply(dirs, entry, &theme, &c, target),
        Desktop::Openbox => {}
    }

    // Fonts: the global ones (fonts.toml) in place of the ones of the theme
    let f = &Global::load(&fonts::file(dirs))?.over(&theme.fonts);
    write_fonts(dirs, tdir, f, target, None);

    // Wallpaper
    let wallpaper = tdir.join(&theme.wallpaper);
    let wallpaper = wallpaper.to_string_lossy();
    if session {
        for head in 0..=10 {
            let h = format!("--head={head}");
            run("nitrogen", &[h.as_str(), "--save", "--set-zoom-fill", &wallpaper]);
        }
    } else {
        // The theme directory of the package is not the one of the user
        if !Path::new(&theme.wallpaper).is_absolute() {
            return Err(format!("{id} : le fond d'écran doit être un chemin absolu : {}", theme.wallpaper));
        }
        let mut cfg = String::new();
        for head in 0..=10 {
            cfg += &format!("[xin_{head}]\nfile={wallpaper}\nmode=5\nbgcolor=#000000\n\n");
        }
        write_file(&conf.join("nitrogen/bg-saved.cfg"), &cfg);
    }

    // Polybar
    let pbar = tdir.join("polybar");
    let mut colors = format!(
        "[color]\n\nBACKGROUND = {}\nFOREGROUND = {}\nALTBACKGROUND = {}\nALTFOREGROUND = {}\nACCENT = {}\n\n",
        c.bar_bg.text, c.bar_fg.text, c.bar_alt_bg.text, c.bar_alt_fg.text, c.bar_accent.text
    );
    let names = ["BLACK", "RED", "GREEN", "YELLOW", "BLUE", "MAGENTA", "CYAN", "WHITE"];
    for (i, col) in c.palette.iter().enumerate() {
        let n = if i < 8 { names[i].to_string() } else { format!("ALT{}", names[i - 8]) };
        colors += &format!("{n} = {}\n", col.text);
    }
    if !c.gradient.is_empty() {
        colors += "\n";
        for (i, g) in c.gradient.iter().enumerate().take(7).skip(1) {
            colors += &format!("BACKGROUND{i} = {}\n", g.text);
        }
    }
    if !c.extra.is_empty() {
        colors += "\n";
        for (k, v) in &c.extra {
            colors += &format!("{k} = {}\n", v.text);
        }
    }
    write_file(&pbar.join("colors.ini"), &colors);
    if theme.polybar.polywins {
        edit_file(&pbar.join("scripts/polywins.sh"), |t| {
            set_line(t, "active_text_color=", &format!("active_text_color='{}'", c.bar_accent.text));
            set_line(t, "inactive_text_color=", &format!("inactive_text_color='{}'", c.polywins_inactive.text));
            set_line(t, "minimized_text_color=", &format!("minimized_text_color='{}'", c.bar_alt_fg.text));
        });
    }

    // Rofi (and the scripts that use the rofi themes of the current theme)
    let border = c.border.rgb.rgb_float();
    for s in ["rofi-screenshot", "ob-screenshot"] {
        edit_file(&obox.join("scripts").join(s), |t| {
            set_line(t, "border=", &format!("border='{border}'"));
        });
    }
    for s in ["rofi-askpass", "rofi-bluetooth", "rofi-launcher", "rofi-powermenu", "rofi-runner", "rofi-screenshot"] {
        edit_file(&obox.join("scripts").join(s), |t| {
            set_line(t, "STYLE=", &format!("STYLE=\"{id}\""));
        });
    }
    // Icon theme (with the folder color of the theme)
    let a = &theme.appearance;
    let icons = icon_theme_for(a, target);
    let rofi_icons = if a.rofi_icons == a.icon_theme { &icons } else { &a.rofi_icons };

    let rofi = tdir.join("rofi");
    let r = &c.rofi;
    write_file(
        &rofi.join("shared/colors.rasi"),
        &format!(
            "* {{\n    background:     {};\n    background-alt: {};\n    foreground:     {};\n    selected:       {};\n    active:         {};\n    urgent:         {};\n}}\n",
            r[0].text, r[1].text, r[2].text, r[3].text, r[4].text, r[5].text
        ),
    );
    edit_file(&conf.join("rofi/config.rasi"), |t| {
        set_line(t, "icon-theme:", &format!("icon-theme: \"{rofi_icons}\";"));
    });
    // Package: ~/... (the home of the package is not the one of the user;
    // rofi expands ~ in -theme)
    let nm_theme = rofi.join("networkmenu.rasi");
    let nm_theme = match (target, nm_theme.strip_prefix(&dirs.home)) {
        (Target::Package(_), Ok(rel)) => format!("~/{}", rel.display()),
        _ => nm_theme.display().to_string(),
    };
    edit_file(&conf.join("networkmanager-dmenu/config.ini"), |t| {
        set_line(t, "dmenu_command = ", &format!("dmenu_command = rofi -dmenu -theme {nm_theme}"));
    });

    // Terminal (kitty)
    let kitty = conf.join("kitty");
    if kitty.join("fonts.conf").is_file() && kitty.join("colors.conf").is_file() {
        let mut t = format!(
            "## Colors configuration\nbackground {bg}\nforeground {fg}\nselection_background {fg}\nselection_foreground {bg}\ncursor {fg}\n\n",
            bg = c.term_bg.text,
            fg = c.term_fg.text
        );
        for i in 0..8 {
            t += &format!("color{i} {}\ncolor{} {}\n", c.palette[i].text, i + 8, c.palette[i + 8].text);
        }
        write_file(&kitty.join("colors.conf"), &t);
        if session {
            run("pkill", &["-USR1", "-x", "kitty"]);
        }
    }

    // Geany
    edit_file(&conf.join("geany/geany.conf"), |t| {
        set_line(t, "color_scheme=", &format!("color_scheme={}", a.geany_colors));
    });

    write_gtk(dirs, id, a, &icons, target);

    // Openbox
    let o = &theme.openbox;
    edit_file(&obox.join("rc.xml"), |x| {
        let th = ("theme", None);
        set_xml_text(x, &[th, ("name", None)], &o.theme);
        set_xml_text(x, &[th, ("titleLayout", None)], &o.layout);
        set_xml_text(x, &[("menu", None), ("file", None)], &o.menu);
        for (side, v) in ["top", "bottom", "left", "right"].iter().zip(o.margins) {
            set_xml_text(x, &[("margins", None), (side, None)], &v.to_string());
        }
    });
    if session {
        run("openbox", &["--reconfigure"]);
    }

    // Dunst
    let d = &theme.dunst;
    edit_file(&conf.join("dunst/dunstrc"), |t| {
        for (k, v) in [
            ("width", d.width.to_string()),
            ("height", d.height.to_string()),
            ("offset", d.offset.clone()),
            ("origin", d.origin.clone()),
            ("frame_width", d.border.to_string()),
            ("separator_height", d.separator.to_string()),
            ("line_height", d.separator.to_string()),
        ] {
            set_line(t, &format!("{k} = "), &format!("{k} = {v}"));
        }
        truncate_at(t, "urgency_low");
        let [bg, fg, frame, urgent] = &c.dunst;
        t.push_str(&format!(
            "[urgency_low]\ntimeout = 2\nbackground = \"{bg}\"\nforeground = \"{fg}\"\nframe_color = \"{frame}\"\n\n\
             [urgency_normal]\ntimeout = 5\nbackground = \"{bg}\"\nforeground = \"{fg}\"\nframe_color = \"{frame}\"\n\n\
             [urgency_critical]\ntimeout = 0\nbackground = \"{bg}\"\nforeground = \"{urgent}\"\nframe_color = \"{urgent}\"\n",
            bg = bg.text, fg = fg.text, frame = frame.text, urgent = urgent.text
        ));
    });
    if session && run("pkill", &["-x", "dunst"]) {
        spawn("dunst", &[], &[]);
    }

    // Fastfetch: only its own file (the one of the user is kept)
    let ff = conf.join("fastfetch/config.jsonc");
    let ours = fs::read_to_string(&ff).map_or(true, |t| t.starts_with(FASTFETCH_MARK));
    if ours {
        match &c.fastfetch {
            Some(col) => write_file(
                &ff,
                &format!(
                    "{FASTFETCH_MARK}\n{{\n  \"logo\": {{ {FASTFETCH_LOGO}, \"color\": {{ \"1\": \"{t}\", \"2\": \"{t}\" }} }},\n  \"display\": {{ \"color\": {{ \"keys\": \"{t}\", \"title\": \"{t}\" }} }},\n  \"modules\": [{}]\n}}\n",
                    FASTFETCH_MODULES.iter().map(|m| format!("\"{m}\"")).collect::<Vec<_>>().join(", "),
                    t = col.text
                ),
            ),
            None => {
                let _ = fs::remove_file(&ff);
            }
        }
    }

    // Compositor
    let pc = &theme.picom;
    edit_file(&conf.join("picom.conf"), |t| {
        set_line(t, "backend = ", &format!("backend = \"{}\";", pc.backend));
        set_block(t, "#-cr-start", "#-cr-end", &format!("corner-radius = {};", pc.corner_radius));
        set_line(t, "shadow-radius = ", &format!("shadow-radius = {};", pc.shadow_radius));
        set_line(t, "shadow-opacity = ", &format!("shadow-opacity = {};", pc.shadow_opacity));
        set_line(t, "shadow-offset-x = ", &format!("shadow-offset-x = {};", pc.shadow_offset_x));
        set_line(t, "shadow-offset-y = ", &format!("shadow-offset-y = {};", pc.shadow_offset_y));
        set_line(t, "method = ", &format!("method = \"{}\";", pc.blur_method));
        set_line(t, "strength = ", &format!("strength = {};", pc.blur_strength));
    });

    if session {
        bar::launch(dirs)?;
        run("xsetroot", &["-cursor_name", "left_ptr"]);
    }
    Ok(())
}

/// Window border theme of xfwm4
fn xfwm4_theme(theme: &Theme) -> &str {
    theme.xfce.xfwm4_theme.as_deref().unwrap_or(&theme.appearance.gtk_theme)
}

/// Application of a theme to the XFCE desktop (the fonts, chosen in its
/// settings, are kept)
fn apply_xfce(dirs: &Dirs, entry: &Entry, theme: &Theme, c: &Colors, target: Target) -> Result<(), String> {
    let id = &entry.id;
    let conf = &dirs.config;
    let a = &theme.appearance;
    let session = matches!(target, Target::Session);
    let wallpaper = entry.dir.join(&theme.wallpaper);
    let wallpaper = wallpaper.to_string_lossy();
    if !session && !Path::new(&theme.wallpaper).is_absolute() {
        return Err(format!("{id} : le fond d'écran doit être un chemin absolu : {}", theme.wallpaper));
    }
    let xfwm4 = xfwm4_theme(theme);
    let icons = icon_theme_for(a, target);
    let palette = c.palette.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join(";");
    let (bg, fg, accent) = (&c.term_bg.text, &c.term_fg.text, &c.bar_accent.text);

    write_gtk(dirs, id, a, &icons, target);
    edit_file(&conf.join("geany/geany.conf"), |t| {
        set_line(t, "color_scheme=", &format!("color_scheme={}", a.geany_colors));
    });

    match target {
        Target::Session => {
            xfce::wallpaper(&wallpaper);
            xfconf("xfwm4", "/general/theme", xfwm4);
            // xfce4-terminal 1.1+: settings in xfconf (terminalrc is only
            // imported at its first start)
            let t = "xfce4-terminal";
            xfconf_new(t, "/color-use-theme", "bool", "false");
            xfconf_new(t, "/color-foreground", "string", fg);
            xfconf_new(t, "/color-background", "string", bg);
            xfconf_new(t, "/color-cursor-use-default", "bool", "false");
            xfconf_new(t, "/color-cursor", "string", accent);
            xfconf_new(t, "/color-bold-use-default", "bool", "true");
            xfconf_new(t, "/color-palette", "string", &palette);
            xfconf_new(t, "/tab-activity-color", "string", accent);
            xfce::update_button(dirs, id);
            run(
                "notify-send",
                &[
                    "-a", "Thèmes", "-i", "preferences-desktop-theme",
                    &format!("Thème appliqué : {}", entry.display_name(Some(theme))),
                ],
            );
        }
        Target::Package(_) => {
            let xml = conf.join("xfce4/xfconf/xfce-perchannel-xml");
            edit_file(&xml.join("xfce4-desktop.xml"), |t| {
                for key in ["image-path", "last-image", "last-single-image"] {
                    set_line(
                        t,
                        &format!("<property name=\"{key}\" "),
                        &format!("<property name=\"{key}\" type=\"string\" value=\"{}\"/>", xml_escape(&wallpaper)),
                    );
                }
            });
            edit_file(&xml.join("xfwm4.xml"), |t| set_xfconf_file(t, "theme", xfwm4));
            xfce::render_button(dirs, id)?;
            edit_file(&conf.join("xfce4/terminal/terminalrc"), |t| {
                set_line(t, "ColorUseTheme=", "ColorUseTheme=FALSE");
                set_line(t, "ColorForeground=", &format!("ColorForeground={fg}"));
                set_line(t, "ColorBackground=", &format!("ColorBackground={bg}"));
                set_line(t, "ColorCursorUseDefault=", "ColorCursorUseDefault=FALSE");
                set_line(t, "ColorCursor=", &format!("ColorCursor={accent}"));
                set_line(t, "ColorBoldUseDefault=", "ColorBoldUseDefault=TRUE");
                set_line(t, "ColorPalette=", &format!("ColorPalette={palette}"));
                set_line(t, "TabActivityColor=", &format!("TabActivityColor={accent}"));
            });
        }
    }
    Ok(())
}

