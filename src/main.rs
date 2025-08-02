use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    println!("Listening on http://127.0.0.1:4221");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Move the stream into a new thread
                std::thread::spawn(|| {
                    handle_client(stream);
                });
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }
}

fn handle_client(mut stream: TcpStream) {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();

    // Read the request line (e.g., GET /echo/abc HTTP/1.1)
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

    // Read the headers
    for line_result in reader.by_ref().lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => return,
        };

        if line.trim().is_empty() {
            break; // End of headers
        }

        // Normalize case-insensitive match for "User-Agent"
        if let Some(value) = line.strip_prefix("User-Agent: ") {
            user_agent = Some(value.to_string());
        }
    }

    // Handle /echo/{str}
    if method == "GET" && path.starts_with("/echo/") {
        let echo_body = &path[6..]; // Skip "/echo/"
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
            echo_body.len(),
            echo_body
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
            // Edge case: User-Agent missing (shouldn't happen in your test)
            let response = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
            let _ = stream.write_all(response.as_bytes());
        }
        return;
    }

    // Handle root path "/"
    if method == "GET" && path == "/" {
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
        return;
    }

    // Default: 404 Not Found
    let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
    let _ = stream.write_all(response.as_bytes());
}
