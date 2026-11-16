//! Las compus con machín se ven en la red (mDNS `_machin._tcp`, con su
//! nombre y el tamaño de su pantalla) y se hablan por TCP: una línea JSON de
//! ida, una de vuelta.
//!
//!   {"pide":"presta","de":"senjougahara","sonido":"aqui","bitrate":35000,"fps":60}
//!                                           ¿me prestas tu pantalla?
//!   {"pide":"suelta","de":"hachikuji"}      ya cerré Moonlight: apágalo
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

pub fn anunciar(tam: &str) -> Result<Anuncio, String> {
    let daemon = ServiceDaemon::new().map_err(|e| format!("mDNS: {e}"))?;
    daemon
        .disable_interface(vec![IfKind::LoopbackV4, IfKind::LoopbackV6])
        .map_err(|e| format!("mDNS: {e}"))?;
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

/// Las otras compus con machín que contestan en `espera`.
pub fn buscar(espera: Duration) -> Result<Vec<Otra>, String> {
    let mdns = ServiceDaemon::new().map_err(|e| format!("mDNS: {e}"))?;
    let eventos = mdns.browse(SERVICIO).map_err(|e| format!("mDNS: {e}"))?;
    let hasta = Instant::now() + espera;
    let yo = yo();
    let mut otras: Vec<Otra> = Vec::new();
    while let Ok(ev) = eventos.recv_timeout(hasta.saturating_duration_since(Instant::now())) {
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
        // las de docker (172.16/12) al último
        ips.sort_by_key(|ip| matches!(ip, IpAddr::V4(v) if v.octets()[0] == 172 && (16..32).contains(&v.octets()[1])));
        otras.push(Otra { nombre, tam, ips, puerto: info.get_port() });
    }
    let _ = mdns.shutdown();
    Ok(otras)
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
