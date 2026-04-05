use std::fmt::{Display, Write};

use crate::tokenizer::Span;

pub trait StrExt {
	fn char_at(&self, index: usize) -> Option<char>;
	fn find_after(&self, pat: char, index: usize) -> Option<usize>;
	fn find_after_str(&self, pat: &str, index: usize) -> Option<usize>;
}
impl StrExt for str {
	fn char_at(&self, index: usize) -> Option<char> {
		self[index..].chars().next()
	}
	fn find_after(&self, pat: char, index: usize) -> Option<usize> {
		self[index..].find(pat).map(|i| i + index)
	}
	fn find_after_str(&self, pat: &str, index: usize) -> Option<usize> {
		self[index..].find(pat).map(|i| i + index)
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
	pub msg: Box<str>,
	pub span: Span,
}
impl Error {
	pub fn new(mut msg: String, span: Span, at: &str) -> Self {
		write!(msg, "\n  --> {at}").unwrap();

		if !span.is_none() {
			write!(msg, ":{span}").unwrap()
		}
		Self { msg: msg.into_boxed_str(), span }
	}
}
impl Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.msg)
	}
}
macro_rules! err {
	($msg:expr, ($span:expr, $at:expr)) => {
		Err(crate::utils::Error::new(format!($msg), $span, $at))
	};
}
pub(crate) use err;
