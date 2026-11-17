#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod drivers;
mod wua;

use eframe::egui::{self, Color32, RichText, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;

const ACCENT: Color32 = Color32::from_rgb(54, 118, 232);
const MUTED: Color32 = Color32::from_rgb(118, 123, 133);

enum Message {
    Scan { drivers: Vec<wua::Candidate>, installed: usize },
    Log(String),
    Finished { reboot: bool },
    Failed(String),
}

#[derive(Clone, Copy, PartialEq)]
enum State { Idle, Scanning, Ready, Installing, Done }

struct App {
    state: State,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    candidates: Vec<wua::Candidate>,
    installed_count: usize,
    logs: Vec<String>,
    backup: bool,
    restore_point: bool,
    inf_folder: String,
    reboot: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = Color32::WHITE;
        visuals.window_fill = Color32::WHITE;
        visuals.selection.bg_fill = ACCENT;
        cc.egui_ctx.set_visuals(visuals);
        let (tx, rx) = channel();
        Self { state: State::Idle, tx, rx, candidates: Vec::new(), installed_count: 0, logs: Vec::new(), backup: true, restore_point: true, inf_folder: String::new(), reboot: false }
    }

    fn data_dir() -> PathBuf {
        std::env::var_os("THODHUB_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("ThodHUB").join("Data").join("thoddriver")
        })
    }

    fn scan(&mut self) {
        self.state = State::Scanning; self.candidates.clear(); self.logs.push("Поиск обновлений через Windows Update…".to_owned());
        let tx = self.tx.clone();
        thread::spawn(move || {
            let installed = drivers::installed().map(|list| list.len()).unwrap_or(0);
            match wua::scan() { Ok(drivers) => { let _ = tx.send(Message::Scan { drivers, installed }); }, Err(error) => { let _ = tx.send(Message::Failed(format!("Windows Update: {error}"))); } }
        });
    }

    fn install(&mut self) {
        let ids: Vec<String> = self.candidates.iter().filter(|driver| driver.selected).map(|driver| driver.id.clone()).collect();
        if ids.is_empty() { return; }
        self.state = State::Installing; self.logs.push("Установка запущена с правами администратора.".to_owned());
        let tx = self.tx.clone(); let backup = self.backup; let restore_point = self.restore_point; let data_dir = Self::data_dir();
        thread::spawn(move || {
            let log = |message: String| { let _ = tx.send(Message::Log(message)); };
            if restore_point { log("Создание точки восстановления…".to_owned()); if let Err(error) = drivers::restore_point() { log(format!("Точка восстановления не создана: {error}")); } }
            if backup { log("Создание резервной копии драйверов…".to_owned()); match drivers::backup_all(&data_dir) { Ok(path) => log(format!("Резервная копия: {path}")), Err(error) => log(format!("Резервная копия не создана: {error}")) } }
            let mut progress = |message: String| log(message);
            match wua::install(&ids, &mut progress) { Ok(reboot) => { let _ = tx.send(Message::Finished { reboot }); }, Err(error) => { let _ = tx.send(Message::Failed(format!("Установка: {error}"))); } }
        });
    }

    fn install_folder(&mut self) {
        let folder = self.inf_folder.trim().to_owned(); if folder.is_empty() { return; }
        self.state = State::Installing; let tx = self.tx.clone();
        thread::spawn(move || { match drivers::install_inf_folder(&folder) { Ok(output) => { let _ = tx.send(Message::Log(output)); let _ = tx.send(Message::Finished { reboot: false }); }, Err(error) => { let _ = tx.send(Message::Failed(format!("Установка INF: {error}"))); } } });
    }

    fn poll(&mut self) {
        while let Ok(message) = self.rx.try_recv() { match message {
            Message::Scan { drivers, installed } => { self.installed_count = installed; self.logs.push(format!("Найдено обновлений: {}", drivers.len())); self.candidates = drivers; self.state = State::Ready; }
            Message::Log(message) => self.logs.push(message),
            Message::Finished { reboot } => { self.reboot = reboot; self.state = State::Done; self.logs.push("Операция завершена.".to_owned()); }
            Message::Failed(error) => { self.logs.push(format!("Ошибка: {error}")); self.state = State::Ready; }
        }}
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll();
        let busy = matches!(self.state, State::Scanning | State::Installing);
        if busy { ctx.request_repaint_after(std::time::Duration::from_millis(100)); }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| { ui.vertical(|ui| {
                ui.label(RichText::new("Драйверы").size(28.0).strong());
                let status = match self.state { State::Idle => "Проверьте обновления драйверов через Windows Update.", State::Scanning => "Windows Update ищет доступные драйверы…", State::Ready if self.candidates.is_empty() => "Подходящих обновлений не найдено.", State::Ready => "Выберите обновления для установки.", State::Installing => "Скачивание и установка драйверов…", State::Done if self.reboot => "Готово. Требуется перезагрузка Windows.", State::Done => "Готово." };
                ui.label(RichText::new(status).color(MUTED));
            }); ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { if ui.add_enabled(!busy, egui::Button::new("Проверить обновления").fill(ACCENT).min_size(Vec2::new(180.0, 36.0))).clicked() { self.scan(); } }); });
            ui.add_space(18.0); ui.separator(); ui.add_space(8.0);
            ui.horizontal(|ui| { ui.checkbox(&mut self.restore_point, "Создать точку восстановления"); ui.checkbox(&mut self.backup, "Сохранить резервную копию"); if ui.button("Выбрать все").clicked() { self.candidates.iter_mut().for_each(|driver| driver.selected = true); } if ui.button("Снять выбор").clicked() { self.candidates.iter_mut().for_each(|driver| driver.selected = false); } });
            ui.add_space(8.0);
            egui::ScrollArea::vertical().max_height(290.0).show(ui, |ui| {
                if self.candidates.is_empty() { ui.vertical_centered(|ui| { ui.add_space(64.0); ui.label(RichText::new("После проверки здесь появятся обновления драйверов.").color(MUTED)); }); }
                for driver in &mut self.candidates { ui.group(|ui| { ui.set_min_width(ui.available_width()); ui.horizontal(|ui| { ui.checkbox(&mut driver.selected, ""); ui.vertical(|ui| { ui.label(RichText::new(&driver.title).strong()); ui.label(RichText::new([driver.class.as_str(), driver.vendor.as_str(), driver.date.as_str()].into_iter().filter(|text| !text.is_empty()).collect::<Vec<_>>().join(" · ")).color(MUTED)); }); }); }); ui.add_space(4.0); }
            });
            ui.separator(); ui.add_space(8.0);
            ui.horizontal(|ui| { ui.label("Папка с INF:"); ui.add_sized([270.0, 26.0], egui::TextEdit::singleline(&mut self.inf_folder).hint_text("C:\\Drivers")); if ui.add_enabled(!busy && !self.inf_folder.trim().is_empty(), egui::Button::new("Установить из папки")).clicked() { self.install_folder(); } });
            ui.add_space(8.0); let selected = self.candidates.iter().filter(|driver| driver.selected).count(); ui.horizontal(|ui| { ui.label(RichText::new(format!("Установлено драйверов: {}", self.installed_count)).color(MUTED)); ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { if ui.add_enabled(!busy && selected > 0, egui::Button::new(format!("Установить выбранное ({selected})")).fill(ACCENT).min_size(Vec2::new(210.0, 36.0))).clicked() { self.install(); } }); });
            ui.add_space(8.0); egui::CollapsingHeader::new("Журнал операции").show(ui, |ui| { egui::ScrollArea::vertical().max_height(130.0).show(ui, |ui| { for line in &self.logs { ui.label(RichText::new(line).monospace()); } }); });
        });
    }
}

fn icon() -> Arc<egui::IconData> {
    let size = 32; let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size { for x in 0..size { let pixel = (y * size + x) * 4; let inside = x > 6 && x < 26 && y > 5 && y < 27; let color = if inside { [54, 118, 232, 255] } else { [242, 246, 252, 255] }; rgba[pixel..pixel + 4].copy_from_slice(&color); } }
    Arc::new(egui::IconData { rgba, width: size as u32, height: size as u32 })
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_title("ThodDriver").with_inner_size([900.0, 640.0]).with_min_inner_size([720.0, 520.0]).with_icon(icon()), ..Default::default() };
    eframe::run_native("ThodDriver", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
