use crate::{exec::exec, resolve::resolve};

mod exec;
pub mod parser;
pub mod resolve;
pub mod tokenizer;
pub mod utils;
mod value;

pub fn run(
	root_path: &Path, loader: Loader, debug_print: Print, pretty_output: bool,
) -> Result<String, Error> {
	let (execution, exec_mod) = resolve(root_path, loader)?;
	exec(execution, exec_mod, debug_print, pretty_output)
}
pub use {
	exec::Print,
	parser::Path,
	resolve::{LoadResult, Loader},
	utils::Error,
	value::Value,
};
