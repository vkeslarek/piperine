use std::env;
use tracing::{error, info};

fn main() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::INFO)
        .try_init();

    let args: Vec<String> = env::args().collect();

    // Worker mode: when invoked with --worker, enter the IPC loop
    if args.iter().any(|a| a == "--worker") {
        if let Err(e) = piperine_ngspice_worker() {
            error!(error = %e, "worker error");
            std::process::exit(1);
        }
        return;
    }

    // Normal mode: demonstrate the API
    info!("Piperine - ergonomic ngspice wrapper");
    info!("Use as a library, or pass --worker for subprocess mode.");
}

fn piperine_ngspice_worker() -> Result<(), Box<dyn std::error::Error>> {
    // The worker_main function is in piperine-ngspice.
    // We can't call it directly here because piperine depends on piperine-pool
    // which depends on piperine-ngspice, but we can re-export through pool.
    // For now, use the pool's re-export or call ngspice directly.

    // Since the binary depends on piperine-pool which depends on piperine-ngspice,
    // we have access to the worker function.
    piperine_pool::worker_main()?;
    Ok(())
}
