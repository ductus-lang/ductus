//! ductus-lsp — a language server for Ductus, run as `ductus lsp`.
//!
//! A synchronous, single-threaded server on `lsp-server`: messages are
//! handled one at a time in arrival order, so the open documents need no
//! locking. Every change re-parses and re-evaluates the document.

mod definition;
mod diagnostics;
mod importer;
mod position;
mod symbols;
mod syntax;

use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, LogMessage,
    Notification as _, PublishDiagnostics,
};
use lsp_types::request::{DocumentSymbolRequest, Formatting, GotoDefinition, Request as _};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentFormattingParams, DocumentSymbol, DocumentSymbolParams,
    DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse, InitializeParams,
    InitializeResult, Location, LogMessageParams, MessageType, OneOf, PublishDiagnosticsParams,
    ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit,
    Uri,
};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use position::Encoding;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tree_sitter::Tree;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Serve over stdin/stdout until the client asks to exit.
pub fn run() -> Result<()> {
    let (connection, io_threads) = Connection::stdio();
    serve(connection)?;
    io_threads.join()?;
    Ok(())
}

/// Initialize, then handle messages on `connection` until exit.
pub fn serve(connection: Connection) -> Result<()> {
    let (id, params) = connection.initialize_start()?;
    let params: InitializeParams = serde_json::from_value(params)?;
    let offered = params
        .capabilities
        .general
        .as_ref()
        .and_then(|general| general.position_encodings.as_deref());
    let encoding = Encoding::negotiate(offered);

    let result = InitializeResult {
        capabilities: ServerCapabilities {
            position_encoding: Some(encoding.kind()),
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            document_formatting_provider: Some(OneOf::Left(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            definition_provider: Some(OneOf::Left(true)),
            ..ServerCapabilities::default()
        },
        server_info: Some(ServerInfo {
            name: "ductus-lsp".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    };
    connection.initialize_finish(id, serde_json::to_value(result)?)?;

    Server {
        connection,
        encoding,
        documents: HashMap::new(),
    }
    .main_loop()
}

struct Document {
    /// As the client sent it; reused when pointing the client back here.
    uri: Uri,
    text: String,
    tree: Tree,
}

struct Server {
    connection: Connection,
    encoding: Encoding,
    /// Open documents by canonical path, the identity imports resolve to.
    documents: HashMap<PathBuf, Document>,
}

impl Server {
    fn main_loop(mut self) -> Result<()> {
        while let Ok(message) = self.connection.receiver.recv() {
            match message {
                Message::Request(request) => {
                    if self.connection.handle_shutdown(&request)? {
                        return Ok(());
                    }
                    self.request(request)?;
                }
                Message::Notification(notification) => self.notification(notification)?,
                Message::Response(_) => {}
            }
        }
        Ok(())
    }

    // ---- Requests ----------------------------------------------------------

    fn request(&self, request: Request) -> Result<()> {
        let response = match request.method.as_str() {
            Formatting::METHOD => self.respond::<Formatting>(request, Self::formatting),
            DocumentSymbolRequest::METHOD => {
                self.respond::<DocumentSymbolRequest>(request, Self::symbols)
            }
            GotoDefinition::METHOD => self.respond::<GotoDefinition>(request, Self::definition),
            method => Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                format!("unsupported request {method}"),
            ),
        };
        self.connection.sender.send(response.into())?;
        Ok(())
    }

    fn respond<R: lsp_types::request::Request>(
        &self,
        request: Request,
        handler: fn(&Self, R::Params) -> R::Result,
    ) -> Response {
        match serde_json::from_value(request.params) {
            Ok(params) => Response::new_ok(request.id, handler(self, params)),
            Err(error) => Response::new_err(
                request.id,
                ErrorCode::InvalidParams as i32,
                error.to_string(),
            ),
        }
    }

    /// The whole document as one edit. `None` when it has syntax errors:
    /// formatting an ERROR tree can destroy code.
    fn formatting(&self, params: DocumentFormattingParams) -> Option<Vec<TextEdit>> {
        let (_, document) = self.document(&params.text_document.uri)?;
        let formatted = ductus_core::format::format(&document.text).ok()?;
        if formatted == document.text {
            return Some(Vec::new());
        }
        let text = &document.text;
        let range = position::range(text, 0, text.len(), self.encoding);
        Some(vec![TextEdit::new(range, formatted)])
    }

    fn symbols(&self, params: DocumentSymbolParams) -> Option<DocumentSymbolResponse> {
        let (_, document) = self.document(&params.text_document.uri)?;
        let symbols = symbols::symbols(&document.tree, &document.text)
            .into_iter()
            .map(|symbol| self.document_symbol(&document.text, symbol))
            .collect();
        Some(DocumentSymbolResponse::Nested(symbols))
    }

    #[allow(deprecated, reason = "DocumentSymbol has no constructor")]
    fn document_symbol(&self, text: &str, symbol: symbols::Symbol) -> DocumentSymbol {
        let range = |bytes: std::ops::Range<usize>| {
            position::range(text, bytes.start, bytes.end, self.encoding)
        };
        DocumentSymbol {
            name: symbol.name,
            detail: symbol.detail,
            kind: symbol.kind,
            tags: None,
            deprecated: None,
            range: range(symbol.range),
            selection_range: range(symbol.selection),
            children: Some(
                symbol
                    .children
                    .into_iter()
                    .map(|child| self.document_symbol(text, child))
                    .collect(),
            ),
        }
    }

    fn definition(&self, params: GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let at = params.text_document_position_params;
        let (path, document) = self.document(&at.text_document.uri)?;
        let offset = position::offset(&document.text, at.position, self.encoding);
        let read = |path: &Path| self.read(path);
        let target = definition::definition(&document.tree, &document.text, &path, offset, &read)?;

        let text = self.read(&target.path)?;
        let range = position::range(&text, target.range.start, target.range.end, self.encoding);
        let uri = match self.documents.get(&target.path) {
            Some(open) => open.uri.clone(),
            None => uri_of(&target.path)?,
        };
        Some(GotoDefinitionResponse::Scalar(Location { uri, range }))
    }

    // ---- Notifications -----------------------------------------------------

    fn notification(&mut self, notification: Notification) -> Result<()> {
        match notification.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let params: DidOpenTextDocumentParams =
                    serde_json::from_value(notification.params)?;
                let document = params.text_document;
                self.update(document.uri, document.text, document.version)
            }
            DidChangeTextDocument::METHOD => {
                let params: DidChangeTextDocumentParams =
                    serde_json::from_value(notification.params)?;
                // Full sync: the last change carries the whole text.
                match params.content_changes.into_iter().last() {
                    Some(change) => {
                        let document = params.text_document;
                        self.update(document.uri, change.text, document.version)
                    }
                    None => Ok(()),
                }
            }
            DidCloseTextDocument::METHOD => {
                let params: DidCloseTextDocumentParams =
                    serde_json::from_value(notification.params)?;
                let uri = params.text_document.uri;
                if let Some(path) = path_of(&uri) {
                    self.documents.remove(&path);
                }
                self.publish(uri, Vec::new(), None)
            }
            _ => Ok(()),
        }
    }

    /// Store the new text, then check it and publish the problems.
    fn update(&mut self, uri: Uri, text: String, version: i32) -> Result<()> {
        let Some(path) = path_of(&uri) else {
            return Ok(());
        };
        let started = Instant::now();
        let tree = ductus_core::parse::parse_lenient(&text);
        self.documents
            .insert(path.clone(), Document { uri, text, tree });

        let document = &self.documents[&path];
        let read = |path: &Path| self.read(path);
        let problems = diagnostics::check(&document.tree, &document.text, &path, &read);
        let elapsed = started.elapsed();

        let diagnostics = problems
            .into_iter()
            .map(|problem| Diagnostic {
                range: position::range(
                    &document.text,
                    problem.range.start,
                    problem.range.end,
                    self.encoding,
                ),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("ductus".to_string()),
                message: problem.message,
                ..Diagnostic::default()
            })
            .collect();
        self.publish(document.uri.clone(), diagnostics, Some(version))?;
        self.log(format!(
            "checked {} in {:.1} ms",
            path.display(),
            elapsed.as_secs_f64() * 1000.0
        ))
    }

    // ---- Helpers -----------------------------------------------------------

    fn document(&self, uri: &Uri) -> Option<(PathBuf, &Document)> {
        let path = path_of(uri)?;
        let document = self.documents.get(&path)?;
        Some((path, document))
    }

    /// The open document's text, else the file on disk.
    fn read(&self, path: &Path) -> Option<String> {
        match self.documents.get(path) {
            Some(document) => Some(document.text.clone()),
            None => std::fs::read_to_string(path).ok(),
        }
    }

    fn publish(&self, uri: Uri, diagnostics: Vec<Diagnostic>, version: Option<i32>) -> Result<()> {
        let params = PublishDiagnosticsParams::new(uri, diagnostics, version);
        self.notify::<PublishDiagnostics>(params)
    }

    fn log(&self, message: String) -> Result<()> {
        let params = LogMessageParams {
            typ: MessageType::LOG,
            message,
        };
        self.notify::<LogMessage>(params)
    }

    fn notify<N: lsp_types::notification::Notification>(&self, params: N::Params) -> Result<()> {
        let notification = Notification::new(N::METHOD.to_string(), params);
        self.connection.sender.send(notification.into())?;
        Ok(())
    }
}

/// Characters escaped in a `file:` URI path: everything but unreserved
/// characters and `/`. Non-ASCII is always escaped.
const PATH: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// The canonical path of a `file:` URI. Unix paths only.
fn path_of(uri: &Uri) -> Option<PathBuf> {
    let encoded = uri.as_str().strip_prefix("file://")?;
    let path = PathBuf::from(percent_decode_str(encoded).decode_utf8().ok()?.as_ref());
    Some(path.canonicalize().unwrap_or(path))
}

fn uri_of(path: &Path) -> Option<Uri> {
    let encoded = utf8_percent_encode(path.to_str()?, PATH);
    format!("file://{encoded}").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uri_round_trip() {
        let path = Path::new("/no/such dir/അ#1.ductus");
        let uri = uri_of(path).unwrap();
        assert_eq!(uri.as_str(), "file:///no/such%20dir/%E0%B4%85%231.ductus");
        assert_eq!(path_of(&uri).unwrap(), path);
    }
}
