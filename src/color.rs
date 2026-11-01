//! Colors of a theme: parsing, the `pastel` operations the themes use
//! (lighten/darken in HSL, gradient in Lab) and the evaluation of the color
//! expressions of theme.toml.
//!
//! An expression is one of:
//!   `#RRGGBB` (or `#AARRGGBB`), kept as written
//!   a name: another color of the `[colors]` table, or `gradient1`..`gradient6`
//!   `lighten(EXPR, AMOUNT)` / `darken(EXPR, AMOUNT)`: HSL lightness +/- AMOUNT

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Rgb {
    pub fn parse(hex: &str) -> Option<Rgb> {
        let h = hex.strip_prefix('#')?;
        // #AARRGGBB (polybar): the color part is the last 6 digits
        let h = match h.len() {
            6 => h,
            8 => &h[2..],
            _ => return None,
        };
        let v = u32::from_str_radix(h, 16).ok()?;
        Some(Rgb {
            r: ((v >> 16) & 0xff) as f64 / 255.0,
            g: ((v >> 8) & 0xff) as f64 / 255.0,
            b: (v & 0xff) as f64 / 255.0,
        })
    }

    pub fn hex(&self) -> String {
        let c = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", c(self.r), c(self.g), c(self.b))
    }

    /// `r,g,b` with 3 decimals (maim/slop border color)
    pub fn rgb_float(&self) -> String {
        format!("{:.3},{:.3},{:.3}", self.r, self.g, self.b)
    }

    fn to_hsl(self) -> (f64, f64, f64) {
        let max = self.r.max(self.g).max(self.b);
        let min = self.r.min(self.g).min(self.b);
        let l = (max + min) / 2.0;
        if max == min {
            return (0.0, 0.0, l);
        }
        let d = max - min;
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let h = if max == self.r {
            (self.g - self.b) / d + if self.g < self.b { 6.0 } else { 0.0 }
        } else if max == self.g {
            (self.b - self.r) / d + 2.0
        } else {
            (self.r - self.g) / d + 4.0
        };
        (h * 60.0, s, l)
    }

    fn from_hsl(h: f64, s: f64, l: f64) -> Rgb {
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let hp = (h / 60.0).rem_euclid(6.0);
        let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
        let (r, g, b) = match hp as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = l - c / 2.0;
        Rgb { r: r + m, g: g + m, b: b + m }
    }

    /// HSL lightness + amount (pastel lighten)
    pub fn lighten(self, amount: f64) -> Rgb {
        let (h, s, l) = self.to_hsl();
        Rgb::from_hsl(h, s, (l + amount).clamp(0.0, 1.0))
    }

    fn to_lab(self) -> (f64, f64, f64) {
        let lin = |c: f64| {
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        let (r, g, b) = (lin(self.r), lin(self.g), lin(self.b));
        let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
        let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
        let f = |t: f64| if t > 216.0 / 24389.0 { t.cbrt() } else { (24389.0 / 27.0 * t + 16.0) / 116.0 };
        let (fx, fy, fz) = (f(x), f(y), f(z));
        (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
    }

    fn from_lab(l: f64, a: f64, b: f64) -> Rgb {
        let fy = (l + 16.0) / 116.0;
        let fx = fy + a / 500.0;
        let fz = fy - b / 200.0;
        let finv = |t: f64| {
            let t3 = t * t * t;
            if t3 > 216.0 / 24389.0 { t3 } else { (116.0 * t - 16.0) * 27.0 / 24389.0 }
        };
        let (x, y, z) = (finv(fx) * 0.95047, finv(fy), finv(fz) * 1.08883);
        let r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
        let g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
        let bb = 0.0557 * x - 0.2040 * y + 1.0570 * z;
        let gam = |c: f64| {
            let c = c.clamp(0.0, 1.0);
            if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
        };
        Rgb { r: gam(r), g: gam(g), b: gam(bb) }
    }

    /// `n` colors from `self` to `other`, interpolated in Lab (pastel gradient)
    pub fn gradient(self, other: Rgb, n: usize) -> Vec<Rgb> {
        let (l1, a1, b1) = self.to_lab();
        let (l2, a2, b2) = other.to_lab();
        (0..n)
            .map(|i| {
                let t = if n > 1 { i as f64 / (n - 1) as f64 } else { 0.0 };
                Rgb::from_lab(l1 + (l2 - l1) * t, a1 + (a2 - a1) * t, b1 + (b2 - b1) * t)
            })
            .collect()
    }
}

/// A resolved color: the text written in the configuration files and its value
#[derive(Clone, Debug)]
pub struct Color {
    pub text: String,
    pub rgb: Rgb,
}

/// Evaluates the expressions of the `[colors]` table, on demand
pub struct Palette<'a> {
    defs: &'a BTreeMap<String, String>,
    cache: BTreeMap<String, Color>,
    pub gradient: Vec<Color>,
}

impl<'a> Palette<'a> {
    pub fn new(defs: &'a BTreeMap<String, String>) -> Palette<'a> {
        Palette { defs, cache: BTreeMap::new(), gradient: Vec::new() }
    }

    /// Value of the color NAME of the table (or gradientN)
    pub fn get(&mut self, name: &str) -> Result<Color, String> {
        self.named(name, &mut Vec::new())
    }

    /// Value of an expression
    pub fn eval(&mut self, expr: &str) -> Result<Color, String> {
        self.eval_in(expr, &mut Vec::new())
    }

    fn named(&mut self, name: &str, stack: &mut Vec<String>) -> Result<Color, String> {
        if let Some(c) = self.cache.get(name) {
            return Ok(c.clone());
        }
        if let Some(n) = name.strip_prefix("gradient").and_then(|n| n.parse::<usize>().ok()) {
            return self
                .gradient
                .get(n)
                .cloned()
                .ok_or_else(|| format!("« {name} » : dégradé absent ([polybar] gradient = true)"));
        }
        let def = self
            .defs
            .get(name)
            .ok_or_else(|| format!("couleur inconnue : « {name} »"))?;
        if stack.iter().any(|s| s == name) {
            return Err(format!("définition circulaire de la couleur « {name} »"));
        }
        stack.push(name.to_string());
        let c = self.eval_in(def, stack)?;
        stack.pop();
        self.cache.insert(name.to_string(), c.clone());
        Ok(c)
    }

    fn eval_in(&mut self, expr: &str, stack: &mut Vec<String>) -> Result<Color, String> {
        let e = expr.trim();
        if e.starts_with('#') {
            let rgb = Rgb::parse(e).ok_or_else(|| format!("couleur invalide : « {e} »"))?;
            return Ok(Color { text: e.to_string(), rgb });
        }
        for (func, sign) in [("lighten", 1.0), ("darken", -1.0)] {
            if let Some(args) = e.strip_prefix(func).map(str::trim_start).and_then(|r| r.strip_prefix('(')) {
                let args = args
                    .strip_suffix(')')
                    .ok_or_else(|| format!("parenthèse manquante : « {e} »"))?;
                let (inner, amount) = args
                    .rsplit_once(',')
                    .ok_or_else(|| format!("{func}(couleur, valeur) attendu : « {e} »"))?;
                let amount: f64 = amount
                    .trim()
                    .parse()
                    .map_err(|_| format!("valeur invalide dans « {e} »"))?;
                let base = self.eval_in(inner, stack)?;
                let rgb = base.rgb.lighten(sign * amount);
                return Ok(Color { text: rgb.hex(), rgb });
            }
        }
        if e.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') && !e.is_empty() {
            return self.named(e, stack);
        }
        Err(format!("expression de couleur invalide : « {e} »"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_float() {
        // pastel format rgb-float, without "rgb(", ")" and spaces
        assert_eq!(Rgb::parse("#5294E2").unwrap().rgb_float(), "0.322,0.580,0.886");
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(Rgb::parse("#1E2128").unwrap().hex(), "#1e2128");
        assert_eq!(Rgb::parse("#FF1E2128").unwrap().hex(), "#1e2128");
    }

    #[test]
    fn lighten_darken() {
        // Values of `pastel color X | pastel lighten/darken F | pastel format hex`
        let bg = Rgb::parse("#1E2128").unwrap();
        assert_eq!(bg.lighten(0.05).hex(), "#292d37");
        let fg = Rgb::parse("#ABB2BF").unwrap();
        assert_eq!(fg.lighten(-0.30).hex(), "#5a6477");
    }

    #[test]
    fn expressions() {
        let mut d = BTreeMap::new();
        d.insert("background".into(), "#1E2128".into());
        d.insert("alt".into(), "lighten(background, 0.05)".into());
        d.insert("loop".into(), "loop".into());
        let mut p = Palette::new(&d);
        assert_eq!(p.get("background").unwrap().text, "#1E2128");
        assert_eq!(p.get("alt").unwrap().text, "#292d37");
        assert!(p.get("loop").is_err());
        assert!(p.get("missing").is_err());
    }

    #[test]
    fn gradient_ends() {
        let a = Rgb::parse("#1e2128").unwrap();
        let b = Rgb::parse("#2a2e37").unwrap();
        let g = a.gradient(b, 7);
        assert_eq!(g.len(), 7);
        assert_eq!(g[0].hex(), "#1e2128");
        assert_eq!(g[6].hex(), "#2a2e37");
        // pastel gradient -n 7 '#1e2128' '#2a2e37'
        let hex: Vec<String> = g.iter().map(|c| c.hex()).collect();
        assert_eq!(hex[1..6], ["#20232a", "#22252d", "#24272f", "#262a32", "#282c34"]);
    }
}
