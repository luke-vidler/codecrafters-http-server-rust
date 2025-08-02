use std::{
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
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();

    if reader.read_line(&mut request_line).is_err() {
        return;
    }

    let parts: Vec<&str> = request_line.trim_end().split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }

    let method = parts[0];
    let path = parts[1];

    let mut user_agent: Option<String> = None;

    for line_result in reader.by_ref().lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => return,
        };

        if line.trim().is_empty() {
            break;
        }

        if let Some(value) = line.strip_prefix("User-Agent: ") {
            user_agent = Some(value.to_string());
        }
    }

    // Handle /echo/{str}
    if method == "GET" && path.starts_with("/echo/") {
        let echo_str = &path[6..];
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
            echo_str.len(),
            echo_str
        );
        let _ = stream.write_all(response.as_bytes());
        return;
    }

    // Handle /user-agent
    if method == "GET" && path == "/user-agent" {
        if let Some(ua) = user_agent {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                ua.len(),
                ua
            );
            let _ = stream.write_all(response.as_bytes());
        } else {
            let response = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
            let _ = stream.write_all(response.as_bytes());
        }
        return;
    }

    // Handle /files/{filename}
    if method == "GET" && path.starts_with("/files/") {
        let filename = &path["/files/".len()..];
        let mut filepath = PathBuf::from(directory);
        filepath.push(filename);

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
        return;
    }

    // Handle GET /
    if method == "GET" && path == "/" {
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
        return;
    }

    // Default 404
    let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
    let _ = stream.write_all(response.as_bytes());
}
