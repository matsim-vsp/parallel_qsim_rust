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
        serde_path_to_error::deserialize(&mut deserializer)
            .unwrap_or_else(|_e| panic!("Problem reading file: {_e:?}"))
    } else {
        panic!(
            "xml_reader::read: Can't open file path: {}. Only files with endings '.xml' or '.xml.gz' are supported.",
            file_path
        );
    }
}

// Adapter from std::fmt::Write to std::io::Write. Needed because of an odd API of quick-xml.
// See https://github.com/tafia/quick-xml/issues/499 for more details.
struct ToFmtWrite<T>(pub T);

impl<T> std::fmt::Write for ToFmtWrite<T>
where
    T: Write,
{
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write_all(s.as_bytes()).map_err(|_| std::fmt::Error)
    }
}

pub fn write_to_file<T: Serialize>(serde_message: &T, path: &Path, dtd_spec: &str) {
    let prefix = path.parent().unwrap();
    fs::create_dir_all(prefix).unwrap();
    let file = File::create(path).unwrap();
    let mut file_writer = BufWriter::new(file);

    info!("Starting to write file to: {path:?}");
    if path.extension().unwrap().eq("gz") {
        let mut compressor = flate2::write::GzEncoder::new(file_writer, Compression::fast());
        compressor
            .write_all(dtd_spec.as_bytes())
            .expect("Failed to write header");
        // serialize the actual message
        quick_xml::se::to_writer(ToFmtWrite(compressor), &serde_message)
            .expect("Failed to write message to file");
    } else if path.extension().unwrap().eq("xml") {
        file_writer
            .write_all(dtd_spec.as_bytes())
            .expect("Failed to write header");
        quick_xml::se::to_writer(ToFmtWrite(file_writer), &serde_message)
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
