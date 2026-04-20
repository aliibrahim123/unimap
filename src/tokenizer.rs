use std::fmt::{Debug, Display};

use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

use crate::utils::{Error, StrExt, err};

/// a line:column range inside the source
///
/// `start` and `end` are (line, column)
///
/// char counted starts from 1, `\n` is line, end is inclusive
// has the possibility to panic if the file is big, but will not increase the size of all structs just to fix it, please keep your files small.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
	pub start: (u16, u16),
	pub end: (u16, u16),
}
impl Span {
	pub fn new(start: (u16, u16), end: (u16, u16)) -> Span {
		Span { start, end }
	}
	/// span one char
	pub fn point(loc: (u16, u16)) -> Span {
		Span { start: loc, end: (loc.0, loc.1) }
	}
	pub fn none() -> Span {
		Span { start: (0, 0), end: (0, 0) }
	}
	pub fn is_point(&self) -> bool {
		self.start.0 == self.end.0 && self.start.1 == self.end.1
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
impl Debug for Span {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if self.is_none() { write!(f, "Span::none") } else { write!(f, "Span({self})") }
	}
}

/// return a parse error of
pub fn unexpected_token<T>(
	token: impl Display, expected: &str, span: Span, file: &str,
) -> Result<T, Error> {
	match expected {
		"" => err!("parse error: unexpected token ({token})", (span, file)),
		_ => err!("parse error: unexpected token ({token}), expected {expected}", (span, file)),
	}
}
/// return a parse error of
pub fn end_of_input<T>(expected: &str, file: &str) -> Result<T, Error> {
	match expected {
		"" => err!("parse error: unexpected end of input", (Span::none(), file)),
		_ => {
			err!("parse error: unexpected end of input, expected {expected}", (Span::none(), file))
		}
	}
}

/// a spanned source unit
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
	pub kind: TokenKind<'a>,
	pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum TokenKind<'a> {
	Ident(&'a str),
	Nb(u64),
	/// `.`
	Dot,
	/// `=`
	Eq,
	/// `:`
	Colon,
	/// `,`
	Comma,
	/// `_`
	Dash,
	/// `|`
	Or,
	/// `=>`
	Arrow,
	/// `|>`
	Pipe,
	/// `;`
	SemiColon,
	/// `(`
	ParenOpen,
	/// `)`
	ParenClose,
	/// `[`
	BraceOpen,
	/// `]`
	BraceClose,
	/// `{`
	BracketOpen,
	/// `}`
	BracketClose,
	Let,
	Symbol,
	Fn,
	Import,
	Eof,
}
impl Display for TokenKind<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Ident(ident) => f.write_str(ident),
			Self::Nb(nb) => write!(f, "{nb}"),
			Self::Dot => f.write_str("."),
			Self::Eq => f.write_str("="),
			Self::Colon => f.write_str(":"),
			Self::Comma => f.write_str(","),
			Self::Dash => f.write_str("_"),
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
			Self::Eof => f.write_str("EOF"),
		}
	}
}
/// implemented for `unexpected_token` usage
impl Display for Token<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self.kind, f)
	}
}

/// update the char (line, column) position inside raw content (comment)
fn update_pos_after(cur_pos: &mut (u16, u16), raw: &str) {
	let mut last_ind = 0;
	while let Some(ind) = raw.find_after('\n', last_ind) {
		*cur_pos = (cur_pos.0 + 1, 1);
		last_ind = ind + 1;
	}
	cur_pos.1 +=
		raw[last_ind..].chars().map(|c| if c == '\t' { 4 } else { 1 }).sum::<usize>() as u16;
}

fn try_match_ident(src: &str, start_ind: usize) -> Option<(&str, u16)> {
	let mut char_count = 0;
	for (ind, cur_char) in src[start_ind..].char_indices() {
		use GeneralCategoryGroup as Group;
		// ident is every unicode char not control or separator, and not used as punctuation by the language
		let is_ident = match cur_char {
			// ascii control
			'\0'..='\x20' | '\x7F' => false,
			'.' | ':' | ',' | ';' | '=' | '|' | '/' | '(' | ')' | '{' | '}' | '[' | ']' => false,
			// ascii printable
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

/// tokenize and simplify the source
pub fn tokenize<'a>(source: &'a str, src_path: &str) -> Result<Vec<Token<'a>>, Error> {
	let mut tokens = Vec::new();
	let mut cur_pos = (1, 1);
	let mut ind = 0;
	use TokenKind as Kind;
	while let Some(cur_char) = source.char_at(ind) {
		macro_rules! push_single {
			($token:expr) => {{
				tokens.push(Token { kind: $token, span: Span::point(cur_pos) });
				cur_pos.1 += 1;
				ind += 1;
			}};
		}
		/// some chars like `=` can be `=` or `=>` tokens
		macro_rules! push_single_or_couple {
			($sec_char:expr, $single:expr, $couple:expr) => {
				if source.char_at(ind + 1) == Some($sec_char) {
					tokens.push(Token {
						kind: $couple,
						span: Span::new(cur_pos, (cur_pos.0, cur_pos.1 + 1)),
					});
					cur_pos.1 += 2;
					ind += 2;
				} else {
					push_single!($single);
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

			'.' => push_single!(Kind::Dot),
			':' => push_single!(Kind::Colon),
			',' => push_single!(Kind::Comma),
			';' => push_single!(Kind::SemiColon),
			'(' => push_single!(Kind::ParenOpen),
			')' => push_single!(Kind::ParenClose),
			'{' => push_single!(Kind::BraceOpen),
			'}' => push_single!(Kind::BraceClose),
			'[' => push_single!(Kind::BracketOpen),
			']' => push_single!(Kind::BracketClose),

			'|' => push_single_or_couple!('>', Kind::Or, Kind::Pipe),
			'=' => push_single_or_couple!('>', Kind::Eq, Kind::Arrow),

			'/' => match source.char_at(ind + 1) {
				Some('/') => {
					ind = source.find_after('\n', ind + 2).unwrap_or(source.len());
					cur_pos = (cur_pos.0 + 1, 1);
				}
				Some('*') => {
					let Some(end) = source.find_after_str("*/", ind + 2) else {
						return end_of_input("*/", src_path);
					};
					update_pos_after(&mut cur_pos, &source[ind..end]);
					ind = end + 2;
					cur_pos.1 += 2;
				}
				_ => return unexpected_token(cur_char, "", Span::point(cur_pos), src_path),
			},

			_ => {
				let Some((ident, char_count)) = try_match_ident(source, ind) else {
					return unexpected_token(cur_char, "", Span::point(cur_pos), src_path);
				};
				let span = Span::new(cur_pos, (cur_pos.0, cur_pos.1 + char_count - 1));
				let kind = match ident {
					"_" => Kind::Dash,
					"let" => Kind::Let,
					"fn" => Kind::Fn,
					"symbol" => Kind::Symbol,
					"import" => Kind::Import,
					_ if ident.starts_with(|c| matches!(c, '1'..='9'))
						&& ident.chars().all(|c| c.is_ascii_digit())
						|| ident == "0" =>
					{
						let Ok(nb) = ident.parse::<u64>() else {
							return err!(
								"parse error: number ({ident}) is so large.",
								(span, src_path)
							);
						};
						Kind::Nb(nb)
					}
					_ => Kind::Ident(ident),
				};
				tokens.push(Token { kind, span });
				ind += ident.len();
				cur_pos.1 += char_count;
			}
		};
	}
	tokens.push(Token { kind: Kind::Eof, span: Span::none() });
	Ok(tokens)
}
