use std::fs::File;
use std::io::BufReader;

use serde::de::DeserializeOwned;
use tracing::info;

pub fn read<T>(file_path: &str) -> T
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
