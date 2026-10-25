#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod net;
mod server;

slint::include_modules!();

use std::cell::RefCell;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;

use server::Event;
use slint::{Model, ModelRc, VecModel};

/// 日志最多保留行数，超出丢弃最早的
const MAX_LOG_ROWS: usize = 500;

thread_local! {
    /// 只在 UI 线程访问，故用 thread_local
    static SERVER: RefCell<Option<server::ServerHandle>> = const { RefCell::new(None) };
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    let adapters = Rc::new(net::list());
    let labels: Vec<slint::SharedString> = adapters
        .iter()
        .map(|a| a.label.clone().into())
        .collect();
    ui.set_adapters(ModelRc::new(VecModel::from(labels)));
    ui.set_adapter_index(0);

    // 默认根目录取启动时的工作目录
    let start_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    ui.set_dir_text(start_dir.display().to_string().into());
    ui.set_hint("选择监听地址和根目录，然后启动服务".into());

    let logs: Rc<VecModel<LogEntry>> = Rc::new(VecModel::default());
    ui.set_logs(ModelRc::from(logs.clone()));

    let (tx, rx) = mpsc::channel::<Event>();

    {
        let weak = ui.as_weak();
        let adapters = Rc::clone(&adapters);
        let logs = Rc::clone(&logs);
        let tx = tx.clone();

        ui.on_toggle_requested(move || {
            let Some(ui) = weak.upgrade() else { return };

            let running = SERVER.with(|c| c.borrow_mut().take());
            if let Some(h) = running {
                h.stop();
                ui.set_running(false);
                ui.set_busy(false);
                ui.set_hint("服务已停止".into());
                ui.set_hint_error(false);
                append(&logs, system_entry("服务已停止".to_string()));
                return;
            }

            let index = ui.get_adapter_index().max(0) as usize;
            let Some(adapter) = adapters.get(index) else {
                ui.set_hint("请选择一个监听地址".into());
                ui.set_hint_error(true);
                return;
            };

            let port = match ui.get_port_text().trim().parse::<u16>() {
                Ok(p) if p > 0 => p,
                _ => {
                    ui.set_hint("端口必须是 1-65535 之间的数字".into());
                    ui.set_hint_error(true);
                    return;
                }
            };

            let dir = PathBuf::from(ui.get_dir_text().trim());
            if !dir.is_dir() {
                ui.set_hint("根目录不存在或不是文件夹".into());
                ui.set_hint_error(true);
                return;
            }

            let addr = SocketAddr::new(adapter.ip, port);
            ui.set_busy(true);
            ui.set_hint(format!("正在启动 {addr} …").into());
            ui.set_hint_error(false);

            match server::start(addr, dir, tx.clone()) {
                Ok(h) => {
                    SERVER.with(|c| *c.borrow_mut() = Some(h));
                }
                Err(err) => {
                    ui.set_busy(false);
                    ui.set_hint(format!("启动失败：{err}").into());
                    ui.set_hint_error(true);
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        ui.on_browse_requested(move || {
            let Some(ui) = weak.upgrade() else { return };
            let current = ui.get_dir_text().to_string();

            let mut dialog = rfd::FileDialog::new().set_title("选择 TFTP 根目录");
            if !current.is_empty() && Path::new(&current).is_dir() {
                dialog = dialog.set_directory(&current);
            }

            if let Some(dir) = dialog.pick_folder() {
                ui.set_dir_text(dir.display().to_string().into());
                ui.set_hint_error(false);
            }
        });
    }

    {
        let logs = Rc::clone(&logs);
        ui.on_clear_requested(move || {
            logs.clear();
        });
    }

    // 事件泵：后台事件转投 UI 线程
    {
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                let weak = weak.clone();
                let posted = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = weak.upgrade() {
                        apply_event(&ui, event);
                    }
                });
                if posted.is_err() {
                    break;
                }
            }
        });
    }

    let result = ui.run();
    // 关窗时停服务，立即释放端口
    SERVER.with(|c| {
        if let Some(h) = c.borrow_mut().take() {
            h.stop();
        }
    });
    result
}

fn apply_event(ui: &AppWindow, event: Event) {
    let logs = ui.get_logs();
    let Some(model) = logs.as_any().downcast_ref::<VecModel<LogEntry>>() else {
        return;
    };
    let mut added = false;

    match event {
        Event::Started(addr) => {
            ui.set_running(true);
            ui.set_busy(false);
            ui.set_hint(format!("正在监听 {addr}（只读模式）").into());
            ui.set_hint_error(false);
            append(model, system_entry(format!("服务已启动，监听 {addr}")));
            added = true;
        }
        Event::Stopped => {
            ui.set_running(false);
            ui.set_busy(false);
        }
        Event::Failed(msg) => {
            let taken = SERVER.with(|c| c.borrow_mut().take());
            if let Some(h) = taken {
                h.stop();
            }
            ui.set_running(false);
            ui.set_busy(false);
            ui.set_hint(format!("出错了：{msg}").into());
            ui.set_hint_error(true);
            append(model, system_entry(format!("服务异常：{msg}")));
            added = true;
        }
        Event::Read { client, file, size } => {
            append(
                model,
                LogEntry {
                    time: now().into(),
                    client: client.ip().to_string().into(),
                    kind: "读取".into(),
                    file: file.into(),
                    size: human_size(size).into(),
                },
            );
            added = true;
        }
        Event::Write { client, file, size } => {
            append(
                model,
                LogEntry {
                    time: now().into(),
                    client: client.ip().to_string().into(),
                    kind: "写入".into(),
                    file: file.into(),
                    size: human_size(size).into(),
                },
            );
            added = true;
        }
        Event::Denied {
            client,
            file,
            reason,
        } => {
            append(
                model,
                LogEntry {
                    time: now().into(),
                    client: client.ip().to_string().into(),
                    kind: "拒绝".into(),
                    file: format!("{file}（{reason}）").into(),
                    size: "—".into(),
                },
            );
            added = true;
        }
    }

    if added {
        let max = ui.get_log_max_scroll();
        if max > 0.0 {
            ui.set_log_scroll(max);
        }
    }
}

fn append(model: &VecModel<LogEntry>, entry: LogEntry) {
    model.push(entry);
    if model.row_count() > MAX_LOG_ROWS {
        model.remove(0);
    }
}

fn system_entry(text: String) -> LogEntry {
    LogEntry {
        time: now().into(),
        client: "—".into(),
        kind: "系统".into(),
        file: text.into(),
        size: "".into(),
    }
}

fn now() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn human_size(size: Option<u64>) -> String {
    match size {
        None => "—".to_string(),
        Some(n) if n < 1024 => format!("{n} B"),
        Some(n) if n < 1024 * 1024 => format!("{:.1} KB", n as f64 / 1024.0),
        Some(n) => format!("{:.1} MB", n as f64 / 1_048_576.0),
    }
}
