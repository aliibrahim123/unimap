use crate::resolve::resolve;

mod containers;
mod exec;
pub mod parser;
pub mod resolve;
pub mod tokenizer;
pub mod utils;
mod value;

pub fn run(root_path: &Path, loader: Loader) -> Result<impl std::fmt::Debug, Error> {
	resolve(root_path, loader)
}
pub use {
	parser::Path,
	resolve::{LoadResult, Loader},
	utils::Error,
};
