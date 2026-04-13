use std::{
	cell::{Cell, UnsafeCell},
	fmt::Write,
	ops::{Index, IndexMut},
	usize,
};

use rustc_hash::FxHashMap;

use crate::exec::{ExecRes, Field, ItemId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Value(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueDec {
	Sym(ItemId),
	Nb(u64),
	Obj(u64),
	Arr(u64),
}

impl Value {
	pub const NB_PRE: u64 = 1 << 63;
	pub const OBJ_PRE: u64 = 1 << 62;
	pub const ARR_PRE: u64 = 0b11 << 61;
	pub const PRE: u64 = 0b111 << 61;
	pub const DUMMY: Value = Value(0);

	pub fn new_sym(id: ItemId) -> Self {
		Self(id as u64)
	}
	pub fn new_nb(nb: u64) -> Self {
		Self(nb | Self::NB_PRE)
	}
	pub fn new_obj(id: u64) -> Self {
		Self(id | Self::OBJ_PRE)
	}
	pub fn new_arr(id: u64) -> Self {
		Self(id | Self::ARR_PRE)
	}

	pub fn decompress(self) -> ValueDec {
		let Self(val) = self;
		if val & Self::NB_PRE != 0 {
			ValueDec::Nb(val & !Self::NB_PRE)
		} else if val & Self::ARR_PRE == 0 {
			ValueDec::Sym(val as ItemId)
		} else if val & Self::ARR_PRE == Self::ARR_PRE {
			ValueDec::Arr(val & !Self::ARR_PRE)
		} else {
			ValueDec::Obj(val & !Self::OBJ_PRE)
		}
	}
	pub fn as_sym(self) -> Option<ItemId> {
		let Self(val) = self;
		(val & Self::PRE == 0).then_some(val as ItemId)
	}
	pub fn as_nb(self) -> Option<u64> {
		let Self(val) = self;
		(val & Self::NB_PRE != 0).then_some(val & !Self::NB_PRE)
	}
	pub fn as_obj(self) -> Option<u64> {
		let Self(val) = self;
		(val & Self::PRE == Self::OBJ_PRE).then_some(val & !Self::OBJ_PRE)
	}
	pub fn as_arr(self) -> Option<u64> {
		let Self(val) = self;
		(val & Self::PRE == Self::ARR_PRE).then_some(val & !Self::ARR_PRE)
	}
	pub fn is_sym(self) -> bool {
		self.as_sym().is_some()
	}
	pub fn is_nb(self) -> bool {
		self.as_nb().is_some()
	}
	pub fn is_obj(self) -> bool {
		self.as_obj().is_some()
	}
	pub fn is_arr(self) -> bool {
		self.as_arr().is_some()
	}

	pub fn is_item(&self) -> bool {
		self.id().is_some()
	}
	pub fn id(self) -> Option<u64> {
		if self.0 & Self::OBJ_PRE != 0 { Some(self.0 & !Self::PRE) } else { None }
	}

	pub fn eq(value1: Value, value2: Value, pool: &ValuePool) -> bool {
		match (value1.decompress(), value2.decompress()) {
			(ValueDec::Nb(nba), ValueDec::Nb(nb2)) => nba == nb2,
			(ValueDec::Sym(id1), ValueDec::Sym(id2)) => id1 == id2,
			(ValueDec::Obj(id1), ValueDec::Obj(id2)) => {
				let obj1 = &pool.obj_pool[id1 as usize];
				let obj2 = &pool.obj_pool[id2 as usize];
				for (field, value1) in obj1 {
					if obj2.get(&field).is_none_or(|value2| !Value::eq(*value1, *value2, pool)) {
						return false;
					}
				}
				obj1.len() == obj2.len()
			}
			(ValueDec::Arr(id1), ValueDec::Arr(id2)) => {
				let arr1 = &pool.arr_pool[id1 as usize];
				let arr2 = &pool.arr_pool[id2 as usize];
				if arr1.len() != arr2.len() {
					return false;
				}
				for (ind, value1) in arr1.iter().enumerate() {
					if !Value::eq(*value1, arr2[ind], pool) {
						return false;
					}
				}
				true
			}
			_ => false,
		}
	}

	pub fn display(&self, pretty: bool, res: &ExecRes, pool: &ValuePool) -> String {
		let mut buf = String::new();
		self.display_item(&mut buf, pretty, 0, res, pool);
		buf
	}
	pub fn display_item(
		self, buf: &mut String, pretty: bool, ident_level: usize, res: &ExecRes, pool: &ValuePool,
	) {
		fn add_ident(buf: &mut String, pretty: bool, ident_level: usize) {
			if pretty {
				buf.push('\n');
				for _ in 0..ident_level {
					buf.push('\t');
				}
			}
		}
		match self.decompress() {
			ValueDec::Nb(nb) => write!(buf, "{nb}").unwrap(),
			ValueDec::Sym(id) => buf.push_str(&res.symbols[id as usize].name.val),
			ValueDec::Arr(id) => {
				let arr = &pool.arr_pool[id as usize];
				if arr.len() == 0 {
					buf.push_str("[]");
					return;
				}
				buf.push('[');
				for item in arr {
					add_ident(buf, pretty, ident_level + 1);
					item.display_item(buf, pretty, ident_level + 1, res, pool);
					buf.push_str(", ");
				}
				if !pretty {
					buf.pop();
					buf.pop();
				}
				add_ident(buf, pretty, ident_level);
				buf.push(']');
			}
			ValueDec::Obj(id) => {
				let obj = &pool.obj_pool[id as usize];
				if obj.len() == 0 {
					buf.push_str("{}");
					return;
				}
				buf.push('{');
				for (field, value) in obj {
					add_ident(buf, pretty, ident_level + 1);
					match field {
						Field::Nb(nb) => write!(buf, "{nb}").unwrap(),
						Field::Symbol(id) => buf.push_str(&res.symbols[*id as usize].name.val),
					}
					buf.push_str(": ");
					value.display_item(buf, pretty, ident_level + 1, res, pool);
					buf.push_str(", ");
				}
				if !pretty {
					buf.pop();
					buf.pop();
				}
				add_ident(buf, pretty, ident_level);
				buf.push('}');
			}
		}
	}
}

pub type Object = FxHashMap<Field, Value>;

trait Clean {
	fn clean(&mut self, shink_full: bool, max_cap: usize);
}
impl Clean for Vec<Value> {
	fn clean(&mut self, shink_full: bool, max_cap: usize) {
		self.clear();
		if shink_full {
			self.shrink_to(0);
		} else if self.capacity() > max_cap {
			self.shrink_to(max_cap);
		}
	}
}
impl Clean for Object {
	fn clean(&mut self, shink_full: bool, max_cap: usize) {
		self.clear();
		if shink_full {
			self.shrink_to(0);
		} else if self.capacity() > max_cap {
			self.shrink_to(max_cap);
		}
	}
}

#[derive(Debug)]
struct Slot<T> {
	cell: UnsafeCell<T>,
	is_active: bool,
	refs: Cell<usize>,
	next_free: usize,
}
#[derive(Debug)]
pub struct TypedPool<T> {
	blocks: Vec<Box<[Slot<T>]>>,
	free_head: usize,
	free_count: usize,
	reserve_head: usize,
	reserve_count: usize,
}
impl<T: Default + Clean> Default for TypedPool<T> {
	fn default() -> Self {
		let mut pool = Self {
			blocks: Vec::new(),
			free_head: usize::MAX,
			free_count: 0,
			reserve_head: usize::MAX,
			reserve_count: 0,
		};
		pool.add_block();
		pool
	}
}
impl<T> TypedPool<T> {
	pub const MAX_CAP: usize = 32;
	pub const MAX_FREE: usize = Self::BLOCK_SIZE;
	pub const BLOCK_SIZE_POW: usize = 12;
	pub const BLOCK_SIZE: usize = 2usize.pow(Self::BLOCK_SIZE_POW as u32);

	fn split_index(index: usize) -> (usize, usize) {
		(index >> Self::BLOCK_SIZE_POW, index & (Self::BLOCK_SIZE - 1))
	}
	pub fn capacity(&self) -> usize {
		self.blocks.len() << Self::BLOCK_SIZE_POW
	}
	pub fn get(&self, index: usize) -> Option<&T> {
		let (high, low) = Self::split_index(index);
		Some(unsafe { &*self.blocks.get(high)?.get(low)?.cell.get() })
	}
	pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
		let (high, low) = Self::split_index(index);
		Some(unsafe { &mut *self.blocks.get(high)?.get(low)?.cell.get() })
	}
	pub fn get_cell(&self, index: usize) -> &UnsafeCell<T> {
		let (high, low) = Self::split_index(index);
		&self.blocks[high][low].cell
	}
}
impl<T: Default + Clean> TypedPool<T> {
	fn add_block(&mut self) {
		let base_ind = self.capacity();
		let mut block = Vec::with_capacity(Self::BLOCK_SIZE);
		for ind in 0..Self::BLOCK_SIZE {
			let slot = Slot {
				cell: UnsafeCell::new(T::default()),
				is_active: false,
				refs: Cell::new(0),
				next_free: self.free_head,
			};
			self.free_head = ind + base_ind;
			block.push(slot);
		}
		self.blocks.push(block.into_boxed_slice());
		self.free_count += Self::BLOCK_SIZE;
	}
	fn clone_value(&self, index: usize) {
		let (high, low) = Self::split_index(index);
		let slot = &self.blocks[high][low];
		slot.refs.update(|refs| refs + 1);
	}
	pub fn alloc(&mut self) -> (&UnsafeCell<T>, usize) {
		if self.free_count == 0 && self.reserve_count == 0 {
			self.add_block();
		}

		let (head, count) = match self.free_count == 0 {
			true => (&mut self.reserve_head, &mut self.reserve_count),
			false => (&mut self.free_head, &mut self.free_count),
		};

		let index = *head;
		let (high, low) = Self::split_index(index);
		let slot = &mut self.blocks[high][low];
		slot.is_active = true;
		slot.refs.update(|refs| refs + 1);

		*head = slot.next_free;
		*count -= 1;

		(&slot.cell, index)
	}
	pub fn free(&mut self, index: usize) {
		let (high, low) = Self::split_index(index);
		let slot = &mut self.blocks[high][low];
		if !slot.is_active {
			panic!("double free");
		}
		slot.refs.update(|refs| refs - 1);
		if slot.refs.get() > 0 {
			return;
		}

		let is_reserve = self.free_count >= Self::MAX_FREE;
		slot.is_active = false;
		unsafe { (*slot.cell.get()).clean(is_reserve, Self::MAX_CAP) };

		let (head, count) = match is_reserve {
			true => (&mut self.reserve_head, &mut self.reserve_count),
			false => (&mut self.free_head, &mut self.free_count),
		};
		slot.next_free = *head;
		*head = index;
		*count += 1;
	}
}

impl<T> Index<usize> for TypedPool<T> {
	type Output = T;
	fn index(&self, index: usize) -> &Self::Output {
		self.get(index).unwrap()
	}
}
impl<T> IndexMut<usize> for TypedPool<T> {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		self.get_mut(index).unwrap()
	}
}

#[derive(Debug)]
pub struct ValuePool {
	pub arr_pool: TypedPool<Vec<Value>>,
	pub obj_pool: TypedPool<Object>,
}
impl ValuePool {
	pub fn clone_value(&self, value: Value) -> Value {
		match value.decompress() {
			ValueDec::Arr(id) => self.arr_pool.clone_value(id as usize),
			ValueDec::Obj(id) => self.obj_pool.clone_value(id as usize),
			_ => (),
		}
		value
	}
	pub fn free_value(&mut self, value: Value) {
		let to_free: Vec<_>;
		match value.decompress() {
			ValueDec::Arr(id) => {
				let id = id as usize;
				to_free = self.arr_pool[id].iter().copied().filter(Value::is_item).collect();
				self.arr_pool.free(id);
			}
			ValueDec::Obj(id) => {
				let id = id as usize;
				to_free = self.obj_pool[id].values().copied().filter(Value::is_item).collect();
				self.obj_pool.free(id);
			}
			_ => return,
		}
		for id in to_free {
			self.free_value(id);
		}
	}
}
