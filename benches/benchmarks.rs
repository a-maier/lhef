use std::io::{BufReader, Read};
use std::fs::File;

use criterion::{criterion_group, criterion_main, Criterion};
use flate2::bufread::GzDecoder;
use lhef::{Reader, Writer};

fn criterion_benchmark(c: &mut Criterion) {

    let file = File::open("test_data/2j.lhe.gz").expect("file not found");
    let mut reader = GzDecoder::new(BufReader::new(file));
    let mut event_txt = Vec::new();
    reader.read_to_end(&mut event_txt).unwrap();
    let event_txt = event_txt;
    c.bench_function(
        "read",
        |b| b.iter(
            || {
                let reader = BufReader::new(event_txt.as_slice());
                let mut lhef = Reader::new(reader).unwrap();
                let mut nevents = 0;
                while let Ok(Some(_)) = lhef.hepeup() {
                    nevents += 1
                }
                assert_eq!(nevents, 1628);
            }
        )
    );

    let mut events = Vec::new();
    let reader = BufReader::new(event_txt.as_slice());
    let mut lhef = Reader::new(reader).unwrap();
    while let Ok(Some(event)) = lhef.hepeup() {
        events.push(event)
    }
    c.bench_function(
        "write",
        |b| b.iter(
            || {
                let mut writer = Writer::new(std::io::sink(),  lhef.version()).unwrap();
                writer.header(lhef.header()).unwrap();
                if let Some(xml_header) = lhef.xml_header() {
                    writer.xml_header(xml_header).unwrap();
                }
                writer.heprup(lhef.heprup()).unwrap();
                for event in &events {
                    writer.hepeup(event).unwrap();
                }
                writer.finish().unwrap();
            }
        )
    );

}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
