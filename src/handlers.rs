use crate::http::request::HttpMethod;
use crate::http::{HttpRequest, HttpResponse};

pub async fn handle_file_request(
    path: &str,
    request: &HttpRequest,
    directory: &str,
) -> HttpResponse {
    let filename = &path[7..];
    let file_path = std::path::Path::new(directory).join(filename);

    match request.method {
        HttpMethod::Get => {
            if file_path.exists() {
                match tokio::fs::read(file_path).await {
                    Ok(content) => HttpResponse::new("200 OK", "application/octet-stream", content),
                    Err(_) => HttpResponse::new("500 Internal Server Error", "text/plain", vec![]),
                }
            } else {
                HttpResponse::new("404 Not Found", "text/plain", vec![])
            }
        }
        HttpMethod::Post => match tokio::fs::write(file_path, &request.body).await {
            Ok(_) => HttpResponse::new("201 Created", "text/plain", vec![]),
            Err(_) => HttpResponse::new("500 Internal Server Error", "text/plain", vec![]),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::request::HttpMethod;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    async fn connected_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_fut = TcpStream::connect(addr);
        let accept_fut = listener.accept();

        let (client_res, server_res) = tokio::join!(client_fut, accept_fut);

        let client = client_res.unwrap();
        let (server, _) = server_res.unwrap();

        (server, client)
    }

    async fn read_to_end(mut client: TcpStream) -> Vec<u8> {
        let mut buf = Vec::new();
        client.read_to_end(&mut buf).await.unwrap();
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

    fn make_temp_dir() -> PathBuf {
        let mut dir = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.push(format!("cc_http_server_test_{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn req_for_send() -> crate::http::HttpRequest {
        // Make the server echo Connection: close so tests can read to end after shutdown
        let mut headers = HashMap::new();
        headers.insert("connection".to_string(), "close".to_string());

        crate::http::HttpRequest {
            method: HttpMethod::Get,
            path: "/".to_string(),
            headers,
            body: vec![],
        }
    }

    #[tokio::test]
    async fn file_get_existing_returns_200_and_body() {
        let dir = make_temp_dir();
        let file_path = dir.join("a.txt");
        fs::write(&file_path, b"abc").unwrap();

        let request = crate::http::HttpRequest {
            method: HttpMethod::Get,
            path: "/files/a.txt".to_string(),
            headers: HashMap::new(),
            body: vec![],
        };

        let resp = handle_file_request("/files/a.txt", &request, dir.to_str().unwrap()).await;

        let (mut server, client) = connected_pair().await;
        let req = req_for_send();
        resp.send(&mut server, &req).await.unwrap();
        server.shutdown().await.unwrap();

        let raw = read_to_end(client).await;
        let (hdrs, body) = split_headers_body(&raw);
        let hdrs_str = std::str::from_utf8(hdrs).unwrap();

        assert!(hdrs_str.starts_with("HTTP/1.1 200 OK\r\n"));
        assert_eq!(body, b"abc");

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn file_get_missing_returns_404() {
        let dir = make_temp_dir();

        let request = crate::http::HttpRequest {
            method: HttpMethod::Get,
            path: "/files/missing.txt".to_string(),
            headers: HashMap::new(),
            body: vec![],
        };

        let resp = handle_file_request("/files/missing.txt", &request, dir.to_str().unwrap()).await;

        let (mut server, client) = connected_pair().await;
        let req = req_for_send();
        resp.send(&mut server, &req).await.unwrap();
        server.shutdown().await.unwrap();

        let raw = read_to_end(client).await;
        let (hdrs, _body) = split_headers_body(&raw);
        let hdrs_str = std::str::from_utf8(hdrs).unwrap();

        assert!(hdrs_str.starts_with("HTTP/1.1 404 Not Found\r\n"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn file_post_creates_file_and_returns_201() {
        let dir = make_temp_dir();

        let request = crate::http::HttpRequest {
            method: HttpMethod::Post,
            path: "/files/new.txt".to_string(),
            headers: HashMap::new(),
            body: b"hello".to_vec(),
        };

        let resp = handle_file_request("/files/new.txt", &request, dir.to_str().unwrap()).await;

        // verify file written
        let written = fs::read(dir.join("new.txt")).unwrap();
        assert_eq!(written, b"hello");

        // verify status 201
        let (mut server, client) = connected_pair().await;
        let req = req_for_send();
        resp.send(&mut server, &req).await.unwrap();
        server.shutdown().await.unwrap();

        let raw = read_to_end(client).await;
        let (hdrs, _body) = split_headers_body(&raw);
        let hdrs_str = std::str::from_utf8(hdrs).unwrap();

        assert!(hdrs_str.starts_with("HTTP/1.1 201 Created\r\n"));

        let _ = fs::remove_dir_all(&dir);
    }
}
