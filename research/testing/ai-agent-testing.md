---
title: "AI Agent Testing Infrastructure for Crux"
description: "How Claude Code and AI agents can test the Crux terminal emulator — testing MCP tools, self-testing architecture, VT conformance, visual regression, CI/CD integration, headless GPU challenges"
date: 2026-02-12
phase: [2, 5]
topics: [testing, mcp, claude-code, ci-cd, vttest, esctest, visual-regression, gpui]
status: final
related:
  - ../integration/mcp-integration.md
  - ../integration/claude-code-strategy.md
  - ../core/terminal-emulation.md
  - ../gpui/framework.md
---

# AI Agent Testing Infrastructure for Crux

> 작성일: 2026-02-12
> 목적: Claude Code가 Crux 터미널 에뮬레이터를 자율적으로 테스트하기 위한 인프라 설계

---

## 목차

1. [문제 정의](#1-문제-정의)
2. [다른 터미널의 테스트 방법](#2-다른-터미널의-테스트-방법)
3. [터미널 테스트 도구](#3-터미널-테스트-도구)
4. [Claude Code의 현재 능력과 한계](#4-claude-code의-현재-능력과-한계)
5. [테스팅 MCP 도구 설계](#5-테스팅-mcp-도구-설계)
6. [셀프 테스팅 아키텍처](#6-셀프-테스팅-아키텍처)
7. [테스트 시나리오](#7-테스트-시나리오)
8. [비주얼 리그레션 테스팅](#8-비주얼-리그레션-테스팅)
9. [CI/CD 통합](#9-cicd-통합)
10. [권장 테스팅 스택](#10-권장-테스팅-스택)

---

## 1. 문제 정의

### 핵심 도전

Claude Code는 **텍스트 기반 AI 에이전트**다. 눈도 없고 마우스도 없다. 그런데 테스트 대상은 **GPU 렌더링 GUI 애플리케이션**이다.

```
Claude Code (text-only AI)
    ↓ 어떻게?
Crux Terminal (Metal GPU rendering, GPUI, macOS native)
```

### 3계층 테스팅 모델

| 계층 | 역할 | Claude Code 접근 방법 |
|------|------|----------------------|
| **Control** (제어) | 입력 전송, 명령 실행 | MCP 도구: `crux_send_keys`, `crux_execute_command` |
| **Observe** (관찰) | 상태 검사, 결과 확인 | MCP 도구: `crux_get_cell`, `crux_get_grid`, `crux_screenshot` |
| **Automate** (자동화) | 테스트 생성, 실행, 보고 | Bash 스크립트 + MCP 도구 조합 |

**핵심 원칙**: Crux는 자신의 내부 상태를 **프로그래밍 인터페이스로 노출**해야 한다. MCP 서버는 단순한 편의 기능이 아니라 **테스트 인프라의 핵심**이다.

---

## 2. 다른 터미널의 테스트 방법

### Alacritty

- **성능 벤치마크**: [vtebench](https://github.com/alacritty/alacritty) — 터미널 처리량 측정
- **VT 파서 테스트**: [alacritty/vte](https://github.com/alacritty/vte) 크레이트의 유닛 테스트
- **레퍼런스 테스트**: `--ref-test` 플래그로 출력 → `tests/ref/` 디렉토리에 기대 결과 저장
- **방식**: 성능 중심, VTE 상태 머신 검증

### Ghostty

- **적합성 중심**: 동작 기준 = (1) 표준 (2) xterm (3) 다른 인기 터미널
- **xterm 감사**: xterm과의 포괄적 비교 + 적합성 테스트 케이스
- **테스트 실행**: `zig build run -Dconformance=<name>`
- **방식**: 표준 준수 우선, 1년+ 베타 테스트

### WezTerm

- **워크스페이스 테스트**: 19+ 크레이트 각각 유닛 테스트
- **termwiz 라운드트립**: 이스케이프 시퀀스 인코드 → 디코드 → 비교
- **방식**: 크레이트별 독립 테스트, encode/decode 왕복 검증

### 비교 매트릭스

| 터미널 | 유닛 테스트 | 적합성 테스트 | 성능 벤치마크 | 비주얼 리그레션 |
|--------|------------|--------------|--------------|----------------|
| Alacritty | cargo test + ref | vtebench | vtebench | No |
| Ghostty | zig test | xterm conformance | No (public) | No |
| WezTerm | cargo test + termwiz | No (public) | No (public) | No |
| **Crux (계획)** | **cargo test + insta** | **esctest2** | **vtebench** | **Zed visual test** |

---

## 3. 터미널 테스트 도구

### esctest2 — 자동화된 적합성 테스트 (최우선)

- **제작**: George Nachman (iTerm2 저자), Thomas E. Dickey (xterm 관리자) 유지보수
- **기능**: 터미널이 이론적 이상과 얼마나 일치하는지 **자동** 검증
- **장점**: 수동 화면 확인 없이 CI에서 실행 가능
- **활용**: Crux의 **주 적합성 테스트 스위트**

```bash
# esctest2 실행 예시
git clone https://github.com/ThomasDickey/esctest2
cd esctest2
./run_tests.sh --terminal=crux
```

### vttest — 수동 VT100/VT220 검증

- **역사**: 1983-85년 Per Lindberg 작성, xterm 확장 포함
- **기능**: VT100/VT102/VT220 기능 테스트 (메뉴 기반)
- **한계**: 사람이 화면을 보고 판단해야 함 (자동화 불가)
- **활용**: 릴리즈 전 수동 QA 체크리스트

### vtebench — 성능 벤치마크

- **제작**: Alacritty 팀
- **기능**: 터미널 에뮬레이터 처리량 정량화
- **활용**: 성능 리그레션 감지, 타 터미널과 비교

### termbench-pro — 고급 벤치마크

- **제작**: Contour 터미널 팀
- **기능**: 더 세밀한 벤치마크 시나리오
- **활용**: vtebench 보완

### expectrl — Rust PTY 인터랙션 테스트

- **역할**: Don Libes' Expect의 Rust 구현
- **기능**: PTY에서 자식 프로세스 제어, 패턴 매칭
- **활용**: 쉘 인터랙션, IME 플로우, 명령 실행 테스트

```rust
use expectrl::{spawn, Expect};

#[test]
fn test_shell_prompt() {
    let mut session = spawn("crux --test-mode").unwrap();
    session.expect("$").unwrap();
    session.send_line("echo hello").unwrap();
    session.expect("hello").unwrap();
}
```

### insta — 스냅샷 테스트

- **기능**: `assert_snapshot!` 매크로로 출력 스냅샷 저장/비교
- **워크플로우**: 테스트 실행 → 실패 → `cargo insta review` → 수락/거부
- **활용**: 터미널 그리드 상태, ANSI 시퀀스 출력 검증

---

## 4. Claude Code의 현재 능력과 한계

### 할 수 있는 것

| 능력 | 터미널 테스트 활용 |
|------|-------------------|
| Shell 명령 실행 (Bash) | 테스트 스크립트 실행, Crux 시작/중지 |
| 파일 읽기/쓰기 | 로그, 설정, golden state 파일 검증 |
| MCP 도구 호출 | Crux IPC/MCP 도구로 상태 검사 |
| 스크린샷 분석 (비전) | 렌더링 결과 이미지 파일 분석 |
| 텍스트 패턴 매칭 | 로그에서 에러/경고 검색 |
| Git 조작 | 리그레션 bisect, 브랜치 비교 |

### 할 수 없는 것 → 대안

| 한계 | 대안 |
|------|------|
| GUI 직접 조작 (마우스 클릭) | MCP 도구로 pane 조작 |
| 실시간 시각 검증 | 스크린샷 기반 비교 |
| 마우스 이벤트 시뮬레이션 | MCP 도구 또는 macOS Accessibility API |
| 키보드 이벤트 주입 | PTY stdin 쓰기 |
| 성능 직접 측정 | MCP 도구로 메트릭 노출 |
| 픽셀 정확도 색상 확인 | MCP 도구로 셀 RGB 값 반환 |

---

## 5. 테스팅 MCP 도구 설계

기존 30개 MCP 도구 외에 **7개 테스팅 전용 도구** 추가:

### 5.1 crux_inspect_cell — 셀 단위 검사

```typescript
// 특정 위치의 문자, 색상, 속성 반환
interface InspectCellRequest {
  pane_id: string;
  row: number;    // 0-based, viewport 기준
  col: number;
}

interface InspectCellResponse {
  char: string;              // UTF-8 문자 (multi-codepoint 가능)
  width: number;             // 1 (ASCII) or 2 (CJK)
  fg: [number, number, number];   // RGB [0-255]
  bg: [number, number, number];
  flags: {
    bold: boolean;
    italic: boolean;
    underline: "none" | "single" | "double" | "curly";
    strikethrough: boolean;
    inverse: boolean;
    hidden: boolean;
  };
}
```

**용도**: SGR 이스케이프 시퀀스 파싱 정확성 검증

### 5.2 crux_dump_grid — 그리드 스냅샷

```typescript
// 전체 터미널 그리드를 구조화된 JSON으로
interface DumpGridRequest {
  pane_id: string;
  region?: { start_row: number; end_row: number; start_col: number; end_col: number; };
}

interface DumpGridResponse {
  rows: Cell[][];                         // 2D 셀 배열
  cursor: { row: number; col: number; visible: boolean; style: string; };
  scroll_region: { top: number; bottom: number; };
  dimensions: { rows: number; cols: number; };
}
```

**용도**: golden state 파일과 전체 그리드 비교

### 5.3 crux_get_terminal_modes — 터미널 상태 머신

```typescript
interface GetTerminalModesResponse {
  mode: {
    application_cursor_keys: boolean;   // DECCKM
    application_keypad: boolean;        // DECNKM
    bracketed_paste: boolean;           // DECSET 2004
    mouse_mode: "none" | "x10" | "button" | "any" | "sgr";
    origin_mode: boolean;               // DECOM
    auto_wrap: boolean;                 // DECAWM
  };
  charset: {
    g0: "ascii" | "special";
    g1: "ascii" | "special";
    active: "g0" | "g1";
  };
  cursor_style: "block" | "underline" | "beam";
  title: string;
  icon_name: string;
}
```

**용도**: 모드 전환 시퀀스 검증 (vttest 시나리오)

### 5.4 crux_get_performance — 성능 지표

```typescript
interface GetPerformanceResponse {
  fps: number;                        // 최근 60프레임 평균
  frame_time_ms: number;              // 평균 프레임 시간
  input_latency_ms: number;           // PTY write → 화면 업데이트
  cell_render_time_us: number;        // 셀당 GPU 시간
  scroll_performance: {
    lines_per_second: number;
  };
  memory_usage_mb: number;
}
```

**용도**: 성능 리그레션 감지 (10% 이상 저하 시 경고)

### 5.5 crux_get_accessibility — 접근성 트리

```typescript
interface GetAccessibilityResponse {
  role: "terminal";
  children: Array<{
    role: "pane";
    label: string;         // "zsh — ~/Projects/crux"
    value: string;         // 마지막 가시 라인
    content: string[];     // 모든 라인 (plain text)
  }>;
}
```

**용도**: 스크린 리더 호환성 검증

### 5.6 crux_subscribe_events — 이벤트 스트림

```typescript
interface SubscribeEventsRequest {
  event_types: ("input" | "output" | "resize" | "mode_change")[];
}

interface TerminalEvent {
  timestamp: number;    // Unix μs
  type: string;
  data: any;
}
```

**용도**: 입출력 시퀀스 기록 → 리플레이 테스트

### 5.7 crux_visual_hash — 렌더링 해시

```typescript
interface VisualHashRequest {
  pane_id: string;
  region?: { x: number; y: number; width: number; height: number; };
}

interface VisualHashResponse {
  hash: string;               // Perceptual hash (pHash)
  screenshot_path: string;    // 임시 PNG 파일 경로
  metadata: {
    viewport: { rows: number; cols: number; };
    cell_size: { width: number; height: number; };
    font: string;
    font_size: number;
  };
}
```

**용도**: 비주얼 리그레션 — 해시 비교로 렌더링 변경 감지

### MCP 도구 합계

| 카테고리 | 기존 | 신규 | 합계 |
|----------|------|------|------|
| Pane Management | 5 | — | 5 |
| Command Execution | 5 | — | 5 |
| State Inspection | 5 | — | 5 |
| Content Capture | 5 | — | 5 |
| Differentiation | 10 | — | 10 |
| **Testing** | — | **7** | **7** |
| **합계** | **30** | **7** | **37** |

---

## 6. 셀프 테스팅 아키텍처

### 개요

Crux는 자신의 MCP 서버를 통해 **자체 테스트가 가능**하다:

```
┌──────────────────────────────────────────────┐
│ Test Harness (Bash + Claude Code)            │
│                                              │
│  1. Launch Crux.app (background)             │
│  2. Wait for MCP server on ~/.crux/mcp.sock  │
│  3. Connect MCP client                       │
│  4. Send test input via MCP tools            │
│  5. Verify grid state via testing MCP tools  │
│  6. Compare actual vs expected (golden file) │
│  7. Report results as JSON                   │
│  8. Kill Crux                                │
└──────────────────────────────────────────────┘
         │
         │ Unix socket (JSON-RPC 2.0)
         ↓
┌──────────────────────────────────────────────┐
│ Crux.app (System Under Test)                 │
│                                              │
│  MCP server: 37 tools                        │
│  PTY: test shell                             │
│  Rendering: Metal GPU                        │
└──────────────────────────────────────────────┘
```

### Golden State 비교 전략

#### 전략 A: JSON Golden Files (AI 에이전트용)

```json
// tests/golden/sgr-bold-red.json
{
  "input": "\u001b[1;31mBOLD RED\u001b[m",
  "expected": {
    "cells": [
      {"char": "B", "fg": [255,0,0], "bold": true},
      {"char": "O", "fg": [255,0,0], "bold": true},
      {"char": "L", "fg": [255,0,0], "bold": true},
      {"char": "D", "fg": [255,0,0], "bold": true}
    ],
    "cursor": {"row": 0, "col": 8}
  }
}
```

Claude Code: `Read` golden file → MCP로 입력 전송 → `crux_dump_grid` → JSON 비교

#### 전략 B: Rust 유닛 테스트 (CI용)

```rust
#[test]
fn test_sgr_bold_red() {
    let mut term = TestTerminal::new(80, 24);
    term.write_all(b"\x1b[1;31mBOLD RED\x1b[m");

    let cell = term.get_cell(0, 0);
    assert_eq!(cell.char, 'B');
    assert_eq!(cell.fg, Rgb(255, 0, 0));
    assert!(cell.flags.contains(Flags::BOLD));
}
```

#### 전략 C: insta 스냅샷 테스트 (하이브리드)

```rust
#[test]
fn test_sgr_bold_red() {
    let mut term = TestTerminal::new(80, 24);
    term.write_all(b"\x1b[1;31mBOLD RED\x1b[m");

    let grid = term.dump_grid();
    insta::assert_json_snapshot!("sgr-bold-red", grid);
}
```

**권장**: 전략 A (AI 에이전트), 전략 C (CI/개발자) 병행

---

## 7. 테스트 시나리오

### 7.1 VT 에뮬레이션 정확성

| 테스트 | 입력 시퀀스 | 검증 도구 | 기대 결과 |
|--------|------------|-----------|-----------|
| 커서 이동 | `\033[5A` (5줄 위) | `crux_dump_grid` | cursor.row 5 감소 |
| SGR 속성 | `\033[1;4;31m` | `crux_inspect_cell` | bold, underline, red |
| 스크롤 영역 | `\033[5;10r` | `crux_get_terminal_modes` | scroll_region={4,9} |
| 문자 세트 | `\033)0` (G1=special) | `crux_get_terminal_modes` | charset.g1="special" |
| 모드 전환 | `\033[?1h` (DECCKM) | `crux_get_terminal_modes` | app_cursor_keys=true |
| 화면 지우기 | `\033[2J` | `crux_dump_grid` | 모든 셀 = ' ' |
| 탭 스톱 | `\033H` + `\t` | `crux_dump_grid` | 다음 탭 위치에 커서 |

### 7.2 유니코드/CJK 렌더링

| 테스트 | 입력 | 검증 | 기대 결과 |
|--------|------|------|-----------|
| CJK 폭 | `한글` | `crux_inspect_cell(0,0)` | width=2, 다음 셀 spacer |
| 이모지 | `😀` (U+1F600) | `crux_inspect_cell(0,0)` | width=2, char="😀" |
| 결합 문자 | `é` (e + ´) | `crux_inspect_cell(0,0)` | char="é" (합성) |
| ZWJ 이모지 | `👨‍👩‍👧` | `crux_inspect_cell(0,0)` | 단일 grapheme cluster |
| 혼합 폭 | `abc한글def` | `crux_dump_grid` | 올바른 열 정렬 |

### 7.3 IME 입력 플로우

| 단계 | MCP 도구 | 검증 |
|------|----------|------|
| Composition 시작 | `crux_type_with_ime` (preedit) | 스크린샷에 밑줄 오버레이 |
| Preedit 업데이트 | `crux_type_with_ime` (new text) | 오버레이 업데이트, PTY 변경 없음 |
| Commit | `crux_type_with_ime` (commit) | 오버레이 사라짐, PTY에 확정 텍스트 |
| Cancel | `crux_type_with_ime` (cancel) | 오버레이 사라짐, PTY 변경 없음 |

### 7.4 분할 창 관리

```bash
# 2x2 그리드 생성
crux cli split-pane --right --pane-id main
crux cli split-pane --down --pane-id main

# MCP로 검증
crux_list_panes → 4개 pane 확인
crux_inspect_cell → 각 pane에서 독립적으로 동작 확인

# 리사이즈 테스트
crux_resize_pane --pane-id pane-1 --cols 50
crux_get_pane_state --pane-id pane-1 → cols == 50 확인
```

### 7.5 MCP 도구 라운드트립

모든 37개 MCP 도구에 대해:
1. 유효한 입력 → 정상 응답 확인
2. 잘못된 입력 → 적절한 에러 반환 확인
3. 경계 조건 → 크래시 없음 확인

### 7.6 성능 벤치마크

```bash
# 스크롤 속도
cat /usr/share/dict/words   # macOS에 235,886 라인
crux_get_performance → lines_per_second > 1,000,000

# FPS (스크롤 중)
crux_get_performance → fps > 55

# 입력 지연
crux_get_performance → input_latency_ms < 16
```

### 7.7 테마/색상 정확성

```bash
# 테마 적용
crux_set_theme("tokyonight")

# 배경색 확인
crux_inspect_cell(0, 0) → bg == [26, 27, 38]  # Tokyo Night 배경

# 16 ANSI 색상 검증
for color_index in 0..16:
    send "\033[38;5;{color_index}m#\033[m"
    crux_inspect_cell → fg == expected_rgb[color_index]
```

---

## 8. 비주얼 리그레션 테스팅

### 문제: Metal은 GitHub Actions에서 사용 불가

- GitHub 호스팅 macOS 러너에 **Metal GPU 없음**
- GPU 패스스루 미지원
- GPUI는 Metal 필수 (SwiftShader는 Vulkan이라 해당 없음)

### 해결 전략

#### 계층 1: 로직 테스트 (CI 가능)

VT 파싱, 그리드 상태, 이스케이프 시퀀스 → `cargo test` + `insta`

```rust
// GPU 없이 테스트 가능
#[test]
fn test_cursor_movement() {
    let mut term = alacritty_terminal::Term::new(/* ... */);
    term.input(b"\033[5;10H");  // 커서 이동
    assert_eq!(term.cursor().point.row, 4);
    assert_eq!(term.cursor().point.col, 9);
}
```

#### 계층 2: 스크린샷 비교 (로컬 macOS만)

Zed의 비주얼 테스트 패턴 채택:

```bash
# 1. 기준선 생성 (main 브랜치에서)
UPDATE_BASELINE=1 cargo run --features visual-tests

# 2. 코드 변경 후 비교
cargo run --bin visual_test_runner --features visual-tests

# 3. 차이 확인
# test_fixtures/visual_tests/ 에 diff 이미지 생성
```

**Rust 크레이트**:
- `pixelmatch` — 픽셀 단위 비교
- `image-compare` — SSIM (구조적 유사도) 메트릭
- `insta` — 바이너리 스냅샷 (PNG 파일)

#### 계층 3: AI 비전 분석 (Claude Code)

```bash
# Crux가 스크린샷 생성
crux_visual_hash --pane-id main → screenshot_path

# Claude Code가 이미지 분석
Read(screenshot_path)  # 비전 모델로 분석
# "이 스크린샷에서 빨간색 텍스트가 보이나요?"
# "박스 드로잉 문자가 올바르게 연결되어 있나요?"
```

### 비주얼 테스트 워크플로우 비교

| 방법 | CI 호환 | 정확도 | 속도 | 유지보수 |
|------|---------|--------|------|----------|
| 로직 테스트 (cargo test) | ✅ | 높음 (로직) | 빠름 | 낮음 |
| insta 스냅샷 | ✅ | 높음 (텍스트) | 빠름 | 중간 |
| 스크린샷 비교 | ❌ macOS만 | 최고 (픽셀) | 느림 | 높음 |
| AI 비전 분석 | ⚠️ API 필요 | 중간 | 느림 | 낮음 |
| pHash 비교 | ❌ macOS만 | 높음 | 중간 | 중간 |

---

## 9. CI/CD 통합

### GitHub Actions 워크플로우

```yaml
name: Test
on: [push, pull_request]

jobs:
  # 계층 1: 유닛 테스트 (모든 플랫폼)
  unit-test:
    runs-on: macos-14  # M1 칩
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: sudo xcode-select --switch /Applications/Xcode.app
      - run: cargo test --workspace

  # 계층 2: VT 적합성 (macOS)
  conformance:
    runs-on: macos-14
    needs: unit-test
    steps:
      - uses: actions/checkout@v4
      - name: Run esctest2
        run: |
          git clone https://github.com/ThomasDickey/esctest2
          cargo build --release -p crux-app
          ./scripts/run-esctest.sh

  # 계층 3: 성능 벤치마크 (macOS, 선택적)
  benchmark:
    runs-on: macos-14
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4
      - name: Run vtebench
        run: |
          cargo install vtebench
          cargo build --release -p crux-app
          vtebench ./target/release/crux-app > benchmark.json
      - uses: actions/upload-artifact@v4
        with:
          name: benchmark-results
          path: benchmark.json

  # 계층 4: MCP 라운드트립 (macOS, 선택적)
  mcp-test:
    runs-on: macos-14
    needs: unit-test
    steps:
      - uses: actions/checkout@v4
      - name: Build and test MCP tools
        run: |
          cargo build --release -p crux-app
          # Launch Crux with hidden window
          ./target/release/crux-app --test-mode &
          sleep 3
          # Run MCP tool tests
          cargo test --test mcp_integration
          kill %1
```

### macOS CI 러너 옵션

| 제공자 | GPU 접근 | Metal | 비용 |
|--------|---------|-------|------|
| GitHub Actions `macos-14` | M1 칩 | ⚠️ 제한적 | 무료 (공개 저장소) |
| GitHub Actions `macos-13` | Intel | ❌ | 무료 |
| Self-hosted Mac Mini | M1/M2 | ✅ 전체 | $ (하드웨어) |
| CircleCI macOS | Intel/M1 | ⚠️ | $$ |

**권장**: 유닛/적합성 테스트는 GitHub Actions `macos-14`, 비주얼 테스트는 self-hosted 러너

### 테스트 결과 보고

```json
{
  "summary": {
    "total": 200,
    "passed": 198,
    "failed": 2,
    "skipped": 0,
    "duration_ms": 45230
  },
  "failures": [
    {
      "test": "vt_emulation::sgr_double_underline",
      "reason": "Expected underline=double, got underline=single",
      "grid_dump": { "...": "..." }
    }
  ],
  "performance": {
    "fps": 58,
    "scroll_lines_per_sec": 1250000,
    "input_latency_ms": 8.3
  }
}
```

---

## 10. 권장 테스팅 스택

### 필수 (CI 호환)

| 도구 | 용도 | 크레이트/링크 |
|------|------|--------------|
| `cargo test` | 크레이트별 유닛 테스트 | 표준 |
| `insta` | 스냅샷 테스트 (그리드 상태, ANSI 출력) | [insta.rs](https://insta.rs/) |
| `esctest2` | 자동화된 VT 적합성 검증 | [GitHub](https://github.com/ThomasDickey/esctest2) |
| `expectrl` | PTY 인터랙션 테스트 | [GitHub](https://github.com/zhiburt/expectrl) |
| `vtebench` | 성능 벤치마크 | Alacritty |

### 권장 (로컬/수동)

| 도구 | 용도 | 비고 |
|------|------|------|
| `vttest` | 수동 VT100/VT220 검증 | 릴리즈 체크리스트 |
| GPUI visual tests | GPU 렌더링 비주얼 리그레션 | Zed 패턴, macOS만 |
| `pixelmatch` | 스크린샷 픽셀 비교 | [crates.io](https://crates.io/crates/pixelmatch) |
| `image-compare` | SSIM 유사도 메트릭 | [crates.io](https://crates.io/crates/image-compare) |

### Crux 전용 인프라

| 컴포넌트 | 설명 |
|----------|------|
| 7개 테스팅 MCP 도구 | 셀 검사, 그리드 덤프, 모드 조회, 성능, 접근성, 이벤트, 비주얼 해시 |
| `crux --test-mode` | 숨김 윈도우 + MCP 서버 활성화 (CI용) |
| `crux --headless` | GPU 렌더링 없이 VT 로직만 테스트 |
| Golden state 파일 | `tests/golden/*.json` — 기대 그리드 상태 |
| Test harness 스크립트 | `scripts/run-tests.sh` — Crux 시작/MCP 연결/테스트/종료 |

### 테스트 커버리지 목표

| 카테고리 | 예상 테스트 수 | 검증 방법 |
|----------|---------------|-----------|
| VT 에뮬레이션 | 50+ | `crux_dump_grid` + golden JSON |
| 유니코드/CJK | 20 | `crux_inspect_cell` + width |
| IME 플로우 | 8 | 스크린샷 + 그리드 비교 |
| 분할 창 관리 | 15 | MCP 라운드트립 |
| MCP 도구 | 37 × 3 = 111 | 유효/무효/경계 입력 |
| 성능 | 5 | `crux_get_performance` 임계값 |
| 테마/색상 | 10 | `crux_inspect_cell` RGB 비교 |
| **합계** | **~220** | |

---

## Sources

### 터미널 테스트 도구
- [esctest2](https://github.com/ThomasDickey/esctest2) — 자동화된 터미널 적합성 테스트
- [vttest](https://invisible-island.net/vttest/) — VT100/VT220 수동 테스트
- [vtebench](https://github.com/alacritty/alacritty) — 터미널 성능 벤치마크
- [termbench-pro](https://github.com/contour-terminal/termbench-pro) — 고급 벤치마크
- [expectrl](https://github.com/zhiburt/expectrl) — Rust PTY 인터랙션 테스트

### 스냅샷/비주얼 테스트
- [insta](https://insta.rs/) — Rust 스냅샷 테스트
- [pixelmatch](https://crates.io/crates/pixelmatch) — 픽셀 비교
- [image-compare](https://crates.io/crates/image-compare) — SSIM 메트릭
- [Ratatui snapshot testing](https://ratatui.rs/recipes/testing/snapshots/) — TUI 스냅샷 패턴

### GPUI 테스트
- [Zed Running & Testing](https://zed.dev/docs/running-testing) — GPUI 비주얼 테스트 패턴
- [GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md) — `gpui::test` 매크로

### CI/CD
- [GitHub Actions Metal 미지원](https://github.com/actions/runner-images/discussions/6138)
- [SwiftShader](https://github.com/google/swiftshader) — Vulkan 소프트웨어 렌더러 (Metal 미지원)

### 터미널 에뮬레이터 테스트 참고
- [Alacritty](https://github.com/alacritty/alacritty) — vtebench, ref tests
- [Ghostty](https://github.com/ghostty-org/ghostty) — xterm conformance
- [WezTerm](https://github.com/wezterm/wezterm) — termwiz round-trip
- [Contour](https://github.com/contour-terminal/contour) — modular test architecture
