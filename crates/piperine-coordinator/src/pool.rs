use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use ipc_channel::ipc::IpcOneShotServer;
use piperine_common::{CmdSender, Handshake, RespReceiver};

pub struct ProcessPool {
    workers: Vec<Worker>,
}

pub struct WorkerHandle {
    pub cmd: CmdSender,
    pub resp: RespReceiver,
}

struct Worker {
    child: Child,
    pub handle: WorkerHandle,
    #[allow(dead_code)]
    id: usize,
}

#[derive(Debug)]
pub struct PoolConfig {
    pub size: usize,
    pub worker_binary: Option<PathBuf>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        PoolConfig { size: 4, worker_binary: None }
    }
}

impl ProcessPool {
    pub fn spawn(config: PoolConfig) -> std::io::Result<Self> {
        let worker_path = config.worker_binary.unwrap_or_else(|| {
            // Prefer explicit env var — important when running from a Python extension
            // where current_exe() resolves to the interpreter (e.g. /usr/bin/python3).
            if let Ok(v) = std::env::var("PIPERINE_WORKER") {
                return PathBuf::from(v);
            }
            let mut p = std::env::current_exe().unwrap();
            p.pop();
            p.push("piperine-worker");
            p
        });

        let servers: Vec<(IpcOneShotServer<Handshake>, String)> = (0..config.size)
            .map(|_| IpcOneShotServer::new().map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            }))
            .collect::<Result<_, _>>()?;

        let mut children: Vec<(Child, usize)> = Vec::with_capacity(config.size);
        for (id, (_, name)) in servers.iter().enumerate() {
            let child = Command::new(&worker_path)
                .arg(name)
                .stdin(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()?;
            children.push((child, id));
        }

        let mut workers = Vec::with_capacity(config.size);
        for ((server, _name), (child, id)) in servers.into_iter().zip(children) {
            let (_, (cmd_tx, resp_rx)) = server.accept().map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?;

            eprintln!("coordinator: worker {id} connected");

            workers.push(Worker {
                child,
                handle: WorkerHandle { cmd: cmd_tx, resp: resp_rx },
                id,
            });
        }

        Ok(ProcessPool { workers })
    }

    pub fn handle(&self, idx: usize) -> &WorkerHandle {
        &self.workers[idx].handle
    }

    /// Take ownership of the first worker's handle.
    /// For MVP single-worker use only.
    pub fn take_first(&mut self) -> WorkerHandle {
        self.workers.remove(0).handle
    }

    pub fn len(&self) -> usize {
        self.workers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.workers.is_empty()
    }

    pub fn kill_all(&mut self) {
        for w in &mut self.workers {
            let _ = w.child.kill();
        }
    }
}

impl Drop for ProcessPool {
    fn drop(&mut self) {
        self.kill_all();
        for w in &mut self.workers {
            let _ = w.child.wait();
        }
    }
}
