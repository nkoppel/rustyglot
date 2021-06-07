use super::*;

use std::io::{Write, BufRead};
use std::convert::TryInto;
use std::cmp::Reverse;

use serde_json::Value;
use serde::de::Deserialize;

impl BookMap {
    pub fn write_txt<W: Write>(&mut self, mut w: &mut W) {
        if book_hash(self.root.clone()) != START_HASH {
            writeln!(w, "{}", fen(&self.root));
        }

        let mut last_weight = 0;
        let mut last_depth = 0;
        let mut depths = Vec::new();

        self.traverse_tree(|depth, pos, entries, ind| {
            if ind == 0 {
                entries.sort_unstable_by_key(|entry| Reverse(entry.weight));
            }

            let only_child = entries.len() == 1;
            let entry = &entries[ind];

            let mov = from_book_move(entry.mov).to_move(pos).unwrap();
            let san = SanPlus::from_move(pos.clone(), &mov);

            while depth >= depths.len() {
                depths.push(0);
            }

            if only_child {
                if depth > 0 {
                    depths[depth] = depths[depth - 1];
                }
                write!(&mut w, ", ");
            } else {
                if depth > 0 {
                    depths[depth] = depths[depth - 1] + 1;
                }
                if depth < last_depth {
                    writeln!(&mut w);
                }
                write!(&mut w, "\n{}", "    ".repeat(depths[depth]));
                last_depth = depth;
            }

            if (entry.weight != 1 && !only_child) || (only_child && entry.weight != last_weight) {
                write!(&mut w, "{} ", entry.weight);
            }
            last_weight = entry.weight;

            write!(&mut w, "{}", san);

            if entry.learn != 0 {
                write!(&mut w, " {}", entry.learn);
            }
        });
    }

    pub fn write_blob<W: Write>(&mut self, mut w: &mut W) {
        if book_hash(self.root.clone()) != START_HASH {
            writeln!(w, "{}", fen(&self.root));
        }

        let mut last_weight = 0;
        let mut last_depth = 0;
        let mut depths = Vec::new();

        self.traverse_tree(|depth, pos, entries, ind| {
            if ind == 0 {
                entries.sort_unstable_by_key(|entry| Reverse(entry.weight));
            }

            let only_child = entries.len() == 1;
            let entry = &entries[ind];

            let mov = from_book_move(entry.mov).to_move(pos).unwrap();
            let san = San::from_move(pos, &mov);

            while depth >= depths.len() {
                depths.push(0);
            }

            if only_child {
                if depth > 0 {
                    depths[depth] = depths[depth - 1];
                }
                write!(&mut w, ",");
            } else {
                if depth > 0 {
                    depths[depth] = depths[depth - 1] + 1;
                }
                if depth < last_depth {
                    write!(&mut w, "{}", ")".repeat(depths[last_depth] - depths[depth]));
                }
                if ind == 0 && depth == 0 {
                } else if ind == 0 {
                    write!(&mut w, "(");
                } else if depth >= last_depth {
                    write!(&mut w, "/");
                }
                last_depth = depth;
            }

            if (entry.weight != 1 && !only_child) || (only_child && entry.weight != last_weight) {
                write!(&mut w, "{}", entry.weight);
            }
            last_weight = entry.weight;

            write!(&mut w, "{}", san);

            if entry.learn != 0 {
                write!(&mut w, " {}", entry.learn);
            }
        });
    }

    pub fn write_json<W: Write>(&mut self, mut w: &mut W) {
        write!(w, "{{\"rootFen\":{:?},\"tree\":{{", fen(&self.root));

        let mut last_depth = -1;

        self.traverse_tree(|depth, pos, entries, ind| {
            let entry = &entries[ind];

            let mov = from_book_move(entry.mov).to_move(pos).unwrap();
            let san = San::from_move(pos, &mov);

            if (depth as isize) < last_depth {
                write!(&mut w, "{}", "}}".repeat(last_depth as usize - depth + 1));
            }
            if depth as isize <= last_depth {
                write!(&mut w, ",");
            }

            write!(&mut w, "\"{}\":{{\"weight\":{},\"learn\":{},\"children\":{{", san, entry.weight, entry.learn);

            last_depth = depth as isize;
        });

        write!(&mut w, "{}", "}}".repeat(last_depth as usize + 2));
    }
}

fn process_line(line: &mut String) -> usize {
    let mut indent = 0;
    let mut start = 0;

    for c in line.chars() {
        match c {
            ' ' => indent += 1,
            '\t' => indent += 4,
            _ => break
        }
        start += 1;
    }

    let end = line.find(";").unwrap_or(line.len());
    *line = line[start..end].trim().to_string();
    line.push('\n');
    indent
}

impl BookMap {
    pub fn read_txt<R: BufRead>(reader: &mut R) -> Self {
        let mut out = BookMap::new();
        let mut stack: Vec<(Chess, usize)> = Vec::new();
        let mut pos = Chess::default();
        let mut paren_indent = 0;
        let mut root = true;

        for (line_number, line) in reader.lines().enumerate() {
            let mut line = line.unwrap();
            let indent = process_line(&mut line) + paren_indent;

            if line.trim().is_empty() {
                continue;
            }

            if root {
                if let Ok(fen) = line.parse::<Fen>() {
                    pos = fen.position(Chess960).expect("Invalid root position");
                    out.root = pos.clone();
                    root = false;
                    continue;
                }
            }

            root = false;

            let mut weight = 1;
            let mut san = None;
            let mut learn = 0;
            let mut entrystart = 0;
            let mut wordstart = 0;
            let mut first_entry = true;
            let mut read_weight = true;

            for (i, c) in line.chars().enumerate() {
                if " \n\t,/()".contains(c) || (!c.is_digit(10) && read_weight) {
                    let word = &line[wordstart..i];

                    if let Ok(n) = word.parse::<u64>() {
                        if san == None {
                            weight = n;
                        } else {
                            learn = n as u32;
                        }
                    } else if let Ok(s) = word.parse::<SanPlus>() {
                        san = Some(s);
                    } else if !word.is_empty() {
                        panic!("Invalid token {:?} at {}:{}", word, line_number + 1, i + 1);
                    }

                    if " \n\t,/()".contains(c) {
                        wordstart = i + 1;
                    } else {
                        wordstart = i;
                    }

                    if c.is_ascii_alphabetic() {
                        read_weight = false;
                    }
                }
                if "\n,/()".contains(c) && entrystart != i {
                    if first_entry {
                        while let Some((_, indent2)) = stack.last() {
                            if *indent2 < indent {
                                break;
                            } else {
                                pos = stack.pop().unwrap().0;
                            }
                        }

                        first_entry = false;
                    }

                    let s = san.expect(&format!("Entry {:?} has no move at {}:{}", &line[entrystart..i], line_number + 1, i + 1));

                    let mov = s.san
                        .to_move(&pos)
                        .expect(&format!("Invalid move {} for position {:?} at {}:{}", s, fen(&pos), line_number + 1, i + 1));

                    let book_move = to_book_move(Uci::from_chess960(&mov));

                    let entry = BookEntry {
                        mov: book_move,
                        depth: Some(stack.len()),
                        weight,
                        learn
                    };

                    out.insert_no_merge(book_hash(pos.clone()), entry);
                    stack.push((pos.clone(), indent));
                    pos.play_unchecked(&mov);

                    san = None;
                    learn = 0;
                    entrystart = i + 1;
                    read_weight = true;
                }
                match c {
                    '/' => {                   first_entry = true; weight = 1}
                    '(' => {paren_indent += 4; first_entry = true; weight = 1}
                    ')' => {paren_indent -= 4; first_entry = true; weight = 1}
                    _ => {}
                }
            }
        }

        out
    }

    pub fn read_json<R: BufRead>(reader: R) -> Self {
        let mut deserializer = serde_json::Deserializer::from_reader(reader);
        deserializer.disable_recursion_limit();
        let deserializer = serde_stacker::Deserializer::new(&mut deserializer);
        let json = Value::deserialize(deserializer).unwrap();

        let mut out = BookMap::new();

        let root = json
            .as_object().unwrap()
            .get("rootFen").unwrap()
            .as_str().unwrap()
            .parse::<Fen>().unwrap();

        out.root = root.position(Chess960).expect("Invalid root position");

        let tree = json
            .as_object().unwrap()
            .get("tree").unwrap()
            .as_object().unwrap();

        let mut stack = Vec::new();
        stack.push((out.root.clone(), tree.iter().collect::<Vec<_>>(), 0));

        while let Some((pos, entries, ind)) = stack.pop() {
            if ind >= entries.len() {
                continue;
            }

            let (san, entry) = entries[ind];

            let entry = entry.as_object().unwrap();
            let san = san.parse::<San>().unwrap();
            let mov = san
                .to_move(&pos)
                .expect(&format!("Invalid move {} for position {:?}", san, fen(&pos)));

            let book_move = to_book_move(Uci::from_chess960(&mov));
            let weight = entry.get("weight").unwrap().as_u64().unwrap();
            let learn  = entry.get("learn" ).unwrap().as_u64().unwrap();

            let out_entry = BookEntry {
                mov: book_move,
                depth: Some(stack.len()),
                weight,
                learn: learn as u32
            };

            out.insert_no_merge(book_hash(pos.clone()), out_entry);
            stack.push((pos.clone(), entries, ind + 1));

            let pos = pos.clone().play(&mov).unwrap();
            let children = entry
                .get("children").unwrap()
                .as_object().unwrap()
                .iter().collect::<Vec<_>>();

            stack.push((pos, children, 0));
        }

        out
    }
}
