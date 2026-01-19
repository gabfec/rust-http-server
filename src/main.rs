use flate2::{Compression, write::GzEncoder};
use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

#[derive(Debug)]
enum HttpMethod {
    Get,
    Post,
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
    fn from_stream(reader: &mut BufReader<&TcpStream>) -> Option<Self> {
        let mut first_line = String::new();
        reader.read_line(&mut first_line).ok()?;
        if first_line.is_empty() {
            return None;
        }

        // Parse Metadata
        let (method, path) = Self::parse_request_line(&first_line)?;
        let headers = Self::parse_headers(reader)?;

        // Handle Body (including multi-read)
        let body = Self::read_body(reader, &headers)?;

        Some(HttpRequest {
            method,
            path,
            headers,
            body,
        })
    }

    // Helper: Parse first line
    fn parse_request_line(line: &str) -> Option<(HttpMethod, String)> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let method = match parts.first()? {
            &"POST" => HttpMethod::Post,
            _ => HttpMethod::Get,
        };
        let path = parts.get(1)?.to_string();
        Some((method, path))
    }

    // Helper: Parse headers into HashMap using functional style
    fn parse_headers(reader: &mut BufReader<&TcpStream>) -> Option<HashMap<String, String>> {
        let mut headers = HashMap::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).ok()?;
            if line == "\r\n" || line == "\n" {
                break;
            }

            if let Some((k, v)) = line.split_once(": ") {
                headers.insert(k.to_lowercase(), v.trim().to_string());
            }
        }
        Some(headers)
    }

    // Helper: Complete the body read
    fn read_body(
        reader: &mut BufReader<&TcpStream>,
        headers: &HashMap<String, String>,
    ) -> Option<Vec<u8>> {
        let len = headers
            .get("content-length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let mut body = vec![0u8; len];
        reader.read_exact(&mut body).ok()?;
        Some(body)
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

fn send_response(mut stream: &TcpStream, req: &HttpRequest, mut res: HttpResponse) {
    // Handle GZIP Compression
    let accept_encoding = req
        .headers
        .get("accept-encoding")
        .map(|s| s.as_str())
        .unwrap_or("");
    if accept_encoding.split(',').any(|s| s.trim() == "gzip") {
        res.body = compress_body(&res.body);
        res.headers
            .insert("Content-Encoding".to_string(), "gzip".to_string());
    }

    // Update Content-Length based on the final body size
    res.headers
        .insert("Content-Length".to_string(), res.body.len().to_string());

    // If the client asked to close, we should echo that back
    if let Some(conn) = req.headers.get("connection")
        && conn.to_lowercase() == "close"
    {
        res.headers
            .insert("Connection".to_string(), "close".to_string());
    }

    // Construct the header string
    let mut response_string = format!("HTTP/1.1 {}\r\n", res.status);
    for (key, value) in &res.headers {
        response_string.push_str(&format!("{}: {}\r\n", key, value));
    }
    response_string.push_str("\r\n"); // The critical empty line

    // Send everything
    stream.write_all(response_string.as_bytes()).unwrap();
    stream.write_all(&res.body).unwrap();
    stream.flush().unwrap(); // Critical for persistent connections!
}

fn handle_file_request(path: &str, request: &HttpRequest, directory: &str) -> HttpResponse {
    let filename = &path[7..];
    let file_path = std::path::Path::new(directory).join(filename);

    match request.method {
        HttpMethod::Get => {
            if file_path.exists() {
                let content = std::fs::read(file_path).unwrap_or_default();
                HttpResponse::new("200 OK", "application/octet-stream", content)
            } else {
                HttpResponse::new("404 Not Found", "text/plain", vec![])
            }
        }
        HttpMethod::Post => match std::fs::write(file_path, &request.body) {
            Ok(_) => HttpResponse::new("201 Created", "text/plain", vec![]),
            Err(_) => HttpResponse::new("500 Internal Server Error", "text/plain", vec![]),
        },
    }
}

fn handle_connection(stream: TcpStream, directory: String) {
    let mut reader = BufReader::new(&stream);

    loop {
        let request = match HttpRequest::from_stream(&mut reader) {
            Some(req) => req,
            None => {
                println!("Connection closed by client.");
                break;
            }
        };

        println!("Request received for path: {}", request.path);

        let response = match request.path.as_str() {
            "/" => HttpResponse::new("200 OK", "text/plain", vec![]),

            p if p.starts_with("/echo/") => {
                let content = p.as_bytes()[6..].to_vec();
                HttpResponse::new("200 OK", "text/plain", content)
            }

            "/user-agent" => {
                let ua = request
                    .headers
                    .get("user-agent")
                    .cloned()
                    .unwrap_or_default();
                HttpResponse::new("200 OK", "text/plain", ua.into_bytes())
            }

            p if p.starts_with("/files/") => handle_file_request(p, &request, &directory),

            _ => HttpResponse::new("404 Not Found", "text/plain", vec![]),
        };

        // This is where the magic happens: GZIP, Headers, and Writing
        send_response(&stream, &request, response);

        // Check if we should close the connection
        // HTTP/1.1 is persistent by default, but clients can send "Connection: close"
        if let Some(conn_header) = request.headers.get("connection")
            && conn_header.to_lowercase() == "close"
        {
            break;
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
                thread::spawn(|| {
                    handle_connection(stream, dir);
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
