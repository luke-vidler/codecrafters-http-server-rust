use flate2::write::GzEncoder;
use flate2::Compression;
use std::{
    collections::HashMap,
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    thread,
};

fn main() {
    let mut args = env::args().skip(1);
    let mut directory = None;

    while let Some(arg) = args.next() {
        if arg == "--directory" {
            if let Some(dir) = args.next() {
                directory = Some(dir);
            }
        }
    }

    let directory = directory.unwrap_or_else(|| ".".to_string());

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    println!("Listening on http://127.0.0.1:4221");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let dir_clone = directory.clone();
                thread::spawn(move || {
                    handle_client(stream, &dir_clone);
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }
}

fn handle_client(mut stream: TcpStream, directory: &str) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());

    loop {
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() || request_line.trim().is_empty() {
            break;
        }

        let parts: Vec<&str> = request_line.trim_end().split_whitespace().collect();
        if parts.len() < 2 {
            break;
        }

        let method = parts[0];
        let path = parts[1];

        let mut headers = HashMap::new();
        let mut content_length = 0;

        for line_result in reader.by_ref().lines() {
            let line = match line_result {
                Ok(l) => l,
                Err(_) => return,
            };

            if line.trim().is_empty() {
                break;
            }

            if let Some((key, value)) = line.split_once(":") {
                let key = key.trim().to_ascii_lowercase();
                let value = value.trim().to_string();

                if key == "content-length" {
                    if let Ok(len) = value.parse::<usize>() {
                        content_length = len;
                    }
                }

                headers.insert(key, value);
            }
        }

        // Handle GET /echo/{str}
        if method == "GET" && path.starts_with("/echo/") {
            let echo_str = &path[6..];
            let accept_encoding = headers.get("accept-encoding");

            let client_accepts_gzip = accept_encoding
                .map(|v| v.split(',').any(|enc| enc.trim() == "gzip"))
                .unwrap_or(false);

            let (body, content_encoding_header) = if client_accepts_gzip {
                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                if encoder.write_all(echo_str.as_bytes()).is_err() {
                    return;
                }
                let compressed_body = encoder.finish().unwrap_or_default();
                (compressed_body, Some("Content-Encoding: gzip\r\n"))
            } else {
                (echo_str.as_bytes().to_vec(), None)
            };

            let mut response = String::from("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n");
            if let Some(enc_header) = content_encoding_header {
                response.push_str(enc_header);
            }
            response.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));

            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&body);
            continue;
        }

        // Handle GET /user-agent
        if method == "GET" && path == "/user-agent" {
            let user_agent = headers.get("user-agent").cloned().unwrap_or_default();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                user_agent.len(),
                user_agent
            );
            let _ = stream.write_all(response.as_bytes());
            continue;
        }

        // Handle GET and POST for /files/{filename}
        if path.starts_with("/files/") {
            let filename = &path["/files/".len()..];
            let mut filepath = PathBuf::from(directory);
            filepath.push(filename);

            match method {
                "GET" => {
                    match fs::read(&filepath) {
                        Ok(contents) => {
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
                                contents.len()
                            );
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.write_all(&contents);
                        }
                        Err(_) => {
                            let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
                            let _ = stream.write_all(response.as_bytes());
                        }
                    }
                    continue;
                }

                "POST" => {
                    let mut body = vec![0; content_length];
                    if reader.read_exact(&mut body).is_err() {
                        let response = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
                        let _ = stream.write_all(response.as_bytes());
                        continue;
                    }

                    match fs::write(&filepath, &body) {
                        Ok(_) => {
                            let response = "HTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n";
                            let _ = stream.write_all(response.as_bytes());
                        }
                        Err(_) => {
                            let response =
                                "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n";
                            let _ = stream.write_all(response.as_bytes());
                        }
                    }
                    continue;
                }

                _ => {}
            }
        }

        // Handle GET /
        if method == "GET" && path == "/" {
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
            let _ = stream.write_all(response.as_bytes());
            continue;
        }

        // Default 404 Not Found
        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
    }
}
