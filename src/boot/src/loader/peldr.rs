// Minimal PE File Loader

use crate::blob::*;
use crate::page::*;
use crate::*;
use bootinfo::pe::*;
use bootinfo::*;
use core::cmp;
use core::mem::*;

pub struct ImageLoader<'a> {
    blob: Blob<'a>,
    ne_ptr: usize,
    sec_tbl: usize,
}

impl<'a> ImageLoader<'a> {
    pub const fn new(blob: &'a [u8]) -> Self {
        Self {
            blob: Blob::new(blob),
            ne_ptr: 0,
            sec_tbl: 0,
        }
    }
}

#[allow(dead_code)]
impl ImageLoader<'_> {
    pub fn recognize(&mut self) -> Result<(), ()> {
        match self.blob.read_u16(0) {
            IMAGE_DOS_SIGNATURE => unsafe {
                self.ne_ptr = self.blob.read_u32(0x3C) as usize;
                let header: &PeHeader64 = self.blob.transmute(self.ne_ptr);
                if header.pe_signature == IMAGE_NT_SIGNATURE
                    && header.coff.machine == ImageFileMachine::AMD64
                    && header.coff.flags.contains(ImageFile::EXECUTABLE_IMAGE)
                    && header.optional.magic == Magic::PE64
                {
                    self.sec_tbl = self.ne_ptr
                        + 4
                        + size_of::<CoffHeader>()
                        + header.coff.size_of_optional as usize;
                    Ok(())
                } else {
                    Err(())
                }
            },
            _ => Err(()),
        }
    }

    pub fn locate(&self, info: &mut BootInfo) -> VirtualAddress {
        unsafe {
            let base = VirtualAddress(info.kernel_base);
            let header: &PeHeader64 = self.blob.transmute(self.ne_ptr);
            let image_base = header.optional.image_base;

            // Step 1 - allocate memory
            let size = header.optional.size_of_image as usize;
            let vmem = PageManager::valloc(base, size) as *const u8 as *mut u8;
            {
                let mut p = vmem;
                for _ in 0..size {
                    p.write_volatile(0);
                    p = p.add(1);
                }
            }

            println!(
                "Kernel Base: {:08x} => {:08x} Size: {:08x}",
                info.kernel_base, vmem as usize, header.optional.size_of_image
            );

            // Step 2 - locate sections
            let sec_tbl: &[SectionTable] = self
                .blob
                .transmute_slice(self.sec_tbl, header.coff.n_sections as usize);

            for section in sec_tbl {
                println!(
                    "Section: {} {:08x} {:08x} {:08x} {:08x} {:08x}",
                    core::str::from_utf8(&section.name).unwrap(),
                    section.vsize,
                    section.rva,
                    section.size,
                    section.file_offset,
                    section.flags.bits()
                );
                if section.size > 0 {
                    let mut p = vmem.add(section.rva as usize);
                    let mut q: *const u8 = self.blob.transmute(section.file_offset as usize);
                    let z = cmp::min(section.vsize, section.size);
                    for _ in 0..z {
                        p.write_volatile(q.read_volatile());
                        p = p.add(1);
                        q = q.add(1);
                    }
                }
            }

            // Step 3 - relocate
            // TODO:
            let reloc = header.dir[ImageDirectoryEntry::BaseReloc];
            let reloc_size = reloc.size as usize;
            let reloc_base = reloc.rva as usize;
            let mut iter = 0;
            println!("Reloc_DIR: {:08x} {:08x}", reloc_base, reloc_size);
            while iter < reloc_size {
                let reloc: &BaseReloc = transmute(vmem.add(reloc_base + iter));
                let count = reloc.count();
                println!(
                    "BaseReloc {:08x} {:08x} {}",
                    reloc_base + iter,
                    reloc.rva_base,
                    count
                );
                for i in 0..count {
                    let entry = reloc.entry(i);
                    let rva = reloc.rva_base as u64 + entry.value() as u64;
                    println!("RelocEntry: {} {}", entry.reloc_type().0, entry.value());
                    match entry.reloc_type() {
                        ImageRelBased::DIR64 => {
                            let p: *mut u64 = transmute(vmem.add(rva as usize));
                            p.write_volatile(p.read_volatile() - image_base + base.0);
                        }
                        _ => (),
                    }
                }
                iter += reloc.size as usize;
            }

            // Step 4 - attributes
            // TODO:

            base + header.optional.entry_point.into()
        }
    }
}
