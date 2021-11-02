#![allow(unused_must_use)]
#![allow(dead_code)]

mod conversions;
mod pgn;
mod books;
mod args;

use conversions::*;
use pgn::*;
use books::*;

use std::io::{self, Cursor, BufReader};
use std::fs::File;

fn main() {
    // let mut reader = BufReader::new(File::open("out2.bin.blob").unwrap());
    // let mut book = BookMap::new();
    // book.extend_from_reader(&mut reader);
    // let mut book = BookMap::read_txt(&mut reader);
    // book.write_blob(&mut File::create("out1.bin.blob").unwrap());
    
    args::run();
}
