//! Piperine Language Server — LSP server for PHDL.

use lsp_server::Connection;
use piperine_lang_server::server::{LanguageServer, server_capabilities};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (connection, io_threads) = Connection::stdio();
    let capabilities = serde_json::to_value(server_capabilities())?;
    
    let _init_params = connection.initialize(capabilities)?;
    
    let mut server = LanguageServer::new(connection);
    
    // Instead of looping directly over connection.receiver here,
    // we call the server's main loop.
    server.run()?;
    
    io_threads.join()?;
    Ok(())
}
