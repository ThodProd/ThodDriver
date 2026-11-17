//! Одноразовый UAC-помощник. Его запускает только worker после действия пользователя.
#[path = "../drivers.rs"]
mod drivers;
#[path = "../job.rs"]
mod job;
#[path = "../wua.rs"]
mod wua;

use std::path::{Path, PathBuf};

fn write_job(path: &Path, job: &job::Job) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec(job).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    std::fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn update(path: &Path, job: &mut job::Job, message: impl Into<String>) {
    let message = message.into();
    eprintln!("{message}");
    job.logs.push(message);
    let _ = write_job(path, job);
}

fn execute(path: &Path, job: &mut job::Job) -> Result<(), String> {
    job.state = "running".to_owned();
    update(path, job, "Операция запущена с правами администратора.");
    let data_dir = std::env::var_os("THODHUB_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| path.parent().and_then(Path::parent).unwrap_or(Path::new(".")).to_path_buf());
    if job.operation == "installFolder" {
        let folder = job.folder.clone().ok_or_else(|| "Не выбрана папка с INF-файлами.".to_owned())?;
        update(path, job, "Установка драйверов из выбранной папки…");
        let output = drivers::install_inf_folder(&folder)?;
        if !output.is_empty() { update(path, job, output); }
        return Ok(());
    }
    if job.create_restore_point {
        update(path, job, "Создание точки восстановления…");
        if let Err(error) = drivers::restore_point() { update(path, job, format!("Точка восстановления не создана: {error}")); }
    }
    if job.create_backup {
        update(path, job, "Создание резервной копии драйверов…");
        match drivers::backup_all(&data_dir) { Ok(folder) => update(path, job, format!("Резервная копия сохранена: {folder}")), Err(error) => update(path, job, format!("Резервная копия не создана: {error}")) }
    }
    let path_for_log = path.to_path_buf();
    let mut current = job.clone();
    let mut log = |message: String| { current.logs.push(message); let _ = write_job(&path_for_log, &current); };
    let reboot_required = wua::install(&job.driver_ids, &mut log).map_err(|error| error.to_string())?;
    *job = current;
    job.reboot_required = reboot_required;
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(position) = args.iter().position(|arg| arg == "--job") else { eprintln!("Нужен параметр --job."); return; };
    let Some(path) = args.get(position + 1).map(PathBuf::from) else { eprintln!("Не указан файл задания."); return; };
    let bytes = match std::fs::read(&path) { Ok(bytes) => bytes, Err(error) => { eprintln!("Не удалось прочитать задание: {error}"); return; } };
    let mut job: job::Job = match serde_json::from_slice(&bytes) { Ok(job) => job, Err(error) => { eprintln!("Некорректное задание: {error}"); return; } };
    match execute(&path, &mut job) {
        Ok(()) => { job.state = "completed".to_owned(); let message = if job.reboot_required { "Готово. Требуется перезагрузка Windows." } else { "Готово." }; update(&path, &mut job, message); }
        Err(error) => { job.state = "failed".to_owned(); job.error = Some(error.clone()); update(&path, &mut job, format!("Ошибка: {error}")); }
    }
}
