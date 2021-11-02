use crate::conversions::*;
use crate::pgn::*;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::convert::TryInto;

mod txt_books;

pub use txt_books::*;

const U16_MAX: u64 = u16::MAX as u64;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BookEntry {
    pub mov: u16,
    pub depth: Option<usize>,
    pub weight: u64,
    pub learn: u32
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
        out.weight = u16::from_be_bytes(bytes[2..4].try_into().unwrap()) as u64;
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

    pub fn map_entries<F>(&mut self, mut f: F)
        where F: FnMut(&mut BookEntry)
    {
        for (_, v) in &mut self.map {
            for entry in v {
                f(entry)
            }
        }
    }

    pub fn filter<F>(&mut self, mut f: F)
        where F: FnMut(&BookEntry) -> bool
    {
        self.map.retain(|_, vec| {
            vec.retain(|x| f(x));
            !vec.is_empty()
        })
    }

    pub fn set_depths(&mut self) {
        self.map_entries(|entry| entry.depth = None);

        self.traverse_tree(|depth, _, entries, ind| {
            entries[ind].depth = Some(depth);
        });
    }

    pub fn set_root(&mut self, root: Chess) {
        self.root = root;

        self.set_depths();
    }

    pub fn remove_disconnected(&mut self) {
        self.set_depths();

        self.filter(|entry| entry.depth.is_some());
    }

    pub fn write<W: Write>(&self, writer: &mut W) {
        let mut vec = self.map
            .iter()
            .map(|(hash, entries)|{
                let mut entries = entries.clone();
                entries.sort_unstable();

                (hash, entries)
            })
            .collect::<Vec<_>>();

        vec.sort_unstable();

        for (hash, entries) in vec {
            let hash_bytes = hash.to_be_bytes();
            let max_weight = entries.iter().map(|e| e.weight).max().unwrap();

            for mut entry in entries {
                if max_weight > U16_MAX {
                    entry.weight *= U16_MAX;
                    entry.weight /= max_weight;
                }
                writer.write_all(&hash_bytes);
                writer.write_all(&entry.to_bytes());
            }
        }
    }

    pub fn extend_from_reader<R: Read>(&mut self, reader: &mut R) {
        let mut buf = [0u8; 16];

        while let Ok(()) = reader.read_exact(&mut buf[..]) {
            let hash = u64::from_be_bytes(buf[0..8].try_into().unwrap());
            let entry = BookEntry::from_bytes(&buf[8..]);

            self.insert(hash, entry);
        }
    }

    pub fn add_game(&mut self, game: &PgnGame, frequency: bool, depth: usize) {
        let mut board = Chess::default();

        for (depth, sanplus) in game.moves.iter().take(depth).enumerate() {
            let hash = book_hash(board.clone());

            let mov = sanplus.san.to_move(&board).unwrap();
            let uci = Uci::from_chess960(&mov);
            board = board.play(&mov).unwrap();

            let weight =
                if frequency {
                    1
                } else if let Outcome::Decisive{winner} = game.outcome {
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

    pub fn extend_from_games(&mut self, games: &[PgnGame], frequency: bool, depth: usize) {
        for game in games.iter() {
            self.add_game(game, frequency, depth);
        }
    }
}
