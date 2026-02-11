---
title: "GPUI 프로젝트 부트스트랩"
description: "Cargo workspace setup for GPUI apps — dependency configuration, build.rs, Info.plist, dev/release profiles, minimal app bootstrap"
date: 2026-02-12
phase: [1]
topics: [cargo, workspace, build-setup, gpui, info-plist, profiles]
status: final
related:
  - framework.md
  - terminal-implementations.md
---

# GPUI 프로젝트 부트스트랩 리서치

> Crux 터미널의 Cargo workspace 설정 및 빌드 환경 가이드
> 작성일: 2026-02-12

---

## 1. GPUI 의존성 설정

### 1.1 GPUI 추가 방법

GPUI는 두 가지 방식으로 추가할 수 있다:

**A. crates.io 버전 (안정 릴리스)**
```toml
[dependencies]
gpui = "0.2.2"
```

**B. Git 의존성 (최신 개발 버전, 커밋 고정)**
```toml
[dependencies]
gpui = { git = "https://github.com/zed-industries/zed", package = "gpui" }
```

특정 커밋에 고정하려면:
```toml
gpui = { git = "https://github.com/zed-industries/zed", package = "gpui", rev = "6016d0b8c6a22e586158d3b6f810b3cebb136118" }
```

**권장**: Crux 프로젝트는 **crates.io 버전(`0.2.2`)으로 시작**하고, 필요 시 git 의존성으로 전환한다. 이유:
- crates.io 버전이 빌드 캐싱에 유리
- git 의존성은 전체 zed 레포를 클론하므로 초기 빌드가 느림
- gpui-ghostty 프로젝트가 git 의존성을 사용하는 이유는 crates.io에 없는 최신 API가 필요했기 때문

### 1.2 gpui-component 의존성

```toml
[dependencies]
gpui-component = "0.5.1"
gpui-component-assets = "0.5.1"  # 아이콘 에셋 (선택)
```

gpui-component는 DockArea, Tabs, ResizablePanel 등 60+ 위젯을 제공한다. Phase 2(탭/분할)부터 필요하지만, 초기부터 설정해두면 좋다.

### 1.3 참고: gpui-ghostty의 Cargo.toml 패턴

gpui-ghostty 프로젝트 (workspace root):
```toml
[workspace]
resolver = "2"
members = ["crates/*", "examples/*"]

[workspace.dependencies]
gpui = { git = "https://github.com/zed-industries/zed", package = "gpui" }
smallvec = { version = "1.15", features = ["const_new"] }
```

gpui-ghostty 터미널 크레이트:
```toml
[package]
name = "gpui_ghostty_terminal"
version = "0.1.0"
edition = "2024"
publish = false
license = "Apache-2.0"

[dependencies]
base64 = "0.22"
ghostty_vt = { path = "../ghostty_vt" }
gpui = { workspace = true }
smallvec = { workspace = true }
unicode-width = "0.2"
```

### 1.4 주의사항: core-foundation 충돌

GPUI를 crates.io에서 독립 빌드할 때 `core-foundation 0.10.1` 의존성 충돌이 발생할 수 있다 ([GitHub Issue #43986](https://github.com/zed-industries/zed/issues/43986)). 해결 방법:
- `cargo update`로 의존성 트리 갱신
- 필요 시 `[patch.crates-io]` 섹션으로 오버라이드

---

## 2. Cargo Workspace 구조

### 2.1 create-gpui-app 템플릿 (공식)

Zed에서 제공하는 [create-gpui-app](https://github.com/zed-industries/create-gpui-app) 도구가 생성하는 표준 구조:

**단일 프로젝트:**
```
my-app/
├── src/
│   └── main.rs
├── Cargo.toml
└── README.md
```

**워크스페이스:**
```
my-app/
├── Cargo.toml          (workspace root)
├── crates/
│   └── my-app/
│       ├── Cargo.toml  (package)
│       └── src/
│           └── main.rs
└── README.md
```

워크스페이스 루트 Cargo.toml 템플릿:
```toml
[workspace]
members = ["crates/PROJECT_NAME"]
default-members = ["crates/PROJECT_NAME"]
resolver = "2"

[workspace.dependencies]
PROJECT_NAME = { path = "crates/PROJECT_NAME" }
gpui = { git = "https://github.com/zed-industries/zed" }
```

하위 크레이트 Cargo.toml 템플릿:
```toml
[package]
name = "PROJECT_NAME"
version = "0.1.0"
edition = "2021"
publish = false

[[bin]]
name = "PROJECT_NAME"
path = "src/main.rs"

[dependencies]
gpui.workspace = true
```

### 2.2 Crux 워크스페이스 구조

```
crux/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── crux-app/                 # 메인 앱, 윈도우 관리
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   ├── crux-terminal/            # 터미널 엔티티, VT 파싱, PTY
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   ├── crux-terminal-view/       # GPUI 렌더링, IME 오버레이
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   ├── crux-ipc/                 # Unix 소켓 서버, JSON-RPC
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   ├── crux-clipboard/           # 클립보드, 드래그 앤 드롭
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   └── crux-protocol/            # 프로토콜 타입 정의
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs
├── resources/
│   └── Info.plist
├── research/
├── README.md
└── PLAN.md
```

---

## 3. build.rs 요구사항

### 3.1 GPUI의 내부 빌드 처리

GPUI의 `build.rs`는 다음을 자동 처리한다:
- **Metal 셰이더 컴파일**: `xcrun -sdk macosx metal`로 `.metal` → `.air` → `.metallib` 변환
- **System 프레임워크 링크**: `println!("cargo:rustc-link-lib=framework=System")`
- GPUI가 직접 Metal, Cocoa, CoreText 등의 프레임워크를 링크하므로 **Crux에서 별도 build.rs가 필요 없다**

### 3.2 Crux에서 build.rs가 필요한 경우

Phase 1에서는 **build.rs가 필요 없다**. GPUI가 모든 macOS 프레임워크 링크를 처리한다.

향후 필요할 수 있는 경우:
- Phase 3 (IME): `objc2-app-kit` 사용 시 추가 프레임워크 링크가 필요할 수 있음
- Phase 4 (Graphics): Kitty Graphics Protocol 구현 시 이미지 처리 관련

### 3.3 Metal 셰이더 컴파일 요구사항

GPUI 빌드에는 Xcode의 `metal`과 `metallib` 도구가 필요하다:
- **Xcode.app 설치 필수** (Command Line Tools만으로는 부족)
- `sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer` 실행 필요
- Nix 등 격리 환경에서는 빌드 실패 가능 ([Discussion #7016](https://github.com/zed-industries/zed/discussions/7016))

`runtime_shaders` feature를 활성화하면 런타임에 셰이더를 컴파일할 수 있어 빌드 시 Xcode 의존성을 줄일 수 있다.

---

## 4. 최소 GPUI 앱 (Minimum Viable App)

### 4.1 create-gpui-app 공식 템플릿

```rust
use gpui::*;

struct HelloWorld {
    text: SharedString,
}

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .bg(rgb(0x2e7d32))
            .size_full()
            .justify_center()
            .items_center()
            .text_xl()
            .text_color(rgb(0xffffff))
            .child(format!("Hello, {}!", &self.text))
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_cx| HelloWorld {
                text: "World".into(),
            })
        })
        .unwrap();
    });
}
```

### 4.2 gpui-component 사용 패턴

```rust
use gpui::*;
use gpui_component::{button::*, *};

pub struct HelloWorld;

impl Render for HelloWorld {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child("Hello, World!")
            .child(
                Button::new("ok")
                    .primary()
                    .label("Let's Go!")
                    .on_click(|_, _, _| println!("Clicked!")),
            )
    }
}

fn main() {
    let app = Application::new().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);  // 필수: 테마/설정 초기화

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|_| HelloWorld);
                cx.new(|cx| Root::new(view, window, cx))
            })?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
```

**핵심 차이점**: gpui-component 사용 시:
- `Application::new().with_assets(gpui_component_assets::Assets)` — 에셋 등록
- `gpui_component::init(cx)` — 반드시 호출 (테마, 설정 초기화)
- `Root::new(view, window, cx)` — 루트 뷰를 Root로 감싸야 함
- `cx.spawn(async move |cx| { ... }).detach()` — 비동기 윈도우 생성 패턴

### 4.3 gpui-ghostty 터미널 앱 패턴

```rust
fn main() {
    use gpui::{App, AppContext, Application, KeyBinding, WindowOptions};
    use gpui_ghostty_terminal::{
        TerminalConfig, TerminalSession, view::Copy, view::Paste, view::SelectAll,
    };

    Application::new().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("cmd-a", SelectAll, None),
            KeyBinding::new("cmd-c", Copy, None),
            KeyBinding::new("cmd-v", Paste, None),
        ]);

        cx.open_window(WindowOptions::default(), |window, cx| {
            cx.new(|cx| {
                let focus_handle = cx.focus_handle();
                focus_handle.focus(window, cx);

                let mut session = TerminalSession::new(TerminalConfig::default()).unwrap();
                session.feed(b"Hello from GPUI + Ghostty VT\r\n").unwrap();
                gpui_ghostty_terminal::view::TerminalView::new(session, focus_handle)
            })
        })
        .unwrap();
    });
}
```

**핵심 패턴**:
- `cx.bind_keys()` — 키 바인딩 등록
- `cx.focus_handle()` — 포커스 핸들 생성
- `focus_handle.focus(window, cx)` — 윈도우에 포커스 설정
- TerminalView가 focus_handle을 소유하여 키 이벤트 수신

### 4.4 GPUI API 핵심 개념 정리

| 개념 | 설명 |
|------|------|
| `Application::new()` | 앱 인스턴스 생성 |
| `.run(\|cx\| { ... })` | 이벤트 루프 시작 |
| `cx.open_window(opts, \|window, cx\| { ... })` | 윈도우 생성 |
| `cx.new(\|cx\| { ... })` | View 모델 생성 |
| `impl Render for T` | 뷰 렌더링 트레잇 |
| `div()` | 플렉스박스 컨테이너 (CSS-like) |
| `.child()` | 자식 요소 추가 |
| `.bg()`, `.text_color()` | 스타일링 |
| `SharedString` | 불변 공유 문자열 |
| `FocusHandle` | 키보드 포커스 관리 |
| `KeyBinding::new()` | 키 바인딩 등록 |

---

## 5. Info.plist와 앱 번들

### 5.1 macOS 앱 번들 구조

```
Crux.app/
├── Contents/
│   ├── Info.plist
│   ├── MacOS/
│   │   └── crux-app          # 실행 바이너리
│   ├── Resources/
│   │   ├── crux.icns          # 앱 아이콘
│   │   └── crux.terminfo      # terminfo 파일 (Phase 5)
│   └── Frameworks/            # (필요 시)
```

### 5.2 Info.plist 초안

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- 기본 식별 -->
    <key>CFBundleName</key>
    <string>Crux</string>
    <key>CFBundleDisplayName</key>
    <string>Crux</string>
    <key>CFBundleIdentifier</key>
    <string>com.crux.terminal</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>

    <!-- 실행 바이너리 -->
    <key>CFBundleExecutable</key>
    <string>crux-app</string>

    <!-- 아이콘 -->
    <key>CFBundleIconFile</key>
    <string>crux</string>

    <!-- macOS 요구사항 -->
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>

    <!-- Retina 디스플레이 지원 -->
    <key>NSHighResolutionCapable</key>
    <true/>

    <!-- 전체 화면 지원 -->
    <key>NSSupportsAutomaticTermination</key>
    <true/>
    <key>NSSupportsSuddenTermination</key>
    <false/>

    <!-- GPU 렌더링 관련 -->
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>

    <!-- 앱 카테고리 -->
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>

    <!-- Info.plist 버전 -->
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
</dict>
</plist>
```

**참고**: `LSUIElement`는 설정하지 않는다. Crux는 Dock에 나타나는 일반 앱이다. LSUIElement=true는 백그라운드 전용 앱에만 사용.

### 5.3 앱 번들 생성 스크립트

```bash
#!/bin/bash
# scripts/bundle.sh
set -euo pipefail

APP_NAME="Crux"
BUNDLE_DIR="target/release/${APP_NAME}.app"
BINARY="target/release/crux-app"

# 빌드
cargo build --release

# 번들 디렉토리 생성
rm -rf "${BUNDLE_DIR}"
mkdir -p "${BUNDLE_DIR}/Contents/MacOS"
mkdir -p "${BUNDLE_DIR}/Contents/Resources"

# 바이너리 복사
cp "${BINARY}" "${BUNDLE_DIR}/Contents/MacOS/"

# Info.plist 복사
cp resources/Info.plist "${BUNDLE_DIR}/Contents/"

# 아이콘 복사 (있는 경우)
if [ -f resources/crux.icns ]; then
    cp resources/crux.icns "${BUNDLE_DIR}/Contents/Resources/"
fi

echo "Bundle created at ${BUNDLE_DIR}"
```

**대안**: `cargo-bundle` 크레이트를 사용하면 `Cargo.toml`에 메타데이터를 추가하여 자동으로 `.app` 번들을 생성할 수 있다.

---

## 6. 개발 환경 설정

### 6.1 Rust 툴체인

```bash
# 최신 stable Rust 설치/업데이트
rustup update stable

# 현재 버전 확인
rustc --version   # 1.84+ 권장
cargo --version
```

### 6.2 Xcode 요구사항

```bash
# Xcode.app 설치 (App Store 또는 developer.apple.com)
# Command Line Tools만으로는 Metal 셰이더 컴파일 불가!

# Xcode 선택 확인
xcode-select -p
# 출력: /Applications/Xcode.app/Contents/Developer

# 필요 시 전환
sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer

# metal 컴파일러 확인
xcrun -sdk macosx metal --version
```

### 6.3 macOS SDK

- **최소 요구**: macOS 13.0 (Ventura) — Metal 3 지원
- **권장**: macOS 14.0+ (Sonoma) — 최신 Metal 기능
- GPUI는 `mmacosx-version-min=10.15.7`로 셰이더를 컴파일하지만, 실제 앱은 Metal 기능에 따라 더 높은 버전 필요

### 6.4 IDE 설정 (rust-analyzer)

`.vscode/settings.json` (또는 에디터 동등 설정):
```json
{
    "rust-analyzer.cargo.features": "all",
    "rust-analyzer.check.command": "clippy",
    "rust-analyzer.procMacro.enable": true,
    "rust-analyzer.imports.granularity.group": "module"
}
```

GPUI는 매크로를 많이 사용하므로 `procMacro.enable: true`가 중요하다.

### 6.5 추가 도구

```bash
# cargo-bundle (앱 번들 생성)
cargo install cargo-bundle

# cargo-watch (파일 변경 시 자동 빌드)
cargo install cargo-watch

# create-gpui-app (GPUI 프로젝트 스캐폴딩)
cargo install create-gpui-app
```

---

## 7. 초안 Cargo 파일들

### 7.1 루트 Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    "crates/crux-app",
    "crates/crux-terminal",
    "crates/crux-terminal-view",
    "crates/crux-ipc",
    "crates/crux-clipboard",
    "crates/crux-protocol",
]
default-members = ["crates/crux-app"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
publish = false

[workspace.dependencies]
# 내부 크레이트
crux-terminal = { path = "crates/crux-terminal" }
crux-terminal-view = { path = "crates/crux-terminal-view" }
crux-ipc = { path = "crates/crux-ipc" }
crux-clipboard = { path = "crates/crux-clipboard" }
crux-protocol = { path = "crates/crux-protocol" }

# GPUI
gpui = "0.2.2"
# gpui-component = "0.5.1"          # Phase 2에서 활성화
# gpui-component-assets = "0.5.1"   # Phase 2에서 활성화

# 터미널 코어
alacritty_terminal = "0.25"
portable-pty = "0.9"

# 유틸리티
unicode-width = "0.2"
smallvec = { version = "1.15", features = ["const_new"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
log = "0.4"
parking_lot = "0.12"

# 비동기
tokio = { version = "1", features = ["full"] }

[profile.release]
opt-level = 3
lto = "thin"           # 빌드 속도와 최적화 균형
codegen-units = 1      # 최대 최적화
strip = "symbols"      # 바이너리 크기 축소

[profile.dev]
opt-level = 1          # 개발 중에도 약간의 최적화 (GPUI 렌더링 성능)

[profile.dev.package."*"]
opt-level = 2          # 의존성은 더 높은 최적화
```

### 7.2 crux-app/Cargo.toml

```toml
[package]
name = "crux-app"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[[bin]]
name = "crux-app"
path = "src/main.rs"

[dependencies]
gpui.workspace = true
crux-terminal.workspace = true
crux-terminal-view.workspace = true
anyhow.workspace = true
log.workspace = true
```

### 7.3 crux-terminal/Cargo.toml

```toml
[package]
name = "crux-terminal"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[lib]
doctest = false

[dependencies]
gpui.workspace = true
alacritty_terminal.workspace = true
portable-pty.workspace = true
unicode-width.workspace = true
parking_lot.workspace = true
serde.workspace = true
anyhow.workspace = true
log.workspace = true
```

### 7.4 crux-terminal-view/Cargo.toml

```toml
[package]
name = "crux-terminal-view"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[lib]
doctest = false

[dependencies]
gpui.workspace = true
crux-terminal.workspace = true
unicode-width.workspace = true
smallvec.workspace = true
anyhow.workspace = true
log.workspace = true
```

### 7.5 crux-protocol/Cargo.toml

```toml
[package]
name = "crux-protocol"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
```

### 7.6 crux-ipc/Cargo.toml (Phase 2용, 초기에는 빈 크레이트)

```toml
[package]
name = "crux-ipc"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
crux-protocol.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
log.workspace = true
```

### 7.7 crux-clipboard/Cargo.toml (Phase 3용, 초기에는 빈 크레이트)

```toml
[package]
name = "crux-clipboard"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
anyhow.workspace = true
log.workspace = true
```

---

## 8. 최소 main.rs (Phase 1 시작점)

### 8.1 단계 0: 윈도우만 열기

```rust
// crates/crux-app/src/main.rs
use gpui::*;

struct CruxApp {
    status: SharedString,
}

impl Render for CruxApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .bg(rgb(0x1e1e2e))      // 다크 배경
            .size_full()
            .justify_center()
            .items_center()
            .text_xl()
            .text_color(rgb(0xcdd6f4))  // 밝은 텍스트
            .child(format!("Crux Terminal — {}", &self.status))
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.0), px(0.0)),
                    size: size(px(800.0), px(600.0)),
                })),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_cx| CruxApp {
                    status: "Starting...".into(),
                })
            },
        )
        .unwrap();
    });
}
```

### 8.2 단계 1: 텍스트 렌더링 + 키 입력

```rust
// crates/crux-app/src/main.rs (단계 1 확장)
use gpui::*;

struct CruxApp {
    lines: Vec<SharedString>,
    input_buffer: String,
}

impl CruxApp {
    fn new() -> Self {
        Self {
            lines: vec!["Welcome to Crux Terminal v0.1.0".into()],
            input_buffer: String::new(),
        }
    }
}

impl Render for CruxApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = cx.focus_handle();

        div()
            .track_focus(&focus_handle)
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e2e))
            .size_full()
            .p_4()
            .text_sm()
            .font_family("Berkeley Mono")  // 또는 "Menlo", "SF Mono"
            .text_color(rgb(0xcdd6f4))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, _cx| {
                if let Some(text) = &event.keystroke.key_char {
                    this.input_buffer.push_str(text);
                    this.lines.push(format!("Input: {}", text).into());
                }
            }))
            .children(
                self.lines.iter().map(|line| {
                    div().child(line.clone())
                })
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.0), px(0.0)),
                    size: size(px(800.0), px(600.0)),
                })),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| {
                    let focus_handle = cx.focus_handle();
                    focus_handle.focus(window, cx);
                    CruxApp::new()
                });
                view
            },
        )
        .unwrap();
    });
}
```

---

## 9. 빌드 및 실행 가이드

### 9.1 초기 빌드

```bash
cd /Users/jjh/Projects/crux

# 워크스페이스 구조 생성
mkdir -p crates/{crux-app/src,crux-terminal/src,crux-terminal-view/src,crux-ipc/src,crux-clipboard/src,crux-protocol/src}
mkdir -p resources

# 각 lib.rs에 최소 내용 추가 (빈 크레이트)
# crux-app/src/main.rs에 위의 최소 main.rs 추가

# 빌드
cargo build

# 실행
cargo run -p crux-app
```

### 9.2 첫 빌드 시 예상 시간

- **첫 빌드**: 5-15분 (GPUI와 의존성 컴파일, Metal 셰이더 컴파일 포함)
- **증분 빌드**: 2-10초
- `profile.dev.package."*".opt-level = 2` 설정으로 의존성 최적화하면 런타임 성능 향상

### 9.3 문제 해결

| 문제 | 해결 |
|------|------|
| `metal: command not found` | `sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer` |
| `core-foundation` 충돌 | `cargo update` 또는 `[patch.crates-io]` |
| `MTLCompilerService` 에러 | macOS 업데이트 후 SDK 불일치 — GPUI 재빌드 필요 |
| 셰이더 컴파일 실패 | Xcode.app 전체 설치 (CLT만으로 불충분) |
| 느린 빌드 | `[profile.dev]`에서 `opt-level = 1`, sccache 도입 고려 |

---

## 10. 참고 프로젝트 비교

| 항목 | create-gpui-app | gpui-ghostty | Crux (계획) |
|------|-----------------|--------------|-------------|
| GPUI 소스 | git (zed repo) | git (zed repo) | crates.io 0.2.2 |
| gpui-component | 미사용 | 미사용 | 0.5.1 |
| workspace | 선택적 | crates/* + examples/* | crates/* |
| 터미널 백엔드 | N/A | ghostty-vt (Zig) | alacritty_terminal |
| PTY | N/A | N/A (예제에서 직접) | portable-pty |
| edition | 2021 | 2024 | 2021 |

---

## 출처

- [GPUI crates.io](https://crates.io/crates/gpui) — 공식 패키지
- [GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)
- [create-gpui-app](https://github.com/zed-industries/create-gpui-app) — 공식 스캐폴딩 도구
- [gpui-component Getting Started](https://longbridge.github.io/gpui-component/docs/getting-started)
- [gpui-ghostty](https://github.com/Xuanwo/gpui-ghostty) — GPUI + 터미널 통합 참고
- [Xuanwo 블로그: Build GPUI + Ghostty without writing code](https://xuanwo.io/2026/01-gpui-ghostty/)
- [GPUI Book: Manual Project](https://matinaniss.github.io/gpui-book/getting-started/manual-project.html)
- [alacritty_terminal Cargo.toml](https://github.com/alacritty/alacritty/blob/master/alacritty_terminal/Cargo.toml)
- [portable-pty crates.io](https://crates.io/crates/portable-pty)
- [Zed GPUI build.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/build.rs)
- [Zed terminal Cargo.toml](https://github.com/zed-industries/zed/blob/main/crates/terminal/Cargo.toml)
- [cargo-bundle](https://github.com/burtonageo/cargo-bundle) — 앱 번들 생성
- [Building GPUI without Xcode metal](https://github.com/zed-industries/zed/discussions/7016)
- [GPUI crates.io core-foundation conflict](https://github.com/zed-industries/zed/issues/43986)
