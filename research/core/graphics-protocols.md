---
title: "터미널 그래픽스 프로토콜 심층 리서치"
description: "Kitty Graphics Protocol, iTerm2 Inline Image, Sixel 프로토콜의 상세 사양과 Ghostty 구현 참조, Crux 아키텍처 전략"
date: 2026-02-12
phase: [4]
topics: [graphics-protocol, kitty-graphics, iterm2-images, sixel, gpu-rendering, metal, image-storage]
status: final
related:
  - terminal-emulation.md
  - terminal-architecture.md
  - ../gpui/framework.md
  - ../gpui/terminal-implementations.md
---

# 터미널 그래픽스 프로토콜 심층 리서치

> 작성일: 2026-02-12
> 목적: Crux 터미널 에뮬레이터의 인라인 이미지 렌더링 구현을 위한 프로토콜 상세 사양 및 구현 전략
> 참고: `terminal-emulation.md` 섹션 3의 개요를 기반으로 한 심층 문서

---

## 목차

1. [프로토콜 비교 요약](#1-프로토콜-비교-요약)
2. [Kitty Graphics Protocol 상세](#2-kitty-graphics-protocol-상세)
3. [iTerm2 Inline Image Protocol 상세](#3-iterm2-inline-image-protocol-상세)
4. [Sixel Graphics 상세](#4-sixel-graphics-상세)
5. [Ghostty 구현 분석](#5-ghostty-구현-분석)
6. [alacritty_terminal 확장 전략](#6-alacritty_terminal-확장-전략)
7. [Crux 구현 아키텍처](#7-crux-구현-아키텍처)
8. [참고 자료](#8-참고-자료)

---

## 1. 프로토콜 비교 요약

### 1.1 기능 비교 매트릭스

| 기능 | Kitty Graphics | iTerm2 (OSC 1337) | Sixel |
|------|---------------|-------------------|-------|
| **이스케이프 시퀀스** | APC (`ESC _G`) | OSC (`ESC ]1337`) | DCS (`ESC P...q`) |
| **이미지 포맷** | PNG, RGB, RGBA | 모든 macOS 지원 포맷 | 자체 6-pixel 인코딩 |
| **색상 깊이** | 트루컬러 (32-bit RGBA) | 트루컬러 | 256색 (팔레트) |
| **알파 채널** | O | O | X |
| **청크 전송** | O (`m=0/1`) | O (MultipartFile, 3.5+) | X (단일 스트림) |
| **파일 경로 전송** | O (`t=f`) | X | X |
| **공유 메모리** | O (`t=s`) | X | X |
| **이미지 ID/재사용** | O (ID + placement) | X | X |
| **Z-인덱스 레이어링** | O | X | X |
| **애니메이션** | O (프레임 기반) | O (GIF만) | X |
| **Unicode placeholder** | O (U+10EEEE) | X | X |
| **상대 배치** | O (parent-child) | X | X |
| **응답 프로토콜** | O (OK/ERROR) | X | X |
| **tmux passthrough** | 부분적 | O (MultipartFile) | O (`--enable-sixel`) |
| **채택 터미널 수** | 증가 중 (~10+) | 중간 (~8+) | 가장 넓음 (~25+) |

### 1.2 구현 우선순위 (Crux)

| 순위 | 프로토콜 | 근거 |
|------|---------|------|
| **1순위** | Kitty Graphics | 가장 현대적, 기능 풍부, Metal GPU 렌더링과 최적 호환 |
| **2순위** | iTerm2 OSC 1337 | macOS 생태계 호환성, imgcat 도구 지원 |
| **3순위** | Sixel | 레거시 호환, tmux 공식 지원, 넓은 도구 생태계 |

---

## 2. Kitty Graphics Protocol 상세

> 공식 사양: https://sw.kovidgoyal.net/kitty/graphics-protocol/

### 2.1 APC 시퀀스 형식

```
ESC _ G <control_data> ; <payload> ESC \
```

- **control_data**: 쉼표로 구분된 `key=value` 쌍
- **payload**: Base64 인코딩된 바이너리 데이터
- **ESC \\**: String Terminator (ST)

### 2.2 전송 모드 (Transmission Medium)

`t` 키로 지정:

| 값 | 모드 | 설명 | 사용 시나리오 |
|----|------|------|-------------|
| `d` | Direct | Base64 데이터를 이스케이프 시퀀스에 인라인 포함 | 원격 SSH, 범용 |
| `f` | File | 터미널이 로컬 파일 경로를 읽음 | 로컬 앱 최적화 |
| `t` | Temp file | 임시 파일 (터미널이 읽은 후 자동 삭제) | 일회성 이미지 |
| `s` | Shared memory | POSIX 공유 메모리 객체 | 최고 성능 (zero-copy) |

**Direct 모드 청크 전송 프로토콜:**

```
# 첫 청크: 모든 제어 데이터 포함
ESC_Ga=T,f=100,s=800,v=600,i=1,m=1;<base64_chunk_1>ESC\

# 중간 청크: m 키만 필요
ESC_Gm=1;<base64_chunk_2>ESC\

# 마지막 청크
ESC_Gm=0;<base64_chunk_n>ESC\
```

- 청크 크기: 최대 4096 바이트 (Base64 인코딩 후)
- 4의 배수 권장 (마지막 청크 제외)
- 첫 청크에만 전체 제어 데이터 필요, 이후 청크는 `m` (과 선택적으로 `q`)만 지정

### 2.3 이미지 포맷

`f` 키로 지정:

| 값 | 포맷 | 필수 추가 키 |
|----|------|------------|
| `24` | 24-bit RGB (3 bytes/pixel) | `s` (너비), `v` (높이) |
| `32` | 32-bit RGBA (4 bytes/pixel, 기본값) | `s` (너비), `v` (높이) |
| `100` | PNG | 자동 추출 (크기 지정 불필요) |

압축: `o=z` (RFC 1950 ZLIB deflate). 압축은 Base64 인코딩 전에 적용. PNG 포맷에서 압축 사용 시 `S` 키로 압축된 데이터 크기 지정 필요.

### 2.4 액션 (Actions)

`a` 키로 지정:

| 값 | 액션 | 설명 |
|----|------|------|
| `T` | Transmit & Display | 이미지 데이터 전송 + 즉시 배치 (기본값) |
| (없음) | Transmit only | 데이터만 저장, 나중에 `a=p`로 표시 |
| `p` | Display (place) | 기존 저장된 이미지를 배치 |
| `d` | Delete | 이미지/배치 삭제 |
| `f` | Frame transmit | 애니메이션 프레임 데이터 전송 |
| `a` | Animation control | 애니메이션 재생/정지/루프 제어 |
| `c` | Compose frames | 프레임 간 영역 합성 |
| `q` | Query | 터미널 지원 여부 확인 (이미지 저장하지 않음) |

### 2.5 이미지 식별 체계

```
i = Image ID     (1 ~ 4,294,967,295, 전송 간 영구 유지)
I = Image Number (비고유, 터미널이 실제 ID 할당)
p = Placement ID (동일 이미지의 개별 배치 인스턴스 식별)
```

**터미널 응답:**
```
ESC_Gi=<id>;OESC\                    # 성공
ESC_Gi=<id>;ERROR:<message>ESC\      # 오류
```

응답 억제: `q=1` (성공 응답 억제), `q=2` (오류 응답 억제)

### 2.6 배치 매개변수 (Placement)

| 키 | 용도 | 범위 |
|----|------|------|
| `c` | 표시 열 수 (셀 너비) | 양의 정수 |
| `r` | 표시 행 수 (셀 높이) | 양의 정수 |
| `x` | 소스 직사각형 왼쪽 오프셋 (픽셀) | 0+ |
| `y` | 소스 직사각형 위쪽 오프셋 (픽셀) | 0+ |
| `w` | 소스 직사각형 너비 (픽셀) | 1+ |
| `h` | 소스 직사각형 높이 (픽셀) | 1+ |
| `X` | 셀 내 X 오프셋 (픽셀) | 0 ~ cell_width-1 |
| `Y` | 셀 내 Y 오프셋 (픽셀) | 0 ~ cell_height-1 |
| `z` | Z-인덱스 (스태킹 순서) | INT32_MIN ~ INT32_MAX |
| `C` | 커서 이동 정책 | 0=이동(기본), 1=이동 안함 |

**Z-인덱스 레이어링 규칙:**
- `z >= 0`: 텍스트 위에 렌더링
- `z < 0`: 텍스트 아래에 렌더링
- `z < -1,073,741,824`: 비기본 배경색 아래에도 렌더링

### 2.7 삭제 모드

`a=d`일 때 `d` 키로 삭제 범위 지정. 소문자는 이미지 데이터 보존, 대문자는 데이터까지 해제:

| 값 | 대상 |
|----|------|
| `a`/`A` | 모든 가시 배치 |
| `i`/`I` | 특정 ID의 이미지 (선택적으로 `p`로 특정 배치만) |
| `n`/`N` | 번호로 지정된 최신 이미지 |
| `c`/`C` | 현재 커서 위치와 교차하는 이미지 |
| `p`/`P` | 좌표 `x`,`y`와 교차하는 이미지 |
| `q`/`Q` | 좌표 `x`,`y` + 특정 `z` 인덱스 |
| `r`/`R` | ID가 `x`~`y` 범위인 이미지 |
| `x`/`X` | 열 `x`와 교차 |
| `y`/`Y` | 행 `y`와 교차 |
| `z`/`Z` | 특정 Z-인덱스 `z` |

### 2.8 Unicode Placeholder (v0.28.0+)

Unicode 문자 U+10EEEE를 이미지 플레이스홀더로 사용. 그래픽스 프로토콜을 모르는 유니코드 인식 앱에서도 이미지 표시 가능.

```
# 플레이스홀더로 배치 생성
ESC_Ga=p,U=1,i=<image_id>,c=<columns>,r=<rows>ESC\
```

- 이미지 ID는 전경색(foreground color)에 인코딩
- 행/열은 Unicode 결합 분음 부호 (U+0305~)로 지정
- tmux 등 중간자 프로그램에서 투명하게 전달 가능

### 2.9 상대 배치 (v0.31.0+)

배치를 다른 배치에 상대적으로 위치 지정:

```
ESC_Ga=p,i=<id>,P=<parent_img>,Q=<parent_placement>ESC\
```

- `H`/`V`: 수평/수직 셀 오프셋
- 부모 삭제 시 자식도 연쇄 삭제
- 체인 깊이 최소 8 지원
- 순환 참조 시 `ECYCLE` 오류 반환

### 2.10 애니메이션 지원 (v0.20.0+)

**프레임 전송** (`a=f`):
- 기본 이미지 위에 프레임 추가
- `x`,`y`,`s`,`v`로 부분 영역 지정
- `Y`로 배경 캔버스 색상 (RGBA), `c`로 이전 프레임 참조
- `X=1`: 대체 (replace), 기본값: 알파 블렌딩
- `z`: 프레임 간 딜레이 (밀리초)

**애니메이션 제어** (`a=a`):
- `s=1`: 정지, `s=2`: 로딩 모드, `s=3`: 루프 재생
- `v`: 루프 횟수 (0=무시, 1=무한, N=N-1회 반복)

### 2.11 지원 감지 (Feature Detection)

```
# 쿼리 전송 + DA1 요청
ESC_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAAEESC\ESC[c
```

DA1 응답 전에 그래픽스 쿼리 응답이 오면 프로토콜 지원 확인.

### 2.12 스토리지 및 할당량

- 세션별 이미지 스토리지 제한 (일반적으로 256MB+)
- 배치 없는 이미지는 할당량 초과 시 우선 삭제
- DoS 방지를 위한 합리적 할당량 적용

---

## 3. iTerm2 Inline Image Protocol 상세

> 공식 문서: https://iterm2.com/documentation-images.html

### 3.1 프로토콜 형식

**원본 방식 (모든 버전):**
```
ESC ] 1337 ; File = [args] : <base64_data> BEL
```

**멀티파트 방식 (v3.5+, tmux 호환):**
```
ESC ] 1337 ; MultipartFile = [args] BEL
ESC ] 1337 ; FilePart = <base64_chunk> BEL
...
ESC ] 1337 ; FileEnd BEL
```

종결자: BEL (0x07) 또는 ST (`ESC \`)

### 3.2 매개변수

| 매개변수 | 설명 | 기본값 |
|---------|------|--------|
| `name` | Base64 인코딩된 파일명 | "Unnamed file" |
| `size` | 파일 크기 (바이트, 진행 표시용) | - |
| `width` | 렌더링 너비 | auto |
| `height` | 렌더링 높이 | auto |
| `preserveAspectRatio` | 종횡비 유지 (0=무시, 1=유지) | 1 |
| `inline` | 인라인 표시 (0=다운로드, 1=표시) | 0 |

### 3.3 크기 단위

| 형식 | 의미 | 예시 |
|------|------|------|
| `N` | 문자 셀 수 | `width=80` |
| `Npx` | 픽셀 | `width=640px` |
| `N%` | 세션 너비/높이의 퍼센트 | `width=50%` |
| `auto` | 원본 크기 | `width=auto` |

### 3.4 지원 포맷

macOS 호환 포맷: PNG, JPEG, GIF (애니메이션 포함), PDF, PICT, BMP, TIFF 등. 이미지가 아닌 파일은 다운로드 폴더로 저장.

### 3.5 imgcat 호환성

```bash
# imgcat 기본 사용법 (iTerm2 유틸리티)
imgcat image.png

# 크기 지정
imgcat --width 40 --height 20 image.png

# 파이프 입력
curl -s https://example.com/image.png | imgcat
```

Crux에서 imgcat 호환을 위해 OSC 1337 파서 구현 필요. `inline=1`인 경우에만 화면에 표시.

### 3.6 Retina 디스플레이 처리

iTerm2 v3.2.0+는 Retina 디스플레이를 올바르게 처리. Crux는 macOS 전용이므로 GPUI의 `scale_factor`를 활용하여 고해상도 이미지를 네이티브 해상도로 렌더링해야 함.

### 3.7 Kitty 프로토콜 대비 한계

| 기능 | iTerm2 | Kitty |
|------|--------|-------|
| 이미지 ID/재사용 | X | O |
| Z-인덱스 레이어링 | X | O |
| 파일/공유 메모리 전송 | X | O |
| 부분 영역 표시 | X | O |
| 응답 프로토콜 | X | O |
| 애니메이션 (프레임 제어) | X (GIF 자동 재생만) | O |

iTerm2 프로토콜은 단순성이 장점. "이미지 한 장 보여주기"에 최적화되어 있으며 복잡한 이미지 관리가 필요 없는 사용 사례에 적합.

---

## 4. Sixel Graphics 상세

> VT3xx 사양: https://vt100.net/docs/vt3xx-gp/chapter14.html
> 호환성: https://www.arewesixelyet.com/

### 4.1 DCS 시퀀스 형식

```
DCS P1 ; P2 ; P3 q <sixel_data> ST
```

- **DCS**: Device Control String (`ESC P` 또는 C1 제어 문자 0x90)
- **ST**: String Terminator (`ESC \` 또는 C1 제어 문자 0x9C)
- **q**: Sixel 식별자

### 4.2 DCS 매개변수

| 매개변수 | 설명 | 값 |
|---------|------|-----|
| P1 | 픽셀 종횡비 (세로:가로) | 0,1=2:1, 2=5:1, 3,4=3:1, 5,6=2:1, 7,8,9=1:1 |
| P2 | 배경 처리 | 0,2=0 픽셀을 현재 배경색으로, 1=기존 색 유지 |
| P3 | 수평 그리드 크기 | VT300에서는 무시 (고정 0.0195cm) |

### 4.3 Sixel 데이터 인코딩

각 Sixel 문자는 6개의 수직 픽셀을 나타냄. 문자 범위: `?` (0x3F) ~ `~` (0x7E).

```
비트값 = 문자코드 - 0x3F

? (0x3F) = 000000  (모든 픽셀 off)
@ (0x40) = 000001  (최상위 1픽셀 on)
A (0x41) = 000010  (두 번째 픽셀 on)
~ (0x7E) = 111111  (모든 픽셀 on)
```

**최하위 비트(LSB)가 최상위 픽셀**에 매핑됨.

### 4.4 제어 기능

| 기호 | 이름 | 형식 | 설명 |
|------|------|------|------|
| `!` | Graphics Repeat Introducer | `!Pn<char>` | Sixel 문자를 Pn번 반복 |
| `"` | Raster Attributes | `"Pan;Pad;Ph;Pv` | 종횡비(Pan/Pad)와 이미지 크기(Ph x Pv) |
| `#` | Color Introducer | `#Pc` 또는 `#Pc;Pu;Px;Py;Pz` | 색상 선택/정의 |
| `$` | Graphics Carriage Return | - | 현재 Sixel 행의 왼쪽으로 복귀 |
| `-` | Graphics New Line | - | 다음 Sixel 행으로 이동 |

### 4.5 색상 레지스터

```
# 색상 선택만
#Pc

# 색상 정의 + 선택
#Pc;Pu;Px;Py;Pz
```

| 매개변수 | 설명 |
|---------|------|
| Pc | 색상 번호 (0-255) |
| Pu | 색 공간 (1=HLS, 2=RGB) |
| Px, Py, Pz | HLS: 색조(0-360), 명도(0-100), 채도(0-100) / RGB: R(0-100), G(0-100), B(0-100) |

**주의**: RGB 값은 0-255가 아닌 0-100 퍼센트 단위.

### 4.6 Sixel Display Mode (DECSDM)

```
CSI ? 80 h    # 활성화: 하단 마진에서 스크롤
CSI ? 80 l    # 비활성화: 하단 마진 넘어가면 무시
```

### 4.7 Sixel 인코딩 예시

```
# 빨간색 2x6 픽셀 블록
ESC P 0;0;0 q
" 1;1;2;6          # 종횡비 1:1, 크기 2x6
# 0;2;100;0;0      # 색상 0 = 빨강 (RGB)
~~                  # 2픽셀 너비, 6픽셀 높이 모두 on
ESC \
```

### 4.8 한계와 문제점

| 한계 | 설명 |
|------|------|
| **256색 제한** | 팔레트 기반, 트루컬러 불가 |
| **알파 채널 없음** | 투명도 미지원 |
| **대역폭 비효율** | Base64보다 큰 인코딩 오버헤드 |
| **이미지 ID 없음** | 재사용/관리 메커니즘 없음 |
| **6-pixel 행 단위** | 세로 해상도가 6의 배수에 제한 |

### 4.9 현재 터미널 지원 현황 (2026년)

**지원하는 주요 터미널:**
foot, WezTerm, iTerm2, Konsole, mintty, mlterm, VS Code Terminal, xterm (VT340 모드), tmux (`--enable-sixel`), Contour, xfce-terminal, Zellij

**미지원 주요 터미널:**
Alacritty, Ghostty (미등록), Kitty (자체 프로토콜 사용), Terminal.app, GNOME Terminal, PuTTY

### 4.10 libsixel vs 직접 구현

| 구현 방식 | 장점 | 단점 |
|----------|------|------|
| **libsixel** (C 라이브러리) | 성숙한 구현, 다양한 이미지 포맷 변환, 디더링 알고리즘 내장 | C FFI 오버헤드, 추가 의존성, 메모리 관리 경계 |
| **직접 Rust 구현** | zero-copy 가능, Rust 메모리 안전성, 의존성 최소화 | 구현 시간, 디코더만 필요 (인코더 불필요) |

**Crux 권장**: Sixel은 3순위이므로 초기에는 간단한 Rust 디코더만 구현. 복잡한 기능은 후순위.

---

## 5. Ghostty 구현 분석

> 소스: https://github.com/ghostty-org/ghostty
> 아키텍처 참조: https://deepwiki.com/ghostty-org/ghostty

### 5.1 아키텍처 개요

Ghostty는 Zig로 작성된 크로스 플랫폼 터미널 에뮬레이터. 핵심 라이브러리 `libghostty`가 터미널 에뮬레이션, 폰트 처리, 렌더링을 담당하고, GUI는 플랫폼별 네이티브 코드:

```
libghostty (Zig, 크로스 플랫폼)
    ├── VT Parser (SIMD 최적화)
    ├── Terminal State (Screen, PageList, Page)
    ├── Font Shaping
    └── Renderer abstraction
         ├── Metal (macOS) ← Swift AppKit/SwiftUI
         └── OpenGL (Linux) ← Zig GTK4
```

### 5.2 VT 파서 파이프라인

```
PTY Output (raw bytes)
    ↓
Parser (상태 기계 + SIMD 최적화)
    ↓ stream.nextSlice() - UTF-8을 SIMD로 고속 처리
    ↓ 제어 시퀀스 발견 시 스칼라 처리로 폴백
Stream Handler (디스패치)
    ↓ print(), execute(), csiDispatch(), oscDispatch(), apcDispatch()
Terminal State (그리드, 커서, 모드, 색상)
    ↓ mutex로 보호
Renderer Thread (별도 스레드)
    ↓ dirty flag로 변경 영역만 다시 그림
GPU Backend (Metal/OpenGL)
```

**SIMD 최적화**: `stream.nextSlice()` 함수가 UTF-8 바이트를 SIMD 명령어로 벡터 처리. 제어 시퀀스가 발견될 때까지 고속으로 일반 텍스트를 처리하고, 이스케이프 시퀀스를 만나면 파서 상태 기계로 전환.

### 5.3 Kitty Graphics Protocol 구현 상태

Ghostty는 Kitty Graphics Protocol을 지원하지만 여전히 미해결 이슈가 있음 (2025년 8월 기준):

| 이슈 | 상태 | 설명 |
|------|------|------|
| #6711 | 미해결 | 기존 이미지 ID로 로드+표시 시 교체 안됨 |
| #6710 | 미해결 | 이미지 스태킹 구현 오류 |
| #6709 | 미해결 | 애니메이션 pause/load frame/composite frame 미구현 |
| #5255 | 미해결 | 애니메이션 프레임 지원 |
| #4323 | 미해결 | 텍스트 스크롤 시 Kitty 이미지가 함께 스크롤되지 않음 |
| #2197 | 미해결 | 자동 생성 이미지 ID가 공개 ID 범위 사용 |

**핵심 교훈**: 이미지 스크롤 동기화(#4323)와 ID 관리(#2197)는 구현 초기에 올바르게 설계해야 할 핵심 과제.

### 5.4 렌더링 파이프라인 (멀티 스레드)

```
I/O Thread                          Renderer Thread
───────────                         ────────────────
PTY read() →                        draw timer (8ms)
  mutex lock →                        mutex lock →
    processOutput() →                   updateFrame() (상태 diff)
      VT parse →                      mutex unlock →
      terminal state update →           GPU 커맨드 실행
  mutex unlock →                      (Metal/OpenGL)
  renderer wakeup signal →
```

**설계 원칙:**
- 뮤텍스 임계 구간 최소화 (파싱/업데이트만)
- dirty flag로 변경 없는 영역 렌더링 스킵
- 스타일/하이퍼링크 참조 카운팅으로 중복 방지
- Arena 할당으로 효율적 메모리 관리

### 5.5 libghostty-vt 라이브러리

Ghostty에서 추출된 독립 라이브러리. 제로 의존성 C ABI:

- SIMD 최적화 파싱
- 우수한 Unicode 지원
- 고도로 최적화된 메모리 사용
- 퍼징 및 Valgrind 테스트
- Kitty Graphics Protocol 파싱 지원
- tmux Control Mode 지원

**Crux와의 관련성**: 현재 Crux는 `alacritty_terminal`을 사용하지만, `libghostty-vt`가 안정화되면 마이그레이션 후보. 특히 Kitty Graphics 내장 지원이 큰 장점.

---

## 6. alacritty_terminal 확장 전략

### 6.1 현재 한계

`alacritty_terminal` 크레이트는 그래픽스 프로토콜을 **전혀 지원하지 않음**:

- Kitty Graphics: 미지원
- Sixel: 미지원 (PR #4763 제출되었으나 미병합)
- iTerm2 Images: 미지원
- Alacritty 프로젝트 자체가 그래픽스 프로토콜 도입에 소극적

### 6.2 확장 접근법

Crux가 `alacritty_terminal` 위에서 그래픽스를 지원하기 위한 전략:

#### 접근법 A: VT 이벤트 핸들러 확장 (권장)

```rust
// alacritty_terminal의 EventListener trait을 확장
pub trait CruxEventListener: EventListener {
    /// APC 시퀀스 수신 시 호출 (Kitty Graphics)
    fn apc_dispatch(&mut self, data: &[u8]);

    /// DCS 시퀀스 수신 시 호출 (Sixel)
    fn dcs_dispatch(&mut self, params: &Params, data: &[u8]);

    /// OSC 시퀀스 수신 시 호출 (iTerm2 Images)
    fn osc_dispatch(&mut self, params: &[&[u8]]);
}
```

**장점**: alacritty_terminal 소스 수정 불필요, 깔끔한 분리
**단점**: alacritty_terminal이 APC/DCS를 소비하고 전달하지 않을 수 있음

#### 접근법 B: PTY 출력 프리프로세서

```rust
/// PTY 출력을 alacritty_terminal에 전달하기 전에 그래픽스 시퀀스를 추출
struct GraphicsPreprocessor {
    state: PreprocessorState,
    image_store: Arc<Mutex<ImageStore>>,
}

impl GraphicsPreprocessor {
    /// PTY 바이트 스트림에서 그래픽스 시퀀스를 분리
    fn process(&mut self, input: &[u8]) -> (Vec<u8>, Vec<GraphicsCommand>) {
        // APC (ESC _G...), DCS (ESC P..q...), OSC 1337 시퀀스 감지
        // 그래픽스 시퀀스는 추출, 나머지는 alacritty_terminal로 전달
        todo!()
    }
}
```

**장점**: alacritty_terminal에 대한 의존성 완전 분리
**단점**: 이중 파싱 오버헤드, 시퀀스 경계 처리 복잡

#### 접근법 C: alacritty_terminal 포크

alacritty_terminal을 포크하여 그래픽스 훅을 직접 추가.

**장점**: 완전한 제어, 최적 성능
**단점**: 업스트림 업데이트 병합 부담, 유지보수 비용

### 6.3 권장 전략

**Phase 1**: 접근법 B (프리프로세서) 로 시작. 최소 침습적이며 빠르게 프로토타이핑 가능.

**Phase 2**: 성능 프로파일링 후 병목이 되면 접근법 C (포크) 로 전환.

**장기**: `libghostty-vt`가 안정화되면 마이그레이션 평가. Kitty Graphics가 내장되어 있어 프리프로세서 레이어가 불필요.

---

## 7. Crux 구현 아키텍처

### 7.1 전체 데이터 흐름

```
PTY Output (raw bytes)
    ↓
GraphicsPreprocessor
    ├── 일반 VT 시퀀스 → alacritty_terminal (기존 파이프라인)
    └── 그래픽스 시퀀스 → GraphicsCommandParser
                              ↓
                         GraphicsCommand (enum)
                              ↓
                         ImageStore (이미지 저장소)
                              ↓
                         CruxTerminalElement (GPUI 렌더러)
                              ↓
                         Metal GPU (텍스처 렌더링)
```

### 7.2 핵심 타입 설계

```rust
/// 그래픽스 프로토콜 구분
#[derive(Debug, Clone)]
pub enum GraphicsProtocol {
    Kitty,
    Iterm2,
    Sixel,
}

/// 파싱된 그래픽스 커맨드
#[derive(Debug)]
pub enum GraphicsCommand {
    /// Kitty: 이미지 전송 (청크 가능)
    KittyTransmit {
        image_id: Option<u32>,
        format: ImageFormat,
        transmission: TransmissionMode,
        action: KittyAction,
        placement: PlacementParams,
        compression: Option<Compression>,
        chunk: ChunkInfo,
        data: Vec<u8>,
    },
    /// Kitty: 이미지 배치/표시
    KittyPlace {
        image_id: u32,
        placement_id: Option<u32>,
        placement: PlacementParams,
    },
    /// Kitty: 이미지/배치 삭제
    KittyDelete {
        target: DeleteTarget,
        free_data: bool,
    },
    /// Kitty: 애니메이션 프레임
    KittyAnimateFrame { /* ... */ },
    /// Kitty: 애니메이션 제어
    KittyAnimateControl { /* ... */ },
    /// Kitty: 지원 쿼리
    KittyQuery { image_id: u32 },
    /// iTerm2: 인라인 이미지
    Iterm2Image {
        name: Option<String>,
        size: Option<usize>,
        width: DimensionSpec,
        height: DimensionSpec,
        preserve_aspect_ratio: bool,
        inline: bool,
        data: Vec<u8>,
    },
    /// Sixel: 비트맵 이미지
    SixelImage {
        params: SixelParams,
        data: Vec<u8>, // 디코딩된 RGBA 픽셀 데이터
    },
}

#[derive(Debug, Clone)]
pub enum ImageFormat {
    Rgb,        // f=24
    Rgba,       // f=32
    Png,        // f=100
}

#[derive(Debug, Clone)]
pub enum TransmissionMode {
    Direct,         // t=d
    File,           // t=f
    TempFile,       // t=t
    SharedMemory,   // t=s
}

#[derive(Debug, Clone)]
pub struct PlacementParams {
    pub columns: Option<u32>,        // c
    pub rows: Option<u32>,           // r
    pub src_x: u32,                  // x
    pub src_y: u32,                  // y
    pub src_width: Option<u32>,      // w
    pub src_height: Option<u32>,     // h
    pub cell_offset_x: u32,         // X
    pub cell_offset_y: u32,         // Y
    pub z_index: i32,               // z
    pub do_not_move_cursor: bool,   // C=1
}
```

### 7.3 ImageStore 설계

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 이미지 저장소 - 전송된 이미지 데이터와 배치 정보 관리
pub struct ImageStore {
    /// Image ID → 디코딩된 이미지 데이터
    images: HashMap<u32, StoredImage>,
    /// 청크 조립 버퍼 (전송 중인 이미지)
    pending_chunks: HashMap<u32, ChunkAssembler>,
    /// 메모리 사용량 추적
    total_bytes: usize,
    /// 할당량 (기본 256MB)
    quota: usize,
    /// GPU 텍스처 캐시 (GPUI와 연동)
    texture_cache: HashMap<u32, GpuTextureHandle>,
}

pub struct StoredImage {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub data: Vec<u8>,          // RGBA 픽셀 데이터
    pub placements: HashMap<u32, Placement>,
    pub created_at: Instant,
}

struct ChunkAssembler {
    image_id: u32,
    control_data: KittyControlData,
    chunks: Vec<Vec<u8>>,
    total_size: usize,
}
```

### 7.4 GPUI Metal 렌더링 통합

```
CruxTerminalElement::paint()
    ↓
1. 일반 셀 그리드 렌더링 (기존)
    ↓
2. 이미지 레이어 렌더링:
   for placement in image_store.visible_placements(viewport) {
       if placement.z_index < 0 {
           // 텍스트 아래 레이어에 렌더링
           render_image_below_text(placement);
       }
   }
    ↓
3. 텍스트 렌더링 (기존)
    ↓
4. 이미지 오버레이 렌더링:
   for placement in image_store.visible_placements(viewport) {
       if placement.z_index >= 0 {
           // 텍스트 위 레이어에 렌더링
           render_image_above_text(placement);
       }
   }
```

**GPUI 이미지 렌더링 핵심:**

```rust
// GPUI의 paint 컨텍스트에서 이미지 렌더링
fn render_image_placement(
    &self,
    cx: &mut PaintContext,
    placement: &Placement,
    image: &StoredImage,
) {
    // 1. 셀 좌표 → 픽셀 좌표 변환
    let origin = self.cell_to_pixel(placement.cursor_row, placement.cursor_col);

    // 2. 표시 크기 계산 (셀 단위 또는 원본 크기)
    let display_size = self.calculate_display_size(placement, image);

    // 3. 소스 영역 추출 (부분 표시인 경우)
    let src_rect = placement.source_rect(image.width, image.height);

    // 4. GPUI ImageSource로 변환하여 렌더링
    //    Metal 텍스처 캐시 활용
    let bounds = Bounds::new(origin, display_size);
    cx.paint_image(bounds, src_rect, &image.gpu_texture);
}
```

### 7.5 메모리 관리 전략

| 계층 | 저장소 | 생명주기 |
|------|--------|---------|
| **청크 버퍼** | `Vec<u8>` | 청크 조립 완료까지 (일시적) |
| **디코딩된 이미지** | `Vec<u8>` (RGBA) | 이미지 삭제 또는 할당량 초과까지 |
| **GPU 텍스처** | Metal Texture | 이미지 삭제 또는 GPU 메모리 회수까지 |
| **배치 메타데이터** | `Placement` struct | 배치 삭제까지 |

**할당량 관리:**
- 기본 할당량: 256MB (디코딩된 이미지 기준)
- 초과 시: 배치 없는 이미지 우선 삭제 (LRU 순서)
- GPU 텍스처: lazy 생성 (첫 렌더링 시), 뷰포트 밖 이미지는 텍스처 해제 가능

### 7.6 스크롤 동기화

Ghostty의 미해결 이슈 #4323에서 배운 교훈. 이미지가 텍스트와 함께 스크롤되어야 함:

```rust
/// 배치의 화면 위치는 셀 그리드 좌표로 관리
/// alacritty_terminal의 스크롤과 자동으로 동기화
struct Placement {
    /// 배치가 시작되는 그리드 행 (스크롤백 포함 절대 행 번호)
    grid_row: usize,
    /// 배치가 시작되는 그리드 열
    grid_col: usize,
    // ...
}

/// 뷰포트 계산 시 이미지 가시성 판단
fn visible_placements(&self, viewport: Range<usize>) -> Vec<&Placement> {
    self.images.values()
        .flat_map(|img| img.placements.values())
        .filter(|p| {
            let end_row = p.grid_row + p.rows as usize;
            p.grid_row < viewport.end && end_row > viewport.start
        })
        .collect()
}
```

### 7.7 구현 로드맵

| 단계 | 내용 | 의존성 |
|------|------|--------|
| **4.1** | `GraphicsPreprocessor` - PTY 출력에서 그래픽스 시퀀스 분리 | `crux-terminal` |
| **4.2** | Kitty 커맨드 파서 - APC 시퀀스 파싱 | 4.1 |
| **4.3** | `ImageStore` - 이미지 저장, 청크 조립, 할당량 관리 | 4.2 |
| **4.4** | GPUI 이미지 렌더링 - `CruxTerminalElement`에 이미지 페인팅 추가 | 4.3, `crux-terminal-view` |
| **4.5** | Kitty 응답 프로토콜 - PTY에 OK/ERROR 응답 쓰기 | 4.2 |
| **4.6** | 지원 감지 (Query action) | 4.5 |
| **4.7** | iTerm2 OSC 1337 파서 + 렌더링 | 4.4 |
| **4.8** | Sixel 디코더 + 렌더링 | 4.4 |
| **4.9** | 애니메이션 지원 (Kitty 프레임) | 4.3, 4.4 |
| **4.10** | Unicode placeholder 지원 | 4.3 |

---

## 8. 참고 자료

### 프로토콜 사양
- [Kitty Graphics Protocol 공식 사양](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
- [iTerm2 Images Documentation](https://iterm2.com/documentation-images.html)
- [VT3xx Sixel Graphics (Chapter 14)](https://vt100.net/docs/vt3xx-gp/chapter14.html)
- [Sixel - Wikipedia](https://en.wikipedia.org/wiki/Sixel)

### 호환성/현황
- [Are We Sixel Yet?](https://www.arewesixelyet.com/)
- [libsixel](https://saitoha.github.io/libsixel/)

### Ghostty 참조
- [Ghostty GitHub](https://github.com/ghostty-org/ghostty)
- [Ghostty Features](https://ghostty.org/docs/features)
- [Ghostty Kitty Graphics Issues (Meta #8272)](https://github.com/ghostty-org/ghostty/issues/8272)
- [libghostty-vt PR #8840](https://github.com/ghostty-org/ghostty/pull/8840)
- [Libghostty Is Coming - Mitchell Hashimoto](https://mitchellh.com/writing/libghostty-is-coming)
- [Ghostty Terminal Emulation Architecture (DeepWiki)](https://deepwiki.com/ghostty-org/ghostty/3-terminal-emulation)

### Alacritty 그래픽스 논의
- [Alacritty Sixel Issue #910](https://github.com/alacritty/alacritty/issues/910)
- [Alacritty Graphics PR #4763](https://github.com/alacritty/alacritty/pull/4763)

### 관련 도구
- [ratatui-image](https://lib.rs/crates/ratatui-image) - Rust TUI 이미지 위젯 (Sixel/Kitty/iTerm2)
- [imgcat (iTerm2)](https://iterm2.com/utilities/imgcat) - 인라인 이미지 표시 CLI
