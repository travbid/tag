use core::convert::TryInto;
use std::{
	collections::HashSet,
	fs::DirEntry,
	io::Write,
	path::{Path, PathBuf},
	vec::Vec,
};

use tag::{
	id3,
	id3::{ID3CommentFrame, ID3Frame},
	mp4,
};

struct Flags {
	title: Option<String>,
	artist: Option<String>,
	track: Option<String>,
	album: Option<String>,
	sort_album: Option<String>,
	content_type: Option<String>, // Genre
	record_date: Option<String>,
	comment: Option<String>,
	combine_comments: bool,
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
	opts.optopt("", "content-type", "Content-type (genre)", "GENRE");
	opts.optopt("", "comment", "Comment data to add", "TEXT");
	opts.optflag("", "combine_comments", "Combine comment frames");
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
		content_type: matches.opt_str("content-type"),
		record_date: matches.opt_str("record-date"),
		comment: matches.opt_str("comment"),
		combine_comments: matches.opt_defined("combine_comments"),
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
	println!("Name: {}", path.display());
	if path.file_name().unwrap_or_default().to_string_lossy().ends_with(".mp3") {
		recode_mp3_file(path, flags)?;
	} else if path.file_name().unwrap_or_default().to_string_lossy().ends_with(".m4a") {
		recode_m4a_file(path)?;
	} else {
		println!("Skipping {}", path.display());
	}
	Ok(())
}

fn recode_m4a_file(path: &Path) -> Result<(), String> {
	let content = match std::fs::read(path) {
		Ok(s) => s,
		Err(e) => {
			return Err(format!("Could not open file: {}: {}", path.display(), e));
		}
	};

	let frames = parse_mp4_frames(&content);
	// println!("mp4: {:?}", frames);
	for f in frames {
		println!("{}", f.string(0));
	}

	Ok(())
}

fn parse_mp4_frames(content: &[u8]) -> Vec<mp4::FileAtom> {
	let mut ret = Vec::new();
	let mut ix = 0;
	while ix < content.len() {
		let sz = u32::from_be_bytes(content[ix..ix + 4].try_into().unwrap());
		let name = std::str::from_utf8(&content[ix + 4..ix + 8]).unwrap();
		println!("parse_mp4_frames {} {} {}", ix, sz, name);
		match name {
			"ftyp" => ret.push(mp4::FileAtom::FileType(mp4::FileTypeBox::parse(
				sz,
				&content[ix + 8..ix + sz as usize],
			))),

			"moov" => ret.push(mp4::FileAtom::Movie(mp4::MovieBox::parse(
				sz,
				&content[ix + 8..ix + sz as usize],
			))),

			"free" => ret.push(mp4::FileAtom::FreeSpace(mp4::FreeSpaceBox::parse(
				sz,
				&content[ix + 8..ix + sz as usize],
			))),

			"mdat" => ret.push(mp4::FileAtom::MediaData(mp4::MediaDataBox::parse(
				sz,
				&content[ix + 8..ix + sz as usize],
			))),
			_ => {
				panic!("Unahndled type: {}", name);
			}
		}
		ix += sz as usize;
	}
	ret
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
		if let Some(ix) = old_list.iter().position(|f| f.id == code) {
			old_list.remove(ix);
		}
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

	let file_name_lossy = match path.file_name() {
		Some(x) => x,
		None => return Err(format!("Path has no file name: {}", path.display())),
	};

	let (mut frames, id3_size) = tag::read_id3_frames(&content);

	frames = frames
		.into_iter()
		.filter(|frame| {
			if cmd_flags.remove.contains(String::from_utf8_lossy(&frame.id).as_ref()) {
				println!("Dropping frame: {}", frame.display());
				return true;
			}

			false
		})
		.collect();

	{
		// Check for more than one frame of the same type
		let mut seen = HashSet::new();
		for frame in &frames {
			if seen.contains(&frame.id) {
				if frame.id == b"COMM".to_owned() && (cmd_flags.comment.is_some() || cmd_flags.combine_comments) {
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
	move_text_item(
		&mut new_frames,
		&mut frames,
		b"TCON".to_owned(),
		&cmd_flags.content_type,
	);

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
		frames = frames.into_iter().filter(|f| &f.id != b"COMM").collect();
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
		frames = frames.into_iter().filter(|f| &f.id != b"COMM").collect();
	}

	if let Some(ix) = frames.iter().position(|f| &f.id == b"APIC") {
		new_frames.push(frames.remove(ix));
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
				file_name_lossy.to_string_lossy(),
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
