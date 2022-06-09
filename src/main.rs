use image::load_from_memory;
use show_image::{create_window, ImageInfo, ImageView, WindowOptions};

use std::env::args;
use std::fmt::{Display, Error, Formatter};
use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;

#[derive(Debug)]
enum ReadResult {
    InvalidResource(String),
}

impl ReadResult {
    fn invalid_resource(rsrc: &[u8; 4]) -> ReadResult {
        ReadResult::InvalidResource(rsrc.iter().map(|x| *x as char).collect::<String>())
    }
}

#[derive(Debug, Copy, Clone)]
enum ChunkType {
    Pict,
    Sound,
    Data,
    Exec,
}

#[derive(Debug)]
enum PictResource {
    Png { data: Vec<u8> },
    Jpeg { data: Vec<u8> },
    Rect { width: usize, height: usize },
}

#[derive(Debug)]
enum SoundResource {
    Aiff { data: Vec<u8> },
    Ogg { data: Vec<u8> },
    Mod { data: Vec<u8> },
}

#[derive(Debug)]
enum DataResource {
    Text { data: Vec<u8> },
    Bina { data: Vec<i32> },
}

#[derive(Debug)]
enum ChunkResource {
    Pict(PictResource),
    Exec(Vec<u8>),
    Sound(SoundResource),
    Data(DataResource),
}

impl Display for ChunkType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self {
            ChunkType::Pict => write!(f, "'Pict'"),
            ChunkType::Sound => write!(f, "'Snd '"),
            ChunkType::Data => write!(f, "'Data'"),
            ChunkType::Exec => write!(f, "'Exec'"),
        }
    }
}

#[derive(Debug)]
struct ChunkInfo {
    usage: ChunkType,
    number: usize,
    start: u64,
    data: Vec<u8>,
}

impl Display for ChunkInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(
            f,
            "[{}: {} @{}{}]",
            self.number,
            self.usage,
            self.start,
            if self.data.is_empty() {
                ""
            } else {
                " (loaded)"
            }
        )
    }
}

const FORM: [u8; 4] = [b'F', b'O', b'R', b'M'];
const IFRS: [u8; 4] = [b'I', b'F', b'R', b'S'];
const RIDX: [u8; 4] = [b'R', b'I', b'd', b'x'];

const PICT: [u8; 4] = [b'P', b'i', b'c', b't'];
const EXEC: [u8; 4] = [b'E', b'x', b'e', b'c'];
const SND_: [u8; 4] = [b'S', b'n', b'd', b' '];
const DATA: [u8; 4] = [b'D', b'a', b't', b'a'];

const JPEG: [u8; 4] = [b'J', b'P', b'E', b'G'];
const PNG_: [u8; 4] = [b'P', b'N', b'G', b' '];
const RECT: [u8; 4] = [b'R', b'e', b'c', b't'];

const OGGV: [u8; 4] = [b'O', b'G', b'G', b'V'];
const AIFF: [u8; 4] = [b'A', b'I', b'F', b'F'];
const MOD_: [u8; 4] = [b'M', b'O', b'D', b' '];

const TEXT: [u8; 4] = [b'T', b'E', b'X', b'T'];
const BINA: [u8; 4] = [b'B', b'I', b'N', b'A'];

#[show_image::main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argv = args().collect::<Vec<String>>();

    if argv.len() == 1 {
        println!("{} <filanem>\n", argv[0]);
        std::process::exit(1);
    }

    let mut f = match File::open(&argv[1]) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Can't open {}: {e}", argv[1]);
            std::process::exit(1);
        }
    };

    let file_type = read_type(&mut f);
    let file_size = read_size(&mut f);
    if file_type != FORM {
        eprintln!("File is not an IFF document");
        std::process::exit(1);
    }

    let form_type = read_type(&mut f);
    if form_type != IFRS {
        eprintln!("IFF document is not a Blorb file");
        std::process::exit(1);
    }

    let ridx_type = read_type(&mut f);
    if ridx_type != RIDX {
        eprintln!("Invalid Blorb file");
        std::process::exit(1);
    }

    let ridx_size = read_size(&mut f);
    let resource_count = read_size(&mut f);
    println!("There are {resource_count} entries in the Blorb file");

    let mut toc = Vec::new();
    for _ in 0..resource_count {
        match read_resource_info(&mut f) {
            Ok(chunk) => toc.push(chunk),
            Err(e) => {
                eprintln!("Invalid Blorb file: {e:?}");
                std::process::exit(1);
            }
        }
    }
    println!("FORM size: {} bytes", file_size);
    println!("RIDx size: {} bytes", ridx_size);
    println!("Resources:");
    for t in &mut toc {
        print!("\t{t} - ");

        match read_chunk(&mut f, t) {
            Ok(ChunkResource::Pict(p)) => match p {
                PictResource::Jpeg { data } => {
                    let (w, h) = show_image(t.number, data)?;
                    println!("jpeg {w}x{h}");
                }
                PictResource::Png { data } => {
                    let (w, h) = show_image(t.number, data)?;
                    println!("png {w}x{h}");
                }
                PictResource::Rect { width, height } => {
                    println!("Rect {width}x{height}")
                }
            },
            Ok(ChunkResource::Exec(_)) => {
                println!("TODO");
            }
            Ok(ChunkResource::Sound(s)) => match s {
                SoundResource::Aiff { data } => println!("AIFF ({} bytes)", data.len()),
                SoundResource::Ogg { data } => println!("Ogg ({} bytes)", data.len()),
                SoundResource::Mod { data } => println!("MOD ({} bytes)", data.len()),
            },
            Ok(ChunkResource::Data(d)) => match d {
                DataResource::Text { data } => println!("Text ({} chars)", data.len()),
                DataResource::Bina { data } => println!("Binary ({} words)", data.len()),
            },
            Err(_) => break,
        }
    }

    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);

    Ok(())
}

fn read_type(f: &mut File) -> [u8; 4] {
    let mut buffer = [0u8; 4];
    let _ = f.read(&mut buffer);
    buffer
}

fn read_size(f: &mut File) -> usize {
    let mut buffer = [0u8; 4];
    let _ = f.read(&mut buffer);
    ((buffer[0] as usize) << 24)
        | ((buffer[1] as usize) << 16)
        | ((buffer[2] as usize) << 8)
        | (buffer[3] as usize)
}

fn read_resource_info(f: &mut File) -> Result<ChunkInfo, ReadResult> {
    let mut usage_id = [0u8; 4];
    let _ = f.read(&mut usage_id);
    let usage = match usage_id {
        PICT => ChunkType::Pict,
        SND_ => ChunkType::Sound,
        DATA => ChunkType::Data,
        EXEC => ChunkType::Exec,
        _ => return Err(ReadResult::invalid_resource(&usage_id)),
    };
    let number = read_size(f);
    let start = read_size(f) as u64;
    Ok(ChunkInfo {
        usage,
        number,
        start,
        data: Vec::new(),
    })
}

fn read_chunk(f: &mut File, chunk: &ChunkInfo) -> Result<ChunkResource, ReadResult> {
    match chunk.usage {
        ChunkType::Pict => Ok(ChunkResource::Pict(read_pict(f, chunk.start))),
        ChunkType::Exec => Ok(ChunkResource::Exec(Vec::new())),
        ChunkType::Sound => Ok(ChunkResource::Sound(read_sound(f, chunk.start)?)),
        ChunkType::Data => Ok(ChunkResource::Data(read_data(f, chunk.start)?)),
    }
}

fn read_pict(f: &mut File, offset: u64) -> PictResource {
    let _ = f.seek(SeekFrom::Start(offset));
    let chunk_type = read_type(f);
    let chunk_len = read_size(f);

    let mut data = vec![0u8; chunk_len];
    let _ = f.read(&mut data);

    match chunk_type {
        JPEG => PictResource::Jpeg { data },
        PNG_ => PictResource::Png { data },
        RECT => {
            let len = read_size(f);
            assert_eq!(len, 8);
            let width = read_size(f);
            let height = read_size(f);
            PictResource::Rect { width, height }
        }
        _ => unimplemented!("{chunk_type:?}"),
    }
}

fn read_sound(f: &mut File, offset: u64) -> Result<SoundResource, ReadResult> {
    let _ = f.seek(SeekFrom::Start(offset));
    let chunk_type = read_type(f);
    let chunk_len = read_size(f);

    let mut data = vec![0u8; chunk_len];
    let _ = f.read(&mut data);

    match chunk_type {
        OGGV => Ok(SoundResource::Ogg { data }),
        MOD_ => Ok(SoundResource::Mod { data }),
        AIFF => Ok(SoundResource::Aiff { data }),
        _ => Err(ReadResult::invalid_resource(&chunk_type)),
    }
}

fn read_data(f: &mut File, offset: u64) -> Result<DataResource, ReadResult> {
    let _ = f.seek(SeekFrom::Start(offset));
    let chunk_type = read_type(f);
    let chunk_len = read_size(f);

    let mut data = vec![0u8; chunk_len];
    let _ = f.read(&mut data);

    match chunk_type {
        TEXT => Ok(DataResource::Text { data }),
        BINA => {
            let mut bin_data = Vec::new();
            for i in data.chunks(4) {
                let xdata: u32 = ((i[0] as u32) << 24)
                    | ((i[1] as u32) << 16)
                    | ((i[2] as u32) << 8)
                    | (i[3] as u32);
                let lower_31 = xdata & 0x7FFF_FFFF;
                let sign_bit = xdata & 0x8000_0000;
                let value: i32 = if sign_bit != 0 {
                    -(lower_31 as i32)
                } else {
                    lower_31 as i32
                };
                bin_data.push(value)
            }
            Ok(DataResource::Bina { data: bin_data })
        }
        _ => Err(ReadResult::invalid_resource(&chunk_type)),
    }
}

fn show_image(id: usize, data: Vec<u8>) -> Result<(u32, u32), Box<dyn std::error::Error>> {
    let decoded_data = load_from_memory(&data)?.into_rgba8();
    let (width, height) = (decoded_data.width(), decoded_data.height());

    let raw_data = decoded_data.as_raw();
    let image = ImageView::new(ImageInfo::rgba8(width, height), raw_data);

    let opts = WindowOptions {
        size: Some([width as u32, height as u32]),
        ..Default::default()
    };

    let window = create_window(format!("Image {}", id), opts)?;
    window.set_image("image-thing", image)?;

    Ok((width, height))
}
