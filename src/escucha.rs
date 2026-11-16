//! `machin escucha`: el servicio de cada máquina (arranca al iniciar sesión).
//! Se anuncia con su nombre y su pantalla, y atiende a la otra:
//!   presta  la otra quiere usar esta pantalla. Solo si está emparejada en
//!           Moonlight (y Moonlight fija su certificado: otra compu que diga
//!           llamarse igual no pasa). De confianza entra directo; si no, se
//!           pregunta (Aceptar / Siempre / No), como picha.
//!   suelta  la otra cerró Moonlight: aquí se apaga machín.

use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::ajustes::Sonido;
use crate::lang::{es, l};
use crate::{estado, hypr, red};

/// a quién le estoy prestando la pantalla ahorita, y el pid de su Moonlight
static PRESTANDO: Mutex<Option<(String, u32)>> = Mutex::new(None);

fn confianza() -> PathBuf {
    crate::home().join(".config/machin/confianza")
}

fn de_confianza(nombre: &str) -> bool {
    std::fs::read_to_string(confianza()).is_ok_and(|t| t.lines().any(|l| l.trim() == nombre))
}

fn confiar(nombre: &str) {
    let path = confianza();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut t = std::fs::read_to_string(&path).unwrap_or_default();
    if !t.lines().any(|l| l.trim() == nombre) {
        t.push_str(nombre);
        t.push('\n');
        let _ = std::fs::write(path, t);
    }
}

/// ¿`nombre` está emparejado en el Moonlight de aquí? (su certificado
/// guardado en Moonlight.conf; sin tocar nada, como sí haría `moonlight list`)
pub fn emparejada(nombre: &str) -> bool {
    let conf = crate::home().join(".config/Moonlight Game Streaming Project/Moonlight.conf");
    let Ok(txt) = std::fs::read_to_string(conf) else { return false };
    let mut nombres = std::collections::HashMap::new();
    let mut certs = std::collections::HashMap::new();
    for line in txt.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        let Some((n, campo)) = k.split_once('\\') else { continue };
        match campo {
            "hostname" => {
                nombres.insert(n.to_string(), v.to_string());
            }
            "srvcert" => {
                certs.insert(n.to_string(), v != "@ByteArray()" && !v.is_empty());
            }
            _ => {}
        }
    }
    nombres
        .iter()
        .any(|(n, h)| h.eq_ignore_ascii_case(nombre) && certs.get(n).copied().unwrap_or(false))
}

fn preguntar(nombre: &str) -> &'static str {
    let titulo = if es() {
        format!("{nombre} quiere usar tu pantalla")
    } else {
        format!("{nombre} wants to use your screen")
    };
    let out = Command::new("notify-send")
        .args(["-a", "machín", "-i", "video-display", "-t", "30000"])
        .arg(format!("--action=si={}", l("Aceptar", "Accept")))
        .arg(format!("--action=siempre={}", l("Siempre", "Always")))
        .arg(format!("--action=no={}", l("No", "No")))
        .arg(titulo)
        .arg(l("como monitor extra", "as an extra monitor"))
        .stderr(Stdio::null())
        .output();
    match out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).as_deref() {
        Ok("si") => "si",
        Ok("siempre") => "siempre",
        _ => "no",
    }
}

fn presta(msg: &Value, nombre: &str, ip: IpAddr) -> Value {
    if !emparejada(nombre) {
        crate::avisa(
            "machín",
            &if es() {
                format!("{nombre} no está emparejada en Moonlight: ábrelo, agrégala y pon el PIN una vez")
            } else {
                format!("{nombre} isn't paired in Moonlight: open it, add it and enter the PIN once")
            },
        );
        return red::no("sin-pareja");
    }
    // la misma que ya está conectada vuelve a pedir: se reconecta con otro
    // sonido o calidad; ya habías aceptado, no se pregunta otra vez
    let reconexion = {
        let mut p = PRESTANDO.lock().unwrap();
        match p.as_ref() {
            Some((n, pid)) if n == nombre => {
                let _ = Command::new("kill").arg(pid.to_string()).status();
                true
            }
            Some(_) => return red::no("ocupada"),
            None => {
                *p = Some((nombre.to_string(), 0));
                false
            }
        }
    };
    if !reconexion {
        let respuesta = if de_confianza(nombre) { "si" } else { preguntar(nombre) };
        if respuesta == "no" {
            *PRESTANDO.lock().unwrap() = None;
            return red::no("no");
        }
        if respuesta == "siempre" {
            confiar(nombre);
        }
    } else {
        // que el Moonlight viejo suelte la pantalla
        thread::sleep(Duration::from_millis(800));
    }
    let sonido = msg["sonido"].as_str().and_then(Sonido::de).unwrap_or(Sonido::Aqui);
    let bitrate = msg["bitrate"].as_u64().map(|b| b.clamp(2000, 150_000) as u32).unwrap_or(35000);
    let fps = msg["fps"].as_u64().map(|f| f.clamp(10, 240) as u32).unwrap_or(60);
    // «allá»: que la principal se calle (el sonido sale aquí)
    let mut hijo = match crate::moonlight(&ip.to_string(), sonido != Sonido::Alla, bitrate, fps).spawn() {
        Ok(h) => h,
        Err(e) => {
            *PRESTANDO.lock().unwrap() = None;
            return red::no(&format!("moonlight: {e}"));
        }
    };
    let pid = hijo.id();
    *PRESTANDO.lock().unwrap() = Some((nombre.to_string(), pid));
    estado::poner(pid, "presta", nombre, "");
    // si Moonlight no aguanta ni dos segundos, no conectó
    thread::sleep(Duration::from_secs(2));
    if let Ok(Some(st)) = hijo.try_wait() {
        soltar(pid);
        return red::no(&format!("moonlight {st}"));
    }
    // al cerrar Moonlight (aquí o porque allá se apagó), avisar allá; si fue
    // por una reconexión, allá no le hacen caso
    let nombre = nombre.to_string();
    thread::spawn(move || {
        let _ = hijo.wait();
        if soltar(pid) {
            println!("machín: {} {nombre}", l("ya no le presto la pantalla a", "no longer lending the screen to"));
        }
        let _ = red::pedir(&[ip], red::PUERTO, &red::suelta_msg(), Duration::from_secs(5));
    });
    red::ok()
}

/// Ya no se presta con el Moonlight `pid` (si no lo reemplazó otro).
/// true: era el de ahorita.
fn soltar(pid: u32) -> bool {
    let mut p = PRESTANDO.lock().unwrap();
    if p.as_ref().is_some_and(|(_, x)| *x == pid) {
        *p = None;
        estado::quitar_pid(pid, "presta");
        true
    } else {
        false
    }
}

fn atender(s: TcpStream) {
    let Ok(peer) = s.peer_addr() else { return };
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let mut linea = String::new();
    if BufReader::new(&s).read_line(&mut linea).is_err() {
        return;
    }
    let msg: Value = serde_json::from_str(linea.trim()).unwrap_or(Value::Null);
    let de = msg["de"].as_str().unwrap_or("").to_string();
    let r = match msg["pide"].as_str() {
        Some("presta") if !de.is_empty() => presta(&msg, &de, peer.ip()),
        Some("suelta") => {
            // mientras se reconecta, el Moonlight viejo que se cierra no cuenta
            if hypr::machin().is_some() && !estado::reconectando() {
                let _ = crate::apaga();
            }
            red::ok()
        }
        _ => red::no("no entiendo"),
    };
    let mut s = s;
    let _ = writeln!(s, "{r}");
}

pub fn escuchar() -> Result<(), String> {
    let l_ = match TcpListener::bind(("0.0.0.0", red::PUERTO)) {
        Ok(l_) => l_,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            println!("machín: {}", l("ya hay un machin escucha", "machin escucha is already running"));
            return Ok(());
        }
        Err(e) => return Err(format!("{}: {e}", red::PUERTO)),
    };
    let mons = hypr::monitors()?;
    let m = hypr::ancla(&mons).ok_or("no encontré ningún monitor")?;
    let tam = format!("{}x{}", m.px_w, m.px_h);
    let _anuncio = red::anunciar(&tam)?;
    println!("machín: {} {} ({tam})", l("escuchando como", "listening as"), red::yo());
    for s in l_.incoming().flatten() {
        thread::spawn(move || atender(s));
    }
    Ok(())
}
