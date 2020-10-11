// Minimal PE File Loader

use crate::blob::*;
use crate::page::*;
use crate::*;
use bootprot::pe::*;
use core::cmp;
use core::mem::*;
use core::ptr;

pub struct ImageLoader<'a> {
    blob: Blob<'a>,
    lfa_new: usize,
    sec_tbl: usize,
}

impl<'a> ImageLoader<'a> {
    pub const fn new(blob: &'a [u8]) -> Self {
        Self {
            blob: Blob::new(blob),
            lfa_new: 0,
            sec_tbl: 0,
        }
    }
}

#[allow(dead_code)]
impl ImageLoader<'_> {
    pub fn recognize(&mut self) -> Result<(), ()> {
        match self.blob.read_u16(0) {
            IMAGE_DOS_SIGNATURE => unsafe {
                self.lfa_new = self.blob.read_u32(0x3C) as usize;
                let header: &PeHeader64 = self.blob.transmute(self.lfa_new);
                if header.is_valid()
                    && header.coff.machine == ImageFileMachine::AMD64
                    && header.coff.flags.contains(ImageFile::EXECUTABLE_IMAGE)
                {
                    self.sec_tbl = self.lfa_new + header.size();
                    Ok(())
                } else {
                    Err(())
                }
            },
            _ => Err(()),
        }
    }

    pub fn locate(&self, base: VirtualAddress) -> VirtualAddress {
        unsafe {
            let header: &PeHeader64 = self.blob.transmute(self.lfa_new);
            let image_base = header.optional.image_base;

            // Step 1 - allocate memory
            let size = header.optional.size_of_image as usize;
            let vmem = PageManager::valloc(base, size) as *const u8 as *mut u8;
            vmem.write_bytes(0, size);

            println!(
                "Kernel Base: {:08x} => {:08x} Size: {:08x}",
                base.0, vmem as usize, header.optional.size_of_image
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
                    let p = vmem.add(section.rva as usize);
                    let q: *const u8 = self.blob.transmute(section.file_offset as usize);
                    let z = cmp::min(section.vsize, section.size) as usize;
                    ptr::copy_nonoverlapping(q, p, z);
                }
            }

            // Step 3 - relocate
            let reloc = header.optional.dir[ImageDirectoryEntry::BASERELOC];
            for iter in BaseReloc::new(vmem.add(reloc.rva as usize), reloc.size as usize) {
                for (ty, rva) in iter {
                    match ty {
                        ImageRelBased::ABSOLUTE => (),
                        ImageRelBased::DIR64 => {
                            let p: *mut u64 = transmute(vmem.add(rva as usize));
                            p.write_volatile(p.read_volatile() - image_base + base.0);
                        }
                        _ => (),
                    }
                }
            }

            // Step 4 - attributes
            for section in sec_tbl {
                let mut prot = MProtect::empty();
                if section.flags.contains(ImageScn::MEM_READ) {
                    prot.insert(MProtect::READ);
                }
                if section.flags.contains(ImageScn::MEM_WRITE) {
                    prot.insert(MProtect::WRITE);
                }
                if section.flags.contains(ImageScn::MEM_EXECUTE) {
                    prot.insert(MProtect::EXEC);
                }
                PageManager::vprotect(base + section.rva, section.vsize as usize, prot);
            }

            base + header.optional.entry_point
        }
    }
}
