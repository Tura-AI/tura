use super::helpers::*;

#[test]
fn pass_web_discover_image_download_writes_image() {
    let _guard = env_lock_blocking();
    let root = temp_workspace("web-discover-image");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let image_url = format!("http://{addr}/image.jpg");
    let endpoint = format!("http://{addr}/images");
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("GET /images") {
                let html = format!(
                    r#"<html><body><a href="/images/detail?mediaurl={}&purl={}"><img alt="Official fixture photo"></a></body></html>"#,
                    image_url.replace(":", "%3a").replace("/", "%2f"),
                    "https%3a%2f%2fofficial.example%2fsource"
                );
                write_http_response(&mut stream, "text/html", &html);
            } else {
                let mut image = image::RgbImage::new(48, 48);
                for (_, _, pixel) in image.enumerate_pixels_mut() {
                    *pixel = image::Rgb([20, 120, 220]);
                }
                let mut bytes = Vec::new();
                image::DynamicImage::ImageRgb8(image)
                    .write_to(
                        &mut std::io::Cursor::new(&mut bytes),
                        image::ImageFormat::Jpeg,
                    )
                    .expect("encode jpeg");
                write_http_response_bytes(&mut stream, "image/jpeg", &bytes);
            }
        }
    });

    std::env::set_var("TURA_IMAGE_SEARCH_ENDPOINT", endpoint);
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": "--type image --query fixture --max-results 1 --download-dir media/image --min-size 100 --max-size 1000000",
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_IMAGE_SEARCH_ENDPOINT");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["command_type"], "web_discover");
    assert_eq!(output["results"][0]["success"], true);
    let downloaded = output["results"][0]["output"]["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert_eq!(downloaded.len(), 1);
    let relative = downloaded[0]["path"].as_str().expect("relative path");
    assert!(root.join(relative).exists());
    assert_eq!(
        downloaded[0]["source_page_url"],
        "https://official.example/source"
    );
}

#[test]
fn pass_web_discover_image_uses_brave_endpoint_when_key_is_set() {
    let _guard = env_lock_blocking();
    let root = temp_workspace("web-discover-brave-image");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let image_url = format!("http://{addr}/brave-image.jpg");
    let endpoint = format!("http://{addr}/brave-images");
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("GET /brave-images") {
                assert!(request.contains("q=fixture"));
                let body = json!({
                    "type": "images",
                    "results": [
                        {
                            "type": "image_result",
                            "title": "Official fixture from Brave",
                            "source": "https://official.example/brave-source",
                            "properties": {
                                "url": image_url,
                                "width": 48,
                                "height": 48
                            },
                            "thumbnail": {
                                "src": "https://imgs.search.brave.com/thumb"
                            },
                            "meta_url": {
                                "hostname": "official.example"
                            }
                        }
                    ]
                })
                .to_string();
                write_http_response(&mut stream, "application/json", &body);
            } else {
                let mut image = image::RgbImage::new(48, 48);
                for (_, _, pixel) in image.enumerate_pixels_mut() {
                    *pixel = image::Rgb([80, 180, 40]);
                }
                let mut bytes = Vec::new();
                image::DynamicImage::ImageRgb8(image)
                    .write_to(
                        &mut std::io::Cursor::new(&mut bytes),
                        image::ImageFormat::Jpeg,
                    )
                    .expect("encode jpeg");
                write_http_response_bytes(&mut stream, "image/jpeg", &bytes);
            }
        }
    });

    std::env::set_var("TURA_BRAVE_SEARCH_API_KEY", "test-key");
    std::env::set_var("TURA_BRAVE_IMAGE_SEARCH_ENDPOINT", endpoint);
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": "--type image --query fixture --max-results 1 --download-dir media/brave --min-size 100 --max-size 1000000",
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_BRAVE_SEARCH_API_KEY");
    std::env::remove_var("TURA_BRAVE_IMAGE_SEARCH_ENDPOINT");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["command_type"], "web_discover");
    assert_eq!(output["results"][0]["success"], true);
    let result = &output["results"][0]["output"]["results"][0];
    assert_eq!(result["source"], "brave_images");
    assert_eq!(result["page_url"], "https://official.example/brave-source");
    let downloaded = output["results"][0]["output"]["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert_eq!(downloaded.len(), 1);
    assert_eq!(
        downloaded[0]["source_page_url"],
        "https://official.example/brave-source"
    );
    let relative = downloaded[0]["path"].as_str().expect("relative path");
    assert!(root.join(relative).exists());
}

#[test]
fn pass_web_discover_image_reads_brave_key_from_tura_config() {
    let _guard = env_lock_blocking();
    let root = temp_workspace("web-discover-brave-config");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let image_url = format!("http://{addr}/brave-config-image.jpg");
    let endpoint = format!("http://{addr}/brave-config-images");
    let env_path = root.join(".env");
    fs::write(
        &env_path,
        format!(
            "TURA_BRAVE_SEARCH_API_KEY=config-test-key\nTURA_BRAVE_IMAGE_SEARCH_ENDPOINT={endpoint}\n"
        ),
    )
    .expect("write tura env");
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("GET /brave-config-images") {
                assert!(request.contains("q=fixture"));
                let body = json!({
                    "type": "images",
                    "results": [
                        {
                            "type": "image_result",
                            "title": "Config Brave fixture",
                            "source": "https://official.example/config-source",
                            "properties": {
                                "url": image_url
                            }
                        }
                    ]
                })
                .to_string();
                write_http_response(&mut stream, "application/json", &body);
            } else {
                let mut image = image::RgbImage::new(48, 48);
                for (_, _, pixel) in image.enumerate_pixels_mut() {
                    *pixel = image::Rgb([120, 40, 190]);
                }
                let mut bytes = Vec::new();
                image::DynamicImage::ImageRgb8(image)
                    .write_to(
                        &mut std::io::Cursor::new(&mut bytes),
                        image::ImageFormat::Jpeg,
                    )
                    .expect("encode jpeg");
                write_http_response_bytes(&mut stream, "image/jpeg", &bytes);
            }
        }
    });

    std::env::remove_var("TURA_BRAVE_SEARCH_API_KEY");
    std::env::remove_var("BRAVE_API_KEY");
    std::env::remove_var("TURA_BRAVE_IMAGE_SEARCH_ENDPOINT");
    std::env::set_var("TURA_ENV_PATH", &env_path);
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": "--type image --query fixture --max-results 1 --download-dir media/config-brave --min-size 100 --max-size 1000000",
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_ENV_PATH");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["command_type"], "web_discover");
    assert_eq!(output["results"][0]["success"], true);
    let result = &output["results"][0]["output"]["results"][0];
    assert_eq!(result["source"], "brave_images");
    assert_eq!(result["page_url"], "https://official.example/config-source");
}

#[test]
fn pass_web_discover_image_uses_duckduckgo_fallback_without_brave() {
    let _guard = env_lock_blocking();
    let root = temp_workspace("web-discover-ddg-image");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let image_url = format!("http://{addr}/ddg-image.jpg");
    let page_endpoint = format!("http://{addr}/ddg");
    let search_endpoint = format!("http://{addr}/i.js");
    let server = thread::spawn(move || {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("GET /ddg") {
                write_http_response(&mut stream, "text/html", "vqd='fixture-vqd';");
            } else if request.starts_with("GET /i.js") {
                assert!(request.contains("q=fixture"));
                assert!(request.contains("vqd=fixture-vqd"));
                let body = json!({
                    "results": [
                        {
                            "title": "Official fixture from DuckDuckGo",
                            "image": image_url,
                            "url": "https://official.example/ddg-source",
                            "source": "official.example"
                        }
                    ]
                })
                .to_string();
                write_http_response(&mut stream, "application/json", &body);
            } else {
                let mut image = image::RgbImage::new(96, 96);
                for (_, _, pixel) in image.enumerate_pixels_mut() {
                    *pixel = image::Rgb([220, 90, 40]);
                }
                let mut bytes = Vec::new();
                image::DynamicImage::ImageRgb8(image)
                    .write_to(
                        &mut std::io::Cursor::new(&mut bytes),
                        image::ImageFormat::Jpeg,
                    )
                    .expect("encode jpeg");
                write_http_response_bytes(&mut stream, "image/jpeg", &bytes);
            }
        }
    });

    std::env::remove_var("TURA_IMAGE_SEARCH_ENDPOINT");
    std::env::remove_var("TURA_BRAVE_SEARCH_API_KEY");
    std::env::remove_var("BRAVE_API_KEY");
    std::env::set_var("TURA_BRAVE_SEARCH_DISABLED", "1");
    std::env::set_var("TURA_EXA_SEARCH_DISABLED", "1");
    std::env::set_var("TURA_DUCKDUCKGO_IMAGE_PAGE_ENDPOINT", page_endpoint);
    std::env::set_var("TURA_DUCKDUCKGO_IMAGE_SEARCH_ENDPOINT", search_endpoint);
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": "--type image --query fixture --max-results 1 --download-dir media/ddg --min-size 1 --max-size 1000000",
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_BRAVE_SEARCH_DISABLED");
    std::env::remove_var("TURA_EXA_SEARCH_DISABLED");
    std::env::remove_var("TURA_DUCKDUCKGO_IMAGE_PAGE_ENDPOINT");
    std::env::remove_var("TURA_DUCKDUCKGO_IMAGE_SEARCH_ENDPOINT");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["command_type"], "web_discover");
    assert_eq!(output["results"][0]["success"], true);
    let result = &output["results"][0]["output"]["results"][0];
    assert_eq!(result["source"], "duckduckgo_images");
    assert_eq!(result["page_url"], "https://official.example/ddg-source");
    let downloaded = output["results"][0]["output"]["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert_eq!(downloaded.len(), 1);
    let relative = downloaded[0]["path"].as_str().expect("relative path");
    assert!(root.join(relative).exists());
}

#[test]
fn pass_web_discover_image_min_size_filters_small_downloads() {
    let _guard = env_lock_blocking();
    let root = temp_workspace("web-discover-ddg-min-size");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let image_url = format!("http://{addr}/tiny.jpg");
    let page_endpoint = format!("http://{addr}/ddg");
    let search_endpoint = format!("http://{addr}/i.js");
    let server = thread::spawn(move || {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("GET /ddg") {
                write_http_response(&mut stream, "text/html", "vqd='tiny-vqd';");
            } else if request.starts_with("GET /i.js") {
                let body = json!({
                    "results": [
                        {
                            "title": "Tiny fixture",
                            "image": image_url,
                            "url": "https://official.example/tiny-source",
                            "source": "official.example"
                        }
                    ]
                })
                .to_string();
                write_http_response(&mut stream, "application/json", &body);
            } else {
                write_http_response_bytes(&mut stream, "image/jpeg", &[1, 2, 3, 4, 5]);
            }
        }
    });

    std::env::remove_var("TURA_IMAGE_SEARCH_ENDPOINT");
    std::env::remove_var("TURA_BRAVE_SEARCH_API_KEY");
    std::env::remove_var("BRAVE_API_KEY");
    std::env::set_var("TURA_BRAVE_SEARCH_DISABLED", "1");
    std::env::set_var("TURA_EXA_SEARCH_DISABLED", "1");
    std::env::set_var("TURA_DUCKDUCKGO_IMAGE_PAGE_ENDPOINT", page_endpoint);
    std::env::set_var("TURA_DUCKDUCKGO_IMAGE_SEARCH_ENDPOINT", search_endpoint);
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": "--type image --query fixture --max-results 1 --download-dir media/ddg --min-size 100 --max-size 1000000",
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_BRAVE_SEARCH_DISABLED");
    std::env::remove_var("TURA_EXA_SEARCH_DISABLED");
    std::env::remove_var("TURA_DUCKDUCKGO_IMAGE_PAGE_ENDPOINT");
    std::env::remove_var("TURA_DUCKDUCKGO_IMAGE_SEARCH_ENDPOINT");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["command_type"], "web_discover");
    assert_eq!(output["results"][0]["success"], true);
    let downloaded = output["results"][0]["output"]["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert!(downloaded.is_empty());
    assert!(root
        .join("media/ddg")
        .read_dir()
        .expect("read dir")
        .next()
        .is_none());
}

#[test]
fn pass_web_discover_website_download_writes_markdown() {
    let root = temp_workspace("web-discover-website");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let page_url = format!("http://{addr}/page");
    let endpoint = format!("http://{addr}/search");
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("POST /search") {
                let body = json!({
                    "results": [
                        {
                            "title": "Fixture Page",
                            "url": page_url,
                            "snippet": "A fixture page"
                        }
                    ]
                })
                .to_string();
                write_http_response(&mut stream, "application/json", &body);
            } else {
                write_http_response(
                    &mut stream,
                    "text/html",
                    "<html><head><title>Fixture Page</title><script>hidden()</script></head><body><h1>Hello Web</h1><p>Clean visible text.</p></body></html>",
                );
            }
        }
    });

    std::env::set_var("TURA_WEB_DISCOVER_ENDPOINT", endpoint);
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": "web_discover website fixture --max-results=1 --download-dir=docs",
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_WEB_DISCOVER_ENDPOINT");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["command_type"], "web_discover");
    assert_eq!(output["results"][0]["success"], true);
    let downloaded = output["results"][0]["output"]["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert_eq!(downloaded.len(), 1);
    let relative = downloaded[0]["path"].as_str().expect("relative path");
    assert!(relative.starts_with("docs"));
    let markdown = fs::read_to_string(root.join(relative)).expect("read markdown");
    assert!(markdown.contains("Clean visible text"));
    assert!(!markdown.contains("hidden()"));
}

#[test]
fn pass_web_discover_direct_website_returns_structured_record_and_markdown() {
    let root = temp_workspace("web-discover-direct-text");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let page_url = format!("http://{addr}/page");
    let long_body = format!("{}{}", "A".repeat(900), "B".repeat(900));
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0u8; 4096];
        let _ = stream.read(&mut buffer).expect("read request");
        let body = format!(
            "<html><head><title>Fixture Page</title><script>hidden()</script></head><body><h1>Hello Web</h1><p>{long_body}</p></body></html>"
        );
        write_http_response(&mut stream, "text/html", &body);
    });

    std::env::set_var("TURA_WEB_READER_DISABLED", "1");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": format!("web_discover website \"{page_url}\""),
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_WEB_READER_DISABLED");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["success"], true);
    let results = output["results"][0]["output"]["results"]
        .as_array()
        .expect("results");
    assert_eq!(results.len(), 1);
    let record = results[0]
        .as_object()
        .expect("website result should be a structured record");
    assert_eq!(
        record.get("url").and_then(Value::as_str),
        Some(page_url.as_str())
    );
    let downloaded = output["results"][0]["output"]["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert_eq!(downloaded.len(), 1);
    let relative = downloaded[0]["path"].as_str().expect("relative path");
    let markdown = fs::read_to_string(root.join(relative)).expect("read markdown");
    assert!(markdown.contains("Hello Web"));
    assert!(!markdown.contains("hidden()"));
}

fn write_http_response(stream: &mut std::net::TcpStream, content_type: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .expect("write response");
}

fn write_http_response_bytes(stream: &mut std::net::TcpStream, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).expect("write header");
    stream.write_all(body).expect("write body");
}
