---
title: "키 입력 → 이스케이프 시퀀스 매핑"
description: "Escape sequence mapping tables, control characters, modifier encoding, Kitty keyboard protocol, macOS Option key handling"
date: 2026-02-12
phase: [1, 4]
topics: [keyboard, escape-sequence, kitty-protocol, modifier-keys, input]
status: final
related:
  - terminal-emulation.md
  - terminfo.md
  - ../platform/ime-clipboard.md
---

# 키 입력 → 이스케이프 시퀀스 매핑 리서치

> 작성일: 2026-02-12
> 목적: Crux 터미널 에뮬레이터에서 키보드 입력을 PTY에 전송할 이스케이프 시퀀스로 변환하기 위한 완전한 매핑 테이블 및 구현 패턴 정리
> 참고: [xterm ctlseqs](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html), [Kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/), [fixterms](http://www.leonerd.org.uk/hacks/fixterms/), [WezTerm key encoding](https://wezterm.org/config/key-encoding.html), [Alacritty source](https://github.com/alacritty/alacritty)

---

## 목차

1. [기본 개념](#1-기본-개념)
2. [일반 문자 입력](#2-일반-문자-입력)
3. [제어 문자 (Ctrl+키)](#3-제어-문자-ctrlkey)
4. [특수 키](#4-특수-키)
5. [커서 키 (화살표)](#5-커서-키-화살표)
6. [기능 키 (F1-F12)](#6-기능-키-f1-f12)
7. [편집/네비게이션 키](#7-편집네비게이션-키)
8. [수정자 키 인코딩](#8-수정자-키-인코딩)
9. [Application 모드 vs Normal 모드](#9-application-모드-vs-normal-모드)
10. [Kitty 키보드 프로토콜](#10-kitty-키보드-프로토콜)
11. [fixterms / CSI u 레거시](#11-fixterms--csi-u-레거시)
12. [macOS Option 키 처리](#12-macos-option-키-처리)
13. [완전한 매핑 테이블](#13-완전한-매핑-테이블)
14. [Rust 구현 패턴](#14-rust-구현-패턴)
15. [Crux 구현 권장사항](#15-crux-구현-권장사항)

---

## 1. 기본 개념

### 이스케이프 시퀀스 표기법

| 표기 | 의미 | 바이트 |
|------|------|--------|
| `ESC` | Escape 문자 | `0x1B` |
| `CSI` | Control Sequence Introducer | `0x1B 0x5B` (= `ESC [`) |
| `SS3` | Single Shift 3 | `0x1B 0x4F` (= `ESC O`) |
| `OSC` | Operating System Command | `0x1B 0x5D` (= `ESC ]`) |
| `ST` | String Terminator | `0x1B 0x5C` (= `ESC \`) |

### 시퀀스 형식

터미널 입력 이스케이프 시퀀스는 크게 3가지 형식을 사용한다:

1. **CSI letter 형식**: `CSI [param;modifier] letter` — 커서 키, F1-F4, Home, End
2. **CSI number ~ 형식**: `CSI number [;modifier] ~` — F5-F12, Insert, Delete, PgUp, PgDn
3. **SS3 letter 형식**: `SS3 letter` — Application 모드 커서 키, F1-F4 (수정자 없을 때)

---

## 2. 일반 문자 입력

### ASCII 출력 가능 문자 (0x20-0x7E)

수정자 없이 누른 일반 문자는 해당 ASCII/UTF-8 바이트를 그대로 PTY에 전송한다.

```
'a' → 0x61
'A' (Shift+a) → 0x41
'1' → 0x31
' ' (Space) → 0x20
```

### UTF-8 문자

ASCII 범위 밖의 문자(한글, 일본어, 이모지 등)는 UTF-8 인코딩된 바이트 시퀀스를 그대로 전송한다.

```
'가' → 0xEA 0xB0 0x80
'é'  → 0xC3 0xA9
```

---

## 3. 제어 문자 (Ctrl+키)

Ctrl 키와 영문자를 조합하면 C0 제어 코드(0x00-0x1F)가 생성된다. 계산 규칙: **해당 문자의 ASCII 코드에서 0x40(64)을 뺀다** (대문자 기준).

### Ctrl + 영문자 매핑

| 키 조합 | 전송 바이트 | 10진수 | 이름 |
|---------|------------|--------|------|
| Ctrl+@ | `0x00` | 0 | NUL |
| Ctrl+A | `0x01` | 1 | SOH |
| Ctrl+B | `0x02` | 2 | STX |
| Ctrl+C | `0x03` | 3 | ETX (인터럽트) |
| Ctrl+D | `0x04` | 4 | EOT (EOF) |
| Ctrl+E | `0x05` | 5 | ENQ |
| Ctrl+F | `0x06` | 6 | ACK |
| Ctrl+G | `0x07` | 7 | BEL (벨) |
| Ctrl+H | `0x08` | 8 | BS (백스페이스) |
| Ctrl+I | `0x09` | 9 | HT (= Tab) |
| Ctrl+J | `0x0A` | 10 | LF (줄바꿈) |
| Ctrl+K | `0x0B` | 11 | VT |
| Ctrl+L | `0x0C` | 12 | FF (화면 지우기) |
| Ctrl+M | `0x0D` | 13 | CR (= Enter) |
| Ctrl+N | `0x0E` | 14 | SO |
| Ctrl+O | `0x0F` | 15 | SI |
| Ctrl+P | `0x10` | 16 | DLE |
| Ctrl+Q | `0x11` | 17 | DC1 (XON) |
| Ctrl+R | `0x12` | 18 | DC2 |
| Ctrl+S | `0x13` | 19 | DC3 (XOFF) |
| Ctrl+T | `0x14` | 20 | DC4 |
| Ctrl+U | `0x15` | 21 | NAK |
| Ctrl+V | `0x16` | 22 | SYN |
| Ctrl+W | `0x17` | 23 | ETB |
| Ctrl+X | `0x18` | 24 | CAN |
| Ctrl+Y | `0x19` | 25 | EM |
| Ctrl+Z | `0x1A` | 26 | SUB (서스펜드) |
| Ctrl+[ | `0x1B` | 27 | ESC |
| Ctrl+\ | `0x1C` | 28 | FS (SIGQUIT) |
| Ctrl+] | `0x1D` | 29 | GS |
| Ctrl+^ | `0x1E` | 30 | RS |
| Ctrl+_ | `0x1F` | 31 | US |

### 충돌 문제

이것이 터미널 키 입력의 근본적 문제점이다:
- **Tab = Ctrl+I** (둘 다 0x09)
- **Enter = Ctrl+M** (둘 다 0x0D)
- **Backspace = Ctrl+H** (일부 터미널에서 0x08)
- **Escape = Ctrl+[** (둘 다 0x1B)

이 문제는 Kitty 프로토콜과 CSI u로 해결된다 (후술).

---

## 4. 특수 키

### 수정자 없는 특수 키

| 키 | 전송 바이트 | 비고 |
|----|------------|------|
| Enter / Return | `0x0D` (CR) | |
| Tab | `0x09` (HT) | |
| Shift+Tab | `CSI Z` = `ESC [ Z` | 역방향 탭 (backtab) |
| Backspace | `0x7F` (DEL) | 현대 터미널 표준; 일부 레거시는 `0x08` |
| Escape | `0x1B` (ESC) | |
| Delete | `CSI 3 ~` = `ESC [ 3 ~` | |
| Space | `0x20` | |
| Ctrl+Space | `0x00` (NUL) | |

---

## 5. 커서 키 (화살표)

커서 키는 **DECCKM** (DEC Cursor Key Mode) 설정에 따라 두 가지 모드로 동작한다.

### Normal 모드 (DECCKM OFF — 기본값)

| 키 | 시퀀스 | 바이트 |
|----|--------|--------|
| ↑ Up | `CSI A` | `1B 5B 41` |
| ↓ Down | `CSI B` | `1B 5B 42` |
| → Right | `CSI C` | `1B 5B 43` |
| ← Left | `CSI D` | `1B 5B 44` |

### Application 모드 (DECCKM ON)

| 키 | 시퀀스 | 바이트 |
|----|--------|--------|
| ↑ Up | `SS3 A` | `1B 4F 41` |
| ↓ Down | `SS3 B` | `1B 4F 42` |
| → Right | `SS3 C` | `1B 4F 43` |
| ← Left | `SS3 D` | `1B 4F 44` |

### 수정자 포함 커서 키

수정자가 있으면 항상 CSI 형식을 사용한다 (Application 모드에서도):

```
CSI 1 ; {modifier} {letter}
```

| 키 조합 | 시퀀스 |
|---------|--------|
| Shift+↑ | `CSI 1;2 A` |
| Alt+↑ | `CSI 1;3 A` |
| Alt+Shift+↑ | `CSI 1;4 A` |
| Ctrl+↑ | `CSI 1;5 A` |
| Ctrl+Shift+↑ | `CSI 1;6 A` |
| Ctrl+Alt+↑ | `CSI 1;7 A` |
| Ctrl+Alt+Shift+↑ | `CSI 1;8 A` |

동일 패턴이 Down(B), Right(C), Left(D)에 적용된다.

---

## 6. 기능 키 (F1-F12)

### F1-F4 (SS3/CSI letter 형식)

수정자 없을 때:

| 키 | Normal 모드 | Application 모드 |
|----|------------|-----------------|
| F1 | `SS3 P` 또는 `CSI 11 ~` | `SS3 P` |
| F2 | `SS3 Q` 또는 `CSI 12 ~` | `SS3 Q` |
| F3 | `SS3 R` 또는 `CSI 13 ~` | `SS3 R` |
| F4 | `SS3 S` 또는 `CSI 14 ~` | `SS3 S` |

> **참고**: 대부분의 현대 터미널은 F1-F4에 `SS3` 형식을 사용한다. xterm의 경우 `oldXtermFKeys` 리소스에 따라 다를 수 있다.

수정자 포함:

```
CSI 1 ; {modifier} P    (F1 + modifier)
CSI 1 ; {modifier} Q    (F2 + modifier)
CSI 1 ; {modifier} R    (F3 + modifier)
CSI 1 ; {modifier} S    (F4 + modifier)
```

### F5-F12 (CSI number ~ 형식)

| 키 | 코드 번호 | 수정자 없음 | 수정자 포함 |
|----|----------|------------|------------|
| F5 | 15 | `CSI 15 ~` | `CSI 15;{mod} ~` |
| F6 | 17 | `CSI 17 ~` | `CSI 17;{mod} ~` |
| F7 | 18 | `CSI 18 ~` | `CSI 18;{mod} ~` |
| F8 | 19 | `CSI 19 ~` | `CSI 19;{mod} ~` |
| F9 | 20 | `CSI 20 ~` | `CSI 20;{mod} ~` |
| F10 | 21 | `CSI 21 ~` | `CSI 21;{mod} ~` |
| F11 | 23 | `CSI 23 ~` | `CSI 23;{mod} ~` |
| F12 | 24 | `CSI 24 ~` | `CSI 24;{mod} ~` |

> **주의**: 번호가 불연속이다 (16, 22 누락). 이는 VT 시리즈 키보드의 역사적 이유이다.

### F13-F20 (확장 기능 키)

| 키 | 코드 번호 | 시퀀스 |
|----|----------|--------|
| F13 | 25 | `CSI 25 ~` |
| F14 | 26 | `CSI 26 ~` |
| F15 | 28 | `CSI 28 ~` |
| F16 | 29 | `CSI 29 ~` |
| F17 | 31 | `CSI 31 ~` |
| F18 | 32 | `CSI 32 ~` |
| F19 | 33 | `CSI 33 ~` |
| F20 | 34 | `CSI 34 ~` |

---

## 7. 편집/네비게이션 키

### 6-Key Editing Pad

| 키 | 코드 번호 | 수정자 없음 | 수정자 포함 |
|----|----------|------------|------------|
| Insert | 2 | `CSI 2 ~` | `CSI 2;{mod} ~` |
| Delete | 3 | `CSI 3 ~` | `CSI 3;{mod} ~` |
| Page Up | 5 | `CSI 5 ~` | `CSI 5;{mod} ~` |
| Page Down | 6 | `CSI 6 ~` | `CSI 6;{mod} ~` |

### Home / End

Home과 End는 두 가지 인코딩이 존재한다:

**xterm 스타일 (CSI letter — 권장):**

| 키 | 수정자 없음 | 수정자 포함 |
|----|------------|------------|
| Home | `CSI H` | `CSI 1;{mod} H` |
| End | `CSI F` | `CSI 1;{mod} F` |

**VT 스타일 (CSI number ~):**

| 키 | 수정자 없음 | 수정자 포함 |
|----|------------|------------|
| Home | `CSI 1 ~` | `CSI 1;{mod} ~` |
| End | `CSI 4 ~` | `CSI 4;{mod} ~` |

> **권장**: Crux는 xterm 스타일(CSI H/F)을 기본으로 사용해야 한다. 이것이 현대 터미널의 표준이다.

---

## 8. 수정자 키 인코딩

### xterm 수정자 파라미터 테이블

CSI 시퀀스에서 수정자 파라미터 값은 다음과 같이 계산된다:

```
modifier_param = 1 + (modifier_bits)
```

| 수정자 조합 | 비트 | 파라미터 값 |
|------------|------|------------|
| (없음) | 0 | 1 (생략됨) |
| Shift | 1 | 2 |
| Alt | 2 | 3 |
| Alt+Shift | 3 | 4 |
| Ctrl | 4 | 5 |
| Ctrl+Shift | 5 | 6 |
| Ctrl+Alt | 6 | 7 |
| Ctrl+Alt+Shift | 7 | 8 |

> **규칙**: 수정자가 없으면 (파라미터=1) 파라미터를 완전히 생략한다. 즉, `CSI 1;1 A`가 아니라 `CSI A`로 전송한다.

### Alt 키의 이중 동작

일반 문자에서 Alt는 두 가지 방식으로 동작한다:

1. **ESC 접두사 방식** (기본): Alt+a → `ESC a` (0x1B 0x61)
2. **8비트 방식** (레거시): Alt+a → `0xE1` (= 0x61 + 0x80)

현대 터미널은 거의 모두 ESC 접두사 방식을 사용한다.

특수 키(화살표, F키 등)에서 Alt는 CSI 수정자 파라미터로 인코딩된다.

---

## 9. Application 모드 vs Normal 모드

### DECCKM (DEC Cursor Key Mode)

| 모드 | 설정 시퀀스 | 해제 시퀀스 | 효과 |
|------|-----------|-----------|------|
| DECCKM | `CSI ? 1 h` | `CSI ? 1 l` | 커서 키가 SS3 형식 사용 |

- **Normal (DECCKM OFF)**: `CSI A` (Up), `CSI B` (Down), `CSI C` (Right), `CSI D` (Left)
- **Application (DECCKM ON)**: `SS3 A`, `SS3 B`, `SS3 C`, `SS3 D`

셸에서는 주로 Normal 모드를 사용하고, vi/vim/less 등 전체화면 애플리케이션에서 Application 모드를 활성화한다.

### smkx/rmkx (terminfo)

terminfo에서 `smkx`/`rmkx` 능력이 이 모드 전환을 제어한다:

- `smkx` (start keypad transmit mode): 애플리케이션이 시작될 때 전송 — Application 모드 활성화
- `rmkx` (reset keypad transmit mode): 애플리케이션이 종료될 때 전송 — Normal 모드 복원

### DECPAM / DECPNM (키패드 모드)

| 모드 | 시퀀스 | 효과 |
|------|--------|------|
| DECPAM (Application) | `ESC =` | 숫자 키패드가 애플리케이션 시퀀스 전송 |
| DECPNM (Numeric) | `ESC >` | 숫자 키패드가 숫자 문자 전송 |

키패드 Application 모드에서:

| 키패드 키 | Application 모드 | Numeric 모드 |
|-----------|-----------------|-------------|
| 0 | `SS3 p` | `0` |
| 1 | `SS3 q` | `1` |
| 2 | `SS3 r` | `2` |
| 3 | `SS3 s` | `3` |
| 4 | `SS3 t` | `4` |
| 5 | `SS3 u` | `5` |
| 6 | `SS3 v` | `6` |
| 7 | `SS3 w` | `7` |
| 8 | `SS3 x` | `8` |
| 9 | `SS3 y` | `9` |
| . | `SS3 n` | `.` |
| + | `SS3 k` | `+` |
| - | `SS3 m` | `-` |
| * | `SS3 j` | `*` |
| / | `SS3 o` | `/` |
| Enter | `SS3 M` | `0x0D` |
| = | `SS3 X` | `=` |

---

## 10. Kitty 키보드 프로토콜

### 개요

Kitty 키보드 프로토콜은 기존 터미널 키 인코딩의 모호성을 해결하기 위한 **점진적 향상(Progressive Enhancement)** 프로토콜이다. 애플리케이션이 opt-in 방식으로 향상된 키 보고를 요청할 수 있다.

### 핵심 형식

```
CSI unicode-key-code:alternate-keys ; modifiers:event-type ; text-as-codepoints u
```

- `CSI` = `0x1B 0x5B`
- 모든 파라미터는 10진수
- 필드는 세미콜론(;)으로 구분
- 서브필드는 콜론(:)으로 구분
- unicode-key-code만 필수, 나머지는 선택

### 수정자 인코딩 (비트 플래그)

Kitty 프로토콜은 xterm보다 더 많은 수정자를 지원한다:

| 수정자 | 비트 | 값 |
|--------|------|-----|
| Shift | bit 0 | 1 |
| Alt | bit 1 | 2 |
| Ctrl | bit 2 | 4 |
| Super (Cmd) | bit 3 | 8 |
| Hyper | bit 4 | 16 |
| Meta | bit 5 | 32 |
| Caps Lock | bit 6 | 64 |
| Num Lock | bit 7 | 128 |

**인코딩된 값 = 1 + (수정자 비트들의 합)**

예시: Ctrl+Shift = 1 + (4 + 1) = 6

### 이벤트 타입

수정자 뒤에 콜론으로 구분하여 이벤트 타입을 보고:

| 이벤트 | 코드 | 설명 |
|--------|------|------|
| Press | 1 | 키 누름 (기본값, 생략 가능) |
| Repeat | 2 | 키 반복 |
| Release | 3 | 키 떼기 |

예시: `CSI 97;5:3 u` = Ctrl+a, release 이벤트

### 향상 플래그

`CSI > flags u`로 push, `CSI < u`로 pop:

| 비트 | 값 | 기능 | 설명 |
|------|-----|------|------|
| 0 | 1 | Disambiguate | ESC/Alt+키 모호성 해결; 비텍스트 키를 CSI 시퀀스로 보고 |
| 1 | 2 | Report events | repeat/release 이벤트 보고 활성화 |
| 2 | 4 | Report alternates | shifted 및 base-layout 변형 포함 |
| 3 | 8 | Report all keys | 텍스트 생성 키도 CSI 시퀀스로 보고 (게임용) |
| 4 | 16 | Report text | CSI 시퀀스에 텍스트 코드포인트 포함 |

### 프로토콜 스택 관리

| 동작 | 시퀀스 | 설명 |
|------|--------|------|
| Push | `CSI > flags u` | 플래그를 스택에 push |
| Pop | `CSI < number u` | 스택에서 number개 pop |
| Query | `CSI ? u` | 현재 플래그 조회 → 응답: `CSI ? flags u` |
| Set | `CSI = flags ; mode u` | mode: 1=set, 2=set bits, 3=reset bits |

### 주요 기능 키 코드 (Private Use Area)

Kitty 프로토콜에서 기능 키는 Unicode Private Use Area를 사용한다:

| 키 | 코드 | | 키 | 코드 |
|----|------|--|-----|------|
| Escape | 27 | | F1 | 57344 (= 0xE000) |
| Enter | 13 | | F2 | 57345 |
| Tab | 9 | | F3 | 57346 |
| Backspace | 127 | | F4 | 57347 |
| Insert | 57348 | | F5 | 57349 |
| Delete | 57350 | | F6 | 57351 |
| Left | 57352 | | F7 | 57353 |
| Right | 57354 | | F8 | 57355 |
| Up | 57356 | | F9 | 57357 |
| Down | 57358 | | F10 | 57359 |
| Page Up | 57360 | | F11 | 57361 |
| Page Down | 57362 | | F12 | 57363 |
| Home | 57364 | | End | 57365 |
| Caps Lock | 57358 | | Scroll Lock | 57359 |
| Num Lock | 57360 | | Print Screen | 57361 |
| Pause | 57362 | | Menu | 57363 |
| F13-F35 | 57376-57398 | | | |
| KP_0-KP_9 | 57399-57408 | | KP_Decimal | 57409 |
| KP_Divide | 57410 | | KP_Multiply | 57411 |
| KP_Subtract | 57412 | | KP_Add | 57413 |
| KP_Enter | 57414 | | KP_Equal | 57415 |
| KP_Separator | 57416 | | KP_Left | 57417 |
| KP_Right | 57418 | | KP_Up | 57419 |
| KP_Down | 57420 | | KP_Page Up | 57421 |
| KP_Page Down | 57422 | | KP_Home | 57423 |
| KP_End | 57424 | | KP_Insert | 57425 |
| KP_Delete | 57426 | | KP_Begin | 57427 |
| Left Shift | 57441 | | Left Ctrl | 57442 |
| Left Alt | 57443 | | Left Super | 57444 |
| Left Hyper | 57445 | | Left Meta | 57446 |
| Right Shift | 57447 | | Right Ctrl | 57448 |
| Right Alt | 57449 | | Right Super | 57450 |
| Right Hyper | 57451 | | Right Meta | 57452 |
| Media Play | 57428 | | Media Pause | 57429 |
| Media Stop | 57432 | | Volume Up | 57436 |
| Volume Down | 57437 | | Mute | 57438 |

### Kitty 프로토콜 레거시 호환성

플래그가 0(기본값)이면, 기존 xterm과 완전히 동일하게 동작한다:
- 일반 문자: UTF-8 바이트 그대로
- 기능 키: `CSI number ; modifier ~` 또는 `SS3 letter`
- Enter: `0x0D`, Tab: `0x09`, Backspace: `0x7F`

플래그=1 (Disambiguate)일 때만 변경되는 사항:
- Escape → `CSI 27 u` (단독 ESC 0x1B와 구분 가능)
- Tab → `CSI 9 u`, Enter → `CSI 13 u` (Ctrl+I, Ctrl+M과 구분)
- 수정자 포함 텍스트 키: `CSI codepoint ; modifier u`

### 지원 터미널

| 터미널 | Kitty 프로토콜 지원 |
|--------|-------------------|
| Kitty | 완전 지원 (발명자) |
| WezTerm | 지원 |
| Ghostty | 지원 |
| foot | 지원 |
| Alacritty | 지원 (0.14+) |
| iTerm2 | 지원 (3.5+) |
| Terminal.app | 미지원 |

---

## 11. fixterms / CSI u 레거시

### 개요

fixterms는 Paul "LeoNerd" Evans가 제안한 키보드 입력 수정 제안서이다. Kitty 프로토콜의 기반이 되었지만 몇 가지 차이가 있다.

### 기본 형식

```
CSI codepoint ; modifier u        (문자 키)
CSI number ; modifier ~           (기능 키)
CSI 1 ; modifier {ABCDFHPQRS}     (커서/F1-F4 키)
```

### fixterms vs Kitty 프로토콜 차이점

| 항목 | fixterms | Kitty |
|------|----------|-------|
| Escape 인코딩 | 8비트 CSI (0x9B) 사용 제안 | `CSI 27 u` |
| Super 수정자 | 미지원 | 지원 (bit 3) |
| Shifted 키 보고 | 미지원 | alternate keys로 보고 |
| Base layout | 미지원 | 비-QWERTY 레이아웃 지원 |
| Release 이벤트 | 미지원 | event-type으로 보고 |
| 텍스트 키 이벤트 | 미지원 | flag 8로 보고 |
| 점진적 향상 | 미지원 | 플래그 스택 기반 |
| 프로토콜 협상 | 없음 | push/pop/query |

### fixterms 주요 매핑

| 키 | 수정자 없음 | Ctrl | Alt |
|----|------------|------|-----|
| a/A | UTF-8 | `CSI 97;5 u` | `ESC a` |
| Tab | `0x09` | `CSI 9;5 u` | `CSI 9;3 u` |
| Enter | `0x0D` | `CSI 13;5 u` | `CSI 13;3 u` |
| Backspace | `0x7F` | `CSI 127;5 u` | `CSI 127;3 u` |
| Space | `0x20` | `0x00` (NUL) | `ESC Space` |

> **iTerm2 참고**: iTerm2는 CSI u 모드를 더 이상 권장하지 않으며, Kitty 프로토콜을 대신 사용할 것을 권장한다.

---

## 12. macOS Option 키 처리

### 문제점

macOS에서 Option 키는 이중 역할을 가진다:
1. **문자 조합 (Character Composition)**: Option+a → `å`, Option+e → `´` (dead key)
2. **Meta/Alt 키**: Option+a → `ESC a` (= Alt+a)

이 두 동작이 충돌하므로 터미널 에뮬레이터가 정책을 결정해야 한다.

### 기존 터미널의 해결 방식

| 터미널 | 기본 동작 | 설정 옵션 |
|--------|----------|----------|
| Terminal.app | 문자 조합 | "Use Option as Meta key" 체크박스 |
| iTerm2 | 문자 조합 | Left/Right Option 각각 설정: Normal/Meta/Esc+ |
| Alacritty | 문자 조합 | `option_as_alt`: `Both`, `OnlyLeft`, `OnlyRight`, `None` |
| WezTerm | Left=Alt, Right=조합 | `send_composed_key_when_left_alt_is_pressed` 등 |
| Kitty | 문자 조합 | `macos_option_as_alt`: `yes`, `no`, `left`, `right` |
| Ghostty | 문자 조합 | `macos-option-as-alt`: `true`, `false`, `left`, `right` |

### Alacritty의 구현 패턴 (Rust)

Alacritty는 `alt_send_esc()` 메서드에서 플랫폼별 처리를 한다:

```rust
fn alt_send_esc(&self) -> bool {
    #[cfg(not(target_os = "macos"))]
    return true;  // macOS 외에서는 항상 ESC 접두사

    #[cfg(target_os = "macos")]
    match self.config.option_as_alt {
        OptionAsAlt::Both => true,
        OptionAsAlt::OnlyLeft => {
            // ModifiersKeyState로 왼쪽 Option만 체크
            self.modifiers.lalt_state() == ModifiersKeyState::Pressed
        }
        OptionAsAlt::OnlyRight => {
            self.modifiers.ralt_state() == ModifiersKeyState::Pressed
        }
        OptionAsAlt::None => false,  // 항상 문자 조합
    }
}
```

### Crux 권장사항

Crux는 다음 옵션을 제공해야 한다:
- `option_as_alt`: `left` (기본값) — 왼쪽 Option=Alt/Meta, 오른쪽 Option=문자 조합
- `both` — 양쪽 모두 Alt/Meta
- `right` — 오른쪽만 Alt/Meta
- `none` — 양쪽 모두 문자 조합

기본값으로 `left`를 권장한다. WezTerm, iTerm2의 기본값과 동일하며 개발자 워크플로에 적합하다.

---

## 13. 완전한 매핑 테이블

### 13.1 일반 문자 + 수정자

| 키 | 없음 | Shift | Alt | Ctrl | Ctrl+Shift | Ctrl+Alt |
|----|------|-------|-----|------|-----------|---------|
| a | `0x61` | `0x41` (A) | `ESC a` | `0x01` | `0x01` | `ESC 0x01` |
| z | `0x7A` | `0x5A` (Z) | `ESC z` | `0x1A` | `0x1A` | `ESC 0x1A` |
| 1 | `0x31` | `0x21` (!) | `ESC 1` | `0x31`* | `0x21`* | `ESC 1`* |
| Space | `0x20` | `0x20` | `ESC 0x20` | `0x00` | `0x00` | `ESC 0x00` |

> *숫자와 특수문자에 Ctrl을 눌러도 대부분 의미있는 제어 코드가 없다. 터미널마다 동작이 다르다.

### 13.2 특수 키 (Normal 모드)

| 키 | 없음 | Shift | Alt | Ctrl | Ctrl+Shift |
|----|------|-------|-----|------|-----------|
| Enter | `0x0D` | `0x0D` | `ESC 0x0D` | `0x0D` | `0x0D` |
| Tab | `0x09` | `CSI Z` | `ESC 0x09` | `0x09`* | `CSI Z`* |
| Backspace | `0x7F` | `0x7F` | `ESC 0x7F` | `0x08` | `0x08` |
| Escape | `0x1B` | `0x1B` | `ESC ESC` | `0x1B` | `0x1B` |

> *Ctrl+Tab과 Ctrl+Shift+Tab은 xterm에서 구분되지 않는다. Kitty 프로토콜에서만 구분 가능.

### 13.3 커서 키 (Normal 모드)

| 키 | 없음 | Shift(2) | Alt(3) | Alt+Shift(4) | Ctrl(5) | Ctrl+Shift(6) | Ctrl+Alt(7) | Ctrl+Alt+Shift(8) |
|----|------|----------|--------|-------------|---------|-------------|-----------|-----------------|
| Up | `CSI A` | `CSI 1;2 A` | `CSI 1;3 A` | `CSI 1;4 A` | `CSI 1;5 A` | `CSI 1;6 A` | `CSI 1;7 A` | `CSI 1;8 A` |
| Down | `CSI B` | `CSI 1;2 B` | `CSI 1;3 B` | `CSI 1;4 B` | `CSI 1;5 B` | `CSI 1;6 B` | `CSI 1;7 B` | `CSI 1;8 B` |
| Right | `CSI C` | `CSI 1;2 C` | `CSI 1;3 C` | `CSI 1;4 C` | `CSI 1;5 C` | `CSI 1;6 C` | `CSI 1;7 C` | `CSI 1;8 C` |
| Left | `CSI D` | `CSI 1;2 D` | `CSI 1;3 D` | `CSI 1;4 D` | `CSI 1;5 D` | `CSI 1;6 D` | `CSI 1;7 D` | `CSI 1;8 D` |

### 13.4 커서 키 (Application 모드, DECCKM ON)

| 키 | 없음 | Shift(2) | Ctrl(5) |
|----|------|----------|---------|
| Up | `SS3 A` | `CSI 1;2 A` | `CSI 1;5 A` |
| Down | `SS3 B` | `CSI 1;2 B` | `CSI 1;5 B` |
| Right | `SS3 C` | `CSI 1;2 C` | `CSI 1;5 C` |
| Left | `SS3 D` | `CSI 1;2 D` | `CSI 1;5 D` |

> **중요**: Application 모드에서도 수정자가 있으면 CSI 형식을 사용한다!

### 13.5 기능 키

| 키 | 없음 | Shift(2) | Alt(3) | Ctrl(5) | Ctrl+Shift(6) |
|----|------|----------|--------|---------|-------------|
| F1 | `SS3 P` | `CSI 1;2 P` | `CSI 1;3 P` | `CSI 1;5 P` | `CSI 1;6 P` |
| F2 | `SS3 Q` | `CSI 1;2 Q` | `CSI 1;3 Q` | `CSI 1;5 Q` | `CSI 1;6 Q` |
| F3 | `SS3 R` | `CSI 1;2 R` | `CSI 1;3 R` | `CSI 1;5 R` | `CSI 1;6 R` |
| F4 | `SS3 S` | `CSI 1;2 S` | `CSI 1;3 S` | `CSI 1;5 S` | `CSI 1;6 S` |
| F5 | `CSI 15~` | `CSI 15;2~` | `CSI 15;3~` | `CSI 15;5~` | `CSI 15;6~` |
| F6 | `CSI 17~` | `CSI 17;2~` | `CSI 17;3~` | `CSI 17;5~` | `CSI 17;6~` |
| F7 | `CSI 18~` | `CSI 18;2~` | `CSI 18;3~` | `CSI 18;5~` | `CSI 18;6~` |
| F8 | `CSI 19~` | `CSI 19;2~` | `CSI 19;3~` | `CSI 19;5~` | `CSI 19;6~` |
| F9 | `CSI 20~` | `CSI 20;2~` | `CSI 20;3~` | `CSI 20;5~` | `CSI 20;6~` |
| F10 | `CSI 21~` | `CSI 21;2~` | `CSI 21;3~` | `CSI 21;5~` | `CSI 21;6~` |
| F11 | `CSI 23~` | `CSI 23;2~` | `CSI 23;3~` | `CSI 23;5~` | `CSI 23;6~` |
| F12 | `CSI 24~` | `CSI 24;2~` | `CSI 24;3~` | `CSI 24;5~` | `CSI 24;6~` |

### 13.6 편집/네비게이션 키

| 키 | 없음 | Shift(2) | Alt(3) | Ctrl(5) | Ctrl+Shift(6) |
|----|------|----------|--------|---------|-------------|
| Home | `CSI H` | `CSI 1;2 H` | `CSI 1;3 H` | `CSI 1;5 H` | `CSI 1;6 H` |
| End | `CSI F` | `CSI 1;2 F` | `CSI 1;3 F` | `CSI 1;5 F` | `CSI 1;6 F` |
| Insert | `CSI 2~` | `CSI 2;2~` | `CSI 2;3~` | `CSI 2;5~` | `CSI 2;6~` |
| Delete | `CSI 3~` | `CSI 3;2~` | `CSI 3;3~` | `CSI 3;5~` | `CSI 3;6~` |
| Page Up | `CSI 5~` | `CSI 5;2~` | `CSI 5;3~` | `CSI 5;5~` | `CSI 5;6~` |
| Page Down | `CSI 6~` | `CSI 6;2~` | `CSI 6;3~` | `CSI 6;5~` | `CSI 6;6~` |

### 13.7 Kitty 프로토콜 대안 (플래그=1, Disambiguate)

| 키 | xterm 레거시 | Kitty CSI u |
|----|-------------|-------------|
| Enter | `0x0D` | `CSI 13 u` |
| Tab | `0x09` | `CSI 9 u` |
| Backspace | `0x7F` | `CSI 127 u` |
| Escape | `0x1B` | `CSI 27 u` |
| Space | `0x20` | `CSI 32 u` |
| Ctrl+I | `0x09` (= Tab) | `CSI 105;5 u` (구분됨!) |
| Ctrl+M | `0x0D` (= Enter) | `CSI 109;5 u` (구분됨!) |
| Ctrl+[ | `0x1B` (= Esc) | `CSI 91;5 u` (구분됨!) |
| a | `0x61` | `0x61` (변경 없음) |
| Ctrl+a | `0x01` | `CSI 97;5 u` |
| Shift+a | `0x41` (A) | `0x41` (변경 없음) |
| Ctrl+Shift+a | `0x01` | `CSI 97;6 u` |

---

## 14. Rust 구현 패턴

### 14.1 데이터 구조

Alacritty의 접근법을 참고한 Crux용 설계:

```rust
use bitflags::bitflags;

/// 터미널 키보드 모드
bitflags! {
    pub struct TermMode: u32 {
        const APP_CURSOR          = 1 << 0;  // DECCKM
        const APP_KEYPAD          = 1 << 1;  // DECPAM
        const ALT_SCREEN          = 1 << 2;
        const KITTY_DISAMBIGUATE  = 1 << 3;  // Kitty flag 1
        const KITTY_REPORT_EVENTS = 1 << 4;  // Kitty flag 2
        const KITTY_REPORT_ALTS   = 1 << 5;  // Kitty flag 4
        const KITTY_REPORT_ALL    = 1 << 6;  // Kitty flag 8
        const KITTY_REPORT_TEXT   = 1 << 7;  // Kitty flag 16
    }
}

/// 수정자 키
bitflags! {
    pub struct Modifiers: u8 {
        const SHIFT   = 0b0001;
        const ALT     = 0b0010;
        const CONTROL = 0b0100;
        const SUPER   = 0b1000;
    }
}

impl Modifiers {
    /// xterm 수정자 파라미터 값 (1 + bits)
    pub fn param(&self) -> u32 {
        self.bits() as u32 + 1
    }

    /// 수정자가 있으면 ";{param}" 문자열, 없으면 빈 문자열
    pub fn suffix(&self) -> String {
        if self.is_empty() {
            String::new()
        } else {
            format!(";{}", self.param())
        }
    }
}

/// 이스케이프 시퀀스 종료자 타입
enum SequenceTerminator {
    /// CSI number ~ 형식
    Tilde,
    /// CSI [1;mod] letter 형식
    Letter(char),
    /// SS3 letter 형식
    Ss3(char),
    /// CSI number [;mod] u 형식 (Kitty)
    KittyU,
}

/// 키 이벤트 타입 (Kitty 프로토콜)
#[derive(Clone, Copy, Default)]
enum KeyEventType {
    #[default]
    Press = 1,
    Repeat = 2,
    Release = 3,
}
```

### 14.2 키 → 이스케이프 시퀀스 변환 함수

```rust
/// GPUI의 KeyDownEvent를 PTY에 전송할 바이트 시퀀스로 변환
pub fn build_key_sequence(
    key: &KeyEvent,       // GPUI 키 이벤트
    mods: Modifiers,
    mode: TermMode,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16);

    // 1. Kitty 프로토콜 모드에서의 처리
    if mode.contains(TermMode::KITTY_REPORT_ALL) {
        return build_kitty_sequence(key, mods, mode);
    }

    // 2. 일반 문자 입력
    if let Some(ch) = key.character() {
        if mods.contains(Modifiers::ALT) && should_alt_send_esc() {
            buf.push(0x1B);  // ESC 접두사
        }
        if mods.contains(Modifiers::CONTROL) {
            if let Some(ctrl_byte) = ctrl_char(ch) {
                buf.push(ctrl_byte);
                return buf;
            }
        }
        // UTF-8 문자 그대로 전송
        let mut utf8_buf = [0u8; 4];
        let s = ch.encode_utf8(&mut utf8_buf);
        buf.extend_from_slice(s.as_bytes());
        return buf;
    }

    // 3. 특수 키 처리
    match key.named_key() {
        // 커서 키
        NamedKey::ArrowUp    => write_cursor_key(&mut buf, 'A', mods, mode),
        NamedKey::ArrowDown  => write_cursor_key(&mut buf, 'B', mods, mode),
        NamedKey::ArrowRight => write_cursor_key(&mut buf, 'C', mods, mode),
        NamedKey::ArrowLeft  => write_cursor_key(&mut buf, 'D', mods, mode),

        // F1-F4 (SS3/CSI letter)
        NamedKey::F1 => write_f1_f4(&mut buf, 'P', mods),
        NamedKey::F2 => write_f1_f4(&mut buf, 'Q', mods),
        NamedKey::F3 => write_f1_f4(&mut buf, 'R', mods),
        NamedKey::F4 => write_f1_f4(&mut buf, 'S', mods),

        // F5-F12 (CSI number ~)
        NamedKey::F5  => write_csi_tilde(&mut buf, 15, mods),
        NamedKey::F6  => write_csi_tilde(&mut buf, 17, mods),
        NamedKey::F7  => write_csi_tilde(&mut buf, 18, mods),
        NamedKey::F8  => write_csi_tilde(&mut buf, 19, mods),
        NamedKey::F9  => write_csi_tilde(&mut buf, 20, mods),
        NamedKey::F10 => write_csi_tilde(&mut buf, 21, mods),
        NamedKey::F11 => write_csi_tilde(&mut buf, 23, mods),
        NamedKey::F12 => write_csi_tilde(&mut buf, 24, mods),

        // 편집/네비게이션
        NamedKey::Insert   => write_csi_tilde(&mut buf, 2, mods),
        NamedKey::Delete   => write_csi_tilde(&mut buf, 3, mods),
        NamedKey::PageUp   => write_csi_tilde(&mut buf, 5, mods),
        NamedKey::PageDown => write_csi_tilde(&mut buf, 6, mods),
        NamedKey::Home     => write_cursor_key(&mut buf, 'H', mods, mode),
        NamedKey::End      => write_cursor_key(&mut buf, 'F', mods, mode),

        // 특수 단일 바이트
        NamedKey::Enter     => { buf.push(0x0D); }
        NamedKey::Tab       => {
            if mods.contains(Modifiers::SHIFT) {
                buf.extend_from_slice(b"\x1B[Z");
            } else {
                buf.push(0x09);
            }
        }
        NamedKey::Backspace => { buf.push(0x7F); }
        NamedKey::Escape    => { buf.push(0x1B); }

        _ => {}
    }

    buf
}

/// 커서 키 시퀀스 작성
fn write_cursor_key(buf: &mut Vec<u8>, letter: char, mods: Modifiers, mode: TermMode) {
    if mods.is_empty() && mode.contains(TermMode::APP_CURSOR) {
        // Application 모드: SS3 letter
        buf.push(0x1B);  // ESC
        buf.push(b'O');  // SS3
        buf.push(letter as u8);
    } else if mods.is_empty() {
        // Normal 모드, 수정자 없음: CSI letter
        buf.push(0x1B);
        buf.push(b'[');
        buf.push(letter as u8);
    } else {
        // 수정자 있음: CSI 1;{mod} letter
        write!(buf, "\x1B[1;{}{}", mods.param(), letter).unwrap();
    }
}

/// F1-F4 시퀀스 작성
fn write_f1_f4(buf: &mut Vec<u8>, letter: char, mods: Modifiers) {
    if mods.is_empty() {
        // SS3 letter
        buf.push(0x1B);
        buf.push(b'O');
        buf.push(letter as u8);
    } else {
        // CSI 1;{mod} letter
        write!(buf, "\x1B[1;{}{}", mods.param(), letter).unwrap();
    }
}

/// CSI number [;mod] ~ 시퀀스 작성
fn write_csi_tilde(buf: &mut Vec<u8>, number: u32, mods: Modifiers) {
    if mods.is_empty() {
        write!(buf, "\x1B[{}~", number).unwrap();
    } else {
        write!(buf, "\x1B[{};{}~", number, mods.param()).unwrap();
    }
}

/// Ctrl + 문자 → 제어 코드 변환
fn ctrl_char(ch: char) -> Option<u8> {
    match ch {
        'a'..='z' => Some(ch as u8 - b'a' + 1),
        'A'..='Z' => Some(ch as u8 - b'A' + 1),
        '@'       => Some(0x00),
        '['       => Some(0x1B),
        '\\'      => Some(0x1C),
        ']'       => Some(0x1D),
        '^'       => Some(0x1E),
        '_'       => Some(0x1F),
        ' '       => Some(0x00),
        '/'       => Some(0x1F),
        '?'       => Some(0x7F),
        _         => None,
    }
}
```

### 14.3 Kitty 프로토콜 구현

```rust
/// Kitty 프로토콜 키보드 모드 스택
pub struct KittyKeyboardState {
    /// 플래그 스택 (최대 4096)
    stack: Vec<u8>,
}

impl KittyKeyboardState {
    pub fn new() -> Self {
        Self { stack: vec![0] }  // 기본값: 플래그 0
    }

    pub fn current_flags(&self) -> u8 {
        *self.stack.last().unwrap_or(&0)
    }

    pub fn push(&mut self, flags: u8) {
        if self.stack.len() < 4096 {
            self.stack.push(flags);
        }
    }

    pub fn pop(&mut self, count: u32) {
        for _ in 0..count {
            if self.stack.len() > 1 {
                self.stack.pop();
            }
        }
    }

    pub fn set(&mut self, flags: u8, mode: u8) {
        let current = self.current_flags();
        let new_flags = match mode {
            1 => flags,                    // 설정/리셋
            2 => current | flags,          // 비트 설정만
            3 => current & !flags,         // 비트 리셋만
            _ => current,
        };
        if let Some(last) = self.stack.last_mut() {
            *last = new_flags;
        }
    }
}

/// Kitty 프로토콜용 시퀀스 빌더
fn build_kitty_sequence(
    key: &KeyEvent,
    mods: Modifiers,
    mode: TermMode,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(32);

    let (code, is_text_key) = kitty_key_code(key);

    // 텍스트 키이고 수정자 없으면 레거시 호환
    if is_text_key && mods.is_empty()
        && !mode.contains(TermMode::KITTY_REPORT_ALL)
    {
        // UTF-8 바이트 그대로 전송
        if let Some(ch) = key.character() {
            let mut utf8_buf = [0u8; 4];
            let s = ch.encode_utf8(&mut utf8_buf);
            buf.extend_from_slice(s.as_bytes());
            return buf;
        }
    }

    // CSI code [;mods[:event_type]] u
    let mod_param = kitty_modifier_param(mods);
    let event_type = if mode.contains(TermMode::KITTY_REPORT_EVENTS) {
        match key.event_type() {
            KeyEventType::Press => None,    // 1은 기본값이므로 생략
            KeyEventType::Repeat => Some(2),
            KeyEventType::Release => Some(3),
        }
    } else {
        None
    };

    buf.extend_from_slice(b"\x1B[");
    write!(buf, "{}", code).unwrap();

    if mod_param > 1 || event_type.is_some() {
        write!(buf, ";{}", mod_param).unwrap();
        if let Some(et) = event_type {
            write!(buf, ":{}", et).unwrap();
        }
    }

    buf.push(b'u');
    buf
}

/// Kitty 수정자 인코딩 (1 + bitflags)
fn kitty_modifier_param(mods: Modifiers) -> u32 {
    let mut bits: u32 = 0;
    if mods.contains(Modifiers::SHIFT)   { bits |= 1; }
    if mods.contains(Modifiers::ALT)     { bits |= 2; }
    if mods.contains(Modifiers::CONTROL) { bits |= 4; }
    if mods.contains(Modifiers::SUPER)   { bits |= 8; }
    bits + 1
}
```

### 14.4 Alacritty의 핵심 설계 패턴

Alacritty의 키 입력 처리 파이프라인:

```
KeyEvent (winit/GPUI)
    │
    ▼
build_sequence()
    │
    ├── SequenceBuilder 생성
    │   ├── 수정자 변환 (ModifiersState → SequenceModifiers)
    │   └── 터미널 모드 확인 (TermMode 플래그)
    │
    ├── 빌더 메서드 우선순위 (fallback chain):
    │   1. try_build_numpad()         — 키패드 키 (Kitty 코드 57399-57426)
    │   2. try_build_named_kitty()    — Kitty 전용 기능 키 (F13-F35, 미디어)
    │   3. try_build_named_normal()   — xterm/VT220 표준 시퀀스
    │   4. try_build_control_char_or_mod() — 제어 문자, 수정자 키
    │
    ├── SequenceBase 결정
    │   ├── payload: 키 코드 문자열 ("15", "A", "27" 등)
    │   └── terminator: Normal(char) | Kitty
    │
    ├── 수정자 인코딩
    │   └── modifier_param = bits + 1
    │
    ├── 이벤트 타입 인코딩 (Kitty 모드)
    │   └── event_type: 1=press, 2=repeat, 3=release
    │
    └── 최종 바이트 조립
        ├── CSI (0x1B 0x5B) 접두사
        ├── payload
        ├── ;modifier[:event_type]
        └── terminator 문자
```

**핵심 설계 원칙:**
1. **Fallback 체인**: 여러 빌더를 순서대로 시도하여 `Option<SequenceBase>`를 반환
2. **모드 기반 분기**: `TermMode` 플래그에 따라 동적으로 인코딩 변경
3. **수정자 분리**: 수정자는 별도 구조체로 관리하여 인코딩 로직과 분리
4. **SS3 → CSI 자동 승격**: 수정자가 있으면 SS3를 CSI로 자동 변환

---

## 15. Crux 구현 권장사항

### 15.1 Phase 1 (MVP) 최소 구현

Phase 1에서는 다음만 구현하면 충분하다:

1. **일반 문자**: UTF-8 바이트 그대로 전송
2. **Ctrl+키**: C0 제어 코드 (0x01-0x1A)
3. **특수 키**: Enter(0x0D), Tab(0x09), Backspace(0x7F), Escape(0x1B)
4. **Shift+Tab**: `CSI Z`
5. **커서 키**: Normal/Application 모드 (CSI/SS3)
6. **F1-F12**: SS3 + CSI 시퀀스
7. **편집 키**: Home/End/Insert/Delete/PgUp/PgDn
8. **수정자 인코딩**: Shift/Alt/Ctrl 조합

### 15.2 Phase 2+ 추가 구현

1. **Kitty 키보드 프로토콜**: CSI > flags u push/pop/query
2. **키패드 Application 모드**: DECPAM/DECPNM
3. **macOS Option 키 설정**: left/right/both/none
4. **modifyOtherKeys**: xterm의 CSI > 4;N m

### 15.3 알아야 할 함정

1. **Alt+문자 vs ESC 시퀀스 구분**: 사용자가 Alt+a를 누른 것인지, ESC를 누르고 a를 빠르게 누른 것인지 구분이 안 됨. 타이밍 기반 휴리스틱이 필요하지만, 우리는 GUI 터미널이므로 키 이벤트에서 Alt 수정자를 직접 확인할 수 있어 이 문제가 없다.

2. **Backspace 바이트**: 현대 표준은 `0x7F`(DEL)이지만, 일부 레거시 시스템은 `0x08`(BS)을 기대한다. `0x7F`를 기본으로 하되 설정 가능하게 만든다.

3. **DECCKM 상태 추적**: alacritty_terminal의 `TermMode`에서 APP_CURSOR 플래그를 읽어 현재 모드를 확인한다. 이 상태는 VT 파서가 `CSI ? 1 h`/`CSI ? 1 l` 시퀀스를 처리할 때 자동으로 갱신된다.

4. **F 키 번호 불연속**: F5-F12의 코드 번호가 15, 17-21, 23-24로 불연속이다. 하드코딩된 매핑 테이블이 필요하다.

5. **Kitty 스택 제한**: 스택 깊이에 합리적인 제한(4096)을 둔다. Alacritty와 동일.

### 15.4 alacritty_terminal 활용

Crux는 alacritty_terminal 크레이트를 VT 파서로 사용하므로, `Term` 구조체의 `mode()` 메서드로 현재 TermMode를 읽고, `PtyWrite` 이벤트로 키 시퀀스를 PTY에 전달한다. 키 → 시퀀스 변환 로직은 Crux 자체에 구현해야 한다 (alacritty_terminal은 파싱만 담당).

```rust
// Crux에서의 사용 패턴
fn handle_key_down(&mut self, event: &KeyDownEvent) {
    let mode = self.terminal.mode();
    let mods = convert_gpui_modifiers(&event.modifiers);
    let bytes = build_key_sequence(event, mods, mode);

    if !bytes.is_empty() {
        self.pty_writer.write_all(&bytes).unwrap();
    }
}
```

---

## 참고 자료

- [xterm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html) — Thomas Dickey의 공식 xterm 문서
- [Kitty Keyboard Protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) — Kovid Goyal의 Kitty 키보드 프로토콜 명세
- [fixterms (Fix Keyboard Input)](http://www.leonerd.org.uk/hacks/fixterms/) — Paul LeoNerd Evans의 원본 제안서
- [WezTerm Key Encoding](https://wezterm.org/config/key-encoding.html) — WezTerm의 키 인코딩 문서
- [Alacritty keyboard.rs](https://github.com/alacritty/alacritty/blob/master/alacritty/src/input/keyboard.rs) — Alacritty의 Rust 키보드 처리 소스
- [ANSI Escape Codes Reference](https://gist.github.com/fnky/458719343aabd01cfb17a3a4f7296797) — 이스케이프 시퀀스 치트시트
- [iTerm2 CSI u Documentation](https://iterm2.com/documentation-csiu.html) — iTerm2의 CSI u 문서

---

## 16. Kitty 키보드 프로토콜 — 심층 구현 가이드

> 이 섹션은 10절의 개요를 보완하여, Crux 구현에 필요한 실전적 세부사항을 다룬다.

### 16.1 프로토콜 개요: 점진적 향상 (Progressive Enhancement)

Kitty 키보드 프로토콜은 기존 터미널 키 입력의 근본적 한계를 해결한다:

**해결하는 문제:**
- Tab과 Ctrl+I의 구분 불가 (둘 다 `0x09`)
- Enter와 Ctrl+M의 구분 불가 (둘 다 `0x0D`)
- Escape와 Ctrl+[의 구분 불가 (둘 다 `0x1B`)
- Alt+키와 ESC 접두사 시퀀스의 모호성
- 키 릴리스 이벤트 부재
- Super(Cmd) 키 수정자 미지원

**핵심 설계 원칙:**
1. **Opt-in**: 애플리케이션이 명시적으로 요청해야 활성화
2. **Backward Compatible**: 기본 상태(플래그=0)에서 레거시와 100% 동일
3. **Stackable**: 중첩된 애플리케이션이 각자의 모드를 관리 가능
4. **Screen-isolated**: 메인/대체 화면에 독립적인 플래그 스택 유지

### 16.2 CSI u 형식 상세

```
CSI unicode-key-code:shifted-key:base-layout-key ; modifiers:event-type ; text-as-codepoints u
```

**각 필드 설명:**

| 필드 | 구분자 | 필수 | 설명 |
|------|--------|------|------|
| `unicode-key-code` | 없음 | **필수** | 키의 Unicode 코드포인트 (소문자 기준) |
| `shifted-key` | `:` | 선택 | Shift 상태의 코드포인트 (플래그 4) |
| `base-layout-key` | `:` | 선택 | 기본 레이아웃의 코드포인트 (플래그 4) |
| `modifiers` | `;` | 선택 | 수정자 비트마스크 + 1 |
| `event-type` | `:` | 선택 | 1=press, 2=repeat, 3=release (플래그 2) |
| `text-as-codepoints` | `;` | 선택 | 생성된 텍스트의 코드포인트들 (플래그 16) |

**인코딩 규칙:**
- 값이 기본값이면 생략 (modifiers=1 생략, event-type=1 생략)
- 뒤에 오는 빈 필드는 생략 (trailing semicolons 없음)
- 중간 빈 서브필드는 콜론으로 유지 (`code::base` = shifted 생략)

**예시:**

```
CSI 97 u                          # 'a' (플래그 8: 모든 키를 CSI u로)
CSI 97;5 u                        # Ctrl+a
CSI 97;6 u                        # Ctrl+Shift+a
CSI 97:65 ;6 u                    # Ctrl+Shift+a (shifted='A'=65 보고, 플래그 4)
CSI 97;5:3 u                      # Ctrl+a release (플래그 2)
CSI 97;5:2 u                      # Ctrl+a repeat (플래그 2)
CSI 97:65:97 ;6:1 ;65 u           # 전체 보고: key=a, shifted=A, base=a, Ctrl+Shift, press, text=A
```

### 16.3 플래그 시스템 심층 분석

#### 플래그 1 — Disambiguate (0b00001)

**가장 일반적으로 사용되는 플래그.** 대부분의 TUI 앱은 이것만으로 충분하다.

변경 사항:
- 레거시와 충돌하는 키만 CSI u 형식으로 변환
- 일반 문자 입력은 여전히 UTF-8 바이트 그대로
- 수정자가 있는 텍스트 키는 CSI u로 보고

| 키 | 레거시 | 플래그 1 적용 후 |
|----|--------|-----------------|
| Enter | `0x0D` | `CSI 13 u` |
| Tab | `0x09` | `CSI 9 u` |
| Backspace | `0x7F` | `CSI 127 u` |
| Escape | `0x1B` | `CSI 27 u` |
| Ctrl+I | `0x09` (= Tab) | `CSI 105;5 u` (구분됨!) |
| Ctrl+M | `0x0D` (= Enter) | `CSI 109;5 u` (구분됨!) |
| a (수정자 없음) | `0x61` | `0x61` (변경 없음) |
| Ctrl+a | `0x01` | `CSI 97;5 u` |

#### 플래그 2 — Report Event Types (0b00010)

키 이벤트의 종류(press/repeat/release)를 구분하여 보고한다.

- 게임, 에디터 키 바인딩에 유용
- event-type은 수정자 필드 뒤에 콜론으로 구분: `;modifier:event-type`
- press(1)는 기본값이므로 생략 가능

```
CSI 97;1:1 u   →   CSI 97 u        (press는 생략)
CSI 97;1:2 u                        (repeat)
CSI 97;1:3 u                        (release)
```

#### 플래그 4 — Report Alternate Keys (0b00100)

Shift 상태의 키와 기본 키보드 레이아웃의 키를 보고한다.

- 비-QWERTY 키보드 레이아웃 지원에 중요
- shifted-key: Shift를 눌렀을 때의 코드포인트
- base-layout-key: 현재 레이아웃과 무관한 물리적 키 위치의 QWERTY 코드포인트

```
# 독일어 키보드에서 'z' 키 (QWERTZ → QWERTY에서는 'y')
CSI 122::121 u    # key=z, base-layout=y
```

#### 플래그 8 — Report All Keys as Escape Codes (0b01000)

**모든** 키 입력을 CSI 시퀀스로 보고한다 (일반 문자 포함).

- 게임용으로 설계 (모든 키를 일관되게 처리)
- 일반적인 TUI 앱에서는 사용하지 않는 것이 좋음
- 텍스트 입력이 필요한 경우 비실용적

```
# 플래그 8 ON일 때
'a'    → CSI 97 u        (레거시에서는 0x61)
'A'    → CSI 97;2 u      (Shift+a)
'1'    → CSI 49 u
```

#### 플래그 16 — Report Associated Text (0b10000)

CSI 시퀀스에 키 입력으로 생성되는 텍스트의 코드포인트를 포함한다.

- 복합 문자, dead key 시퀀스 처리에 유용
- 세 번째 세미콜론 이후에 코드포인트 목록으로 전달

```
# Shift+a → text='A'(65)
CSI 97;2;65 u

# 복합 문자: é (dead key 사용 시)
CSI 101;1;233 u    # key=e, text=é(233)
```

### 16.4 스택 메커니즘 상세

#### Push / Pop / Query

```
CSI > flags u       # Push: 새 플래그를 스택에 push
CSI < number u      # Pop: 스택에서 number개 pop (기본값 1)
CSI ? u             # Query: 현재 플래그 조회
```

**Query 응답:**
```
CSI ? flags u       # 터미널 → 애플리케이션
```

#### Set (CSI =)

```
CSI = flags ; mode u
```

| mode | 동작 | 설명 |
|------|------|------|
| 1 | Replace | 현재 스택 탑을 flags로 교체 |
| 2 | Set bits | 현재 플래그에 flags 비트 OR |
| 3 | Reset bits | 현재 플래그에서 flags 비트 제거 |

#### 스택 격리 규칙

- **메인 화면과 대체 화면은 독립적인 스택을 유지해야 한다**
- 대체 화면에서 push한 플래그는 메인 화면에 영향을 주지 않는다
- 스택 깊이 제한: 합리적인 상한 (Alacritty: 4096, 권장: 256)
- DoS 방어: 무한 push 공격 방지

```rust
// 스택 격리 구현 (Crux)
struct TerminalState {
    main_keyboard_stack: Vec<u8>,     // 메인 화면 스택
    alt_keyboard_stack: Vec<u8>,      // 대체 화면 스택
    is_alt_screen: bool,
}

impl TerminalState {
    fn current_keyboard_stack(&mut self) -> &mut Vec<u8> {
        if self.is_alt_screen {
            &mut self.alt_keyboard_stack
        } else {
            &mut self.main_keyboard_stack
        }
    }
}
```

### 16.5 기능 키 인코딩 — 레거시 호환 매핑

기능 키는 Kitty 프로토콜에서도 레거시와 동일한 형식을 유지할 수 있다:

| 키 | 레거시 형식 | Kitty 형식 (플래그 8) |
|----|------------|---------------------|
| F1 | `SS3 P` 또는 `CSI 1 P` | `CSI 1;modifier P` |
| F5 | `CSI 15 ~` | `CSI 15;modifier ~` |
| Up | `CSI A` 또는 `SS3 A` | `CSI 1;modifier A` |
| Home | `CSI H` | `CSI 1;modifier H` |
| Insert | `CSI 2 ~` | `CSI 2;modifier ~` |

> **핵심**: 기능 키는 레거시 형식을 그대로 사용하되, 수정자가 있을 때 CSI 파라미터로 인코딩한다. CSI u 형식(`CSI code u`)은 텍스트 키와 특수 키(Enter, Tab 등)에만 사용된다.

### 16.6 alacritty_terminal에서의 Kitty 프로토콜 지원

Alacritty는 PR #7125에서 Kitty 키보드 프로토콜을 구현했다 (0.14+).

**alacritty_terminal 0.25에서의 지원 수준:**
- CSI > flags u (Push) 처리 ✓
- CSI < number u (Pop) 처리 ✓
- CSI ? u (Query) 응답 ✓
- CSI = flags ; mode u (Set) 처리 ✓
- 메인/대체 화면 스택 격리 ✓
- TermMode 플래그로 현재 Kitty 모드 노출 ✓

**알려진 이슈:**
1. C0 제어 코드에 매핑되는 키(Tab, Enter, Backspace, Escape)에 associated text를 포함하여 전송 — 다른 터미널과 동작이 다름
2. 일부 기능 키의 이스케이프 시퀀스가 Kitty 레퍼런스 구현과 미세하게 다름

**Crux에서의 활용:**
- alacritty_terminal이 VT 파서 측에서 CSI > u / CSI < u / CSI ? u를 처리
- Crux는 키 입력 → 이스케이프 시퀀스 변환 시 `Term::mode()`의 Kitty 플래그를 확인
- 14절의 `build_kitty_sequence()` 함수가 이 변환을 담당

```rust
// alacritty_terminal의 TermMode 플래그 확인
let mode = self.terminal.mode();
if mode.contains(TermMode::KITTY_KEYBOARD_PROTOCOL) {
    // Kitty 프로토콜 인코딩 사용
    let flags = self.terminal.kitty_keyboard_flags();
    // flags에 따라 적절한 시퀀스 생성
}
```

### 16.7 Ghostty의 Kitty 키보드 프로토콜 구현

#### 주요 특성

1. **fixterms 기본 사용**: Ghostty는 기본적으로 fixterms 인코딩을 사용하며, Kitty 프로토콜은 애플리케이션이 CSI > u로 명시적 활성화해야 한다

2. **macOS Cmd 키 처리**: Super(Cmd) 수정자가 Kitty 프로토콜 비트 3으로 올바르게 인코딩됨. 단, `Cmd+Backspace` → `\x15`(Ctrl-U) 문제가 보고됨

3. **AltGr 호환성 이슈**: Linux 독일어 키보드에서 AltGr+`+`가 `~` 대신 `+`를 생성하는 이슈 — Crux는 macOS 전용이므로 이 문제 없음

4. **기본 키 바인딩 우선순위**: Ghostty의 기본 키 바인딩(Alt+Left → `ESC b`)이 Kitty 프로토콜보다 우선 적용되어 혼동을 야기. **Crux는 Kitty 프로토콜이 활성화되면 기본 바인딩을 비활성화해야 한다.**

#### 교훈

- Kitty 프로토콜 활성화 시 레거시 키 바인딩과의 충돌을 명확히 해결해야 한다
- 일부 macOS 시스템 키 조합(Cmd+Q, Cmd+W 등)은 Kitty 프로토콜과 무관하게 시스템 수준에서 처리해야 한다
- Vim은 Ghostty에서 Kitty 프로토콜을 자동으로 활성화하지 않으므로, `TERM` 이름 또는 DA 응답으로 프로토콜 지원을 광고해야 한다

### 16.8 Crux 구현 로드맵

#### Phase 1: 지원 불필요
- 기본 xterm 레거시 인코딩만 구현
- Kitty 플래그 스택은 alacritty_terminal이 자동 관리

#### Phase 4: 기본 구현
- [ ] CSI > u → Push 플래그 (alacritty_terminal 처리)
- [ ] CSI < u → Pop 플래그 (alacritty_terminal 처리)
- [ ] CSI ? u → 현재 플래그 응답 (alacritty_terminal 처리)
- [ ] 키 입력 변환에서 Kitty 플래그 확인
- [ ] 플래그 1 (Disambiguate): Tab/Ctrl+I 등 구분
- [ ] 플래그 2 (Report events): release 이벤트 전송
- [ ] 메인/대체 화면 스택 격리 확인

#### Phase 5: 완전 구현
- [ ] 플래그 4 (Report alternates): Shift/base-layout 보고
- [ ] 플래그 8 (Report all keys): 모든 키를 CSI u로
- [ ] 플래그 16 (Report text): 텍스트 코드포인트 포함
- [ ] CSI = flags ; mode u (Set) 처리
- [ ] tmux extkeys 연동 테스트
- [ ] Kitty 프로토콜 활성화 시 기본 키 바인딩 비활성화

### 16.9 테스트 전략

```bash
# Kitty 프로토콜 테스트 도구
# 1. kitten을 사용한 테스트 (Kitty 설치 필요)
kitten show_key

# 2. 수동 테스트: 플래그 1 활성화 후 키 입력 확인
printf '\e[>1u'        # Disambiguate 활성화
# Tab 누르기 → CSI 9 u 가 보이면 성공
# Ctrl+I 누르기 → CSI 105;5 u 가 보이면 성공
printf '\e[<u'         # Pop (복원)

# 3. 플래그 쿼리 테스트
printf '\e[?u'         # 현재 플래그 조회
# 응답: CSI ? 0 u (기본 상태)

# 4. 스택 테스트
printf '\e[>1u'        # push 1
printf '\e[?u'         # → CSI ? 1 u
printf '\e[>3u'        # push 3
printf '\e[?u'         # → CSI ? 3 u
printf '\e[<2u'        # pop 2
printf '\e[?u'         # → CSI ? 0 u (기본 상태)

# 5. tmux에서 테스트
tmux set -s extended-keys on
tmux set -s extended-keys-format csi-u
# vim 내에서 Ctrl+I와 Tab이 구분되는지 확인
```
