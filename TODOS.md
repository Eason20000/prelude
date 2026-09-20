# TODOS — Deferred and Open Items

This file consolidates all confirmed but not-yet-fixed items after `fa0bcce`.
Prior decisions: **malformed MIDI will be handled uniformly by a future
state-machine parser rewrite**, **seek while paused stays blank until playback
resumes**, **backward page turns cut directly**, **Stop clears the canvas**, and
**growth rate remains tied to note length**. `README.md` intentionally contains
no roadmap; `AGENTS.md` is the authoritative source of constraints, while this
file is the detailed checklist.

Convention: each entry provides full context, defect location, expected
behavior, and rationale. Priority levels reuse the previously reviewed scale
(mainly P2/P3; deferred items are not ranked as P0 to avoid misleading
severity). `file:line` references are based on the `fa0bcce` snapshot and may
drift by about one line with subsequent edits.

______________________________________________________________________

## A. Engine — Deferred to State-Machine Rewrite

The subsections in `src/engine.rs` below are facets of the same parser. Patching
them individually would create conflicting fixes, so they are deferred together
for subsequent research.

### A1. `tempo=0` Causes `calculate_measure_map` to Hang — `src/engine.rs:469-490`

- **Context**: `calculate_measure_map` advances `current_tick` by time
  signature, but `current_time` is derived from `seconds_for_tick(end_tick)`;
  `end_time` is clamped with `.max(current_time)`.
- **Location**: `src/engine.rs:476` uses `.max` to mask non-monotonic tempo.
  When `us_per_quarter==0`, time never advances, the exit condition
  `current_time > total_length+5.0` is never met, and zero-length measures are
  appended indefinitely.
- **Expected**: In the future state-machine parser, reject `us_per_quarter==0`
  or clamp it to a valid lower bound instead of masking with `max`.
- **Rationale**: Requires a malformed file; low probability but severity is
  hang-level. Best addressed uniformly in a single rewrite; deferred to
  subsequent research.

### A2. Phantom Measures and Empty-File Fallback — `src/engine.rs:488-500`

- **Context**: `calculate_measure_map` uses `total_length+5.0` as a fudge to
  generate one extra beat; when the table is empty it falls back to a synthetic
  `0..max(total,2.0)` 4/4 measure.
- **Location**: `src/engine.rs:488` magic number; `src/engine.rs:493-499` empty
  file is disguised as a 2 s song.
- **Expected**: Define empty-file and tail semantics uniformly in the state
  machine (reject or represent explicitly as "no measures") instead of relying
  on magic numbers.
- **Rationale**: Currently affects only display and density ratio, with no
  playback-correctness risk; deferred to subsequent research.

### A3. Silent Time-Signature Fix and Barline Drift — `src/engine.rs:465-468`

- **Context**: `num.max(1)` fixes `num==0`, `den=1<<den_pow.min(16)` silently
  clamps a corrupt `den_pow`, and `ticks_per_measure=num*tpq*4/den` truncates
  with integer division.
- **Location**: `src/engine.rs:466-468` does not report corrupt time signatures
  and uses integer division, causing cumulative barline drift.
- **Expected**: In the state machine, report corrupt time signatures or apply an
  explicit policy, and keep barline precision with rational/fractional
  arithmetic in the tick domain.
- **Rationale**: Rare in real libraries; a fix would require fractional
  arithmetic and is best handled together with the rewrite; deferred to
  subsequent research.

### A4. Mid-Measure Time-Signature Change Misaligned — `src/engine.rs:459-464`

- **Context**: Time-signature switches only at measure boundaries
  (`while meter_idx+1 … current_tick >= …`).
- **Location**: A change that falls inside a measure is stretched until the next
  measure takes effect, shifting all downstream downbeats.
- **Expected**: Split the current measure at the change tick instead of waiting
  for the next measure.
- **Rationale**: Rare but a genuine error; fixing it requires restructuring
  measure generation and is best deferred to the state-machine rewrite; for
  subsequent research.

### A5. FIFO Pairing Mismatch for Same `(channel,key)` — `src/engine.rs:516-522`

- **Context**: `note_intervals` uses a `Vec` with linear `position()` to find
  the first pending note; dangling `NoteOn` is extended to `total_length`.
- **Location**: On legato or re-strike of the same key, the second `NoteOn` is
  mismatched via FIFO and the second dangling note becomes an EOF ghost; a LIFO
  stack is required.
- **Expected**: Use `HashMap<(ch,key), Vec<start>>` with stack (LIFO) pairing.
- **Rationale**: Affects visual ghost notes and density counts, but is part of
  parsing semantics; to be unified in the state machine; deferred to subsequent
  research.

### A6. Tail-Length Policy for Dangling and Orphan Notes — `src/engine.rs:425-426,534-544`

- **Context**: Dangling `NoteOn` is uniformly extended to `total_length`, where
  `total_length` is taken from the last event regardless of type; orphan
  `NoteOff` is silently ignored; zero-length `(t-start).max(0.0)` is retained.
- **Location**: `src/engine.rs:425` produces an overlong tail if the last event
  is CC/SysEx, and zero duration if the last event itself is `NoteOn`;
  `src/engine.rs:516-530` invisible zero-length notes still contribute to
  density; `src/engine.rs:518` discarding orphans hides truncated-file issues.
- **Expected**: Define an explicit tail policy in the state machine (cut by last
  NoteOff / trailing silence), a zero-length policy, and orphan warnings.
- **Rationale**: The three aspects are interrelated; fixing one in isolation
  would conflict with the others; deferred to subsequent research.

### A7. Same-Tick Ordering and SysEx Discard — `src/engine.rs:386-423`

- **Context**: `flatten` collects tempo/meter and raw events;
  `tempo_changes/meters` are sorted only by tick without deduplication; `SysEx`
  reassembly assumes delimiters have been stripped and `Escape` falls into
  `_ => {}`; `events.sort_by(total_cmp)` stably preserves track order.
- **Location**: Ordering within the same tick is undefined, `0xF7` continuations
  are discarded, and simultaneous `NoteOff`/`NoteOn` may be inverted.
- **Expected**: In the state machine, sort and deduplicate by `(tick, kind)`
  with a secondary key and reassemble SysEx per spec.
- **Rationale**: Low-probability data issue; a fix requires an explicit priority
  table and is best handled in the rewrite; deferred to subsequent research.

### A8. Performance: Linear Scan of Tempo Table and `pending` Scan — `src/engine.rs:505-547`

- **Context**: `seconds_for_tick` linearly scans `tempo_map` on every call (once
  per event and once per measure); `pending` uses linear `position()`.
- **Location**: `O(n·m)` and `O(pending)`, observable with dense polyphony.
- **Expected**: After sorting and deduplication, use a cursor/binary search and
  a `HashMap` stack.
- **Rationale**: One-time cost at load time; typical files have very few tempo
  segments; P3 with no perceptible impact; deferred to subsequent research.

______________________________________________________________________

## B. Page View — Rendering and Animation

### B1. Zero-Duration Notes Are Invisible (For Subsequent Research) — `src/page_view.rs:424-429`

- **Context**: In `rebuild_cache`, `clip_start>=clip_end` causes a direct
  `continue`; NoteOn/NoteOff on the same tick (common for percussion and
  ornaments) is dropped.
- **Location**: `src/page_view.rs:428` a note that sounds has no pixels.
- **Expected**: Candidate (a) assign a minimum 1 px tick width, or (b) keep
  discarding and document the behavior. Choice deferred to subsequent research
  on whether percussion warrants a dedicated pixel.
- **Rationale**: Audible-but-invisible is information loss, but if zero-length
  notes are all dirty data then discarding is more correct; for subsequent
  research.

### B2. Missing Hysteresis on Barlines — `src/page_view.rs:502-508`

- **Context**: `find_measure` binary-searches only by `start`; a time exactly on
  a barline is assigned to the next measure.
- **Location**: Scrubbing back and forth near a barline causes page-number
  jitter.
- **Expected**: Keep current behavior for normal playback (assigning to the next
  measure on monotonic advance is correct). If revisited, add ± a few ms of
  hysteresis.
- **Rationale**: Musically "barline is the start of the new measure" is correct;
  P3; for subsequent research.

### B3. Dual Source of Truth for Shrink Value — `src/page_view.rs:330-333,448-468`

- **Context**: On page turn, `shrink` is manually set to `0.0`, and
  `play_spring` also calls `reset()+set_value_from(0)`; the seek branch calls
  `pause()` without `reset()`.
- **Location**: The same fact is written in two places; the internal value and
  the manually set `1.0` may diverge (harmless when there is no `cached_prev`).
- **Expected**: Use the spring object as the single source of truth; manual
  assignment only as the initial-frame value with a comment.
- **Rationale**: Currently harmless, but future readers of the spring state
  after `seek` may be misled; P3.

### B4. `SHRINK_INTERVAL_MS=0` Dead Branch — `src/page_view.rs:17,349,448-457`

- **Context**: When `SHRINK_INTERVAL_MS==0`, the `transition_wait` waiting
  branch is never reachable.
- **Location**: Dead code contradicts the "hold visible" comment.
- **Expected**: Keep the constant as a tuning knob and annotate the dead branch
  as enabled only when `SHRINK_INTERVAL_MS>0`.
- **Rationale**: Scaffolding for a tunable parameter; P3.

### B5. Double Spring for Cross-Page Sustained Notes — `src/page_view.rs:413-442`

- **Context**: A sustained note spanning a barline naturally enters two pages
  and is clipped separately, each with independent
  `scale`/`started`/`mass_mult`.
- **Location**: The same physical note is counted and grown twice; `mass_mult`
  is computed inconsistently from each clipped length.
- **Expected**: Keep dual-page clipping (avoiding gaps is correct), but compute
  mass from the original length or document that double-counting is intentional.
- **Rationale**: Few boundary notes; P3 hygiene item.

### B6. Horizontal `measure()` Returns `(0,0)` — `src/page_view.rs:163-168` (`midi_view.rs:43` same)

- **Context**: Horizontal natural size returns 0 and relies entirely on
  `hexpand(true)` in code.
- **Location**: Removing `hexpand` would collapse width to 0; layout contract is
  incomplete.
- **Expected**: Return a sane natural width corresponding to `CONTENT_HEIGHT`
  for the horizontal axis.
- **Rationale**: Currently covered by `hexpand`; P3.

### B7. `NOTE_HEIGHT_RATIO`/`NOTE_AREA_SCALE==1.0` Is a No-Op — `src/page_view.rs:28-30,178-180`

- **Context**: `eff_h==h` and `top==0`; adjacent pitches leave only a hairline
  gap from the difference between `eff_h/127` spacing and `eff_h/128` height.
- **Location**: Tuning knobs nominally exist but currently have no gap
  semantics; readers may expect them to produce a gap.
- **Expected**: Keep the constants but annotate that at `1.0` the gap comes only
  from the spacing difference, or set a default `<1`.
- **Rationale**: Pure tuning clarity; P3.

### B8. Per-Frame Scan and Growth-Spring Cost — `src/page_view.rs:360-500`

- **Context**: `tick` scans both caches each frame to find notes to trigger, and
  on page turn scans all of `eng.notes()`; already mitigated with
  `MAX_GROWTH_SPAWNS_PER_TICK=16` throttling, `imp.growth` retention, a
  16-channel LUT, and early-break.
- **Location**: Complexity is already acceptable; the remaining cost is multiple
  `queue_draw` callbacks within the same frame.
- **Expected**: Keep as is; add indexing/batching to rebuild only if needed
  later.
- **Rationale**: Files with several thousand notes are already smooth; P2 was
  mitigated by throttling, remaining is P3.

### B9. `CachedNote::clone` Shared/Copy Mix — `src/page_view.rs:111-122,330`

- **Context**: `Clone` shares `scale:Rc` and copies `started:Cell`; the old page
  is cloned into `cached_prev`, after which stale springs only touch orphaned
  cells.
- **Location**: Subtle semantics, correct but easy to be "optimized" away.
- **Expected**: Keep a comment stating that sharing is intentional, or unify to
  `Rc<Cell>`.
- **Rationale**: Currently correct; P3.

### B10. `reset()` Omits `growth` Clear and Tick Strong-Reference Note — `src/page_view.rs:152-159,294-307`

- **Context**: `reset()` clears caches/page/shrink/last_elapsed but does not
  explicitly clear the `growth` vector; `add_tick_callback` relies on strong
  capture that detaches on GTK destruction.
- **Location**: `growth` self-heals via `drain` in `tick`; the strong-reference
  comment assumes a single-window model.
- **Expected**: Add `growth.borrow_mut().clear()` in `reset()` and solidify the
  single-window assumption in the comment.
- **Rationale**: P3 hygiene item.

______________________________________________________________________

## C. Application — Wiring and Semantics

### C1. Seek Loses Timbre (CC/Program Snapshot) — `src/engine.rs:193, application.rs:480-512`

- **Context**: `seek` repositions `next_idx` via `partition_point(t < pos)` and
  replays events exactly at `pos`, but does not replay prior CC/Program.
- **Location**: After a backward seek, notes use stale timbre; `all_notes_off`
  masks stuck notes.
- **Expected**: In the state machine, replay a snapshot of the last CC/Program
  before the seek position, or document the behavior as unsupported.
- **Rationale**: Classic naive-player debt; a fix requires a state snapshot and
  is best handled in the state machine; P3; deferred to subsequent research.

### C2. `view_root` Insertion Order by Convention — `src/application.rs:141-149, ui/window.blp:123` — **Fixed**

- **Context**: Previously `prepend(page)` + `insert_child_after(density, page)`
  yielded `[page, density, Clamp]`; `view_root` had no placeholder in `.blp` and
  `vexpand` was set only in code.
- **Location**: Invisible in the Blueprint editor; layout depended on code
  convention.
- **Expected**: Fixed in `ui/window.blp:123-153` by adding
  `Label label_name[xalign 0.0 ellipsize end title-1 top]` +
  `Box page/density_placeholder[vexpand true/false]` as parent containers;
  `src/application.rs:150-156` now uses `placeholder.append(child)`, making
  order declarative `[label, page_ph→page, density_ph→density, Clamp]` without
  `remove`/`prepend`. This entry is retained to record the decision; `AGENTS.md`
  has been synchronized.
- **Rationale**: P3 maintainability; closed.

### C3. `GFile.path().unwrap_or_default()` Obscures Cause — `src/application.rs:206-210,386-390`

- **Context**: Non-local files are coerced to `""`; `load_file:67` already
  reports an `error-view` for empty paths, but the call site still uses
  `unwrap_or_default()`.
- **Location**: The error message loses the specific cause.
- **Expected**: Guard at the call site with `if let Some(path)=file.path()` and
  otherwise report the non-local cause directly.
- **Rationale**: Single-line guard; P3.

### C4. Port Selection Triggers `RefCell` Panic (Found in Review, Latent in HEAD) — `src/application.rs:288-304,563-576` / `src/application.rs:46-58` / `src/application.rs:310-322`

- **Context**: The `selected` property of `port_row`/`port_dropdown` is bound
  via `StringList`; `splice` in `populate_ports()` or `set_selected()` in
  `select_port()` synchronously emits `selected_notify`, while the call sites in
  `port_action` and the initial `Refresh ports` block invoke `select_port` while
  an immutable `engine.borrow()` is still live.
- **Location**: `src/application.rs:290-292` / `src/application.rs:564` does
  `let current = engine.borrow(); select_port(&port_row, &ports, current.port_name());`
  where `current: Ref` lives into `select_port:46` `row.set_selected()`, and the
  synchronous callback `port_row.connect_selected_notify:310` calls
  `engine.borrow_mut().open_port()`, causing an `already borrowed` panic. The
  same shape existed for `DropDown` at `HEAD:278-279`; the `ComboRow` migration
  merely reproduces it.
- **Expected**: Separate the borrow from `set_selected`:
  `let cur = engine.borrow().port_name().map(|s| s.to_string());` drop before
  `select_port(&port_row, &ports, cur.as_deref())`; or split `populate_ports`
  splicing and selection into two phases.
- **Rationale**: `RefCell` panic is high severity, though it triggers only when
  `current` is `Some` and differs from the current `selected`. Synchronous
  emission makes it non-deterministic and it has been reported by sanity checks;
  P1; deferred to subsequent research.

### C5. Refresh Ports Always Resets to `0` (Found in Review, Already in HEAD) — `src/application.rs:323-340`

- **Context**: The `port-settings` dialog preserves the current port via
  `select_port` on open; the `btn_port_refresh` callback at
  `src/application.rs:325` directly calls `port_row.set_selected(0)`.
- **Location**: `src/application.rs:330` forces a switch to the first device and
  calls `open_port` on every refresh, ignoring `engine.port_name()` and
  contradicting the preservation semantics on dialog open; same at `HEAD:293`.
- **Expected**: After refresh, also do
  `let cur = engine.borrow().port_name()...; select_port(&port_row, &ports, cur)`;
  if a reset is intentional, annotate explicitly as "refresh means reset to
  first port".
- **Rationale**: Medium severity; the user-selected port is silently switched on
  refresh; P2; deferred to subsequent research.

### C6. `MidiDensityView` `vexpand` Ownership Confusion (Found in Review, Partially Introduced Here) — `src/midi_view.rs:113` / `src/application.rs:150-156` / `ui/window.blp:142,147`

- **Context**: `MidiDensityView::new:113` sets `set_vexpand(true)`; after
  splitting `view_root` into parent
  `Box page/density_placeholder[vexpand true/false]` at `ui/window.blp:142,147`
  and changing to `placeholder.append(child)` at `src/application.rs:150`, the
  outer placeholder's `vexpand` already decides whether to consume remaining
  space.
- **Location**: `MidiDensityView` itself still has `vexpand true`, so whether it
  actually expands is truncated by the parent `Box` with `vexpand false`;
  redundant and misleading (`src/application.rs:150` already removed the
  `density_view.set_vexpand(false)` line).
- **Expected**: Single ownership of the decision: let the placeholder own
  `vexpand`; remove `set_vexpand(true)` in `MidiDensityView::new` or explicitly
  call `density_view.set_vexpand(false)` in `application.rs` with a comment
  "decided by placeholder".
- **Rationale**: P3 readability/layout-contract issue; to be tidied the next
  time `midi_view.rs` is touched; deferred to subsequent research.

______________________________________________________________________

## D. UI / Style / Docs

### D1. `BAR_WIDTH`/`GAP=2.0` Hardcoded — `src/midi_view.rs:11-13`

- **Context**: Logical pixels that scale with the scale factor; no longer a
  HiDPI issue.
- **Location**: No tuning comment.
- **Expected**: Keep the constants and add a "logical pixels" comment.
- **Rationale**: P3.

### D2. Dead CSS — `ui/style.css:11-16`

- **Context**: `.midi-icon` is not referenced by any `.blp`/`.rs`;
  `.midi-density-view`/`.page-turn-view` now both have a corresponding
  `add_css_class`.
- **Location**: `.midi-icon` is a dead rule.
- **Expected**: Remove it or annotate the intent to keep it.
- **Rationale**: P3.

### D3. Asymmetric Margins and Spacing — `ui/window.blp:123-130,184`

- **Context**: `view_root` has 24 px margins on all sides + `spacing:12`; the
  full-bleed visualization is inset; `controls_box` has an extra
  `margin-bottom:9` offset.
- **Location**: Edge-to-edge loss and asymmetry.
- **Expected**: Converge margins per visual spec or annotate the inset as
  intentional.
- **Rationale**: P3 pure visual tuning.

### D4. Short-Window Overflow Risk — `ui/window.blp:132-134`

- **Context**: `view_root` vertical stack
  `page(200)+density(96)+controls(~200)+48 margins` just fits the default height
  of 640; in an extremely short window, `Clamp(valign:end)` squeezes the canvas
  first.
- **Location**: No scrolling/compression strategy; needs verification on real
  hardware.
- **Expected**: Verify on real hardware; deferred to subsequent research pending
  runtime confirmation.
- **Rationale**: P3 layout reasoning, not yet confirmed at runtime.

______________________________________________________________________

## E. By-Design (Not Bugs — Recorded to Prevent Mistaken Fixes)

- **Backward page turn cuts directly**: `page_view.rs:328` only `page==last+1`
  has the kashiwade animation; all other cases (backward by one, forward by N)
  are handled as a seek — intentional, cuts directly.
- **Growth rate tied to length**: `page_view.rs:34-40,439` mass is clamped
  `0.25-4x` by visible length, preserving monotonicity — intentional, retained.
- **Seek while paused stays blank until playback**: `page_view.rs:360` gated by
  `if playing` with 16/frame throttling — intentional, stays blank with priority
  on convenience.
- **EOF stays at end**: `engine.rs:233-238` and `application.rs:634-639`
  intentionally do not `re-stop()`; the label and view remain at total duration
  / last page — intentional.
