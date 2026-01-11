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
        let mut buffer = [0; 2048];
        let bytes_read = stream.read(&mut buffer).ok()?;

        if bytes_read == 0 {
            return None;
        }

        // Find the delimiter between Headers and Body
        let separator = buffer[..bytes_read]
            .windows(4)
            .position(|window| window == b"\r\n\r\n")?;

        // Parse Headers from the header_bytes
        let header_str = String::from_utf8_lossy(&buffer[..separator]);
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
        let content_length = headers.get("content-length")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);

        let mut body = Vec::new();
        // Add what was already read into the initial buffer after the headers
        let initial_body_part = &buffer[separator + 4..bytes_read];
        body.extend_from_slice(initial_body_part);

        // Continue reading if the body was truncated in the first read
        while body.len() < content_length {
            let mut chunk = [0; 2048];
            let n = stream.read(&mut chunk).ok()?;
            if n == 0 { break; }
            body.extend_from_slice(&chunk[..n]);
        }

        Some(HttpRequest { method, path, headers, body })
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
