//! ductus-lsp — a language server for Ductus, run as `ductus lsp`.
//!
//! A synchronous, single-threaded server on `lsp-server`: messages are
//! handled one at a time in arrival order, so the open documents need no
//! locking. Every change re-parses and re-evaluates the document.

mod builtins;
mod completion;
mod definition;
mod diagnostics;
mod importer;
mod position;
mod references;
mod rename;
mod scope;
mod signature;
mod symbols;
mod syntax;
mod workspace;

use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, LogMessage,
    Notification as _, PublishDiagnostics,
};
use lsp_types::request::Completion;
use lsp_types::request::{
    DocumentSymbolRequest, Formatting, GotoDefinition, PrepareRenameRequest, References, Rename,
    Request as _, SignatureHelpRequest, WorkspaceSymbolRequest,
};
use lsp_types::{
    CompletionItem, CompletionOptions, CompletionParams, CompletionResponse, InsertTextFormat,
};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentFormattingParams, DocumentSymbol, DocumentSymbolParams,
    DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse, InitializeParams,
    InitializeResult, Location, LogMessageParams, MessageType, OneOf, PrepareRenameResponse,
    PublishDiagnosticsParams, ReferenceParams, RenameOptions, RenameParams, ServerCapabilities,
    ServerInfo, TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextEdit, Uri, WorkDoneProgressOptions, WorkspaceEdit,
};
use lsp_types::{
    Documentation, MarkupContent, MarkupKind, ParameterInformation, ParameterLabel, SignatureHelp,
    SignatureHelpOptions, SignatureHelpParams, SignatureInformation, SymbolInformation,
    WorkspaceSymbolParams, WorkspaceSymbolResponse,
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
    let snippets = params
        .capabilities
        .text_document
        .as_ref()
        .and_then(|document| document.completion.as_ref())
        .and_then(|completion| completion.completion_item.as_ref())
        .and_then(|item| item.snippet_support)
        .unwrap_or(false);
    #[allow(deprecated, reason = "older clients send only root_uri")]
    let roots = match params.workspace_folders {
        Some(folders) => folders
            .iter()
            .filter_map(|folder| path_of(&folder.uri))
            .collect(),
        None => params
            .root_uri
            .as_ref()
            .and_then(path_of)
            .into_iter()
            .collect(),
    };

    let result = InitializeResult {
        capabilities: ServerCapabilities {
            position_encoding: Some(encoding.kind()),
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            document_formatting_provider: Some(OneOf::Left(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Right(RenameOptions {
                prepare_provider: Some(true),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            })),
            workspace_symbol_provider: Some(OneOf::Left(true)),
            completion_provider: Some(CompletionOptions {
                trigger_characters: Some(vec![".".to_string()]),
                ..CompletionOptions::default()
            }),
            signature_help_provider: Some(SignatureHelpOptions {
                trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                retrigger_characters: None,
                work_done_progress_options: WorkDoneProgressOptions::default(),
            }),
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
        snippets,
        roots,
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
    /// The client expands snippets: completed calls get argument
    /// placeholders.
    snippets: bool,
    /// Workspace folders; references and rename search their files.
    roots: Vec<PathBuf>,
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
            Formatting::METHOD => {
                self.respond::<Formatting>(request, |server, params| Ok(server.formatting(params)))
            }
            DocumentSymbolRequest::METHOD => self
                .respond::<DocumentSymbolRequest>(request, |server, params| {
                    Ok(server.symbols(params))
                }),
            GotoDefinition::METHOD => self
                .respond::<GotoDefinition>(request, |server, params| Ok(server.definition(params))),
            References::METHOD => {
                self.respond::<References>(request, |server, params| Ok(server.references(params)))
            }
            PrepareRenameRequest::METHOD => self
                .respond::<PrepareRenameRequest>(request, |server, params| {
                    Ok(server.prepare_rename(params))
                }),
            Rename::METHOD => self.respond::<Rename>(request, Self::rename),
            WorkspaceSymbolRequest::METHOD => self
                .respond::<WorkspaceSymbolRequest>(request, |server, params| {
                    Ok(server.workspace_symbols(params))
                }),
            Completion::METHOD => {
                self.respond::<Completion>(request, |server, params| Ok(server.completion(params)))
            }
            SignatureHelpRequest::METHOD => self
                .respond::<SignatureHelpRequest>(request, |server, params| {
                    Ok(server.signature_help(params))
                }),
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
        handler: fn(&Self, R::Params) -> std::result::Result<R::Result, String>,
    ) -> Response {
        match serde_json::from_value(request.params).map(|params| handler(self, params)) {
            Ok(Ok(result)) => Response::new_ok(request.id, result),
            Ok(Err(message)) => {
                Response::new_err(request.id, ErrorCode::RequestFailed as i32, message)
            }
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
        self.location(&target).map(GotoDefinitionResponse::Scalar)
    }

    fn references(&self, params: ReferenceParams) -> Option<Vec<Location>> {
        let binding = self.binding_at(&params.text_document_position)?;
        let read = |path: &Path| self.read(path);
        let found = references::references(&binding, &self.files(Some(&binding.path)), &read);
        let include = params.context.include_declaration;
        Some(
            found
                .iter()
                .filter(|site| include || **site != binding)
                .filter_map(|site| self.location(site))
                .collect(),
        )
    }

    fn prepare_rename(&self, at: TextDocumentPositionParams) -> Option<PrepareRenameResponse> {
        let (path, document) = self.document(&at.text_document.uri)?;
        let offset = position::offset(&document.text, at.position, self.encoding);
        let read = |path: &Path| self.read(path);
        let range = rename::prepare(&document.tree, &document.text, &path, offset, &read)?;
        let range = position::range(&document.text, range.start, range.end, self.encoding);
        Some(PrepareRenameResponse::Range(range))
    }

    fn rename(&self, params: RenameParams) -> std::result::Result<Option<WorkspaceEdit>, String> {
        let binding = self
            .binding_at(&params.text_document_position)
            .ok_or("nothing to rename here")?;
        let read = |path: &Path| self.read(path);
        let files = self.files(Some(&binding.path));
        let edits = rename::rename(&binding, &params.new_name, &files, &read)?;
        #[allow(clippy::mutable_key_type, reason = "WorkspaceEdit is keyed by Uri")]
        let mut changes = HashMap::new();
        for (path, ranges) in edits {
            let text = self.read(&path).ok_or("cannot read a renamed file")?;
            let edits = ranges
                .into_iter()
                .map(|range| {
                    let range = position::range(&text, range.start, range.end, self.encoding);
                    TextEdit::new(range, params.new_name.clone())
                })
                .collect();
            changes.insert(self.uri(&path).ok_or("cannot name a renamed file")?, edits);
        }
        Ok(Some(WorkspaceEdit::new(changes)))
    }

    /// Top-level symbols of every workspace file whose name matches the
    /// query. Glyph locals are left out: every glyph has an `advance`.
    fn workspace_symbols(&self, params: WorkspaceSymbolParams) -> Option<WorkspaceSymbolResponse> {
        let mut found = Vec::new();
        for path in self.files(None) {
            let Some(text) = self.read(&path) else {
                continue;
            };
            let Some(uri) = self.uri(&path) else {
                continue;
            };
            let tree = ductus_core::parse::parse_lenient(&text);
            let symbols = symbols::symbols(&tree, &text)
                .into_iter()
                .filter(|symbol| symbol.kind != lsp_types::SymbolKind::FILE)
                .filter(|symbol| symbols::matches(&params.query, &symbol.name));
            for symbol in symbols {
                let range =
                    position::range(&text, symbol.range.start, symbol.range.end, self.encoding);
                #[allow(deprecated, reason = "SymbolInformation has no constructor")]
                found.push(SymbolInformation {
                    name: symbol.name,
                    kind: symbol.kind,
                    tags: None,
                    deprecated: None,
                    location: Location::new(uri.clone(), range),
                    container_name: None,
                });
            }
        }
        Some(WorkspaceSymbolResponse::Flat(found))
    }

    fn completion(&self, params: CompletionParams) -> Option<CompletionResponse> {
        let at = params.text_document_position;
        let (path, document) = self.document(&at.text_document.uri)?;
        let offset = position::offset(&document.text, at.position, self.encoding);
        let read = |path: &Path| self.read(path);
        let items = completion::complete(&document.tree, &document.text, &path, offset, &read)
            .into_iter()
            .map(|item| {
                let snippet = item.snippet.filter(|_| self.snippets);
                CompletionItem {
                    sort_text: Some(format!("{}{}", item.group, item.label)),
                    insert_text_format: snippet.as_ref().map(|_| InsertTextFormat::SNIPPET),
                    insert_text: snippet,
                    label: item.label,
                    kind: Some(item.kind),
                    detail: item.detail,
                    documentation: item.doc.map(|doc| {
                        Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: doc,
                        })
                    }),
                    ..CompletionItem::default()
                }
            })
            .collect();
        Some(CompletionResponse::Array(items))
    }

    fn signature_help(&self, params: SignatureHelpParams) -> Option<SignatureHelp> {
        let at = params.text_document_position_params;
        let (path, document) = self.document(&at.text_document.uri)?;
        let offset = position::offset(&document.text, at.position, self.encoding);
        let read = |path: &Path| self.read(path);
        let help = signature::signature_help(&document.tree, &document.text, &path, offset, &read)?;
        let signatures = help
            .signatures
            .into_iter()
            .map(|signature| SignatureInformation {
                label: signature.label,
                documentation: signature.doc.map(|doc| {
                    Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: doc,
                    })
                }),
                // Labels are ASCII, so byte offsets are also UTF-16 offsets.
                parameters: Some(
                    signature
                        .parameters
                        .into_iter()
                        .map(|range| ParameterInformation {
                            label: ParameterLabel::LabelOffsets([
                                range.start as u32,
                                range.end as u32,
                            ]),
                            documentation: None,
                        })
                        .collect(),
                ),
                active_parameter: None,
            })
            .collect();
        Some(SignatureHelp {
            signatures,
            active_signature: Some(help.active_signature as u32),
            active_parameter: Some(help.active_parameter as u32),
        })
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

    /// The binding the identifier at a position resolves to. Built-ins and
    /// import paths have no binding.
    fn binding_at(&self, at: &TextDocumentPositionParams) -> Option<definition::Target> {
        let (path, document) = self.document(&at.text_document.uri)?;
        let offset = position::offset(&document.text, at.position, self.encoding);
        let read = |path: &Path| self.read(path);
        definition::definition(&document.tree, &document.text, &path, offset, &read)
            .filter(|target| !target.range.is_empty())
    }

    /// The files a workspace-wide request considers: the `.ductus` files
    /// under the workspace folders, the open documents, and `path`. Without
    /// workspace folders, the directories of `path` and the open documents
    /// stand in for them.
    fn files(&self, path: Option<&Path>) -> Vec<PathBuf> {
        let known = || {
            path.into_iter()
                .chain(self.documents.keys().map(PathBuf::as_path))
        };
        let roots: Vec<PathBuf> = match self.roots.is_empty() {
            true => known()
                .filter_map(Path::parent)
                .map(Path::to_path_buf)
                .collect(),
            false => self.roots.clone(),
        };
        let mut files = workspace::ductus_files(&roots);
        files.extend(known().map(Path::to_path_buf));
        files.sort();
        files.dedup();
        files
    }

    fn location(&self, target: &definition::Target) -> Option<Location> {
        let text = self.read(&target.path)?;
        let range = position::range(&text, target.range.start, target.range.end, self.encoding);
        Some(Location::new(self.uri(&target.path)?, range))
    }

    /// The URI the client knows an open document by, else one for the path.
    fn uri(&self, path: &Path) -> Option<Uri> {
        match self.documents.get(path) {
            Some(open) => Some(open.uri.clone()),
            None => uri_of(path),
        }
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
