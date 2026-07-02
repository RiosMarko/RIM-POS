use chrono::Utc;
use serde::Serialize;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;

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

fn command_lines(program: &str, args: &[&str]) -> Vec<String> {
    let Ok(output) = Command::new(program).args(args).output() else {
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

fn detect_windows_printers(devices: &mut Vec<HardwareDevice>, seen: &mut HashSet<String>) {
    let command = "Get-Printer | ForEach-Object { \"$($_.Name)|$($_.DriverName)|$($_.PortName)\" }";
    for line in command_lines("powershell", &["-NoProfile", "-Command", command]) {
        let parts: Vec<&str> = line.split('|').collect();
        let name = clean_device_text(parts.first().copied().unwrap_or(""));
        if name.is_empty() {
            continue;
        }
        let driver = clean_device_text(parts.get(1).copied().unwrap_or(""));
        let port = clean_device_text(parts.get(2).copied().unwrap_or(""));
        add_device(
            devices,
            seen,
            HardwareDevice {
                id: name.clone(),
                name,
                device_type: "printer".into(),
                connection: "windows-printer".into(),
                detail: format!("{driver} {port}").trim().into(),
                is_default: false,
            },
        );
    }
}

fn detect_serial_paths(devices: &mut Vec<HardwareDevice>, seen: &mut HashSet<String>) {
    #[cfg(windows)]
    {
        let command =
            "Get-CimInstance Win32_SerialPort | ForEach-Object { \"$($_.DeviceID)|$($_.Name)\" }";
        for line in command_lines("powershell", &["-NoProfile", "-Command", command]) {
            let parts: Vec<&str> = line.split('|').collect();
            let id = clean_device_text(parts.first().copied().unwrap_or(""));
            if id.is_empty() {
                continue;
            }
            let name = clean_device_text(parts.get(1).copied().unwrap_or(&id));
            add_device(
                devices,
                seen,
                HardwareDevice {
                    id: id.clone(),
                    name,
                    device_type: "serial".into(),
                    connection: "serial".into(),
                    detail: "Puerto serial local".into(),
                    is_default: false,
                },
            );
        }
    }

    #[cfg(not(windows))]
    {
        let prefixes = [
            "ttyUSB", "ttyACM", "tty.usb", "cu.usb", "ttyS", "cu.SLAB", "tty.SLAB",
        ];
        if let Ok(entries) = fs::read_dir("/dev") {
            for entry in entries.flatten() {
                let filename = entry.file_name().to_string_lossy().to_string();
                if !prefixes.iter().any(|prefix| filename.starts_with(prefix)) {
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

pub fn device_list() -> Vec<HardwareDevice> {
    let mut devices = Vec::new();
    let mut seen = HashSet::new();
    if env::consts::OS == "windows" {
        detect_windows_printers(&mut devices, &mut seen);
    } else {
        detect_unix_printers(&mut devices, &mut seen);
    }
    detect_serial_paths(&mut devices, &mut seen);

    if devices.is_empty() {
        add_device(
            &mut devices,
            &mut seen,
            HardwareDevice {
                id: "mock-printer-80mm".into(),
                name: "Mock 80mm".into(),
                device_type: "printer".into(),
                connection: "mock".into(),
                detail: "Dispositivo de prueba".into(),
                is_default: true,
            },
        );
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
    #[cfg(windows)]
    {
        if raw {
            return Err("Pulso raw de cajon no soportado por PowerShell. Usa impresora ESC/POS compartida por puerto raw.".into());
        }
        let path = ps_single_quote(&file.to_string_lossy());
        let printer = ps_single_quote(&printer);
        let command = format!("Get-Content -Raw '{path}' | Out-Printer -Name '{printer}'");
        let output = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(command)
            .output()
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
        let output = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(command)
            .output()
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
    use super::parse_scale_weight;

    #[test]
    fn parses_common_scale_output() {
        assert_eq!(parse_scale_weight("ST,GS,+  1.235 kg"), Some(1.235));
        assert_eq!(parse_scale_weight("PESO: 0,750kg"), Some(0.750));
        assert_eq!(parse_scale_weight("sin peso"), None);
    }
}
