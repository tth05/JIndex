use std::fs::OpenOptions;
use std::io::{BufReader, BufWriter, Read, Write};

use flate2::bufread::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use speedy::{Readable, Reader, Writable, Writer};

use crate::ClassIndex;

pub fn load_class_index_from_file(path: String) -> ClassIndex {
    let file = OpenOptions::new().read(true).open(path).unwrap();

    let file_size = file.metadata().unwrap().len();

    let reader = BufReader::new(file);
    let mut output_buf = Vec::with_capacity(file_size as usize);
    let mut decoder = GzDecoder::new(reader);
    decoder
        .read_to_end(&mut output_buf)
        .expect("Decompression failed");

    ClassIndex::read_from_buffer(&output_buf).expect("Deserialization failed")
}

pub fn save_class_index_to_file(class_index: &ClassIndex, path: String) {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(path)
        .unwrap();

    let serialized_buf = class_index.write_to_vec().expect("Serialization failed");

    let mut writer = BufWriter::new(file);
    let output_buf = Vec::with_capacity(serialized_buf.len() / 2);

    let mut encoder = GzEncoder::new(output_buf, Compression::best());
    encoder
        .write_all(&serialized_buf)
        .expect("Compression failed");
    writer
        .write_all(&encoder.finish().unwrap())
        .expect("Write to file failed");
}
