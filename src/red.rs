//! Las compus con machín se ven en la red (mDNS `_machin._tcp`, con su
//! nombre y el tamaño de su pantalla) y se hablan por TCP: una línea JSON de
//! ida, una de vuelta.
//!
//!   {"pide":"presta","de":"senjougahara","sonido":"aqui","bitrate":35000,"fps":60}
//!                                           ¿me prestas tu pantalla?
//!   {"pide":"suelta","de":"hachikuji"}      ya cerré Moonlight: apágalo
//!   {"pide":"hola","de":"senjougahara"}     ¿sigues ahí? (cuando mDNS no la
//!                                           encuentra) → {"ok":true,"de":…,"tam":"WxH"}
//!   → {"ok":true} | {"ok":false,"por":"no|sin-pareja|ocupada|…"}

use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent, ServiceInfo};
use serde_json::{Value, json};

pub const SERVICIO: &str = "_machin._tcp.local.";
pub const PUERTO: u16 = 47392;

pub fn yo() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "discada".into())
}

/// Anunciarse mientras viva.
pub struct Anuncio {
    daemon: ServiceDaemon,
    fullname: String,
}

/// Interfaces que no son la LAN: docker y compañía. Si se dejan, se anuncian
/// sus IPs (172.x) y se pregunta también por ahí.
fn de_mas() -> Vec<IfKind> {
    let Ok(dir) = std::fs::read_dir("/sys/class/net") else { return Vec::new() };
    dir.flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| ["docker", "br-", "veth", "virbr", "vnet"].iter().any(|p| n.starts_with(p)))
        .map(IfKind::Name)
        .collect()
}

fn daemon() -> Result<ServiceDaemon, String> {
    let daemon = ServiceDaemon::new().map_err(|e| format!("mDNS: {e}"))?;
    let mut fuera = vec![IfKind::LoopbackV4, IfKind::LoopbackV6];
    fuera.extend(de_mas());
    daemon.disable_interface(fuera).map_err(|e| format!("mDNS: {e}"))?;
    Ok(daemon)
}

pub fn anunciar(tam: &str) -> Result<Anuncio, String> {
    let daemon = daemon()?;
    let nombre = yo();
    let info = ServiceInfo::new(
        SERVICIO,
        &nombre,
        &format!("{nombre}.local."),
        "",
        PUERTO,
        &[("tam", tam), ("v", env!("CARGO_PKG_VERSION"))][..],
    )
    .map_err(|e| format!("mDNS: {e}"))?
    .enable_addr_auto();
    let fullname = info.get_fullname().to_string();
    daemon.register(info).map_err(|e| format!("mDNS: {e}"))?;
    Ok(Anuncio { daemon, fullname })
}

impl Drop for Anuncio {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

#[derive(Debug, Clone)]
pub struct Otra {
    pub nombre: String,
    pub tam: Option<(i64, i64)>,
    /// todas las IPv4 que anuncia (con docker hay de más: se prueba cada una)
    pub ips: Vec<IpAddr>,
    pub puerto: u16,
}

/// Las otras compus con machín que contestan antes de `espera`. Se vuelve a
/// preguntar cada segundo (el módem pierde multicast entre el cable y el
/// Wi-Fi) y se sale poco después de la primera respuesta.
pub fn buscar(espera: Duration) -> Result<Vec<Otra>, String> {
    let mdns = daemon()?;
    let mut eventos = mdns.browse(SERVICIO).map_err(|e| format!("mDNS: {e}"))?;
    let inicio = Instant::now();
    let mut hasta = inicio + espera;
    let mut otra_vez = inicio + Duration::from_secs(1);
    let yo = yo();
    let mut otras: Vec<Otra> = Vec::new();
    loop {
        let ahora = Instant::now();
        if ahora >= hasta {
            break;
        }
        if otras.is_empty() && ahora >= otra_vez {
            // browse de nuevo = otra pregunta ya (y lo ya resuelto sale del caché)
            if let Ok(ev) = mdns.browse(SERVICIO) {
                eventos = ev;
            }
            otra_vez = ahora + Duration::from_secs(1);
        }
        let ev = match eventos.recv_timeout(hasta.min(otra_vez).saturating_duration_since(ahora)) {
            Ok(ev) => ev,
            Err(mdns_sd::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        let ServiceEvent::ServiceResolved(info) = ev else { continue };
        let nombre = info
            .get_fullname()
            .strip_suffix(&format!(".{SERVICIO}"))
            .unwrap_or(info.get_fullname())
            .to_string();
        if nombre == yo || otras.iter().any(|o| o.nombre == nombre) {
            continue;
        }
        let tam = info.get_property_val_str("tam").and_then(crate::tam);
        let mut ips: Vec<IpAddr> = info.get_addresses_v4().into_iter().map(IpAddr::V4).collect();
        // todavía sin dirección (acaba de arrancar): esperar a que la anuncie,
        // si no, se pedía «sin IPs»
        if ips.is_empty() {
            continue;
        }
        // las de docker (172.16/12) al último
        ips.sort_by_key(|ip| matches!(ip, IpAddr::V4(v) if v.octets()[0] == 172 && (16..32).contains(&v.octets()[1])));
        otras.push(Otra { nombre, tam, ips, puerto: info.get_port() });
        // ya hay una: un ratito más por si hay otra, no los segundos completos
        hasta = hasta.min(Instant::now() + Duration::from_millis(400));
    }
    let _ = mdns.shutdown();
    Ok(otras)
}

fn ultima_path() -> std::path::PathBuf {
    crate::home().join(".config/machin/ultima")
}

/// La última compu con la que se conectó, para cuando mDNS no la encuentra.
pub fn guardar_ultima(o: &Otra) {
    let (w, h) = o.tam.unwrap_or((0, 0));
    let v = json!({
        "nombre": o.nombre,
        "ips": o.ips.iter().map(|i| i.to_string()).collect::<Vec<_>>(),
        "puerto": o.puerto,
        "tam": format!("{w}x{h}"),
    });
    let p = ultima_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, format!("{v}\n"));
}

pub fn ultima() -> Option<Otra> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(ultima_path()).ok()?).ok()?;
    let ips: Vec<IpAddr> = v["ips"].as_array()?.iter().filter_map(|i| i.as_str()?.parse().ok()).collect();
    if ips.is_empty() {
        return None;
    }
    Some(Otra {
        nombre: v["nombre"].as_str()?.to_string(),
        tam: v["tam"].as_str().and_then(crate::tam),
        ips,
        puerto: v["puerto"].as_u64().map(|p| p as u16).unwrap_or(PUERTO),
    })
}

/// Sin mDNS: ¿la última sigue escuchando? Contesta con su pantalla (una
/// 0.3.1 contesta «no entiendo», que también dice que está ahí).
pub fn directo(o: &Otra) -> Option<Otra> {
    let v = pedir(&o.ips, o.puerto, &json!({ "pide": "hola", "de": yo() }), Duration::from_secs(2)).ok()?;
    let mut o = o.clone();
    if let Some(t) = v["tam"].as_str().and_then(crate::tam) {
        o.tam = Some(t);
    }
    if let Some(n) = v["de"].as_str() {
        o.nombre = n.to_string();
    }
    Some(o)
}

pub fn hola(tam: &str) -> Value {
    json!({ "ok": true, "de": yo(), "tam": tam })
}

/// Manda `msg` a la primera IP que conteste y espera la respuesta hasta
/// `espera` (la otra puede estar preguntándote si aceptas).
pub fn pedir(ips: &[IpAddr], puerto: u16, msg: &Value, espera: Duration) -> Result<Value, String> {
    let mut ultimo = String::from("sin IPs");
    for ip in ips {
        let addr = SocketAddr::new(*ip, puerto);
        let mut s = match TcpStream::connect_timeout(&addr, Duration::from_secs(2)) {
            Ok(s) => s,
            Err(e) => {
                ultimo = format!("{addr}: {e}");
                continue;
            }
        };
        let _ = s.set_read_timeout(Some(espera));
        writeln!(s, "{msg}").map_err(|e| format!("{addr}: {e}"))?;
        let mut linea = String::new();
        BufReader::new(&s).read_line(&mut linea).map_err(|e| format!("{addr}: {e}"))?;
        return serde_json::from_str(linea.trim()).map_err(|e| format!("{addr}: {e}"));
    }
    Err(ultimo)
}

/// Lo que la principal decide: el sonido y la calidad.
pub fn presta_msg(a: &crate::ajustes::Ajustes) -> Value {
    json!({
        "pide": "presta",
        "de": yo(),
        "sonido": a.sonido.clave(),
        "bitrate": a.bitrate,
        "fps": a.fps,
    })
}

pub fn suelta_msg() -> Value {
    json!({ "pide": "suelta", "de": yo() })
}

pub fn ok() -> Value {
    json!({ "ok": true })
}

pub fn no(por: &str) -> Value {
    json!({ "ok": false, "por": por })
}
