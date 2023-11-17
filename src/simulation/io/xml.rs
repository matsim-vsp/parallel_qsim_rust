use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

use flate2::Compression;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tracing::info;

pub fn read_from_file<T>(file_path: &str) -> T
where
    T: DeserializeOwned,
{
    info!("xml_reader::read: Starting to read file at: {}", file_path);
    let file = File::open(file_path)
        .unwrap_or_else(|_| panic!("xml_reader::read: Could not open file at {}", file_path));
    let buffered_reader = BufReader::new(file);

    // I guess this could be prettier, but I don't know how to achieve this in Rust yet :-/
    if file_path.ends_with(".xml.gz") {
        // use full name, to avoid ambiguity
        let decoder = flate2::read::GzDecoder::new(buffered_reader);
        let buffered_decoder = BufReader::new(decoder);
        let mut deserializer = quick_xml::de::Deserializer::from_reader(buffered_decoder);

        match serde_path_to_error::deserialize(&mut deserializer) {
            Ok(parsed) => parsed,
            Err(_err) => {
                panic!("{_err:#?}");
            }
        }
    } else if file_path.ends_with(".xml") {
        let mut deserializer = quick_xml::de::Deserializer::from_reader(buffered_reader);
        match serde_path_to_error::deserialize(&mut deserializer) {
            Ok(x) => x,
            Err(_e) => {
                panic!("Problem reading file: {_e:?}")
            }
        }
    } else {
        panic!(
            "xml_reader::read: Can't open file path: {}. Only files with endings '.xml' or '.xml.gz' are supported.",
            file_path
        );
    }
}

pub fn write_to_file<T: Serialize>(serde_message: &T, path: &Path, dtd_spec: &str) {
    // Create the file and all necessary directories
    // this doesn't cover some edge cases, but this will do for now
    //let path = Path::new(file_path);
    let prefix = path.parent().unwrap();
    fs::create_dir_all(prefix).unwrap();
    let file = File::create(path).unwrap();
    let mut file_writer = BufWriter::new(file);
    //let header = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE network SYSTEM \"http://www.matsim.org/files/dtd/{dtd_spec}\">");

    info!("Starting to write file to: {path:?}");
    if path.extension().unwrap().eq("gz") {
        let mut compressor = flate2::write::GzEncoder::new(file_writer, Compression::fast());
        compressor
            .write_all(dtd_spec.as_bytes())
            .expect("Failed to write header");
        // serialize the actual message
        quick_xml::se::to_writer(compressor, &serde_message)
            .expect("Failed to write message to file");
    } else if path.extension().unwrap().eq("xml") {
        file_writer
            .write_all(dtd_spec.as_bytes())
            .expect("Failed to write header");
        quick_xml::se::to_writer(file_writer, &serde_message)
            .expect("failed to write serde message");
    } else {
        panic!("Tried to write {path:?}. File format not supported. Either use `.xml`, `.xml.gz`, or `.binpb` as extension");
    }
    info!("Finished writing file to: {path:?}");
}

#[cfg(test)]
mod tests {
    use crate::simulation::io::xml::read_from_file;

    // only testing the invalid case here, since the other cases
    // are implicitly tested when data containers are loaded e.g. in
    // network and population
    #[test]
    #[should_panic]
    fn unsupported_ending() {
        read_from_file("file-path-with-unsupported.ending")
    }
}
