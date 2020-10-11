// Views

use super::*;
use crate::io::fonts::*;
use crate::io::graphics::*;
use crate::num::*;
use alloc::boxed::Box;
use alloc::vec::*;
use bitflags::*;

pub trait ViewTrait {
    // View Hierarchies
    fn base_view(&self) -> &dyn ViewTrait;
    fn base_view_mut(&mut self) -> &mut dyn ViewTrait;

    fn subviews(&self) -> &[Box<dyn ViewTrait>] {
        self.base_view().subviews()
    }

    fn add_subview(&mut self, view: Box<dyn ViewTrait>) {
        self.base_view_mut().add_subview(view);
    }

    fn move_to(&mut self, window: Option<WindowHandle>) {
        self.base_view_mut().move_to(window);
    }

    fn window(&self) -> Option<WindowHandle> {
        self.base_view().window()
    }

    // Size and Position
    fn intrinsic_size(&self) -> Size<isize> {
        self.base_view().intrinsic_size()
    }

    fn frame(&self) -> Rect<isize> {
        self.base_view().frame()
    }

    fn set_frame(&mut self, frame: Rect<isize>) {
        self.base_view_mut().set_frame(frame)
    }

    fn bounds(&self) -> Rect<isize> {
        self.frame().size.into()
    }

    fn center(&self) -> Point<isize> {
        self.frame().center()
    }

    // Drawing
    fn draw_in_rect(&self, rect: Rect<isize>);

    fn background_color(&self) -> Color {
        self.base_view().background_color()
    }
    fn set_background_color(&mut self, color: Color) {
        self.base_view_mut().set_background_color(color)
    }
    fn tint_color(&self) -> Color {
        self.base_view().tint_color()
    }
    fn set_tint_color(&mut self, color: Color) {
        self.base_view_mut().set_tint_color(color)
    }
}

#[allow(dead_code)]
pub struct View {
    window: Option<WindowHandle>,
    frame: Rect<isize>,
    subviews: Vec<Box<dyn ViewTrait>>,
    background_color: Color,
    tint_color: Color,
    flaqs: ViewFlag,
}

bitflags! {
    struct ViewFlag: usize {
        const NEEDS_LAYOUT = 0b0000_0001;
        const NEEDS_DISPLAY = 0b0000_0001;
    }
}

impl View {
    pub fn with_frame(frame: Rect<isize>) -> Self {
        let mut view = Self::default();
        view.frame = frame;
        view
    }
}

impl Default for View {
    fn default() -> Self {
        Self {
            subviews: Vec::new(),
            window: None,
            frame: Rect::zero(),
            background_color: Color::WHITE,
            tint_color: Color::BLACK,
            flaqs: ViewFlag::empty(),
        }
    }
}

impl ViewTrait for View {
    fn base_view(&self) -> &dyn ViewTrait {
        self
    }

    fn base_view_mut(&mut self) -> &mut dyn ViewTrait {
        self
    }

    fn subviews(&self) -> &[Box<dyn ViewTrait>] {
        self.subviews.as_slice()
    }

    fn add_subview(&mut self, mut view: Box<dyn ViewTrait>) {
        view.move_to(self.window);
        self.subviews.push(view);
    }

    fn move_to(&mut self, window: Option<WindowHandle>) {
        self.window = window;
        for view in &mut self.subviews {
            view.move_to(window);
        }
    }

    fn window(&self) -> Option<WindowHandle> {
        self.window
    }

    fn intrinsic_size(&self) -> Size<isize> {
        self.frame.size
    }

    fn frame(&self) -> Rect<isize> {
        self.frame
    }

    fn set_frame(&mut self, frame: Rect<isize>) {
        self.frame = frame;
        // self.layout_subviews();
    }

    fn draw_in_rect(&self, rect: Rect<isize>) {
        if let Some(window) = self.window {
            window
                .draw_in_rect(rect, |bitmap| {
                    bitmap.fill_rect(self.frame, self.background_color);

                    for view in &self.subviews {
                        view.draw_in_rect(view.frame());
                    }
                })
                .unwrap();
        }
    }

    fn background_color(&self) -> Color {
        self.background_color
    }

    fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
    }

    fn tint_color(&self) -> Color {
        self.tint_color
    }

    fn set_tint_color(&mut self, color: Color) {
        self.tint_color = color;
    }
}

#[allow(dead_code)]
pub struct TextView<'a> {
    base_view: View,
    text: &'a str,
    font: &'a FontDriver<'a>,
    max_lines: usize,
}

impl<'a> TextView<'a> {
    pub fn new(text: &'a str) -> Box<Self> {
        let mut view = Box::new(Self {
            base_view: View::default(),
            text,
            font: FontDriver::system_font(),
            max_lines: 1,
        });
        view.set_background_color(Color::TRANSPARENT);
        view
    }
}

impl ViewTrait for TextView<'_> {
    fn base_view(&self) -> &dyn ViewTrait {
        &self.base_view
    }

    fn base_view_mut(&mut self) -> &mut dyn ViewTrait {
        &mut self.base_view
    }

    fn draw_in_rect(&self, rect: Rect<isize>) {
        if let Some(window) = self.window() {
            let _ = window.draw_in_rect(rect, |bitmap| {
                bitmap.fill_rect(rect, self.background_color());
                bitmap.draw_string(self.font, rect, self.tint_color(), self.text)
            });
        }
    }
}
