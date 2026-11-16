//! ~/.config/machin/machin.conf (clave=valor). Los lee la principal y le
//! manda a la otra lo que le toca (sonido, calidad), así manda una sola.

use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sonido {
    /// solo en la principal: Sunshine no manda el audio
    Aqui,
    /// solo en la que presta la pantalla: la principal se silencia
    Alla,
    /// en las dos
    Ambos,
}

impl Sonido {
    pub fn de(s: &str) -> Option<Sonido> {
        match s {
            "aqui" | "aquí" | "here" => Some(Sonido::Aqui),
            "alla" | "allá" | "there" => Some(Sonido::Alla),
            "ambos" | "both" => Some(Sonido::Ambos),
            _ => None,
        }
    }

    pub fn clave(self) -> &'static str {
        match self {
            Sonido::Aqui => "aqui",
            Sonido::Alla => "alla",
            Sonido::Ambos => "ambos",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Ajustes {
    pub tam: (i64, i64),
    pub izquierda: bool,
    pub fps: u32,
    pub bitrate: u32,
    pub app: String,
    pub sonido: Sonido,
}

fn archivo() -> PathBuf {
    crate::home().join(".config/machin/machin.conf")
}

pub fn leer() -> Ajustes {
    let mut a = Ajustes {
        tam: (1920, 1200),
        izquierda: false,
        fps: 60,
        bitrate: 35000,
        app: "Desktop".into(),
        sonido: Sonido::Aqui,
    };
    let Ok(txt) = std::fs::read_to_string(archivo()) else { return a };
    for line in txt.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim();
        match k.trim() {
            "tam" => match crate::tam(v) {
                Some(t) => a.tam = t,
                None => eprintln!("machin: machin.conf: tam={v}?"),
            },
            "lado" => a.izquierda = v == "izquierda" || v == "left",
            "fps" => a.fps = v.parse().unwrap_or(a.fps),
            "bitrate" => a.bitrate = v.parse().unwrap_or(a.bitrate),
            "app" if !v.is_empty() => a.app = v.to_string(),
            "sonido" => a.sonido = Sonido::de(v).unwrap_or(a.sonido),
            _ => {}
        }
    }
    a
}

/// Cambia (o agrega) `clave=valor`; lo demás del archivo se queda igual.
pub fn poner(clave: &str, valor: &str) -> Result<(), String> {
    let path = archivo();
    let viejo = std::fs::read_to_string(&path).unwrap_or_default();
    let mut hecho = false;
    let mut lineas: Vec<String> = viejo
        .lines()
        .map(|l| {
            if l.split('=').next().map(str::trim) == Some(clave) && !l.trim_start().starts_with('#') {
                hecho = true;
                format!("{clave}={valor}")
            } else {
                l.to_string()
            }
        })
        .collect();
    if !hecho {
        lineas.push(format!("{clave}={valor}"));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&path, lineas.join("\n") + "\n").map_err(|e| format!("{}: {e}", path.display()))
}
