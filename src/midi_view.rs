use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib;
use gtk::glib::subclass::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use glib::clone;

const BAR_WIDTH: f64 = 2.0;
const BAR_GAP: f64 = 2.0;
const BAR_SPACING: f64 = BAR_WIDTH + BAR_GAP;
const PLAYED_ALPHA: f32 = 0.7;
const UPCOMING_ALPHA: f32 = 0.35;
const MIN_BAR_HEIGHT: f64 = 1.0;
const CONTENT_HEIGHT: i32 = 96;

type PositionCallback = Box<dyn Fn(f64)>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct MidiDensityView {
        pub peaks: RefCell<Vec<f64>>,
        pub position: Cell<f64>,
        pub on_position_changed: RefCell<Option<PositionCallback>>,
        pub dragging: Cell<bool>,
        pub accent_notify: RefCell<Option<glib::SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MidiDensityView {
        const NAME: &'static str = "PreludeMidiDensityView";
        type Type = super::MidiDensityView;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for MidiDensityView {}

    impl WidgetImpl for MidiDensityView {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Vertical => (CONTENT_HEIGHT, CONTENT_HEIGHT, -1, -1),
                _ => (0, 0, -1, -1),
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let peaks = self.peaks.borrow();
            if peaks.is_empty() {
                return;
            }

            let w = widget.width() as f64;
            let h = widget.height() as f64;
            if w <= 0.0 || h <= 0.0 {
                return;
            }
            let center_x = w / 2.0;

            let accent = adw::StyleManager::default().accent_color_rgba();
            let foreground = widget.color();
            let played = accent.with_alpha(PLAYED_ALPHA);
            let upcoming = foreground.with_alpha(UPCOMING_ALPHA);

            let first_bar_x = center_x - self.position.get() * peaks.len() as f64 * BAR_SPACING;

            for (i, &peak) in peaks.iter().enumerate() {
                let pixel_x = first_bar_x + i as f64 * BAR_SPACING;
                if pixel_x + BAR_WIDTH < 0.0 || pixel_x > w {
                    continue;
                }

                let bar_height = (peak * (h * 0.8)).max(MIN_BAR_HEIGHT);
                let y0 = h / 2.0 - bar_height / 2.0;
                let color = if pixel_x < center_x {
                    &played
                } else {
                    &upcoming
                };

                snapshot.append_color(
                    color,
                    &graphene::Rect::new(
                        (pixel_x - BAR_WIDTH / 2.0) as f32,
                        y0 as f32,
                        BAR_WIDTH as f32,
                        bar_height as f32,
                    ),
                );
            }

            snapshot.append_color(
                &foreground,
                &graphene::Rect::new((center_x - 1.0) as f32, 0.0, 2.0, h as f32),
            );
        }
    }
}

glib::wrapper! {
    pub struct MidiDensityView(ObjectSubclass<imp::MidiDensityView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl MidiDensityView {
    pub(crate) fn new() -> Self {
        let view: Self = glib::Object::new();
        view.set_vexpand(true);
        view.set_hexpand(true);
        view.add_css_class("midi-density-view");

        let drag = gtk::GestureDrag::new();
        let drag_start = Rc::new(Cell::new(0.0));

        drag.connect_drag_begin(clone!(
            #[strong]
            view,
            #[strong]
            drag_start,
            move |gesture, _x, _y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                drag_start.set(view.imp().position.get());
                view.imp().dragging.set(true);
            },
        ));

        drag.connect_drag_update(clone!(
            #[strong]
            view,
            #[strong]
            drag_start,
            move |_gesture, offset_x, _offset_y| {
                let peaks = view.imp().peaks.borrow();
                if peaks.is_empty() {
                    return;
                }
                let pos = Self::position_from_offset(drag_start.get(), offset_x, peaks.len());
                drop(peaks);
                view.imp().position.set(pos);
                view.queue_draw();
            },
        ));

        drag.connect_drag_end(clone!(
            #[strong]
            view,
            move |_gesture, _offset_x, _offset_y| {
                view.imp().dragging.set(false);
                let peaks = view.imp().peaks.borrow();
                if peaks.is_empty() {
                    return;
                }
                if let Some(ref cb) = *view.imp().on_position_changed.borrow() {
                    cb(view.imp().position.get());
                }
            },
        ));

        view.add_controller(drag);

        // The rendered colors come from AdwStyleManager (not from this widget's
        // CSS), so GTK won't redraw us on accent changes by itself. Re-snapshot
        // whenever the system accent color changes.
        let style_manager = adw::StyleManager::default();
        let accent_notify = style_manager.connect_accent_color_rgba_notify(clone!(
            #[weak]
            view,
            move |_| {
                view.queue_draw();
            },
        ));
        *view.imp().accent_notify.borrow_mut() = Some(accent_notify);

        view
    }

    pub(crate) fn widget(&self) -> &gtk::Widget {
        self.upcast_ref()
    }

    pub(crate) fn set_peaks(&self, peaks: Vec<f64>) {
        *self.imp().peaks.borrow_mut() = peaks;
        self.queue_draw();
    }

    pub(crate) fn set_position(&self, pos: f64) {
        self.imp().position.set(pos);
        self.queue_draw();
    }

    pub(crate) fn position(&self) -> f64 {
        self.imp().position.get()
    }

    pub(crate) fn set_on_position_changed<F: Fn(f64) + 'static>(&self, f: F) {
        *self.imp().on_position_changed.borrow_mut() = Some(Box::new(f));
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.imp().dragging.get()
    }

    fn position_from_offset(start: f64, offset_x: f64, peak_count: usize) -> f64 {
        (start - offset_x / (peak_count as f64 * BAR_SPACING)).clamp(0.0, 1.0)
    }
}
