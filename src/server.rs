//! TFTP 服务端封装。
//!
//! 后台线程里跑一个双工作线程的 tokio 运行时，用自定义 `Handler` 把每次请求
//! 上报给界面。服务端固定为只读：这台工具的作用是把文件发给设备，允许写入只
//! 会增加误覆盖根目录的风险。

use async_tftp::packet::Error as TftpError;
use async_tftp::server::{Handler, TftpServerBuilder};
use blocking::Unblock;
use std::fs::File;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum Event {
    Read {
        client: SocketAddr,
        file: String,
        size: Option<u64>,
    },
    Write {
        client: SocketAddr,
        file: String,
        size: Option<u64>,
    },
    Denied {
        client: SocketAddr,
        file: String,
        reason: &'static str,
    },
    Started(SocketAddr),
    Stopped,
    Failed(String),
}

struct LogHandler {
    root: PathBuf,
    tx: Sender<Event>,
}

impl LogHandler {
    /// 把客户端的相对路径安全地拼到根目录下，任何越界成分直接拒绝。
    fn resolve(&self, path: &Path) -> Option<PathBuf> {
        let mut full = self.root.clone();
        for comp in path.components() {
            match comp {
                Component::Normal(seg) => full.push(seg),
                Component::CurDir => {}
                _ => return None,
            }
        }
        Some(full)
    }
}

impl Handler for LogHandler {
    type Reader = Unblock<File>;
    type Writer = Unblock<File>;

    async fn read_req_open(
        &mut self,
        client: &SocketAddr,
        path: &Path,
    ) -> Result<(Self::Reader, Option<u64>), TftpError> {
        let name = path.display().to_string();

        let Some(full) = self.resolve(path) else {
            let _ = self.tx.send(Event::Denied {
                client: *client,
                file: name,
                reason: "非法路径",
            });
            return Err(TftpError::PermissionDenied);
        };

        let file = match File::open(&full) {
            Ok(f) => f,
            Err(err) => {
                let (reason, tftp_err) = match err.kind() {
                    std::io::ErrorKind::NotFound => ("文件不存在", TftpError::FileNotFound),
                    std::io::ErrorKind::PermissionDenied => {
                        ("无权读取", TftpError::PermissionDenied)
                    }
                    _ => ("打开失败", TftpError::UnknownError),
                };
                let _ = self.tx.send(Event::Denied {
                    client: *client,
                    file: name,
                    reason,
                });
                return Err(tftp_err);
            }
        };

        let size = file.metadata().ok().map(|m| m.len());
        let _ = self.tx.send(Event::Read {
            client: *client,
            file: name,
            size,
        });
        Ok((Unblock::new(file), size))
    }

    async fn write_req_open(
        &mut self,
        client: &SocketAddr,
        path: &Path,
        size: Option<u64>,
    ) -> Result<Self::Writer, TftpError> {
        let name = path.display().to_string();
        let _ = self.tx.send(Event::Write {
            client: *client,
            file: name.clone(),
            size,
        });
        let _ = self.tx.send(Event::Denied {
            client: *client,
            file: name,
            reason: "只读模式",
        });
        Err(TftpError::PermissionDenied)
    }
}

pub struct ServerHandle {
    shutdown: Option<oneshot::Sender<()>>,
    runtime: Option<Runtime>,
}

impl ServerHandle {
    pub fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(rt) = self.runtime.take() {
            rt.shutdown_timeout(Duration::from_millis(400));
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        // 关键：必须先通知任务退出，否则 Runtime 析构会等一个永不结束的 serve
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

pub fn start(addr: SocketAddr, root: PathBuf, tx: Sender<Event>) -> Result<ServerHandle, String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .thread_name("tftp-io")
        .build()
        .map_err(|e| format!("创建网络线程失败：{e}"))?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let tx_task = tx.clone();

    runtime.spawn(async move {
        let handler = LogHandler {
            root,
            tx: tx_task.clone(),
        };

        let server = match TftpServerBuilder::with_handler(handler)
            .bind(addr)
            .timeout(Duration::from_secs(1))
            .build()
            .await
        {
            Ok(s) => s,
            Err(err) => {
                let _ = tx_task.send(Event::Failed(format!("{err}")));
                return;
            }
        };

        if let Ok(bound) = server.listen_addr() {
            let _ = tx_task.send(Event::Started(bound));
        }

        tokio::select! {
            result = server.serve() => {
                if let Err(err) = result {
                    let _ = tx_task.send(Event::Failed(format!("{err}")));
                }
            }
            _ = shutdown_rx => {}
        }

        let _ = tx_task.send(Event::Stopped);
    });

    Ok(ServerHandle {
        shutdown: Some(shutdown_tx),
        runtime: Some(runtime),
    })
}
