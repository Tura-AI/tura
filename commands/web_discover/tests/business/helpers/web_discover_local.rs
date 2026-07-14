pub(crate) use serde_json::{json, Value};
pub(crate) use std::io::{Read, Write};
pub(crate) use std::net::TcpListener;
pub(crate) use std::path::Path;
pub(crate) use std::process::{Command, Stdio};
pub(crate) use std::sync::{Mutex, OnceLock};
pub(crate) use std::thread;

pub(crate) struct PageServer {
    pub(crate) url: String,
    pub(crate) join: thread::JoinHandle<String>,
}

impl PageServer {
    pub(crate) fn join(self) -> String {
        self.join.join().expect("server joins")
    }
}

pub(crate) struct MultiResponseServer {
    pub(crate) addr: std::net::SocketAddr,
    pub(crate) join: thread::JoinHandle<Vec<String>>,
}

impl MultiResponseServer {
    pub(crate) fn url_for(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    pub(crate) fn join(self) -> Vec<String> {
        self.join.join().expect("multi response server joins")
    }
}

pub(crate) struct StubResponse {
    pub(crate) path: String,
    pub(crate) status: u16,
    pub(crate) content_type: String,
    pub(crate) body: Vec<u8>,
}

impl StubResponse {
    pub(crate) fn ok(path: &str, content_type: &str, body: Vec<u8>) -> Self {
        Self::status(path, 200, content_type, body)
    }

    pub(crate) fn status(path: &str, status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            path: path.to_string(),
            status,
            content_type: content_type.to_string(),
            body,
        }
    }
}

pub(crate) fn spawn_multi_response_server(mut responses: Vec<StubResponse>) -> MultiResponseServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind multi response server");
    let addr = listener.local_addr().expect("multi response server addr");
    let join = thread::spawn(move || {
        let mut requests = Vec::new();
        let expected = responses.len();
        for _ in 0..expected {
            let (mut stream, _) = listener.accept().expect("accept multi response request");
            let request = read_request_head(&mut stream);
            let request_path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
            let index = responses
                .iter()
                .position(|response| response.path == request_path)
                .unwrap_or_else(|| {
                    panic!("unexpected response request path {request_path}; request was {request}")
                });
            let response = responses.remove(index);
            let reason = if response.status == 200 {
                "OK"
            } else {
                "ERROR"
            };
            let headers = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.status,
                reason,
                response.content_type,
                response.body.len()
            );
            stream
                .write_all(headers.as_bytes())
                .expect("write multi response headers");
            stream
                .write_all(&response.body)
                .expect("write multi response body");
            requests.push(request);
        }
        requests
    });
    MultiResponseServer { addr, join }
}

pub(crate) fn spawn_page_server(title: &str) -> PageServer {
    spawn_response_server(
        200,
        "text/html; charset=utf-8",
        html_page(title),
        "/article",
    )
}

pub(crate) fn spawn_cf_retry_page_server() -> RetryPageServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local retry server");
    let addr = listener.local_addr().expect("retry server addr");
    let join = thread::spawn(move || {
        let mut requests = Vec::new();
        let (mut first, _) = listener.accept().expect("accept challenge request");
        requests.push(read_request_head(&mut first));
        first
            .write_all(
                concat!(
                    "HTTP/1.1 403 Forbidden\r\n",
                    "Content-Type: text/html; charset=utf-8\r\n",
                    "cf-mitigated: challenge\r\n",
                    "Connection: close\r\n",
                    "Content-Length: 22\r\n",
                    "\r\n",
                    "<html>challenge</html>"
                )
                .as_bytes(),
            )
            .expect("write challenge response");
        drop(first);

        let (mut second, _) = listener.accept().expect("accept retry request");
        requests.push(read_request_head(&mut second));
        let body = html_page("Retry Local Page").replace(
            "business sentinel paragraph",
            "local retry success body paragraph",
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        second
            .write_all(response.as_bytes())
            .expect("write retry response");
        requests
    });
    RetryPageServer {
        url: format!("http://{addr}/challenge"),
        join,
    }
}

pub(crate) struct RetryPageServer {
    pub(crate) url: String,
    pub(crate) join: thread::JoinHandle<Vec<String>>,
}

impl RetryPageServer {
    pub(crate) fn join(self) -> Vec<String> {
        self.join.join().expect("retry server joins")
    }
}

pub(crate) struct ReaderFallbackServer {
    pub(crate) addr: std::net::SocketAddr,
    pub(crate) url: String,
    pub(crate) join: thread::JoinHandle<Vec<String>>,
}

impl ReaderFallbackServer {
    pub(crate) fn join(self) -> Vec<String> {
        self.join.join().expect("reader fallback server joins")
    }
}

pub(crate) struct SearchRouteFallbackServer {
    pub(crate) brave_url: String,
    pub(crate) duckduckgo_url: String,
    pub(crate) join: thread::JoinHandle<Vec<String>>,
}

impl SearchRouteFallbackServer {
    pub(crate) fn join(self) -> Vec<String> {
        self.join.join().expect("search fallback server joins")
    }
}

pub(crate) struct CustomSearchWithPagesServer {
    pub(crate) search_url: String,
    pub(crate) join: thread::JoinHandle<Vec<String>>,
}

impl CustomSearchWithPagesServer {
    pub(crate) fn join(self) -> Vec<String> {
        self.join.join().expect("custom search server joins")
    }
}

pub(crate) fn spawn_custom_search_with_pages_server() -> CustomSearchWithPagesServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind custom search server");
    let addr = listener.local_addr().expect("custom search addr");
    let join = thread::spawn(move || {
        let mut requests = Vec::new();

        let (mut search, _) = listener.accept().expect("accept custom search request");
        let search_request = read_http_request(&mut search);
        let search_body = json!({
            "results": [
                {
                    "title": "Custom One",
                    "url": format!("http://{addr}/custom-one"),
                    "snippet": "first custom endpoint result"
                },
                {
                    "name": "Custom Two",
                    "link": format!("http://{addr}/custom-two"),
                    "description": "second custom endpoint result",
                    "sourceUrl": format!("http://{addr}/source-two")
                },
                {
                    "title": "Missing URL from custom endpoint"
                }
            ]
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            search_body.len(),
            search_body
        );
        search
            .write_all(response.as_bytes())
            .expect("write custom search response");
        requests.push(search_request);
        drop(search);

        for (path, title, marker) in [
            (
                "/custom-one",
                "Custom One",
                "custom one business body paragraph",
            ),
            (
                "/custom-two",
                "Custom Two",
                "custom two business body paragraph",
            ),
        ] {
            let (mut page, _) = listener.accept().expect("accept custom result page");
            let page_request = read_request_head(&mut page);
            assert!(
                page_request.starts_with(&format!("GET {path} ")),
                "unexpected custom result page request: {page_request}"
            );
            let body = html_page(title).replace("business sentinel paragraph", marker);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            page.write_all(response.as_bytes())
                .expect("write custom result page");
            requests.push(page_request);
        }
        requests
    });
    CustomSearchWithPagesServer {
        search_url: format!("http://{addr}/search"),
        join,
    }
}

pub(crate) fn spawn_search_route_fallback_server(result_url: &str) -> SearchRouteFallbackServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind search fallback server");
    let addr = listener.local_addr().expect("search fallback addr");
    let result_url = result_url.to_string();
    let join = thread::spawn(move || {
        let mut requests = Vec::new();

        let (mut brave, _) = listener.accept().expect("accept brave search request");
        requests.push(read_request_head(&mut brave).to_ascii_lowercase());
        brave
            .write_all(
                concat!(
                    "HTTP/1.1 500 Internal Server Error\r\n",
                    "Content-Type: application/json\r\n",
                    "Connection: close\r\n",
                    "Content-Length: 26\r\n",
                    "\r\n",
                    "{\"error\":\"local failure\"}"
                )
                .as_bytes(),
            )
            .expect("write brave failure");
        drop(brave);

        let (mut duck, _) = listener.accept().expect("accept duckduckgo search request");
        requests.push(read_request_head(&mut duck).to_ascii_lowercase());
        let body = format!(
            r#"<!doctype html>
<html>
  <body>
    <a class="result__a" href="{result_url}">Search Fallback Local Page</a>
    <a class="result__snippet" href="{result_url}">local search fallback snippet</a>
  </body>
</html>"#
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        duck.write_all(response.as_bytes())
            .expect("write duckduckgo results");
        requests
    });
    SearchRouteFallbackServer {
        brave_url: format!("http://{addr}/brave"),
        duckduckgo_url: format!("http://{addr}/duck"),
        join,
    }
}

pub(crate) fn spawn_reader_fallback_server() -> ReaderFallbackServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local reader fallback server");
    let addr = listener.local_addr().expect("reader fallback server addr");
    let join = thread::spawn(move || {
        let mut requests = Vec::new();

        let (mut primary, _) = listener.accept().expect("accept primary request");
        requests.push(read_request_head(&mut primary));
        let primary_body =
            "<html><head><title>Tiny Primary</title></head><body>tiny primary</body></html>";
        let primary_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            primary_body.len(),
            primary_body
        );
        primary
            .write_all(primary_response.as_bytes())
            .expect("write primary response");
        drop(primary);

        let (mut reader, _) = listener.accept().expect("accept reader request");
        requests.push(read_request_head(&mut reader));
        let reader_body = format!(
            "Title: Reader Fallback Business Page\n\n{}",
            "reader fallback business body ".repeat(40)
        );
        let reader_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/markdown; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            reader_body.len(),
            reader_body
        );
        reader
            .write_all(reader_response.as_bytes())
            .expect("write reader response");
        requests
    });
    ReaderFallbackServer {
        addr,
        url: format!("http://{addr}/short"),
        join,
    }
}

pub(crate) fn spawn_truncated_response_server() -> PageServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind truncated response server");
    let addr = listener
        .local_addr()
        .expect("truncated response server addr");
    let join = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("accept truncated response request");
        let request = read_request_head(&mut stream);
        stream
            .write_all(
                concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Type: text/html; charset=utf-8\r\n",
                    "Connection: close\r\n",
                    "Content-Length: 8192\r\n",
                    "\r\n",
                    "<html><head><title>Truncated</title></head><body>partial body that should not be trusted"
                )
                .as_bytes(),
            )
            .expect("write truncated response");
        request
    });
    PageServer {
        url: format!("http://{addr}/truncated"),
        join,
    }
}

pub(crate) fn spawn_response_server(
    status: u16,
    content_type: &str,
    body: String,
    path: &str,
) -> PageServer {
    spawn_binary_response_server(status, content_type, body.into_bytes(), path)
}

pub(crate) fn spawn_binary_response_server(
    status: u16,
    content_type: &str,
    body: Vec<u8>,
    path: &str,
) -> PageServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local server");
    let addr = listener.local_addr().expect("server addr");
    let content_type = content_type.to_string();
    let reason = if status == 200 { "OK" } else { "ERROR" }.to_string();
    let path = path.to_string();
    let join = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let request = read_request_head(&mut stream);
        let headers = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream
            .write_all(headers.as_bytes())
            .expect("write response headers");
        stream.write_all(&body).expect("write response body");
        request
    });
    PageServer {
        url: format!("http://{addr}{path}"),
        join,
    }
}

pub(crate) fn read_request_head(stream: &mut std::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 512];
    loop {
        let read = stream.read(&mut chunk).expect("read request");
        assert!(read > 0, "client closed before request headers");
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&buffer).to_string()
}

pub(crate) fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 512];
    let mut header_end = None;
    loop {
        let read = stream.read(&mut chunk).expect("read http request");
        assert!(read > 0, "client closed before request");
        buffer.extend_from_slice(&chunk[..read]);
        if header_end.is_none() {
            header_end = buffer
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|index| index + 4);
        }
        if let Some(end) = header_end {
            let header = String::from_utf8_lossy(&buffer[..end]).to_string();
            let content_length = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if name.eq_ignore_ascii_case("content-length") {
                        value.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if buffer.len() >= end + content_length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&buffer).to_string()
}

pub(crate) fn html_page(title: &str) -> String {
    let long_body = (0..80)
        .map(|index| {
            format!(
                "business sentinel paragraph {index}: local loopback content proves the command can fetch and save a webpage without public internet."
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        r#"<!doctype html>
<html>
  <head><title>{title}</title></head>
  <body>
    <main>
      <h1>{title}</h1>
      <p>{long_body}</p>
    </main>
  </body>
</html>"#
    )
}

pub(crate) fn tiny_png_bytes() -> &'static [u8] {
    &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60,
        0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xe2, 0x21, 0xbc, 0x33, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ]
}

pub(crate) fn write_fake_ytdlp(dir: &Path) -> std::path::PathBuf {
    #[cfg(windows)]
    {
        let script = dir.join("fake-ytdlp.cmd");
        let ps1 = dir.join("fake-ytdlp.ps1");
        std::fs::write(
            &ps1,
            r#"$template = $null
for ($index = 0; $index -lt $args.Count; $index++) {
  if ($args[$index] -eq '-o' -and ($index + 1) -lt $args.Count) {
    $template = $args[$index + 1]
    $index++
  }
}
if ([string]::IsNullOrWhiteSpace($template)) {
  exit 2
}
$path = $template.Replace('%(title).80s', 'business-audio').Replace('%(id)s', 'local').Replace('%(ext)s', 'mp3')
$isVideo = $args -contains 'best[height<=540][ext=mp4]/best[height<=540]/best'
if ($isVideo) {
  $path = $template.Replace('%(title).80s', 'business-video').Replace('%(id)s', 'local').Replace('%(ext)s', 'mp4')
}
New-Item -ItemType Directory -Force -Path (Split-Path -LiteralPath $path) | Out-Null
if ($isVideo) {
  [System.IO.File]::WriteAllText($path, 'fake local video bytes', [System.Text.Encoding]::ASCII)
} else {
  [System.IO.File]::WriteAllText($path, 'fake local audio bytes', [System.Text.Encoding]::ASCII)
}
"#,
        )
        .expect("write fake yt-dlp ps1");
        std::fs::write(
            &script,
            r#"@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0fake-ytdlp.ps1" %*
exit /b %ERRORLEVEL%
"#,
        )
        .expect("write fake yt-dlp cmd");
        script
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("fake-ytdlp.sh");
        std::fs::write(
            &script,
            r#"#!/usr/bin/env sh
set -eu
template=""
is_video=0
while [ "$#" -gt 0 ]; do
  if [ "$1" = "best[height<=540][ext=mp4]/best[height<=540]/best" ]; then
    is_video=1
  fi
  if [ "$1" = "-o" ]; then
    shift
    template="$1"
  fi
  shift || true
done
if [ -z "$template" ]; then
  exit 2
fi
if [ "$is_video" -eq 1 ]; then
  path=$(printf '%s' "$template" | sed 's/%(title).80s/business-video/g; s/%(id)s/local/g; s/%(ext)s/mp4/g')
else
  path=$(printf '%s' "$template" | sed 's/%(title).80s/business-audio/g; s/%(id)s/local/g; s/%(ext)s/mp3/g')
fi
mkdir -p "$(dirname "$path")"
if [ "$is_video" -eq 1 ]; then
  printf 'fake local video bytes' > "$path"
else
  printf 'fake local audio bytes' > "$path"
fi
"#,
        )
        .expect("write fake yt-dlp sh");
        pub(crate) use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script)
            .expect("fake yt-dlp metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).expect("chmod fake yt-dlp");
        script
    }
}

pub(crate) fn run_protocol(request: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tura-command-web-discover"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn web_discover binary");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(request.to_string().as_bytes())
        .expect("write request");
    let output = child.wait_with_output().expect("protocol output");
    assert!(
        output.status.success(),
        "web_discover protocol process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("protocol json response")
}

pub(crate) fn normalize_path(path: &str) -> String {
    Path::new(path).to_string_lossy().replace('\\', "/")
}

pub(crate) fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) fn restore_env(name: &str, value: Option<std::ffi::OsString>) {
    if let Some(value) = value {
        std::env::set_var(name, value);
    } else {
        std::env::remove_var(name);
    }
}
