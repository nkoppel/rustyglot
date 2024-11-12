#![allow(unused_must_use)]
#![allow(dead_code)]

mod args;
mod books;
mod conversions;
mod pgn;

fn main() {
    // let mut reader = BufReader::new(File::open("out2.bin.blob").unwrap());
    // let mut book = BookMap::new();
    // book.extend_from_reader(&mut reader);
    // let mut book = BookMap::read_txt(&mut reader);
    // book.write_blob(&mut File::create("out1.bin.blob").unwrap());

    args::run();
}
