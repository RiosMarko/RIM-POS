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
    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|error| format!("{program} fallo al leer salida: {error}"));
            }
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
    }
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
"#;
    for line in command_lines_timeout("powershell", &powershell_args(SCAN_SCRIPT), 6000) {
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
    detect_unix_printers(&mut devices, &mut seen);
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
            return Err("Pulso raw de cajon no soportado por PowerShell. Usa impresora ESC/POS compartida por puerto raw.".into());
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
    use super::{network_endpoint, parse_scale_weight};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

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
