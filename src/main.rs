//! machín: la pantalla de tu otra compu como monitor extra.
//!
//!   machin              el atajo (SUPER+Shift+M): prende o apaga
//!   machin menu         el menú (sonido, lado, calidad, confianza)
//!   machin escucha      el servicio de cada máquina (al iniciar sesión)
//!   machin prende [ANCHOxALTO] [--izquierda] | apaga | presta HOST | estado
//!   machin --help | --version
//!
//! Cualquier otro argumento es un error y no hace nada.

mod ajustes;
mod escucha;
mod estado;
mod hypr;
mod lang;
mod menu;
mod red;
mod sunshine;

use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

use ajustes::Sonido;
use lang::l;

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

pub fn tam(s: &str) -> Option<(i64, i64)> {
    let (w, h) = s.split_once('x')?;
    let (w, h) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
    (w >= 320 && h >= 240 && w <= 7680 && h <= 4320).then_some((w, h))
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
  machin menu                               sonido, lado, calidad y compus de confianza
  machin escucha                            el servicio de cada máquina (al iniciar
                                            sesión): se anuncia y atiende a la otra
  machin prende [ANCHOxALTO] [--izquierda]  a mano: crea el monitor MACHIN y lo transmite
  machin apaga                              lo quita (o deja de prestar la pantalla)
  machin presta HOST                        a mano: Moonlight a pantalla completa contra HOST
  machin estado                             principal OTRA | presta OTRA | prendido WxH | apagado
  machin busca                              qué compus con machín ve en la red (y la última)

Ajustes en ~/.config/machin/machin.conf (clave=valor):
  sonido=aqui|alla|ambos  lado=derecha|izquierda  bitrate=35000  fps=60
  tam=1920x1200  app=Desktop
";

const AYUDA_EN: &str = "\
machín: your other computer's screen as an extra monitor

  machin                                      the shortcut (SUPER+Shift+M): finds the other
                                              computer, asks for its screen and puts it next
                                              to yours; if it's already on, removes it
  machin menu                                 sound, side, quality and trusted computers
  machin escucha                              each machine's service (at login): announces
                                              itself and answers the other one
  machin prende [WIDTHxHEIGHT] [--izquierda]  by hand: creates the MACHIN monitor and streams it
  machin apaga                                removes it (or stops lending the screen)
  machin presta HOST                          by hand: Moonlight fullscreen against HOST
  machin estado                               principal OTHER | presta OTHER | prendido WxH | apagado
  machin busca                                which machín computers it sees (and the last one)

Settings in ~/.config/machin/machin.conf (key=value):
  sonido=aqui|alla|ambos  lado=derecha|izquierda  bitrate=35000  fps=60
  tam=1920x1200  app=Desktop
";

enum Pedido {
    Ayuda,
    Version,
    Alterna,
    Menu,
    Escucha,
    Estado,
    Busca,
    Prende { tam: Option<(i64, i64)>, izquierda: bool },
    Apaga,
    Presta(String),
    /// interno: lo lanza `prende` (re-aplica la regla de MACHIN al recargar)
    Vigila,
}

fn vigila_pid_archivo() -> PathBuf {
    estado::dir().join("vigila.pid")
}

/// El pid del vigilante, si sigue vivo y es un `machin vigila`.
fn vigila_pid() -> Option<u32> {
    let pid = std::fs::read_to_string(vigila_pid_archivo()).ok()?.trim().parse::<u32>().ok()?;
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .is_ok_and(|c| c.split(|b| *b == 0).any(|a| a == b"vigila"))
        .then_some(pid)
}

fn lanzar_vigila() {
    if vigila_pid().is_some() {
        return;
    }
    let Ok(yo) = std::env::current_exe() else { return };
    let _ = std::fs::create_dir_all(estado::dir());
    // setsid -f: que no sea hijo de este proceso (si no, al morir queda
    // zombi hasta que este termine). Él anota su pid al arrancar.
    let _ = Command::new("setsid")
        .arg("-f")
        .arg(yo)
        .arg("vigila")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let hasta = std::time::Instant::now() + Duration::from_secs(2);
    while vigila_pid().is_none() && std::time::Instant::now() < hasta {
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn argumentos(args: &[String]) -> Result<Pedido, String> {
    let Some(cmd) = args.first() else { return Ok(Pedido::Alterna) };
    let resto = &args[1..];
    let raro = |x: &str| format!("{} «{x}»", l("no entiendo", "I don't understand"));
    let sobra = |r: &[String]| match r.first() {
        Some(x) => Err(raro(x)),
        None => Ok(()),
    };
    match cmd.as_str() {
        "-h" | "--help" | "help" | "ayuda" => Ok(Pedido::Ayuda),
        "-V" | "--version" | "version" => Ok(Pedido::Version),
        "estado" | "status" => sobra(resto).map(|_| Pedido::Estado),
        "busca" | "find" => sobra(resto).map(|_| Pedido::Busca),
        "menu" | "menú" => sobra(resto).map(|_| Pedido::Menu),
        "apaga" | "off" => sobra(resto).map(|_| Pedido::Apaga),
        "vigila" => sobra(resto).map(|_| Pedido::Vigila),
        "escucha" | "listen" => sobra(resto).map(|_| Pedido::Escucha),
        "prende" | "on" => {
            let (mut t, mut izq) = (None, false);
            for a in resto {
                match a.as_str() {
                    "--izquierda" | "--left" => izq = true,
                    x if t.is_none() && tam(x).is_some() => t = tam(x),
                    x => return Err(raro(x)),
                }
            }
            Ok(Pedido::Prende { tam: t, izquierda: izq })
        }
        "presta" | "lend" => match resto {
            [host] if !host.starts_with('-') => Ok(Pedido::Presta(host.clone())),
            [] => Err(l("presta necesita el HOST (la compu principal)", "presta needs the HOST (the main computer)").into()),
            [_, x, ..] | [x] => Err(raro(x)),
        },
        x => Err(format!("{} (machin --help)", raro(x))),
    }
}

fn estado_txt() -> String {
    if let Some(a) = estado::activo("principal") {
        return format!("principal {}", a.otra);
    }
    if let Some(a) = estado::activo("presta") {
        return format!("presta {}", a.otra);
    }
    match hypr::machin() {
        Some(m) => {
            let tx = if sunshine::corriendo() { "" } else { l(" · sin sunshine", " · no sunshine") };
            format!("prendido {}x{}{tx}", m.px_w, m.px_h)
        }
        None => "apagado".into(),
    }
}

pub fn prende(t: Option<(i64, i64)>, izquierda: bool) -> Result<hypr::Monitor, String> {
    let a = ajustes::leer();
    let (w, h) = t.unwrap_or(a.tam);
    let izquierda = izquierda || a.izquierda;
    let m = hypr::crear(w, h, izquierda)?;
    sunshine::preparar_conf(a.sonido != Sonido::Aqui)?;
    sunshine::parar()?;
    if let Err(e) = sunshine::arrancar() {
        // sin transmisión el monitor no sirve: no dejarlo colgado
        let _ = hypr::quitar();
        return Err(e);
    }
    lanzar_vigila();
    Ok(m)
}

/// Pedirle a `otra` (el JSON de estado::otra) que se conecte, con el
/// sonido y la calidad de aquí.
fn pedir_presta(otra: &serde_json::Value) -> Result<(), String> {
    let nombre = otra["nombre"].as_str().unwrap_or("?");
    let ips: Vec<std::net::IpAddr> = otra["ips"]
        .as_array()
        .map(|v| v.iter().filter_map(|i| i.as_str()?.parse().ok()).collect())
        .unwrap_or_default();
    let puerto = otra["puerto"].as_u64().unwrap_or(red::PUERTO as u64) as u16;
    let a = ajustes::leer();
    // la otra puede estar preguntando: hasta 45 s
    let r = red::pedir(&ips, puerto, &red::presta_msg(&a), Duration::from_secs(45));
    match r {
        Ok(v) if v["ok"].as_bool() == Some(true) => Ok(()),
        otro => {
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
            Err(format!("{nombre}: {por}"))
        }
    }
}

fn marcar_principal(nombre: &str, w: i64, h: i64) {
    if let Some(pid) = vigila_pid() {
        estado::poner(pid, "principal", nombre, &format!("{w}x{h}"));
    }
}

/// El atajo: si está prendido lo apaga (también si esta presta su pantalla);
/// si no, busca la otra compu, crea MACHIN del tamaño de su pantalla y le
/// pide que se conecte.
pub fn alterna() -> Result<(), String> {
    if hypr::machin().is_some() || estado::activo("presta").is_some() {
        return apaga();
    }
    // el módem pierde seguido el multicast entre cable y Wi-Fi: mientras se
    // busca por mDNS, se prueba directo con la última compu
    let directo = std::thread::spawn(|| red::ultima().as_ref().and_then(red::directo));
    let mut otras = red::buscar(Duration::from_millis(1500))?;
    if otras.is_empty() {
        match directo.join().ok().flatten() {
            Some(o) => otras.push(o),
            None => otras = red::buscar(Duration::from_millis(2500))?,
        }
    }
    let otra = match otras.len() {
        0 => {
            return Err(l(
                "no veo otra compu con machín en la red (¿tiene `machin escucha`?)",
                "no other computer with machín on the network (is `machin escucha` running?)",
            )
            .into());
        }
        1 => otras[0].clone(),
        _ => menu::elegir_otra(&otras).ok_or(l("no elegiste ninguna", "none chosen"))?,
    };
    let (w, h) = otra.tam.unwrap_or(ajustes::leer().tam);
    println!("machín: {} {} ({w}x{h})…", l("pidiéndole la pantalla a", "asking for the screen of"), otra.nombre);
    let info = serde_json::json!({
        "nombre": otra.nombre,
        "ips": otra.ips.iter().map(|i| i.to_string()).collect::<Vec<_>>(),
        "puerto": otra.puerto,
        "tam": format!("{w}x{h}"),
    });
    estado::guardar_otra(&info);
    // si allá seguía abierto un Moonlight de antes, al reemplazarlo manda
    // «suelta» (hasta la 0.3.1): que no apague lo que se está prendiendo
    estado::marcar_reconexion();
    let r = prende(Some((w, h)), false).and_then(|_| pedir_presta(&info));
    estado::desmarcar_reconexion();
    if let Err(e) = r {
        let _ = apaga();
        return Err(e);
    }
    marcar_principal(&otra.nombre, w, h);
    red::guardar_ultima(&red::Otra { tam: Some((w, h)), ..otra.clone() });
    let lado = if ajustes::leer().izquierda { l("izquierda", "left") } else { l("derecha", "right") };
    let msg = if lang::es() {
        format!("{} es tu monitor de la {lado}", otra.nombre)
    } else {
        format!("{} is your monitor on the {lado}", otra.nombre)
    };
    avisa("machín", &msg);
    Ok(())
}

/// Para revisar la red: lo que contesta por mDNS y si la última contesta directo.
fn busca() -> Result<(), String> {
    let t0 = std::time::Instant::now();
    let otras = red::buscar(Duration::from_secs(4))?;
    let ms = t0.elapsed().as_millis();
    let fmt = |o: &red::Otra| {
        let ips: Vec<String> = o.ips.iter().map(|i| i.to_string()).collect();
        let tam = o.tam.map(|(w, h)| format!("{w}x{h}")).unwrap_or("?".into());
        format!("{} {tam} {}:{}", o.nombre, ips.join(","), o.puerto)
    };
    if otras.is_empty() {
        println!("mDNS ({ms} ms): {}", l("nadie", "nobody"));
    }
    for o in &otras {
        println!("mDNS ({ms} ms): {}", fmt(o));
    }
    match red::ultima() {
        None => println!("{}: {}", l("última", "last"), l("ninguna guardada", "none saved")),
        Some(u) => match red::directo(&u) {
            Some(o) => println!("{}: {} ({})", l("última", "last"), fmt(&o), l("contesta", "answers")),
            None => println!("{}: {} ({})", l("última", "last"), fmt(&u), l("no contesta", "no answer")),
        },
    }
    Ok(())
}

/// Cambió un ajuste con machín prendido: Sunshine y la otra se reconectan
/// con lo nuevo sin apagar nada (el «suelta» del Moonlight viejo no cuenta).
pub fn reconecta() -> Result<(), String> {
    let Some(otra) = estado::otra() else { return Ok(()) };
    if estado::activo("principal").is_none() {
        return Ok(());
    }
    let nombre = otra["nombre"].as_str().unwrap_or("?").to_string();
    let t = otra["tam"].as_str().and_then(tam);
    estado::marcar_reconexion();
    let r = prende(t, false).and_then(|m| pedir_presta(&otra).map(|_| m));
    estado::desmarcar_reconexion();
    match r {
        Ok(m) => {
            marcar_principal(&nombre, m.px_w, m.px_h);
            Ok(())
        }
        Err(e) => {
            let _ = apaga();
            Err(e)
        }
    }
}

/// Esta máquina es la pantalla de `host`: Moonlight a pantalla completa con
/// la resolución de este monitor. Moonlight (SDL) pide que no se apague la
/// pantalla mientras transmite, y hypridle lo respeta. `audio_en_host`: que
/// la principal siga sonando.
pub fn moonlight(host: &str, audio_en_host: bool, bitrate: u32, fps: u32) -> Command {
    let a = ajustes::leer();
    let res = hypr::monitors()
        .ok()
        .and_then(|m| hypr::ancla(&m))
        .map(|m| format!("{}x{}", m.px_w, m.px_h))
        .unwrap_or_else(|| format!("{}x{}", a.tam.0, a.tam.1));
    let mut c = Command::new("moonlight");
    c.args(["stream", host, &a.app, "--resolution", &res])
        .args(["--fps", &fps.to_string(), "--bitrate", &bitrate.to_string()])
        .args(["--display-mode", "fullscreen", "--no-quit-after"])
        .arg(if audio_en_host { "--audio-on-host" } else { "--no-audio-on-host" })
        .stdin(Stdio::null());
    c
}

pub fn apaga() -> Result<(), String> {
    // esta presta su pantalla: cerrar Moonlight (escucha le avisa a la otra)
    if let Some(a) = estado::activo("presta") {
        let _ = Command::new("kill").arg(a.pid.to_string()).status();
        estado::quitar("presta");
        println!("machín: {} {}", l("ya no le prestas la pantalla a", "no longer lending the screen to"), a.otra);
        return Ok(());
    }
    let habia = hypr::machin().is_some();
    // aunque Sunshine tarde en irse, el monitor se quita: que las ventanas
    // regresen pase lo que pase
    let parado = sunshine::parar();
    hypr::quitar()?;
    let principal = estado::activo("principal").is_some();
    estado::quitar("principal");
    // que allá cierren Moonlight (con --no-quit-after se quedaba abierto); ya
    // sin MACHIN, el «suelta» que regresa no hace nada
    if habia && principal && !estado::reconectando() {
        if let Some(otra) = estado::otra() {
            let ips: Vec<std::net::IpAddr> = otra["ips"]
                .as_array()
                .map(|v| v.iter().filter_map(|i| i.as_str()?.parse().ok()).collect())
                .unwrap_or_default();
            let puerto = otra["puerto"].as_u64().unwrap_or(red::PUERTO as u64) as u16;
            let _ = red::pedir(&ips, puerto, &red::suelta_msg(), Duration::from_secs(2));
        }
    }
    parado?;
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
    let a = ajustes::leer();
    println!("machín: {} {host}", l("prestando la pantalla a", "lending the screen to"));
    let st = moonlight(host, a.sonido != Sonido::Alla, a.bitrate, a.fps)
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
        Pedido::Estado => {
            println!("{}", estado_txt());
            return ExitCode::SUCCESS;
        }
        Pedido::Busca => busca(),
        Pedido::Alterna => alterna(),
        Pedido::Menu => menu::menu(),
        Pedido::Escucha => escucha::escuchar(),
        Pedido::Prende { tam, izquierda } => prende(tam, izquierda).map(|m| {
            let lado = if izquierda || ajustes::leer().izquierda {
                l("a la izquierda", "on the left")
            } else {
                l("a la derecha", "on the right")
            };
            let msg = format!("{}x{} {lado} · workspace {}", m.px_w, m.px_h, m.workspace);
            println!("machín: {msg}");
            avisa(l("machín prendido", "machín on"), &msg);
        }),
        Pedido::Apaga => apaga(),
        Pedido::Presta(h) => presta(&h),
        Pedido::Vigila => {
            let _ = std::fs::create_dir_all(estado::dir());
            let _ = std::fs::write(vigila_pid_archivo(), std::process::id().to_string());
            let r = hypr::vigilar();
            let _ = std::fs::remove_file(vigila_pid_archivo());
            estado::quitar("principal");
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Un HOME de mentira: los tests no tocan tu ~/.config.
    fn home_falso(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("machin-test-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        // SAFETY: un solo test por proceso de cargo toca HOME (ver `todo`)
        unsafe { std::env::set_var("HOME", &d) };
        d
    }

    #[test]
    fn todo() {
        let h = home_falso("todo");
        // ajustes: de entrada el sonido va solo en la principal
        assert_eq!(ajustes::leer().sonido, Sonido::Aqui);
        ajustes::poner("sonido", "alla").unwrap();
        ajustes::poner("bitrate", "60000").unwrap();
        ajustes::poner("sonido", "ambos").unwrap();
        let a = ajustes::leer();
        assert_eq!(a.sonido, Sonido::Ambos);
        assert_eq!(a.bitrate, 60000);
        let txt = std::fs::read_to_string(h.join(".config/machin/machin.conf")).unwrap();
        assert_eq!(txt.matches("sonido=").count(), 1, "{txt}");

        // sunshine.conf: lo tuyo se respeta, lo de machín se pone una vez
        let sc = h.join(".config/sunshine/sunshine.conf");
        std::fs::create_dir_all(sc.parent().unwrap()).unwrap();
        std::fs::write(&sc, "encoder = vaapi\noutput_name = 1\nstream_audio = enabled\n").unwrap();
        sunshine::preparar_conf(false).unwrap();
        let t = std::fs::read_to_string(&sc).unwrap();
        assert!(t.contains("encoder = vaapi"));
        assert!(t.contains("output_name = MACHIN") && !t.contains("output_name = 1"));
        assert!(t.contains("stream_audio = disabled"));
        assert!(t.contains("capture = wlr"));
        sunshine::preparar_conf(true).unwrap();
        let t = std::fs::read_to_string(&sc).unwrap();
        assert_eq!(t.matches("stream_audio").count(), 1);
        assert!(t.contains("stream_audio = enabled"));

        // argumentos raros: error, nada
        assert!(argumentos(&["menu".into(), "x".into()]).is_err());
        assert!(argumentos(&["--rarito".into()]).is_err());
        let _ = std::fs::remove_dir_all(&h);
    }
}
