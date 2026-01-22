use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;

#[derive(Debug)]
pub enum HttpMethod {
    Get,
    Post,
}

#[derive(Debug)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn from_stream(reader: &mut BufReader<&TcpStream>) -> Option<Self> {
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
