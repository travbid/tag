use super::mp4;

use mp4::BaseBox;

pub enum Atom {
	Meaning(MeaningBox),
}

// data
struct _DataBox {
	pub base: BaseBox,
	locale: u32, // = 0
	value: Vec<u8>,
}

// ilist
struct _MetaItemsBox {
	pub base: BaseBox,
	items: Vec<_MetaItemBox>,
}

// ----
struct _MetaItemBox {
	pub base: BaseBox,
	extended_meaning: MeaningBox,
	// name: NameBox, // optional
	values: Vec<_DataBox>,
}

pub struct MeaningBox {}

struct _TypeIndicator {
	reserved: u16,           // = 0
	type_set_identifier: u8, // = 0,
	type_code: u8,
}
