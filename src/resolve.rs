use std::collections::{HashMap, HashSet};

use compact_str::CompactString;

use crate::{
	exec::{
		ArrayItem, Const, ExecMode, ExecRes, Expr, ExprId, ExprKind, Field, FieldPat, Fn, ItemId,
		LocalId, MapArm, ObjectItem, Pat, PatId, PatKind, Stat, Symbol, SymbolKind,
	},
	parser::{
		ArrItemPat as ArrItemPatSrc, ArrayItem as ArrayItemSrc, Const as ConstSrc, Expr as ExprSrc,
		ExprKind as ExprSrcKind, FieldKind, FieldPat as FieldPatSrc, File as FileSrc, Fn as FnSrc,
		Ident, Import, MapArm as MapArmSrc, ObjectItem as ObjectItemSrc, Pat as PatSrc,
		PatKind as PatSrcKind, Path, Symbol as SymbolSrc, SymbolKind as SymbolSrcKind, parse_file,
	},
	tokenizer::Span,
	utils::{Error, err},
};

/// a tree structure of the import tree, used to resolve imports
#[derive(Debug, Clone, PartialEq, Eq)]
enum PathNode {
	Tree(PathTree),
	/// the index of the file
	Leaf(usize),
}
type PathTree = HashMap<CompactString, PathNode>;
impl PathNode {
	pub fn insert(path: &Path, file: usize, root: &mut PathTree) {
		let mut cur_branch = root;
		for segment in &path.segments[..path.segments.len() - 1] {
			// add branch if necessary
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
					let parent = Path::display(&path.segments[..=index]);
					return err!(
						"import error: can not load path \"{path}\" since its parent \"{parent}\" is not a directory",
						(path.span, src_path)
					);
				}
				None => return Ok(None),
			}
		}

		match cur_branch.get(&path.last().val) {
			Some(PathNode::Leaf(file)) => Ok(Some(*file)),
			Some(PathNode::Tree(_)) => err!(
				"import error: can not load path \"{path}\" since it is a directory",
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

/// loads a file at a given path, called with the importer
pub type Loader<'a> = &'a dyn std::ops::Fn(&Path, &str) -> Result<LoadResult, Error>;
/// result of `Loader`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadResult {
	pub file: String,
	pub src_path: String,
}

fn load_loop(
	path: &Path, root: &mut PathTree, files: &mut Vec<FileSrc>, importer: &str, loader: Loader,
) -> Result<(), Error> {
	// reserve place
	PathNode::insert(path, 0, root);

	let LoadResult { file, src_path } = loader(path, importer)?;
	let file = parse_file(&file, &src_path)?;

	for Import { path, .. } in &file.imports {
		if PathNode::try_get(path, root, &src_path)?.is_none() {
			load_loop(path, root, files, &src_path, loader)?
		}
	}

	let id = files.len();
	PathNode::insert(path, id, root);
	files.push(file);
	Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
enum ItemType {
	Symbol,
	Const,
	Fn,
}
/// item name -> (type, id)
type ItemScope = HashMap<CompactString, (ItemType, u32)>;
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct File {
	pub items: ItemScope,
	/// local items + imports
	pub top_scope: ItemScope,
	pub fns: Vec<ItemId>,
	pub consts: Vec<ItemId>,
}

/// enum id -> (variant name -> symbol id)
type VarMap = HashMap<ItemId, HashMap<CompactString, ItemId>>;
/// gather local items
fn gather(src: &FileSrc, var_map: &mut VarMap, exec: &mut ExecRes) -> Result<File, Error> {
	let ExecRes { file_names, consts, fns, symbols, .. } = exec;
	let mut file = File::default();
	let src_path = file_names.len();
	file_names.push(src.path.to_string());

	let mut gather_item = |name: &Ident, item_type, id| {
		if file.items.contains_key(&name.val) {
			return err!(
				"resolve error: multiple items with the same name \"{name}\"",
				(name.span, &src.path)
			);
		}
		file.items.insert(name.val.clone(), (item_type, id));
		Ok(())
	};

	for SymbolSrc { name, kind } in &src.symbols {
		let id = symbols.len() as ItemId;
		gather_item(name, ItemType::Symbol, id)?;
		symbols.push(Symbol { name: name.clone(), kind: SymbolKind::Atom, src_path });

		symbols[id as usize].kind = match kind {
			SymbolSrcKind::Atom => SymbolKind::Atom,
			SymbolSrcKind::Enum(variants) => {
				let mut vars = HashSet::new();
				let mut map = HashMap::new();
				for var in variants {
					if map.contains_key(&var.val) {
						return err!(
							"resolve error: variant \"{var}\" is already defined",
							(var.span, &src.path)
						);
					}
					let var_id = symbols.len() as ItemId;
					symbols.push(Symbol {
						name: var.clone(),
						kind: SymbolKind::Var { enum_id: id },
						src_path,
					});
					vars.insert(var_id);
					map.insert(var.val.clone(), var_id);
				}
				var_map.insert(id, map);
				SymbolKind::Enum { vars }
			}
		};
	}

	for ConstSrc { name, .. } in &src.consts {
		let id = consts.len() as ItemId;
		gather_item(name, ItemType::Const, id)?;
		consts.push(Const::new(name.clone(), src_path));
		file.consts.push(id);
	}

	for FnSrc { name, args, .. } in &src.fns {
		let id = fns.len() as ItemId;
		gather_item(name, ItemType::Fn, id)?;
		fns.push(Fn::new(name.clone(), args.len() as u16, src_path));
		file.fns.push(id);
	}

	file.top_scope = file.items.clone();
	Ok(file)
}
/// resolve imports and add to the file top scope
fn resolve_imports(
	file_id: usize, src: &FileSrc, files: &mut [File], root: &PathTree,
) -> Result<(), Error> {
	for Import { path, items } in &src.imports {
		let import_file = PathNode::get(path, root);
		for item in items {
			let import_file = &files[import_file];
			let file = &files[file_id];

			if file.top_scope.contains_key(&item.val) {
				return err!(
					"resolve error: import \"{path}.{item}\" conflicts with another item",
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct Scopes<'a> {
	pub items: &'a ItemScope,
	// reused within a file
	pub locals: Vec<LocalScope>,
	pub local_top: usize,
	pub local_counter: LocalId,
}
type LocalScope = HashMap<CompactString, LocalId>;
impl<'a> Scopes<'a> {
	pub fn new(items: &'a ItemScope) -> Scopes<'a> {
		Self { items, locals: vec![HashMap::new()], local_top: 0, local_counter: 0 }
	}
	pub fn add_scope(&mut self) {
		self.local_top += 1;
		if self.local_top >= self.locals.len() {
			self.locals.push(HashMap::new());
		}
	}
	pub fn top(&self) -> &LocalScope {
		&self.locals[self.local_top]
	}
	pub fn add_local(&mut self, name: CompactString) -> LocalId {
		self.locals[self.local_top].insert(name, self.local_counter);
		let id = self.local_counter;
		self.local_counter += 1;
		id
	}
	pub fn pop_scope(&mut self) {
		self.local_counter -= self.locals[self.local_top].len() as LocalId;
		self.locals[self.local_top].clear();
		self.local_top -= 1;
	}
	pub fn resolve_local(&self, name: &Ident) -> Option<LocalId> {
		for scope in self.locals[..=self.local_top].iter().rev() {
			if let Some(&id) = scope.get(&name.val) {
				return Some(id);
			}
		}
		None
	}
	pub fn resolve_item(&self, name: &Ident, src_path: &str) -> Result<(ItemType, ItemId), Error> {
		let Some((item_type, item_id)) = self.items.get(&name.val) else {
			return err!("resolve error: can not find \"{name}\"", (name.span, src_path));
		};
		Ok((*item_type, *item_id))
	}
}
#[derive(Debug)]
struct ResolveCtx<'a> {
	pub stat: &'a mut Stat,
	pub scopes: Scopes<'a>,
	pub exec: &'a ExecRes,
	pub var_map: &'a VarMap,
	pub src_path: &'a str,
}

fn expect_args(
	expected: u16, actual: u16, fun: &str, span: Span, src_path: &str,
) -> Result<(), Error> {
	if expected != actual {
		return err!(
			"resolve error: function \"{fun}\" expects {expected} arguments, {actual} was given",
			(span, src_path)
		);
	}
	Ok(())
}

fn resolve_field(field: &FieldKind, ctx: &ResolveCtx) -> Result<Field, Error> {
	let ResolveCtx { scopes, src_path, exec, .. } = ctx;
	let field = match field {
		FieldKind::Ident(ident) => ident,
		FieldKind::Nb(nb) => return Ok(Field::Nb(*nb)),
	};

	let (field_type, field_id) = scopes.resolve_item(field, src_path)?;
	if field_type != ItemType::Symbol {
		let kind = if field_type == ItemType::Const { "constant" } else { "function" };
		return err!(
			"resolve error: {kind} \"{field}\" can not be used as a field",
			(field.span, src_path)
		);
	}
	if exec.symbols[field_id as usize].kind != SymbolKind::Atom {
		return err!(
			"resolve error: symbol enum \"{field}\" can not be used as a field",
			(field.span, src_path)
		);
	}
	Ok(Field::Symbol(field_id))
}
fn try_resolve_symbol(name: Option<&Ident>, scopes: &Scopes) -> Option<ItemId> {
	let name = name?;
	let Some((ItemType::Symbol, id)) = scopes.items.get(&name.val) else { return None };
	scopes.resolve_local(name).is_none().then_some(*id)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdentResolve {
	Local(LocalId),
	Const(ItemId),
	/// (id, is_enum)
	Symbol(ItemId, bool),
}
fn resolve_ident(name: &Ident, ctx: &ResolveCtx, with_enum: bool) -> Result<IdentResolve, Error> {
	let ResolveCtx { scopes, src_path, exec, .. } = ctx;
	if let Some(local_id) = scopes.resolve_local(name) {
		return Ok(IdentResolve::Local(local_id));
	}

	let (item_type, item_id) = scopes.resolve_item(name, src_path)?;
	if item_type == ItemType::Symbol {
		let kind = &exec.symbols[item_id as usize].kind;
		if !with_enum && *kind != SymbolKind::Atom {
			err!("resolve error: item \"{name}\" is a symbol enum", (name.span, src_path))
		} else {
			Ok(IdentResolve::Symbol(item_id, matches!(kind, SymbolKind::Enum { .. })))
		}
	} else if item_type == ItemType::Const {
		Ok(IdentResolve::Const(item_id))
	} else {
		err!("resolve error: function \"{name}\" can not be used as a value", (name.span, src_path))
	}
}
fn resolve_pat_enum(enumm: &Ident, var: &Ident, ctx: &mut ResolveCtx) -> Result<PatKind, Error> {
	let ResolveCtx { src_path, scopes, var_map, .. } = ctx;
	if scopes.resolve_local(enumm).is_some() {
		return err!("resolve error: \"{enumm}\" is not a symbol enum", (enumm.span, src_path));
	}

	let (_, enum_id) = scopes.resolve_item(enumm, src_path)?;
	let Some(map) = var_map.get(&enum_id) else {
		return err!("resolve error: \"{enumm}\" is not a symbol enum", (enumm.span, src_path));
	};
	let Some(&id) = map.get(&var.val) else {
		return err!(
			"resolve error: symbol enum \"{enumm}\" doesnt have variant \"{var}\"",
			(var.span, src_path)
		);
	};
	Ok(PatKind::Symbol { id, is_enum: false })
}
fn resolve_pat_obj(
	fields_src: &[FieldPatSrc], allow_let: bool, ctx: &mut ResolveCtx,
) -> Result<PatKind, Error> {
	let mut fields = Vec::with_capacity(fields_src.len());
	for field in fields_src {
		fields.push(match field {
			FieldPatSrc::Key(field, pat) => {
				FieldPat::Key(resolve_field(field, ctx)?, resolve_pat(pat, allow_let, ctx)?)
			}
			FieldPatSrc::Index(index, pat) => {
				FieldPat::Index(resolve_expr(index, ctx)?, resolve_pat(pat, allow_let, ctx)?)
			}
		})
	}
	Ok(PatKind::Object(fields.into_boxed_slice()))
}
fn resolve_pat_let(
	ident: &Ident, pat: &PatSrc, allow_let: bool, span: Span, ctx: &mut ResolveCtx,
) -> Result<PatKind, Error> {
	if !allow_let {
		return err!(
			"resolve error: can not have let binding \"{ident}\" inside an or pattern",
			(span, ctx.src_path)
		);
	}

	let pat = resolve_pat(pat, allow_let, ctx)?;
	if ctx.scopes.top().contains_key(&ident.val) {
		return err!(
			"resolve error: local \"{ident}\" is already defined",
			(ident.span, ctx.src_path)
		);
	}
	let id = ctx.scopes.add_local(ident.val.clone());
	Ok(PatKind::Let(id, pat))
}
fn resolve_pat(pat: &PatSrc, allow_let: bool, ctx: &mut ResolveCtx) -> Result<PatId, Error> {
	let kind = match &pat.kind {
		// every Stat has a Any pattern as the first pat
		PatSrcKind::Any => return Ok(0),
		PatSrcKind::Ident(name) => match resolve_ident(name, ctx, true)? {
			IdentResolve::Local(local) => PatKind::Local(local),
			IdentResolve::Symbol(id, is_enum) => PatKind::Symbol { id, is_enum },
			IdentResolve::Const(id) => PatKind::Const(id),
		},
		PatSrcKind::Nb(nb) => PatKind::Nb(*nb),
		PatSrcKind::Var(enumm, var) => resolve_pat_enum(enumm, var, ctx)?,
		PatSrcKind::Let(ident, pat) => resolve_pat_let(ident, pat, allow_let, pat.span, ctx)?,
		PatSrcKind::Object(items_src) => resolve_pat_obj(items_src, allow_let, ctx)?,
		PatSrcKind::Array(items_src) => {
			let mut items = Vec::with_capacity(items_src.len());
			let mut rest = None;
			for item in items_src {
				if rest.is_some() {
					return err!(
						"resolve error: rest pattern must be the last",
						(item.span(), ctx.src_path)
					);
				}
				match item {
					ArrItemPatSrc::One(pat) => items.push(resolve_pat(pat, allow_let, ctx)?),
					ArrItemPatSrc::Rest(pat) => rest = Some(resolve_pat(pat, allow_let, ctx)?),
				};
			}
			PatKind::Array(items.into_boxed_slice(), rest)
		}
		PatSrcKind::Or(pats_src) => {
			let mut pats = Vec::with_capacity(pats_src.len());
			for pat in pats_src {
				pats.push(resolve_pat(pat, false, ctx)?);
			}
			PatKind::Or(pats.into_boxed_slice())
		}
	};
	let id = ctx.stat.pats.len();
	ctx.stat.pats.push(Pat { span: pat.span, kind });
	Ok(id as PatId)
}
fn resolve_expr_call(
	fun: &Ident, exprs: &[ExprSrc], ctx: &mut ResolveCtx,
) -> Result<ExprKind, Error> {
	let ResolveCtx { scopes, exec, src_path, .. } = ctx;
	if scopes.resolve_local(fun).is_some() {
		return err!("resolve error: \"{fun}\" is not a function", (fun.span, src_path));
	}
	let resolve_res = scopes.resolve_item(fun, src_path);
	if fun.val == "dbg" && resolve_res.is_err() {
		expect_args(1, exprs.len() as u16, "dbg", fun.span, src_path)?;
		return Ok(ExprKind::Dbg(resolve_expr(&exprs[0], ctx)?));
	}

	let (fun_type, fun_id) = resolve_res?;
	if fun_type != ItemType::Fn {
		return err!("resolve error: \"{fun}\" is not a function", (fun.span, src_path));
	}

	let args_expected = exec.fns[fun_id as usize].args_count;
	expect_args(args_expected, exprs.len() as u16, &fun.val, fun.span, src_path)?;

	let mut args = Vec::with_capacity(exprs.len());
	for expr in exprs {
		args.push(resolve_expr(expr, ctx)?);
		ctx.scopes.local_counter += 1;
	}
	ctx.scopes.local_counter -= args.len() as LocalId;

	Ok(ExprKind::Call(fun_id, args.into_boxed_slice()))
}

fn resolve_expr_field(
	expr: &ExprSrc, field: &FieldKind, ctx: &mut ResolveCtx,
) -> Result<ExprKind, Error> {
	let ResolveCtx { scopes, var_map, src_path, .. } = ctx;
	// is enum.var
	if let FieldKind::Ident(field) = field
		&& let Some(name) = expr.as_ident()
		&& let Some(id) = try_resolve_symbol(Some(name), scopes)
		&& let Some(map) = var_map.get(&id)
	{
		let Some(id) = map.get(&field.val) else {
			return err!(
				"resolve error: symbol enum \"{name}\" doesnt have variant \"{field}\"",
				(field.span, src_path)
			);
		};
		return Ok(ExprKind::Symbol(*id));
	}

	let field = resolve_field(field, ctx)?;
	let expr = resolve_expr(expr, ctx)?;
	Ok(ExprKind::Field(expr, field))
}
fn resolve_expr_obj(items_src: &[ObjectItemSrc], ctx: &mut ResolveCtx) -> Result<ExprKind, Error> {
	let mut items = Vec::with_capacity(items_src.len());
	for item in items_src {
		items.push(match item {
			ObjectItemSrc::IndexValue(index, value) => {
				ObjectItem::IndexValue(resolve_expr(index, ctx)?, resolve_expr(value, ctx)?)
			}
			ObjectItemSrc::Spread(expr) => ObjectItem::Spread(resolve_expr(expr, ctx)?),
			ObjectItemSrc::KeyValue(field, value) => {
				ObjectItem::KeyValue(resolve_field(field, ctx)?, resolve_expr(value, ctx)?)
			}
		})
	}
	Ok(ExprKind::Object(items.into_boxed_slice()))
}
fn resolve_expr_map(
	expr: &ExprSrc, arms_src: &[MapArmSrc], ctx: &mut ResolveCtx,
) -> Result<ExprKind, Error> {
	let expr = resolve_expr(expr, ctx)?;
	let is_simple = |pat: &PatSrc| {
		try_resolve_symbol(pat.as_ident(), &ctx.scopes).is_some()
			|| matches!(pat.kind, PatSrcKind::Nb(_))
	};
	// jump table optimization
	if arms_src.iter().all(|arm| is_simple(&arm.pat)) {
		let mut table = HashMap::with_capacity(arms_src.len());
		for MapArmSrc { map, pat } in arms_src {
			let pat = match pat.kind {
				PatSrcKind::Nb(nb) => Field::Nb(nb),
				_ => Field::Symbol(ctx.scopes.items[&pat.as_ident().unwrap().val].1),
			};
			let expr = resolve_expr(map, ctx)?;
			table.insert(pat, expr);
		}
		Ok(ExprKind::JumpTable(expr, Box::new(table)))
	}
	// normal map
	else {
		let mut arms = Vec::with_capacity(arms_src.len());
		for MapArmSrc { pat, map } in arms_src {
			ctx.scopes.add_scope();
			let pat = resolve_pat(pat, true, ctx)?;
			let stack_slots = ctx.scopes.top().len() as LocalId;
			if stack_slots == 0 {
				ctx.scopes.pop_scope();
			}
			let expr = resolve_expr(map, ctx)?;
			if stack_slots != 0 {
				ctx.scopes.pop_scope();
			}
			arms.push(MapArm { pat, expr, stack_slots });
		}
		Ok(ExprKind::Map(expr, arms.into_boxed_slice()))
	}
}
fn resolve_expr(expr: &ExprSrc, ctx: &mut ResolveCtx) -> Result<ExprId, Error> {
	let kind = match &expr.kind {
		// every Stat has a Cur expr as the first expr
		ExprSrcKind::Cur => return Ok(0),
		ExprSrcKind::Call(fun, exprs) => resolve_expr_call(fun, exprs, ctx)?,
		ExprSrcKind::Ident(name) => match resolve_ident(name, ctx, false)? {
			IdentResolve::Local(local) => ExprKind::Local(local),
			IdentResolve::Symbol(id, _) => ExprKind::Symbol(id),
			IdentResolve::Const(id) => ExprKind::Const(id),
		},
		ExprSrcKind::Nb(nb) => ExprKind::Nb(*nb),
		ExprSrcKind::Field(expr, index) => resolve_expr_field(expr, index, ctx)?,
		ExprSrcKind::Index(expr, index) => {
			ExprKind::Index(resolve_expr(expr, ctx)?, resolve_expr(index, ctx)?)
		}
		ExprSrcKind::Object(items) => resolve_expr_obj(items, ctx)?,
		ExprSrcKind::Array(items_src) => {
			let mut items = Vec::with_capacity(items_src.len());
			for item in items_src {
				items.push(match item {
					ArrayItemSrc::One(expr) => ArrayItem::One(resolve_expr(expr, ctx)?),
					ArrayItemSrc::Spread(expr) => ArrayItem::Spread(resolve_expr(expr, ctx)?),
				})
			}
			ExprKind::Array(items.into_boxed_slice())
		}
		ExprSrcKind::Map(expr, arms) => resolve_expr_map(expr, arms, ctx)?,
		ExprSrcKind::Pipe(exprs_src) => {
			let mut exprs = Vec::with_capacity(exprs_src.len());
			for expr in exprs_src {
				exprs.push(resolve_expr(expr, ctx)?);
			}
			ExprKind::Pipe(exprs.into_boxed_slice())
		}
	};
	let id = ctx.stat.exprs.len();
	ctx.stat.exprs.push(Expr { span: expr.span, kind });
	Ok(id as ExprId)
}

fn resolve_file(
	file: &mut File, file_src: &FileSrc, exec: &mut ExecRes, var_map: &mut VarMap,
) -> Result<(), Error> {
	let src_path = &file_src.path;

	for (ind, id) in file.fns.iter().enumerate() {
		let src = &file_src.fns[ind];
		let mut body = Stat::new();
		let mut scopes = Scopes::new(&file.top_scope);

		for arg in &src.args {
			if scopes.top().contains_key(&arg.val) {
				return err!(
					"resolve error: argument \"{arg}\" defined multiple times",
					(arg.span, src_path)
				);
			}
			scopes.add_local(arg.val.clone());
		}

		let mut res_ctx = ResolveCtx { stat: &mut body, exec: &*exec, var_map, src_path, scopes };
		body.root_expr = resolve_expr(&src.body, &mut res_ctx)?;
		exec.fns[*id as usize].body = body;
	}
	for (ind, id) in file.consts.iter().enumerate() {
		let src = &file_src.consts[ind];
		let mut init = Stat::new();
		let scopes = Scopes::new(&file.top_scope);

		let mut res_ctx = ResolveCtx { stat: &mut init, exec: &*exec, var_map, src_path, scopes };
		init.root_expr = resolve_expr(&src.init, &mut res_ctx)?;
		exec.consts[*id as usize].init = init;
	}
	Ok(())
}

fn resolve_exec_mode(files: &[File], files_src: &[FileSrc]) -> Result<ExecMode, Error> {
	let root = files.last().unwrap();
	let root_src = files_src.last().unwrap();
	let find_fn =
		|name: &str| root_src.fns.iter().enumerate().find(|(_, fun)| fun.name.val == name);

	let exec_mode = if let Some((id, fun)) = find_fn("main") {
		expect_args(0, fun.args.len() as u16, "main", fun.name.span, &root_src.path)?;
		ExecMode::Main(root.fns[id])
	} else if let Some((loop_id, loop_fn)) = find_fn("loop") {
		expect_args(1, loop_fn.args.len() as u16, "loop", loop_fn.name.span, &root_src.path)?;

		let Some((init_id, init_fn)) = find_fn("init") else {
			return err!(
				"resolve error: expected an \"init\" function with \"loop\"",
				(Span::none(), &root_src.path)
			);
		};
		expect_args(0, init_fn.args.len() as u16, "init", init_fn.name.span, &root_src.path)?;

		ExecMode::Const { init: root.fns[init_id], looper: root.fns[loop_id] }
	} else {
		return err!(
			"resolve error: can not execute the program, expected a \"main\" or \"loop\" function",
			(Span::none(), &root_src.path)
		);
	};

	Ok(exec_mode)
}

// load files, resolve items, validate and simplify the expression tree
pub fn resolve(root_path: &Path, loader: Loader) -> Result<(ExecRes, ExecMode), Error> {
	let mut exec = ExecRes::default();

	// load
	let mut path_tree = HashMap::new();
	let mut files_src = Vec::new();
	load_loop(root_path, &mut path_tree, &mut files_src, "", loader)?;

	// gather
	let mut files = Vec::new();
	let var_map = &mut HashMap::new();
	for file in &files_src {
		files.push(gather(file, var_map, &mut exec)?);
	}
	for (file_id, src) in files_src.iter().enumerate() {
		resolve_imports(file_id, src, &mut files, &path_tree)?;
	}

	// resolve
	for (file_id, file) in files.iter_mut().enumerate() {
		resolve_file(file, &files_src[file_id], &mut exec, var_map)?;
	}

	let exec_mode = resolve_exec_mode(&files, &files_src)?;

	Ok((exec, exec_mode))
}
