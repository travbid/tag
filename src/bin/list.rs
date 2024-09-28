use std::{fs::DirEntry, path::Path};
use tag::{id3, id3::ID3FrameType};

fn main() -> Result<(), i32> {
	let args: Vec<String> = std::env::args().collect();

	let opts = getopts::Options::new();
	let matches = match opts.parse(&args[1..]) {
		Ok(x) => x,
		Err(e) => {
			println!("Argument error: {}", e);
			return Err(1);
		}
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
		return match list_frames(Path::new(path)) {
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
		if let Err(e) = list_frames(&path.path()) {
			println!("Error on {}: {}", path.file_name().to_str().unwrap(), e);
			return Err(1);
		}
	}

	Ok(())
}

fn list_frames(path: &Path) -> Result<(), String> {
	println!("Name: {}", path.display());
	if path.file_name().unwrap_or_default().to_string_lossy().ends_with(".mp3") {
		list_mp3_frames(path)?;
	} else {
		panic!("Unhandled extension: {}", path.display());
	}
	Ok(())
}

fn list_mp3_frames(path: &Path) -> Result<(), String> {
	let content = match std::fs::read(path) {
		Ok(s) => s,
		Err(e) => {
			return Err(format!("Could not open file: {}: {}", path.display(), e));
		}
	};

	let (frames, _) = tag::read_id3_frames(&content);
	for frame in frames {
		println!("---------------");
		let flag = ((frame.flags[0] as u16) << 8) & frame.flags[1] as u16;
		println!(
			"{}{}{}{} {:#04X}",
			frame.id[0] as char, frame.id[1] as char, frame.id[2] as char, frame.id[3] as char, flag
		);
		match frame.data {
			ID3FrameType::Comment(f) => {
				println!(
					"   language: {}{}{}",
					f.language[0] as char, f.language[1] as char, f.language[2] as char
				);
				println!("description: {}", f.content_desc);
				println!("   encoding: {}", f.encoding);
				println!("       text: {}", f.text);
			}
			ID3FrameType::Picture(f) => {
				println!(
					"       type: {:#X} ({})",
					f.pic_type,
					id3::pic_type_name(f.pic_type).unwrap_or("???")
				);
				println!("       mime: {}", f.mime);
				println!("description: {}", f.description);
				println!("       size: {} bytes", f.data.len());
			}
			ID3FrameType::Text(f) => {
				if frame.id == *b"TXXX" {
					let mut description = String::new();
					let mut char_count = 0;
					for x in f.data.chars() {
						char_count += 1;
						if x == 0 as char {
							break;
						}
						description.push(x);
					}
					let value: String = f.data.chars().skip(char_count).collect();
					println!("{}: {}", description, value);
				} else {
					println!("   encoding: {}", f.encoding);
					println!("       text: {}", f.data);
				}
			}
		};
	}

	Ok(())
}
