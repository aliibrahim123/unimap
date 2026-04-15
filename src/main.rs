use std::{
	fmt::{Debug, Display},
	fs::{File, canonicalize, create_dir_all, read_to_string, write},
	io::{Write, stdout},
	path::absolute,
};

use clap::{Parser, ValueEnum};
use unimap::{Error, LoadResult, Print, run};

/// how to print `dbg` expressions
#[derive(ValueEnum, Debug, Clone, PartialEq, Eq)]
enum DebugPrint {
	Silent,
	Stdout,
	File,
}
impl Display for DebugPrint {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			DebugPrint::Silent => "silent",
			DebugPrint::Stdout => "stdout",
			DebugPrint::File => "file",
		})
	}
}

/// run a unimap source file
#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[command(author, version, about = "a tool to run unimap files", long_about = None)]
struct Args {
	/// the entry file to process
	#[arg(value_name = "FILE")]
	entry: String,
	/// the imports root directory (default is the entry directory)
	#[arg(short, long, value_name = "DIR")]
	base_dir: Option<String>,
	/// where to print `dbg` expressions
	#[arg(short, long, default_value_t = DebugPrint::Stdout)]
	debug_print: DebugPrint,
	/// pretty print debug output
	#[arg(long, default_value_t = false)]
	debug_pretty: bool,
	/// the debug output file if `debug_print` is `file`
	#[arg(long, requires = "debug_print")]
	debug_file: Option<String>,
	/// output file path (default to stdout)
	#[arg(short, long)]
	outfile: Option<String>,
	/// disable pretty printing of output
	#[arg(long)]
	no_pretty: bool,
}

fn main() {
	if let Err(err) = mainer() {
		eprintln!("{err}");
	}
}
fn mainer() -> Result<(), String> {
	let Args { base_dir, entry, debug_file, debug_pretty, debug_print, outfile, no_pretty } =
		Args::parse();

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

			let entry = canonicalize(&entry).map_err(errors::read_file(&entry.display()))?;
			(base, entry)
		}
		None => {
			let entry = canonicalize(&entry).map_err(errors::read_file(&entry))?;
			(entry.parent().unwrap().to_path_buf(), entry)
		}
	};

	let mut file = None;
	_ = file;
	let debug_output: Option<&mut dyn std::io::Write> = match debug_print {
		DebugPrint::File => {
			let Some(debug_file) = debug_file else {
				return Err("debug output file not specified".to_string());
			};
			let debug_path = absolute(&debug_file).map_err(errors::create_file(&debug_file))?;

			create_dir_all(debug_path.parent().unwrap())
				.map_err(errors::create_file(&debug_file))?;

			file = Some(File::create(&debug_file).map_err(errors::create_file(&debug_file))?);
			Some(&mut file.unwrap())
		}
		DebugPrint::Stdout => Some(&mut stdout()),
		DebugPrint::Silent => None,
	};
	let debug_print = Print { output: debug_output, pretty: debug_pretty };

	// convert /path/to/base_dir/then/entry.unim to then.entry
	entry.set_extension("");
	let root = entry.strip_prefix(&base_dir).unwrap();
	let segments = root.components().map(|v| v.as_os_str().to_str().unwrap());
	let root = unimap::Path::from_iter(segments);

	let loader = |path: &unimap::Path, importer: &str| {
		// convert abc.def to /path/to/base_dir/abc/def.unim
		let mut path_full = base_dir.clone();
		path_full.extend(path.segments.iter().map(|v| &v.val));
		path_full.set_extension("unim");

		let Ok(file) = read_to_string(&path_full) else {
			let msg = format!(
				"import error: can not load \"{path}\", can not find \"{}\"",
				path_full.display()
			);
			return Err(Error::new(msg, path.span, importer));
		};
		Ok(LoadResult { file, path: path_full.to_str().unwrap().to_string() })
	};

	let value = run(&root, &loader, debug_print, !no_pretty).map_err(|err| err.to_string())?;

	match outfile {
		Some(file) => write(&file, value).map_err(errors::create_file(&file))?,
		None => stdout().write_all(value.as_bytes()).map_err(errors::write_stdout)?,
	}

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
	pub fn create_file<T>(path: &impl Display) -> impl FnOnce(T) -> String {
		move |_| format!("unable to create file \"{path}\"")
	}
	pub fn write_stdout<T>(_: T) -> String {
		"unable to write to stdout".to_string()
	}
}
