//! labwc desktop (packarch-labwc-settings): the same themes, copies of the
//! theme.toml files in /etc/skel/.config/labwc/themes. labwc reads the
//! Openbox window themes (OB-*, openbox-3 directory), so "apply" writes, as
//! on Openbox, the window theme, title bar buttons and fonts of the theme into
//! rc.xml, with the rounded corners and shadows of its picom settings
//! (cornerRadius, dropShadows, shadows of themerc-override). Also: the
//! wallpaper (swaybg), the colors of waybar, rofi, mako, kitty and hyprlock,
//! the GTK, icon and cursor themes (GSettings and packarch-theme, read again
//! at each start of labwc) and the Geany colors, then reloads labwc and
//! relaunches swaybg, waybar and mako (scripts/startup).

use crate::apply::{edit_file, icon_theme_for, kitty_colors, write_file, write_gtk, Colors, Target};
use crate::color::Color;
use crate::edit::{set_block, set_line, set_xml_text, truncate_at};
use crate::theme::{Dunst, Entry, Theme};
use crate::{run, spawn, Dirs};
use std::path::Path;

/// Line of mako/config after which the colors are written
const MAKO_MARK: &str = "# Mako_Colors";

/// Lines of themerc-override between which the shadows are written
const OVERRIDE_START: &str = "# -- packarch-skin : début --";
const OVERRIDE_END: &str = "# -- packarch-skin : fin --";

/// "rgba(R, G, B, ALPHA)" (hyprlock)
fn rgba(c: &Color, alpha: f64) -> String {
    let v = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("rgba({}, {}, {}, {alpha:.2})", v(c.rgb.r), v(c.rgb.g), v(c.rgb.b))
}

/// Title bar layout of labwc ("desk:iconify,max,close") from the Openbox
/// titleLayout of the theme ("DLIMC"): the buttons before the label (L) on
/// the left, the others on the right; N: window icon, D: all desktops, S:
/// shade, I: iconify, M: maximize, C: close. Without a label, every button
/// is on the right.
pub fn titlebar_layout(openbox: &str) -> String {
    let button = |c: char| match c.to_ascii_uppercase() {
        'N' => Some("icon"),
        'D' => Some("desk"),
        'S' => Some("shade"),
        'I' => Some("iconify"),
        'M' => Some("max"),
        'C' => Some("close"),
        _ => None,
    };
    let (left, right) = match openbox.to_ascii_uppercase().find('L') {
        Some(i) => (&openbox[..i], &openbox[i + 1..]),
        None => ("", openbox),
    };
    let list = |s: &str| s.chars().filter_map(button).collect::<Vec<_>>().join(",");
    format!("{}:{}", list(left), list(right))
}

/// Shadows of themerc-override: the picom settings of the theme (size:
/// shadow_radius, color: shadow_color with shadow_opacity)
fn shadow_lines(theme: &Theme, c: &Colors) -> String {
    let pc = &theme.picom;
    let opacity = shadow_opacity(theme);
    let color = format!("{}{:02x}", c.shadow.rgb.hex(), (opacity * 255.0).round() as u8);
    let mut t = String::new();
    for state in ["active", "inactive"] {
        t += &format!("window.{state}.shadow.size: {}\nwindow.{state}.shadow.color: {color}\n", pc.shadow_radius);
    }
    t.trim_end().to_string()
}

fn shadow_opacity(theme: &Theme) -> f64 {
    theme.picom.shadow_opacity.trim().parse::<f64>().unwrap_or(0.5).clamp(0.0, 1.0)
}

/// waybar/colors.css
fn waybar_colors(c: &Colors) -> String {
    let mut t = String::from("/* Couleurs du thème, écrites par packarch-skin */\n\n");
    for (name, col) in [
        ("background", &c.bar_bg),
        ("background-alt", &c.bar_alt_bg),
        ("foreground", &c.bar_fg),
        ("foreground-alt", &c.bar_alt_fg),
        ("selected", &c.bar_accent),
    ] {
        t += &format!("@define-color {name:<14} {};\n", col.text);
    }
    let names = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
    for (name, col) in names.iter().zip(&c.palette) {
        t += &format!("@define-color {name:<14} {};\n", col.text);
    }
    t
}

/// Colors and timeouts of mako/config (after MAKO_MARK): the dunst roles and
/// timeouts of the theme
fn mako_colors(c: &Colors, d: &Dunst) -> String {
    let [bg, fg, frame, urgent] = &c.dunst;
    format!(
        "{MAKO_MARK}\nbackground-color={bg}\ntext-color={fg}\nborder-color={frame}\nprogress-color=over {accent}\n\n\
         [urgency=low]\nborder-color={frame}\ndefault-timeout={low}\n\n\
         [urgency=normal]\nborder-color={frame}\ndefault-timeout={normal}\n\n\
         [urgency=high]\nborder-color={urgent}\ntext-color={urgent}\ndefault-timeout={critical}\n",
        low = d.timeout_low * 1000,
        normal = d.timeout_normal * 1000,
        critical = d.timeout_critical * 1000,
        bg = bg.text,
        fg = fg.text,
        frame = frame.text,
        urgent = urgent.text,
        accent = c.bar_accent.text
    )
}

/// VALUE between single quotes for the shell (packarch-theme is sourced)
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Application of a theme to the labwc desktop
pub fn apply(dirs: &Dirs, entry: &Entry, theme: &Theme, c: &Colors, target: Target) -> Result<(), String> {
    let id = &entry.id;
    let session = matches!(target, Target::Session);
    if !session && !Path::new(&theme.wallpaper).is_absolute() {
        return Err(format!("{id} : le fond d'écran doit être un chemin absolu : {}", theme.wallpaper));
    }
    let wallpaper = entry.dir.join(&theme.wallpaper);
    let wallpaper = wallpaper.to_string_lossy();
    let labwc = dirs.config.join("labwc");
    let a = &theme.appearance;
    let f = &theme.fonts;
    let icons = icon_theme_for(a, target);

    // Wallpaper of every output (swaybg, scripts/wallpaper)
    write_file(&labwc.join("wallpaper"), &format!("{wallpaper}\n"));

    // Window theme, buttons, fonts, rounded corners and shadows
    let o = &theme.openbox;
    let shadows = theme.picom.shadow_radius > 0 && shadow_opacity(theme) > 0.0;
    edit_file(&labwc.join("rc.xml"), |x| {
        let th = ("theme", None);
        set_xml_text(x, &[th, ("name", None)], &o.theme);
        set_xml_text(x, &[th, ("icon", None)], &icons);
        set_xml_text(x, &[th, ("titlebar", None), ("layout", None)], &titlebar_layout(&o.layout));
        set_xml_text(x, &[th, ("cornerRadius", None)], &theme.picom.corner_radius.to_string());
        set_xml_text(x, &[th, ("dropShadows", None)], if shadows { "yes" } else { "no" });
        for (place, weight) in [
            ("ActiveWindow", "bold"),
            ("InactiveWindow", "normal"),
            ("MenuHeader", "bold"),
            ("MenuItem", "normal"),
            ("OnScreenDisplay", "bold"),
        ] {
            let font = ("font", Some(("place", place)));
            set_xml_text(x, &[th, font, ("name", None)], &f.openbox);
            set_xml_text(x, &[th, font, ("size", None)], &f.openbox_size.to_string());
            set_xml_text(x, &[th, font, ("weight", None)], weight);
            set_xml_text(x, &[th, font, ("slant", None)], "normal");
        }
        for (side, v) in ["top", "bottom", "left", "right"].iter().zip(o.margins) {
            set_xml_text(x, &[("margin", None), (side, None)], &v.to_string());
        }
    });
    edit_file(&labwc.join("themerc-override"), |t| {
        set_block(t, OVERRIDE_START, OVERRIDE_END, &shadow_lines(theme, c));
    });

    // Cursor of labwc (read again by labwc --reconfigure)
    edit_file(&labwc.join("environment"), |t| {
        set_line(t, "XCURSOR_THEME=", &format!("XCURSOR_THEME={}", a.cursor_theme));
    });

    // GTK, icon and cursor themes, read by the autostart of labwc
    // (gsettings) and by scripts/asroot (root applications)
    write_file(
        &labwc.join("packarch-theme"),
        &format!(
            "# Thème courant ({id}), écrit par packarch-skin : relu par autostart\n\
             # (gsettings) et scripts/asroot\n\
             gtk_theme={}\nicon_theme={}\ncursor_theme={}\ncolor_scheme={}\nfont={}\n",
            shell_quote(&a.gtk_theme),
            shell_quote(&icons),
            shell_quote(&a.cursor_theme),
            shell_quote(&a.color_scheme),
            shell_quote(&f.gtk)
        ),
    );

    write_file(&labwc.join("waybar/colors.css"), &waybar_colors(c));

    let r = &c.rofi;
    write_file(
        &labwc.join("rofi/shared/colors.rasi"),
        &format!(
            "/* Couleurs du thème, écrites par packarch-skin */\n\n* {{\n    background:     {};\n    background-alt: {};\n    foreground:     {};\n    selected:       {};\n    active:         {};\n    urgent:         {};\n}}\n",
            r[0].text, r[1].text, r[2].text, r[3].text, r[4].text, r[5].text
        ),
    );
    let rofi_icons = if a.rofi_icons == a.icon_theme { &icons } else { &a.rofi_icons };
    for file in ["launcher.rasi", "runner.rasi"] {
        edit_file(&labwc.join("rofi").join(file), |t| {
            set_line(t, "icon-theme:", &format!("icon-theme:                 \"{rofi_icons}\";"));
        });
    }

    edit_file(&labwc.join("mako/config"), |t| {
        truncate_at(t, MAKO_MARK);
        t.push_str(&mako_colors(c, &theme.dunst));
    });

    let kitty = labwc.join("kitty");
    if kitty.join("kitty.conf").is_file() {
        write_file(&kitty.join("colors.conf"), &kitty_colors(c));
    }

    // Lock screen
    let [accent, text, field, check, fail] = &c.lock;
    let field_alpha = theme.labwc.lock_ring_opacity.clamp(0.0, 1.0);
    let fg_hex = text.text.trim_start_matches('#');
    edit_file(&labwc.join("hyprlock.conf"), |t| {
        for (key, value) in [
            ("$wallpaper =", wallpaper.to_string()),
            ("$accent_color =", rgba(accent, 1.0)),
            ("$text_color =", rgba(text, 1.0)),
            // "##": a "#" for hyprlang
            ("$text_color_hex =", format!("##{fg_hex}FF")),
            ("$outer_color =", rgba(field, field_alpha)),
            ("$inner_color =", rgba(field, field_alpha)),
            ("$check_color =", rgba(check, 1.0)),
            ("$fail_color =", rgba(fail, 1.0)),
        ] {
            set_line(t, key, &format!("{key} {value}"));
        }
    });

    edit_file(&dirs.config.join("geany/geany.conf"), |t| {
        set_line(t, "color_scheme=", &format!("color_scheme={}", a.geany_colors));
    });
    write_gtk(dirs, id, a, &icons, target);

    if session {
        // labwc reads rc.xml, themerc-override and environment again
        // (LABWC_PID: inherited from the session)
        if !run("labwc", &["--reconfigure"]) {
            run("pkill", &["-HUP", "-x", "labwc"]);
        }
        run("gsettings", &["set", "org.gnome.desktop.interface", "font-name", &f.gtk]);
        // swaybg, waybar and mako relaunched with the new wallpaper and colors
        let startup = labwc.join("scripts/startup");
        spawn(&startup.to_string_lossy(), &[], &[]);
        run("pkill", &["-USR1", "-x", "kitty"]);
        // Once mako is back
        let msg = format!("Thème appliqué : {}", entry.display_name(Some(theme)));
        spawn(
            "sh",
            &["-c", "sleep 1.5; notify-send -a Thèmes -i preferences-desktop-theme \"$1\"", "sh", &msg],
            &[],
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{rgba, shell_quote, titlebar_layout};
    use crate::color::{Color, Rgb};

    #[test]
    fn rgba_hyprlock() {
        let c = Color { text: "#E06B74".into(), rgb: Rgb::parse("#E06B74").unwrap() };
        assert_eq!(rgba(&c, 1.0), "rgba(224, 107, 116, 1.00)");
        assert_eq!(rgba(&c, 0.5), "rgba(224, 107, 116, 0.50)");
    }

    #[test]
    fn layouts() {
        assert_eq!(titlebar_layout("DLIMC"), "desk:iconify,max,close");
        assert_eq!(titlebar_layout("NLIMC"), "icon:iconify,max,close");
        assert_eq!(titlebar_layout("LIMC"), ":iconify,max,close");
        assert_eq!(titlebar_layout("IMC"), ":iconify,max,close");
        assert_eq!(titlebar_layout("CLSD"), "close:shade,desk");
    }

    #[test]
    fn quotes() {
        assert_eq!(shell_quote("Noto Sans 10"), "'Noto Sans 10'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }
}
