use std::{
	collections::HashSet,
	fs::DirEntry,
	io::Write,
	path::{Path, PathBuf},
	vec::Vec,
};

use tag::{
	id3::{self, ID3CommentFrame, ID3Frame, ID3PictureFrame},
	mp4,
};

#[derive(Clone)]
struct PictureArg {
	typ: u8,
	mime: String,
	description: String,
	path: String,
}

#[derive(Clone)]
struct Flags {
	title: Option<String>,
	artist: Option<String>,
	track: Option<String>,
	album: Option<String>,
	sort_album: Option<String>,
	genre: Option<String>, // Genre
	record_date: Option<String>,
	comment: Option<String>,
	combine_comments: bool,
	pictures: Vec<PictureArg>,
	remove: HashSet<String>,
	//
	out_path: PathBuf,
}

fn main() -> Result<(), i32> {
	let args: Vec<String> = std::env::args().collect();

	let mut opts = getopts::Options::new();
	opts.optopt("", "title", "Title data to add", "TITLE");
	opts.optopt("", "artist", "Artist / Album Artist data to add", "ARTIST");
	opts.optopt("", "track", "Track data to add", "TRACK");
	opts.optopt("", "album", "Album data to add", "ALBUM");
	opts.optopt("", "sort-album", "Sort Album name", "ALBUM");
	opts.optopt("", "record-date", "Date of recording", "YYYY-MM-DD");
	opts.optopt("", "genre", "Genre", "GENRE");
	opts.optopt("", "comment", "Comment data to add", "TEXT");
	opts.optflag("", "combine_comments", "Combine comment frames");
	opts.optmulti(
		"",
		"picture",
		"Picture to add in format {type}:{path to image}",
		"PICTURE",
	);
	opts.optopt(
		"",
		"remove",
		"Semicolon-separated list of frame types to remove",
		"TXXX;COMM",
	);
	opts.optopt("", "output", "Path to output file", "FILE");
	let matches = match opts.parse(&args[1..]) {
		Ok(x) => x,
		Err(e) => {
			println!("Argument error: {}", e);
			return Err(1);
		}
	};

	let out_path = match matches.opt_str("output") {
		Some(x) => x,
		None => {
			println!("Required: output path");
			return Err(1);
		}
	};

	let flags = Flags {
		title: matches.opt_str("title"),
		artist: matches.opt_str("artist"),
		track: matches.opt_str("track"),
		album: matches.opt_str("album"),
		sort_album: matches.opt_str("sort-album"),
		genre: matches.opt_str("genre"),
		record_date: matches.opt_str("record-date"),
		comment: matches.opt_str("comment"),
		combine_comments: matches.opt_defined("combine_comments"),
		pictures: matches
			.opt_strs("picture")
			.iter()
			.map(|arg| {
				let mut split = arg.split(':');
				let typ = split.next();
				let mime = split.next();
				let description = split.next();
				let path = split.collect::<Vec<_>>().join(":");
				if typ.is_none() || mime.is_none() || description.is_none() || path.is_empty() {
					println!(
						"picture flag format must be {{type}}:{{mime type}}:{{description}}:{{path}}. Found {}",
						arg
					);
					return Err(1);
				}
				let typ = match match_pic_type(typ.unwrap()) {
					Some(x) => x,
					None => {
						println!("picture flag type is invalid. Found \"{}\"", typ.unwrap());
						return Err(1);
					}
				};
				Ok(PictureArg {
					typ,
					mime: mime.unwrap().to_owned(),
					description: description.unwrap().to_owned(),
					path,
				})
			})
			.collect::<Result<Vec<_>, i32>>()?,
		remove: matches
			.opt_str("remove")
			.unwrap_or(String::new())
			.split(';')
			.map(String::from)
			.collect(),
		out_path: Path::new(&out_path).to_path_buf(),
	};

	if matches.free.len() != 1 {
		println!("File or directory path required");
		return Err(1);
	}

	let path = &matches.free[0];

	let metadata = match std::fs::metadata(path) {
		Ok(x) => x,
		Err(e) => {
			println!("Could not read path {}: {}", path, e);
			return Err(1);
		}
	};

	if metadata.is_file() {
		return match recode_path(Path::new(path), &flags) {
			Ok(_) => Ok(()),
			Err(e) => {
				println!("{}", e);
				Err(1)
			}
		};
	}

	let files = match std::fs::read_dir(path) {
		Ok(x) => x,
		Err(e) => {
			println!("Error reading directory {}: {}", path, e);
			return Err(0);
		}
	};

	let mut paths: Vec<DirEntry> = files.map(|r| r.unwrap()).collect();

	paths.sort_by_key(|dir| dir.path());
	for path in paths {
		if let Err(e) = recode_path(&path.path(), &flags) {
			println!("Error on {}: {}", path.file_name().to_str().unwrap(), e);
			return Err(1);
		}
	}

	Ok(())
}

fn recode_path(path: &Path, flags: &Flags) -> Result<(), String> {
	if path.file_name().unwrap_or_default().to_string_lossy().ends_with(".mp3") {
		recode_mp3_file(path, flags)?;
	} else if path.file_name().unwrap_or_default().to_string_lossy().ends_with(".m4a") {
		recode_m4a_file(path, flags.clone())?;
	} else {
		println!("Skipping {}", path.display());
	}
	Ok(())
}

fn u32_from_be(slice: &[u8]) -> u32 {
	u32::from_be_bytes(slice.try_into().unwrap())
}

enum Data<'a> {
	Vec(Vec<u8>),
	Slice(&'a [u8]),
}
impl<'a> Data<'a> {
	fn len(&'a self) -> usize {
		match self {
			Self::Vec(v) => v.len(),
			Self::Slice(s) => s.len(),
		}
	}
}

fn find_offset(content: &[u8], tag: &[u8; 4]) -> Option<usize> {
	let mut ix = 0;
	while ix < content.len() {
		let size = u32_from_be(&content[ix..ix + 4]) as usize;
		if size == 0 {
			panic!("Found 0 size");
		}
		let boxtype = &content[ix + 4..ix + 8];
		if boxtype == tag {
			return Some(ix);
		}
		ix += size;
	}
	None
}

fn inject_ilst<'content>(content: &'content [u8], ilst_cfg: &mp4::ItemListConfig) -> Vec<Data<'content>> {
	let mut ret: Vec<Data> = Vec::new();
	let mut ix = 0;
	while ix < content.len() {
		let size = u32_from_be(&content[ix..ix + 4]);
		let boxtype = &content[ix + 4..ix + 8];
		if size == 0 {
			panic!("0 size");
		}
		match boxtype {
			// extends Box
			b"moov" | b"udta" | b"trak" | b"mdia" | b"minf" | b"stbl" => {
				let atoms = inject_ilst(&content[ix + 8..ix + size as usize], ilst_cfg);
				let new_size: u32 = 8 + atoms.iter().fold(0, |acc, atom| acc + atom.len() as u32);
				let mut data = new_size.to_be_bytes().to_vec();
				data.extend_from_slice(boxtype);
				ret.push(Data::Vec(data));
				ret.extend(atoms.into_iter());
			}
			// extends FullBox
			b"meta" => {
				let atoms = inject_ilst(&content[ix + 12..ix + size as usize], ilst_cfg);
				let new_size: u32 = 12 + atoms.iter().fold(0, |acc, atom| acc + atom.len() as u32);
				let mut data = new_size.to_be_bytes().to_vec();
				data.extend_from_slice(boxtype);
				data.extend_from_slice(&content[8..12]);
				ret.push(Data::Vec(data));

				ret.extend(atoms.into_iter());
			}
			b"ilst" => {
				let ilst = mp4::ItemList::parse(size, &content[ix..ix + size as usize]).apply_config(ilst_cfg.clone());
				let byt = ilst.bytes();
				ret.push(Data::Vec(byt));
			}
			// b"stco" => {}
			_ => {
				ret.push(Data::Slice(&content[ix..ix + size as usize]));
			}
		}
		ix += size as usize;
	}
	ret
}

fn recode_m4a_file(path: &Path, cmd_flags: Flags) -> Result<(), String> {
	let content = match std::fs::read(path) {
		Ok(s) => s,
		Err(e) => {
			return Err(format!("Could not open file: {}: {}", path.display(), e));
		}
	};
	let ilst = mp4::ItemListConfig {
		title: cmd_flags.title,
		artist: cmd_flags.artist.clone(),
		album_artist: cmd_flags.artist,
		track: cmd_flags.track.map(|track| track.parse::<u32>().unwrap()),
		album: cmd_flags.album,
		sort_album: cmd_flags.sort_album,
		genre: cmd_flags.genre,
		record_date: cmd_flags.record_date,
		comment: cmd_flags.comment,
		// combine_comments: cmd_flags.combine_comments,
		// remove: cmd_flags.remove,
	};
	let top_level_atoms = inject_ilst(&content, &ilst);

	let mdat_offset_before = find_offset(&content, b"mdat").unwrap();

	let mdat_offset_after = top_level_atoms
		.iter()
		.take_while(|atom| match atom {
			Data::Vec(v) => v[4..8] != *b"mdat",
			Data::Slice(v) => v[4..8] != *b"mdat",
		})
		.fold(0, |acc, atom| {
			let slice: &[u8] = match atom {
				Data::Vec(v) => v,
				Data::Slice(v) => v,
			};
			acc + slice.len()
		});

	let offset_to_add: i32 = (mdat_offset_after - mdat_offset_before).try_into().unwrap();

	let modified_atoms = top_level_atoms.into_iter().map(|mut atom| {
		let slice: &[u8] = match atom {
			Data::Vec(ref v) => v,
			Data::Slice(v) => v,
		};
		if slice[4..8] == *b"stco" {
			let size = u32_from_be(&slice[..4]);
			let mut chunk_offset_atom = mp4::ChunkOffsetBox::parse(size, slice);
			for offset in chunk_offset_atom.chunk_offsets.iter_mut() {
				*offset = offset.checked_add_signed(offset_to_add as i32).unwrap();
			}
			atom = Data::Vec(chunk_offset_atom.bytes());
		}
		// TODO: co64
		atom
	});
	let mut f = match std::fs::File::create(&cmd_flags.out_path) {
		Ok(x) => x,
		Err(e) => {
			return Err(format!(
				"Could not create file: {}: {}",
				cmd_flags.out_path.display(),
				e
			));
		}
	};

	for atom in modified_atoms {
		let byte_slice = match &atom {
			Data::Slice(s) => s,
			Data::Vec(v) => v.as_slice(),
		};
		let atom_size = u32_from_be(&byte_slice[..4]);
		if byte_slice.len() != atom_size as usize {
			match &byte_slice[4..8] {
				b"moov" | b"udta" | b"meta" | b"trak" | b"mdia" | b"minf" | b"stbl" => {}
				_ => panic!("byte_slice.len() != atom_size: {} != {}", byte_slice.len(), atom_size),
			}
		}
		match f.write_all(byte_slice) {
			Ok(_) => (),
			Err(e) => {
				return Err(format!("Error writing bytes: {}", e));
			}
		};
	}

	Ok(())
}

// #[derive(Debug)]
// struct MFrame {
// 	id: String,
// 	size: usize,
// 	data: String,
// 	children: Vec<MFrame>,
// }

// impl MFrame {
// 	fn string(&self, depth: usize) -> String {
// 		let mut s = String::from("{ \"id\": \"") + &self.id + "\", \"size\": " + &self.size.to_string() + ", ";
// 		if !self.data.is_empty() {
// 			s += &(String::from("\"data\": \"") + &self.data + "\" ");
// 		}
// 		if self.children.is_empty() {
// 			//
// 		} else {
// 			s += "\"data\": [\n";
// 			for (i, child) in self.children.iter().enumerate() {
// 				for _ in 0..depth + 1 {
// 					s += "  ";
// 				}
// 				s += &child.string(depth + 1);
// 				if i + 1 < self.children.len() {
// 					s += ",\n";
// 				}
// 				// s += "\n";
// 			}
// 			s += ",\n";
// 			for i in 0..depth {
// 				s += "  ";
// 			}
// 			s += "]";
// 		}
// 		s += "}";

// 		s
// 	}
// }

// fn parse_mp4_frames(content: &[u8], depth: usize) -> (Vec<mp4::Atom>, usize) {
// 	let mut frames = Vec::<mp4::Atom>::new();
// 	let mut ix = 0;

// 	while ix < content.len() {
// 		let (f, sz) = parse_mp4_frame(&content[ix..], depth + 1);
// 		frames.push(f);
// 		ix += sz
// 	}

// 	(frames, ix)
// }

// fn parse_Xtra_frames(content: &[u8], depth: usize) -> (Vec<mp4::Atom>, usize) {
// 	let mut ix = 0;
// 	let mut frames = Vec::<mp4::Atom>::new();
// 	// while ix < content.len() {
// 		let sz1 = u32::from_be_bytes(content[0..4].try_into().unwrap()) as usize;
// 		let sz2 = u32::from_be_bytes(content[4..8].try_into().unwrap()) as usize;
// 		let name = std::str::from_utf8(&content[8..8+sz2]).unwrap();
// 		let sz2 = u32::from_be_bytes(content[4..8].try_into().unwrap()) as usize;
// 		frames.push(mp4::Atom{
// 			id: "Xtra".to_string(),
// 			size: sz1,
// 			data: name.to_string(),
// 			children: Vec::<mp4::Atom>::new(),
// 		});
// 		ix += sz1;
// 	// }

// 	(frames, sz1)
// }

// fn parse_mp4_frame(content: &[u8], depth: usize) -> (mp4::Atom, usize) {
// 	let sz = u32::from_be_bytes(content[0..4].try_into().unwrap()) as usize;
// 	let typ: &[u8] = &content[4..8];
// 	let typ = {
// 		let mut ret: [u8; 4] = [0, 0, 0, 0];
// 		for (i, t) in typ.iter().enumerate() {
// 			if !((*t >= 'a' as u8 && *t <= 'z' as u8)
// 				|| (*t >= 'A' as u8 && *t <= 'Z' as u8)
// 				|| (*t >= '0' as u8 && *t <= '9' as u8))
// 			{
// 				ret[i] = '?' as u8;
// 			} else {
// 				ret[i] = *t;
// 			}
// 		}
// 		ret
// 	};
// 	let code = std::str::from_utf8(&typ).unwrap();
// 	for i in 0..depth {
// 		print!(" ");
// 	}
// 	println!("parse {} {}", sz, code);
// 	match code {
// 		"meta" => {
// 			let (v, msz) = parse_mp4_frames(&content[12..sz], depth + 1);
// 			(
// 				mp4::Atom::Meta(MetaBox {
// 					base: mp4::FullBox {
// 						version: ,
// 						flags: ,
// 					},
// 					handler: mp4::HandlerBox {

// 					}
// 				}),
// 				sz,
// 			)
// 		}
// 		"ftyp" | "mvhd" | "tkhd" | "mdhd" | "hdlr" | "smhd" | "dref" | "stbl" | "free" | "mdat" | "data" | "mean"
// 		| "name" => (
// 			MFrame {
// 				id: code.to_string(),
// 				size: sz,
// 				data: match std::str::from_utf8(&content[12..sz]) {
// 					Ok(x) => x.to_string(),
// 					Err(e) => String::from("err"),
// 				},
// 				children: Vec::<MFrame>::new(),
// 			},
// 			sz,
// 		),
// 		"moov" | "trak" | "mdia" | "minf" | "dinf" | "udta" | "ilst" => {
// 			let (v, msz) = parse_mp4_frames(&content[8..sz], depth + 1);
// 			// (f, sz + msz)
// 			(
// 				MFrame {
// 					id: code.to_string(),
// 					size: sz,
// 					data: String::new(),
// 					children: v,
// 				},
// 				sz,
// 			)
// 		}
// 		"Xtra" => {
// 			let (v, msz) = parse_Xtra_frames(&content[8..sz], depth + 1);
// 			// (f, sz + msz)
// 			(
// 				MFrame {
// 					id: code.to_string(),
// 					size: sz,
// 					data: String::new(),
// 					children: v,
// 				},
// 				sz,
// 			)
// 		}
// 		// "????" => panic!("Unhandled frame type: {}, {:?}", code, typ),
// 		_ => {
// 			let (v, msz) = parse_mp4_frames(&content[8..sz], depth + 1);
// 			// (f, sz + msz)
// 			(
// 				MFrame {
// 					id: code.to_string(),
// 					size: sz,
// 					data: String::new(),
// 					// match std::str::from_utf8(&content[16..sz]) {
// 					// 	Ok(x) => x.to_string(),
// 					// 	Err(e) => String::new(),
// 					// },
// 					children: v,
// 				},
// 				sz,
// 			)
// 		}
// 	}
// }

// const fn bytes(s: &'static str) -> [u8; 4] {
// 	s.into()
// }

fn move_text_item(new_list: &mut Vec<ID3Frame>, old_list: &mut Vec<ID3Frame>, code: [u8; 4], opt: &Option<String>) {
	if let Some(item) = opt {
		new_list.push(id3::ID3Frame {
			id: code,
			flags: [0, 0],
			data: id3::ID3FrameType::Text(id3::ID3TextFrame {
				data: item.clone(),
				encoding: if item.chars().all(|c| c.is_ascii()) { 0 } else { 3 },
			}),
		});
		old_list.retain(|f| f.id != code);
	} else if let Some(ix) = old_list.iter().position(|f| f.id == code) {
		new_list.push(old_list.remove(ix));
	}
}

fn recode_mp3_file(path: &Path, cmd_flags: &Flags) -> Result<(), String> {
	let content = match std::fs::read(path) {
		Ok(s) => s,
		Err(e) => {
			return Err(format!("Could not open file: {}: {}", path.display(), e));
		}
	};

	let (mut frames, id3_size) = tag::read_id3_frames(&content);

	frames.retain(|frame| {
		if cmd_flags.remove.contains(String::from_utf8_lossy(&frame.id).as_ref()) {
			println!("Dropping frame: {}", frame.display());
			return false;
		}
		true
	});

	{
		// Check for more than one frame of the same type
		let mut seen = HashSet::new();
		for frame in &frames {
			if seen.contains(&frame.id) {
				if frame.id == *b"COMM" && (cmd_flags.comment.is_some() || cmd_flags.combine_comments) {
					// Skip
				} else {
					panic!("More than one frame containing {}", String::from_utf8_lossy(&frame.id));
				}
			}
			seen.insert(frame.id);
		}
	}

	let mut new_frames = Vec::with_capacity(frames.len());

	move_text_item(&mut new_frames, &mut frames, b"TIT2".to_owned(), &cmd_flags.title);

	if let Some(artist) = &cmd_flags.artist {
		new_frames.push(id3::ID3Frame {
			id: b"TPE1".to_owned(),
			flags: [0, 0],
			data: id3::ID3FrameType::Text(id3::ID3TextFrame {
				data: artist.clone(),
				encoding: if artist.chars().all(|c| c.is_ascii()) { 0 } else { 3 },
			}),
		});
		new_frames.push(id3::ID3Frame {
			id: b"TPE2".to_owned(),
			flags: [0, 0],
			data: id3::ID3FrameType::Text(id3::ID3TextFrame {
				data: artist.clone(),
				encoding: if artist.chars().all(|c| c.is_ascii()) { 0 } else { 3 },
			}),
		});
		if let Some(ix) = frames.iter().position(|f| &f.id == b"TPE1") {
			frames.remove(ix);
		}
		if let Some(ix) = frames.iter().position(|f| &f.id == b"TPE2") {
			frames.remove(ix);
		}
	} else {
		if let Some(ix) = frames.iter().position(|f| &f.id == b"TPE1") {
			new_frames.push(frames.remove(ix));
		}
		if let Some(ix) = frames.iter().position(|f| &f.id == b"TPE2") {
			new_frames.push(frames.remove(ix));
		}
	}

	move_text_item(&mut new_frames, &mut frames, b"TRCK".to_owned(), &cmd_flags.track);
	move_text_item(&mut new_frames, &mut frames, b"TALB".to_owned(), &cmd_flags.album);
	move_text_item(&mut new_frames, &mut frames, b"TSOA".to_owned(), &cmd_flags.sort_album);
	move_text_item(&mut new_frames, &mut frames, b"TCON".to_owned(), &cmd_flags.genre);

	if let Some(item) = &cmd_flags.record_date {
		new_frames.push(id3::ID3Frame {
			id: b"TDRC".to_owned(),
			flags: [0, 0],
			data: id3::ID3FrameType::Text(id3::ID3TextFrame {
				data: item.clone(),
				encoding: if item.chars().all(|c| c.is_ascii()) { 0 } else { 3 },
			}),
		});
		if let Some(ix) = frames.iter().position(|f| &f.id == b"TDRC") {
			frames.remove(ix);
		}
	} else if let Some(ix) = frames.iter().position(|f| &f.id == b"TDRC") {
		let mut frame = frames.remove(ix);
		if let id3::ID3FrameType::Text(tf) = &mut frame.data {
			let parts: Vec<&str> = tf.data.split(';').collect();
			let date = parts[0].replace('.', "-");
			if parts.len() > 1 {
				let time = parts[1].replace('.', ":");
				tf.data = date + "T" + &time;
			} else {
				tf.data = date;
			}
		} else {
			panic!("TDRC frame is not text");
		}
		new_frames.push(frame);
	}

	if let Some(comment) = &cmd_flags.comment {
		new_frames.push(id3::ID3Frame {
			id: b"COMM".to_owned(),
			flags: [0, 0],
			data: id3::ID3FrameType::Comment(id3::ID3CommentFrame {
				language: [b'e', b'n', b'g'], // eng
				content_desc: String::new(),
				text: comment.clone(),
				encoding: if comment.chars().all(|c| c.is_ascii()) { 0 } else { 3 },
			}),
		});
		frames.retain(|f| &f.id != b"COMM");
	} else if cmd_flags.combine_comments {
		let mut comments = Vec::<(String, String)>::new();
		for f in &frames {
			let comm = match &f.data {
				id3::ID3FrameType::Comment(comm) => comm,
				_ => continue,
			};
			if let Some(pos) = comments.iter().position(|h| comm.text == h.1) {
				let h = &mut comments[pos];
				if !h.0.is_empty() && !comm.content_desc.is_empty() {
					h.0 += ";"
				}
				h.0 += &comm.content_desc;
			} else {
				comments.push((comm.content_desc.clone(), comm.text.clone()));
			}
		}
		for comment in comments {
			let (content_desc, text) = comment;
			let encoding = if text.chars().all(|c| c.is_ascii()) && content_desc.chars().all(|c| c.is_ascii()) {
				0
			} else {
				3
			};
			new_frames.push(ID3Frame {
				id: b"COMM".to_owned(),
				flags: [0, 0],
				data: id3::ID3FrameType::Comment(ID3CommentFrame {
					language: b"eng".to_owned(),
					content_desc,
					text,
					encoding,
				}),
			});
		}
		frames.retain(|f| &f.id != b"COMM");
	}

	for pic in &cmd_flags.pictures {
		let data = match std::fs::read(&pic.path) {
			Ok(x) => x,
			Err(e) => {
				let err_msg = format!("Error reading picture path {}: {}", pic.path, e);
				return Err(err_msg);
			}
		};
		new_frames.push(ID3Frame {
			id: b"APIC".to_owned(),
			flags: [0, 0],
			data: id3::ID3FrameType::Picture(ID3PictureFrame {
				mime: pic.mime.clone(),
				pic_type: pic.typ,
				description: pic.description.clone(),
				data,
			}),
		});
	}

	if !cmd_flags.pictures.is_empty() {
		frames.retain(|frame| &frame.id != b"APIC");
	}

	for frame in frames {
		println!("Remaining frame: {}", frame.display());
		new_frames.push(frame);
	}

	let new_size = new_frames
		.iter()
		.fold(0u32, |accum, frame| -> u32 { accum + 10 + frame.data.len() as u32 });
	// println!("Final size: {}", new_size);

	let do_foot = true;

	let id3 = id3::ID3v240Tag {
		header: id3::ID3Header {
			version_major: 4,
			version_minor: 0,
			flags: if do_foot { 0b0001_0000 } else { 0 },
			size: new_size,
		},
		extended_header: None,
		frames: new_frames,
		padding: 0,
		has_footer: do_foot,
	};

	let mut f = match std::fs::File::create(&cmd_flags.out_path) {
		Ok(x) => x,
		Err(e) => {
			return Err(format!(
				"Could not create file: {}: {}",
				cmd_flags.out_path.display(),
				e
			));
		}
	};

	match f.write_all(&id3.bytes()) {
		Ok(_) => (),
		Err(e) => {
			return Err(format!("Error writing bytes: {}", e));
		}
	};

	// Omit ID3v1 tag
	let last_idx = content.len();
	let has_id3v1_tag =
		content[last_idx - 128] == b'T' && content[1 + last_idx - 128] == b'A' && content[2 + last_idx - 128] == b'G';
	let mp3_byte_range = if has_id3v1_tag {
		&content[id3_size..last_idx - 128]
	} else {
		&content[id3_size..]
	};

	match f.write_all(mp3_byte_range) {
		Ok(_) => (),
		Err(e) => {
			return Err(format!("Error writing bytes: {}", e));
		}
	};

	Ok(())
}

fn match_pic_type(typ: &str) -> Option<u8> {
	match typ {
		"Other" => Some(0x00),
		"32x32 pixels 'file icon' (PNG only)" => Some(0x01),
		"Other file icon" => Some(0x02),
		"Cover (front)" => Some(0x03),
		"Cover (back)" => Some(0x04),
		"Leaflet page" => Some(0x05),
		"Media (e.g. label side of CD)" => Some(0x06),
		"Lead artist/lead performer/soloist" => Some(0x07),
		"Artist/performer" => Some(0x08),
		"Conductor" => Some(0x09),
		"Band/Orchestra" => Some(0x0A),
		"Composer" => Some(0x0B),
		"Lyricist/text writer" => Some(0x0C),
		"Recording Location" => Some(0x0D),
		"During recording" => Some(0x0E),
		"During performance" => Some(0x0F),
		"Movie/video screen capture" => Some(0x10),
		"A bright coloured fish" => Some(0x11),
		"Illustration" => Some(0x12),
		"Band/artist logotype" => Some(0x13),
		"Publisher/Studio logotype" => Some(0x14),
		_ => None,
	}
}
