pub mod completion;
pub mod diagnostics;
pub mod goto_def;
pub mod hover;
pub mod symbols;
pub mod formatting;
pub mod references;
pub mod rename;
pub mod signature_help;
pub mod semantic_tokens;
pub mod code_actions;
pub mod inlay_hints;
pub mod folding_range;
pub mod selection_range;
pub mod workspace_symbols;
pub mod document_highlight;

use lsp_server::{Connection, Request, RequestId, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Request-side protocol plumbing shared by every handler: deserialize the
/// params or answer the request with `InvalidParams` (every request id must
/// get a response — silently dropping one is a protocol violation).
pub(crate) trait RequestExt {
    fn parse<P: DeserializeOwned>(self, connection: &Connection) -> Option<(RequestId, P)>;
}

impl RequestExt for Request {
    fn parse<P: DeserializeOwned>(self, connection: &Connection) -> Option<(RequestId, P)> {
        match serde_json::from_value(self.params) {
            Ok(params) => Some((self.id, params)),
            Err(e) => {
                connection.respond_invalid(self.id, format!("invalid params: {e}"));
                None
            }
        }
    }
}

/// Response-side plumbing: serialize and send, swallowing send errors (the
/// channel only closes when the client disconnects — not worth panicking).
pub(crate) trait ConnectionExt {
    fn respond<T: Serialize>(&self, id: RequestId, result: T);
    fn respond_invalid(&self, id: RequestId, message: String);
}

impl ConnectionExt for Connection {
    fn respond<T: Serialize>(&self, id: RequestId, result: T) {
        let _ = self.sender.send(Response::new_ok(id, result).into());
    }

    fn respond_invalid(&self, id: RequestId, message: String) {
        let code = lsp_server::ErrorCode::InvalidParams as i32;
        let _ = self.sender.send(Response::new_err(id, code, message).into());
    }
}
