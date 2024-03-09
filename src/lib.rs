use core::{convert::TryInto, panic};

use id3::ID3FrameType;

pub mod id3;
mod itunes;
pub mod mp4;

pub fn read_id3_frames(content: &[u8]) -> (Vec<id3::ID3Frame>, usize) {
	// let s = std::str::from_utf8(&content[0..3]).unwrap();
	// println!("Header:");
	// println!("0-2: {}", s);
	let major_version = content[3];
	// println!("version: id3v2.{}.{}", major_version, content[4]);

	// let flags = content[5];
	// if flags != 0 {
	// 	panic!("flags != 0: {}", flags);
	// }
	// println!("flags: {}", flags);
	// let sz: u32 = &content[5..8].try_into().unwrap();
	let arr: [u8; 4] = content[6..10].try_into().unwrap();
	let id3_size: usize = id3::from_synchsafe(arr) as usize;
	// let sz = u32::from_be_bytes(arr) as usize;
	// println!("size: {}", id3_size);
	// for content[0..3]

	// let header = ID3Header {
	// 	version_major: 4,
	// 	version_minor: 0,
	// 	flags: flags,
	// 	size: 0,
	// };

	let mut frames = Vec::<id3::ID3Frame>::new();

	let mut tdat_month: Option<&str> = None;
	let mut tdat_day: Option<&str> = None;
	let mut tyer: Option<&str> = None;

	let mut ix: usize = 10;
	while ix < id3_size {
		// println!("---------------- {} / {}", ix, id3_size);
		if content[ix] == 0 && content[ix + 1] == 0 && content[ix + 2] == 0 && content[ix + 3] == 0 {
			break;
		}
		let mut code = std::str::from_utf8(&content[ix..ix + 4]).unwrap();
		ix += 4;
		let sz = if major_version <= 3 {
			u32::from_be_bytes(content[ix..ix + 4].try_into().unwrap()) as usize
		} else {
			id3::from_synchsafe(content[ix..ix + 4].try_into().unwrap()) as usize
		};
		ix += 4;
		let flags = &content[ix..ix + 2];
		if flags[0] != 0 || flags[1] != 0 {
			panic!("flags != 0: {} {}", flags[0], flags[1]);
		}
		ix += 2;

		let data: id3::ID3FrameType = match code {
			// Attached Picture
			"APIC" => id3::ID3FrameType::Picture(handle_pic(&content[ix..ix + sz])),
			// Comments
			"COMM" => id3::ID3FrameType::Comment(handle_comm(&content[ix..ix + sz])),
			// Album/Movie/Show title
			"TALB" => id3::ID3FrameType::Text(handle_t(&content[ix..ix + sz])),
			// Content
			"TCON" => id3::ID3FrameType::Text(handle_t(&content[ix..ix + sz])),
			// Date
			"TDAT" => {
				let day_bytes: &[u8] = content[ix + 1..ix + 1 + 2].try_into().unwrap();
				let month_bytes: &[u8] = content[ix + 3..ix + 3 + 2].try_into().unwrap();
				let day = match std::str::from_utf8(day_bytes) {
					Err(e) => {
						println!("Could not parse TDAT day: {}: {:?}", e, &content[ix + 1..ix + 1 + 4]);
						ix += sz;
						continue;
					}
					Ok(x) => x,
				};
				let month = match std::str::from_utf8(month_bytes) {
					Err(e) => {
						println!("Could not parse TDAT month: {}: {:?}", e, &content[ix + 1..ix + 1 + 4]);
						ix += sz;
						continue;
					}
					Ok(x) => x,
				};
				println!("TDAT: {day} {month}");
				tdat_day = Some(day);
				tdat_month = Some(month);
				if let Some(year) = tyer {
					code = "TDRC";
					id3::ID3FrameType::Text(id3::ID3TextFrame {
						data: year.to_string() + "-" + month + "-" + day,
						encoding: 0,
					})
				} else {
					// panic!("TYER not found. Required for TDAT.");
					// Saved and handled in TYER
					ix += sz;
					continue;
				}
			}
			// Recording time
			"TDRC" => id3::ID3FrameType::Text(handle_t(&content[ix..ix + sz])),
			// Title/songname/content description
			"TIT2" => id3::ID3FrameType::Text(handle_t(&content[ix..ix + sz])),
			// The length of the audio file in milliseconds, represented as a numeric string.
			"TLEN" => {
				let frame = handle_t(&content[ix..ix + sz]);
				println!("Ignoring TLEN frame: {}", frame.data);
				ix += sz;
				continue;
			}
			// Lead performer(s)/Soloist(s)
			"TPE1" => id3::ID3FrameType::Text(handle_t(&content[ix..ix + sz])),
			// Band/orchestra/accompaniment
			"TPE2" => id3::ID3FrameType::Text(handle_t(&content[ix..ix + sz])),
			// Track number/Position in set
			"TRCK" => id3::ID3FrameType::Text(handle_t(&content[ix..ix + sz])),
			// Album sort order
			"TSOA" => id3::ID3FrameType::Text(handle_t(&content[ix..ix + sz])),
			// Software/Hardware and settings used for encodin
			"TSSE" => id3::ID3FrameType::Text(handle_t(&content[ix..ix + sz])),
			// User defined text information frame
			"TXXX" => id3::ID3FrameType::Text(handle_t(&content[ix..ix + sz])),
			// Year
			"TYER" => {
				let year_bytes: &[u8] = content[ix + 1..ix + 1 + 4].try_into().unwrap();
				let year = match std::str::from_utf8(year_bytes) {
					Err(e) => {
						println!("Could not parse TYER year: {}: {:?}", e, &content[ix + 1..ix + 5]);
						ix += sz;
						continue;
					}
					Ok(x) => x,
				};
				tyer = Some(year);
				if let Some(month) = tdat_month {
					if let Some(day) = tdat_day {
						code = "TDRC";
						id3::ID3FrameType::Text(id3::ID3TextFrame {
							data: year.to_string() + "-" + month + "-" + day,
							encoding: 0,
						})
					} else {
						ix += sz;
						continue;
					}
				} else {
					ix += sz;
					continue;
				}
			}
			// Unsynchronised lyric/text transcription
			"USLT" => id3::ID3FrameType::Comment(handle_uslt(&content[ix..ix + sz])),
			_ => panic!("Unhandled tag: {}", code),
		};

		// new_size += 10 + &data.len();

		let is_empty = match &data {
			ID3FrameType::Comment(f) => f.text.is_empty() && f.language.is_empty() && f.content_desc.is_empty(),
			ID3FrameType::Picture(f) => f.data.is_empty() && f.description.is_empty() && f.mime.is_empty(),
			ID3FrameType::Text(f) => f.data.is_empty(),
		};

		if !is_empty {
			frames.push(id3::ID3Frame {
				id: code.as_bytes().try_into().unwrap(),
				// size: sz as u32,
				flags: flags[0..2].try_into().unwrap(),
				data,
			});
		}

		ix += sz;
	}

	(frames, id3_size)
}

fn read_to_null(content: &[u8], encoding: u8) -> (String, usize) {
	match encoding {
		0 | 3 => {
			let mut i = 0;
			while content[i] != 0 {
				i += 1;
				if i >= content.len() {
					break;
				}
			}
			(std::str::from_utf8(&content[0..i]).unwrap().to_string(), i + 1)
		}
		1 => {
			let mut i = 0;
			while !(content[i] == 0 && content[i + 1] == 0) {
				i += 1;
				if i >= content.len() {
					break;
				}
			}
			if i == 0 {
				return (String::new(), i + 2);
			}
			if content[0] == 0xFF && content[1] == 0xFE {
				let uv: Vec<u16> = content[2..i]
					.chunks_exact(2)
					.map(|a| u16::from_be_bytes([a[1], a[0]]))
					.collect();
				i += 2;
				(String::from_utf16(&uv).unwrap(), i)
			} else if content[0] == 0xFE && content[1] == 0xFF {
				let uv: Vec<u16> = content[2..i]
					.chunks_exact(2)
					.map(|a| u16::from_be_bytes([a[0], a[1]]))
					.collect();
				i += 2;
				(String::from_utf16(&uv).unwrap(), i)
			} else {
				panic!("Expected FF FE or FE FF, got {:x} {:x}", content[0], content[1]);
			}
		}
		2 => {
			let mut i = 0;
			while !(content[i] == 0 && content[i + 1] == 0) {
				i += 1;
				if i >= content.len() {
					break;
				}
			}
			if i == 0 {
				return (String::new(), i + 2);
			}
			let uv: Vec<u16> = content[0..i]
				.chunks_exact(2)
				.map(|a| u16::from_be_bytes([a[1], a[0]]))
				.collect();
			i += 2;
			(String::from_utf16(&uv).unwrap(), i)
		}
		_ => panic!("unhandled encoding: {}", encoding),
	}
}

fn read_as_utf8(content: &[u8], encoding: u8) -> String {
	match encoding {
		0 | 3 => std::str::from_utf8(content).unwrap().to_string(),
		1 => {
			if content[0] == 0xFF && content[1] == 0xFE {
				let uv: Vec<u16> = content[2..]
					.chunks_exact(2)
					.map(|a| u16::from_be_bytes([a[1], a[0]]))
					.collect();
				String::from_utf16(&uv).unwrap()
			} else if content[0] == 0xFE && content[1] == 0xFF {
				let uv: Vec<u16> = content[2..]
					.chunks_exact(2)
					.map(|a| u16::from_be_bytes([a[0], a[1]]))
					.collect();
				String::from_utf16(&uv).unwrap()
			} else {
				panic!(
					"Expected FF FE or FE FF, got {:x} {:x} {}",
					content[0],
					content[1],
					content.len()
				);
			}
		}
		2 => {
			let uv: Vec<u16> = content
				.chunks_exact(2)
				.map(|a| u16::from_be_bytes([a[1], a[0]]))
				.collect();
			String::from_utf16(&uv).unwrap()
		}
		_ => panic!("unhandled encoding: {}", encoding),
	}
}

fn handle_other_text(content: &[u8]) -> id3::ID3CommentFrame {
	let mut ix = 0;
	let encoding: u8 = content[ix];
	ix += 1;

	// let lang = std::str::from_utf8(&content[ix..ix + 3]).unwrap();
	ix += 3;

	let (content_descriptor, last) = read_to_null(&content[ix..], encoding);
	ix += last;
	let data = read_as_utf8(&content[ix..], encoding);

	// println!("code:   {}", code);
	// println!("name:   {}", name);
	// println!("length: {}", content.len());
	// println!("encode: {}", encoding);
	// println!("lang:   {}", lang);
	// println!("c_desc: {}", content_descriptor);
	// println!("text:   {}", data);

	id3::ID3CommentFrame {
		language: [content[1], content[2], content[3]],
		content_desc: content_descriptor,
		text: data,
		// text: "12345是".to_string(),
		encoding,
	}
}

fn handle_uslt(content: &[u8]) -> id3::ID3CommentFrame {
	handle_other_text(content)
}

fn handle_comm(content: &[u8]) -> id3::ID3CommentFrame {
	handle_other_text(content)
}

fn handle_t(content: &[u8]) -> id3::ID3TextFrame {
	let mut ix = 0;
	let len = content.len();
	let encoding = content[ix];
	ix += 1;
	let text: String = match encoding {
		0 | 3 => std::str::from_utf8(&content[ix..ix + len - 1]).unwrap().to_string(),
		1 => {
			if content[ix] == 0xFF && content[ix + 1] == 0xFE {
				let uv: Vec<u16> = content[ix + 2..(ix + 2 + len - 6)]
					.chunks_exact(2)
					.map(|a| u16::from_be_bytes([a[1], a[0]]))
					.collect();
				String::from_utf16(&uv).unwrap()
			} else if content[ix] == 0xFE && content[ix + 1] == 0xFF {
				let uv: Vec<u16> = content[ix + 2..(ix + 2 + len - 6)]
					.chunks_exact(2)
					.map(|a| u16::from_be_bytes([a[0], a[1]]))
					.collect();
				String::from_utf16(&uv).unwrap()
			} else {
				panic!("Expected FF FE or FE FF, got {:x} {:x}", content[ix], content[ix + 1]);
			}
		}
		2 => {
			let uv: Vec<u16> = content[ix + 2..(ix + 2 + len - 6)]
				.chunks_exact(2)
				.map(|a| u16::from_be_bytes([a[1], a[0]]))
				.collect();
			String::from_utf16(&uv).unwrap()
		}
		_ => panic!("Forbidden encoding: {}", encoding),
	};

	// println!("code:   {}", code);
	// println!("name:   {}", name);
	// println!("length: {}", len);
	// println!("encode: {}", encoding);
	// println!("text:   {}", text);

	id3::ID3TextFrame { data: text, encoding }
}

fn handle_pic(content: &[u8]) -> id3::ID3PictureFrame {
	let encoding = content[0];
	let (mime, last) = read_to_null(&content[1..], encoding);
	let pic_type = content[last + 1];
	let (description, nlast) = read_to_null(&content[last + 1 + 1..], encoding);
	// println!("code:   {}", code);
	// println!("name:   {}", name);
	// println!("length: {}", content.len());
	// println!("encode: {}", encoding);
	// println!("mime:   {}", mime);
	// println!("pic_typ:{:X}", pic_type);
	// println!("desc:   {}", description);
	// println!("size:   {}", content.len() - (nlast + last + 2));

	id3::ID3PictureFrame {
		mime,
		pic_type,
		description,
		data: content[last + nlast + 2..].to_vec(),
	}
}
