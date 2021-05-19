use crate::conversions::*;

#[derive(Clone)]
pub struct BinEntry {
    hash: u64,
    weight: usize,
    mov: u16,
    next: Option<u64>,
    learning: u32
}

#[derive(Clone)]
pub struct PgnGame {
    pub headers: Vec<(String, String)>,
    white_elo: Option<usize>,
    black_elo: Option<usize>,
    time: Option<usize>,
    increment: Option<usize>,
    pub outcome: Outcome,
    pub moves: Vec<SanPlus>
}

#[derive(Clone)]
pub struct PgnFilter {
    min_elo: usize,
    max_elo: usize,
    min_high_elo: usize,
    max_low_elo: usize,
    min_time: usize,
    max_time: usize,
    min_increment: usize,
    max_increment: usize,
    draws: bool,
    white_wins: bool,
    black_wins: bool,
}

struct PgnVisitor {
    game: PgnGame,
    filter: PgnFilter,
}

impl PgnGame {
    fn new() -> Self {
        PgnGame {
            headers: Vec::new(),
            white_elo: Some(0),
            black_elo: Some(0),
            time: Some(0),
            increment: Some(0),
            outcome: Outcome::Draw,
            moves: Vec::new()
        }
    }
}

use std::fmt;

impl fmt::Display for PgnGame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.headers.is_empty() {
            writeln!(f, "[Event \"Generated Game\"]")?;
            writeln!(f, "[Date  \"????.??.??\"]")?;
            writeln!(f, "[White \"?\"]")?;
            writeln!(f, "[Black \"?\"]")?;
            writeln!(f, "[Result \"{}\"]", self.outcome)?;
        } else {
            for (k, v) in &self.headers {
                writeln!(f, "[{} {:?}]", k, v)?;
            }
        }
        writeln!(f)?;

        for (i, m) in self.moves.iter().enumerate() {
            if i % 2 == 0 {
                write!(f, "{}. ", i / 2 + 1)?;
            }

            write!(f, "{} ", m)?;
        }

        writeln!(f)
    }
}

impl PgnFilter {
    pub fn new() -> Self {
        PgnFilter {
            min_elo: 0,
            max_elo: usize::MAX,
            min_high_elo: 0,
            max_low_elo: usize::MAX,
            min_time: 0,
            max_time: usize::MAX,
            min_increment: 0,
            max_increment: usize::MAX,
            draws: true,
            white_wins: true,
            black_wins: true,
        }
    }

    pub fn matches(&self, game: &PgnGame) -> bool {
        match game.outcome {
            Outcome::Decisive{winner: Color::White}
                if !self.white_wins => return false,
            Outcome::Decisive{winner: Color::Black}
                if !self.black_wins => return false,
            Outcome::Draw if !self.draws => return false,
            _ => {}
        }

        if let (Some(white), Some(black)) = (game.white_elo, game.black_elo) {
            let low_elo  = white.max(black);
            let high_elo = white.min(black);

            if high_elo < self.min_high_elo || high_elo > self.max_elo ||
               low_elo  > self.max_low_elo  || low_elo  < self.min_elo
            {
                return false;
            }
        }

        if let Some(time) = game.time {
            if time < self.min_time || time > self.max_time {
                return false;
            }
        }

        if let Some(increment) = game.increment {
            if increment < self.min_increment ||
               increment > self.max_increment
            {
                return false;
            }
        }

        true
    }

    pub fn set_args(&mut self, args: &[String]) {
        let mut iter = args.iter();

        while let Some(a) = iter.next() {
            match &a[..] {
                "-no-draws" => self.draws = false,
                "-no-white-wins" => self.white_wins = false,
                "-no-black-wins" => self.black_wins = false,
                "-no-wins" => {
                    self.white_wins = false;
                    self.black_wins = false
                }
                _ => if let Some(n) = iter.next() {
                    let num = n.parse::<usize>().unwrap();

                    match &a[..] {
                        "-min-elo" => self.min_elo = num,
                        "-max-elo" => self.max_elo = num,
                        "-min-high-elo" => self.min_high_elo = num,
                        "-max-low-elo"  => self.max_low_elo  = num,

                        "-min-time" => self.min_time = num,
                        "-max-time" => self.max_time = num,
                        "-min-increment" => self.min_increment = num,
                        "-max-increment" => self.max_increment = num,
                        _ => {}
                    }
                }
            }
        }
    }
}

impl PgnVisitor {
    fn new() -> Self {
        PgnVisitor {
            game: PgnGame::new(),
            filter: PgnFilter::new(),
        }
    }

    fn with_filter(filter: PgnFilter) -> Self {
        PgnVisitor {
            game: PgnGame::new(),
            filter
        }
    }

    fn into_game(self) -> PgnGame {
        self.game
    }
}

use pgn_reader::{Visitor, Skip, BufferedReader};

impl Visitor for PgnVisitor {
    type Result = PgnGame;

    fn begin_game(&mut self) {
        self.game = PgnGame::new();
    }

    fn header(&mut self, key: &[u8], value: pgn_reader::RawHeader) {
        let k = std::str::from_utf8(key).unwrap().to_string();
        let v = value.decode_utf8().unwrap().to_string();

        match &k[..] {
            "TimeControl" => {
                let ws: Vec<&str> = v.split(|c| "/+-?*".contains(c)).collect();

                match ws.len() {
                    1 => {
                        self.game.time = Some(ws[0].parse::<usize>().unwrap());
                        self.game.increment = Some(0);
                    }
                    2 if v.contains('+') => {
                        self.game.time = Some(ws[0].parse::<usize>().unwrap());
                        self.game.increment = Some(ws[1].parse::<usize>().unwrap());
                    }
                    2 => {
                        self.game.time = Some(ws[1].parse::<usize>().unwrap());
                        self.game.increment = Some(0);
                    }
                    3 => {
                        self.game.time = Some(ws[1].parse::<usize>().unwrap());
                        self.game.increment = Some(ws[2].parse::<usize>().unwrap());
                    }
                    _ => {},
                }
            }
            "WhiteElo" => {
                let e = v.parse::<usize>().expect("Invalid White Elo!");
                self.game.white_elo = Some(e);
            }
            "BlackElo" =>  {
                let e = v.parse::<usize>().expect("Invalid Black Elo!");
                self.game.black_elo = Some(e);
            }
            "Result" => {
                match &v[..] {
                    "1-0" => self.game.outcome = Outcome::Decisive{winner: Color::White},
                    "0-1" => self.game.outcome = Outcome::Decisive{winner: Color::Black},
                    _ => {}
                }
            }
            _ => {}
        }

        self.game.headers.push((k, v));
    }

    fn end_headers(&mut self) -> Skip {
        if self.filter.matches(&self.game) {
            Skip(false)
        } else {
            Skip(true)
        }
    }

    fn san(&mut self, san: SanPlus) {
        self.game.moves.push(san);
    }

    fn begin_variation(&mut self) -> Skip { Skip(true) }

    fn end_game(&mut self) -> PgnGame {
        std::mem::replace(&mut self.game, PgnGame::new())
    }
}

use std::io::{Read, Write};

pub fn read_games<R: Read>(filter: PgnFilter, read: R) -> Vec<PgnGame> {
    let mut visitor = PgnVisitor::with_filter(filter);
    BufferedReader::new(read)
        .into_iter(&mut visitor)
        .map(|x| x.unwrap())
        .collect()
}

pub fn write_games<W: Write>(w: &mut W, games: &[PgnGame]) {
    for g in games {
        writeln!(w, "{}", g).expect("Unable to write games!");
    }
}
