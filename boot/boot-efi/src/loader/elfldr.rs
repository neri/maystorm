// Minimal ELF File Loader

use super::{elf::*, *};
use crate::{blob::*, page::*};
use core::ptr::copy_nonoverlapping;

pub struct ElfLoader<'a> {
    blob: Blob<'a>,
    image_base: VirtualAddress,
    image_size: usize,
}

impl<'a> ElfLoader<'a> {
    #[inline]
    pub const fn new(blob: &'a [u8]) -> Self {
        Self {
            blob: Blob::new(blob),
            image_base: VirtualAddress(0),
            image_size: 0,
        }
    }

    #[inline]
    fn program_header(&self) -> ElfProgramHeaderIter {
        unsafe {
            let elf_hdr: &Elf64Hdr = self.blob.transmute(0);
            ElfProgramHeaderIter::new(
                self.blob.base().add(elf_hdr.e_phoff as usize),
                elf_hdr.e_phentsize as usize,
                elf_hdr.e_phnum as usize,
            )
        }
    }
}

impl ImageLoader for ElfLoader<'_> {
    fn recognize(&mut self) -> Result<(), ()> {
        let preferred_machine = Machine::X86_64;
        let elf_hdr = unsafe { self.blob.transmute::<Elf64Hdr>(0) };
        if elf_hdr.is_valid()
            && elf_hdr.e_type == ElfType::EXEC
            && elf_hdr.e_machine == preferred_machine
        {
            let ph = self.program_header();
            let page_mask = PageConfig::UEFI_PAGE_SIZE - 1;
            let image_base = VirtualAddress(
                ph.clone()
                    .filter(|v| v.p_type == ElfSegmentType::LOAD)
                    .fold(u64::MAX, |a, v| u64::min(a, v.p_vaddr))
                    & !page_mask,
            );
            let image_size = ((ph
                .clone()
                .filter(|v| v.p_type == ElfSegmentType::LOAD)
                .fold(0, |a, v| u64::max(a, v.p_vaddr + v.p_memsz))
                - image_base.as_u64()
                + page_mask)
                & !page_mask) as usize;

            self.image_base = image_base;
            self.image_size = image_size;

            Ok(())
        } else {
            Err(())
        }
    }

    #[inline]
    fn image_bounds(&self) -> (VirtualAddress, usize) {
        (self.image_base, self.image_size)
    }

    fn locate(&self, _base: VirtualAddress) -> VirtualAddress {
        unsafe {
            let elf_hdr = self.blob.transmute::<Elf64Hdr>(0);
            let image_base = self.image_base;
            let image_size = self.image_size;

            // Step 1 - allocate memory
            let page_mask = PageConfig::UEFI_PAGE_SIZE - 1;
            let vmem = PageManager::valloc(image_base, image_size) as *mut u8;
            vmem.write_bytes(0, image_size);

            // Step 2 - locate segments
            for item in self.program_header() {
                if item.p_type == ElfSegmentType::LOAD {
                    let rva = (item.p_vaddr - image_base.as_u64()) as usize;
                    let p = vmem.add(rva);
                    let q: *const u8 = self.blob.transmute(item.p_offset as usize);
                    let z = item.p_filesz as usize;
                    copy_nonoverlapping(q, p, z);
                }
            }

            // Step 3 - relocation
            // TODO:

            // Step 4 - attributes
            for item in self.program_header() {
                if item.p_type == ElfSegmentType::LOAD {
                    let va = VirtualAddress(item.p_vaddr & !page_mask);
                    let size = ((item.p_memsz + item.p_vaddr - va.as_u64() + page_mask)
                        & !page_mask) as usize;
                    let prot = MProtect::from_bits_truncate(item.p_flags as usize);
                    PageManager::vprotect(va, size, prot);
                }
            }

            VirtualAddress(elf_hdr.e_entry)
        }
    }
}

#[derive(Clone)]
struct ElfProgramHeaderIter {
    base: *const u8,
    entry_size: usize,
    n_entries: usize,
    index: usize,
}

impl ElfProgramHeaderIter {
    #[inline]
    const fn new(base: *const u8, entry_size: usize, n_entries: usize) -> Self {
        Self {
            base,
            entry_size,
            n_entries,
            index: 0,
        }
    }
}

impl Iterator for ElfProgramHeaderIter {
    type Item = Elf64Phdr;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.n_entries {
            let item = unsafe {
                let p = self.base.add(self.index * self.entry_size) as *const Elf64Phdr;
                *p
            };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}
