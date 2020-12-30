// SVC Function Numbers (AUTO GENERATED)
use core::convert::TryFrom;

#[repr(u32)]
#[derive(Debug, Copy, Clone)]
pub enum Function {
    /// 0 : exit
    Exit = 0,
    /// 1 : print_string
    PrintString = 1,
    /// 2 : monotonic
    Monotonic = 2,
    /// 3 : usleep
    Usleep = 3,
    /// 4 : get_version
    GetVersion = 4,
    /// 5 : new_window
    NewWindow = 5,
    /// 6 : draw_text
    DrawText = 6,
    /// 7 : fill_rect
    FillRect = 7,
    /// 8 : wait_key
    WaitKey = 8,
    /// 9 : blt8
    Blt8 = 9,
    /// 10 : blt1
    Blt1 = 10,
    /// 11 : flash_window
    FlashWindow = 11,
    /// 12 : rand
    Rand = 12,
    /// 13 : srand
    Srand = 13,
    /// 14 : alloc
    Alloc = 14,
    /// 15 : free
    Free = 15,
}

impl TryFrom<u32> for Function {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Exit),
            1 => Ok(Self::PrintString),
            2 => Ok(Self::Monotonic),
            3 => Ok(Self::Usleep),
            4 => Ok(Self::GetVersion),
            5 => Ok(Self::NewWindow),
            6 => Ok(Self::DrawText),
            7 => Ok(Self::FillRect),
            8 => Ok(Self::WaitKey),
            9 => Ok(Self::Blt8),
            10 => Ok(Self::Blt1),
            11 => Ok(Self::FlashWindow),
            12 => Ok(Self::Rand),
            13 => Ok(Self::Srand),
            14 => Ok(Self::Alloc),
            15 => Ok(Self::Free),
            _ => Err(()),
        }
    }
}
