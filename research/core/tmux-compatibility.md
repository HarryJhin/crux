---
title: "tmux 호환성 구현 가이드"
description: "tmux가 요구하는 VT 기능 테스트 매트릭스, 마우스 모드, 브래킷 붙여넣기, 포커스 이벤트, DECLRMM, 제어 모드, terminal-features 설정, Ghostty 참고사항"
date: 2026-02-12
phase: [5]
topics: [tmux, compatibility, vt100, mouse, control-mode, DECLRMM, terminal-features]
status: final
related:
  - terminal-emulation.md
  - keymapping.md
  - terminfo.md
  - ../integration/ipc-protocol-design.md
---

# tmux 호환성 구현 가이드

> 작성일: 2026-02-12
> 목적: Crux 터미널 에뮬레이터가 tmux와 완전히 호환되기 위해 구현해야 할 VT 기능, 프로토콜, 설정을 체계적으로 정리
> 참고: [tmux wiki](https://github.com/tmux/tmux/wiki), [Ghostty VT docs](https://ghostty.org/docs/vt), [xterm ctlseqs](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)

---

## 목차

1. [tmux 필수 VT 기능 테스트 매트릭스](#1-tmux-필수-vt-기능-테스트-매트릭스)
2. [마우스 모드 호환성](#2-마우스-모드-호환성)
3. [브래킷 붙여넣기](#3-브래킷-붙여넣기-bracketed-paste)
4. [포커스 이벤트](#4-포커스-이벤트-focus-events)
5. [DECLRMM (좌우 마진)](#5-declrmm-좌우-마진)
6. [tmux 제어 모드](#6-tmux-제어-모드-control-mode)
7. [terminal-features 설정](#7-terminal-features-설정)
8. [Ghostty의 tmux 호환성](#8-ghostty의-tmux-호환성)
9. [Crux 구현 권장사항](#9-crux-구현-권장사항)

---

## 1. tmux 필수 VT 기능 테스트 매트릭스

tmux는 내부적으로 VT 에뮬레이터를 실행하며, 외부 터미널(Crux)에도 다양한 VT 시퀀스를 출력한다. 아래는 tmux가 정상 동작하기 위해 Crux가 반드시 지원해야 하는 기능 목록이다.

### 1.1 커서 이동 (Cursor Movement)

| 시퀀스 | 이름 | 설명 | 우선순위 |
|--------|------|------|----------|
| `CSI A` | CUU | 커서 위로 이동 | 필수 |
| `CSI B` | CUD | 커서 아래로 이동 | 필수 |
| `CSI C` | CUF | 커서 오른쪽으로 이동 | 필수 |
| `CSI D` | CUB | 커서 왼쪽으로 이동 | 필수 |
| `CSI H` | CUP | 커서 절대 위치 이동 | 필수 |
| `CSI G` | CHA | 커서 열 절대 이동 | 필수 |
| `CSI d` | VPA | 커서 행 절대 이동 | 필수 |
| `CSI f` | HVP | CUP과 동일 (레거시) | 필수 |
| `ESC 7` | DECSC | 커서 저장 | 필수 |
| `ESC 8` | DECRC | 커서 복원 | 필수 |
| `CSI s` | ANSISYSSC | 커서 저장 (ANSI) | 필수* |
| `CSI u` | ANSISYSRC | 커서 복원 (ANSI) | 필수* |

> *`CSI s`는 DECLRMM 모드 69가 활성화되면 DECSLRM으로 동작한다 (5절 참조).

### 1.2 화면 지우기 (Erase)

| 시퀀스 | 이름 | 파라미터 | 설명 |
|--------|------|----------|------|
| `CSI J` | ED | 0: 커서→끝, 1: 시작→커서, 2: 전체, 3: 스크롤백 포함 | 화면 지우기 |
| `CSI K` | EL | 0: 커서→끝, 1: 시작→커서, 2: 전체 행 | 행 지우기 |
| `CSI ? J` | DECSED | ED와 동일 (보호 속성 무시) | 선택적 지우기 |
| `CSI ? K` | DECSEL | EL과 동일 (보호 속성 무시) | 선택적 행 지우기 |

### 1.3 스크롤 영역 (Scroll Regions)

| 시퀀스 | 이름 | 설명 | 우선순위 |
|--------|------|------|----------|
| `CSI Pt;Pb r` | DECSTBM | 상하 스크롤 영역 설정 | **최우선** |
| `CSI S` | SU | 위로 스크롤 (n줄) | 필수 |
| `CSI T` | SD | 아래로 스크롤 (n줄) | 필수 |
| `ESC D` | IND | 인덱스 (커서 아래 + 스크롤) | 필수 |
| `ESC M` | RI | 역인덱스 (커서 위 + 스크롤) | 필수 |

> **DECSTBM은 tmux의 핵심 기능이다.** tmux의 모든 분할 창(pane)은 독립적인 스크롤 영역을 사용한다. DECSTBM이 올바르게 구현되지 않으면 tmux는 전혀 동작하지 않는다.

### 1.4 문자 속성 (SGR — Select Graphic Rendition)

| 파라미터 | 설명 | 우선순위 |
|----------|------|----------|
| 0 | 모든 속성 리셋 | 필수 |
| 1 | 볼드 | 필수 |
| 2 | 흐리게 (dim/faint) | 필수 |
| 3 | 이탤릭 | 필수 |
| 4 | 밑줄 | 필수 |
| 4:0-5 | 밑줄 스타일 (none/single/double/curly/dotted/dashed) | 권장 |
| 5 | 깜빡임 (blink) | 선택 |
| 7 | 반전 (reverse) | 필수 |
| 8 | 숨김 (hidden) | 필수 |
| 9 | 취소선 (strikethrough) | 권장 |
| 21 | 이중 밑줄 | 권장 |
| 22 | 볼드/흐리게 해제 | 필수 |
| 23 | 이탤릭 해제 | 필수 |
| 24 | 밑줄 해제 | 필수 |
| 25 | 깜빡임 해제 | 필수 |
| 27 | 반전 해제 | 필수 |
| 28 | 숨김 해제 | 필수 |
| 29 | 취소선 해제 | 권장 |
| 30-37 | 전경색 (기본 8색) | 필수 |
| 38;5;n | 전경색 (256색) | 필수 |
| 38;2;r;g;b | 전경색 (24비트 트루컬러) | 필수 |
| 39 | 기본 전경색 | 필수 |
| 40-47 | 배경색 (기본 8색) | 필수 |
| 48;5;n | 배경색 (256색) | 필수 |
| 48;2;r;g;b | 배경색 (24비트 트루컬러) | 필수 |
| 49 | 기본 배경색 | 필수 |
| 53 | 오버라인 (overline) | 권장 |
| 55 | 오버라인 해제 | 권장 |
| 58;2;r;g;b | 밑줄 색상 (트루컬러) | 권장 |
| 58;5;n | 밑줄 색상 (256색) | 권장 |
| 59 | 기본 밑줄 색상 | 권장 |
| 90-97 | 밝은 전경색 | 필수 |
| 100-107 | 밝은 배경색 | 필수 |

### 1.5 행/문자 삽입 삭제

| 시퀀스 | 이름 | 설명 | 우선순위 |
|--------|------|------|----------|
| `CSI L` | IL | n줄 삽입 (현재 위치에서 아래로 밀기) | 필수 |
| `CSI M` | DL | n줄 삭제 (아래에서 올라오기) | 필수 |
| `CSI @` | ICH | n개 문자 삽입 (오른쪽으로 밀기) | 필수 |
| `CSI P` | DCH | n개 문자 삭제 (왼쪽으로 당기기) | 필수 |
| `CSI X` | ECH | n개 문자 지우기 (이동 없음) | 필수 |

### 1.6 탭 스톱 (Tab Stops)

| 시퀀스 | 이름 | 설명 | 우선순위 |
|--------|------|------|----------|
| `ESC H` | HTS | 현재 열에 탭 스톱 설정 | 필수 |
| `CSI g` | TBC | 0: 현재 열 탭 지우기, 3: 모든 탭 지우기 | 필수 |
| `0x09` | HT | 다음 탭 스톱으로 이동 | 필수 |
| `CSI Z` | CBT | 이전 탭 스톱으로 이동 (backtab) | 필수 |

### 1.7 문자 세트 (Character Sets)

| 시퀀스 | 설명 | 우선순위 |
|--------|------|----------|
| `ESC ( 0` | G0를 DEC Special Graphics로 설정 | 필수 |
| `ESC ( B` | G0를 US-ASCII로 설정 | 필수 |
| `ESC ) 0` | G1을 DEC Special Graphics로 설정 | 필수 |
| `ESC ) B` | G1을 US-ASCII로 설정 | 필수 |
| `0x0E` (SO) | G1 활성화 | 필수 |
| `0x0F` (SI) | G0 활성화 | 필수 |

> **DEC Special Graphics**는 tmux의 창 테두리(border)를 그리는 데 사용된다. 이것이 없으면 tmux 분할선이 `q`, `x`, `l` 등 알파벳 문자로 표시된다.

### 1.8 창 조작 (Window Manipulation)

| 시퀀스 | 파라미터 | 설명 | 우선순위 |
|--------|----------|------|----------|
| `CSI t` | 8;rows;cols | 창 크기 변경 | 선택 |
| `CSI t` | 14 | 텍스트 영역 크기 조회 (픽셀) | 권장 |
| `CSI t` | 18 | 텍스트 영역 크기 조회 (문자) | 권장 |
| `CSI t` | 22;0/1/2 | 창 제목/아이콘 스택에 push | 선택 |
| `CSI t` | 23;0/1/2 | 창 제목/아이콘 스택에서 pop | 선택 |

### 1.9 DEC Private 모드 (tmux가 사용하는 것)

| 모드 | 시퀀스 | 설명 | 우선순위 |
|------|--------|------|----------|
| DECCKM | `CSI ? 1 h/l` | 커서 키 Application/Normal 모드 | 필수 |
| DECAWM | `CSI ? 7 h/l` | 자동 줄 바꿈 모드 | 필수 |
| DECTCEM | `CSI ? 25 h/l` | 커서 표시/숨김 | 필수 |
| Alt Screen | `CSI ? 1049 h/l` | 대체 화면 버퍼 전환 | 필수 |
| Alt Screen (47) | `CSI ? 47 h/l` | 레거시 대체 화면 | 필수 |
| Alt Screen (1047) | `CSI ? 1047 h/l` | xterm 대체 화면 | 필수 |
| Save Cursor (1048) | `CSI ? 1048 h/l` | 커서 저장/복원 (1049와 함께) | 필수 |
| Origin Mode | `CSI ? 6 h/l` | 커서 원점 모드 (스크롤 영역 기준) | 필수 |
| DECLRMM | `CSI ? 69 h/l` | 좌우 마진 모드 | 권장 |

### 1.10 기타 필수 시퀀스

| 시퀀스 | 설명 | 우선순위 |
|--------|------|----------|
| `CSI n` | DSR (Device Status Report) — 6: 커서 위치 보고 | 필수 |
| `CSI c` | DA1 (Primary Device Attributes) | 필수 |
| `CSI > c` | DA2 (Secondary Device Attributes) | 필수 |
| `OSC 0;title ST` | 창 제목 설정 | 필수 |
| `OSC 2;title ST` | 창 제목 설정 | 필수 |
| `CSI ! p` | DECSTR (Soft Terminal Reset) | 필수 |
| `ESC c` | RIS (Full Reset) | 필수 |

---

## 2. 마우스 모드 호환성

tmux는 `set -g mouse on` 설정 시 마우스 이벤트를 처리한다. 이때 외부 터미널(Crux)에 마우스 모드 활성화/비활성화 시퀀스를 전송한다.

### 2.1 마우스 추적 모드

| 모드 | 시퀀스 | 설명 | tmux 사용 여부 |
|------|--------|------|---------------|
| 1000 | `CSI ? 1000 h/l` | Normal tracking — 버튼 클릭/릴리스 보고 | 사용 |
| 1001 | `CSI ? 1001 h/l` | Highlight tracking (레거시) | 미사용 |
| 1002 | `CSI ? 1002 h/l` | Button-event tracking — 버튼 누른 상태의 이동 보고 | 사용 |
| 1003 | `CSI ? 1003 h/l` | Any-event tracking — 모든 마우스 이동 보고 | 사용 |

### 2.2 마우스 인코딩 형식

| 모드 | 시퀀스 | 형식 | tmux 사용 여부 |
|------|--------|------|---------------|
| X10 호환 | (기본) | `CSI M Cb Cx Cy` (바이트 + 32 인코딩) | 레거시 |
| UTF-8 | `CSI ? 1005 h/l` | UTF-8 확장 좌표 | 미사용 |
| **SGR** | `CSI ? 1006 h/l` | `CSI < Cb;Cx;Cy M/m` | **필수** |
| URXVT | `CSI ? 1015 h/l` | `CSI Cb;Cx;Cy M` | 미사용 |
| SGR-Pixel | `CSI ? 1016 h/l` | SGR과 동일하나 픽셀 좌표 | 선택 |

> **SGR 확장 모드(1006)는 현대 tmux에서 필수이다.** X10 호환 모드는 223열/행 제한이 있어 큰 터미널에서 동작하지 않는다. tmux는 SGR 모드를 우선 협상한다.

### 2.3 SGR 마우스 이벤트 형식

```
CSI < button;column;row M    (버튼 누름)
CSI < button;column;row m    (버튼 릴리스)
```

**button 비트 구성:**
- 비트 0-1: 버튼 번호 (0=왼쪽, 1=가운데, 2=오른쪽)
- 비트 2: Shift
- 비트 3: Meta/Alt
- 비트 4: Ctrl
- 비트 5-6: 이동(32) 또는 휠(64)

| 이벤트 | button 값 | 설명 |
|--------|-----------|------|
| 왼쪽 클릭 | 0 | `CSI < 0;col;row M` |
| 가운데 클릭 | 1 | `CSI < 1;col;row M` |
| 오른쪽 클릭 | 2 | `CSI < 2;col;row M` |
| 왼쪽 릴리스 | 0 | `CSI < 0;col;row m` |
| 휠 위 | 64 | `CSI < 64;col;row M` |
| 휠 아래 | 65 | `CSI < 65;col;row M` |
| 이동 (왼쪽 누름) | 32 | `CSI < 32;col;row M` |
| Ctrl+왼쪽 클릭 | 16 | `CSI < 16;col;row M` |

### 2.4 tmux 마우스 모드 협상

tmux가 마우스를 활성화할 때의 시퀀스:

```
CSI ? 1006 h    (SGR 확장 모드 활성화)
CSI ? 1002 h    (Button-event tracking 활성화)
```

비활성화 시:
```
CSI ? 1002 l    (Button-event tracking 비활성화)
CSI ? 1006 l    (SGR 확장 모드 비활성화)
```

> tmux 내부 애플리케이션(vim 등)이 1003 모드를 요청하면 tmux가 이를 외부 터미널로 전달한다.

---

## 3. 브래킷 붙여넣기 (Bracketed Paste)

### 3.1 프로토콜

| 동작 | 시퀀스 | 설명 |
|------|--------|------|
| 활성화 | `CSI ? 2004 h` | 브래킷 붙여넣기 모드 ON |
| 비활성화 | `CSI ? 2004 l` | 브래킷 붙여넣기 모드 OFF |
| 붙여넣기 시작 | `ESC [ 200 ~` | 터미널 → 애플리케이션 |
| 붙여넣기 종료 | `ESC [ 201 ~` | 터미널 → 애플리케이션 |

### 3.2 동작 원리

1. 애플리케이션(tmux/vim 등)이 `CSI ? 2004 h`로 모드 활성화
2. 사용자가 클립보드에서 붙여넣기 (Cmd+V)
3. 터미널이 클립보드 내용을 `ESC [ 200 ~` ... `ESC [ 201 ~`로 감싸서 전송
4. 애플리케이션은 감싸진 텍스트를 "붙여넣기된 것"으로 인식하여 자동 들여쓰기, 명령 실행 등을 방지

### 3.3 tmux 상호작용

- tmux는 외부 터미널에 `CSI ? 2004 h`를 전송하여 브래킷 모드를 요청
- tmux 내부 애플리케이션이 브래킷 모드를 요청하면 tmux가 외부로 전달
- `tmux paste-buffer`는 브래킷 붙여넣기를 사용하지 않음 (직접 입력으로 처리)
- `set -g set-clipboard on` 설정 시 OSC 52로 클립보드 동기화 가능

### 3.4 Crux 구현 요구사항

```rust
// 브래킷 붙여넣기 구현 의사코드
fn handle_paste(&mut self, text: &str) {
    if self.terminal_mode.contains(Mode::BRACKETED_PASTE) {
        self.pty_write(b"\x1b[200~");
        self.pty_write(text.as_bytes());
        self.pty_write(b"\x1b[201~");
    } else {
        // 개행 문자를 CR로 변환하여 전송
        let sanitized = text.replace('\n', "\r");
        self.pty_write(sanitized.as_bytes());
    }
}
```

---

## 4. 포커스 이벤트 (Focus Events)

### 4.1 프로토콜

| 동작 | 시퀀스 | 설명 |
|------|--------|------|
| 활성화 | `CSI ? 1004 h` | 포커스 이벤트 보고 모드 ON |
| 비활성화 | `CSI ? 1004 l` | 포커스 이벤트 보고 모드 OFF |
| 포커스 획득 | `CSI I` | 터미널 → 애플리케이션 |
| 포커스 상실 | `CSI O` | 터미널 → 애플리케이션 |

### 4.2 tmux 설정

```bash
# tmux.conf에서 포커스 이벤트 전달 활성화
set -g focus-events on
```

이 옵션이 켜지면:
1. tmux가 외부 터미널에 `CSI ? 1004 h`를 전송
2. 외부 터미널 창이 포커스를 얻거나 잃을 때 `CSI I`/`CSI O`를 tmux에 전송
3. tmux가 이를 내부의 활성 창(pane)에 전달

### 4.3 Crux 구현 요구사항

```rust
// GPUI의 WindowFocusEvent를 포커스 이벤트로 변환
fn handle_focus_change(&mut self, focused: bool) {
    if self.terminal_mode.contains(Mode::FOCUS_EVENTS) {
        if focused {
            self.pty_write(b"\x1b[I");
        } else {
            self.pty_write(b"\x1b[O");
        }
    }
}
```

---

## 5. DECLRMM (좌우 마진)

### 5.1 개요

DECLRMM(Left-Right Margin Mode)은 수직 스크롤 영역(DECSTBM)의 수평 버전이다. tmux는 수평 분할(horizontal splits)에서 좌우 마진을 사용하여 각 pane의 수평 범위를 제한한다.

### 5.2 관련 시퀀스

| 시퀀스 | 이름 | 설명 |
|--------|------|------|
| `CSI ? 69 h` | DECLRMM 활성화 | 좌우 마진 모드 ON |
| `CSI ? 69 l` | DECLRMM 비활성화 | 좌우 마진 모드 OFF (마진 리셋) |
| `CSI Pl;Pr s` | DECSLRM | 좌우 마진 설정 (DECLRMM ON일 때만) |

### 5.3 CSI s 충돌 문제

`CSI s`는 두 가지 의미를 가진다:
- **DECLRMM OFF**: ANSISYSSC (커서 저장) — `CSI s` = 커서 위치 저장
- **DECLRMM ON**: DECSLRM (좌우 마진 설정) — `CSI Pl;Pr s` = 마진 설정

**Ghostty의 해결 방식:**
- 모드 69가 **비활성화** 상태: `CSI s` → 커서 저장 (ANSISYSSC)
- 모드 69가 **활성화** 상태: `CSI s` → 좌우 마진 설정 (DECSLRM)
- 커서 저장은 항상 `ESC 7` (DECSC)로도 사용 가능

### 5.4 tmux의 DECLRMM 사용

tmux는 수평으로 분할된 pane을 렌더링할 때 DECLRMM을 사용한다:

```
# tmux가 외부 터미널에 전송하는 시퀀스 예시 (2개 수평 pane, 80열)
CSI ? 69 h          # DECLRMM 활성화
CSI 1;40 s          # 왼쪽 pane: 1~40열
CSI 1;24 r          # 상하 마진: 1~24행
[왼쪽 pane 내용 출력]
CSI 42;80 s         # 오른쪽 pane: 42~80열
CSI 1;24 r          # 상하 마진: 1~24행
[오른쪽 pane 내용 출력]
CSI ? 69 l          # DECLRMM 비활성화 (마진 리셋)
```

### 5.5 터미널 지원 현황

| 터미널 | DECLRMM 지원 | 비고 |
|--------|-------------|------|
| xterm | 지원 | 원조 구현 |
| Ghostty | 지원 | CSI s 충돌을 모드 기반으로 해결 |
| Kitty | 지원 | |
| WezTerm | 지원 | |
| Alacritty | 미지원 (2025 기준) | tmux 수평 분할 시 렌더링 문제 |
| iTerm2 | 지원 | |
| Terminal.app | 미지원 | |

> **Crux는 DECLRMM을 지원해야 한다.** DECLRMM 없이도 tmux는 동작하지만 (fallback으로 전체 화면 다시 그리기), 성능이 크게 저하된다. 특히 수평 분할 시 깜빡임이 발생할 수 있다.

### 5.6 Crux 구현 참고

```rust
// DECLRMM 상태에 따른 CSI s 해석
fn handle_csi_s(&mut self, params: &[i64]) {
    if self.mode.contains(Mode::DECLRMM) {
        // DECSLRM: 좌우 마진 설정
        let left = params.get(0).copied().unwrap_or(1) as usize;
        let right = params.get(1).copied().unwrap_or(self.cols) as usize;
        self.set_left_right_margins(left, right);
    } else {
        // ANSISYSSC: 커서 저장
        self.save_cursor();
    }
}
```

---

## 6. tmux 제어 모드 (Control Mode)

### 6.1 개요

tmux 제어 모드는 터미널 에뮬레이터가 tmux를 프로그래밍 방식으로 제어할 수 있는 텍스트 기반 프로토콜이다. iTerm2가 이 모드를 사용하여 tmux 세션을 네이티브 탭/분할로 표시한다.

### 6.2 제어 모드 진입

```bash
# 단일 -C: 정상적인 터미널 동작 유지 (테스트용)
tmux -C new-session
tmux -C attach-session -t mysession

# 이중 -CC: 캐노니컬 모드 비활성화 (애플리케이션용)
tmux -CC new-session
tmux -CC attach-session -t mysession
```

`-CC` 모드에서:
- 연결 시 DCS 시퀀스 전송: `\033P1000p` (7바이트)
- 종료 시 `%exit`와 ST(`\033\`) 전송
- 빈 줄(Enter만) 입력 시 클라이언트 분리

### 6.3 명령 응답 형식

모든 명령은 `%begin`/`%end`(또는 `%error`) 블록으로 응답한다:

```
%begin <timestamp> <command_number> <flags>
[명령 출력]
%end <timestamp> <command_number> <flags>
```

실패 시:
```
%begin <timestamp> <command_number> <flags>
[에러 메시지]
%error <timestamp> <command_number> <flags>
```

**필드:**
- `timestamp`: epoch 초
- `command_number`: 고유 명령 식별자 (begin/end/error 매칭용)
- `flags`: 현재 항상 `1`

### 6.4 Pane 출력 알림

```
%output %<pane_id> <escaped_text>
```

- `pane_id`: `%0`, `%1`, ... 형식의 pane 식별자
- `escaped_text`: ASCII 32 미만 문자와 백슬래시는 8진수로 이스케이프 (`\134` = `\`)

### 6.5 알림 메시지 전체 목록

| 알림 | 형식 | 설명 |
|------|------|------|
| `%output` | `%output %pane data` | Pane 출력 |
| `%window-add` | `%window-add @window` | 창 추가됨 |
| `%window-close` | `%window-close @window` | 창 닫힘 |
| `%window-renamed` | `%window-renamed @window name` | 창 이름 변경 |
| `%window-pane-changed` | `%window-pane-changed @window %pane` | 활성 pane 변경 |
| `%session-changed` | `%session-changed $session name` | 세션 변경 |
| `%session-renamed` | `%session-renamed $session name` | 세션 이름 변경 |
| `%session-window-changed` | `%session-window-changed $session @window` | 세션의 현재 창 변경 |
| `%sessions-changed` | `%sessions-changed` | 세션 생성/삭제 |
| `%pane-mode-changed` | `%pane-mode-changed %pane` | Pane 모드 변경 |
| `%client-session-changed` | `%client-session-changed client $session name` | 다른 클라이언트 세션 변경 |
| `%unlinked-window-add` | `%unlinked-window-add @window` | 다른 세션에서 창 추가 |
| `%unlinked-window-close` | `%unlinked-window-close @window` | 다른 세션에서 창 닫힘 |
| `%unlinked-window-renamed` | `%unlinked-window-renamed @window name` | 다른 세션에서 창 이름 변경 |
| `%pause` | `%pause %pane` | Pane 일시정지 (흐름 제어) |
| `%continue` | `%continue %pane` | Pane 재개 (흐름 제어) |
| `%extended-output` | `%extended-output %pane ms : data` | 확장 출력 (흐름 제어 시) |

### 6.6 흐름 제어 (Flow Control)

```bash
# 흐름 제어 활성화: pane이 지정된 시간 이상 뒤처지면 일시정지
refresh-client -f pause-after=1

# Pane별 제어
refresh-client -A '%0:continue'   # pane 0 재개
refresh-client -A '%0:pause'      # pane 0 일시정지
refresh-client -A '%0:off'        # pane 0 출력 비활성화
```

### 6.7 크기 협상

```bash
# 제어 모드 클라이언트의 크기 설정
refresh-client -C 120x40
```

이 명령 없이는 제어 모드 클라이언트가 다른 클라이언트의 창 크기에 영향을 주지 않는다.

### 6.8 Crux에서의 활용 가능성

Crux는 Phase 5에서 tmux 제어 모드를 활용하여:
1. tmux 세션을 네이티브 GPUI 탭/분할로 표시
2. tmux pane 출력을 직접 터미널 뷰로 렌더링
3. tmux 키 바인딩을 GPUI 액션으로 매핑
4. 세션 reconnection/detach를 투명하게 처리

이는 iTerm2의 tmux 통합과 유사한 사용자 경험을 제공한다.

---

## 7. terminal-features 설정

### 7.1 개요

tmux 3.2+에서 도입된 `terminal-features` 옵션은 터미널의 기능을 선언적으로 설정한다. 이전 버전의 `terminal-overrides`를 대체한다.

### 7.2 설정 형식

```bash
# 기본 형식
set -as terminal-features ',<terminal-pattern>:<feature1>:<feature2>:...'

# 예시: Crux 터미널 설정
set -as terminal-features ',xterm-crux:RGB:usstyle:sync:extkeys:overline:strikethrough:margins:rectfill'
```

### 7.3 지원 기능 플래그 전체 목록

| 플래그 | 설명 | 관련 시퀀스 |
|--------|------|------------|
| `256` | 256색 지원 | SGR 38;5;n / 48;5;n |
| `RGB` | 24비트 트루컬러 | SGR 38;2;r;g;b / 48;2;r;g;b |
| `usstyle` | 밑줄 스타일 및 색상 | SGR 4:x (curly 등) + SGR 58;2;r;g;b |
| `overline` | 오버라인 속성 | SGR 53 / 55 |
| `strikethrough` | 취소선 속성 | SGR 9 / 29 |
| `sync` | 동기화된 업데이트 | `CSI ? 2026 h/l` 또는 BSU/ESU |
| `extkeys` | 확장 키 (CSI u / Kitty) | `CSI > flags u` |
| `margins` | DECSLRM 좌우 마진 | `CSI ? 69 h` + `CSI Pl;Pr s` |
| `rectfill` | DECFRA 직사각형 채우기 | `CSI Pc;Pt;Pl;Pb;Pr $ x` |
| `title` | OSC 창 제목 설정 | `OSC 0/2;title ST` |
| `clipboard` | OSC 52 클립보드 | `OSC 52;c;base64 ST` |
| `ccolour` | 커서 색상 설정 | `OSC 12;color ST` |
| `cstyle` | 커서 스타일 설정 | `CSI q` (DECSCUSR) |
| `hyperlinks` | 하이퍼링크 | `OSC 8;params;uri ST` |

### 7.4 terminal-overrides (레거시)

tmux 3.2 이전 버전이나 추가적인 terminfo 재정의가 필요할 때 사용:

```bash
# 트루컬러 (Tc 플래그)
set -as terminal-overrides ",xterm-crux:Tc"

# 밑줄 스타일 (Smulx, Setulc)
set -as terminal-overrides ',xterm-crux:Smulx=\E[4::%p1%dm'
set -as terminal-overrides ',xterm-crux:Setulc=\E[58::2::%p1%{65536}%/%d::%p1%{256}%/%{255}%&%d::%p1%{255}%&%d%;m'

# 동기화된 업데이트
set -as terminal-overrides ',xterm-crux:Sync=\E[?2026h:\E[?2026l'
```

### 7.5 Crux의 terminfo와 연동

Crux의 `xterm-crux` terminfo 엔트리에 이 기능들을 올바르게 선언하면, tmux가 자동으로 감지하여 `terminal-features` 수동 설정 없이도 동작한다.

```bash
# terminfo 확인
tmux display -p '#{client_termfeatures}'
# 예상 출력: 256,RGB,clipboard,cstyle,extkeys,margins,overline,rectfill,strikethrough,sync,title,usstyle
```

### 7.6 확장 키 (extkeys) 설정

tmux에서 Kitty 키보드 프로토콜 또는 xterm CSI u를 사용하려면:

```bash
# tmux.conf
set -s extended-keys on
set -s extended-keys-format csi-u
set -as terminal-features 'xterm-crux:extkeys'
```

---

## 8. Ghostty의 tmux 호환성

### 8.1 알려진 이슈와 해결책

**1. terminfo 미설치 문제 (SSH 환경)**

원격 서버에 `xterm-ghostty` terminfo가 없으면 `missing or unsuitable terminal` 에러가 발생한다.

해결책:
```bash
# 방법 1: infocmp로 원격 설치
infocmp -x xterm-ghostty | ssh remote 'tic -x -'

# 방법 2: TERM 변경
export TERM=xterm-256color

# 방법 3: Ghostty의 SSH 통합 (개발 중)
# shell-integration-features = ssh-env,ssh-terminfo
```

**2. 확장 키 설정**

```bash
# Ghostty용 tmux.conf
set -s extended-keys on
set -s extended-keys-format csi-u
set -as terminal-features 'xterm-ghostty:extkeys'
```

**3. DECLRMM 지원**

Ghostty는 DECLRMM을 지원하므로 tmux 수평 분할이 정상 동작한다. 모드 69에 따라 `CSI s`의 의미를 동적으로 전환한다.

**4. Kitty 키보드 프로토콜 관련 주의사항**

- Ghostty는 기본적으로 fixterms 인코딩을 사용하며, Kitty 프로토콜은 애플리케이션이 명시적으로 활성화해야 한다
- macOS에서 `Cmd+Backspace`가 `\x15`(Ctrl-U)로 전송되는 이슈가 있었음
- 일부 기본 키 바인딩(Alt+Left/Right)이 Kitty 프로토콜 시퀀스 대신 레거시 시퀀스(`ESC b`/`ESC f`)를 전송

### 8.2 Ghostty의 tmux 추천 설정

```bash
# Ghostty 사용자를 위한 tmux.conf
set -g default-terminal "tmux-256color"
set -as terminal-features ',xterm-ghostty:RGB:usstyle:sync:extkeys:margins'
set -g focus-events on
set -g mouse on
set -s extended-keys on
set -s extended-keys-format csi-u
```

### 8.3 Crux가 배울 점

1. **terminfo를 올바르게 작성하면 대부분의 설정이 자동화된다** — `terminal-features` 수동 설정 불필요
2. **DECLRMM 지원은 tmux 성능에 큰 영향** — 수평 분할 시 화면 전체를 다시 그리지 않아도 됨
3. **확장 키(CSI u)는 opt-in** — 기본 동작은 레거시와 호환되어야 함
4. **SSH 환경에서 terminfo 배포 전략이 필요** — Ghostty의 `ssh-terminfo` 접근 참고

---

## 9. Crux 구현 권장사항

### 9.1 Phase 1 (MVP) — 기본 tmux 호환

Phase 1에서 반드시 구현해야 할 항목:
- [ ] DECSTBM (스크롤 영역) — tmux 동작의 핵심
- [ ] 모든 커서 이동 시퀀스 (CUU/CUD/CUF/CUB/CUP/CHA)
- [ ] 화면/행 지우기 (ED/EL 모든 파라미터)
- [ ] 행/문자 삽입 삭제 (IL/DL/ICH/DCH/ECH)
- [ ] SGR 기본 속성 (bold, italic, underline, reverse, 8/256/truecolor)
- [ ] DEC Special Graphics 문자 세트 (G0/G1)
- [ ] 대체 화면 버퍼 (1049/47/1047)
- [ ] DECCKM (커서 키 모드)
- [ ] 탭 스톱 (HTS/TBC/HT/CBT)
- [ ] DSR (커서 위치 보고)
- [ ] DA1/DA2 (디바이스 속성)

### 9.2 Phase 2 — 마우스 및 붙여넣기

- [ ] 마우스 모드 1000/1002/1003
- [ ] SGR 마우스 인코딩 (1006) — **필수**
- [ ] 브래킷 붙여넣기 (2004)
- [ ] 포커스 이벤트 (1004)
- [ ] OSC 52 클립보드

### 9.3 Phase 5 — 고급 tmux 호환

- [ ] DECLRMM (좌우 마진 모드 69)
- [ ] DECSLRM (좌우 마진 설정)
- [ ] 동기화된 업데이트 (CSI ? 2026)
- [ ] DECFRA (직사각형 채우기)
- [ ] 확장 키 (CSI u / Kitty 프로토콜)
- [ ] tmux 제어 모드 통합 (네이티브 탭/분할)

### 9.4 terminfo 엔트리 (`xterm-crux`)

```terminfo
# Crux가 지원해야 할 기능의 terminfo 선언 (요약)
xterm-crux|Crux terminal emulator,
    am, bce, ccc, hs, km, mc5i, mir, msgr, xenl,
    colors#0x1000000, cols#80, lines#24, pairs#0x10000,
    # 트루컬러
    setrgbf=\E[38;2;%p1%d;%p2%d;%p3%dm,
    setrgbb=\E[48;2;%p1%d;%p2%d;%p3%dm,
    # 밑줄 스타일
    Smulx=\E[4\:%p1%dm,
    Setulc=\E[58\:\:2\:\:%p1%{65536}%/%d\:\:%p1%{256}%/%{255}%&%d\:\:%p1%{255}%&%d%;m,
    # 동기화된 업데이트
    Sync=\E[?2026%?%p1%{1}%-%tl%eh,
    # 스크롤 영역
    csr=\E[%i%p1%d;%p2%dr,
    # DECSLRM 마진
    Cmg=\E[?69h,
    Clmg=\E[?69l,
    Smglr=\E[%i%p1%d;%p2%ds,
```

### 9.5 테스트 전략

```bash
# tmux 호환성 테스트 체크리스트
tmux new-session -d -s test

# 1. 기본 동작
tmux split-window -h          # 수평 분할 (DECLRMM 테스트)
tmux split-window -v          # 수직 분할 (DECSTBM 테스트)

# 2. 마우스
tmux set mouse on             # 마우스 이벤트 테스트

# 3. 색상
echo -e "\e[38;2;255;0;0mRed\e[0m"   # 트루컬러 테스트

# 4. 밑줄 스타일
echo -e "\e[4:3mCurly\e[0m"          # 웨이브 밑줄 테스트

# 5. 문자 세트 (테두리)
tmux display-panes                    # DEC Special Graphics 테스트

# 6. 스크롤
yes | head -100                       # 스크롤 영역 테스트
```

---

## 참고 자료

- [tmux wiki — Control Mode](https://github.com/tmux/tmux/wiki/Control-Mode) — 제어 모드 공식 문서
- [tmux wiki — FAQ](https://github.com/tmux/tmux/wiki/FAQ) — 자주 묻는 질문
- [tmux(1) man page](https://man7.org/linux/man-pages/man1/tmux.1.html) — 전체 매뉴얼
- [Ghostty DECSLRM docs](https://ghostty.org/docs/vt/csi/decslrm) — Ghostty의 DECLRMM 구현
- [xterm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html) — VT 시퀀스 레퍼런스
- [Ghostty tmux discussions](https://github.com/ghostty-org/ghostty/discussions/2839) — Ghostty tmux 제어 모드 논의
- [Ghostty tmux control mode issue](https://github.com/ghostty-org/ghostty/issues/1935) — Ghostty tmux 제어 모드 구현 이슈
