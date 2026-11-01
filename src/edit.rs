//! Text edits of the configuration files (what apply.sh did with sed and
//! xmlstarlet).

/// In each line, replaces from KEY to the end of the line with LINE (sed
/// "s/KEY.*/LINE/"), where KEY is not the end of a longer name (the character
/// before it is not alphanumeric, '_' or '-': "width = " does not match
/// "frame_width = "). Returns the number of lines changed.
pub fn set_line(text: &mut String, key: &str, line: &str) -> usize {
    let mut n = 0;
    let out: Vec<String> = text
        .split('\n')
        .map(|l| match find_key(l, key) {
            Some(i) => {
                n += 1;
                format!("{}{}", &l[..i], line)
            }
            None => l.to_string(),
        })
        .collect();
    *text = out.join("\n");
    n
}

fn find_key(line: &str, key: &str) -> Option<usize> {
    let mut start = 0;
    while let Some(off) = line[start..].find(key) {
        let i = start + off;
        let prev = line[..i].chars().next_back();
        if !matches!(prev, Some(c) if c.is_alphanumeric() || c == '_' || c == '-') {
            return Some(i);
        }
        start = i + key.len();
    }
    None
}

/// Replaces the lines between the line START and the line END (both kept)
/// with INNER. Returns false when the markers are missing.
pub fn set_block(text: &mut String, start: &str, end: &str, inner: &str) -> bool {
    let lines: Vec<&str> = text.split('\n').collect();
    let Some(s) = lines.iter().position(|l| l.trim() == start) else { return false };
    let Some(e) = lines[s + 1..].iter().position(|l| l.trim() == end).map(|e| e + s + 1) else {
        return false;
    };
    let mut out: Vec<String> = lines[..=s].iter().map(|l| l.to_string()).collect();
    out.extend(inner.split('\n').map(str::to_string));
    out.extend(lines[e..].iter().map(|l| l.to_string()));
    *text = out.join("\n");
    true
}

/// Removes the first line containing NEEDLE and everything after it
/// (sed "/NEEDLE/Q")
pub fn truncate_at(text: &mut String, needle: &str) {
    let mut out = Vec::new();
    for l in text.split('\n') {
        if l.contains(needle) {
            break;
        }
        out.push(l);
    }
    // Every kept line ends with a newline, an empty last line included
    let mut t = out.join("\n");
    if !out.is_empty() {
        t.push('\n');
    }
    *text = t;
}

// --- XML (rc.xml) -------------------------------------------------------------

struct Tag {
    start: usize,
    end: usize,
    name: String,
    closing: bool,
    empty: bool,
}

/// Next tag at or after POS (comments, declarations and processing
/// instructions are skipped)
fn next_tag(xml: &str, mut pos: usize) -> Option<Tag> {
    loop {
        let start = pos + xml[pos..].find('<')?;
        let rest = &xml[start..];
        if rest.starts_with("<!--") {
            pos = start + rest.find("-->")? + 3;
            continue;
        }
        if rest.starts_with("<?") || rest.starts_with("<!") {
            pos = start + rest.find('>')? + 1;
            continue;
        }
        let end = start + rest.find('>')? + 1;
        let inner = &xml[start + 1..end - 1];
        let closing = inner.starts_with('/');
        let empty = inner.ends_with('/');
        let name: String = inner
            .trim_start_matches('/')
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '/' && *c != '>')
            .collect();
        return Some(Tag { start, end, name, closing, empty });
    }
}

/// Content range (between the start and end tags) of the first child element
/// of the range FROM..TO named NAME (with ATTR="VALUE" when given)
fn child(xml: &str, from: usize, to: usize, name: &str, attr: Option<(&str, &str)>) -> Option<(usize, usize)> {
    let mut pos = from;
    let mut depth = 0usize;
    let mut found: Option<usize> = None;
    while let Some(t) = next_tag(xml, pos) {
        if t.start >= to {
            break;
        }
        pos = t.end;
        if t.closing {
            if depth == 0 {
                break;
            }
            depth -= 1;
            if depth == 0 {
                if let Some(s) = found {
                    return Some((s, t.start));
                }
            }
            continue;
        }
        if t.empty {
            continue;
        }
        if depth == 0 && t.name == name {
            let ok = match attr {
                Some((a, v)) => xml[t.start..t.end].contains(&format!("{a}=\"{v}\"")),
                None => true,
            };
            if ok {
                found = Some(t.end);
            }
        }
        depth += 1;
    }
    None
}

/// Sets the text of the element at PATH (from the root element), e.g.
/// [("theme", None), ("font", Some(("place", "MenuItem"))), ("size", None)].
/// Returns false when the element does not exist.
pub fn set_xml_text(xml: &mut String, path: &[(&str, Option<(&str, &str)>)], value: &str) -> bool {
    let mut range = (0, xml.len());
    // Root element
    let mut pos = 0;
    while let Some(t) = next_tag(xml, pos) {
        if !t.closing && !t.empty {
            range = (t.end, xml.rfind(&format!("</{}>", t.name)).unwrap_or(xml.len()));
            break;
        }
        pos = t.end;
    }
    for (name, attr) in path {
        match child(xml, range.0, range.1, name, *attr) {
            Some(r) => range = r,
            None => return false,
        }
    }
    let escaped = value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    xml.replace_range(range.0..range.1, &escaped);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_keys() {
        let mut t = "width = 1\nframe_width = 2\n  height = 3\nline_height = 4".to_string();
        assert_eq!(set_line(&mut t, "width = ", "width = 10"), 1);
        assert_eq!(set_line(&mut t, "height = ", "height = 30"), 1);
        assert_eq!(t, "width = 10\nframe_width = 2\n  height = 30\nline_height = 4");
        let mut t = "# blur-method =\nmethod = \"a\";".to_string();
        assert_eq!(set_line(&mut t, "method = ", "method = \"b\";"), 1);
        assert_eq!(t, "# blur-method =\nmethod = \"b\";");
        let mut t = "active_text_color='a'\ninactive_text_color='b'".to_string();
        set_line(&mut t, "active_text_color=", "active_text_color='x'");
        assert_eq!(t, "active_text_color='x'\ninactive_text_color='b'");
    }

    #[test]
    fn blocks() {
        let mut t = "a\n#-cr-start\ncorner-radius = 0;\n#-cr-end\nb".to_string();
        assert!(set_block(&mut t, "#-cr-start", "#-cr-end", "corner-radius = 6;"));
        assert_eq!(t, "a\n#-cr-start\ncorner-radius = 6;\n#-cr-end\nb");
        let mut t = "x\n\n[urgency_low]\ny".to_string();
        truncate_at(&mut t, "urgency_low");
        assert_eq!(t, "x\n\n");
    }

    #[test]
    fn xml() {
        let mut x = r#"<?xml version="1.0"?>
<openbox_config xmlns="http://openbox.org/3.4/rc">
  <!-- <theme><name>no</name></theme> -->
  <theme>
    <name>Old</name>
    <titleLayout>NLIMC</titleLayout>
    <font place="ActiveWindow"><name>A</name><size>8</size></font>
    <font place="MenuItem"><name>B</name><size>9</size></font>
  </theme>
  <margins><top>0</top><bottom>0</bottom></margins>
</openbox_config>"#
            .to_string();
        assert!(set_xml_text(&mut x, &[("theme", None), ("name", None)], "OB-Arc"));
        assert!(set_xml_text(&mut x, &[("theme", None), ("font", Some(("place", "MenuItem"))), ("size", None)], "10"));
        assert!(set_xml_text(&mut x, &[("margins", None), ("bottom", None)], "12"));
        assert!(!set_xml_text(&mut x, &[("menu", None), ("file", None)], "x"));
        assert!(x.contains("<name>OB-Arc</name>"));
        assert!(x.contains(r#"<font place="MenuItem"><name>B</name><size>10</size></font>"#));
        assert!(x.contains(r#"<font place="ActiveWindow"><name>A</name><size>8</size></font>"#));
        assert!(x.contains("<bottom>12</bottom>"));
        assert!(x.contains("<!-- <theme><name>no</name></theme> -->"));
    }
}
