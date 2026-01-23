mod handlers;
mod http;
mod server;
mod utils;

use std::env;

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    let args: Vec<String> = env::args().collect();
    let directory = if args.len() > 2 && args[1] == "--directory" {
        args[2].clone()
    } else {
        ".".to_string() // Default to current dir
    };

    let server = server::Server::new("127.0.0.1:4221".to_string());
    server.run(directory);
}
