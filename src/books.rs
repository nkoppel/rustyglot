use crate::conversions::*;
use crate::pgn::*;

use std::collections::HashMap;
use std::io::{Read, Write, BufRead};
use std::convert::TryInto;
use std::cmp::Reverse;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BookEntry {
    mov: u16,
    depth: Option<usize>,
    weight: usize,
    learn: u32
}

pub struct BookMap {
    map: HashMap<u64, Vec<BookEntry>>,
    root: Chess
}

impl BookEntry {
    pub fn new() -> Self {
        BookEntry {
            mov: 0,
            depth: None,
            weight: 0,
            learn: 0,
        }
    }

    pub fn merge(&mut self, other: &BookEntry) -> bool {
        if self.mov != other.mov {
            return false;
        }

        self.weight += other.weight;
        true
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();

        out.extend_from_slice(&self.mov.to_be_bytes());
        out.extend_from_slice(&(self.weight as u16).to_be_bytes());
        out.extend_from_slice(&self.learn.to_be_bytes());

        out
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        let mut out = Self::new();

        out.mov    = u16::from_be_bytes(bytes[0..2].try_into().unwrap());
        out.weight = u16::from_be_bytes(bytes[2..4].try_into().unwrap()) as usize;
        out.learn  = u32::from_be_bytes(bytes[4..8].try_into().unwrap());

        out
    }
}

impl BookMap {
    pub fn new() -> Self {
        BookMap {
            map: HashMap::new(),
            root: Chess::default(),
        }
    }

    pub fn insert(&mut self, hash: u64, entry: BookEntry) {
        if let Some(v) = self.map.get_mut(&hash) {
            for entry2 in v.iter_mut() {
                if entry2.merge(&entry) {
                    return;
                }
            }
            v.push(entry);
        } else {
            self.map.insert(hash, vec![entry]);
        }
    }

    pub fn insert_no_merge(&mut self, hash: u64, entry: BookEntry) {
        if let Some(v) = self.map.get_mut(&hash) {
            for entry2 in v.iter_mut() {
                if entry2.mov == entry.mov {
                    return;
                }
            }
            v.push(entry);
        } else {
            self.map.insert(hash, vec![entry]);
        }
    }

    pub fn merge(&mut self, other: BookMap) {
        for (hash, v) in other.map {
            for entry in v {
                self.insert(hash, entry);
            }
        }
    }

    fn traverse_tree<F>(&mut self, mut f: F)
        where F: FnMut(usize, &Chess, &mut Vec<BookEntry>, usize)
    {
        let mut stack = vec![(self.root.clone(), 0)];

        while let Some((pos, ind)) = stack.pop() {
            let hash = book_hash(pos.clone());

            if let Some(mut entries) = self.map.get_mut(&hash) {
                if ind < entries.len() {
                    f(stack.len(), &pos, &mut entries, ind);
                }

                if ind < entries.len() {
                    let mov =
                        from_book_move(entries[ind].mov)
                            .to_move(&pos)
                            .unwrap();

                    stack.push((pos.clone(), ind + 1));
                    stack.push((pos.play(&mov).unwrap(), 0));
                } else if entries.is_empty() {
                    self.map.remove(&hash);
                }
            }
        }
    }

    pub fn map_nodes<F>(&mut self, mut f: F)
        where F: FnMut(&mut Vec<BookEntry>)
    {
        self.map.retain(|_, vec| {
            f(vec);
            !vec.is_empty()
        });
    }

    pub fn map_entries<F>(&mut self, mut f: F) where F: FnMut(&mut BookEntry) {
        for (_, v) in &mut self.map {
            for entry in v {
                f(entry)
            }
        }
    }

    pub fn filter<F>(&mut self, mut f: F) where F: FnMut(&BookEntry) -> bool {
        self.map.retain(|_, vec| {
            vec.retain(|x| f(x));
            !vec.is_empty()
        })
    }

    pub fn set_root(&mut self, root: Chess) {
        self.root = root;

        self.map_entries(|entry| entry.depth = None);

        self.traverse_tree(|depth, _, entries, ind| {
            entries[ind].depth = Some(depth);
        });
    }

    pub fn into_writer<W: Write>(self, writer: &mut W) {
        let mut vec = self.map
            .into_iter()
            .map(|(hash, mut entries)|{
                entries.sort_unstable();
                (hash, entries)
            })
            .collect::<Vec<_>>();

        vec.sort_unstable();

        for (hash, entries) in vec {
            let hash_bytes = hash.to_be_bytes();

            for entry in entries {
                writer.write_all(&hash_bytes);
                writer.write_all(&entry.to_bytes());
            }
        }
    }

    pub fn into_txt_writer<W: Write>(&mut self, mut w: &mut W) {
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
}

fn process_line<'a>(line: &'a str) -> (&'a str, usize) {
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
    (line[start..end].trim(), indent)
}

impl BookMap {
    pub fn from_txt_reader<R: BufRead>(reader: &mut R) -> Self {
        let mut out = BookMap::new();
        let mut stack: Vec<(Chess, usize)> = Vec::new();
        let mut pos = Chess::default();
        let mut root = true;

        for (line_number, l) in reader.lines().enumerate() {
            let l = l.unwrap();
            let (line, indent) = process_line(&l);

            if line.is_empty() {
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
            let mut first_entry = true;

            for entry in line.split(',') {
                let mut san = None;
                let mut learn = 0;

                for word in entry.split_whitespace() {
                    if let Ok(n) = word.parse::<usize>() {
                        if san == None {
                            weight = n;
                        } else {
                            learn = n as u32;
                        }
                    } else if let Ok(s) = word.parse::<SanPlus>() {
                        san = Some(s);
                    } else {
                        panic!("Invalid token {:?} on line {}", word, line_number + 1);
                    }
                }

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

                let san = san.expect(&format!("Entry without move on line {}", line_number));

                let mov = san.san
                    .to_move(&pos)
                    .expect(&format!("Invalid move {} for position {:?} on line {}", san, fen(&pos), line_number));

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
            }
        }

        out
    }

    pub fn extend_from_reader<R: Read>(&mut self, reader: &mut R) {
        let mut buf = [0u8; 16];

        while let Ok(()) = reader.read_exact(&mut buf[..]) {
            let hash = u64::from_be_bytes(buf[0..8].try_into().unwrap());
            let entry = BookEntry::from_bytes(&buf[8..]);

            self.insert(hash, entry);
        }
    }

    pub fn extend_from_games(&mut self, games: &[PgnGame], depth: usize) {
        for game in games.iter() {
            let mut board = Chess::default();

            for (depth, sanplus) in game.moves.iter().take(depth).enumerate() {
                let hash = book_hash(board.clone());

                let mov = sanplus.san.to_move(&board).unwrap();
                let uci = Uci::from_chess960(&mov);
                board = board.play(&mov).unwrap();

                let weight =
                    if let Outcome::Decisive{winner} = game.outcome {
                        if (winner == Color::White) == (depth % 2 == 0) {
                            2
                        } else {
                            0
                        }
                    } else {
                        1
                    };

                self.insert(hash,
                    BookEntry {
                        mov: to_book_move(uci),
                        depth: Some(depth),
                        weight,
                        learn: 0
                    }
                )
            }
        }
    }
}
