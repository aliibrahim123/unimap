use std::collections::HashMap;

use compact_str::CompactString;

use crate::{
	parser::{
		ArrItemPat as ArrItemPatSrc, ArrayItem as ArrayItemSrc, Const as ConstSrc, Expr as ExprSrc,
		ExprKind as ExprSrcKind, FieldPat as FieldPatSrc, File as FileSrc, Fn as FnSrc, Ident,
		Import, MapArm as MapArmSrc, ObjectItem as ObjectItemSrc, Pat as PatSrc,
		PatKind as PatSrcKind, Path, Symbol as SymbolSrc, SymbolKind as SymbolSrcKind, parse_file,
	},
	tokenizer::Span,
	utils::{Error, err},
	value::Value,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
	Atom,
	Enum(HashMap<CompactString, ItemId>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
	pub name: Ident,
	pub kind: SymbolKind,
	pub src_path: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstValue {
	Uninit,
	Computing,
	Computed(Value),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Const {
	pub name: Ident,
	pub init: Stat,
	pub value: ConstValue,
	pub src_path: usize,
}
impl Const {
	pub fn new(name: Ident, src_path: usize) -> Self {
		Self { name, init: Stat::dummy(), value: ConstValue::Uninit, src_path }
	}
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fn {
	pub name: Ident,
	pub args_count: u16,
	pub body: Stat,
	pub src_path: usize,
}
impl Fn {
	pub fn new(name: Ident, args_count: u16, src_path: usize) -> Self {
		Self { name, args_count, body: Stat::dummy(), src_path }
	}
}

// pub type ValueId = u64;
pub type ItemId = u32;

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
type ItemScope = HashMap<CompactString, (ItemType, u32)>;
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct File {
	pub items: ItemScope,
	pub top_scope: ItemScope,
	pub fns: Vec<ItemId>,
	pub consts: Vec<ItemId>,
}

fn gather(src: &FileSrc, ctx: &mut ExecCtx) -> Result<File, Error> {
	let ExecCtx { file_names, consts, fns, symbols } = ctx;
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
		let kind = match kind {
			SymbolSrcKind::Atom => SymbolKind::Atom,
			SymbolSrcKind::Enum(variants) => {
				let mut map = HashMap::new();
				for var in variants {
					let id = symbols.len();
					symbols.push(Symbol { name: name.clone(), kind: SymbolKind::Atom, src_path });
					map.insert(var.val.clone(), id as ItemId);
				}
				SymbolKind::Enum(map)
			}
		};
		gather_item(name, ItemType::Symbol, symbols.len() as ItemId)?;
		symbols.push(Symbol { name: name.clone(), kind, src_path });
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
fn resolve_imports(
	file_id: usize, src: &FileSrc, files: &mut Vec<File>, root: &PathTree,
) -> Result<(), Error> {
	for Import { path, items } in &src.imports {
		let import_file = PathNode::get(path, root);
		for item in items {
			let import_file = &files[import_file];
			let file = &files[file_id];

			if file.items.contains_key(&item.val) || file.top_scope.contains_key(&item.val) {
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

pub type ExprId = u16;
pub type PatId = u16;
pub type ScopeId = u16;
pub type LocalId = u16;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
	pub span: Span,
	pub kind: ExprKind,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectItem {
	KeyValue(ItemId, ExprId),
	IndexValue(ExprId, ExprId),
	Rest(ExprId),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayItem {
	One(ExprId),
	Rest(ExprId),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapArm {
	pat: PatId,
	scope_slots: LocalId,
	expr: ExprId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
	Cur,
	Symbol(ItemId),
	Const(ItemId),
	Local(ScopeId, LocalId),
	Call(ItemId, Box<[ExprId]>),
	Field(ExprId, ItemId),
	Index(ExprId, ExprId),
	Object(Box<[ObjectItem]>),
	Array(Box<[ArrayItem]>),
	JumpTable(ExprId, Box<HashMap<ItemId, ExprId>>),
	Map(ExprId, Box<[MapArm]>),
	Pipe(Box<[ExprId]>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pat {
	pub span: Span,
	pub kind: PatKind,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldPat {
	Key(ItemId, PatId),
	Index(ExprId, PatId),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrItemPat {
	One(PatId),
	Rest(PatId),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatKind {
	Any,
	Symbol(ItemId),
	Const(ItemId),
	Local(ScopeId, LocalId),
	Let(LocalId, PatId),
	Object(Box<[FieldPat]>),
	Array(Box<[ArrItemPat]>),
	Or(Box<[PatId]>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
	pub exprs: Vec<Expr>,
	pub pats: Vec<Pat>,
	pub root_expr: ExprId,
}
impl Stat {
	fn new() -> Self {
		Self {
			exprs: vec![Expr { span: Span::none(), kind: ExprKind::Cur }],
			pats: vec![Pat { span: Span::none(), kind: PatKind::Any }],
			root_expr: 0,
		}
	}
	fn dummy() -> Self {
		Self { exprs: Vec::new(), pats: Vec::new(), root_expr: 0 }
	}
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Scopes<'a> {
	pub items: &'a ItemScope,
	pub locals: Vec<LocalScope>,
	pub local_top: usize,
}
type LocalScope = HashMap<CompactString, LocalId>;
impl<'a> Scopes<'a> {
	pub fn new(items: &'a ItemScope) -> Scopes<'a> {
		Self { items, locals: vec![HashMap::new()], local_top: 0 }
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
	pub fn top_mut(&mut self) -> &mut LocalScope {
		&mut self.locals[self.local_top]
	}
	pub fn pop_scope(&mut self) {
		self.locals[self.local_top].clear();
		self.local_top -= 1;
	}
	pub fn resolve_local(&self, name: &Ident) -> Option<(ScopeId, LocalId)> {
		for (scope_id, scope) in self.locals[..=self.local_top].iter().enumerate().rev() {
			if let Some(&id) = scope.get(&name.val) {
				return Some((scope_id as ScopeId, id));
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
#[derive(Debug, PartialEq, Eq)]
struct ResolveCtx<'a> {
	pub stat: &'a mut Stat,
	pub scopes: Scopes<'a>,
	pub exec_ctx: &'a ExecCtx,
	pub src_path: &'a str,
}
fn resolve_field(field: &Ident, ctx: &ResolveCtx) -> Result<ItemId, Error> {
	let ResolveCtx { scopes, src_path, .. } = ctx;
	if scopes.resolve_local(field).is_some() {
		return err!(
			"resolve error: local \"{field}\" can not be used as a field",
			(field.span, src_path)
		);
	}
	let (field_type, field_id) = scopes.resolve_item(field, src_path)?;
	if field_type != ItemType::Symbol {
		let kind = if field_type == ItemType::Const { "constant" } else { "function" };
		return err!(
			"resolve error: {kind} \"{field}\" can not be used as a field",
			(field.span, src_path)
		);
	}
	if ctx.exec_ctx.symbols[field_id as usize].kind != SymbolKind::Atom {
		return err!(
			"resolve error: symbol enum \"{field}\" can not be used as a field",
			(field.span, src_path)
		);
	}
	Ok(field_id)
}
fn try_resolve_symbol(name: Option<&Ident>, scopes: &Scopes) -> Option<ItemId> {
	let name = name?;
	let Some((ItemType::Symbol, id)) = scopes.items.get(&name.val) else { return None };
	scopes.resolve_local(name).is_none().then_some(*id)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdentResolve {
	Local(ScopeId, LocalId),
	Const(ItemId),
	Symbol(ItemId),
}
fn resolve_ident(name: &Ident, ctx: &ResolveCtx) -> Result<IdentResolve, Error> {
	let ResolveCtx { scopes, src_path, exec_ctx, .. } = ctx;
	if let Some((scope_id, local_id)) = scopes.resolve_local(name) {
		return Ok(IdentResolve::Local(scope_id, local_id));
	}

	let (item_type, item_id) = scopes.resolve_item(name, src_path)?;
	if item_type == ItemType::Symbol {
		if exec_ctx.symbols[item_id as usize].kind != SymbolKind::Atom {
			err!("resolve error: item \"{name}\" is a symbol enum", (name.span, src_path))
		} else {
			Ok(IdentResolve::Symbol(item_id))
		}
	} else if item_type == ItemType::Const {
		Ok(IdentResolve::Const(item_id))
	} else {
		err!("resolve error: function \"{name}\" can not be used as a value", (name.span, src_path))
	}
}
fn resolve_pat_enum(enumm: &Ident, var: &Ident, ctx: &mut ResolveCtx) -> Result<PatKind, Error> {
	let ResolveCtx { src_path, scopes, .. } = ctx;
	if scopes.resolve_local(enumm).is_some() {
		return err!("resolve error: \"{enumm}\" is not a symbol", (enumm.span, src_path));
	}
	let (enum_type, enum_id) = scopes.resolve_item(enumm, src_path)?;
	if enum_type != ItemType::Symbol {
		return err!("resolve error: \"{enumm}\" is not a symbol", (enumm.span, src_path));
	}
	let Symbol { kind: SymbolKind::Enum(map), .. } = &ctx.exec_ctx.symbols[enum_id as usize] else {
		return err!("resolve error: \"{enumm}\" is not a symbol enum", (enumm.span, src_path));
	};
	let Some(&id) = map.get(&var.val) else {
		return err!(
			"resolve error: symbol enum \"{enumm}\" doesnt have variant \"{var}\"",
			(var.span, src_path)
		);
	};
	Ok(PatKind::Symbol(id))
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
	ident: &Ident, pat: &PatSrc, allow_let: bool, ctx: &mut ResolveCtx,
) -> Result<PatKind, Error> {
	let top = ctx.scopes.top_mut();
	if top.contains_key(&ident.val) {
		return err!(
			"resolve error: local \"{ident}\" is already defined",
			(ident.span, ctx.src_path)
		);
	}
	let id = top.len() as LocalId;
	top.insert(ident.val.clone(), id);
	let pat = resolve_pat(pat, allow_let, ctx)?;
	Ok(PatKind::Let(id, pat))
}
fn resolve_pat(pat: &PatSrc, allow_let: bool, ctx: &mut ResolveCtx) -> Result<PatId, Error> {
	let kind = match &pat.kind {
		PatSrcKind::Any => return Ok(0),
		PatSrcKind::Ident(name) => match resolve_ident(name, ctx)? {
			IdentResolve::Local(scope, local) => PatKind::Local(scope, local),
			IdentResolve::Symbol(id) => PatKind::Symbol(id),
			IdentResolve::Const(id) => PatKind::Const(id),
		},
		PatSrcKind::Enum(enumm, var) => resolve_pat_enum(enumm, var, ctx)?,
		PatSrcKind::Let(ident, pat) => resolve_pat_let(ident, pat, allow_let, ctx)?,
		PatSrcKind::Object(items_src) => resolve_pat_obj(items_src, allow_let, ctx)?,
		PatSrcKind::Array(items_src) => {
			let mut items = Vec::with_capacity(items_src.len());
			for item in items_src {
				items.push(match item {
					ArrItemPatSrc::One(pat) => ArrItemPat::One(resolve_pat(pat, allow_let, ctx)?),
					ArrItemPatSrc::Rest(pat) => ArrItemPat::Rest(resolve_pat(pat, allow_let, ctx)?),
				});
			}
			PatKind::Array(items.into_boxed_slice())
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
	let ResolveCtx { scopes, exec_ctx, src_path, .. } = ctx;
	if scopes.resolve_local(fun).is_some() {
		return err!("resolve error: \"{fun}\" is not a function", (fun.span, src_path));
	}
	let (fun_type, fun_id) = scopes.resolve_item(fun, src_path)?;
	if fun_type != ItemType::Fn {
		return err!("resolve error: \"{fun}\" is not a function", (fun.span, src_path));
	}

	let args_expected = exec_ctx.fns[fun_id as usize].args_count;
	let args_given = exprs.len();
	if args_expected != exprs.len() as u16 {
		return err!(
			"resolve error: function \"{fun}\" expects {args_expected} arguments but {args_given} was given",
			(fun.span, src_path)
		);
	}

	let mut args = Vec::with_capacity(exprs.len());
	for expr in exprs {
		args.push(resolve_expr(expr, ctx)?);
	}
	Ok(ExprKind::Call(fun_id, args.into_boxed_slice()))
}

fn resolve_expr_field(
	expr: &ExprSrc, field: &Ident, ctx: &mut ResolveCtx,
) -> Result<ExprKind, Error> {
	let ResolveCtx { scopes, src_path, exec_ctx, .. } = ctx;
	if let Some(id) = try_resolve_symbol(expr.as_ident(), scopes)
		&& let Symbol { name, kind: SymbolKind::Enum(map), .. } = &exec_ctx.symbols[id as usize]
	{
		let Some(&id) = map.get(&field.val) else {
			return err!(
				"resolve error: symbol enum \"{name}\" doesnt have variant \"{field}\"",
				(field.span, src_path)
			);
		};
		return Ok(ExprKind::Symbol(id));
	}

	let field_id = resolve_field(field, ctx)?;
	let expr = resolve_expr(expr, ctx)?;
	Ok(ExprKind::Field(expr, field_id))
}
fn resolve_expr_obj(items_src: &[ObjectItemSrc], ctx: &mut ResolveCtx) -> Result<ExprKind, Error> {
	let mut items = Vec::with_capacity(items_src.len());
	for item in items_src {
		items.push(match item {
			ObjectItemSrc::IndexValue(index, value) => {
				ObjectItem::IndexValue(resolve_expr(index, ctx)?, resolve_expr(value, ctx)?)
			}
			ObjectItemSrc::Rest(expr) => ObjectItem::Rest(resolve_expr(expr, ctx)?),
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
	let is_symbol = |ident| try_resolve_symbol(ident, &ctx.scopes).is_some();
	if arms_src.iter().all(|arm| is_symbol(arm.pat.as_ident())) {
		let mut table = HashMap::with_capacity(arms_src.len());
		for MapArmSrc { map, pat } in arms_src {
			let symbol = ctx.scopes.items[&pat.as_ident().unwrap().val].1;
			let expr = resolve_expr(map, ctx)?;
			table.insert(symbol, expr);
		}
		Ok(ExprKind::JumpTable(expr, Box::new(table)))
	} else {
		let mut arms = Vec::with_capacity(arms_src.len());
		for MapArmSrc { pat, map } in arms_src {
			ctx.scopes.add_scope();
			let pat = resolve_pat(pat, true, ctx)?;
			let scope_slots = ctx.scopes.top().len() as LocalId;
			if ctx.scopes.top().len() == 0 {
				ctx.scopes.pop_scope();
			}
			let expr = resolve_expr(map, ctx)?;
			if ctx.scopes.top().len() != 0 {
				ctx.scopes.pop_scope();
			}
			arms.push(MapArm { pat, expr, scope_slots });
		}
		Ok(ExprKind::Map(expr, arms.into_boxed_slice()))
	}
}
fn resolve_expr(expr: &ExprSrc, ctx: &mut ResolveCtx) -> Result<ExprId, Error> {
	let kind = match &expr.kind {
		ExprSrcKind::Cur => return Ok(0),
		ExprSrcKind::Call(fun, exprs) => resolve_expr_call(fun, exprs, ctx)?,
		ExprSrcKind::Ident(name) => match resolve_ident(name, ctx)? {
			IdentResolve::Local(scope, local) => ExprKind::Local(scope, local),
			IdentResolve::Symbol(id) => ExprKind::Symbol(id),
			IdentResolve::Const(id) => ExprKind::Const(id),
		},
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
					ArrayItemSrc::Rest(expr) => ArrayItem::Rest(resolve_expr(expr, ctx)?),
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

	for (file_id, file) in files.iter_mut().enumerate() {
		let file_src = &files_src[file_id];
		for (ind, id) in file.fns.iter().enumerate() {
			let src = &file_src.fns[ind];
			let mut body = Stat::new();
			let mut res_ctx = ResolveCtx {
				stat: &mut body,
				exec_ctx: &ctx,
				src_path: &ctx.file_names[file_id],
				scopes: Scopes::new(&file.top_scope),
			};
			body.root_expr = resolve_expr(&src.body, &mut res_ctx)?;
			ctx.fns[*id as usize].body = body;
		}
		for (ind, id) in file.consts.iter().enumerate() {
			let src = &file_src.consts[ind];
			let mut init = Stat::new();
			let mut res_ctx = ResolveCtx {
				stat: &mut init,
				exec_ctx: &ctx,
				src_path: &ctx.file_names[file_id],
				scopes: Scopes::new(&file.top_scope),
			};
			init.root_expr = resolve_expr(&src.init, &mut res_ctx)?;
			ctx.consts[*id as usize].init = init;
		}
	}

	Ok(ctx)
}
