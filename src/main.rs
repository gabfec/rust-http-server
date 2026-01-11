use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::env;
use std::thread;
use flate2::{Compression, write::GzEncoder};

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

#[derive(Debug)]
struct HttpResponse {
    status: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl HttpRequest {
    fn from_stream(mut stream: &TcpStream) -> Option<Self> {
        let mut buffer = [0; 4096]; // Slightly larger buffer is safer
        let bytes_read = stream.read(&mut buffer).ok()?;
        if bytes_read == 0 {
            return None;
        }

        let (header_part, body_initial) = Self::split_request(&buffer[..bytes_read])?;

        // Parse Metadata
        let mut lines = header_part.lines();
        let (method, path) = Self::parse_request_line(lines.next()?)?;
        let headers = Self::parse_headers(&mut lines);

        // Handle Body (including multi-read)
        let body = Self::read_full_body(stream, body_initial, &headers);

        Some(HttpRequest { method, path, headers, body })
    }

    // Helper: Split bytes into header string and initial body slice
    fn split_request(buffer: &[u8]) -> Option<(String, &[u8])> {
        let pos = buffer.windows(4).position(|w| w == b"\r\n\r\n")?;
        let header_str = String::from_utf8_lossy(&buffer[..pos]).to_string();
        Some((header_str, &buffer[pos + 4..]))
    }

    // Helper: Parse first line
    fn parse_request_line(line: &str) -> Option<(HttpMethod, String)> {
        let mut parts = line.split_whitespace();
        let method = match parts.next()? {
            "POST" => HttpMethod::POST,
            _ => HttpMethod::GET,
        };
        let path = parts.next()?.to_string();
        Some((method, path))
    }

    // Helper: Parse headers into HashMap using functional style
    fn parse_headers(lines: &mut std::str::Lines) -> HashMap<String, String> {
        lines
            .filter_map(|line| line.split_once(": "))
            .map(|(k, v)| (k.to_lowercase(), v.to_string()))
            .collect()
    }

    // Helper: Complete the body read
    fn read_full_body(mut stream: &TcpStream, initial: &[u8], headers: &HashMap<String, String>) -> Vec<u8> {
        let content_length = headers.get("content-length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let mut body = initial.to_vec();
        while body.len() < content_length {
            let mut chunk = [0; 2048];
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => body.extend_from_slice(&chunk[..n]),
            }
        }
        body
    }
}

impl HttpResponse {
    // A helper to make creating common responses easier
    fn new(status: &str, content_type: &str, body: Vec<u8>) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), content_type.to_string());

        Self {
            status: status.to_string(),
            headers,
            body,
        }
    }
}

fn compress_body(data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap() // Returns the compressed Vec<u8>
}

fn send_response(mut stream: TcpStream, req: HttpRequest, mut res: HttpResponse) {
    // Handle GZIP Compression
    let accept_encoding = req.headers.get("accept-encoding").map(|s| s.as_str()).unwrap_or("");
    if accept_encoding.split(',').any(|s| s.trim() == "gzip") {
        res.body = compress_body(&res.body);
        res.headers.insert("Content-Encoding".to_string(), "gzip".to_string());
    }

    // Update Content-Length based on the final body size
    res.headers.insert("Content-Length".to_string(), res.body.len().to_string());

    // Construct the header string
    let mut response_string = format!("HTTP/1.1 {}\r\n", res.status);
    for (key, value) in &res.headers {
        response_string.push_str(&format!("{}: {}\r\n", key, value));
    }
    response_string.push_str("\r\n"); // The critical empty line

    // Send everything
    stream.write_all(response_string.as_bytes()).unwrap();
    stream.write_all(&res.body).unwrap();
}

fn handle_file_request(path: &str, request: &HttpRequest, directory: &str) -> HttpResponse {
    let filename = &path[7..];
    let file_path = std::path::Path::new(directory).join(filename);

    match request.method {
        HttpMethod::GET => {
            if file_path.exists() {
                let content = std::fs::read(file_path).unwrap_or_default();
                HttpResponse::new("200 OK", "application/octet-stream", content)
            } else {
                HttpResponse::new("404 Not Found", "text/plain", vec![])
            }
        }
        HttpMethod::POST => {
            match std::fs::write(file_path, &request.body) {
                Ok(_) => HttpResponse::new("201 Created", "text/plain", vec![]),
                Err(_) => HttpResponse::new("500 Internal Server Error", "text/plain", vec![]),
            }
        }
    }
}

fn handle_connection(stream: TcpStream, directory: String) {
    let request = match HttpRequest::from_stream(&stream) {
        Some(req) => req,
        None => return,
    };

    println!("Request received for path: {}", request.path);

    let response = match request.path.as_str() {
        "/" => HttpResponse::new("200 OK", "text/plain", vec![]),

        p if p.starts_with("/echo/") => {
            let content = p[6..].as_bytes().to_vec();
            HttpResponse::new("200 OK", "text/plain", content)
        }

        "/user-agent" => {
            let ua = request.headers.get("user-agent").cloned().unwrap_or_default();
            HttpResponse::new("200 OK", "text/plain", ua.into_bytes())
        }

        p if p.starts_with("/files/") => {
            handle_file_request(p, &request, &directory)
        }

        _ => HttpResponse::new("404 Not Found", "text/plain", vec![]),
    };

    // This is where the magic happens: GZIP, Headers, and Writing
    send_response(stream, request, response);
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
