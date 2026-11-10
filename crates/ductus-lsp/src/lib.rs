//! ductus-lsp — a language server for Ductus, run as `ductus lsp`.
//!
//! A synchronous, single-threaded server on `lsp-server`: messages are
//! handled one at a time in arrival order, so the open documents need no
//! locking. Every change re-parses and re-evaluates the document. The live
//! preview's HTTP threads only pass settings in and frames out.

mod builtins;
mod completion;
mod definition;
mod diagnostics;
mod hints;
mod hover;
mod importer;
mod position;
mod preview;
mod references;
mod rename;
mod scope;
mod signature;
mod symbols;
mod syntax;
mod values;
mod workspace;

use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, LogMessage,
    Notification as _, PublishDiagnostics,
};
use lsp_types::request::{Completion, ExecuteCommand, HoverRequest, InlayHintRequest};
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
use lsp_types::{ExecuteCommandOptions, ExecuteCommandParams};
use lsp_types::{
    Hover, HoverContents, HoverParams, HoverProviderCapability, InlayHint, InlayHintLabel,
    InlayHintParams,
};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use position::Encoding;
use preview::{Focus, Preview, Rendered, Settings};
use serde_json::json;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tree_sitter::Tree;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Starts the live preview and answers `{ url, viewers }`.
const PREVIEW_COMMAND: &str = "ductus.preview";
/// From the client: the cursor moved, `TextDocumentPositionParams`.
const FOCUS_NOTIFICATION: &str = "ductus/focus";

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

    let preview_port = params
        .initialization_options
        .as_ref()
        .and_then(|options| options.get("previewPort"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .unwrap_or(preview::DEFAULT_PORT);

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
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            inlay_hint_provider: Some(OneOf::Left(true)),
            completion_provider: Some(CompletionOptions {
                trigger_characters: Some(vec![".".to_string()]),
                ..CompletionOptions::default()
            }),
            signature_help_provider: Some(SignatureHelpOptions {
                trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                retrigger_characters: None,
                work_done_progress_options: WorkDoneProgressOptions::default(),
            }),
            execute_command_provider: Some(ExecuteCommandOptions {
                commands: vec![PREVIEW_COMMAND.to_string()],
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

    let (settings_sender, settings) = crossbeam_channel::unbounded();
    Server {
        connection,
        encoding,
        snippets,
        roots,
        documents: HashMap::new(),
        preview_port,
        settings_sender,
        preview: None,
        settings: Settings::default(),
        focus: None,
        last_render: None,
    }
    .main_loop(settings)
}

struct Document {
    /// As the client sent it; reused when pointing the client back here.
    uri: Uri,
    text: String,
    tree: Tree,
    /// The last evaluation without errors, for hover values and hints.
    evaluation: Option<Evaluation>,
}

struct Evaluation {
    document: ductus_core::doc::Document,
    /// The text has changed since, and the change has errors.
    stale: bool,
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
    /// Where the preview listens, unless taken.
    preview_port: u16,
    /// Handed to the preview's HTTP threads for the page's settings.
    settings_sender: crossbeam_channel::Sender<Settings>,
    /// The live preview, once started.
    preview: Option<Preview>,
    settings: Settings,
    focus: Option<Focus>,
    /// Shown, dimmed, while the focused glyph has errors.
    last_render: Option<Rendered>,
}

impl Server {
    fn main_loop(mut self, settings: crossbeam_channel::Receiver<Settings>) -> Result<()> {
        let messages = self.connection.receiver.clone();
        loop {
            crossbeam_channel::select! {
                recv(messages) -> message => {
                    let Ok(message) = message else {
                        return Ok(());
                    };
                    match message {
                        Message::Request(request) => {
                            if self.connection.handle_shutdown(&request)? {
                                return Ok(());
                            }
                            if request.method == ExecuteCommand::METHOD {
                                self.execute_command(request)?;
                            } else {
                                self.request(request)?;
                            }
                        }
                        Message::Notification(notification) => self.notification(notification)?,
                        Message::Response(_) => {}
                    }
                }
                recv(settings) -> received => {
                    if let Ok(received) = received {
                        self.settings = received;
                        self.refresh_preview(false)?;
                    }
                }
            }
        }
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
            HoverRequest::METHOD => {
                self.respond::<HoverRequest>(request, |server, params| Ok(server.hover(params)))
            }
            InlayHintRequest::METHOD => self
                .respond::<InlayHintRequest>(request, |server, params| {
                    Ok(server.inlay_hints(params))
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

    fn hover(&self, params: HoverParams) -> Option<Hover> {
        let at = params.text_document_position_params;
        let (path, document) = self.document(&at.text_document.uri)?;
        let offset = position::offset(&document.text, at.position, self.encoding);
        let read = |path: &Path| self.read(path);
        let evaluated = document
            .evaluation
            .as_ref()
            .map(|evaluation| hover::Evaluated {
                document: &evaluation.document,
                stale: evaluation.stale,
            });
        let (markdown, range) = hover::hover(
            &document.tree,
            &document.text,
            &path,
            offset,
            &read,
            evaluated,
        )?;
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: Some(position::range(
                &document.text,
                range.start,
                range.end,
                self.encoding,
            )),
        })
    }

    fn inlay_hints(&self, params: InlayHintParams) -> Option<Vec<InlayHint>> {
        let (_, document) = self.document(&params.text_document.uri)?;
        let evaluation = document.evaluation.as_ref()?;
        let text = &document.text;
        let range = position::offset(text, params.range.start, self.encoding)
            ..position::offset(text, params.range.end, self.encoding);
        let hints = hints::hints(&document.tree, text, &evaluation.document, range)
            .into_iter()
            .map(|hint| InlayHint {
                position: position::position(text, hint.offset, self.encoding),
                label: InlayHintLabel::String(hint.label),
                kind: None,
                text_edits: None,
                tooltip: None,
                padding_left: Some(true),
                padding_right: None,
                data: None,
            })
            .collect();
        Some(hints)
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

    // ---- Preview -----------------------------------------------------------

    /// `ductus.preview`: focus on the position given as the argument, start
    /// the preview if needed, and answer where it is.
    fn execute_command(&mut self, request: Request) -> Result<()> {
        let params: ExecuteCommandParams = match serde_json::from_value(request.params) {
            Ok(params) => params,
            Err(error) => {
                let code = ErrorCode::InvalidParams as i32;
                let response = Response::new_err(request.id, code, error.to_string());
                self.connection.sender.send(response.into())?;
                return Ok(());
            }
        };
        if params.command != PREVIEW_COMMAND {
            let code = ErrorCode::InvalidParams as i32;
            let message = format!("unknown command {}", params.command);
            self.connection
                .sender
                .send(Response::new_err(request.id, code, message).into())?;
            return Ok(());
        }
        let at = params.arguments.into_iter().next();
        if let Some(at) = at.and_then(|at| serde_json::from_value(at).ok()) {
            self.focus_on(&at);
        }
        if self.preview.is_none() {
            match Preview::start(self.preview_port, self.settings_sender.clone()) {
                Ok(preview) => {
                    self.log(format!("preview at {}", preview.url()))?;
                    self.preview = Some(preview);
                }
                Err(error) => {
                    let code = ErrorCode::RequestFailed as i32;
                    let message = format!("cannot start the preview: {error}");
                    self.connection
                        .sender
                        .send(Response::new_err(request.id, code, message).into())?;
                    return Ok(());
                }
            }
        }
        self.refresh_preview(false)?;
        let preview = self.preview.as_ref().expect("started above");
        let answer = json!({ "url": preview.url(), "viewers": preview.viewers() });
        self.connection
            .sender
            .send(Response::new_ok(request.id, answer).into())?;
        Ok(())
    }

    /// Move the focus for a cursor at `at`; true when it changed.
    fn focus_on(&mut self, at: &TextDocumentPositionParams) -> bool {
        let Some((path, document)) = self.document(&at.text_document.uri) else {
            return false;
        };
        let offset = position::offset(&document.text, at.position, self.encoding);
        let focus = preview::refocus(
            self.focus.as_ref(),
            &path,
            &document.tree,
            &document.text,
            offset,
        );
        let changed = focus != self.focus;
        self.focus = focus;
        changed
    }

    /// Render the focused glyph and send it to the page. `reuse` takes the
    /// evaluation the focused document's own update just made, when the
    /// params are at their defaults; otherwise, as after a change to a file
    /// it imports, evaluate again.
    fn refresh_preview(&mut self, reuse: bool) -> Result<()> {
        let Some(preview) = &self.preview else {
            return Ok(());
        };
        let Some(focus) = &self.focus else {
            preview.publish(preview::waiting());
            return Ok(());
        };
        let Some(document) = self.documents.get(&focus.path) else {
            return Ok(());
        };
        let started = Instant::now();
        let glyph = preview::shown(&self.settings.choice, focus);
        let reusable = document
            .evaluation
            .as_ref()
            .filter(|evaluation| reuse && !evaluation.stale && self.settings.overrides.is_empty());
        let read = |path: &Path| self.read(path);
        let fresh;
        let evaluated = match reusable {
            Some(evaluation) => Ok(&evaluation.document),
            None => {
                let overrides = &self.settings.overrides;
                match diagnostics::check(
                    &document.tree,
                    &document.text,
                    &focus.path,
                    &read,
                    overrides,
                ) {
                    Ok(evaluated) => {
                        fresh = evaluated;
                        Ok(&fresh)
                    }
                    Err(problems) => Err(failure(&document.text, problems)),
                }
            }
        };
        let rendered = evaluated.and_then(|evaluated| {
            preview::render(evaluated, glyph.as_deref(), self.settings.debug).map_err(|message| {
                preview::Failure {
                    message,
                    line: None,
                }
            })
        });

        let file = focus.path.file_name().unwrap_or_default().to_string_lossy();
        let debug = self.settings.debug;
        let frame = match &rendered {
            Ok(rendered) => preview::frame(&file, glyph.as_deref(), debug, Some(rendered), None),
            Err(failure) => {
                let last = self.last_render.as_ref();
                preview::frame(&file, glyph.as_deref(), debug, last, Some(failure))
            }
        };
        preview.publish(frame);
        let message = format!(
            "previewed {file} {} in {:.1} ms",
            glyph.as_deref().unwrap_or("(all)"),
            started.elapsed().as_secs_f64() * 1000.0
        );
        if let Ok(rendered) = rendered {
            self.last_render = Some(rendered);
        }
        self.log(message)
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
            FOCUS_NOTIFICATION => {
                let at: TextDocumentPositionParams = serde_json::from_value(notification.params)?;
                if self.focus_on(&at) {
                    self.refresh_preview(false)?;
                }
                Ok(())
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
        let evaluation = self
            .documents
            .remove(&path)
            .and_then(|previous| previous.evaluation);
        let document = Document {
            uri,
            text,
            tree,
            evaluation,
        };
        self.documents.insert(path.clone(), document);

        let document = &self.documents[&path];
        let read = |path: &Path| self.read(path);
        let checked = diagnostics::check(
            &document.tree,
            &document.text,
            &path,
            &read,
            &HashMap::new(),
        );
        let elapsed = started.elapsed();

        let document = self.documents.get_mut(&path).expect("inserted above");
        let problems = match checked {
            Ok(evaluated) => {
                document.evaluation = Some(Evaluation {
                    document: evaluated,
                    stale: false,
                });
                Vec::new()
            }
            Err(problems) => {
                if let Some(evaluation) = &mut document.evaluation {
                    evaluation.stale = true;
                }
                problems
            }
        };
        let document = &self.documents[&path];

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
        ))?;
        let focused = self.focus.as_ref().is_some_and(|focus| focus.path == path);
        self.refresh_preview(focused)
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

/// The first problem, as the preview shows it.
fn failure(text: &str, problems: Vec<diagnostics::Problem>) -> preview::Failure {
    match problems.into_iter().next() {
        Some(problem) => preview::Failure {
            line: Some(text[..problem.range.start].matches('\n').count() + 1),
            message: problem.message,
        },
        None => preview::Failure {
            message: "evaluation failed".to_string(),
            line: None,
        },
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
