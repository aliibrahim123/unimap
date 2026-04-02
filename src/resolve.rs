use std::collections::HashMap;

use compact_str::CompactString;

use crate::{
	parse_file,
	parser::{
		Const as ConstSrc, File as FileSrc, Fn as FnSrc, Ident, Import, Path, Symbol as SymbolSrc,
	},
	utils::{Error, err},
	value::Id,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathNode {
	Tree(PathTree),
	Leaf(usize),
}
type PathTree = HashMap<CompactString, PathNode>;
impl PathNode {
	pub fn insert(path: &Path, file: usize, root: &mut PathTree) {
		let mut cur_branch = root;
		for segment in &path.segments[..path.segments.len() - 1] {
			if !cur_branch.contains_key(&segment.val) {
				cur_branch.insert(segment.val.clone(), PathNode::Tree(HashMap::new()));
			}
			cur_branch = match cur_branch.get_mut(&segment.val) {
				Some(PathNode::Tree(node)) => node,
				_ => panic!("how"),
			}
		}

		cur_branch.insert(path.last().val.clone(), PathNode::Leaf(file));
	}
	pub fn try_get(path: &Path, root: &PathTree, src_path: &str) -> Result<Option<usize>, Error> {
		let mut cur_branch = root;
		for (index, segment) in path.segments[..path.segments.len() - 1].iter().enumerate() {
			match cur_branch.get(&segment.val) {
				Some(PathNode::Tree(branch)) => cur_branch = branch,
				Some(PathNode::Leaf(_)) => {
					let parent = Path::display(&path.segments[..index]);
					return err!(
						"resolve error: can not load path \"{path}\" since its parent \"{parent}\" is not a directory",
						(path.span, src_path)
					);
				}
				None => return Ok(None),
			}
		}

		match cur_branch.get(&path.last().val) {
			Some(PathNode::Leaf(file)) => Ok(Some(*file)),
			Some(PathNode::Tree(_)) => err!(
				"resolve error: can not load path \"{path}\" since it is a directory",
				(path.span, src_path)
			),
			None => Ok(None),
		}
	}
	pub fn get(path: &Path, root: &PathTree) -> usize {
		let mut cur_branch = root;
		for segment in &path.segments[..path.segments.len() - 1] {
			match cur_branch.get(&segment.val) {
				Some(PathNode::Tree(branch)) => cur_branch = branch,
				_ => panic!("how"),
			}
		}
		let PathNode::Leaf(file) = cur_branch[&path.last().val] else {
			panic!("how");
		};
		file
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadResult<'a> {
	pub file: &'a str,
	pub path: &'a str,
}

fn load_loop(
	path: &Path, root: &mut PathTree, files: &mut Vec<FileSrc>,
	loader: &fn(&Path) -> Result<LoadResult, Error>,
) -> Result<(), Error> {
	PathNode::insert(path, 0, root);
	let LoadResult { file, path: src_path } = loader(path)?;
	let file = parse_file(file, src_path)?;

	for Import { path, .. } in &file.imports {
		if PathNode::try_get(path, root, src_path)?.is_none() {
			load_loop(path, root, files, loader)?
		}
	}

	let id = files.len();
	PathNode::insert(path, id, root);
	files.push(file);
	Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Const {}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fn {}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Symbol {}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecCtx {
	pub file_names: Vec<String>,
	pub consts: Vec<Const>,
	pub fns: Vec<Fn>,
	pub symbols: Vec<Symbol>,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
enum ItemType {
	Symbol,
	Const,
	Fn,
}
type Scope = HashMap<CompactString, (ItemType, Id)>;
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct File {
	pub items: Scope,
	pub top_scope: Scope,
}

fn gather(src: &FileSrc, ctx: &mut ExecCtx) -> Result<File, Error> {
	let mut file = File::default();

	let mut gather_item = |name: &Ident, item_type, id| {
		if file.items.contains_key(&name.val) {
			return err!(
				"resolve error: multiple items with the same name \"{name}\"",
				(name.span, &src.path)
			);
		}
		file.items.insert(name.val.clone(), (item_type, Id(id)));
		Ok(())
	};

	for SymbolSrc { name, .. } in &src.symbols {
		gather_item(name, ItemType::Symbol, ctx.symbols.len())?;
		ctx.symbols.push(Symbol::default());
	}
	for ConstSrc { name, .. } in &src.consts {
		gather_item(name, ItemType::Const, ctx.consts.len())?;
		ctx.consts.push(Const::default());
	}
	for FnSrc { name, .. } in &src.fns {
		gather_item(name, ItemType::Fn, ctx.fns.len())?;
		ctx.fns.push(Fn::default());
	}

	file.top_scope = file.items.clone();
	Ok(file)
}
fn resolve_imports(
	file_id: usize, src: &FileSrc, files: &mut Vec<File>, root: &PathTree,
) -> Result<(), Error> {
	for Import { path, items } in &src.imports {
		let import_file = PathNode::get(path, root);
		for item in items {
			let import_file = &files[import_file];
			let file = &files[file_id];

			if file.items.contains_key(&item.val) {
				return err!(
					"resolve error: import \"{path}.{item}\" conflicts with existing item ",
					(item.span, &src.path)
				);
			}
			if file.top_scope.contains_key(&item.val) {
				return err!(
					"resolve error: import \"{path}.{item}\" conflicts with an imported item",
					(item.span, &src.path)
				);
			}
			let Some(&imported) = import_file.items.get(&item.val) else {
				return err!(
					"resolve error: importing unexisting item \"{item}\" from file \"{path}\"",
					(item.span, &src.path)
				);
			};

			files[file_id].top_scope.insert(item.val.clone(), imported);
		}
	}
	Ok(())
}

pub fn resolve(loader: &fn(&Path) -> Result<LoadResult, Error>) -> Result<ExecCtx, Error> {
	let mut ctx = ExecCtx::default();

	let mut path_tree = HashMap::new();
	let mut files_src = Vec::new();
	load_loop(&Path::root(), &mut path_tree, &mut files_src, loader)?;

	let mut files = Vec::new();
	for file in &files_src {
		files.push(gather(file, &mut ctx)?);
	}
	for (file_id, src) in files_src.iter().enumerate() {
		resolve_imports(file_id, src, &mut files, &path_tree)?;
	}

	Ok(ctx)
}
