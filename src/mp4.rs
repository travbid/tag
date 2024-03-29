use core::convert::TryInto;

use super::itunes;

fn spacer(depth: u16) -> String {
	let mut ret = Vec::<u8>::new();
	ret.resize((depth * 2u16) as usize, ' ' as u8);
	String::from_utf8(ret).unwrap()
}

const fn array_str(s: &str) -> [u8; 4] {
	let b = s.as_bytes();
	[b[0] as u8, b[1] as u8, b[2] as u8, b[3] as u8]
}

pub struct BaseBox {
	pub size: u32,
	pub boxtype: [u8; 4],
}

pub struct FullBox {
	pub base: BaseBox,
	pub version: u8,
	pub flags: [u8; 3],
}

// ftyp
pub struct FileTypeBox {
	pub base: BaseBox,
	pub major_brand: u32,
	pub minor_version: u32,
	pub compatible_brands: Vec<[u8; 4]>,
}

impl FileTypeBox {
	pub fn parse(sz: u32, data: &[u8]) -> FileTypeBox {
		let mut compatible_brands = Vec::new();
		let mut ix: usize = 16;
		while ix < sz as usize {
			compatible_brands.push([data[ix + 0], data[ix + 1], data[ix + 2], data[ix + 3]]);
			ix += 32;
		}
		FileTypeBox {
			base: BaseBox {
				size: sz,
				boxtype: ['f' as u8, 't' as u8, 'y' as u8, 'p' as u8],
			},
			major_brand: u32::from_be_bytes(data[0..4].try_into().unwrap()),
			minor_version: u32::from_be_bytes(data[4..8].try_into().unwrap()),
			compatible_brands,
		}
	}

	pub fn string(&self, depth: u16) -> String {
		let mut ret = String::from("ftyp: {\n");
		ret += &(spacer(depth + 1) + "major_brand: " + &self.major_brand.to_string() + ",\n");
		ret += &(spacer(depth + 1) + "minor_version: " + &self.minor_version.to_string() + ",\n");
		ret += &(spacer(depth + 1) + "compatible_brands: [");
		if self.compatible_brands.is_empty() {
			ret += "]\n";
		} else {
			ret += "\n";
			for brand in &self.compatible_brands {
				let name = match String::from_utf8(brand[0..4].to_vec()) {
					Ok(x) => x,
					Err(e) => {
						eprintln!("{}", e);
						let mut s = String::from("[");
						for b in brand {
							s += &(" ".to_owned() + &b.to_string());
						}
						s += "]";
						s
					}
				};
				ret += &(spacer(depth + 2) + &name + ",\n");
			}
			ret += &(spacer(depth + 1) + "]\n");
		}
		ret += &(spacer(depth) + "}");
		ret
	}
}

// moov
pub struct MovieBox {
	pub base: BaseBox,
	pub children: Vec<MovieAtom>,
}

impl MovieBox {
	pub fn parse(sz: u32, data: &[u8]) -> MovieBox {
		println!("MovieBox::parse({}, {})", sz, data.len());
		let mut children = Vec::new();
		let mut ix: usize = 0;
		while ix < sz as usize - 8 {
			let inner_sz = u32::from_be_bytes(data[ix + 0..ix + 4].try_into().unwrap());
			let name = match std::str::from_utf8(&data[ix + 4..ix + 8]) {
				Ok(x) => x,
				Err(e) => panic!("from_utf8: {} {} {:?}", e, ix, &data[ix + 4..ix + 8]),
			};
			let inner_data = &data[ix + 8..ix + inner_sz as usize];
			let child = match name {
				// "ipmc" => Atom::IPMPControl(IPMPControlBox::parse(inner_data)),
				"mvhd" => MovieAtom::MovieHeader(MovieHeaderBox::parse(inner_sz, inner_data)),
				"trak" => MovieAtom::Track(TrackBox::parse(inner_sz, inner_data)),
				"mvex" => MovieAtom::MovieExtends(MovieExtendsBox::parse(inner_sz, inner_data)),
				"meta" => MovieAtom::Meta(MetaBox::parse(inner_sz, inner_data)),
				"udta" => MovieAtom::UserData(UserDataBox::parse(inner_sz, inner_data)),
				_ => panic!("Undhandled type in moov: {}, {:?}", name, &data[ix + 4..ix + 8]),
			};
			children.push(child);
			ix += inner_sz as usize;
		}
		MovieBox {
			base: BaseBox {
				size: sz,
				boxtype: array_str("moov"),
			},
			children,
		}
	}

	pub fn string(&self, depth: u16) -> String {
		let mut ret = String::from("moov: {\n");
		ret += &(spacer(depth + 1) + "children: [");
		if self.children.is_empty() {
			ret += "]\n";
		} else {
			ret += "\n";
			for child in &self.children {
				ret += &(spacer(depth + 2) + &child.string(depth + 2) + ",\n");
			}
			ret += &(spacer(depth + 1) + "]\n");
		}
		ret += &(spacer(depth) + "}");
		ret
	}
}

pub struct MovieExtendsBox {
	pub base: BaseBox,
	pub children: Vec<MovieExtendsAtom>,
}

impl MovieExtendsBox {
	fn parse(sz: u32, data: &[u8]) -> MovieExtendsBox {
		println!("MovieExtendsBox::parse({}, {})", sz, data.len());
		let mut children = Vec::new();
		let mut ix: usize = 16;
		while ix < sz as usize {
			let inner_sz = u32::from_be_bytes(data[0..4].try_into().unwrap());
			let name = std::str::from_utf8(&data[4..8]).unwrap();
			let inner_data = &data[8..inner_sz as usize];
			let child = match name {
				_ => panic!("Undhandled type in mvex: {}, {:?}", name, &data[4..8]),
			};
			children.push(child);
			ix += inner_sz as usize;
		}
		MovieExtendsBox {
			base: BaseBox {
				size: sz,
				boxtype: array_str("mvex"),
			},
			children,
		}
	}
}

// mvhd
pub struct MovieHeaderBox {
	pub base: FullBox, // flags = 7 (Track_enabled, Track_in_movie, Track_in_preview)

	pub creation_time: u64,
	pub modification_time: u64,
	pub timescale: u32, // 44,100
	pub duration: u64,  // seconds * 44100

	pub rate: u32,             // = 0x0001_0000 (1.0)
	pub volume: u16,           // = 0x01_00 (1.0)
	reserved1: u16,            // = 0
	reserved2: [u32; 2],       // = 0
	pub matrix: [u32; 9],      // = { 0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000 };
	pub pre_defined: [u32; 6], // = 0
	pub next_track_id: u32,
}

impl MovieHeaderBox {
	fn parse(sz: u32, data: &[u8]) -> MovieHeaderBox {
		println!("MovieHeaderBox::parse({}, {})", sz, data.len());
		let version = data[0];
		if version != 0 && version != 1 {
			panic!("mvhd version must be 0 or 1");
		}
		let creation_time;
		let modification_time;
		let timescale;
		let duration;
		let off;
		if version == 0 {
			creation_time = u32::from_be_bytes(data[4..8].try_into().unwrap()) as u64;
			modification_time = u32::from_be_bytes(data[8..12].try_into().unwrap()) as u64;
			timescale = u32::from_be_bytes(data[12..16].try_into().unwrap());
			duration = u32::from_be_bytes(data[16..20].try_into().unwrap()) as u64;
			off = 20;
		} else {
			creation_time = u64::from_be_bytes(data[4..12].try_into().unwrap());
			modification_time = u64::from_be_bytes(data[12..20].try_into().unwrap());
			timescale = u32::from_be_bytes(data[20..24].try_into().unwrap());
			duration = u64::from_be_bytes(data[24..32].try_into().unwrap());
			off = 32;
		};

		MovieHeaderBox {
			base: FullBox {
				base: BaseBox {
					size: sz,
					boxtype: array_str("mvhd"),
				},
				version,
				flags: [data[1], data[2], data[3]],
			},
			creation_time,
			modification_time,
			timescale,
			duration,

			rate: u32::from_be_bytes(data[off..off + 4].try_into().unwrap()),
			volume: u16::from_be_bytes(data[off + 4..off + 6].try_into().unwrap()),
			reserved1: u16::from_be_bytes(data[off + 6..off + 8].try_into().unwrap()),
			reserved2: [
				u32::from_be_bytes(data[off + 8..off + 12].try_into().unwrap()),
				u32::from_be_bytes(data[off + 12..off + 16].try_into().unwrap()),
			],
			matrix: [
				u32::from_be_bytes(data[off + 16..off + 20].try_into().unwrap()),
				u32::from_be_bytes(data[off + 20..off + 24].try_into().unwrap()),
				u32::from_be_bytes(data[off + 24..off + 28].try_into().unwrap()),
				u32::from_be_bytes(data[off + 28..off + 32].try_into().unwrap()),
				u32::from_be_bytes(data[off + 32..off + 36].try_into().unwrap()),
				u32::from_be_bytes(data[off + 36..off + 40].try_into().unwrap()),
				u32::from_be_bytes(data[off + 40..off + 44].try_into().unwrap()),
				u32::from_be_bytes(data[off + 44..off + 48].try_into().unwrap()),
				u32::from_be_bytes(data[off + 48..off + 52].try_into().unwrap()),
			],
			pre_defined: [
				u32::from_be_bytes(data[off + 52..off + 56].try_into().unwrap()),
				u32::from_be_bytes(data[off + 56..off + 60].try_into().unwrap()),
				u32::from_be_bytes(data[off + 60..off + 64].try_into().unwrap()),
				u32::from_be_bytes(data[off + 64..off + 68].try_into().unwrap()),
				u32::from_be_bytes(data[off + 68..off + 72].try_into().unwrap()),
				u32::from_be_bytes(data[off + 72..off + 76].try_into().unwrap()),
			],
			next_track_id: u32::from_be_bytes(data[off + 76..off + 80].try_into().unwrap()),
		}
	}

	pub fn string(&self, depth: u16) -> String {
		let mut ret = String::from("mvhd: {\n");
		// chrono::
		ret += &(spacer(depth + 1) + "creation_time: " + &self.creation_time.to_string());

		ret
	}
}

// trak
pub struct TrackBox {
	pub base: BaseBox,
	pub children: Vec<TrackAtom>,
}

impl TrackBox {
	fn parse(sz: u32, data: &[u8]) -> TrackBox {
		println!("TrackBox::parse({}, {})", sz, data.len());
		let mut children = Vec::new();
		let mut ix: usize = 0;
		while ix < sz as usize - 8 {
			let inner_sz = u32::from_be_bytes(data[ix + 0..ix + 4].try_into().unwrap());
			let name = match std::str::from_utf8(&data[ix + 4..ix + 8]) {
				Ok(x) => x,
				Err(e) => panic!("from_utf8: {} {} {:?}", e, ix, &data[ix + 4..ix + 8]),
			};
			let inner_data = &data[ix + 8..ix + inner_sz as usize];
			let child = match name {
				"tkhd" => TrackAtom::TrackHeader(TrackHeaderBox::parse(inner_sz, inner_data)),
				"mdia" => TrackAtom::Media(MediaBox::parse(inner_sz, inner_data)),
				_ => panic!("Undhandled type in trak: {}, {:?}", name, &data[ix + 4..ix + 8]),
			};
			children.push(child);
			ix += inner_sz as usize;
		}
		TrackBox {
			base: BaseBox {
				size: sz,
				boxtype: array_str("trak"),
			},
			children,
		}
	}
}

// tkhd
pub struct TrackHeaderBox {
	pub base: FullBox, // flags = 7 (Track_enabled, Track_in_movie, Track_in_preview)

	pub creation_time: u64,
	pub modification_time: u64,
	pub track_id: u32,
	reserved1: u32,    // = 0
	pub duration: u64, // seconds * 44100

	reserved2: [u32; 2],
	pub layer: u16,           // = 0
	pub alternate_group: u16, // = 0,
	pub volume: u16,          // {if track_is_audio 0x0100 else 0};
	reserved3: u16,           // = 0
	pub matrix: [u32; 9],     // = { 0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000 };
	pub width: u32,
	pub height: u32,
}

impl TrackHeaderBox {
	pub fn parse(sz: u32, data: &[u8]) -> TrackHeaderBox {
		println!("TrackHeaderBox::parse({}, {})", sz, data.len());
		let version = data[0];
		if version != 0 && version != 1 {
			panic!("tkhd version must be 0 or 1");
		}
		let creation_time;
		let modification_time;
		let track_id;
		let reserved1;
		let duration;
		let off;
		if version == 0 {
			creation_time = u32::from_be_bytes(data[4..8].try_into().unwrap()) as u64;
			modification_time = u32::from_be_bytes(data[8..12].try_into().unwrap()) as u64;
			track_id = u32::from_be_bytes(data[12..16].try_into().unwrap());
			reserved1 = u32::from_be_bytes(data[16..20].try_into().unwrap());
			duration = u32::from_be_bytes(data[20..24].try_into().unwrap()) as u64;
			off = 24;
		} else {
			creation_time = u64::from_be_bytes(data[4..12].try_into().unwrap());
			modification_time = u64::from_be_bytes(data[12..20].try_into().unwrap());
			track_id = u32::from_be_bytes(data[20..24].try_into().unwrap());
			reserved1 = u32::from_be_bytes(data[24..28].try_into().unwrap());
			duration = u64::from_be_bytes(data[28..36].try_into().unwrap());
			off = 36;
		};

		TrackHeaderBox {
			base: FullBox {
				base: BaseBox {
					size: sz,
					boxtype: array_str("tkhd"),
				},
				version,
				flags: [data[1], data[2], data[3]],
			},
			creation_time,
			modification_time,
			track_id,
			reserved1,
			duration,

			reserved2: [
				u32::from_be_bytes(data[off..off + 4].try_into().unwrap()),
				u32::from_be_bytes(data[off + 4..off + 8].try_into().unwrap()),
			],

			layer: u16::from_be_bytes(data[off + 8..off + 10].try_into().unwrap()),
			alternate_group: u16::from_be_bytes(data[off + 10..off + 12].try_into().unwrap()),
			volume: u16::from_be_bytes(data[off + 12..off + 14].try_into().unwrap()),
			reserved3: u16::from_be_bytes(data[off + 14..off + 16].try_into().unwrap()),

			matrix: [
				u32::from_be_bytes(data[off + 16..off + 20].try_into().unwrap()),
				u32::from_be_bytes(data[off + 20..off + 24].try_into().unwrap()),
				u32::from_be_bytes(data[off + 24..off + 28].try_into().unwrap()),
				u32::from_be_bytes(data[off + 28..off + 32].try_into().unwrap()),
				u32::from_be_bytes(data[off + 32..off + 36].try_into().unwrap()),
				u32::from_be_bytes(data[off + 36..off + 40].try_into().unwrap()),
				u32::from_be_bytes(data[off + 40..off + 44].try_into().unwrap()),
				u32::from_be_bytes(data[off + 44..off + 48].try_into().unwrap()),
				u32::from_be_bytes(data[off + 48..off + 52].try_into().unwrap()),
			],

			width: u32::from_be_bytes(data[off + 52..off + 56].try_into().unwrap()),
			height: u32::from_be_bytes(data[off + 56..off + 60].try_into().unwrap()),
		}
	}
}

// mdia
pub struct MediaBox {
	pub base: BaseBox,
	pub children: Vec<MediaAtom>,
}

impl MediaBox {
	fn parse(sz: u32, data: &[u8]) -> MediaBox {
		println!("MediaBox::parse({}, {})", sz, data.len());
		let mut children = Vec::new();
		let mut ix: usize = 0;
		while ix < sz as usize - 8 {
			let inner_sz = u32::from_be_bytes(data[ix + 0..ix + 4].try_into().unwrap());
			let name = match std::str::from_utf8(&data[ix + 4..ix + 8]) {
				Ok(x) => x,
				Err(e) => panic!("from_utf8: {} {} {:?}", e, ix, &data[ix + 4..ix + 8]),
			};
			let inner_data = &data[ix + 8..ix + inner_sz as usize];
			let child = match name {
				"mdhd" => MediaAtom::MediaHeader(MediaHeaderBox::parse(inner_sz, inner_data)),
				"hdlr" => MediaAtom::Handler(HandlerBox::parse(inner_sz, inner_data)),
				"minf" => MediaAtom::MediaInformation(MediaInformationBox::parse(inner_sz, inner_data)),
				_ => panic!("Undhandled type in mdia: {}, {:?}", name, &data[ix + 4..ix + 8]),
			};
			children.push(child);
			ix += inner_sz as usize;
		}
		MediaBox {
			base: BaseBox {
				size: sz,
				boxtype: array_str("mdia"),
			},
			children,
		}
	}
}

// mdhd
pub struct MediaHeaderBox {
	pub base: FullBox,

	pub creation_time: u64,
	pub modification_time: u64,
	pub timescale: u32, // 44,100
	pub duration: u64,  // seconds * 44100

	pub language: u16,
	pub pre_defined: u16, // = 0
}

impl MediaHeaderBox {
	pub fn parse(sz: u32, data: &[u8]) -> MediaHeaderBox {
		println!("MediaHeaderBox::parse({}, {})", sz, data.len());
		let version = data[0];
		if version != 0 && version != 1 {
			panic!("mvhd version must be 0 or 1");
		}
		let creation_time;
		let modification_time;
		let timescale;
		let duration;
		let off;
		if version == 0 {
			creation_time = u32::from_be_bytes(data[4..8].try_into().unwrap()) as u64;
			modification_time = u32::from_be_bytes(data[8..12].try_into().unwrap()) as u64;
			timescale = u32::from_be_bytes(data[12..16].try_into().unwrap());
			duration = u32::from_be_bytes(data[16..20].try_into().unwrap()) as u64;
			off = 20;
		} else {
			creation_time = u64::from_be_bytes(data[4..12].try_into().unwrap());
			modification_time = u64::from_be_bytes(data[12..20].try_into().unwrap());
			timescale = u32::from_be_bytes(data[20..24].try_into().unwrap());
			duration = u64::from_be_bytes(data[24..32].try_into().unwrap());
			off = 32;
		};

		MediaHeaderBox {
			base: FullBox {
				base: BaseBox {
					size: sz,
					boxtype: array_str("mvhd"),
				},
				version,
				flags: [data[1], data[2], data[3]],
			},
			creation_time,
			modification_time,
			timescale,
			duration,

			language: u16::from_be_bytes(data[off + 0..off + 2].try_into().unwrap()),
			pre_defined: u16::from_be_bytes(data[off + 2..off + 4].try_into().unwrap()),
		}
	}
}

// hdlr
pub struct HandlerBox {
	pub base: FullBox,

	pub pre_defined: u32,
	pub handler_type: [u8; 4],
	pub reserved: [u32; 3], // = 0
	pub name: String,
}

impl HandlerBox {
	fn parse(sz: u32, data: &[u8]) -> HandlerBox {
		HandlerBox {
			base: FullBox {
				base: BaseBox {
					size: sz,
					boxtype: array_str("hdlr"),
				},
				version: data[0],
				flags: [data[1], data[2], data[3]],
			},
			pre_defined: u32::from_be_bytes(data[4..8].try_into().unwrap()),
			handler_type: [data[8], data[9], data[10], data[11]],
			reserved: [
				u32::from_be_bytes(data[12..16].try_into().unwrap()),
				u32::from_be_bytes(data[16..20].try_into().unwrap()),
				u32::from_be_bytes(data[20..24].try_into().unwrap()),
			],
			name: std::str::from_utf8(&data[24..]).unwrap().to_string(),
		}
	}
}

// minf
pub struct MediaInformationBox {
	pub base: BaseBox,
	pub children: Vec<MediaInformationAtom>,
}

impl MediaInformationBox {
	fn parse(sz: u32, data: &[u8]) -> MediaInformationBox {
		println!("MediaInformationBox::parse({}, {})", sz, data.len());
		let mut children = Vec::new();
		let mut ix: usize = 0;
		while ix < sz as usize - 8 {
			let inner_sz = u32::from_be_bytes(data[ix + 0..ix + 4].try_into().unwrap());
			let name = match std::str::from_utf8(&data[ix + 4..ix + 8]) {
				Ok(x) => x,
				Err(e) => panic!("from_utf8: {} {} {:?}", e, ix, &data[ix + 4..ix + 8]),
			};
			let inner_data = &data[ix + 8..ix + inner_sz as usize];
			let child = match name {
				"smhd" => MediaInformationAtom::SoundMediaHeader(SoundMediaHeaderBox::parse(inner_sz, inner_data)),
				"dinf" => MediaInformationAtom::DataInformation(DataInformationBox::parse(inner_sz, inner_data)),
				"stbl" => MediaInformationAtom::SampleTable(SampleTableBox::parse(inner_sz, inner_data)),
				_ => panic!("Undhandled type in minf: {}, {:?}", name, &data[ix + 4..ix + 8]),
			};
			children.push(child);
			ix += inner_sz as usize;
		}
		MediaInformationBox {
			base: BaseBox {
				size: sz,
				boxtype: array_str("minf"),
			},
			children,
		}
	}
}

//smhd
pub struct SoundMediaHeaderBox {
	pub base: FullBox,
	pub balance: u16, // = 0;
	reserved: u16,    // = 0;
}

impl SoundMediaHeaderBox {
	fn parse(sz: u32, data: &[u8]) -> SoundMediaHeaderBox {
		SoundMediaHeaderBox {
			base: FullBox {
				base: BaseBox {
					size: sz,
					boxtype: array_str("smhd"),
				},
				version: data[0],
				flags: [data[1], data[2], data[3]],
			},
			balance: u16::from_be_bytes(data[4..6].try_into().unwrap()),
			reserved: u16::from_be_bytes(data[6..8].try_into().unwrap()),
		}
	}
}

// dinf
pub struct DataInformationBox {
	pub base: BaseBox,
	pub children: Vec<Atom>,
}

impl DataInformationBox {
	fn parse(sz: u32, data: &[u8]) -> DataInformationBox {
		println!("DataInformationBox::parse({}, {})", sz, data.len());
		let mut children = Vec::new();
		let mut ix: usize = 0;
		while ix < sz as usize - 8 {
			let inner_sz = u32::from_be_bytes(data[ix + 0..ix + 4].try_into().unwrap());
			let name = match std::str::from_utf8(&data[ix + 4..ix + 8]) {
				Ok(x) => x,
				Err(e) => panic!("from_utf8: {} {} {:?}", e, ix, &data[ix + 4..ix + 8]),
			};
			let inner_data = &data[ix + 8..ix + inner_sz as usize];
			let child = match name {
				"dref" => Atom::DataReference(DataReferenceBox::parse(inner_sz, inner_data)),
				_ => panic!("Undhandled type in dinf: {}, {:?}", name, &data[ix + 4..ix + 8]),
			};
			children.push(child);
			ix += inner_sz as usize;
		}
		DataInformationBox {
			base: BaseBox {
				size: sz,
				boxtype: array_str("dinf"),
			},
			children,
		}
	}
}

// dref
pub struct DataReferenceBox {
	pub base: FullBox,
	pub entry_count: u32,
	pub entries: Vec<DataEntryBox>,
}

impl DataReferenceBox {
	pub fn parse(sz: u32, data: &[u8]) -> DataReferenceBox {
		println!("DataReferenceBox::parse({}, {})", sz, data.len());
		let entry_count = u32::from_be_bytes(data[4..8].try_into().unwrap());
		let mut entries = Vec::<DataEntryBox>::new();
		let mut ix = 0;
		for _ in 0..entry_count {
			let d = DataEntryBox::parse(&data[8 + (ix)..]);
			ix += d.len() as usize;
			entries.push(d);
		}
		DataReferenceBox {
			base: FullBox {
				base: BaseBox {
					size: sz,
					boxtype: array_str("dref"),
				},
				version: data[0],
				flags: [data[1], data[2], data[3]],
			},
			entry_count,
			entries,
		}
	}
}

pub enum DataEntryBox {
	Url(DataEntryUrlBox),
	Urn(DataEntryUrnBox),
}

impl DataEntryBox {
	fn parse(data: &[u8]) -> DataEntryBox {
		println!("DataEntryBox::parse({})", data.len());
		let sz = u32::from_be_bytes(data[0..4].try_into().unwrap());
		let name = std::str::from_utf8(&data[4..8]).unwrap();
		let version = data[8];
		let flags = [data[9], data[10], data[11]];
		let base = FullBox {
			base: BaseBox {
				size: sz,
				boxtype: array_str(name),
			},
			version,
			flags,
		};
		if name == "urn " {
			DataEntryBox::Urn(DataEntryUrnBox {
				base,
				name: String::new(),
				location: String::new(),
			})
		} else if name == "url " {
			DataEntryBox::Url(DataEntryUrlBox {
				base,
				location: if flags[2] == 1 {
					String::new()
				} else {
					std::str::from_utf8(&data[16..]).unwrap().to_string()
				},
			})
		} else {
			panic!("Unhandled DataEntryBox type: {} {:?}", name, &data[8..12]);
		}
	}
	fn len(&self) -> u32 {
		match self {
			DataEntryBox::Url(x) => x.base.base.size,
			DataEntryBox::Urn(x) => x.base.base.size,
		}
	}
}

// url
pub struct DataEntryUrlBox {
	pub base: FullBox, // flags = 1
	pub location: String,
}

// urn
pub struct DataEntryUrnBox {
	pub base: FullBox,
	pub name: String,
	pub location: String,
}

// stbl
pub struct SampleTableBox {
	pub base: BaseBox,
	pub children: Vec<SampleTableAtom>,
}

impl SampleTableBox {
	fn parse(sz: u32, data: &[u8]) -> SampleTableBox {
		println!("SampleTableBox::parse({}, {})", sz, data.len());
		let mut children = Vec::new();
		let mut ix: usize = 0;
		while ix < sz as usize - 8 {
			let inner_sz = u32::from_be_bytes(data[ix + 0..ix + 4].try_into().unwrap());
			let name = match std::str::from_utf8(&data[ix + 4..ix + 8]) {
				Ok(x) => x,
				Err(e) => panic!("from_utf8: {} {} {:?}", e, ix, &data[ix + 4..ix + 8]),
			};
			let inner_data = &data[ix + 8..ix + inner_sz as usize];
			let child = match name {
				"stsd" => SampleTableAtom::SampleDescription(SampleDescriptionBox::parse(inner_sz, inner_data)),
				"stts" => SampleTableAtom::TimeToSample(TimeToSampleBox::parse(inner_sz, inner_data)),
				"stsc" => SampleTableAtom::SampleToChunk(SampleToChunkBox::parse(inner_sz, inner_data)),
				"stsz" => SampleTableAtom::SampleSize(SampleSizeBox::parse(inner_sz, inner_data)),
				"stco" => SampleTableAtom::ChunkOffset(ChunkOffsetBox::parse(inner_sz, inner_data)),
				_ => panic!("Undhandled type in stbl: {}, {:?}", name, &data[ix + 4..ix + 8]),
			};
			children.push(child);
			ix += inner_sz as usize;
		}
		SampleTableBox {
			base: BaseBox {
				size: sz,
				boxtype: array_str("stbl"),
			},
			children,
		}
	}
}

// stsd
pub struct SampleDescriptionBox {
	pub base: FullBox,
	pub entry_count: u32,
	pub entries: Vec<SampleEntryEnum>,
}

impl SampleDescriptionBox {
	fn parse(sz: u32, data: &[u8]) -> SampleDescriptionBox {
		println!("SampleDescriptionBox::parse({}, {})", sz, data.len());
		let entry_count = u32::from_be_bytes(data[4..8].try_into().unwrap());
		let mut entries = Vec::<SampleEntryEnum>::new();
		let mut ix = 0;
		for _ in 0..entry_count {
			let sz2 = u32::from_be_bytes(data[8 + ix..12 + ix].try_into().unwrap());
			let name: [u8; 4] = data[12 + ix..16 + ix].try_into().unwrap();
			const VIDE: [u8; 4] = array_str("vide");
			const SOUN: [u8; 4] = array_str("soun");
			const HINT: [u8; 4] = array_str("hint");
			match name {
				VIDE => entries.push(SampleEntryEnum::Visual(VisualSampleEntry::parse(sz2, &data[16 + ix..]))),
				SOUN => entries.push(SampleEntryEnum::Audio(AudioSampleEntry::parse(sz2, &data[16 + ix..]))),
				HINT => entries.push(SampleEntryEnum::Hint(HintSampleEntry::parse(sz2, &data[16 + ix..]))),
				_ => {
					println!(
						"Unknown SampleEntry Type: {:?} {}",
						name,
						String::from_utf8(name.to_vec()).unwrap_or("????".to_string())
					);
					entries.push(SampleEntryEnum::Unknown(UnknownBox::parse(sz2, name, &data[16 + ix..])));
				}
			};
			ix += sz2 as usize;
		}
		SampleDescriptionBox {
			base: FullBox {
				base: BaseBox {
					size: sz,
					boxtype: array_str("stsd"),
				},
				version: data[0],
				flags: [data[1], data[2], data[3]],
			},
			entry_count,
			entries,
		}
	}
}

pub enum SampleEntryEnum {
	Hint(HintSampleEntry),
	Visual(VisualSampleEntry),
	Audio(AudioSampleEntry),
	Unknown(UnknownBox),
}

// impl SampleEntryEnum {
// fn parse(data:&[u8]) -> SampleEntryEnum {
// 	println!("SampleEntryEnum::parse({})", data.len());
// 		let sz = u32::from_be_bytes(data[0..4].try_into().unwrap());
// 		let name = std::str::from_utf8(&data[4..8]).unwrap();
// 		let version = data[8];
// 		let flags = [data[9], data[10], data[11]];
// 		let base = FullBox {
// 			base: BaseBox {
// 				size: sz,
// 				boxtype: array_str(name),
// 			},
// 			version,
// 			flags,
// 		};
// 	}
// }

pub struct SampleEntry {
	pub base: BaseBox,
	// size
	// pub boxtype: [u8; 4],
	reserved: [u8; 6], // = 0
	pub data_reference_index: u16,
}

pub struct HintSampleEntry {
	pub base: SampleEntry,
	pub data: Vec<u8>,
}

impl HintSampleEntry {
	pub fn parse(sz: u32, data: &[u8]) -> HintSampleEntry {
		println!("HintSampleEntry::parse");
		if data.len() + 8 != sz as usize {
			panic!("sz != data.len() + 8")
		}
		let reserved: [u8; 6] = data[0..6].try_into().unwrap();
		let data_reference_index = u16::from_be_bytes(data[6..8].try_into().unwrap());
		HintSampleEntry {
			base: SampleEntry {
				base: BaseBox {
					size: sz,
					boxtype: array_str("hint"),
				},
				reserved,
				data_reference_index,
			},
			data: data[8..(sz as usize - 8)].to_vec(),
		}
	}
}

pub struct VisualSampleEntry {
	pub base: SampleEntry,

	pre_defined1: u16,      // = 0
	reserved1: u16,         // = 0
	pre_defined2: [u32; 3], // = 0
	pub width: u16,
	pub height: u16,
	horizresolution: u32, // = 0x00480000; // 72 dpi
	vertresolution: u32,  // = 0x00480000; // 72 dpi
	reserved2: u32,       // = 0
	frame_count: u16,     // = 1
	pub compressor_name: [u8; 32],
	depth: u16,        // 0x0018,
	pre_defined3: i16, // = -1
}

impl VisualSampleEntry {
	pub fn parse(sz: u32, data: &[u8]) -> VisualSampleEntry {
		let reserved: [u8; 6] = data[0..6].try_into().unwrap();
		let data_reference_index = u16::from_be_bytes(data[6..8].try_into().unwrap());
		let pre_defined1 = u16::from_be_bytes(data[8..10].try_into().unwrap());
		let reserved1 = u16::from_be_bytes(data[10..12].try_into().unwrap());
		let pre_defined2: [u32; 3] = [
			u32::from_be_bytes(data[12..16].try_into().unwrap()),
			u32::from_be_bytes(data[16..20].try_into().unwrap()),
			u32::from_be_bytes(data[20..24].try_into().unwrap()),
		];
		let width = u16::from_be_bytes(data[24..26].try_into().unwrap());
		let height = u16::from_be_bytes(data[26..28].try_into().unwrap());
		let horizresolution = u32::from_be_bytes(data[28..32].try_into().unwrap());
		let vertresolution = u32::from_be_bytes(data[32..36].try_into().unwrap());
		let reserved2 = u32::from_be_bytes(data[36..40].try_into().unwrap());
		let frame_count = u16::from_be_bytes(data[40..42].try_into().unwrap());
		let compressor_name: [u8; 32] = data[42..74].try_into().unwrap();
		let depth = u16::from_be_bytes(data[74..76].try_into().unwrap());
		let pre_defined3 = i16::from_be_bytes(data[76..78].try_into().unwrap());
		VisualSampleEntry {
			base: SampleEntry {
				base: BaseBox {
					size: sz,
					boxtype: array_str("vide"),
				},
				reserved,
				data_reference_index,
			},
			pre_defined1,
			reserved1,
			pre_defined2,
			width,
			height,
			horizresolution,
			vertresolution,
			reserved2,
			frame_count,
			compressor_name,
			depth,
			pre_defined3,
		}
	}
}

pub struct AudioSampleEntry {
	pub base: SampleEntry,

	reserved1: [u32; 2],   // = 0
	pub channelcount: u16, // = 2,
	pub samplesize: u16,   // = 2,

	pre_defined: u16,    // = 0
	reserved2: u16,      // = 0
	pub samplerate: u32, // {timescale of media} << 16
}

impl AudioSampleEntry {
	pub fn parse(sz: u32, data: &[u8]) -> AudioSampleEntry {
		let reserved: [u8; 6] = data[0..6].try_into().unwrap();
		let data_reference_index = u16::from_be_bytes(data[6..8].try_into().unwrap());
		let reserved1: [u32; 2] = [
			u32::from_be_bytes(data[8..12].try_into().unwrap()),
			u32::from_be_bytes(data[12..16].try_into().unwrap()),
		];
		let channelcount = u16::from_be_bytes(data[16..18].try_into().unwrap());
		let samplesize = u16::from_be_bytes(data[18..20].try_into().unwrap());
		let pre_defined = u16::from_be_bytes(data[20..22].try_into().unwrap());
		let reserved2 = u16::from_be_bytes(data[22..24].try_into().unwrap());
		let samplerate = u32::from_be_bytes(data[24..28].try_into().unwrap());
		AudioSampleEntry {
			base: SampleEntry {
				base: BaseBox {
					size: sz,
					boxtype: array_str("soun"),
				},
				reserved,
				data_reference_index,
			},
			reserved1,
			channelcount,
			samplesize,
			pre_defined,
			reserved2,
			samplerate,
		}
	}
}

pub struct UnknownBox {
	pub base: BaseBox,
	pub data: Vec<u8>,
}

impl UnknownBox {
	pub fn parse(sz: u32, name: [u8; 4], data: &[u8]) -> UnknownBox {
		UnknownBox {
			base: BaseBox {
				size: sz,
				boxtype: name,
			},
			data: data.to_vec(),
		}
	}
}

pub struct TimeToSampleBox {
	pub base: FullBox,
	pub entry_count: u32,
	pub samples: Vec<(u32, u32)>,
}

impl TimeToSampleBox {
	fn parse(sz: u32, data: &[u8]) -> TimeToSampleBox {
		println!("TimeToSampleBox::parse({}, {})", sz, data.len());
		let entry_count = u32::from_be_bytes(data[4..8].try_into().unwrap());
		let mut samples = Vec::<(u32, u32)>::new();
		for i in 0..entry_count as usize {
			samples.push((
				u32::from_be_bytes(data[8 + (i * 8)..12 + (i * 8)].try_into().unwrap()),
				u32::from_be_bytes(data[12 + (i * 8)..16 + (i * 8)].try_into().unwrap()),
			));
		}
		TimeToSampleBox {
			base: FullBox {
				base: BaseBox {
					size: sz,
					boxtype: array_str("stts"),
				},
				version: data[0],
				flags: [data[1], data[2], data[3]],
			},
			entry_count,
			samples,
		}
	}
}

pub struct SampleToChunkBox {
	pub base: FullBox,
	pub entry_count: u32,
	pub samples: Vec<(u32, u32, u32)>,
}

impl SampleToChunkBox {
	fn parse(sz: u32, data: &[u8]) -> SampleToChunkBox {
		println!("SampleToChunkBox::parse({}, {})", sz, data.len());
		let entry_count = u32::from_be_bytes(data[4..8].try_into().unwrap());
		let mut samples = Vec::<(u32, u32, u32)>::new();
		for i in 0..entry_count as usize {
			samples.push((
				u32::from_be_bytes(data[8 + (i * 12)..12 + (i * 12)].try_into().unwrap()),
				u32::from_be_bytes(data[12 + (i * 12)..16 + (i * 12)].try_into().unwrap()),
				u32::from_be_bytes(data[16 + (i * 12)..20 + (i * 12)].try_into().unwrap()),
			));
		}
		SampleToChunkBox {
			base: FullBox {
				base: BaseBox {
					size: sz,
					boxtype: array_str("stsc"),
				},
				version: data[0],
				flags: [data[1], data[2], data[3]],
			},
			entry_count,
			samples,
		}
	}
}

pub struct SampleSizeBox {
	pub base: FullBox,
	pub sample_size: u32,
	pub sample_count: u32,
	pub entry_sizes: Vec<u32>,
}

impl SampleSizeBox {
	fn parse(sz: u32, data: &[u8]) -> SampleSizeBox {
		println!("SampleSizeBox::parse({}, {})", sz, data.len());
		let sample_size = u32::from_be_bytes(data[4..8].try_into().unwrap());
		let sample_count = u32::from_be_bytes(data[8..12].try_into().unwrap());
		let mut entry_sizes = Vec::<u32>::new();
		for i in 0..sample_count as usize {
			entry_sizes.push(u32::from_be_bytes(data[12 + (i * 4)..16 + (i * 4)].try_into().unwrap()));
		}
		SampleSizeBox {
			base: FullBox {
				base: BaseBox {
					size: sz,
					boxtype: array_str("stsz"),
				},
				version: data[0],
				flags: [data[1], data[2], data[3]],
			},
			sample_size,
			sample_count,
			entry_sizes,
		}
	}
}

pub struct ChunkOffsetBox {
	pub base: FullBox,
	pub entry_count: u32,
	pub chunk_offsets: Vec<u32>,
}

impl ChunkOffsetBox {
	fn parse(sz: u32, data: &[u8]) -> ChunkOffsetBox {
		println!("ChunkOffsetBox::parse({}, {})", sz, data.len());
		let entry_count = u32::from_be_bytes(data[4..8].try_into().unwrap());
		println!("entrycount: {} {:?}", entry_count, &data[4..8]);
		let mut chunk_offsets = Vec::<u32>::new();
		for i in 0..entry_count as usize {
			chunk_offsets.push(u32::from_be_bytes(data[4 + (i * 4)..8 + (i * 4)].try_into().unwrap()));
		}
		ChunkOffsetBox {
			base: FullBox {
				base: BaseBox {
					size: sz,
					boxtype: array_str("stco"),
				},
				version: data[0],
				flags: [data[1], data[2], data[3]],
			},
			entry_count,
			chunk_offsets,
		}
	}
}

pub trait UserDataType {}

// udta
pub struct UserDataBox {
	pub base: BaseBox,
	// pub children: Vec<std::boxed::Box<dyn UserDataType>>,
	pub children: Vec<Vec<u8>>,
}

impl UserDataBox {
	fn parse(sz: u32, data: &[u8]) -> UserDataBox {
		println!("UserDataBox::parse({}, {})", sz, data.len());
		let mut total = 0;
		let mut children = Vec::<Vec<u8>>::new();
		while total < sz - 8 {
			println!("total: {}", total);
			let sz2 = u32::from_be_bytes(data[total as usize..total as usize + 4].try_into().unwrap());
			println!("sz2: {}", sz2);
			let v = data[total as usize..(total + sz2) as usize].to_vec();
			children.push(v);
			total += sz2;
		}

		UserDataBox {
			base: BaseBox {
				size: sz,
				boxtype: array_str("udta"),
			},
			children,
		}
	}
}

// meta
pub struct MetaBox {
	pub base: FullBox,
	pub handler: HandlerBox,
	// primary_resource: PrimaryItemBox,
	// file_locations: DataInformationBox,
	// item_locations: ItemLocationBox,
	// protections: ItemProtectionBox,
	// item_infos: ItemInfoBox,
	// ipmp_control: IPMPControlBox,
	// other_boxes: Vec<Box>,
}

impl MetaBox {
	fn parse(sz: u32, data: &[u8]) -> MetaBox {
		println!("MetaBox::parse({}, {})", sz, data.len());
		let hdlr_sz = u32::from_be_bytes(data[4..8].try_into().unwrap());
		let handler_name = std::str::from_utf8(&data[8..12]).unwrap();
		if handler_name != "hdlr" {
			panic!("Unhandle handler type in meta: {}, {:?}", handler_name, &data[8..12])
		}
		MetaBox {
			base: FullBox {
				base: BaseBox {
					size: sz,
					boxtype: ['m' as u8, 'e' as u8, 't' as u8, 'a' as u8],
				},
				version: data[0],
				flags: [data[1], data[2], data[3]],
			},
			handler: HandlerBox::parse(hdlr_sz, &data[12..]),
		}
	}
}

// mdat
pub struct MediaDataBox {
	pub base: BaseBox,
	pub data: Vec<u8>,
}

impl MediaDataBox {
	pub fn parse(sz: u32, _data: &[u8]) -> MediaDataBox {
		MediaDataBox {
			base: BaseBox {
				size: sz,
				boxtype: array_str("mdat"),
			},
			data: Vec::new(),
		}
	}
}

pub struct FreeSpaceBox {
	pub base: BaseBox,
}

impl FreeSpaceBox {
	pub fn parse(sz: u32, data: &[u8]) -> FreeSpaceBox {
		FreeSpaceBox {
			base: BaseBox {
				size: sz,
				boxtype: array_str("free"),
			},
		}
	}
}

// pub enum BaseAtom {
// 	FreeSpace(FreeSpaceBox),
// }

pub enum FileAtom {
	FileType(FileTypeBox),
	Movie(MovieBox),
	MediaData(MediaDataBox),
	// MovieFragment(MovieFragmentBox),
	// MovieFragmentRandomAccess(MovieFragmentRandomAccessBox),
	Meta(MetaBox),
	FreeSpace(FreeSpaceBox),
}

impl FileAtom {
	pub fn string(&self, depth: u16) -> String {
		let ret = String::from("FileAtom: {\n")
			+ &spacer(depth + 1)
			+ &(match self {
				FileAtom::FileType(x) => x.string(depth + 1),
				FileAtom::Movie(x) => x.string(depth + 1),
				FileAtom::MediaData(x) => String::new(), //&x.string(depth + 1),
				FileAtom::Meta(x) => String::new(),      //&x.string(depth + 1),
				FileAtom::FreeSpace(x) => String::new(), //&x.string(depth + 1),
			}) + "\n}";
		ret
	}
}

pub enum MovieAtom {
	MovieHeader(MovieHeaderBox),
	Track(TrackBox),
	UserData(UserDataBox),
	MovieExtends(MovieExtendsBox),
	Meta(MetaBox),
}

impl MovieAtom {
	pub fn string(&self, depth: u16) -> String {
		let ret = String::from("")
			+ &match self {
				MovieAtom::MovieHeader(x) => x.string(depth + 1),
				MovieAtom::Track(x) => String::new(),
				MovieAtom::UserData(x) => String::new(),
				MovieAtom::MovieExtends(x) => String::new(),
				MovieAtom::Meta(x) => String::new(),
			};
		ret
	}
}

pub enum TrackAtom {
	TrackHeader(TrackHeaderBox),
	// TrackReference(TrackReferenceBox),
	Media(MediaBox),
	// Edit(EditBox),
	UserData(UserDataBox),
	Meta(MetaBox),
}

pub enum MediaAtom {
	MediaHeader(MediaHeaderBox),
	Handler(HandlerBox),
	MediaInformation(MediaInformationBox),
}

pub enum MediaInformationAtom {
	SoundMediaHeader(SoundMediaHeaderBox),
	DataInformation(DataInformationBox),
	SampleTable(SampleTableBox),
}

pub enum DataInformationAtom {
	DataReference(DataReferenceBox),
}

pub enum SampleTableAtom {
	TimeToSample(TimeToSampleBox),
	SampleDescription(SampleDescriptionBox),
	SampleSize(SampleSizeBox),
	SampleToChunk(SampleToChunkBox),
	ChunkOffset(ChunkOffsetBox),
	// SyncSample(SyncSampleBox),
	// ShadowSyncSample(ShadowSyncSampleBox),
	// DegradationPriority(DegradationPriorityBox),
	// PaddingBits(PaddingBitsBox),
}

// pub enum EditAtom {
// 	EditList(EditListBox),
// }

pub enum UserDataAtom {
	// Copyright(CopyrightBox),
	Other(std::boxed::Box<dyn UserDataType>),
}

pub enum MovieExtendsAtom {
	// MovieExtendsHeader(MovieExtendsHeaderBox),
	// TrackExtends(TrackExtendsBox),
}

// pub enum MovieFragmentAtom {
// 	MovieFragmentHeader(MovieFragmentHeaderBox),
// 	TrackFragment(TrackFragmentBox),
// }

// pub enum TrackFragmentAtom {
// 	TrackFragmentHeader(TrackFragmentHeaderBox),
// 	TrackFragmentRun(TrackFragmentRunBox),
// }

// pub enum MovieFragmentRandomAccessAtom {
// 	TrackFragmentRandomAccess(TrackFragmentRandomAccessBox),
// 	MovieFragmentRandomAccessOffset(MovieFragmentRandomAccessOffsetBox),
// }

pub enum MetaAtom {
	Handler(HandlerBox),
	DataInformation(DataInformationBox),
}

pub enum Atom {
	FileType(FileTypeBox),
	Movie(MovieBox),
	MediaData(MediaDataBox),
	MovieExtends(MovieExtendsBox),
	MovieHeader(MovieHeaderBox),
	Track(TrackBox),
	TrackHeader(TrackHeaderBox),
	UserData(UserDataBox),
	Meta(MetaBox),
	Media(MediaBox),
	MediaHeader(MediaHeaderBox),
	MediaInformation(MediaInformationBox),
	DataInformation(DataInformationBox),
	DataReference(DataReferenceBox),
	SoundMediaHeader(SoundMediaHeaderBox),
	SampleTable(SampleTableBox),
	SampleDescription(SampleDescriptionBox),
	TimeToSample(TimeToSampleBox),
	SampleToChunk(SampleToChunkBox),
	SampleSize(SampleSizeBox),
	ChunkOffset(ChunkOffsetBox),
	Handler(HandlerBox),
	ITunes(itunes::Atom),
}
