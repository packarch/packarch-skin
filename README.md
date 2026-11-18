<div align="center">
  <h1>Packarch Skin</h1>
  <p><strong>Theme chooser of the Packarch Openbox, XFCE and Packarch (packarch compositor) desktops, written in Rust</strong></p>
</div>

Packarch Skin applies the themes of [Packarch](https://github.com/packarch/Packarch).
Each theme is described by a `theme.toml` file (colors, fonts, GTK, xfwm4, icon and cursor
themes, folder color, wallpaper, Openbox, dunst and picom settings).

On every desktop, the themes applied are the ones of the user, in
`~/.config/<desktop>/themes/<theme>/`: `openbox`, `xfce4` or `packarch`. The themes of the
system are copied there: all of them the first time packarch-skin runs, then only the ones
the user does not have (a theme added by an update of the package, or removed by the user).
A theme already there is never replaced, so the changes of the user are kept, and it does not
get the changes of the package either. Themes of the system: the same directories in
`/etc/skel` (new accounts get them from there), from `packarch-openbox-settings` (next to
the polybar and rofi layouts of each theme), `packarch-xfce-settings` and
`packarch-desktop-settings` (copies of the same `theme.toml` files).

The packages are built from the
[myrepo-pkgs](https://github.com/packarch/myrepo-pkgs) repository (`apps/packarch-skin`).

## Usage

```
packarch-skin list                    available themes (identifier and name)
packarch-skin current                 current theme
packarch-skin apply [THEME]           applies THEME (default: the current theme)
packarch-skin render ROOT [THEME]     writes THEME (default: "default") into the files of
                                      ROOT/etc/skel, without running anything (package build)
packarch-skin bar                     relaunches the bar of the current theme (Openbox)
packarch-skin font [TARGET]           font of each program, or of TARGET (Openbox)
packarch-skin font TARGET FAMILY STYLE SIZE
                                      sets the font of TARGET for every theme (Openbox)
packarch-skin wallpaper [THEME]       path of the wallpaper of THEME
packarch-skin edit [THEME]            opens the theme.toml of THEME (default: the current theme)
                                      in the editor
packarch-skin check [THEME...]        checks the theme.toml files
packarch-skin openbox-menu            Openbox pipemenu of the themes (Openbox, Packarch)
packarch-skin chooser                 theme chooser window (yad), with "Couleur des dossiers"
packarch-skin panel                   puts the theme button back on the XFCE panel, or updates it
```

The messages are in French.

### Changing a theme

Everything `apply` writes comes from the `theme.toml` of the theme: change
`~/.config/<desktop>/themes/<theme>/theme.toml` (`edit` opens it with `$VISUAL`, else
Geany), then run `packarch-skin apply`, without rebuilding packarch-skin. A new directory
there adds a theme (`thumbnail.png` next to its `theme.toml`: thumbnail of the chooser).
Removing the directory of a theme gives the theme of the system back at the next command.
The themes of `~/.config/packarch-skin/themes` (packarch-skin 1.4.0) are moved into the
directory of the desktop.

Optional keys (default: the behavior of the themes without them):

```toml
[roles]                                  # color expressions, like the other roles
terminal_cursor = "accent"               # kitty: terminal_foreground; xfce4-terminal: bar_accent
terminal_selection_background = "..."    # kitty: terminal_foreground
terminal_selection_foreground = "..."    # kitty: terminal_background
lock_accent = "..."                      # Packarch lock screen (hyprlock): bar_accent
lock_text = "..."                        # bar_foreground
lock_field = "..."                       # bar_alt_background
lock_check = "..."                       # color2
lock_fail = "..."                        # color1

[dunst]                                  # seconds, 0: never (dunst and mako)
timeout_low = 2
timeout_normal = 5
timeout_critical = 0

[picom]
shadow_color = "#000000"                 # picom and packarch, with shadow_opacity

[labwc]                                  # Packarch desktop (name kept for the existing themes)
lock_ring_opacity = 0.5                  # password field of the lock screen
```

### Openbox

`apply` writes the colors, fonts and settings of the theme into polybar, rofi, kitty, Geany,
GTK (xfconf and the GTK 2/3 files), Openbox (`rc.xml`), dunst, picom and fastfetch, sets the
wallpaper with nitrogen and reloads Openbox, dunst, kitty and the bar. The fonts chosen with
`font` (`~/.config/openbox/themes/fonts.toml`) replace those of every theme.

### XFCE

On XFCE (`XDG_CURRENT_DESKTOP`, or no Openbox themes), `apply` sets the wallpaper (every
setting of xfdesktop, and those per monitor and workspace of xfdesktop 4.20), the GTK, xfwm4,
icon and cursor themes, the color scheme, the colors of xfce4-terminal and Geany, and puts the
theme first in the theme launcher of the panel. Fonts, bar and menus are left to XFCE (`bar`,
`font` and `openbox-menu` are refused). The themes are in `~/.config/xfce4/themes`, the
current theme in `~/.config/xfcethemes/.current_theme`.

The theme button of the panel is a launcher plugin with one item per theme
(`theme-<id>.desktop`, `packarch-skin apply <id>`, icon
`/usr/share/xfcethemes/icons/<id>.png`) and `folder-color.desktop`
(packarch-folder-color) last, at the top of its menu when the panel is at the
bottom of the screen. `apply` puts the applied theme first (the button shows
it); `render` writes the items and their files into `/etc/skel`; `panel`
creates the button again (first panel, before the window list) if it was
removed, or updates it. `chooser` shows the themes in a window, without the
panel.

Optional section of `theme.toml` for XFCE:

```toml
[xfce]
xfwm4_theme = "Default"   # default: the GTK theme, which must have an xfwm4 directory
```

### Packarch

On Packarch (`XDG_CURRENT_DESKTOP`, or the `~/.config/packarch/packarch-theme` of
`packarch-desktop-settings` without Openbox themes), `apply` writes, as on Openbox,
the window theme (the same `OB-*` themes: packarch reads their `openbox-3` directory),
the title bar buttons (`layout` of `[openbox]`, converted: `DLIMC` gives
`desk:iconify,max,close`) and the fonts (`openbox`, `openbox_size`) into
`~/.config/packarch/rc.xml`, with the rounded corners (`cornerRadius`: `corner_radius` of
`[picom]`) and shadows (`dropShadows`, and their size and color in `themerc-override`:
`shadow_radius`, `shadow_color`, `shadow_opacity`), and the menus in `themerc-override`
(rounded and shadowed menus): `menu.corner.radius`
(`corner_radius`) and `menu.shadow.*` (`shadow_radius` + 4, darker than the windows),
so that they stand out from the windows (colors of the window theme). Also: the wallpaper
(`~/.config/packarch/wallpaper`, shown by swaybg), the cursor (`XCURSOR_THEME` of
`environment`), the GTK, icon and cursor themes (gsettings, and `packarch-theme`, read
again at each start of packarch and by the root applications), the bar of the theme (as its
polybar on Openbox: `themes/<id>/waybar/config` and `style.css`, copied to
`~/.config/packarch/waybar` with the `@color@` of the config replaced by the colors of the
theme, and the scripts of `~/.config/labwc/` of older bars by those of `~/.config/packarch/`; colors, gradient and polybar font of the theme in `waybar/colors.css`; a theme of
`~/.config/packarch/themes` without bar takes the one of the system theme), the colors of rofi,
mako, kitty and hyprlock (all in `~/.config/packarch`) and of Geany. Then it runs
`packarch --reconfigure` and `~/.config/packarch/scripts/startup`, which relaunches swaybg,
waybar and mako. `bar`, `font` and `panel` are refused; `openbox-menu` gives the themes
menu of the packarch menu. The themes are in `~/.config/packarch/themes`, the current theme in
`~/.config/packarch/.current_theme`. `chooser` (Super+Shift+t, theme button of waybar) shows
the themes with the thumbnails of `/usr/share/packarch/desktop/icons/<id>.png`.

On every desktop, `chooser` opens a single window: while one is open (lock
`$XDG_RUNTIME_DIR/packarch-skin-chooser.instance`), starting it again does
nothing.

### Folder colors

The `folder_color` of a theme (`#rrggbb`) is applied by
[packarch-folder-color](https://github.com/packarch/packarch-folder-color), which makes a copy
of the icon theme with folders of that color.

## Building

```sh
cargo build --release
cargo test --release
```

`PACKARCH_SKIN_DRY=1` prints the external commands instead of running them;
`PACKARCH_SKIN_XFCE_THEMES` and `PACKARCH_SKIN_PACKARCH_THEMES` set another directory of XFCE or
Packarch themes of the system (tests).

## License

GPL-3.0-or-later. The Openbox configuration and themes come from the work of Aditya Shakya
(adi1090x), GPL3.
