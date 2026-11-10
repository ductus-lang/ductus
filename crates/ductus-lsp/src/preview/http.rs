//! A small HTTP/1.1 server on `127.0.0.1` for the preview page: the page
//! itself, a server-sent event stream of frames, and a settings endpoint.
//! One thread per connection; event streams wait on a condvar for frames.

use super::Settings;
use crossbeam_channel::Sender;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

const PAGE: &str = include_str!("page.html");
/// A comment on idle event streams, so a closed page ends its thread.
const KEEP_ALIVE: Duration = Duration::from_secs(15);
/// Settings are a few hundred bytes; refuse anything much larger.
const MAX_BODY: usize = 64 * 1024;

pub struct Preview {
    url: String,
    shared: Arc<Shared>,
}

struct Shared {
    /// The latest frame and its version; 0 before the first.
    frame: Mutex<(u64, String)>,
    changed: Condvar,
    /// Pages connected to the event stream.
    viewers: AtomicUsize,
}

impl Preview {
    /// Listen on `port`, or on a free port when it is taken. Settings the
    /// page posts go to `settings`.
    pub fn start(port: u16, settings: Sender<Settings>) -> io::Result<Preview> {
        let listener = TcpListener::bind(("127.0.0.1", port))
            .or_else(|_| TcpListener::bind(("127.0.0.1", 0)))?;
        let port = listener.local_addr()?.port();
        let shared = Arc::new(Shared {
            frame: Mutex::new((0, String::new())),
            changed: Condvar::new(),
            viewers: AtomicUsize::new(0),
        });
        let accepting = Arc::clone(&shared);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let shared = Arc::clone(&accepting);
                let settings = settings.clone();
                thread::spawn(move || {
                    // A failed connection only affects that page.
                    let _ = handle(stream, &shared, &settings, port);
                });
            }
        });
        Ok(Preview {
            url: format!("http://127.0.0.1:{port}/"),
            shared,
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn viewers(&self) -> usize {
        self.shared.viewers.load(Ordering::Relaxed)
    }

    pub fn publish(&self, frame: String) {
        let mut latest = self
            .shared
            .frame
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *latest = (latest.0 + 1, frame);
        self.shared.changed.notify_all();
    }
}

fn handle(
    stream: TcpStream,
    shared: &Shared,
    settings: &Sender<Settings>,
    port: u16,
) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));

    let mut host = String::new();
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "host" => host = value.trim().to_string(),
                "content-length" => length = value.trim().parse().unwrap_or(0),
                _ => {}
            }
        }
    }

    // Only our own origin: a page elsewhere resolving a name to 127.0.0.1
    // (DNS rebinding) sends its own host name.
    if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
        return respond(stream, "403 Forbidden", "text/plain", b"forbidden");
    }
    match (method, target) {
        ("GET", "/") => respond(
            stream,
            "200 OK",
            "text/html; charset=utf-8",
            PAGE.as_bytes(),
        ),
        ("GET", "/events") => events(stream, shared),
        ("POST", "/settings") if length <= MAX_BODY => {
            let mut body = vec![0; length];
            reader.read_exact(&mut body)?;
            match Settings::from_json(&String::from_utf8_lossy(&body)) {
                Some(parsed) => {
                    let _ = settings.send(parsed);
                    respond(stream, "204 No Content", "text/plain", b"")
                }
                None => respond(stream, "400 Bad Request", "text/plain", b"bad settings"),
            }
        }
        _ => respond(stream, "404 Not Found", "text/plain", b"not found"),
    }
}

fn respond(mut stream: TcpStream, status: &str, content_type: &str, body: &[u8]) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

/// Counts a connected page for as long as its stream lives.
struct Viewer<'a>(&'a AtomicUsize);

impl Drop for Viewer<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// The latest frame now, then every new one, until the page goes away.
fn events(mut stream: TcpStream, shared: &Shared) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\n\
         Connection: keep-alive\r\n\r\n"
    )?;
    stream.flush()?;
    shared.viewers.fetch_add(1, Ordering::Relaxed);
    let _viewer = Viewer(&shared.viewers);
    let mut seen = 0;
    loop {
        let latest = shared.frame.lock().unwrap_or_else(PoisonError::into_inner);
        let (latest, _) = shared
            .changed
            .wait_timeout_while(latest, KEEP_ALIVE, |(version, _)| *version == seen)
            .unwrap_or_else(PoisonError::into_inner);
        let (version, frame) = (latest.0, latest.1.clone());
        drop(latest);
        if version == seen {
            stream.write_all(b": keep-alive\n\n")?;
        } else {
            // Frames are single-line JSON, so one `data:` line carries one.
            write!(stream, "event: frame\ndata: {frame}\n\n")?;
            seen = version;
        }
        stream.flush()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    fn start() -> (Preview, crossbeam_channel::Receiver<Settings>, u16) {
        let (sender, receiver) = unbounded();
        let preview = Preview::start(0, sender).unwrap();
        let port = preview
            .url()
            .trim_end_matches('/')
            .rsplit(':')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        (preview, receiver, port)
    }

    /// Send `request` and read until the connection closes.
    fn exchange(port: u16, request: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    #[test]
    fn serves_the_page_to_its_own_origin_only() {
        let (_preview, _settings, port) = start();
        let page = exchange(
            port,
            &format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
        );
        assert!(page.starts_with("HTTP/1.1 200 OK"), "{page}");
        assert!(page.contains("<!doctype html>"));
        let foreign = exchange(port, "GET / HTTP/1.1\r\nHost: evil.example:80\r\n\r\n");
        assert!(foreign.starts_with("HTTP/1.1 403"), "{foreign}");
    }

    #[test]
    fn settings_reach_the_main_loop() {
        let (_preview, settings, port) = start();
        let body = r#"{"overrides":{"weight":700},"debug":true,"glyph":"o"}"#;
        let response = exchange(
            port,
            &format!(
                "POST /settings HTTP/1.1\r\nHost: localhost:{port}\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        );
        assert!(response.starts_with("HTTP/1.1 204"), "{response}");
        let received = settings.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(received.overrides["weight"], 700.0);
    }

    #[test]
    fn streams_the_latest_frame_then_new_ones() {
        let (preview, _settings, port) = start();
        preview.publish("{\"n\":1}".to_string());
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            stream,
            "GET /events HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
        )
        .unwrap();
        let mut reader = BufReader::new(stream);
        let mut data = || loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(data) = line.strip_prefix("data: ") {
                return data.trim().to_string();
            }
        };
        assert_eq!(data(), "{\"n\":1}");
        assert_eq!(preview.viewers(), 1);
        preview.publish("{\"n\":2}".to_string());
        assert_eq!(data(), "{\"n\":2}");
    }
}
