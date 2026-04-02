use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use flate2::Compression;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::info;

pub mod attributes;
pub mod network;
pub mod population;
pub mod vehicles;

use crate::simulation::io::is_url;
use flate2::read::GzDecoder;

pub fn read_from_file<T>(file_path: impl AsRef<Path>) -> T
where
    T: DeserializeOwned,
{
    use quick_xml::de::Deserializer;

    // Check if it's a URL or local file and if it's gzipped or not
    let is_gz = file_path.as_ref().extension().unwrap() == "gz";
    if is_gz {
        assert!(
            file_path
                .as_ref()
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with("xml"),
            "File has .gz extension but the underlying file does not have .xml extension"
        );
    }

    // Build one `BufRead` reader for all cases
    let reader: Box<dyn BufRead> = if is_url(file_path.as_ref()) {
        #[cfg(feature = "http")]
        {
            url_file_reader(file_path, is_gz)
        }
        #[cfg(not(feature = "http"))]
        {
            panic!("Tried to read from URL, but feature http is not enabled");
        }
    } else {
        local_file_reader(file_path.as_ref(), is_gz)
    };

    // Parse the XML
    info!("Starting to read from file: {:?}", file_path.as_ref());
    let mut de = Deserializer::from_reader(reader);
    let res = match serde_path_to_error::deserialize(&mut de) {
        Ok(parsed) => parsed,
        Err(err) => {
            panic!("Failed to deserialize XML:\n{err:#?}");
        }
    };
    info!("Finished reading from file: {:?}", file_path.as_ref());
    res
}

fn local_file_reader(file_path: impl AsRef<Path>, is_gz: bool) -> Box<dyn BufRead> {
    // Local file path
    let file = File::open(file_path.as_ref()).unwrap_or_else(|_| {
        panic!(
            "xml_reader::read: Could not open file at {:?}",
            file_path.as_ref()
        )
    });

    if is_gz {
        // Local .xml.gz
        let gz = GzDecoder::new(file);
        Box::new(BufReader::new(gz))
    } else {
        // Local plain .xml
        Box::new(BufReader::new(file))
    }
}

#[cfg(feature = "http")]
fn url_file_reader(file_path: &str, is_gz: bool) -> Box<dyn BufRead> {
    // URL path
    let resp = reqwest::blocking::get(file_path).expect("Could not fetch URL");
    let bytes = resp.bytes().expect("Could not read response body");

    if is_gz {
        // URL .xml.gz
        let gz = GzDecoder::new(std::io::Cursor::new(bytes));
        Box::new(BufReader::new(gz))
    } else {
        // URL .xml
        Box::new(BufReader::new(std::io::Cursor::new(bytes)))
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

pub fn write_to_file<T: Serialize>(serde_message: &T, path: impl AsRef<Path>, dtd_spec: &str) {
    let prefix = path.as_ref().parent().unwrap();
    fs::create_dir_all(prefix).unwrap();
    let file = File::create(path.as_ref()).unwrap();
    let mut file_writer = BufWriter::new(file);

    info!("Starting to write file to: {:?}", path.as_ref());
    if path.as_ref().extension().unwrap().eq("gz") {
        let mut compressor = flate2::write::GzEncoder::new(file_writer, Compression::fast());

        // write header. first: dtd spec
        compressor
            .write_all(dtd_spec.as_bytes())
            .expect("Failed to write header");

        // then add two new lines for visual separation
        compressor
            .write_all(b"\n\n")
            .expect("Failed to write newlines");

        // set up serializer
        let mut tfw_compressor = ToFmtWrite(compressor);
        let mut serializer = quick_xml::se::Serializer::new(&mut tfw_compressor);
        serializer.indent(' ', 4); // configure indentation

        // serialize the actual message
        serde_message
            .serialize(serializer)
            .expect("Failed to write message to file");
    } else if path.as_ref().extension().unwrap().eq("xml") {
        // write header. first: dtd spec
        file_writer
            .write_all(dtd_spec.as_bytes())
            .expect("Failed to write header");

        // then add two new lines for visual separation
        file_writer
            .write_all(b"\n\n")
            .expect("Failed to write newlines");

        // set up serializer
        let mut tfw_file_writer = ToFmtWrite(file_writer);
        let mut serializer = quick_xml::se::Serializer::new(&mut tfw_file_writer);
        serializer.indent(' ', 4);

        // serialize the actual message
        serde_message
            .serialize(serializer)
            .expect("failed to write serde message");
    } else {
        panic!(
            "Tried to write {:?}. File format not supported. Either use `.xml`, `.xml.gz`, or `.binpb` as extension",
            path.as_ref()
        );
    }
    info!("Finished writing file to: {:?}", path.as_ref());
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
