use compact_str::{CompactString, CompactStringExt};

use crate::{
	tokenizer::{Span, Token, TokenKind, end_of_input, tokenize, unexpected_token},
	utils::{Error, err},
};
use std::{cell::Cell, fmt::Display};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
	pub val: CompactString,
	pub span: Span,
}
impl Ident {
	pub fn into_expr(self) -> Expr {
		Expr { span: self.span, kind: ExprKind::Ident(self) }
	}
}
impl Display for Ident {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.val)
	}
}

/// `list<ident, '.'>` refering to an item
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
	pub segments: Vec<Ident>,
	pub span: Span,
}
impl Path {
	pub fn display(segments: &[Ident]) -> String {
		segments.iter().map(|v| &v.val).join_compact(".").to_string()
	}
	pub fn last(&self) -> &Ident {
		self.segments.last().unwrap()
	}
	/// Empty path
	pub fn root() -> Path {
		Path { segments: vec![], span: Span::none() }
	}
}
impl Display for Path {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", Path::display(&self.segments))
	}
}
impl<T: Into<CompactString>> FromIterator<T> for Path {
	fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
		let segments = iter.into_iter().map(|v| Ident { val: v.into(), span: Span::none() });
		Path { segments: segments.collect(), span: Span::none() }
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldPat {
	/// `field ':' pat`
	///
	/// there is also `"let" ident (':' pat)?` shorthand
	Key(FieldKind, Pat),
	/// `'[' expr ']' ':' pat`
	Index(Expr, Pat),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrItemPat {
	One(Pat),
	/// ".." pat
	Rest(Pat),
}
impl ArrItemPat {
	pub fn span(&self) -> Span {
		match self {
			ArrItemPat::One(pat) => pat.span,
			ArrItemPat::Rest(pat) => pat.span,
		}
	}
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatKind {
	/// `'_'`
	Any,
	/// local, constant or symbol
	Ident(Ident),
	Nb(u64),
	/// `ident '.' ident`
	Enum(Ident, Box<Ident>),
	/// `"let" ident ('=' pat)?`
	Let(Ident, Box<Pat>),
	/// `'{' list<field_pat, ','>? '}'`
	Object(Vec<FieldPat>),
	/// `'[' list<arr_item_pat, ','>? ']'`
	Array(Vec<ArrItemPat>),
	/// `list<pat, '|'>`
	Or(Vec<Pat>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pat {
	pub kind: PatKind,
	pub span: Span,
}
impl Pat {
	pub fn any(span: Span) -> Pat {
		Pat { kind: PatKind::Any, span }
	}
	pub fn as_ident(&self) -> Option<&Ident> {
		match &self.kind {
			PatKind::Ident(ident) => Some(ident),
			_ => None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectItem {
	/// `ident '=' expr`
	KeyValue(FieldKind, Expr),
	/// `'[' expr ']' '=' expr`
	IndexValue(Expr, Expr),
	/// `".." expr`
	Spread(Expr),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayItem {
	One(Expr),
	/// `".." expr`
	Spread(Expr),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapArm {
	pub pat: Pat,
	pub map: Expr,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
	Ident(Ident),
	Nb(u64),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
	/// `'_'`
	Cur,
	/// constant, local or symbol
	Ident(Ident),
	Nb(u64),
	/// `ident '(' list<expr, ','>? ')'`
	Call(Ident, Vec<Expr>),
	/// `'{' list<object_item, ','>? '}'`
	Object(Vec<ObjectItem>),
	/// `'[' list<expr, ','>? ']'`
	Array(Vec<ArrayItem>),
	/// `expr '.' field`
	Field(Box<Expr>, FieldKind),
	/// `expr '[' expr ']'`
	Index(Box<Expr>, Box<Expr>),
	/// `expr ':' '{' list<pat "=>" expr, ','> '}'`
	Map(Box<Expr>, Vec<MapArm>),
	/// `list<expr, "|>">`
	Pipe(Vec<Expr>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
	pub span: Span,
	pub kind: ExprKind,
}
impl Expr {
	pub fn as_ident(&self) -> Option<&Ident> {
		match &self.kind {
			ExprKind::Ident(ident) => Some(ident),
			_ => None,
		}
	}
}

/// `"import" path '{' list<ident, ','> '} ';''`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
	pub path: Path,
	pub items: Vec<Ident>,
}
/// `"symbol" list<symbol_kind, ','> ';'`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
	pub name: Ident,
	pub kind: SymbolKind,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
	/// `ident`
	Atom,
	/// `ident '{' list<ident, ','> '}'`
	Enum(Vec<Ident>),
}
/// `"let" ident = expr ';'`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Const {
	pub name: Ident,
	pub init: Expr,
}
/// `"fn" ident '(' list<ident, ','>? ')' "=>" expr ';'`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fn {
	pub name: Ident,
	pub args: Vec<Ident>,
	pub body: Expr,
}

/// `(import | symbol | const | fn)+`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct File {
	pub path: String,
	pub imports: Vec<Import>,
	pub symbols: Vec<Symbol>,
	pub consts: Vec<Const>,
	pub fns: Vec<Fn>,
}

/// smart tokens index
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
		self.ind.get() >= self.tokens.len() - 1
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
	/// a span from `start` till `self.last()`
	pub fn span_from(&self, start: Span) -> Span {
		start.join(self.last().span)
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
	/// like `try_eat` but returns a span
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
			return Some(Ident { val: CompactString::new(ident), span: *span });
		}
		None
	}
	pub fn consume_ident(&self) -> Result<Ident, Error> {
		let Some(ident) = self.try_consume_ident() else {
			return self.err_expected("identifier");
		};
		Ok(ident)
	}
	pub fn try_consume_nb(&self) -> Option<(u64, Span)> {
		if let Token { kind: TokenKind::Nb(nb), span } = self.peek() {
			self.skip();
			return Some((*nb, *span));
		}
		None
	}
	pub fn consume_any(&self) -> Option<&Token<'_>> {
		let token = self.peek();
		if token.kind == TokenKind::Eof {
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
	Ok(Path { segments: path, span })
}
/// parse `start list<!~end item, sep> sep? end`
fn parse_delim_list<T>(
	cur: &Cursor, start: TokenKind, end: TokenKind, sep: TokenKind,
	item_parser: impl std::ops::Fn(&Cursor) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
	let mut items = Vec::new();
	cur.consume(start)?;

	if !cur.test(end) {
		items.push(item_parser(cur)?);
		while cur.try_eat(sep) && !cur.test(end) {
			items.push(item_parser(cur)?);
		}
	}

	cur.consume(end)?;
	Ok(items)
}

fn parse_field(cur: &Cursor) -> Result<FieldKind, Error> {
	if let Some((nb, _)) = cur.try_consume_nb() {
		Ok(FieldKind::Nb(nb))
	} else {
		Ok(FieldKind::Ident(cur.consume_ident()?))
	}
}

fn parse_pat_obj(cur: &Cursor) -> Result<Pat, Error> {
	let start_span = cur.peek().span;
	let items = parse_delim_list(cur, BraceOpen, BraceClose, Comma, |cur| {
		// [index]: pat
		if cur.try_eat(BracketOpen) {
			let index = parse_expr(cur)?;
			cur.consume(BracketClose)?;
			cur.consume(Colon)?;
			let pat = parse_pat(cur)?;
			Ok(FieldPat::Index(index, pat))
		}
		// let shorthand
		else if let Some(let_span) = cur.try_consume(Let) {
			let field = cur.consume_ident()?;
			let pat = if cur.try_eat(Colon) {
				parse_pat(cur)?
			} else {
				// let ident shorthand
				Pat::any(let_span.join(field.span))
			};

			let let_pat = Pat {
				span: let_span.join(pat.span),
				kind: PatKind::Let(field.clone(), Box::new(pat)),
			};
			Ok(FieldPat::Key(FieldKind::Ident(field), let_pat))
		}
		// field: pat
		else {
			let field = parse_field(cur)?;
			cur.consume(Colon)?;
			let pat = parse_pat(cur)?;
			Ok(FieldPat::Key(field, pat))
		}
	})?;

	Ok(Pat { span: cur.span_from(start_span), kind: PatKind::Object(items) })
}
fn parse_pat_primary(cur: &Cursor) -> Result<Pat, Error> {
	// _
	if let Some(span) = cur.try_consume(Dash) {
		Ok(Pat { span, kind: PatKind::Any })
	}
	// nb
	else if let Some((nb, span)) = cur.try_consume_nb() {
		Ok(Pat { kind: PatKind::Nb(nb), span })
	}
	// ident / enum
	else if let Some(ident) = cur.try_consume_ident() {
		if cur.try_eat(Dot) {
			let var = cur.consume_ident()?;
			let span = ident.span.join(var.span);
			Ok(Pat { span, kind: PatKind::Enum(ident, Box::new(var)) })
		} else {
			Ok(Pat { span: ident.span, kind: PatKind::Ident(ident) })
		}
	}
	// let
	else if let Some(let_span) = cur.try_consume(Let) {
		let ident = cur.consume_ident()?;
		let pat = if cur.try_eat(Colon) {
			parse_pat(cur)?
		} else {
			// let ident shorthand
			Pat::any(let_span.join(ident.span))
		};
		let span = let_span.join(pat.span);
		Ok(Pat { span, kind: PatKind::Let(ident, Box::new(pat)) })
	}
	// object
	else if cur.test(BraceOpen) {
		parse_pat_obj(cur)
	}
	// array
	else if cur.test(BracketOpen) {
		let start_span = cur.peek().span;
		let items = parse_delim_list(cur, BracketOpen, BracketClose, Comma, |cur| {
			if let Some(rest_span) = cur.try_consume(Dot) {
				cur.consume(Dot)?;
				// `..` shorthand
				if cur.test(Comma) || cur.test(BracketClose) {
					Ok(ArrItemPat::Rest(Pat::any(cur.span_from(rest_span))))
				} else {
					Ok(ArrItemPat::Rest(parse_pat(cur)?))
				}
			} else {
				Ok(ArrItemPat::One(parse_pat(cur)?))
			}
		})?;
		Ok(Pat { span: cur.span_from(start_span), kind: PatKind::Array(items) })
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
		pats.push(parse_pat_primary(cur)?);
	}

	Ok(Pat { kind: PatKind::Or(pats), span: cur.span_from(start_span) })
}

fn parse_expr_obj(cur: &Cursor) -> Result<Expr, Error> {
	let start_span = cur.peek().span;
	let items = parse_delim_list(cur, BraceOpen, BraceClose, Comma, |cur| {
		// `..`
		if cur.try_eat(Dot) {
			cur.consume(Dot)?;
			Ok(ObjectItem::Spread(parse_expr(cur)?))
		}
		// [index] = value
		else if cur.try_eat(BracketOpen) {
			let index = parse_expr(cur)?;
			cur.consume(BracketClose)?;
			cur.consume(Eq)?;
			let value = parse_expr(cur)?;
			Ok(ObjectItem::IndexValue(index, value))
		}
		// field = value
		else {
			let field = parse_field(cur)?;
			cur.consume(Eq)?;
			let value = parse_expr(cur)?;
			Ok(ObjectItem::KeyValue(field, value))
		}
	})?;
	Ok(Expr { span: cur.span_from(start_span), kind: ExprKind::Object(items) })
}
fn parse_expr_primary(cur: &Cursor) -> Result<Expr, Error> {
	// _
	if let Some(span) = cur.try_consume(Dash) {
		Ok(Expr { span, kind: ExprKind::Cur })
	}
	// nb
	else if let Some((nb, span)) = cur.try_consume_nb() {
		Ok(Expr { span, kind: ExprKind::Nb(nb) })
	}
	// local, constant, symbol or call
	else if let Some(ident) = cur.try_consume_ident() {
		if cur.test(ParenOpen) {
			let args = parse_delim_list(cur, ParenOpen, ParenClose, Comma, parse_expr)?;
			Ok(Expr { span: cur.span_from(ident.span), kind: ExprKind::Call(ident, args) })
		} else {
			Ok(ident.into_expr())
		}
	}
	// (expr)
	else if cur.try_eat(ParenOpen) {
		let expr = parse_expr(cur)?;
		cur.consume(ParenClose)?;
		Ok(expr)
	}
	// object
	else if cur.test(BraceOpen) {
		parse_expr_obj(cur)
	}
	// array
	else if cur.test(BracketOpen) {
		let start_span = cur.peek().span;
		let items = parse_delim_list(cur, BracketOpen, BracketClose, Comma, |cur| {
			if cur.try_eat(Dot) {
				cur.consume(Dot)?;
				Ok(ArrayItem::Spread(parse_expr(cur)?))
			} else {
				Ok(ArrayItem::One(parse_expr(cur)?))
			}
		})?;
		Ok(Expr { span: cur.span_from(start_span), kind: ExprKind::Array(items) })
	} else {
		cur.err_expected("an expression")
	}
}
fn parse_expr_postfix(cur: &Cursor) -> Result<Expr, Error> {
	let mut expr = parse_expr_primary(cur)?;
	loop {
		let start_span = expr.span;
		let kind = 
		// expr.field
		if cur.try_eat(Dot) {
			ExprKind::Field(Box::new(expr), parse_field(cur)?)
		} 
		// expr[index]
		else if cur.try_eat(BracketOpen) {
			let index = parse_expr(cur)?;
			cur.consume(BracketClose)?;
			ExprKind::Index(Box::new(expr), Box::new(index))
		} 
		// expr: { pat => expr, .. }
		else if cur.try_eat(Colon) {
			let brace_start = cur.peek().span;
			let arms = parse_delim_list(cur, BraceOpen, BraceClose, Comma, |cur| {
				let pat = parse_pat(cur)?;
				cur.consume(Arrow)?;
				let map = parse_expr(cur)?;
				Ok(MapArm { pat, map })
			})?;
			if arms.is_empty() {
				return err!(
					"parse error: map expression must have at least 1 arm",
					(cur.span_from(brace_start), cur.src_path)
				);
			}
			ExprKind::Map(Box::new(expr), arms)
		} else {
			break;
		};
		expr = Expr { kind, span: cur.span_from(start_span) };
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
	Ok(Expr { kind: ExprKind::Pipe(exprs), span: cur.span_from(start_span) })
}

fn parse_import(cur: &Cursor) -> Result<Import, Error> {
	let path = parse_path(cur)?;
	let items = parse_delim_list(cur, BraceOpen, BraceClose, Comma, |cur| cur.consume_ident())?;
	cur.consume(SemiColon)?;
	Ok(Import { path, items })
}
fn parse_symbol(cur: &Cursor, symbols: &mut Vec<Symbol>) -> Result<(), Error> {
	loop {
		let name = cur.consume_ident()?;  
		if cur.test(BraceOpen) {
			let items = parse_delim_list(cur, BraceOpen, BraceClose, Comma, |cur| 
				cur.consume_ident()
			)?;
			symbols.push(Symbol { name, kind: SymbolKind::Enum(items) });
		} else {
			symbols.push(Symbol { name, kind: SymbolKind::Atom });
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
	Ok(Const { name, init: expr })
}
fn parse_fn(cur: &Cursor) -> Result<Fn, Error> {
	let name = cur.consume_ident()?;
	let args = parse_delim_list(cur, ParenOpen, ParenClose, Comma, |cur| cur.consume_ident())?;
	cur.consume(Arrow)?;
	let expr = parse_expr(cur)?;
	cur.consume(SemiColon)?;
	Ok(Fn { name, args, body: expr })
}

pub fn parse_file(source: &str, src_path: &str) -> Result<File, Error> {
	let tokens = tokenize(source, src_path)?;
	let cur = Cursor::new(&tokens, src_path);
	let mut file = File { path: src_path.to_string(), ..File::default() };

	while let Some(cur_token) = cur.consume_any() {
		match cur_token.kind {
			Import => file.imports.push(parse_import(&cur)?),
			Symbol => parse_symbol(&cur, &mut file.symbols)?,
			Let => file.consts.push(parse_const(&cur)?),
			Fn => file.fns.push(parse_fn(&cur)?),
			_ => {
				let expected = "a top level declaration";
				return unexpected_token(cur_token, expected, cur_token.span, src_path);
			}
		}
	}
	Ok(file)
}
