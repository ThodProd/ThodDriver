use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub operation: String,
    pub driver_ids: Vec<String>,
    pub folder: Option<String>,
    pub create_backup: bool,
    pub create_restore_point: bool,
    pub state: String,
    pub logs: Vec<String>,
    pub reboot_required: bool,
    pub error: Option<String>,
}

impl Job {
    pub fn queued(id: String, operation: &str) -> Self {
        Self { id, operation: operation.to_owned(), driver_ids: Vec::new(), folder: None,
            create_backup: true, create_restore_point: true, state: "queued".to_owned(),
            logs: vec!["Задание ожидает подтверждения UAC.".to_owned()], reboot_required: false, error: None }
    }
}
