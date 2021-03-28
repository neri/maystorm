// ELF to CEEF
// Copyright (c) 2021 MEG-OS project

use core::mem::transmute;
use elf2ceef::{ceef::*, elf::*};
use std::io::Write;
use std::{cmp, env};
use std::{fs::File, process};
use std::{io::Read, path::Path};

fn usage() {
    let mut args = env::args_os();
    let arg = args.next().unwrap();
    let path = Path::new(&arg);
    let lpc = path.file_name().unwrap();
    eprintln!("{} INFILE OUTFILE", lpc.to_str().unwrap());
    process::exit(1);
}

fn main() {
    let mut args = env::args();
    let _ = args.next().unwrap();

    let in_file = match args.next() {
        Some(v) => v,
        None => return usage(),
    };
    let out_file = match args.next() {
        Some(v) => v,
        None => return usage(),
    };

    let mut is = File::open(in_file).unwrap();
    let mut blob = Vec::new();
    let _ = is.read_to_end(&mut blob).unwrap();

    let mut data: Vec<u8> = Vec::with_capacity(blob.len());

    let header: &Elf32Hdr = unsafe { transmute(&blob[0]) };

    assert!(
        header.is_valid() && header.e_type == ElfType::EXEC && header.e_machine == Machine::_386,
        "Bad executable"
    );

    const BASE_ADDR_MASK: u32 = 0xFFFFF000;
    let mut base_addr = u32::MAX;
    let mut minalloc = 0;
    let n_segments = header.e_phnum as usize;
    let mut ceef_sec_hdr: Vec<CeefSecHeader> = Vec::with_capacity(n_segments);

    println!("number of program headers {}", n_segments);
    for i in 0..n_segments {
        let phdr: &Elf32Phdr = unsafe {
            transmute(&blob[header.e_phoff as usize + (header.e_phentsize as usize) * i])
        };

        let ceef_hdr = CeefSecHeader::new(
            phdr.p_flags as u8,
            phdr.p_vaddr,
            phdr.p_filesz,
            phdr.p_memsz,
            if phdr.p_align != 0 {
                phdr.p_align.trailing_zeros() as u8
            } else {
                0
            },
        );

        println!(
            "Phdr #{} {} {} {:08x} {:08x} {:x}({:?}) {} {}",
            i,
            ceef_hdr.attr(),
            ceef_hdr.align(),
            ceef_hdr.vaddr,
            ceef_hdr.memsz,
            phdr.p_type as usize,
            phdr.p_type,
            phdr.p_offset,
            ceef_hdr.filesz,
        );

        if phdr.p_type == ElfSegmentType::LOAD {
            let max_addr = phdr.p_vaddr + phdr.p_memsz;
            base_addr = cmp::min(base_addr, phdr.p_vaddr & BASE_ADDR_MASK);
            minalloc = cmp::max(minalloc, max_addr);

            if phdr.p_filesz > 0 {
                let f_offset = phdr.p_offset as usize;
                let f_size = phdr.p_filesz as usize;
                data.extend(blob[f_offset..f_offset + f_size].iter());

                ceef_sec_hdr.push(ceef_hdr);
            }
        }
    }

    let mut new_header = CeefHeader::default();
    new_header.n_secs = ceef_sec_hdr.len() as u8;
    new_header.base = base_addr;
    new_header.minalloc = minalloc - base_addr;
    new_header.entry = header.e_entry;

    let mut os = File::create(out_file).unwrap();
    os.write_all(&new_header.as_bytes()).unwrap();
    for section in ceef_sec_hdr {
        os.write_all(&section.as_bytes()).unwrap();
    }
    os.write_all(data.as_slice()).unwrap();
}
