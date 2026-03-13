use crate::handlers;
use crate::http::{HttpRequest, HttpResponse};
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};

pub struct Server {
    addr: String,
}

impl Server {
    pub fn new(addr: String) -> Self {
        Self { addr }
    }

    pub async fn run(self, directory: String) {
        let listener = TcpListener::bind(&self.addr).await.unwrap();

        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    println!("accepted new connection");
                    let dir = directory.clone();

                    tokio::spawn(async move {
                        Server::handle_connection(stream, dir).await;
                    });
                }
                Err(e) => {
                    eprintln!("error accepting connection: {e}");
                }
            }
        }
    }

    async fn handle_connection(stream: TcpStream, directory: String) {
        let mut reader = BufReader::new(stream);

        loop {
            let request = match HttpRequest::from_stream(&mut reader).await {
                Some(req) => req,
                None => {
                    println!("Connection closed by client.");
                    break;
                }
            };

            println!("request received for path: {}", request.path);

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

                p if p.starts_with("/files/") => {
                    handlers::handle_file_request(p, &request, &directory).await
                }

                _ => HttpResponse::new("404 Not Found", "text/plain", vec![]),
            };

            // This is where the magic happens: GZIP, Headers, and Writing
            let stream = reader.get_mut();
            if response.send(stream, &request).await.is_err() {
                eprintln!("error sending response");
                break;
            }

            // Check if we should close the connection
            // HTTP/1.1 is persistent by default, but clients can send "Connection: close"
            if let Some(conn_header) = request.headers.get("connection")
                && conn_header.to_lowercase() == "close"
            {
                break;
            }
        }
    }
}
