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
	value::{Value, ValueDec, ValuePool},
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
	JumpTable(ExprId, Box<HashMap<Field, ExprId>>),
	Map(ExprId, Box<[MapArm]>),
	Pipe(Box<[ExprId]>),
	Dbg(ExprId),
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
pub enum PatKind {
	Any,
	Symbol { id: ItemId, is_enum: bool },
	Nb(u64),
	Const(ItemId),
	Local(LocalId),
	Let(LocalId, PatId),
	Object(Box<[FieldPat]>),
	Array(Box<[PatId]>, Option<PatId>),
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

pub struct Print<'a> {
	pub output: Option<&'a mut dyn std::io::Write>,
	pub pretty: bool,
}
impl Debug for Print<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("DebugPrint").field("pretty", &self.pretty).finish()
	}
}
#[derive(Debug)]
pub struct Execution<'a, 'b, 'c> {
	stat: &'a Stat,
	pool: &'a mut ValuePool,
	stack: &'a mut Vec<Value>,
	frame_start: usize,
	cur_value: Option<Value>,
	src_path: &'a str,
	debug_print: &'c mut Print<'b>,
}

#[derive(Debug)]
pub enum ExecMode {
	Main(ItemId),
	Const { init: ItemId, looper: ItemId },
}

pub fn exec(
	res: ExecRes, mode: ExecMode, mut debug_print: Print, pretty_output: bool,
) -> Result<String, Error> {
	let init_fn = &res.fns[match mode {
		ExecMode::Main(main) => main,
		ExecMode::Const { init, .. } => init,
	} as usize];
	let mut exec = Execution {
		stat: &init_fn.body,
		pool: &mut ValuePool::default(),
		stack: &mut Vec::new(),
		frame_start: 0,
		cur_value: None,
		src_path: &res.file_names[init_fn.src_path],
		debug_print: &mut debug_print,
	};
	let final_value = match mode {
		ExecMode::Main(_) => exec_expr(init_fn.body.root_expr, &res, &mut exec)?,
		ExecMode::Const { looper, .. } => {
			let mut cur_value = exec_expr(init_fn.body.root_expr, &res, &mut exec)?;

			let looper = &res.fns[looper as usize];
			exec.stat = &looper.body;
			exec.cur_value = None;
			exec.stack.push(cur_value);

			let expect_ret = |exec: &Execution| {
				err!(
					"execution error: expected \"loop\" return to be `[continue / end, value]`",
					(looper.name.span, exec.src_path)
				)
			};

			loop {
				cur_value = exec_expr(looper.body.root_expr, &res, &mut exec)?;
				exec.pool.free_value(exec.stack[0]);

				let Some(ret_id) = cur_value.as_arr() else { return expect_ret(&exec) };
				let ret = &exec.pool.arr_pool[ret_id as usize];
				if ret.len() != 2 {
					return expect_ret(&exec);
				}

				let Some(control) = ret[0].as_sym() else { return expect_ret(&exec) };
				match res.symbols[control as usize].name.val.as_str() {
					"continue" => cur_value = exec.pool.clone_value(ret[1]),
					"end" => break ret[1],
					_ => return expect_ret(&exec),
				}

				exec.pool.free_value(Value::new_arr(ret_id));
				exec.stack[0] = cur_value;
				exec.cur_value = None;
			}
		}
	};

	Ok(final_value.display(pretty_output, &res, &exec.pool))
}

fn exec_index(
	value: Value, field: Field, span: Span, res: &ExecRes, exec: &mut Execution,
) -> Result<Value, Error> {
	let Execution { pool, src_path, .. } = exec;
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
			err!("execution error: using an {ty} as index", (span, exec.src_path))
		}
	}
}
fn resolve_const(id: ItemId, res: &ExecRes, exec: &mut Execution) -> Result<Value, Error> {
	let Execution { pool, stack, .. } = exec;
	let Const { name, init, status, src_path } = &res.consts[id as usize];
	let src_path = &res.file_names[*src_path];
	match status.get() {
		ConstStatus::Computed(value) => Ok(value),
		ConstStatus::Uninit => {
			status.set(ConstStatus::Computing);
			let mut exec_fork = Execution {
				stat: init,
				cur_value: None,
				frame_start: stack.len(),
				src_path,
				pool,
				stack,
				debug_print: exec.debug_print,
			};
			let value = exec_expr(init.root_expr, res, &mut exec_fork)?;
			status.set(ConstStatus::Computed(value));
			Ok(value)
		}
		ConstStatus::Computing => err!(
			"execution error: detected circular dependency at constant \"{name}\"",
			(name.span, src_path)
		),
	}
}
fn exec_pat_array(
	items: &[PatId], rest: &Option<PatId>, value: Value, res: &ExecRes, exec: &mut Execution,
) -> Result<bool, Error> {
	let Some(id) = value.as_arr() else { return Ok(false) };
	let arr = unsafe { &*exec.pool.arr_pool.get_cell(id as usize).get() };
	if arr.len() < items.len() {
		return Ok(false);
	}
	for (index, item) in items.iter().enumerate() {
		if !exec_pat(*item, arr[index], res, exec)? {
			return Ok(false);
		}
	}
	if let Some(pat) = rest {
		let (slice, id) = exec.pool.arr_pool.alloc();
		let slice = unsafe { &mut *slice.get() };
		slice.extend(&arr[items.len()..]);
		for value in slice {
			exec.pool.clone_value(*value);
		}
		let slice = Value::new_arr(id as u64);
		let res = exec_pat(*pat, slice, res, exec)?;
		exec.pool.free_value(slice);
		Ok(res)
	} else {
		Ok(items.len() == arr.len())
	}
}
fn exec_pat_object(
	fields: &[FieldPat], value: Value, res: &ExecRes, exec: &mut Execution,
) -> Result<bool, Error> {
	let Some(id) = value.as_obj() else { return Ok(false) };
	let obj = unsafe { &*exec.pool.obj_pool.get_cell(id as usize).get() };

	for field in fields {
		let (field, pat) = match field {
			FieldPat::Key(field, pat) => (*field, *pat),
			FieldPat::Index(index, pat) => {
				let index_span = exec.stat.exprs[*index as usize].span;
				let field = into_field(exec_expr(*index, res, exec)?, index_span, exec)?;
				(field, *pat)
			}
		};
		let Some(value) = obj.get(&field) else { return Ok(false) };
		if !exec_pat(pat, *value, res, exec)? {
			return Ok(false);
		}
	}

	Ok(true)
}
fn exec_pat(pat: PatId, value: Value, res: &ExecRes, exec: &mut Execution) -> Result<bool, Error> {
	let Execution { stat, pool, stack, frame_start, .. } = exec;
	let pat = &stat.pats[pat as usize];
	Ok(match &pat.kind {
		PatKind::Any => true,
		PatKind::Nb(nb) => value.as_nb() == Some(*nb),
		PatKind::Symbol { id, is_enum: false } => value.as_sym() == Some(*id),
		PatKind::Symbol { id, is_enum: true } => {
			let SymbolKind::Enum(vars) = &res.symbols[*id as usize].kind else { unreachable!() };
			value.as_sym().map_or(false, |sym| vars.contains(&sym))
		}
		PatKind::Const(id) => Value::eq(resolve_const(*id, res, exec)?, value, exec.pool),
		PatKind::Local(id) => Value::eq(stack[*frame_start + *id as usize], value, pool),
		PatKind::Let(id, pat) => {
			if !exec_pat(*pat, value, res, exec)? {
				return Ok(false);
			}
			exec.stack[exec.frame_start + *id as usize] = exec.pool.clone_value(value);
			true
		}
		PatKind::Array(items, rest) => exec_pat_array(items, rest, value, res, exec)?,
		PatKind::Object(fields) => exec_pat_object(fields, value, res, exec)?,
		PatKind::Or(pats) => {
			for pat in pats {
				if exec_pat(*pat, value, res, exec)? {
					return Ok(true);
				}
			}
			false
		}
	})
}
fn exec_expr_array(
	items: &[ArrayItem], res: &ExecRes, exec: &mut Execution,
) -> Result<Value, Error> {
	let (arr, id) = exec.pool.arr_pool.alloc();
	let arr = unsafe { &mut *arr.get() };
	for item in items {
		match item {
			ArrayItem::One(expr) => {
				let item = exec_expr(*expr, res, exec)?;
				arr.push(item);
			}
			ArrayItem::Spread(expr) => {
				let other = exec_expr(*expr, res, exec)?;
				let Execution { stat, pool, src_path, .. } = exec;
				let Some(other_id) = other.as_arr() else {
					return err!(
						"execution error: can not spread non array value",
						(stat.exprs[*expr as usize].span, src_path)
					);
				};
				for value in &pool.arr_pool[other_id as usize] {
					arr.push(pool.clone_value(*value));
				}

				pool.free_value(other);
			}
		}
	}
	Ok(Value::new_arr(id as u64))
}
fn exec_expr_object(
	items: &[ObjectItem], res: &ExecRes, exec: &mut Execution,
) -> Result<Value, Error> {
	let (obj, id) = exec.pool.obj_pool.alloc();
	let obj = unsafe { &mut *obj.get() };
	for item in items {
		match item {
			ObjectItem::KeyValue(field, value) => {
				if let Some(value) = obj.insert(*field, exec_expr(*value, res, exec)?) {
					exec.pool.free_value(value)
				}
			}
			ObjectItem::IndexValue(index, value) => {
				let index_span = exec.stat.exprs[*index as usize].span;
				let field = into_field(exec_expr(*index, res, exec)?, index_span, exec)?;
				if let Some(value) = obj.insert(field, exec_expr(*value, res, exec)?) {
					exec.pool.free_value(value)
				}
			}
			ObjectItem::Spread(expr) => {
				let other = exec_expr(*expr, res, exec)?;
				let Execution { stat, pool, src_path, .. } = exec;
				let Some(other_id) = other.as_obj() else {
					return err!(
						"execution error: can not spread non object value",
						(stat.exprs[*expr as usize].span, src_path)
					);
				};
				let other_obj = unsafe { &*pool.obj_pool.get_cell(other_id as usize).get() };
				for (field, value) in other_obj {
					if let Some(value) = obj.insert(*field, pool.clone_value(*value)) {
						pool.free_value(value);
					}
				}
				pool.free_value(other);
			}
		}
	}
	Ok(Value::new_obj(id as u64))
}
fn exec_expr_call(
	fun: ItemId, args_exprs: &[ExprId], res: &ExecRes, exec: &mut Execution,
) -> Result<Value, Error> {
	let Fn { body, src_path, .. } = &res.fns[fun as usize];
	let frame_start = exec.stack.len();
	for arg in args_exprs {
		let arg = exec_expr(*arg, res, exec)?;
		exec.stack.push(arg);
	}
	let mut exec = Execution {
		stat: body,
		cur_value: None,
		frame_start,
		src_path: &res.file_names[*src_path],
		pool: exec.pool,
		stack: exec.stack,
		debug_print: exec.debug_print,
	};
	let ret = exec_expr(body.root_expr, res, &mut exec)?;
	for value in exec.stack.drain(frame_start..) {
		exec.pool.free_value(value);
	}
	Ok(ret)
}
fn exec_expr_map(
	expr: ExprId, arms: &[MapArm], span: Span, res: &ExecRes, exec: &mut Execution,
) -> Result<Value, Error> {
	let value = exec_expr(expr, res, exec)?;
	for arm in arms {
		let stack_top = exec.stack.len();
		if arm.stack_slots != 0 {
			exec.stack.resize(stack_top + arm.stack_slots as usize, Value::DUMMY);
		}
		let res = exec_pat(arm.pat, value, res, exec)?.then(|| exec_expr(arm.expr, res, exec));
		for value in exec.stack.drain(stack_top..) {
			exec.pool.free_value(value);
		}
		if let Some(res) = res {
			exec.pool.free_value(value);
			return res;
		}
	}
	err!("execution error: not exhaustive patterns", (span, exec.src_path))
}
fn exec_expr(expr: ExprId, res: &ExecRes, exec: &mut Execution) -> Result<Value, Error> {
	let Execution { pool, stack, stat, cur_value, frame_start, src_path, .. } = exec;
	let expr = &stat.exprs[expr as usize];
	let span = expr.span;
	match &expr.kind {
		ExprKind::Cur => match cur_value {
			Some(value) => Ok(pool.clone_value(*value)),
			None => err!("execution error: no intermediate value available", (span, src_path)),
		},
		ExprKind::Nb(nb) => Ok(Value::new_nb(*nb)),
		ExprKind::Symbol(id) => Ok(Value::new_sym(*id)),
		ExprKind::Const(id) => {
			let value = resolve_const(*id, res, exec)?;
			Ok(exec.pool.clone_value(value))
		}
		ExprKind::Local(id) => Ok(pool.clone_value(stack[*frame_start + *id as usize])),
		ExprKind::Field(expr, field) => {
			exec_index(exec_expr(*expr, res, exec)?, *field, span, res, exec)
		}
		ExprKind::Index(expr, index) => {
			let target = exec_expr(*expr, res, exec)?;
			let field = into_field(exec_expr(*index, res, exec)?, span, exec)?;
			exec_index(target, field, span, res, exec)
		}
		ExprKind::Call(fun, args) => exec_expr_call(*fun, args, res, exec),
		ExprKind::Array(items) => exec_expr_array(items, res, exec),
		ExprKind::Object(items) => exec_expr_object(items, res, exec),
		ExprKind::JumpTable(expr, table) => {
			let value = exec_expr(*expr, res, exec)?;
			let Ok(value) = into_field(value, exec.stat.exprs[*expr as usize].span, exec) else {
				return err!("execution error: non exhaustive patterns", (span, exec.src_path));
			};
			let Some(value) = table.get(&value) else {
				return err!("execution error: non exhaustive patterns", (span, exec.src_path));
			};
			exec_expr(*value, res, exec)
		}
		ExprKind::Map(expr, arms) => exec_expr_map(*expr, arms, span, res, exec),
		ExprKind::Pipe(exprs) => {
			let prev_value = *cur_value;
			for (id, expr) in exprs.iter().enumerate() {
				let value = exec_expr(*expr, res, exec)?;
				if id != 0 {
					exec.pool.free_value(exec.cur_value.unwrap());
				}
				exec.cur_value = Some(value);
			}
			let value = exec.cur_value.unwrap();
			exec.cur_value = prev_value;
			Ok(value)
		}
		ExprKind::Dbg(expr) => {
			let expr = exec_expr(*expr, res, exec)?;
			if let Print { pretty, output: Some(out) } = exec.debug_print {
				if write!(out, "{}\n", expr.display(*pretty, res, exec.pool)).is_err() {
					return err!(
						"execution error: failed to write to debug output",
						(span, exec.src_path)
					);
				};
			}
			Ok(expr)
		}
	}
}
