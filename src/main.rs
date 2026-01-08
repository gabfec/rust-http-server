#[allow(unused_imports)]
use std::net::TcpListener;
use std::io::{Read, Write};
use std::net::TcpStream;

#[derive(Debug)]
enum HttpMethod {
    GET,
    POST,
}

#[derive(Debug)]
struct HttpRequest {
    method: HttpMethod,
    path: String,
}

impl HttpRequest {
    fn from_stream(mut stream: &TcpStream) -> Option<Self> {
        let mut buffer = [0; 1024];
        let bytes_read = stream.read(&mut buffer).ok()?;

        if bytes_read == 0 {
            return None;
        }

        let request_str = String::from_utf8_lossy(&buffer);
        let mut lines = request_str.lines();

        // Parse the Request Line (e.g., "GET /index.html HTTP/1.1")
        if let Some(first_line) = lines.next() {
            let parts: Vec<&str> = first_line.split_whitespace().collect();

            if parts.len() >= 2 {
                let method = match parts[0] {
                    "POST" => HttpMethod::POST,
                    _ => HttpMethod::GET, // Default to GET for now
                };

                return Some(HttpRequest {
                    method: method,
                    path: parts[1].to_string(),
                });
            }
        }
        None
    }
}

fn handle_connection(mut stream: TcpStream) {
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
            }
            _ => stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").unwrap(),
        }
    }
}

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("accepted new connection");
                handle_connection(stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
