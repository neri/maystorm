//! MEG-OS Arlequin System Call Function Numbers

use num_derive::FromPrimitive;
// use num_traits::FromPrimitive;

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum Function {
    /// Terminate the process normally
    Exit = 0,
    /// Display a string
    PrintString,
    /// Get the value of the monotonic timer in microseconds
    Monotonic,
    /// Perform the time service
    Time,
    /// Blocks a thread for the specified microseconds
    Usleep,
    /// Get the system information
    GetSystemInfo,
    /// Create a new window
    NewWindow,
    /// Close a window
    CloseWindow,
    /// Create a drawing context
    BeginDraw,
    /// Discard the drawing context and reflect it to the screen
    EndDraw,
    /// Draw a string in a window
    DrawString,
    /// Fill a rectangle in a window
    FillRect,
    /// Draw a rectangle in a window
    DrawRect,
    /// Draw a line in a window
    DrawLine,
    /// Draw a bitmap in a window
    Blt8,
    /// Draw a bitmap in a window
    Blt1,
    /// Draw a bitmap in a window
    Blt32,
    /// Blend (test)
    BlendRect,
    /// Wait for char event
    WaitChar,
    /// Read a char event
    ReadChar,

    /// Draw a view
    DrawView,

    /// Return a random number
    Rand = 100,
    /// Set the seed of the random number
    Srand,
    /// Allocates memory blocks with a simple allocator
    Alloc,
    /// Frees an allocated memory block
    Dealloc,
}
