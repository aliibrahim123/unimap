use std::{
	borrow::Cow,
	cell::Cell,
	collections::{HashMap, HashSet},
	fmt::Debug,
};

use crate::{
	parser::Ident,
	tokenizer::Span,
	utils::{Error, err},
	value::{Object, TypedPool, Value, ValueDec, ValuePool},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
	Atom,
	Enum(HashSet<ItemId>),
	Var(ItemId),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
	pub name: Ident,
	pub kind: SymbolKind,
	pub src_path: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstStatus {
	Uninit,
	Computing,
	Computed(Value),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Const {
	pub name: Ident,
	pub init: Stat,
	pub status: Cell<ConstStatus>,
	pub src_path: usize,
}
impl Const {
	pub fn new(name: Ident, src_path: usize) -> Self {
		Self { name, init: Stat::dummy(), status: Cell::new(ConstStatus::Uninit), src_path }
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

pub type ItemId = u32;

#[derive(Debug, Default)]
pub struct ExecRes {
	pub file_names: Vec<String>,
	pub consts: Vec<Const>,
	pub fns: Vec<Fn>,
	pub symbols: Vec<Symbol>,
}

pub type ExprId = u16;
pub type PatId = u16;
pub type LocalId = u32;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
	pub span: Span,
	pub kind: ExprKind,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
	Symbol(ItemId),
	Nb(u64),
}
impl Field {
	pub fn display<'a>(&self, res: &'a ExecRes) -> Cow<'a, str> {
		match self {
			Field::Symbol(id) => Cow::from(&res.symbols[*id as usize].name.val),
			Field::Nb(nb) => Cow::from(format!("{}", nb)),
		}
	}
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectItem {
	KeyValue(Field, ExprId),
	IndexValue(ExprId, ExprId),
	Spread(ExprId),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayItem {
	One(ExprId),
	Spread(ExprId),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapArm {
	pub pat: PatId,
	pub stack_slots: LocalId,
	pub expr: ExprId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
	Cur,
	Symbol(ItemId),
	Nb(u64),
	Const(ItemId),
	Local(LocalId),
	Call(ItemId, Box<[ExprId]>),
	Field(ExprId, Field),
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
	Key(Field, PatId),
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
	Symbol { id: ItemId, is_enum: bool },
	Nb(u64),
	Const(ItemId),
	Local(LocalId),
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
	pub fn new() -> Self {
		Self {
			exprs: vec![Expr { span: Span::none(), kind: ExprKind::Cur }],
			pats: vec![Pat { span: Span::none(), kind: PatKind::Any }],
			root_expr: 0,
		}
	}
	pub fn dummy() -> Self {
		Self { exprs: Vec::new(), pats: Vec::new(), root_expr: 0 }
	}
}

#[derive(Debug)]
struct Execution<'a> {
	res: &'a ExecRes,
	stat: &'a Stat,
	pool: ValuePool,
	stack: Vec<Value>,
	frame_start: usize,
	cur_value: Option<Value>,
	src_path: usize,
}
fn exec_stat(stat: &Stat, ctx: &Execution) -> Result<Value, Error> {
	todo!()
}
fn exec_expr_const(id: ItemId, exec: &mut Execution) -> Result<Value, Error> {
	let Execution { res, pool, .. } = exec;
	let Const { name, init, status, src_path } = &res.consts[id as usize];
	match status.get() {
		ConstStatus::Computed(value) => Ok(pool.clone_value(value)),
		ConstStatus::Uninit => {
			status.set(ConstStatus::Computing);
			let value = exec_stat(init, exec)?;
			status.set(ConstStatus::Computed(value));
			Ok(value)
		}
		ConstStatus::Computing => err!(
			"execution error: detected circular dependency at constant \"{name}\"",
			(name.span, &res.file_names[*src_path])
		),
	}
}
fn exec_index(
	value: Value, field: Field, span: Span, exec: &mut Execution,
) -> Result<Value, Error> {
	let Execution { res, pool, src_path, .. } = exec;
	let src_path = &res.file_names[*src_path];
	let result = match (value.decompress(), field) {
		(ValueDec::Arr(id), Field::Nb(nb)) => match pool.arr_pool[id as usize].get(nb as usize) {
			Some(value) => pool.clone_value(*value),
			None => return err!("execution error: index {nb} is out of bounds", (span, src_path)),
		},
		(ValueDec::Obj(id), field) => match pool.obj_pool[id as usize].get(&field) {
			Some(value) => pool.clone_value(*value),
			None => {
				let field = field.display(&res);
				return err!(
					"execution error: object does not have field \"{field}\"",
					(span, src_path)
				);
			}
		},
		(_, Field::Nb(_)) => {
			return err!("execution error: indexing a non array / object value", (span, src_path));
		}
		(_, Field::Symbol(_)) => {
			return err!(
				"execution error: accessing field on a non object value",
				(span, src_path)
			);
		}
	};
	pool.free_value(value);
	Ok(result)
}
fn into_field(value: Value, span: Span, exec: &mut Execution) -> Result<Field, Error> {
	match value.decompress() {
		ValueDec::Nb(nb) => Ok(Field::Nb(nb)),
		ValueDec::Sym(id) => Ok(Field::Symbol(id)),
		index => {
			let ty = if matches!(index, ValueDec::Arr(_)) { "array" } else { "object" };
			err!(
				"execution error: using an {ty} as index",
				(span, &exec.res.file_names[exec.src_path])
			)
		}
	}
}
fn exec_expr_array(items: &[ArrayItem], exec: &mut Execution) -> Result<Value, Error> {
	let (arr, id) = exec.pool.arr_pool.alloc();
	let arr = unsafe { &mut *arr.get() };
	for item in items {
		match item {
			ArrayItem::One(expr) => {
				let item = exec_expr(*expr, exec)?;
				arr.push(item);
			}
			ArrayItem::Spread(expr) => {
				let other = exec_expr(*expr, exec)?;
				let Execution { res, stat, pool, src_path, .. } = exec;
				let Some(other_id) = other.as_arr() else {
					return err!(
						"execution error: can not spread non array value",
						(stat.exprs[*expr as usize].span, &res.file_names[*src_path])
					);
				};
				arr.extend(pool.arr_pool[other_id as usize].drain(..));
				pool.free_value(other);
			}
		}
	}
	Ok(Value::new_arr(id as u64))
}
fn exec_expr_object(items: &[ObjectItem], exec: &mut Execution) -> Result<Value, Error> {
	let (obj, id) = exec.pool.obj_pool.alloc();
	let obj = unsafe { &mut *obj.get() };
	for item in items {
		match item {
			ObjectItem::KeyValue(field, value) => {
				if let Some(value) = obj.insert(*field, exec_expr(*value, exec)?) {
					exec.pool.free_value(value)
				}
			}
			ObjectItem::IndexValue(index, value) => {
				let index_span = exec.stat.exprs[*index as usize].span;
				let field = into_field(exec_expr(*index, exec)?, index_span, exec)?;
				if let Some(value) = obj.insert(field, exec_expr(*value, exec)?) {
					exec.pool.free_value(value)
				}
			}
			ObjectItem::Spread(expr) => {
				let other = exec_expr(*expr, exec)?;
				let Execution { res, stat, pool, src_path, .. } = exec;
				let Some(other_id) = other.as_obj() else {
					return err!(
						"execution error: can not spread non object value",
						(stat.exprs[*expr as usize].span, &res.file_names[*src_path])
					);
				};
				let mut to_remove = Vec::new();
				for (field, value) in pool.obj_pool[other_id as usize].drain() {
					if let Some(value) = obj.insert(field, value) {
						to_remove.push(value)
					}
				}
				exec.pool.free_value(other);
				for value in to_remove {
					exec.pool.free_value(value);
				}
			}
		}
	}
	Ok(Value::new_obj(id as u64))
}
fn exec_expr(expr: ExprId, exec: &mut Execution) -> Result<Value, Error> {
	let Execution { res, pool, stack, stat, cur_value, frame_start, src_path } = exec;
	let expr = &stat.exprs[expr as usize];
	let span = expr.span;
	match &expr.kind {
		ExprKind::Cur => match cur_value {
			Some(value) => Ok(pool.clone_value(*value)),
			None => err!(
				"execution error: no intermediate value available",
				(span, &res.file_names[*src_path])
			),
		},
		ExprKind::Nb(nb) => Ok(Value::new_nb(*nb)),
		ExprKind::Symbol(id) => Ok(Value::new_sym(*id)),
		ExprKind::Const(id) => exec_expr_const(*id, exec),
		ExprKind::Local(id) => Ok(pool.clone_value(stack[*frame_start + *id as usize])),
		ExprKind::Field(expr, field) => exec_index(exec_expr(*expr, exec)?, *field, span, exec),
		ExprKind::Index(expr, index) => {
			let target = exec_expr(*expr, exec)?;
			let field = into_field(exec_expr(*index, exec)?, span, exec)?;
			exec_index(target, field, span, exec)
		}
		ExprKind::Array(items) => exec_expr_array(items, exec),
		ExprKind::Object(items) => exec_expr_object(items, exec),
		ExprKind::Pipe(exprs) => {
			let prev_value = *cur_value;
			for (id, expr) in exprs.iter().enumerate() {
				let value = exec_expr(*expr, exec)?;
				if id != 0 {
					exec.pool.free_value(exec.cur_value.unwrap());
				}
				exec.cur_value = Some(value);
			}
			let value = exec.cur_value.unwrap();
			exec.cur_value = prev_value;
			Ok(value)
		}
		_ => todo!(),
	}
}
