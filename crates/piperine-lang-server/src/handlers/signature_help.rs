use lsp_server::{Connection, Request};
use lsp_types::{
    ParameterInformation, SignatureHelp, SignatureHelpParams, SignatureInformation,
};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<SignatureHelpParams>(connection) else { return };

    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let result = state.documents.get(&uri).and_then(|doc| {
        let offset = crate::text_pos::position_to_byte(&doc.source, pos);
        let (callee, active_param) = enclosing_call(&doc.source, offset)?;

        // PHDL calls with signatures worth showing: module instantiations.
        let design = doc.design.as_ref()?;
        let m = design.modules().find(|m| m.name == callee)?;

        let mut params = Vec::new();
        let mut label = format!("{} (", m.name);
        for (i, p) in m.ports.iter().enumerate() {
            let param_label = format!("{:?} {}", p.direction, p.name);
            params.push(ParameterInformation {
                label: lsp_types::ParameterLabel::Simple(param_label.clone()),
                documentation: None,
            });
            label.push_str(&param_label);
            if i < m.ports.len() - 1 {
                label.push_str(", ");
            }
        }
        label.push(')');

        Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label,
                documentation: None,
                parameters: Some(params),
                active_parameter: None,
            }],
            active_signature: Some(0),
            active_parameter: Some(active_param),
        })
    });

    connection.respond(id, result);
}

/// Walk backwards from `offset` to the unbalanced `(` that encloses the
/// cursor and return the identifier before it plus the comma-derived
/// active-parameter index.
fn enclosing_call(source: &str, offset: usize) -> Option<(String, u32)> {
    let bytes = source.as_bytes();
    let mut curr = offset.min(bytes.len());
    let mut depth = 0;
    let mut active_param = 0;

    loop {
        if curr == 0 {
            return None;
        }
        curr -= 1;
        match bytes[curr] {
            b',' if depth == 0 => active_param += 1,
            b')' => depth += 1,
            b'(' if depth == 0 => break,
            b'(' => depth -= 1,
            _ => {}
        }
    }

    while curr > 0 && bytes[curr - 1].is_ascii_whitespace() {
        curr -= 1;
    }
    let end_word = curr;
    while curr > 0 && (bytes[curr - 1].is_ascii_alphanumeric() || bytes[curr - 1] == b'_') {
        curr -= 1;
    }
    (curr < end_word).then(|| (source[curr..end_word].to_string(), active_param))
}
