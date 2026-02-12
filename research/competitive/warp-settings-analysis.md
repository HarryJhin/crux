---
title: "Warp Terminal Settings Analysis"
description: "Warp 터미널 설정 기능 조사 — 앱 설정으로 채택할 기능 분석, 우선순위 분류, Crux 기존 설계와의 갭 분석"
date: 2026-02-12
phase: [5]
topics: [config, settings, warp, competitive-analysis, gui, ux]
status: final
related:
  - ghostty-warp-analysis.md
  - ../core/config-system.md
---

# Warp Terminal Settings Analysis

> 작성일: 2026-02-12
> 목적: Warp 터미널의 설정 기능을 체계적으로 조사하고, Crux 앱 설정에 채택할 기능을 우선순위별로 분류

---

## 목차

1. [조사 배경](#1-조사-배경)
2. [Warp 설정 카테고리 전체 목록](#2-warp-설정-카테고리-전체-목록)
3. [Crux 기존 설계와의 갭 분석](#3-crux-기존-설계와의-갭-분석)
4. [채택 우선순위 분류](#4-채택-우선순위-분류)
5. [제안 TOML 구조 확장](#5-제안-toml-구조-확장)
6. [구현 타이밍](#6-구현-타이밍)
7. [채택 비추천 기능](#7-채택-비추천-기능)
8. [핵심 인사이트](#8-핵심-인사이트)

---

## 1. 조사 배경

Warp은 GPU 가속 터미널 + AI를 결합한 현대적 터미널로, 전통적인 텍스트 설정 파일 없이 **GUI-only 설정**을 제공한다. Crux는 반대로 **TOML 파일 + GUI 양방향 동기화**를 설계했으므로, Warp의 설정 항목 중 Crux의 TOML 스키마에 추가할 가치가 있는 것을 식별하는 것이 이 문서의 목적이다.

### 조사 방법론

- Warp 공식 문서 (docs.warp.dev) 전수 조사
- Settings 패널 카테고리별 개별 설정 항목 수집
- Crux `config-system.md` 및 `PLAN.md` Phase 5.7과 교차 비교

---

## 2. Warp 설정 카테고리 전체 목록

### 2.1 Appearance

| 설정 항목 | 설명 | 타입 |
|-----------|------|------|
| Theme | 프리로드 테마 선택 + 커스텀 테마 | enum / custom |
| OS Theme Sync | Light/Dark 자동 전환 | bool |
| Custom Theme from Image | 배경 이미지에서 자동 테마 생성 | 고급 기능 |
| Font family | 시스템 폰트 선택 | string |
| Font size | 폰트 크기 | float |
| Cursor style | Bar / Block / Underline | enum |
| Cursor blink | 커서 깜빡임 | bool |
| Window opacity | 창 투명도 (1-100%) | float |
| Background blur | 배경 블러 (macOS only) | bool |
| Input position | 입력란 위치 (top/bottom) | enum |
| Tab indicators | 탭 상태 인디케이터 표시 | bool |
| Tab title + ANSI colors | 탭 커스텀 타이틀/색상 | string + color |

### 2.2 Terminal Behavior

| 설정 항목 | 설명 | 타입 |
|-----------|------|------|
| Session restoration | 종료 시 윈도우/탭/패널 복원 | bool |
| Tab restoration | 최근 닫은 탭 복원 (60초) | bool + duration |
| Launch configurations | YAML로 레이아웃 저장/복원 | file-based |
| Audible bell | 터미널 벨 소리 | bool |
| Desktop notifications | 장시간 명령 완료 알림 | bool + threshold |
| Subshell warpify | 서브쉘에서 Warp 기능 활성화 | bool + list |

### 2.3 Input / Editor

| 설정 항목 | 설명 | 타입 |
|-----------|------|------|
| Vim keybindings | 에디터 Vim 모드 | bool |
| TAB key behavior | 탭 키 동작 커스텀 | enum |
| Input format | Standard / Classic 입력 | enum |
| Autosuggestions | 히스토리 기반 자동 제안 | bool |
| Tab completions | 자동 완성 메뉴 | bool |

### 2.4 AI Features

| 설정 항목 | 설명 | 타입 |
|-----------|------|------|
| AI toggle (global) | AI 기능 전체 활성/비활성 | bool |
| Active AI | 에러 기반 proactive 추천 | bool |
| Agent Mode | 자연어 명령 해석 | bool |
| Secret redaction | AI 요청에서 시크릿 자동 제거 | always-on |

### 2.5 Keyboard Shortcuts

| 설정 항목 | 설명 | 타입 |
|-----------|------|------|
| Custom keybindings | 액션별 단축키 재정의 | key-action map |
| YAML keybindings file | `~/.warp/keybindings.yaml` | file |
| Global hotkey (Quake) | 시스템 전역 핫키 토글 | key + position + size |

### 2.6 Privacy & Telemetry

| 설정 항목 | 설명 | 타입 |
|-----------|------|------|
| Telemetry opt-out | 사용 통계 수집 비활성화 | bool |
| Network log viewer | 송수신 데이터 확인 | viewer |

### 2.7 Performance & System

| 설정 항목 | 설명 | 타입 |
|-----------|------|------|
| Prefer integrated GPU | 저전력 GPU 선호 | bool |
| Graphics backend | Vulkan / OpenGL 선택 (Linux) | enum |

### 2.8 SSH & Remote

| 설정 항목 | 설명 | 타입 |
|-----------|------|------|
| SSH session detection | SSH 세션 자동 감지 | bool |
| Warpify SSH | 원격 세션에서 Warp 기능 활성화 | bool |

### 2.9 Collaboration (SaaS)

| 설정 항목 | 설명 | 타입 |
|-----------|------|------|
| Settings sync | 클라우드 설정 동기화 | bool |
| Session sharing | 실시간 터미널 공유 | feature |
| Warp Drive | 워크플로/노트북/환경변수 공유 | cloud workspace |
| Teams | 팀 멤버 관리 | feature |

---

## 3. Crux 기존 설계와의 갭 분석

### 이미 커버된 항목 (PLAN.md 5.7 기준)

| Warp 기능 | Crux 설계 위치 | 상태 |
|-----------|---------------|------|
| Font family/size | `[font]` 섹션 | ✅ 설계 완료 |
| Cursor style/blink | `cursor_style`, `cursor_blink` | ✅ 설계 완료 |
| Window opacity/blur | `[window]` 섹션 | ✅ 설계 완료 |
| Color theme | ANSI + fg/bg/cursor | ✅ 설계 완료 |
| Scrollback | `scrollback_lines` | ✅ 설계 완료 |
| Default shell | `[shell]` 섹션 | ✅ 설계 완료 |
| Key bindings | Keybindings 탭 | ✅ 설계 완료 |
| Option as Alt | `option_as_alt` | ✅ 설계 완료 |
| Hot reload | `notify` crate 기반 | ✅ 설계 완료 |
| GUI settings (⌘,) | 6탭 구조 | ✅ 설계 완료 |
| MCP security | `[mcp.security]` | ✅ 설계 완료 |
| IME settings | IME 탭 | ✅ 설계 완료 |

### 갭: Crux에 없는 Warp 기능

| 기능 | 카테고리 | 채택 가치 |
|------|----------|-----------|
| OS 테마 연동 (auto light/dark) | Appearance | **높음** |
| 세션 복원 | Session | **높음** |
| 벨 설정 (audible + visual) | Terminal | **높음** |
| Global Hotkey (Quake 모드) | System | **높음** |
| 탭 표시 설정 | Tabs | **높음** |
| 통합 GPU 선호 모드 | Performance | **중간** |
| 장시간 명령 알림 | Notifications | **중간** |
| 입력 위치 (top/bottom) | Appearance | **중간** |
| Autosuggestion | Completions | **중간** |
| 프롬프트 커스텀 | Prompt | **중간** |
| Launch Configuration | Session | **낮음** (Phase 2 이후) |
| Background Image | Appearance | **낮음** |
| Settings Sync | Cloud | **비추천** |
| 내장 AI | Feature | **비추천** |
| Session Sharing | Collaboration | **비추천** |
| Teams / Warp Drive | SaaS | **비추천** |

---

## 4. 채택 우선순위 분류

### Tier 1 — 강력 추천 (터미널 기본 품질)

#### 4.1 OS 테마 연동

- **이유**: macOS 사용자의 기본 기대치. 다크 모드 전환 시 터미널만 안 바뀌면 어색함
- **구현**: GPUI `SystemAppearance` 이벤트 → 테마 자동 전환
- **설정**: `[appearance] theme_mode = "auto" | "light" | "dark"`
- **복잡도**: 낮음 (GPUI가 이미 시스템 appearance 이벤트 제공)

#### 4.2 세션 복원

- **이유**: 파워 유저 생산성의 핵심. 탭/패널 배치를 매번 재구성하는 것은 큰 마찰
- **구현**: 종료 시 윈도우/탭/패널 상태 JSON 저장, 시작 시 복원
- **설정**: `[session] restore_on_launch = true`
- **복잡도**: 중간 (Phase 2 탭/패널 구현 이후에야 의미 있음)
- **의존성**: Phase 2 (tabs, panes)

#### 4.3 벨(Bell) 설정

- **이유**: VT 이뮬레이터에서 BEL(0x07) 처리는 필수. visual bell 옵션은 차별화 포인트
- **구현**: `alacritty_terminal`의 Bell 이벤트 → 소리 재생 또는 화면 플래시
- **설정**: `[terminal] audible_bell = false`, `visual_bell = true`, `visual_bell_duration_ms = 100`
- **복잡도**: 낮음

#### 4.4 탭 표시 설정

- **이유**: Phase 2에서 탭 구현 시 자연스럽게 필요
- **설정**: `[tabs] show_indicator = true`, `tab_title_format = "{process}: {cwd}"`
- **복잡도**: 낮음 (Phase 2와 동시 구현)
- **의존성**: Phase 2 (tabs)

#### 4.5 Global Hotkey (Quake 모드)

- **이유**: iTerm2의 킬러 기능. 개발자들이 가장 많이 언급하는 기능 중 하나
- **구현**: `NSEvent.addGlobalMonitorForEvents` 또는 `CGEvent` tap
- **설정**: `[global_hotkey] enabled = false`, `key = "Ctrl+\``", `position = "top"`, `size = 0.4`
- **복잡도**: 중간 (Accessibility 권한 필요, 앱이 백그라운드에서도 동작)
- **참고**: Ghostty도 Quake 모드 지원

### Tier 2 — 권장 (차별화 및 편의성)

#### 4.6 통합 GPU 모드

- **이유**: 맥북 배터리 절약. Metal에서 디바이스 선택 가능
- **구현**: `MTLCopyAllDevices()`에서 `isLowPower` 디바이스 선택
- **설정**: `[performance] prefer_integrated_gpu = false`
- **복잡도**: 낮음 (GPUI 레벨에서 디바이스 선택 지원 여부 확인 필요)

#### 4.7 장시간 명령 완료 알림

- **이유**: 셸 통합과 결합하면 실용적. `cargo build` 등 장시간 명령 완료 시 알림
- **구현**: OSC 133 명령 경계 + 시간 측정 → `NSUserNotification` 또는 `UNUserNotificationCenter`
- **설정**: `[notifications] enabled = true`, `long_running_command_sec = 10`
- **복잡도**: 중간 (셸 통합 필요)
- **의존성**: Phase 2 (shell integration)

#### 4.8 입력 위치 설정

- **이유**: Warp의 시그니처 UX. 옵션으로 제공하면 흥미로운 차별화
- **설정**: `[appearance] input_position = "bottom" | "top"`
- **복잡도**: 높음 (레이아웃 시스템 근본 변경 필요)
- **참고**: 실험적 기능으로 분류

#### 4.9 Autosuggestion (히스토리 기반)

- **이유**: fish shell의 킬러 기능. 셸 통합 후 히스토리 접근 가능하면 구현 가치 있음
- **설정**: `[completions] autosuggestions = true`, `source = "history"`
- **복잡도**: 높음 (셸 히스토리 파싱 + 인라인 렌더링)
- **의존성**: Phase 2 (shell integration)

#### 4.10 프롬프트 커스터마이징

- **이유**: Warp의 context chips (git 상태, 디렉토리, K8s 등) 매력적
- **설정**: `[prompt] native = false` (기본은 셸 프롬프트 존중)
- **복잡도**: 높음
- **의존성**: Phase 2 (shell integration)

### Tier 3 — 장기 고려

#### 4.11 Launch Configuration

- YAML/TOML로 윈도우/탭/패널 레이아웃 프리셋 저장
- Phase 2 + IPC 완성 후 `~/.config/crux/layouts/` 디렉토리로 구현 가능
- Crux IPC의 `crux_coordinate_panes` 도구와 자연스럽게 연결

#### 4.12 Background Image

- `[appearance] background_image = "path"`
- GPU 렌더러에서 텍스처 레이어 추가 필요
- 인상적이지만 핵심 기능은 아님

---

## 5. 제안 TOML 구조 확장

기존 `config-system.md`의 TOML 스키마에 추가할 섹션:

```toml
# ─── Tier 1: 기본 품질 ───

[appearance]
theme_mode = "auto"              # "auto" | "light" | "dark"
                                 # auto: macOS appearance 연동

[terminal]
audible_bell = false             # BEL(0x07) → 시스템 사운드
visual_bell = true               # BEL(0x07) → 화면 플래시
visual_bell_duration_ms = 100    # 플래시 지속 시간
visual_bell_color = "#ff6b6b"    # 플래시 색상

[session]
restore_on_launch = true         # 마지막 세션 윈도우/탭/패널 복원
restore_timeout_sec = 60         # 비정상 종료 후 복원 제한 시간

[tabs]
show_indicator = true            # 탭 상태 인디케이터 (활동, 벨 등)
tab_title_format = "{process}"   # 탭 타이틀 포맷

[global_hotkey]
enabled = false
key = "Ctrl+`"                   # 시스템 전역 핫키
position = "top"                 # "top" | "bottom" | "left" | "right"
screen = "current"               # "current" | "main"
size = 0.4                       # 화면 대비 비율 (0.1 ~ 1.0)
animation = true                 # 슬라이드 애니메이션

# ─── Tier 2: 차별화 ───

[performance]
prefer_integrated_gpu = false    # true → 저전력 GPU 선호 (배터리 절약)

[notifications]
enabled = true
long_running_command_sec = 10    # N초 이상 실행 시 완료 알림
show_command_in_notification = true  # 알림에 명령어 포함

[completions]
autosuggestions = false          # fish-style 히스토리 기반 자동완성
```

---

## 6. 구현 타이밍

| 시점 | 추가할 설정 | 이유 |
|------|-------------|------|
| **Phase 1과 함께** | `theme_mode`, `audible_bell`, `visual_bell` | VT 이뮬레이터 기본 기능 |
| **Phase 2와 함께** | `session restore`, `tabs`, `notifications`, `autosuggestions` | 탭/패널/셸 통합 의존 |
| **Phase 5와 함께** | `global_hotkey`, `prefer_integrated_gpu`, GUI에 전체 반영 | 설정 시스템 구축 시점 |
| **Phase 5 이후** | `launch_config`, `background_image`, `input_position` | 폴리시 단계 |

---

## 7. 채택 비추천 기능

| Warp 기능 | 비추천 이유 |
|-----------|------------|
| **내장 AI (Warp AI, Agent Mode)** | Crux는 MCP + Claude Code Agent Teams으로 AI를 외부에서 주입. 터미널에 AI를 내장하는 것은 범위 초과이며, MCP 기반 접근이 더 유연함 |
| **세션 공유 (Session Sharing)** | 클라우드 인프라 필요. 오픈소스 로컬 퍼스트 터미널에 부적합. tmux attach 또는 tmate가 대안 |
| **Teams / Warp Drive** | SaaS 모델. 로컬 퍼스트 철학과 충돌. Git 기반 dotfile 동기화가 대안 |
| **Settings Sync (클라우드)** | TOML 파일 기반이므로 dotfile 관리 도구(chezmoi, stow)로 자연스럽게 동기화 가능 |
| **내장 에디터 Vim 모드** | 터미널 앱 레벨 에디터는 셸의 readline/zle와 충돌 위험. Warp 특유의 "input editor" 개념이므로 전통적 터미널에는 불필요 |
| **Telemetry** | 오픈소스 프로젝트에서 텔레메트리는 신뢰 이슈. 필요 시 opt-in으로만 |
| **Subshell Warpify** | Warp 전용 기능. 표준 터미널에서는 의미 없음 |

---

## 8. 핵심 인사이트

### 8.1 설정 접근 방식의 차이

Warp은 **GUI-only** 설정을 사용하며, 사용자에게 설정 파일을 노출하지 않는다. 이는 진입 장벽을 낮추지만, 파워 유저의 자동화(dotfile, 스크립팅)를 어렵게 만든다.

Crux의 **TOML + GUI 양방향 동기화**는 양쪽 모두를 만족시키는 설계로, Alacritty의 텍스트 파일 접근과 Warp의 GUI 접근의 장점을 결합한다.

### 8.2 Global Hotkey의 가치

iTerm2, Guake, Yakuake 등에서 검증된 기능. macOS에서는:
- `NSEvent.addGlobalMonitorForEvents(matching:handler:)` — Accessibility 권한 필요
- `CGEvent` tap — 더 로우레벨, 모든 키 이벤트 가로채기 가능
- 앱이 백그라운드에서도 핫키를 잡아야 하므로 LSUIElement 또는 NSApplication activation 처리 필요

### 8.3 셸 통합의 레버리지 효과

Warp의 많은 기능(Blocks, notifications, autosuggestions, prompt context)은 **셸 통합(OSC 133 명령 경계 마커)**에 의존한다. Crux의 Phase 2에서 셸 통합을 구현할 때, 이 마커들을 심어두면 여러 기능의 토대가 된다.

### 8.4 Blocks 시스템에 대한 고려

Warp의 가장 혁신적인 차별화 요소는 **Blocks** (명령별 분리된 출력 블록)이다. 이는 전통적 터미널의 연속 스크롤과 근본적으로 다르며, 셸 통합에 깊이 의존한다. Crux에서 직접 구현은 범위를 벗어나지만, OSC 133 마커를 통해 향후 블록 기반 UI로 확장할 가능성은 열어두어야 한다.

---

## References

- [Warp All Features](https://www.warp.dev/all-features)
- [Warp Docs: Customizing](https://docs.warp.dev/getting-started/readme/customizing-warp)
- [Warp Docs: Themes](https://docs.warp.dev/terminal/appearance/themes)
- [Warp Docs: Text, Fonts, Cursor](https://docs.warp.dev/terminal/appearance/text-fonts-cursor)
- [Warp Docs: Opacity & Blurring](https://docs.warp.dev/terminal/appearance/size-opacity-blurring)
- [Warp Docs: Keyboard Shortcuts](https://docs.warp.dev/getting-started/keyboard-shortcuts)
- [Warp Docs: Global Hotkey](https://docs.warp.dev/terminal/windows/global-hotkey)
- [Warp Docs: Session Management](https://docs.warp.dev/terminal/sessions)
- [Warp Docs: Notifications](https://docs.warp.dev/terminal/more-features/notifications)
- [Warp Docs: Audible Bell](https://docs.warp.dev/terminal/more-features/audible-bell)
- [Warp Docs: Completions](https://docs.warp.dev/terminal/command-completions/completions)
- [Warp Docs: Vim Keybindings](https://docs.warp.dev/terminal/editor/vim)
- [Warp Docs: Privacy](https://docs.warp.dev/privacy/privacy)
- [Warp Docs: SSH](https://docs.warp.dev/terminal/warpify/ssh)
- [Warp Docs: Warp AI](https://docs.warp.dev/agents/warp-ai)
- [Warp Docs: Settings Sync](https://docs.warp.dev/terminal/more-features/settings-sync)
- [Warp Blog: How Warp Works](https://www.warp.dev/blog/how-warp-works)
- [Warp Blog: Telemetry Optional](https://www.warp.dev/blog/telemetry-now-optional-in-warp)
- Crux 내부: [config-system.md](../core/config-system.md), [PLAN.md](../../PLAN.md) Phase 5.7-5.8
