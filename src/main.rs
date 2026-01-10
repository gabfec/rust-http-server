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
    body: Vec<u8>,
}

impl HttpRequest {
    fn from_stream(mut stream: &TcpStream) -> Option<Self> {
        let mut buffer = [0; 2048];
        let bytes_read = stream.read(&mut buffer).ok()?;

        if bytes_read == 0 {
            return None;
        }

        // Find the delimiter between Headers and Body
        let separator = buffer[..bytes_read]
            .windows(4)
            .position(|window| window == b"\r\n\r\n")?;

        let header_bytes = &buffer[..separator];
        let body_bytes = &buffer[separator + 4..bytes_read]; // Skip the 4 bytes of \r\n\r\n

        // Parse Headers from the header_bytes
        let header_str = String::from_utf8_lossy(header_bytes);
        let mut lines = header_str.lines();

        // Parse Request line
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

        // Handle Body
        let mut body = Vec::new();
        body.extend_from_slice(body_bytes);

        Some(HttpRequest { method, path, headers, body })
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

                match request.method {
                    HttpMethod::GET => {
                        println!("GET {}", file_path.display());

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
                    HttpMethod::POST => {
                        println!("POST {}", file_path.display());

                        // Write the body to the file
                        match fs::write(file_path, &request.body) {
                            Ok(_) => {
                                stream.write_all(b"HTTP/1.1 201 Created\r\n\r\n").unwrap();
                            }
                            Err(_) => {
                                stream.write_all(b"HTTP/1.1 500 Internal Server Error\r\n\r\n").unwrap();
                            }
                        }
                    },
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
