use std::fs::{canonicalize, read_to_string};

use clap::Parser;
use unimap::{Error, LoadResult, run};

#[derive(Parser, Debug, Clone, PartialEq, Eq)]
struct Args {
	#[arg(short, long)]
	base_dir: Option<String>,
	#[arg(short, long)]
	entry: String,
}

fn main() {
	if let Err(e) = mainer() {
		eprintln!("error: {}", e);
	}
}
fn mainer() -> Result<(), String> {
	let Args { base_dir, entry } = Args::parse();
	let (base_dir, mut entry) = match base_dir {
		Some(base_dir) => {
			let base = canonicalize(&base_dir).map_err(errors::read_dir(&base_dir))?;
			if !base.is_dir() {
				return Err(format!("base directory \"{}\" is not a directory", base.display()));
			}
			let entry = base.join(&entry);
			if !entry.starts_with(&base) {
				return Err(format!(
					"entry \"{}\" is not inside the base directory \"{}\"",
					entry.display(),
					base.display()
				));
			}
			(base, entry)
		}
		None => {
			let entry = canonicalize(&entry).map_err(errors::read_file(&entry))?;
			(entry.parent().unwrap().to_path_buf(), entry)
		}
	};

	entry.set_extension("");
	let root = entry.strip_prefix(&base_dir).unwrap();
	let segments = root.components().map(|v| v.as_os_str().to_str().unwrap());
	let root = unimap::Path::from_iter(segments);

	let loader = |path: &unimap::Path, importer: &str| {
		let mut path_full = base_dir.clone();
		path_full.extend(path.segments.iter().map(|v| &v.val));
		path_full.set_extension("unim");
		println!("requesting: {}", path_full.display());
		let Ok(file) = read_to_string(&path_full) else {
			let msg = format!(
				"import error: can not load \"{path}\", can not find \"{}\"",
				path_full.display()
			);
			return Err(Error::new(msg, path.span, importer));
		};
		Ok(LoadResult { file, path: path_full.to_str().unwrap().to_string() })
	};
	let ctx = run(&root, &loader).map_err(|err| err.to_string())?;
	println!("ctx: {ctx:#?}");
	Ok(())
}

mod errors {
	use std::fmt::Display;
	pub fn read_dir<T>(path: &impl Display) -> impl FnOnce(T) -> String {
		move |_| format!("unable to read directory \"{path}\"")
	}
	pub fn read_file<T>(path: &impl Display) -> impl FnOnce(T) -> String {
		move |_| format!("unable to read file \"{path}\"")
	}
}
