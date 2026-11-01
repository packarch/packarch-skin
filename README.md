<div align="center">
  <h1>Packarch Skin</h1>
  <p><strong>Theme chooser of the Packarch Openbox and XFCE desktops, written in Rust</strong></p>
</div>

Packarch Skin applies the themes of [Packarch](https://github.com/packarch/CachyOS-Packarch).
Each theme is described by a `theme.toml` file (colors, fonts, GTK, xfwm4, icon and cursor
themes, folder color, wallpaper, Openbox, dunst and picom settings), shipped by the
`packarch-openbox-settings` package (`/etc/skel/.config/openbox/themes/<theme>/`, next to the
polybar and rofi layouts of the theme). The XFCE desktop uses the same themes, copied by
`packarch-xfce-settings` to `/usr/share/xfcethemes/themes`.

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
packarch-skin check [THEME...]        checks the theme.toml files
packarch-skin openbox-menu            Openbox pipemenu of the themes
```

The messages are in French.

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
`font` and `openbox-menu` are refused). The current theme is in
`~/.config/xfcethemes/.current_theme`.

Optional section of `theme.toml` for XFCE:

```toml
[xfce]
xfwm4_theme = "Default"   # default: the GTK theme, which must have an xfwm4 directory
```

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
`PACKARCH_SKIN_XFCE_THEMES` sets another directory of XFCE themes (tests).

## License

GPL-3.0-or-later. The Openbox configuration and themes come from the work of Aditya Shakya
(adi1090x), GPL3.
