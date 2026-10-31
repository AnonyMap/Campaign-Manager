//! Dice expression engine modelled on the 5etools dice roller.
//!
//! Supported syntax (case-insensitive, whitespace ignored):
//!
//! * Arithmetic: `+ - * / ^` (also `**`, `×`, `÷`), parentheses, decimals
//! * Dice: `XdY`, `dY` (one die), `d%` (d100), chained dice `2d3d4`,
//!   computed counts/faces `(1+1)d(2*3)`
//! * Keep / drop: `4d6kh3`, `4d6dl1`, `3d4dh1`, `3d4kl1`
//! * Reroll once: `2d4r1`, `2d4r<2`, `2d4r<=2`, `2d4r>2`, `2d4r>=3`
//! * Explode: `2d4x4`, `2d4x<2`, `2d4x<=2`, `2d4x>2`, `2d4x>=3` (max 999 extra dice)
//! * Count successes: `2d4cs=4`, `cs<`, `cs<=`, `cs>`, `cs>=`
//! * Margin of success: `2d4ms=4` (and the comparison variants)
//! * Dice pools: `{2d8, 1d6}`, `{1d20+7, 10}kh1`
//! * Functions: `floor ceil round trunc sign abs sqrt cbrt exp log pow min max random`
//!   plus `avg(8d6)`, `dmax(8d6)`, `dmin(8d6)`
//! * Natural language: `plus`, `minus`, `times`, `over`, `divided by`
//!
//! Command-level syntax (see [`parse_command`]):
//! * `Label: 8d6` — anything before the first colon is a label
//! * `#macro` — expands a saved macro
//! * `... @Name` — trailing target, used by the app to save the roll to a character
//! * `/help`, `/clear`, `/macro add|remove|list`, `/iterroll <roll> <count> [target]`

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Maximum number of dice rolled for a single `XdY` term (guards against `99999999d6`).
pub const MAX_DICE: i64 = 10_000;
/// Maximum number of extra dice an explode modifier may add (matches 5etools).
pub const MAX_EXPLOSIONS: usize = 999;
/// Most dice (and sub-rolls) one roll may use, counting re-rolls and explosions.
pub const MAX_WORK: usize = 20_000;
/// Longest expression (in numbers, operators and brackets) we'll parse.
pub const MAX_TOKENS: usize = 400;
/// Deepest bracket nesting we'll parse.
pub const MAX_NESTING: usize = 32;

// ---------------------------------------------------------------------------
// Random source
// ---------------------------------------------------------------------------

/// Source of randomness. Abstracted so tests can supply fixed sequences.
pub trait DiceRng {
    /// Uniform integer in `1..=faces`.
    fn roll_die(&mut self, faces: u32) -> u32;
    /// Uniform float in `[0, 1)`.
    fn random_f64(&mut self) -> f64;
}

impl DiceRng for fastrand::Rng {
    fn roll_die(&mut self, faces: u32) -> u32 {
        self.u32(1..=faces.max(1))
    }
    fn random_f64(&mut self) -> f64 {
        self.f64()
    }
}

/// Deterministic RNG that replays a fixed list of die results (cycling).
/// Values larger than the die's face count are clamped.
#[derive(Debug, Clone)]
pub struct SequenceRng {
    values: Vec<u32>,
    pos: usize,
}

impl SequenceRng {
    pub fn new(values: Vec<u32>) -> Self {
        Self { values, pos: 0 }
    }
}

impl DiceRng for SequenceRng {
    fn roll_die(&mut self, faces: u32) -> u32 {
        if self.values.is_empty() {
            return 1;
        }
        let v = self.values[self.pos % self.values.len()];
        self.pos += 1;
        v.clamp(1, faces.max(1))
    }
    fn random_f64(&mut self) -> f64 {
        0.5
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DiceError {
    #[error("empty expression")]
    Empty,
    #[error("unexpected character '{0}'")]
    UnexpectedChar(char),
    #[error("unknown word '{0}'")]
    UnknownWord(String),
    #[error("unexpected end of expression")]
    UnexpectedEnd,
    #[error("unexpected token '{0}'")]
    UnexpectedToken(String),
    #[error("{0}")]
    Invalid(String),
    #[error("unknown macro '#{0}'")]
    UnknownMacro(String),
}

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cmp {
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Cmp {
    fn test(self, v: f64, n: f64) -> bool {
        match self {
            Cmp::Eq => v == n,
            Cmp::Lt => v < n,
            Cmp::Le => v <= n,
            Cmp::Gt => v > n,
            Cmp::Ge => v >= n,
        }
    }
    fn symbol(self) -> &'static str {
        match self {
            Cmp::Eq => "",
            Cmp::Lt => "<",
            Cmp::Le => "<=",
            Cmp::Gt => ">",
            Cmp::Ge => ">=",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModKind {
    KeepHighest,
    KeepLowest,
    DropHighest,
    DropLowest,
    Reroll(Cmp),
    Explode(Cmp),
    CountSuccess(Cmp),
    MarginSuccess(Cmp),
}

impl ModKind {
    fn is_keep_drop(self) -> bool {
        matches!(self, ModKind::KeepHighest | ModKind::KeepLowest | ModKind::DropHighest | ModKind::DropLowest)
    }
    fn label(self) -> String {
        match self {
            ModKind::KeepHighest => "kh".into(),
            ModKind::KeepLowest => "kl".into(),
            ModKind::DropHighest => "dh".into(),
            ModKind::DropLowest => "dl".into(),
            ModKind::Reroll(c) => format!("r{}", c.symbol()),
            ModKind::Explode(c) => format!("x{}", c.symbol()),
            ModKind::CountSuccess(c) => format!("cs{}", if c == Cmp::Eq { "=" } else { c.symbol() }),
            ModKind::MarginSuccess(c) => format!("ms{}", if c == Cmp::Eq { "=" } else { c.symbol() }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Floor,
    Ceil,
    Round,
    Trunc,
    Sign,
    Abs,
    Sqrt,
    Cbrt,
    Exp,
    Log,
    Pow,
    Min,
    Max,
    Random,
    Avg,
    DMax,
    DMin,
}

impl Func {
    fn from_word(w: &str) -> Option<Func> {
        Some(match w {
            "floor" => Func::Floor,
            "ceil" => Func::Ceil,
            "round" => Func::Round,
            "trunc" => Func::Trunc,
            "sign" => Func::Sign,
            "abs" => Func::Abs,
            "sqrt" => Func::Sqrt,
            "cbrt" => Func::Cbrt,
            "exp" => Func::Exp,
            "log" => Func::Log,
            "pow" => Func::Pow,
            "min" => Func::Min,
            "max" => Func::Max,
            "random" => Func::Random,
            "avg" => Func::Avg,
            "dmax" => Func::DMax,
            "dmin" => Func::DMin,
            _ => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Func::Floor => "floor",
            Func::Ceil => "ceil",
            Func::Round => "round",
            Func::Trunc => "trunc",
            Func::Sign => "sign",
            Func::Abs => "abs",
            Func::Sqrt => "sqrt",
            Func::Cbrt => "cbrt",
            Func::Exp => "exp",
            Func::Log => "log",
            Func::Pow => "pow",
            Func::Min => "min",
            Func::Max => "max",
            Func::Random => "random",
            Func::Avg => "avg",
            Func::DMax => "dmax",
            Func::DMin => "dmin",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Plus,
    Minus,
    Mul,
    Div,
    Pow,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    D,
    Mod(ModKind),
    Func(Func),
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tok::Num(n) => write!(f, "{}", fmt_num(*n)),
            Tok::Plus => write!(f, "+"),
            Tok::Minus => write!(f, "-"),
            Tok::Mul => write!(f, "*"),
            Tok::Div => write!(f, "/"),
            Tok::Pow => write!(f, "^"),
            Tok::LParen => write!(f, "("),
            Tok::RParen => write!(f, ")"),
            Tok::LBrace => write!(f, "{{"),
            Tok::RBrace => write!(f, "}}"),
            Tok::Comma => write!(f, ","),
            Tok::D => write!(f, "d"),
            Tok::Mod(m) => write!(f, "{}", m.label()),
            Tok::Func(func) => write!(f, "{}", func.name()),
        }
    }
}

/// Normalise input in the same spirit as 5etools.
fn normalise(input: &str) -> String {
    let mut s = format!(" {} ", input.trim().to_lowercase());
    for (from, to) in [
        (" divided by ", " / "),
        (" plus ", " + "),
        (" minus ", " - "),
        (" times ", " * "),
        (" over ", " / "),
    ] {
        while s.contains(from) {
            s = s.replace(from, to);
        }
    }
    let mut s: String = s
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
            '×' => '*',
            '÷' => '/',
            other => other,
        })
        .collect();
    s = s.replace("**", "^");
    s = s.replace("d%", "d100");
    loop {
        let before = s.clone();
        s = s.replace("--", "+").replace("+-", "-").replace("-+", "-").replace("++", "+");
        if s == before {
            break;
        }
    }
    s
}

fn lex(input: &str) -> Result<Vec<Tok>, DiceError> {
    let s = normalise(input);
    let chars: Vec<char> = s.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let n: f64 = text.parse().map_err(|_| DiceError::Invalid(format!("bad number '{text}'")))?;
            toks.push(Tok::Num(n));
            continue;
        }
        if c.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            // Optional comparison operator following r / x / cs / ms.
            let mut cmp = String::new();
            while i < chars.len() && matches!(chars[i], '<' | '>' | '=') && cmp.len() < 2 {
                cmp.push(chars[i]);
                i += 1;
            }
            let parse_cmp = |cmp: &str, eq_required: bool| -> Result<Cmp, DiceError> {
                Ok(match cmp {
                    "" if !eq_required => Cmp::Eq,
                    "=" => Cmp::Eq,
                    "<" => Cmp::Lt,
                    "<=" => Cmp::Le,
                    ">" => Cmp::Gt,
                    ">=" => Cmp::Ge,
                    other => {
                        return Err(DiceError::Invalid(format!("bad comparison '{other}' after '{word}'")));
                    }
                })
            };
            let tok = match word.as_str() {
                "d" => Tok::D,
                "kh" => Tok::Mod(ModKind::KeepHighest),
                "kl" => Tok::Mod(ModKind::KeepLowest),
                "dh" => Tok::Mod(ModKind::DropHighest),
                "dl" => Tok::Mod(ModKind::DropLowest),
                "r" => Tok::Mod(ModKind::Reroll(parse_cmp(&cmp, false)?)),
                "x" => Tok::Mod(ModKind::Explode(parse_cmp(&cmp, false)?)),
                "cs" => Tok::Mod(ModKind::CountSuccess(parse_cmp(&cmp, true)?)),
                "ms" => Tok::Mod(ModKind::MarginSuccess(parse_cmp(&cmp, true)?)),
                w => match Func::from_word(w) {
                    Some(f) => Tok::Func(f),
                    None => return Err(DiceError::UnknownWord(word.clone())),
                },
            };
            if !cmp.is_empty()
                && !matches!(
                    tok,
                    Tok::Mod(ModKind::Reroll(_) | ModKind::Explode(_) | ModKind::CountSuccess(_) | ModKind::MarginSuccess(_))
                )
            {
                return Err(DiceError::UnexpectedChar(cmp.chars().next().unwrap()));
            }
            toks.push(tok);
            continue;
        }
        let tok = match c {
            '+' => Tok::Plus,
            '-' => Tok::Minus,
            '*' => Tok::Mul,
            '/' => Tok::Div,
            '^' => Tok::Pow,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            '{' => Tok::LBrace,
            '}' => Tok::RBrace,
            ',' => Tok::Comma,
            other => return Err(DiceError::UnexpectedChar(other)),
        };
        toks.push(tok);
        i += 1;
    }
    Ok(toks)
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Modifier {
    kind: ModKind,
    arg: Node,
}

#[derive(Debug, Clone)]
enum Node {
    Num(f64),
    Neg(Box<Node>),
    Bin(Box<Node>, BinOp, Box<Node>),
    Paren(Box<Node>),
    Dice {
        count: Option<Box<Node>>,
        faces: Box<Node>,
        mods: Vec<Modifier>,
    },
    Pool {
        items: Vec<Node>,
        mods: Vec<Modifier>,
    },
    Call(Func, Vec<Node>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

impl BinOp {
    fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => " + ",
            BinOp::Sub => " - ",
            BinOp::Mul => " × ",
            BinOp::Div => " ÷ ",
            BinOp::Pow => "^",
        }
    }
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: Tok) -> Result<(), DiceError> {
        match self.next() {
            Some(ref got) if *got == t => Ok(()),
            Some(got) => Err(DiceError::UnexpectedToken(got.to_string())),
            None => Err(DiceError::UnexpectedEnd),
        }
    }

    fn expr(&mut self) -> Result<Node, DiceError> {
        let mut left = self.term()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Plus) => BinOp::Add,
                Some(Tok::Minus) => BinOp::Sub,
                _ => break,
            };
            self.pos += 1;
            let right = self.term()?;
            left = Node::Bin(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn term(&mut self) -> Result<Node, DiceError> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Mul) => BinOp::Mul,
                Some(Tok::Div) => BinOp::Div,
                _ => break,
            };
            self.pos += 1;
            let right = self.unary()?;
            left = Node::Bin(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Node, DiceError> {
        if self.eat(&Tok::Minus) {
            return Ok(Node::Neg(Box::new(self.unary()?)));
        }
        if self.eat(&Tok::Plus) {
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Result<Node, DiceError> {
        let base = self.dice()?;
        if self.eat(&Tok::Pow) {
            let exp = self.unary()?; // right associative
            return Ok(Node::Bin(Box::new(base), BinOp::Pow, Box::new(exp)));
        }
        Ok(base)
    }

    fn dice(&mut self) -> Result<Node, DiceError> {
        let mut node = if self.peek() == Some(&Tok::D) { None } else { Some(self.atom()?) };
        while self.eat(&Tok::D) {
            let faces = self.atom()?;
            let mods = self.mods()?;
            node = Some(Node::Dice {
                count: node.map(Box::new),
                faces: Box::new(faces),
                mods,
            });
        }
        node.ok_or(DiceError::UnexpectedEnd)
    }

    fn mods(&mut self) -> Result<Vec<Modifier>, DiceError> {
        let mut mods = Vec::new();
        while let Some(Tok::Mod(kind)) = self.peek().cloned() {
            self.pos += 1;
            let has_arg = matches!(self.peek(), Some(Tok::Num(_)) | Some(Tok::LParen));
            let arg = if has_arg {
                self.atom()?
            } else if kind.is_keep_drop() {
                Node::Num(1.0)
            } else {
                return Err(DiceError::Invalid(format!("modifier '{}' needs a number", kind.label())));
            };
            mods.push(Modifier { kind, arg });
        }
        Ok(mods)
    }

    fn atom(&mut self) -> Result<Node, DiceError> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Node::Num(n)),
            Some(Tok::LParen) => {
                let inner = self.expr()?;
                self.expect(Tok::RParen)?;
                Ok(Node::Paren(Box::new(inner)))
            }
            Some(Tok::LBrace) => {
                let mut items = vec![self.expr()?];
                while self.eat(&Tok::Comma) {
                    items.push(self.expr()?);
                }
                self.expect(Tok::RBrace)?;
                let mods = self.mods()?;
                Ok(Node::Pool { items, mods })
            }
            Some(Tok::Func(f)) => {
                self.expect(Tok::LParen)?;
                let mut args = Vec::new();
                if !self.eat(&Tok::RParen) {
                    args.push(self.expr()?);
                    while self.eat(&Tok::Comma) {
                        args.push(self.expr()?);
                    }
                    self.expect(Tok::RParen)?;
                }
                let arity_ok = match f {
                    Func::Random => true,
                    Func::Pow => args.len() == 2,
                    Func::Min | Func::Max => !args.is_empty(),
                    _ => args.len() == 1,
                };
                if !arity_ok {
                    return Err(DiceError::Invalid(format!("wrong number of arguments for {}()", f.name())));
                }
                Ok(Node::Call(f, args))
            }
            // Unary minus inside an atom position, e.g. `2d(-1)` -> handled by expr; here we
            // allow `-` before atoms used as modifier args / counts.
            Some(Tok::Minus) => Ok(Node::Neg(Box::new(self.atom()?))),
            Some(t) => Err(DiceError::UnexpectedToken(t.to_string())),
            None => Err(DiceError::UnexpectedEnd),
        }
    }
}

/// A parsed, reusable dice expression.
#[derive(Debug, Clone)]
pub struct DiceExpr {
    source: String,
    root: Node,
}

impl DiceExpr {
    pub fn parse(input: &str) -> Result<DiceExpr, DiceError> {
        let toks = lex(input)?;
        if toks.is_empty() {
            return Err(DiceError::Empty);
        }
        // Parsing and rolling walk the expression recursively: keep it to a sane size so
        // pasted junk can't overflow the stack.
        if toks.len() > MAX_TOKENS {
            return Err(DiceError::Invalid(format!("that's too long to roll (over {MAX_TOKENS} parts)")));
        }
        let mut depth = 0usize;
        for t in &toks {
            match t {
                Tok::LParen => {
                    depth += 1;
                    if depth > MAX_NESTING {
                        return Err(DiceError::Invalid(format!("too many brackets inside each other (over {MAX_NESTING})")));
                    }
                }
                Tok::RParen => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        let mut p = Parser { toks, pos: 0 };
        let root = p.expr()?;
        if let Some(t) = p.peek() {
            return Err(DiceError::UnexpectedToken(t.to_string()));
        }
        Ok(DiceExpr {
            source: input.trim().to_string(),
            root,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// True when the expression contains no dice (a plain calculation).
    pub fn is_constant(&self) -> bool {
        fn has_dice(n: &Node) -> bool {
            match n {
                Node::Num(_) => false,
                Node::Neg(a) | Node::Paren(a) => has_dice(a),
                Node::Bin(a, _, b) => has_dice(a) || has_dice(b),
                Node::Dice { .. } => true,
                Node::Pool { items, .. } => items.iter().any(has_dice),
                Node::Call(Func::Random, _) => true,
                Node::Call(_, args) => args.iter().any(has_dice),
            }
        }
        !has_dice(&self.root)
    }

    pub fn roll(&self, rng: &mut dyn DiceRng) -> Result<RollOutcome, DiceError> {
        self.eval_mode(rng, Mode::Roll)
    }

    pub fn average(&self) -> Result<f64, DiceError> {
        let mut rng = SequenceRng::new(vec![1]);
        Ok(self.eval_mode(&mut rng, Mode::Avg)?.total)
    }

    pub fn minimum(&self) -> Result<f64, DiceError> {
        let mut rng = SequenceRng::new(vec![1]);
        Ok(self.eval_mode(&mut rng, Mode::Min)?.total)
    }

    pub fn maximum(&self) -> Result<f64, DiceError> {
        let mut rng = SequenceRng::new(vec![1]);
        Ok(self.eval_mode(&mut rng, Mode::Max)?.total)
    }

    fn eval_mode(&self, rng: &mut dyn DiceRng, mode: Mode) -> Result<RollOutcome, DiceError> {
        let mut ctx = EvalCtx {
            rng,
            segs: Vec::new(),
            nat20: false,
            nat1: false,
            work: 0,
        };
        let total = ctx.eval(&self.root, mode, true)?;
        if !total.is_finite() {
            return Err(DiceError::Invalid("result is not a finite number".into()));
        }
        Ok(RollOutcome {
            total,
            segments: merge_segments(ctx.segs),
            nat20: ctx.nat20,
            nat1: ctx.nat1,
        })
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Roll,
    Avg,
    Min,
    Max,
}

/// Kind of a breakdown segment, so a UI can colour dropped / exploded dice etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegKind {
    Text,
    Die,
    DieMax,
    DieMin,
    Dropped,
    Exploded,
    Success,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub text: String,
    pub kind: SegKind,
}

fn merge_segments(segs: Vec<Segment>) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::with_capacity(segs.len());
    for s in segs {
        if let Some(last) = out.last_mut() {
            if last.kind == SegKind::Text && s.kind == SegKind::Text {
                last.text.push_str(&s.text);
                continue;
            }
        }
        out.push(s);
    }
    out
}

/// Result of evaluating an expression.
#[derive(Debug, Clone, PartialEq)]
pub struct RollOutcome {
    pub total: f64,
    pub segments: Vec<Segment>,
    /// A kept d20 showed a natural 20.
    pub nat20: bool,
    /// A kept d20 showed a natural 1.
    pub nat1: bool,
}

impl RollOutcome {
    pub fn breakdown(&self) -> String {
        self.segments.iter().map(|s| s.text.as_str()).collect()
    }
}

#[derive(Debug, Clone)]
struct DieVal {
    val: f64,
    dropped: bool,
    exploded: bool,
    success: bool,
    /// Display override (margin of success).
    display: Option<String>,
    /// For pools: the item to re-evaluate on reroll/explode, and its breakdown.
    item: Option<usize>,
    item_segs: Vec<Segment>,
}

impl DieVal {
    fn new(val: f64) -> Self {
        DieVal {
            val,
            dropped: false,
            exploded: false,
            success: false,
            display: None,
            item: None,
            item_segs: Vec::new(),
        }
    }
}

struct EvalCtx<'a> {
    rng: &'a mut dyn DiceRng,
    segs: Vec<Segment>,
    nat20: bool,
    nat1: bool,
    /// Dice rolled and sub-expressions evaluated so far (see MAX_WORK).
    work: usize,
}

fn as_int(v: f64, what: &str) -> Result<i64, DiceError> {
    if v.fract() != 0.0 || !v.is_finite() {
        return Err(DiceError::Invalid(format!("{what} ({}) must be a whole number", fmt_num(v))));
    }
    Ok(v as i64)
}

impl<'a> EvalCtx<'a> {
    fn text(&mut self, t: impl Into<String>) {
        self.segs.push(Segment {
            text: t.into(),
            kind: SegKind::Text,
        });
    }

    /// Evaluate a node without recording its breakdown (used for counts/faces/args).
    fn eval_quiet(&mut self, n: &Node, mode: Mode) -> Result<f64, DiceError> {
        let saved = std::mem::take(&mut self.segs);
        let r = self.eval(n, mode, false);
        self.segs = saved;
        r
    }

    /// Count work done, and stop once a single roll has done far too much (exploding
    /// pools inside exploding pools can otherwise run for minutes).
    fn spend(&mut self, n: usize) -> Result<(), DiceError> {
        self.work = self.work.saturating_add(n);
        if self.work > MAX_WORK {
            return Err(DiceError::Invalid(format!("that roll needs too many dice (over {MAX_WORK})")));
        }
        Ok(())
    }

    fn eval(&mut self, n: &Node, mode: Mode, top: bool) -> Result<f64, DiceError> {
        self.spend(1)?;
        match n {
            Node::Num(v) => {
                self.text(fmt_num(*v));
                Ok(*v)
            }
            Node::Neg(a) => {
                self.text("-");
                Ok(-self.eval(a, mode, false)?)
            }
            Node::Paren(a) => {
                self.text("(");
                let v = self.eval(a, mode, false)?;
                self.text(")");
                Ok(v)
            }
            Node::Bin(a, op, b) => {
                let x = self.eval(a, mode, false)?;
                self.text(op.symbol());
                let y = self.eval(b, mode, false)?;
                Ok(match op {
                    BinOp::Add => x + y,
                    BinOp::Sub => x - y,
                    BinOp::Mul => x * y,
                    BinOp::Div => {
                        if y == 0.0 {
                            return Err(DiceError::Invalid("division by zero".into()));
                        }
                        x / y
                    }
                    BinOp::Pow => x.powf(y),
                })
            }
            Node::Call(f, args) => self.eval_call(*f, args, mode),
            Node::Dice { count, faces, mods } => {
                let _ = top;
                let count_v = match count {
                    None => 1.0,
                    Some(c) => {
                        // chained dice (2d3d4) show the inner roll in brackets
                        if matches!(**c, Node::Dice { .. }) {
                            let saved = std::mem::take(&mut self.segs);
                            let v = self.eval(c, mode, false)?;
                            let inner = std::mem::replace(&mut self.segs, saved);
                            self.text("(");
                            self.segs.extend(inner);
                            self.text(")");
                            v
                        } else {
                            self.eval_quiet(c, mode)?
                        }
                    }
                };
                let faces_v = self.eval_quiet(faces, mode)?;
                if mode == Mode::Avg {
                    // 5etools ignores modifiers in average mode; allow fractional counts
                    // (e.g. avg(1d4d6)).
                    if faces_v < 1.0 {
                        return Err(DiceError::Invalid("dice need at least 1 face".into()));
                    }
                    let v = count_v * (faces_v + 1.0) / 2.0;
                    self.text(format!("{}d{}≈{}", fmt_num(count_v), fmt_num(faces_v), fmt_num(v)));
                    return Ok(v);
                }
                let count_i = as_int(count_v, "number of dice")?;
                let faces_i = as_int(faces_v, "die faces")?;
                if count_i < 0 {
                    return Err(DiceError::Invalid("number of dice can't be negative".into()));
                }
                if count_i > MAX_DICE {
                    return Err(DiceError::Invalid(format!("too many dice (max {MAX_DICE})")));
                }
                if faces_i < 1 {
                    return Err(DiceError::Invalid("dice need at least 1 face".into()));
                }
                if faces_i > u32::MAX as i64 {
                    return Err(DiceError::Invalid("die is too large".into()));
                }
                self.eval_dice(count_i as usize, faces_i as u32, mods, mode)
            }
            Node::Pool { items, mods } => self.eval_pool(items, mods, mode),
        }
    }

    fn eval_call(&mut self, f: Func, args: &[Node], mode: Mode) -> Result<f64, DiceError> {
        match f {
            Func::Avg | Func::DMax | Func::DMin => {
                let sub = match f {
                    Func::Avg => Mode::Avg,
                    Func::DMax => Mode::Max,
                    _ => Mode::Min,
                };
                self.text(format!("{}(", f.name()));
                let v = self.eval(&args[0], sub, false)?;
                self.text(")");
                Ok(v)
            }
            Func::Random => {
                self.text("random()");
                Ok(match mode {
                    Mode::Roll => self.rng.random_f64(),
                    Mode::Avg => 0.5,
                    Mode::Min => 0.0,
                    Mode::Max => 1.0,
                })
            }
            _ => {
                self.text(format!("{}(", f.name()));
                let mut vals = Vec::with_capacity(args.len());
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.text(", ");
                    }
                    vals.push(self.eval(a, mode, false)?);
                }
                self.text(")");
                let x = vals[0];
                Ok(match f {
                    Func::Floor => x.floor(),
                    Func::Ceil => x.ceil(),
                    // JS Math.round rounds .5 toward +infinity
                    Func::Round => (x + 0.5).floor(),
                    Func::Trunc => x.trunc(),
                    Func::Sign => {
                        if x > 0.0 {
                            1.0
                        } else if x < 0.0 {
                            -1.0
                        } else {
                            0.0
                        }
                    }
                    Func::Abs => x.abs(),
                    Func::Sqrt => x.sqrt(),
                    Func::Cbrt => x.cbrt(),
                    Func::Exp => x.exp(),
                    Func::Log => x.ln(),
                    Func::Pow => x.powf(vals[1]),
                    Func::Min => vals.iter().cloned().fold(f64::INFINITY, f64::min),
                    Func::Max => vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                    Func::Random | Func::Avg | Func::DMax | Func::DMin => unreachable!(),
                })
            }
        }
    }

    fn face_value(&mut self, faces: u32, mode: Mode) -> f64 {
        match mode {
            Mode::Roll => self.rng.roll_die(faces) as f64,
            Mode::Avg => (faces as f64 + 1.0) / 2.0,
            Mode::Min => 1.0,
            Mode::Max => faces as f64,
        }
    }

    fn eval_dice(&mut self, count: usize, faces: u32, mods: &[Modifier], mode: Mode) -> Result<f64, DiceError> {
        self.spend(count)?;
        if mode == Mode::Avg {
            // 5etools ignores modifiers in average mode
            let v = count as f64 * (faces as f64 + 1.0) / 2.0;
            self.text(format!("{count}d{faces}≈{}", fmt_num(v)));
            return Ok(v);
        }
        let mut vals: Vec<DieVal> = (0..count).map(|_| DieVal::new(self.face_value(faces, mode))).collect();
        let success_mode = self.apply_mods(&mut vals, mods, mode, &mut |ctx: &mut EvalCtx, _idx| {
            Ok((ctx.face_value(faces, mode), Vec::new()))
        })?;

        // Natural 20 / 1 tracking for kept d20s.
        if faces == 20 && mode == Mode::Roll {
            for v in vals.iter().filter(|v| !v.dropped && v.display.is_none()) {
                if v.val == 20.0 {
                    self.nat20 = true;
                }
                if v.val == 1.0 {
                    self.nat1 = true;
                }
            }
        }

        let total = finish_total(&vals, success_mode);
        // Breakdown: [a]+[b]+...
        if vals.is_empty() {
            self.text("[0]");
        }
        for (i, v) in vals.iter().enumerate() {
            if i > 0 {
                self.text("+");
            }
            if let Some(d) = &v.display {
                self.text(d.clone());
                continue;
            }
            let kind = if v.dropped {
                SegKind::Dropped
            } else if v.success {
                SegKind::Success
            } else if v.exploded {
                SegKind::Exploded
            } else if v.val == faces as f64 && faces > 1 {
                SegKind::DieMax
            } else if v.val == 1.0 && faces > 1 {
                SegKind::DieMin
            } else {
                SegKind::Die
            };
            self.segs.push(Segment {
                text: format!("[{}]", fmt_num(v.val)),
                kind,
            });
        }
        if success_mode {
            self.text(format!(" ⇒ {} successes", fmt_num(total)));
        }
        Ok(total)
    }

    fn eval_pool(&mut self, items: &[Node], mods: &[Modifier], mode: Mode) -> Result<f64, DiceError> {
        let mut vals = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            let saved = std::mem::take(&mut self.segs);
            let v = self.eval(item, mode, false)?;
            let item_segs = std::mem::replace(&mut self.segs, saved);
            let mut dv = DieVal::new(v);
            dv.item = Some(i);
            dv.item_segs = item_segs;
            vals.push(dv);
        }
        let success_mode = if mode == Mode::Avg {
            false
        } else {
            self.apply_mods(&mut vals, mods, mode, &mut |ctx: &mut EvalCtx, idx| {
                let saved = std::mem::take(&mut ctx.segs);
                let v = ctx.eval(&items[idx], mode, false)?;
                let s = std::mem::replace(&mut ctx.segs, saved);
                Ok((v, s))
            })?
        };
        let total = finish_total(&vals, success_mode);
        self.text("{");
        for (i, v) in vals.iter().enumerate() {
            if i > 0 {
                self.text(", ");
            }
            if let Some(d) = &v.display {
                self.text(d.clone());
                continue;
            }
            if v.dropped {
                let txt: String = v.item_segs.iter().map(|s| s.text.as_str()).collect();
                self.segs.push(Segment {
                    text: format!("({txt})"),
                    kind: SegKind::Dropped,
                });
            } else {
                self.segs.extend(v.item_segs.iter().cloned());
            }
        }
        self.text("}");
        if success_mode {
            self.text(format!(" ⇒ {} successes", fmt_num(total)));
        }
        Ok(total)
    }

    /// Apply modifiers in order. `reroll(ctx, item_index)` produces a fresh value.
    /// Returns whether the result should be a success count.
    fn apply_mods(
        &mut self,
        vals: &mut Vec<DieVal>,
        mods: &[Modifier],
        mode: Mode,
        reroll: &mut dyn FnMut(&mut EvalCtx, usize) -> Result<(f64, Vec<Segment>), DiceError>,
    ) -> Result<bool, DiceError> {
        let mut success_mode = false;
        for m in mods {
            let n = self.eval_quiet(&m.arg, mode)?;
            // indices of alive values sorted by value descending (stable)
            let mut alive: Vec<usize> = (0..vals.len()).filter(|&i| !vals[i].dropped).collect();
            alive.sort_by(|&a, &b| vals[b].val.partial_cmp(&vals[a].val).unwrap_or(std::cmp::Ordering::Equal));
            match m.kind {
                ModKind::KeepHighest | ModKind::KeepLowest | ModKind::DropHighest | ModKind::DropLowest => {
                    let n = as_int(n, "keep/drop count")?.max(0) as usize;
                    let is_highest = matches!(m.kind, ModKind::KeepHighest | ModKind::DropHighest);
                    let split = if is_highest { n.min(alive.len()) } else { alive.len().saturating_sub(n) };
                    let (high, low) = alive.split_at(split);
                    let to_drop: &[usize] = match m.kind {
                        ModKind::DropHighest | ModKind::KeepLowest => high,
                        _ => low,
                    };
                    for &i in to_drop {
                        vals[i].dropped = true;
                    }
                }
                ModKind::Reroll(cmp) => {
                    let targets: Vec<usize> = alive.into_iter().filter(|&i| cmp.test(vals[i].val, n)).collect();
                    for i in targets {
                        vals[i].dropped = true;
                        let item = vals[i].item;
                        self.spend(1)?;
                        let (v, segs) = reroll(self, item.unwrap_or(0))?;
                        let mut dv = DieVal::new(v);
                        dv.item = item;
                        dv.item_segs = segs;
                        vals.push(dv);
                    }
                }
                ModKind::Explode(cmp) => {
                    let mut frontier: Vec<usize> = alive;
                    let mut added = 0usize;
                    while !frontier.is_empty() && added < MAX_EXPLOSIONS {
                        let mut next = Vec::new();
                        for i in frontier {
                            if added >= MAX_EXPLOSIONS {
                                break;
                            }
                            if !vals[i].exploded && cmp.test(vals[i].val, n) {
                                vals[i].exploded = true;
                                let item = vals[i].item;
                                self.spend(1)?;
                                let (v, segs) = reroll(self, item.unwrap_or(0))?;
                                let mut dv = DieVal::new(v);
                                dv.item = item;
                                dv.item_segs = segs;
                                vals.push(dv);
                                next.push(vals.len() - 1);
                                added += 1;
                            }
                        }
                        frontier = next;
                    }
                }
                ModKind::CountSuccess(cmp) => {
                    success_mode = true;
                    for i in alive {
                        if cmp.test(vals[i].val, n) {
                            vals[i].success = true;
                        }
                    }
                }
                ModKind::MarginSuccess(cmp) => {
                    let total: f64 = alive.iter().map(|&i| vals[i].val).sum();
                    let dice_txt: String = vals.iter().map(|v| format!("[{}]", fmt_num(v.val))).collect::<Vec<_>>().join("+");
                    let (delta, disp) = match cmp {
                        Cmp::Lt | Cmp::Le => (n - total, format!("{}-({dice_txt})", fmt_num(n))),
                        _ => (total - n, format!("({dice_txt})-{}", fmt_num(n))),
                    };
                    vals.clear();
                    let mut dv = DieVal::new(delta);
                    dv.display = Some(disp);
                    vals.push(dv);
                    success_mode = false;
                }
            }
        }
        Ok(success_mode)
    }
}

fn finish_total(vals: &[DieVal], success_mode: bool) -> f64 {
    if success_mode {
        vals.iter().filter(|v| !v.dropped && v.success).count() as f64
    } else {
        vals.iter().filter(|v| !v.dropped).map(|v| v.val).sum()
    }
}

/// Format a number without a trailing `.0`, and with at most 3 decimals.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

// ---------------------------------------------------------------------------
// Commands, labels, macros
// ---------------------------------------------------------------------------

/// A stored roll in the history log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RollRecord {
    pub id: uuid::Uuid,
    /// What was typed (after macro expansion, without label/target).
    pub expression: String,
    pub label: String,
    /// Who it was rolled for / by (e.g. a character's name), optional.
    pub roller: String,
    pub total: f64,
    pub breakdown: Vec<Segment>,
    pub nat20: bool,
    pub nat1: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl RollRecord {
    pub fn breakdown_text(&self) -> String {
        self.breakdown.iter().map(|s| s.text.as_str()).collect()
    }
    pub fn total_text(&self) -> String {
        fmt_num(self.total)
    }
}

/// Parsed roller input.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Roll {
        label: String,
        expression: String,
        target: Option<String>,
    },
    IterRoll {
        expression: String,
        count: u32,
        target: Option<f64>,
    },
    Help,
    Clear,
    MacroList,
    MacroAdd {
        name: String,
        body: String,
    },
    MacroRemove {
        name: String,
    },
}

/// Split `Label: expr @Target` into its parts.
fn split_roll(input: &str) -> (String, String, Option<String>) {
    let mut s = input.trim().to_string();
    let mut target = None;
    if let Some(at) = s.rfind('@') {
        let t = s[at + 1..].trim().to_string();
        if !t.is_empty() {
            target = Some(t);
        }
        s.truncate(at);
    }
    let (label, expr) = match s.find(':') {
        Some(idx) => (s[..idx].trim().to_string(), s[idx + 1..].trim().to_string()),
        None => (String::new(), s.trim().to_string()),
    };
    (label, expr, target)
}

/// Parse a line typed into the dice roller.
pub fn parse_command(input: &str) -> Result<Command, DiceError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(DiceError::Empty);
    }
    if let Some(rest) = trimmed.strip_prefix('/') {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        let op = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
        return match op.as_str() {
            "help" | "h" => Ok(Command::Help),
            "clear" | "c" | "cls" => Ok(Command::Clear),
            "macro" => match parts.get(1).map(|s| s.to_lowercase()).as_deref() {
                Some("list") => Ok(Command::MacroList),
                Some("add") if parts.len() >= 4 => {
                    let name = parts[2].trim_start_matches('#').to_string();
                    if name.contains('#') {
                        return Err(DiceError::Invalid("macro names can't contain '#'".into()));
                    }
                    Ok(Command::MacroAdd {
                        name,
                        body: parts[3..].join(" "),
                    })
                }
                Some("remove") if parts.len() == 3 => Ok(Command::MacroRemove {
                    name: parts[2].trim_start_matches('#').to_string(),
                }),
                _ => Err(DiceError::Invalid("usage: /macro list | /macro add name 1d2+3 | /macro remove name".into())),
            },
            "iterroll" => {
                if parts.len() < 3 {
                    return Err(DiceError::Invalid("usage: /iterroll <roll> <count> [target]".into()));
                }
                let count: u32 = parts[2].parse().map_err(|_| DiceError::Invalid("count must be a whole number".into()))?;
                if count == 0 || count > 1000 {
                    return Err(DiceError::Invalid("count must be between 1 and 1000".into()));
                }
                let target = match parts.get(3) {
                    Some(t) => Some(t.parse::<f64>().map_err(|_| DiceError::Invalid("target must be a number".into()))?),
                    None => None,
                };
                Ok(Command::IterRoll {
                    expression: parts[1].to_string(),
                    count,
                    target,
                })
            }
            other => Err(DiceError::Invalid(format!("unknown command '/{other}'"))),
        };
    }
    let (label, expression, target) = split_roll(trimmed);
    if expression.is_empty() {
        return Err(DiceError::Empty);
    }
    Ok(Command::Roll { label, expression, target })
}

/// Replace `#name` references with macro bodies (non-recursive beyond 8 levels).
pub fn expand_macros(expr: &str, macros: &BTreeMap<String, String>) -> Result<String, DiceError> {
    let mut s = expr.to_string();
    for _ in 0..8 {
        if !s.contains('#') {
            return Ok(s);
        }
        let mut out = String::new();
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '#' {
                let start = i + 1;
                let mut j = start;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '-') {
                    j += 1;
                }
                let name: String = chars[start..j].iter().collect();
                let body = macros
                    .get(&name)
                    .or_else(|| macros.iter().find(|(k, _)| k.eq_ignore_ascii_case(&name)).map(|(_, v)| v))
                    .ok_or_else(|| DiceError::UnknownMacro(name.clone()))?;
                out.push('(');
                out.push_str(body);
                out.push(')');
                i = j;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        if out.len() > 2_000 {
            return Err(DiceError::Invalid("macros expand to something too long (does one use itself?)".into()));
        }
        s = out;
    }
    Err(DiceError::Invalid("macros nest too deeply".into()))
}

/// Convenience: evaluate an expression (with macros) and build a history record.
pub fn roll_to_record(label: &str, expression: &str, roller: &str, macros: &BTreeMap<String, String>, rng: &mut dyn DiceRng) -> Result<RollRecord, DiceError> {
    let expanded = expand_macros(expression, macros)?;
    let expr = DiceExpr::parse(&expanded)?;
    let out = expr.roll(rng)?;
    Ok(RollRecord {
        id: uuid::Uuid::new_v4(),
        expression: expression.trim().to_string(),
        label: label.trim().to_string(),
        roller: roller.trim().to_string(),
        total: out.total,
        breakdown: out.segments,
        nat20: out.nat20,
        nat1: out.nat1,
        timestamp: chrono::Utc::now(),
    })
}

/// Quick one-shot evaluation with a fresh RNG.
pub fn roll(expression: &str) -> Result<RollOutcome, DiceError> {
    let mut rng = fastrand::Rng::new();
    DiceExpr::parse(expression)?.roll(&mut rng)
}

/// Help text shown by `/help`.
pub const HELP_TEXT: &str = "\
Basic:        1d20+5   d20   2d6+1d4+3   d%   (1+1)d6   2d3d4 (chained)
Keep/drop:    4d6kh3   4d6dl1   3d4dh1   3d4kl1   2d20kh1 (advantage)
Reroll once:  2d4r1   2d4r<2   2d4r<=2   2d4r>2   2d4r>=3
Explode:      2d4x4   2d4x<2   2d4x<=2   2d4x>2   2d4x>=3
Successes:    2d4cs=4   2d4cs<2   2d4cs<=2   2d4cs>2   2d4cs>=3
Margin:       2d4ms=4
Pools:        {2d8, 1d6}   {1d20+7, 10}kh1
Functions:    floor() ceil() round() trunc() abs() sign() sqrt() cbrt() exp() log()
              pow(a,b) min(a,b,..) max(a,b,..) random()  avg(8d6) dmax(8d6) dmin(8d6)
Math:         + - * / ^ ( )   also 'plus', 'minus', 'times', 'over'
Labels:       Fireball: 8d6        (text before ':' is a label)
Save to NPC:  Bite: 1d6+2 @Goblin  (saves the roll as a custom roll on that character)
Macros:       /macro add sneak 3d6   then  #sneak   /macro list   /macro remove sneak
Repeat:       /iterroll 1d20+5 10 [target]
Other:        /clear  /help   Up/Down arrows cycle input history";

#[cfg(test)]
mod tests {
    use super::*;

    fn seq(v: &[u32]) -> SequenceRng {
        SequenceRng::new(v.to_vec())
    }

    fn eval(expr: &str, dice: &[u32]) -> f64 {
        DiceExpr::parse(expr).unwrap().roll(&mut seq(dice)).unwrap().total
    }

    #[test]
    fn arithmetic() {
        assert_eq!(eval("5+6", &[]), 11.0);
        assert_eq!(eval("2+3*4", &[]), 14.0);
        assert_eq!(eval("(2+3)*4", &[]), 20.0);
        assert_eq!(eval("10/4", &[]), 2.5);
        assert_eq!(eval("2^3^2", &[]), 512.0); // right assoc
        assert_eq!(eval("2**3", &[]), 8.0);
        assert_eq!(eval("-3+5", &[]), 2.0);
        assert_eq!(eval("5--3", &[]), 8.0);
        assert_eq!(eval("5 plus 3 times 2", &[]), 11.0);
        assert_eq!(eval("9 divided by 3", &[]), 3.0);
        assert_eq!(eval("7 × 2 ÷ 7", &[]), 2.0);
    }

    #[test]
    fn basic_dice() {
        assert_eq!(eval("1d20+5", &[12]), 17.0);
        assert_eq!(eval("d20", &[7]), 7.0);
        assert_eq!(eval("D20", &[7]), 7.0);
        assert_eq!(eval("2d6+1d4+3", &[3, 4, 2]), 12.0);
        assert_eq!(eval("d%", &[55]), 55.0);
        assert_eq!(eval("(1+1)d6", &[2, 5]), 7.0);
        assert_eq!(eval("1d(2*3)", &[6]), 6.0);
        assert_eq!(eval("0d6", &[]), 0.0);
    }

    #[test]
    fn chained_dice() {
        // 2d3 -> 1+2 = 3 dice of d4 -> 4+4+4
        assert_eq!(eval("2d3d4", &[1, 2, 4, 4, 4]), 12.0);
    }

    #[test]
    fn keep_drop() {
        assert_eq!(eval("4d6kh3", &[1, 5, 3, 6]), 14.0);
        assert_eq!(eval("4d6dl1", &[1, 5, 3, 6]), 14.0);
        assert_eq!(eval("3d4dh1", &[4, 2, 3]), 5.0);
        assert_eq!(eval("3d4kl1", &[4, 2, 3]), 2.0);
        assert_eq!(eval("2d20kh1+5", &[3, 17]), 22.0);
        assert_eq!(eval("2d20kl1", &[3, 17]), 3.0);
        assert_eq!(eval("2d20kh", &[3, 17]), 17.0); // default 1
        assert_eq!(eval("2d6kh5", &[3, 4]), 7.0); // keep more than rolled
    }

    #[test]
    fn reroll_once() {
        // 2d4r1: [1,3] -> 1 rerolled to 1 again (once only) -> 1+3 = 4
        assert_eq!(eval("2d4r1", &[1, 3, 1]), 4.0);
        assert_eq!(eval("2d4r1", &[1, 3, 4]), 7.0);
        assert_eq!(eval("2d4r<2", &[1, 3, 2]), 5.0);
        assert_eq!(eval("2d4r<=2", &[2, 1, 4, 4]), 8.0);
        assert_eq!(eval("2d4r>2", &[3, 1, 2]), 3.0);
        assert_eq!(eval("2d4r>=3", &[3, 4, 1, 2]), 3.0);
    }

    #[test]
    fn explode() {
        // 2d4x4: [4,2] -> explode 4 -> [3] = 9
        assert_eq!(eval("2d4x4", &[4, 2, 3]), 9.0);
        // chain explosions
        assert_eq!(eval("1d4x4", &[4, 4, 4, 1]), 13.0);
        assert_eq!(eval("2d4x>=3", &[3, 1, 2]), 6.0);
        assert_eq!(eval("2d4x<2", &[1, 4, 3]), 8.0);
        // explosion cap prevents infinite loops
        let r = DiceExpr::parse("1d1x1").unwrap().roll(&mut seq(&[1])).unwrap();
        assert_eq!(r.total, (MAX_EXPLOSIONS + 1) as f64);
    }

    #[test]
    fn count_successes() {
        assert_eq!(eval("4d6cs>=5", &[5, 6, 2, 1]), 2.0);
        assert_eq!(eval("2d4cs=4", &[4, 4]), 2.0);
        assert_eq!(eval("2d4cs<2", &[1, 3]), 1.0);
        assert_eq!(eval("3d4cs<=2", &[1, 2, 3]), 2.0);
        assert_eq!(eval("2d4cs>2", &[3, 4]), 2.0);
    }

    #[test]
    fn margin_of_success() {
        assert_eq!(eval("2d4ms=4", &[3, 4]), 3.0);
        assert_eq!(eval("2d4ms<10", &[3, 4]), 3.0);
    }

    #[test]
    fn combined_modifiers() {
        // reroll 1s then keep highest 3
        assert_eq!(eval("4d6r1kh3", &[1, 2, 3, 4, 6]), 13.0);
    }

    #[test]
    fn pools() {
        assert_eq!(eval("{2d8, 1d6}", &[3, 4, 5]), 12.0);
        assert_eq!(eval("{1d20+7, 10}kh1", &[2]), 10.0);
        assert_eq!(eval("{1d20+7, 10}kh1", &[15]), 22.0);
        assert_eq!(eval("{1d20, 1d20}kh1", &[5, 18]), 18.0);
    }

    #[test]
    fn functions() {
        assert_eq!(eval("floor(1.5)", &[]), 1.0);
        assert_eq!(eval("ceil(1.5)", &[]), 2.0);
        assert_eq!(eval("round(1.5)", &[]), 2.0);
        assert_eq!(eval("round(-1.5)", &[]), -1.0); // JS semantics
        assert_eq!(eval("avg(8d6)", &[]), 28.0);
        assert_eq!(eval("dmax(8d6)", &[]), 48.0);
        assert_eq!(eval("dmin(8d6)", &[]), 8.0);
        assert_eq!(eval("abs(1d6-6)", &[2]), 4.0);
        assert_eq!(eval("sign(1d6-3)", &[1]), -1.0);
        assert_eq!(eval("max(1, 5, 3)", &[]), 5.0);
        assert_eq!(eval("min(1d20, 10)", &[15]), 10.0);
        assert_eq!(eval("pow(2, 10)", &[]), 1024.0);
        assert_eq!(eval("sqrt(16)+cbrt(27)", &[]), 7.0);
        assert_eq!(eval("trunc(-2.7)", &[]), -2.0);
    }

    #[test]
    fn stats_helpers() {
        let e = DiceExpr::parse("2d6+3").unwrap();
        assert_eq!(e.average().unwrap(), 10.0);
        assert_eq!(e.minimum().unwrap(), 5.0);
        assert_eq!(e.maximum().unwrap(), 15.0);
        assert!(!e.is_constant());
        assert!(DiceExpr::parse("5+6").unwrap().is_constant());
    }

    #[test]
    fn errors() {
        assert!(DiceExpr::parse("").is_err());
        assert!(DiceExpr::parse("1d").is_err());
        assert!(DiceExpr::parse("1d20+").is_err());
        assert!(DiceExpr::parse("1d20 banana").is_err());
        assert!(DiceExpr::parse("(1d20").is_err());
        assert!(DiceExpr::parse("2d4r").is_err());
        assert!(DiceExpr::parse("1d20$").is_err());
        let r = DiceExpr::parse("1d0").unwrap().roll(&mut seq(&[1]));
        assert!(r.is_err());
        let r = DiceExpr::parse("1.5d6").unwrap().roll(&mut seq(&[1]));
        assert!(r.is_err());
        let r = DiceExpr::parse("99999d6").unwrap().roll(&mut seq(&[1]));
        assert!(r.is_err());
        let r = DiceExpr::parse("5/0").unwrap().roll(&mut seq(&[1]));
        assert!(r.is_err());
    }

    #[test]
    fn breakdown_and_nat() {
        let r = DiceExpr::parse("2d20kh1+5").unwrap().roll(&mut seq(&[3, 20])).unwrap();
        assert_eq!(r.total, 25.0);
        assert!(r.nat20);
        assert!(!r.nat1); // the 1... wasn't rolled; the 3 was dropped anyway
        let text = r.breakdown();
        assert!(text.contains("[3]"), "{text}");
        assert!(text.contains("[20]"), "{text}");
        assert!(r.segments.iter().any(|s| s.kind == SegKind::Dropped && s.text == "[3]"));
        assert!(r.segments.iter().any(|s| s.kind == SegKind::DieMax));

        let r = DiceExpr::parse("2d20kh1").unwrap().roll(&mut seq(&[1, 15])).unwrap();
        assert!(!r.nat1, "dropped natural 1 shouldn't count");
    }

    #[test]
    fn commands() {
        assert_eq!(
            parse_command("Fireball: 8d6").unwrap(),
            Command::Roll {
                label: "Fireball".into(),
                expression: "8d6".into(),
                target: None
            }
        );
        assert_eq!(
            parse_command("Bite: 1d6+2 @Goblin Boss").unwrap(),
            Command::Roll {
                label: "Bite".into(),
                expression: "1d6+2".into(),
                target: Some("Goblin Boss".into())
            }
        );
        // comparison '=' is not a label separator
        assert_eq!(
            parse_command("2d4cs=4").unwrap(),
            Command::Roll {
                label: String::new(),
                expression: "2d4cs=4".into(),
                target: None
            }
        );
        assert_eq!(parse_command("/help").unwrap(), Command::Help);
        assert_eq!(parse_command("/clear").unwrap(), Command::Clear);
        assert_eq!(parse_command("/macro list").unwrap(), Command::MacroList);
        assert_eq!(
            parse_command("/macro add sneak 3d6").unwrap(),
            Command::MacroAdd {
                name: "sneak".into(),
                body: "3d6".into()
            }
        );
        assert_eq!(parse_command("/macro remove sneak").unwrap(), Command::MacroRemove { name: "sneak".into() });
        assert_eq!(
            parse_command("/iterroll 1d20+5 10 15").unwrap(),
            Command::IterRoll {
                expression: "1d20+5".into(),
                count: 10,
                target: Some(15.0)
            }
        );
        assert!(parse_command("/bogus").is_err());
        assert!(parse_command("   ").is_err());
    }

    #[test]
    fn macros() {
        let mut m = BTreeMap::new();
        m.insert("sneak".to_string(), "3d6".to_string());
        m.insert("atk".to_string(), "1d20+#bonus".to_string());
        m.insert("bonus".to_string(), "5".to_string());
        assert_eq!(expand_macros("1d8+#sneak", &m).unwrap(), "1d8+(3d6)");
        assert_eq!(expand_macros("#atk", &m).unwrap(), "(1d20+(5))");
        assert!(expand_macros("#nope", &m).is_err());
        let rec = roll_to_record("Sneak", "#sneak", "Rogue", &m, &mut seq(&[1, 2, 3])).unwrap();
        assert_eq!(rec.total, 6.0);
        assert_eq!(rec.label, "Sneak");
        // recursion guard
        let mut loop_m = BTreeMap::new();
        loop_m.insert("a".to_string(), "#a".to_string());
        assert!(expand_macros("#a", &loop_m).is_err());
    }

    #[test]
    fn real_rng_in_range() {
        let mut rng = fastrand::Rng::with_seed(42);
        let e = DiceExpr::parse("3d6").unwrap();
        for _ in 0..1000 {
            let t = e.roll(&mut rng).unwrap().total;
            assert!((3.0..=18.0).contains(&t));
        }
        // distribution sanity: d20 hits every face
        let d20 = DiceExpr::parse("d20").unwrap();
        let mut seen = [false; 21];
        for _ in 0..2000 {
            seen[d20.roll(&mut rng).unwrap().total as usize] = true;
        }
        assert!(seen[1..].iter().all(|s| *s));
    }

    #[test]
    fn fmt_numbers() {
        assert_eq!(fmt_num(3.0), "3");
        assert_eq!(fmt_num(2.5), "2.5");
        assert_eq!(fmt_num(1.0 / 3.0), "0.333");
        assert_eq!(fmt_num(-4.0), "-4");
    }
}
