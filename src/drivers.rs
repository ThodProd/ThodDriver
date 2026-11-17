//! Работа с установленными драйверами и системными утилитами Windows.
use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct InstalledDriver { pub device_name: Option<String>, pub manufacturer: Option<String>, pub driver_version: Option<String>, pub driver_date: Option<String> }

pub fn powershell(script: &str) -> Result<String, String> {
    let full = format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8; $ProgressPreference='SilentlyContinue'; {script}");
    let output = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &full]).creation_flags(CREATE_NO_WINDOW).output().map_err(|error| error.to_string())?;
    if output.status.success() { Ok(String::from_utf8_lossy(&output.stdout).to_string()) } else { Err(String::from_utf8_lossy(&output.stderr).trim().to_owned()) }
}

pub fn installed() -> Result<Vec<InstalledDriver>, String> {
    let json = powershell("ConvertTo-Json -Depth 2 -InputObject @(Get-CimInstance Win32_PnPSignedDriver | Where-Object { $_.DeviceName } | Select-Object DeviceName,Manufacturer,DriverVersion,@{n='DriverDate';e={ if($_.DriverDate){ $_.DriverDate.ToString('yyyy-MM-dd') } }})")?;
    serde_json::from_str(json.trim()).map_err(|error| error.to_string())
}

pub fn backup_all(data_dir: &Path) -> Result<String, String> {
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|duration| duration.as_secs()).unwrap_or(0);
    let directory = data_dir.join("backup").join(format!("drivers-{timestamp}"));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let status = Command::new("pnputil").args(["/export-driver", "*", &directory.to_string_lossy()]).creation_flags(CREATE_NO_WINDOW).status().map_err(|error| error.to_string())?;
    if status.success() { Ok(directory.display().to_string()) } else { Err("pnputil не смог экспортировать драйверы".to_owned()) }
}

pub fn restore_point() -> Result<(), String> {
    powershell("Enable-ComputerRestore -Drive $env:SystemDrive -ErrorAction SilentlyContinue; Checkpoint-Computer -Description 'ThodDriver' -RestorePointType MODIFY_SETTINGS").map(|_| ())
}

pub fn install_inf_folder(path: &str) -> Result<String, String> {
    let output = Command::new("pnputil").args(["/add-driver", &format!("{path}\\*.inf"), "/subdirs", "/install"]).creation_flags(CREATE_NO_WINDOW).output().map_err(|error| error.to_string())?;
    if output.status.success() { Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned()) } else { Err(String::from_utf8_lossy(&output.stderr).trim().to_owned()) }
}
