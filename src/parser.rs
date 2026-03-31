use compact_str::CompactString;

use crate::{
	tokenizer::{Span, Token, TokenKind, end_of_input, tokenize, unexpected_token},
	utils::Error,
};
use std::{
	cell::{Cell, RefCell},
	ops::Range,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
	pub name: CompactString,
	pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
	pub seqments: Vec<Ident>,
	pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldPat {
	Key(Ident, Pat),
	Index(Expr, Pat),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrItemPat {
	One(Pat),
	Rest(Pat),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaTokenKind {
	Any,
	Path(Path),
	Let(Ident, Box<Pat>),
	Object(Vec<FieldPat>),
	Array(Vec<ArrItemPat>),
	Or(Vec<Pat>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pat {
	pub kind: PaTokenKind,
	pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectItem {
	KeyValue(Expr, Expr),
	Rest(Expr),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayItem {
	One(Expr),
	Rest(Expr),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchArm {
	pub pat: Pat,
	pub map: Expr,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
	Cur,
	Ident(Ident),
	Call(Ident, Vec<Expr>),
	Object(Vec<ObjectItem>),
	Array(Vec<ArrayItem>),
	Field(Box<Expr>, Ident),
	Index(Box<Expr>, Box<Expr>),
	Map(Box<Expr>, Vec<MatchArm>),
	Pipe(Vec<Expr>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
	span: Span,
	kind: ExprKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
	pub path: Path,
	pub items: Vec<Ident>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Symbol {
	Ident(Ident),
	Enum { name: Ident, items: Vec<Ident> },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Const {
	pub name: Ident,
	pub expr: Expr,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fn {
	pub name: Ident,
	pub args: Vec<Ident>,
	pub expr: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct File {
	pub imports: Vec<Import>,
	pub symbols: Vec<Symbol>,
	pub consts: Vec<Const>,
	pub fns: Vec<Fn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Cursor<'a> {
	pub tokens: &'a [Token<'a>],
	pub ind: Cell<usize>,
	pub src_path: &'a str,
}
impl<'a> Cursor<'a> {
	pub fn new<'b>(tokens: &'b [Token<'b>], src_path: &'b str) -> Cursor<'b> {
		Cursor { tokens, ind: Cell::new(0), src_path }
	}
	pub fn ind(&self) -> usize {
		self.ind.get()
	}
	pub fn is_end(&self) -> bool {
		self.ind.get() >= self.tokens.len()
	}
	pub fn peek(&self) -> &Token<'a> {
		&self.tokens[self.ind.get()]
	}
	pub fn consume_any(&self) -> Option<&Token<'a>> {
		let ind = self.ind.get();
		self.ind.set(ind + 1);
		self.tokens.get(ind).filter(|t| t.kind != TokenKind::EOF)
	}
	pub fn consume(&self, kind: &TokenKind) -> Result<(), Error> {
		let token = self.peek();
		if &token.kind != kind {
			return unexpected_token(token, &format!("({kind})"), token.span, self.src_path);
		}
		self.skip();
		Ok(())
	}
	pub fn try_consume(&self, kind: &TokenKind<'a>) -> bool {
		if &self.peek().kind != kind {
			return false;
		}
		self.skip();
		true
	}
	pub fn consume_ident(&self) -> Result<Ident, Error> {
		match self.consume_any() {
			Some(Token { kind: TokenKind::Ident(ident), span }) => {
				Ok(Ident { name: CompactString::new(ident), span: *span })
			}
			Some(token) => unexpected_token(token, "an identifier", token.span, self.src_path),
			None => end_of_input("an identifier", self.src_path),
		}
	}
	pub fn last(&self) -> &Token<'a> {
		debug_assert!(self.ind.get() > 0);
		&self.tokens[self.ind.get() - 1]
	}
	/*pub fn try_consume_ident(&self) -> Option<Ident> {
		match self.peek() {
			Token { kind: TokenKind::Ident(ident), span } => {
				Some(Ident { name: CompactString::new(ident), span: *span })
			}
			_ => None,
		}
	}*/
	pub fn skip(&self) {
		self.ind.set(self.ind.get() + 1);
	}
}

use TokenKind::*;
fn parse_path<'a>(cur: &Cursor<'a>) -> Result<Path, Error> {
	let start_span = cur.peek().span;
	let mut path = vec![cur.consume_ident()?];
	while cur.try_consume(&TokenKind::Dot) {
		path.push(cur.consume_ident()?);
	}
	let span = start_span.join(path.last().unwrap().span);
	Ok(Path { seqments: path, span })
}
fn parse_delim_list<T>(
	cur: &Cursor, start: &TokenKind, end: &TokenKind, sep: &TokenKind,
	item_parser: impl std::ops::Fn(&Cursor) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
	let mut items = Vec::new();
	cur.consume(start)?;

	if &cur.peek().kind != end {
		items.push(item_parser(cur)?);
		while cur.try_consume(sep) && &cur.peek().kind != end {
			items.push(item_parser(cur)?);
		}
	}

	cur.consume(end)?;
	Ok(items)
}

fn parse_expr(cur: &Cursor) -> Result<Expr, Error> {
	unimplemented!()
}

fn parse_import(cur: &Cursor) -> Result<Import, Error> {
	let path = parse_path(cur)?;
	let items = parse_delim_list(cur, &BraceOpen, &BraceClose, &Comma, |cur| cur.consume_ident())?;
	cur.consume(&SemiColon)?;
	Ok(Import { path, items })
}
fn parse_symbol(cur: &Cursor, symbols: &mut Vec<Symbol>) -> Result<(), Error> {
	loop {
		let name = cur.consume_ident()?;
		if cur.peek().kind == Colon {
			let items = parse_delim_list(cur, &BraceOpen, &BraceClose, &Comma, |cur| {
				return cur.consume_ident();
			})?;
			symbols.push(Symbol::Enum { name, items });
		} else {
			symbols.push(Symbol::Ident(name));
		}

		if !cur.try_consume(&Comma) {
			break;
		}
	}
	cur.consume(&SemiColon)?;
	Ok(())
}
fn parse_const(cur: &Cursor) -> Result<Const, Error> {
	let name = cur.consume_ident()?;
	cur.consume(&Eq)?;
	let expr = parse_expr(cur)?;
	cur.consume(&SemiColon)?;
	Ok(Const { name, expr })
}
fn parse_fn(cur: &Cursor) -> Result<Fn, Error> {
	let name = cur.consume_ident()?;
	let args = parse_delim_list(cur, &ParenOpen, &ParenClose, &Comma, |cur| cur.consume_ident())?;
	cur.consume(&Arrow)?;
	let expr = parse_expr(cur)?;
	cur.consume(&SemiColon)?;
	Ok(Fn { name, args, expr })
}

pub fn parse_file(source: &str, src_path: &str) -> Result<File, Error> {
	let tokens = tokenize(source, src_path)?;
	let cur = Cursor::new(&tokens, src_path);
	let mut file = File::default();

	while let Some(cur_token) = cur.consume_any() {
		match cur_token.kind {
			Import => file.imports.push(parse_import(&cur)?),
			Symbol => parse_symbol(&cur, &mut file.symbols)?,
			Let => file.consts.push(parse_const(&cur)?),
			Fn => file.fns.push(parse_fn(&cur)?),
			_ => {
				let msg = "a top level decleration";
				return unexpected_token(cur_token, msg, cur_token.span, src_path);
			}
		}
	}
	Ok(file)
}
