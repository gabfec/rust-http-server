use crate::http::HttpRequest;
use crate::utils;
use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;

#[derive(Debug)]
pub struct HttpResponse {
    status: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl HttpResponse {
    // A helper to make creating common responses easier
    pub fn new(status: &str, content_type: &str, body: Vec<u8>) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), content_type.to_string());

        Self {
            status: status.to_string(),
            headers,
            body,
        }
    }

    pub fn send(mut self, mut stream: &TcpStream, req: &HttpRequest) {
        // Handle GZIP Compression
        let accept_encoding = req
            .headers
            .get("accept-encoding")
            .map(|s| s.as_str())
            .unwrap_or("");
        if accept_encoding.split(',').any(|s| s.trim() == "gzip") {
            self.body = utils::compress_body(&self.body);
            self.headers
                .insert("Content-Encoding".to_string(), "gzip".to_string());
        }

        // Update Content-Length based on the final body size
        self.headers
            .insert("Content-Length".to_string(), self.body.len().to_string());

        // If the client asked to close, we should echo that back
        if let Some(conn) = req.headers.get("connection")
            && conn.to_lowercase() == "close"
        {
            self.headers
                .insert("Connection".to_string(), "close".to_string());
        }

        // Construct the header string
        let mut response_string = format!("HTTP/1.1 {}\r\n", self.status);
        for (key, value) in &self.headers {
            response_string.push_str(&format!("{}: {}\r\n", key, value));
        }
        response_string.push_str("\r\n"); // The critical empty line

        // Send everything
        stream.write_all(response_string.as_bytes()).unwrap();
        stream.write_all(&self.body).unwrap();
        stream.flush().unwrap(); // Critical for persistent connections!
    }
}
