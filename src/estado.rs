//! Lo que está pasando, en $XDG_RUNTIME_DIR/machin, para empalme y el menú.
//! Como picha: un archivo `<pid>-<rol>` con un renglón `<rol> <otra> <tam>`;
//! si el proceso ya no vive, no cuenta (uno que se cayó no deja el ícono).
//!   principal  esta usa la pantalla de <otra> (pid: el vigilante)
//!   presta     esta le presta su pantalla a <otra> (pid: Moonlight)
//! Además `otra` (a quién se conectó la principal, para reconectar) y
//! `reconectando` (mientras se reconecta, el «suelta» de la otra no apaga).

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub fn dir() -> PathBuf {
    let run = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    run.join("machin")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Activo {
    pub pid: i32,
    pub rol: String,
    pub otra: String,
}

fn vivo(pid: i32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

pub fn poner(pid: u32, rol: &str, otra: &str, tam: &str) {
    let d = dir();
    let _ = std::fs::create_dir_all(&d);
    quitar(rol);
    let _ = std::fs::write(d.join(format!("{pid}-{rol}")), format!("{rol} {otra} {tam}\n"));
}

pub fn quitar(rol: &str) {
    let Ok(es) = std::fs::read_dir(dir()) else { return };
    for e in es.flatten() {
        if e.file_name().to_string_lossy().ends_with(&format!("-{rol}")) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

pub fn quitar_pid(pid: u32, rol: &str) {
    let _ = std::fs::remove_file(dir().join(format!("{pid}-{rol}")));
}

pub fn activo(rol: &str) -> Option<Activo> {
    let es = std::fs::read_dir(dir()).ok()?;
    for e in es.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        let Some((pid, r)) = n.split_once('-') else { continue };
        let Ok(pid) = pid.parse::<i32>() else { continue };
        if r != rol {
            continue;
        }
        if !vivo(pid) {
            let _ = std::fs::remove_file(e.path());
            continue;
        }
        let txt = std::fs::read_to_string(e.path()).unwrap_or_default();
        let otra = txt.split_whitespace().nth(1).unwrap_or("").to_string();
        return Some(Activo { pid, rol: rol.to_string(), otra });
    }
    None
}

/// A quién se conectó la principal (nombre, IPs, puerto, tam), en JSON.
pub fn guardar_otra(v: &serde_json::Value) {
    let d = dir();
    let _ = std::fs::create_dir_all(&d);
    let _ = std::fs::write(d.join("otra"), v.to_string());
}

pub fn otra() -> Option<serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(dir().join("otra")).ok()?).ok()
}

pub fn marcar_reconexion() {
    let d = dir();
    let _ = std::fs::create_dir_all(&d);
    let _ = std::fs::write(d.join("reconectando"), "");
}

pub fn desmarcar_reconexion() {
    let _ = std::fs::remove_file(dir().join("reconectando"));
}

/// ¿Se está reconectando? (la marca dura 30 s como mucho, por si algo falla)
pub fn reconectando() -> bool {
    std::fs::metadata(dir().join("reconectando"))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|d| d < Duration::from_secs(30))
}
