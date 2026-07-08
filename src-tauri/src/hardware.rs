use chrono::Utc;
use serde::Serialize;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

type CommandResult<T> = Result<T, String>;

#[derive(Debug, Serialize)]
pub struct HardwareDevice {
    pub id: String,
    pub name: String,
    pub device_type: String,
    pub connection: String,
    pub detail: String,
    pub is_default: bool,
}

fn clean_device_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(180)
        .collect()
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg_attr(not(windows), allow(unused_variables))]
fn configure_command(command: &mut Command) {
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }
}

fn command_output(program: &str, args: &[&str], timeout_ms: u64) -> CommandResult<Output> {
    let mut command = Command::new(program);
    command.args(args);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    configure_command(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("No se pudo ejecutar {program}: {error}"))?;

    // Drain stdout/stderr on background threads while we poll for exit.
    // Without this, a child that writes more than the OS pipe buffer
    // (~64KB on Windows) blocks on write() forever once the buffer fills,
    // because nothing reads the pipe until the process exits -- and we only
    // called wait_with_output() after exit. A PowerShell scan listing many
    // printers (real + virtual, like "Fax"/"Microsoft Print to PDF") plus PnP
    // devices easily exceeds that, so the process hung silently until our
    // timeout killed it, yielding zero devices every time no matter how many
    // were actually connected.
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let stdout_handle = thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(pipe) = stdout_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buffer);
        }
        buffer
    });
    let stderr_handle = thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(pipe) = stderr_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buffer);
        }
        buffer
    });

    let started_at = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started_at.elapsed() >= Duration::from_millis(timeout_ms) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("{program} tardo demasiado"));
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(format!("{program} fallo: {error}")),
        }
    };
    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();
    Ok(Output { status, stdout, stderr })
}

#[cfg(windows)]
fn powershell_args(command: &str) -> [&str; 6] {
    [
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        command,
    ]
}

fn command_lines(program: &str, args: &[&str]) -> Vec<String> {
    command_lines_timeout(program, args, 2500)
}

fn command_lines_timeout(program: &str, args: &[&str], timeout_ms: u64) -> Vec<String> {
    let Ok(output) = command_output(program, args, timeout_ms) else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(clean_device_text)
        .filter(|line| !line.is_empty())
        .collect()
}

fn add_device(
    devices: &mut Vec<HardwareDevice>,
    seen: &mut HashSet<String>,
    device: HardwareDevice,
) {
    if seen.insert(format!("{}:{}", device.device_type, device.id)) {
        devices.push(device);
    }
}

fn extract_ipv4(value: &str) -> Option<Ipv4Addr> {
    value.split(|character: char| !(character.is_ascii_digit() || character == '.'))
        .find_map(|part| part.parse::<Ipv4Addr>().ok())
}

fn is_usable_ipv4(ip: &Ipv4Addr) -> bool {
    !ip.is_loopback() && !ip.is_unspecified() && !ip.octets().starts_with(&[169, 254])
}

fn local_ipv4_addresses() -> Vec<Ipv4Addr> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    #[cfg(windows)]
    {
        let command = "Get-NetIPAddress -AddressFamily IPv4 | ForEach-Object { $_.IPAddress }";
        for line in command_lines("powershell", &powershell_args(command)) {
            if let Some(ip) = extract_ipv4(&line).filter(is_usable_ipv4) {
                if seen.insert(ip) {
                    result.push(ip);
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        for line in command_lines("ifconfig", &[]) {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("inet ") {
                if let Some(ip) = extract_ipv4(rest).filter(is_usable_ipv4) {
                    if seen.insert(ip) {
                        result.push(ip);
                    }
                }
            }
        }
        if result.is_empty() {
            for line in command_lines("hostname", &["-I"]) {
                for part in line.split_whitespace() {
                    if let Some(ip) = extract_ipv4(part).filter(is_usable_ipv4) {
                        if seen.insert(ip) {
                            result.push(ip);
                        }
                    }
                }
            }
        }
    }

    result
}

fn arp_ipv4_hosts() -> Vec<Ipv4Addr> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for line in command_lines("arp", &["-a"]) {
        if let Some(ip) = extract_ipv4(&line).filter(is_usable_ipv4) {
            if seen.insert(ip) {
                result.push(ip);
            }
        }
    }
    result
}

fn network_endpoint(device: &str) -> Option<SocketAddr> {
    let endpoint = device.strip_prefix("tcp://")?;
    let (host, port) = endpoint.rsplit_once(':')?;
    let port = port.parse::<u16>().ok()?;
    format!("{host}:{port}")
        .to_socket_addrs()
        .ok()?
        .find(|address| address.is_ipv4())
}

fn resolve_first_ipv4(host: &str, port: u16) -> Option<Ipv4Addr> {
    format!("{host}:{port}")
        .to_socket_addrs()
        .ok()?
        .find_map(|address| match address.ip() {
            IpAddr::V4(ip) => Some(ip),
            IpAddr::V6(_) => None,
        })
}

fn tcp_port_open(ip: Ipv4Addr, port: u16, timeout_ms: u64) -> bool {
    let address = SocketAddr::new(IpAddr::V4(ip), port);
    TcpStream::connect_timeout(&address, Duration::from_millis(timeout_ms)).is_ok()
}

fn write_network_device(device: &str, bytes: &[u8]) -> CommandResult<bool> {
    let Some(address) = network_endpoint(device) else {
        return Ok(false);
    };
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(1500))
        .map_err(|error| format!("No se pudo abrir socket {address}: {error}"))?;
    let _ = stream.set_write_timeout(Some(Duration::from_millis(1500)));
    stream
        .write_all(bytes)
        .map_err(|error| format!("No se pudo escribir a {address}: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("No se pudo cerrar envio a {address}: {error}"))?;
    Ok(true)
}

fn detect_bonjour_printers(devices: &mut Vec<HardwareDevice>, seen: &mut HashSet<String>) {
    let regtypes = ["_ipp._tcp", "_ipps._tcp"];
    for regtype in regtypes {
        let lines = command_lines(
            "ippfind",
            &[
                "-4",
                "-T",
                "2",
                regtype,
                "--remote",
                "--exec",
                "echo",
                "{service_name}|{service_hostname}|{service_port}|{service_uri}",
                ";",
            ],
        );
        for line in lines {
            let parts: Vec<&str> = line.split('|').collect();
            let name = clean_device_text(parts.first().copied().unwrap_or(""));
            let hostname = clean_device_text(parts.get(1).copied().unwrap_or(""));
            let port = parts
                .get(2)
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(631);
            let uri = clean_device_text(parts.get(3).copied().unwrap_or(""));
            if name.is_empty() || hostname.is_empty() {
                continue;
            }

            let Some(ip) = resolve_first_ipv4(&hostname, port) else {
                continue;
            };

            if tcp_port_open(ip, 9100, 180) {
                let endpoint = format!("tcp://{ip}:9100");
                add_device(
                    devices,
                    seen,
                    HardwareDevice {
                        id: endpoint,
                        name,
                        device_type: "printer".into(),
                        connection: "network-bonjour-raw".into(),
                        detail: if uri.is_empty() {
                            format!("Bonjour {hostname} · RAW TCP 9100")
                        } else {
                            format!("Bonjour {hostname} · {uri} · RAW TCP 9100")
                        },
                        is_default: false,
                    },
                );
            }
        }
    }
}

fn detect_network_printers(devices: &mut Vec<HardwareDevice>, seen: &mut HashSet<String>) {
    let local_ips = local_ipv4_addresses();
    if local_ips.is_empty() {
        return;
    }

    let mut candidate_ips = HashSet::new();
    for ip in arp_ipv4_hosts() {
        candidate_ips.insert(ip);
    }
    for ip in &local_ips {
        let [a, b, c, current] = ip.octets();
        for host in 1..=254 {
            if host == current {
                continue;
            }
            candidate_ips.insert(Ipv4Addr::new(a, b, c, host));
        }
    }

    let mut workers = Vec::new();
    for ip in candidate_ips {
        workers.push(thread::spawn(move || {
            if tcp_port_open(ip, 9100, 120) {
                Some(ip)
            } else {
                None
            }
        }));
    }

    for worker in workers {
        let Ok(Some(ip)) = worker.join() else {
            continue;
        };
        let endpoint = format!("tcp://{ip}:9100");
        add_device(
            devices,
            seen,
            HardwareDevice {
                id: endpoint,
                name: format!("Impresora LAN {ip}"),
                device_type: "printer".into(),
                connection: "network-raw".into(),
                detail: "Socket RAW TCP 9100 en red local".into(),
                is_default: false,
            },
        );
    }
}

fn detect_unix_printers(devices: &mut Vec<HardwareDevice>, seen: &mut HashSet<String>) {
    let default_printer = command_lines("lpstat", &["-d"])
        .into_iter()
        .find_map(|line| {
            line.strip_prefix("system default destination: ")
                .map(str::to_string)
        });
    let uri_lines = command_lines("lpstat", &["-v"]);
    for line in command_lines("lpstat", &["-p"]) {
        let Some(rest) = line.strip_prefix("printer ") else {
            continue;
        };
        let name = rest.split_whitespace().next().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }
        let detail = uri_lines
            .iter()
            .find_map(|uri_line| {
                uri_line
                    .strip_prefix(&format!("device for {name}: "))
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "Impresora del sistema".into());
        add_device(
            devices,
            seen,
            HardwareDevice {
                id: name.into(),
                name: name.into(),
                device_type: "printer".into(),
                connection: "system".into(),
                detail,
                is_default: default_printer.as_deref() == Some(name),
            },
        );
    }
}

/// Printers plugged in via USB (or otherwise live-detected) that don't have a
/// CUPS queue yet. `lpstat -p` (detect_unix_printers) only lists printers
/// already added as a queue, so a receipt printer connected but never "added"
/// in Impresoras y Escaneres is invisible to it -- this is why a USB thermal
/// printer can show up as "not detected" even though macOS/Linux can see it.
/// `lpinfo -v` reports every live device CUPS can currently talk to, including
/// unconfigured ones, as "<class> <uri>" lines (e.g. "direct usb://Brother/...").
/// Parses one `lpinfo -v` line into (uri, readable_name) when it's a live
/// USB-attached ("direct") device with an actual URI. Bare backend listings
/// like "network https" (no live device using that backend right now) return
/// None. Pulled out as a pure function so the parsing can be unit tested
/// without depending on a real CUPS install.
fn parse_lpinfo_direct_line(line: &str) -> Option<(String, String)> {
    let (class, uri) = line.split_once(' ')?;
    if class != "direct" || !uri.contains("://") {
        return None;
    }
    let uri = uri.trim().to_string();
    let readable = uri
        .split("://")
        .nth(1)
        .unwrap_or(&uri)
        .split('?')
        .next()
        .unwrap_or(&uri)
        .replace("%20", " ")
        .replace('/', " ");
    let name = clean_device_text(&readable);
    let name = if name.is_empty() { "Impresora USB".to_string() } else { name };
    Some((uri, name))
}

fn detect_unconfigured_unix_printers(devices: &mut Vec<HardwareDevice>, seen: &mut HashSet<String>) {
    let configured_uris: HashSet<String> = command_lines("lpstat", &["-v"])
        .into_iter()
        .filter_map(|line| line.splitn(2, ": ").nth(1).map(|uri| uri.trim().to_string()))
        .collect();
    for line in command_lines("lpinfo", &["-v"]) {
        let Some((uri, name)) = parse_lpinfo_direct_line(&line) else {
            continue;
        };
        if configured_uris.contains(&uri) {
            continue;
        }
        add_device(
            devices,
            seen,
            HardwareDevice {
                id: uri,
                name,
                device_type: "unconfigured".into(),
                connection: "unix-usb-unconfigured".into(),
                detail: "Conectada por USB sin cola de impresion. Agregala en Ajustes del Sistema > Impresoras y Escaneres (o Preferencias del Sistema en macOS antiguo).".into(),
                is_default: false,
            },
        );
    }
}

#[cfg(windows)]
fn detect_windows_hardware(devices: &mut Vec<HardwareDevice>, seen: &mut HashSet<String>) {
    // Single PowerShell process covers printers, serial ports and driver-less
    // plugged-in devices. Each separate PS spawn used to cost 300-800ms of cold
    // start alone, so merging 5 calls into 1 is what actually fixes the freeze.
    const SCAN_SCRIPT: &str = r#"
$printers = Get-CimInstance Win32_Printer
$defaultName = ($printers | Where-Object { $_.Default }).Name | Select-Object -First 1
foreach ($p in $printers) { "PRINTER|$($p.Name)|$($p.DriverName)|$($p.PortName)|$([bool]($p.Name -eq $defaultName))" }
Get-CimInstance Win32_SerialPort | ForEach-Object { "SERIAL|$($_.DeviceID)|$($_.Name)|$($_.PNPDeviceID)" }
Get-CimInstance Win32_PnPEntity | Where-Object { $_.Name -match '\(COM[0-9]+\)' } | ForEach-Object { if ($_.Name -match '(COM[0-9]+)') { "PNPCOM|$($Matches[1])|$($_.Name)|$($_.PNPDeviceID)" } }
Get-PnpDevice -PresentOnly | Where-Object { $_.Class -in @('Printer','Ports','USB') -and $_.Status -ne 'OK' } | ForEach-Object { "UNCONF|$($_.FriendlyName)|$($_.Class)|$($_.InstanceId)" }
Get-PnpDevice -PresentOnly -Class Printer | ForEach-Object { "PRINTERPNP|$($_.FriendlyName)|$($_.Status)|$($_.InstanceId)" }
"#;
    // Names already registered as a Windows spooler printer (Win32_Printer),
    // so the broader Class-Printer PnP scan below doesn't add a duplicate row
    // for the same physical printer.
    let mut spooler_printer_names: HashSet<String> = HashSet::new();
    for line in command_lines_timeout("powershell", &powershell_args(SCAN_SCRIPT), 9000) {
        let parts: Vec<&str> = line.split('|').collect();
        let Some(tag) = parts.first().copied() else {
            continue;
        };
        match tag {
            "PRINTER" => {
                let name = clean_device_text(parts.get(1).copied().unwrap_or(""));
                if name.is_empty() {
                    continue;
                }
                spooler_printer_names.insert(name.to_ascii_lowercase());
                let driver = clean_device_text(parts.get(2).copied().unwrap_or(""));
                let port = clean_device_text(parts.get(3).copied().unwrap_or(""));
                let is_default = parts
                    .get(4)
                    .map(|value| value.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                add_device(
                    devices,
                    seen,
                    HardwareDevice {
                        id: name.clone(),
                        name,
                        device_type: "printer".into(),
                        connection: "windows-printer".into(),
                        detail: format!("{driver} {port}").trim().into(),
                        is_default,
                    },
                );
            }
            "SERIAL" => {
                let id = clean_device_text(parts.get(1).copied().unwrap_or(""));
                if id.is_empty() {
                    continue;
                }
                let name = clean_device_text(parts.get(2).copied().unwrap_or(&id));
                add_device(
                    devices,
                    seen,
                    HardwareDevice {
                        id: id.clone(),
                        name,
                        device_type: "serial".into(),
                        connection: "serial".into(),
                        detail: clean_device_text(parts.get(3).copied().unwrap_or("Puerto serial local")),
                        is_default: false,
                    },
                );
            }
            "PNPCOM" => {
                let id = clean_device_text(parts.get(1).copied().unwrap_or(""));
                if id.is_empty() {
                    continue;
                }
                let name = clean_device_text(parts.get(2).copied().unwrap_or(&id));
                add_device(
                    devices,
                    seen,
                    HardwareDevice {
                        id: id.clone(),
                        name,
                        device_type: "serial".into(),
                        connection: "windows-pnp-port".into(),
                        detail: clean_device_text(parts.get(3).copied().unwrap_or("Puerto COM")),
                        is_default: false,
                    },
                );
            }
            "UNCONF" => {
                let name = clean_device_text(parts.get(1).copied().unwrap_or(""));
                if name.is_empty() {
                    continue;
                }
                let class = clean_device_text(parts.get(2).copied().unwrap_or(""));
                let instance_id = clean_device_text(parts.get(3).copied().unwrap_or(""));
                add_device(
                    devices,
                    seen,
                    HardwareDevice {
                        id: instance_id.clone(),
                        name,
                        device_type: "unconfigured".into(),
                        connection: "windows-driver-missing".into(),
                        detail: format!("{class} - falta instalar driver ({instance_id})"),
                        is_default: false,
                    },
                );
            }
            // A printer-class Plug & Play device present on the USB bus but
            // with no matching entry in Win32_Printer: Windows sees the USB
            // hardware but never created a spooler queue for it (missing/
            // incomplete driver install). Without this, such a printer is
            // completely invisible to the app even though it's plugged in.
            "PRINTERPNP" => {
                let name = clean_device_text(parts.get(1).copied().unwrap_or(""));
                if name.is_empty() || spooler_printer_names.contains(&name.to_ascii_lowercase()) {
                    continue;
                }
                let status = clean_device_text(parts.get(2).copied().unwrap_or(""));
                let instance_id = clean_device_text(parts.get(3).copied().unwrap_or(""));
                add_device(
                    devices,
                    seen,
                    HardwareDevice {
                        id: instance_id.clone(),
                        name,
                        device_type: "unconfigured".into(),
                        connection: "windows-printer-no-queue".into(),
                        detail: format!("USB detectado ({status}) sin cola de impresion en Windows. Agregala en Configuracion > Impresoras y escaneres."),
                        is_default: false,
                    },
                );
            }
            _ => {}
        }
    }
}

fn detect_serial_paths(devices: &mut Vec<HardwareDevice>, seen: &mut HashSet<String>) {
    #[cfg(not(windows))]
    {
        let prefixes = [
            "ttyUSB",
            "ttyACM",
            "tty.usb",
            "cu.usb",
            "ttyS",
            "cu.SLAB",
            "tty.SLAB",
            "cu.wchusbserial",
            "tty.wchusbserial",
            "cu.usbserial",
            "tty.usbserial",
            "cu.usbmodem",
            "tty.usbmodem",
        ];
        if let Ok(entries) = fs::read_dir("/dev") {
            for entry in entries.flatten() {
                let filename = entry.file_name().to_string_lossy().to_string();
                let lower = filename.to_ascii_lowercase();
                if lower.contains("bluetooth") || lower.contains("incoming-port") {
                    continue;
                }
                let likely_serial = prefixes.iter().any(|prefix| filename.starts_with(prefix))
                    || (lower.contains("serial")
                        || lower.contains("ch34")
                        || lower.contains("cp210")
                        || lower.contains("ftdi")
                        || lower.contains("pl2303"))
                        && (filename.starts_with("tty") || filename.starts_with("cu."));
                if !likely_serial {
                    continue;
                }
                let path = format!("/dev/{filename}");
                add_device(
                    devices,
                    seen,
                    HardwareDevice {
                        id: path.clone(),
                        name: filename,
                        device_type: "serial".into(),
                        connection: "serial".into(),
                        detail: path,
                        is_default: false,
                    },
                );
            }
        }
        if let Ok(entries) = fs::read_dir("/dev/serial/by-id") {
            for entry in entries.flatten() {
                let path = entry.path().to_string_lossy().to_string();
                let name = entry.file_name().to_string_lossy().to_string();
                add_device(
                    devices,
                    seen,
                    HardwareDevice {
                        id: path.clone(),
                        name,
                        device_type: "serial".into(),
                        connection: "serial-by-id".into(),
                        detail: path,
                        is_default: false,
                    },
                );
            }
        }
    }
}

fn detect_raw_printer_paths(devices: &mut Vec<HardwareDevice>, seen: &mut HashSet<String>) {
    #[cfg(not(windows))]
    {
        let raw_dirs = ["/dev/usb", "/dev"];
        for dir in raw_dirs {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let filename = entry.file_name().to_string_lossy().to_string();
                    let lower = filename.to_ascii_lowercase();
                    let likely_printer = lower.starts_with("lp")
                        || lower.starts_with("usblp")
                        || lower.starts_with("ulpt")
                        || lower.contains("escpos")
                        || lower.contains("pos");
                    if !likely_printer {
                        continue;
                    }
                    let path = entry.path().to_string_lossy().to_string();
                    add_device(
                        devices,
                        seen,
                        HardwareDevice {
                            id: path.clone(),
                            name: filename,
                            device_type: "printer".into(),
                            connection: "raw-device".into(),
                            detail: path,
                            is_default: false,
                        },
                    );
                }
            }
        }
    }
}

pub fn device_list(include_network: bool) -> Vec<HardwareDevice> {
    let mut devices = Vec::new();
    let mut seen = HashSet::new();
    #[cfg(windows)]
    detect_windows_hardware(&mut devices, &mut seen);
    #[cfg(not(windows))]
    {
        detect_unix_printers(&mut devices, &mut seen);
        detect_unconfigured_unix_printers(&mut devices, &mut seen);
    }
    detect_raw_printer_paths(&mut devices, &mut seen);
    detect_serial_paths(&mut devices, &mut seen);
    if include_network {
        detect_bonjour_printers(&mut devices, &mut seen);
        detect_network_printers(&mut devices, &mut seen);
    }
    devices
}

pub fn temp_hardware_file(prefix: &str, extension: &str) -> PathBuf {
    let clean_prefix = prefix
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .collect::<String>();
    env::temp_dir().join(format!(
        "{clean_prefix}-{}.{}",
        Utc::now().timestamp_nanos_opt().unwrap_or(0),
        extension
    ))
}

#[cfg(windows)]
fn ps_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

// C# helper (single-quoted here-string so its double quotes stay literal) that
// sends bytes to a Windows print spooler with the RAW datatype via winspool.
#[cfg(windows)]
const WINDOWS_RAW_PRINT_CSHARP: &str = r#"$src = @'
using System;
using System.Runtime.InteropServices;
public static class RimRawPrint {
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
  public struct DOCINFOW { [MarshalAs(UnmanagedType.LPWStr)] public string pDocName; [MarshalAs(UnmanagedType.LPWStr)] public string pOutputFile; [MarshalAs(UnmanagedType.LPWStr)] public string pDatatype; }
  [DllImport("winspool.Drv", EntryPoint="OpenPrinterW", SetLastError=true, CharSet=CharSet.Unicode, ExactSpelling=true)] public static extern bool OpenPrinter(string src, out IntPtr hPrinter, IntPtr pd);
  [DllImport("winspool.Drv", EntryPoint="ClosePrinter", SetLastError=true, ExactSpelling=true)] public static extern bool ClosePrinter(IntPtr hPrinter);
  [DllImport("winspool.Drv", EntryPoint="StartDocPrinterW", SetLastError=true, CharSet=CharSet.Unicode, ExactSpelling=true)] public static extern bool StartDocPrinter(IntPtr hPrinter, int level, ref DOCINFOW di);
  [DllImport("winspool.Drv", EntryPoint="EndDocPrinter", SetLastError=true, ExactSpelling=true)] public static extern bool EndDocPrinter(IntPtr hPrinter);
  [DllImport("winspool.Drv", EntryPoint="StartPagePrinter", SetLastError=true, ExactSpelling=true)] public static extern bool StartPagePrinter(IntPtr hPrinter);
  [DllImport("winspool.Drv", EntryPoint="EndPagePrinter", SetLastError=true, ExactSpelling=true)] public static extern bool EndPagePrinter(IntPtr hPrinter);
  [DllImport("winspool.Drv", EntryPoint="WritePrinter", SetLastError=true, ExactSpelling=true)] public static extern bool WritePrinter(IntPtr hPrinter, byte[] pBytes, int dwCount, out int dwWritten);
  public static void Send(string printerName, byte[] bytes) {
    IntPtr h;
    if(!OpenPrinter(printerName, out h, IntPtr.Zero)) throw new Exception("OpenPrinter fallo: " + Marshal.GetLastWin32Error());
    try {
      DOCINFOW di = new DOCINFOW(); di.pDocName = "RIM-POS Ticket"; di.pDatatype = "RAW";
      if(!StartDocPrinter(h, 1, ref di)) throw new Exception("StartDocPrinter fallo: " + Marshal.GetLastWin32Error());
      try {
        if(!StartPagePrinter(h)) throw new Exception("StartPagePrinter fallo: " + Marshal.GetLastWin32Error());
        int written;
        if(!WritePrinter(h, bytes, bytes.Length, out written)) throw new Exception("WritePrinter fallo: " + Marshal.GetLastWin32Error());
        EndPagePrinter(h);
      } finally { EndDocPrinter(h); }
    } finally { ClosePrinter(h); }
  }
}
'@
Add-Type -TypeDefinition $src -Language CSharp
[RimRawPrint]::Send($printer, $bytes)
"#;

// Sends a file to a Windows spooler queue as RAW (unfiltered) so ESC/POS tickets
// print at 58mm and cut instead of being re-rendered by the driver to a page.
#[cfg(windows)]
fn windows_raw_spool_print(printer: &str, file: &PathBuf) -> CommandResult<()> {
    let mut script = String::new();
    script.push_str("$ErrorActionPreference = 'Stop'\n");
    script.push_str(&format!("$printer = '{}'\n", ps_single_quote(printer)));
    script.push_str(&format!(
        "$path = '{}'\n",
        ps_single_quote(&file.to_string_lossy())
    ));
    script.push_str("$bytes = [System.IO.File]::ReadAllBytes($path)\n");
    script.push_str(WINDOWS_RAW_PRINT_CSHARP);

    let script_file = temp_hardware_file("rim-pos-rawprint", "ps1");
    fs::write(&script_file, &script)
        .map_err(|error| format!("No se pudo crear script de impresion: {error}"))?;
    let script_path = script_file.to_string_lossy().to_string();
    let args = [
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        script_path.as_str(),
    ];
    let output = command_output("powershell", &args, 8000);
    let _ = fs::remove_file(&script_file);
    let output = output.map_err(|error| format!("No se pudo imprimir raw: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
}

pub fn run_print_file(printer: &str, file: &PathBuf, raw: bool) -> CommandResult<()> {
    let printer = clean_device_text(printer);
    if printer.is_empty() || printer.starts_with("mock-") {
        return Err("Configura una impresora real en Configuracion".into());
    }
    let bytes = fs::read(file).map_err(|error| format!("No se pudo leer impresion: {error}"))?;
    if write_network_device(&printer, &bytes)? {
        return Ok(());
    }
    let direct = printer.starts_with("/dev/")
        || printer.to_ascii_uppercase().starts_with("COM")
        || printer.starts_with("\\\\.\\");
    if direct {
        write_raw_device(&printer, &bytes)?;
        return Ok(());
    }
    #[cfg(windows)]
    {
        if raw {
            // USB/driver spooler queues can't take raw via Out-Printer, so push
            // the ESC/POS bytes straight to the spooler with the RAW datatype.
            return windows_raw_spool_print(&printer, file);
        }
        let path = ps_single_quote(&file.to_string_lossy());
        let printer = ps_single_quote(&printer);
        let command = format!("Get-Content -Raw '{path}' | Out-Printer -Name '{printer}'");
        let output = command_output("powershell", &powershell_args(&command), 5000)
            .map_err(|error| format!("No se pudo imprimir: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    #[cfg(not(windows))]
    {
        // CUPS stops a queue after a failed job (default error policy), which
        // silently holds every later job. Re-enable and un-pause the queue
        // before printing so a past jam doesn't keep new tickets stuck.
        let _ = Command::new("cupsenable").arg(&printer).output();
        let _ = Command::new("cupsaccept").arg(&printer).output();

        let file_path = file.to_string_lossy().to_string();
        let mut command = Command::new("lp");
        if raw {
            command.args(["-o", "raw"]);
        }
        let output = command
            .arg("-d")
            .arg(&printer)
            .arg(&file_path)
            .output()
            .map_err(|error| format!("No se pudo imprimir con lp: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn write_raw_device(device: &str, bytes: &[u8]) -> CommandResult<bool> {
    let device = clean_device_text(device);
    if device.is_empty() || device.starts_with("mock-") {
        return Ok(false);
    }
    if write_network_device(&device, bytes)? {
        return Ok(true);
    }
    let direct = device.starts_with("/dev/")
        || device.to_ascii_uppercase().starts_with("COM")
        || device.starts_with("\\\\.\\");
    if !direct {
        return Ok(false);
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(&device)
        .map_err(|error| format!("No se pudo abrir dispositivo {device}: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("No se pudo escribir a {device}: {error}"))?;
    file.flush()
        .map_err(|error| format!("No se pudo cerrar envio a {device}: {error}"))?;
    Ok(true)
}

pub fn parse_scale_weight(raw: &str) -> Option<f64> {
    let normalized = raw.replace(',', ".");
    normalized
        .split(|character: char| {
            !(character.is_ascii_digit()
                || character == '.'
                || character == '-'
                || character == '+')
        })
        .filter_map(|part| {
            let value = part.trim();
            if value.is_empty() || value == "-" || value == "+" {
                return None;
            }
            value
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
        })
        .last()
}

pub fn read_serial_scale(
    device: &str,
    baud_rate: u32,
    timeout_ms: u64,
) -> CommandResult<(f64, String)> {
    let device = clean_device_text(device);
    if device.is_empty() || device.starts_with("mock-") {
        return Err("Configura una bascula serial real en Configuracion".into());
    }
    let baud_rate = baud_rate.clamp(1200, 115200);
    let timeout_ms = timeout_ms.clamp(200, 5000);

    #[cfg(windows)]
    {
        let device = ps_single_quote(&device);
        let command = format!(
            "$p = New-Object System.IO.Ports.SerialPort('{device}', {baud_rate}, 'None', 8, 'One'); \
             $p.ReadTimeout = {timeout_ms}; \
             try {{ $p.Open(); Start-Sleep -Milliseconds 250; $text = $p.ReadExisting(); \
                    if ([string]::IsNullOrWhiteSpace($text)) {{ try {{ $text = $p.ReadLine() }} catch {{ $text = '' }} }}; \
                    $text }} finally {{ if ($p.IsOpen) {{ $p.Close() }} }}"
        );
        let output = command_output("powershell", &powershell_args(&command), timeout_ms + 1500)
            .map_err(|error| format!("No se pudo leer bascula: {error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let weight = parse_scale_weight(&raw)
            .ok_or_else(|| format!("Bascula no devolvio peso numerico: {raw}"))?;
        return Ok((weight, raw));
    }

    #[cfg(not(windows))]
    {
        let flag = if env::consts::OS == "macos" {
            "-f"
        } else {
            "-F"
        };
        let baud = baud_rate.to_string();
        let time_deciseconds = (timeout_ms / 100).max(1).to_string();
        let _ = Command::new("stty")
            .args([
                flag,
                &device,
                &baud,
                "cs8",
                "-cstopb",
                "-parenb",
                "raw",
                "-echo",
                "time",
                &time_deciseconds,
                "min",
                "0",
            ])
            .output();
        let mut file = fs::OpenOptions::new()
            .read(true)
            .open(&device)
            .map_err(|error| format!("No se pudo abrir bascula {device}: {error}"))?;
        let mut buffer = vec![0_u8; 256];
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("No se pudo leer bascula {device}: {error}"))?;
        let raw = String::from_utf8_lossy(&buffer[..count]).trim().to_string();
        let weight = parse_scale_weight(&raw)
            .ok_or_else(|| format!("Bascula no devolvio peso numerico: {raw}"))?;
        Ok((weight, raw))
    }
}

#[cfg(test)]
mod tests {
    use super::{command_lines, network_endpoint, parse_lpinfo_direct_line, parse_scale_weight};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    // Regression test for a real deadlock: a child process writing more than
    // the OS pipe buffer (~64KB) used to hang forever because nothing drained
    // stdout until the process exited, so it blocked on write() and our
    // polling loop just timed out and killed it -- returning zero lines no
    // matter how much real output there was. This is exactly what made
    // Windows device detection return nothing (not even the always-present
    // "Microsoft Print to PDF"/"Fax" virtual printers) once enough printers
    // made the PowerShell scan's output big enough to fill the pipe.
    #[cfg(not(windows))]
    #[test]
    fn command_output_does_not_deadlock_on_large_output() {
        // `seq 1 20000` prints ~20000 lines, comfortably over 64KB, and exits
        // quickly on its own -- unlike the old code, this must NOT time out.
        let lines = command_lines("seq", &["1", "20000"]);
        assert_eq!(lines.len(), 20000, "expected all lines, not a timeout-truncated empty result");
        assert_eq!(lines.first().map(String::as_str), Some("1"));
        assert_eq!(lines.last().map(String::as_str), Some("20000"));
    }

    #[cfg(windows)]
    #[test]
    fn command_output_does_not_deadlock_on_large_output() {
        // Mirrors the real device-scan shape: a PowerShell pipeline producing
        // enough lines to exceed the pipe buffer, must come back whole.
        let lines = command_lines("powershell", &["-NoProfile", "-Command", "1..20000 | ForEach-Object { $_ }"]);
        assert_eq!(lines.len(), 20000, "expected all lines, not a timeout-truncated empty result");
    }

    #[test]
    fn parses_direct_usb_printer_line() {
        let (uri, name) = parse_lpinfo_direct_line("direct usb://EPSON/TM-T88V?serial=X1Y2Z3").unwrap();
        assert_eq!(uri, "usb://EPSON/TM-T88V?serial=X1Y2Z3");
        assert_eq!(name, "EPSON TM-T88V");
    }

    #[test]
    fn parses_direct_usb_line_with_encoded_spaces() {
        let (uri, name) = parse_lpinfo_direct_line("direct usb://Brother/DCP-L2540DW%20series?serial=ABC").unwrap();
        assert_eq!(uri, "usb://Brother/DCP-L2540DW%20series?serial=ABC");
        assert_eq!(name, "Brother DCP-L2540DW series");
    }

    #[test]
    fn ignores_bare_backend_listings_without_a_live_device() {
        assert_eq!(parse_lpinfo_direct_line("network https"), None);
        assert_eq!(parse_lpinfo_direct_line("network ipp"), None);
        assert_eq!(parse_lpinfo_direct_line("serial serial"), None);
    }

    #[test]
    fn ignores_non_direct_classes_even_with_a_uri() {
        // Already-configured network printers surface through detect_bonjour/
        // detect_network instead; this scan is only for unconfigured USB.
        assert_eq!(
            parse_lpinfo_direct_line("network dnssd://Brother._ipp._tcp.local./?uuid=abc"),
            None
        );
    }

    #[test]
    fn parses_common_scale_output() {
        assert_eq!(parse_scale_weight("ST,GS,+  1.235 kg"), Some(1.235));
        assert_eq!(parse_scale_weight("PESO: 0,750kg"), Some(0.750));
        assert_eq!(parse_scale_weight("sin peso"), None);
    }

    #[test]
    fn parses_network_tcp_endpoint() {
        assert_eq!(
            network_endpoint("tcp://192.168.1.50:9100"),
            Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50)), 9100))
        );
        assert_eq!(network_endpoint("/dev/ttyUSB0"), None);
    }
}
