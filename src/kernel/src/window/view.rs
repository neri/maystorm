// Views

use super::*;
// use crate::io::fonts::*;
use crate::io::graphics::*;
// use crate::num::*;
// use crate::*;
use alloc::boxed::Box;
// use alloc::vec::*;
// use bitflags::*;

pub trait ViewTrait {
    fn draw_if_needed(&mut self, _ctx: &Bitmap) {}
    fn move_to(&mut self, _window: Option<WindowHandle>) {}
    fn layout_if_needed(&mut self) {}
    fn set_needs_layout(&mut self) {}
    fn set_background_color(&mut self, _color: Color) {}
}

pub struct View {}

impl View {
    pub fn with_frame(_frame: Rect<isize>) -> Box<Self> {
        Box::new(Self {})
    }
}

impl ViewTrait for View {}
