use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use glib::clone;

use adw::prelude::*;

use crate::engine::{Measure, MidiEngine, State};

// ── Tunable constants (compile-time only; adjust to taste) ──
// Shrink (kashiwade page-wipe) spring
const SHRINK_INTERVAL_MS: u32 = 0; // hold the previous page fully visible before shrinking
const SHRINK_DAMPING_RATIO: f64 = 1.0;
const SHRINK_MASS: f64 = 1.0;
const SHRINK_STIFFNESS: f64 = 500.0;
const SHRINK_EPSILON: f64 = 0.00001;
// Note growth spring
const GROWTH_DAMPING_RATIO: f64 = 1.0;
const GROWTH_MASS: f64 = 1.0;
const GROWTH_STIFFNESS: f64 = 200.0;
const GROWTH_EPSILON: f64 = 0.00001;
const MEASURES_PER_PAGE: usize = 1;
const NOTE_HEIGHT_RATIO: f64 = 1.0; // fraction of a pitch slot (<1 leaves a gap)
const NOTE_AREA_SCALE: f64 = 1.0;
const CONTENT_HEIGHT: i32 = 200;
// Growth spring mass is the note's visible length in quarter-note units, but
// clamped: grace notes would otherwise pop instantly (near-zero mass) and
// very long notes would never settle within the page lifetime (huge mass).
const GROWTH_MASS_MIN_MULT: f64 = 0.25;
const GROWTH_MASS_MAX_MULT: f64 = 4.0;
// Resume thundering-herd guard: at most this many growth springs are fired per
// frame; the rest wait for the following frames (their `started` stays false).
// Paused seeks intentionally leave passed notes blank until resume.
const MAX_GROWTH_SPAWNS_PER_TICK: usize = 16;
// ── Per-track color variation (tune by eye) ──
// Each MIDI channel gets a deterministic lightness/saturation offset from the
// accent color; the hue always stays identical to the accent. Crank the two
// *_DEVIATION constants until the difference between channels is obvious.
const TRACK_SAT_SEED: f64 = 0.618_033_988_749_895; // golden ratio: well-spread per channel
const TRACK_LIGHT_SEED: f64 = 0.754_877_666_246_692_7;
const TRACK_SAT_DEVIATION: f64 = 0.5;
const TRACK_LIGHT_DEVIATION: f64 = 0.3;

/// Per-channel note color: same hue as the accent, with a deterministic
/// lightness/saturation deviation per MIDI channel.
fn track_color(accent: &gdk::RGBA, channel: u8) -> gdk::RGBA {
    let (h, s, l) = rgb_to_hsl(accent);
    let t1 = (channel as f64 * TRACK_SAT_SEED).fract();
    let t2 = (channel as f64 * TRACK_LIGHT_SEED).fract();
    let s = (s * (1.0 + (t1 - 0.5) * 2.0 * TRACK_SAT_DEVIATION)).clamp(0.0, 1.0);
    let l = (l * (1.0 + (t2 - 0.5) * 2.0 * TRACK_LIGHT_DEVIATION)).clamp(0.0, 1.0);
    let (r, g, b) = hsl_to_rgb(h, s, l);
    gdk::RGBA::new(r as f32, g as f32, b as f32, accent.alpha())
}

fn rgb_to_hsl(c: &gdk::RGBA) -> (f64, f64, f64) {
    let r = c.red() as f64;
    let g = c.green() as f64;
    let b = c.blue() as f64;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0) * 60.0
    } else if max == g {
        ((b - r) / d + 2.0) * 60.0
    } else {
        ((r - g) / d + 4.0) * 60.0
    };
    (h, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = if hp < 1.0 {
        (c, x, 0.0)
    } else if hp < 2.0 {
        (x, c, 0.0)
    } else if hp < 3.0 {
        (0.0, c, x)
    } else if hp < 4.0 {
        (0.0, x, c)
    } else if hp < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = l - c / 2.0;
    (r1 + m, g1 + m, b1 + m)
}

/// A note pre-mapped onto the page's normalized coordinate space.
///
/// `Clone` deliberately shares `scale: Rc` but copies `started: Cell`: when a
/// page turns, the old page is cloned into `cached_prev` and the clone keeps
/// driving the same growth cell, so in-flight springs keep animating the
/// outgoing page instead of restarting. `channel` is kept (not just the baked
/// `color`) so an accent-color change can recompute colors without a rebuild.
#[derive(Clone)]
pub(crate) struct CachedNote {
    ratio_x: f64,
    ratio_w: f64,
    norm_pitch: f64,
    time: f64,
    color: gdk::RGBA,
    channel: u8,
    scale: Rc<Cell<f64>>, // growth progress, driven by a per-note spring
    started: Cell<bool>,  // the growth spring has been fired
    mass_mult: f64,       // visible length in quarter-note units, scales the spring mass
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PageTurnView {
        pub(crate) engine: RefCell<Option<Rc<RefCell<MidiEngine>>>>,
        pub(crate) cached_current: RefCell<Vec<CachedNote>>,
        pub(crate) cached_prev: RefCell<Vec<CachedNote>>,
        pub last_page: Cell<i32>,
        pub shrink: Cell<f64>,
        pub last_elapsed: Cell<f64>,
        pub spring: RefCell<Option<adw::SpringAnimation>>,
        pub transition_wait: Cell<Option<f64>>,
        pub accent_notify: RefCell<Option<glib::SignalHandlerId>>,
        /// In-flight per-note growth springs. Fire-and-forget locals would rely
        /// on the animation framework self-keeping the object while playing
        /// (undocumented); holding them here until they report Done removes
        /// that assumption. Drained in tick().
        pub growth: RefCell<Vec<adw::SpringAnimation>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PageTurnView {
        const NAME: &'static str = "PreludePageTurnView";
        type Type = super::PageTurnView;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for PageTurnView {
        fn dispose(&self) {
            // Tick callbacks are removed by GTK itself on widget destroy, so
            // only the global StyleManager handler needs manual disconnect.
            if let Some(handler) = self.accent_notify.borrow_mut().take() {
                adw::StyleManager::default().disconnect(handler);
            }
        }
    }

    impl WidgetImpl for PageTurnView {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Vertical => (CONTENT_HEIGHT, CONTENT_HEIGHT, -1, -1),
                _ => (0, 0, -1, -1),
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let w = widget.width() as f64;
            let h = widget.height() as f64;
            if w <= 0.0 || h <= 0.0 {
                return;
            }

            let eff_h = h * NOTE_AREA_SCALE;
            let top = (h - eff_h) / 2.0;
            let note_h = eff_h / 128.0 * NOTE_HEIGHT_RATIO; // one pitch slot, scaled with height

            // Previous page: shrinking left-to-right (kashiwade)
            let shrink = self.shrink.get();
            for n in self.cached_prev.borrow().iter() {
                let eaten = n.ratio_w * shrink;
                let vis_start = n.ratio_x + eaten;
                let vis_end = (n.ratio_x + n.ratio_w * n.scale.get().clamp(0.0, 1.0)).min(1.0);
                if vis_start >= vis_end {
                    continue;
                }
                let x = vis_start * w;
                let width = (vis_end - vis_start) * w;
                let y = top + (eff_h - note_h) - n.norm_pitch * (eff_h - note_h);
                snapshot.append_color(
                    &n.color,
                    &graphene::Rect::new(x as f32, y as f32, width as f32, note_h as f32),
                );
            }

            // Current page: notes grow in via their per-note spring
            for n in self.cached_current.borrow().iter() {
                let scale = n.scale.get().clamp(0.0, 1.0);
                let width = n.ratio_w * scale * w;
                if width <= 0.0 {
                    continue;
                }
                let x = n.ratio_x * w;
                let y = top + (eff_h - note_h) - n.norm_pitch * (eff_h - note_h);
                snapshot.append_color(
                    &n.color,
                    &graphene::Rect::new(x as f32, y as f32, width as f32, note_h as f32),
                );
            }
        }
    }
}

glib::wrapper! {
    pub struct PageTurnView(ObjectSubclass<imp::PageTurnView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PageTurnView {
    pub(crate) fn new(engine: Rc<RefCell<MidiEngine>>) -> Self {
        let view: Self = glib::Object::new();
        view.imp().engine.replace(Some(engine));
        view.set_vexpand(true);
        view.set_hexpand(true);
        view.add_css_class("page-turn-view");

        // Kashiwade shrink: a spring from 0 → 1 drives the "eaten" width.
        let target = adw::CallbackAnimationTarget::new(clone!(
            #[weak]
            view,
            move |value| {
                view.imp().shrink.set(value.clamp(0.0, 1.0));
                view.queue_draw();
            },
        ));
        let spring = adw::SpringAnimation::new(
            &view,
            0.0,
            1.0,
            adw::SpringParams::new(SHRINK_DAMPING_RATIO, SHRINK_MASS, SHRINK_STIFFNESS),
            target,
        );
        spring.set_initial_velocity(0.0);
        spring.set_epsilon(SHRINK_EPSILON);
        *view.imp().spring.borrow_mut() = Some(spring);

        // Frame-driven update loop, synchronized with the display refresh.
        // The strong capture keeps the view alive for the process lifetime
        // (the callback lives on the widget's frame clock), which is fine —
        // the view is created once per window. GTK removes tick callbacks on
        // widget destroy, so the cycle breaks there; no manual removal needed.
        view.add_tick_callback(clone!(
            #[strong]
            view,
            move |_widget, _clock| {
                view.tick();
                glib::ControlFlow::Continue
            },
        ));

        // Accent color changes don't trigger a redraw on their own, and note
        // colors are baked at rebuild time: recompute them from the stored
        // channel so the view follows the system accent without waiting for
        // the next page turn.
        let style_manager = adw::StyleManager::default();
        let accent_notify = style_manager.connect_accent_color_rgba_notify(clone!(
            #[weak]
            view,
            move |manager| {
                let accent = manager.accent_color_rgba();
                let imp = view.imp();
                for cache in [&imp.cached_current, &imp.cached_prev] {
                    for n in cache.borrow_mut().iter_mut() {
                        n.color = track_color(&accent, n.channel);
                    }
                }
                view.queue_draw();
            },
        ));
        *view.imp().accent_notify.borrow_mut() = Some(accent_notify);

        view
    }

    pub(crate) fn widget(&self) -> &gtk::Widget {
        self.upcast_ref()
    }

    pub(crate) fn reset(&self) {
        let imp = self.imp();
        *imp.cached_current.borrow_mut() = Vec::new();
        *imp.cached_prev.borrow_mut() = Vec::new();
        imp.last_page.set(-1);
        imp.shrink.set(1.0);
        imp.last_elapsed.set(0.0);
        imp.transition_wait.set(None);
        if let Some(spring) = imp.spring.borrow().as_ref() {
            spring.pause();
            spring.reset();
        }
        self.queue_draw();
    }

    /// Per-frame update: page detection, cache rebuild and transition start.
    fn tick(&self) {
        let imp = self.imp();
        let Some(engine) = imp.engine.borrow().as_ref().map(Rc::clone) else {
            return;
        };
        let eng = engine.borrow();
        let measures = eng.measures();
        if measures.is_empty() {
            return;
        }
        let elapsed = eng.elapsed();
        let playing = eng.state() == State::Playing;

        let active = Self::find_measure(measures, elapsed);
        let page = (active / MEASURES_PER_PAGE) as i32;
        let last = imp.last_page.get();

        if page != last {
            if last >= 0 && page == last + 1 {
                // Forward page turn: keep the old page for the kashiwade wipe.
                *imp.cached_prev.borrow_mut() = imp.cached_current.borrow().clone();
                imp.shrink.set(0.0);
                self.start_transition(elapsed);
            } else {
                // Seek / jump: skip the transition entirely.
                *imp.cached_prev.borrow_mut() = Vec::new();
                imp.shrink.set(1.0);
                imp.transition_wait.set(None);
                if let Some(spring) = imp.spring.borrow().as_ref() {
                    spring.pause();
                }
            }
            imp.last_page.set(page);
            self.rebuild_cache(measures, page);
        }

        // Kashiwade wait: hold the old page fully visible for the interval,
        // then play the shrink spring. Elapsed-based, so pausing freezes it.
        if let Some(wait_start) = imp.transition_wait.get() {
            if elapsed - wait_start >= SHRINK_INTERVAL_MS as f64 / 1000.0 {
                imp.transition_wait.set(None);
                self.play_spring();
            }
        }

        // Note growth: fire a spring per note when its start time passes.
        // Covers both the current page and the outgoing page, so notes that
        // hadn't started at the page turn still grow (and get wiped) visibly.
        // Paused seeks intentionally leave passed notes blank until resume;
        // the per-frame cap spreads the resume burst over several frames.
        if playing {
            let mut spawned = 0usize;
            'outer: for cache in [&imp.cached_current, &imp.cached_prev] {
                for n in cache.borrow().iter() {
                    if spawned >= MAX_GROWTH_SPAWNS_PER_TICK {
                        break 'outer;
                    }
                    if !n.started.get() && elapsed >= n.time {
                        n.started.set(true);
                        self.spawn_growth_spring(n);
                        spawned += 1;
                    }
                }
            }
        }

        // Drain finished growth springs (see spawn_growth_spring).
        imp.growth
            .borrow_mut()
            .retain(|s| s.state() == adw::AnimationState::Playing);

        let spring_running = imp
            .spring
            .borrow()
            .as_ref()
            .is_some_and(|s| s.state() == adw::AnimationState::Playing);
        let elapsed_changed = (elapsed - imp.last_elapsed.get()).abs() > 1e-9;
        if playing || spring_running || elapsed_changed {
            imp.last_elapsed.set(elapsed);
            self.queue_draw();
        }
    }

    /// Cache the notes belonging to `page`, mapped onto normalized coordinates.
    fn rebuild_cache(&self, measures: &[Measure], page: i32) {
        let imp = self.imp();
        let Some(engine) = imp.engine.borrow().as_ref().map(Rc::clone) else {
            return;
        };
        let eng = engine.borrow();
        // NOTE: page * MEASURES_PER_PAGE, not page alone — with
        // MEASURES_PER_PAGE == 1 the two coincide, which hid this bug.
        let start_idx = (page as usize * MEASURES_PER_PAGE).min(measures.len() - 1);
        let end_idx = (start_idx + MEASURES_PER_PAGE - 1).min(measures.len() - 1);
        let page_start = measures[start_idx].start;
        let page_end = measures[end_idx].end;
        let page_duration = (page_end - page_start).max(1e-6);

        let accent = adw::StyleManager::default().accent_color_rgba();
        // 16-entry per-channel LUT: one HSL round-trip per channel, not per note.
        let lut: Vec<gdk::RGBA> = (0..16u8).map(|ch| track_color(&accent, ch)).collect();
        let quarter = measures[start_idx].quarter.max(1e-6);
        let mut cached = Vec::new();
        for n in eng.notes() {
            // eng.notes() is sorted by time: once a note starts past the page
            // end, so do all following ones.
            if n.time > page_end {
                break;
            }
            let note_start = n.time;
            let note_end = n.time + n.duration;
            if note_end < page_start || note_start > page_end {
                continue;
            }
            // Clip notes spanning the page boundaries into this page's slice.
            let clip_start = note_start.max(page_start);
            let clip_end = note_end.min(page_end);
            if clip_start >= clip_end {
                continue;
            }
            cached.push(CachedNote {
                ratio_x: (clip_start - page_start) / page_duration,
                ratio_w: (clip_end - clip_start) / page_duration,
                norm_pitch: (n.midi as f64 / 127.0).clamp(0.0, 1.0),
                time: clip_start,
                color: lut[(n.channel & 0x0F) as usize],
                channel: n.channel,
                scale: Rc::new(Cell::new(0.0)),
                started: Cell::new(false),
                mass_mult: ((clip_end - clip_start) / quarter)
                    .clamp(GROWTH_MASS_MIN_MULT, GROWTH_MASS_MAX_MULT),
            });
        }
        *imp.cached_current.borrow_mut() = cached;
    }

    /// Record the start of the kashiwade wait; the spring itself is played
    /// from `tick()` once the interval has elapsed.
    /// NOTE: with SHRINK_INTERVAL_MS == 0 the wait branch below is dead code
    /// by construction (the tunable is kept for experimentation).
    fn start_transition(&self, elapsed: f64) {
        let imp = self.imp();
        if SHRINK_INTERVAL_MS == 0 {
            self.play_spring();
            return;
        }
        imp.transition_wait.set(Some(elapsed));
    }

    fn play_spring(&self) {
        let imp = self.imp();
        if let Some(spring) = imp.spring.borrow().as_ref() {
            spring.reset();
            spring.set_value_from(0.0);
            spring.set_value_to(1.0);
            spring.set_initial_velocity(0.0);
            spring.play();
        }
    }

    /// Fire-and-forget growth spring for a note: animates its `scale` cell
    /// from 0 to 1. The animation is held in `imp.growth` until it reports
    /// Done (drained in tick()) instead of relying on the framework
    /// self-keeping a playing animation alive after the local is dropped;
    /// stale callbacks only touch their own orphaned cell.
    fn spawn_growth_spring(&self, note: &CachedNote) {
        let view = self.clone();
        let scale = note.scale.clone();
        let target = adw::CallbackAnimationTarget::new(clone!(
            #[weak]
            view,
            move |value| {
                scale.set(value);
                view.queue_draw();
            },
        ));
        let spring = adw::SpringAnimation::new(
            &view,
            0.0,
            1.0,
            adw::SpringParams::new(
                GROWTH_DAMPING_RATIO,
                GROWTH_MASS * note.mass_mult,
                GROWTH_STIFFNESS,
            ),
            target,
        );
        spring.set_epsilon(GROWTH_EPSILON);
        spring.play();
        self.imp().growth.borrow_mut().push(spring);
    }

    /// Index of the measure containing `time` (binary search).
    fn find_measure(measures: &[Measure], time: f64) -> usize {
        match measures.binary_search_by(|m| m.start.total_cmp(&time)) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    }
}
