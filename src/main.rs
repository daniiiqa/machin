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

mod escucha;
mod hypr;
mod lang;
mod red;
mod sunshine;

use std::time::Duration;

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

pub fn tam(s: &str) -> Option<(i64, i64)> {
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

pub fn avisa(titulo: &str, cuerpo: &str) {
    let _ = Command::new("notify-send")
        .args(["-a", "machín", "-i", "video-display", titulo, cuerpo])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

const AYUDA_ES: &str = "\
machín: la pantalla de tu otra compu como monitor extra

  machin                                    el atajo (SUPER+Shift+M): busca la otra
                                            compu, le pide su pantalla y la pone junto
                                            a la tuya; si ya está, la quita
  machin escucha                            el servicio de cada máquina (al iniciar
                                            sesión): se anuncia y atiende a la otra
  machin prende [ANCHOxALTO] [--izquierda]  a mano: crea el monitor MACHIN y lo transmite
  machin apaga                              lo quita; las ventanas regresan
  machin presta HOST                        a mano: Moonlight a pantalla completa contra HOST
  machin estado                             prendido ANCHOxALTO | apagado

Ajustes en ~/.config/machin/machin.conf (clave=valor):
  tam=1920x1200  lado=derecha|izquierda  fps=60  bitrate=35000  app=Desktop
";

const AYUDA_EN: &str = "\
machín: your other computer's screen as an extra monitor

  machin                                      the shortcut (SUPER+Shift+M): finds the other
                                              computer, asks for its screen and puts it next
                                              to yours; if it's already on, removes it
  machin escucha                              each machine's service (at login): announces
                                              itself and answers the other one
  machin prende [WIDTHxHEIGHT] [--izquierda]  by hand: creates the MACHIN monitor and streams it
  machin apaga                                removes it; windows come back
  machin presta HOST                          by hand: Moonlight fullscreen against HOST
  machin estado                               prendido WxH | apagado

Settings in ~/.config/machin/machin.conf (key=value):
  tam=1920x1200  lado=derecha|izquierda  fps=60  bitrate=35000  app=Desktop
";

enum Pedido {
    Ayuda,
    Version,
    Alterna,
    Escucha,
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
    let Some(cmd) = args.first() else { return Ok(Pedido::Alterna) };
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
        "escucha" | "listen" => sobra(resto).map(|_| Pedido::Escucha),
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

/// El atajo: si está prendido lo apaga; si no, busca la otra compu, crea
/// MACHIN del tamaño de su pantalla y le pide que se conecte.
fn alterna() -> Result<(), String> {
    if hypr::machin().is_some() {
        // al parar Sunshine, el Moonlight de allá se cierra solo
        return apaga();
    }
    let otras = red::buscar(Duration::from_millis(2500))?;
    let otra = match otras.len() {
        0 => {
            return Err(l(
                "no veo otra compu con machín en la red (¿tiene `machin escucha`?)",
                "no other computer with machín on the network (is `machin escucha` running?)",
            )
            .into());
        }
        1 => otras[0].clone(),
        _ => elegir(&otras).ok_or(l("no elegiste ninguna", "none chosen"))?,
    };
    let (w, h) = otra.tam.unwrap_or(ajustes().tam);
    println!("machín: {} {} ({w}x{h})…", l("pidiéndole la pantalla a", "asking for the screen of"), otra.nombre);
    prende(Some((w, h)), false)?;
    // la otra puede estar preguntando: hasta 45 s
    let r = red::pedir(&otra.ips, otra.puerto, &red::presta_msg(), Duration::from_secs(45));
    match r {
        Ok(v) if v["ok"].as_bool() == Some(true) => {
            let msg = if lang::es() {
                format!("{} es tu monitor de la derecha", otra.nombre)
            } else {
                format!("{} is your monitor on the right", otra.nombre)
            };
            avisa("machín", &msg);
            Ok(())
        }
        otro => {
            let _ = apaga();
            let por = match &otro {
                Ok(v) => match v["por"].as_str().unwrap_or("") {
                    "no" => l("dijo que no", "said no").to_string(),
                    "sin-pareja" => l(
                        "no está emparejada: en su Moonlight agrega esta compu y pon el PIN una vez",
                        "not paired: in its Moonlight add this computer and enter the PIN once",
                    )
                    .to_string(),
                    "ocupada" => l("ya le presta su pantalla a otra", "already lending its screen").to_string(),
                    x => x.to_string(),
                },
                Err(e) => e.clone(),
            };
            Err(format!("{}: {por}", otra.nombre))
        }
    }
}

/// Varias compus: que elijas con jale.
fn elegir(otras: &[red::Otra]) -> Option<red::Otra> {
    use std::io::Write;
    let jale = home().join(".local/bin/jale");
    let mut hijo = Command::new(jale)
        .args(["-dmenu", "-p", "machín"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    {
        let mut entrada = hijo.stdin.take()?;
        for o in otras {
            let _ = writeln!(entrada, "{}", o.nombre);
        }
    }
    let out = hijo.wait_with_output().ok()?;
    let elegida = String::from_utf8_lossy(&out.stdout).trim().to_string();
    otras.iter().find(|o| o.nombre == elegida).cloned()
}

/// Moonlight a pantalla completa contra `host`, con la resolución de este
/// monitor. Moonlight (SDL) pide que no se apague la pantalla mientras
/// transmite, y hypridle lo respeta.
pub fn moonlight(host: &str) -> Command {
    let a = ajustes();
    let res = hypr::monitors()
        .ok()
        .and_then(|m| hypr::ancla(&m))
        .map(|m| format!("{}x{}", m.px_w, m.px_h))
        .unwrap_or_else(|| format!("{}x{}", a.tam.0, a.tam.1));
    let mut c = Command::new("moonlight");
    c.args(["stream", host, &a.app, "--resolution", &res])
        .args(["--fps", &a.fps.to_string(), "--bitrate", &a.bitrate.to_string()])
        .args(["--display-mode", "fullscreen", "--audio-on-host", "--no-quit-after"])
        .stdin(Stdio::null());
    c
}

pub fn apaga() -> Result<(), String> {
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

/// A mano: esta máquina es la pantalla de `host`.
fn presta(host: &str) -> Result<(), String> {
    println!("machín: {} {host}", l("prestando la pantalla a", "lending the screen to"));
    let st = moonlight(host)
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
        Pedido::Alterna => alterna(),
        Pedido::Escucha => escucha::escuchar(),
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
