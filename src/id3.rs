// use std::vec::Vec;

pub struct ID3v240Tag {
	pub header: ID3Header,
	pub extended_header: Option<ID3ExtendedHeader>,
	pub frames: Vec<ID3Frame>,
	pub padding: u32,
	pub has_footer: bool,
}

pub struct ID3Header {
	pub version_major: u8,
	pub version_minor: u8,
	pub flags: u8,
	pub size: u32,
}

impl ID3v240Tag {
	pub fn bytes(&self) -> Vec<u8> {
		let mut ret = Vec::<u8>::new();
		ret.extend(self.header.bytes());

		if let Some(ex) = &self.extended_header {
			ret.extend(ex.bytes());
		}

		for frame in &self.frames {
			ret.extend(frame.bytes());
		}

		ret.extend(std::iter::repeat(0).take(self.padding as usize));

		if self.has_footer {
			ret.extend(self.header.footer_bytes());
		}

		ret
	}
}

impl ID3Header {
	pub fn unsynchronisation(&self) -> bool {
		self.flags & 0b0100_0000 != 0
	}
	pub fn extended_header(&self) -> bool {
		self.flags & 0b0010_0000 != 0
	}
	pub fn experimental_indicator(&self) -> bool {
		self.flags & 0b0001_0000 != 0
	}
	pub fn footer_present(&self) -> bool {
		self.flags & 0b0000_1000 != 0
	}
	pub fn bytes(&self) -> Vec<u8> {
		let mut ret = Vec::<u8>::new();
		ret.reserve_exact(3 + 2 + 1 + 4);
		ret.extend("ID3".as_bytes());
		ret.push(self.version_major);
		ret.push(self.version_minor);
		ret.push(self.flags);
		ret.extend(&synchsafe_bytes(self.size));
		ret
	}
	pub fn footer_bytes(&self) -> Vec<u8> {
		let mut ret = Vec::<u8>::new();
		ret.reserve_exact(3 + 2 + 1 + 4);
		ret.extend("3DI".as_bytes());
		ret.push(self.version_major);
		ret.push(self.version_minor);
		ret.push(self.flags);
		ret.extend(&synchsafe_bytes(self.size));
		ret
	}
}

pub struct ID3ExtendedHeader {
	pub size: u32,
	pub fields: Vec<ID3ExtendedFlag>,
}

impl ID3ExtendedHeader {
	pub fn bytes(&self) -> Vec<u8> {
		let mut ret = Vec::<u8>::new();
		ret.extend(&synchsafe_bytes(self.size));
		for field in &self.fields {
			ret.extend(field.bytes());
		}
		ret
	}
}

pub struct ID3ExtendedFlag {
	// n: u8,
	pub flags: Vec<u8>,
}

impl ID3ExtendedFlag {
	pub fn bytes(&self) -> Vec<u8> {
		let mut ret = Vec::<u8>::new();
		ret.insert(0, self.flags.len() as u8);
		ret
	}
}

#[derive(Clone)]
pub struct ID3Frame {
	pub id: [u8; 4],
	pub flags: [u8; 2],
	pub data: ID3FrameType,
}

impl ID3Frame {
	pub fn bytes(&self) -> Vec<u8> {
		let mut ret = Vec::<u8>::new();
		ret.extend(&self.id);
		// ret.extend(&synchsafe_bytes(self.size));
		// ret.extend(&self.size.to_be_bytes());
		let data_bytes = &self.data.bytes();
		ret.extend(&synchsafe_bytes(data_bytes.len() as u32));
		ret.extend(&self.flags);
		ret.extend(data_bytes);
		ret
	}
	pub fn display(&self) -> String {
		String::from_utf8_lossy(&self.id).into_owned() + ":" + &self.data.display()
	}
}

#[derive(Clone)]
pub enum ID3FrameType {
	Text(ID3TextFrame),
	Picture(ID3PictureFrame),
	Comment(ID3CommentFrame),
}

impl ID3FrameType {
	pub fn len(&self) -> usize {
		match self {
			ID3FrameType::Text(f) => 1 + f.data.len(),
			ID3FrameType::Picture(f) => 1 + f.mime.len() + 1 + 1 + f.description.len() + 1 + f.data.len(),
			ID3FrameType::Comment(f) => 1 + 3 + f.content_desc.len() + 1 + f.text.len(),
		}
	}
	pub fn bytes(&self) -> Vec<u8> {
		match self {
			ID3FrameType::Text(f) => f.bytes(),
			ID3FrameType::Picture(f) => f.bytes(),
			ID3FrameType::Comment(f) => f.bytes(),
		}
	}
	pub fn display(&self) -> String {
		match self {
			ID3FrameType::Text(f) => f.data.clone(),
			ID3FrameType::Picture(_) => String::from("Picture"),
			ID3FrameType::Comment(f) => {
				let lang = String::from_utf8_lossy(&f.language);
				f.content_desc.clone() + ":" + &lang + ":" + &f.text
			}
		}
	}
}

#[derive(Clone)]
pub struct ID3TextFrame {
	// lang: [u8; 3],
	pub data: String,
	pub encoding: u8,
}

impl ID3TextFrame {
	pub fn bytes(&self) -> Vec<u8> {
		let encoding = if self.data.chars().all(|x| x.is_ascii()) { 0 } else { 3 };
		let mut ret = Vec::<u8>::with_capacity(1 + self.data.len());
		ret.push(encoding);
		ret.extend(self.data.as_bytes());
		ret
	}
}

#[derive(Clone)]
pub struct ID3PictureFrame {
	pub mime: String,
	pub pic_type: u8,
	pub description: String,
	pub data: Vec<u8>,
}

pub enum ID3PictureType {
	Other = 0,
	FileIcon32x32 = 1,
	OtherFileIcon = 2,
	CoverFront = 3,
	CoverBack = 4,
	LeafletPage = 5,
	Media = 6,
	LeadArtist = 7,
	ArtistPerformer = 0x08,
	Conductor = 0x09,
	BandOrchestra = 0x0A,
	Composer = 0x0B,
	LyricistTextWriter = 0x0C,
	RecordingLocation = 0x0D,
	DuringRecording = 0x0E,
	DuringPerformance = 0x0F,
	ScreenCapture = 0x10,
	ABrightColouredFish = 0x11,
	Illustration = 0x12,
	BandArtistLogotype = 0x13,
	PublisherStudioLogotype = 0x14,
}

pub fn pic_type_name(b: u8) -> Option<&'static str> {
	match b {
		0x00 => Some("Other"),
		0x01 => Some("32x32 pixels 'file icon' (PNG only)"),
		0x02 => Some("Other file icon"),
		0x03 => Some("Cover (front)"),
		0x04 => Some("Cover (back)"),
		0x05 => Some("Leaflet page"),
		0x06 => Some("Media (e.g. label side of CD)"),
		0x07 => Some("Lead artist/lead performer/soloist"),
		0x08 => Some("Artist/performer"),
		0x09 => Some("Conductor"),
		0x0A => Some("Band/Orchestra"),
		0x0B => Some("Composer"),
		0x0C => Some("Lyricist/text writer"),
		0x0D => Some("Recording Location"),
		0x0E => Some("During recording"),
		0x0F => Some("During performance"),
		0x10 => Some("Movie/video screen capture"),
		0x11 => Some("A bright coloured fish"),
		0x12 => Some("Illustration"),
		0x13 => Some("Band/artist logotype"),
		0x14 => Some("Publisher/Studio logotype"),
		_ => None,
	}
}

impl ID3PictureFrame {
	pub fn bytes(&self) -> Vec<u8> {
		let encoding = if self.description.chars().all(|x| x.is_ascii()) && self.mime.chars().all(|x| x.is_ascii()) {
			0
		} else {
			3
		};

		let mut ret =
			Vec::<u8>::with_capacity(1 + self.mime.len() + 1 + 1 + self.description.len() + 1 + self.data.len());
		ret.push(encoding);
		ret.extend(self.mime.as_bytes());
		ret.push(0);
		ret.push(self.pic_type);
		ret.extend(self.description.as_bytes());
		ret.push(0);
		ret.extend(&self.data);
		ret
	}
}

#[derive(Clone)]
pub struct ID3CommentFrame {
	// encoding: u8,
	pub language: [u8; 3], // eng
	pub content_desc: String,
	pub text: String,
	pub encoding: u8,
}

impl ID3CommentFrame {
	pub fn bytes(&self) -> Vec<u8> {
		let encoding = if self.content_desc.chars().all(|x| x.is_ascii()) && self.text.chars().all(|x| x.is_ascii()) {
			0
		} else {
			3
		};
		let mut ret = Vec::<u8>::with_capacity(1 + 3 + self.content_desc.len() + 1 + self.text.len());
		ret.push(encoding);
		ret.extend(&self.language);
		ret.extend(self.content_desc.as_bytes());
		ret.push(0);
		ret.extend(self.text.as_bytes());
		ret
	}
}

pub fn synchsafe_bytes(mut n: u32) -> [u8; 4] {
	let mut b: [u8; 4] = [0, 0, 0, 0];
	b[3] = (n % 128) as u8;
	n = n >> 7;
	b[2] = (n % 128) as u8;
	n = n >> 7;
	b[1] = (n % 128) as u8;
	n = n >> 7;
	b[0] = (n % 128) as u8;
	b
}

pub fn from_synchsafe(b: [u8; 4]) -> u32 {
	((b[0] as u32) << 21) + ((b[1] as u32) << 14) + ((b[2] as u32) << 7) + b[3] as u32
}
