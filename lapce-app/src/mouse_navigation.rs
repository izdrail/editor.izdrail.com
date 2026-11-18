//! Capture mouse side buttons before child controls can consume them or move a caret.
use floem::{
    View, ViewId,
    context::EventCx,
    event::{Event, EventPropagation},
    pointer::{MouseButton, PointerInputEvent},
};

fn side_button(event: &PointerInputEvent) -> bool {
    matches!(event.button.mouse_button(), MouseButton::X1 | MouseButton::X2)
}

fn route(event: &Event, dispatch: &dyn Fn(&PointerInputEvent)) -> EventPropagation {
    match event {
        Event::PointerDown(pointer) if side_button(pointer) => {
            dispatch(pointer);
            EventPropagation::Stop
        }
        // Navigation is once per press. Do not let release activate a child control.
        Event::PointerUp(pointer) if side_button(pointer) => EventPropagation::Stop,
        _ => EventPropagation::Continue,
    }
}

pub struct MouseNavigation {
    id: ViewId,
    dispatch: Box<dyn Fn(&PointerInputEvent)>,
}

pub fn mouse_navigation(
    child: impl View + 'static,
    dispatch: impl Fn(&PointerInputEvent) + 'static,
) -> MouseNavigation {
    let id = ViewId::new();
    id.set_children([child]);
    MouseNavigation { id, dispatch: Box::new(dispatch) }
}

impl View for MouseNavigation {
    fn id(&self) -> ViewId { self.id }

    fn event_before_children(&mut self, _cx: &mut EventCx, event: &Event) -> EventPropagation {
        route(event, &*self.dispatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use floem::{keyboard::Modifiers, pointer::PointerButton, peniko::kurbo::Point};
    use crate::keypress::{keymap::{KeyMapKey, KeyMapPress}};

    fn pointer(button: PointerButton) -> PointerInputEvent {
        PointerInputEvent { button, pos: Point::new(50.0, 100.0), modifiers: Modifiers::empty(), count: 1 }
    }

    #[test]
    fn mouse_navigation_dispatches_once_and_captures_release() {
        for button in [PointerButton::Mouse(MouseButton::X1), PointerButton::Mouse(MouseButton::X2), PointerButton::Unknown(3), PointerButton::Unknown(4)] {
            let calls = Cell::new(0);
            let dispatch = |_: &PointerInputEvent| calls.set(calls.get() + 1);
            let p = pointer(button);
            assert!(route(&Event::PointerDown(p.clone()), &dispatch).is_processed());
            assert!(route(&Event::PointerUp(p), &dispatch).is_processed());
            assert_eq!(calls.get(), 1);
        }
    }

    #[test]
    fn mouse_navigation_preserves_regular_buttons_and_other_events() {
        for button in [MouseButton::Primary, MouseButton::Secondary, MouseButton::Auxiliary, MouseButton::None] {
            let p = pointer(PointerButton::Mouse(button));
            assert!(!route(&Event::PointerDown(p.clone()), &|_| panic!("not a side button")).is_processed());
            assert!(!route(&Event::PointerUp(p), &|_| panic!("release dispatched")).is_processed());
        }
        assert!(!route(&Event::PointerLeave, &|_| panic!("not a press")).is_processed());
    }

    #[test]
    fn mouse_navigation_names_parse_roundtrip_and_support_modifiers() {
        for (name, button) in [("MouseBackward", MouseButton::X1), ("MouseForward", MouseButton::X2), ("MouseMiddle", MouseButton::Auxiliary)] {
            for label in [name.to_string(), name.to_lowercase(), format!("Ctrl+{name}")] {
                let keys = KeyMapPress::parse(&label);
                assert_eq!(keys.len(), 1);
                assert_eq!(keys[0].key, KeyMapKey::Pointer(PointerButton::Mouse(button)));
                assert_eq!(KeyMapPress::parse(&keys[0].to_string()), keys);
            }
        }
    }

}
