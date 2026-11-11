//! packarch-skin — theme (skin) chooser of the Packarch Openbox, XFCE and
//! Sway desktops.
//!
//! On each desktop, the themes are the directories of
//! ~/.config/<desktop>/themes (openbox, xfce4, sway) that hold a theme.toml
//! (colors, fonts, GTK/icon/cursor themes, Openbox, dunst, picom
//! settings...): the user changes a theme there, without root. The themes of
//! the system that the user does not have are copied there by every command
//! (all of them the first time); a theme of the user is never replaced.
//! Themes of the system: the same directories in /etc/skel (new accounts
//! get them from there), from packarch-openbox-settings (with their polybar/
//! and rofi/ layouts), packarch-xfce-settings and packarch-sway-settings
//! (copies of the same theme.toml files).
//!
//! Openbox: applying a theme writes its settings into the configuration of
//! each program and relaunches the bar. The fonts of
//! openbox/themes/fonts.toml, chosen with "font", replace the ones of every
//! theme.
//!
//! XFCE (XDG_CURRENT_DESKTOP, or no Openbox themes): "apply" sets the
//! wallpaper, GTK, xfwm4, icon and cursor themes, the colors of
//! xfce4-terminal and the theme button of the panel.
//!
//! Sway (XDG_CURRENT_DESKTOP, or the Sway configuration of
//! packarch-sway-settings without Openbox themes): "apply" sets the
//! wallpaper, the colors of the windows, waybar, rofi, mako, kitty and
//! hyprlock and the GTK, icon and cursor themes (sway.rs).
//!
//! "render" writes the default theme into the /etc/skel of these packages
//! when they are built, so that the first session starts with it.

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
                          courant) dans l'éditeur
  check [THÈME...]        vérifie les fichiers theme.toml
  openbox-menu            menu Openbox (pipemenu) des thèmes
  chooser                 fenêtre de choix du thème (yad), avec « Couleur
                          des dossiers »
  panel                   remet le bouton des thèmes dans le tableau de
                          bord XFCE, ou le met à jour (XFCE)

Thèmes : ~/.config/openbox/themes, ~/.config/xfce4/themes ou
~/.config/sway/themes selon le bureau, modifiables ; les thèmes du système
absents y sont copiés, un thème déjà présent n'est jamais remplacé.
Bureau XFCE : sans les commandes bar, font et openbox-menu.
Bureau Sway : sans les commandes bar, font, openbox-menu et panel.";

/// Skeleton of the new accounts, under the root: the themes of the system are
/// in its .config/<desktop>/themes (packarch-openbox-settings,
/// packarch-xfce-settings, packarch-sway-settings)
pub const SKEL: &str = "etc/skel";

/// Themes of the Sway desktop (packarch-sway-settings), under the root
pub const SWAY_THEMES: &str = "etc/skel/.config/sway/themes";

/// Themes of the system of packarch-xfce-settings before 1.0.9-15 and
/// packarch-sway-settings before 1.0-20, under the root: used when /etc/skel
/// has none (packages not updated yet)
const XFCE_THEMES_OLD: &str = "usr/share/xfcethemes/themes";
const SWAY_THEMES_OLD: &str = "usr/share/packarch/sway/themes";

/// Thumbnails of the themes (<id>.png) of XFCE and Sway, under the root
const XFCE_THUMBS: &str = "usr/share/xfcethemes/icons";
const SWAY_THUMBS: &str = "usr/share/packarch/sway/icons";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Desktop {
    Openbox,
    Xfce,
    Sway,
}

impl Desktop {
    /// Themes of the desktop in the configuration directory CONFIG
    /// (~/.config, or the one of /etc/skel)
    fn themes_in(self, config: &Path) -> PathBuf {
        let d = match self {
            Desktop::Openbox => "openbox",
            Desktop::Xfce => "xfce4",
            Desktop::Sway => "sway",
        };
        config.join(d).join("themes")
    }
}

/// Copies the directory FROM to TO (symbolic links kept as links)
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let (src, dst) = (e.path(), to.join(e.file_name()));
        let kind = e.file_type()?;
        if kind.is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(&src)?, &dst)?;
        } else if kind.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

pub struct Dirs {
    pub home: PathBuf,
    pub config: PathBuf,
    /// Themes that are applied: ~/.config/<desktop>/themes (openbox, xfce4,
    /// sway) in the session; the themes of the package for render
    pub themes: PathBuf,
    /// Themes of the system, copied into THEMES when they are missing there
    /// (session only)
    pub system: Option<PathBuf>,
    /// Thumbnails of the themes of the system (<id>.png)
    pub thumbs: PathBuf,
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
        // The themes of the system: the ones of /etc/skel (tests: another
        // directory)
        let var = match desktop {
            Desktop::Openbox => None,
            Desktop::Xfce => Some("PACKARCH_SKIN_XFCE_THEMES"),
            Desktop::Sway => Some("PACKARCH_SKIN_SWAY_THEMES"),
        };
        let old = match desktop {
            Desktop::Openbox => None,
            Desktop::Xfce => Some(root.join(XFCE_THEMES_OLD)),
            Desktop::Sway => Some(root.join(SWAY_THEMES_OLD)),
        };
        let system = var.and_then(env::var_os).map(PathBuf::from).unwrap_or_else(|| {
            let skel = desktop.themes_in(&root.join(SKEL).join(".config"));
            match old {
                Some(o) if !skel.is_dir() && o.is_dir() => o,
                _ => skel,
            }
        });
        dirs.system = Some(system);
        dirs
    }

    /// Directories of the package root ROOT (render: home ROOT/etc/skel,
    /// whose themes are the themes of the package): Openbox when it has the
    /// Openbox themes, else Sway when it has the Sway themes and
    /// configuration, else XFCE when it has the XFCE themes
    fn at(root: &Path) -> Dirs {
        let home = root.join(SKEL);
        let config = home.join(".config");
        let desktop = if config.join("openbox/themes").is_dir() {
            Desktop::Openbox
        } else if is_sway(&config, root) {
            Desktop::Sway
        } else if Desktop::Xfce.themes_in(&config).is_dir() {
            Desktop::Xfce
        } else {
            Desktop::Openbox
        };
        Dirs::with(home, config, root, desktop)
    }

    /// THEMES: ~/.config/<desktop>/themes (CONFIG/<desktop>/themes)
    fn with(home: PathBuf, config: PathBuf, root: &Path, desktop: Desktop) -> Dirs {
        let themes = desktop.themes_in(&config);
        let (state, thumbs) = match desktop {
            Desktop::Sway => (config.join("sway/.current_theme"), root.join(SWAY_THUMBS)),
            // The current theme: file of the former theme switcher of XFCE
            // (read by init-backdrop.sh of packarch-xfce-settings)
            Desktop::Xfce => (config.join("xfcethemes/.current_theme"), root.join(XFCE_THUMBS)),
            Desktop::Openbox => (themes.join(".current"), config.join("openbox/icons")),
        };
        Dirs { home, config, themes, system: None, thumbs, state, desktop }
    }

    /// The themes of the user (or of the package for render)
    pub fn list(&self) -> Vec<theme::Entry> {
        theme::list(&self.themes)
    }

    /// Copies the themes of the system that the user does not have (all of
    /// them the first time): a theme of the user is never replaced. The
    /// themes of ~/.config/packarch-skin/themes (packarch-skin 1.4.0) are
    /// moved there first.
    fn add_missing_themes(&self) {
        let Some(system) = &self.system else { return };
        let old = self.config.join("packarch-skin/themes");
        for e in theme::list(&old) {
            let to = self.themes.join(&e.id);
            if !to.exists() && fs::create_dir_all(&self.themes).is_ok() && fs::rename(&e.dir, &to).is_ok() {
                eprintln!("Thème {} déplacé dans {}", e.id, self.themes.display());
            }
        }
        let _ = fs::remove_dir(&old).and_then(|()| fs::remove_dir(self.config.join("packarch-skin")));
        for e in theme::list(system) {
            let to = self.themes.join(&e.id);
            if !to.exists() {
                if let Err(err) = copy_dir(&e.dir, &to) {
                    eprintln!("packarch-skin : {} : {err}", to.display());
                }
            }
        }
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
        && (root.join(SWAY_THEMES).is_dir()
            || root.join(SWAY_THEMES_OLD).is_dir()
            || env::var_os("PACKARCH_SKIN_SWAY_THEMES").is_some())
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

/// "edit": theme.toml of ENTRY in the editor
fn edit(entry: &theme::Entry) -> Result<(), String> {
    let file = entry.dir.join("theme.toml");
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
    if !matches!(arg(0), None | Some("render" | "-h" | "--help" | "help")) {
        dirs.add_missing_themes();
    }
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
            find(&dirs, &id).and_then(|e| edit(&e))
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
