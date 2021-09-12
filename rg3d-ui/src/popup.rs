use crate::{
    border::BorderBuilder,
    core::{algebra::Vector2, pool::Handle},
    message::{
        ButtonState, MessageData, MessageDirection, OsEvent, PopupMessage, UiMessage,
        UiMessageData, WidgetMessage,
    },
    node::UINode,
    widget::{Widget, WidgetBuilder},
    BuildContext, Control, NodeHandleMapping, RestrictionEntry, Thickness, UserInterface,
    BRUSH_DARKER, BRUSH_LIGHTER,
};
use std::ops::{Deref, DerefMut};

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Placement<M: MessageData, C: Control<M, C>> {
    /// A popup should be placed relative to given widget at the left top corner of the widget screen bounds.
    /// Widget handle could be `NONE`, in this case the popup will be placed at the left top corner of the screen.
    LeftTop(Handle<UINode<M, C>>),

    /// A popup should be placed relative to given widget at the right top corner of the widget screen bounds.
    /// Widget handle could be `NONE`, in this case the popup will be placed at the right top corner of the screen.
    RightTop(Handle<UINode<M, C>>),

    /// A popup should be placed relative to given widget at the center of the widget screen bounds.
    /// Widget handle could be `NONE`, in this case the popup will be placed at the center of the screen.
    Center(Handle<UINode<M, C>>),

    /// A popup should be placed relative to given widget at the left bottom corner of the widget screen bounds.
    /// Widget handle could be `NONE`, in this case the popup will be placed at the left bottom corner of the screen.
    LeftBottom(Handle<UINode<M, C>>),

    /// A popup should be placed relative to given widget at the right bottom corner of the widget screen bounds.
    /// Widget handle could be `NONE`, in this case the popup will be placed at the right bottom corner of the screen.
    RightBottom(Handle<UINode<M, C>>),

    /// A popup should be placed at the cursor position. The widget handle could be either `NONE` or a handle of a
    /// widget that is directly behind the cursor.
    Cursor(Handle<UINode<M, C>>),

    /// A popup should be placed at given screen-space position.
    Position {
        /// Screen-space position.
        position: Vector2<f32>,

        /// A handle of the node that is located behind the given position. Could be `NONE` if there is nothing behind
        /// given position.
        target: Handle<UINode<M, C>>,
    },
}

#[derive(Clone)]
pub struct Popup<M: MessageData, C: Control<M, C>> {
    widget: Widget<M, C>,
    placement: Placement<M, C>,
    stays_open: bool,
    is_open: bool,
    content: Handle<UINode<M, C>>,
    body: Handle<UINode<M, C>>,
}

crate::define_widget_deref!(Popup<M, C>);

impl<M: MessageData, C: Control<M, C>> Control<M, C> for Popup<M, C> {
    fn resolve(&mut self, node_map: &NodeHandleMapping<M, C>) {
        node_map.resolve(&mut self.content);
        node_map.resolve(&mut self.body);
    }

    fn handle_routed_message(
        &mut self,
        ui: &mut UserInterface<M, C>,
        message: &mut UiMessage<M, C>,
    ) {
        self.widget.handle_routed_message(ui, message);

        match &message.data() {
            UiMessageData::Popup(msg) if message.destination() == self.handle() => match msg {
                PopupMessage::Open => {
                    if !self.is_open {
                        self.is_open = true;
                        ui.send_message(WidgetMessage::visibility(
                            self.handle(),
                            MessageDirection::ToWidget,
                            true,
                        ));
                        ui.push_picking_restriction(RestrictionEntry {
                            handle: self.handle(),
                            stop: false,
                        });
                        ui.send_message(WidgetMessage::topmost(
                            self.handle(),
                            MessageDirection::ToWidget,
                        ));
                        let position = match self.placement {
                            Placement::LeftTop(target) => ui
                                .try_get_node(target)
                                .map(|n| n.screen_position())
                                .unwrap_or_default(),
                            Placement::RightTop(target) => ui
                                .try_get_node(target)
                                .map(|n| n.screen_position() + Vector2::new(n.actual_size().x, 0.0))
                                .unwrap_or_else(|| {
                                    Vector2::new(
                                        ui.screen_size().x - self.widget.actual_size().x,
                                        0.0,
                                    )
                                }),
                            Placement::Center(target) => ui
                                .try_get_node(target)
                                .map(|n| n.screen_position() + n.actual_size().scale(0.5))
                                .unwrap_or_else(|| {
                                    (ui.screen_size - self.widget.actual_size()).scale(0.5)
                                }),
                            Placement::LeftBottom(target) => ui
                                .try_get_node(target)
                                .map(|n| n.screen_position() + Vector2::new(0.0, n.actual_size().y))
                                .unwrap_or_else(|| {
                                    Vector2::new(
                                        0.0,
                                        ui.screen_size().y - self.widget.actual_size().y,
                                    )
                                }),
                            Placement::RightBottom(target) => ui
                                .try_get_node(target)
                                .map(|n| n.screen_position() + n.actual_size())
                                .unwrap_or_else(|| ui.screen_size - self.widget.actual_size()),
                            Placement::Cursor(_) => ui.cursor_position(),
                            Placement::Position { position, .. } => position,
                        };
                        ui.send_message(WidgetMessage::desired_position(
                            self.handle(),
                            MessageDirection::ToWidget,
                            position,
                        ));
                    }
                }
                PopupMessage::Close => {
                    if self.is_open {
                        self.is_open = false;
                        ui.send_message(WidgetMessage::visibility(
                            self.handle(),
                            MessageDirection::ToWidget,
                            false,
                        ));
                        ui.remove_picking_restriction(self.handle());
                        if ui.captured_node() == self.handle() {
                            ui.release_mouse_capture();
                        }
                    }
                }
                PopupMessage::Content(content) => {
                    if self.content.is_some() {
                        ui.send_message(WidgetMessage::remove(
                            self.content,
                            MessageDirection::ToWidget,
                        ));
                    }
                    self.content = *content;

                    ui.send_message(WidgetMessage::link(
                        self.content,
                        MessageDirection::ToWidget,
                        self.body,
                    ));
                }
                PopupMessage::Placement(placement) => {
                    self.placement = placement.clone();
                    self.invalidate_layout();
                }
            },
            _ => {}
        }
    }

    fn handle_os_event(
        &mut self,
        self_handle: Handle<UINode<M, C>>,
        ui: &mut UserInterface<M, C>,
        event: &OsEvent,
    ) {
        if let OsEvent::MouseInput { state, .. } = event {
            if let Some(top_restriction) = ui.top_picking_restriction() {
                if *state == ButtonState::Pressed
                    && top_restriction.handle == self_handle
                    && self.is_open
                {
                    let pos = ui.cursor_position();
                    if !self.widget.screen_bounds().contains(pos) && !self.stays_open {
                        ui.send_message(PopupMessage::close(
                            self.handle(),
                            MessageDirection::ToWidget,
                        ));
                    }
                }
            }
        }
    }
}

pub struct PopupBuilder<M: MessageData, C: Control<M, C>> {
    widget_builder: WidgetBuilder<M, C>,
    placement: Placement<M, C>,
    stays_open: bool,
    content: Handle<UINode<M, C>>,
}

impl<M: MessageData, C: Control<M, C>> PopupBuilder<M, C> {
    pub fn new(widget_builder: WidgetBuilder<M, C>) -> Self {
        Self {
            widget_builder,
            placement: Placement::Cursor(Default::default()),
            stays_open: false,
            content: Default::default(),
        }
    }

    pub fn with_placement(mut self, placement: Placement<M, C>) -> Self {
        self.placement = placement;
        self
    }

    pub fn stays_open(mut self, value: bool) -> Self {
        self.stays_open = value;
        self
    }

    pub fn with_content(mut self, content: Handle<UINode<M, C>>) -> Self {
        self.content = content;
        self
    }

    pub fn build(self, ctx: &mut BuildContext<M, C>) -> Handle<UINode<M, C>> {
        let body = BorderBuilder::new(
            WidgetBuilder::new()
                .with_background(BRUSH_DARKER)
                .with_foreground(BRUSH_LIGHTER)
                .with_child(self.content),
        )
        .with_stroke_thickness(Thickness::uniform(1.0))
        .build(ctx);

        let popup = Popup {
            widget: self
                .widget_builder
                .with_child(body)
                .with_visibility(false)
                .with_handle_os_events(true)
                .build(),
            placement: self.placement,
            stays_open: self.stays_open,
            is_open: false,
            content: self.content,
            body,
        };

        ctx.add_node(UINode::Popup(popup))
    }
}
