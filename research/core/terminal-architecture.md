---
title: "Rust 터미널 에뮬레이터 아키텍처 분석"
description: "Architecture analysis of Alacritty, WezTerm, Rio, Ghostty — crate structure, rendering pipeline, event loop patterns applicable to Crux"
date: 2026-02-11
phase: [1]
topics: [architecture, alacritty, wezterm, rio, ghostty, design-patterns]
status: final
related:
  - terminal-emulation.md
  - ../gpui/framework.md
  - ../gpui/terminal-implementations.md
---

# Rust 기반 터미널 에뮬레이터 아키텍처 분석

> Alacritty, WezTerm, Rio, Ghostty의 아키텍처 패턴을 분석하고 Crux에 적용 가능한 패턴을 도출한다.

---

## 1. Alacritty 아키텍처

### 1.1 크레이트 구조

```
alacritty/                  # GUI 애플리케이션 (윈도우, 렌더러, 입력)
alacritty_terminal/         # 터미널 코어 라이브러리 (PTY, 그리드, 파서)
alacritty_config/           # 설정 시스템 (TOML 파싱, 유효성 검사)
alacritty_config_derive/    # 설정 매크로 (derive 프로시저)
```

**총 4개 크레이트** (Edition 2024, MSRV 1.85.0)

**핵심 설계**: 터미널 로직(`alacritty_terminal`)과 렌더링(`alacritty`)을 완전히 분리. `alacritty_terminal`은 독립 크레이트로 다른 프로젝트에서 재사용 가능하다(실제로 여러 프로젝트에서 의존성으로 사용).

### 1.2 PTY 이벤트 루프 아키텍처

```
┌─────────────────────────────────────────────────┐
│                   Main Thread                    │
│  ┌──────────┐   ┌───────────┐   ┌────────────┐ │
│  │  winit   │──▶│  Event    │──▶│  Display   │ │
│  │ EventLoop│   │  Handler  │   │  (Render)  │ │
│  └──────────┘   └───────────┘   └────────────┘ │
│        ▲              │                          │
│        │         EventLoopSender                 │
│   Event::Wakeup      │                          │
│        │              ▼                          │
│  ┌──────────────────────────────────────┐       │
│  │     Arc<FairMutex<Term<U>>>          │       │
│  │  (공유 터미널 상태)                    │       │
│  └──────────────────────────────────────┘       │
│        ▲              │                          │
│        │         Msg::Input                      │
│   pty_read()     Msg::Resize                     │
│        │         Msg::Shutdown                   │
│        ▼              ▼                          │
│  ┌──────────────────────────────────────┐       │
│  │         PTY EventLoop Thread          │       │
│  │  polling::Poller (I/O 멀티플렉싱)     │       │
│  │  vte::Parser (VT 시퀀스 파싱)         │       │
│  └──────────────────────────────────────┘       │
└─────────────────────────────────────────────────┘
```

**핵심 패턴들**:

1. **FairMutex**: `parking_lot` 기반의 공정 뮤텍스. PTY 스레드가 터미널 상태를 업데이트하고, 메인 스레드가 렌더링을 위해 읽는다. 공정성이 중요한 이유는 PTY 스레드가 렌더링보다 우선해야 하기 때문.

2. **Lease 메커니즘**: `terminal.lease()`로 다음 락을 예약하여 PTY 스레드가 렌더러에 의해 기아(starvation) 상태가 되지 않도록 보장.

3. **채널 기반 통신**: `mpsc::channel`로 메인→PTY 방향 메시지 전달 (`Msg::Input`, `Msg::Resize`, `Msg::Shutdown`). PTY→메인 방향은 `Event::Wakeup` 이벤트로 "다시 그려라" 시그널만 전달.

4. **읽기 버퍼 전략**:
   - `READ_BUFFER_SIZE = 0x10_0000` (1MB): 강제 동기화 전 최대 읽기량
   - `MAX_LOCKED_READ = u16::MAX`: 락 보유 중 최대 처리량
   - 비차단(try_lock) → 버퍼 초과 시 차단(lock)으로 전환

### 1.3 Term 구조체 (핵심 터미널 상태)

```rust
pub struct Term<T> {
    grid: Grid<Cell>,           // 현재 화면 그리드
    inactive_grid: Grid<Cell>,  // 대체 화면(Alt Screen)
    mode: TermMode,             // 비트플래그 기반 터미널 모드
    scroll_region: Range<Line>,
    cursor_style: CursorStyle,
    colors: Colors,
    title: Option<String>,
    damage: TermDamage,         // 변경 추적 (damage tracking)
    selection: Option<Selection>,
    // ...
}
```

**TermMode 비트플래그**: `u32` 기반으로 32개 모드를 효율적으로 관리. Kitty keyboard protocol 지원 포함.

### 1.4 렌더링 파이프라인

```
Term → RenderableContent → RenderableCell[] → OpenGL Renderer
```

1. **RenderableContent**: `Term`에서 렌더링에 필요한 데이터만 추출하는 이터레이터 패턴
2. **두 가지 렌더러**: GLES2 (구형 GPU) / GLSL3 (신형 GPU) 자동 선택
3. **crossfont**: 자체 폰트 래스터라이제이션 크레이트. FreeType/fontconfig (Linux), CoreText (macOS), DirectWrite (Windows)

### 1.5 Damage Tracking (변경 추적)

```rust
pub struct LineDamageBounds {
    pub line: usize,
    pub left: usize,
    pub right: usize,
}
```

- 줄 단위로 변경된 영역만 추적
- `DamageTracker`가 프레임 간 변경사항을 관리하여 부분 렌더링 지원
- Wayland 환경에서 특히 유효 (부분 surface 업데이트)

### 1.6 설정 시스템

- **TOML** 기반 설정 파일
- **Live Reload**: 파일 시스템 감시(inotify/kqueue)로 설정 변경 감지
- 설정 마이그레이션 시스템 내장 (`migrate` 모듈)

### 1.7 Crux에 적용 가능한 패턴

| 패턴 | 적용 방안 |
|------|----------|
| `FairMutex` + `Lease` | PTY↔렌더러 간 터미널 상태 공유에 동일 패턴 적용 |
| 크레이트 분리 (terminal/gui) | `crux-terminal` (코어) + `crux` (GPUI 앱) 구조 |
| Damage Tracking | GPUI 리페인트 최적화에 활용 |
| `EventLoopSender` | PTY→UI 방향 Wakeup 시그널 패턴 |
| `READ_BUFFER_SIZE` 전략 | 대량 출력 시 UI 응답성 유지 |

---

## 2. WezTerm 아키텍처

### 2.1 Mux (멀티플렉서) 아키텍처

WezTerm의 핵심은 **Mux** (멀티플렉서) 패턴이다. tmux와 유사하게 하나의 프로세스가 여러 터미널 세션을 관리한다.

```
┌───────────────────────────────────────────────────┐
│                     Mux                            │
│  ┌─────────┐  ┌──────────┐  ┌──────────────────┐ │
│  │ Windows  │  │  Tabs    │  │  Panes           │ │
│  │ HashMap  │  │ HashMap  │  │  HashMap         │ │
│  │ <WindowId│  │ <TabId,  │  │  <PaneId,        │ │
│  │  Window> │  │  Arc<Tab>│  │   Arc<dyn Pane>> │ │
│  └─────────┘  └──────────┘  └──────────────────┘ │
│  ┌──────────────────┐  ┌──────────────────────┐  │
│  │ Domains           │  │ Subscribers          │  │
│  │ HashMap<DomainId, │  │ (MuxNotification     │  │
│  │  Arc<dyn Domain>> │  │  pub/sub 시스템)      │  │
│  └──────────────────┘  └──────────────────────┘  │
└───────────────────────────────────────────────────┘
```

**Mux 구조체 핵심 필드**:
```rust
pub struct Mux {
    tabs: RwLock<HashMap<TabId, Arc<Tab>>>,
    panes: RwLock<HashMap<PaneId, Arc<dyn Pane>>>,
    windows: RwLock<HashMap<WindowId, Window>>,
    domains: RwLock<HashMap<DomainId, Arc<dyn Domain>>>,
    subscribers: RwLock<HashMap<usize, Box<dyn Fn(MuxNotification) -> bool + Send + Sync>>>,
    // ...
}
```

### 2.2 Tab/Pane 관리 (바이너리 트리 기반 분할)

```rust
pub type Tree = bintree::Tree<Arc<dyn Pane>, SplitDirectionAndSize>;

struct TabInner {
    id: TabId,
    pane: Option<Tree>,     // 바이너리 트리로 분할 pane 관리
    size: TerminalSize,
    active: usize,          // 활성 pane 인덱스
    zoomed: Option<Arc<dyn Pane>>,
    // ...
}
```

**분할 시스템**:
- `bintree::Tree`를 사용하여 수평/수직 분할을 재귀적으로 표현
- 각 리프 노드가 `Arc<dyn Pane>`, 내부 노드가 `SplitDirectionAndSize`
- `PositionedPane`으로 각 pane의 절대 위치(top, left, width, height)를 계산
- `SplitRequest`로 분할 요청: 방향, 크기(셀/퍼센트), 위치(활성 pane/최상위)

### 2.3 Domain 시스템

```rust
pub trait Domain: Downcast + Send + Sync {
    fn spawn(/* ... */) -> Result<Arc<dyn Pane>>;
    fn domain_id(&self) -> DomainId;
    fn domain_name(&self) -> &str;
    fn state(&self) -> DomainState;
    // ...
}
```

Domain은 pane의 "출처"를 추상화:
- **LocalDomain**: 로컬 PTY 프로세스
- **SshDomain**: SSH 원격 연결
- **TlsDomain**: TLS 기반 원격 연결
- **TmuxDomain**: tmux 세션 통합

### 2.4 CLI 서버 아키텍처

```
┌─────────────┐     Unix Socket      ┌──────────────────┐
│ wezterm cli  │ ←─────────────────▶ │  WezTerm Server   │
│ (Client)     │     PDU Protocol    │  (GUI Process)    │
│              │                      │                   │
│ ProxyCommand │                      │  Mux              │
│ - SetClientId│                      │  ├── Windows      │
│ - encode PDU │                      │  ├── Tabs         │
│ - netcat mode│                      │  └── Panes        │
└─────────────┘                      └──────────────────┘
```

**서버 연결 패턴**:
1. `wezterm cli` 실행 시 Unix 소켓으로 실행 중인 인스턴스에 연결
2. 연결 실패 시 자동으로 서버 프로세스 시작 (`unix_connect_with_retry`)
3. PDU(Protocol Data Unit) 기반 직렬화된 메시지 교환
4. `ProxyCommand`: stdin/stdout을 소켓으로 중계하는 netcat 모드
5. 비동기 RPC 매크로: `rpc!(method_name, RequestType, ResponseType)`

**MuxNotification 이벤트**:
```rust
pub enum MuxNotification {
    PaneOutput(PaneId),
    PaneAdded(PaneId),
    PaneRemoved(PaneId),
    WindowCreated(WindowId),
    WindowRemoved(WindowId),
    Alert { pane_id: PaneId, alert: Alert },
    // ...
}
```
Pub/sub 패턴으로 Mux 상태 변경을 모든 구독자에게 알림.

### 2.5 PTY 데이터 파싱 (coalesce 전략)

```rust
fn parse_buffered_data(pane: Weak<dyn Pane>, dead: &Arc<AtomicBool>, mut rx: FileDescriptor) {
    let mut parser = termwiz::escape::parser::Parser::new();
    let mut actions = vec![];
    let mut hold = false;        // Synchronized Output 모드
    let mut action_size = 0;
    let mut delay = Duration::from_millis(coalesce_delay_ms);
    // ...
}
```

**핵심 최적화**:
1. **Synchronized Output 지원**: `DECRPM 2026` 시퀀스를 감지하여 hold 모드 진입/해제
2. **Coalesce 지연**: 작은 데이터일 때 짧은 대기를 통해 "프레임" 단위로 묶어서 전달
3. **Action 기반 처리**: 바이트가 아닌 파싱된 `Action`을 pane에 전달
4. **Weak 참조**: pane이 삭제되면 자동으로 읽기 스레드 종료

### 2.6 폰트 시스템

- **termwiz**: 자체 터미널 위젯 라이브러리 (VT 파서 포함)
- **harfbuzz**: 고급 텍스트 셰이핑 (리거쳐, CJK 지원)
- **freetype**: 폰트 래스터라이제이션
- WGPU + OpenGL 듀얼 렌더링 지원 (`shader.wgsl`, `glyph-frag.glsl`)

### 2.7 Crux에 적용 가능한 패턴

| 패턴 | 적용 방안 |
|------|----------|
| Mux 멀티플렉서 | 탭/분할 pane 관리의 핵심 아키텍처 |
| Binary Tree 분할 | Split pane 구현에 `bintree` 패턴 적용 |
| Domain 추상화 | 로컬/원격/SSH 터미널 소스 추상화 |
| Unix Socket + PDU | CLI↔서버 IPC 구현 |
| Synchronized Output | 프레임 단위 렌더링 최적화 |
| MuxNotification pub/sub | 이벤트 기반 UI 업데이트 |

---

## 3. Rio 아키텍처

### 3.1 프로젝트 구조

```
rio-backend/        # 터미널 코어 (crosswords, ansi, config, event)
rio-window/         # 윈도우 관리 (winit 기반)
sugarloaf/          # WGPU 렌더링 엔진
teletypewriter/     # PTY 추상화
frontends/          # 플랫폼별 프론트엔드
copa/               # 모르비우스 코파 (추가 기능)
corcovado/          # I/O 이벤트 루프 (mio 포크)
```

### 3.2 Sugarloaf 렌더링 엔진

```rust
pub struct Sugarloaf<'a> {
    pub ctx: Context<'a>,           // WGPU 컨텍스트
    quad_brush: QuadBrush,          // 사각형 렌더링 (배경, 커서)
    rich_text_brush: RichTextBrush, // 텍스트 렌더링 (글리프 아틀라스)
    layer_brush: LayerBrush,        // 레이어 합성 (이미지, 그래픽)
    state: SugarState,              // 렌더링 상태 관리
    background_color: Option<wgpu::Color>,
    background_image: Option<ImageProperties>,
    graphics: Graphics,             // 그래픽스 프리미티브
    filters_brush: Option<FiltersBrush>, // 포스트 프로세싱 필터
}
```

**렌더링 레이어 구조**:
```
┌──────────────────────────┐
│ FiltersBrush (후처리)     │  ← CRT 효과, 블러 등
├──────────────────────────┤
│ LayerBrush (이미지/그래픽)│  ← Sixel, iTerm2 이미지
├──────────────────────────┤
│ RichTextBrush (텍스트)    │  ← 글리프 아틀라스 기반 텍스트
├──────────────────────────┤
│ QuadBrush (사각형)        │  ← 배경색, 커서, 선택 영역
├──────────────────────────┤
│ Background               │  ← 배경색 / 배경 이미지
└──────────────────────────┘
```

### 3.3 Colorspace 처리

```rust
pub enum Colorspace {
    Srgb,
    DisplayP3,
    Rec2020,
}

// macOS에서 기본값이 DisplayP3
#[cfg(target_os = "macos")]
impl Default for Colorspace {
    fn default() -> Colorspace {
        Colorspace::DisplayP3
    }
}
```

macOS에서 P3 Wide Color Gamut을 기본 지원하여 더 넓은 색상 범위를 표현.

### 3.4 이벤트 시스템

```rust
pub enum Msg {
    Input(Cow<'static, [u8]>),
    Shutdown,
    Resize(WinsizeBuilder),
}

pub enum RioEvent {
    PrepareRender(u64),
    Render,
    Wakeup(usize),
    UpdateGraphics { route_id: usize, queues: UpdateQueues },
    Paste,
    Copy(String),
    Title(String),
    // ...
}
```

- **라우트 기반 렌더링**: `route_id`로 특정 탭/pane만 갱신
- **TerminalDamage 3단계**: `Full` / `Partial(BTreeSet<LineDamage>)` / `CursorOnly`
- **EventLoopProxy**: winit 이벤트 루프에 커스텀 이벤트 주입

### 3.5 Crosswords (터미널 그리드)

Alacritty의 `alacritty_terminal`과 유사한 구조이나 자체 구현:
- `grid/` - 터미널 그리드 (행/열 기반)
- `square.rs` - 셀 데이터 (Alacritty의 `Cell`에 해당)
- `pos.rs` - 위치 타입
- `attr.rs` - 셀 속성 (색상, 스타일)
- `vi_mode.rs` - Vi 모드 지원
- `search.rs` - 텍스트 검색

### 3.6 CJK 문자 처리

Rio는 `unicode_width` 크레이트를 사용하여 CJK 문자의 폭(1칸 vs 2칸)을 결정한다. Sugarloaf의 `RichTextBrush`에서 wide 문자를 렌더링할 때 두 셀에 걸쳐 글리프를 배치한다.

### 3.7 Crux에 적용 가능한 패턴

| 패턴 | 적용 방안 |
|------|----------|
| Sugarloaf 레이어 구조 | GPUI에서 유사한 다중 레이어 렌더링 |
| DisplayP3 기본 지원 | macOS에서 Wide Color Gamut 활용 |
| TerminalDamage 3단계 | 세밀한 리페인트 최적화 |
| route_id 기반 갱신 | 탭/pane별 독립 렌더링 |
| corcovado (mio 포크) | 커스텀 I/O 이벤트 루프 가능성 |

---

## 4. Ghostty 아키텍처

### 4.1 핵심 설계 철학: libghostty 분리

Ghostty는 Zig로 작성되었지만, 가장 중요한 아키텍처 패턴을 제공한다.

```
┌──────────────────────────────────────────────┐
│             Application Layer                 │
│  ┌────────────┐  ┌──────┐  ┌──────────────┐ │
│  │ macOS App  │  │ GTK  │  │ Browser/WASM │ │
│  │ (Swift/    │  │ App  │  │              │ │
│  │  AppKit)   │  │      │  │              │ │
│  └─────┬──────┘  └──┬───┘  └──────┬───────┘ │
│        │            │              │          │
│        ▼            ▼              ▼          │
│  ┌──────────────────────────────────────┐    │
│  │          apprt (App Runtime)         │    │
│  │  embedded | gtk | browser | none     │    │
│  └──────────────────┬───────────────────┘    │
│                     │                         │
│  ┌──────────────────▼───────────────────┐    │
│  │           libghostty (C API)          │    │
│  │  ghostty_init()                       │    │
│  │  ghostty_surface_*()                  │    │
│  │  ghostty_config_*()                   │    │
│  └──────────────────┬───────────────────┘    │
│                     │                         │
│  ┌──────────────────▼───────────────────┐    │
│  │              Core Layer               │    │
│  │  Surface | Terminal | Renderer        │    │
│  │  Font | Config | Input               │    │
│  └──────────────────────────────────────┘    │
└──────────────────────────────────────────────┘
```

**핵심**: `apprt` (App Runtime)이 컴파일 타임에 선택되어 런타임 오버헤드 없이 플랫폼별 구현을 교체한다.

```zig
pub const runtime = switch (build_config.artifact) {
    .exe => switch (build_config.app_runtime) {
        .none => none,
        .gtk => gtk,
    },
    .lib => embedded,        // macOS Swift 앱에서 사용
    .wasm_module => browser,
};
```

### 4.2 Surface 추상화 (핵심 패턴)

```zig
/// Surface는 단일 터미널 "표면"을 나타낸다. 터미널 표면은
/// 터미널이 그려지고 키보드/마우스 등의 이벤트에 응답하는
/// 최소한의 "위젯"이다.
///
/// "surface"라는 단어를 사용하는 이유는 상위 앱 런타임이
/// 이 표면을 윈도우, 탭, 분할, 미리보기 등으로 결정하기
/// 때문이다. 이 구조체는 신경 쓰지 않는다: 그저 그리고
/// 이벤트에 응답할 뿐이다.
const Surface = @This();
```

**Surface 구조**:
```zig
alloc: Allocator,
app: *App,
rt_app: *apprt.runtime.App,
rt_surface: *apprt.runtime.Surface,

// 폰트
font_grid_key: font.SharedGridSet.Key,
font_size: font.face.DesiredSize,
font_metrics: font.Metrics,

// 렌더러 (별도 스레드)
renderer: Renderer,
renderer_state: rendererpkg.State,
renderer_thread: rendererpkg.Thread,
renderer_thr: std.Thread,

// 터미널 I/O (별도 스레드)
io: termio.Termio,
io_thread: termio.Thread,
io_thr: std.Thread,

// 상태
size: rendererpkg.Size,
config: DerivedConfig,
focused: bool = true,
child_exited: bool = false,
readonly: bool = false,
```

**스레드 모델**:
```
┌─────────────────┐
│   Main Thread    │ ← UI 이벤트 처리, Surface 관리
│   (App Runtime)  │
└────────┬─────────┘
         │
    ┌────┴────────────────────┐
    │                          │
    ▼                          ▼
┌──────────────┐    ┌──────────────────┐
│ Renderer     │    │ Termio Thread    │
│ Thread       │    │ (PTY I/O)        │
│ - Metal/GL   │    │ - 읽기/쓰기      │
│ - 글리프 렌더 │    │ - VT 파서        │
│ - 프레임 합성 │    │ - 이벤트 디스패치 │
└──────────────┘    └──────────────────┘
```

### 4.3 Termio (터미널 I/O) 아키텍처

```zig
/// Termio의 구성 요소:
///   - Termio: 모든 백엔드에 공통 로직을 가진 메인 공유 구조체
///   - Backend: 실제 물리적 I/O 담당. 예: 서브프로세스 생성, PTY 할당
///   - Mailbox: 백엔드에 이벤트 메시지를 저장/배포. 단일/멀티 스레드 지원
```

**Mailbox 패턴**: 메시지 큐를 백엔드와 분리하여 동기/비동기 모드를 컴파일 타임에 선택.

### 4.4 렌더러 추상화

```zig
pub const Renderer = switch (build_config.renderer) {
    .metal => GenericRenderer(Metal),
    .opengl => GenericRenderer(OpenGL),
    .webgl => WebGL,
};
```

- **GenericRenderer<T>**: 제네릭 래퍼로 Metal/OpenGL 공통 로직 공유
- **Metal**: macOS 네이티브 (최고 성능)
- **OpenGL**: Linux/크로스 플랫폼
- **WebGL**: 브라우저 (WASM 타겟)
- **별도 렌더러 스레드**: UI 스레드와 독립적으로 프레임 생성

### 4.5 SIMD 최적화 VT 파서

```zig
// C++ SIMD 구현을 Zig에서 FFI로 호출
extern "c" fn ghostty_simd_decode_utf8_until_control_seq(
    input: [*]const u8,
    count: usize,
    output: [*]u32,
    output_count: *usize,
) usize;
```

**핵심 전략**:
1. UTF-8 바이트 스트림에서 제어 시퀀스(ESC, 0x1B)까지의 일반 텍스트를 SIMD로 일괄 디코딩
2. 제어 시퀀스 발견 시 일반 파서로 전환
3. 스칼라 폴백 구현도 동일 인터페이스로 제공
4. C++로 SIMD 구현 → Zig FFI로 호출 (NEON/AVX2 최적화)

### 4.6 IME 처리

Surface가 IME 위치(`IMEPos`)를 apprt에 전달하여 플랫폼별 IME 구현에 위임. macOS에서는 `embedded` apprt가 `NSTextInputClient`를 통해 한국어 IME를 처리.

### 4.7 Crux에 적용 가능한 패턴

| 패턴 | 적용 방안 |
|------|----------|
| Surface 추상화 | GPUI Element로 Surface 패턴 구현 |
| libghostty C API | Crux를 라이브러리로 분리할 때 참고 |
| apprt 컴파일타임 선택 | Rust feature flag로 유사 패턴 구현 |
| 3스레드 모델 | Main + Renderer + IO 스레드 분리 |
| Termio Mailbox | 메시지 큐 기반 PTY 통신 |
| SIMD VT 파서 | 고성능 VT 파싱 (향후 최적화) |
| GenericRenderer<T> | 렌더러 백엔드 추상화 |

---

## 5. 프로젝트 구조 비교 (2026-02-12 보강)

> 상세 분석: [competitive/terminal-structures.md](../competitive/terminal-structures.md)

### 5.1 크레이트 수 비교

| 프로젝트 | 크레이트 수 | 적정성 평가 |
|----------|-----------|------------|
| Zed Terminal | 2 | 최소 — 에디터 내장이므로 적합 |
| Alacritty | 4 | 미니멀 — 독립 터미널의 하한 |
| Crux | 6 | 적절 — 기능 확장 가능 여지 |
| Rio | 8 | 적절 — 독립 렌더러(Sugarloaf) 포함 |
| WezTerm | 55-60+ | 과다 — 유지보수 부담, 메인테이너 번아웃 |

### 5.2 핵심 발견

1. **Zed Terminal이 Crux의 직접적 레퍼런스**: 동일한 GPUI + alacritty_terminal 조합. Entity-View-Element 패턴, 이벤트 배칭(4ms/100개), Arc<FairMutex<>> 상태 공유 패턴 검증됨.

2. **렌더링 최적화 3대 패턴**:
   - **이벤트 배칭** (Zed): 4ms 타임아웃 또는 100개 이벤트 배치 처리
   - **셀 배칭** (Zed): BatchedTextRun으로 동일 스타일 인접 셀 병합 (~10셀/배치)
   - **배경 병합** (Zed): 같은 색상의 인접 배경 사각형 수평/수직 병합

3. **Damage Tracking**:
   - Ghostty: 3단계 (false/partial/full) — 가장 정교
   - alacritty_terminal: TermDamage 내장 — Crux가 활용 가능
   - Rio: Redux 스타일 상태 머신 — 변경 없는 행 스킵

4. **텍스트 런 캐싱** (Rio 고유):
   - 256-버킷 해시 테이블 + LRU 이빅션
   - 반복 콘텐츠 셰이핑 오버헤드 96% 감소
   - Crux Phase 2에서 도입 고려

5. **IPC 프로토콜 패턴** (WezTerm):
   - codec 크레이트: varbincode + zstd 압축
   - 3가지 모드: in-process, 로컬 소켓, 원격 TLS
   - Crux의 crux-ipc JSON-RPC 설계에 참고

---

## 6. 비교 분석 및 Crux 적용 권장사항

### 6.1 아키텍처 레이어 비교

| 레이어 | Alacritty | WezTerm | Rio | Ghostty |
|--------|-----------|---------|-----|---------|
| VT 파서 | `vte` 크레이트 | `termwiz` (자체) | 자체 `ansi` | SIMD + 자체 |
| 터미널 상태 | `Term<T>` + `Grid` | `wezterm-term` | `crosswords` | `terminal` |
| PTY | `alacritty_terminal::tty` | `portable-pty` | `teletypewriter` | 자체 `pty.zig` |
| 렌더러 | OpenGL (glutin) | WGPU + OpenGL | WGPU (sugarloaf) | Metal/OpenGL/WebGL |
| 멀티플렉싱 | 없음 (단일 창) | Mux (탭/분할/원격) | 탭 지원 | apprt 위임 |
| IPC | 없음 | Unix Socket + PDU | 없음 | apprt IPC |

### 6.2 PTY ↔ 렌더러 통신 패턴 비교

| 프로젝트 | 패턴 | 공유 상태 | 장점 | 단점 |
|----------|------|----------|------|------|
| Alacritty | FairMutex + Wakeup | `Arc<FairMutex<Term>>` | 단순, 효율적 | 단일 창 전용 |
| WezTerm | Action + Notification | `Arc<dyn Pane>` | 멀티 pane/원격 | 복잡도 높음 |
| Rio | Event + Damage | EventLoopProxy | WGPU 최적화 | Alacritty 유사 |
| Ghostty | Mailbox + Thread | State + Message | 3스레드 분리 | Zig 특화 |

### 6.3 Crux 권장 아키텍처

```
┌──────────────────────────────────────────────────┐
│                   Crux Terminal                    │
│                                                    │
│  ┌──────────────────────────────────────────┐     │
│  │           GPUI Application                │     │
│  │  ┌────────────────────────────────────┐  │     │
│  │  │     TerminalView (GPUI Element)    │  │     │
│  │  │     ← Ghostty Surface 패턴         │  │     │
│  │  └───────────────┬────────────────────┘  │     │
│  └──────────────────┼───────────────────────┘     │
│                     │                              │
│  ┌──────────────────▼───────────────────────┐     │
│  │         Mux (WezTerm 패턴)                │     │
│  │  ┌─────────┐  ┌──────┐  ┌────────────┐  │     │
│  │  │ Windows │  │ Tabs │  │ Panes      │  │     │
│  │  └─────────┘  └──────┘  │(BinaryTree)│  │     │
│  │                          └────────────┘  │     │
│  └──────────────────┬───────────────────────┘     │
│                     │                              │
│  ┌──────────────────▼───────────────────────┐     │
│  │      Terminal Core (Alacritty 패턴)       │     │
│  │  ┌───────────┐  ┌────────┐  ┌─────────┐ │     │
│  │  │ Term<T>   │  │ Grid   │  │ Damage  │ │     │
│  │  │+ FairMutex│  │+ Cell  │  │ Tracker │ │     │
│  │  └───────────┘  └────────┘  └─────────┘ │     │
│  └──────────────────┬───────────────────────┘     │
│                     │                              │
│  ┌──────────────────▼───────────────────────┐     │
│  │           PTY Layer                       │     │
│  │  ┌────────────────┐  ┌────────────────┐  │     │
│  │  │ EventLoop      │  │ VT Parser      │  │     │
│  │  │ (별도 스레드)    │  │ (vte 크레이트)  │  │     │
│  │  └────────────────┘  └────────────────┘  │     │
│  └──────────────────────────────────────────┘     │
│                                                    │
│  ┌──────────────────────────────────────────┐     │
│  │           IPC Layer (WezTerm 패턴)        │     │
│  │  Unix Socket + PDU + CLI Server           │     │
│  └──────────────────────────────────────────┘     │
└──────────────────────────────────────────────────┘
```

### 6.4 핵심 구현 권장사항

#### 1) 크레이트 구조 (Alacritty + WezTerm 하이브리드)
```
crux-terminal/     # 터미널 코어 (Grid, Cell, VT 파서, PTY)
crux-mux/          # 멀티플렉서 (탭, 분할, Domain)
crux/              # GPUI 애플리케이션
crux-cli/          # CLI 도구 (IPC 클라이언트)
```

#### 2) PTY 이벤트 루프 (Alacritty 패턴 채택)
- `FairMutex<Term>` 공유 상태
- `Lease` 메커니즘으로 PTY 스레드 우선순위 보장
- `READ_BUFFER_SIZE` 기반 적응적 읽기
- `Event::Wakeup` 시그널로 UI 갱신 트리거

#### 3) 탭/분할 관리 (WezTerm 패턴 채택)
- `bintree::Tree` 기반 분할 pane 관리
- `Arc<dyn Pane>` 트레이트 객체로 pane 다형성
- `MuxNotification` pub/sub으로 UI 업데이트

#### 4) 렌더링 (Ghostty + Rio 패턴)
- GPUI가 렌더링을 담당하므로 별도 렌더러 스레드 불필요
- `RenderableContent` 이터레이터로 터미널 상태 → GPUI 엘리먼트 변환
- `DamageTracker`로 변경된 영역만 리페인트

#### 5) SIMD 최적화 (Ghostty 패턴, 향후)
- 일반 텍스트 구간을 SIMD로 일괄 UTF-8 디코딩
- 제어 시퀀스(ESC) 검색도 SIMD indexOf로 가속
- 초기에는 스칼라 구현으로 시작, 프로파일링 후 SIMD 적용

#### 6) Surface 추상화 (Ghostty 패턴)
- 각 터미널 pane이 독립적인 "Surface"
- Surface는 자신의 폰트, 크기, 설정을 소유
- 앱 런타임(GPUI)은 Surface를 어디에 배치할지만 결정

### 6.5 성능 최적화 우선순위

| 우선순위 | 최적화 | 출처 | 효과 |
|----------|--------|------|------|
| 1 | Damage Tracking | Alacritty/Rio | 불필요한 리드로잉 방지 |
| 2 | Synchronized Output | WezTerm | TUI 앱 깜빡임 제거 |
| 3 | FairMutex + Lease | Alacritty | PTY↔렌더러 균형 |
| 4 | Coalesce 지연 | WezTerm | 프레임 단위 배치 처리 |
| 5 | 적응적 읽기 버퍼 | Alacritty | 대량 출력 시 응답성 |
| 6 | SIMD VT 파싱 | Ghostty | UTF-8 디코딩 가속 |

---

## 7. 의존성 크레이트 추천

| 용도 | 크레이트 | 사용처 |
|------|---------|--------|
| VT 파서 | `vte` | Alacritty, 많은 프로젝트에서 사용 |
| PTY | `portable-pty` | WezTerm 작성자 유지보수 |
| 유니코드 폭 | `unicode-width` | 모든 프로젝트에서 사용 |
| 비트플래그 | `bitflags` | 터미널 모드 관리 |
| 뮤텍스 | `parking_lot` | FairMutex 구현 |
| I/O 폴링 | `polling` | Alacritty 이벤트 루프 |
| 설정 파일 | `toml` | Alacritty/Rio 설정 |
| 직렬화 | `serde` | 설정, IPC PDU |
