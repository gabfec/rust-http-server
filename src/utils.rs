use flate2::{Compression, write::GzEncoder};
use std::io::Write;

pub fn compress_body(data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap() // Returns the compressed Vec<u8>
}
