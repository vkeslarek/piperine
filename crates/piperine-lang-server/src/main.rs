//! Piperine Language Server — LSP server for PHDL.
//!
//! Implements the Language Server Protocol over stdio using `lsp-server`
//! (the rust-analyzer stack). Provides diagnostics, hover, completion,
//! go-to-definition, and document symbols for `.phdl` files.

mod dispatch;
mod handlers;
mod server;
mod state;

use lsp_server::{Connection, Message};

use server::LanguageServer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("piperine-lang-server starting");

    let (connection, io_threads) = Connection::stdio();

    let capabilities = serde_json::to_value(server::server_capabilities())?;

    let _init_params = connection.initialize(capabilities)?;
    eprintln!("piperine-lang-server initialized");

    let mut server = LanguageServer::new(&connection);

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    eprintln!("shutting down");
                    break;
                }
                server.handle_request(req, &connection);
            }
            Message::Notification(not) => {
                server.handle_notification(not, &connection);
            }
            Message::Response(_resp) => {}
        }
    }

    io_threads.join()?;
    eprintln!("piperine-lang-server stopped");
    Ok(())
}
