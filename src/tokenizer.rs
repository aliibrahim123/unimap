use std::fmt::Display;

use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

use crate::utils::{Error, StrExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
	start: (u32, u32),
	end: (u32, u32),
}
impl Span {
	pub fn new(start: (u32, u32), end: (u32, u32)) -> Span {
		Span { start, end }
	}
	pub fn point(loc: (u32, u32)) -> Span {
		Span { start: loc, end: (loc.0, loc.1 + 1) }
	}
	pub fn is_point(&self) -> bool {
		self.start.0 == self.end.0 && self.start.1 + 1 == self.end.1
	}
}
impl Display for Span {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if self.is_point() {
			write!(f, "{}:{}", self.start.0, self.start.1)
		} else {
			write!(f, "{}:{}-{}:{}", self.start.0, self.start.1, self.end.0, self.end.1)
		}
	}
}

pub fn unexpected_token<T>(token: impl Display, span: Span, file: &str) -> Result<T, Error> {
	Err(Error::new(format!("parse error: unexpected token ({token})"), span, file))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token<'a> {
	pub kind: TokenKind<'a>,
	pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenKind<'a> {
	Ident(&'a str),
	Number(usize),
	Dot,
	Eq,
	Colon,
	Comma,
	Arrow,
	Pipe,
	SemiColon,
	ParenOpen,
	ParenClose,
	BraceOpen,
	BraceClose,
	BracketOpen,
	BracketClose,
	Let,
	Symbol,
	Fn,
	Import,
}

fn update_pos_after(cur_pos: &mut (u32, u32), raw: &str) {
	let mut last_ind = 0;
	while let Some(ind) = raw.find_after('\n', last_ind) {
		*cur_pos = (cur_pos.0 + 1, 1);
		last_ind = ind + 1;
	}
	cur_pos.1 += raw[last_ind..].chars().count() as u32;
}

fn match_symbol(src: &str, start_ind: usize) -> Option<(&str, u32)> {
	let mut char_count = 0;
	for (ind, cur_char) in src[start_ind..].char_indices() {
		use GeneralCategoryGroup as Group;
		let is_ident = match cur_char {
			'\0'..='\x20' | '\x7F' => false,
			'.' | ':' | ',' | ';' | '=' | '|' | '/' | '(' | ')' | '{' | '}' | '[' | ']' => false,
			'\x21'..='\x7E' => true,
			c => !matches!(c.general_category_group(), Group::Separator | Group::Other),
		};
		if !is_ident {
			return (char_count > 0).then(|| (&src[start_ind..start_ind + ind], char_count));
		}
		char_count += 1;
	}
	(char_count > 0).then(|| (&src[start_ind..], char_count))
}

fn parse_nb(src: &str, pos: (u32, u32), file: &str) -> Result<TokenKind<'static>, Error> {
	let bytes = src.as_bytes();
	for (ind, _) in bytes.iter().enumerate().filter(|(_, c)| **c == b'_') {
		if ind == 0 || bytes[ind - 1] == b'_' || bytes.get(ind + 1).is_none_or(|c| *c == b'_') {
			return unexpected_token('_', Span::point((pos.0, pos.1 + ind as u32)), file);
		}
	}
	Ok(TokenKind::Number(src.replace('_', "").parse().unwrap()))
}

pub fn tokenize<'a>(source: &'a str, file: &str) -> Result<Vec<Token<'a>>, Error> {
	let mut tokens = Vec::new();
	let mut cur_pos = (1, 1);
	let mut ind = 0;
	'main: while let Some(cur_char) = source.char_at(ind) {
		use TokenKind as Kind;
		let mut add_symbol = |token_ty: Kind<'static>, ind: &mut usize| {
			tokens.push(Token { kind: token_ty, span: Span::point(cur_pos) });
			cur_pos.1 += 1;
			*ind += 1;
		};
		match cur_char {
			' ' | '\t' | '\r' => {
				cur_pos.1 += 1;
				ind += 1
			}
			'\n' => {
				cur_pos = (cur_pos.0 + 1, 1);
				ind += 1
			}

			'.' => add_symbol(Kind::Dot, &mut ind),
			':' => add_symbol(Kind::Colon, &mut ind),
			',' => add_symbol(Kind::Comma, &mut ind),
			';' => add_symbol(Kind::SemiColon, &mut ind),
			'(' => add_symbol(Kind::ParenOpen, &mut ind),
			')' => add_symbol(Kind::ParenClose, &mut ind),
			'{' => add_symbol(Kind::BraceOpen, &mut ind),
			'}' => add_symbol(Kind::BraceClose, &mut ind),
			'[' => add_symbol(Kind::BracketOpen, &mut ind),
			']' => add_symbol(Kind::BracketClose, &mut ind),

			'|' => {
				if source.char_at(ind + 1) == Some('>') {
					tokens.push(Token {
						kind: Kind::Pipe,
						span: Span::new(cur_pos, (cur_pos.0, cur_pos.1 + 2)),
					});
					cur_pos.1 += 2;
					ind += 2;
				} else {
					return unexpected_token(cur_char, Span::point(cur_pos), file);
				}
			}
			'=' => {
				if source.char_at(ind + 1) == Some('>') {
					tokens.push(Token {
						kind: Kind::Arrow,
						span: Span::new(cur_pos, (cur_pos.0, cur_pos.1 + 2)),
					});
					cur_pos.1 += 2;
					ind += 2;
				} else {
					add_symbol(Kind::Eq, &mut ind);
				}
			}

			'/' => match source.char_at(ind + 1) {
				Some('/') => {
					ind = source.find_after('\n', ind + 2).unwrap_or(source.len());
				}
				Some('*') => {
					let Some(end) = source.find_after_str("*/", ind + 2) else {
						let msg = "parse error: unended comment".into();
						return Err(Error::new(msg, Span::point(cur_pos), file));
					};
					update_pos_after(&mut cur_pos, &source[ind..end]);
					ind = end + 2;
					cur_pos.1 += 2;
				}
				_ => return unexpected_token(cur_char, Span::point(cur_pos), file),
			},

			_ => {
				let Some((ident, char_count)) = match_symbol(source, ind) else {
					return unexpected_token(cur_char, Span::point(cur_pos), file);
				};
				let span = Span::new(cur_pos, (cur_pos.0, cur_pos.1 + char_count));
				let kind = match ident {
					"let" => Kind::Let,
					"fn" => Kind::Fn,
					"symbol" => Kind::Symbol,
					"import" => Kind::Import,
					_ if ident.chars().all(|c| c.is_ascii_digit() || c == '_') => {
						parse_nb(ident, cur_pos, file)?
					}
					_ => Kind::Ident(ident),
				};
				tokens.push(Token { kind, span });
				ind += ident.len();
				cur_pos.1 += char_count;
			}
		}
	}
	Ok(tokens)
}
