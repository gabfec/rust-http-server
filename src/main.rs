use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::env;
use std::fs;
use std::thread;
use std::path::Path;

#[derive(Debug)]
enum HttpMethod {
    GET,
    POST,
}

#[derive(Debug)]
struct HttpRequest {
    method: HttpMethod,
    path: String,
    headers: HashMap<String, String>,
}

impl HttpRequest {
    fn from_stream(mut stream: &TcpStream) -> Option<Self> {
        let mut buffer = [0; 2048];
        let bytes_read = stream.read(&mut buffer).ok()?;

        if bytes_read == 0 {
            return None;
        }

        let request_str = String::from_utf8_lossy(&buffer);
        let mut lines = request_str.lines();

        // Parse Request Line
        let first_line = lines.next()?;
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        let method = match parts.get(0)? {
            &"POST" => HttpMethod::POST,
            _ => HttpMethod::GET,
        };
        let path = parts.get(1)?.to_string();

        // Parse Headers
        let mut headers = HashMap::new();
        for line in lines {
            if line.is_empty() {
                break; // End of headers
            }
            if let Some((key, value)) = line.split_once(": ") {
                headers.insert(key.to_lowercase(), value.to_string());
            }
        }

        Some(HttpRequest { method, path, headers })
    }
}

fn handle_connection(mut stream: TcpStream, directory: String) {
    if let Some(request) = HttpRequest::from_stream(&stream) {
        println!("Request received for path: {}", request.path);

        match request.path.as_str() {
            "/" => stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n").unwrap(),
            // Fill the body of the response with the content of the path
            path if path.starts_with("/echo/") => {
                let content = &path[6..];
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    content.len(),
                    content
                );
                stream.write_all(response.as_bytes()).unwrap();
            },
            path if path.starts_with("/files/") => {
                let filename = &path[7..]; // Strip "/files/"
                let file_path = Path::new(&directory).join(filename);

                if file_path.exists() {
                    let content = fs::read(&file_path).unwrap();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
                        content.len()
                    );
                    // Send headers first, then the raw binary body
                    stream.write_all(response.as_bytes()).unwrap();
                    stream.write_all(&content).unwrap();
                } else {
                    stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").unwrap();
                }
            }
            "/user-agent" => {
                // Look up the header (keys are lowercase because we normalized them)
                let ua = request.headers.get("user-agent").map(|s| s.as_str()).unwrap_or("");
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    ua.len(),
                    ua
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
            _ => stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").unwrap(),
        }
    }
}

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    let args: Vec<String> = env::args().collect();
    let directory = if args.len() > 2 && args[1] == "--directory" {
        args[2].clone()
    } else {
        ".".to_string() // Default to current dir
    };

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("accepted new connection");
                let dir = directory.clone();
                thread::spawn( || {
                    handle_connection(stream, dir);
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
