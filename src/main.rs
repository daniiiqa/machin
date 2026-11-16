//! machín: la pantalla de tu otra compu como monitor extra.
//!
//!   machin prende [ANCHOxALTO] [--izquierda]  esta es la principal: crea el
//!                                             monitor MACHIN y lo transmite
//!   machin apaga                              lo quita; las ventanas regresan
//!   machin presta HOST                        esta es la pantalla: Moonlight
//!                                             a pantalla completa contra HOST
//!   machin estado                             prendido ANCHOxALTO | apagado
//!   machin --help | --version
//!
//! Cualquier otro argumento es un error y no hace nada.
//! Ajustes (opcionales) en ~/.config/machin/machin.conf, clave=valor:
//!   tam=1920x1200  lado=derecha|izquierda  fps=60  bitrate=35000  app=Desktop

mod hypr;
mod lang;
mod sunshine;

use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

use lang::l;

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

struct Ajustes {
    tam: (i64, i64),
    izquierda: bool,
    fps: u32,
    bitrate: u32,
    app: String,
}

fn tam(s: &str) -> Option<(i64, i64)> {
    let (w, h) = s.split_once('x')?;
    let (w, h) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
    (w >= 320 && h >= 240 && w <= 7680 && h <= 4320).then_some((w, h))
}

fn ajustes() -> Ajustes {
    let mut a = Ajustes { tam: (1920, 1200), izquierda: false, fps: 60, bitrate: 35000, app: "Desktop".into() };
    let path = home().join(".config/machin/machin.conf");
    let Ok(txt) = std::fs::read_to_string(&path) else { return a };
    for line in txt.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim();
        match k.trim() {
            "tam" => match tam(v) {
                Some(t) => a.tam = t,
                None => eprintln!("machin: machin.conf: tam={v}?"),
            },
            "lado" => a.izquierda = v == "izquierda" || v == "left",
            "fps" => a.fps = v.parse().unwrap_or(a.fps),
            "bitrate" => a.bitrate = v.parse().unwrap_or(a.bitrate),
            "app" if !v.is_empty() => a.app = v.to_string(),
            _ => {}
        }
    }
    a
}

fn avisa(titulo: &str, cuerpo: &str) {
    let _ = Command::new("notify-send")
        .args(["-a", "machín", "-i", "video-display", titulo, cuerpo])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

const AYUDA_ES: &str = "\
machín: la pantalla de tu otra compu como monitor extra

  machin prende [ANCHOxALTO] [--izquierda]  esta es la principal: crea el monitor
                                            MACHIN junto al tuyo y lo transmite
  machin apaga                              lo quita; las ventanas regresan
  machin presta HOST                        esta es la pantalla: Moonlight a
                                            pantalla completa contra HOST
  machin estado                             prendido ANCHOxALTO | apagado

Ajustes en ~/.config/machin/machin.conf (clave=valor):
  tam=1920x1200  lado=derecha|izquierda  fps=60  bitrate=35000  app=Desktop
";

const AYUDA_EN: &str = "\
machín: your other computer's screen as an extra monitor

  machin prende [WIDTHxHEIGHT] [--izquierda]  this is the main one: creates the
                                              MACHIN monitor next to yours and streams it
  machin apaga                                removes it; windows come back
  machin presta HOST                          this is the screen: Moonlight
                                              fullscreen against HOST
  machin estado                               prendido WxH | apagado

Settings in ~/.config/machin/machin.conf (key=value):
  tam=1920x1200  lado=derecha|izquierda  fps=60  bitrate=35000  app=Desktop
";

enum Pedido {
    Ayuda,
    Version,
    Estado,
    Prende { tam: Option<(i64, i64)>, izquierda: bool },
    Apaga,
    Presta(String),
    /// interno: lo lanza `prende` (re-aplica la regla de MACHIN al recargar)
    Vigila,
}

fn vigila_pid() -> PathBuf {
    let run = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    run.join("machin/vigila.pid")
}

/// El vigilante ya corre si su pid sigue vivo y es un `machin vigila`.
fn vigila_vivo() -> bool {
    let Ok(pid) = std::fs::read_to_string(vigila_pid()) else { return false };
    std::fs::read(format!("/proc/{}/cmdline", pid.trim()))
        .is_ok_and(|c| c.split(|b| *b == 0).any(|a| a == b"vigila"))
}

fn lanzar_vigila() {
    if vigila_vivo() {
        return;
    }
    let Ok(yo) = std::env::current_exe() else { return };
    if let Ok(hijo) = Command::new("setsid")
        .arg(yo)
        .arg("vigila")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        // setsid sin -f hace exec: el pid es el del vigilante
        let _ = std::fs::write(vigila_pid(), hijo.id().to_string());
    }
}

fn argumentos(args: &[String]) -> Result<Pedido, String> {
    let Some(cmd) = args.first() else { return Ok(Pedido::Estado) };
    let resto = &args[1..];
    let sobra = |r: &[String]| match r.first() {
        Some(x) => Err(format!("{} «{x}»", l("no entiendo", "I don't understand"))),
        None => Ok(()),
    };
    match cmd.as_str() {
        "-h" | "--help" | "help" | "ayuda" => Ok(Pedido::Ayuda),
        "-V" | "--version" | "version" => Ok(Pedido::Version),
        "estado" | "status" => sobra(resto).map(|_| Pedido::Estado),
        "apaga" | "off" => sobra(resto).map(|_| Pedido::Apaga),
        "vigila" => sobra(resto).map(|_| Pedido::Vigila),
        "prende" | "on" => {
            let (mut t, mut izq) = (None, false);
            for a in resto {
                match a.as_str() {
                    "--izquierda" | "--left" => izq = true,
                    x if t.is_none() && tam(x).is_some() => t = tam(x),
                    x => return Err(format!("{} «{x}»", l("no entiendo", "I don't understand"))),
                }
            }
            Ok(Pedido::Prende { tam: t, izquierda: izq })
        }
        "presta" | "lend" => match resto {
            [host] if !host.starts_with('-') => Ok(Pedido::Presta(host.clone())),
            [] => Err(l("presta necesita el HOST (la compu principal)", "presta needs the HOST (the main computer)").into()),
            [_, x, ..] | [x] => Err(format!("{} «{x}»", l("no entiendo", "I don't understand"))),
        },
        x => Err(format!("{} «{x}» (machin --help)", l("no entiendo", "I don't understand"))),
    }
}

fn estado() -> ExitCode {
    match hypr::machin() {
        Some(m) => {
            let tx = if sunshine::corriendo() { "" } else { l(" · sin sunshine", " · no sunshine") };
            println!("prendido {}x{}{tx}", m.px_w, m.px_h);
        }
        None => println!("apagado"),
    }
    ExitCode::SUCCESS
}

fn prende(t: Option<(i64, i64)>, izquierda: bool) -> Result<(), String> {
    let a = ajustes();
    let (w, h) = t.unwrap_or(a.tam);
    let izquierda = izquierda || a.izquierda;
    let m = hypr::crear(w, h, izquierda)?;
    sunshine::preparar_conf()?;
    sunshine::parar()?;
    if let Err(e) = sunshine::arrancar() {
        // sin transmisión el monitor no sirve: no dejarlo colgado
        let _ = hypr::quitar();
        return Err(e);
    }
    lanzar_vigila();
    let lado = if izquierda { l("a la izquierda", "on the left") } else { l("a la derecha", "on the right") };
    let msg = format!("{}x{} {lado} · workspace {}", m.px_w, m.px_h, m.workspace);
    println!("machín: {msg}");
    avisa(l("machín prendido", "machín on"), &msg);
    Ok(())
}

fn apaga() -> Result<(), String> {
    let habia = hypr::machin().is_some();
    sunshine::parar()?;
    hypr::quitar()?;
    if habia {
        println!("machín: {}", l("apagado; las ventanas regresaron", "off; windows are back"));
        avisa(l("machín apagado", "machín off"), l("las ventanas regresaron", "windows are back"));
    } else {
        println!("machín: {}", l("ya estaba apagado", "already off"));
    }
    Ok(())
}

/// Esta máquina es la pantalla: Moonlight a pantalla completa con la
/// resolución de este monitor. Moonlight (SDL) pide que no se apague la
/// pantalla mientras transmite, y hypridle lo respeta.
fn presta(host: &str) -> Result<(), String> {
    let a = ajustes();
    let mons = hypr::monitors()?;
    let m = hypr::ancla(&mons).ok_or("no encontré ningún monitor")?;
    let res = format!("{}x{}", m.px_w, m.px_h);
    println!("machín: {} {host} ({res})", l("prestando la pantalla a", "lending the screen to"));
    let st = Command::new("moonlight")
        .args(["stream", host, &a.app, "--resolution", &res])
        .args(["--fps", &a.fps.to_string(), "--bitrate", &a.bitrate.to_string()])
        .args(["--display-mode", "fullscreen", "--audio-on-host", "--no-quit-after"])
        .status()
        .map_err(|e| format!("moonlight: {e} (¿está instalado? pacman -S moonlight-qt)"))?;
    if st.success() { Ok(()) } else { Err(format!("moonlight {st}")) }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pedido = match argumentos(&args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("machin: {e}");
            return ExitCode::from(2);
        }
    };
    let r = match pedido {
        Pedido::Ayuda => {
            print!("{}", if lang::es() { AYUDA_ES } else { AYUDA_EN });
            return ExitCode::SUCCESS;
        }
        Pedido::Version => {
            println!("machin {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Pedido::Estado => return estado(),
        Pedido::Prende { tam, izquierda } => prende(tam, izquierda),
        Pedido::Apaga => apaga(),
        Pedido::Presta(h) => presta(&h),
        Pedido::Vigila => {
            let r = hypr::vigilar();
            let _ = std::fs::remove_file(vigila_pid());
            if let Err(e) = r {
                eprintln!("machin vigila: {e}");
                return ExitCode::FAILURE;
            }
            return ExitCode::SUCCESS;
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("machin: {e}");
            avisa("machín", &e);
            ExitCode::FAILURE
        }
    }
}
