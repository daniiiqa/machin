//! Hablarle a Hyprland (config en Lua desde 0.57): `hyprctl -j monitors`,
//! `hyprctl output create|remove` y las reglas con `hyprctl eval '<lua>'`.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub const SALIDA: &str = "MACHIN";

#[derive(Debug, Clone)]
pub struct Monitor {
    pub name: String,
    pub x: i64,
    pub y: i64,
    /// tamaño lógico (pixeles / scale), el que cuenta para acomodar
    pub w: i64,
    /// tamaño en pixeles
    pub px_w: i64,
    pub px_h: i64,
    pub focused: bool,
    pub workspace: String,
}

fn hyprctl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("hyprctl")
        .args(args)
        .output()
        .map_err(|e| format!("hyprctl: {e}"))?;
    let txt = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || txt.starts_with("error") || txt.contains("unexpected symbol") {
        return Err(format!("hyprctl {}: {txt}", args.join(" ")));
    }
    Ok(txt)
}

pub fn monitors() -> Result<Vec<Monitor>, String> {
    let txt = hyprctl(&["-j", "monitors"])?;
    let v: serde_json::Value = serde_json::from_str(&txt).map_err(|e| format!("monitors: {e}"))?;
    let arr = v.as_array().ok_or("monitors: no es una lista")?;
    Ok(arr
        .iter()
        .map(|m| {
            let i = |k: &str| m[k].as_i64().unwrap_or(0);
            let scale = m["scale"].as_f64().filter(|s| *s > 0.0).unwrap_or(1.0);
            Monitor {
                name: m["name"].as_str().unwrap_or("").to_string(),
                x: i("x"),
                y: i("y"),
                w: (i("width") as f64 / scale).round() as i64,
                px_w: i("width"),
                px_h: i("height"),
                focused: m["focused"].as_bool().unwrap_or(false),
                workspace: m["activeWorkspace"]["name"].as_str().unwrap_or("").to_string(),
            }
        })
        .collect())
}

pub fn machin() -> Option<Monitor> {
    monitors().ok()?.into_iter().find(|m| m.name == SALIDA)
}

/// El monitor de verdad junto al que se pone MACHIN: el enfocado (si no es
/// MACHIN), si no el primero.
pub fn ancla(mons: &[Monitor]) -> Option<Monitor> {
    let reales: Vec<&Monitor> = mons.iter().filter(|m| m.name != SALIDA).collect();
    reales
        .iter()
        .find(|m| m.focused)
        .or_else(|| reales.first())
        .map(|m| (*m).clone())
}

/// Crea MACHIN de `w`x`h` a la derecha (o izquierda) del monitor ancla y
/// espera a que Hyprland lo tenga con ese tamaño.
pub fn crear(w: i64, h: i64, izquierda: bool) -> Result<Monitor, String> {
    let mons = monitors()?;
    let a = ancla(&mons).ok_or("no encontré ningún monitor")?;
    let (x, y) = if izquierda { (a.x - w, a.y) } else { (a.x + a.w, a.y) };
    if !mons.iter().any(|m| m.name == SALIDA) {
        hyprctl(&["output", "create", "headless", SALIDA])?;
    }
    let r = Regla { w, h, x, y };
    r.aplicar()?;
    r.guardar();
    let hasta = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(m) = machin().filter(|m| m.px_w == w && m.px_h == h) {
            return Ok(m);
        }
        if Instant::now() > hasta {
            return Err(format!("Hyprland no puso {SALIDA} en {w}x{h}"));
        }
        sleep(Duration::from_millis(100));
    }
}

pub fn quitar() -> Result<(), String> {
    let _ = std::fs::remove_file(Regla::archivo());
    if machin().is_none() {
        return Ok(());
    }
    hyprctl(&["output", "remove", SALIDA]).map(|_| ())
}

/// Tamaño y lugar de MACHIN. Se aplica en caliente (`hyprctl eval`), así que
/// una recarga de la config la borra (MACHIN vuelve a 1920x1080, scale 2):
/// por eso se guarda y `vigilar` la vuelve a poner.
#[derive(Debug, Clone, Copy)]
pub struct Regla {
    w: i64,
    h: i64,
    x: i64,
    y: i64,
}

impl Regla {
    fn archivo() -> PathBuf {
        let run = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        run.join("machin/regla")
    }

    fn aplicar(&self) -> Result<(), String> {
        let Regla { w, h, x, y } = *self;
        let lua = format!(
            r#"hl.monitor({{ output = "{SALIDA}", mode = "{w}x{h}@60", position = "{x}x{y}", scale = 1.0 }})"#
        );
        hyprctl(&["eval", &lua]).map(|_| ())
    }

    fn guardar(&self) {
        let path = Self::archivo();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, format!("{} {} {} {}\n", self.w, self.h, self.x, self.y));
    }

    fn leer() -> Option<Regla> {
        let txt = std::fs::read_to_string(Self::archivo()).ok()?;
        let n: Vec<i64> = txt.split_whitespace().filter_map(|s| s.parse().ok()).collect();
        match n[..] {
            [w, h, x, y] => Some(Regla { w, h, x, y }),
            _ => None,
        }
    }
}

/// Escucha los eventos de Hyprland (.socket2.sock): en cada recarga de la
/// config le vuelve a poner a MACHIN su regla; cuando MACHIN se va, se va.
pub fn vigilar() -> Result<(), String> {
    let run = std::env::var_os("XDG_RUNTIME_DIR").ok_or("sin XDG_RUNTIME_DIR")?;
    let firma = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").ok_or("sin HYPRLAND_INSTANCE_SIGNATURE")?;
    let sock = PathBuf::from(run).join("hypr").join(firma).join(".socket2.sock");
    let s = UnixStream::connect(&sock).map_err(|e| format!("{}: {e}", sock.display()))?;
    for linea in BufReader::new(s).lines() {
        let linea = linea.map_err(|e| e.to_string())?;
        let (ev, dato) = linea.split_once(">>").unwrap_or((&linea, ""));
        match ev {
            "configreloaded" => {
                if machin().is_none() {
                    return Ok(());
                }
                if let Some(r) = Regla::leer() {
                    // la recarga termina de acomodar monitores un poco después
                    sleep(Duration::from_millis(300));
                    if let Err(e) = r.aplicar() {
                        eprintln!("machin vigila: {e}");
                    }
                }
            }
            "monitorremoved" if dato == SALIDA => return Ok(()),
            "monitorremovedv2" if dato.split(',').nth(1) == Some(SALIDA) => return Ok(()),
            _ => {}
        }
    }
    Ok(())
}
