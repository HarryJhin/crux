---
title: "IME 및 리치 클립보드 연구"
description: "NSTextInputClient protocol, Korean IME failure analysis (Alacritty/Ghostty/WezTerm), NSPasteboard rich clipboard, objc2 Rust bindings, drag-and-drop"
date: 2026-02-11
phase: [3]
topics: [ime, korean, cjk, nstextinputclient, nspasteboard, clipboard, objc2, drag-and-drop]
status: final
related:
  - ../core/keymapping.md
  - ../gpui/framework.md
---

# Crux 터미널 에뮬레이터: IME 및 리치 클립보드 연구 문서

> **작성일**: 2026-02-11
> **목적**: Crux 터미널의 핵심 차별화 기능인 한국어/CJK IME 지원 및 바이너리 클립보드 입력에 대한 기술 조사

---

## 목차

1. [macOS IME 통합 (NSTextInputClient)](#1-macos-ime-통합-nstextinputclient)
2. [기존 터미널의 한국어 IME 실패 사례 분석](#2-기존-터미널의-한국어-ime-실패-사례-분석)
3. [리치 클립보드 (NSPasteboard)](#3-리치-클립보드-nspasteboard)
4. [Rust-macOS 바인딩](#4-rust-macos-바인딩)
5. [터미널 특화 IME 과제](#5-터미널-특화-ime-과제)
6. [참고 자료](#6-참고-자료)

---

## 1. macOS IME 통합 (NSTextInputClient)

### 1.1 NSTextInputClient 프로토콜 개요

macOS에서 IME(입력기)와 앱 간의 통신은 `NSTextInputClient` 프로토콜을 통해 이루어진다. 이 프로토콜은 AppKit의 텍스트 입력 관리 시스템과 상호작용하기 위해 텍스트 뷰가 구현해야 하는 메서드들을 정의한다.

**참고**: [Apple Developer Documentation - NSTextInputClient](https://developer.apple.com/documentation/appkit/nstextinputclient)

### 1.2 필수/핵심 메서드 목록

#### 텍스트 삽입 및 수정

| 메서드 | 설명 |
|--------|------|
| `insertText(_:replacementRange:)` | IME가 확정(commit)한 텍스트를 삽입. `replacementRange`가 현재 마크된 범위와 다르면, 현재 조합을 먼저 확정한 후 처리 |
| `setMarkedText(_:selectedRange:replacementRange:)` | 조합 중인(marked) 텍스트를 설정. 앱은 이 텍스트를 시각적으로 구분되게 렌더링해야 함 (밑줄 등) |
| `unmarkText()` | 마크된 텍스트를 제거하고 해당 텍스트를 일반 삽입 텍스트로 처리 |
| `doCommandBySelector(_:)` | IME가 보내는 명령(예: 줄바꿈, 삭제)을 처리 |

#### 텍스트 상태 조회

| 메서드 | 설명 |
|--------|------|
| `hasMarkedText() -> Bool` | 현재 조합 중인 텍스트가 있는지 여부 반환 |
| `markedRange() -> NSRange` | 마크된 텍스트의 범위 반환. 없으면 `{NSNotFound, 0}` |
| `selectedRange() -> NSRange` | 현재 선택 영역의 범위 반환 |
| `attributedString() -> NSAttributedString` | 전체 텍스트를 속성 문자열로 반환 (선택적) |

#### 레이아웃 및 좌표

| 메서드 | 설명 |
|--------|------|
| `firstRectForCharacterRange(_:actualRange:) -> NSRect` | 지정된 문자 범위의 **화면 좌표** 사각형 반환. **후보(candidate) 창 위치 결정에 핵심적** |
| `characterIndexForPoint(_:) -> Int` | 화면 좌표를 문서 내 문자 인덱스로 변환 |
| `attributedSubstringForProposedRange(_:actualRange:)` | 제안된 범위의 속성 문자열 반환 |

#### 서식 및 표시

| 메서드 | 설명 |
|--------|------|
| `validAttributesForMarkedText() -> [NSAttributedString.Key]` | 마크된 텍스트에서 지원하는 속성 키 배열 반환. 빈 배열 반환 가능 |
| `fractionOfDistanceThroughGlyphForPoint(_:) -> CGFloat` | 글리프 내 상대 위치 반환 |
| `baselineDeltaForCharacterAtIndex(_:) -> CGFloat` | 문자의 베이스라인 델타 반환 |
| `windowLevel() -> Int` | 창 레벨 반환 |

### 1.3 objc2-app-kit의 Rust 트레이트 시그니처

`objc2-app-kit` 크레이트(v0.3.2)에서 `NSTextInputClient`는 다음과 같은 Rust 트레이트로 정의된다:

```rust
// objc2-app-kit의 NSTextInputClient 트레이트 (핵심 메서드)
pub unsafe trait NSTextInputClient {
    // 텍스트 삽입/수정
    unsafe fn insertText_replacementRange(
        &self, string: &AnyObject, replacement_range: NSRange
    );
    unsafe fn setMarkedText_selectedRange_replacementRange(
        &self, string: &AnyObject, selected_range: NSRange, replacement_range: NSRange
    );
    fn unmarkText(&self);
    unsafe fn doCommandBySelector(&self, selector: Sel);

    // 상태 조회
    fn hasMarkedText(&self) -> bool;
    fn markedRange(&self) -> NSRange;
    fn selectedRange(&self) -> NSRange;
    fn attributedString(&self) -> Retained<NSAttributedString>;

    // 좌표/레이아웃
    unsafe fn firstRectForCharacterRange_actualRange(
        &self, range: NSRange, actual_range: NSRangePointer
    ) -> NSRect;
    fn characterIndexForPoint(&self, point: NSPoint) -> NSUInteger;
    unsafe fn attributedSubstringForProposedRange_actualRange(
        &self, range: NSRange, actual_range: NSRangePointer
    ) -> Option<Retained<NSAttributedString>>;

    // 서식
    fn validAttributesForMarkedText(&self) -> Retained<NSArray<NSAttributedStringKey>>;
}
```

**참고**: [objc2-app-kit NSTextInputClient docs](https://docs.rs/objc2-app-kit/latest/x86_64-unknown-linux-gnu/objc2_app_kit/trait.NSTextInputClient.html)

### 1.4 한글 조합 과정 (상태 기계)

한글 입력은 자모(jamo)를 음절(syllable)로 조합하는 유한 상태 기계(FSM)로 동작한다.

#### 3-레지스터 모델

| 레지스터 | 역할 | 값 범위 |
|----------|------|---------|
| **x** (초성/choseong) | 첫소리 자음 | 0-18 (19개) |
| **y** (중성/jungseong) | 가운뎃소리 모음 | 0-20 (21개) |
| **z** (종성/jongseong) | 끝소리 자음 | 0-27 (28개, 0=종성 없음) |

**유니코드 계산 공식**:
```
코드포인트 = 0xAC00 + (588 × x) + (28 × y) + z
```

여기서 588 = 21 × 28 (중성 수 × 종성 수)

#### 상태 전이 다이어그램

```
[시작] ──자음──→ [X 상태] ──모음──→ [Y 상태] ──자음──→ [Z 상태]
                    │                   │                  │
                    │                   ├──복합모음──→ [Y' 상태]
                    │                   │                  │
                    │                   │              ──모음──→ [받침 분리]
                    │                   │                        (z→다음 x)
                    │                   │
                    │                   └──완성──→ [출력]
                    └──완성──→ [출력]
```

#### 복합 모음 처리

중성에서 ㅗ, ㅜ, ㅡ는 복합 모음의 시작일 수 있다:
- ㅗ + ㅏ → ㅘ, ㅗ + ㅐ → ㅙ, ㅗ + ㅣ → ㅚ
- ㅜ + ㅓ → ㅝ, ㅜ + ㅔ → ㅞ, ㅜ + ㅣ → ㅟ
- ㅡ + ㅣ → ㅢ

이를 위해 Y 상태에서 중간 상태(Yㅗ, Yㅜ, Yㅡ)로 전이하여 다음 입력을 확인한다.

#### 받침(종성) 분리 로직

**핵심 알고리즘**: Z 상태에서 자음이 입력되면, 그 자음이 현재 음절의 종성인지 다음 음절의 초성인지 판단해야 한다.

```
판단 기준: 다음 입력이 모음인가?
  - 예 → 현재 자음은 다음 음절의 초성 (받침 분리)
  - 아니오 → 현재 자음은 종성으로 확정
```

이 로직은 재귀적으로 구현된다 — 파서가 자기 자신을 호출하여 다음 자모가 유효한 음절 시작을 형성할 수 있는지 테스트한다.

#### 예시: "한글" 입력 과정

입력 시퀀스: `ㅎ → ㅏ → ㄴ → ㄱ → ㅡ → ㄹ`

**NSTextInputClient 호출 관점에서의 흐름:**

| 키 입력 | 상태 | setMarkedText 호출 | 표시 |
|---------|------|-------------------|------|
| `ㅎ` | X(x=18) | `setMarkedText("ㅎ")` | `ㅎ` |
| `ㅏ` | Y(x=18, y=0) | `setMarkedText("하")` | `하` |
| `ㄴ` | Z(x=18, y=0, z=4) | `setMarkedText("한")` | `한` |
| `ㄱ` | 받침분리 → 새 X(x=0) | `insertText("한")` + `setMarkedText("ㄱ")` | `한ㄱ` |
| `ㅡ` | Y(x=0, y=18) | `setMarkedText("그")` | `한그` |
| `ㄹ` | Z(x=0, y=18, z=8) | `setMarkedText("글")` | `한글` |
| (종료) | 확정 | `insertText("글")` | `한글` |

**받침 분리 핵심**: `ㄴ` 다음에 `ㄱ`이 입력될 때, 시스템은 `ㄱ` 다음에 모음(`ㅡ`)이 올 것을 감지하여 `ㄴ`을 `한`의 종성으로 확정하고, `ㄱ`을 새 음절의 초성으로 처리한다.

#### 유니코드 계산 검증

```
한: 0xAC00 + 588×18 + 28×0 + 4 = 0xAC00 + 10584 + 0 + 4 = 0xD55C ✓
글: 0xAC00 + 588×0 + 28×18 + 8 = 0xAC00 + 0 + 504 + 8 = 0xAE00 ✓
```

#### UTF-8 인코딩

한글 코드포인트(U+AC00~U+D7A3)는 항상 3바이트를 사용한다:

```rust
fn hangul_to_utf8(codepoint: u32) -> [u8; 3] {
    [
        0xE0 | ((codepoint >> 12) & 0x0F) as u8,
        0x80 | ((codepoint >> 6) & 0x3F) as u8,
        0x80 | (codepoint & 0x3F) as u8,
    ]
}
```

### 1.5 후보 창(Candidate Window) 위치 지정

`firstRectForCharacterRange` 메서드가 후보 창 위치를 결정한다:

1. IME 시스템이 `firstRectForCharacterRange(_:actualRange:)`를 호출
2. 앱은 마크된 텍스트의 **화면 좌표** 사각형을 반환
3. IME는 이 좌표 아래에 후보 창을 표시

**터미널에서의 구현 전략**:
```rust
// 의사 코드: 커서 좌표 → 화면 좌표 변환
fn first_rect_for_character_range(&self, range: NSRange) -> NSRect {
    let cursor_pos = self.terminal.cursor_position(); // (col, row)
    let cell_size = self.terminal.cell_size();         // (width, height)
    let window_origin = self.window.frame().origin;

    // 터미널 셀 좌표 → 창 좌표 → 화면 좌표
    let x = window_origin.x + (cursor_pos.col as f64 * cell_size.width);
    let y = window_origin.y + (cursor_pos.row as f64 * cell_size.height);

    NSRect::new(
        NSPoint::new(x, y),
        NSSize::new(cell_size.width, cell_size.height),
    )
}
```

### 1.6 터미널 맥락에서의 조합 오버레이 전략

**핵심 원칙**: 조합 중인 텍스트는 PTY에 보내지 않고 오버레이로 렌더링해야 한다.

```
┌─────────────────────────────────────┐
│ PTY 버퍼 (실제 터미널 내용)           │
│  $ echo "hello"                      │
│  hello                               │
│  $ █                                 │
├─────────────────────────────────────┤
│ 조합 오버레이 (PTY와 분리)            │
│      [한]  ← setMarkedText로 표시    │
│      ----  ← 밑줄로 조합 중 표시      │
└─────────────────────────────────────┘
```

**참고**: [How Korean input methods work](https://m10k.eu/2025/03/08/hangul-utf8.html)

---

## 2. 기존 터미널의 한국어 IME 실패 사례 분석

### 2.1 Alacritty: 한국어 입력 시 창 프리즈 (#4469)

**증상**: 한국어 IME(uim)로 문자 입력 시 Alacritty 창이 완전히 프리즈되어 어떤 키/이벤트 입력도 받지 못함.

**근본 원인**:
- `uim`(Unimagin Input Method)이 문자 입력 중 세그폴트(segfault) 발생
- libX11의 `XIfEvent()`가 반환하지 않음 → 이벤트 루프 데드락
- GDB 백트레이스: `XFilterEvent` 호출 중 프리즈 확인
- **핵심**: 문제는 Alacritty 자체가 아니라 winit(windowing 라이브러리)에서 발생

**교훈**:
- emacs/urxvt는 IME 크래시를 우아하게 처리하는 반면, winit 기반 앱은 복구 메커니즘이 없음
- **Crux 설계 시사점**: IME 크래시에 대한 타임아웃/복구 메커니즘 필수

```rust
// 잘못된 패턴 (Alacritty/winit)
fn handle_ime_event(event: &XEvent) {
    XFilterEvent(event, None); // uim 크래시 시 영원히 블록
}

// 올바른 패턴 (Crux 제안)
fn handle_ime_event(event: &XEvent) -> Result<(), ImeError> {
    // 타임아웃이 있는 IME 이벤트 처리
    match with_timeout(Duration::from_millis(100), || {
        XFilterEvent(event, None)
    }) {
        Ok(_) => Ok(()),
        Err(Timeout) => {
            // IME 상태 리셋 후 복구
            self.reset_ime_state();
            Err(ImeError::Timeout)
        }
    }
}
```

**참고**: [Alacritty #4469](https://github.com/alacritty/alacritty/issues/4469), [winit #1813](https://github.com/rust-windowing/winit/issues/1813)

### 2.2 WezTerm/Ghostty: 조합 중 문자 사라짐

**증상**: 한글(Hangul) 문자를 입력할 때, 완성된 글자가 잠깐 표시되었다가 즉시 사라짐. macOS Terminal.app에서는 발생하지 않음.

**Ghostty 근본 원인**:

1. **수식키(modifier) 처리 오류**: Ctrl, Shift 등 수식키를 누르면 Ghostty가 이를 잘못 처리하여 preedit 텍스트가 사라짐
2. **제어 문자 처리 충돌**: IME가 Ctrl+키 조합으로 텍스트를 확정할 때, Ghostty가 이벤트를 가로채서 IME에 전달하지 않음

**Ghostty 수정 사항** (v1.1.0 ~ v1.2.0):

```
PR #4649: "macos: ignore modifier changes while IM is active"
  → 수식키만 누를 때 IME의 preedit 상태에 간섭하지 않도록 함

PR #4854: 두 가지 핵심 변경
  1. preedit 상태에서 ctrl+key 입력을 libghostty가 오버라이드하지 않음
  2. macOS에서 IME가 텍스트를 확정할 때 control modifier를 제거하여
     특수 ctrl+key 핸들링 트리거를 방지
```

**WezTerm 근본 원인**:
- NFD(분해형) 한글 문자 렌더링 문제 ([#1474](https://github.com/wezterm/wezterm/issues/1474))
- 전각 문자 너비 계산 오류
- preedit 문자열이 모든 분할 패널의 커서 위치에 표시되는 버그 ([#2569](https://github.com/wezterm/wezterm/issues/2569))

**Crux 설계 시사점**:
```rust
// 핵심: preedit 상태에서는 수식키 이벤트를 무시해야 함
fn handle_key_event(&self, event: KeyEvent) -> bool {
    if self.has_marked_text() {
        match event {
            // preedit 중 수식키만 누르면 무시
            KeyEvent::ModifierOnly(_) => return true, // 이벤트 소비, 아무 작업 안 함

            // preedit 중 ctrl+key는 IME에 먼저 전달
            KeyEvent::KeyWithModifier { modifier: Ctrl, .. } => {
                // IME에 먼저 전달 시도
                if self.ime_context.handle_key(event) {
                    return true;
                }
                // IME가 처리하지 않으면 일반 처리
            }

            _ => {}
        }
    }
    false
}
```

**참고**: [Ghostty #4634](https://github.com/ghostty-org/ghostty/issues/4634), [Ghostty #7225](https://github.com/ghostty-org/ghostty/issues/7225), [WezTerm #1474](https://github.com/wezterm/wezterm/issues/1474)

### 2.3 Alacritty: CJK 이중 스페이스 입력 (#8079)

**증상**: 한국어 IME 모드에서 스페이스바를 한 번 누르면 두 개의 스페이스가 입력됨.

**근본 원인**: macOS의 한국어 IME가 스페이스바 입력 시 **이중 이벤트**를 생성:

```
스페이스바 1회 누름 →
  1. Ime(Commit(" "))  ← IME 이벤트로 스페이스 전달
  2. KeyboardInput(text: Some(" "))  ← 키보드 이벤트로도 스페이스 전달
```

**원인 분석**:
- 일본어/중국어 IME는 스페이스를 조합/변환 키로 사용하므로, preedit 버퍼에 포함하는 것이 합리적
- 한국어는 스페이스가 단순 입력키여야 하지만, 같은 IME 패턴을 따르도록 설계됨
- 결과적으로 IME 이벤트와 키보드 이벤트 양쪽 모두에서 스페이스가 전달됨
- 문제는 winit 또는 macOS 한국어 IME 구현에서 발생

**우회 방법**: macOS 내장 IME 대신 **구름(Gureum)** 한국어 입력기 사용

**Crux 설계 시사점**:
```rust
// IME 커밋 이벤트와 키보드 이벤트의 중복 제거
fn process_events(&mut self, events: &[Event]) {
    let mut last_ime_commit: Option<(String, Instant)> = None;

    for event in events {
        match event {
            Event::Ime(ImeEvent::Commit(text)) => {
                self.insert_text(text);
                last_ime_commit = Some((text.clone(), Instant::now()));
            }
            Event::KeyboardInput { text: Some(text), .. } => {
                // IME 커밋 직후의 동일 텍스트 키보드 이벤트는 무시
                if let Some((ref committed, time)) = last_ime_commit {
                    if committed == text && time.elapsed() < Duration::from_millis(10) {
                        continue; // 중복 이벤트 무시
                    }
                }
                self.insert_text(text);
            }
            _ => {}
        }
    }
}
```

**참고**: [Alacritty #8079](https://github.com/alacritty/alacritty/issues/8079)

### 2.4 Claude Code: IME 커서 위치 오류 (#19207)

**증상**: CJK IME 후보 창이 입력 위치가 아닌 화면 왼쪽 하단에 표시됨.

**근본 원인**: TUI 앱(Ink 기반)의 "가짜 커서" vs 터미널의 "실제 커서" 불일치

```
TUI가 보여주는 것:     실제 터미널 커서 위치:

$ 입력하세요: 한█       $ 입력하세요: 한
                        █  ← 실제 커서는 좌하단에 있음

→ IME는 실제 커서(firstRect)를 참조하므로 후보 창이 엉뚱한 곳에 표시
```

**기술적 설명**:
1. Ink 기반 TUI는 실제 터미널 커서를 숨기고 ANSI 스타일링으로 "가짜 커서"를 렌더링
2. macOS IME는 터미널의 `firstRect` (실제 커서 위치)를 참조하여 후보 창 위치를 결정
3. 가짜 커서와 실제 커서의 위치가 다르므로 후보 창이 잘못된 곳에 표시됨

**Claude Code의 해결 방식 (PR #17127)**: "Declared Cursor System"

```
1. 렌더 트리에 CURSOR_MARKER(\u001B[999m) 배치
2. 렌더링 시 마커를 Save Cursor Position(\u001B[s)으로 치환
3. 출력 완료 후 Restore Cursor Position(\u001B[u) + Show Cursor(\u001B[?25h])
4. 결과: 터미널의 실제 커서가 마커 위치로 이동 → IME 후보 창 정상 표시
```

**Crux 설계 시사점**: Crux는 터미널 에뮬레이터 자체이므로 이 문제가 근본적으로 다르다:
- TUI 앱이 보내는 커서 이동 시퀀스를 직접 해석하므로 실제 커서 위치를 항상 알고 있음
- `firstRectForCharacterRange` 구현 시 정확한 커서 셀 좌표를 바로 사용 가능
- **Crux의 장점**: 터미널 에뮬레이터 레벨에서 문제를 해결하므로 모든 TUI 앱에서 IME가 올바르게 동작

**참고**: [Claude Code #19207](https://github.com/anthropics/claude-code/issues/19207), [Claude Code #16372](https://github.com/anthropics/claude-code/issues/16372)

### 2.5 올바른 구현의 모습

기존 터미널들의 실패 패턴을 종합하면, **올바른 IME 구현**은 다음 조건을 만족해야 한다:

| 요구사항 | 설명 | 실패 사례 |
|----------|------|-----------|
| IME 크래시 내성 | IME 프로세스 크래시 시 터미널이 데드락되지 않아야 함 | Alacritty #4469 |
| preedit 상태에서 수식키 무시 | Ctrl/Shift/Cmd 단독 입력이 preedit를 파괴하면 안 됨 | Ghostty #4634 |
| 이벤트 중복 방지 | IME 커밋과 키보드 이벤트의 중복 처리 방지 | Alacritty #8079 |
| 정확한 후보 창 위치 | firstRect가 실제 커서 셀 위치를 반환해야 함 | Claude Code #19207 |
| 전각 문자 너비 처리 | CJK 문자의 2-cell 너비를 정확히 계산해야 함 | WezTerm |
| preedit/PTY 분리 | 조합 중 텍스트를 PTY 버퍼와 분리하여 렌더링해야 함 | 대부분의 터미널 |

---

## 3. 리치 클립보드 (NSPasteboard)

### 3.1 NSPasteboard 개요

macOS의 클립보드 시스템은 `NSPasteboard`를 통해 관리되며, 텍스트뿐 아니라 이미지, 파일, 리치 텍스트 등 다양한 데이터 타입을 지원한다.

**참고**: [Apple Developer Documentation - NSPasteboard](https://developer.apple.com/documentation/appkit/nspasteboard)

### 3.2 클립보드 이미지 데이터 읽기

#### 지원되는 이미지 타입

| 타입 | UTI | 설명 |
|------|-----|------|
| PNG | `public.png` / `NSPasteboardTypePNG` | 스크린샷 등 |
| TIFF | `public.tiff` / `NSPasteboardTypeTIFF` | macOS 기본 이미지 형식 |
| PDF | `com.adobe.pdf` | PDF 문서 |
| 파일 URL | `public.file-url` | 파일 경로 참조 |

> **주의**: macOS는 내부적으로 TIFF를 기본 이미지 형식으로 사용하는 경우가 많다. `NSPasteboardTypePNG`로 요청해도 실제로는 TIFF 데이터가 반환될 수 있다.

#### Objective-C 코드 패턴

```objc
NSPasteboard *pb = [NSPasteboard generalPasteboard];

// 1. 사용 가능한 타입 확인
NSArray *types = [pb types];

// 2. 이미지 데이터 읽기 (우선순위: PNG > TIFF)
NSData *imageData = nil;
if ([pb canReadItemWithDataConformingToTypes:@[NSPasteboardTypePNG]]) {
    imageData = [pb dataForType:NSPasteboardTypePNG];
} else if ([pb canReadItemWithDataConformingToTypes:@[NSPasteboardTypeTIFF]]) {
    imageData = [pb dataForType:NSPasteboardTypeTIFF];
}

// 3. 임시 파일로 저장
if (imageData) {
    NSString *tmpPath = [NSTemporaryDirectory() stringByAppendingPathComponent:@"clipboard.png"];
    [imageData writeToFile:tmpPath atomically:YES];
}
```

### 3.3 클립보드 컨텐츠 타입 감지

```rust
// Crux에서의 클립보드 타입 감지 (의사 코드)
#[derive(Debug)]
enum ClipboardContent {
    Text(String),
    Image { data: Vec<u8>, format: ImageFormat },
    Files(Vec<PathBuf>),
    RichText(String), // HTML/RTF
    Empty,
}

#[derive(Debug)]
enum ImageFormat {
    Png,
    Tiff,
    Pdf,
}

fn detect_clipboard_content(pasteboard: &NSPasteboard) -> ClipboardContent {
    let types = pasteboard.types();

    // 우선순위: 파일 > 이미지 > 리치텍스트 > 텍스트
    if types.contains("public.file-url") {
        let urls = pasteboard.read_objects_for_classes::<NSURL>();
        ClipboardContent::Files(urls.into_iter().map(|u| u.path()).collect())
    } else if types.contains("public.png") || types.contains("public.tiff") {
        let (data, format) = if let Some(png) = pasteboard.data_for_type("public.png") {
            (png, ImageFormat::Png)
        } else {
            (pasteboard.data_for_type("public.tiff").unwrap(), ImageFormat::Tiff)
        };
        ClipboardContent::Image { data, format }
    } else if types.contains("public.html") {
        let html = pasteboard.string_for_type("public.html").unwrap();
        ClipboardContent::RichText(html)
    } else if types.contains("public.utf8-plain-text") {
        let text = pasteboard.string_for_type("public.utf8-plain-text").unwrap();
        ClipboardContent::Text(text)
    } else {
        ClipboardContent::Empty
    }
}
```

### 3.4 클립보드 이미지 → 임시 파일 변환

터미널 앱(예: Claude Code)에 이미지를 붙여넣으려면 바이너리 데이터를 임시 파일로 저장한 뒤 경로를 전달해야 한다:

```rust
use std::fs;
use std::path::PathBuf;
use std::env;

fn clipboard_image_to_temp_file(data: &[u8], format: ImageFormat) -> Result<PathBuf, io::Error> {
    let ext = match format {
        ImageFormat::Png => "png",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Pdf => "pdf",
    };

    let tmp_dir = env::temp_dir().join("crux-clipboard");
    fs::create_dir_all(&tmp_dir)?;

    // 고유 파일명 생성
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let path = tmp_dir.join(format!("paste-{}.{}", timestamp, ext));

    // TIFF → PNG 변환 (필요 시)
    if matches!(format, ImageFormat::Tiff) {
        let png_data = convert_tiff_to_png(data)?;
        fs::write(&path, &png_data)?;
    } else {
        fs::write(&path, data)?;
    }

    Ok(path)
}
```

### 3.5 드래그 앤 드롭 (NSDraggingDestination)

macOS에서 드래그 앤 드롭을 받으려면 `NSDraggingDestination` 프로토콜을 구현해야 한다.

#### 드래그 세션 생명주기

```
[드래그 시작] ──경계 진입──→ draggingEntered:
                              │
                    ┌─────────┤
                    │         ▼
                    │    draggingUpdated: (반복)
                    │         │
                    │    ┌────┴────┐
                    │    │         │
                    ▼    ▼         ▼
              draggingExited:   prepareForDragOperation:
              (경계 이탈)           │
                                   ▼
                            performDragOperation:
                                   │
                                   ▼
                            concludeDragOperation:
```

#### 등록 및 구현

```rust
// NSView에 드래그 타입 등록
unsafe fn register_drag_types(view: &NSView) {
    let types = NSArray::from_vec(vec![
        NSPasteboardType::fileURL(),     // 파일 드롭
        NSPasteboardType::png(),         // PNG 이미지
        NSPasteboardType::tiff(),        // TIFF 이미지
        NSPasteboardType::string(),      // 텍스트
    ]);
    view.registerForDraggedTypes(&types);
}

// NSDraggingDestination 프로토콜 구현
impl NSDraggingDestination for CruxTerminalView {
    fn dragging_entered(&self, sender: &NSDraggingInfo) -> NSDragOperation {
        let pasteboard = sender.dragging_pasteboard();
        let types = pasteboard.types();

        if types.contains_image_type() || types.contains_file_type() {
            NSDragOperation::Copy
        } else {
            NSDragOperation::None
        }
    }

    fn perform_drag_operation(&self, sender: &NSDraggingInfo) -> bool {
        let pasteboard = sender.dragging_pasteboard();

        // 파일 URL인 경우
        if let Some(urls) = pasteboard.read_file_urls() {
            for url in urls {
                self.handle_dropped_file(url);
            }
            return true;
        }

        // 이미지 데이터인 경우
        if let Some(image_data) = pasteboard.read_image_data() {
            let temp_path = clipboard_image_to_temp_file(&image_data, ImageFormat::Png);
            self.handle_dropped_image(temp_path);
            return true;
        }

        false
    }
}
```

### 3.6 터미널 앱에 바이너리 데이터 전달 (Claude Code 이미지 붙여넣기 워크플로우)

```
사용자가 Cmd+V로 이미지 붙여넣기
       │
       ▼
┌─ Crux 터미널 에뮬레이터 ─────────────────────┐
│ 1. NSPasteboard에서 이미지 데이터 읽기          │
│ 2. /tmp/crux-clipboard/paste-XXXX.png에 저장    │
│ 3. 앱에 파일 경로를 OSC 시퀀스로 전달            │
│    또는: 앱이 지원하면 Base64 인코딩으로 전달     │
└──────────────────────────────────────────────┘
       │
       ▼
┌─ 터미널 앱 (예: Claude Code) ────────────────┐
│ - OSC 52 클립보드 시퀀스로 데이터 수신           │
│ - 또는: 특수 이스케이프 시퀀스로 파일 경로 수신    │
│ - 이미지 처리 및 표시                           │
└──────────────────────────────────────────────┘
```

**참고**: [NSPasteboard Documentation](https://developer.apple.com/documentation/appkit/nspasteboard), [Drag and Drop Tutorial](https://www.kodeco.com/1016-drag-and-drop-tutorial-for-macos)

---

## 4. Rust-macOS 바인딩

### 4.1 objc2 크레이트 생태계

`objc2`는 Rust에서 Apple 프레임워크에 접근하기 위한 현대적인 바인딩 시스템이다.

#### 크레이트 구조

| 크레이트 | 역할 | 버전 |
|----------|------|------|
| `objc2` | Objective-C 런타임 바인딩 핵심 | 최신 |
| `objc2-foundation` | Foundation 프레임워크 (NSString, NSArray 등) | 최신 |
| `objc2-app-kit` | AppKit 프레임워크 (NSView, NSWindow, NSTextInputClient 등) | 최신 |
| `objc2-core-foundation` | Core Foundation (CFString, CFArray 등) | v0.3.1 |

#### Cargo.toml 설정

```toml
[dependencies]
objc2 = "0.5"
objc2-foundation = { version = "0.2", features = [
    "NSString", "NSArray", "NSAttributedString", "NSRange",
    "NSNotification", "NSDictionary"
] }
objc2-app-kit = { version = "0.2", features = [
    "NSView", "NSWindow", "NSTextInputClient", "NSPasteboard",
    "NSEvent", "NSResponder", "NSDragging", "NSImage",
    "NSText", "NSAttributedString"
] }
```

#### NSTextInputClient 구현 예시 (Zed/GPUI 참고)

```rust
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{declare_class, msg_send, msg_send_id, ClassType, DeclaredClass};
use objc2_app_kit::*;
use objc2_foundation::*;

// 커스텀 NSView 서브클래스 선언
declare_class!(
    struct CruxTerminalView;

    unsafe impl ClassType for CruxTerminalView {
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "CruxTerminalView";
    }

    impl DeclaredClass for CruxTerminalView {
        type Ivars = TerminalViewState;
    }

    // NSTextInputClient 프로토콜 구현
    unsafe impl NSTextInputClient for CruxTerminalView {
        #[method(insertText:replacementRange:)]
        unsafe fn insertText_replacementRange(
            &self,
            string: &AnyObject,
            replacement_range: NSRange,
        ) {
            // AnyObject에서 문자열 추출
            let text: Retained<NSString> = if msg_send![string, isKindOfClass: NSAttributedString::class()] {
                let attr_str: &NSAttributedString = &*(string as *const _ as *const _);
                attr_str.string().copy()
            } else {
                Retained::cast(Retained::retain(string).unwrap())
            };

            let rust_string = text.to_string();

            // 마크된 텍스트 해제
            self.ivars().marked_text.borrow_mut().take();

            // PTY에 확정된 텍스트 전송
            self.ivars().pty_writer.write(rust_string.as_bytes());
        }

        #[method(setMarkedText:selectedRange:replacementRange:)]
        unsafe fn setMarkedText_selectedRange_replacementRange(
            &self,
            string: &AnyObject,
            selected_range: NSRange,
            replacement_range: NSRange,
        ) {
            let text: String = extract_string_from_any_object(string);

            // 마크된 텍스트를 오버레이로만 저장 (PTY에는 보내지 않음!)
            *self.ivars().marked_text.borrow_mut() = if text.is_empty() {
                None
            } else {
                Some(MarkedText {
                    text,
                    selected_range,
                })
            };

            // 렌더링 갱신 요청
            self.setNeedsDisplay(true);
        }

        #[method(unmarkText)]
        fn unmarkText(&self) {
            // 조합 완료: 마크된 텍스트를 확정 텍스트로 처리
            if let Some(marked) = self.ivars().marked_text.borrow_mut().take() {
                self.ivars().pty_writer.write(marked.text.as_bytes());
            }
        }

        #[method(hasMarkedText)]
        fn hasMarkedText(&self) -> bool {
            self.ivars().marked_text.borrow().is_some()
        }

        #[method(markedRange)]
        fn markedRange(&self) -> NSRange {
            if let Some(ref marked) = *self.ivars().marked_text.borrow() {
                NSRange::new(0, marked.text.len())
            } else {
                NSRange::new(NSNotFound as usize, 0)
            }
        }

        #[method(selectedRange)]
        fn selectedRange(&self) -> NSRange {
            // 터미널에서는 일반적으로 커서 위치
            let cursor = self.ivars().terminal.cursor_position();
            NSRange::new(cursor, 0)
        }

        #[method_id(validAttributesForMarkedText)]
        fn validAttributesForMarkedText(&self) -> Retained<NSArray<NSAttributedStringKey>> {
            NSArray::new() // 빈 배열 (특수 속성 불필요)
        }

        #[method(firstRectForCharacterRange:actualRange:)]
        unsafe fn firstRectForCharacterRange_actualRange(
            &self,
            range: NSRange,
            actual_range: NSRangePointer,
        ) -> NSRect {
            let terminal = self.ivars().terminal.borrow();
            let cursor = terminal.cursor_position();
            let cell_size = terminal.cell_size();

            // 셀 좌표 → 뷰 좌표
            let view_x = cursor.col as f64 * cell_size.width;
            let view_y = cursor.row as f64 * cell_size.height;
            let view_rect = NSRect::new(
                NSPoint::new(view_x, view_y),
                NSSize::new(cell_size.width, cell_size.height),
            );

            // 뷰 좌표 → 창 좌표 → 화면 좌표
            let window_rect = self.convertRect_toView(view_rect, None);
            let window = self.window().unwrap();
            window.convertRectToScreen(window_rect)
        }

        #[method(characterIndexForPoint:)]
        fn characterIndexForPoint(&self, point: NSPoint) -> NSUInteger {
            // 화면 좌표 → 셀 좌표 변환
            // 대부분의 구현에서 NSNotFound 반환도 가능
            NSNotFound as NSUInteger
        }

        #[method(doCommandBySelector:)]
        unsafe fn doCommandBySelector(&self, selector: Sel) {
            // IME 명령 처리 (예: insertNewline:, deleteBackward:)
            let sel_name = selector.name();
            match sel_name {
                "insertNewline:" => self.ivars().pty_writer.write(b"\r"),
                "deleteBackward:" => self.ivars().pty_writer.write(b"\x7f"),
                "insertTab:" => self.ivars().pty_writer.write(b"\t"),
                _ => {
                    // 처리하지 않는 명령은 super로 전달
                    let _: () = msg_send![super(self), doCommandBySelector: selector];
                }
            }
        }
    }
);
```

**참고**: [objc2 GitHub](https://github.com/madsmtm/objc2), [Zed GPUI window.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/platform/mac/window.rs)

### 4.2 objc2 vs core-foundation-rs

| 기준 | objc2 | core-foundation-rs |
|------|-------|-------------------|
| **코드 생성** | Xcode SDK에서 자동 생성 → 모든 API 사용 가능 | 수동 바인딩 → 제한적 |
| **메모리 관리** | `Retained`/`CFRetained` 자동 관리 | 수동 관리 필요한 경우 있음 |
| **타입 안전성** | 강한 타입 체크 | extension trait 기반 (약함) |
| **API 커버리지** | 포괄적 (최신 SDK 반영) | 제한적 |
| **안정성** | 덜 안정적 (아직 pre-1.0) | 더 안정적 (오래됨) |
| **감사** | 미감사 | 감사됨 |
| **권장** | **신규 프로젝트에 권장** | 레거시 유지보수용 |

**결론**: Crux는 신규 프로젝트이므로 `objc2` 생태계를 사용해야 한다. `core-foundation-rs`는 더 이상 신규 프로젝트에 권장되지 않으며, `objc2-core-foundation` v0.3.1이 기존 `core-foundation`의 모든 기능을 대체한다.

**참고**: [core-foundation-rs #729](https://github.com/servo/core-foundation-rs/issues/729), [objc2 #719](https://github.com/madsmtm/objc2/issues/719)

### 4.3 NSPasteboard 접근 (Rust)

#### clipboard-rs 크레이트 사용 (고수준 API)

```rust
// Cargo.toml
// [dependencies]
// clipboard-rs = "0.3"

use clipboard_rs::{Clipboard, ClipboardContext, ClipboardHandler,
                   ClipboardWatcher, ClipboardWatcherContext, ContentFormat};
use clipboard_rs::common::RustImage;

// 1. 텍스트 읽기
fn read_text() -> Option<String> {
    let ctx = ClipboardContext::new().ok()?;
    ctx.get_text().ok()
}

// 2. 이미지 읽기 및 임시 파일 저장
fn read_image_to_temp() -> Option<String> {
    let ctx = ClipboardContext::new().ok()?;
    let img = ctx.get_image().ok()?;
    let path = "/tmp/crux-clipboard/paste.png";
    img.save_to_path(path).ok()?;
    Some(path.to_string())
}

// 3. 컨텐츠 타입 감지
fn detect_content_type() -> ContentType {
    let ctx = ClipboardContext::new().unwrap();
    let formats = ctx.available_formats().unwrap_or_default();

    if ctx.has(ContentFormat::Image) {
        ContentType::Image
    } else if ctx.has(ContentFormat::Html) {
        ContentType::RichText
    } else if ctx.has(ContentFormat::Text) {
        ContentType::PlainText
    } else {
        ContentType::Unknown
    }
}

// 4. 파일 URL 읽기
fn read_file_urls() -> Vec<String> {
    let ctx = ClipboardContext::new().unwrap();
    match ctx.get_buffer("public.file-url-list") {
        Ok(buffer) => {
            String::from_utf8(buffer)
                .unwrap_or_default()
                .lines()
                .map(|s| s.to_string())
                .collect()
        }
        Err(_) => vec![],
    }
}

// 5. 클립보드 변경 모니터링
struct ClipboardMonitor {
    ctx: ClipboardContext,
}

impl ClipboardHandler for ClipboardMonitor {
    fn on_clipboard_change(&mut self) {
        println!("클립보드 변경 감지!");
        if let Ok(text) = self.ctx.get_text() {
            println!("텍스트: {}", text);
        }
    }
}

fn watch_clipboard() {
    let monitor = ClipboardMonitor {
        ctx: ClipboardContext::new().unwrap(),
    };
    let mut watcher = ClipboardWatcherContext::new().unwrap();
    let shutdown = watcher.add_handler(monitor).get_shutdown_channel();

    // 10초 후 종료
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(10));
        shutdown.stop();
    });

    watcher.start_watch();
}
```

#### 직접 objc2로 NSPasteboard 접근 (저수준 API)

```rust
use objc2_app_kit::{NSPasteboard, NSPasteboardType};
use objc2_foundation::{NSArray, NSData, NSString};

fn read_image_from_pasteboard() -> Option<Vec<u8>> {
    unsafe {
        let pasteboard = NSPasteboard::generalPasteboard();

        // PNG 타입 확인
        let png_type = NSPasteboardType::from(NSString::from_str("public.png"));
        let tiff_type = NSPasteboardType::from(NSString::from_str("public.tiff"));

        let types = NSArray::from_vec(vec![png_type.clone(), tiff_type.clone()]);

        // 사용 가능한 타입 중 선택
        if let Some(available_type) = pasteboard.availableTypeFromArray(&types) {
            if let Some(data) = pasteboard.dataForType(&available_type) {
                return Some(data.bytes().to_vec());
            }
        }

        None
    }
}
```

**참고**: [clipboard-rs GitHub](https://github.com/ChurchTao/clipboard-rs), [arboard GitHub](https://github.com/1Password/arboard)

### 4.4 GPUI의 NSTextInputClient 구현 (Zed 참조)

Zed 에디터의 GPUI 프레임워크는 `crates/gpui/src/platform/mac/window.rs`에서 `NSTextInputClient`를 구현한다. 핵심 구현 패턴:

| 메서드 | GPUI 구현 방식 |
|--------|---------------|
| `insertText` | NSString/NSAttributedString 모두 처리, `replace_text_in_range` 호출 |
| `setMarkedText` | 텍스트 추출 후 `replace_and_mark_text_in_range` 호출 |
| `unmarkText` | `unmark_text` 호출로 조합 완료 |
| `hasMarkedText` | `marked_text_range` 조회 → BOOL 변환 |
| `firstRectForCharacterRange` | 입력 핸들러에서 bounds 조회 → 창 좌표 변환 → 화면 좌표 변환 |
| `doCommandBySelector` | 키스트로크 추출 후 이벤트 콜백으로 디스패치 |
| `validAttributesForMarkedText` | 빈 NSArray 반환 |

**참고**: [Zed GPUI window.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/platform/mac/window.rs)

---

## 5. 터미널 특화 IME 과제

### 5.1 조합 오버레이 렌더링

터미널에서 IME 조합 텍스트를 렌더링하는 두 가지 전략이 있다:

#### 전략 A: Builtin 렌더링 (WezTerm 기본)

조합 텍스트를 터미널 자체에서 렌더링:

```
장점:
  - 터미널 폰트와 동일한 모양
  - window:composition_status()와 연동 가능
  - 일관된 사용자 경험

단점:
  - 긴 preedit 문자열이 창 끝에서 잘릴 수 있음
  - 커서 위치 계산이 복잡함
```

#### 전략 B: System 렌더링

OS가 자체 오버레이 창으로 조합 텍스트를 표시:

```
장점:
  - 잘림 문제 없음
  - 구현이 단순
  - OS IME UI와 일관된 경험

단점:
  - 터미널 폰트와 다른 모양
  - 위치 제어가 제한적
```

#### Crux 권장 전략: 하이브리드

```rust
enum PreeditRenderMode {
    /// 터미널 셀 위에 직접 렌더링 (기본값)
    Builtin,
    /// OS IME 창에 위임
    System,
}

struct PreeditOverlay {
    text: String,
    cursor_position: (u16, u16),  // 터미널 셀 좌표
    selected_range: Range<usize>,
    render_mode: PreeditRenderMode,
}

impl CruxTerminal {
    fn render_preedit(&self, overlay: &PreeditOverlay) {
        match overlay.render_mode {
            PreeditRenderMode::Builtin => {
                // PTY 버퍼 위에 오버레이로 렌더링
                let (col, row) = overlay.cursor_position;
                let cell_size = self.cell_size();

                // 1. 커서 위치의 원래 셀 내용을 저장
                let saved_cells = self.save_cells_at(col, row, overlay.text.len());

                // 2. 조합 텍스트를 밑줄 스타일로 렌더링
                for (i, ch) in overlay.text.chars().enumerate() {
                    let width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
                    self.render_cell(
                        col + i as u16,
                        row,
                        ch,
                        CellStyle {
                            underline: true,         // 조합 중임을 시각적으로 표시
                            foreground: Color::Blue,  // 구분되는 색상
                            ..Default::default()
                        },
                    );
                }

                // 3. 조합 완료 시 원래 셀 복원
            }
            PreeditRenderMode::System => {
                // OS에 위임 (firstRectForCharacterRange만 정확히 반환하면 됨)
                // 별도 렌더링 코드 불필요
            }
        }
    }
}
```

**참고**: [WezTerm ime_preedit_rendering](https://wezterm.org/config/lua/config/ime_preedit_rendering.html)

### 5.2 커서 좌표 → 화면 좌표 변환

`firstRectForCharacterRange`의 핵심은 정확한 좌표 변환이다:

```
터미널 셀 좌표 (col, row)
      │
      ▼ (× cell_size)
뷰 좌표 (x, y) - NSView 내부 좌표
      │
      ▼ (convertRect:toView:nil)
창 좌표 - NSWindow 내부 좌표
      │
      ▼ (convertRectToScreen:)
화면 좌표 - 전역 화면 좌표 (IME가 사용)
```

```rust
fn cursor_to_screen_rect(&self, col: u16, row: u16) -> NSRect {
    let cell = self.cell_size();
    let content_offset = self.content_offset(); // 패딩, 스크롤바 등

    // 1단계: 셀 좌표 → 뷰 좌표
    // macOS는 좌하단이 원점이므로 Y축 반전 필요
    let total_rows = self.visible_rows();
    let view_x = content_offset.x + (col as f64 * cell.width);
    let view_y = content_offset.y + ((total_rows - row - 1) as f64 * cell.height);
    let view_rect = NSRect::new(
        NSPoint::new(view_x, view_y),
        NSSize::new(cell.width, cell.height),
    );

    // 2단계: 뷰 좌표 → 창 좌표
    let window_rect = unsafe { self.view.convertRect_toView(view_rect, None) };

    // 3단계: 창 좌표 → 화면 좌표
    let window = unsafe { self.view.window().unwrap() };
    unsafe { window.convertRectToScreen(window_rect) }
}
```

### 5.3 전각 문자(Wide Character) 커서 위치

CJK 문자는 터미널에서 2셀 너비를 차지한다. 이는 커서 위치 계산에 영향을 준다.

#### wcwidth 호환성

```rust
use unicode_width::UnicodeWidthChar;

fn cell_width(ch: char) -> u16 {
    // CJK 한글, 한자, 가나 등은 2셀
    UnicodeWidthChar::width(ch).unwrap_or(1) as u16
}

// 전각 문자 커서 예시:
// "hello한글world"
//  ^^^^^    ^^^^^  ← 각각 1셀
//       ^^^^       ← 각각 2셀
//
// 셀 위치: h=0, e=1, l=2, l=3, o=4, 한=5(+6), 글=7(+8), w=9, ...
```

#### 주의사항

| 문제 | 설명 | 해결 |
|------|------|------|
| Ambiguous Width | 일부 유니코드 문자의 너비가 모호함 | 설정으로 1/2셀 선택 가능하게 |
| wcwidth 불일치 | 터미널과 앱의 wcwidth 테이블이 다르면 렌더링 깨짐 | Unicode 15.1 기반 통일 |
| NFD 한글 | 분해형(NFD) 한글 처리 필요 | NFC 정규화 후 렌더링 |
| Zero-width | 결합 문자, ZWJ 등 0너비 문자 | 이전 셀에 결합 |

```rust
// Ambiguous width 설정
#[derive(Clone, Copy, PartialEq)]
enum AmbiguousWidth {
    Narrow, // 1셀 (기본값)
    Wide,   // 2셀 (CJK 로케일에서 사용)
}

fn effective_width(ch: char, config: AmbiguousWidth) -> u16 {
    match UnicodeWidthChar::width(ch) {
        Some(w) => w as u16,
        None => 0, // 제어 문자
    }
    // 참고: unicode-width 크레이트는 ambiguous width를 자동 처리하지 않음
    // East Asian Ambiguous 카테고리 문자는 별도 처리 필요
}
```

**참고**: [wcwidth documentation](https://wcwidth.readthedocs.io/en/latest/intro.html), [Windows Terminal #2066](https://github.com/microsoft/terminal/issues/2066)

### 5.4 Vim/Neovim IME 핸들링과의 상호작용

Vim/Neovim에서 한국어 IME를 사용할 때의 근본적 문제와 해결책:

#### 핵심 문제

Normal 모드에서는 영어 키 입력이 필요하지만, Insert 모드에서 한국어를 입력한 후 Esc로 Normal 모드로 전환하면 IME가 여전히 한국어 모드에 있다. 이로 인해 `j`, `dd` 같은 명령이 한국어 문자로 입력된다.

#### 기존 해결책

| 솔루션 | 설명 | 플랫폼 |
|--------|------|--------|
| [im-select.nvim](https://github.com/keaising/im-select.nvim) | 편집 모드에 따라 자동으로 입력기 전환 | 크로스 플랫폼 |
| [Korean-IME.nvim](https://github.com/kiyoon/Korean-IME.nvim) | OS 독립적 한글 입력기, 영어 키를 한국어로 변환 | 크로스 플랫폼 |
| [vim-barbaric](https://github.com/rlue/vim-barbaric) | IME 상태 자동 감지 및 전환 | 크로스 플랫폼 |

#### Crux 터미널이 도울 수 있는 방법

Crux는 터미널 에뮬레이터 레벨에서 vim의 모드 전환을 감지할 수 있다:

```rust
// 터미널 에뮬레이터 레벨에서의 IME 자동 전환
struct ImeAutoSwitch {
    enabled: bool,
    /// vim의 모드 감지를 위한 이스케이프 시퀀스 파서
    mode_detector: VimModeDetector,
}

impl ImeAutoSwitch {
    fn on_pty_output(&mut self, data: &[u8]) {
        if !self.enabled { return; }

        // vim이 커서 모양을 변경하는 이스케이프 시퀀스를 감지
        // 예: \e[2 q (블록 커서 = Normal), \e[6 q (바 커서 = Insert)
        if let Some(mode) = self.mode_detector.detect_mode_change(data) {
            match mode {
                VimMode::Normal | VimMode::Visual => {
                    // IME를 영어 모드로 자동 전환
                    self.switch_ime_to_ascii();
                }
                VimMode::Insert | VimMode::Replace => {
                    // 이전 IME 상태 복원
                    self.restore_previous_ime();
                }
            }
        }
    }

    fn switch_ime_to_ascii(&self) {
        // macOS: TISSelectInputSource를 사용하여 ASCII 입력 소스로 전환
        unsafe {
            let ascii_source = TISCopyCurrentASCIICapableKeyboardInputSource();
            TISSelectInputSource(ascii_source);
        }
    }
}
```

> **참고**: Zed 에디터에서도 `jj` 매핑과 한국어 IME 충돌 문제가 보고됨 ([Zed #38616](https://github.com/zed-industries/zed/issues/38616)). 이 문제는 `jj`를 Normal 모드로 매핑할 때 한국어 `ㅓㅓ`가 먼저 입력되어 트리거되지 않는 것이다.

**참고**: [im-select.nvim](https://github.com/keaising/im-select.nvim), [Korean-IME.nvim](https://github.com/kiyoon/Korean-IME.nvim), [Neovim #16052](https://github.com/neovim/neovim/issues/16052)

---

## 6. 참고 자료

### Apple 공식 문서
- [NSTextInputClient Protocol](https://developer.apple.com/documentation/appkit/nstextinputclient) - IME 프로토콜 정의
- [NSPasteboard](https://developer.apple.com/documentation/appkit/nspasteboard) - 클립보드 API
- [Drag and Drop Concepts](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/DragandDrop/DragandDrop.html) - 드래그 앤 드롭 가이드
- [Text Input Architecture](https://developer.apple.com/library/archive/documentation/TextFonts/Conceptual/CocoaTextArchitecture/TextEditing/TextEditing.html) - 텍스트 입력 아키텍처

### Rust 크레이트
- [objc2](https://github.com/madsmtm/objc2) - Apple 프레임워크 바인딩
- [objc2-app-kit NSTextInputClient](https://docs.rs/objc2-app-kit/latest/x86_64-unknown-linux-gnu/objc2_app_kit/trait.NSTextInputClient.html) - Rust NSTextInputClient 트레이트
- [clipboard-rs](https://github.com/ChurchTao/clipboard-rs) - 크로스 플랫폼 클립보드 라이브러리
- [arboard](https://github.com/1Password/arboard) - 1Password의 클립보드 라이브러리
- [unicode-width](https://crates.io/crates/unicode-width) - 유니코드 문자 너비 계산

### 터미널 IME 버그 사례
- [Alacritty #4469](https://github.com/alacritty/alacritty/issues/4469) - 한국어 IME 프리즈
- [Alacritty #8079](https://github.com/alacritty/alacritty/issues/8079) - CJK 이중 스페이스
- [Ghostty #4634](https://github.com/ghostty-org/ghostty/issues/4634) - preedit 사라짐 (수정됨)
- [Ghostty #7225](https://github.com/ghostty-org/ghostty/issues/7225) - Backspace 조합 문자 삭제 버그
- [Claude Code #19207](https://github.com/anthropics/claude-code/issues/19207) - IME 커서 위치 오류 (수정됨)
- [Claude Code #21382](https://github.com/anthropics/claude-code/issues/21382) - Quick Launcher 한국어 IME 실패
- [WezTerm #1474](https://github.com/wezterm/wezterm/issues/1474) - NFD 한글 렌더링
- [WezTerm #2569](https://github.com/wezterm/wezterm/issues/2569) - preedit이 모든 패널에 표시

### 한글 유니코드/조합
- [How Korean input methods work](https://m10k.eu/2025/03/08/hangul-utf8.html) - 한글 입력기 동작 원리 상세 설명
- [Korean IME - Microsoft](https://learn.microsoft.com/en-us/globalization/input/korean-ime) - Microsoft 한국어 IME 가이드
- [Hangul Unicode FAQ](https://corp.unicode.org/~asmus/proposed_faq/korean.html) - 유니코드 한글 FAQ
- [wcwidth documentation](https://wcwidth.readthedocs.io/en/latest/intro.html) - 문자 너비 표준

### Zed 에디터 (GPUI)
- [GPUI window.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/platform/mac/window.rs) - NSTextInputClient 구현 참조
- [GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md) - GPUI 개요

### Vim/Neovim IME
- [im-select.nvim](https://github.com/keaising/im-select.nvim) - Neovim IME 자동 전환
- [Korean-IME.nvim](https://github.com/kiyoon/Korean-IME.nvim) - Neovim 한글 입력기
- [Zed #38616](https://github.com/zed-industries/zed/issues/38616) - jj 매핑과 한국어 IME 충돌

### WezTerm IME 설정
- [WezTerm ime_preedit_rendering](https://wezterm.org/config/lua/config/ime_preedit_rendering.html) - preedit 렌더링 옵션
- [WezTerm use_ime](https://wezterm.org/config/lua/config/use_ime.html) - IME 사용 설정

### 마이그레이션 가이드
- [core-foundation-rs → objc2 마이그레이션](https://github.com/servo/core-foundation-rs/issues/729) - objc2로의 전환 논의
- [objc2 tracking issue](https://github.com/madsmtm/objc2/issues/719) - core-foundation-rs 호환 추적

---

## 부록: Crux IME 구현 체크리스트

### 필수 (P0)

- [ ] `NSTextInputClient` 프로토콜 전체 구현
- [ ] 한글 조합 중 `setMarkedText` → 오버레이 렌더링 (PTY 미전송)
- [ ] `insertText` → PTY에 확정 텍스트 전송
- [ ] `firstRectForCharacterRange` → 정확한 화면 좌표 반환
- [ ] preedit 중 수식키 입력 무시 (Ghostty 교훈)
- [ ] IME 커밋/키보드 이벤트 중복 방지 (Alacritty 교훈)
- [ ] 전각 문자 2셀 너비 처리 (wcwidth)

### 중요 (P1)

- [ ] NSPasteboard 이미지 읽기 (PNG, TIFF)
- [ ] 클립보드 이미지 → 임시 파일 변환
- [ ] 드래그 앤 드롭 지원 (NSDraggingDestination)
- [ ] 클립보드 컨텐츠 타입 감지
- [ ] NFD 한글 정규화 (NFC 변환)

### 차별화 (P2)

- [ ] Vim 모드 감지를 통한 IME 자동 전환
- [ ] Builtin/System preedit 렌더링 모드 선택
- [ ] 클립보드 변경 모니터링
- [ ] 조합 상태 시각적 피드백 (밑줄, 색상 구분)
- [ ] IME 크래시 타임아웃/복구 메커니즘
- [ ] Ambiguous Width 문자 설정 (1셀/2셀)

---

## 7. 심화 연구: Ghostty IME PR 상세 분석

### 7.1 PR #4649: "macos: ignore modifier changes while IM is active"

**문제**: macOS에서 일본어/한국어 IME로 입력 중 수식키(Shift, Ctrl, Option, Command)를 누르면 preedit 텍스트가 화면에서 사라짐. 내부 조합 상태는 유지되지만 시각적으로 사라지는 현상.

**근본 원인**: Ghostty의 `flagsChanged` 이벤트 핸들러가 IME 조합 활성 여부를 확인하지 않고 수식키 이벤트를 처리. 이로 인해 수식키만 단독으로 눌러도 IME의 preedit 상태 관리에 간섭.

**수정 패턴**:
```swift
// SurfaceView_AppKit.swift - flagsChanged 핸들러
override func flagsChanged(with event: NSEvent) {
    // 핵심: IME 조합 중이면 수식키 변경을 무시
    if self.hasMarkedText() {
        return  // 이벤트 소비, preedit 상태 보호
    }

    // IME 비활성 시에만 수식키 처리
    super.flagsChanged(with: event)
}
```

**영향**: 이 패턴은 macOS 기본 IME, Google 일본어 입력, macSKK 등 모든 서드파티 IME에 적용됨.

### 7.2 PR #4854: "macos: Handle ctrl characters in IME input"

**문제 1**: IME가 Ctrl+키 조합을 명령으로 사용할 때(예: Ctrl+H로 변환 취소), Ghostty의 libghostty가 이를 가로채서 터미널 제어 문자로 해석.

**문제 2**: IME가 텍스트를 확정(commit)할 때 Ctrl 수식자가 남아있으면, Ghostty가 확정된 텍스트를 Ctrl+문자로 잘못 해석하여 터미널 제어 시퀀스를 생성.

**수정 패턴**:
```swift
// 수정 1: preedit 상태에서 Ctrl+key를 libghostty에 전달하지 않음
func keyDown(with event: NSEvent) {
    if self.hasMarkedText() {
        // IME에 우선권 부여 - libghostty 우회
        self.inputContext?.handleEvent(event)
        return
    }
    // ...일반 키 처리
}

// 수정 2: IME 확정 텍스트에서 control modifier 제거
func insertText(_ string: Any, replacementRange: NSRange) {
    // control modifier를 strip하여
    // 특수 ctrl+key 핸들링 트리거 방지
    let cleanText = stripControlModifiers(string)
    self.ptyWriter.write(cleanText)
}
```

### 7.3 v1.2.0 추가 수정: "Key input that clears preedit without text shouldn't encode to pty"

**문제**: 사용자가 preedit를 취소(ESC 등)하면 빈 텍스트로 preedit가 클리어되는데, 이때 해당 키 이벤트가 PTY로도 전달되어 의도하지 않은 동작 발생.

**교훈**: preedit 클리어 시 생성되는 키 이벤트는 PTY에 인코딩하면 안 됨.

```rust
// Crux 구현 시 적용할 패턴
fn handle_key_event(&mut self, event: KeyEvent) -> bool {
    let had_preedit = self.has_marked_text();

    // IME에 먼저 전달
    let handled = self.input_context.handle_event(&event);

    let has_preedit_now = self.has_marked_text();

    // preedit가 클리어되었지만 텍스트 커밋이 없는 경우
    // → 키 이벤트를 PTY에 전달하지 않음
    if had_preedit && !has_preedit_now && !self.text_was_committed {
        return true; // 이벤트 소비
    }

    handled
}
```

**참고**: [Ghostty #4634](https://github.com/ghostty-org/ghostty/issues/4634), [Ghostty 1.1.0 Release Notes](https://ghostty.org/docs/install/release-notes/1-1-0), [Ghostty 1.2.0 Release Notes](https://ghostty.org/docs/install/release-notes/1-2-0)

---

## 8. 심화 연구: Zed GPUI의 EntityInputHandler 아키텍처

### 8.1 InputHandler 추상화 계층

Zed의 GPUI는 플랫폼별 IME를 통합하기 위해 다층 추상화를 사용한다:

```
macOS NSTextInputClient (Objective-C 프로토콜)
        │
        ▼
PlatformInputHandler (플랫폼 추상화 래퍼)
        │
        ▼
InputHandler 트레이트 (GPUI 코어)
        │
        ▼
EntityInputHandler 트레이트 (에디터 구현)
```

### 8.2 InputHandler 트레이트 핵심 메서드

```rust
// GPUI의 InputHandler 트레이트 (crates/gpui/src/input.rs)
pub trait InputHandler: 'static {
    /// 현재 선택 범위 반환 (커서 위치)
    fn selected_text_range(&mut self, cx: &mut WindowContext) -> Option<Range<usize>>;

    /// 조합 중인(marked) 텍스트 범위 반환
    fn marked_text_range(&self, cx: &mut WindowContext) -> Option<Range<usize>>;

    /// IME 후보 창 위치를 위한 bounds 반환
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        cx: &mut WindowContext,
    ) -> Option<Bounds<Pixels>>;

    /// 텍스트를 범위에 삽입 (IME 확정 시 호출)
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        cx: &mut WindowContext,
    );

    /// 텍스트를 삽입하고 marked로 설정 (IME 조합 중 호출)
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        cx: &mut WindowContext,
    );

    /// marked 텍스트 해제
    fn unmark_text(&mut self, cx: &mut WindowContext);
}
```

### 8.3 macOS NSTextInputClient → GPUI InputHandler 매핑

| NSTextInputClient 메서드 | GPUI InputHandler 메서드 |
|--------------------------|-------------------------|
| `insertText:replacementRange:` | `replace_text_in_range()` |
| `setMarkedText:selectedRange:replacementRange:` | `replace_and_mark_text_in_range()` |
| `unmarkText` | `unmark_text()` |
| `hasMarkedText` | `marked_text_range().is_some()` |
| `markedRange` | `marked_text_range()` |
| `selectedRange` | `selected_text_range()` |
| `firstRectForCharacterRange:actualRange:` | `bounds_for_range()` → 좌표 변환 |
| `validAttributesForMarkedText` | 빈 NSArray 반환 (GPUI에서 직접 처리) |

### 8.4 PlatformInputHandler 좌표 변환 흐름

```rust
// crates/gpui/src/platform/mac/window.rs 에서의 좌표 변환
extern "C" fn first_rect_for_character_range(
    this: &WinitWindow,
    _: Sel,
    range: NSRange,
    actual_range: *mut NSRange,
) -> NSRect {
    // 1. InputHandler에서 bounds 조회
    let bounds = input_handler.bounds_for_range(range.location..range.location + range.length);

    // 2. GPUI 논리 좌표 → 물리 픽셀 좌표
    let scale = window.scale_factor();
    let origin = bounds.origin.scale(scale);
    let size = bounds.size.scale(scale);

    // 3. 뷰 좌표 → 창 좌표 (Y축 반전)
    let content_height = view.frame().size.height;
    let window_y = content_height - origin.y - size.height;

    // 4. 창 좌표 → 화면 좌표
    let window_rect = NSRect::new(
        NSPoint::new(origin.x as f64, window_y as f64),
        NSSize::new(size.width as f64, size.height as f64),
    );
    window.convertRectToScreen(window_rect)
}
```

### 8.5 Zed #28174: CJK IME 키맵 우선순위 충돌

**문제**: Zed Vim 모드에서 `jj → Escape` 키바인딩을 설정하면, 중국어 IME로 `j`로 시작하는 글자를 입력할 수 없음. Zed가 키맵을 IME보다 먼저 평가하기 때문.

**일반 Vim에서의 동작**: 콘텐츠가 먼저 IME에 의해 차단되고, 중국어로 변환된 후 에디터에 입력됨. 즉 `j`가 IME를 통과하여 한자가 되므로 `jj` 바인딩이 트리거되지 않음.

**Zed의 변경 (2024년 11월)**: 키보드 단축키가 IME 시스템 트리거 전에 디스패치되도록 변경. 이는 일본어 키보드에서 IME 메뉴 없이 vim normal 모드를 사용할 수 있게 하지만, 중국어/한국어 IME 사용자에게는 문제를 야기함.

**Crux 설계 시사점**:
```rust
// IME 활성 시 키맵 평가 순서
enum KeyDispatchOrder {
    /// IME 우선 (CJK 입력에 적합)
    /// 키 → IME → (IME가 처리 안 하면) → 키맵
    ImeFirst,

    /// 키맵 우선 (vim normal 모드에 적합)
    /// 키 → 키맵 → (키맵 매칭 없으면) → IME
    KeymapFirst,
}

// 설정으로 제어 가능하게 하되, 상황에 따라 자동 전환
fn get_dispatch_order(&self) -> KeyDispatchOrder {
    if self.vim_mode == VimMode::Insert && self.has_active_ime() {
        KeyDispatchOrder::ImeFirst  // Insert 모드에서는 IME 우선
    } else {
        KeyDispatchOrder::KeymapFirst  // Normal 모드에서는 키맵 우선
    }
}
```

**참고**: [Zed #28174](https://github.com/zed-industries/zed/issues/28174), [Zed #12678](https://github.com/zed-industries/zed/pull/12678)

---

## 9. 심화 연구: WezTerm NFD/NFC 한글 정규화

### 9.1 문제의 본질

macOS는 파일시스템(HFS+/APFS)에서 유니코드 문자열을 **NFD(Canonical Decomposition)** 형태로 저장한다. 한글의 경우:

| 형태 | 표현 | 코드포인트 |
|------|------|-----------|
| **NFC** (조합형) | 한 | U+D55C (단일 코드포인트) |
| **NFD** (분해형) | 한 | U+1112 U+1161 U+11AB (초성ㅎ + 중성ㅏ + 종성ㄴ) |

Finder에서 생성된 파일명이나 `ls` 출력이 NFD로 전달되면, 터미널의 텍스트 셰이핑 엔진이 이를 올바르게 처리해야 한다.

### 9.2 WezTerm의 NFD 문제점

1. **Bold 텍스트 렌더링 깨짐**: NFD 한글이 일반 weight에서는 정상 렌더링되지만, bold 스타일링 적용 시 글자가 깨짐
2. **tmux 선택 시 깨짐**: tmux의 마우스 선택 기능이 셀 속성을 변경하면서 NFD 텍스트를 별도의 셰이핑 런(run)으로 분리 → 완전한 자모 시퀀스가 원자적으로 처리되지 않음
3. **커서 위치 오계산**: NFD 한글 문자의 텍스트 너비 계산 로직이 분해된 자모 각각을 별도 문자로 계산

### 9.3 WezTerm의 해결 접근

```rust
// WezTerm의 normalize_output_to_unicode_nfc 설정
// wezterm.lua:
// config.normalize_output_to_unicode_nfc = true  (기본값)

// 내부 구현 패턴:
fn process_pty_output(&mut self, data: &[u8]) {
    let text = String::from_utf8_lossy(data);

    if self.config.normalize_output_to_unicode_nfc {
        // 모든 PTY 출력을 NFC로 정규화
        use unicode_normalization::UnicodeNormalization;
        let normalized: String = text.nfc().collect();
        self.terminal.feed(&normalized);
    } else {
        self.terminal.feed(&text);
    }
}

// 한계: 속성 경계를 넘는 정규화는 불가능
// 예: tmux가 셀 1은 bold, 셀 2는 normal로 만들면
//     NFD 자모가 분리된 셰이핑 런에 걸쳐 있게 됨
```

### 9.4 알려진 한계: normalize_output_to_unicode_nfc = false

WezTerm #3732에서 보고된 바와 같이, `normalize_output_to_unicode_nfc = false`로 설정해도 NFD 문자열이 항상 NFC로 정규화되는 버그가 존재. 이는 내부적으로 셰이퍼가 암시적으로 NFC 변환을 수행하기 때문.

### 9.5 Crux 설계 권장사항

```rust
// Crux에서의 한글 정규화 전략
use unicode_normalization::UnicodeNormalization;

/// 터미널 입출력의 유니코드 정규화 설정
struct NormalizationConfig {
    /// PTY 출력을 NFC로 정규화 (기본: true)
    normalize_output: bool,
    /// IME 입력을 NFC로 정규화 (기본: true, 한국어에 필수)
    normalize_input: bool,
}

impl Terminal {
    fn feed_output(&mut self, data: &[u8]) {
        let text = String::from_utf8_lossy(data);

        if self.config.normalize_output {
            // 셰이핑 런 경계를 고려한 정규화
            // 같은 속성의 연속 셀을 하나의 런으로 묶어 정규화
            for run in self.group_by_attributes(&text) {
                let normalized: String = run.text.nfc().collect();
                self.process_run(&normalized, &run.attributes);
            }
        } else {
            self.process_text(&text);
        }
    }

    /// 셀 너비 계산 시 NFD도 올바르게 처리
    fn calculate_text_width(&self, text: &str) -> usize {
        // NFD 한글을 NFC로 변환 후 너비 계산
        let nfc: String = text.nfc().collect();
        unicode_width::UnicodeWidthStr::width(nfc.as_str())
    }
}
```

**참고**: [WezTerm #2482](https://github.com/wezterm/wezterm/issues/2482), [WezTerm #3732](https://github.com/wezterm/wezterm/issues/3732), [WezTerm normalize_output_to_unicode_nfc](https://wezterm.org/config/lua/config/normalize_output_to_unicode_nfc.html)

---

## 10. 심화 연구: CJK 폰트 렌더링 모범사례

### 10.1 CoreText 폰트 폴백 체인

macOS에서 CJK 폰트 폴백을 올바르게 구성하려면 `CTFontCopyDefaultCascadeListForLanguages` API를 사용한다:

```rust
use core_text::font::CTFont;

/// CoreText의 언어 인식 폰트 폴백 체인 구성
fn build_cjk_fallback_chain(
    primary_font: &CTFont,
    locale: &str,  // "ko", "ja", "zh-Hans", "zh-Hant"
) -> Vec<CTFontDescriptor> {
    // CTFontCopyDefaultCascadeListForLanguages는
    // 지정된 로케일에 맞춰 CJK 폰트를 우선 정렬
    let languages = CFArray::from_cftype_pairs(&[
        CFString::new(locale),
    ]);

    let cascade_list = primary_font
        .copy_default_cascade_list_for_languages(&languages);

    // 반환된 리스트는 로케일에 맞는 순서로 정렬됨
    // 예: locale="ko" → Apple SD Gothic Neo, PingFang SC, ...
    // 예: locale="ja" → Hiragino Sans, PingFang SC, ...
    cascade_list
}
```

### 10.2 Han Unification 처리

같은 유니코드 코드포인트(예: U+9AA8 骨)가 한국어/일본어/중국어에서 다르게 렌더링되어야 한다:

| 코드포인트 | 한국어 | 일본어 | 중국어 간체 | 중국어 번체 |
|-----------|--------|--------|------------|------------|
| U+9AA8 骨 | 한국식 글리프 | 일본식 글리프 | 간체 글리프 | 번체 글리프 |
| U+76F4 直 | 한국식 | 일본식 | 간체 | 번체 |

**해결 전략**:
```rust
/// 로케일 기반 폰트 선택
/// CTFontCopyDefaultCascadeListForLanguages가 로케일을 받아
/// Han Unification 문자에 대해 올바른 글리프 변형을 선택
fn select_font_for_codepoint(
    codepoint: char,
    locale: &str,
    fallback_chain: &[CTFontDescriptor],
) -> Option<CTFont> {
    // 1. 캐스케이드 리스트에서 해당 코드포인트를 포함하는 첫 번째 폰트 선택
    // 2. 로케일에 맞는 폰트가 우선 정렬되어 있으므로
    //    자연스럽게 올바른 글리프 변형이 선택됨
    for descriptor in fallback_chain {
        let font = CTFont::from_descriptor(descriptor, size);
        if font.contains_glyph_for_char(codepoint) {
            return Some(font);
        }
    }
    None
}
```

### 10.3 CJK 폰트 크기 조정

CJK 폰트는 일반적으로 유용한 cap-height 메트릭이 없어, 라틴 기본 폰트와 함께 사용할 때 크기 불균형이 발생한다:

```rust
/// WezTerm 스타일의 폰트 폴백 크기 조정
struct FontFallbackConfig {
    /// 기본 폰트 (라틴)
    primary: FontSpec,
    /// 폴백 폰트 체인
    fallbacks: Vec<FallbackFontSpec>,
}

struct FallbackFontSpec {
    font: FontSpec,
    /// CJK 폰트의 스케일링 팩터 (기본 1.0)
    /// CJK 폰트가 작게 보이면 1.1~1.2로 설정
    scale: f64,
    /// 수직 오프셋 조정 (픽셀)
    vertical_offset: f64,
}

// WezTerm 설정 예시:
// wezterm.font_with_fallback({
//   'JetBrains Mono',
//   { family = 'Apple SD Gothic Neo', scale = 1.1 },
// })
```

### 10.4 텍스트 셰이핑과 HarfBuzz

터미널에서의 텍스트 셰이핑은 일반 에디터보다 제약이 있지만, CJK 텍스트를 위해 HarfBuzz 기반 셰이핑이 권장된다:

```rust
// 셰이핑 파이프라인
fn shape_text_run(
    &self,
    text: &str,
    font: &Font,
    direction: Direction,  // LTR for CJK
) -> Vec<GlyphInfo> {
    let buffer = hb::Buffer::new();
    buffer.add_str(text);
    buffer.set_direction(direction);

    // CJK에 중요: 스크립트와 언어 설정
    buffer.set_script(hb::Script::Hangul); // 또는 Han, Katakana 등
    buffer.set_language(hb::Language::from_string("ko"));

    hb::shape(&font.hb_font, &buffer, &[]);

    buffer.glyph_infos().iter().zip(buffer.glyph_positions()).map(|(info, pos)| {
        GlyphInfo {
            codepoint: info.codepoint,
            cluster: info.cluster,
            x_advance: pos.x_advance,
            y_advance: pos.y_advance,
            x_offset: pos.x_offset,
            y_offset: pos.y_offset,
        }
    }).collect()
}
```

**참고**: [Font Fallback Deep Dive - Raph Levien](https://raphlinus.github.io/rust/skribo/text/2019/04/04/font-fallback.html), [WezTerm font_with_fallback](https://wezterm.org/config/lua/wezterm/font_with_fallback.html)

---

## 11. 심화 연구: Mode 2027 및 그래핌 클러스터 너비

### 11.1 기존 wcwidth의 한계

전통적인 `wcwidth()`는 단일 코드포인트 기반으로 문자 너비를 계산한다. 이는 다음 상황에서 실패한다:

| 상황 | wcwidth 결과 | 실제 기대 | 문제 |
|------|-------------|----------|------|
| 한글 자모 (NFD) | 각 자모 1 | 조합 후 2 | NFD 한글이 3-6셀로 표시 |
| 이모지 ZWJ 시퀀스 | 각 코드포인트 합산 | 2 | 가족 이모지가 8셀+ |
| 국기 이모지 | Regional Indicator 각 1 | 2 | 국기가 2개 문자로 분리 |
| VS16 이모지 | 기본 너비 | 2 | 텍스트 표현과 이모지 표현 혼동 |

### 11.2 Mode 2027 (terminal-unicode-core)

Mode 2027은 Contour 터미널 작성자가 제안한 DEC Private Mode로, 그래핌 클러스터 기반 너비 계산을 활성화한다:

```
# Mode 2027 쿼리 (DECRQM)
CSI ? 2027 $ p

# 응답:
CSI ? 2027 ; Ps $ y
# Ps=1: set (지원, 활성화됨)
# Ps=2: reset (지원, 비활성화됨)
# Ps=0: not recognized (미지원)

# Mode 2027 활성화
CSI ? 2027 h

# Mode 2027 비활성화
CSI ? 2027 l
```

### 11.3 그래핌 클러스터 너비 계산

```rust
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Mode 2027 활성화 시 그래핌 클러스터 기반 너비 계산
fn grapheme_cluster_width(text: &str, mode_2027: bool) -> usize {
    if mode_2027 {
        // 그래핌 클러스터 단위로 너비 계산
        text.graphemes(true)
            .map(|g| grapheme_display_width(g))
            .sum()
    } else {
        // 레거시: 코드포인트 단위
        UnicodeWidthStr::width(text)
    }
}

/// 개별 그래핌 클러스터의 표시 너비
fn grapheme_display_width(grapheme: &str) -> usize {
    let chars: Vec<char> = grapheme.chars().collect();

    match chars.len() {
        0 => 0,
        1 => {
            // 단일 코드포인트: 전통적 wcwidth 사용
            unicode_width::UnicodeWidthChar::width(chars[0]).unwrap_or(0)
        }
        _ => {
            // 다중 코드포인트 그래핌 클러스터
            // 기본 문자의 East Asian Width 속성 확인
            let base = chars[0];
            if is_emoji_presentation(grapheme) {
                2  // 이모지는 항상 2셀
            } else if is_hangul_syllable_nfd(grapheme) {
                2  // NFD 한글 음절은 2셀
            } else {
                unicode_width::UnicodeWidthChar::width(base).unwrap_or(1)
            }
        }
    }
}

/// NFD 한글 음절 감지
fn is_hangul_syllable_nfd(grapheme: &str) -> bool {
    grapheme.chars().next().map_or(false, |c| {
        // 한글 자모 범위: U+1100-U+11FF (초성, 중성, 종성)
        matches!(c, '\u{1100}'..='\u{115F}')  // 초성 (Leading Jamo)
    })
}
```

### 11.4 지원 현황

| 터미널 | Mode 2027 지원 | 그래핌 클러스터 처리 |
|--------|---------------|-------------------|
| Contour | 완전 지원 (제안자) | 전체 |
| WezTerm | 부분 지원 (#4320) | 일부 |
| Ghostty | 완전 지원 | 전체 |
| Kitty | 논의 중 (#7799) | 독자 구현 |
| Alacritty | 미지원 | 레거시 wcwidth |

### 11.5 Crux 권장 구현

```rust
/// Crux 터미널의 문자 너비 계산 엔진
struct WidthEngine {
    mode_2027: bool,
    ambiguous_width: AmbiguousWidth,
    unicode_version: UnicodeVersion,  // Unicode 15.1 기반
}

impl WidthEngine {
    fn cell_width(&self, text: &str) -> usize {
        if self.mode_2027 {
            self.grapheme_width(text)
        } else {
            self.legacy_width(text)
        }
    }

    /// 앱이 Mode 2027을 쿼리하면 지원을 알림
    fn handle_decrqm(&self, mode: u16) -> DecrmResponse {
        match mode {
            2027 => DecrqmResponse {
                mode: 2027,
                value: if self.mode_2027 { 1 } else { 2 },
            },
            _ => DecrqmResponse { mode, value: 0 },
        }
    }
}
```

**참고**: [Grapheme Clusters and Terminal Emulators - Mitchell Hashimoto](https://mitchellh.com/writing/grapheme-clusters-in-terminals), [WezTerm #4320](https://github.com/wezterm/wezterm/issues/4320), [Kitty #7799](https://github.com/kovidgoyal/kitty/issues/7799), [UAX #11: East Asian Width](https://www.unicode.org/reports/tr11/tr11-40.html)

---

## 12. 심화 연구: Vim 모드 IME 자동 전환 상세

### 12.1 TISSelectInputSource API의 CJK 버그

macOS의 `TISSelectInputSource` API에는 CJKV 입력 소스 전환 시 알려진 버그가 있다:

**증상**: `TISSelectInputSource()`를 호출하면 메뉴바의 입력 소스 아이콘은 변경되지만, 실제 입력 소스는 변경되지 않는 경우가 있음. 다른 앱을 활성화했다가 돌아오면 정상 동작.

**원인**: Carbon 라이브러리의 레거시 코드가 복잡한 CJKV 입력 소스의 활성화/비활성화를 제대로 처리하지 못함.

### 12.2 macism: 신뢰할 수 있는 입력 소스 전환

`macism`은 이 버그를 우회하는 유일한 CLI 도구이다:

```swift
// macism의 핵심 구현 (macism.swift + InputSourceManager.swift)
class InputSourceManager {
    static func initialize() {
        // 모든 가용 입력 소스 목록 캐시
    }

    static func getCurrentSource() -> String {
        // TISCopyCurrentKeyboardInputSource() 사용
        let source = TISCopyCurrentKeyboardInputSource().takeRetainedValue()
        let id = TISGetInputSourceProperty(source, kTISPropertyInputSourceID)
        return Unmanaged<CFString>.fromOpaque(id!).takeUnretainedValue() as String
    }

    func select(source: InputSource) {
        // 핵심 우회 방법:
        // 1. TISSelectInputSource() 호출
        // 2. 짧은 지연 후 CGEvent를 생성하여 키보드 이벤트 시뮬레이션
        // 3. 이를 통해 시스템이 실제로 입력 소스를 전환하도록 강제
        TISSelectInputSource(source.ref)

        // 버그 우회를 위한 이벤트 트릭
        usleep(waitTimeMs * 1000)
        let event = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: true)
        event?.post(tap: .cghidEventTap)
    }
}
```

**사용 예**:
```bash
# 현재 입력 소스 조회
macism
# 출력: com.apple.inputmethod.Korean.2SetKorean

# 영문 입력으로 전환
macism com.apple.keylayout.ABC

# 한국어 입력으로 전환
macism com.apple.inputmethod.Korean.2SetKorean
```

### 12.3 커서 모양 이스케이프 시퀀스를 통한 Vim 모드 감지

DECSCUSR(Set Cursor Style) 시퀀스를 파싱하여 Vim의 모드 전환을 감지할 수 있다:

```
DECSCUSR 형식: ESC [ Ps SP q

Ps 값:
  0 또는 1: 깜빡이는 블록 (Normal 모드 기본)
  2: 고정 블록 (Normal 모드)
  3: 깜빡이는 밑줄
  4: 고정 밑줄
  5: 깜빡이는 바 (Insert 모드 기본)
  6: 고정 바 (Insert 모드)
```

```rust
/// DECSCUSR 파서를 통한 Vim 모드 감지
struct VimModeDetector {
    /// 마지막으로 감지된 커서 스타일
    last_cursor_style: Option<CursorStyle>,
    /// 이전 IME 상태 (복원용)
    saved_ime_source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CursorStyle {
    BlinkingBlock = 1,
    SteadyBlock = 2,
    BlinkingUnderline = 3,
    SteadyUnderline = 4,
    BlinkingBar = 5,
    SteadyBar = 6,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum VimMode {
    Normal,   // 블록 커서 (1, 2)
    Insert,   // 바 커서 (5, 6)
    Replace,  // 밑줄 커서 (3, 4)
    Visual,   // 블록 커서 (Normal과 동일)
}

impl VimModeDetector {
    /// PTY 출력에서 DECSCUSR 시퀀스 감지
    fn detect_mode_change(&mut self, data: &[u8]) -> Option<VimMode> {
        // ESC [ Ps SP q 패턴 검색
        // 바이트 시퀀스: 0x1B 0x5B [digit] 0x20 0x71
        for window in data.windows(5) {
            if window[0] == 0x1B && window[1] == 0x5B
               && window[3] == 0x20 && window[4] == 0x71
            {
                let ps = window[2] - b'0';
                let style = match ps {
                    0 | 1 => CursorStyle::BlinkingBlock,
                    2 => CursorStyle::SteadyBlock,
                    3 => CursorStyle::BlinkingUnderline,
                    4 => CursorStyle::SteadyUnderline,
                    5 => CursorStyle::BlinkingBar,
                    6 => CursorStyle::SteadyBar,
                    _ => continue,
                };

                let prev = self.last_cursor_style;
                self.last_cursor_style = Some(style);

                // 이전 스타일과 비교하여 모드 전환 감지
                if prev != Some(style) {
                    return Some(match style {
                        CursorStyle::BlinkingBlock | CursorStyle::SteadyBlock
                            => VimMode::Normal,
                        CursorStyle::BlinkingBar | CursorStyle::SteadyBar
                            => VimMode::Insert,
                        CursorStyle::BlinkingUnderline | CursorStyle::SteadyUnderline
                            => VimMode::Replace,
                    });
                }
            }
        }
        None
    }
}
```

### 12.4 TISSelectInputSource Rust 바인딩

```rust
// Crux에서 사용할 macOS 입력 소스 전환 API
use core_foundation::string::CFString;
use std::ffi::c_void;

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCopyCurrentKeyboardInputSource() -> *mut c_void;
    fn TISCopyCurrentASCIICapableKeyboardInputSource() -> *mut c_void;
    fn TISSelectInputSource(inputSource: *mut c_void) -> i32;
    fn TISGetInputSourceProperty(
        inputSource: *mut c_void,
        propertyKey: *const c_void,
    ) -> *mut c_void;
    fn TISCreateInputSourceList(
        properties: *const c_void,
        includeAllInstalled: bool,
    ) -> *mut c_void;

    static kTISPropertyInputSourceID: *const c_void;
    static kTISPropertyInputSourceCategory: *const c_void;
}

/// macOS 입력 소스 관리자
struct InputSourceManager {
    /// ASCII 입력 소스 캐시
    ascii_source: *mut c_void,
    /// 사용자의 이전 입력 소스
    previous_source: Option<*mut c_void>,
}

impl InputSourceManager {
    fn new() -> Self {
        unsafe {
            Self {
                ascii_source: TISCopyCurrentASCIICapableKeyboardInputSource(),
                previous_source: None,
            }
        }
    }

    /// 현재 입력 소스 ID 조회
    fn current_source_id(&self) -> String {
        unsafe {
            let source = TISCopyCurrentKeyboardInputSource();
            let id_ptr = TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
            let cf_string = id_ptr as *const CFString;
            (*cf_string).to_string()
        }
    }

    /// ASCII 입력 소스로 전환 (vim Normal 모드 진입 시)
    fn switch_to_ascii(&mut self) {
        unsafe {
            // 현재 소스 저장 (나중에 복원하기 위해)
            self.previous_source = Some(TISCopyCurrentKeyboardInputSource());

            // ASCII로 전환
            TISSelectInputSource(self.ascii_source);
        }
    }

    /// 이전 입력 소스로 복원 (vim Insert 모드 진입 시)
    fn restore_previous(&mut self) {
        if let Some(source) = self.previous_source.take() {
            unsafe {
                TISSelectInputSource(source);
            }
        }
    }
}
```

### 12.5 Crux의 통합 IME 자동 전환 설계

```rust
/// Crux 설정: IME 자동 전환
struct ImeAutoSwitchConfig {
    /// 기능 활성화
    enabled: bool,
    /// Normal 모드에서 사용할 입력 소스
    /// 기본: 시스템의 ASCII 입력 소스
    normal_mode_source: Option<String>,
    /// 커서 스타일 기반 감지 활성화
    detect_cursor_style: bool,
    /// 모드 전환 시 지연 (ms) - macism의 waitTimeMs와 유사
    switch_delay_ms: u32,
}

impl Default for ImeAutoSwitchConfig {
    fn default() -> Self {
        Self {
            enabled: true,  // 한국어 우선 설계
            normal_mode_source: None,  // ASCII 자동 감지
            detect_cursor_style: true,
            switch_delay_ms: 50,
        }
    }
}
```

**참고**: [macism GitHub](https://github.com/laishulu/macism), [im-select GitHub](https://github.com/daipeihust/im-select), [im-select.nvim](https://github.com/keaising/im-select.nvim), [Vim cursor shape tips](https://vim.fandom.com/wiki/Change_cursor_shape_in_different_modes)

---

## 13. Preedit 오버레이 렌더링: 터미널별 비교

### 13.1 Alacritty 방식 (인라인 렌더링)

Alacritty는 v0.11.0부터 인라인 IME를 지원한다. PR #5790의 핵심:

- **preedit 커서 추적**: `cursor_start`(바이트 오프셋 시작), `cursor_end`(바이트 오프셋 끝) 두 위치를 관리
- **렌더링**: 커서 위치에 preedit 텍스트를 직접 그리되, 밑줄 스타일로 일반 텍스트와 구분
- **IME 이벤트 처리**: `IME::Enabled`, `Preedit`, `Commit` 세 가지 이벤트 유형 처리
- **플랫폼 제약**: X11에서는 libX11의 한계로 off-the-spot(창 하단) preedit만 가능. macOS/Wayland에서는 on-the-spot(인라인) 지원

```
Alacritty 렌더링 흐름:
  Preedit 이벤트 → 커서 위치 확인 →
  해당 셀 위에 오버레이 렌더링 (밑줄 + Beam 커서)
```

### 13.2 WezTerm 방식 (하이브리드)

WezTerm은 `ime_preedit_rendering` 설정으로 두 가지 모드를 제공:

**Builtin 모드 (기본)**:
- 터미널 자체 폰트로 커서 위치에 직접 렌더링
- `window:composition_status()`와 연동 가능
- **한계**: 긴 preedit이 창 끝에서 잘림, 줄바꿈 불가
- **macOS에서**: 항상 이 모드 사용 (설정 무시)

**System 모드**:
- OS의 IME 창이 렌더링을 담당
- 잘림 문제 없음, 하지만 터미널 폰트와 시각적 불일치

### 13.3 Ghostty 방식 (네이티브 AppKit)

Ghostty는 macOS에서 Swift/AppKit의 네이티브 IME 시스템을 직접 활용:
- `SurfaceView_AppKit.swift`에서 `NSTextInputClient` 프로토콜을 직접 구현
- 네이티브 마크된 텍스트 렌더링 활용
- `flagsChanged`에서 `hasMarkedText()` 체크로 preedit 보호

### 13.4 Zed GPUI 방식 (추상화 기반)

Zed는 GPUI의 `InputHandler` 추상화를 통해 플랫폼 독립적으로 처리:
- `replace_and_mark_text_in_range()` → 에디터 버퍼에 marked 텍스트 삽입
- 에디터가 직접 밑줄 스타일로 렌더링
- `bounds_for_range()` → IME 후보 창 위치 제공

### 13.5 Crux 권장: 네이티브 + 커스텀 하이브리드

```rust
/// Crux의 preedit 렌더링 엔진
struct PreeditRenderer {
    mode: PreeditRenderMode,
}

enum PreeditRenderMode {
    /// 터미널 셀 위에 직접 렌더링 (Alacritty/WezTerm Builtin 스타일)
    /// 장점: 일관된 폰트, 색상 제어 가능
    /// 단점: 긴 preedit 처리 복잡
    Inline {
        /// 밑줄 스타일
        underline_style: UnderlineStyle,
        /// preedit 텍스트 색상
        foreground: Color,
        /// 배경 하이라이트
        background: Option<Color>,
    },

    /// macOS 네이티브 IME 창에 위임
    /// 장점: 구현 단순, OS와 일관
    /// 단점: 폰트 불일치
    System,
}

impl PreeditRenderer {
    /// 셀 그리드 위에 preedit 오버레이 렌더링
    fn render(
        &self,
        ctx: &mut RenderContext,
        preedit: &PreeditState,
        grid: &Grid,
        cell_size: Size,
    ) {
        match self.mode {
            PreeditRenderMode::Inline { ref underline_style, foreground, background } => {
                let cursor = grid.cursor_position();
                let start_x = cursor.col as f32 * cell_size.width;
                let y = cursor.row as f32 * cell_size.height;

                let mut x_offset = 0.0;
                for (i, grapheme) in preedit.text.graphemes(true).enumerate() {
                    let width = grapheme_display_width(grapheme) as f32;
                    let cell_width = width * cell_size.width;

                    // 배경 하이라이트
                    if let Some(bg) = background {
                        ctx.fill_rect(
                            Rect::new(start_x + x_offset, y, cell_width, cell_size.height),
                            bg,
                        );
                    }

                    // 텍스트 렌더링
                    ctx.draw_text(
                        grapheme,
                        Point::new(start_x + x_offset, y),
                        foreground,
                    );

                    // 밑줄
                    let underline_y = y + cell_size.height - 1.0;
                    ctx.draw_line(
                        Point::new(start_x + x_offset, underline_y),
                        Point::new(start_x + x_offset + cell_width, underline_y),
                        foreground,
                        1.0,
                    );

                    x_offset += cell_width;
                }

                // preedit 커서 (Beam 스타일)
                let cursor_x = start_x + preedit.cursor_byte_offset_to_pixels(cell_size);
                ctx.draw_line(
                    Point::new(cursor_x, y),
                    Point::new(cursor_x, y + cell_size.height),
                    foreground,
                    2.0,
                );
            }
            PreeditRenderMode::System => {
                // firstRectForCharacterRange만 정확히 반환하면 OS가 처리
            }
        }
    }
}
```

**참고**: [Alacritty PR #5790](https://github.com/alacritty/alacritty/pull/5790), [Alacritty PR #7883](https://github.com/alacritty/alacritty/pull/7883), [WezTerm ime_preedit_rendering](https://wezterm.org/config/lua/config/ime_preedit_rendering.html)
