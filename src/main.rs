#[allow(unused_imports)]
use std::net::TcpListener;
use std::io::{Read, Write};
use std::net::TcpStream;

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();

    let request_str = String::from_utf8_lossy(&buffer);
    let lines: Vec<&str> = request_str.split("\r\n").collect();

    if let Some(request_line) = lines.get(0) {
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        let _method = parts[0];
        let path = parts[1];

        // Routing logic
        match path {
            "/" => stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n").unwrap(),
            _ => stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").unwrap(),
        }
    }
}

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("accepted new connection");
                handle_connection(stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
