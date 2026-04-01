use compact_str::CompactString;

use crate::{
	tokenizer::{Span, Token, TokenKind, end_of_input, tokenize, unexpected_token},
	utils::Error,
};
use std::cell::Cell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
	pub name: CompactString,
	pub span: Span,
}
impl Ident {
	pub fn into_expr(self) -> Expr {
		Expr { span: self.span, kind: ExprKind::Ident(self) }
	}
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
pub enum PatKind {
	Any,
	Path(Path),
	Let(Ident, Box<Pat>),
	Object(Vec<FieldPat>),
	Array(Vec<ArrItemPat>),
	Or(Vec<Pat>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pat {
	pub kind: PatKind,
	pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectItem {
	KeyValue(Ident, Expr),
	IndexValue(Expr, Expr),
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
	pub fn is_end(&self) -> bool {
		self.ind.get() >= self.tokens.len()
	}
	pub fn back(&self) {
		debug_assert!(self.ind.get() > 0);
		self.ind.set(self.ind.get() - 1);
	}
	pub fn last(&self) -> &Token<'a> {
		debug_assert!(self.ind.get() > 0);
		&self.tokens[self.ind.get() - 1]
	}
	pub fn peek(&self) -> &Token<'a> {
		&self.tokens[self.ind.get()]
	}
	pub fn skip(&self) {
		self.ind.set(self.ind.get() + 1);
	}
	pub fn test(&self, kind: TokenKind) -> bool {
		self.peek().kind == kind
	}
	pub fn consume(&self, kind: TokenKind) -> Result<Span, Error> {
		if self.is_end() {
			return end_of_input(&format!("({kind})"), self.src_path);
		}
		let token = self.peek();
		if token.kind != kind {
			return unexpected_token(token, &format!("({kind})"), token.span, self.src_path);
		}
		self.skip();
		Ok(token.span)
	}
	pub fn try_eat(&self, kind: TokenKind<'a>) -> bool {
		if self.peek().kind != kind {
			return false;
		}
		self.skip();
		true
	}
	pub fn try_consume(&self, kind: TokenKind) -> Option<Span> {
		let token = self.peek();
		(token.kind == kind).then(|| {
			self.skip();
			token.span
		})
	}
	pub fn try_consume_ident(&self) -> Option<Ident> {
		if let Token { kind: TokenKind::Ident(ident), span } = self.peek() {
			self.skip();
			return Some(Ident { name: CompactString::new(ident), span: *span });
		}
		None
	}
	pub fn consume_ident(&self) -> Result<Ident, Error> {
		let Some(ident) = self.try_consume_ident() else {
			return self.err_expected("identifier");
		};
		Ok(ident)
	}
	pub fn consume_any(&self) -> Option<&Token<'_>> {
		let token = self.peek();
		if token.kind == TokenKind::EOF {
			return None;
		}
		self.skip();
		Some(token)
	}
	pub fn err_expected<T>(&self, expected: &str) -> Result<T, Error> {
		if self.is_end() {
			return end_of_input(expected, self.src_path);
		}
		unexpected_token(self.peek(), expected, self.peek().span, self.src_path)
	}
}

use TokenKind::*;
fn parse_path<'a>(cur: &Cursor<'a>) -> Result<Path, Error> {
	let start_span = cur.peek().span;
	let mut path = vec![cur.consume_ident()?];
	while cur.try_eat(TokenKind::Dot) {
		path.push(cur.consume_ident()?);
	}
	let span = start_span.join(path.last().unwrap().span);
	Ok(Path { seqments: path, span })
}
fn parse_delim_list<T>(
	cur: &Cursor, start: TokenKind, end: TokenKind, sep: TokenKind,
	item_parser: impl std::ops::Fn(&Cursor) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
	let mut items = Vec::new();
	cur.consume(start)?;

	if !cur.test(end.clone()) {
		items.push(item_parser(cur)?);
		while cur.try_eat(sep.clone()) && !cur.test(end.clone()) {
			items.push(item_parser(cur)?);
		}
	}

	cur.consume(end)?;
	Ok(items)
}

fn parse_pat_obj(cur: &Cursor) -> Result<Pat, Error> {
	let start_span = cur.peek().span;
	let items = parse_delim_list(cur, BraceOpen, BraceClose, Comma, |cur| {
		if cur.try_eat(BracketOpen) {
			let index = parse_expr(cur)?;
			cur.consume(BracketClose)?;
			cur.consume(Colon)?;
			let pat = parse_pat(cur)?;
			Ok(FieldPat::Index(index, pat))
		} else if let Some(let_span) = cur.try_consume(Let) {
			let field = cur.consume_ident()?;
			let pat = if cur.try_eat(Colon) {
				parse_pat(cur)?
			} else {
				Pat { span: let_span.join(field.span), kind: PatKind::Any }
			};
			let let_pat = Pat {
				span: let_span.join(pat.span),
				kind: PatKind::Let(field.clone(), Box::new(pat)),
			};
			Ok(FieldPat::Key(field, let_pat))
		} else {
			let field = cur.consume_ident()?;
			cur.consume(Colon)?;
			let pat = parse_pat(cur)?;
			Ok(FieldPat::Key(field, pat))
		}
	})?;
	let span = start_span.join(cur.last().span);
	Ok(Pat { span, kind: PatKind::Object(items) })
}
fn parse_pat_primary(cur: &Cursor) -> Result<Pat, Error> {
	if let Some(span) = cur.try_consume(Dash) {
		Ok(Pat { span, kind: PatKind::Any })
	} else if matches!(cur.peek().kind, Ident(_)) {
		let path = parse_path(cur)?;
		Ok(Pat { span: path.span, kind: PatKind::Path(path) })
	} else if let Some(let_span) = cur.try_consume(Let) {
		let ident = cur.consume_ident()?;
		let pat = if cur.try_eat(Colon) {
			parse_pat(cur)?
		} else {
			Pat { span: let_span.join(ident.span), kind: PatKind::Any }
		};
		let span = let_span.join(pat.span);
		Ok(Pat { span, kind: PatKind::Let(ident, Box::new(pat)) })
	} else if cur.test(BraceOpen) {
		parse_pat_obj(cur)
	} else if cur.test(BracketOpen) {
		let start_span = cur.peek().span;
		let items = parse_delim_list(cur, BracketOpen, BracketClose, Comma, |cur| {
			if let Some(rest_span) = cur.try_consume(Dot) {
				cur.consume(Dot)?;
				if cur.test(Comma) || cur.test(BracketClose) {
					let any = Pat { span: rest_span.join(cur.last().span), kind: PatKind::Any };
					return Ok(ArrItemPat::Rest(any));
				}
				Ok(ArrItemPat::Rest(parse_pat(cur)?))
			} else {
				Ok(ArrItemPat::One(parse_pat(cur)?))
			}
		})?;
		let span = start_span.join(cur.last().span);
		Ok(Pat { span, kind: PatKind::Array(items) })
	} else {
		cur.err_expected("a pattern")
	}
}
fn parse_pat(cur: &Cursor) -> Result<Pat, Error> {
	let start_span = cur.peek().span;
	let pat = parse_pat_primary(cur)?;
	if !cur.test(Or) {
		return Ok(pat);
	}
	let mut pats = vec![pat];
	while cur.try_eat(Or) {
		pats.push(parse_pat(cur)?);
	}
	let span = start_span.join(cur.last().span);
	Ok(Pat { kind: PatKind::Or(pats), span })
}

fn parse_expr_obj(cur: &Cursor) -> Result<Expr, Error> {
	cur.back();
	let start_span = cur.peek().span;
	let items = parse_delim_list(cur, BraceOpen, BraceClose, Comma, |cur| {
		if cur.try_eat(Dot) {
			cur.consume(Dot)?;
			return Ok(ObjectItem::Rest(parse_expr(cur)?));
		}
		if cur.try_eat(BracketOpen) {
			let index = parse_expr(cur)?;
			cur.consume(BracketClose)?;
			cur.consume(Eq)?;
			let value = parse_expr(cur)?;
			Ok(ObjectItem::IndexValue(index, value))
		} else {
			let field = cur.consume_ident()?;
			let value = if cur.test(Comma) || cur.test(BraceClose) {
				field.clone().into_expr()
			} else {
				cur.consume(Eq)?;
				parse_expr(cur)?
			};
			Ok(ObjectItem::KeyValue(field, value))
		}
	})?;
	let span = start_span.join(cur.last().span);
	Ok(Expr { span, kind: ExprKind::Object(items) })
}
fn parse_expr_primary(cur: &Cursor) -> Result<Expr, Error> {
	if let Some(span) = cur.try_consume(Dash) {
		Ok(Expr { span, kind: ExprKind::Cur })
	} else if let Some(ident) = cur.try_consume_ident() {
		if cur.test(ParenOpen) {
			let args = parse_delim_list(cur, ParenOpen, ParenClose, Comma, |cur| {
				return parse_expr(cur);
			})?;
			let span = ident.span.join(cur.last().span);
			Ok(Expr { span, kind: ExprKind::Call(ident, args) })
		} else {
			Ok(ident.into_expr())
		}
	} else if cur.test(BraceOpen) {
		parse_expr_obj(cur)
	} else if cur.test(BracketOpen) {
		let start_span = cur.peek().span;
		let items = parse_delim_list(cur, BracketOpen, BracketClose, Comma, |cur| {
			if cur.try_eat(Dot) {
				cur.consume(Dot)?;
				Ok(ArrayItem::Rest(parse_expr(cur)?))
			} else {
				Ok(ArrayItem::One(parse_expr(cur)?))
			}
		})?;
		let span = start_span.join(cur.last().span);
		Ok(Expr { span, kind: ExprKind::Array(items) })
	} else {
		cur.err_expected("an expression")
	}
}
fn parse_expr_postfix(cur: &Cursor) -> Result<Expr, Error> {
	let mut expr = parse_expr_primary(cur)?;
	loop {
		let start_span = expr.span;
		let kind = if cur.try_eat(Dot) {
			ExprKind::Field(Box::new(expr), cur.consume_ident()?)
		} else if cur.try_eat(BracketOpen) {
			let index = parse_expr(cur)?;
			cur.consume(BracketClose)?;
			ExprKind::Index(Box::new(expr), Box::new(index))
		} else if cur.try_eat(Colon) {
			let arms = parse_delim_list(cur, BraceOpen, BraceClose, Comma, |cur| {
				let pat = parse_pat(cur)?;
				cur.consume(Arrow)?;
				let map = parse_expr(cur)?;
				Ok(MatchArm { pat, map })
			})?;
			ExprKind::Map(Box::new(expr), arms)
		} else {
			break;
		};
		expr = Expr { kind, span: start_span.join(cur.last().span) };
	}
	Ok(expr)
}
fn parse_expr(cur: &Cursor) -> Result<Expr, Error> {
	let start_span = cur.peek().span;
	let expr = parse_expr_postfix(cur)?;
	if !cur.test(Pipe) {
		return Ok(expr);
	}
	let mut exprs = vec![expr];
	while cur.try_eat(Pipe) {
		exprs.push(parse_expr_postfix(cur)?);
	}
	let span = start_span.join(cur.last().span);
	Ok(Expr { kind: ExprKind::Pipe(exprs), span })
}

fn parse_import(cur: &Cursor) -> Result<Import, Error> {
	let path = parse_path(cur)?;
	let items = parse_delim_list(cur, BraceOpen, BraceClose, Comma, |cur| {
		return cur.consume_ident();
	})?;
	cur.consume(SemiColon)?;
	Ok(Import { path, items })
}
fn parse_symbol(cur: &Cursor, symbols: &mut Vec<Symbol>) -> Result<(), Error> {
	loop {
		let name = cur.consume_ident()?;
		if cur.test(Colon) {
			let items = parse_delim_list(cur, BraceOpen, BraceClose, Comma, |cur| {
				return cur.consume_ident();
			})?;
			symbols.push(Symbol::Enum { name, items });
		} else {
			symbols.push(Symbol::Ident(name));
		}

		if !cur.try_eat(Comma) {
			break;
		}
	}
	cur.consume(SemiColon)?;
	Ok(())
}
fn parse_const(cur: &Cursor) -> Result<Const, Error> {
	let name = cur.consume_ident()?;
	cur.consume(Eq)?;
	let expr = parse_expr(cur)?;
	cur.consume(SemiColon)?;
	Ok(Const { name, expr })
}
fn parse_fn(cur: &Cursor) -> Result<Fn, Error> {
	let name = cur.consume_ident()?;
	let args = parse_delim_list(cur, ParenOpen, ParenClose, Comma, |cur| cur.consume_ident())?;
	cur.consume(Arrow)?;
	let expr = parse_expr(cur)?;
	cur.consume(SemiColon)?;
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
