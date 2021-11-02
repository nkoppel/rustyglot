use crate::conversions::fen_to_chess;
use crate::pgn::*;
use crate::books::*;

use std::env;
use std::io::{self, Read, Write, BufReader};
use std::fs::File;

#[derive(Clone, Copy, Debug, PartialEq)]
enum FileType {
    Json,
    Pgn,
    Bin,
    Tree(bool),
}

use FileType::*;

fn get_input_files(args: &[String]) -> Vec<(FileType, String)> {
    let types = vec![Json      , Pgn      , Bin      , Tree(false)];
    let tags  = vec!["-in-json", "-in-pgn", "-in-bin", "-in-tree" ];
    let exts  = vec![".json"   , ".pgn"   , ".bin"   , ".tree"    ];

    let mut out = Vec::new();
    let mut i = 0;

    while i < args.len() - 1 {
        let arg = &args[i];

        if let Some(j) = tags.iter().position(|x| *x == arg) {
            if i < args.len() - 1 {
                out.push((types[j], args[i + 1].clone()));
                i += 1;
            }
        } else if let Some(j) = exts.iter().position(|x| arg[arg.len() - x.len()..] == **x) {
            out.push((types[j], args[i].clone()));
        }

        i += 1
    }

    out
}

fn get_output_files(args: &[String]) -> Vec<(FileType, String)> {
    let types = vec![Json       , Bin       , Tree(true)      , Tree(false)];
    let tags  = vec!["-out-json", "-out-bin", "-out-tree-blob", "-out-tree"];
    let exts  = vec![".json"    , ".bin"    , ".blob.tree"    , ".tree"    ];

    let mut out = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        if let Some(j) = tags.iter().position(|x| *x == arg) {
            if i < args.len() - 1 {
                out.push((types[j], args[i + 1].clone()));
                i += 1;
            }
        } else if let Some(j) = exts.iter().position(|x| arg[arg.len() - x.len()..] == **x) {
            out.push((types[j], args[i].clone()));
        }

        i += 1
    }

    out
}

fn book_from_pgns(args: &[String], files: &[(FileType, String)]) -> BookMap {
    let filter = PgnFilter::from_args(args);
    let mut book = BookMap::new();

    let frequency = args.contains(&"-frequency".to_string());

    let depth =
        if let Some(pos) = args.iter().position(|x| x == "-pgn-depth") {
            args[pos].parse::<usize>().unwrap_or(10)
        } else {
            10
        };

    for (_, filename) in files.iter().filter(|x| x.0 == Pgn) {
        let reader: Box<dyn Read> =
            if filename == "-" {
                Box::new(io::stdin())
            } else {
                Box::new(File::open(filename).expect(&format!("Failure reading file {}", filename)))
            };

        fold_games(
            filter.clone(),
            reader,
            &mut |game| book.add_game(&game, frequency, depth)
        );
    }

    book
}

fn merge_book_files(book: &mut BookMap, files: &[(FileType, String)]) {
    for (filetype, filename) in files.iter().filter(|x| x.0 != Pgn) {
        let mut reader: Box<dyn Read> =
            if filename == "-" {
                Box::new(io::stdin())
            } else {
                Box::new(File::open(filename).expect(&format!("Failure reading file {}", filename)))
            };

        if *filetype == Bin {
            book.extend_from_reader(&mut reader)
        } else {
            let book2 =
                match filetype {
                    Json => BookMap::read_json(&mut BufReader::new(reader)),
                    Tree(_) => BookMap::read_txt(&mut BufReader::new(reader)),
                    _ => panic!()
                };

            book.merge(book2);
        }
    }
    book.set_depths();
}

fn modify_book(book: &mut BookMap, args: &[String]) {
    let mut i = 0;

    while i < args.len() {
        if i < args.len() - 1 {
            i += 1;

            match &args[i][..] {
                "-set-root" => {
                    book.set_root(fen_to_chess(&args[i]));
                }
                "-min-weight" => {
                    let weight = args[i].parse::<u64>().unwrap();

                    book.filter(|entry| entry.weight >= weight);
                }
                "-max-weight" => {
                    let weight = args[i].parse::<u64>().unwrap();

                    book.filter(|entry| entry.weight <= weight);
                }
                "-depth" => {
                    let depth = args[i].parse::<usize>().unwrap();

                    book.filter(|entry| entry.depth.unwrap_or(0) < depth);
                }
                "-keep-best" => {
                    let best = args[i].parse::<usize>().unwrap();

                    book.map_nodes(|node| {
                        // y, x
                        node.sort_unstable_by(|x, y| y.weight.cmp(&x.weight));

                        node.truncate(best);
                    })
                }
                "-keep-worst" => {
                    let worst = args[i].parse::<usize>().unwrap();

                    book.map_nodes(|node| {
                        // x, y
                        node.sort_unstable_by(|x, y| x.weight.cmp(&y.weight));

                        node.truncate(worst);
                    })
                }
                _ => i -= 1
            }
        }

        match &args[i][..] {
            "-remove-disconnected" => {
                book.remove_disconnected();
            }
            "-white-only" => {
                book.filter(|entry| entry.depth.unwrap_or(1) % 2 == 0)
            }
            "-black-only" => {
                book.filter(|entry| entry.depth.unwrap_or(0) % 2 == 1)
            }
            "-clear-learning" => {
                book.map_entries(|entry| entry.learn = 0)
            }
            "-uniform" => {
                book.map_entries(|entry| entry.weight = 1)
            }
            _ => {}
        }

        i += 1
    }
}

fn write_book(book: &mut BookMap, outputs: &[(FileType, String)]) {
    for (filetype, filename) in outputs {
        let mut writer: Box<dyn Write> =
            if filename == "-" {
                Box::new(io::stdout())
            } else {
                Box::new(File::create(filename).unwrap())
            };

        println!("{:?}", filetype);

        match filetype {
            Bin => book.write(&mut writer),
            Json => book.write_json(&mut writer),
            Tree(false) => book.write_txt(&mut writer),
            Tree(true) => book.write_blob(&mut writer),
            _ => {}
        }
    }
}

pub fn run() {
    let args = env::args().skip(1).collect::<Vec<_>>();

    println!("a");

    let inputs = get_input_files(&args);
    let outputs = get_output_files(&args);

    println!("a");

    let mut book = book_from_pgns(&args, &inputs);

    println!("a");

    merge_book_files(&mut book, &inputs);
    println!("a");
    modify_book(&mut book, &args);
    println!("a");
    write_book(&mut book, &outputs);
}
