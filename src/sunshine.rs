//! Sunshine, el que transmite. Solo ve los monitores que ya existen al
//! arrancar, así que se reinicia cada vez que MACHIN se crea. Se elige el
//! monitor por NOMBRE (`output_name = MACHIN`): con el número (1) Sunshine
//! 2026.914 lo ignora y transmite el monitor principal.

use std::fs::{self, File};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::hypr::SALIDA;

fn conf() -> PathBuf {
    crate::home().join(".config/sunshine/sunshine.conf")
}

pub fn log() -> PathBuf {
    crate::home().join(".local/state/machin/sunshine.log")
}

/// Deja en sunshine.conf `capture = wlr`, `output_name = MACHIN` y
/// `stream_audio` (si el sonido va a la otra compu o no); lo demás (encoder,
/// adapter_name, lo que pongas en su panel web) se respeta.
pub fn preparar_conf(audio: bool) -> Result<(), String> {
    let path = conf();
    let viejo = fs::read_to_string(&path).unwrap_or_default();
    let quiero = [
        ("capture", "wlr"),
        ("output_name", SALIDA),
        ("stream_audio", if audio { "enabled" } else { "disabled" }),
    ];
    let mut vistos = [false; 3];
    let mut lineas: Vec<String> = Vec::new();
    for line in viejo.lines() {
        let clave = line.split('=').next().unwrap_or("").trim();
        if let Some(i) = quiero.iter().position(|(k, _)| *k == clave) {
            if !vistos[i] {
                lineas.push(format!("{} = {}", quiero[i].0, quiero[i].1));
                vistos[i] = true;
            }
            continue;
        }
        lineas.push(line.to_string());
    }
    for (i, (k, v)) in quiero.iter().enumerate() {
        if !vistos[i] {
            lineas.push(format!("{k} = {v}"));
        }
    }
    let nuevo = lineas.join("\n") + "\n";
    if nuevo == viejo {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("conf.machin");
    fs::write(&tmp, &nuevo).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Los Sunshine vivos de verdad: un zombi (ya muerto, que su padre no ha
/// recogido) no cuenta. `pgrep` sí los cuenta, y por eso un Sunshine que
/// arrancó el menú y luego se mató «no se quería ir».
fn pids() -> Vec<u32> {
    let Ok(es) = fs::read_dir("/proc") else { return Vec::new() };
    es.flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| {
            // /proc/<pid>/stat: «pid (comm) estado …»
            let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else { return false };
            let Some((izq, der)) = stat.rsplit_once(')') else { return false };
            izq.ends_with("(sunshine") && der.trim_start().chars().next().is_some_and(|c| c != 'Z')
        })
        .collect()
}

pub fn corriendo() -> bool {
    !pids().is_empty()
}

fn senal(s: &str) {
    for pid in pids() {
        let _ = Command::new("kill").args([s, &pid.to_string()]).stderr(Stdio::null()).status();
    }
}

pub fn parar() -> Result<(), String> {
    if !corriendo() {
        return Ok(());
    }
    senal("-TERM");
    // con una transmisión abierta Sunshine tarda en cerrar; luego, a la mala
    // (soltar la GPU también toma su rato)
    let term = Instant::now() + Duration::from_secs(5);
    let kill = term + Duration::from_secs(3);
    let mut matado = false;
    while corriendo() {
        if Instant::now() > kill {
            return Err("sunshine no se quiso ir".into());
        }
        if !matado && Instant::now() > term {
            senal("-KILL");
            matado = true;
        }
        sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Arranca Sunshine (en su propia sesión, que no muera con la terminal) y
/// espera a que vea MACHIN en su lista de monitores.
pub fn arrancar() -> Result<(), String> {
    let log = log();
    if let Some(dir) = log.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let out = File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    // setsid -f: Sunshine no queda como hijo de este proceso (el menú vive un
    // rato y no lo recogería al morir: quedaba zombi); setsid sale luego luego
    Command::new("setsid")
        .args(["-f", "sunshine"])
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .status()
        .map_err(|e| format!("sunshine: {e} (¿está instalado? yay -S sunshine-bin)"))?;
    let marca = format!("is {SALIDA}:");
    let inicio = Instant::now();
    loop {
        let txt = fs::read_to_string(&log).unwrap_or_default();
        if txt.contains(&marca) {
            return Ok(());
        }
        // se cayó al arrancar (ya pasó un segundo y no hay proceso)
        if inicio.elapsed() > Duration::from_secs(1) && !corriendo() {
            let ultima = txt.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
            return Err(format!("sunshine se cayó al arrancar: {ultima}"));
        }
        if inicio.elapsed() > Duration::from_secs(15) {
            return Err(format!("sunshine no vio {SALIDA} (mira {})", log.display()));
        }
        sleep(Duration::from_millis(200));
    }
}
