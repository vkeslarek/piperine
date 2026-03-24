//! Worker pool for managing ngspice worker processes.

use crate::ipc;
use piperine_ngspice::protocol::*;
use std::io::{self, BufReader, BufWriter};
use std::process::{Child, Command, Stdio};

/// A handle to a single worker process.
pub struct WorkerHandle {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    writer: BufWriter<std::process::ChildStdin>,
}

impl WorkerHandle {
    /// Spawn a new worker process.
    fn spawn(exe_path: &str) -> io::Result<Self> {
        let mut child = Command::new(exe_path)
            .arg("--worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take().expect("failed to get stdin");
        let stdout = child.stdout.take().expect("failed to get stdout");

        let mut handle = Self {
            child,
            reader: BufReader::new(stdout),
            writer: BufWriter::new(stdin),
        };

        // Wait for Ready signal
        let msg: WorkerToMain = ipc::read_msg(&mut handle.reader)?;
        match msg {
            WorkerToMain::Ready => Ok(handle),
            WorkerToMain::Error { message } => {
                Err(io::Error::new(io::ErrorKind::Other, format!("worker init failed: {message}")))
            }
            other => {
                Err(io::Error::new(io::ErrorKind::Other, format!("unexpected message: {other:?}")))
            }
        }
    }

    /// Send a message to the worker.
    pub fn send(&mut self, msg: &MainToWorker) -> io::Result<()> {
        ipc::write_msg(&mut self.writer, msg)
    }

    /// Read a message from the worker.
    pub fn recv(&mut self) -> io::Result<WorkerToMain> {
        ipc::read_msg(&mut self.reader)
    }

    /// Drive a simulation, handling external source callbacks.
    pub fn drive_simulation(
        &mut self,
        handler: Option<&dyn piperine_api::engine::ExternalSourceHandler>,
    ) -> io::Result<WorkerToMain> {
        loop {
            let msg = self.recv()?;
            match msg {
                WorkerToMain::SimulationComplete { .. } => return Ok(msg),
                WorkerToMain::Error { .. } => return Ok(msg),
                WorkerToMain::ExternalSourceRequest { request_id, source_name, time } => {
                    let value = handler
                        .map(|h| h.get_value(&source_name, time))
                        .unwrap_or(0.0);
                    self.send(&MainToWorker::ExternalSourceValue { request_id, value })?;
                }
                WorkerToMain::Ok | WorkerToMain::Ready => {
                    // Unexpected during simulation, continue waiting
                }
            }
        }
    }

    /// Shut down the worker process.
    pub fn shutdown(mut self) -> io::Result<()> {
        let _ = self.send(&MainToWorker::Shutdown);
        let _ = self.child.wait();
        Ok(())
    }
}

/// Pool of worker processes for parallel simulation.
pub struct WorkerPool {
    workers: Vec<Option<WorkerHandle>>,
    exe_path: String,
}

impl WorkerPool {
    /// Create a new pool with the given number of workers.
    pub fn new(exe_path: &str, num_workers: usize) -> io::Result<Self> {
        let mut workers = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            workers.push(Some(WorkerHandle::spawn(exe_path)?));
        }
        Ok(Self {
            workers,
            exe_path: exe_path.to_string(),
        })
    }

    /// Get an available worker (takes it from the pool temporarily).
    pub fn take_worker(&mut self) -> io::Result<(usize, WorkerHandle)> {
        for (i, slot) in self.workers.iter_mut().enumerate() {
            if slot.is_some() {
                return Ok((i, slot.take().unwrap()));
            }
        }
        // All workers busy, spawn a temporary one
        let w = WorkerHandle::spawn(&self.exe_path)?;
        self.workers.push(None);
        let idx = self.workers.len() - 1;
        Ok((idx, w))
    }

    /// Return a worker to the pool.
    pub fn return_worker(&mut self, idx: usize, worker: WorkerHandle) {
        if idx < self.workers.len() {
            self.workers[idx] = Some(worker);
        }
    }

    /// Number of workers in the pool.
    pub fn size(&self) -> usize {
        self.workers.len()
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        for slot in self.workers.drain(..) {
            if let Some(worker) = slot {
                let _ = worker.shutdown();
            }
        }
    }
}
