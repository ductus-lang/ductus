//! One whole session over an in-memory connection.

use lsp_server::{Connection, Message, Notification, Request, RequestId};
use serde_json::{Value, json};

const URI: &str = "file:///nonexistent/ductus-lsp/main.ductus";

struct Client {
    connection: Connection,
}

impl Client {
    fn request(&self, id: i32, method: &str, params: Value) -> Value {
        let request = Request::new(id.into(), method.to_string(), params);
        self.connection.sender.send(request.into()).unwrap();
        loop {
            match self.connection.receiver.recv().unwrap() {
                Message::Response(response) if response.id == RequestId::from(id) => {
                    return response.response_result.unwrap();
                }
                _ => {}
            }
        }
    }

    fn notify(&self, method: &str, params: Value) {
        let notification = Notification::new(method.to_string(), params);
        self.connection.sender.send(notification.into()).unwrap();
    }

    fn wait_for(&self, method: &str) -> Value {
        loop {
            if let Message::Notification(notification) = self.connection.receiver.recv().unwrap()
                && notification.method == method
            {
                return notification.params;
            }
        }
    }
}

#[test]
fn session() {
    let (server, client) = Connection::memory();
    let thread = std::thread::spawn(move || ductus_lsp::serve(server).map_err(|e| e.to_string()));
    let client = Client { connection: client };

    let initialized = client.request(
        1,
        "initialize",
        json!({ "capabilities": { "general": { "positionEncodings": ["utf-16", "utf-8"] } } }),
    );
    assert_eq!(initialized["capabilities"]["positionEncoding"], "utf-8");
    client.notify("initialized", json!({}));

    let text = "u =   1;\nv = u;\nv = 2;\n";
    client.notify(
        "textDocument/didOpen",
        json!({ "textDocument": { "uri": URI, "languageId": "ductus", "version": 1, "text": text } }),
    );
    let published = client.wait_for("textDocument/publishDiagnostics");
    let diagnostics = published["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0]["range"]["start"],
        json!({ "line": 2, "character": 0 })
    );

    let document = json!({ "uri": URI });
    let edits = client.request(
        2,
        "textDocument/formatting",
        json!({ "textDocument": document, "options": { "tabSize": 2, "insertSpaces": true } }),
    );
    assert_eq!(edits[0]["newText"], "u = 1;\nv = u;\nv = 2;\n");

    let symbols = client.request(
        3,
        "textDocument/documentSymbol",
        json!({ "textDocument": document }),
    );
    assert_eq!(symbols.as_array().unwrap().len(), 3);

    let location = client.request(
        4,
        "textDocument/definition",
        json!({ "textDocument": document, "position": { "line": 1, "character": 4 } }),
    );
    assert_eq!(location["uri"], URI);
    assert_eq!(
        location["range"]["start"],
        json!({ "line": 0, "character": 0 })
    );

    let at_use = json!({ "textDocument": document, "position": { "line": 1, "character": 4 } });
    let mut with_context = at_use.clone();
    with_context["context"] = json!({ "includeDeclaration": true });
    let locations = client.request(5, "textDocument/references", with_context);
    assert_eq!(locations.as_array().unwrap().len(), 2, "{locations}");

    let range = client.request(6, "textDocument/prepareRename", at_use.clone());
    assert_eq!(range["start"], json!({ "line": 1, "character": 4 }));

    let mut rename = at_use;
    rename["newName"] = json!("w");
    let edit = client.request(7, "textDocument/rename", rename);
    let edits = edit["changes"][URI].as_array().unwrap();
    assert_eq!(edits.len(), 2, "{edit}");
    assert!(edits.iter().all(|edit| edit["newText"] == "w"));

    let symbols = client.request(8, "workspace/symbol", json!({ "query": "u" }));
    let names: Vec<&Value> = symbols
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["name"])
        .collect();
    assert_eq!(names, [&json!("u")]);

    client.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": URI, "version": 2 }, "contentChanges": [{ "text": "x = min(1, " }] }),
    );
    client.wait_for("textDocument/publishDiagnostics");
    let help = client.request(
        9,
        "textDocument/signatureHelp",
        json!({ "textDocument": document, "position": { "line": 0, "character": 11 } }),
    );
    assert_eq!(help["signatures"][0]["label"], "min(a, b)");
    assert_eq!(
        help["signatures"][0]["parameters"][1]["label"],
        json!([7, 8])
    );
    assert_eq!(help["activeParameter"], 1);

    client.request(10, "shutdown", Value::Null);
    client.notify("exit", Value::Null);
    thread.join().unwrap().unwrap();
}
