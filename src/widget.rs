use crate::{
    AnimState, AnimStateKey, Color, Frame, Point, Border, Align, 
    Layout, WidthRelative, HeightRelative, Rect,
};
use crate::{frame::{RendGroup}, font::FontSummary, image::ImageHandle};
use crate::theme::{WidgetTheme};
use crate::window::WindowBuilder;
use crate::scrollpane::ScrollpaneBuilder;

pub struct Widget {
    // identifier for persistent state
    id: String,
    rend_group: RendGroup,

    // TODO potentially move these out and store current parent data
    // in the frame for a small perf boost
    // stored in the widget for parent ref purposes
    scroll: Point,
    cursor: Point,
    theme_id: String,
    child_align: Align,
    layout: Layout,
    layout_spacing: Point,

    // stored in the widget for drawing purposes
    clip: Rect,
    text: Option<String>,
    text_color: Color,
    text_align: Align,
    font: Option<FontSummary>,
    background: Option<ImageHandle>,
    foreground: Option<ImageHandle>,
    pos: Point,
    size: Point,
    border: Border,
    anim_state: AnimState,
    visible: bool,
}

impl Widget {
    pub(crate) fn root(size: Point) -> Widget {
        Widget {
            theme_id: String::new(),
            text: None,
            text_align: Align::default(),
            text_color: Color::default(),
            font: None,
            background: None,
            foreground: None,
            layout: Layout::default(),
            layout_spacing: Point::default(),
            child_align: Align::default(),
            pos: Point::default(),
            scroll: Point::default(),
            cursor: Point::default(),
            border: Border::default(),
            size,
            id: String::new(),
            rend_group: RendGroup::default(),
            anim_state: AnimState::normal(),
            visible: true,
            clip: Rect { pos: Point::default(), size },
        }
    }

    fn create(parent: &Widget, theme: &WidgetTheme, id: String) -> (WidgetData, Widget) {
        let font = theme.font;
        let border = theme.border.unwrap_or_default();
        let raw_size = theme.size.unwrap_or_default();
        let width_from = theme.width_from.unwrap_or_default();
        let height_from = theme.height_from.unwrap_or_default();
        let size = size(parent, raw_size, border, font, width_from, height_from);

        let align = theme.align.unwrap_or(parent.child_align);
        let manual_pos = theme.pos.is_some() || align != parent.child_align;
        let cursor_pos = if align == parent.child_align {
            parent.cursor + parent.scroll
        } else {
            parent.scroll
        };
        let raw_pos = theme.pos.unwrap_or(cursor_pos) + parent.scroll;
        let pos = pos(parent, raw_pos, size, align);

        let data = WidgetData {
            manual_pos,
            wants_mouse: theme.wants_mouse.unwrap_or_default(),
            wants_scroll: theme.wants_scroll.unwrap_or_default(),
            raw_size,
            raw_pos,
            width_from,
            height_from,
            align,
            enabled: true,
            active: false,
            recalc_pos_size: true,
            next_render_group: false,
        };

        let widget = Widget {
            layout: theme.layout.unwrap_or_default(),
            layout_spacing: theme.layout_spacing.unwrap_or_default(),
            child_align: theme.child_align.unwrap_or_default(),
            theme_id: theme.full_id.to_string(),
            text: theme.text.clone(),
            text_color: theme.text_color.unwrap_or_default(),
            text_align: theme.text_align.unwrap_or_default(),
            font,
            background: theme.background,
            foreground: theme.foreground,
            pos,
            scroll: Point::default(),
            cursor: Point::default(),
            border,
            size,
            id,
            rend_group: RendGroup::default(),
            anim_state: AnimState::normal(),
            visible: true,
            clip: parent.clip,
        };

        (data, widget)
    }

    pub fn clip(&self) -> Rect { self.clip }
    pub fn visible(&self) -> bool { self.visible }
    pub fn text_color(&self) -> Color { self.text_color }
    pub fn text_align(&self) -> Align { self.text_align }
    pub fn text(&self) -> Option<&str> { self.text.as_deref() }
    pub fn font(&self) -> Option<FontSummary> { self.font }
    pub fn foreground(&self) -> Option<ImageHandle> { self.foreground }
    pub fn background(&self) -> Option<ImageHandle> { self.background }
    pub fn border(&self) -> Border { self.border }
    pub fn id(&self) -> &str { &self.id }
    pub fn theme_id(&self) -> &str { &self.theme_id }
    pub fn anim_state(&self) -> AnimState { self.anim_state }
    pub fn size(&self) -> Point { self.size }
    pub fn pos(&self) -> Point { self.pos }

    pub fn inner_size(&self) -> Point {
        Point { x: self.size.x - self.border.horizontal(), y: self.size.y - self.border.vertical() }
    }

    pub fn set_cursor(&mut self, x: f32, y: f32) {
        self.cursor = Point { x, y };
    }

    pub fn cursor(&self) -> Point {
        self.cursor
    }

    pub fn gap(&mut self, gap: f32) {
        match self.layout {
            Layout::Horizontal => self.cursor.x += gap,
            Layout::Vertical => self.cursor.y += gap,
            Layout::Free => (),
        }
    }

    pub(crate) fn rend_group(&self) -> RendGroup { self.rend_group }

    pub(crate) fn set_rend_group(&mut self, group: RendGroup) {
        self.rend_group = group;
    }
}

/// The current state of a widget on this frame, this is returned when you finish
/// most widgets, such as with a call to [`WidgetBuilder.finish`](struct.WidgetBuilder.html#method.finish).
pub struct WidgetState {
    /// Whether this widget was drawn.  In general, if a widget is not visible, any children
    /// were not created and closures, such as passed to [`WidgetBuilder.children`](struct.WidgetBuilder.html#method.children)
    /// were not executed.
    pub visible: bool,

    /// Whether the mouse is hovering over this widget on the current frame
    pub hovered: bool,

    /// Whether the mouse is pressed on this widget on the current frame
    pub pressed: bool,

    /// Whether the mouse clicked on this widget on the current frame.  This field will only be `true` once
    /// per click.
    pub clicked: bool,

    /// How far the mouse has been dragged or scrolled on this widget, in logical pixels.
    pub moved: Point,
}

impl WidgetState {
    fn hidden() -> WidgetState {
        WidgetState {
            visible: false,
            hovered: false,
            pressed: false,
            clicked: false,
            moved: Point::default(),
        }
    }

    fn new(anim_state: AnimState, clicked: bool, moved: Point) -> WidgetState {
        let (hovered, pressed) = if anim_state.contains(AnimStateKey::Pressed) {
            (true, true)
        } else if anim_state.contains(AnimStateKey::Hover) {
            (true, false)
        } else {
            (false, false)
        };

        WidgetState {
            visible: true,
            hovered,
            pressed,
            clicked,
            moved,
        }
    }
}

fn size(
    parent: &Widget,
    size: Point,
    border: Border,
    font: Option<FontSummary>,
    width_from: WidthRelative,
    height_from: HeightRelative,
) -> Point {
    let x = match width_from {
        WidthRelative::Normal => size.x,
        WidthRelative::Parent => size.x + parent.size.x - parent.border.horizontal(),
    };
    let y = match height_from {
        HeightRelative::Normal => size.y,
        HeightRelative::Parent => size.y + parent.size.y - parent.border.vertical(),
        HeightRelative::FontLine => size.y + font.map_or(0.0,
            |sum| sum.line_height) + border.vertical(),
    };
    Point { x, y }
}

fn pos(parent: &Widget, pos: Point, self_size: Point, align: Align) -> Point {
    let size = parent.size;
    let border = parent.border;

    let pos = parent.pos + match align {
        Align::Left => Point {
            x: border.left + pos.x,
            y: border.top + (size.y - border.vertical()) / 2.0 + pos.y
        },
        Align::Right => Point {
            x: size.x - border.right - pos.x,
            y: border.top + (size.y - border.vertical()) / 2.0 + pos.y
        },
        Align::Bot => Point {
            x: border.left + (size.x - border.horizontal()) / 2.0 + pos.x,
            y: size.y - border.bot - pos.y
        },
        Align::Top => Point {
            x: border.left + (size.x - border.horizontal()) / 2.0 + pos.x,
            y: border.top + pos.y
        },
        Align::Center => Point {
            x: border.left + (size.x - border.horizontal()) / 2.0 + pos.x,
            y: border.top + (size.y - border.vertical()) / 2.0 + pos.y
        },
        Align::BotLeft => Point {
            x: border.left + pos.x,
            y: size.y - border.bot - pos.y
        },
        Align::BotRight => Point {
            x: size.x - border.right - pos.x,
            y: size.y - border.bot - pos.y
        },
        Align::TopLeft => Point {
            x: border.left + pos.x,
            y: border.top + pos.y
        },
        Align::TopRight => Point {
            x: size.x - border.right - pos.x,
            y: border.top + pos.y
        },
    };

    pos - align.adjust_for(self_size).round()
}

pub(crate) struct WidgetData {
    manual_pos: bool,
    wants_mouse: bool,
    wants_scroll: bool,

    raw_pos: Point,
    raw_size: Point,
    width_from: WidthRelative,
    height_from: HeightRelative,
    align: Align,

    enabled: bool,
    active: bool,
    recalc_pos_size: bool,
    next_render_group: bool,
}

/// A `WidgetBuilder` is used to customize widgets within your UI tree, following a builder pattern.
///
///Although there are several convenience methods on
/// [`Frame`](struct.Frame.html) for simple [`buttons`](struct.Frame.html#method.button), [`labels`](struct.Frame.html#method.label),
/// etc, widgets with more complex behavior will usually be created via [`Frame.start`](struct.Frame.html#method.start) and then
/// customized using the methods here.  Note also that many methods here have an equivalent in the widget's [`theme`](struct.Context.html)
/// definition.
///
/// Each method here takes the WidgetBuilder by value, modifies it, and then returns it, allowing you to use a builder pattern.
/// The [`window`](#method.window) method will transform this into a [`WindowBuilder`](struct.WindowBuilder.html), while the
/// [`finish`](#method.finish) and [`children`](#method.children) methods will complete the widget and add it to the frame's widget tree.
pub struct WidgetBuilder<'a> {
    pub frame: &'a mut Frame,
    pub parent: usize,
    pub widget: Widget,
    data: WidgetData,    
}

impl<'a> WidgetBuilder<'a> {
    #[must_use]
    pub(crate) fn new(frame: &'a mut Frame, parent: usize, theme_id: String, base_theme: &str) -> WidgetBuilder<'a> {
        let (data, widget) = {
            let context = std::rc::Rc::clone(&frame.context_internal());
            let context = context.borrow();
            let theme = match context.themes().theme(&theme_id) {
                None => {
                    match context.themes().theme(base_theme) {
                        None => {
                            // TODO remove unwrap
                            println!("Unable to locate theme either at {} or {}", theme_id, base_theme);
                            panic!();
                        }, Some(theme) => theme,
                    }
                }, Some(theme) => theme,
            };

            let id = {
                let parent_widget = frame.widget(parent);
                if parent_widget.id.is_empty() {
                    theme.id.to_string()
                } else {
                    format!("{}/{}", parent_widget.id, theme.id)
                }
            };

            let id = frame.generate_id(id);
            let parent_widget = frame.widget(parent);

            let (data, widget) = Widget::create(parent_widget, theme, id);

            (data, widget)
        };

        WidgetBuilder {
            frame,
            parent,
            widget,
            data,
        }
    }

    fn recalculate_pos_size(&mut self, state_moved: Point, state_resize: Point) {
        {
            let parent = self.frame.widget(self.parent);
            let widget = &self.widget;
            let size = size (
                parent,
                self.data.raw_size,
                widget.border,
                widget.font,
                self.data.width_from,
                self.data.height_from
            );

            self.widget.size = size;
        }

        {
            let parent = self.frame.widget(self.parent);
            let widget = &self.widget;
            let pos = pos(parent, self.data.raw_pos, widget.size, self.data.align);
            self.widget.pos = pos + state_moved;
        }

        self.widget.size = self.widget.size + state_resize;

        self.data.recalc_pos_size = false;
    }

    fn parent(&self) -> &Widget {
        self.frame.widget(self.parent)
    }
    
    /// Specifies that this widget and its children should be part of a new Render Group.  Render groups are used to handle cases where
    /// widgets may overlap, and determine input routing and draw order in those cases.  If your UI doesn't have moveable elements such as
    /// windows, you should generally be ok to draw your entire UI in one render group, with the exception of modal popups.
    /// [`Windows`](struct.WindowBuilder.html) make use of render groups.
    #[must_use]
    pub fn new_render_group(mut self) -> WidgetBuilder<'a> {
        self.data.next_render_group = true;
        self
    }

    /// Sets whether this widget will interact with the mouse.  By default, widgets will not interact with the mouse, so this is set to `true`
    /// for buttons and similar.
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn wants_mouse(mut self, wants_mouse: bool) -> WidgetBuilder<'a> {
        self.data.wants_mouse = wants_mouse;
        self
    }

    /// Sets whether this widget will receive mouse scrollwheel events.  By default, widgets will not receive scroll wheel events, so this is set
    /// to `true` for scrollpanes.
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn wants_scroll(mut self, wants_scroll: bool) -> WidgetBuilder<'a> {
        self.data.wants_scroll = wants_scroll;
        self
    }

    /// Sets an `id` for this widget.  This `id` is used internally to associate the widget with its [`PersistentState`](struct.PersistentState.html).
    /// You will need to specify an `id` if you want to make changes to the [`PersistentState`](struct.PersistentState.html).  Otherwise,
    /// Thyme can usually generate a unique internal ID for most elements.
    #[must_use]
    pub fn id<T: Into<String>>(mut self, id: T) -> WidgetBuilder<'a> {
        self.widget.id = id.into();
        self.data.recalc_pos_size = true;
        self
    }

    /// Specify whether this widget is initially `open`, or [`visible`](#method.visible).  By default,
    /// widgets are initially open.  If set to false, the widget will not be shown until it is set to open
    /// using one of the methods on [`Frame`](struct.Frame.html) to modify its [`PersistentState`](struct.PersistentState.html).
    #[must_use]
    pub fn initially_open(self, open: bool) -> WidgetBuilder<'a> {
        {
            let mut context = self.frame.context_internal().borrow_mut();
            context.init_state(&self.widget.id, open);
        }
        self
    }

    /// Specify a [`Color`](struct.Color.html) for the text of this widget to display.  The default
    /// color is white.
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn text_color(mut self, color: Color) -> WidgetBuilder<'a> {
        self.widget.text_color = color;
        self
    }

    /// Specify the [`alignment`](enum.Align.html) of the widget's text within the widget's
    /// inner area, as defined by its overall [`size`](#method.size) and [`border`](#method.border).
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn text_align(mut self, align: Align) -> WidgetBuilder<'a> {
        self.widget.text_align = align;
        self
    }

    /// Specify `text` to display for this widget.  The widget must have a [`font`](#method.font)
    /// specified to render text.
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn text<T: Into<String>>(mut self, text: T) -> WidgetBuilder<'a> {
        self.widget.text = Some(text.into());
        self
    }

    /// Specify a `font` for any text rendered by this widget.  A widget must have a font
    /// specified to render text.  The `font` must be registered in the theme's font definitions.
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn font(mut self, font: &str) -> WidgetBuilder<'a> {
        let font = {
            let context = self.frame.context_internal();
            let context = context.borrow();
            context.themes().find_font(Some(font))
        };

        self.widget.font = font;
        self.data.recalc_pos_size = true;
        self
    }

    /// Specify a foreground image for this widget.  The image ID, `fg` must be registered in the theme's
    /// image definitions.  The ID consists of "{image_set_id}/{image_id}".
    /// Foreground images are drawn below text but above the background.
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn foreground(mut self, fg: &str) -> WidgetBuilder<'a> {
        let fg = {
            let context = self.frame.context_internal();
            let context = context.borrow();
            context.themes().find_image(Some(fg))
        };

        self.widget.foreground = fg;
        self
    }

    /// Specify a background image for this widget.  The image ID, `bg` must be registered in the theme's
    /// image definitions.  The ID consists of "{image_set_id}/{image_id}".
    /// Background images are drawn below text and any children.
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn background(mut self, bg: &str) -> WidgetBuilder<'a> {
        let bg = {
            let context = self.frame.context_internal();
            let context = context.borrow();
            context.themes().find_image(Some(bg))
        };

        self.widget.background = bg;
        self
    }

    /// Specifies the default alignment of children added to this widget.  See [`Align`](enum.Align.html).
    /// This may be overridden by the child, either in the theme or by calling [`align`](#method.align).
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn child_align(mut self, align: Align) -> WidgetBuilder<'a> {
        self.widget.child_align = align;
        self
    }

    /// Specifies the spacing, in logical pixels, to use between children that are laid out in this widget.
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn layout_spacing(mut self, spacing: Point) -> WidgetBuilder<'a> {
        self.widget.layout_spacing = spacing;
        self
    }

    /// Specifies that the children of this widget should be laid out vertically.  See [`Layout`](enum.Layout.html).
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn layout_horizontal(self) -> WidgetBuilder<'a> {
        self.layout(Layout::Horizontal)
    }

    /// Specifies that the children of this widget should be laid out vertically.  See [`Layout`](enum.Layout.html).
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn layout_vertical(self) -> WidgetBuilder<'a> {
        self.layout(Layout::Vertical)
    }

    /// Specifies the `layout` for children of this widget.  See [`Layout`](enum.Layout.html).
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn layout(mut self, layout: Layout) -> WidgetBuilder<'a> {
        self.widget.layout = layout;
        self
    }

    /// Manually specify a position for this widget, basedon the specified
    /// `x` and `y` logical pixel positions.  This position ignores alignment
    /// or any other considerations.
    #[must_use]
    pub fn screen_pos(mut self, x: f32, y: f32) -> WidgetBuilder<'a> {
        self.data.raw_pos = Point { x, y };
        self.widget.pos = Point { x, y };
        self.data.align = Align::TopLeft;
        self.data.manual_pos = true;
        self.data.recalc_pos_size = false;
        self
    }

    /// Specify the position of the widget, with respect to its alignment within the parent.
    /// The `x` and `` values are in logical pixels.
    /// See [`align`](#method.align).
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn pos(mut self, x: f32, y: f32) -> WidgetBuilder<'a> {
        self.data.raw_pos = Point { x, y } + self.parent().scroll;
        self.data.manual_pos = true;
        self.data.recalc_pos_size = true;
        self
    }

    /// Specify the alignment of this widget with respect to its parent.  See [`Align`](enum.Align.html).
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn align(mut self, align: Align) -> WidgetBuilder<'a> {
        self.data.align = align;
        self.data.manual_pos = true;
        self.data.recalc_pos_size = true;
        self
    }
    
    /// Specify the widget's border size, which determines the inner size of the widget
    /// relative to its [`size`](#method.size).  See [`Border`](struct.Border.html).
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn border(mut self, border: Border) -> WidgetBuilder<'a> {
        self.widget.border = border;
        self.data.recalc_pos_size = true;
        self
    }

    /// Specify the widget's `size` in logical pixels.  This may or may not be an
    /// absolute size, depending on [`WidthRelative`](enum.WidthRelative.html) and
    /// [`HeightRelative`](enum.HeightRelative.html)
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn size(mut self, x: f32, y: f32) -> WidgetBuilder<'a> {
        self.data.raw_size = Point { x, y };
        self.data.recalc_pos_size = true;
        self
    }

    /// Specify how to compute the widget's width from its [`size`](#method.size).
    /// See [`WidthRelative`](enum.WidthRelative.html).
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn width_from(mut self, from: WidthRelative) -> WidgetBuilder<'a> {
        self.data.width_from = from;
        self.data.recalc_pos_size = true;
        self
    }

    /// Specify how to compute the widget's height from its [`size`](#method.size).
    /// See [`HeightRelative`](enum.HeightRelative.html)
    /// This may also be specified in the widget's [`theme`](struct.Context.html).
    #[must_use]
    pub fn height_from(mut self, from: HeightRelative) -> WidgetBuilder<'a> {
        self.data.height_from = from;
        self.data.recalc_pos_size = true;
        self
    }

    /// Sets the widget's clip [`Rectangle`](struct.Rect.html).  By default,
    /// a widget will have a clip rectangle set from its `size` and `position`,
    /// calculated based on the theme and the various methods such as [`size`](#method.size),
    /// [`pos`](#method.pos), [`width_from`](#method.width_from), [`height_from`](#method.height_from),
    /// etc.  You can override that behavior with this method.  This is useful to display part of an image,
    /// such as in a [`progress bar`](struct.Frame.html#method.progress_bar), or to limit the size of child
    /// content, such as in a [`scrollpane`](#method.scrollpane).
    /// Widgets always inherit their `clip` as the minimum extent of their parent's clip and their own clip.
    /// See [`Rect.min`](struct.Rect.html#method.min).
    #[must_use]
    pub fn clip(mut self, clip: Rect) -> WidgetBuilder<'a> {
        let cur_clip = self.widget.clip;
        self.widget.clip = cur_clip.min(clip);
        self
    }

    /// Sets whether the widget's [`AnimState`](struct.AnimState.html) will
    /// include the `active` [`AnimStateKey`](enum.AnimStateKey.html).
    #[must_use]
    pub fn active(mut self, active: bool) -> WidgetBuilder<'a> {
        self.data.active = active;
        self
    }

    /// Sets whether this widget will be `visible`.  If the widget is not
    /// visible, it will not be shown and any child closures (such as passed in
    /// [`children`](#method.children)) will not be run.
    #[must_use]
    pub fn visible(mut self, visible: bool) -> WidgetBuilder<'a> {
        self.widget.visible = visible;
        self
    }

    /// Sets whether this widget will be `enabled`.  If the widget is not
    /// enabled, it will not interact with any user input.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> WidgetBuilder<'a> {
        self.data.enabled = enabled;
        self
    }

    
    /// Force the widget to layout its `size` and `position` immediately.
    /// Assuming these attributes are not changed after this method is
    /// called, these attributes will have their final values after this
    /// method returns.  The size and position are written to the passed
    /// in [`Rect`](struct.Rect.html).
    #[must_use]
    pub fn trigger_layout(mut self, rect: &mut Rect) -> WidgetBuilder<'a> {
        let (state_moved, state_resize) = {
            let internal = self.frame.context_internal().borrow();
            let state = internal.state(&self.widget.id);
            (state.moved, state.resize)
        };
        if self.data.recalc_pos_size {
            self.recalculate_pos_size(state_moved, state_resize);
        }

        rect.pos = self.widget.pos;
        rect.size = self.widget.size;
        self
    }

    /// Force the widget to layout its `size` and `position` immediately.
    /// Assuming these attributes are not changed after this is method is
    /// called, they will have their final values after this method returns.
    /// The inner size and position (size and position adjusted by the
    /// [`Border`](struct.Border.html) are written to the passed in
    /// [`Rect`](struct.Rect.html)
    #[must_use]
    pub fn trigger_layout_inner(mut self, rect: &mut Rect) -> WidgetBuilder<'a> {
        let (state_moved, state_resize) = {
            let internal = self.frame.context_internal().borrow();
            let state = internal.state(&self.widget.id);
            (state.moved, state.resize)
        };
        if self.data.recalc_pos_size {
            self.recalculate_pos_size(state_moved, state_resize);
        }

        rect.pos = self.widget.pos + self.widget.border.tl();
        rect.size = Point::new(
            self.widget.size.x - self.widget.border.horizontal(),
            self.widget.size.y - self.widget.border.vertical(),
        );
        self
    }

    /// Causes this widget to layout its current text.  The final position of the text
    /// cursor is written into `pos`.  If this widget does not have a font or has no text,
    /// nothing is written into `pos`.
    #[must_use]
    pub fn trigger_text_layout(mut self, cursor: &mut Point) -> WidgetBuilder<'a> {
        // recalculate pos size and calculate text, if needed
        let (text, state_moved, state_resize) = {
            let internal = self.frame.context_internal().borrow();
            let state = internal.state(&self.widget.id);
            (
                state.text.as_ref().map(|t| t.to_string()),
                state.moved,
                state.resize,
            )
        };

        if self.data.recalc_pos_size {
            self.recalculate_pos_size(state_moved, state_resize);
        }

        if let Some(text) = text {
            self.widget.text = Some(text);
        }

        let text = match &self.widget.text {
            None => return self,
            Some(text) => text,
        };

        let font_def = match self.widget.font {
            None => return self,
            Some(def) => def,
        };

        {
            let widget = &self.widget;
            let fg_pos = Point::default();
            let fg_size = widget.inner_size();
            let align = widget.text_align();

            let internal = self.frame.context_internal().borrow();
            let scale = internal.scale_factor();
            let font = internal.themes().font(font_def.handle);

            let mut scaled_cursor = *cursor * scale;

            font.layout(fg_size * scale, fg_pos * scale, text, align, &mut scaled_cursor);

            *cursor = scaled_cursor / scale;
        }

        self
    }

    /// Turns this builder into a WindowBuilder.  You should use all `WidgetBuilder` methods
    /// before calling this method.  The window must still be completed with one of the
    /// [`WindowBuilder`](struct.WindowBuilder.html) methods.  You must pass a unique `id` for each window
    /// created by your application.
    #[must_use]
    pub fn window(self, id: &str) -> WindowBuilder<'a> {
        WindowBuilder::new(self.id(id).new_render_group())
    }

    /// Turns this builder into a [`ScrollpaneBuilder`](struct.ScrollpaneBuilder.html).  You should use all
    /// `WidgetBuilder` methods before calling this method.  The scrollpane must still be completed
    /// with one of the methods on [`ScrollpaneBuilder`](struct.ScrollpaneBuilder.html).  You must pass a unique
    /// `content_id` for the scrollpane's content.
    #[must_use]
    pub fn scrollpane(self, content_id: &str) -> ScrollpaneBuilder<'a> {
        ScrollpaneBuilder::new(self.wants_scroll(true), content_id)
    }

    /// Consumes the builder and adds a widget to the current frame.  The
    /// returned data includes information about the animation state and
    /// mouse interactions of the created element.
    /// If you wish this widget to have one or more child widgets, you should
    /// call [`children`](#method.children) instead.
    pub fn finish(self) -> WidgetState {
        self.finish_with(None::<fn(&mut Frame)>).1
    }

    /// Consumes the builder and adds a widget to the current frame.  The
    /// returned data includes information about the animation state and
    /// mouse interactions of the created element.
    /// The provided closure is called to enable adding children to this widget.
    /// If you don't want to add children, you can just call
    /// [`finish`](#method.finish) instead.
    pub fn children<F: FnOnce(&mut Frame)>(self, f: F) -> WidgetState {
        self.finish_with(Some(f)).1
    }

    pub(crate) fn finish_with<F: FnOnce(&mut Frame)>(mut self, f: Option<F>) -> (&'a mut Frame, WidgetState) {
        if !self.widget.visible { return (self.frame, WidgetState::hidden()); }

        let (state, text, in_modal_tree) = {
            let internal = self.frame.context_internal().borrow();
            let state = internal.state(&self.widget.id);

            let text = match &state.text {
                None => None,
                Some(text) => Some(text.to_string())
            };

            let in_modal_tree = Some(self.widget.id()) == internal.modal_id();

            (state.copy_data(), text, in_modal_tree)
        };

        if let Some(text) = text {
            self.widget.text = Some(text);
        }

        self.widget.scroll = state.scroll;
        self.widget.cursor = self.widget.cursor;

        if !state.is_open {
            self.widget.visible = false;
            return (self.frame, WidgetState::hidden());
        }

        if self.data.recalc_pos_size {
            self.recalculate_pos_size(state.moved, state.resize);
        }

        let self_pos = self.widget.pos;
        let self_size = self.widget.size;
        let self_bounds = Rect::new(self_pos, self_size);
        let old_max_child_bounds = self.frame.max_child_bounds();

        // set modal tree value only if a match is found
        if in_modal_tree {
            {
                let mut internal = self.frame.context_internal().borrow_mut();
                internal.mut_modal(|modal| {
                    modal.bounds = self_bounds;
                });
            }
            self.frame.in_modal_tree = true;
        }

        let prev_rend_group = self.frame.cur_render_group();

        if self.data.next_render_group {
            self.frame.next_render_group(self_bounds, self.widget.id.to_string());
        }

        let widget_index = self.frame.num_widgets();
        self.frame.push_widget(self.widget);

        // if there is a child function
        if let Some(f) = f {
            // push the max_child pos and parent index
            self.frame.set_max_child_bounds(self_bounds);
            let old_parent_index = self.frame.parent_index();
            self.frame.set_parent_index(widget_index);

            // build all children
            (f)(self.frame);

            self.frame.set_parent_index(old_parent_index);
            let this_children_max_bounds = self.frame.max_child_bounds();
            self.frame.set_parent_max_child_bounds(this_children_max_bounds);
        }

        self.frame.set_max_child_bounds(old_max_child_bounds.max(self_bounds));

        let (clicked, mut anim_state, mut dragged) = if self.data.enabled && self.data.wants_mouse {
            let mouse_state = self.frame.check_mouse_state(widget_index);
            (mouse_state.clicked, mouse_state.anim, mouse_state.dragged)
        } else {
            (false, AnimState::disabled(), Point::default())
        };

        if self.data.wants_scroll {
            if let Some(wheel) = self.frame.check_mouse_wheel(widget_index) {
                dragged.x += wheel.x;
                dragged.y += wheel.y;
            }
        }

        if self.data.next_render_group {
            self.frame.prev_render_group(prev_rend_group);
        }

        // unset modal tree value only if this widget was the modal one
        if in_modal_tree {
            self.frame.in_modal_tree = false;
        }

        if self.data.active {
            anim_state.add(AnimStateKey::Active);
        }

        self.frame.widget_mut(widget_index).anim_state = anim_state;

        let state = WidgetState::new(anim_state, clicked, dragged);
        let size = self.frame.widget(widget_index).size;
        if !self.data.manual_pos {
            use Align::*;
            let (x, y) = match self.frame.widget(self.parent).child_align {
                Left => (size.x, 0.0),
                Right => (-size.x, 0.0),
                Bot => (0.0, -size.y),
                Top => (0.0, size.y),
                Center => (0.0, 0.0),
                BotLeft => (size.x, -size.y),
                BotRight => (-size.x, -size.y),
                TopLeft => (size.x, size.y),
                TopRight => (-size.x, size.y),
            };

            let parent = self.frame.widget_mut(self.parent);
            use Layout::*;
            match parent.layout {
                Horizontal => parent.cursor.x += x + parent.layout_spacing.x,
                Vertical => parent.cursor.y += y + parent.layout_spacing.y,
                Free => (),
            }
        }
        
        (self.frame, state)
    }
}