use crate::books::*;
use crate::conversions::fen_to_chess;
use crate::pgn::*;

use std::env;
use std::fs::File;
use std::io::{self, BufReader, Read, Write};

#[derive(Clone, Copy, Debug, PartialEq)]
enum FileType {
    Json,
    Pgn,
    Bin,
    Tree(bool),
}

use FileType::*;

fn get_input_files(args: &[String]) -> Vec<(FileType, String)> {
    let types = [Json, Pgn, Bin, Tree(false)];
    let tags = ["-in-json", "-in-pgn", "-in-bin", "-in-tree"];
    let exts = [".json", ".pgn", ".bin", ".tree"];

    let mut out = Vec::new();
    let mut i = 0;

    while i < args.len() - 1 {
        let arg = &args[i];

        if let Some(j) = tags.iter().position(|x| *x == arg) {
            if i < args.len() - 1 {
                out.push((types[j], args[i + 1].clone()));
                i += 1;
            }
        } else if let Some(j) = exts
            .iter()
            .position(|x| arg[arg.len().saturating_sub(x.len())..] == **x)
        {
            out.push((types[j], args[i].clone()));
        }

        i += 1
    }

    out
}

fn get_output_files(args: &[String]) -> Vec<(FileType, String)> {
    let types = [Json, Bin, Tree(true), Tree(false)];
    let tags = ["-out-json", "-out-bin", "-out-tree-blob", "-out-tree"];
    let exts = [".json", ".bin", ".blob.tree", ".tree"];

    let mut out = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        if let Some(j) = tags.iter().position(|x| *x == arg) {
            if i < args.len() - 1 {
                out.push((types[j], args[i + 1].clone()));
                i += 1;
            }
        } else if let Some(j) = exts
            .iter()
            .position(|x| arg[arg.len().saturating_sub(x.len())..] == **x)
        {
            out.push((types[j], args[i].clone()));
        }

        i += 1
    }

    out
}

fn book_from_pgns(args: &[String], files: &[(FileType, String)]) -> BookMap {
    let filter = PgnFilter::from_args(args);
    let mut book = BookMap::new();

    let frequency = args.iter().any(|a| a == "-frequency");

    let depth = if let Some(pos) = args.iter().position(|x| x == "-pgn-depth") {
        args[pos + 1].parse::<usize>().unwrap_or(usize::MAX)
    } else {
        usize::MAX
    };

    let mut i = 0;

    for (_, filename) in files.iter().filter(|x| x.0 == Pgn) {
        let reader: Box<dyn Read> = if filename == "-" {
            Box::new(io::stdin())
        } else {
            Box::new(
                File::open(filename)
                    .unwrap_or_else(|_| panic!("Failure reading file {}", filename)),
            )
        };

        fold_games(filter.clone(), reader, &mut |game| {
            i += 1;
            book.add_game(&game, frequency, depth)
        });
    }

    println!("Wrote entries from {} games", i);

    book
}

fn merge_book_files(book: &mut BookMap, files: &[(FileType, String)], args: &[String]) {
    let combine = args.contains(&"-combine-entries".to_string());
    let mut merged = false;

    for (filetype, filename) in files.iter().filter(|x| x.0 != Pgn) {
        let mut reader: Box<dyn Read> = if filename == "-" {
            Box::new(io::stdin())
        } else {
            Box::new(
                File::open(filename)
                    .unwrap_or_else(|_| panic!("Failure reading file {}", filename)),
            )
        };

        if *filetype == Bin {
            if combine {
                book.extend_from_reader_combine(&mut reader)
            } else {
                book.extend_from_reader(&mut reader)
            }
        } else {
            let book2 = match filetype {
                Json => BookMap::read_json(&mut BufReader::new(reader)),
                Tree(_) => BookMap::read_txt(&mut BufReader::new(reader)),
                _ => panic!(),
            };

            if combine {
                book.merge_combine(book2);
            } else {
                book.merge(book2);
            }
        }
        merged = true;
    }
    if merged {
        book.set_depths();
    }
}

fn modify_book(book: &mut BookMap, args: &[String]) {
    let mut i = 0;

    while i < args.len() {
        if i < args.len() - 1 {
            i += 1;

            match &args[i - 1][..] {
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
                        node.sort_by_key(|x| u64::MAX - x.weight);
                        node.truncate(best);
                    })
                }
                "-keep-worst" => {
                    let worst = args[i].parse::<usize>().unwrap();

                    book.map_nodes(|node| {
                        node.sort_by_key(|x| x.weight);
                        node.truncate(worst);
                    })
                }
                "-scale-weights" => {
                    let factor = args[i].parse::<f64>().unwrap();

                    book.map_entries(|entry| entry.weight = (entry.weight as f64 * factor) as u64)
                }
                _ => i -= 1,
            }
        }

        match &args[i][..] {
            "-remove-disconnected" => {
                book.remove_disconnected();
            }
            "-white-only" => book.filter(|entry| entry.depth.unwrap_or(1) % 2 == 0),
            "-black-only" => book.filter(|entry| entry.depth.unwrap_or(0) % 2 == 1),
            "-clear-learning" => book.map_entries(|entry| entry.learn = 0),
            "-uniform" => book.map_entries(|entry| entry.weight = 1),
            _ => {}
        }

        i += 1
    }
}

fn write_book(book: &mut BookMap, outputs: &[(FileType, String)]) {
    for (filetype, filename) in outputs {
        let mut writer: Box<dyn Write> = if filename == "-" {
            Box::new(io::stdout())
        } else {
            Box::new(File::create(filename).unwrap())
        };

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

    let inputs = get_input_files(&args);
    let outputs = get_output_files(&args);

    println!("Building book from pgn files...");
    let mut book = book_from_pgns(&args, &inputs);

    println!("Created {} entries in book", book.len());

    println!("Combining pgn book with other book files...");
    merge_book_files(&mut book, &inputs, &args);
    println!("Applying modifications to book...");
    modify_book(&mut book, &args);
    println!("Writing book to output...");
    write_book(&mut book, &outputs);
    println!("Done!");
}
