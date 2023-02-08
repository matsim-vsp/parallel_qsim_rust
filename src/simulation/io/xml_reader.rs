use log::{debug, info};
use quick_xml::de::from_reader;
use serde::de::DeserializeOwned;
use std::fs;
use std::fs::File;
use std::io::BufReader;

pub fn read<T>(file_path: &str) -> T
where
    T: DeserializeOwned,
{
    info!("xml_reader::read: Starting to read file at: {}", file_path);
    let file = File::open(file_path)
        .unwrap_or_else(|_| panic!("xml_reader::read: Could not open file at {}", file_path));
    let buffered_reader = BufReader::new(file);

    // I guess this could be prettier, but I don't know how to achieve this in Rust yet :-/
    return if file_path.ends_with(".xml.gz") {
        // use full name, to avoid ambiguity
        let decoder = flate2::read::GzDecoder::new(buffered_reader);
        let buffered_decoder = BufReader::new(decoder);
        let result: T = from_reader(buffered_decoder).unwrap();
        result
    } else if file_path.ends_with(".xml") {
        let s = fs::read_to_string(file_path).expect("Couldn't find file.");
        debug!("File content of {}:\n{}", file_path, s);
        let result: Result<T, _> = from_reader(buffered_reader);
        match result {
            Ok(x) => x,
            Err(e) => panic!("Problem reading file: {:?}", e),
        }
    } else {
        panic!(
            "xml_reader::read: Can't open file path: {}. Only files with endings '.xml' or '.xml.gz' are supported.",
            file_path
        );
    };
}

#[cfg(test)]
mod tests {
    use crate::simulation::io::xml_reader::read;

    // only testing the invalid case here, since the other cases
    // are implicitly tested when data containers are loaded e.g. in
    // network and population
    #[test]
    #[should_panic]
    fn unsupported_ending() {
        read("file-path-with-unsupported.ending")
    }
}
