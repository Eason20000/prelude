use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

const BAR_WIDTH: f64 = 2.0;
const BAR_GAP: f64 = 2.0;
const BAR_SPACING: f64 = BAR_WIDTH + BAR_GAP;
const PLAYED_ALPHA: f64 = 0.7;
const UPCOMING_ALPHA: f64 = 0.35;
const MIN_BAR_HEIGHT: f64 = 1.0;

type PositionCallback = Box<dyn Fn(f64)>;

pub(crate) struct MidiDensityView {
    area: gtk::DrawingArea,
    peaks: Rc<RefCell<Vec<f64>>>,
    position: Rc<Cell<f64>>,
    on_position_changed: Rc<RefCell<Option<PositionCallback>>>,
    dragging: Rc<Cell<bool>>,
}

impl Clone for MidiDensityView {
    fn clone(&self) -> Self {
        Self {
            area: self.area.clone(),
            peaks: self.peaks.clone(),
            position: self.position.clone(),
            on_position_changed: self.on_position_changed.clone(),
            dragging: self.dragging.clone(),
        }
    }
}

impl MidiDensityView {
    pub(crate) fn new() -> Self {
        let peaks = Rc::new(RefCell::new(Vec::new()));
        let position = Rc::new(Cell::new(0.0));
        let on_position_changed: Rc<RefCell<Option<PositionCallback>>> =
            Rc::new(RefCell::new(None));
        let dragging = Rc::new(Cell::new(false));

        let area = gtk::DrawingArea::new();
        area.set_vexpand(true);
        area.set_hexpand(true);
        area.set_content_height(96);
        area.add_css_class("midi-density-view");

        {
            let peaks = peaks.clone();
            let position = position.clone();
            area.set_draw_func(move |area, cr, width, height| {
                let p = peaks.borrow();
                if p.is_empty() {
                    return;
                }
                Self::draw(area, cr, width, height, &p, position.get());
            });
        }

        let drag = gtk::GestureDrag::new();
        {
            let drag_start = Rc::new(Cell::new(0.0));

            let dsp = position.clone();
            let dsa = drag_start.clone();
            let drag1 = dragging.clone();
            drag.connect_drag_begin(move |gesture, _x, _y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                dsa.set(dsp.get());
                drag1.set(true);
            });

            let dsp2 = position.clone();
            let peaks2 = peaks.clone();
            let area2 = area.clone();
            let dsa2 = drag_start.clone();
            drag.connect_drag_update(move |_gesture, offset_x, _offset_y| {
                let p = peaks2.borrow();
                if p.is_empty() {
                    return;
                }
                let pos = Self::position_from_offset(dsa2.get(), offset_x, p.len());
                dsp2.set(pos);
                area2.queue_draw();
            });

            let peaks3 = peaks.clone();
            let dsp3 = position.clone();
            let on_pos3 = on_position_changed.clone();
            let drag3 = dragging.clone();
            drag.connect_drag_end(move |_gesture, _offset_x, _offset_y| {
                drag3.set(false);
                let p = peaks3.borrow();
                if p.is_empty() {
                    return;
                }
                if let Some(ref cb) = *on_pos3.borrow() {
                    cb(dsp3.get());
                }
            });
        }
        area.add_controller(drag);

        Self {
            area,
            peaks,
            position,
            on_position_changed,
            dragging,
        }
    }

    pub(crate) fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    pub(crate) fn set_peaks(&self, p: Vec<f64>) {
        *self.peaks.borrow_mut() = p;
        self.area.queue_draw();
    }

    pub(crate) fn set_position(&self, pos: f64) {
        self.position.set(pos);
        self.area.queue_draw();
    }

    #[allow(dead_code)]
    pub(crate) fn position(&self) -> f64 {
        self.position.get()
    }

    pub(crate) fn set_on_position_changed<F: Fn(f64) + 'static>(&self, f: F) {
        *self.on_position_changed.borrow_mut() = Some(Box::new(f));
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.dragging.get()
    }

    fn position_from_offset(start: f64, offset_x: f64, peak_count: usize) -> f64 {
        (start - offset_x / (peak_count as f64 * BAR_SPACING)).clamp(0.0, 1.0)
    }

    #[allow(deprecated)]
    fn draw(
        area: &gtk::DrawingArea,
        cr: &cairo::Context,
        width: i32,
        height: i32,
        peaks: &[f64],
        position: f64,
    ) {
        let w = width as f64;
        let h = height as f64;
        let center_x = w / 2.0;
        let total_peaks = peaks.len() as f64;

        let style = area.style_context();
        let accent = style.lookup_color("accent_color");
        let dimmed = style.lookup_color("dimmed_color");
        let fallback = style.color();

        let accent_color = accent.unwrap_or(fallback);
        let dimmed_color = dimmed.unwrap_or(fallback);

        cr.set_line_width(BAR_WIDTH);
        cr.set_line_cap(cairo::LineCap::Butt);

        let first_bar_x = center_x - position * total_peaks * BAR_SPACING;

        for (i, &peak) in peaks.iter().enumerate() {
            let pixel_x = first_bar_x + i as f64 * BAR_SPACING;
            if pixel_x + BAR_WIDTH < 0.0 || pixel_x > w {
                continue;
            }

            let raw_height = peak * (h * 0.8);
            let bar_height = raw_height.max(MIN_BAR_HEIGHT);
            let y0 = h / 2.0 - bar_height / 2.0;
            let y1 = h / 2.0 + bar_height / 2.0;

            let color = if pixel_x < center_x {
                &accent_color
            } else {
                &dimmed_color
            };

            let alpha = if pixel_x < center_x {
                PLAYED_ALPHA
            } else {
                UPCOMING_ALPHA
            };

            cr.set_source_rgba(
                color.red().into(),
                color.green().into(),
                color.blue().into(),
                alpha,
            );

            cr.move_to(pixel_x, y0);
            cr.line_to(pixel_x, y1);
            cr.stroke().unwrap();
        }

        cr.set_source_rgba(
            accent_color.red().into(),
            accent_color.green().into(),
            accent_color.blue().into(),
            accent_color.alpha().into(),
        );
        cr.set_line_width(2.0);
        cr.move_to(center_x, 0.0);
        cr.line_to(center_x, h);
        cr.stroke().unwrap();
    }
}
