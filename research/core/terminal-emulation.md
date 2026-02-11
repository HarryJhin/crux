---
title: "터미널 코어 기술 리서치"
description: "VT parser comparison (alacritty_terminal vs vte vs libghostty), PTY management, graphics protocols, tmux compatibility, Unicode/CJK handling"
date: 2026-02-11
phase: [1, 4, 5]
topics: [vt-parser, pty, graphics-protocol, tmux, unicode, cjk]
status: final
related:
  - ../gpui/terminal-implementations.md
  - keymapping.md
  - terminfo.md
  - terminal-architecture.md
---

# Crux 터미널 코어 기술 리서치

> 작성일: 2026-02-11
> 목적: Rust 기반 macOS 터미널 에뮬레이터 "Crux" 개발을 위한 핵심 기술 조사

---

## 목차

1. [VT 파서 크레이트 비교](#1-vt-파서-크레이트-비교)
2. [PTY 관리](#2-pty-관리)
3. [터미널 그래픽스 프로토콜](#3-터미널-그래픽스-프로토콜)
4. [tmux 호환성](#4-tmux-호환성)
5. [유니코드/CJK 처리](#5-유니코드cjk-처리)
6. [스크롤백 버퍼](#6-스크롤백-버퍼)
7. [Crux를 위한 권장사항 요약](#7-crux를-위한-권장사항-요약)

---

## 1. VT 파서 크레이트 비교

터미널 에뮬레이터의 핵심은 VT100/xterm 이스케이프 시퀀스를 파싱하고 터미널 상태를 관리하는 것이다. Rust 생태계에는 세 가지 주요 옵션이 있다.

### 1.1 alacritty_terminal

- **최신 버전**: `0.25.1` (2025년 10월)
- **크레이트**: [crates.io/crates/alacritty_terminal](https://crates.io/crates/alacritty_terminal)
- **문서**: [docs.rs/alacritty_terminal](https://docs.rs/alacritty_terminal/latest/alacritty_terminal/)
- **라이선스**: Apache-2.0

#### 개요

Alacritty 터미널 에뮬레이터에서 추출한 라이브러리 크레이트. VT 파싱뿐 아니라 터미널 그리드, 이벤트 루프, 셀렉션 등 터미널 에뮬레이터의 핵심 기능 전체를 제공한다.

#### 모듈 구조

```
alacritty_terminal/
├── event          # 이벤트 처리
├── event_loop     # PTY I/O 메인 이벤트 루프
├── grid           # 터미널 최적화된 2D 그리드
├── index          # Line/Column 강타입 뉴타입
├── selection      # 텍스트 선택 상태 관리
├── sync           # 동기화 타입
├── term           # Term 고수준 API
├── thread         # 스레드 유틸리티
├── tty            # TTY/PTY 관련
└── vi_mode        # Vi 모드 구현
```

#### 핵심 타입: `Term`

```rust
use alacritty_terminal::term::Term;
use alacritty_terminal::event_loop::EventLoop;
use alacritty_terminal::grid::Grid;

// 주요 메서드
impl Term {
    fn new(config, dimensions, event_proxy) -> Term;
    fn grid(&self) -> &Grid;            // 읽기 전용 그리드 접근
    fn grid_mut(&mut self) -> &mut Grid; // 가변 그리드 접근
    fn renderable_content(&self) -> RenderableContent; // 렌더링용 콘텐츠
    fn resize(&mut self, size);          // 터미널 크기 변경
    fn scroll_display(&mut self, dir);   // 스크롤
    fn selection_to_string(&self) -> Option<String>; // 선택 텍스트
    fn search_next(&self, regex, dir);   // 정규식 검색
    fn damage(&self) -> TermDamage;      // 변경 추적 (렌더링 최적화)
    fn mode(&self) -> TermMode;          // 터미널 모드 상태
    fn colors(&self) -> &Colors;         // 색상 설정
    fn cursor_style(&self) -> CursorStyle; // 커서 스타일
}
```

#### 핵심 타입: `EventLoop`

PTY I/O의 메인 이벤트 루프. PTY에서 읽어온 데이터를 VTE 파서를 통해 처리하여 `Term` 상태를 업데이트한다.

#### 의존성

- `vte` (VT 파서), `regex-automata` (검색), `parking_lot` (동기화)
- `unicode-width` (문자 폭), `base64`, `bitflags`
- 플랫폼: `rustix` (Unix), `windows-sys` (Windows)

#### 장점

- **완전한 터미널 구현**: 파싱 + 그리드 + 이벤트 루프 + 검색 + 선택 모두 포함
- **실전 검증됨**: Alacritty가 가장 널리 사용되는 GPU 가속 터미널 중 하나
- **활발한 유지보수**: 꾸준한 업데이트, vte 0.15 통합
- **Damage tracking**: 변경된 영역만 렌더링 가능 (GPU 렌더링에 유리)
- **Vi 모드, 검색 내장**: 추가 구현 불필요

#### 단점

- **높은 결합도**: Alacritty 아키텍처에 맞춰 설계됨 → 커스텀 렌더러 연결 시 어댑터 필요
- **PTY 관리 포함**: 자체 tty 모듈이 있어 portable-pty와 중복 가능
- **API 불안정**: 시맨틱 버저닝이지만 라이브러리가 아닌 앱 우선 설계
- **Kitty Graphics Protocol 미지원**: 이미지 프로토콜은 별도 구현 필요

---

### 1.2 vte (Alacritty VTE 파서)

- **최신 버전**: `0.15.0` (2025년 2월)
- **크레이트**: [crates.io/crates/vte](https://crates.io/crates/vte)
- **문서**: [docs.rs/vte](https://docs.rs/vte/0.15.0/vte/)
- **소스**: [github.com/alacritty/vte](https://github.com/alacritty/vte)
- **라이선스**: Apache-2.0 OR MIT

#### 개요

Paul Williams의 ANSI 파서 상태 머신을 구현한 저수준 VT 파서. **상태 머신 자체는 파싱된 데이터에 의미를 부여하지 않는다** — `Perform` 트레이트를 구현하여 각 시퀀스에 대한 동작을 정의해야 한다.

#### 핵심 API

```rust
use vte::{Parser, Perform, Params};

struct MyHandler;

impl Perform for MyHandler {
    fn print(&mut self, c: char) {
        // 일반 문자 출력
    }

    fn execute(&mut self, byte: u8) {
        // C0/C1 제어 문자 (예: \n, \r, \t)
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8],
                     ignore: bool, action: char) {
        // CSI 시퀀스 (예: 커서 이동, 색상 설정)
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        // ESC 시퀀스
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        // OSC 시퀀스 (예: 윈도우 타이틀, 하이퍼링크)
    }

    fn hook(&mut self, params: &Params, intermediates: &[u8],
            ignore: bool, action: char) {
        // DCS 시퀀스 시작
    }

    fn unhook(&mut self) {
        // DCS 시퀀스 종료
    }

    fn put(&mut self, byte: u8) {
        // DCS 데이터
    }
}

// 사용법
let mut parser = Parser::new();
let mut handler = MyHandler;
for byte in input_bytes {
    parser.advance(&mut handler, *byte);
}
```

#### 장점

- **최소 의존성**: `arrayvec`, `memchr`만 필요
- **매우 가벼움**: 파싱 레이어만 담당
- **유연성**: 터미널 상태 관리를 완전히 커스텀 가능
- **`#![no_std]` 지원**: 임베디드 환경에서도 사용 가능
- **Alacritty 내부적으로 사용**: 실전에서 검증됨

#### 단점

- **파서만 제공**: 그리드, 상태 관리, 스크롤백 등 모두 직접 구현 필요
- **터미널 에뮬레이터 구축에 방대한 추가 작업 필요**
- **의미 해석 없음**: CSI/OSC 시퀀스의 의미를 직접 매핑해야 함

---

### 1.3 vt100 크레이트 (참고)

- **최신 버전**: `0.16.2` (2025년)
- **크레이트**: [crates.io/crates/vt100](https://crates.io/crates/vt100)
- **문서**: [docs.rs/vt100](https://docs.rs/vt100)

#### 개요

vte와 별개의 크레이트. 터미널 바이트 스트림을 파싱하고 **렌더링된 화면의 인메모리 표현**을 제공한다. `screen`이나 `tmux` 같은 터미널 멀티플렉서를 구현하는 데 적합.

```rust
use vt100::Parser;

let mut parser = vt100::Parser::new(24, 80, 0); // rows, cols, scrollback
parser.process(b"\x1b[31mHello\x1b[m World");

let screen = parser.screen();
let cell = screen.cell(0, 0).unwrap();
println!("문자: {}", cell.contents()); // "H"
println!("전경색: {:?}", cell.fgcolor()); // Color::Idx(1) (빨강)
```

**Crux에서의 활용**: 주력 렌더링 파이프라인보다는 테스트나 스냅샷 비교에 유용할 수 있다.

---

### 1.4 libghostty-vt

- **상태**: 개발 중 (2026년 안정 릴리스 목표)
- **언어**: Zig (C ABI 제공 예정)
- **소스**: [github.com/ghostty-org/ghostty](https://github.com/ghostty-org/ghostty)
- **블로그**: [Libghostty Is Coming](https://mitchellh.com/writing/libghostty-is-coming)

#### 개요

Ghostty 터미널 에뮬레이터에서 추출한 VT 파서 및 터미널 상태 라이브러리. Mitchell Hashimoto가 개발.

#### 제공 기능

- **터미널 시퀀스 파싱**: SIMD 최적화
- **터미널 상태 관리**: 커서 위치, 스타일, 텍스트 래핑
- **유니코드 지원**: 그래핌 클러스터 처리 포함
- **최적화된 메모리 사용**: PageList 구조
- **프로토콜 지원**: Kitty Graphics Protocol, tmux Control Mode

#### 현재 상태

| API | 상태 |
|-----|------|
| Zig 모듈 | 사용 가능 (실험적) |
| C API | 개발 중 |
| Rust 바인딩 | `ghostty-sys` 크레이트 존재하나 비공식 |
| 안정 릴리스 | 6개월 내 목표 (Mitchell Hashimoto 언급) |

#### 향후 로드맵 (libghostty 전체)

- 입력 처리 라이브러리
- GPU 렌더링 라이브러리
- GTK 위젯, Swift 프레임워크 통합

#### 장점

- **최신 설계**: Ghostty의 실전 코드에서 추출
- **SIMD 최적화**: 파싱 성능 우수
- **Kitty Graphics Protocol 내장**
- **그래핌 클러스터 지원**: Mode 2027 포함
- **제로 의존성**: libc도 불필요

#### 단점

- **아직 안정되지 않음**: API 변경 가능성 높음
- **Zig 의존성**: Rust에서 사용하려면 Zig 컴파일러 + FFI 필요
- **동적 라이브러리**: 현재 정적 컴파일 미지원 → 배포 시 .dylib 포함 필요
- **비공식 Rust 바인딩**: 유지보수 불확실

---

### 1.5 비교 요약

| 기준 | alacritty_terminal | vte | libghostty-vt |
|------|-------------------|-----|---------------|
| **추상화 수준** | 높음 (전체 터미널) | 낮음 (파서만) | 중간 (파서+상태) |
| **최신 버전** | 0.25.1 | 0.15.0 | 미릴리스 |
| **언어** | Rust | Rust | Zig (C ABI) |
| **안정성** | 높음 | 높음 | 낮음 (개발 중) |
| **그리드 관리** | 포함 | 미포함 | 포함 |
| **이벤트 루프** | 포함 | 미포함 | 미포함 |
| **검색/선택** | 포함 | 미포함 | 미포함 |
| **Kitty Graphics** | 미지원 | 미지원 | 지원 |
| **SIMD 최적화** | 없음 | 없음 | 있음 |
| **그래핌 클러스터** | 부분적 | 해당 없음 | 완전 지원 |
| **커스텀 가능성** | 중간 | 최고 | 중간 |
| **문서화** | 양호 | 양호 | 부족 |
| **의존성 크기** | 중간 | 최소 | FFI 필요 |

#### Crux 권장 전략

**1차 선택: `alacritty_terminal`**

- 터미널 에뮬레이터의 핵심 로직이 이미 구현되어 있어 개발 속도가 빠름
- Damage tracking이 GPUI 렌더링과 잘 맞음
- 부족한 기능 (Kitty Graphics, 그래핌 클러스터 완전 지원) 은 별도 레이어로 보완

**장기 전략: libghostty-vt 모니터링**

- 안정화되면 마이그레이션 검토 (Kitty Graphics 내장, SIMD 성능)
- C API 안정 후 Rust FFI 바인딩 품질 평가

---

## 2. PTY 관리

### 2.1 portable-pty 크레이트

- **최신 버전**: `0.9.0`
- **크레이트**: [crates.io/crates/portable-pty](https://crates.io/crates/portable-pty)
- **문서**: [docs.rs/portable-pty](https://docs.rs/portable-pty/latest/portable_pty/)
- **소스**: WezTerm 프로젝트의 일부
- **라이선스**: MIT

#### 핵심 API

```rust
use portable_pty::{native_pty_system, PtySize, CommandBuilder};

// 1. PTY 시스템 초기화
let pty_system = native_pty_system();

// 2. PTY 쌍 생성
let pair = pty_system.openpty(PtySize {
    rows: 24,
    cols: 80,
    pixel_width: 0,
    pixel_height: 0,
})?;

// 3. 커맨드 빌더로 쉘 프로세스 생성
let mut cmd = CommandBuilder::new("/bin/zsh");
cmd.env("TERM", "xterm-256color");
cmd.env("LANG", "ko_KR.UTF-8");
cmd.cwd("/Users/jjh");

// 4. 쉘 프로세스 스폰
let child = pair.slave.spawn_command(cmd)?;

// 5. Master에서 I/O
let mut reader = pair.master.try_clone_reader()?;
let mut writer = pair.master.take_writer()?;

// 6. 터미널 리사이즈
pair.master.resize(PtySize {
    rows: 40,
    cols: 120,
    pixel_width: 0,
    pixel_height: 0,
})?;
```

#### 트레이트 구조

```
PtySystem          // PTY 구현 선택 (native_pty_system())
├── openpty()      // → PtyPair { master, slave }
│
MasterPty          // 마스터 측 (터미널 에뮬레이터)
├── resize()       // 윈도우 크기 변경 (TIOCSWINSZ)
├── try_clone_reader() // 읽기 핸들 복제
├── take_writer()  // 쓰기 핸들 획득
│
SlavePty           // 슬레이브 측 (쉘 프로세스)
├── spawn_command() // 자식 프로세스 생성
│
Child              // 자식 프로세스
├── wait()         // 종료 대기
├── kill()         // 프로세스 종료
│
CommandBuilder     // 프로세스 설정
├── new(program)   // 프로그램 경로
├── arg(s)         // 인자 추가
├── env(k, v)      // 환경 변수
├── cwd(path)      // 작업 디렉토리
```

#### macOS 지원 상태

- macOS에서 POSIX PTY (`/dev/ptmx`)를 사용
- `openpty(3)` 시스템 콜을 내부적으로 사용
- `TIOCSWINSZ` ioctl로 리사이즈 처리
- WezTerm이 macOS에서 활발히 사용 중이므로 검증됨

### 2.2 PTY 리사이즈와 SIGWINCH

#### 리사이즈 플로우

```
사용자가 윈도우 크기 변경
    ↓
Crux 렌더러가 새 크기 감지 (GPUI window resize 이벤트)
    ↓
새 행/열 수 계산 (폰트 크기 기반)
    ↓
master.resize(PtySize { rows, cols, ... })
    ↓ (내부적으로)
ioctl(master_fd, TIOCSWINSZ, &winsize)
    ↓
커널이 SIGWINCH 시그널을 슬레이브 프로세스 그룹에 전송
    ↓
쉘/애플리케이션이 SIGWINCH 수신 → 화면 재그리기
```

#### Rust에서의 구현 참고 (WezTerm unix.rs)

```rust
// WezTerm의 리사이즈 구현 (참고)
fn resize(&self, size: PtySize) -> Result<()> {
    let ws = libc::winsize {
        ws_row: size.rows,
        ws_col: size.cols,
        ws_xpixel: size.pixel_width,
        ws_ypixel: size.pixel_height,
    };
    unsafe {
        libc::ioctl(self.fd, libc::TIOCSWINSZ, &ws);
    }
    Ok(())
}
```

> **참고**: [WezTerm pty/src/unix.rs](https://github.com/wez/wezterm/blob/main/pty/src/unix.rs)

### 2.3 환경 변수 전달

터미널 에뮬레이터가 쉘에 전달해야 하는 주요 환경 변수:

| 환경 변수 | 값 (예시) | 목적 |
|-----------|----------|------|
| `TERM` | `xterm-256color` | 터미널 capabilities 식별 |
| `COLORTERM` | `truecolor` | 24비트 색상 지원 명시 |
| `LANG` | `ko_KR.UTF-8` | 로케일 (CJK 중요) |
| `TERM_PROGRAM` | `Crux` | 터미널 프로그램 식별 |
| `TERM_PROGRAM_VERSION` | `0.1.0` | 버전 정보 |
| `SHELL` | `/bin/zsh` | 기본 쉘 |
| `HOME` | `/Users/jjh` | 홈 디렉토리 |
| `LC_TERMINAL` | `Crux` | iTerm2 호환 식별 |

```rust
let mut cmd = CommandBuilder::new(shell_path);

// 기본 환경 변수 상속
for (key, value) in std::env::vars() {
    cmd.env(key, value);
}

// Crux 전용 오버라이드
cmd.env("TERM", "xterm-256color");
cmd.env("COLORTERM", "truecolor");
cmd.env("TERM_PROGRAM", "Crux");
cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
```

### 2.4 대안: 직접 PTY 구현

`portable-pty`를 사용하지 않고 직접 구현할 경우:

```rust
use std::os::unix::io::RawFd;

// macOS/POSIX PTY 직접 생성
unsafe {
    let mut master: RawFd = 0;
    let mut slave: RawFd = 0;
    let mut ws = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    // openpty 시스템 콜
    libc::openpty(&mut master, &mut slave,
                  std::ptr::null_mut(),
                  std::ptr::null_mut(),
                  &mut ws);

    // fork하여 자식 프로세스에서 쉘 실행
    match libc::fork() {
        0 => {
            // 자식: 새 세션 리더, 슬레이브를 stdin/stdout/stderr로 설정
            libc::setsid();
            libc::dup2(slave, 0);
            libc::dup2(slave, 1);
            libc::dup2(slave, 2);
            libc::close(master);
            libc::close(slave);
            libc::execvp(/* shell */);
        }
        pid => {
            // 부모: 마스터 FD로 I/O
            libc::close(slave);
            // master_fd로 read/write
        }
    }
}
```

> **Crux 권장**: 초기에는 `portable-pty` 사용 (빠른 프로토타이핑), 이후 필요시 직접 구현으로 전환.

---

## 3. 터미널 그래픽스 프로토콜

### 3.1 Kitty Graphics Protocol

- **공식 사양**: [sw.kovidgoyal.net/kitty/graphics-protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
- **지원 터미널**: Kitty, WezTerm, Ghostty, Konsole, Contour 등

#### 프로토콜 형식

```
<ESC>_G<제어 데이터>;<페이로드><ESC>\
```

- **제어 데이터**: 쉼표로 구분된 `key=value` 쌍
- **페이로드**: Base64 인코딩된 바이너리 데이터

#### 핵심 제어 키

| 키 | 설명 | 값 |
|----|------|-----|
| `a` | 액션 | `T`=전송+표시, `p`=기존 이미지 배치, `d`=삭제, `f`=프레임, `a`=애니메이션 |
| `f` | 포맷 | `24`=RGB, `32`=RGBA (기본), `100`=PNG |
| `t` | 전송 방식 | `d`=직접, `f`=파일, `t`=임시파일, `s`=공유메모리 |
| `s`, `v` | 이미지 크기 | 너비/높이 (픽셀) |
| `i` | 이미지 ID | 고유 식별자 (응답 매칭용) |
| `c`, `r` | 표시 크기 | 열/행 (문자 셀 단위) |
| `x`, `y` | 소스 영역 오프셋 | 픽셀 |
| `w`, `h` | 소스 영역 크기 | 픽셀 |
| `z` | Z-인덱스 | 음수: 텍스트 아래, 양수: 텍스트 위 |
| `o` | 압축 | `z`=zlib/deflate |
| `m` | 청크 | `1`=계속, `0`=마지막 청크 |
| `q` | 응답 억제 | `1`=성공만, `2`=실패만 |

#### 이미지 전송 예시 (직접 전송)

```
# 작은 2x2 RGBA 이미지 전송 + 표시
<ESC>_Ga=T,f=32,s=2,v=2;<base64 데이터><ESC>\

# 큰 이미지: 청크 분할 전송
<ESC>_Ga=T,f=100,m=1;<첫 번째 청크 base64><ESC>\
<ESC>_Gm=1;<두 번째 청크 base64><ESC>\
<ESC>_Gm=0;<마지막 청크 base64><ESC>\

# 파일에서 이미지 로드
<ESC>_Ga=T,t=f,f=100;<base64 인코딩된 파일 경로><ESC>\
```

#### 터미널 응답 형식

```
<ESC>_Gi=<id>[,p=<placement_id>];OK<ESC>\        # 성공
<ESC>_Gi=<id>;ENOENT:파일 없음<ESC>\               # 오류
```

#### Rust 구현 스케치

```rust
use base64::{Engine, engine::general_purpose::STANDARD};

fn send_kitty_image(writer: &mut impl Write, png_data: &[u8]) -> io::Result<()> {
    let b64 = STANDARD.encode(png_data);
    let chunk_size = 4096;

    for (i, chunk) in b64.as_bytes().chunks(chunk_size).enumerate() {
        let is_first = i == 0;
        let is_last = (i + 1) * chunk_size >= b64.len();

        if is_first {
            write!(writer, "\x1b_Ga=T,f=100,m={};", if is_last { 0 } else { 1 })?;
        } else {
            write!(writer, "\x1b_Gm={};", if is_last { 0 } else { 1 })?;
        }
        writer.write_all(chunk)?;
        write!(writer, "\x1b\\")?;
    }
    Ok(())
}
```

### 3.2 iTerm2 Image Protocol (OSC 1337)

- **공식 문서**: [iterm2.com/documentation-images.html](https://iterm2.com/documentation-images.html)
- **지원 터미널**: iTerm2, WezTerm, Mintty, Hyper 등

#### 프로토콜 형식

```
# 단일 전송 (원본 방식)
ESC ] 1337 ; File = [args] : <base64 데이터> BEL

# 멀티파트 전송 (iTerm2 3.5+, tmux 호환)
ESC ] 1337 ; MultipartFile = [args] BEL
ESC ] 1337 ; FilePart = <base64 청크> BEL
...
ESC ] 1337 ; FileEnd BEL
```

#### 지원 매개변수

| 매개변수 | 설명 | 기본값 |
|---------|------|--------|
| `name` | Base64 인코딩된 파일명 | "Unnamed file" |
| `size` | 파일 크기 (바이트) | - |
| `width` | 렌더링 너비 | auto |
| `height` | 렌더링 높이 | auto |
| `preserveAspectRatio` | 비율 유지 | 1 (유지) |
| `inline` | 인라인 표시 | 0 (다운로드만) |

크기 단위: 숫자 (문자 셀), `Npx` (픽셀), `N%` (퍼센트), `auto`

#### 지원 포맷

macOS가 지원하는 모든 이미지 포맷: PNG, GIF (애니메이션 포함), JPEG, PDF, PICT, BMP 등

### 3.3 Sixel

- **위키**: [en.wikipedia.org/wiki/Sixel](https://en.wikipedia.org/wiki/Sixel)
- **호환성**: [arewesixelyet.com](https://www.arewesixelyet.com/)
- **라이브러리**: [libsixel](https://saitoha.github.io/libsixel/)

#### 개요

DEC VT 시리즈에서 유래한 레거시 비트맵 그래픽스 형식. 이미지를 6픽셀 높이의 수평 스트립으로 분할하여 인코딩한다.

#### 현재 지원 상황 (2025년)

| 터미널 | Sixel 지원 |
|--------|-----------|
| XTerm | O (VT340 모드) |
| tmux | O (--enable-sixel 빌드 옵션) |
| VS Code Terminal | O (1.80+) |
| Foot | O |
| Contour | O |
| WezTerm | O |
| Alacritty | X |
| Ghostty | X (계획 중) |

#### 장단점

- **장점**: 가장 넓은 호환성, tmux 공식 지원, CLI 도구 풍부
- **단점**: 256색 제한, 인코딩 비효율적, 현대적 기능 부족 (알파 채널 없음)

### 3.4 Crux 그래픽스 구현 권장 순서

| 우선순위 | 프로토콜 | 이유 |
|---------|---------|------|
| **1순위** | Kitty Graphics Protocol | 가장 현대적, 기능 풍부, 주요 터미널 채택 증가 |
| **2순위** | iTerm2 (OSC 1337) | macOS 사용자에게 익숙, imgcat 호환 |
| **3순위** | Sixel | 레거시 호환, tmux passthrough |

---

## 4. tmux 호환성

### 4.1 tmux가 요구하는 VT100/xterm 기능

tmux는 호스트 터미널에 다음 기능을 요구한다:

#### 필수 기능

| 기능 | 설명 | 이스케이프 시퀀스 |
|------|------|------------------|
| **기본 커서 이동** | 상하좌우 | `CSI n A/B/C/D` |
| **커서 위치 설정** | 절대 위치 | `CSI row;col H` |
| **화면 지우기** | 전체/부분 | `CSI n J`, `CSI n K` |
| **스크롤 영역** | 상/하 마진 | `CSI top;bottom r` (DECSTBM) |
| **문자 속성** | 볼드, 색상 등 | `CSI n m` (SGR) |
| **TERM 설정** | screen/tmux 계열 | `TERM=tmux-256color` |

#### 고급 기능

| 기능 | 설명 | tmux 옵션 |
|------|------|-----------|
| **좌우 마진** (VT420) | 수평 분할 최적화 | `DECLRMM` |
| **xterm 확장 키** | 수정키 조합 | `extended-keys` |
| **포커스 이벤트** | 윈도우 포커스 알림 | `focus-events on` |

### 4.2 256 색상 및 True Color

#### 256 색상

```bash
# tmux 내부 TERM 설정
set -g default-terminal "tmux-256color"

# 터미널이 지원하는 경우
# CSI 38;5;n m  (전경색, n=0-255)
# CSI 48;5;n m  (배경색, n=0-255)
```

#### True Color (24비트) Passthrough

```bash
# tmux 3.2+ terminal-features 옵션
set -as terminal-features ",xterm-256color:RGB"

# 또는 tmux 확장 (구버전 호환)
set -ag terminal-overrides ",xterm-256color:Tc"
```

Crux는 `COLORTERM=truecolor` 환경 변수와 함께 `Tc` / `RGB` terminfo 플래그를 모두 지원해야 한다.

#### True Color 이스케이프 시퀀스

```
CSI 38;2;r;g;b m   # 전경색 (24비트 RGB)
CSI 48;2;r;g;b m   # 배경색 (24비트 RGB)
```

### 4.3 마우스 이벤트

tmux는 `set -g mouse on` 으로 마우스를 활성화하며, 터미널에 다음 모드를 요청한다:

| 모드 | 시퀀스 | 설명 |
|------|--------|------|
| **Normal tracking** | `CSI ? 1000 h` | 버튼 클릭 보고 |
| **Button tracking** | `CSI ? 1002 h` | 드래그 보고 |
| **Any event** | `CSI ? 1003 h` | 모든 마우스 이동 보고 |
| **SGR encoding** | `CSI ? 1006 h` | 확장 좌표 인코딩 (223열 이상 지원) |
| **UTF-8 encoding** | `CSI ? 1005 h` | UTF-8 좌표 인코딩 |

**SGR 마우스 이벤트 형식** (Crux 필수 구현):

```
CSI < button;col;row M    # 버튼 누름
CSI < button;col;row m    # 버튼 놓음
```

### 4.4 Bracketed Paste Mode

터미널이 붙여넣기를 할 때 시작/종료 마커로 감싸는 기능. 에디터가 붙여넣기와 타이핑을 구분할 수 있게 한다.

```
# 활성화 요청
CSI ? 2004 h

# 비활성화 요청
CSI ? 2004 l

# 붙여넣기 시 터미널이 전송:
CSI 200 ~   <붙여넣기 데이터>   CSI 201 ~
```

### 4.5 포커스 이벤트

```bash
# tmux에서 활성화
set -g focus-events on
```

```
# 터미널이 포커스 이벤트 모드 활성화 요청
CSI ? 1004 h

# 포커스 획득 시 터미널이 전송
CSI I

# 포커스 상실 시 터미널이 전송
CSI O
```

Neovim, Vim 등이 포커스 이벤트를 활용하여 자동 리로드, 상태 업데이트 등을 수행한다.

### 4.6 tmux Control Mode (-CC)

- **문서**: [tmux Control Mode Wiki](https://github.com/tmux/tmux/wiki/Control-Mode)
- **구현 예시**: iTerm2 (유일한 완전 구현체)

#### 개요

tmux control mode는 터미널 에뮬레이터가 tmux와 프로그래밍 방식으로 통신하기 위한 텍스트 기반 프로토콜이다. George Nachman (iTerm2 개발자)이 설계했다.

#### 진입 방식

```bash
# 단일 -C: 테스트용 (에코 활성)
tmux -C new-session

# 이중 -CC: 애플리케이션용 (canonical mode 비활성)
tmux -CC new-session
# → \033P1000p DSC 시퀀스 전송 (터미널 감지용)
```

#### 프로토콜 구조

**명령 응답 형식**:
```
%begin <타임스탬프> <명령번호> <플래그>
<출력>
%end <타임스탬프> <명령번호> <플래그>
```

오류 시:
```
%begin <타임스탬프> <명령번호> <플래그>
<에러 메시지>
%error <타임스탬프> <명령번호> <플래그>
```

#### 비동기 알림

| 알림 | 설명 |
|------|------|
| `%output %pane content` | 패인 출력 데이터 |
| `%window-add @window` | 윈도우 생성 |
| `%window-close @window` | 윈도우 닫힘 |
| `%window-renamed @window name` | 윈도우 이름 변경 |
| `%session-changed $session name` | 세션 변경 |
| `%pane-mode-changed %pane` | 패인 모드 변경 |
| `%pause %pane` | 플로우 컨트롤 일시정지 |
| `%continue %pane` | 플로우 컨트롤 재개 |

#### Crux에서의 활용

tmux control mode를 구현하면:
- tmux 패인/윈도우를 Crux의 네이티브 탭/분할로 매핑
- tmux 세션을 Crux UI에서 직접 관리
- SSH 원격 tmux 세션의 투명한 통합
- iTerm2와 동일한 수준의 tmux 통합 경험

**구현 난이도**: 높음. iTerm2 외에 완전 구현한 터미널이 없음. 장기 목표로 설정 권장.

---

## 5. 유니코드/CJK 처리

### 5.1 문자 폭 계산 (wcwidth)

#### unicode-width 크레이트

- **최신 버전**: (최신 안정)
- **크레이트**: [crates.io/crates/unicode-width](https://crates.io/crates/unicode-width)
- **문서**: [docs.rs/unicode-width](https://docs.rs/unicode-width/latest/unicode_width/)

```rust
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// 기본 폭 계산
assert_eq!('A'.width(), Some(1));    // ASCII
assert_eq!('가'.width(), Some(2));   // 한글 (Wide)
assert_eq!('é'.width(), Some(1));    // 악센트 문자
assert_eq!('\0'.width(), Some(0));   // 제어 문자

// CJK 컨텍스트 (Ambiguous → 2칸)
assert_eq!('★'.width_cjk(), Some(2)); // CJK 모드에서 2칸
assert_eq!('★'.width(), Some(1));     // 비CJK 모드에서 1칸

// 문자열 폭
assert_eq!("Hello".width(), 5);
assert_eq!("안녕하세요".width(), 10);
assert_eq!("Hello안녕".width(), 9);
```

#### 기능 플래그

```toml
[dependencies]
unicode-width = { version = "0.2", features = ["cjk"] }  # CJK 기본 활성화
# 또는
unicode-width = { version = "0.2", default-features = false }  # CJK 비활성화 (크기 최소화)
```

#### 주의사항

- **Ambiguous 카테고리**: CJK 로케일에서 2칸, 비CJK에서 1칸 → Crux는 사용자 설정 필요
- **실제 렌더링과 차이 가능**: 폰트에 따라 렌더링 폭이 다를 수 있음
- **결합 문자**: 잘못된 combining sequence는 예상과 다른 폭을 가질 수 있음

### 5.2 그래핌 클러스터 처리

#### 문제점

전통적인 `wcwidth`는 개별 코드포인트 단위로 폭을 계산한다. 하지만 사용자가 인식하는 하나의 "문자"(grapheme)는 여러 코드포인트로 구성될 수 있다.

```
🧑‍🌾 (농부 이모지) = U+1F9D1 + U+200D (ZWJ) + U+1F33E
  wcwidth 방식: 2 + 0 + 2 = 4칸 ❌
  올바른 폭:    2칸 ✅
```

#### 터미널별 렌더링 차이 (Mitchell Hashimoto 조사)

| 터미널 | 🧑‍🌾 렌더링 폭 |
|--------|------------|
| Ghostty (Mode 2027) | 2칸 |
| WezTerm | 2칸 |
| iTerm2 | 2칸 |
| Foot | 2칸 |
| **Alacritty** | **4칸** |
| **Kitty** | **4칸** |
| tmux | 4칸 |
| Terminal.app | 5-6칸 (비정상) |

> **참고**: [Grapheme Clusters and Terminal Emulators](https://mitchellh.com/writing/grapheme-clusters-in-terminals)

#### unicode-segmentation 크레이트

- **최신 버전**: `1.9.0`
- **크레이트**: [crates.io/crates/unicode-segmentation](https://crates.io/crates/unicode-segmentation)
- **표준**: UAX #29 (Unicode Text Segmentation)

```rust
use unicode_segmentation::UnicodeSegmentation;

// 그래핌 클러스터 분할
let graphemes: Vec<&str> = "a̐éö̲\r\n".graphemes(true).collect();
// → ["a̐", "é", "ö̲", "\r\n"]

// 인덱스 포함
let indices: Vec<(usize, &str)> = "안녕🧑‍🌾".grapheme_indices(true).collect();
// → [(0, "안"), (3, "녕"), (6, "🧑‍🌾")]

// ZWJ 이모지 시퀀스
let family = "👨‍👩‍👧‍👦";
let count = family.graphemes(true).count();
// → 1 (하나의 그래핌 클러스터)
```

#### Mode 2027 (그래핌 클러스터 모드)

프로그램이 터미널의 그래핌 클러스터 지원을 쿼리하는 표준 제안:

```
# 지원 여부 쿼리
CSI ? 2027 $ p

# 응답 (지원하는 경우)
CSI ? 2027 ; 1 $ y

# 응답 (미지원)
CSI ? 2027 ; 2 $ y
```

**Crux 권장**: Mode 2027을 지원하여, 이를 인식하는 프로그램에게 올바른 그래핌 클러스터 처리를 보장.

### 5.3 이모지 렌더링

#### ZWJ (Zero Width Joiner) 시퀀스

```
👨‍👩‍👧‍👦 = U+1F468 + U+200D + U+1F469 + U+200D + U+1F467 + U+200D + U+1F466
(7 코드포인트, 1개 그래핌, 렌더링 폭 2칸)
```

#### Variation Selector

```
☺    (U+263A)         텍스트 스타일
☺️   (U+263A + U+FE0F) 이모지 스타일 (VS16)
```

#### 렌더링 파이프라인

```
바이트 스트림 → 코드포인트 디코딩 → 그래핌 클러스터링 (UAX #29)
    → 폭 계산 (unicode-width + 보정) → 폰트 셰이핑 (HarfBuzz)
    → 글리프 래스터라이징 → GPU 렌더링
```

### 5.4 CJK 폰트 폴백

#### macOS CoreText 활용

```
CTFontCopyDefaultCascadeListForLanguages(font, ["ko", "ja", "zh-Hans"])
```

이 API는 Han Unification을 고려하여 선호 언어 순서대로 CJK 폰트를 정렬한다.

#### Han Unification 문제

같은 코드포인트(예: U+9AA8 "骨")라도 한국어/일본어/중국어에서 다른 글리프로 렌더링되어야 한다. 폰트 폴백 리스트는 단순한 폰트 목록이 아니라 **언어 메타데이터**를 포함해야 한다.

#### 주요 터미널의 구현 방식

- **Kitty**: CoreText를 직접 사용하여 플랫폼 네이티브 폰트 탐색
- **WezTerm**: `font_with_fallback()` 설정으로 수동 폴백 체인 + CJK 스케일링 팩터 지원
- **Ghostty**: CoreText 기반이지만 CJK 폰트 메트릭 추정 문제 있음 (이슈 [#8712](https://github.com/ghostty-org/ghostty/issues/8712))

#### Crux 권장 구현

```rust
// 의사코드: 폰트 폴백 체인
struct FontFallback {
    primary: Font,           // 사용자 지정 기본 폰트
    cjk_ko: Font,            // 한국어 CJK (예: Apple SD Gothic Neo)
    cjk_ja: Font,            // 일본어 CJK (예: Hiragino Sans)
    cjk_zh_hans: Font,       // 중국어 간체 (예: PingFang SC)
    emoji: Font,             // 이모지 (Apple Color Emoji)
    symbols: Font,           // 기호/아이콘 (SF Symbols, Nerd Font)
}

// 글리프 탐색 순서:
// 1. 기본 폰트에서 검색
// 2. 코드포인트가 CJK 범위 → 로케일 기반 CJK 폰트
// 3. 이모지 범위 → 이모지 폰트
// 4. CoreText 폴백 캐스케이드
```

### 5.5 관련 Rust 크레이트 요약

| 크레이트 | 버전 | 용도 |
|---------|------|------|
| `unicode-width` | 0.2.x | 문자 폭 계산 (wcwidth) |
| `unicode-segmentation` | 1.9.0 | 그래핌 클러스터 분할 (UAX #29) |
| `runefix-core` | 최신 | 이모지+CJK 통합 폭 계산 (실험적) |
| `harfbuzz_rs` | - | 폰트 셰이핑 (HarfBuzz 바인딩) |
| `core-text` | - | macOS CoreText 바인딩 |

---

## 6. 스크롤백 버퍼

### 6.1 설계 접근 방식

#### 방식 1: 순환 버퍼 (Circular Buffer)

**사용**: Windows Terminal, 대부분의 전통적 터미널

```
[Page 3][Page 4][Page 5][Page 0][Page 1][Page 2]
                                  ↑ _firstRow
```

- `_firstRow` 인덱스로 논리적 행 0을 추적
- 새 행 추가 시 가장 오래된 행을 덮어씀
- 데이터 복사 없이 스크롤 가능

**장점**:
- O(1) 행 추가/삭제
- 메모리 사용량 예측 가능 (고정 크기)
- 구현 간단

**단점**:
- 크기 변경 시 전체 재할당 필요
- 행 길이가 가변적인 경우 메모리 낭비

#### 방식 2: 이중 연결 리스트 페이지 (Doubly Linked List of Pages)

**사용**: Ghostty (PageList)

```
[Page A] ←→ [Page B] ←→ [Page C] ←→ [Page D]
 (표준)      (비표준)     (표준)      (표준)
```

각 페이지는 mmap으로 할당된 메모리 블록이며, 문자, 스타일, 하이퍼링크 등을 저장한다.

**장점**:
- 페이지 단위 할당/해제로 유연한 메모리 관리
- 비표준 크기 페이지 지원 (복잡한 그래핌 저장)
- 다른 기능 (검색, 선택) 구현에 유리한 구조

**단점**:
- 포인터 오버헤드
- 캐시 미스 가능성 (비연속 메모리)
- **메모리 누수 주의**: Ghostty에서 비표준 페이지 재사용 시 메모리 누수 발생 이력

> **참고**: [Finding and Fixing Ghostty's Largest Memory Leak](https://mitchellh.com/writing/ghostty-memory-leak-fix)

#### Ghostty 메모리 누수 교훈

Ghostty에서 스크롤백 가지치기(pruning) 시 오래된 페이지를 새 페이지로 재사용하는 최적화가 있었다. 문제는 **비표준 크기 페이지**(복잡한 그래핌으로 인해 확장된)를 표준 크기로 메타데이터만 리셋하고 실제 mmap은 그대로 둔 것. 이후 해제 시 풀에서 온 것으로 판단하여 munmap을 호출하지 않아 영구적 메모리 누수 발생.

**해결책**: 비표준 페이지는 재사용하지 않고 파괴 후 새 표준 페이지를 할당.

**Crux 교훈**: 페이지 기반 구현 시 표준/비표준 페이지를 명확히 구분하고, 재사용 최적화는 동일 크기일 때만 적용.

### 6.2 검색 기능

스크롤백 내 텍스트 검색은 터미널 에뮬레이터의 핵심 기능이다.

#### alacritty_terminal의 검색 API

```rust
// alacritty_terminal Term에 내장된 검색
impl Term {
    fn search_next(&self, regex: &RegexSearch, direction: Direction)
        -> Option<Match>;
    fn regex_search_left(&self, ...) -> Option<Match>;
    fn regex_search_right(&self, ...) -> Option<Match>;
    fn semantic_search_left(&self, point: Point) -> Point;
    fn semantic_search_right(&self, point: Point) -> Point;
    fn bracket_search(&self, point: Point) -> Option<Point>;
}
```

`regex-automata` 크레이트를 사용하여 정규식 검색을 지원한다.

#### 효율적인 검색을 위한 고려사항

1. **역방향 검색**: 스크롤백의 최근 부분부터 검색 (사용자 기대)
2. **점진적 검색**: 타이핑할 때마다 결과 업데이트
3. **래핑된 행 처리**: 논리적 줄 단위로 검색 (물리적 행이 아닌)
4. **유니코드 정규화**: NFD/NFC 차이를 고려한 매칭

### 6.3 메모리 관리 전략

| 전략 | 설명 | 적합한 경우 |
|------|------|-----------|
| **고정 크기** | N줄로 제한, 초과 시 삭제 | 메모리 예측 필요 시 |
| **무제한** | 모든 출력 보존 | 파워 유저 |
| **디스크 스왑** | 임계값 초과 시 디스크에 저장 | 대용량 로그 |
| **압축** | 오래된 행 압축 | 메모리 효율 |

#### WezTerm의 접근 (참고)

> "The larger the scrollback buffer value, the more memory is required to manage the tab." — [WezTerm Scrollback](https://wezterm.org/scrollback.html)

WezTerm은 기본 3500줄의 스크롤백을 제공하며, 사용자가 무제한으로 설정할 수 있다.

#### Crux 권장 구현

```rust
struct ScrollbackConfig {
    max_lines: Option<usize>,  // None = 무제한
    compress_after: usize,     // N줄 이후 압축 (선택적)
}

// 기본값 제안
impl Default for ScrollbackConfig {
    fn default() -> Self {
        Self {
            max_lines: Some(10_000),
            compress_after: 50_000, // 무제한 모드에서만 활성
        }
    }
}
```

---

## 7. Crux를 위한 권장사항 요약

### 핵심 의존성 스택

```toml
[dependencies]
# VT 파싱 + 터미널 상태
alacritty_terminal = "0.25"

# PTY 관리
portable-pty = "0.9"

# 유니코드
unicode-width = "0.2"
unicode-segmentation = "1.9"

# 이미지 (Kitty Protocol 구현용)
base64 = "0.22"
png = "0.17"
image = "0.25"

# 정규식 (스크롤백 검색)
regex-automata = "0.4"   # alacritty_terminal에 포함
```

### 구현 우선순위

| 단계 | 기능 | 핵심 크레이트/기술 |
|------|------|------------------|
| **Phase 1** | 기본 VT 에뮬레이션 | alacritty_terminal |
| **Phase 1** | PTY 생성/관리 | portable-pty |
| **Phase 1** | 기본 텍스트 렌더링 | unicode-width |
| **Phase 2** | 256색 + True Color | SGR 시퀀스 |
| **Phase 2** | 마우스 이벤트 | SGR 마우스 인코딩 |
| **Phase 2** | Bracketed Paste | CSI 2004 |
| **Phase 2** | 포커스 이벤트 | CSI 1004 |
| **Phase 2** | 스크롤백 + 검색 | 순환 버퍼 + regex |
| **Phase 3** | CJK/이모지 완전 지원 | unicode-segmentation, Mode 2027 |
| **Phase 3** | 폰트 폴백 체인 | CoreText |
| **Phase 3** | Kitty Graphics | 직접 구현 |
| **Phase 4** | iTerm2 이미지 프로토콜 | OSC 1337 |
| **Phase 4** | Sixel 지원 | libsixel 또는 직접 구현 |
| **Phase 5** | tmux Control Mode | 직접 구현 (장기 목표) |

### 핵심 아키텍처 결정

1. **VT 파서**: `alacritty_terminal` 채택 (완성도 높음, Damage tracking 유용)
2. **PTY**: `portable-pty` 시작, 필요시 직접 구현으로 전환
3. **그래핌 클러스터**: Mode 2027 지원 → 올바른 이모지/CJK 렌더링
4. **스크롤백**: 순환 버퍼 기반으로 시작, 검색 기능은 alacritty_terminal 내장 활용
5. **그래픽스**: Kitty Protocol 우선 구현, iTerm2/Sixel 후순위
6. **tmux**: 기본 VT 호환 먼저, Control Mode는 장기 목표

---

## 참고 자료

### VT 파서
- [alacritty_terminal crates.io](https://crates.io/crates/alacritty_terminal)
- [alacritty_terminal docs.rs](https://docs.rs/alacritty_terminal/latest/alacritty_terminal/)
- [vte crates.io](https://crates.io/crates/vte)
- [vte GitHub (alacritty/vte)](https://github.com/alacritty/vte)
- [vt100 crates.io](https://crates.io/crates/vt100)
- [Libghostty Is Coming - Mitchell Hashimoto](https://mitchellh.com/writing/libghostty-is-coming)
- [ghostty-vt PR #8840](https://github.com/ghostty-org/ghostty/pull/8840)

### PTY
- [portable-pty docs.rs](https://docs.rs/portable-pty/latest/portable_pty/)
- [WezTerm pty/src/unix.rs](https://github.com/wez/wezterm/blob/main/pty/src/unix.rs)
- [Playing with SIGWINCH](https://www.rkoucha.fr/tech_corner/sigwinch.html)

### 그래픽스 프로토콜
- [Kitty Graphics Protocol 공식 사양](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
- [iTerm2 Images Documentation](https://iterm2.com/documentation-images.html)
- [Are We Sixel Yet?](https://www.arewesixelyet.com/)
- [libsixel](https://saitoha.github.io/libsixel/)

### tmux
- [tmux FAQ Wiki](https://github.com/tmux/tmux/wiki/FAQ)
- [tmux Control Mode Wiki](https://github.com/tmux/tmux/wiki/Control-Mode)
- [iTerm2 tmux Integration](https://iterm2.com/documentation-tmux-integration.html)

### 유니코드
- [unicode-width crates.io](https://crates.io/crates/unicode-width)
- [unicode-segmentation crates.io](https://crates.io/crates/unicode-segmentation)
- [Grapheme Clusters and Terminal Emulators - Mitchell Hashimoto](https://mitchellh.com/writing/grapheme-clusters-in-terminals)
- [runefix-core GitHub](https://github.com/runefix-labs/runefix-core)
- [Font Fallback Deep Dive - Raph Levien](https://raphlinus.github.io/rust/skribo/text/2019/04/04/font-fallback.html)

### 스크롤백
- [WezTerm Scrollback](https://wezterm.org/scrollback.html)
- [Finding and Fixing Ghostty's Largest Memory Leak](https://mitchellh.com/writing/ghostty-memory-leak-fix)
- [Text Buffer System - Windows Terminal DeepWiki](https://deepwiki.com/microsoft/terminal/2.2-text-buffer-system)
