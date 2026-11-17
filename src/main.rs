//! ThodDriver worker: компактный JSON-RPC 2.0 по stdin/stdout для ThodHUB.
mod drivers;
mod job;
mod wua;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_LINE: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Request { jsonrpc: String, id: String, method: String, #[serde(default)] params: Value }
#[derive(Serialize)]
struct Response<'a> { jsonrpc: &'a str, id: String, #[serde(skip_serializing_if = "Option::is_none")] result: Option<Value>, #[serde(skip_serializing_if = "Option::is_none")] error: Option<RpcError> }
#[derive(Serialize)]
struct RpcError { code: i32, message: String }

fn data_dir() -> PathBuf { std::env::var_os("THODHUB_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("ThodDriver")) }
fn job_path(data_dir: &Path, id: &str) -> PathBuf { data_dir.join("runtime").join(format!("{id}.json")) }
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec(value).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    std::fs::rename(temporary, path).map_err(|error| error.to_string())
}
fn read_job(path: &Path) -> Result<job::Job, String> { serde_json::from_slice(&std::fs::read(path).map_err(|error| error.to_string())?).map_err(|error| error.to_string()) }
fn create_id() -> String { format!("job-{}-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|duration| duration.as_millis()).unwrap_or(0), std::process::id()) }
fn start_elevated(helper: &Path, job: &Path) -> Result<(), String> {
    let status = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", "Start-Process -FilePath $args[0] -ArgumentList @('--job', $args[1]) -Verb RunAs", &helper.to_string_lossy(), &job.to_string_lossy()]).status().map_err(|error| error.to_string())?;
    if status.success() { Ok(()) } else { Err("Не удалось запросить права администратора.".to_owned()) }
}
fn start_job(params: &Value, data_dir: &Path) -> Result<Value, String> {
    let operation = params.get("operation").and_then(Value::as_str).unwrap_or("install");
    if operation != "install" && operation != "installFolder" { return Err("Неизвестная операция.".to_owned()); }
    let mut job = job::Job::queued(create_id(), operation);
    job.driver_ids = params.get("driverIds").and_then(Value::as_array).map(|values| values.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default();
    job.folder = params.get("folder").and_then(Value::as_str).map(str::to_owned);
    job.create_backup = params.get("createBackup").and_then(Value::as_bool).unwrap_or(true);
    job.create_restore_point = params.get("createRestorePoint").and_then(Value::as_bool).unwrap_or(true);
    if operation == "install" && job.driver_ids.is_empty() { return Err("Не выбраны драйверы для установки.".to_owned()); }
    if operation == "installFolder" && job.folder.as_deref().map(str::is_empty).unwrap_or(true) { return Err("Не выбрана папка с INF-файлами.".to_owned()); }
    let path = job_path(data_dir, &job.id); write_json(&path, &job)?;
    let helper = std::env::current_exe().map_err(|error| error.to_string())?.parent().ok_or_else(|| "Не найдена папка worker.".to_owned())?.join("thod-driver-helper.exe");
    if !helper.is_file() { return Err("Не найден помощник установки.".to_owned()); }
    if let Err(error) = start_elevated(&helper, &path) { job.state = "failed".to_owned(); job.error = Some(error.clone()); job.logs.push(error.clone()); let _ = write_json(&path, &job); return Err(error); }
    Ok(json!({"jobId": job.id}))
}
fn handle(method: &str, params: &Value, data_dir: &Path) -> Result<Value, String> {
    match method.strip_prefix("worker.").unwrap_or(method) {
        "scan" => Ok(serde_json::to_value(wua::scan().map_err(|error| error.to_string())?).map_err(|error| error.to_string())?),
        "installed" => Ok(serde_json::to_value(drivers::installed()?).map_err(|error| error.to_string())?),
        "job.start" => start_job(params, data_dir),
        "job.status" => { let id = params.get("jobId").and_then(Value::as_str).ok_or_else(|| "Не указан jobId.".to_owned())?; Ok(serde_json::to_value(read_job(&job_path(data_dir, id))?).map_err(|error| error.to_string())?) }
        _ => Err("Неизвестный метод worker.".to_owned()),
    }
}
fn main() {
    let data_dir = data_dir(); if let Err(error) = std::fs::create_dir_all(data_dir.join("runtime")) { eprintln!("Нельзя создать папку данных: {error}"); return; }
    let stdin = io::stdin(); let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let response = match line {
            Ok(line) if line.len() <= MAX_LINE => match serde_json::from_str::<Request>(&line) {
                Ok(request) if request.jsonrpc == "2.0" => match handle(&request.method, &request.params, &data_dir) { Ok(result) => Response { jsonrpc: "2.0", id: request.id, result: Some(result), error: None }, Err(message) => Response { jsonrpc: "2.0", id: request.id, result: None, error: Some(RpcError { code: -32000, message }) } },
                Ok(request) => Response { jsonrpc: "2.0", id: request.id, result: None, error: Some(RpcError { code: -32600, message: "Неверная версия JSON-RPC.".to_owned() }) },
                Err(error) => Response { jsonrpc: "2.0", id: String::new(), result: None, error: Some(RpcError { code: -32700, message: error.to_string() }) },
            },
            Ok(_) => Response { jsonrpc: "2.0", id: String::new(), result: None, error: Some(RpcError { code: -32600, message: "Слишком длинный запрос.".to_owned() }) },
            Err(_) => break,
        };
        if let Ok(json) = serde_json::to_string(&response) { let _ = writeln!(stdout, "{json}"); let _ = stdout.flush(); }
    }
}
