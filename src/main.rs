//! packarch-skin — theme (skin) chooser of the Packarch Openbox desktop.
//!
//! The themes are the directories of ~/.config/openbox/themes that hold a
//! theme.toml (colors, fonts, GTK/icon/cursor themes, Openbox, dunst,
//! picom settings) next to their polybar/ and rofi/ layouts. Applying
//! a theme writes these settings into the configuration of each program and
//! relaunches the bar. The fonts of openbox/themes/fonts.toml, chosen with
//! "font", replace the ones of every theme. "render" writes the default
//! theme into the /etc/skel of the packarch-openbox-settings package, so that the
//! first session starts with it.
//!
//! On the XFCE desktop (XDG_CURRENT_DESKTOP, or no Openbox themes), the
//! themes are copies of the same theme.toml files, in
//! /usr/share/xfcethemes/themes (packarch-xfce-settings): "apply" sets the
//! wallpaper, GTK, xfwm4, icon and cursor themes, the colors of
//! xfce4-terminal and the theme button of the panel; "render" writes the
//! default theme into the /etc/skel of packarch-xfce-settings.
//!
//! On the Sway desktop (XDG_CURRENT_DESKTOP, or the Sway configuration of
//! packarch-sway-settings without Openbox themes), the themes are other
//! copies, in /usr/share/packarch/sway/themes: "apply" sets the wallpaper,
//! the colors of the windows, waybar, rofi, mako, kitty and hyprlock and the
//! GTK, icon and cursor themes (sway.rs).
//!
//! On XFCE and Sway, a theme of ~/.config/packarch-skin/themes replaces the
//! theme of the system with the same identifier, or adds one ("edit" copies
//! a theme of the system there): the themes can be changed without root, and
//! the updates of the packages keep the changes.

mod apply;
mod bar;
mod color;
mod edit;
mod fonts;
mod sway;
mod theme;
mod xfce;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Stdio};

const USAGE: &str = "\
Usage : packarch-skin COMMANDE

  list                    thèmes disponibles (identifiant et nom)
  current                 thème courant
  apply [THÈME]           applique THÈME (par défaut le thème courant)
  render RACINE [THÈME]   écrit THÈME (par défaut « default ») dans les
                          fichiers de RACINE/etc/skel, sans rien lancer
                          (construction des paquets packarch-openbox-settings,
                          packarch-xfce-settings et packarch-sway-settings)
  bar                     relance la barre du thème courant (Openbox)
  font [CIBLE]            police de chaque programme, ou de CIBLE (polybar,
                          rofi, terminal, geany, gtk, openbox, dunst)
  font CIBLE FAMILLE STYLE TAILLE
                          choisit la police de CIBLE pour tous les thèmes
                          (openbox/themes/fonts.toml) et l'applique
  wallpaper [THÈME]       chemin du fond d'écran de THÈME
  edit [THÈME]            ouvre le theme.toml de THÈME (par défaut le thème
                          courant) dans l'éditeur ; XFCE et Sway : le copie
                          d'abord dans ~/.config/packarch-skin/themes
  check [THÈME...]        vérifie les fichiers theme.toml
  openbox-menu            menu Openbox (pipemenu) des thèmes
  chooser                 fenêtre de choix du thème (yad), avec « Couleur
                          des dossiers »
  panel                   remet le bouton des thèmes dans le tableau de
                          bord XFCE, ou le met à jour (XFCE)

Bureau XFCE : mêmes thèmes (/usr/share/xfcethemes/themes), sans les
commandes bar, font et openbox-menu.
Bureau Sway : mêmes thèmes (/usr/share/packarch/sway/themes), sans les
commandes bar, font, openbox-menu et panel.
XFCE et Sway : un thème de ~/.config/packarch-skin/themes remplace le thème
du système de même identifiant (ou en ajoute un).";

/// Themes of the XFCE desktop (packarch-xfce-settings), under the root
pub const XFCE_THEMES: &str = "usr/share/xfcethemes/themes";

/// Themes of the Sway desktop (packarch-sway-settings), under the root
pub const SWAY_THEMES: &str = "usr/share/packarch/sway/themes";

/// Themes of the user on XFCE and Sway, under the configuration directory
pub const USER_THEMES: &str = "packarch-skin/themes";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Desktop {
    Openbox,
    Xfce,
    Sway,
}

pub struct Dirs {
    pub home: PathBuf,
    pub config: PathBuf,
    pub themes: PathBuf,
    /// Themes of the user, before the ones of the system (XFCE and Sway
    /// session; none with Openbox, whose themes are already the user's, and
    /// for render)
    pub user_themes: Option<PathBuf>,
    /// File of the current theme
    pub state: PathBuf,
    pub desktop: Desktop,
}

impl Dirs {
    fn new() -> Dirs {
        let home = PathBuf::from(env::var_os("HOME").unwrap_or_else(|| "/".into()));
        let config = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".config"));
        let current = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let is = |name: &str| current.split(':').any(|x| x.eq_ignore_ascii_case(name));
        let root = Path::new("/");
        let desktop = if is("sway") {
            Desktop::Sway
        } else if is("xfce") {
            Desktop::Xfce
        } else if config.join("openbox/themes").is_dir() {
            Desktop::Openbox
        } else if is_sway(&config, root) {
            Desktop::Sway
        } else {
            Desktop::Xfce
        };
        let mut dirs = Dirs::with(home, config, root, desktop);
        if desktop != Desktop::Openbox {
            dirs.user_themes = Some(dirs.config.join(USER_THEMES));
        }
        dirs
    }

    /// Directories of the package root ROOT (render: home ROOT/etc/skel):
    /// Openbox when it has the Openbox themes, else Sway when it has the Sway
    /// themes and configuration, else XFCE when it has the XFCE themes
    fn at(root: &Path) -> Dirs {
        let home = root.join("etc/skel");
        let config = home.join(".config");
        let desktop = if config.join("openbox/themes").is_dir() {
            Desktop::Openbox
        } else if is_sway(&config, root) {
            Desktop::Sway
        } else if root.join(XFCE_THEMES).is_dir() {
            Desktop::Xfce
        } else {
            Desktop::Openbox
        };
        Dirs::with(home, config, root, desktop)
    }

    fn with(home: PathBuf, config: PathBuf, root: &Path, desktop: Desktop) -> Dirs {
        if desktop == Desktop::Sway {
            // PACKARCH_SKIN_SWAY_THEMES: other themes directory (tests)
            let themes = env::var_os("PACKARCH_SKIN_SWAY_THEMES")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join(SWAY_THEMES));
            let state = config.join("sway/.current_theme");
            Dirs { home, config, themes, user_themes: None, state, desktop }
        } else if desktop == Desktop::Xfce {
            // PACKARCH_SKIN_XFCE_THEMES: other themes directory (tests)
            let themes = env::var_os("PACKARCH_SKIN_XFCE_THEMES")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join(XFCE_THEMES));
            // The file of the former theme switcher of XFCE
            let state = config.join("xfcethemes/.current_theme");
            Dirs { home, config, themes, user_themes: None, state, desktop: Desktop::Xfce }
        } else {
            let themes = config.join("openbox/themes");
            let state = themes.join(".current");
            Dirs { home, config, themes, user_themes: None, state, desktop: Desktop::Openbox }
        }
    }

    /// The themes: the ones of the user first, then the ones of the system
    pub fn list(&self) -> Vec<theme::Entry> {
        let mut d: Vec<PathBuf> = self.user_themes.iter().cloned().collect();
        d.push(self.themes.clone());
        theme::list_in(&d)
    }

    /// Error for the commands of the XFCE desktop only
    fn xfce_only(&self, cmd: &str) -> Result<(), String> {
        match self.desktop {
            Desktop::Xfce => Ok(()),
            _ => Err(format!("« {cmd} » : bureau XFCE seulement")),
        }
    }

    /// Error for the commands of the Openbox desktop only
    fn openbox_only(&self, cmd: &str) -> Result<(), String> {
        match self.desktop {
            Desktop::Openbox => Ok(()),
            _ => Err(format!("« {cmd} » : bureau Openbox seulement")),
        }
    }
}

/// The Sway configuration of packarch-sway-settings (CONFIG/sway/sway-theme)
/// and the Sway themes under ROOT
fn is_sway(config: &Path, root: &Path) -> bool {
    config.join("sway/sway-theme").is_file()
        && (root.join(SWAY_THEMES).is_dir() || env::var_os("PACKARCH_SKIN_SWAY_THEMES").is_some())
}

/// PACKARCH_SKIN_DRY=1: the external commands are printed, not run (tests)
pub fn dry() -> bool {
    env::var_os("PACKARCH_SKIN_DRY").is_some_and(|v| v == "1")
}

/// Runs a command, waits for it; false (with a message if it is missing)
/// when it fails
pub fn run(cmd: &str, args: &[&str]) -> bool {
    run_input(cmd, args, None)
}

/// Same as run, with INPUT written to the standard input of the command
pub fn run_input(cmd: &str, args: &[&str], input: Option<&str>) -> bool {
    if dry() {
        println!("[dry] {cmd} {}", args.join(" "));
        return true;
    }
    let mut c = Command::new(cmd);
    c.args(args).stdout(Stdio::null()).stderr(Stdio::null());
    if input.is_some() {
        c.stdin(Stdio::piped());
    }
    let child = c.spawn().and_then(|mut ch| {
        if let (Some(text), Some(mut stdin)) = (input, ch.stdin.take()) {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        }
        ch.wait()
    });
    match child {
        Ok(s) => s.success(),
        Err(_) => {
            eprintln!("packarch-skin : commande introuvable : {cmd}");
            false
        }
    }
}

/// Starts a command in the background (it keeps running after us)
pub fn spawn(cmd: &str, args: &[&str], envs: &[(&str, &str)]) {
    if dry() {
        let e: Vec<String> = envs.iter().map(|(k, v)| format!("{k}={v} ")).collect();
        println!("[dry] {}{cmd} {} &", e.concat(), args.join(" "));
        return;
    }
    let mut c = Command::new(cmd);
    c.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    for (k, v) in envs {
        c.env(k, v);
    }
    if c.spawn().is_err() {
        eprintln!("packarch-skin : commande introuvable : {cmd}");
    }
}

/// Identifier of the current theme (themes/.current with Openbox), "default"
/// if none or if it is not a theme (a color of the former XFCE theme switcher)
pub fn current(dirs: &Dirs) -> String {
    fs::read_to_string(&dirs.state)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && dirs.list().iter().any(|e| e.id == *s))
        .unwrap_or_else(|| "default".into())
}

fn find(dirs: &Dirs, id: &str) -> Result<theme::Entry, String> {
    dirs.list()
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| format!("thème inconnu : {id} (voir « packarch-skin list »)"))
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// "edit": theme.toml of ENTRY in the editor. On XFCE and Sway, a theme of
/// the system is first copied into the themes of the user (its relative
/// wallpaper made absolute), which then replaces it.
fn edit(dirs: &Dirs, entry: &theme::Entry) -> Result<(), String> {
    let mut file = entry.dir.join("theme.toml");
    if let Some(user) = &dirs.user_themes
        && !entry.dir.starts_with(user)
    {
        let theme = entry.load()?;
        let mut text = fs::read_to_string(&file).map_err(|e| format!("{} : {e}", file.display()))?;
        if !Path::new(&theme.wallpaper).is_absolute() {
            let w = entry.dir.join(&theme.wallpaper);
            edit::set_line(&mut text, "wallpaper =", &format!("wallpaper = \"{}\"", w.display()));
        }
        let dir = user.join(&entry.id);
        file = dir.join("theme.toml");
        fs::create_dir_all(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
        fs::write(&file, text).map_err(|e| format!("{} : {e}", file.display()))?;
        if let Some(t) = entry.thumbnail(dirs.themes.parent().map(|p| p.join("icons")).as_deref()) {
            let _ = fs::copy(t, dir.join("thumbnail.png"));
        }
        println!("Thème {} copié dans {}", entry.id, dir.display());
        println!("(supprimer ce dossier pour revenir au thème du système)");
    }
    println!("{}", file.display());
    println!("Après modification : packarch-skin apply {}", entry.id);
    // VISUAL, else Geany (the editor of the three desktops), else the
    // default application (EDITOR is often a terminal editor)
    let f = file.to_string_lossy();
    let editor = env::var("VISUAL").ok().filter(|v| !v.trim().is_empty());
    match editor {
        Some(v) => spawn("sh", &["-c", &format!("{v} \"$1\""), "sh", &f], &[]),
        None if has_command("geany") => spawn("geany", &[&f], &[]),
        None => spawn("xdg-open", &[&f], &[]),
    }
    Ok(())
}

/// COMMAND is in the PATH
fn has_command(cmd: &str) -> bool {
    env::var_os("PATH")
        .is_some_and(|p| env::split_paths(&p).any(|d| d.join(cmd).is_file()))
}

fn openbox_menu(dirs: &Dirs) {
    let themes = dirs.list();
    let cur = current(dirs);
    let mut out = String::from("<openbox_pipe_menu>\n");
    out += &format!("  <separator label=\"⁌ Thèmes ({}) ⁍\"/>\n", themes.len());
    for (i, e) in themes.iter().enumerate() {
        let name = e.display_name(e.load().ok().as_ref());
        let mark = if e.id == cur { "  ✓" } else { "" };
        out += &format!(
            "  <item label=\"{}. {}{}\">\n    <action name=\"Execute\">\n      <command>packarch-skin apply {}</command>\n    </action>\n  </item>\n",
            i + 1,
            xml_escape(&name).replace('_', "__"),
            mark,
            xml_escape(&e.id)
        );
    }
    out += "</openbox_pipe_menu>\n";
    print!("{out}");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let dirs = Dirs::new();
    let arg = |i: usize| args.get(i).map(String::as_str);
    let result = match arg(0) {
        Some("list") => {
            for e in dirs.list() {
                let name = e.display_name(e.load().ok().as_ref());
                println!("{}\t{}", e.id, name);
            }
            Ok(())
        }
        Some("current") => {
            println!("{}", current(&dirs));
            Ok(())
        }
        Some("apply") => {
            let id = arg(1).map(str::to_string).unwrap_or_else(|| current(&dirs));
            find(&dirs, &id).and_then(|e| apply::apply(&dirs, &e, apply::Target::Session))
        }
        Some("render") => match arg(1) {
            Some(root) => {
                let root = PathBuf::from(root);
                let skel = Dirs::at(&root);
                let id = arg(2).unwrap_or("default");
                find(&skel, id).and_then(|e| apply::apply(&skel, &e, apply::Target::Package(&root)))
            }
            None => {
                eprintln!("{USAGE}");
                exit(2);
            }
        },
        Some("bar") => dirs.openbox_only("bar").and_then(|()| bar::launch(&dirs)),
        Some("font") => dirs.openbox_only("font").and_then(|()| fonts::command(&dirs, &args[1..])),
        Some("wallpaper") => {
            let id = arg(1).map(str::to_string).unwrap_or_else(|| current(&dirs));
            find(&dirs, &id).and_then(|e| {
                let t = e.load()?;
                println!("{}", e.dir.join(t.wallpaper).display());
                Ok(())
            })
        }
        Some("check") => {
            let all = dirs.list();
            let wanted: Vec<&str> = args[1..].iter().map(String::as_str).collect();
            let mut errors = 0;
            for e in all.iter().filter(|e| wanted.is_empty() || wanted.contains(&e.id.as_str())) {
                match e.load().and_then(|t| apply::check(&t)) {
                    Ok(()) => println!("{} : ok", e.id),
                    Err(err) => {
                        println!("{} : {err}", e.id);
                        errors += 1;
                    }
                }
            }
            if errors > 0 { Err(format!("{errors} thème(s) en erreur")) } else { Ok(()) }
        }
        Some("edit") => {
            let id = arg(1).map(str::to_string).unwrap_or_else(|| current(&dirs));
            find(&dirs, &id).and_then(|e| edit(&dirs, &e))
        }
        Some("panel") => dirs.xfce_only("panel").and_then(|()| xfce::panel(&dirs)),
        Some("chooser") => xfce::chooser(&dirs),
        Some("openbox-menu") => dirs.openbox_only("openbox-menu").map(|()| openbox_menu(&dirs)),
        Some("-h") | Some("--help") | Some("help") => {
            println!("{USAGE}");
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("packarch-skin : {e}");
        exit(1);
    }
}
