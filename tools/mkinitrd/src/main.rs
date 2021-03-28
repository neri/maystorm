// Make an initrd image
// Copyright(c) 2021 The MEG-OS Project

use byteorder::*;
use std::{env, fs::File, io::Read, io::Write, path::Path, process};

fn usage() -> ! {
    let mut args = env::args_os();
    let arg = args.next().unwrap();
    let path = Path::new(&arg);
    let lpc = path.file_name().unwrap();
    eprintln!("{} [OPTIONS] OUTPUT [FILES...]", lpc.to_str().unwrap());
    process::exit(1);
}

fn main() {
    let mut args = env::args();
    let _ = args.next().unwrap();

    let mut path_output = None;

    while let Some(arg) = args.next() {
        let arg = arg.as_str();
        if arg.chars().next().unwrap_or_default() == '-' {
            match arg {
                "--" => {
                    path_output = args.next();
                    break;
                }
                _ => panic!("unknown option: {}", arg),
            }
        } else {
            path_output = Some(arg.to_owned());
            break;
        }
    }

    let path_output = match path_output {
        Some(v) => v,
        None => usage(),
    };

    let mut fs = InitRamfs::new();
    println!("CREATING archive: {}", path_output);

    for arg in args {
        let path = Path::new(&arg);
        let lpc = path.file_name().unwrap();
        let basename = lpc.to_str().unwrap();
        println!("COPYING: {} <= {}", basename, arg);

        let dir_ent = DirEnt::new(basename).expect("file name");
        let mut buf = Vec::new();
        {
            let mut is = File::open(path).expect("cannot open file");
            is.read_to_end(&mut buf).expect("read file error");
        }
        fs.append_file(&dir_ent, buf.as_slice());
    }

    let mut os = File::create(path_output).unwrap();
    fs.flush(&mut os).unwrap();
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct DirEnt {
    flag: u8,
    name: [u8; Self::MAX_NANE_LEN],
    _reserved: (u32, u32),
    offset: u32,
    file_size: u32,
}

impl DirEnt {
    const MAX_NANE_LEN: usize = 15;

    pub fn new(file_name: &str) -> Option<Self> {
        if file_name.len() > Self::MAX_NANE_LEN {
            return None;
        }
        let flag = file_name.len() as u8;
        let mut array = [0; Self::MAX_NANE_LEN];
        for (index, c) in file_name.char_indices() {
            array[index] = c as u8;
        }

        Some(Self {
            flag,
            name: array,
            _reserved: (0, 0),
            offset: 0,
            file_size: 0,
        })
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        unsafe { core::mem::transmute(self) }
    }
}

pub struct InitRamfs {
    blob: Vec<u8>,
    dir: Vec<DirEnt>,
}

impl InitRamfs {
    const PADDING: usize = 16;

    pub const fn new() -> Self {
        Self {
            blob: Vec::new(),
            dir: Vec::new(),
        }
    }

    pub fn append_file(&mut self, dir_ent: &DirEnt, blob: &[u8]) {
        let mut dir_ent = *dir_ent;
        dir_ent.offset = self.blob.len() as u32;
        dir_ent.file_size = blob.len() as u32;
        self.dir.push(dir_ent);
        self.blob.extend_from_slice(blob);
        match self.blob.len() % Self::PADDING {
            0 => (),
            remain => {
                let padding = Self::PADDING - remain;
                let mut vec = Vec::with_capacity(padding);
                vec.resize(padding, 0);
                self.blob.extend_from_slice(vec.as_slice());
            }
        }
    }

    pub fn flush(&self, os: &mut dyn Write) -> Result<(), VirtualDiskError> {
        let mut dir = Vec::with_capacity(self.dir.len());
        for dir_ent in &self.dir {
            dir.extend_from_slice(dir_ent.as_bytes());
        }
        const HEADER_SIZE: usize = 16;
        let mut header = [0u8; HEADER_SIZE];
        LE::write_u32(&mut header[0..4], 0x0001beef);
        LE::write_u32(&mut header[4..8], (HEADER_SIZE + self.blob.len()) as u32);
        LE::write_u32(&mut header[8..12], self.dir.len() as u32);
        LE::write_u32(
            &mut header[12..16],
            (HEADER_SIZE + self.blob.len() + dir.len()) as u32,
        );

        os.write_all(&header)
            .and_then(|_| os.write_all(self.blob.as_slice()))
            .and_then(|_| os.write_all(dir.as_slice()))
            .map_err(|_| VirtualDiskError::IoError)
    }
}

#[derive(Debug)]
pub enum VirtualDiskError {
    OutOfBounds,
    IoError,
}
