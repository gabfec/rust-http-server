use crate::handlers;
use crate::http::{HttpRequest, HttpResponse};
use std::io::BufReader;
use std::net::TcpStream;

pub fn handle_connection(stream: TcpStream, directory: String) {
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

            p if p.starts_with("/files/") => handlers::handle_file_request(p, &request, &directory),

            _ => HttpResponse::new("404 Not Found", "text/plain", vec![]),
        };

        // This is where the magic happens: GZIP, Headers, and Writing
        response.send(&stream, &request);

        // Check if we should close the connection
        // HTTP/1.1 is persistent by default, but clients can send "Connection: close"
        if let Some(conn_header) = request.headers.get("connection")
            && conn_header.to_lowercase() == "close"
        {
            break;
        }
    }
}
