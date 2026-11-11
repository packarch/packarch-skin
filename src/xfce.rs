//! XFCE desktop: wallpaper, theme button of the panel (a launcher plugin with
//! one item per theme and "Couleur des dossiers"), and the theme chooser
//! window (yad), which works without the panel.

use crate::apply::{self, write_file, xfconf, xfconf_get, Target};
use crate::{current, dry, run, spawn, Dirs};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

/// Thumbnails of the wallpapers of the themes (packarch-xfce-settings),
/// <id>.png
const ICONS: &str = "/usr/share/xfcethemes/icons";

/// Item of the theme launcher that is not a theme (it opens
/// packarch-folder-color): always last, at the top of the menu of a panel at
/// the bottom of the screen (xfce shows the items in reverse order there)
pub const FOLDER_COLOR: &str = "folder-color.desktop";

/// Launcher item of the theme ID
fn item(id: &str) -> String {
    format!("theme-{id}.desktop")
}

/// Sets an xfconf property of type KIND, created if it does not exist
pub fn xfconf_new(channel: &str, prop: &str, kind: &str, value: &str) {
    if dry() || xfconf_get(channel, prop).is_none() {
        run("xfconf-query", &["-c", channel, "-p", prop, "-n", "-t", kind, "-s", value]);
    } else {
        xfconf(channel, prop, value);
    }
}

/// Output of xfconf-query ARGS (None in dry mode or on failure)
fn xfconf_out(args: &[&str]) -> Option<String> {
    if dry() {
        return None;
    }
    let out = Command::new("xfconf-query").args(args).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Values of an array property ("Value is an array with N items:" then one
/// value per line)
fn xfconf_array(channel: &str, prop: &str) -> Vec<String> {
    xfconf_out(&["-c", channel, "-p", prop])
        .unwrap_or_default()
        .lines()
        .skip_while(|l| !l.trim().is_empty())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Sets the array property PROP to VALUES of type KIND (created if missing)
fn xfconf_set_array(channel: &str, prop: &str, kind: &str, values: &[String]) {
    let mut args = vec!["-c", channel, "-p", prop];
    if dry() || xfconf_out(&["-c", channel, "-p", prop]).is_none() {
        args.push("-n");
    }
    args.push("--force-array");
    for v in values {
        args.extend(["-t", kind, "-s", v]);
    }
    run("xfconf-query", &args);
}

/// Names of the monitors (xrandr --listmonitors: the last field of each
/// line after the first)
fn monitors() -> Vec<String> {
    if dry() {
        return Vec::new();
    }
    Command::new("xrandr")
        .arg("--listmonitors")
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .skip(1)
                .filter_map(|l| l.split_whitespace().last().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Wallpaper: every image setting of xfdesktop, and the ones of xfdesktop
/// 4.20 (/backdrop/screen0/monitor<OUTPUT>/workspace<N>), created for each
/// monitor and desktop (missing in a new session)
pub fn wallpaper(image: &str) {
    for prop in xfconf_out(&["-c", "xfce4-desktop", "-l"]).unwrap_or_default().lines() {
        if prop.ends_with("/image-path") || prop.ends_with("/last-image") {
            xfconf("xfce4-desktop", prop, image);
        }
    }
    let count = xfconf_get("xfwm4", "/general/workspace_count")
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(4);
    for mon in monitors() {
        for ws in 0..count {
            let base = format!("/backdrop/screen0/monitor{mon}/workspace{ws}");
            xfconf_new("xfce4-desktop", &format!("{base}/last-image"), "string", image);
            xfconf_new("xfce4-desktop", &format!("{base}/image-style"), "int", "5");
        }
    }
    run("xfdesktop", &["--reload"]);
}

/// Items of the theme launcher: the theme CURRENT first, the other themes
/// (IDS order), "Couleur des dossiers" last
pub fn items(ids: &[String], current: &str) -> Vec<String> {
    let mut v = Vec::with_capacity(ids.len() + 1);
    if ids.iter().any(|i| i == current) {
        v.push(item(current));
    }
    v.extend(ids.iter().filter(|i| *i != current).map(|i| item(i)));
    v.push(FOLDER_COLOR.to_string());
    v
}

/// Files of the theme launcher: (file name, content) of each theme of DIRS
/// and of "Couleur des dossiers"
fn launcher_files(dirs: &Dirs) -> Vec<(String, String)> {
    // Thumbnails next to the themes (ROOT/usr/share/xfcethemes when rendering)
    let thumbs = Some(dirs.thumbs.clone());
    let mut files: Vec<(String, String)> = dirs
        .list()
        .iter()
        .map(|e| {
            let name = e.display_name(e.load().ok().as_ref());
            // The thumbnail of a theme of the user is in its directory; the
            // ones of the system in ICONS (under ROOT when rendering)
            let icon = match e.thumbnail(thumbs.as_deref()) {
                Some(t) if t.starts_with(&e.dir) => t.display().to_string(),
                Some(_) => format!("{ICONS}/{}.png", e.id),
                None => "preferences-desktop-theme".into(),
            };
            (
                item(&e.id),
                format!(
                    "[Desktop Entry]\nName=Thème {name}\nComment=Appliquer le thème {name}\nExec=packarch-skin apply {}\nIcon={icon}\nTerminal=false\nType=Application\nCategories=Utility;\n",
                    e.id
                ),
            )
        })
        .collect();
    files.push((
        FOLDER_COLOR.to_string(),
        "[Desktop Entry]\nName=Couleur des dossiers\nComment=Changer la couleur des dossiers du thème d'icônes\nExec=packarch-folder-color\nIcon=folder\nTerminal=false\nType=Application\nCategories=Settings;\nStartupNotify=true\n".to_string(),
    ));
    files
}

/// Writes the launcher files into DIR, removing the theme items that are not
/// themes any more
fn write_launcher(dirs: &Dirs, dir: &Path) {
    let files = launcher_files(dirs);
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if n.starts_with("theme-") && n.ends_with(".desktop") && !files.iter().any(|(f, _)| *f == n) {
            let _ = fs::remove_file(e.path());
        }
    }
    for (name, text) in &files {
        let file = dir.join(name);
        if fs::read_to_string(&file).ok().as_deref() != Some(text.as_str()) {
            write_file(&file, text);
        }
    }
}

fn theme_ids(dirs: &Dirs) -> Vec<String> {
    dirs.list().into_iter().map(|e| e.id).collect()
}

/// Identifier of the launcher plugin of the panel that holds the themes (an
/// item theme-*.desktop)
fn find_button() -> Option<u32> {
    let props = xfconf_out(&["-c", "xfce4-panel", "-l"]).unwrap_or_default();
    props.lines().find_map(|prop| {
        let n = prop.strip_prefix("/plugins/plugin-")?.strip_suffix("/items")?.parse::<u32>().ok()?;
        xfconf_array("xfce4-panel", prop)
            .iter()
            .any(|i| i.starts_with("theme-") && i.ends_with(".desktop"))
            .then_some(n)
    })
}

/// Files and items of the launcher plugin N, with the theme CURRENT first
fn sync_button(dirs: &Dirs, n: u32, current: &str) {
    write_launcher(dirs, &dirs.config.join(format!("xfce4/panel/launcher-{n}")));
    // The launcher does not move the clicked item first: "Couleur des
    // dossiers" stays last, packarch-skin puts the applied theme first
    xfconf_new("xfce4-panel", &format!("/plugins/plugin-{n}/move-first"), "bool", "false");
    let prop = format!("/plugins/plugin-{n}/items");
    let new = items(&theme_ids(dirs), current);
    if xfconf_array("xfce4-panel", &prop) != new {
        xfconf_set_array("xfce4-panel", &prop, "string", &new);
    }
}

/// After "apply": the theme button (if the panel has one) shows the theme ID
pub fn update_button(dirs: &Dirs, id: &str) {
    if let Some(n) = find_button() {
        sync_button(dirs, n, id);
    }
}

/// "render": launcher files and items of the theme button of the
/// xfce4-panel.xml of /etc/skel (the launcher plugin whose items are themes)
pub fn render_button(dirs: &Dirs, id: &str) -> Result<(), String> {
    let xml = dirs.config.join("xfce4/xfconf/xfce-perchannel-xml/xfce4-panel.xml");
    let Ok(text) = fs::read_to_string(&xml) else { return Ok(()) };
    let lines: Vec<&str> = text.lines().collect();
    let Some(first) = lines.iter().position(|l| l.contains("value=\"theme-")) else {
        return Err(format!("{} : pas de lanceur des thèmes (élément theme-*.desktop)", xml.display()));
    };
    let start = (0..first)
        .rev()
        .find(|&i| lines[i].contains("<property name=\"items\" type=\"array\">"))
        .ok_or("xfce4-panel.xml : liste « items » du lanceur des thèmes introuvable")?;
    let n = (0..start)
        .rev()
        .find_map(|i| {
            let rest = lines[i].trim().strip_prefix("<property name=\"plugin-")?;
            rest.split('"').next()?.parse::<u32>().ok()
        })
        .ok_or("xfce4-panel.xml : greffon du lanceur des thèmes introuvable")?;
    let end = (start..lines.len())
        .find(|&i| lines[i].trim() == "</property>")
        .ok_or("xfce4-panel.xml : fin de la liste « items » introuvable")?;
    let indent: String = lines[first].chars().take_while(|c| c.is_whitespace()).collect();
    let mut out: Vec<String> = lines[..=start].iter().map(|l| l.to_string()).collect();
    for i in items(&theme_ids(dirs), id) {
        out.push(format!("{indent}<value type=\"string\" value=\"{i}\"/>"));
    }
    out.extend(lines[end..].iter().map(|l| l.to_string()));
    let mut new = out.join("\n");
    if text.ends_with('\n') {
        new.push('\n');
    }
    if new != text {
        write_file(&xml, &new);
    }
    write_launcher(dirs, &dirs.config.join(format!("xfce4/panel/launcher-{n}")));
    Ok(())
}

/// "panel": puts the theme button back on the panel (in the first panel,
/// before the window list), or updates it if it is there
pub fn panel(dirs: &Dirs) -> Result<(), String> {
    let cur = current(dirs);
    if let Some(n) = find_button() {
        sync_button(dirs, n, &cur);
        println!("Bouton des thèmes à jour (greffon {n})");
        return Ok(());
    }
    if !dry() && xfconf_out(&["-c", "xfce4-panel", "-l"]).is_none() {
        return Err("réglages du tableau de bord XFCE (xfce4-panel) illisibles".into());
    }
    let listing = xfconf_out(&["-c", "xfce4-panel", "-l", "-v"]).unwrap_or_default();
    // Plugins: "/plugins/plugin-N   TYPE"
    let plugins: Vec<(u32, String)> = listing
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let n = w.next()?.strip_prefix("/plugins/plugin-")?.parse::<u32>().ok()?;
            Some((n, w.next().unwrap_or("").to_string()))
        })
        .collect();
    let n = plugins.iter().map(|(n, _)| *n).max().unwrap_or(0) + 1;
    let panel = xfconf_array("xfce4-panel", "/panels").into_iter().next().unwrap_or_else(|| "1".into());
    let ids_prop = format!("/panels/panel-{panel}/plugin-ids");
    let mut ids = xfconf_array("xfce4-panel", &ids_prop);
    let tasklist = plugins.iter().find(|(_, t)| t == "tasklist").map(|(n, _)| n.to_string());
    match tasklist.and_then(|t| ids.iter().position(|i| *i == t)) {
        Some(pos) => ids.insert(pos, n.to_string()),
        None => ids.push(n.to_string()),
    }
    xfconf_new("xfce4-panel", &format!("/plugins/plugin-{n}"), "string", "launcher");
    sync_button(dirs, n, &cur);
    xfconf_set_array("xfce4-panel", &ids_prop, "int", &ids);
    // The panel only creates its plugins at startup
    spawn("xfce4-panel", &["-r"], &[]);
    run(
        "notify-send",
        &["-a", "Thèmes", "-i", "preferences-desktop-theme", "Bouton des thèmes remis dans le tableau de bord"],
    );
    println!("Bouton des thèmes ajouté (greffon {n}, tableau de bord {panel})");
    Ok(())
}

/// Lock of the theme chooser: one window at a time, whatever starts it (menu
/// entry, theme button, keyboard shortcut). Not the file of the flock of
/// packarch-sway-settings 1.0-3 (packarch-skin-chooser.lock): flock holds it
/// while it runs the chooser, which would then never open. None when another
/// chooser holds it.
fn chooser_lock() -> Option<Option<fs::File>> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .filter(|d| d.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    // The lock cannot be taken (file not writable...): the chooser opens anyway
    let Ok(file) = fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join("packarch-skin-chooser.instance")) else {
        return Some(None);
    };
    match file.try_lock() {
        Ok(()) => Some(Some(file)),
        Err(fs::TryLockError::WouldBlock) => None,
        Err(_) => Some(None),
    }
}

/// "chooser": window with the themes (thumbnail, name, ✓ for the current
/// one) and "Couleur des dossiers" (yad); applies the chosen theme and opens
/// again, until "Fermer". Does nothing when a chooser is already open.
pub fn chooser(dirs: &Dirs) -> Result<(), String> {
    // Kept until the end of the function
    let Some(_lock) = (if dry() { Some(None) } else { chooser_lock() }) else {
        return Ok(());
    };
    let thumbs = Some(dirs.thumbs.clone());
    loop {
        let cur = current(dirs);
        let mut args: Vec<String> = [
            "--list", "--title=Thèmes Packarch", "--window-icon=preferences-desktop-theme",
            "--width=380", "--height=600", "--center", "--no-headers",
            "--column=:IMG", "--column=Thème", "--column=id:HD",
            "--print-column=3", "--separator=", "--button=Fermer:1", "--button=Appliquer:0",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        args.extend(["folder".into(), "Couleur des dossiers".into(), "folder-color".into()]);
        for e in dirs.list() {
            let name = e.display_name(e.load().ok().as_ref());
            let icon = e.thumbnail(thumbs.as_deref()).map_or("preferences-desktop-theme".into(), |p| p.display().to_string());
            let mark = if e.id == cur { "  ✓" } else { "" };
            args.extend([icon, format!("{name}{mark}"), e.id]);
        }
        if dry() {
            println!("[dry] yad {}", args.join(" "));
            return Ok(());
        }
        let out = Command::new("yad")
            .args(&args)
            .stderr(Stdio::null())
            .output()
            .map_err(|_| "commande introuvable : yad".to_string())?;
        let choice = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !out.status.success() || choice.is_empty() {
            return Ok(());
        }
        if choice == "folder-color" {
            spawn("packarch-folder-color", &[], &[]);
            return Ok(());
        }
        let entry = dirs
            .list()
            .into_iter()
            .find(|e| e.id == choice)
            .ok_or_else(|| format!("thème inconnu : {choice}"))?;
        if let Err(e) = apply::apply(dirs, &entry, Target::Session) {
            run("notify-send", &["-a", "Thèmes", "-u", "critical", &format!("Erreur : {e}")]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::items;

    #[test]
    fn items_current_first_folder_color_last() {
        let ids: Vec<String> = ["default", "hacking", "spark"].iter().map(|s| s.to_string()).collect();
        assert_eq!(
            items(&ids, "spark"),
            ["theme-spark.desktop", "theme-default.desktop", "theme-hacking.desktop", "folder-color.desktop"]
        );
        // Unknown current theme: the themes in order
        assert_eq!(
            items(&ids, "old"),
            ["theme-default.desktop", "theme-hacking.desktop", "theme-spark.desktop", "folder-color.desktop"]
        );
    }
}
