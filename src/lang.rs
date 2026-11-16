//! El idioma de discada: LANGUAGE en ~/.config/hypr/discada.conf, si no $LANG.

use std::sync::OnceLock;

static ES: OnceLock<bool> = OnceLock::new();

pub fn es() -> bool {
    *ES.get_or_init(|| {
        let conf = crate::home().join(".config/hypr/discada.conf");
        if let Ok(txt) = std::fs::read_to_string(conf) {
            for line in txt.lines() {
                if let Some(v) = line.trim().strip_prefix("LANGUAGE=") {
                    let v = v.split_whitespace().next().unwrap_or("");
                    if v == "es" || v == "en" {
                        return v == "es";
                    }
                }
            }
        }
        std::env::var("LANG").is_ok_and(|l| l.starts_with("es"))
    })
}

/// L("español", "english")
pub fn l(es_txt: &'static str, en_txt: &'static str) -> &'static str {
    if es() { es_txt } else { en_txt }
}
