use flate2::bufread::GzDecoder;
use quick_xml::de::from_reader;
use serde::de::DeserializeOwned;
use std::fs::File;
use std::io::BufReader;

pub fn read<T>(file_path: &str) -> T
where
    T: DeserializeOwned,
{
    let file = File::open(file_path).unwrap();
    let buffered_reader = BufReader::new(file);

    // I guess this could be prettier, but I don't know how to achieve this in Rust yet :-/
    return if file_path.ends_with(".xml.gz") {
        let decoder = GzDecoder::new(buffered_reader);
        let buffered_decoder = BufReader::new(decoder);
        let result: T = from_reader(buffered_decoder).unwrap();
        result
    } else if file_path.ends_with(".xml") {
        let result: T = from_reader(buffered_reader).unwrap();
        return result;
    } else {
        panic!(
            "Can't open file path: {}. Only files with endings '.xml' or '.xml.gz' are supported.",
            file_path
        );
    };
}

#[cfg(test)]
mod tests {
    use crate::container::xml_reader::read;

    // only testing the invalid case here, since the other cases
    // are implicitly tested when data containers are loaded e.g. in
    // network and population
    #[test]
    #[should_panic]
    fn unsupported_ending() {
        read("file-path-with-unsupported.ending")
    }
}
