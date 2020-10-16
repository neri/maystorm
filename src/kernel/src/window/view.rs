// Views

use super::*;
use crate::io::fonts::*;
use crate::io::graphics::*;
use crate::num::*;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use bitflags::*;

pub trait ViewTrait {
    fn base_view(&self) -> &dyn ViewTrait;
    fn base_view_mut(&mut self) -> &mut dyn ViewTrait;
    fn class_name(&self) -> &str;

    // View Hierarchies
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

    fn set_needs_layout(&mut self) {
        self.base_view_mut().set_needs_layout();
    }

    fn layout_subviews(&mut self) {
        self.base_view_mut().layout_subviews();
    }

    fn layout_if_needed(&mut self) {
        self.base_view_mut().layout_if_needed();
    }

    // Drawing

    fn set_needs_display(&mut self) {
        self.base_view_mut().set_needs_display();
    }

    fn draw_in_context(&self, ctx: Bitmap) {
        self.base_view().draw_in_context(ctx);
    }

    fn draw_if_needed(&self, ctx: Bitmap) {
        self.base_view().draw_if_needed(ctx);
    }

    // Basic Properties

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

    fn border_color(&self) -> Color {
        self.base_view().border_color()
    }
    fn set_border_color(&mut self, color: Color) {
        self.base_view_mut().set_border_color(color)
    }

    fn corner_radius(&self) -> isize {
        self.base_view().corner_radius()
    }
    fn set_corner_radius(&mut self, radius: isize) {
        self.base_view_mut().set_corner_radius(radius);
    }

    fn is_enabled(&self) -> bool {
        self.base_view().is_enabled()
    }
    fn set_is_enabled(&mut self, enabled: bool) {
        self.base_view_mut().set_is_enabled(enabled);
    }

    fn is_selected(&self) -> bool {
        self.base_view().is_selected()
    }
    fn set_is_selected(&mut self, selected: bool) {
        self.base_view_mut().set_is_selected(selected);
    }

    fn is_hidden(&self) -> bool {
        self.base_view().is_hidden()
    }
    fn set_is_hidden(&mut self, hidden: bool) {
        self.base_view_mut().set_is_hidden(hidden);
    }
}

/// Base implemantation of all views
pub struct BaseView {
    window: Option<WindowHandle>,
    frame: Rect<isize>,
    subviews: Vec<Box<dyn ViewTrait>>,
    background_color: Color,
    tint_color: Color,
    border_color: Color,
    corner_radius: isize,
    flags: ViewFlag,
}

bitflags! {
    struct ViewFlag: usize {
        const ENABLED       = 0b0000_0000_0001;
        const SELECTED      = 0b0000_0000_0010;
        const HIDDEN        = 0b0000_0000_0100;
        const NEEDS_LAYOUT  = 0b0000_0001_0000;
        const NEEDS_DISPLAY = 0b0000_0010_0000;

        const DEFAULT = Self::NEEDS_DISPLAY.bits() | Self::NEEDS_LAYOUT.bits();
    }
}

impl BaseView {
    pub fn with_frame(frame: Rect<isize>) -> Self {
        let mut view = Self::default();
        view.frame = frame;
        view
    }

    fn common_draw(&self, ctx: &Bitmap) {
        if !self.background_color().is_transparent() {
            if self.corner_radius() > 0 {
                ctx.fill_round_rect(self.bounds(), self.corner_radius(), self.background_color());
            } else {
                ctx.blend_rect(self.bounds(), self.background_color());
            }
        }
        if !self.border_color().is_transparent() {
            ctx.draw_round_rect(self.bounds(), self.corner_radius(), self.border_color());
        }
    }
}

impl Default for BaseView {
    fn default() -> Self {
        Self {
            subviews: Vec::new(),
            window: None,
            frame: Rect::zero(),
            background_color: Color::WHITE,
            tint_color: IndexedColor::Black.into(),
            border_color: Color::TRANSPARENT,
            corner_radius: 0,
            flags: ViewFlag::DEFAULT,
        }
    }
}

impl ViewTrait for BaseView {
    fn base_view(&self) -> &dyn ViewTrait {
        self
    }

    fn base_view_mut(&mut self) -> &mut dyn ViewTrait {
        self
    }

    fn class_name(&self) -> &str {
        "BaseView"
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
        self.set_needs_layout();
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

        self.set_needs_layout();
    }

    fn set_needs_layout(&mut self) {
        self.flags.insert(ViewFlag::NEEDS_LAYOUT);

        for view in &mut self.subviews {
            view.set_needs_layout();
        }
    }

    fn layout_subviews(&mut self) {
        self.flags.remove(ViewFlag::NEEDS_LAYOUT);

        for view in &mut self.subviews {
            view.layout_subviews();
        }

        // self.set_needs_display();
    }

    fn layout_if_needed(&mut self) {
        if self.flags.contains(ViewFlag::NEEDS_LAYOUT) {
            let old_frame = self.frame;
            self.layout_subviews();
            if old_frame != self.frame {
                self.set_needs_display();
            }
        } else {
            for view in &mut self.subviews {
                view.layout_if_needed();
            }
        }
    }

    fn set_needs_display(&mut self) {
        self.flags.insert(ViewFlag::NEEDS_DISPLAY);
    }

    fn draw_if_needed(&self, ctx: Bitmap) {
        if self.flags.contains(ViewFlag::NEEDS_DISPLAY) {
            self.draw_in_context(ctx);
        }
    }

    fn draw_in_context(&self, ctx: Bitmap) {
        for view in &self.subviews {
            if let Some(ctx) = ctx.view(view.frame()) {
                view.draw_in_context(ctx);
            }
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

    fn border_color(&self) -> Color {
        self.border_color
    }
    fn set_border_color(&mut self, color: Color) {
        self.border_color = color;
    }

    fn corner_radius(&self) -> isize {
        self.corner_radius
    }
    fn set_corner_radius(&mut self, radius: isize) {
        self.corner_radius = radius;
    }

    fn is_enabled(&self) -> bool {
        self.flags.contains(ViewFlag::ENABLED)
    }
    fn set_is_enabled(&mut self, enabled: bool) {
        self.flags.set(ViewFlag::ENABLED, enabled);
    }

    fn is_selected(&self) -> bool {
        self.flags.contains(ViewFlag::SELECTED)
    }
    fn set_is_selected(&mut self, selected: bool) {
        self.flags.set(ViewFlag::SELECTED, selected);
    }

    fn is_hidden(&self) -> bool {
        self.flags.contains(ViewFlag::HIDDEN)
    }
    fn set_is_hidden(&mut self, hidden: bool) {
        self.flags.set(ViewFlag::HIDDEN, hidden);
    }
}

/// Plain View
pub struct View {
    base_view: BaseView,
}

impl View {
    pub fn with_frame(frame: Rect<isize>) -> Box<Self> {
        Box::new(Self {
            base_view: BaseView::with_frame(frame),
        })
    }
}

impl ViewTrait for View {
    fn base_view(&self) -> &dyn ViewTrait {
        &self.base_view
    }

    fn base_view_mut(&mut self) -> &mut dyn ViewTrait {
        &mut self.base_view
    }

    fn class_name(&self) -> &str {
        "View"
    }

    fn draw_in_context(&self, ctx: Bitmap) {
        // println!("View::draw_in_context {:08x}", &self as *const _ as usize);

        self.base_view.common_draw(&ctx);

        self.base_view().draw_in_context(ctx);
    }
}

/// Text View
#[allow(dead_code)]
pub struct TextView<'a> {
    base_view: BaseView,
    text: AttributedString<'a>,
    max_lines: usize,
    intrinsic_size: Size<isize>,
}

impl<'a> TextView<'a> {
    pub fn new() -> Box<Self> {
        let mut view = Box::new(Self {
            base_view: BaseView::default(),
            text: AttributedString::new(""),
            max_lines: 1,
            intrinsic_size: Size::zero(),
        });
        view.set_background_color(Color::TRANSPARENT);
        view
    }

    pub fn with_text(text: &'a str) -> Box<Self> {
        let mut view = Self::new();
        view.set_text(text);
        view
    }

    pub fn set_text(&mut self, text: &'a str) {
        self.text.set_text(text);
        self.set_needs_layout();
    }

    pub fn set_font(&mut self, font: FontDescriptor) {
        self.text.set_font(font);
        self.set_needs_layout();
    }

    pub fn set_max_lines(&mut self, max_lines: usize) {
        self.max_lines = max_lines;
        self.set_needs_layout();
    }

    pub fn max_libnes(&self) -> usize {
        self.max_lines
    }
}

impl ViewTrait for TextView<'_> {
    fn base_view(&self) -> &dyn ViewTrait {
        &self.base_view
    }

    fn base_view_mut(&mut self) -> &mut dyn ViewTrait {
        &mut self.base_view
    }

    fn class_name(&self) -> &str {
        "TextView"
    }

    fn layout_subviews(&mut self) {
        self.base_view_mut().layout_subviews();

        let mut max_width = self.frame().width();
        if max_width <= 0 {
            max_width = isize::MAX;
        }
        let max_height = if self.max_lines > 0 {
            self.max_lines as isize * self.text.font().line_height()
        } else {
            isize::MAX
        };
        self.intrinsic_size = if self.text.text().len() > 0 {
            self.text.bounding_size(Size::new(max_width, max_height))
        } else {
            Size::new(0, 0)
        };
    }

    fn intrinsic_size(&self) -> Size<isize> {
        self.intrinsic_size
    }

    fn set_tint_color(&mut self, color: Color) {
        self.text.set_color(color);
        self.base_view_mut().set_tint_color(color);
    }

    fn draw_in_context(&self, ctx: Bitmap) {
        self.base_view.common_draw(&ctx);

        // ctx.draw_rect(self.intrinsic_size().into(), IndexedColor::Blue.into());
        // ctx.draw_rect(self.bounds(), IndexedColor::Red.into());

        if self.text.text().len() > 0 {
            self.text.draw(&ctx, self.intrinsic_size().into());
        }

        self.base_view().draw_in_context(ctx);
    }
}

/// A Button
pub struct Button<'a> {
    base_view: BaseView,
    title_label: Box<TextView<'a>>,
    title_insets: EdgeInsets<isize>,
    button_type: ButtonType,
}

#[derive(Debug, Clone, Copy)]
pub enum ButtonType {
    Default,
    Destructive,
    Normal,
}

impl<'a> Button<'a> {
    pub fn new(button_type: ButtonType) -> Box<Self> {
        let mut button = Box::new(Self {
            base_view: BaseView::default(),
            title_label: TextView::new(),
            title_insets: EdgeInsets::new(4, 16, 4, 16),
            button_type,
        });
        button.set_corner_radius(12);
        button.set_button_type(button_type);
        button.title_label.set_font(FontDescriptor::label_font());

        button
    }

    pub fn set_title(&mut self, text: &'a str) {
        self.title_label.set_text(text);
    }

    pub fn set_button_type(&mut self, button_type: ButtonType) {
        self.button_type = button_type;
        match button_type {
            ButtonType::Default => {
                self.set_background_color(IndexedColor::LightBlue.into());
                self.set_tint_color(Color::WHITE);
                self.set_border_color(Color::TRANSPARENT);
            }
            ButtonType::Destructive => {
                self.set_background_color(IndexedColor::LightRed.into());
                self.set_tint_color(Color::WHITE);
                self.set_border_color(Color::TRANSPARENT);
            }
            ButtonType::Normal => {
                self.set_background_color(Color::WHITE);
                self.set_tint_color(IndexedColor::DarkGray.into());
                self.set_border_color(IndexedColor::DarkGray.into());
            }
        }
    }
}

impl<'a> ViewTrait for Button<'a> {
    fn base_view(&self) -> &dyn ViewTrait {
        &self.base_view
    }

    fn base_view_mut(&mut self) -> &mut dyn ViewTrait {
        &mut self.base_view
    }

    fn class_name(&self) -> &str {
        "Button"
    }

    fn set_tint_color(&mut self, color: Color) {
        self.title_label.set_tint_color(color);
        self.base_view_mut().set_tint_color(color);
    }

    fn layout_subviews(&mut self) {
        self.title_label.layout_subviews();
        let mut rect = self.bounds().insets_by(self.title_insets);
        let size = self.title_label.intrinsic_size();
        rect.size.width = isize::min(rect.width(), size.width);
        rect.size.height = isize::min(rect.height(), size.height);
        rect.origin.x = (self.frame().width() - rect.width()) / 2;
        rect.origin.y = (self.frame().height() - rect.height()) / 2;
        self.title_label.set_frame(rect);

        self.base_view_mut().layout_subviews();
    }

    fn draw_in_context(&self, ctx: Bitmap) {
        self.base_view.common_draw(&ctx);

        // ctx.draw_rect(self.title_label.frame(), Color::BLACK);
        self.title_label.text.draw(&ctx, self.title_label.frame());

        self.base_view().draw_in_context(ctx);
    }
}
