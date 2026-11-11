//! Sway desktop (packarch-sway-settings): the same themes, copies of the
//! theme.toml files in /usr/share/packarch/sway/themes. "apply" writes the
//! wallpaper and the colors of the windows (sway-output, sway-theme), waybar,
//! rofi, mako, kitty and hyprlock, the GTK, icon and cursor themes and the
//! Geany colors, then reloads Sway (which relaunches waybar and mako). The
//! fonts of the configuration are kept. With SwayFX (packarch-sway-settings
//! 1.0-11), the rounded corners and shadows of the windows are the picom
//! settings of the theme (corner_radius, shadow_radius, shadow_opacity), as on
//! Openbox.

use crate::apply::{edit_file, icon_theme_for, kitty_colors, write_file, write_gtk, Colors, Target};
use crate::color::Color;
use crate::edit::{set_line, truncate_at};
use crate::theme::{Dunst, Entry, Theme};
use crate::{run, spawn, Dirs};
use std::path::Path;

/// Line of mako/config after which the colors are written
const MAKO_MARK: &str = "# Mako_Colors";

/// "rgba(R, G, B, ALPHA)" (hyprlock)
fn rgba(c: &Color, alpha: f64) -> String {
    let v = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("rgba({}, {}, {}, {alpha:.2})", v(c.rgb.r), v(c.rgb.g), v(c.rgb.b))
}

/// Colors of the windows: "set $sway_cl_col_<name> <color>" lines of sway-theme
/// (window_* roles of the theme)
fn window_colors(c: &Colors) -> [(&'static str, &Color); 8] {
    let [bg, fg, focused, focused_inactive, unfocused, urgent, indicator] = &c.window;
    [
        ("bg", bg),
        ("fg", fg),
        ("in", indicator),
        ("afoc", focused),
        ("ifoc", focused_inactive),
        ("ufoc", unfocused),
        ("urgt", urgent),
        ("phol", bg),
    ]
}

/// SwayFX lines of sway-theme from the picom settings of the theme: (key,
/// line). Lines missing from the file (plain Sway) are not added.
fn fx_lines(theme: &Theme, c: &Colors) -> [(&'static str, String); 3] {
    let pc = &theme.picom;
    let opacity = pc.shadow_opacity.trim().parse::<f64>().unwrap_or(0.5).clamp(0.0, 1.0);
    [
        ("corner_radius ", format!("corner_radius {}", pc.corner_radius)),
        ("shadow_blur_radius ", format!("shadow_blur_radius {}", pc.shadow_radius)),
        ("shadow_color ", format!("shadow_color {}{:02X}", c.shadow.rgb.hex(), (opacity * 255.0).round() as u8)),
    ]
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

/// Application of a theme to the Sway desktop
pub fn apply(dirs: &Dirs, entry: &Entry, theme: &Theme, c: &Colors, target: Target) -> Result<(), String> {
    let id = &entry.id;
    let session = matches!(target, Target::Session);
    if !session && !Path::new(&theme.wallpaper).is_absolute() {
        return Err(format!("{id} : le fond d'écran doit être un chemin absolu : {}", theme.wallpaper));
    }
    let wallpaper = entry.dir.join(&theme.wallpaper);
    let wallpaper = wallpaper.to_string_lossy();
    let sway = dirs.config.join("sway");
    let a = &theme.appearance;
    let icons = icon_theme_for(a, target);

    // Wallpaper of every output
    edit_file(&sway.join("sway-output"), |t| {
        set_line(t, "output * bg ", &format!("output * bg \"{wallpaper}\" fill"));
    });

    // Colors of the windows, GTK, icon and cursor themes (set again by
    // gsettings at each start of Sway)
    edit_file(&sway.join("sway-theme"), |t| {
        for (name, col) in window_colors(c) {
            let key = format!("set $sway_cl_col_{name} ");
            set_line(t, &key, &format!("{key}{}", col.text));
        }
        for (name, value) in [
            ("gtk_theme", a.gtk_theme.as_str()),
            ("icon_theme", icons.as_str()),
            ("cursor_theme", a.cursor_theme.as_str()),
            ("color_scheme", a.color_scheme.as_str()),
        ] {
            let key = format!("set $sway_{name} ");
            set_line(t, &key, &format!("{key}{value}"));
        }
        for (key, line) in fx_lines(theme, c) {
            set_line(t, key, &line);
        }
    });

    write_file(&sway.join("waybar/colors.css"), &waybar_colors(c));

    let r = &c.rofi;
    write_file(
        &sway.join("rofi/shared/colors.rasi"),
        &format!(
            "/* Couleurs du thème, écrites par packarch-skin */\n\n* {{\n    background:     {};\n    background-alt: {};\n    foreground:     {};\n    selected:       {};\n    active:         {};\n    urgent:         {};\n}}\n",
            r[0].text, r[1].text, r[2].text, r[3].text, r[4].text, r[5].text
        ),
    );
    let rofi_icons = if a.rofi_icons == a.icon_theme { &icons } else { &a.rofi_icons };
    for f in ["launcher.rasi", "runner.rasi"] {
        edit_file(&sway.join("rofi").join(f), |t| {
            set_line(t, "icon-theme:", &format!("icon-theme:                 \"{rofi_icons}\";"));
        });
    }

    edit_file(&sway.join("mako/config"), |t| {
        truncate_at(t, MAKO_MARK);
        t.push_str(&mako_colors(c, &theme.dunst));
    });

    let kitty = sway.join("kitty");
    if kitty.join("kitty.conf").is_file() {
        write_file(&kitty.join("colors.conf"), &kitty_colors(c));
    }

    // Lock screen
    let [accent, text, field, check, fail] = &c.lock;
    let field_alpha = theme.sway.lock_ring_opacity.clamp(0.0, 1.0);
    let fg_hex = text.text.trim_start_matches('#');
    edit_file(&sway.join("hyprlock.conf"), |t| {
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
        // Sway reads sway-output and sway-theme again; its startup script
        // relaunches waybar and mako with their new colors
        run("swaymsg", &["reload"]);
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
    use super::rgba;
    use crate::color::{Color, Rgb};

    #[test]
    fn rgba_hyprlock() {
        let c = Color { text: "#E06B74".into(), rgb: Rgb::parse("#E06B74").unwrap() };
        assert_eq!(rgba(&c, 1.0), "rgba(224, 107, 116, 1.00)");
        assert_eq!(rgba(&c, 0.5), "rgba(224, 107, 116, 0.50)");
    }
}
