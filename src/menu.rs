//! `machin menu`: prender / apagar y los ajustes (sonido, lado, calidad,
//! compus de confianza), con jale y el tema del menú de discada. Si machín
//! está prendido, un cambio reconecta solo.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::ajustes::{self, Sonido};
use crate::lang::{es, l};
use crate::{estado, red};

/// jale -dmenu: el índice de lo que elegiste (None: Esc).
fn pick(prompt: &str, mesg: &str, opciones: &[String], fila: usize) -> Option<usize> {
    let jale = crate::home().join(".local/bin/jale");
    let tema = crate::home().join(".config/jale/config-edit.rasi");
    let mut c = Command::new(jale);
    c.args(["-dmenu", "-i", "-format", "i", "-no-custom", "-p", prompt])
        .args(["-selected-row", &fila.to_string()]);
    if tema.exists() {
        c.arg("-config").arg(tema);
    }
    if !mesg.is_empty() {
        c.args(["-mesg", mesg]);
    }
    let mut hijo = c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    {
        let mut entrada = hijo.stdin.take()?;
        for o in opciones {
            let _ = writeln!(entrada, "{o}");
        }
    }
    let out = hijo.wait_with_output().ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// Varias compus con machín: cuál.
pub fn elegir_otra(otras: &[red::Otra]) -> Option<red::Otra> {
    let nombres: Vec<String> = otras.iter().map(|o| o.nombre.clone()).collect();
    let i = pick("machín", l("¿de quién usas la pantalla?", "whose screen?"), &nombres, 0)?;
    otras.get(i).cloned()
}

fn sonido_txt(s: Sonido) -> &'static str {
    match s {
        Sonido::Aqui => l("en esta compu", "on this computer"),
        Sonido::Alla => l("en la otra", "on the other one"),
        Sonido::Ambos => l("en las dos", "on both"),
    }
}

const CALIDADES: [(u32, &str, &str); 3] =
    [(15000, "ahorro · 15 Mbps", "saver · 15 Mbps"), (35000, "normal · 35 Mbps", "normal · 35 Mbps"), (60000, "alta · 60 Mbps", "high · 60 Mbps")];

fn calidad_txt(b: u32) -> String {
    CALIDADES
        .iter()
        .find(|(x, _, _)| *x == b)
        .map(|(_, e, n)| l(e, n).to_string())
        .unwrap_or_else(|| format!("{} Mbps", b / 1000))
}

fn confianza() -> Vec<String> {
    std::fs::read_to_string(crate::home().join(".config/machin/confianza"))
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn olvidar(nombre: &str) {
    let resto: Vec<String> = confianza().into_iter().filter(|n| n != nombre).collect();
    let txt = if resto.is_empty() { String::new() } else { resto.join("\n") + "\n" };
    let _ = std::fs::write(crate::home().join(".config/machin/confianza"), txt);
}

fn marca(si: bool) -> &'static str {
    if si { "  ✓" } else { "" }
}

/// Un ajuste cambió: si machín está prendido como principal, reconectar.
fn aplicar() {
    if estado::activo("principal").is_some() {
        crate::avisa("machín", l("reconectando con lo nuevo…", "reconnecting with the new settings…"));
        if let Err(e) = crate::reconecta() {
            crate::avisa("machín", &e);
        }
    }
}

pub fn menu() -> Result<(), String> {
    // otro menú de jale abierto: cerrarlo, como el menú de discada
    let _ = Command::new("pkill").args(["-x", "jale"]).status();
    let mut fila = 0;
    loop {
        let a = ajustes::leer();
        let principal = estado::activo("principal");
        let presta = estado::activo("presta");
        let primera = match (&principal, &presta) {
            (Some(p), _) => {
                if es() { format!("apagar · usando la pantalla de {}", p.otra) } else { format!("turn off · using {}'s screen", p.otra) }
            }
            (None, Some(p)) => {
                if es() { format!("dejar de prestarle la pantalla a {}", p.otra) } else { format!("stop lending the screen to {}", p.otra) }
            }
            _ => l("prender: la otra compu de monitor (SUPER+Shift+M)", "turn on: the other computer as a monitor (SUPER+Shift+M)").to_string(),
        };
        let lado = if a.izquierda { l("izquierda", "left") } else { l("derecha", "right") };
        let opciones = vec![
            primera,
            format!("{}: {} ›", l("sonido", "sound"), sonido_txt(a.sonido)),
            format!("{}: {lado}", l("lado", "side")),
            format!("{}: {} ›", l("calidad", "quality"), calidad_txt(a.bitrate)),
            format!("{}: {} ›", l("compus de confianza", "trusted computers"), confianza().len()),
        ];
        let mesg = if presta.is_some() {
            l("el sonido y la calidad los decide la principal", "the main computer decides sound and quality")
        } else {
            l("la pantalla de tu otra compu como monitor extra", "your other computer's screen as an extra monitor")
        };
        let Some(i) = pick("machín", mesg, &opciones, fila) else { return Ok(()) };
        fila = i;
        match i {
            0 => {
                // prender/apagar es lo último: el menú se cierra
                return if principal.is_some() || presta.is_some() { crate::apaga() } else { crate::alterna() };
            }
            1 => {
                let vals = [Sonido::Aqui, Sonido::Alla, Sonido::Ambos];
                let ops: Vec<String> = vals.iter().map(|s| format!("{}{}", sonido_txt(*s), marca(*s == a.sonido))).collect();
                let actual = vals.iter().position(|s| *s == a.sonido).unwrap_or(0);
                let mesg = l(
                    "«esta compu» es la principal (donde corren tus apps)",
                    "«this computer» is the main one (where your apps run)",
                );
                if let Some(j) = pick(l("sonido", "sound"), mesg, &ops, actual) {
                    if vals[j] != a.sonido {
                        ajustes::poner("sonido", vals[j].clave())?;
                        aplicar();
                    }
                }
            }
            2 => {
                ajustes::poner("lado", if a.izquierda { "derecha" } else { "izquierda" })?;
                aplicar();
            }
            3 => {
                let ops: Vec<String> = CALIDADES.iter().map(|(b, e, n)| format!("{}{}", l(e, n), marca(*b == a.bitrate))).collect();
                let actual = CALIDADES.iter().position(|(b, _, _)| *b == a.bitrate).unwrap_or(1);
                let mesg = l("más alta se ve más nítido y pide más Wi-Fi", "higher looks sharper and needs more Wi-Fi");
                if let Some(j) = pick(l("calidad", "quality"), mesg, &ops, actual) {
                    if CALIDADES[j].0 != a.bitrate {
                        ajustes::poner("bitrate", &CALIDADES[j].0.to_string())?;
                        aplicar();
                    }
                }
            }
            4 => {
                let nombres = confianza();
                if nombres.is_empty() {
                    let _ = pick(
                        l("confianza", "trust"),
                        l("ninguna: la primera vez que te pidan la pantalla, dale «Siempre»", "none yet: the first time one asks for your screen, pick «Always»"),
                        &[l("‹ volver", "‹ back").to_string()],
                        0,
                    );
                    continue;
                }
                let mut ops: Vec<String> = nombres.iter().map(|n| format!("{} {n}", l("olvidar", "forget"))).collect();
                ops.push(l("‹ volver", "‹ back").to_string());
                let mesg = l("entran sin preguntar; olvidada, vuelve a preguntar", "they get in without asking; forgotten, it asks again");
                if let Some(j) = pick(l("confianza", "trust"), mesg, &ops, 0) {
                    if let Some(n) = nombres.get(j) {
                        olvidar(n);
                    }
                }
            }
            _ => return Ok(()),
        }
    }
}
