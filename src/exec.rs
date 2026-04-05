use std::collections::HashMap;

use compact_str::CompactString;

use crate::{parser::Ident, tokenizer::Span, value::Value};

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

pub type ItemId = u32;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecCtx {
	pub file_names: Vec<String>,
	pub consts: Vec<Const>,
	pub fns: Vec<Fn>,
	pub symbols: Vec<Symbol>,
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
pub enum Field {
	Symbol(ItemId),
	Nb(u64),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectItem {
	KeyValue(Field, ExprId),
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
	pub pat: PatId,
	pub scope_slots: LocalId,
	pub expr: ExprId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
	Cur,
	Symbol(ItemId),
	Nb(u64),
	Const(ItemId),
	Local(ScopeId, LocalId),
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
	Symbol(ItemId),
	Nb(u64),
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
