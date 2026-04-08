use std::{
	collections::HashMap,
	fmt::Write,
	ops::{Index, IndexMut},
	usize,
};

use crate::exec::{Execution, ItemId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

	pub fn decompress(&self) -> ValueDec {
		let Self(val) = self;
		if val & Self::NB_PRE != 0 {
			ValueDec::Nb(val & !Self::NB_PRE)
		} else if val & Self::ARR_PRE == 0 {
			ValueDec::Sym(*val as ItemId)
		} else if val & Self::ARR_PRE == Self::ARR_PRE {
			ValueDec::Arr(val & !Self::ARR_PRE)
		} else {
			ValueDec::Obj(val & !Self::OBJ_PRE)
		}
	}
	pub fn as_sym(&self) -> Option<ItemId> {
		let Self(val) = self;
		(val & Self::PRE == 0).then_some(*val as ItemId)
	}
	pub fn as_nb(&self) -> Option<u64> {
		let Self(val) = self;
		(val & Self::NB_PRE != 0).then_some(val & !Self::NB_PRE)
	}
	pub fn as_obj(&self) -> Option<u64> {
		let Self(val) = self;
		(val & Self::PRE == Self::OBJ_PRE).then_some(val & !Self::OBJ_PRE)
	}
	pub fn as_arr(&self) -> Option<u64> {
		let Self(val) = self;
		(val & Self::PRE == Self::ARR_PRE).then_some(val & !Self::ARR_PRE)
	}
	pub fn is_sym(&self) -> bool {
		self.as_sym().is_some()
	}
	pub fn is_nb(&self) -> bool {
		self.as_nb().is_some()
	}
	pub fn is_obj(&self) -> bool {
		self.as_obj().is_some()
	}
	pub fn is_arr(&self) -> bool {
		self.as_arr().is_some()
	}

	pub fn id(&self) -> Option<u64> {
		if self.0 & Self::OBJ_PRE != 0 { Some(self.0 & !Self::PRE) } else { None }
	}

	pub fn display(&self, buf: &mut String, pretty: bool, ident_level: usize, exec: &Execution) {
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
			ValueDec::Sym(id) => buf.push_str(&exec.symbols[id as usize].name.val),
			ValueDec::Arr(id) => {
				let arr = &exec.pool[id as usize];
				if arr.len() == 0 {
					buf.push_str("[]");
					return;
				}
				buf.push('[');
				for item in arr.arr_items() {
					add_ident(buf, pretty, ident_level + 1);
					item.display(buf, pretty, ident_level + 1, exec);
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
				let pbj = &exec.pool[id as usize];
				if pbj.len() == 0 {
					buf.push_str("{}");
					return;
				}
				buf.push('{');
				for (key, value) in pbj.obj_items() {
					add_ident(buf, pretty, ident_level + 1);
					buf.push_str(&exec.symbols[key as usize].name.val);
					buf.push_str(": ");
					value.display(buf, pretty, ident_level + 1, exec);
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

const MAX_INLINE: usize = 12;
#[derive(Debug)]
pub enum ValueUnit {
	Inline { len: u8, items: [Value; MAX_INLINE] },
	Arr(Vec<Value>),
	Obj(HashMap<ItemId, Value>),
}
impl ValueUnit {
	pub fn len(&self) -> usize {
		match self {
			Self::Inline { len, .. } => *len as usize,
			Self::Arr(arr) => arr.len(),
			Self::Obj(obj) => obj.len(),
		}
	}
	pub fn map_get(&self, key: ItemId) -> Option<&Value> {
		match self {
			Self::Obj(map) => map.get(&key),
			Self::Inline { len, items } => {
				for ind in 0..*len as usize / 2 {
					if items[ind * 2].as_sym() == Some(key) {
						return Some(&items[ind * 2 + 1]);
					}
				}
				None
			}
			_ => unreachable!(),
		}
	}
	pub fn arr_get(&self, index: usize) -> Option<&Value> {
		match self {
			Self::Arr(arr) => arr.get(index),
			Self::Inline { len, items } => (index < *len as usize).then(|| &items[index]),
			_ => unreachable!(),
		}
	}
	pub fn insert(&mut self, key: ItemId, value: Value) {
		match self {
			Self::Inline { len, items } => {
				let lenn = *len as usize;
				if lenn != MAX_INLINE {
					items[lenn] = Value::new_sym(key);
					items[lenn + 1] = value;
					*len += 2;
				} else {
					let mut map = HashMap::with_capacity(MAX_INLINE / 2 + 1);
					for ind in 0..MAX_INLINE / 2 {
						map.insert(items[ind * 2].as_sym().unwrap(), items[ind * 2 + 1].clone());
					}
					map.insert(key, value);
					*self = Self::Obj(map)
				}
			}
			Self::Obj(map) => {
				map.insert(key, value);
			}
			_ => unreachable!(),
		}
	}
	pub fn push(&mut self, value: Value) {
		match self {
			Self::Inline { len, items } => {
				let lenn = *len as usize;
				if lenn != MAX_INLINE {
					items[lenn] = value;
					*len += 1;
				} else {
					let mut arr = Vec::with_capacity(MAX_INLINE + 1);
					for ind in 0..MAX_INLINE {
						arr.push(items[ind].clone());
					}
					arr.push(value);
					*self = Self::Arr(arr)
				}
			}
			Self::Arr(arr) => arr.push(value),
			_ => unreachable!(),
		}
	}
	pub fn arr_items(&self) -> impl Iterator<Item = &Value> {
		match self {
			Self::Inline { len, items } => items[0..*len as usize].iter(),
			Self::Arr(arr) => arr.iter(),
			_ => unreachable!(),
		}
	}
	pub fn obj_items(&self) -> impl Iterator<Item = (ItemId, &Value)> {
		enum Iter<'a> {
			Inline(std::slice::Iter<'a, Value>),
			Map(std::collections::hash_map::Iter<'a, ItemId, Value>),
		}
		let mut iter = match self {
			Self::Inline { len, items } => Iter::Inline(items[0..*len as usize].iter()),
			Self::Obj(obj) => Iter::Map(obj.iter()),
			_ => unreachable!(),
		};
		std::iter::from_fn(move || match &mut iter {
			Iter::Map(iter) => iter.next().map(|(k, v)| (*k, v)),
			Iter::Inline(iter) => iter.next().map(|k| (k.as_sym().unwrap(), iter.next().unwrap())),
		})
	}
}
impl Default for ValueUnit {
	fn default() -> Self {
		ValueUnit::Inline { len: 0, items: [Value::DUMMY; MAX_INLINE] }
	}
}

#[derive(Debug)]
struct Slot {
	unit: ValueUnit,
	is_active: bool,
	next_free: usize,
}

#[derive(Debug)]
pub struct ValuePool {
	blocks: Vec<Box<[Slot]>>,
	free_head: usize,
	free_count: usize,
}
impl Default for ValuePool {
	fn default() -> Self {
		let mut pool = Self { blocks: Vec::new(), free_head: usize::MAX, free_count: 0 };
		pool.add_block();
		pool
	}
}
impl ValuePool {
	pub const MAX_CAP: usize = 32;
	pub const MAX_FREE: usize = Self::BLOCK_SIZE;
	pub const BLOCK_SIZE_POW: usize = 12;
	pub const BLOCK_SIZE: usize = 2usize.pow(Self::BLOCK_SIZE_POW as u32);

	fn split_index(index: usize) -> (usize, usize) {
		(index >> Self::BLOCK_SIZE_POW, index & (Self::BLOCK_SIZE - 1))
	}
	pub fn capacity(&self) -> usize {
		self.blocks.len() >> Self::BLOCK_SIZE_POW
	}
	pub fn get(&self, index: usize) -> Option<&ValueUnit> {
		let (high, low) = Self::split_index(index);
		Some(&self.blocks.get(high)?.get(low)?.unit)
	}
	pub fn get_mut(&mut self, index: usize) -> Option<&mut ValueUnit> {
		let (high, low) = Self::split_index(index);
		Some(&mut self.blocks.get_mut(high)?.get_mut(low)?.unit)
	}
	fn add_block(&mut self) {
		let mut block = Vec::with_capacity(Self::BLOCK_SIZE);
		for ind in 0..Self::BLOCK_SIZE {
			let slot =
				Slot { unit: ValueUnit::default(), is_active: false, next_free: self.free_head };
			self.free_head = ind + self.capacity();
			block.push(slot);
		}
		self.blocks.push(block.into_boxed_slice());
		self.free_count += Self::BLOCK_SIZE;
	}
	fn alloc(&mut self) -> (&mut Slot, usize) {
		if self.free_count == 0 {
			self.add_block();
		}
		let index = self.free_head;
		let (high, low) = Self::split_index(index);
		let slot = &mut self.blocks[high][low];
		slot.is_active = true;
		self.free_head = slot.next_free;
		(slot, index)
	}
	fn free(&mut self, index: usize) {
		let (high, low) = Self::split_index(index);
		let slot = &mut self.blocks[high][low];
		slot.is_active = false;
		slot.next_free = self.free_head;
		self.free_head = index;
		self.free_count += 1;
		let mut to_free = Vec::new();
		match &mut slot.unit {
			ValueUnit::Inline { len, items } => {
				for ind in 0..*len {
					if let Some(id) = items[ind as usize].id() {
						to_free.push(id)
					}
				}
				*len = 0
			}
			ValueUnit::Arr(arr) => {
				for item in &*arr {
					if let Some(id) = item.id() {
						to_free.push(id)
					}
				}
				arr.clear();
				if self.free_count >= Self::MAX_FREE {
					arr.shrink_to(0);
				} else if arr.capacity() > Self::MAX_CAP {
					arr.shrink_to(Self::MAX_CAP);
				}
			}
			ValueUnit::Obj(obj) => {
				for item in obj.values() {
					if let Some(id) = item.id() {
						to_free.push(id)
					}
				}
				obj.clear();
				if self.free_count >= Self::MAX_FREE {
					obj.shrink_to(0);
				} else if obj.capacity() > Self::MAX_CAP / 2 {
					obj.shrink_to(Self::MAX_CAP / 2);
				}
			}
		}
		for item in to_free {
			self.free(item as usize);
		}
	}
}

impl Index<usize> for ValuePool {
	type Output = ValueUnit;
	fn index(&self, index: usize) -> &Self::Output {
		self.get(index).unwrap()
	}
}
impl IndexMut<usize> for ValuePool {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		self.get_mut(index).unwrap()
	}
}
