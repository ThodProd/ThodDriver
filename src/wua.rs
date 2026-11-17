//! Windows Update Agent (COM): поиск, скачивание и установка драйверов
//! из каталога Microsoft Update.
use serde::Serialize;
use windows::core::{Interface, BSTR};
use windows::Win32::System::Com::*;
use windows::Win32::System::UpdateAgent::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub title: String,
    pub class: String,
    pub vendor: String,
    pub hwid: String,
    pub date: String,
    pub selected: bool,
}

struct Com;
impl Com {
    fn new() -> Self {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        Com
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}

const CRITERIA: &str = "IsInstalled=0 and Type='Driver' and IsHidden=0";

fn search(session: &IUpdateSession) -> windows::core::Result<ISearchResult> {
    unsafe {
        let s = session.CreateUpdateSearcher()?;
        s.Search(&BSTR::from(CRITERIA))
    }
}

/// OLE DATE (дни с 1899-12-30) -> "ГГГГ-ММ-ДД"
fn ole_date(d: f64) -> String {
    let days_from_1970 = d.floor() as i64 - 25_569;
    let z = days_from_1970 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{day:02}")
}

pub fn scan() -> windows::core::Result<Vec<Candidate>> {
    let _com = Com::new();
    unsafe {
        let session: IUpdateSession = CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER)?;
        let res = search(&session)?;
        let updates = res.Updates()?;
        let mut out = Vec::new();
        for i in 0..updates.Count()? {
            let u = updates.get_Item(i)?;
            let id = u.Identity()?.UpdateID()?.to_string();
            let title = u.Title()?.to_string();
            let (mut class, mut vendor, mut hwid, mut date) =
                (String::new(), String::new(), String::new(), String::new());
            if let Ok(d) = u.cast::<IWindowsDriverUpdate>() {
                class = d.DriverClass().map(|s| s.to_string()).unwrap_or_default();
                vendor = d.DriverManufacturer().map(|s| s.to_string()).unwrap_or_default();
                hwid = d.DriverHardwareID().map(|s| s.to_string()).unwrap_or_default();
                date = d.DriverVerDate().map(ole_date).unwrap_or_default();
            }
            out.push(Candidate { id, title, class, vendor, hwid, date, selected: true });
        }
        Ok(out)
    }
}

/// Скачивает и устанавливает выбранные драйверы. Возвращает true, если нужна перезагрузка.
pub fn install(ids: &[String], log: &dyn Fn(String)) -> windows::core::Result<bool> {
    let _com = Com::new();
    unsafe {
        let session: IUpdateSession = CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER)?;
        let res = search(&session)?;
        let updates = res.Updates()?;
        let coll: IUpdateCollection = CoCreateInstance(&UpdateCollection, None, CLSCTX_INPROC_SERVER)?;
        for i in 0..updates.Count()? {
            let u = updates.get_Item(i)?;
            let id = u.Identity()?.UpdateID()?.to_string();
            if ids.contains(&id) {
                if !u.EulaAccepted()?.as_bool() {
                    u.AcceptEula()?;
                }
                coll.Add(&u)?;
            }
        }
        let n = coll.Count()?;
        if n == 0 {
            log("Нет выбранных драйверов".into());
            return Ok(false);
        }
        log(format!("Скачивание драйверов: {n} шт. …"));
        let dl = session.CreateUpdateDownloader()?;
        dl.SetUpdates(&coll)?;
        let dr = dl.Download()?;
        log(format!("Скачивание завершено, код {}", dr.ResultCode()?.0));

        log("Установка …".into());
        let inst = session.CreateUpdateInstaller()?;
        inst.SetUpdates(&coll)?;
        let ir = inst.Install()?;
        for i in 0..n {
            let title = coll.get_Item(i)?.Title()?.to_string();
            let code = ir.GetUpdateResult(i)?.ResultCode()?.0;
            // 2 = orcSucceeded, 3 = SucceededWithErrors
            log(format!("{} — {}", title, if code == 2 { "установлен" } else { "ошибка" }));
        }
        Ok(ir.RebootRequired()?.as_bool())
    }
}
