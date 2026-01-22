use crate::http::request::HttpMethod;
use crate::http::{HttpRequest, HttpResponse};

pub fn handle_file_request(path: &str, request: &HttpRequest, directory: &str) -> HttpResponse {
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
