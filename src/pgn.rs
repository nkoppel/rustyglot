use crate::conversions::*;

#[derive(Clone)]
pub struct BinEntry {
    hash: u64,
    weight: usize,
    mov: u16,
    next: Option<u64>,
    learning: u32,
}

#[derive(Clone)]
pub struct PgnGame {
    pub headers: Vec<(String, String)>,
    white_elo: Option<usize>,
    black_elo: Option<usize>,
    time: Option<usize>,
    increment: Option<usize>,
    pub outcome: Outcome,
    pub moves: Vec<SanPlus>,
}

#[derive(Clone)]
pub struct PgnFilter {
    min_elo: usize,
    max_elo: usize,
    min_high_elo: usize,
    max_low_elo: usize,
    min_elo_diff: usize,
    max_elo_diff: usize,
    min_time: usize,
    max_time: usize,
    min_increment: usize,
    max_increment: usize,
    min_game_length: usize,
    max_game_length: usize,
    draws: bool,
    white_wins: bool,
    black_wins: bool,
}

struct PgnVisitor {
    game: PgnGame,
    filter: PgnFilter,
    skip: bool,
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
            moves: Vec::new(),
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
            min_elo_diff: 0,
            max_elo_diff: usize::MAX,
            min_time: 0,
            max_time: usize::MAX,
            min_increment: 0,
            max_increment: usize::MAX,
            min_game_length: 0,
            max_game_length: usize::MAX,
            draws: true,
            white_wins: true,
            black_wins: true,
        }
    }

    fn header_matches(&self, game: &PgnGame) -> bool {
        match game.outcome {
            Outcome::Decisive {
                winner: Color::White,
            } if !self.white_wins => return false,
            Outcome::Decisive {
                winner: Color::Black,
            } if !self.black_wins => return false,
            Outcome::Draw if !self.draws => return false,
            _ => {}
        }

        if let (Some(white), Some(black)) = (game.white_elo, game.black_elo) {
            let high_elo = white.max(black);
            let low_elo = white.min(black);

            let diff_elo = high_elo - low_elo;

            if low_elo < self.min_elo
                || high_elo > self.max_elo
                || high_elo < self.min_high_elo
                || low_elo > self.max_low_elo
                || diff_elo < self.min_elo_diff
                || diff_elo > self.max_elo_diff
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
            if increment < self.min_increment || increment > self.max_increment {
                return false;
            }
        }

        true
    }

    fn moves_match(&self, game: &PgnGame) -> bool {
        game.moves.len() >= self.min_game_length && game.moves.len() <= self.max_game_length
    }

    pub fn matches(&self, game: &PgnGame) -> bool {
        self.header_matches(game) && self.moves_match(game)
    }

    pub fn from_args(args: &[String]) -> Self {
        let mut out = Self::new();
        let mut i = 0;

        while i < args.len() {
            match &args[i][..] {
                "-no-draws" => out.draws = false,
                "-no-white-wins" => out.white_wins = false,
                "-no-black-wins" => out.black_wins = false,
                "-no-wins" => {
                    out.white_wins = false;
                    out.black_wins = false
                }
                _ => {
                    if i + 1 < args.len() {
                        if let Ok(num) = args[i + 1].parse::<usize>() {
                            match &args[i][..] {
                                "-min-elo" => out.min_elo = num,
                                "-max-elo" => out.max_elo = num,
                                "-min-high-elo" => out.min_high_elo = num,
                                "-max-low-elo" => out.max_low_elo = num,

                                "-min-elo-diff" => out.min_elo_diff = num,
                                "-max-elo-diff" => out.max_elo_diff = num,

                                "-min-game-length" => out.min_game_length = num,
                                "-max-game-length" => out.max_game_length = num,

                                "-min-time" => out.min_time = num,
                                "-max-time" => out.max_time = num,
                                "-min-increment" => out.min_increment = num,
                                "-max-increment" => out.max_increment = num,
                                _ => {}
                            }

                            i += 1;
                        }
                    }
                }
            }

            i += 1;
        }

        out
    }
}

impl PgnVisitor {
    fn new() -> Self {
        PgnVisitor {
            game: PgnGame::new(),
            filter: PgnFilter::new(),
            skip: false,
        }
    }

    fn with_filter(filter: PgnFilter) -> Self {
        PgnVisitor {
            game: PgnGame::new(),
            filter,
            skip: false,
        }
    }

    fn into_game(self) -> PgnGame {
        self.game
    }
}

use pgn_reader::{BufferedReader, Skip, Visitor};

impl Visitor for PgnVisitor {
    type Result = PgnGame;

    fn begin_game(&mut self) {
        self.skip = false;
        self.game = PgnGame::new();
    }

    fn header(&mut self, key: &[u8], value: pgn_reader::RawHeader) {
        let k = std::str::from_utf8(key).unwrap().to_string();
        let v = value.decode_utf8().unwrap().to_string();

        match &k[..] {
            "TimeControl" => {
                let vs: Vec<&str> = v.split(|c| "/+-?*".contains(c)).collect();
                let mut nums = Vec::new();

                for num in vs {
                    if let Ok(n) = num.parse::<usize>() {
                        nums.push(n);
                    }
                }

                match nums.len() {
                    1 => {
                        self.game.time = Some(nums[0]);
                        self.game.increment = Some(0);
                    }
                    2 if v.contains('+') => {
                        self.game.time = Some(nums[0]);
                        self.game.increment = Some(nums[1]);
                    }
                    2 => {
                        self.game.time = Some(nums[1]);
                        self.game.increment = Some(0);
                    }
                    3 => {
                        self.game.time = Some(nums[1]);
                        self.game.increment = Some(nums[2]);
                    }
                    _ => {}
                }
            }
            "WhiteElo" => {
                if let Ok(e) = v.parse::<usize>() {
                    self.game.white_elo = Some(e);
                } else {
                    self.skip = true;
                }
            }
            "BlackElo" => {
                if let Ok(e) = v.parse::<usize>() {
                    self.game.black_elo = Some(e);
                } else {
                    self.skip = true;
                }
            }
            "Result" => match &v[..] {
                "1-0" => {
                    self.game.outcome = Outcome::Decisive {
                        winner: Color::White,
                    }
                }
                "0-1" => {
                    self.game.outcome = Outcome::Decisive {
                        winner: Color::Black,
                    }
                }
                _ => {}
            },
            // Useful when dealing with Lichess exports
            "Variant" => {
                if v != "Standard" {
                    self.skip = true
                }
            }
            _ => {}
        }

        self.game.headers.push((k, v));
    }

    fn end_headers(&mut self) -> Skip {
        Skip(!self.filter.header_matches(&self.game) || self.skip)
    }

    fn san(&mut self, san: SanPlus) {
        self.game.moves.push(san);
    }

    fn begin_variation(&mut self) -> Skip {
        Skip(true)
    }

    fn end_game(&mut self) -> PgnGame {
        std::mem::replace(&mut self.game, PgnGame::new())
    }
}

use std::io::{Read, Write};

pub fn read_games<R: Read>(filter: PgnFilter, read: R) -> Vec<PgnGame> {
    let mut visitor = PgnVisitor::with_filter(filter.clone());

    BufferedReader::new(read)
        .into_iter(&mut visitor)
        .map(|x| x.unwrap())
        .filter(|game| filter.matches(game))
        .collect()
}

pub fn fold_games<R, F>(filter: PgnFilter, read: R, f: &mut F)
where
    R: Read,
    F: FnMut(PgnGame),
{
    let mut visitor = PgnVisitor::with_filter(filter.clone());

    for game in BufferedReader::new(read).into_iter(&mut visitor) {
        let game = game.unwrap();

        if filter.matches(&game) {
            f(game)
        }
    }
}

pub fn write_games<W: Write>(w: &mut W, games: &[PgnGame]) {
    for g in games {
        writeln!(w, "{}", g).expect("Unable to write games!");
    }
}
