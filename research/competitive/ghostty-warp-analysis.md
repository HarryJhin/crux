---
title: "Ghostty & Warp 경쟁 분석: Crux 포지셔닝 전략"
description: "Ghostty 아키텍처 심층 분석 및 Warp 대체 전략. Crux의 차별화 포인트와 경쟁 포지셔닝 매트릭스 포함."
phase: [1, 2, 3, 4, 5, 6]
topics: [competitive-analysis, ghostty, warp, positioning, architecture, rendering]
related:
  - research/core/terminal-emulation.md
  - research/core/terminal-architecture.md
  - research/gpui/framework.md
  - research/gpui/terminal-implementations.md
  - research/integration/ipc-protocol-design.md
  - research/platform/ime-clipboard.md
  - PLAN.md
---

# Ghostty & Warp 경쟁 분석

> **목적**: Crux 터미널의 아키텍처 결정과 시장 포지셔닝을 위한 경쟁사 심층 분석
> **최종 업데이트**: 2026-02

---

## 목차

1. [Part 1: Ghostty 아키텍처 심층 분석](#part-1-ghostty-아키텍처-심층-분석)
2. [Part 2: Warp 대체 분석](#part-2-warp-대체-분석)
3. [Part 3: 경쟁 포지셔닝 매트릭스](#part-3-경쟁-포지셔닝-매트릭스)
4. [Part 4: Crux 전략적 시사점](#part-4-crux-전략적-시사점)

---

## Part 1: Ghostty 아키텍처 심층 분석

Ghostty는 Mitchell Hashimoto(HashiCorp 공동 창업자)가 개발한 고성능 크로스 플랫폼 터미널 에뮬레이터이다. MIT 라이센스, Zig로 작성되었으며, 2025년 1월 1.0 공개 출시 후 빠르게 성장하여 2025년 12월 비영리 재단(fiscal sponsorship) 전환을 발표했다. Crux의 **1차 아키텍처 참조 대상**이다.

### 1.1 코어 아키텍처: libghostty 패턴

Ghostty의 가장 핵심적인 아키텍처 결정은 **코어와 플랫폼 셸의 완전한 분리**이다.

```
┌─────────────────────────────────────────────────┐
│                 libghostty (Zig)                 │
│  ┌───────────┐ ┌──────────┐ ┌────────────────┐  │
│  │ VT Parser │ │ Font Sys │ │ Renderer Core  │  │
│  │ (SIMD)    │ │ (Shaper) │ │ (Metal/OpenGL) │  │
│  └───────────┘ └──────────┘ └────────────────┘  │
│  ┌───────────┐ ┌──────────┐ ┌────────────────┐  │
│  │ Terminal  │ │ Input    │ │ Shell          │  │
│  │ State     │ │ Handling │ │ Integration    │  │
│  └───────────┘ └──────────┘ └────────────────┘  │
│                   C ABI                          │
└─────────────┬───────────────────┬───────────────┘
              │                   │
    ┌─────────▼──────┐  ┌────────▼────────┐
    │ macOS App      │  │ Linux App       │
    │ (Swift/AppKit) │  │ (Zig/GTK4)     │
    └────────────────┘  └─────────────────┘
```

**핵심 설계 원칙:**

- **C-ABI 호환 라이브러리**: libghostty는 C 호환 ABI를 통해 어떤 언어에서든 사용 가능
- **플랫폼 네이티브 UI**: macOS는 Swift + AppKit/SwiftUI, Linux는 Zig + GTK4
- **제로 의존성 서브 라이브러리**: libghostty-vt는 libc에도 의존하지 않는 순수 VT 파서
- **모듈화된 확장 계획**: vt, input, rendering, widgets를 독립 라이브러리로 분리 예정

**Crux와의 비교:**

| 측면 | Ghostty | Crux |
|------|---------|------|
| 코어 언어 | Zig | Rust |
| VT 파서 | 자체 구현 (SIMD) | alacritty_terminal (0.25) |
| UI 프레임워크 | AppKit/SwiftUI (macOS) | GPUI (0.2.2) |
| 코어-UI 분리 | libghostty C ABI | Rust crate 경계 |
| 렌더러 | Metal/OpenGL 직접 구현 | GPUI 위임 |
| PTY | 자체 구현 | portable-pty (0.9) |

> **시사점**: Ghostty는 모든 것을 밑바닥부터 구현하는 전략. Crux는 검증된 크레이트(alacritty_terminal, GPUI)를 조합하는 전략으로 개발 속도에서 이점을 가진다.

### 1.2 VT Parser: SIMD 최적화

Ghostty의 VT 파서는 vt100.net 상태 기계 명세를 따르며 현대적 확장을 포함한다.

**SIMD 최적화 전략:**

```
UTF-8 입력 스트림
    │
    ▼
simd.vt.utf8DecodeUntilControlSeq()  ← SIMD 벡터 처리
    │                                   (여러 코드포인트 동시 디코딩)
    │
    ├─ 일반 텍스트 → 고속 벌크 처리
    │
    └─ ESC (0x1B) 감지 → 스칼라 상태 기계로 전환
                          │
                          ▼
                    Action enum 생성
                          │
                          ▼
                    터미널 상태 업데이트
```

- **일반 텍스트는 SIMD로 벌크 처리**: ESC를 만날 때까지 여러 바이트를 한 번에 디코딩
- **제어 시퀀스는 스칼라 처리**: 정확성이 중요한 파싱은 전통적 상태 기계 사용
- **메모리 최적화**: 셀별로 전체 스타일 정보를 저장하지 않고 고유 스타일 ID를 참조하는 look-aside 방식. 고유 스타일이 적을 때 메모리 사용량이 크게 감소 (Alacritty는 셀마다 전체 스타일 복사)

**libghostty-vt 독립 라이브러리 (2025.09 출시):**

- Zig + C API로 패키징
- 제로 의존성 (libc 불필요)
- macOS, Linux, Windows, WebAssembly 지원
- Kitty Graphics Protocol, tmux Control Mode 등 고급 프로토콜 지원

**Crux 전략:**

Crux는 `alacritty_terminal 0.25`를 사용한다. 이미 프로덕션 검증된 파서이며, Alacritty와 동일한 VT 호환성을 상속받는다. SIMD 최적화는 없지만 대부분의 사용 시나리오에서 충분한 성능을 제공한다. 향후 성능 병목이 확인되면 핫 패스에 SIMD를 도입하거나 libghostty-vt 라이브러리를 C FFI로 통합하는 옵션도 가능하다.

### 1.3 렌더링 파이프라인

**Metal 렌더러 (macOS):**

Ghostty는 **iTerm2를 제외하고 Metal을 직접 사용하는 유일한 터미널**이다. 더 나아가 **Metal 렌더러에서 리가처를 지원하는 유일한 터미널**이기도 하다 (iTerm2는 리가처 활성화 시 CPU 렌더러로 폴백).

```
Terminal State → Font Shaping → GPU Draw Commands → Metal/OpenGL
     │              │                │
     │              │                └─ v1.2에서 코어 로직 통합
     │              │                   (Metal/OpenGL 공유 렌더러)
     │              │
     │              └─ CoreText (macOS) / FreeType+HarfBuzz (Linux)
     │
     └─ 적절한 locking으로 터미널 상태 읽기
```

**v1.2.0 렌더러 리팩터링 (2025.09):**

- Metal과 OpenGL 렌더러의 코어 로직을 공유하도록 재작성
- 기능 동등성(feature parity) 보장
- 유지보수성 향상 및 향후 개선 속도 증가

**성능 벤치마크:**

| 시나리오 | Ghostty | 대비 |
|----------|---------|------|
| 플레인 텍스트 읽기 | 기준 | iTerm2/Kitty 대비 4x, Terminal.app 대비 2x |
| 멀티 탭 메모리 | ~150MB (10탭) | iTerm2 대비 48% 절감 |
| FPS (일반) | >144 FPS | Alacritty와 동급 |
| "DOOM Fire" 벤치마크 | 최고 수준 | 베타 테스터 기여로 40% 추가 향상 |

> **주의**: 임베디드/Linux 환경에서는 Ghostty ~300MB, Alacritty ~250MB, GNOME Terminal ~150MB로 Ghostty가 메모리를 더 사용한다는 2026년 벤치마크도 있음. 플랫폼과 워크로드에 따라 결과가 달라진다.

**Crux 전략:**

GPUI가 Metal 렌더링을 전담하므로 Crux는 렌더러를 직접 구현할 필요가 없다. Zed 에디터 수준의 렌더링 품질을 상속받되, 터미널 특화 최적화(damage tracking, 셀 기반 업데이트)에 집중한다.

### 1.4 폰트 시스템

Mitchell Hashimoto는 "Ghostty는 70%가 폰트 렌더링 엔진이고 30%가 터미널 에뮬레이터"라고 농담할 정도로 폰트 처리가 핵심이다.

**아키텍처:**

```
Font Discovery → Font Loading → Text Shaping → Glyph Rasterization → Cache
     │               │              │                │                  │
     │               │              │                │                  └─ SharedGridSet
     │               │              │                │                     (서피스 간 공유)
     │               │              │                │
     │               │              │                └─ CoreText (macOS)
     │               │              │                   FreeType (Linux)
     │               │              │
     │               │              └─ CoreText/HarfBuzz
     │               │                 (플러거블 백엔드)
     │               │
     │               └─ 멀티 백엔드: CoreText, FreeType,
     │                  CoreText+FreeType, CoreText+HarfBuzz
     │
     └─ CoreText (macOS) / Fontconfig (Linux)
```

**CJK 폰트 처리:**

- `font-family`를 여러 번 설정하여 폴백 체인 구성 가능
- v1.2.0에서 폴백 폰트 크기 자동 조정 기능 추가
- **알려진 이슈**: CJK 전용 폰트를 명시하지 않으면 폴백 로직이 double-em-width 추정치에 의존하여 글자가 과도하게 크게 표시됨 (issue #8712)
- 리가처 지원: `-calt`, `-liga`, `-dlig` 등으로 세밀한 제어 가능
- Shaping break 설정으로 리가처 형성 범위 조절 가능

**Crux 전략:**

GPUI의 폰트 시스템을 사용하되, CJK 폰트 폴백과 크기 조정은 별도로 구현해야 한다. Ghostty가 겪은 CJK 폴백 문제(과도한 글자 크기)를 사전에 방지하기 위해 `ic-width` 기반 정확한 셀 크기 계산이 필요하다.

### 1.5 설정 시스템

**설정 파일:**

- 위치: `~/.config/ghostty/config` (XDG 표준 준수)
- 형식: `key = value` (단순 텍스트, TOML/YAML이 아님)
- **핫 리로드**: `Cmd+Shift+,` (macOS) / `Ctrl+Shift+,` (Linux)
- 런타임 적용 가능한 설정과 재시작 필요한 설정이 구분됨

**키바인딩 시스템:**

```
keybind = ctrl+shift+c=copy_to_clipboard
keybind = ctrl+a>x=close_surface     # 시퀀스 키
keybind = global:ctrl+grave=toggle_quick_terminal  # 글로벌 (macOS만)
```

- 단일 키, 수정자+키, 시퀀스 키, 글로벌 키 지원
- macOS에서 글로벌 키바인딩은 접근성 권한 필요
- 200개 이상의 액션 레퍼런스

**macOS Option 키 처리:**

```
macos-option-as-alt = true    # 양쪽 Option → Alt
macos-option-as-alt = left    # 왼쪽만 Alt
macos-option-as-alt = right   # 오른쪽만 Alt
```

이 설정은 Option+Arrow 단어 이동, Option+Backspace 단어 삭제 등에 필수. 활성화하면 Unicode 입력(예: Option+e → accent)이 비활성화됨.

**Crux 전략:**

Phase 5에서 설정 시스템 구현 예정. Ghostty의 단순한 `key=value` 형식보다는 TOML을 채택하여 중첩 설정과 타입 안전성을 확보하는 것이 Rust 생태계에서 자연스럽다. `macos-option-as-alt` 동일 기능은 필수.

### 1.6 IME 처리

**macOS IME (NSTextInputClient):**

- bash, zsh, fish, elvish 자동 셸 통합
- v1.1.0에서 CJK IME 안정성 대폭 개선
- AquaSKK, macSKK 등 일본어 IME 기본 지원
- `ctrl+h` 등 제어 문자가 IME와 올바르게 상호작용
- `window-padding` 설정 시 IME 창 위치 수정

**Linux IME:**

- fcitx, ibus 호환성 (X11, Wayland)
- v1.1.0에서 Linux CJK IME 기본 지원 추가 (이전에는 작동하지 않음)
- ibus 1.5.29 dead key 버그 수정

**알려진 이슈:**

- **IME 후보 창 위치 문제**: CJK 입력 시 후보 창이 커서 위치가 아닌 화면 하단-좌측에 표시되는 경우가 있음. `firstRect` 콜백이 실제 커서 위치를 정확히 반환하지 못하는 문제.
- 일부 입력 메소드의 키바인드가 관통(penetrate)하는 문제 (Discussion #2628)

**Crux 차별화 기회:**

Ghostty의 IME 처리는 "충분히 좋은" 수준이지만 완벽하지 않다. Crux는 **한국어/CJK IME를 1등 시민으로** 설계하여:

1. Preedit 텍스트를 오버레이로 렌더링 (PTY에 절대 전달하지 않음)
2. `firstRect` 정확한 구현으로 후보 창 위치 보장
3. `objc2` + `objc2-app-kit`으로 NSTextInputClient 완전 구현
4. Vim 모드 전환 시 IME 자동 전환

### 1.7 Terminfo 전략

**xterm-ghostty 결정 과정:**

프라이빗 베타에서 `TERM=ghostty`를 시도했으나 너무 많은 애플리케이션이 깨졌다. `xterm-` 접두사가 필요한 이유:

- 많은 프로그램이 `TERM` 값에서 "xterm" 문자열을 검색하여 기능 추정
- 순수 `ghostty`로는 전체 생태계를 수정하는 것이 비현실적
- Vim 9.0이 Kitty Keyboard Protocol을 지원하지만 terminfo DB를 무시하고 터미널 이름을 하드코딩

**SSH 호환성 (v1.2.0):**

```
shell-integration-features = ssh-env,ssh-terminfo
```

- `ssh-env`: 환경 변수 호환성 설정
- `ssh-terminfo`: 원격 호스트에 terminfo 자동 설치
- SSH 연결 시 `xterm-ghostty` terminfo가 없으면 "missing or unsuitable terminal" 에러 발생

**Crux 전략:**

Crux는 `TERM=xterm-crux`를 사용한다. Ghostty의 학습을 그대로 적용:
- `xterm-` 접두사 필수 (CLAUDE.md에 이미 명시)
- SSH terminfo 자동 설치 기능 Phase 2에서 구현
- `tic -x -e xterm-crux,crux,crux-direct extra/crux.terminfo`로 로컬 설치

### 1.8 탭 & 스플릿

**네이티브 탭:**

- macOS에서 **네이티브 NSWindow 탭바** 사용 (커스텀 드로잉 아님)
- 플랫폼 네이티브 UI 철학의 핵심 요소
- 탭별 독립 터미널 상태

**스플릿 관리:**

- 수평/수직 분할 지원
- 스플릿 줌 기능: 현재 스플릿을 탭 전체 크기로 확대/축소
- 분할 방향 지정 가능
- 하지만 **리사이즈 핸들, 드래그 앤 드롭 재배치는 미지원**

**Crux 차별화:**

GPUI의 `gpui-component`(`DockArea`, `ResizablePanel`, `Tabs`)를 활용하면:
- 드래그 앤 드롭으로 탭/스플릿 재배치
- 유연한 DockArea 레이아웃 (Zed 수준)
- Claude Code Agent Teams용 프로그래매틱 pane 제어

### 1.9 셸 통합

**자동 셸 통합:**

- bash, zsh, fish, elvish 지원
- Ghostty 실행 시 자동으로 셸 통합 스크립트 주입
- 명시적 설정 없이 기본 동작

**기능:**

| 기능 | 구현 방식 |
|------|----------|
| 프롬프트 마킹 | OSC 133 (semantic prompt) |
| 현재 디렉토리 추적 | OSC 7 (`kitty-shell-cwd://`) |
| 새 터미널 디렉토리 상속 | 이전 포커스 터미널의 CWD |
| sudo terminfo 보존 | sudo 래퍼 (기본 비활성) |
| SSH terminfo | v1.2.0에서 자동 설치 |

### 1.10 그래픽 프로토콜

**Kitty Graphics Protocol:**

- 완전 지원
- 인라인 이미지 렌더링 가능

**Sixel:**

- **의도적으로 미지원** (설계 결정)
- Mitchell Hashimoto의 근거:
  1. Sixel은 명세되지 않은 엣지 케이스가 너무 많음
  2. libsixel 라이브러리 품질 문제
  3. 성능 영향이 불확실하지만 제로는 아님
  4. Kitty Graphics Protocol이 이미 더 나은 대안을 제공

**Crux 전략:**

Kitty Graphics Protocol 우선 구현 (Phase 4), Sixel은 선택적. Ghostty와 동일한 전략이 합리적.

---

## Part 2: Warp 대체 분석

Warp는 Zach Lloyd가 설립한 Rust 기반 GPU 가속 터미널로, 2022년 공개 베타 이후 "AI-Powered Terminal"에서 "Agentic Development Environment"로 포지셔닝을 변경했다. **Crux의 가장 큰 가치 제안은 Warp를 대체하는 것**이다.

### 2.1 Warp의 핵심 기술 아키텍처

**렌더링:**

```
Element Tree → Primitives (rect, image, glyph) → Metal Shaders
                                                    │
                                                    └─ <250줄 셰이더 코드
                                                       (백엔드 교체 용이)
```

- Rust + Metal (macOS), 추후 OpenGL/WebGL 계획
- 프리미티브 추상화 위에 고수준 UI 요소 구성
- 144+ FPS 유지

**내부 데이터 모델:**

- Alacritty의 VT100 그리드 모델을 Rust로 포크하여 시작
- 블록 개념을 그리드 위에 추가 레이어로 구현
- DCS (Device Control String) + 인코딩된 JSON으로 셸과 통신

### 2.2 사용자가 좋아하는 Warp 기능

#### 2.2.1 블록 (Blocks) — 가장 과소평가된 킬러 기능

블록은 명령어와 출력을 하나의 원자적 단위로 그룹화한다.

**기술적 구현:**

```
셸 hook (precmd/preexec)
    │
    ▼
DCS 전송 (인코딩된 JSON 메타데이터)
    │
    ▼
Warp 파싱 → 새 Block 객체 생성
    │
    ├─ 명령어 입력 영역
    ├─ 출력 영역
    └─ 블록 액션 (복사, 검색, 공유 등)
```

- OSC 133 semantic prompt와 유사하지만 Warp 독자 프로토콜
- `precmd`(프롬프트 전) / `preexec`(명령 실행 전) 셸 훅 사용
- 블록별 복사, 검색, 공유, AI 분석 가능

> **핵심 인사이트**: "블록이 Warp의 가장 덜 마케팅되는 기능이지만 실제로 작업 방식을 바꾸는 진짜 킬러 기능"이라는 평가가 지배적. AI보다 블록이 Warp의 진짜 가치.

#### 2.2.2 AI 에이전트 (Warp 2.0)

**Agent Mode:**

- 자연어로 작업 설명 → 실행 가능한 명령어로 변환
- 복잡한 요청을 단계별로 분해
- 실패한 명령어 자동 분석 및 디버깅 제안
- 다중 에이전트 동시 실행 가능

**Warp Pair:**

- AI와 페어 프로그래밍 경험 제공
- 의사결정에 사용자를 적극 참여시킴

**Warp Drive:**

- 워크플로우, 명령어, 프롬프트, 환경 설정을 팀 단위로 공유
- AI가 공유된 컨텍스트를 활용하여 더 정확한 제안

#### 2.2.3 모던 입력 영역

- 출력과 분리된 IDE 스타일 입력 에디터
- 자동완성, 구문 강조
- 멀티라인 편집 지원

#### 2.2.4 기타

- 테마/커스터마이징 (다양한 테마 지원)
- Windows, macOS, Linux 크로스 플랫폼
- 풍부한 키보드 단축키 시스템

### 2.3 Warp의 문제점 (사용자 이탈 요인)

#### 2.3.1 프라이버시 & 텔레메트리

| 이슈 | 상태 (2026) |
|------|-------------|
| 계정/로그인 필수 | **해제됨** (2024년부터 불필요) |
| 텔레메트리 | 기본 **활성화**, 수동 opt-out 필요 |
| 데이터 수집 | Segment를 통한 메타데이터 수집 |
| 콘솔 출력 수집 | 하지 않는다고 명시 |
| 오프라인 시 텔레메트리 | 비활성화해도 수동으로 해제해야 함 |

- 초기 Segment 데이터 전송 발견(issue #1346)으로 대규모 신뢰 위기
- "VC-funded, closed-source terminal"이라는 비판
- 로그인 필수 정책이 추천 장벽으로 작용했던 역사

#### 2.3.2 가격 정책 혼란

| 기간 | 모델 |
|------|------|
| ~2024 | 무료 + Pro/Team 구독 |
| 2025.10 | Build 플랜 ($20/월, 1500 크레딧) 도입 |
| 2025.12 | 기존 구독자 전환 시작 |

- **무료 티어**: 첫 2개월 150 크레딧/월, 이후 75 크레딧/월
- **터미널 기능은 무료**, AI 기능만 유료
- 소비 기반(consumption-based) 모델로 전환 → 비용 예측 불가 불만
- 가격 정책 반복 변경으로 사용자 신뢰 하락

#### 2.3.3 CJK/한국어 입력 문제

Warp의 한국어 입력 문제는 **심각한 수준**이다:

| 이슈 | GitHub Issue |
|------|-------------|
| 한국어 입력 표시/타이핑 문제 | #428 |
| 한국어 입력 지연 (영어는 즉시) | #6749 |
| IME 지원 전반적 문제 | #6891 |
| 비영어 입력 소스에서 단축키 미작동 | #341, #8547 |
| 사이드바 중국어 파일명 표시 오류 | #7436 |
| Linux에서 한국어 자모 분리 표시 | #6591 |

**구체적 증상:**

1. **입력 지연**: 한국어 입력이 스페이스바를 눌러야만 확정됨
2. **미완성 글자 미표시**: 단일 키 입력 시 미완성 한글이 전혀 표시되지 않음 (다른 에디터와 다른 동작)
3. **단축키 호환성**: 한국어 IME 활성 상태에서 `Cmd+P`가 `Cmd+ㅔ`로 인식되지 않아 단축키 미작동
4. **IME 토글 문제**: `Cmd+I` 토글이 영어 입력 소스에서만 작동

> **핵심 기회**: Warp의 한국어 입력 문제는 구조적이다. Crux가 한국어 IME를 완벽하게 지원하면 한국 개발자 시장에서 Warp를 직접 대체할 수 있다.

#### 2.3.4 폐쇄성

- **클로즈드 소스**: 코드 감사 불가
- **벤더 종속**: Warp 서버 의존적 기능들
- **커스터마이징 제한**: 전통 터미널 대비 낮은 자유도

### 2.4 Crux가 복제할 수 있는 것

| Warp 기능 | Crux 구현 전략 | Phase |
|-----------|---------------|-------|
| 블록 (Blocks) | OSC 133 semantic zones + 셸 통합 | 2 |
| AI 통합 | Claude Code Agent Teams (IPC) | 2, 5 |
| GPU 가속 렌더링 | GPUI Metal 렌더러 | 1 |
| 스플릿 페인 | DockArea + ResizablePanel | 2 |
| 리치 클립보드 | NSPasteboard + 드래그앤드롭 | 3 |
| 모던 입력 | GPUI 입력 컴포넌트 | 2 |
| 워크플로우/공유 | JSON-RPC IPC + 설정 파일 | 5 |

### 2.5 Crux의 고유 차별화 포인트

**1. 오픈소스 (MIT + Apache 2.0)**

- 코드 감사 가능
- 커뮤니티 기여
- 벤더 종속 없음
- 계정/로그인 불필요
- 텔레메트리 제로

**2. 한국어/CJK IME 1등 시민**

- NSTextInputClient 완전 구현 (objc2)
- Preedit 오버레이 렌더링 (PTY 격리)
- firstRect 정확한 구현
- Vim 모드 전환 시 IME 자동 전환
- Warp가 구조적으로 해결하지 못한 문제 영역

**3. Claude Code 네이티브 통합**

- 다른 어떤 터미널도 제공하지 않는 기능
- Unix 소켓 + JSON-RPC 2.0 IPC
- 프로그래매틱 pane 제어 (13개 PaneBackend 메서드)
- Agent Teams가 독립 pane에서 병렬 작업

**4. GPUI 생태계**

- Zed 에디터 수준의 렌더링 품질
- 60개 이상의 기성 위젯
- DockArea 기반 유연한 레이아웃

**5. Ghostty 수준 성능 목표**

- GPUI Metal 렌더링
- Damage tracking (alacritty_terminal 상속)
- 이벤트 배칭 (100 이벤트 / 4ms 윈도우)

---

## Part 3: 경쟁 포지셔닝 매트릭스

### 3.1 종합 비교표

| 기능 | Crux | Warp | Ghostty | Kitty | iTerm2 | Alacritty |
|------|------|------|---------|-------|--------|-----------|
| **라이센스** | MIT+Apache | 독점 | MIT | GPL3 | GPL2 | Apache |
| **언어** | Rust | Rust | Zig | C+Python | Obj-C | Rust |
| **GPU 가속** | Metal (GPUI) | Metal | Metal/OpenGL | OpenGL | Metal | OpenGL |
| **계정 필요** | No | No (변경됨) | No | No | No | No |
| **텔레메트리** | 없음 | 기본 활성 | 없음 | 없음 | 없음 | 없음 |
| **가격** | 무료 | 프리미엄($20/월) | 무료 | 무료 | 무료 | 무료 |
| **플랫폼** | macOS | Win/Mac/Linux | Mac/Linux | Mac/Linux | macOS | 크로스 |
| **VT 파서** | alacritty_terminal | Alacritty 포크 | 자체 (SIMD) | 자체 | 자체 | 자체 |
| **폰트 리가처** | GPUI 지원 | 지원 | Metal에서 지원 | 지원 | CPU 폴백 | 미지원 |
| **탭** | DockArea | 커스텀 | 네이티브 | 커스텀 | 커스텀 | 미지원 |
| **스플릿** | DockArea | 커스텀 | 네이티브 | 자체 | 자체 | 미지원 |
| **드래그 재배치** | DockArea | 제한적 | 미지원 | 미지원 | 지원 | N/A |
| **블록/시맨틱** | OSC 133 (계획) | DCS 독자 | OSC 133 | 미지원 | 미지원 | 미지원 |
| **AI 통합** | Claude Code IPC | 내장 AI | 없음 | 없음 | 없음 | 없음 |
| **한국어 IME** | 1등 시민 (계획) | 심각한 문제 | 양호 | 양호 | 양호 | 기본 |
| **CJK 폰트** | 집중 지원 (계획) | 문제 있음 | 양호 (이슈 있음) | 양호 | 양호 | 기본 |
| **그래픽 프로토콜** | Kitty (계획) | 미지원 | Kitty | Kitty+Sixel | Sixel+iTerm2 | 미지원 |
| **Kitty 키보드** | 계획 | 미지원 | 지원 | 지원 | 미지원 | 미지원 |
| **tmux 통합** | 계획 | 제한적 | 미지원 (계획) | 미지원 | CC모드 지원 | 미지원 |
| **셸 통합** | 계획 | 자체 훅 | 자동 주입 | 자체 | 자체 | 없음 |
| **설정 핫리로드** | 계획 | 지원 | 지원 | 지원 | 지원 | 지원 |
| **Quick Terminal** | 계획 | 지원 | 지원 (v1.2) | 미지원 | HotKey | 미지원 |

### 3.2 성능 비교 (추정)

| 메트릭 | Crux (목표) | Warp | Ghostty | Kitty | iTerm2 | Alacritty |
|--------|-------------|------|---------|-------|--------|-----------|
| 플레인 텍스트 처리량 | Ghostty급 | 고속 | 최고 | 고속 | 보통 | 최고 |
| 메모리 (기본) | <100MB | ~200MB | ~80MB | ~50MB | ~300MB | ~30MB |
| 메모리 (10탭) | <200MB | ~400MB | ~150MB | ~100MB | ~500MB | N/A |
| 시작 시간 | <200ms | ~500ms | <100ms | <100ms | ~800ms | <50ms |
| FPS (일반) | >120 | >144 | >144 | >120 | ~60 | >120 |

> **주의**: Crux 수치는 GPUI 기반 추정 목표치. 실제 벤치마크는 Phase 1 완료 후 측정 필요.

### 3.3 타겟 사용자 세그먼트

| 세그먼트 | 현재 도구 | Crux 가치 제안 | 전환 동기 |
|----------|----------|---------------|----------|
| **한국 개발자** | iTerm2/Warp | 완벽한 한글 IME + 현대적 UX | Warp 한글 버그, iTerm2 노후화 |
| **Claude Code 사용자** | Ghostty/iTerm2 | 네이티브 Agent Teams 통합 | 프로그래매틱 pane 제어 |
| **Warp 이탈자** | Warp → ? | 오픈소스 + 블록 + AI (무료) | 텔레메트리, 가격, 폐쇄성 |
| **성능 중시** | Alacritty/Kitty | GPU 가속 + 풍부한 기능 | 기능 부족 (탭, IME) |
| **Zed 사용자** | 다양 | GPUI 일관성, Zed 통합 가능성 | 일관된 UX 경험 |

---

## Part 4: Crux 전략적 시사점

### 4.1 Ghostty에서 배울 것

| 교훈 | 적용 |
|------|------|
| `xterm-` 접두사 필수 | `TERM=xterm-crux` (이미 적용) |
| 네이티브 UI 철학 | GPUI로 macOS 네이티브 경험 |
| 폰트가 핵심 복잡도 | CJK 폰트 처리 사전 설계 |
| 셸 통합 자동화 | Phase 2에서 자동 주입 구현 |
| Sixel 배제 가능 | Kitty Graphics Protocol 우선 |
| SSH terminfo | Phase 2에서 자동 설치 구현 |
| IME 후보 창 위치 | `firstRect` 정확 구현 필수 |

### 4.2 Warp를 이기는 전략

```
Warp의 강점           Crux의 대응
──────────────────    ──────────────────
블록 시스템       →    OSC 133 표준 기반 구현
AI 에이전트       →    Claude Code 네이티브 통합 (더 강력)
GPU 렌더링       →    GPUI Metal (동급)
팀 협업          →    오픈소스 + IPC 프로토콜
크로스 플랫폼    →    macOS 집중 (품질 우선)

Warp의 약점           Crux의 차별화
──────────────────    ──────────────────
한국어 IME 깨짐  →    1등 시민 지원
텔레메트리       →    제로 텔레메트리
폐쇄 소스       →    MIT + Apache 2.0
가격 변동       →    완전 무료
계정 히스토리   →    계정 불필요
```

### 4.3 Phase별 경쟁력 확보 로드맵

| Phase | 경쟁력 |
|-------|--------|
| **Phase 1** | 기본 터미널 MVP — Alacritty급 기본기 |
| **Phase 2** | 탭/스플릿/IPC — Ghostty급 기능 + Claude Code 통합 시작 |
| **Phase 3** | 한국어 IME — **Warp 대비 결정적 차별화** |
| **Phase 4** | 그래픽/Kitty 키보드 — Kitty/Ghostty급 호환성 |
| **Phase 5** | tmux/Claude Code FR — **고유 가치 제안 완성** |
| **Phase 6** | 배포 — Homebrew/공증으로 사용자 접근성 |

### 4.4 핵심 메시지

> **Crux = Ghostty의 성능 + Warp의 혁신 + 완벽한 한국어 + Claude Code 네이티브**
>
> 오픈소스, 무료, 텔레메트리 제로.

---

## 참고 자료

### Ghostty

- [Ghostty 공식 문서](https://ghostty.org/docs)
- [Ghostty GitHub](https://github.com/ghostty-org/ghostty)
- [libghostty 발표](https://mitchellh.com/writing/libghostty-is-coming)
- [Ghostty 1.0 회고](https://mitchellh.com/writing/ghostty-1-0-reflection)
- [Ghostty DeepWiki](https://deepwiki.com/ghostty-org/ghostty)
- [Ghostty 1.2.0 릴리스 노트](https://ghostty.org/docs/install/release-notes/1-2-0)
- [Ghostty 1.1.0 릴리스 노트](https://ghostty.org/docs/install/release-notes/1-1-0)
- [Ghostty 성능 논의](https://github.com/ghostty-org/ghostty/discussions/4837)
- [Ghostty CJK 폰트 이슈](https://github.com/ghostty-org/ghostty/issues/8712)
- [Ghostty IME 논의](https://github.com/ghostty-org/ghostty/discussions/2628)
- [Ghostty Sixel 논의](https://github.com/ghostty-org/ghostty/discussions/2496)

### Warp

- [Warp 공식](https://www.warp.dev)
- [Warp 작동 원리](https://www.warp.dev/blog/how-warp-works)
- [Warp 블록 문서](https://docs.warp.dev/terminal/blocks)
- [Warp AI 에이전트](https://docs.warp.dev/agents/using-agents)
- [Warp 프라이버시](https://docs.warp.dev/privacy/privacy)
- [Warp 가격 변경](https://www.warp.dev/blog/warp-new-pricing-flexibility-byok)
- [Warp 로그인 요구 해제](https://www.warp.dev/blog/lifting-login-requirement)
- [Warp 한국어 이슈 #428](https://github.com/warpdotdev/Warp/issues/428)
- [Warp 한국어 지연 #6749](https://github.com/warpdotdev/warp/issues/6749)
- [Warp IME 이슈 #6891](https://github.com/warpdotdev/warp/issues/6891)
- [Warp 텔레메트리 이슈 #1346](https://github.com/warpdotdev/Warp/issues/1346)

### 비교 분석

- [터미널 호환성 매트릭스](https://tmuxai.dev/terminal-compatibility/)
- [Modern Terminals Showdown (Alacritty, Kitty, Ghostty)](https://blog.codeminer42.com/modern-terminals-alacritty-kitty-and-ghostty/)
- [macOS 터미널 선택 가이드 2025](https://medium.com/@dynamicy/choosing-a-terminal-on-macos-2025-iterm2-vs-ghostty-vs-wezterm-vs-kitty-vs-alacritty-d6a5e42fd8b3)
- [Warp 오픈소스 대안](https://openalternative.co/alternatives/warp)
- [Is Warp Worth It 2026](https://www.isitworth.site/reviews/warp-terminal)
