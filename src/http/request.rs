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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Write};
    use std::net::{Shutdown, TcpListener, TcpStream};

    fn connected_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let client = TcpStream::connect(addr).unwrap();
        let (server, _) = listener.accept().unwrap();
        (server, client)
    }

    fn write_request_and_make_reader<'a>(
        server: &'a TcpStream,
        req: &[u8],
        mut client: TcpStream,
    ) -> BufReader<&'a TcpStream> {
        client.write_all(req).unwrap();
        client.flush().unwrap();
        client.shutdown(Shutdown::Write).unwrap(); // ensure server doesn't hang waiting for more data
        BufReader::new(server)
    }

    #[test]
    fn parse_request_line_get_defaults_to_get() {
        let (m, path) = HttpRequest::parse_request_line("GET /hello HTTP/1.1\r\n").unwrap();
        assert!(matches!(m, HttpMethod::Get));
        assert_eq!(path, "/hello");
    }

    #[test]
    fn parse_request_line_post() {
        let (m, path) = HttpRequest::parse_request_line("POST /files/a.txt HTTP/1.1\r\n").unwrap();
        assert!(matches!(m, HttpMethod::Post));
        assert_eq!(path, "/files/a.txt");
    }

    #[test]
    fn from_stream_parses_get_no_body() {
        let (server, client) = connected_pair();
        let req_bytes = b"GET /echo/hello HTTP/1.1\r\nHost: localhost\r\nUser-Agent: curl\r\n\r\n";

        let mut reader = write_request_and_make_reader(&server, req_bytes, client);
        let req = HttpRequest::from_stream(&mut reader).unwrap();

        assert!(matches!(req.method, HttpMethod::Get));
        assert_eq!(req.path, "/echo/hello");
        assert_eq!(
            req.headers.get("host").map(|s| s.as_str()),
            Some("localhost")
        );
        assert_eq!(
            req.headers.get("user-agent").map(|s| s.as_str()),
            Some("curl")
        );
        assert!(req.body.is_empty());
    }

    #[test]
    fn from_stream_parses_post_with_body() {
        let (server, client) = connected_pair();

        let body = b"hello world";
        let req = format!(
            "POST /files/x.txt HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            std::str::from_utf8(body).unwrap()
        );

        let mut reader = write_request_and_make_reader(&server, req.as_bytes(), client);
        let req = HttpRequest::from_stream(&mut reader).unwrap();

        assert!(matches!(req.method, HttpMethod::Post));
        assert_eq!(req.path, "/files/x.txt");
        assert_eq!(
            req.headers.get("content-length").unwrap(),
            &body.len().to_string()
        );
        assert_eq!(req.body, body);
    }

    #[test]
    fn header_keys_are_lowercased() {
        let (server, client) = connected_pair();
        let req_bytes = b"GET / HTTP/1.1\r\nUser-Agent: TestUA\r\nX-Custom: Value\r\n\r\n";

        let mut reader = write_request_and_make_reader(&server, req_bytes, client);
        let req = HttpRequest::from_stream(&mut reader).unwrap();

        assert_eq!(req.headers.get("user-agent").unwrap(), "TestUA");
        assert_eq!(req.headers.get("x-custom").unwrap(), "Value");
        assert!(req.headers.get("User-Agent").is_none());
    }

    #[test]
    fn returns_none_on_closed_connection() {
        let (server, client) = connected_pair();
        // Immediately close client's write side without sending anything
        client.shutdown(Shutdown::Write).unwrap();

        let mut reader = BufReader::new(&server);
        let req = HttpRequest::from_stream(&mut reader);
        assert!(req.is_none());
    }
}
