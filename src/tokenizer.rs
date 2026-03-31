use std::fmt::Display;

use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

use crate::utils::{Error, StrExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
	start: (u16, u16),
	end: (u16, u16),
}
impl Span {
	pub fn new(start: (u16, u16), end: (u16, u16)) -> Span {
		Span { start, end }
	}
	pub fn point(loc: (u16, u16)) -> Span {
		Span { start: loc, end: (loc.0, loc.1 + 1) }
	}
	pub fn none() -> Span {
		Span { start: (0, 0), end: (0, 0) }
	}
	pub fn is_point(&self) -> bool {
		self.start.0 == self.end.0 && self.start.1 + 1 == self.end.1
	}
	pub fn is_none(&self) -> bool {
		*self == Span::none()
	}
	pub fn join(self, other: Span) -> Span {
		Span { start: self.start, end: other.end }
	}
}
impl Display for Span {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if self.is_none() {
			Ok(())
		} else if self.is_point() {
			write!(f, "{}:{}", self.start.0, self.start.1)
		} else {
			write!(f, "{}:{}-{}:{}", self.start.0, self.start.1, self.end.0, self.end.1)
		}
	}
}

pub fn unexpected_token<T>(
	token: impl Display, expected: &str, span: Span, file: &str,
) -> Result<T, Error> {
	let msg = match expected {
		"" => format!("parse error: unexpected token ({token})"),
		_ => format!("parse error: unexpected token ({token}), expected {expected}"),
	};
	Err(Error::new(msg, span, file))
}
pub fn end_of_input<T>(expected: &str, file: &str) -> Result<T, Error> {
	let msg = match expected {
		"" => format!("parse error: unexpected end of input"),
		_ => format!("parse error: unexpected end of input, expected {expected}"),
	};
	Err(Error::new(msg, Span::none(), file))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
	pub kind: TokenKind<'a>,
	pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind<'a> {
	Ident(&'a str),
	Dot,
	Eq,
	Colon,
	Comma,
	Dash,
	Or,
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
	EOF,
}
impl Display for TokenKind<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Ident(ident) => f.write_str(ident),
			Self::Dot => f.write_str("."),
			Self::Eq => f.write_str("="),
			Self::Colon => f.write_str(":"),
			Self::Comma => f.write_str(","),
			Self::Dash => f.write_str("-"),
			Self::Or => f.write_str("|"),
			Self::Arrow => f.write_str("=>"),
			Self::Pipe => f.write_str("|>"),
			Self::SemiColon => f.write_str(";"),
			Self::ParenOpen => f.write_str("("),
			Self::ParenClose => f.write_str(")"),
			Self::BraceOpen => f.write_str("{"),
			Self::BraceClose => f.write_str("}"),
			Self::BracketOpen => f.write_str("["),
			Self::BracketClose => f.write_str("]"),
			Self::Let => f.write_str("let"),
			Self::Symbol => f.write_str("symbol"),
			Self::Fn => f.write_str("fn"),
			Self::Import => f.write_str("import"),
			Self::EOF => f.write_str("EOF"),
		}
	}
}
impl Display for Token<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.kind.fmt(f)
	}
}

fn update_pos_after(cur_pos: &mut (u16, u16), raw: &str) {
	let mut last_ind = 0;
	while let Some(ind) = raw.find_after('\n', last_ind) {
		*cur_pos = (cur_pos.0 + 1, 1);
		last_ind = ind + 1;
	}
	cur_pos.1 += raw[last_ind..].chars().count() as u16;
}

fn match_symbol(src: &str, start_ind: usize) -> Option<(&str, u16)> {
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

pub fn tokenize<'a>(source: &'a str, file: &str) -> Result<Vec<Token<'a>>, Error> {
	let mut tokens = Vec::new();
	let mut cur_pos = (1, 1);
	let mut ind = 0;
	use TokenKind as Kind;
	while let Some(cur_char) = source.char_at(ind) {
		macro_rules! single {
			($token:expr) => {{
				tokens.push(Token { kind: $token, span: Span::point(cur_pos) });
				cur_pos.1 += 1;
				ind += 1;
			}};
		}
		macro_rules! single_or_couple {
			($sec_char:expr, $single:expr, $couple:expr) => {
				if source.char_at(ind + 1) == Some($sec_char) {
					tokens.push(Token {
						kind: $couple,
						span: Span::new(cur_pos, (cur_pos.0, cur_pos.1 + 2)),
					});
					cur_pos.1 += 2;
					ind += 2;
				} else {
					single!($single);
				}
			};
		}

		match cur_char {
			' ' | '\t' | '\r' => {
				cur_pos.1 += 1;
				ind += 1
			}
			'\n' => {
				cur_pos = (cur_pos.0 + 1, 1);
				ind += 1
			}

			'.' => single!(Kind::Dot),
			':' => single!(Kind::Colon),
			',' => single!(Kind::Comma),
			';' => single!(Kind::SemiColon),
			'(' => single!(Kind::ParenOpen),
			')' => single!(Kind::ParenClose),
			'{' => single!(Kind::BraceOpen),
			'}' => single!(Kind::BraceClose),
			'[' => single!(Kind::BracketOpen),
			']' => single!(Kind::BracketClose),

			'|' => single_or_couple!('>', Kind::Or, Kind::Pipe),
			'=' => single_or_couple!('>', Kind::Eq, Kind::Arrow),

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
				_ => return unexpected_token(cur_char, "", Span::point(cur_pos), file),
			},

			_ => {
				let Some((ident, char_count)) = match_symbol(source, ind) else {
					return unexpected_token(cur_char, "", Span::point(cur_pos), file);
				};
				let span = Span::new(cur_pos, (cur_pos.0, cur_pos.1 + char_count));
				let kind = match ident {
					"_" => Kind::Dash,
					"let" => Kind::Let,
					"fn" => Kind::Fn,
					"symbol" => Kind::Symbol,
					"import" => Kind::Import,
					_ if ident.chars().all(|c| c.is_ascii_digit()) => Kind::Ident(ident),
					_ => Kind::Ident(ident),
				};
				tokens.push(Token { kind, span });
				ind += ident.len();
				cur_pos.1 += char_count;
			}
		};
	}
	tokens.push(Token { kind: Kind::EOF, span: Span::none() });
	Ok(tokens)
}
