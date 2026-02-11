---
title: "GPUI 프레임워크 연구"
description: "GPUI framework deep-dive: rendering pipeline, component system, IME support, limitations, and implications for Crux"
date: 2026-02-11
phase: [1, 2, 3]
topics: [gpui, metal, rendering, component-system, ime]
status: final
related:
  - terminal-implementations.md
  - bootstrap.md
  - ../core/terminal-architecture.md
---

# GPUI 프레임워크 연구 보고서

> Crux 터미널 에뮬레이터 프로젝트를 위한 GPUI 기술 조사
> 작성일: 2026-02-11

---

## 목차

1. [GPUI 프레임워크 개요](#1-gpui-프레임워크-개요)
2. [UI 컴포넌트 시스템](#2-ui-컴포넌트-시스템)
3. [IME 지원 및 입력 처리](#3-ime-지원-및-입력-처리)
4. [터미널 통합](#4-터미널-통합)
5. [제한사항 및 고려사항](#5-제한사항-및-고려사항)
6. [Crux 프로젝트에 대한 시사점](#6-crux-프로젝트에-대한-시사점)
7. [참고 자료](#7-참고-자료)

---

## 1. GPUI 프레임워크 개요

### 1.1 GPUI란?

GPUI는 Zed 에디터 개발팀이 만든 **GPU 가속 UI 프레임워크**로, Rust로 작성되었다. 즉각적(immediate) 모드와 유보적(retained) 모드를 혼합한 하이브리드 렌더링 모델을 채택하여, 선언적 UI 구성의 편의성과 GPU 가속의 고성능을 동시에 달성한다.

- **라이선스**: Apache 2.0
- **현재 버전**: 0.2.2 (pre-1.0, 활발한 개발 중)
- **crates.io**: [gpui](https://crates.io/crates/gpui)
- **공식 사이트**: [gpui.rs](https://www.gpui.rs/)

### 1.2 현재 상태 및 안정성

GPUI는 Zed 에디터의 핵심 구성요소로서 **프로덕션에서 실사용** 중이지만, 독립 크레이트로는 아직 pre-1.0 상태이다.

| 항목 | 상태 |
|------|------|
| 프로덕션 사용 | Zed 에디터에서 매일 사용 중 |
| API 안정성 | 버전 간 Breaking Change 빈번 |
| 문서화 | 공식 docs.rs + 비공식 튜토리얼 존재, 부족한 편 |
| crates.io 배포 | v0.2.2 (2024년~) |
| 독립 앱 지원 | create-gpui-app 스캐폴딩 도구 제공 |

> **주의**: "GPUI is still in active development as we work on the Zed code editor, and is still pre-1.0. There will often be breaking changes between versions." — [GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)

### 1.3 독립 GPUI 애플리케이션 만들기

Zed 외부에서 독립 GPUI 앱을 만드는 방법:

**방법 1: create-gpui-app (공식 스캐폴딩)**

```bash
cargo install create-gpui-app
create-gpui-app --name my-app
cd my-app
cargo run
```

생성되는 프로젝트 구조:
```
my-app/
├── Cargo.toml
├── README.md
└── crates/
    └── my-app/
        ├── Cargo.toml
        └── src/
            └── main.rs
```

**방법 2: 수동 설정 (gpui-component 사용)**

```toml
# Cargo.toml
[dependencies]
gpui = "0.2.2"
gpui-component = "0.5.1"
gpui-component-assets = "0.5.1"  # 선택적 - 기본 아이콘
```

**기본 애플리케이션 구조:**

```rust
use gpui::*;
use gpui_component::{button::*, *};

pub struct MyApp;

impl Render for MyApp {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()        // 수직 플렉스 레이아웃
            .gap_2()         // 간격
            .size_full()     // 전체 크기
            .items_center()  // 중앙 정렬
            .child("Hello, Crux!")
            .child(
                Button::new("ok")
                    .primary()
                    .label("시작")
                    .on_click(|_, _, _| println!("클릭!"))
            )
    }
}

fn main() {
    let app = Application::new().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);  // 필수 초기화

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|_| MyApp);
                cx.new(|cx| Root::new(view, window, cx))
            })?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
```

**핵심 흐름**: `Application::new()` → `Application::run()` → `App::open_window()` → Root View 등록

### 1.4 Metal 렌더링 (macOS)

GPUI는 macOS에서 **Metal**을 사용하여 GPU 가속 렌더링을 수행한다. 이 아키텍처는 게임 엔진의 렌더링 패턴을 차용하여 **120 FPS**를 목표로 한다.

#### 렌더링 파이프라인

```
상태 업데이트 → 레이아웃 계산(Taffy) → Scene 생성 → Metal 커맨드 버퍼 → GPU 렌더링
     ↓                  ↓                    ↓                ↓
  Entity 변경      Prepaint Phase       Paint Phase      Drawable + VSync
```

#### 렌더링 프리미티브

| 프리미티브 | 렌더링 방법 | 최적화 |
|-----------|------------|--------|
| 사각형 | SDF (Signed Distance Function) | 인스턴스 렌더링 |
| 그림자 | Gaussian 오류 함수 직접 계산 | 블러 샘플링 불필요 |
| 텍스트 | 글리프 아틀라스 + 서브픽셀 위치 지정 | 16개 서브픽셀 변형 캐싱 |
| 아이콘 | SVG → CPU 래스터화 (alpha only) | 단일 색상 곱셈 |
| 이미지 | 풀 컬러 텍스처 업로드 | - |

#### Scene 배칭 및 그리기 순서

```
그림자 → 사각형 → 글리프(텍스트) → 아이콘 → 이미지
```

스태킹 컨텍스트를 통해 Painter's Algorithm으로 임의 z-index 위치를 지원한다.

#### 텍스트 렌더링 3단계

| 단계 | 입력 | 출력 | 구현 |
|------|------|------|------|
| **셰이핑** | String + Font | LineLayout | 플랫폼 텍스트 API (macOS: CoreText) |
| **캐싱** | GlyphId + Font | Atlas 좌표 | PlatformAtlas |
| **렌더링** | Sprite + Position | Scene 커맨드 | paint_glyph() |

- `WindowTextSystem`은 윈도우별 글리프 캐시를 관리
- **서브픽셀 위치 지정**: 4x2 = 8가지 변형을 캐싱하여 선명한 텍스트 렌더링
- **아틀라스**: 알파 채널만 저장하여 메모리 절약, 어떤 색상이든 단일 글리프 복사본으로 렌더링

### 1.5 성능 특성

| 항목 | 값/설명 |
|------|---------|
| 목표 프레임레이트 | **120 FPS** (8.33ms/프레임) |
| 렌더링 방식 | GPU 가속 (Metal/Vulkan/DirectX) |
| 레이아웃 엔진 | Taffy (Flexbox 기반) |
| 텍스트 셰이핑 | 프레임 간 캐싱으로 비용 분산 |
| 인스턴스 렌더링 | 단일 드로 콜로 다수 UI 요소 렌더링 |
| GC 없음 | Rust 소유권 모델로 GC 일시정지 없음 |

### 1.6 플랫폼 추상화

GPUI는 `Platform` 트레이트를 통해 다양한 OS를 추상화한다:

| 플랫폼 | 윈도우 API | 렌더러 | 텍스트 시스템 |
|--------|-----------|--------|-------------|
| **macOS** | Cocoa NSWindow | Metal | Core Text |
| **Linux** | X11 XCB / Wayland | Blade (Vulkan) | Cosmic Text |
| **Windows** | Win32 HWND | DirectX 11 | DirectWrite |

### 1.7 아키텍처: 3계층 시스템

GPUI는 세 가지 계층으로 구성된다:

**1. Entity 기반 상태 관리**
- `Rc`와 유사한 소유 스마트 포인터로 애플리케이션 상태를 관리
- GPUI가 직접 관리하며 컴포넌트 간 통신의 기반

**2. 고수준 선언적 View**
- `Render` 트레이트를 구현하여 매 프레임마다 UI 트리를 재구성
- Tailwind 스타일의 레이아웃/스타일링 API 제공
- `div()`가 범용 빌딩 블록 역할

**3. 저수준 명령적 Element**
- 렌더링과 레이아웃에 대한 세밀한 제어 제공
- 커스텀 레이아웃 (코드 에디터 등), 리스트 가상화에 적합

---

## 2. UI 컴포넌트 시스템

### 2.1 내장 레이아웃 시스템

GPUI는 **Taffy** 레이아웃 엔진을 통합하여 CSS Flexbox와 유사한 레이아웃을 지원한다.

```rust
div()
    .flex()                    // display: flex
    .flex_row()                // flex-direction: row
    .flex_col()                // flex-direction: column (= v_flex())
    .gap_2()                   // gap: 0.5rem
    .items_center()            // align-items: center
    .justify_center()          // justify-content: center
    .p_4()                     // padding: 1rem
    .size_full()               // width: 100%; height: 100%
    .overflow_hidden()         // overflow: hidden
    .rounded_md()              // border-radius: medium
    .bg(cx.theme().background) // background-color
```

레이아웃 처리 흐름:
1. `Window::request_layout()` → Taffy에 레이아웃 요청
2. **Prepaint Phase**: 제약조건이 부모→자식으로 전파
3. 크기가 자식→부모로 역전파
4. **Paint Phase**: 계산된 위치에 GPU 커맨드 생성

### 2.2 gpui-component 라이브러리

[gpui-component](https://github.com/longbridge/gpui-component)는 Longbridge에서 개발한 **60개 이상의 UI 컴포넌트** 라이브러리이다.

- **공식 문서**: https://longbridge.github.io/gpui-component/
- **현재 버전**: 0.5.1
- **특징**: 가상화 테이블/리스트, Markdown 지원, 차트, 20+ 테마, 다크 모드

#### 전체 컴포넌트 목록

**기본 컴포넌트:**

| 컴포넌트 | 설명 |
|---------|------|
| Accordion | 접을 수 있는 콘텐츠 패널 |
| Alert | 알림 메시지 |
| Avatar | 사용자 아바타 |
| Badge | 카운트 배지 |
| Button | 다양한 변형을 가진 버튼 |
| Checkbox | 체크박스 |
| Collapsible | 접기/펼치기 콘텐츠 |
| DropdownButton | 드롭다운 메뉴가 있는 버튼 |
| Icon | 아이콘 |
| Image | 이미지 표시 |
| Kbd | 키보드 단축키 표시 |
| Label | 레이블 |
| Progress | 진행 표시줄 |
| Radio | 라디오 버튼 |
| Skeleton | 로딩 플레이스홀더 |
| Slider | 범위 선택 슬라이더 |
| Spinner | 로딩 스피너 |
| Switch | 토글 스위치 |
| Tag | 태그/라벨 |
| Toggle | 토글 버튼 |
| Tooltip | 호버 툴팁 |

**폼 컴포넌트:**

| 컴포넌트 | 설명 |
|---------|------|
| Input | 텍스트 입력 필드 |
| Select | 드롭다운 선택 |
| NumberInput | 숫자 입력 |
| DatePicker | 날짜 선택기 |
| OtpInput | OTP 입력 |
| ColorPicker | 색상 선택기 |
| Editor | 멀티라인 텍스트/코드 에디터 |
| Form | 폼 컨테이너 |

**레이아웃 컴포넌트:**

| 컴포넌트 | 설명 |
|---------|------|
| DescriptionList | 키-값 쌍 표시 |
| GroupBox | 그룹화된 콘텐츠 |
| Dialog | 다이얼로그/모달 |
| Notification | 토스트 알림 |
| Popover | 플로팅 콘텐츠 |
| **Resizable** | **리사이즈 가능한 패널** |
| Scrollable | 스크롤 컨테이너 |
| Sheet | 슬라이드인 패널 |
| Sidebar | 네비게이션 사이드바 |

**고급 컴포넌트:**

| 컴포넌트 | 설명 |
|---------|------|
| Calendar | 캘린더 |
| Chart | 차트 (Line, Bar, Area, Pie, Candlestick) |
| List | 리스트 |
| Menu | 메뉴, 컨텍스트 메뉴 |
| Settings | 설정 UI |
| **Table** | **고성능 데이터 테이블 (가상화)** |
| **Tabs** | **탭 인터페이스** |
| Tree | 계층 트리 |
| **VirtualList** | **대량 데이터용 가상화 리스트** |
| WebView | 내장 웹 브라우저 |

### 2.3 탭 (Tabs) 구현

gpui-component의 탭 시스템은 다음으로 구성된다:

- **Tab**: 개별 탭 요소
- **TabBar**: 다수의 탭을 담는 컨테이너
- **TabPanel**: Dock 레이아웃 내 탭 지원 패널

`Selectable` 트레이트를 구현하여 활성 탭 상태를 관리한다.

### 2.4 분할 패널 레이아웃 (Dock/Resizable)

gpui-component의 **DockArea** 컴포넌트는 IDE 스타일의 복잡한 패널 레이아웃을 지원한다:

```
DockArea
├── DockItem::Split      # 수평/수직 분할 + 리사이즈 가능한 구분선
├── DockItem::Tabs       # 탭 인터페이스로 다수 패널
├── DockItem::Panel      # 단일 패널
└── DockItem::Tiles      # 자유형 캔버스 (드래그 & 리사이즈)
```

| 컴포넌트 | 역할 |
|---------|------|
| DockArea | 패널 레이아웃 전체를 관리하는 메인 컨테이너 |
| TabPanel | 탭이 있는 패널 |
| StackPanel | 스택 형태 패널 |
| ResizablePanel | 크기 조절 가능한 패널 |

초기화: `dock::init(cx)` 호출 필요

> **Crux에 대한 시사점**: DockArea + Tabs + ResizablePanel 조합으로 터미널 분할 패널 레이아웃을 구현할 수 있다. 이미 IDE 수준의 패널 관리가 가능하므로 별도 구현 없이 활용 가능.

### 2.5 이벤트 처리 시스템

GPUI의 이벤트 처리는 웹 브라우저의 캡처/버블링 모델과 유사하다:

```
플랫폼 이벤트 → PlatformInput (정규화)
                    ↓
            Window::dispatch_event()
                    ↓
            캡처 단계 (Root → Target)
                    ↓
            버블링 단계 (Target → Root)
                    ↓
            Action Handler / Element Handler
```

**이벤트 처리 패턴:**

```rust
div()
    .key_context("TerminalPanel")           // 디스패치 스코프 설정
    .on_click(|event, window, cx| {         // 클릭 이벤트
        println!("클릭: {:?}", event);
    })
    .on_mouse_down(MouseButton::Left, |event, window, cx| {
        // 마우스 다운 처리
    })
    .on_key_down(|event, window, cx| {
        // 키보드 이벤트 처리
    })
    .on_action(cx.listener(Self::handle_copy))  // 액션 핸들러
```

**Action 시스템:**

```rust
// 1. 액션 정의
actions!(terminal, [Copy, Paste, Clear, SelectAll]);

// 2. 키 바인딩 등록
cx.bind_keys([
    KeyBinding::new("cmd-c", Copy, Some("Terminal")),
    KeyBinding::new("cmd-v", Paste, Some("Terminal")),
]);

// 3. 핸들러 구현
fn handle_copy(&mut self, _: &Copy, window: &mut Window, cx: &mut Context<Self>) {
    // 복사 로직
}
```

---

## 3. IME 지원 및 입력 처리

### 3.1 GPUI의 IME 아키텍처

Zed 에디터는 `EntityInputHandler` 트레이트를 통해 IME (입력기) 지원을 구현한다. 이는 한국어, 중국어, 일본어 등 CJK 입력에 필수적이다.

#### 핵심 트레이트: EntityInputHandler

```rust
// IME 조합 중 임시 텍스트 삽입
fn replace_and_mark_text_in_range(
    &mut self,
    range: Option<Range<usize>>,
    new_text: &str,
    new_selected_range: Option<Range<usize>>,
    cx: &mut Context<Self>,
);

// 현재 조합 중인 텍스트 범위 반환
fn marked_text_ranges(&self, cx: &Context<Self>) -> Option<Vec<Range<usize>>>;
```

### 3.2 플랫폼별 IME 구현

| 플랫폼 | 프로토콜 | 구현 방식 |
|--------|---------|----------|
| **macOS** | NSTextInputClient | Cocoa 프로토콜 구현 |
| **Windows** | WM_IME_COMPOSITION | ImmGetCompositionString |
| **Linux (X11)** | XIM | X Input Method 프로토콜 |
| **Linux (Wayland)** | zwp_text_input_v3 | Wayland 텍스트 입력 프로토콜 |

### 3.3 macOS NSTextInputClient 통합

macOS에서 GPUI는 Cocoa의 `NSTextInputClient` 프로토콜을 구현하여 시스템 IME와 통신한다:

1. 시스템 IME가 조합(composition) 시작
2. `replace_and_mark_text_in_range()` 호출로 미확정(pre-edit) 텍스트 표시
3. 사용자가 후보 선택 또는 확정
4. 확정된 텍스트가 최종 입력으로 전달

### 3.4 포커스 관리 시스템

GPUI는 정교한 포커스 관리 시스템을 갖추고 있다:

- **FocusHandle**: 포커스 가능한 요소와 연결
- **FocusId**: 고유 포커스 식별자
- **FocusMap**: 윈도우별 포커스 추적

```rust
// 포커스 핸들 생성
let focus_handle = window.focus_handle(cx);

// 요소에 포커스 연결
div()
    .track_focus(&focus_handle)
    .on_focus(|_, _, cx| { /* 포커스 획득 */ })
    .on_blur(|_, _, cx| { /* 포커스 상실 */ })
```

포커스는 `Focusable` 트레이트를 구현하는 요소에서만 작동한다.

### 3.5 터미널에서의 IME 입력 흐름

Zed의 터미널에서 IME 입력은 다음과 같이 처리된다:

```
OS IME → marked (pre-edit) 텍스트 수신
  ↓
TerminalView가 ime_state에 저장
  ↓
UI에 밑줄 표시된 미확정 텍스트 표시
  ↓
사용자 확정 → committed 텍스트로 변환
  ↓
확정된 텍스트를 터미널 PTY에 전송
```

### 3.6 알려진 IME 이슈

- **키맵 우선순위 충돌**: CJK IME 입력이 Zed의 키맵에 의해 가로챌 수 있는 문제 ([Issue #28174](https://github.com/zed-industries/zed/issues/28174))
- **Windows IME 토글**: 일부 일본어 IME (ATOK 등)에서 토글 키가 작동하지 않는 문제
- **Vim 모드 IME 전환**: 삽입 모드와 노멀 모드 전환 시 IME 자동 전환 미지원

> **Crux에 대한 시사점**: GPUI의 IME 지원은 기본 구조는 갖추어져 있으나, 터미널에서의 완벽한 CJK 입력을 위해서는 추가 작업이 필요하다. 특히 키맵과 IME 간의 우선순위 처리에 주의해야 한다.

---

## 4. 터미널 통합

### 4.1 gpui-ghostty 프로젝트 분석

[gpui-ghostty](https://github.com/Xuanwo/gpui-ghostty)는 Ghostty의 터미널 코어를 GPUI 렌더링과 결합한 프로젝트이다.

**블로그 포스트**: https://xuanwo.io/2026/01-gpui-ghostty/

#### 아키텍처

```
┌─────────────────────────────────────────────────┐
│                    GPUI App                       │
│  ┌─────────────────────────────────────────────┐ │
│  │         gpui_ghostty_terminal               │ │
│  │    (GPUI TerminalView + 입력/선택/렌더링)     │ │
│  └─────────────┬───────────────────────────────┘ │
│                │                                  │
│  ┌─────────────▼───────────────────────────────┐ │
│  │            ghostty_vt                        │ │
│  │      (안전한 Rust 래퍼)                       │ │
│  └─────────────┬───────────────────────────────┘ │
│                │                                  │
│  ┌─────────────▼───────────────────────────────┐ │
│  │          ghostty_vt_sys                      │ │
│  │    (Zig 빌드 + C ABI, Ghostty VT 코어)       │ │
│  └─────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

#### 크레이트 구조

```
crates/
├── ghostty_vt_sys        # C ABI + Zig 빌드 (Ghostty VT 코어)
├── ghostty_vt            # 안전한 Rust 래퍼
└── gpui_ghostty_terminal # GPUI View + 입출력/선택/렌더링 글루코드

examples/
├── vt_dump               # VT 파싱 데모
├── basic_terminal        # 최소 GPUI 뷰
├── pty_terminal          # 셸 PTY 통합
└── split_pty_terminal    # 멀티 패널 데모
```

#### 버전 핀닝

| 의존성 | 버전 | 비고 |
|--------|------|------|
| Ghostty | v1.2.3 (vendor 서브모듈) | VT 파서/상태 관리 |
| Zig | 0.14.1 | Ghostty 코어 컴파일에 필수 |
| GPUI/Zed | 커밋 `6016d0b8c6` 핀닝 | 안정 버전 고정 |

#### 지원 기능

- DSR 응답 (CSI 5n / CSI 6n): 커서 위치/상태 쿼리
- OSC 타이틀 추적 (OSC 0/2)
- OSC 52 클립보드 쓰기
- OSC 10/11 기본 색상 쿼리
- SGR 마우스 모드 + 스크롤백 네비게이션
- IME 조합 (프리에딧 오버레이 + 커밋)
- DEC 특수 그래픽 및 박스 드로잉 (절차적 렌더링)

#### 빌드 방법

```bash
git submodule update --init --recursive
./scripts/bootstrap-zig.sh    # .context/zig/zig에 설치
cargo test
cargo run -p pty_terminal     # PTY 터미널 예제 실행
```

#### Public API

```rust
// 주요 내보내기 타입
TerminalConfig      // 설정
TerminalSession     // 세션 관리
TerminalView        // GPUI 컴포넌트
TerminalInput       // 입력 처리
Copy, Paste, SelectAll  // 액션

// 호스트 앱에서의 사용
TerminalConfig {
    update_window_title: false,  // 호스트 앱이 타이틀 관리
    ..Default::default()
}
```

#### 언어 구성

- Rust: 77.9% | Zig: 19.4% | C: 2.0% | Shell: 0.7%

> **핵심 통찰**: gpui-ghostty는 Ghostty의 렌더러를 재사용하지 않는다. 대신 Ghostty의 VT 파서에 바이트를 공급하고, GPUI가 독자적으로 그 상태를 렌더링한다. 이 분리된 접근은 Crux에서도 참고할 만한 아키텍처이다.

### 4.2 Zed의 내장 터미널

Zed 에디터의 터미널 시스템은 **alacritty_terminal** 크레이트를 기반으로 한다.

#### 계층 구조

```
TerminalPanel (dock 패널, 다수 터미널 관리)
    ↓
TerminalView (워크스페이스 아이템, 탭/분할 지원)
    ↓
Terminal (Entity, alacritty_terminal 래퍼)
    ↓
alacritty_terminal::Term<ZedListener> (VT 에뮬레이션)
    ↓
PTY (pseudo-terminal, 셸 프로세스)
```

#### Terminal Entity 구조

```rust
struct Terminal {
    term: Arc<FairMutex<Term<ZedListener>>>,    // Alacritty 인스턴스
    terminal_type: TerminalType,                 // Pty | DisplayOnly
    events: VecDeque<InternalEvent>,             // 버퍼된 이벤트
    last_content: TerminalContent,               // 캐싱된 렌더링 스냅샷
    task: Option<TaskState>,                     // 태스크 메타데이터
}
```

#### PTY 관리

PTY 생성 시 설정되는 환경변수:

```bash
ZED_TERM=true
TERM_PROGRAM=zed
TERM=xterm-256color
COLORTERM=truecolor
```

셸 선택 우선순위: 태스크 명령 → 원격 클라이언트 셸 → 설정의 terminal.shell → 시스템 기본 셸

#### 이벤트 처리 흐름

```
Alacritty 이벤트 (Title, Wakeup, PtyWrite, Bell 등)
    ↓
이벤트 루프: 최대 100개 배치 또는 4ms 대기
    ↓
Terminal Entity 업데이트 (foreground executor)
    ↓
cx.notify() → GPUI 프레임 갱신 스케줄링
```

#### TerminalContent (렌더링 스냅샷)

```rust
struct TerminalContent {
    cells: Vec<IndexedCell>,              // 렌더링할 셀들
    mode: TermMode,                        // 터미널 모드
    display_offset: usize,                 // 스크롤백 오프셋
    selection: Option<SelectionRange>,     // 현재 선택 영역
    cursor: RenderableCursor,              // 커서 위치/형태
    cursor_char: char,                     // 커서 아래 문자
    terminal_bounds: TerminalBounds,       // 크기 정보
}
```

#### TerminalElement 렌더링 최적화

**텍스트 런 배칭:**
- 동일 스타일의 인접 셀을 `BatchedTextRun`으로 묶어 GPU 드로 콜 최소화
- 줄이 바뀌거나 스타일이 변경될 때 새 배치 시작
- Zero-width 결합 문자 처리, 와이드 문자 스페이서 건너뛰기

**배경 영역 병합:**
- 기본값이 아닌 셀의 배경색을 사각형으로 수집
- 같은 줄의 인접 열은 확장
- 수직 인접 영역 다단계 병합
- 병합된 영역을 레이아웃 사각형으로 변환

#### 스크롤백 버퍼

- 일반 터미널: 기본 **10,000줄** (설정 가능)
- 태스크 터미널: 최대 스크롤백 (출력이 유한하므로)
- `display_offset`으로 스크롤 위치 추적

### 4.3 gpui-terminal 프로젝트

[gpui-terminal](https://github.com/zortax/gpui-terminal)은 또 다른 GPUI 터미널 컴포넌트로, alacritty_terminal을 사용한다.

| 항목 | 내용 |
|------|------|
| VT 파서 | alacritty_terminal |
| PTY | portable-pty (교체 가능) |
| I/O 모델 | Push 기반 비동기, 임의 Read/Write 스트림 |
| 색상 | 16 ANSI + 256색 + 24비트 RGB |
| 스타일 | 볼드, 이탤릭, 밑줄 |
| 클립보드 | OSC 52 |
| 스크롤백 | 10,000줄 (설정 가능) |
| 상태 | 초기 단계 (16 stars, 6 commits) |
| 제한사항 | 마우스 텍스트 선택 미구현, 스크롤백 네비게이션 미지원 |

### 4.4 CJK 폰트 렌더링

터미널에서 CJK 문자를 올바르게 렌더링하는 것은 고유한 도전 과제이다:

#### Unicode Han Unification 문제

동일한 유니코드 코드포인트가 한국어, 중국어(간체/번체), 일본어에서 다른 시각적 형태를 요구한다. 올바른 폰트 선택이 가독성을 위해 중요하다.

#### GPUI의 텍스트 렌더링 파이프라인

| 플랫폼 | 텍스트 셰이핑 | 글리프 래스터화 |
|--------|-------------|---------------|
| macOS | CoreText | CoreText + 글리프 아틀라스 |
| Linux | Cosmic Text (HarfBuzz 기반) | Cosmic Text |
| Windows | DirectWrite | DirectWrite |

macOS의 CoreText는 한글을 포함한 CJK 문자에 대해 우수한 폰트 폴백과 셰이핑을 제공한다. 서브픽셀 안티앨리어싱도 지원한다.

#### 알려진 이슈

- Linux (Wayland)에서 cosmic-text 관련 폰트 렌더링 회귀 버그 발생 ([Issue #30526](https://github.com/zed-industries/zed/issues/30526))
- cosmic-text 0.14.x 버전 간 API 변경으로 호환성 문제

> **Crux에 대한 시사점**: macOS를 주 타겟으로 하면 CoreText 기반 텍스트 렌더링이 CJK에 양호하다. 그러나 한글 자모 조합 입력과 폰트 폴백에 대한 테스트가 필수이다.

---

## 5. 제한사항 및 고려사항

### 5.1 플랫폼 지원 상태

| 플랫폼 | 상태 | 렌더러 | 비고 |
|--------|------|--------|------|
| **macOS** | **안정** | Metal | 주 개발 플랫폼 |
| **Linux** | 지원 중 | Blade (Vulkan) | X11 + Wayland 모두 지원, 일부 이슈 존재 |
| **Windows** | 프라이빗 베타 | DirectX 11 | Zed의 Windows 지원은 2025년 안정 릴리스 목표 |

> macOS만 타겟으로 하는 Crux에게는 가장 안정적인 환경이다.

### 5.2 Breaking Changes

GPUI는 pre-1.0이므로 버전 간 Breaking Change가 빈번하다:

- crates.io에 0.2.0 → 0.2.1 → 0.2.2 등 릴리스가 있으나 상세 Changelog 부재
- gpui-ghostty는 특정 Zed 커밋에 핀닝하여 안정성 확보
- gpui-component도 GPUI 버전에 맞춰 갱신 필요

**권장 전략**: Crux도 특정 GPUI 커밋 또는 버전에 핀닝하고, 주기적으로 업그레이드하는 방식을 채택해야 한다.

### 5.3 문서화 품질

| 자료 | 품질 | URL |
|------|------|-----|
| docs.rs API 레퍼런스 | 기본적 (타입/트레이트 문서) | [docs.rs/gpui](https://docs.rs/gpui) |
| GPUI README | 간략한 개요 | [GitHub](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md) |
| gpui.rs 공식 사이트 | 예제 위주 | [gpui.rs](https://www.gpui.rs/) |
| gpui-book (비공식) | 개념 설명 | [GitHub](https://github.com/MatinAniss/gpui-book) |
| gpui-tutorial | Hello World ~ 컴포넌트 | [GitHub](https://github.com/hedge-ops/gpui-tutorial) |
| Zed 소스 코드 | **가장 중요한 참고 자료** | [GitHub](https://github.com/zed-industries/zed) |
| DeepWiki | 상세한 아키텍처 분석 | [deepwiki.com](https://deepwiki.com/zed-industries/zed) |
| Zed Discord | 커뮤니티 Q&A | - |

> "The best way to learn about gpui APIs is to read the Zed source code, ask at fireside hacks, or drop questions in the Zed Discord." — GPUI README

### 5.4 커뮤니티 규모 및 생태계

**awesome-gpui 기준 (2026년 2월):**

| 카테고리 | 수량 | 주요 프로젝트 |
|---------|------|-------------|
| 앱 | 21개 | Loungy (런처), helix-gpui (에디터), hummingbird (음악) |
| 라이브러리 | 11개 | gpui-component, gpui-router, gpui-nav |
| 도구 | 3개 | create-gpui-app, React Native GPUI |
| 학습 자료 | 2개 | gpui-book, YouTube 시리즈 |

**생태계 특징:**
- 빠르게 성장 중이나 아직 작은 규모
- gpui-component가 사실상의 표준 컴포넌트 라이브러리
- 다양한 분야 (에디터, 음악, 데이터베이스 GUI, API 클라이언트)에서 활용
- Zed 팀의 공식 지원 (awesome-gpui, create-gpui-app)

### 5.5 GPUI의 장점 (Crux 관점)

1. **120 FPS GPU 렌더링**: 터미널 에뮬레이터에 이상적인 성능
2. **Rust 네이티브**: 메모리 안전성 + 고성능
3. **macOS Metal 최적화**: 주 타겟 플랫폼에 최적
4. **Flexbox 레이아웃**: 분할 패널, 탭 등 UI 구성 용이
5. **포커스 관리 + IME**: 입력 처리 기반 인프라 존재
6. **gpui-component**: 60+ 컴포넌트로 빠른 UI 개발

### 5.6 GPUI의 위험 요소 (Crux 관점)

1. **Breaking Changes**: pre-1.0이므로 업그레이드 시 코드 변경 필요
2. **빈약한 문서**: Zed 소스 코드 읽기가 필수
3. **작은 커뮤니티**: 문제 해결 시 자체 조사 필요
4. **IME 이슈**: CJK 입력에 알려진 버그 존재
5. **Linux 폰트 렌더링**: cosmic-text 관련 이슈 (macOS에는 해당 없음)

---

## 6. Crux 프로젝트에 대한 시사점

### 6.1 추천 아키텍처

gpui-ghostty와 Zed의 터미널 구현을 참고하여 다음 아키텍처를 제안한다:

```
┌──────────────────────────────────────────────────────────┐
│                      Crux App (GPUI)                      │
│  ┌──────────────────────────────────────────────────────┐ │
│  │  DockArea (gpui-component)                           │ │
│  │  ├── TabPanel: 탭으로 다수 터미널                      │ │
│  │  └── Split: 수평/수직 분할 레이아웃                     │ │
│  └──────────────┬───────────────────────────────────────┘ │
│                 │                                         │
│  ┌──────────────▼───────────────────────────────────────┐ │
│  │  CruxTerminalView (GPUI Element)                     │ │
│  │  ├── 셀 렌더링 (BatchedTextRun)                       │ │
│  │  ├── 커서 렌더링                                      │ │
│  │  ├── 선택 영역 렌더링                                  │ │
│  │  └── IME 조합 텍스트 표시                              │ │
│  └──────────────┬───────────────────────────────────────┘ │
│                 │                                         │
│  ┌──────────────▼───────────────────────────────────────┐ │
│  │  CruxTerminal (Entity)                               │ │
│  │  ├── VT 파서 (alacritty_terminal 또는 ghostty_vt)     │ │
│  │  ├── PTY 관리                                         │ │
│  │  ├── 이벤트 큐                                        │ │
│  │  └── TerminalContent (렌더링 스냅샷)                   │ │
│  └──────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

### 6.2 VT 백엔드 선택

| 옵션 | 장점 | 단점 |
|------|------|------|
| **alacritty_terminal** | 안정적, Zed에서 검증됨, Rust 순수 | 일부 최신 VT 기능 부족 |
| **ghostty_vt** | 최신 VT 지원, Ghostty 생태계 | Zig 빌드 의존성, 복잡한 빌드 체인 |
| **직접 구현** | 완전한 제어 | 막대한 개발 비용 |

**권장**: alacritty_terminal로 시작하여 안정적인 기반을 마련한 후, 필요에 따라 ghostty_vt로 전환을 고려한다.

### 6.3 핵심 의존성 매트릭스

```toml
[dependencies]
gpui = "0.2.2"
gpui-component = "0.5.1"
gpui-component-assets = "0.5.1"
alacritty_terminal = "0.24"     # VT 에뮬레이션
portable-pty = "0.8"            # PTY 관리
```

### 6.4 구현 우선순위

| 우선순위 | 기능 | 기반 기술 |
|---------|------|----------|
| P0 | 기본 터미널 렌더링 | GPUI Element + alacritty_terminal |
| P0 | PTY 연결 | portable-pty |
| P0 | 키보드 입력 | GPUI Action 시스템 |
| P1 | 탭 인터페이스 | gpui-component Tabs |
| P1 | 분할 패널 | gpui-component DockArea + Resizable |
| P1 | 한글 IME | EntityInputHandler |
| P2 | 스크롤백 | TerminalContent.display_offset |
| P2 | 텍스트 선택 & 복사 | 마우스 이벤트 + 클립보드 |
| P3 | 하이퍼링크 감지 | OSC 8 + 패턴 매칭 |
| P3 | 테마 시스템 | gpui-component 테마 |

---

## 7. 참고 자료

### 공식 문서 및 소스

- [GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md) — 프레임워크 개요
- [GPUI docs.rs](https://docs.rs/gpui) — API 레퍼런스
- [gpui.rs](https://www.gpui.rs/) — 공식 사이트
- [Zed Blog: Leveraging Rust and the GPU to render at 120 FPS](https://zed.dev/blog/videogame) — Metal 렌더링 아키텍처 상세

### 컴포넌트 라이브러리

- [gpui-component GitHub](https://github.com/longbridge/gpui-component) — 60+ UI 컴포넌트
- [gpui-component 문서](https://longbridge.github.io/gpui-component/) — 컴포넌트 사용법
- [gpui-component Getting Started](https://longbridge.github.io/gpui-component/docs/getting-started) — 시작 가이드

### 터미널 통합 프로젝트

- [gpui-ghostty GitHub](https://github.com/Xuanwo/gpui-ghostty) — Ghostty + GPUI 통합
- [gpui-ghostty 블로그](https://xuanwo.io/2026/01-gpui-ghostty/) — 아키텍처 설명
- [gpui-terminal GitHub](https://github.com/zortax/gpui-terminal) — alacritty_terminal + GPUI

### Zed 터미널 아키텍처

- [Zed Terminal Core (DeepWiki)](https://deepwiki.com/zed-industries/zed/9.1-terminal-core) — 터미널 코어 분석
- [Zed Terminal View (DeepWiki)](https://deepwiki.com/zed-industries/zed/9.2-terminal-view-and-rendering) — 렌더링 파이프라인
- [GPUI Framework (DeepWiki)](https://deepwiki.com/zed-industries/zed/2.2-ui-framework-(gpui)) — 프레임워크 상세 분석

### GPUI 아키텍처

- [GPUI Deep Dive (Medium)](https://beckmoulton.medium.com/gpui-a-technical-overview-of-the-high-performance-rust-ui-framework-powering-zed-ac65975cda9f) — 기술 개요
- [Zed Blog: Linux When?](https://zed.dev/blog/zed-decoded-linux-when) — Linux 지원 현황

### 생태계

- [awesome-gpui](https://github.com/zed-industries/awesome-gpui) — 프로젝트 목록
- [create-gpui-app](https://github.com/zed-industries/create-gpui-app) — 앱 스캐폴딩
- [gpui-book](https://github.com/MatinAniss/gpui-book) — 학습 가이드
- [gpui-tutorial](https://github.com/hedge-ops/gpui-tutorial) — 초보자 튜토리얼

### IME 관련

- [Zed IME Issues](https://github.com/zed-industries/zed/issues/28174) — CJK 키맵 충돌 이슈
- [Zed Event Flow (DeepWiki)](https://deepwiki.com/zed-industries/zed/2.4-keybinding-and-action-dispatch) — 키바인딩/액션 디스패치
