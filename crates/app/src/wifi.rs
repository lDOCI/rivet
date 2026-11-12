//! Wi-Fi through wpa_supplicant's control socket.
//!
//! Moonraker has no Wi-Fi API, but Rivet runs on the printer itself, where
//! wpa_supplicant listens on /var/run/wpa_supplicant/<interface> (KlipperMod
//! starts it with update_config=1, so SAVE_CONFIG keeps a joined network).
//! These calls block: run them off the UI thread.
//!
//! Joining is careful: a new network goes in as a separate entry and is kept
//! only once wpa_supplicant reports it connected. Otherwise the entry goes and
//! the previous network is selected again, so a wrong password never leaves
//! the printer off the network.

use std::{
    io,
    os::unix::net::UnixDatagram,
    path::{Path, PathBuf},
    thread::sleep,
    time::{Duration, Instant},
};

use model::{WifiAuth, WifiNet};

const CTRL_DIRS: [&str; 2] = ["/var/run/wpa_supplicant", "/run/wpa_supplicant"];
/// How long a join may take before it counts as failed.
const JOIN_TIMEOUT: Duration = Duration::from_secs(25);

struct Ctrl {
    sock: UnixDatagram,
    local: PathBuf,
}

impl Ctrl {
    fn open() -> io::Result<Self> {
        let iface = CTRL_DIRS
            .iter()
            .filter_map(|d| std::fs::read_dir(d).ok())
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| !n.starts_with("p2p"))
            })
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no wpa_supplicant socket"))?;
        let local = std::env::temp_dir().join(format!("rivet-wpa-{}", std::process::id()));
        let _ = std::fs::remove_file(&local);
        let sock = UnixDatagram::bind(&local)?;
        sock.connect(&iface)?;
        sock.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(Self { sock, local })
    }

    fn cmd(&self, command: &str) -> io::Result<String> {
        self.sock.send(command.as_bytes())?;
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            let n = self.sock.recv(&mut buf)?;
            let reply = String::from_utf8_lossy(&buf[..n]).into_owned();
            // Event messages ("<3>CTRL-EVENT-…") only come after ATTACH; skip any.
            if !reply.starts_with('<') {
                return Ok(reply);
            }
        }
    }

    /// A command that answers "OK".
    fn ok(&self, command: &str) -> Result<(), String> {
        match self.cmd(command) {
            Ok(r) if r.trim() == "OK" => Ok(()),
            Ok(r) => Err(format!("{command}: {}", r.trim())),
            Err(e) => Err(format!("{command}: {e}")),
        }
    }
}

impl Drop for Ctrl {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.local);
    }
}

/// Whether this machine has wpa_supplicant to talk to.
pub fn available() -> bool {
    CTRL_DIRS
        .iter()
        .any(|d| std::fs::read_dir(d).is_ok_and(|mut e| e.next().is_some()))
}

/// What the Network screen shows.
#[derive(Debug, Default)]
pub struct View {
    /// The connected network.
    pub current: Option<String>,
    pub nets: Vec<WifiNet>,
    /// Networks wpa_supplicant knows (joined before).
    pub saved: Vec<String>,
}

/// Look around: ask for a scan, give it a few seconds, read what it found.
pub fn scan() -> Result<View, String> {
    let c = Ctrl::open().map_err(|e| e.to_string())?;
    // FAIL-BUSY when a scan is already running: its results serve as well.
    let _ = c.cmd("SCAN");
    sleep(Duration::from_secs(4));
    let results = c.cmd("SCAN_RESULTS").map_err(|e| e.to_string())?;
    let saved = model::parse_wifi_saved(&c.cmd("LIST_NETWORKS").map_err(|e| e.to_string())?);
    let status = model::parse_wpa_status(&c.cmd("STATUS").map_err(|e| e.to_string())?);
    let current = (status.get("wpa_state").map(String::as_str) == Some("COMPLETED"))
        .then(|| status.get("ssid").map(|s| model::wpa_unescape(s)))
        .flatten();
    Ok(View {
        current,
        nets: model::parse_wifi_scan(&results),
        saved: saved.into_iter().map(|(_, s, _)| s).collect(),
    })
}

/// Join `ssid`. An empty password joins a saved network as it was saved (or
/// an open one). Ok once connected and saved; on failure the printer is back
/// on the network it had.
pub fn join(ssid: &str, auth: WifiAuth, password: &str) -> Result<(), String> {
    let c = Ctrl::open().map_err(|e| e.to_string())?;
    let saved = model::parse_wifi_saved(&c.cmd("LIST_NETWORKS").map_err(|e| e.to_string())?);
    let previous = saved.iter().find(|n| n.2).map(|n| n.0);
    let known = saved.iter().find(|n| n.1 == ssid).map(|n| n.0);

    // A saved network without a new password: just switch to it.
    let (id, added) = match (known, password.is_empty()) {
        (Some(id), true) => (id, false),
        _ => {
            let id: u32 = c
                .cmd("ADD_NETWORK")
                .map_err(|e| e.to_string())?
                .trim()
                .parse()
                .map_err(|_| "ADD_NETWORK failed".to_owned())?;
            let setup = || -> Result<(), String> {
                c.ok(&format!("SET_NETWORK {id} ssid {}", model::wpa_hex(ssid)))?;
                // Hidden networks answer only when asked by name.
                c.ok(&format!("SET_NETWORK {id} scan_ssid 1"))?;
                match auth {
                    WifiAuth::Open => c.ok(&format!("SET_NETWORK {id} key_mgmt NONE")),
                    WifiAuth::Sae => {
                        c.ok(&format!("SET_NETWORK {id} key_mgmt SAE"))?;
                        c.ok(&format!("SET_NETWORK {id} sae_password \"{password}\""))?;
                        c.ok(&format!("SET_NETWORK {id} ieee80211w 2"))
                    }
                    _ => c.ok(&format!("SET_NETWORK {id} psk \"{password}\"")),
                }
            };
            if let Err(e) = setup() {
                let _ = c.ok(&format!("REMOVE_NETWORK {id}"));
                return Err(e);
            }
            (id, true)
        }
    };

    c.ok(&format!("SELECT_NETWORK {id}"))?;
    let start = Instant::now();
    let joined = loop {
        sleep(Duration::from_secs(1));
        let status = c
            .cmd("STATUS")
            .map(|s| model::parse_wpa_status(&s))
            .unwrap_or_default();
        if status.get("wpa_state").map(String::as_str) == Some("COMPLETED")
            && status.get("id").map(String::as_str) == Some(id.to_string().as_str())
        {
            break true;
        }
        if start.elapsed() > JOIN_TIMEOUT {
            break false;
        }
    };

    if !joined {
        if added {
            let _ = c.ok(&format!("REMOVE_NETWORK {id}"));
        }
        if let Some(prev) = previous {
            let _ = c.ok(&format!("SELECT_NETWORK {prev}"));
        }
        let _ = c.ok("ENABLE_NETWORK all");
        return Err("not joined".into());
    }

    // Older entries of the same name would only get in the way.
    if added {
        for (old, name, _) in &saved {
            if name == ssid {
                let _ = c.ok(&format!("REMOVE_NETWORK {old}"));
            }
        }
    }
    // SELECT_NETWORK disabled the others: let them back for later.
    let _ = c.ok("ENABLE_NETWORK all");
    c.ok("SAVE_CONFIG")?;
    renew_dhcp("wlan0");
    Ok(())
}

/// A new network wants a new address: ask udhcpc to renew now.
fn renew_dhcp(iface: &str) {
    let pid_file = Path::new("/var/run").join(format!("udhcpc.{iface}.pid"));
    if let Some(pid) = std::fs::read_to_string(pid_file)
        .ok()
        .and_then(|p| p.trim().parse::<i32>().ok())
    {
        // SIGUSR1: renew the lease.
        unsafe {
            libc::kill(pid, libc::SIGUSR1);
        }
    }
}
