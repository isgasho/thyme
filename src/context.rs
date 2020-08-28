use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use crate::{Point, Error, Frame, Rect, frame::{RendGroup, RendGroupDef}};
use crate::widget::Widget;
use crate::theme::ThemeSet;
use crate::theme_definition::{ThemeDefinition, AnimState, AnimStateKey};
use crate::render::{Renderer, IO, TextureData, TextureHandle};
use crate::font::FontSource;

/// Structure to register resources and ultimately build the main Thyme [`Context`](struct.Context.html).
///
/// This will hold references to your chosen [`Renderer`](trait.Renderer.html) and [`IO`](trait.IO.html).
/// You pass resources to it to register them with Thyme.  Once this process is complete, call
/// [`build`](struct.ContextBuilder.html#method.build) to create your [`Context`](struct.Context.html).
pub struct ContextBuilder<'a, R: Renderer, I: IO> {
    renderer: &'a mut R,
    io: &'a mut I,
    font_sources: HashMap<String, FontSource>,
    textures: HashMap<String, TextureData>,
    next_texture_handle: TextureHandle,
    theme_def: ThemeDefinition,
}

impl<'a, R: Renderer, I: IO> ContextBuilder<'a, R, I> {
    /// Creates a new `ContextBuilder`, from the specified [`Renderer`](trait.Renderer.html) and [`IO`](trait.IO.html).  The theme for your UI will be deserialized from
    /// `theme`.  For example, `theme` could be a [`serde_json Value`](https://docs.serde.rs/serde_json/value/enum.Value.html) or
    /// [`serde_yaml Value`](https://docs.serde.rs/serde_yaml/enum.Value.html).  See [`the crate root`](index.html) for a discussion of the theme format.
    pub fn new<T: serde::Deserializer<'a>>(theme: T, renderer: &'a mut R, io: &'a mut I) -> Result<ContextBuilder<'a, R, I>, T::Error> {
        let theme_def: ThemeDefinition = serde::Deserialize::deserialize(theme)?;

        Ok(ContextBuilder {
            renderer,
            io,
            font_sources: HashMap::new(),
            textures: HashMap::new(),
            next_texture_handle: TextureHandle::default(),
            theme_def,
        })
    }

    /// Registers the font data for use with Thyme via the specified `id`.  The `data` must consist
    /// of the full binary for a valid TTF or OTF file.
    /// Once the font has been registered, it can be accessed in your theme file via the font `source`.
    pub fn register_font_source<T: Into<String>>(&mut self, id: T, data: Vec<u8>) -> Result<(), Error> {
        let font = match rusttype::Font::try_from_vec(data) {
            Some(font) => font,
            None => return Err(
                Error::FontSource(format!("Unable to parse '{}' as ttf", id.into()))
            )
        };
        self.font_sources.insert(id.into(), FontSource { font });

        Ok(())
    }

    /// Registers the image data for use with Thyme via the specified `id`.  The `data` must consist of
    /// raw binary image data in RGBA format, with 4 bytes per pixel.  The data must start at the
    /// bottom-left hand corner pixel and progress left-to-right and bottom-to-top.  `data.len()` must
    /// equal `dimensions.0 * dimensions.1 * 4`
    /// Once the image has been registered, it can be accessed in your theme file via the image `source`.
    pub fn register_texture<T: Into<String>>(
        &mut self,
        id: T,
        data: &[u8],
        dimensions: (u32, u32),
    ) -> Result<(), Error> {
        let handle = self.next_texture_handle;
        let data = self.renderer.register_texture(handle, data, dimensions)?;
        self.textures.insert(id.into(), data);
        self.next_texture_handle = handle.next();

        Ok(())
    }

    /// Consumes this builder and releases the borrows on the [`Renderer`](trait.Renderer.html) and [`IO`](trait.IO.html), so they can
    /// be used further.  Builds a [`Context`](struct.Context.html).
    pub fn build(self) -> Result<Context, Error> {
        let scale_factor = self.io.scale_factor();
        let display_size = self.io.display_size();
        let textures = self.textures;
        let fonts = self.font_sources;
        let themes = ThemeSet::new(self.theme_def, textures, fonts, self.renderer, scale_factor)?;
        Ok(Context::new(themes, display_size, scale_factor))
    }
}

#[derive(Copy, Clone)]
pub(crate) struct PersistentStateData {
    pub is_open: bool,
    pub resize: Point,
    pub moved: Point,
    pub scroll: Point,
}

/// The internal state stored by Thyme for a given Widget that
/// persists between frames.
///
/// Note that Thyme will generally be able to automatically generate
/// unique IDs for many widgets such as buttons.  But, if you want to
/// access this data for a particular widget you will need to specify
/// a known ID for that widget.
#[derive(Debug)]
pub struct PersistentState {
    /// Whether the widget will be shown.  Defaults to true.
    pub is_open: bool,

    /// An amount, in logical pixels that the widget has been resized by.  Default to zero.
    pub resize: Point,

    /// An amount, in logical pizels that the widget has been moved by.  Defaults to zero.
    pub moved: Point,

    /// An amount, in logical pixels that the internal content has been
    /// scrolled by.  Defaults to zero.
    pub scroll: Point,

    /// The "zero" time for timed images associated with this widget.  Defaults to zero,
    /// which is the internal [`Context`](struct.Context.html) init time.
    pub base_time_millis: u32,

    /// Any characters that have been sent to this widget from the keyboard.  Defaults to
    /// empty.  Widgets should typically drain this list as they work with input.
    pub characters: Vec<char>,

    /// The text for this widget, overriding default text.  Defaults to `None`.
    pub text: Option<String>,
}

impl PersistentState {
    pub(crate) fn copy_data(&self) -> PersistentStateData {
        PersistentStateData {
            is_open: self.is_open,
            resize: self.resize,
            moved: self.moved,
            scroll: self.scroll,
        }
    }
}

impl Default for PersistentState {
    fn default() -> Self {
        PersistentState {
            is_open: true,
            resize: Point::default(),
            moved: Point::default(),
            scroll: Point::default(),
            base_time_millis: 0,
            characters: Vec::default(),
            text: None,
        }
    }
}

pub struct ContextInternal {
    themes: ThemeSet,
    mouse_taken_last_frame: Option<(String, RendGroup)>,
    mouse_in_rend_group_last_frame: Option<RendGroup>,
    top_rend_group: RendGroup,
    check_set_top_rend_group: Option<String>,

    modal: Option<Modal>,

    mouse_pressed_outside: [bool; 3],

    keyboard_focus_widget: Option<String>,
    persistent_state: HashMap<String, PersistentState>,
    empty_persistent_state: PersistentState,

    last_mouse_pos: Point,
    mouse_pos: Point,
    mouse_pressed: [bool; 3],
    mouse_clicked: [bool; 3],
    mouse_wheel: Point,

    display_size: Point,
    scale_factor: f32,

    start_instant: Instant,
    time_millis: u32,
}

impl ContextInternal {
    pub(crate) fn mut_modal<F: FnOnce(&mut Modal)>(&mut self, f: F) {
        if let Some(modal) = self.modal.as_mut() {
            (f)(modal);
        }
    }

    pub(crate) fn modal_id(&self) -> Option<&str> {
        self.modal.as_ref().map(|modal| modal.id.as_ref())
    }

    pub(crate) fn has_modal(&self) -> bool {
        self.modal.is_some()
    }

    pub(crate) fn clear_modal_if_match(&mut self, id: &str) {
        if self.modal_id() == Some(id) {
            self.modal.take();
        }
    }

    pub(crate) fn set_modal(&mut self, id: String) {
        self.modal = Some(Modal::new(id));
    }

    pub(crate) fn mouse_in_rend_group_last_frame(&self) -> Option<RendGroup> {
        self.mouse_in_rend_group_last_frame
    }

    pub(crate) fn set_top_rend_group(&mut self, group: RendGroup) {
        self.top_rend_group = group;
    }

    pub(crate) fn top_rend_group(&self) -> RendGroup { self.top_rend_group }

    pub(crate) fn set_top_rend_group_id(&mut self, id: &str) {
        self.check_set_top_rend_group = Some(id.to_string());
    }

    pub(crate) fn check_set_rend_group_top(&mut self, groups: &[RendGroupDef]) {
        let id = match self.check_set_top_rend_group.take() {
            None => return,
            Some(id) => id,
        };

        for group in groups {
            if group.id() == id {
                self.top_rend_group = group.group();
                break;
            }
        }
    }

    pub(crate) fn base_time_millis_for(&self, id: &str) -> u32 {
        self.persistent_state.get(id).map_or(0, |state| state.base_time_millis)
    }

    pub(crate) fn time_millis(&self) -> u32 { self.time_millis }
    pub(crate) fn mouse_pos(&self) -> Point { self.mouse_pos }
    pub(crate) fn last_mouse_pos(&self) -> Point { self.last_mouse_pos }
    pub(crate) fn mouse_pressed(&self, index: usize) -> bool { self.mouse_pressed[index] }
    pub(crate) fn mouse_clicked(&self, index: usize) -> bool { self.mouse_clicked[index] }

    pub (crate) fn set_focus_keyboard(&mut self, id: String) {
        self.keyboard_focus_widget = Some(id);
    }

    pub (crate) fn is_focus_keyboard(&self, id: &str) -> bool {
        self.keyboard_focus_widget.as_deref() == Some(id)
    }

    pub(crate) fn take_mouse_wheel(&mut self) -> Point {
        let result = self.mouse_wheel;
        self.mouse_wheel = Point::default();
        result
    }

    pub(crate) fn mouse_taken_last_frame_id(&self) -> Option<&str> {
        self.mouse_taken_last_frame.as_ref().map(|(id, _)| id.as_ref())
    }

    pub(crate) fn scale_factor(&self) -> f32 { self.scale_factor }
    pub(crate) fn display_size(&self) -> Point { self.display_size }

    pub(crate) fn themes(&self) -> &ThemeSet { &self.themes }

    pub(crate) fn init_state<T: Into<String>>(&mut self, id: T, open: bool) {
        self.persistent_state.entry(id.into()).or_insert(
            PersistentState {
                is_open: open,
                ..Default::default()
            }
        );
    }

    pub(crate) fn clear_state(&mut self, id: &str) {
        self.persistent_state.remove(id);
    }

    pub(crate) fn state(&self, id: &str) -> &PersistentState {
        match self.persistent_state.get(id) {
            None => &self.empty_persistent_state,
            Some(state) => state,
        }
    }

    pub(crate) fn state_mut<T: Into<String>>(&mut self, id: T) -> &mut PersistentState {
        self.persistent_state.entry(id.into()).or_default()
    }

    pub(crate) fn mouse_pressed_outside(&self) -> bool {
        for pressed in self.mouse_pressed_outside.iter() {
            if *pressed { return true; }
        }
        false
    }

    pub(crate) fn next_frame(&mut self, mouse_taken: Option<(String, RendGroup)>, mouse_in_rend_group: Option<RendGroup>) {
        let mut clear_modal = false;
        if let Some(modal) = self.modal.as_mut() {
            if modal.prevent_close {
                modal.prevent_close = false;
            } else if modal.close_on_click_outside && self.mouse_clicked[0] && !modal.bounds.is_inside(self.mouse_pos) {
                clear_modal = true;
            }
        }

        if clear_modal {
            let modal = self.modal.take().unwrap();
            self.state_mut(modal.id).is_open = false;
        }

        self.mouse_wheel = Point::default();
        self.mouse_clicked = [false; 3];
        self.mouse_taken_last_frame = mouse_taken;
        self.last_mouse_pos = self.mouse_pos;
        self.mouse_in_rend_group_last_frame = mouse_in_rend_group;
    }
}

/**
The main Thyme Context that holds internal [`PersistentState`](struct.PersistentState.html)
and is responsible for creating [`Frames`](struct.Frame.html).

This is created by [`build`](struct.ContextBuilder.html#method.build) on
[`ContextBuilder`](struct.ContextBuilder.html) after resource registration is complete.
**/
pub struct Context {
    internal: Rc<RefCell<ContextInternal>>,
}

impl Context {
    fn new(themes: ThemeSet, display_size: Point, scale_factor: f32) -> Context {
        let internal = ContextInternal {
            display_size,
            scale_factor,
            themes,
            persistent_state: HashMap::new(),
            empty_persistent_state: PersistentState::default(),
            mouse_pos: Point::default(),
            last_mouse_pos: Point::default(),
            mouse_pressed: [false; 3],
            mouse_clicked: [false; 3],
            mouse_wheel: Point::default(),
            mouse_taken_last_frame: None,
            mouse_in_rend_group_last_frame: None,
            top_rend_group: RendGroup::default(),
            check_set_top_rend_group: None,
            mouse_pressed_outside: [false; 3],
            modal: None,
            time_millis: 0,
            start_instant: Instant::now(),
            keyboard_focus_widget: None,
        };

        println!("{}", std::mem::size_of::<crate::WidgetState>());

        Context {
            internal: Rc::new(RefCell::new(internal))
        }
    }

    /// Returns true if thyme wants to use the mouse in the current frame, generally
    /// because the mouse is over a Thyme widget.  If this returns true, you probably
    /// want Thyme to handle input this frame, while if it returns false, your application
    /// or game logic should handle input.
    pub fn wants_mouse(&self) -> bool {
        let internal = self.internal.borrow();
        internal.mouse_taken_last_frame.is_some()
    }

    pub(crate) fn internal(&self) -> &Rc<RefCell<ContextInternal>> {
        &self.internal
    }

    pub(crate) fn set_scale_factor(&mut self, scale: f32) {
        let mut internal = self.internal.borrow_mut();
        internal.scale_factor = scale;
    }

    pub(crate) fn set_display_size(&mut self, size: Point) {
        let mut internal = self.internal.borrow_mut();
        internal.display_size = size;
    }

    pub(crate) fn add_mouse_wheel(&mut self, delta: Point) {
        let mut internal = self.internal.borrow_mut();

        internal.mouse_wheel = internal.mouse_wheel + delta;
    }

    pub(crate) fn set_mouse_pressed(&mut self, pressed: bool, index: usize) {
        let mut internal = self.internal.borrow_mut();

        if index >= internal.mouse_pressed.len() {
            return;
        }

        // don't take a mouse press that started outside the GUI elements
        if pressed && internal.mouse_taken_last_frame.is_none() {
            internal.mouse_pressed_outside[index] = true;
        }

        if !pressed && internal.mouse_pressed_outside[index] {
            internal.mouse_pressed_outside[index] = false;
        }

        if internal.mouse_pressed[index] && !pressed {
            internal.mouse_clicked[index] = true;
            internal.keyboard_focus_widget = None;
        }

        internal.mouse_pressed[index] = pressed;
    }

    pub(crate) fn push_character(&mut self, c: char) {
        let mut internal = self.internal.borrow_mut();

        let id = match &internal.keyboard_focus_widget {
            Some(id) => id.to_string(),
            None => return,
        };

        let state = internal.state_mut(id);
        state.characters.push(c);
    }

    pub(crate) fn set_mouse_pos(&mut self, pos: Point) {
        let mut internal = self.internal.borrow_mut();
        internal.mouse_pos = pos;
    }

    /// Creates a [`Frame`](struct.Frame.html), the main object that should pass through
    /// your UI building functions and is responsible for constructing the widget tree.
    /// This method should be called each frame you want to draw / interact with the UI.
    pub fn create_frame(&mut self) -> Frame {
        let now = Instant::now();

        let anim_state;
        let display_size = {
            let mut context = self.internal.borrow_mut();

            let elapsed = (now - context.start_instant).as_millis() as u32;
            context.time_millis = elapsed;

            if context.mouse_pressed[0] {
                anim_state = AnimState::new(AnimStateKey::Pressed);
            } else {
                anim_state = AnimState::normal();
            }

            context.display_size() / context.scale_factor()
        };

        let context = Context { internal: Rc::clone(&self.internal) };

        let root = Widget::root(display_size);
        Frame::new(context, root, anim_state)
    }
}

pub(crate) struct Modal {
    pub(crate) id: String,
    pub(crate) close_on_click_outside: bool,
    pub(crate) bounds: Rect,
    pub(crate) prevent_close: bool,
}

impl Modal {
    fn new(id: String) -> Modal {
        Modal {
            id,
            close_on_click_outside: false,
            bounds: Rect::default(),
            prevent_close: true,
        }
    }
}