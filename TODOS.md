# TODOS — deferred and open items

本文件收敛 `fa0bcce` 后所有已确认但未在当前提交中修复的条目。按你此前的决定：**畸形 MIDI 统一由未来的状态机解析库重写**、**暂停时
seek 保持空白直到播放**、**后退翻页直接切页**、**Stop 清空画布**、**生长速度保持长度挂钩**。`README.md`
刻意不含路线图；`AGENTS.md` 为权威约束，本文件为详细清单。

约定：每条给出完整上下文、错误点位置、期望行为与为何如此期望。优先级沿用此前复核后的定级（P2/P3 为主，延期项不按 P0
计以免误导）。`file:line` 基于 `fa0bcce` 快照，可能随后续编辑偏移一行左右。

______________________________________________________________________

## A. Engine — 延期至状态机重写（本次不修）

以下 `src/engine.rs` 小节均为同一解析器的不同切面，逐个打补丁会互相打架，统一延期。

### A1. `tempo=0` 导致 `calculate_measure_map` 假死 — `src/engine.rs:469-490`

- **上下文**：`calculate_measure_map` 循环按拍号推进 `current_tick`，但 `current_time` 由
  `seconds_for_tick(end_tick)` 决定；`end_time` 被 `.max(current_time)` 兜住。
- **错误点**：`src/engine.rs:476` 的 `.max` 掩盖非单调 tempo；当 `us_per_quarter==0`
  时时间永不推进，`current_time > total_length+5.0` 的跳出条件永不到，零长小节被无限追加。
- **期望**：状态机解析库中对 `us_per_quarter==0` 报错或钳为合法下界，不再用 `max` 掩盖。
- **为何**：触发需畸形文件，概率低但为 hang 级别，适合在一次重写中统一处理而非在此打补丁。

### A2. 幻影小节与空文件回退 — `src/engine.rs:488-500`

- **上下文**：`calculate_measure_map` 用 `total_length+5.0` 作为“多生一拍”的 fudge；空表时回退成
  `0..max(total,2.0)` 的假 4/4 小节。
- **错误点**：`src/engine.rs:488` 魔法数、`src/engine.rs:493-499` 空文件伪装成 2s 歌曲。
- **期望**：空文件与尾部口径由状态机统一定义（拒绝或显式“无小节”），不再靠魔法数凑数。
- **为何**：当前仅影响数据层展示与 density 比例，无播放正确性风险，延期最省事。

### A3. 拍号静默修复与 barline 漂移 — `src/engine.rs:465-468`

- **上下文**：`num.max(1)` 修复 `num==0`，`den=1<<den_pow.min(16)` 静默钳 corrupt
  `den_pow`，`ticks_per_measure=num*tpq*4/den` 整数截断。
- **错误点**：`src/engine.rs:466-468` 对 corrupt 拍号不报错且用整数除法致 barline 累积漂移。
- **期望**：状态机中对 corrupt 拍号报错或显式策略，并在 tick 域用有理数/分数保持 barline 精度。
- **为何**：真实曲库极少命中，修需引入分数运算，适合重写时一并处理。

### A4. 小节中途变拍错位 — `src/engine.rs:459-464`

- **上下文**：拍号只在小节边界切换（`while meter_idx+1 … current_tick >= …`）。
- **错误点**：落在小节内的变拍被拉长到下一小节才生效，下游强拍全偏。
- **期望**：在变拍 tick 处切分当前小节，而非等下一小节。
- **为何**：变拍文件少见但为真错，修需重构 measure 生成，延期到状态机最合适。

### A5. 同 `(channel,key)` 的 FIFO 配对错配 — `src/engine.rs:516-522`

- **上下文**：`note_intervals` 用 `Vec` + 线性 `position()` 找第一个 pending，悬空 `NoteOn` 延到
  `total_length`。
- **错误点**：legato/重击同一键时第二个 `NoteOn` 被 FIFO 误配，第二个悬空成 EOF 幽灵音；应为 LIFO 栈。
- **期望**：`HashMap<(ch,key), Vec<start>>` 栈式配对，后进先出。
- **为何**：影响视觉幽灵音与 density 计数，但为解析语义的一部分，适合状态机中统一。

### A6. 悬空与孤儿音符的尾长口径 — `src/engine.rs:425-426,534-544`

- **上下文**：悬空 `NoteOn` 一律延到 `total_length`，而 `total_length` 取“最后一个事件”无论类型；孤儿
  `NoteOff` 静默忽略；零长 ` (t-start).max(0.0)` 被保留。
- **错误点**：`src/engine.rs:425` 末尾若为 CC/SysEx 则超长拖尾，若末尾自身为 `NoteOn` 则 0
  时长；`src/engine.rs:516-530` 零长不可见但参与 density；`src/engine.rs:518` 孤儿丢弃隐藏截断文件问题。
- **期望**：状态机中明确尾长策略（按最后 NoteOff/文件尾静音裁量）、零长策略、孤儿告警。
- **为何**：三者互相关联，单点修会互斥。

### A7. 同 tick 排序与 SysEx 丢弃 — `src/engine.rs:386-423`

- **上下文**：`flatten` 收集 tempo/meter 与 raw 事件，`tempo_changes/meters` 仅按 tick
  排序，无去重；`SysEx` 重组假设分隔符已剥离，`Escape` 落入 `_ => {}`；`events.sort_by(total_cmp)`
  稳定保留 track 序。
- **错误点**：同 tick 先后未定义、`0xF7` continuation 丢弃、同时刻 `NoteOff/NoteOn` 可能颠倒。
- **期望**：状态机中按 `(tick, kind)` 二级排序去重，SysEx 按规范重组。
- **为何**：低概率数据问题，修需明确优先级表，适合重写。

### A8. 性能：线性扫 tempo 表与 `pending` 扫描 — `src/engine.rs:505-547`

- **上下文**：`seconds_for_tick` 每次线性扫 `tempo_map`（每事件+每小节各一次），`pending` 线性
  `position()`。
- **错误点**：`O(n·m)` 与 `O(pending)`， dense 复音下可观。
- **期望**：排序去重后用游标/二分与 `HashMap` 栈。
- **为何**：`load` 期一次性开销，典型文件 tempo 段很少，P3，延期无感。

______________________________________________________________________

## B. Page view — 渲染与动画

### B1. 零时长音符不可见（待你决策）— `src/page_view.rs:424-429`

- **上下文**：`rebuild_cache` 中 `clip_start>=clip_end` 直接 `continue`；同 tick 的
  NoteOn/NoteOff（鼓点、装饰音常见）被丢。
- **错误点**：`src/page_view.rs:428` 能发声的音符无像素。
- **期望**：候选 (a) 给最小 1px 宽的 tick，(b) 维持丢弃并在文档注明；需你拍板打击乐是否值得一个像素。
- **为何**：可闻不可见是信息丢失，但若零长全为脏数据则丢弃更对。

### B2. 小节线迟滞缺失 — `src/page_view.rs:502-508`

- **上下文**：`find_measure` 仅按 `start` 二分，恰落在小节线上的时间判给后一小节。
- **错误点**：seek 在小节线附近来回拨时页号抖动。
- **期望**：正常播放保持现状（单调推进下判给后一小节是对的）；若以后要修，加 ±几毫秒迟滞。
- **为何**：音乐语义上“线即新小节开始”是对的，P3。

### B3. 收缩值的双事实源 — `src/page_view.rs:330-333,448-468`

- **上下文**：翻页时手设 `shrink=0.0`，`play_spring` 内又 `reset()+set_value_from(0)`；seek
  分支 `pause()` 不 `reset()`。
- **错误点**：同一事实写两遍，内部值与手设 `1.0` 可能发散（当前无 `cached_prev` 时无害）。
- **期望**：以弹簧对象为唯一事实源，手设仅作首帧初始值并注释。
- **为何**：现在无害，但后人读 `seek` 后弹簧状态会踩坑，P3。

### B4. `SHRINK_INTERVAL_MS=0` 死分支 — `src/page_view.rs:17,349,448-457`

- **上下文**：`SHRINK_INTERVAL_MS==0` 时 `transition_wait` 等待分支永不可达。
- **错误点**：死代码与“hold 可见”注释矛盾。
- **期望**：保留常量作调参位，给死分支加 `SHRINK_INTERVAL_MS>0 时启用` 注明。
- **为何**：可调参数的支架代码，P3。

### B5. 跨页延音的双份弹簧 — `src/page_view.rs:413-442`

- **上下文**：横跨小节线的延音天然进入两页并各自裁剪，各自独立 `scale/started/mass_mult`。
- **错误点**：同一物理音符算两遍、长两遍；`mass_mult` 按裁剪后长度各算各的，不一致。
- **期望**：维持双页（防缝隙是对的），但质量按原长算或注明双份是故意的。
- **为何**：边界音符数量少，P3 洁癖项。

### B6. 横向 `measure()` 返回 `(0,0)` — `src/page_view.rs:163-168`（`midi_view.rs:43` 同）

- **上下文**：横向 natural 给 0，全靠代码里 `hexpand(true)` 撑着。
- **错误点**：去掉 `hexpand` 即塌为 0 宽，布局契约不完整。
- **期望**：横向给与 `CONTENT_HEIGHT` 对应的 sane natural。
- **为何**：当前有 `hexpand` 兜底，P3。

### B7. `NOTE_HEIGHT_RATIO/NOTE_AREA_SCALE==1.0` 为 no-op — `src/page_view.rs:28-30,178-180`

- **上下文**：`eff_h==h`、`top==0`，相邻音高仅靠 `eff_h/127` 间距与 `eff_h/128` 高的差值留极细缝。
- **错误点**：调参位名义上存在但当前无缝隙语义，读者以为可调出缝隙。
- **期望**：保留常量但注明“`1.0` 时仅靠间距差留缝”，或给默认 `<1`。
- **为何**：纯调参清晰度，P3。

### B8. 每帧扫描与生长弹簧开销 — `src/page_view.rs:360-500`

- **上下文**：`tick` 每帧扫两份缓存找待触发音符，翻页时全扫 `eng.notes()`；已加
  `MAX_GROWTH_SPAWNS_PER_TICK=16` 限流与 `imp.growth` 持有、16 通道 LUT 与 early-break。
- **错误点**：复杂度本身已可接受，剩余为同帧多 `queue_draw` 回调开销。
- **期望**：保持现状，必要时再对重建加索引/分箱。
- **为何**：几千音符文件已不顿，P2 已通过限流缓解，P3 余量。

### B9. `CachedNote::clone` 共享/拷贝混杂 — `src/page_view.rs:111-122,330`

- **上下文**：`Clone` 共享 `scale:Rc`、拷贝 `started:Cell`；旧页克隆进 `cached_prev` 后 stale
  弹簧只碰孤儿 cell。
- **错误点**：语义 subtle，正确但易被后人“优化”掉。
- **期望**：保留注释说明“共享是故意的”，或统一为 `Rc<Cell>`。
- **为何**：现在正确，P3。

### B10. `reset()` 漏清 `growth` 与 tick 强引用说明 — `src/page_view.rs:152-159,294-307`

- **上下文**：`reset()` 清缓存/页号/shrink/last_elapsed，未显式清 `growth`
  向量；`add_tick_callback` 强捕获靠 GTK 销毁自摘。
- **错误点**：`growth` 靠 `tick` 的 drain 自愈，强引用说明虽已注释但仍为单窗口假设。
- **期望**：`reset()` 中 `growth.borrow_mut().clear()` 并在注释中固化单窗口假设。
- **为何**：P3 卫生项。

______________________________________________________________________

## C. Application — 接线与语义

### C1. Seek 丢失音色（CC/Program 快照）— `src/engine.rs:193, application.rs:480-512`

- **上下文**：`seek` 用 `partition_point(t < pos)` 重定 `next_idx`，恰在 `pos` 的事件重放，但此前
  CC/Program 不重放。
- **错误点**：往回跳后音符用 stale 音色，靠 `all_notes_off` 掩盖不断音。
- **期望**：状态机中在 seek 时回放此前最后的 CC/Program 快照，或文档注明不支持。
- **为何**：朴素播放器经典债，修需状态快照，延期到状态机更合适，P3。

### C2. `view_root` 插入顺序靠约定 — `src/application.rs:141-149, ui/window.blp:123` — **已在本轮修复**

- **上下文**：此前 `prepend(page)` + `insert_child_after(density, page)` 得
  `[page, density, Clamp]`，`view_root` 在 `.blp` 中无占位，`vexpand` 仅代码设。
- **错误点**：Blueprint 编辑器不可见，布局依赖代码约定。
- **本次修复**：`ui/window.blp:123-153` 新增
  `Label label_name[xalign 0.0 ellipsize end title-1 顶置]` +
  `Box page/density_placeholder[vexpand true/false]`
  作为父容器；`src/application.rs:150-156` 改为 `placeholder.append(child)`，顺序即声明式
  `[label, page_ph→page, density_ph→density, Clamp]`，不再 `remove/prepend`。`TODOS`
  保留以记录决策，`AGENTS.md` 已同步。
- **为何**：P3 可维护性，已闭环。

### C3. `GFile.path().unwrap_or_default()` 掩盖真因 — `src/application.rs:206-210,386-390`

### C4. Port 选择触发 `RefCell` panic（本次 review 发现，`HEAD` 已潜伏）— `src/application.rs:288-304,563-576` / `src/application.rs:46-58` / `src/application.rs:310-322`

- **上下文**：`port_row`/`port_dropdown` 的 `selected` 属性通过 `StringList`
  绑定；`populate_ports()` 中 `splice` 或 `select_port()` 中 `set_selected()` 会同步发射
  `selected_notify`，而调用点 `port_action` 与启动时 `Refresh ports` 块在 `engine.borrow()`
  的不可变借用仍存活期间调用 `select_port`。
- **错误点**：`src/application.rs:290-292` / `src/application.rs:564` 的
  `let current = engine.borrow(); select_port(&port_row, &ports, current.port_name());`
  中 `current: Ref` 存活进入 `select_port:46` 的 `row.set_selected()`，同步回调
  `port_row.connect_selected_notify:310` 内 `engine.borrow_mut().open_port()` 导致
  `already borrowed` panic。`HEAD:278-279` 对 `DropDown` 已同形，本次 `ComboRow`
  仅复现，未放大。
- **期望**：借用与 `set_selected`
  分离：`let cur = engine.borrow().port_name().map(|s| s.to_string()); drop` 后再
  `select_port(&port_row, &ports, cur.as_deref())`；或将 `populate_ports` 的
  `splice` 与选择分离为两阶段。
- **为何**：`RefCell` panic 为高严重，虽需特定 `current Some` 且与当前 `selected` 不同才触发，但
  `selected` 同步发射决定不可控，`sanity check` 已报，需在下次触及 `application.rs` 时单行修复，优先级
  P1，暂列延期以免阻塞本次视觉变更。

### C5. 刷新端口总是重置为 `0`（本次 review 发现，`HEAD` 已有）— `src/application.rs:323-340`

- **上下文**：`port-settings` dialog 打开时通过 `select_port` 保留 `current`
  端口；`btn_port_refresh` 回调 `src/application.rs:325` 直接
  `port_row.set_selected(0)`。
- **错误点**：`src/application.rs:330` 每次刷新后无视 `engine.port_name()` 已选，强制切到第一设备并
  `open_port`，与打开对话框的保留语义不一致；`HEAD:293` 同样为 `set_selected(0)`。
- **期望**：刷新后同样
  `let cur = engine.borrow().port_name()...; select_port(&port_row, &ports, cur)`；若刻意重置则显式注释“refresh
  意为重置为第一端口”。
- **为何**：中严重，用户已选端口在刷新后被静默切换，P2，延期。

### C6. `MidiDensityView` 的 `vexpand` 归属混乱（本次 review 发现，部分本次引入）— `src/midi_view.rs:113` / `src/application.rs:150-156` / `ui/window.blp:142,147`

- **上下文**：`MidiDensityView::new:113` 内 `set_vexpand(true)`；本次将 `view_root`
  占位拆为父容器 `Box page/density_placeholder[vexpand true/false]`
  `ui/window.blp:142,147` 后，`src/application.rs:150` 改为
  `placeholder.append(child)`，外层 `placeholder` 的 `vexpand` 已决定是否吃剩余空间。
- **错误点**：`MidiDensityView` 自身仍 `vexpand true`，实际是否伸展取决于父 `Box` 的
  `vexpand false` 截断，冗余且易误导后人（`src/application.rs:150` 已删
  `density_view.set_vexpand(false)` 行）。
- **期望**：策略归一：让 `placeholder` 拥有 `vexpand` 决策，`MidiDensityView::new` 去掉
  `set_vexpand(true)` 或在 `application.rs` 显式 `density_view.set_vexpand(false)`
  并注释“由 placeholder 决定”。
- **为何**：P3，可读性/布局契约问题，需下次动 `midi_view.rs` 时顺手理清。

### C3. `GFile.path().unwrap_or_default()` 掩盖真因 — `src/application.rs:206-210,386-390`

- **上下文**：非本地文件被压成 `""`，`load_file:67` 虽已对空路径报 `error-view`，但调用点仍用
  `unwrap_or_default()`。
- **错误点**：错误信息丢失具体原因。
- **期望**：调用点改为 `if let Some(path)=file.path()` 否则直接报非本地原因。
- **为何**：一行 guard，P3。

______________________________________________________________________

## D. UI / Style / Docs

### D1. `BAR_WIDTH/GAP=2.0` 硬编码 — `src/midi_view.rs:11-13`

- **上下文**：逻辑像素，随 scale factor 缩放，已非 HiDPI 问题。
- **错误点**：无调参注释。
- **期望**：保留常量，加“逻辑像素”注释。
- **为何**：P3。

### D2. 死 CSS — `ui/style.css:11-16`

- **上下文**：`.midi-icon` 无任何 `.blp/.rs` 引用，`.midi-density-view`/`.page-turn-view`
  现均有对应 `add_css_class`。
- **错误点**：`.midi-icon` 为死规则。
- **期望**：删除或注明保留意图。
- **为何**：P3。

### D3. 边距与间距不对称 — `ui/window.blp:123-130,184`

- **上下文**：`view_root` 24px 全边距 +
  `spacing:12`，全幅可视化被内缩；`controls_box margin-bottom:9` 额外偏移。
- **错误点**：边缘到边缘的损失，不对称。
- **期望**：按视觉稿收敛边距，或注明有意内缩。
- **为何**：P3 纯视觉调参。

### D4. 矮窗溢出风险 — `ui/window.blp:132-134`

- **上下文**：`view_root` 纵向 `page(200)+density(96)+controls(~200)+48 边距` 在默认高 640
  下刚好，极矮窗口下 `Clamp(valign:end)` 先挤画布。
- **错误点**：无滚动/压缩策略，真机需验证。
- **期望**：真机验证，不验证不改。
- **为何**：P3 布局推理，未经运行时确认。

______________________________________________________________________

## E. By-design（非 bug，记录以免误修）

- **后退翻页直接切页**：`page_view.rs:328` 仅 `page==last+1` 有 kashiwade 动画，其余（含后退一页、前进 N
  页）按 seek 处理 — 你已确认“直接切页”。
- **生长速度长度挂钩**：`page_view.rs:34-40,439` 质量按可见长度钳 `0.25-4x`，保留单调性 — 你已确认保留。
- **暂停 seek 空白直到播放**：`page_view.rs:360` `if playing` 门控与 16/frame 限流 —
  你已确认“保持空白、方便优先”。
- **EOF 停在结尾**：`engine.rs:233-238` 与 `application.rs:634-639` 刻意不
  `re-stop()`，标签与画面停在总时长/最后一页 — 你已确认。
