---
title: "GPUI 터미널 구현체 소스코드 분석"
description: "Source code analysis of gpui-ghostty, Zed terminal, gpui-terminal — Element/View patterns, rendering strategies, PTY integration"
date: 2026-02-11
phase: [1]
topics: [gpui, gpui-ghostty, zed-terminal, source-analysis, element, view]
status: final
related:
  - framework.md
  - bootstrap.md
  - ../core/terminal-architecture.md
  - ../core/terminal-emulation.md
---

# GPUI 기반 터미널 구현체 소스코드 분석 보고서

> 작성일: 2026-02-11
> 분석 대상: gpui-ghostty, Zed terminal, gpui-terminal

---

## 1. gpui-ghostty (Xuanwo/gpui-ghostty)

**GitHub**: https://github.com/Xuanwo/gpui-ghostty
**라이선스**: Apache 2.0

### 1.1 프로젝트 개요

Ghostty의 VT 파서를 GPUI 렌더러와 결합한 임베더블 터미널 라이브러리. Ghostty VT 코어를 Zig로 빌드한 뒤 C ABI를 통해 Rust에서 사용하는 구조.

**핵심 버전 고정**:
- Ghostty: v1.2.3 (vendor/ghostty 서브모듈)
- Zig: 0.14.1 (Ghostty 빌드에 필요)
- GPUI: Zed 커밋 `6016d0b8c6a22e586158d3b6f810b3cebb136118`에 고정

### 1.2 크레이트 구조

```
crates/
├── ghostty_vt_sys/          # Zig 빌드 + C ABI 바인딩
│   ├── build.rs             # Zig 빌드 시스템 호출
│   ├── include/ghostty_vt.h # C 헤더 파일
│   ├── zig/build.zig        # Zig 빌드 스크립트
│   ├── zig/lib.zig          # Zig-side 래퍼 (Ghostty → C ABI)
│   └── src/lib.rs           # extern "C" fn 선언
│
├── ghostty_vt/              # Safe Rust 래퍼
│   └── src/lib.rs           # Terminal, Rgb, StyleRun, CellStyle 등
│
└── gpui_ghostty_terminal/   # GPUI 뷰 통합
    └── src/
        ├── lib.rs           # 공개 API: TerminalConfig, TerminalSession
        ├── config.rs        # TerminalConfig (cols, rows, fg, bg)
        ├── font.rs          # 기본 폰트 설정
        ├── session.rs       # TerminalSession (VT 상태 + 프로토콜 파싱)
        ├── view/mod.rs      # TerminalView (GPUI Render) + TerminalTextElement
        └── tests.rs
```

### 1.3 아키텍처 레이어

```
┌──────────────────────────────────────┐
│        TerminalView (GPUI Render)    │  ← GPUI div() + Element
├──────────────────────────────────────┤
│        TerminalSession               │  ← VT 상태 + 프로토콜 스캐닝
├──────────────────────────────────────┤
│        ghostty_vt (Safe Rust)        │  ← Terminal, StyleRun, Rgb
├──────────────────────────────────────┤
│        ghostty_vt_sys (C ABI)        │  ← extern "C" fn
├──────────────────────────────────────┤
│     libghostty-vt (Zig/Ghostty)      │  ← VT 파서 + 그리드 관리
└──────────────────────────────────────┘
```

### 1.4 핵심 데이터 구조

#### Terminal (ghostty_vt)
```rust
// FFI opaque 포인터 래퍼
pub struct Terminal {
    ptr: NonNull<c_void>,
}

// 주요 메서드:
// - new(cols, rows) → Result<Self, Error>
// - feed(bytes) → Result<(), Error>       // VT 파서에 바이트 입력
// - resize(cols, rows) → Result<(), Error>
// - dump_viewport() → Result<String, Error>  // 뷰포트 텍스트 덤프
// - dump_viewport_row(row) → Result<String, Error>
// - dump_viewport_row_style_runs(row) → Result<Vec<StyleRun>, Error>
// - cursor_position() → Option<(u16, u16)>
// - scroll_viewport(delta) → Result<(), Error>
// - take_dirty_viewport_rows(rows) → Vec<u16>  // 변경된 행만 반환
// - take_viewport_scroll_delta() → i32  // 스크롤 오프셋 델타
```

#### StyleRun
```rust
pub struct StyleRun {
    pub start_col: u16,
    pub end_col: u16,
    pub fg: Rgb,
    pub bg: Rgb,
    pub flags: u8,  // bold(0x02), italic(0x04), underline(0x08), faint(0x10), strikethrough(0x40)
}
```

#### TerminalConfig
```rust
pub struct TerminalConfig {
    pub cols: u16,           // 기본값: 80
    pub rows: u16,           // 기본값: 24
    pub default_fg: Rgb,     // 기본값: 흰색
    pub default_bg: Rgb,     // 기본값: 검정
    pub update_window_title: bool,
}
```

#### TerminalSession
```rust
pub struct TerminalSession {
    config: TerminalConfig,
    terminal: Terminal,              // ghostty_vt::Terminal
    bracketed_paste_enabled: bool,   // CSI ?2004h/l 추적
    mouse_x10_enabled: bool,         // CSI ?1000h/l
    mouse_button_event_enabled: bool,// CSI ?1002h/l
    mouse_any_event_enabled: bool,   // CSI ?1003h/l
    mouse_sgr_enabled: bool,         // CSI ?1006h/l
    title: Option<String>,           // OSC 0/2 제목
    clipboard_write: Option<String>, // OSC 52 클립보드
    parse_tail: Vec<u8>,             // 출력 바이트 버퍼 (모드 스캔용)
    dsr_state: DsrScanState,        // DSR(Device Status Report) 상태 머신
    osc_query_state: OscQueryScanState, // OSC 컬러 쿼리 상태 머신
}
```

**핵심 설계 특이점**: `TerminalSession`은 Ghostty VT가 처리하지 않는 프로토콜 기능을 직접 파싱한다:
- **모드 추적**: CSI ?h/l 시퀀스를 `parse_tail` 버퍼에서 수동 스캔 (bracketed paste, 마우스 모드)
- **DSR 응답**: `\x1b[5n` (Device Status), `\x1b[6n` (Cursor Position) 요청 감지 → 응답 생성
- **OSC 컬러 쿼리**: `\x1b]10;?\x07` (전경색), `\x1b]11;?\x07` (배경색) 요청 감지 → 응답 생성
- **OSC 52 클립보드**: 클립보드 쓰기 시퀀스 파싱 + base64 디코딩
- **OSC 제목**: OSC 0/2 시퀀스에서 윈도우 제목 추출

### 1.5 TerminalView 렌더링 패턴

#### Render 트레이트 구현 (`view/mod.rs`)
```rust
pub struct TerminalView {
    session: TerminalSession,
    viewport_lines: Vec<String>,              // 뷰포트 행별 텍스트
    viewport_line_offsets: Vec<usize>,         // 행별 바이트 오프셋
    viewport_total_len: usize,
    viewport_style_runs: Vec<Vec<StyleRun>>,   // 행별 스타일 런
    line_layouts: Vec<Option<gpui::ShapedLine>>, // 셰이핑된 텍스트 캐시
    line_layout_key: Option<(Pixels, Pixels)>,   // (font_size, line_height) 캐시 키
    last_bounds: Option<Bounds<Pixels>>,
    focus_handle: FocusHandle,
    input: Option<TerminalInput>,              // PTY 입력 전송 함수
    pending_output: Vec<u8>,                   // 대기 중인 PTY 출력
    pending_refresh: bool,
    selection: Option<ByteSelection>,          // 텍스트 선택
    marked_text: Option<SharedString>,         // IME 조합 텍스트
    marked_selected_range_utf16: Range<usize>, // IME 선택 범위
    font: gpui::Font,
}
```

#### 이중 렌더링 패턴: View + Element

`Render::render()` 에서 `div()` 기반 뷰를 반환하되, 실제 텍스트 그리기는 커스텀 `TerminalTextElement`에 위임:

```rust
impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 1. pending_output 처리 → VT 파서 피딩
        // 2. pending_refresh면 뷰포트 새로고침
        // 3. OSC 제목 업데이트

        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_paste))
            .on_key_down(cx.listener(Self::on_key_down))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            // ... 기타 마우스 이벤트
            .child(TerminalTextElement { view: cx.entity() })
    }
}
```

#### TerminalTextElement (커스텀 Element)

```rust
struct TerminalTextElement {
    view: gpui::Entity<TerminalView>,
}

impl Element for TerminalTextElement {
    type RequestLayoutState = ();
    type PrepaintState = TerminalPrepaintState;

    fn request_layout(...) -> (LayoutId, ()) {
        // relative(1.) 크기 요청 (부모 채움)
    }

    fn prepaint(...) -> TerminalPrepaintState {
        // 1. 폰트 메트릭스 계산 (cell_width, line_height)
        // 2. 행별 ShapedLine 생성 (text + TextRun 배열)
        //    - StyleRun → TextRunKey(fg, flags) → TextRun 변환
        //    - 캐시 히트 검사: 텍스트 동일하면 재사용
        // 3. 배경색 Quad 수집 (기본 배경과 다른 셀만)
        // 4. 선택 영역 Quad 수집
        // 5. 박스 드로잉 문자 Quad 수집 (별도 렌더링)
        // 6. IME 마크드 텍스트 ShapedLine + 배경 생성
        // 7. 커서 Quad 생성 (포커스 시에만)
    }

    fn paint(...) {
        // paint_layer() 사용하여 레이어별 페인팅:
        // 1. 기본 배경 Quad
        // 2. 셀 배경 Quad들
        // 3. 선택 영역 Quad들
        // 4. 텍스트 행 (ShapedLine.paint())
        // 5. 박스 드로잉 Quad들
        // 6. IME 마크드 텍스트 배경 + 텍스트
        // 7. 커서 Quad
    }
}
```

#### TerminalPrepaintState
```rust
struct TerminalPrepaintState {
    line_height: Pixels,
    shaped_lines: Vec<gpui::ShapedLine>,  // 행별 셰이핑된 텍스트
    background_quads: Vec<PaintQuad>,     // 배경색 사각형
    selection_quads: Vec<PaintQuad>,      // 선택 영역 하이라이트
    box_drawing_quads: Vec<PaintQuad>,    // 박스 드로잉 문자 사각형
    marked_text: Option<(gpui::ShapedLine, gpui::Point<Pixels>)>,  // IME
    marked_text_background: Option<PaintQuad>,
    cursor: Option<PaintQuad>,
}
```

### 1.6 텍스트 렌더링 최적화

**StyleRun → TextRun 배칭**:
```rust
// 행별로 StyleRun 배열을 순회하면서 TextRun 배열 생성
// 1. StyleRun의 start_col/end_col → byte_index 변환 (CJK 와이드 문자 고려)
// 2. 인접한 스타일 사이의 갭을 기본 스타일 TextRun으로 채움
// 3. 각 StyleRun을 TextRunKey(fg, flags)로 변환하여 TextRun 생성

let key = TextRunKey { fg: style.fg, flags: style.flags & RELEVANT_FLAGS };
runs.push(text_run_for_key(&run_font, key, byte_len));
```

**ShapedLine 캐시**: `line_layout_key`가 `(font_size, line_height)`와 같고, 텍스트 내용이 동일하면 이전 ShapedLine 재사용.

**force_width (고정폭 렌더링)**:
```rust
// 와이드 문자가 없는 행에 대해 cell_width 강제 적용
let force_width = cell_width.and_then(|cell_width| {
    let has_wide = text.chars().any(|ch| ch.width().unwrap_or(0) > 1);
    (!has_wide).then_some(cell_width)
});
let shaped = window.text_system().shape_line(text, font_size, &runs, force_width);
```

### 1.7 셀 메트릭스 계산
```rust
fn cell_metrics(window: &mut Window, font: &gpui::Font) -> Option<(f32, f32)> {
    // "M" 문자로 셀 너비 측정
    let lines = window.text_system().shape_text(
        SharedString::from("M"), font_size, &[run], None, Some(1)
    ).ok()?;
    let cell_width = f32::from(lines.first()?.width()).max(1.0);
    let cell_height = f32::from(line_height).max(1.0);
    Some((cell_width, cell_height))
}
```

### 1.8 Dirty Row 최적화

Ghostty VT는 **변경된 행만 추적**하는 기능 제공:
```rust
fn reconcile_dirty_viewport_after_output(&mut self) {
    // 1. 스크롤 델타 적용 → 행 배열 rotate
    let delta = self.session.take_viewport_scroll_delta();
    self.apply_viewport_scroll_delta(delta);

    // 2. dirty rows만 갱신
    let dirty = self.session.take_dirty_viewport_rows();
    if !dirty.is_empty() && !self.apply_dirty_viewport_rows(&dirty) {
        self.pending_refresh = true;  // 실패 시 전체 새로고침
    }
}
```

스크롤 시 `rotate_left/right`로 기존 행 재활용:
```rust
fn apply_viewport_scroll_delta(&mut self, delta: i32) {
    if delta > 0 {
        self.viewport_lines.rotate_left(delta_abs);
        self.viewport_style_runs.rotate_left(delta_abs);
        self.line_layouts.rotate_left(delta_abs);
        // 새로 노출된 행만 갱신
        let dirty_rows = (rows - delta_abs..rows).map(|row| row as u16).collect();
        self.apply_dirty_viewport_rows(&dirty_rows);
    }
}
```

### 1.9 IME 통합 패턴

`EntityInputHandler` 트레이트 구현으로 macOS IME 지원:

```rust
impl EntityInputHandler for TerminalView {
    fn text_for_range(...) -> Option<String> {
        // marked_text에서 UTF-16 범위 → UTF-8 변환하여 반환
    }

    fn selected_text_range(...) -> Option<UTF16Selection> {
        // marked_selected_range_utf16 반환
    }

    fn marked_text_range(...) -> Option<Range<usize>> {
        // marked_text가 있으면 0..len 반환
    }

    fn unmark_text(...) {
        self.clear_marked_text(cx);
    }

    fn replace_text_in_range(... text: &str ...) {
        // marked text 클리어 + 확정 텍스트를 PTY에 전송
        self.clear_marked_text(cx);
        self.commit_text(text, cx);
    }

    fn replace_and_mark_text_in_range(... new_text: &str ...) {
        // 조합 중 텍스트 설정 (PTY에 전송하지 않음!)
        self.set_marked_text(new_text.to_string(), new_selected_range, cx);
    }

    fn bounds_for_range(...) -> Option<Bounds<Pixels>> {
        // 커서 위치 기반 IME 후보 창 위치 계산
        let (col, row) = self.session.cursor_position()?;
        let base_x = element_bounds.left() + px(cell_width * (col - 1) as f32);
        let base_y = element_bounds.top() + px(cell_height * (row - 1) as f32);
        // ...
    }
}
```

**IME 키 이벤트 필터링**:
```rust
fn should_skip_key_down_for_ime(has_input: bool, keystroke: &gpui::Keystroke) -> bool {
    if !has_input || !keystroke.is_ime_in_progress() {
        return false;
    }
    // IME 조합 중에는 Enter만 통과시킴
    !matches!(keystroke.key.as_str(), "enter" | "return" | "kp_enter" | "numpad_enter")
}
```

### 1.10 PTY 통합 예제 패턴 (`examples/pty_terminal/`)

```rust
// 핵심 I/O 루프 패턴:
// 1. portable-pty로 PTY 생성
// 2. 백그라운드 스레드에서 PTY stdout → mpsc 채널
// 3. 백그라운드 스레드에서 mpsc 채널 → PTY stdin
// 4. GPUI async task에서 16ms 주기로 채널 배치 읽기

window.spawn(cx, async move |cx| {
    loop {
        cx.background_executor().timer(Duration::from_millis(16)).await;
        let mut batch = Vec::new();
        while let Ok(chunk) = stdout_rx.try_recv() {
            batch.extend_from_slice(&chunk);
        }
        if !batch.is_empty() {
            cx.update(|_, cx| {
                view.update(cx, |this, cx| {
                    this.queue_output_bytes(&batch, cx);
                });
            }).ok();
        }
    }
}).detach();
```

**리사이즈 처리**: `observe_window_bounds`로 윈도우 크기 변경 감지 → 셀 메트릭스 재계산 → PTY + TerminalView 동시 리사이즈

---

## 2. Zed 에디터 내장 터미널

**GitHub**: https://github.com/zed-industries/zed
**경로**: `crates/terminal/`, `crates/terminal_view/`

### 2.1 크레이트 분리 패턴

```
crates/terminal/           # 터미널 엔티티 (VT + PTY + 이벤트)
├── src/
│   ├── terminal.rs        # Terminal 구조체 (핵심)
│   ├── terminal_hyperlinks.rs  # 하이퍼링크 감지
│   ├── terminal_settings.rs    # 설정 스키마
│   ├── pty_info.rs        # PTY 프로세스 정보
│   └── mappings/
│       ├── colors.rs      # Alacritty ↔ GPUI 색상 변환
│       ├── keys.rs        # 키 → 이스케이프 시퀀스
│       └── mouse.rs       # 마우스 이벤트 → 그리드 좌표

crates/terminal_view/      # 렌더링 + UI
├── src/
│   ├── terminal_view.rs   # TerminalView (GPUI 뷰)
│   ├── terminal_element.rs # TerminalElement (GPUI Element)
│   ├── terminal_panel.rs  # 패널 통합
│   ├── terminal_scrollbar.rs
│   ├── terminal_path_like_target.rs
│   ├── terminal_slash_command.rs
│   └── persistence.rs     # 세션 저장/복원
```

### 2.2 Terminal Entity (핵심 구조체)

```rust
pub struct Terminal {
    terminal_type: TerminalType,               // Pty | DisplayOnly
    term: Arc<FairMutex<Term<ZedListener>>>,   // alacritty_terminal::Term
    term_config: Config,
    events: VecDeque<InternalEvent>,           // 내부 이벤트 큐
    last_content: TerminalContent,             // 렌더 스냅샷 (!!!)
    last_mouse: Option<(AlacPoint, AlacDirection)>,
    matches: Vec<RangeInclusive<AlacPoint>>,
    selection_head: Option<AlacPoint>,
    breadcrumb_text: String,
    scroll_px: Pixels,
    next_link_id: usize,
    selection_phase: SelectionPhase,
    hyperlink_regex_searches: RegexSearches,
    task: Option<TaskState>,
    vi_mode_enabled: bool,
    child_exited: Option<ExitStatus>,
    event_loop_task: Task<Result<(), anyhow::Error>>,
    // ...
}

enum TerminalType {
    Pty { pty_tx: Notifier, info: Arc<PtyProcessInfo> },
    DisplayOnly,
}
```

### 2.3 이벤트 배칭 패턴 (100 이벤트 / 4ms 윈도우)

Zed의 핵심 최적화: Alacritty 이벤트 루프에서 오는 이벤트를 배칭하여 처리.

```rust
pub fn subscribe(mut self, cx: &Context<Terminal>) -> Terminal {
    self.terminal.event_loop_task = cx.spawn(async move |terminal, cx| {
        while let Some(event) = self.events_rx.next().await {
            // 첫 이벤트는 즉시 처리 (레이턴시 최소화)
            terminal.update(cx, |terminal, cx| {
                terminal.process_event(event, cx);
            })?;

            'outer: loop {
                let mut events = Vec::new();
                let mut timer = cx.background_executor()
                    .timer(Duration::from_millis(4))  // 4ms 배칭 윈도우
                    .fuse();
                let mut wakeup = false;

                loop {
                    futures::select_biased! {
                        _ = timer => break,           // 4ms 타임아웃
                        event = self.events_rx.next() => {
                            if let Some(event) = event {
                                if matches!(event, AlacTermEvent::Wakeup) {
                                    wakeup = true;    // Wakeup은 중복 제거
                                } else {
                                    events.push(event);
                                }
                                if events.len() > 100 { break; }  // 100개 제한
                            }
                        },
                    }
                }

                // 배치된 이벤트 한꺼번에 처리
                terminal.update(cx, |this, cx| {
                    if wakeup { this.process_event(AlacTermEvent::Wakeup, cx); }
                    for event in events {
                        this.process_event(event, cx);
                    }
                })?;
            }
        }
    });
    self.terminal
}
```

### 2.4 TerminalContent 스냅샷 패턴

렌더 스레드와 터미널 상태 사이의 **데이터 분리** 핵심 패턴:

```rust
pub struct TerminalContent {
    pub cells: Vec<IndexedCell>,           // 모든 보이는 셀 복사본
    pub mode: TermMode,
    pub display_offset: usize,
    pub selection_text: Option<String>,
    pub selection: Option<SelectionRange>,
    pub cursor: RenderableCursor,
    pub cursor_char: char,
    pub terminal_bounds: TerminalBounds,
    pub last_hovered_word: Option<HoveredWord>,
    pub scrolled_to_top: bool,
    pub scrolled_to_bottom: bool,
}

// sync()가 호출될 때마다 Term을 잠그고 스냅샷 생성
pub fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let term = self.term.clone();
    let mut terminal = term.lock_unfair();  // FairMutex 잠금

    // 내부 이벤트 큐 처리
    while let Some(e) = self.events.pop_front() {
        self.process_terminal_event(&e, &mut terminal, window, cx);
    }

    // 스냅샷 생성 (Term 잠금 보유 중)
    self.last_content = Self::make_content(&terminal, &self.last_content);
}

fn make_content(term: &Term<ZedListener>, last_content: &TerminalContent) -> TerminalContent {
    let content = term.renderable_content();

    let mut cells = Vec::with_capacity(content.display_iter.size_hint().0);
    cells.extend(content.display_iter.map(|ic| IndexedCell {
        point: ic.point,
        cell: ic.cell.clone(),
    }));

    TerminalContent {
        cells,
        mode: content.mode,
        display_offset: content.display_offset,
        selection: content.selection,
        cursor: content.cursor,
        cursor_char: term.grid()[content.cursor.point].c,
        terminal_bounds: last_content.terminal_bounds,  // 이전 bounds 유지
        // ...
    }
}
```

**핵심 통찰**: `last_content`는 렌더링에 필요한 모든 데이터를 **소유**하므로, Element가 그리는 동안 Term을 잠글 필요가 없다.

### 2.5 BatchedTextRun 렌더링 최적화

```rust
pub struct BatchedTextRun {
    pub start_point: AlacPoint<i32, i32>,
    pub text: String,
    pub cell_count: usize,
    pub style: TextRun,
    pub font_size: AbsoluteLength,
}

impl BatchedTextRun {
    // 동일 스타일 셀은 하나의 TextRun으로 합침
    fn can_append(&self, other_style: &TextRun) -> bool {
        self.style.font == other_style.font
            && self.style.color == other_style.color
            && self.style.background_color == other_style.background_color
            && self.style.underline == other_style.underline
            && self.style.strikethrough == other_style.strikethrough
    }

    pub fn paint(&self, origin: Point<Pixels>, dimensions: &TerminalBounds,
                 window: &mut Window, cx: &mut App) {
        let pos = Point::new(
            origin.x + self.start_point.column as f32 * dimensions.cell_width,
            origin.y + self.start_point.line as f32 * dimensions.line_height,
        );
        // shape_line으로 텍스트 셰이핑 후 페인팅
        window.text_system()
            .shape_line(self.text.clone().into(), font_size, &[self.style], Some(cell_width))
            .paint(pos, line_height, TextAlign::Left, None, window, cx);
    }
}
```

### 2.6 TerminalView 구조

```rust
pub struct TerminalView {
    terminal: Entity<Terminal>,          // Terminal 엔티티 참조
    workspace: WeakEntity<Workspace>,
    focus_handle: FocusHandle,
    has_bell: bool,
    cursor_shape: CursorShape,
    blink_manager: Entity<BlinkManager>,
    mode: TerminalMode,                  // Standalone | Embedded
    ime_state: Option<ImeState>,
    scroll_top: Pixels,
    scroll_handle: TerminalScrollHandle,
    // ...
}

struct ImeState {
    marked_text: String,
}
```

### 2.7 TerminalElement (GPUI Element)

```rust
pub struct TerminalElement {
    terminal: Entity<Terminal>,
    terminal_view: Entity<TerminalView>,
    workspace: WeakEntity<Workspace>,
    focus: FocusHandle,
    focused: bool,
    cursor_visible: bool,
    interactivity: Interactivity,
    mode: TerminalMode,
    block_below_cursor: Option<Rc<BlockProperties>>,
}

// LayoutState = prepaint 결과
pub struct LayoutState {
    hitbox: Hitbox,
    batched_text_runs: Vec<BatchedTextRun>,  // 배칭된 텍스트
    rects: Vec<LayoutRect>,                   // 배경 사각형
    relative_highlighted_ranges: Vec<(RangeInclusive<AlacPoint>, Hsla)>,
    cursor: Option<CursorLayout>,
    ime_cursor_bounds: Option<Bounds<Pixels>>,
    background_color: Hsla,
    dimensions: TerminalBounds,
    // ...
}
```

### 2.8 Alacritty EventLoop 통합

Zed는 Alacritty의 `EventLoop`을 직접 사용:
```rust
let event_loop = EventLoop::new(
    term.clone(),
    ZedListener(events_tx),
    pty,
    pty_options.drain_on_exit,
    false,
).context("failed to create event loop")?;

let pty_tx = event_loop.channel();  // PTY 쓰기 채널
let _io_thread = event_loop.spawn(); // I/O 스레드 시작
```

**ZedListener**: Alacritty EventListener 트레이트 구현, 이벤트를 `UnboundedSender<AlacTermEvent>`로 전달:
```rust
pub struct ZedListener(pub UnboundedSender<AlacTermEvent>);

impl EventListener for ZedListener {
    fn send_event(&self, event: AlacTermEvent) {
        self.0.unbounded_send(event).ok();
    }
}
```

### 2.9 포커스 관리

```rust
impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// TerminalView에서:
// - focus_handle.focus(window, cx) 호출로 포커스 설정
// - focus_handle.is_focused(window) 체크로 커서 표시 결정
// - Terminal::focus_in() / focus_out()으로 포커스 이벤트 시퀀스 전송
```

---

## 3. gpui-terminal (zortax/gpui-terminal)

**GitHub**: https://github.com/zortax/gpui-terminal
**라이선스**: Apache 2.0 / MIT

### 3.1 프로젝트 개요

가장 단순한 GPUI 터미널 구현체. alacritty_terminal + 임의의 Read/Write 스트림(PTY 불문)을 조합한 재사용 가능 터미널 컴포넌트.

### 3.2 소스 파일 구조

```
src/
├── lib.rs            # 공개 API 재내보내기
├── terminal.rs       # TerminalState (Arc<Mutex<Term>> 래퍼)
├── view.rs           # TerminalView (GPUI Render + I/O)
├── render.rs         # TerminalRenderer (텍스트 배칭 + 페인팅)
├── input.rs          # keystroke_to_bytes 변환
├── event.rs          # GpuiEventProxy (Alacritty EventListener)
├── colors.rs         # ColorPalette (16+256+fg/bg/cursor)
├── mouse.rs          # 마우스 이벤트 처리
├── clipboard.rs      # 클립보드 통합
├── box_drawing.rs    # 박스 드로잉 문자 렌더링
└── main.rs           # 예제 (portable-pty 연동)
```

### 3.3 TerminalState

```rust
pub struct TerminalState {
    term: Arc<Mutex<Term<GpuiEventProxy>>>,  // parking_lot::Mutex
    parser: Processor,                        // VTE 파서 (외부에 보관)
    cols: usize,
    rows: usize,
}

impl TerminalState {
    pub fn new(cols, rows, event_proxy) -> Self {
        let term = Term::new(config, &dimensions, event_proxy);
        let parser = Processor::new();
        // ...
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) {
        let mut term = self.term.lock();
        self.parser.advance(&mut *term, bytes);  // VTE 파서 직접 구동
    }

    pub fn with_term<F, R>(&self, f: F) -> R { ... }     // 읽기 접근
    pub fn with_term_mut<F, R>(&self, f: F) -> R { ... }  // 쓰기 접근
    pub fn term_arc(&self) -> Arc<Mutex<Term<...>>> { ... } // Arc 공유
}
```

**Zed과의 차이점**:
- Zed: Alacritty의 `EventLoop` 사용 (Alacritty가 VTE 파싱)
- gpui-terminal: `Processor::advance()` 직접 호출 (VTE 파서 수동 구동)
- Zed: `FairMutex` 사용 (Alacritty crate 내장)
- gpui-terminal: `parking_lot::Mutex` 사용

### 3.4 Push-based I/O 아키텍처

```rust
// 1단계: 백그라운드 스레드 - PTY stdout → flume 채널
thread::spawn(move || {
    let mut buf = [0u8; 4096];
    loop {
        match stdout_reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => { bytes_tx.send(buf[..n].to_vec()).ok(); }
            Err(_) => break,
        }
    }
});

// 2단계: async 태스크 - flume 채널 → VT 파서 → cx.notify()
let reader_task = cx.spawn(async move |this, cx| {
    loop {
        match bytes_rx.recv_async().await {
            Ok(bytes) => {
                this.update(cx, |view, cx| {
                    view.state.process_bytes(&bytes);
                    cx.notify();  // GPUI에 다시 그리라고 통지
                });
            }
            Err(_) => break,  // EOF → 종료
        }
    }
});
```

### 3.5 TerminalView 구조

```rust
pub struct TerminalView {
    state: TerminalState,
    renderer: TerminalRenderer,
    focus_handle: FocusHandle,
    stdin_writer: Arc<parking_lot::Mutex<Box<dyn Write + Send>>>,
    event_rx: mpsc::Receiver<TerminalEvent>,
    config: TerminalConfig,
    _reader_task: Task<()>,
    resize_callback: Option<Arc<ResizeCallback>>,
    key_handler: Option<Arc<KeyHandler>>,
    bell_callback: Option<BellCallback>,
    title_callback: Option<TitleCallback>,
    clipboard_store_callback: Option<ClipboardStoreCallback>,
    exit_callback: Option<ExitCallback>,
}
```

**빌더 패턴 콜백 구성**:
```rust
let terminal = cx.new(|cx| {
    TerminalView::new(writer, reader, config, cx)
        .with_resize_callback(move |cols, rows| { /* PTY 리사이즈 */ })
        .with_exit_callback(|_, cx| cx.quit())
        .with_key_handler(|event| false)
        .with_bell_callback(|w, cx| { /* 벨 처리 */ })
        .with_title_callback(|w, cx, title| { /* 제목 업데이트 */ })
});
```

### 3.6 렌더링 파이프라인 (TerminalRenderer)

```
Terminal Grid → Layout Phase → Paint Phase
                     │              │
                     ├─ 배경 수집 (BackgroundRect)
                     ├─ 텍스트 배칭 (BatchedTextRun)
                     │              │
                     │              ├─ 기본 배경 페인트
                     │              ├─ 비기본 배경 페인트
                     │              ├─ 텍스트 페인트
                     │              └─ 커서 페인트
```

**셀 측정**: `│` (BOX DRAWINGS LIGHT VERTICAL) 문자로 셀 크기 측정 (고정폭 보장)

---

## 4. 프로젝트 간 비교 분석

### 4.1 아키텍처 비교

| 항목 | gpui-ghostty | Zed terminal | gpui-terminal |
|------|-------------|-------------|---------------|
| **VT 백엔드** | Ghostty (Zig, C ABI) | alacritty_terminal | alacritty_terminal |
| **VT 파싱** | Ghostty 내부 | Alacritty EventLoop | Processor::advance() 수동 |
| **PTY** | portable-pty (예제) | alacritty_terminal::tty | 임의 Read/Write |
| **잠금** | 없음 (단일 소유) | FairMutex | parking_lot::Mutex |
| **스냅샷** | dump_viewport() 텍스트 | TerminalContent 셀 복사 | with_term() 직접 접근 |
| **렌더링** | View + Element 이중 | View + Element 이중 | View + Renderer |
| **텍스트 배칭** | StyleRun → TextRun | BatchedTextRun (셀 단위) | BatchedTextRun (셀 단위) |
| **Dirty 추적** | take_dirty_viewport_rows | Alacritty 내장 damage | 없음 (전체 갱신) |
| **IME** | EntityInputHandler | ImeState + Element | 없음 |
| **코드 규모** | ~2,500줄 | ~6,000줄+ | ~3,000줄 |

### 4.2 데이터 흐름 비교

**gpui-ghostty**:
```
PTY → [mpsc 채널] → GPUI async task (16ms 배치)
    → queue_output_bytes() → pending_output 버퍼
    → render() 시 flush → feed_output_bytes_to_session()
    → reconcile_dirty_viewport_after_output() (dirty row만)
    → cx.notify()
```

**Zed terminal**:
```
PTY → [Alacritty EventLoop/IO 스레드] → AlacTermEvent
    → [UnboundedSender] → subscribe() async task
    → 첫 이벤트 즉시 + 4ms 배칭 (최대 100개)
    → process_event() → events VecDeque
    → sync() 호출 시 Term 잠금 → make_content() 스냅샷
    → Element가 last_content 읽어서 렌더링
```

**gpui-terminal**:
```
PTY → [백그라운드 스레드, 4KB 읽기] → flume 채널
    → async task → process_bytes() (즉시)
    → cx.notify()
    → render() 시 with_term() 잠금 → 그리드 직접 읽기
```

### 4.3 Crux에 대한 권장사항

#### 채택해야 할 패턴

1. **Zed의 Entity ↔ View 분리**: `Terminal` (엔티티, 상태 관리) + `TerminalView` (뷰, UI) + `TerminalElement` (엘리먼트, 렌더링). 이 3단 분리는 상태 관리와 렌더링의 관심사를 깔끔하게 분리한다.

2. **Zed의 이벤트 배칭**: 첫 이벤트 즉시 처리 + 4ms 윈도우 + 100개 상한. 레이턴시와 처리량의 최적 균형.

3. **Zed의 TerminalContent 스냅샷**: 렌더링 시 Term 잠금 불필요. 복잡한 Element prepaint/paint가 락 없이 진행 가능.

4. **gpui-ghostty의 dirty row 최적화**: 변경된 행만 갱신하는 패턴. Ghostty VT 고유 기능이지만, alacritty_terminal의 damage tracking으로도 유사하게 구현 가능.

5. **gpui-ghostty의 IME EntityInputHandler**: GPUI의 표준 IME 통합 경로. `replace_and_mark_text_in_range`로 조합 텍스트 관리, `replace_text_in_range`으로 확정 텍스트 전송.

6. **BatchedTextRun**: 세 프로젝트 모두 동일 스타일 셀을 하나의 TextRun으로 합치는 최적화 적용. 필수 패턴.

7. **cell_metrics via "M" 문자**: gpui-ghostty는 "M"으로, gpui-terminal은 "│"로 셀 크기 측정. 두 방식 모두 유효하지만, 터미널 폰트에서는 "M"이 더 보편적.

#### 피해야 할 패턴

1. **gpui-ghostty의 parse_tail 수동 파싱**: Ghostty VT가 모드 상태를 노출하지 않아서 출력 바이트를 수동 스캔하는 방식. alacritty_terminal은 `TermMode`로 모든 모드를 직접 노출하므로 불필요.

2. **gpui-terminal의 전체 갱신**: dirty tracking 없이 매 프레임 전체 그리드를 순회하는 방식. 대형 터미널에서 성능 이슈.

3. **gpui-terminal의 직접 잠금 렌더링**: `with_term()` 호출로 렌더링 중 Term 잠금 보유. I/O 스레드와 경합 발생 가능.

#### GPUI 버전 고정 전략

gpui-ghostty가 Zed 특정 커밋에 고정하는 것처럼, Crux도 GPUI를 특정 버전에 고정해야 한다:
```toml
[dependencies]
gpui = "0.2.2"  # 또는 Git 커밋 고정
```

GPUI API가 아직 불안정하므로, 주기적으로 업그레이드하되 테스트 스위트로 호환성 검증.

---

## 5. 참조 소스 코드 경로

### gpui-ghostty
- `crates/gpui_ghostty_terminal/src/session.rs` — TerminalSession (VT 래퍼)
- `crates/gpui_ghostty_terminal/src/view/mod.rs` — TerminalView + TerminalTextElement
- `crates/gpui_ghostty_terminal/src/config.rs` — TerminalConfig
- `crates/ghostty_vt/src/lib.rs` — Safe Rust FFI 래퍼
- `examples/pty_terminal/src/main.rs` — PTY 통합 예제

### Zed terminal
- `crates/terminal/src/terminal.rs` — Terminal Entity + TerminalContent
- `crates/terminal_view/src/terminal_view.rs` — TerminalView
- `crates/terminal_view/src/terminal_element.rs` — TerminalElement + BatchedTextRun + LayoutState
- `crates/terminal/src/mappings/keys.rs` — 키 → 이스케이프 변환
- `crates/terminal/src/mappings/mouse.rs` — 마우스 → 그리드 좌표

### gpui-terminal
- `src/terminal.rs` — TerminalState (Arc<Mutex<Term>>)
- `src/view.rs` — TerminalView (I/O + Render)
- `src/render.rs` — TerminalRenderer (배칭 + 페인팅)
- `src/input.rs` — keystroke_to_bytes
- `src/main.rs` — 예제 (portable-pty)
