use std::{path::Path, string::String};

fn main() -> Result<(), i32> {
	let args: Vec<String> = std::env::args().collect();

	// for arg in &args{
	// 	println!("arg: {}", arg);
	// }

	let path = &args[1];
	let comment = match read_comment(Path::new(path)) {
		Ok(x) => x,
		Err(e) => {
			eprintln!("{}", e);
			return Err(1);
		}
	};

	print!("{}", comment);
	Ok(())
}

fn read_comment(path: &Path) -> Result<String, String> {
	let content = match std::fs::read(path) {
		Ok(s) => s,
		Err(e) => {
			return Err(format!("Could not open file: {}: {}", path.display(), e));
		}
	};

	let (frames, _) = tag::read_id3_frames(&content);
	let comments = frames.iter().try_fold(Vec::new(), |mut acc, frame| {
		let code = std::str::from_utf8(&frame.id).unwrap();
		if code == "COMM" {
			match &frame.data {
				tag::id3::ID3FrameType::Comment(comment) => {
					acc.push(comment.text.clone());
					return Ok(acc);
				}
				_ => return Err(format!("COMM data does not have comment type")),
			}
		}
		Ok(acc)
	});

	comments.map(|v| v.join("\n"))
}
