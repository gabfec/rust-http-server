# HTTP Server in Rust (Built from Scratch)

[![Tests](https://github.com/gabfec/rust-http-server/actions/workflows/rust.yml/badge.svg)](https://github.com/gabfec/rust-http-server/actions)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)]()
[![Made with Rust](https://img.shields.io/badge/Made%20with-Rust-red.svg)]()
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org)

A minimal HTTP/1.1 server implemented from scratch in Rust using raw TCP sockets.

---

## Features

- Manual HTTP/1.1 request parsing
- Manual response construction
- Thread-per-connection concurrency
- Persistent connections (keep-alive)
- Gzip compression (when `Accept-Encoding: gzip` is sent)
- Static file serving
- File upload via POST
- Content-Length handling
- Proper CRLF formatting

---

## Supported Routes

| Route | Method | Description |
|-------|--------|-------------|
| `/` | GET | Returns `200 OK` |
| `/echo/{text}` | GET | Returns `{text}` |
| `/user-agent` | GET | Returns the `User-Agent` header |
| `/files/{filename}` | GET | Serves file from directory |
| `/files/{filename}` | POST | Writes body to file |

Unknown routes return `404 Not Found`.

---

## Run

Start the server:

```bash
cargo run
```

Serve files from a directory:

```bash
cargo run -- --directory ./public
```

Server runs on:

```
127.0.0.1:4221
```

---

## Example

```bash
curl http://localhost:4221/echo/hello
curl -X POST --data "data" http://localhost:4221/files/test.txt
curl -H "Accept-Encoding: gzip" http://localhost:4221/echo/hello
```

---

## Project Structure

```
.
├── main.rs
├── server.rs
├── handlers.rs
├── utils.rs
└── http/
    ├── request.rs
    └── response.rs
```

---

## Tests

Run tests:

```bash
cargo test
```

## Educational Purpose

This project was built to understand TCP networking and HTTP/1.1 internals by implementing a server without external frameworks.

Challenge by CodeCrafters: https://codecrafters.io

---

## License

MIT License — see the `LICENSE` file.
