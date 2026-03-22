use piperine_spice::{NgspicePool, worker_main};
use std::io;
use tracing::{debug, info};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // If invoked with the worker flag, run worker mode and exit early
    if std::env::args().any(|arg| arg == "--worker") {
        return worker_main();
    }

    // Initialize tracing with default filter from RUST_LOG environment variable
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug")),
        )
        .with_writer(io::stderr)
        .with_target(true)
        .with_level(true)
        .init();

    info!("=== Piperine-Spice Demo ===");

    // Create a pool with 2 workers
    debug!("Creating pool with 2 workers");
    let mut pool = NgspicePool::with_size(2)?;
    info!("Created pool with {} workers", pool.worker_count());

    // Example 1: Simple voltage divider DC analysis
    info!("Running DC analysis on voltage divider...");
    match pool.run_netlist(
        &[
            "Voltage Divider",
            "V1 in 0 DC 10",
            "R1 in out 1k",
            "R2 out 0 1k",
            ".end",
        ],
        "op",
    ) {
        Ok(result) => {
            info!("DC analysis completed successfully");
            info!("Plots: {}", result.plots.len());
            for (name, plot) in &result.plots {
                info!("  Plot '{}': {} vectors", name, plot.vectors.len());
            }
        }
        Err(e) => {
            info!(
                "DC analysis error (expected - worker process may not have started): {}",
                e
            );
        }
    }

    // Example 2: RC circuit transient
    info!("Running transient analysis on RC circuit...");
    match pool.run_netlist(
        &[
            "RC Circuit",
            "V1 in 0 DC 5",
            "R1 in out 1k",
            "C1 out 0 10n",
            ".end",
        ],
        "tran 1n 100n",
    ) {
        Ok(result) => {
            info!("Transient analysis completed successfully");
            info!("Plots: {}", result.plots.len());
        }
        Err(e) => {
            info!(
                "Transient analysis error (expected - worker process may not have started): {}",
                e
            );
        }
    }

    info!("Shutting down pool...");
    pool.shutdown();

    info!("Done!");
    Ok(())
}
