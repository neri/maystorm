// Make an initrd image
// Copyright(c) 2021 The MEG-OS Project

use byteorder::*;
use std::{
    cell::UnsafeCell,
    env,
    fs::{read_dir, File},
    io::Read,
    io::Write,
    path::Path,
    process,
};

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
        fs.append_file(path);
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
    const FLAG_DIR: u8 = 0x80;

    #[inline]
    pub fn file(name: &str) -> Option<Self> {
        Self::make_ent(name, 0)
    }

    #[inline]
    pub fn dir(name: &str) -> Option<Self> {
        Self::make_ent(name, Self::FLAG_DIR)
    }

    pub fn make_ent(name: &str, flag: u8) -> Option<Self> {
        if name.len() > Self::MAX_NANE_LEN {
            return None;
        }
        let flag = flag | name.len() as u8;
        let mut array = [0; Self::MAX_NANE_LEN];
        for (index, c) in name.char_indices() {
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
    vec: UnsafeCell<Vec<u8>>,
    dir: Vec<DirEnt>,
    path: String,
}

impl InitRamfs {
    const PADDING: usize = 16;

    #[inline]
    pub fn new() -> Self {
        Self {
            vec: UnsafeCell::new(Vec::new()),
            dir: Vec::new(),
            path: "".to_string(),
        }
    }

    #[inline]
    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    #[inline]
    pub fn appending_path(&self, lpc: &str) -> String {
        format!("{}/{lpc}", self.path)
    }

    #[inline]
    #[track_caller]
    pub fn child_dir<F, R>(&mut self, lpc: &str, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let mut dir_ent = DirEnt::dir(lpc).unwrap();

        let new_path = self.appending_path(lpc);
        println!("MAKE_DIR {new_path}");

        let Self {
            vec,
            dir: _,
            path: _,
        } = self;

        let vec = unsafe {
            let dummy = Vec::new();
            vec.get().replace(dummy)
        };

        let mut child = Self {
            vec: UnsafeCell::new(vec),
            dir: Vec::new(),
            path: new_path,
        };
        let result = f(&mut child);
        let vec = child._finalize_child_dir(&mut dir_ent);

        unsafe {
            self.vec.get().replace(vec);
        }

        self.dir.push(dir_ent);

        result
    }

    pub fn append_file(&mut self, path: &Path) {
        let lpc = path.file_name().unwrap().to_str().unwrap();
        if path.is_dir() {
            self.child_dir(lpc, |child| {
                for entry in read_dir(path).unwrap() {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if !path.file_name().unwrap().to_str().unwrap().starts_with(".") {
                        child.append_file(&path);
                    }
                }
            });
        } else if path.is_file() {
            println!(
                "APPEND FILE {} <= {}",
                self.appending_path(lpc),
                path.to_str().unwrap()
            );
            let dir_ent = DirEnt::file(lpc).expect("file name");
            let mut buf = Vec::new();
            let mut is = File::open(path).expect("cannot open file");
            is.read_to_end(&mut buf).expect("read file error");
            self._append_data(&dir_ent, buf.as_slice());
        } else {
            if path.ends_with("*") {
                //
            } else {
                todo!();
            }
        }
    }

    fn _finalize_child_dir(mut self, dir_ent: &mut DirEnt) -> Vec<u8> {
        let mut data = Vec::new();
        for dir_ent in self.dir {
            data.extend_from_slice(dir_ent.as_bytes());
        }
        let blob = self.vec.get_mut();
        dir_ent.offset = blob.len() as u32;
        dir_ent.file_size = data.len() as u32;
        blob.extend_from_slice(data.as_slice());
        match blob.len() % Self::PADDING {
            0 => (),
            remain => blob.resize(blob.len() + Self::PADDING - remain, 0),
        }

        self.vec.into_inner()
    }

    fn _append_data(&mut self, dir_ent: &DirEnt, data: &[u8]) {
        let mut dir_ent = *dir_ent;
        let blob = self.vec.get_mut();
        dir_ent.offset = blob.len() as u32;
        dir_ent.file_size = data.len() as u32;
        self.dir.push(dir_ent);
        blob.extend_from_slice(data);
        match blob.len() % Self::PADDING {
            0 => (),
            remain => blob.resize(blob.len() + Self::PADDING - remain, 0),
        }
    }

    pub fn flush(self, os: &mut dyn Write) -> Result<(), VirtualDiskError> {
        let blob = unsafe { &*self.vec.get() };
        let mut dir = Vec::with_capacity(self.dir.len());
        for dir_ent in &self.dir {
            dir.extend_from_slice(dir_ent.as_bytes());
        }
        const HEADER_SIZE: usize = 16;
        let mut header = [0u8; HEADER_SIZE];
        LE::write_u32(&mut header[0..4], 0x0001beef);
        LE::write_u32(&mut header[4..8], (HEADER_SIZE + blob.len()) as u32);
        LE::write_u32(&mut header[8..12], self.dir.len() as u32);
        LE::write_u32(
            &mut header[12..16],
            (HEADER_SIZE + blob.len() + dir.len()) as u32,
        );

        os.write_all(&header)
            .and_then(|_| os.write_all(blob.as_slice()))
            .and_then(|_| os.write_all(dir.as_slice()))
            .map_err(|_| VirtualDiskError::IoError)
    }
}

#[derive(Debug)]
pub enum VirtualDiskError {
    OutOfBounds,
    IoError,
}
