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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::request::HttpMethod;
    use std::collections::HashMap;
    use std::io::Read;
    use std::net::{Shutdown, TcpListener, TcpStream};

    fn connected_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let client = TcpStream::connect(addr).unwrap();
        let (server, _) = listener.accept().unwrap();
        (server, client)
    }

    fn read_all(mut stream: TcpStream) -> Vec<u8> {
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).unwrap();
        buf
    }

    fn split_headers_body(resp: &[u8]) -> (&[u8], &[u8]) {
        let needle = b"\r\n\r\n";
        let idx = resp
            .windows(needle.len())
            .position(|w| w == needle)
            .expect("missing header/body separator");

        (&resp[..idx], &resp[idx + needle.len()..])
    }

    fn get_header_value(headers: &str, name: &str) -> Option<String> {
        let wanted = name.to_lowercase();
        for line in headers.lines() {
            if let Some((k, v)) = line.split_once(": ") {
                if k.to_lowercase() == wanted {
                    return Some(v.to_string());
                }
            }
        }
        None
    }

    fn make_request(headers: HashMap<String, String>) -> HttpRequest {
        HttpRequest {
            method: HttpMethod::Get,
            path: "/".to_string(),
            headers,
            body: vec![],
        }
    }

    #[test]
    fn new_sets_status_content_type_and_body() {
        let resp = HttpResponse::new("200 OK", "text/plain", b"hello".to_vec());

        assert_eq!(resp.status, "200 OK");
        assert_eq!(
            resp.headers.get("Content-Type").map(|s| s.as_str()),
            Some("text/plain")
        );
        assert_eq!(resp.body, b"hello");
    }

    #[test]
    fn send_writes_status_headers_and_body() {
        let (server, client) = connected_pair();

        let req = make_request(HashMap::new());
        let resp = HttpResponse::new("200 OK", "text/plain", b"hello".to_vec());

        resp.send(&server, &req);
        server.shutdown(Shutdown::Write).unwrap();

        let raw = read_all(client);
        let (headers, body) = split_headers_body(&raw);
        let headers_str = std::str::from_utf8(headers).unwrap();

        assert!(headers_str.starts_with("HTTP/1.1 200 OK\r\n"));
        assert_eq!(
            get_header_value(headers_str, "Content-Type").as_deref(),
            Some("text/plain")
        );
        assert_eq!(
            get_header_value(headers_str, "Content-Length").as_deref(),
            Some("5")
        );
        assert_eq!(body, b"hello");
    }

    #[test]
    fn send_adds_connection_close_if_requested() {
        let (server, client) = connected_pair();

        let mut headers = HashMap::new();
        headers.insert("connection".to_string(), "close".to_string());

        let req = make_request(headers);
        let resp = HttpResponse::new("200 OK", "text/plain", vec![]);

        resp.send(&server, &req);
        server.shutdown(Shutdown::Write).unwrap();

        let raw = read_all(client);
        let (headers, _body) = split_headers_body(&raw);
        let headers_str = std::str::from_utf8(headers).unwrap();

        assert_eq!(
            get_header_value(headers_str, "Connection").as_deref(),
            Some("close")
        );
    }

    #[test]
    fn send_gzips_body_when_accept_encoding_contains_gzip() {
        use flate2::read::GzDecoder;

        let (server, client) = connected_pair();

        let mut headers = HashMap::new();
        headers.insert("accept-encoding".to_string(), "gzip".to_string());

        let req = make_request(headers);
        let resp = HttpResponse::new("200 OK", "text/plain", b"hello gzip".to_vec());

        resp.send(&server, &req);
        server.shutdown(Shutdown::Write).unwrap();

        let raw = read_all(client);
        let (headers, body) = split_headers_body(&raw);
        let headers_str = std::str::from_utf8(headers).unwrap();

        assert_eq!(
            get_header_value(headers_str, "Content-Encoding").as_deref(),
            Some("gzip")
        );

        let content_length = get_header_value(headers_str, "Content-Length")
            .expect("missing Content-Length")
            .parse::<usize>()
            .unwrap();
        assert_eq!(content_length, body.len());

        let mut decoder = GzDecoder::new(body);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();

        assert_eq!(decompressed, b"hello gzip");
    }

    #[test]
    fn send_gzips_body_when_accept_encoding_is_a_list_containing_gzip() {
        use flate2::read::GzDecoder;

        let (server, client) = connected_pair();

        let mut headers = HashMap::new();
        headers.insert(
            "accept-encoding".to_string(),
            "br, gzip, deflate".to_string(),
        );

        let req = make_request(headers);
        let resp = HttpResponse::new("200 OK", "text/plain", b"abc123".to_vec());

        resp.send(&server, &req);
        server.shutdown(Shutdown::Write).unwrap();

        let raw = read_all(client);
        let (headers, body) = split_headers_body(&raw);
        let headers_str = std::str::from_utf8(headers).unwrap();

        assert_eq!(
            get_header_value(headers_str, "Content-Encoding").as_deref(),
            Some("gzip")
        );

        let mut decoder = GzDecoder::new(body);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();

        assert_eq!(decompressed, b"abc123");
    }

    #[test]
    fn send_does_not_gzip_when_not_requested() {
        let (server, client) = connected_pair();

        let req = make_request(HashMap::new());
        let resp = HttpResponse::new("200 OK", "text/plain", b"plain body".to_vec());

        resp.send(&server, &req);
        server.shutdown(Shutdown::Write).unwrap();

        let raw = read_all(client);
        let (headers, body) = split_headers_body(&raw);
        let headers_str = std::str::from_utf8(headers).unwrap();

        assert_eq!(get_header_value(headers_str, "Content-Encoding"), None);
        assert_eq!(body, b"plain body");
    }
}
