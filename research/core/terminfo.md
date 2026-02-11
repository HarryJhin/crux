---
title: "Terminfo 항목 생성 연구"
description: "Terminfo source format, existing terminal analysis, compilation/installation, TERM strategy, modern capabilities for Crux"
date: 2026-02-12
phase: [1]
topics: [terminfo, term-env, capabilities, tic, ncurses]
status: final
related:
  - terminal-emulation.md
  - keymapping.md
---

# Crux 터미널 에뮬레이터 Terminfo 항목 생성 연구

## 목차
1. [terminfo 소스 형식 (.ti 파일)](#1-terminfo-소스-형식-ti-파일)
2. [기존 터미널 에뮬레이터 terminfo 분석](#2-기존-터미널-에뮬레이터-terminfo-분석)
3. [컴파일 및 설치](#3-컴파일-및-설치)
4. [TERM 환경변수 전략](#4-term-환경변수-전략)
5. [현대적 capability 목록](#5-현대적-capability-목록)
6. [crux.terminfo 초안](#6-cruxterminfo-초안)

---

## 1. terminfo 소스 형식 (.ti 파일)

### 1.1 기본 구조

terminfo 소스 파일은 터미널의 capability(능력)를 기술하는 텍스트 파일이다. 각 항목은 쉼표(`,`)로 구분된 필드들로 구성되며, 첫 번째 필드는 터미널의 이름(들)을 정의한다.

```
# 주석은 '#'으로 시작
터미널이름|별명|긴 설명,
    capability1, capability2=값,
    capability3#숫자,
```

**핵심 규칙:**
- 첫 번째 필드는 반드시 첫 번째 컬럼에서 시작
- 이후 줄들은 탭이나 공백으로 들여쓰기
- 각 capability는 쉼표로 구분
- `#`으로 시작하는 줄은 주석
- 이름 필드에서 `|`로 별명 구분, 마지막 이름은 긴 설명

### 1.2 세 가지 capability 유형

| 유형 | 형식 | 예시 | 설명 |
|------|------|------|------|
| **Boolean** | `이름` | `am`, `bce`, `XT` | 있으면 해당 기능 지원 |
| **Numeric** | `이름#값` | `cols#80`, `colors#0x100` | 정수값을 가진 capability |
| **String** | `이름=값` | `el=\EK`, `cup=\E[%i%p1%d;%p2%dH` | 이스케이프 시퀀스 등 문자열 |

### 1.3 `use=` 지시어 (상속)

`use=` 지시어를 통해 다른 terminfo 항목을 상속받을 수 있다. 이는 `xterm-256color` 같은 기존 정의를 기반으로 확장할 때 핵심이다.

```
crux|crux terminal emulator,
    use=xterm-256color,
    # 여기에 추가/재정의할 capability들
```

**상속 규칙:**
- `use=` 앞에 정의된 capability가 상속된 것을 덮어씀 (override)
- 여러 `use=`를 사용할 수 있으며, 역순으로 처리됨
- `capability@` 형식으로 상속된 capability를 취소(cancel)할 수 있음

### 1.4 파라미터화된 문자열

terminfo는 printf와 유사한 스택 기반 파라미터 시스템을 사용한다:

| 코드 | 의미 |
|------|------|
| `%p1` | 첫 번째 매개변수를 스택에 push |
| `%d` | 스택 top을 10진수로 출력 |
| `%c` | 스택 top을 문자로 출력 |
| `%s` | 스택 top을 문자열로 출력 |
| `%{n}` | 정수 n을 스택에 push |
| `%+`, `%-`, `%*`, `%/` | 산술 연산 |
| `%?...%t...%e...%;` | if-then-else 조건문 |
| `%i` | 첫 두 매개변수에 1 더하기 (1-based 변환) |

**예시 - `cup` (커서 위치 지정):**
```
cup=\E[%i%p1%d;%p2%dH
```
→ `\E[` + (row+1) + `;` + (col+1) + `H`

### 1.5 조각(Fragment) 패턴

Alacritty에서 사용하는 패턴으로, 공통 capability를 별도 조각으로 분리:

```
# 공통 조각 (이름에 '+' 포함)
alacritty+common|alacritty common capabilities,
    am, bce, ...

# 256색 버전
alacritty|alacritty terminal emulator,
    use=alacritty+common,
    colors#0x100, ...

# Direct color 버전
alacritty-direct|alacritty with direct color indexing,
    use=alacritty+common,
    RGB, colors#0x1000000, ...
```

---

## 2. 기존 터미널 에뮬레이터 terminfo 분석

### 2.1 Alacritty (`alacritty.info`)

**소스:** [`extra/alacritty.info`](https://github.com/alacritty/alacritty/blob/master/extra/alacritty.info)

**구조:**
- `alacritty+common`: 공통 capability 조각 (독립형, 시스템 의존성 없음)
- `alacritty`: 256색 모드 (`colors#0x100`)
- `alacritty-direct`: Direct color 모드 (`colors#0x1000000`, `RGB`)

**설치 명령:**
```bash
sudo tic -xe alacritty,alacritty-direct extra/alacritty.info
```

**주요 특징:**
- 완전히 독립적인 정의 (xterm-256color에 의존하지 않음)
- `alacritty+common`에 모든 capability를 자체 정의
- `XT` boolean으로 xterm 호환성 표시
- `AX` boolean으로 aixterm 스타일 컬러 지원
- `Ss`/`Se` 커서 스타일 변경/리셋 지원
- `Smulx` 스타일 밑줄 지원
- Synchronized output 지원

**xterm-256color 대비 추가 capability:**
- `Ss=\E[%p1%d q` / `Se=\E[0 q` (커서 스타일)
- `Smulx=\E[4\:%p1%dm` (스타일 밑줄)
- Bracketed paste mode
- 포커스 이벤트

### 2.2 WezTerm (`wezterm.terminfo`)

**소스:** [`termwiz/data/wezterm.terminfo`](https://github.com/wezterm/wezterm/blob/main/termwiz/data/wezterm.terminfo)

**구조:**
- 단일 `wezterm` 항목
- `infocmp`로 xterm-256color에서 재구성 후 확장

**주요 추가 capability (xterm-256color 대비):**

| Capability | 값 | 용도 |
|-----------|-----|------|
| `Tc` | boolean | True color 지원 (tmux용) |
| `Su` | boolean | 스타일 밑줄 지원 |
| `sitm`/`ritm` | `\E[3m`/`\E[23m` | 이탤릭 시작/종료 |
| `Ms` | string | 클립보드 수정 (OSC 52) |
| `Ss`/`Se` | string | 커서 스타일 (DECSCUSR) |
| `Smulx` | `\E[4\:%p1%dm` | kitty 스타일 밑줄 |
| `Setulc` | string | 밑줄 색상 설정 |
| `hs`/`dsl`/`fsl`/`tsl` | string | 상태 표시줄 (nvim용) |

**기본 TERM:** `xterm-256color` (기본값), `wezterm` (선택적 설치)

### 2.3 Ghostty (`xterm-ghostty`)

**소스:** Ghostty 리소스 디렉토리 내 `ghostty.terminfo` (비공개 리포지토리, ncurses 6.5-20241228에 포함)

**TERM 전략:**
- `xterm-ghostty` 사용 (처음에는 `ghostty`를 시도했으나 호환성 문제로 `xterm-` 접두사 채택)
- 많은 앱이 TERM 값에서 "xterm" 문자열을 검색하여 capability 판단
- Vim 9.0 등에서 하드코딩된 터미널 목록으로 인한 호환성 문제

**핵심 기능:**
- XTGETTCAP 구현: VT 이스케이프 시퀀스로 terminfo 쿼리 가능
- terminfo 파일과 XTGETTCAP이 동일 소스에서 생성 → 항상 동기화
- ncurses 6.5-20241228+ 에 공식 등록
- `Su` (styled underlines), `Smulx`, `Setulc` 지원
- `Sync` capability 포함
- SSH 자동 terminfo 전파 기능 (`ssh-terminfo` shell integration)

### 2.4 Kitty (`xterm-kitty`)

**TERM:** `xterm-kitty`
- Kitty 자체 terminfo 정의를 관리
- xterm-kitty는 kitty가 구현하는 기능을 정확히 반영하도록 제어
- ncurses 데이터베이스 제출 논의 진행 중

**핵심 capability:**
- `Su` boolean: 스타일/컬러 밑줄 감지용
- Kitty 키보드 프로토콜 (별도 이스케이프 시퀀스 기반)
- `Smulx`: 밑줄 스타일 (`4:0`~`4:5`)
- 밑줄 색상: `CSI 58...m` (설정), `CSI 59m` (리셋)

### 2.5 Rio (`rio.terminfo`)

**소스:** [`misc/rio.terminfo`](https://github.com/raphamorim/rio/blob/main/misc/rio.terminfo)

**구조:**
- `rio+base`: 공통 capability 조각
- `xterm-rio`: `rio+base` 사용 (v0.2.28+ 기본값)
- `rio`: `xterm-rio` 확장, 색상 capability 추가

**설치:**
```bash
curl -o rio.terminfo https://raw.githubusercontent.com/raphamorim/rio/main/misc/rio.terminfo
sudo tic -xe xterm-rio,rio rio.terminfo
```

**TERM 전략 변화:**
- v0.2.27까지: `TERM=rio`
- v0.2.28부터: `TERM=xterm-rio` (xterm 접두사로 호환성 개선)

### 2.6 비교 요약

| 터미널 | TERM 값 | 독립형? | xterm 접두사 | 주요 확장 |
|--------|---------|---------|-------------|----------|
| Alacritty | `alacritty` | O (완전 독립) | X | Ss/Se, Smulx |
| WezTerm | `wezterm` / `xterm-256color` | X (재구성) | X (기본값은 xterm-256color) | Tc, Su, Ms, Smulx, Setulc |
| Ghostty | `xterm-ghostty` | O | O | Su, Smulx, Setulc, Sync, XTGETTCAP |
| Kitty | `xterm-kitty` | O | O | Su, Smulx, 키보드 프로토콜 |
| Rio | `xterm-rio` | O | O | 기본적 |

---

## 3. 컴파일 및 설치

### 3.1 `tic` 명령어 사용법

`tic` (terminfo compiler)은 terminfo 소스 파일을 컴파일된 바이너리로 변환한다.

```bash
# 기본 컴파일 (사용자 디렉토리에 설치)
tic crux.terminfo

# 특정 항목만 컴파일
tic -xe xterm-crux,crux crux.terminfo

# 확장 capability 포함 (중요!)
tic -x crux.terminfo

# 시스템 전역 설치
sudo tic -xe xterm-crux,crux crux.terminfo

# 출력 디렉토리 지정
tic -o /path/to/output crux.terminfo
```

**주요 옵션:**

| 옵션 | 설명 |
|------|------|
| `-x` | 사용자 정의 capability 포함 (Tc, Su 등 비표준 확장) |
| `-xe 이름1,이름2` | 특정 항목만 컴파일 |
| `-o 디렉토리` | 출력 디렉토리 지정 |
| `-c` | 문법 검사만 (설치하지 않음) |
| `-v` | 자세한 출력 |

**중요:** `-x` 플래그 없이는 `Tc`, `Su`, `Smulx`, `Setulc` 같은 비표준 확장 capability가 무시된다!

### 3.2 설치 경로

terminfo 파일은 해시 디렉토리 구조로 저장된다. 이름의 첫 글자를 디렉토리명으로 사용:

```
~/.terminfo/x/xterm-crux    # 사용자별 (우선순위 최고)
/usr/share/terminfo/x/xterm-crux  # 시스템 전역
/usr/lib/terminfo/x/xterm-crux    # 일부 배포판
```

**검색 순서 (ncurses):**
1. `$TERMINFO` 환경변수가 가리키는 디렉토리
2. `$HOME/.terminfo/`
3. `$TERMINFO_DIRS`에 나열된 디렉토리들
4. 시스템 기본 경로 (`/usr/share/terminfo/`)

### 3.3 앱 번들링 전략

#### macOS App Bundle

```
Crux.app/
  Contents/
    Resources/
      terminfo/
        x/
          xterm-crux
        c/
          crux
```

앱 실행 시 `TERMINFO` 환경변수를 설정:
```rust
std::env::set_var("TERMINFO", "/path/to/Crux.app/Contents/Resources/terminfo");
```

또는 `TERMINFO_DIRS`에 추가:
```rust
let dirs = format!(
    "{}:{}",
    app_terminfo_path,
    existing_terminfo_dirs.unwrap_or_default()
);
std::env::set_var("TERMINFO_DIRS", dirs);
```

#### Homebrew Formula

```ruby
class Crux < Formula
  # ...

  def install
    # 바이너리/앱 설치
    bin.install "crux"

    # terminfo 소스에서 컴파일하여 설치
    system "tic", "-x", "-o", "#{share}/terminfo", "extra/crux.terminfo"

    # 또는 미리 컴파일된 terminfo를 복사
    (share/"terminfo").install Dir["terminfo/*"]
  end

  def post_install
    # 사용자 홈 디렉토리에도 설치 (선택적)
    ohai "Installing terminfo to ~/.terminfo"
    system "tic", "-x", "-o", "#{Dir.home}/.terminfo", "#{share}/crux.terminfo"
  end

  def caveats
    <<~EOS
      terminfo has been installed to:
        #{share}/terminfo

      To install to your home directory:
        tic -x -o ~/.terminfo #{share}/crux.terminfo

      Set TERM=xterm-crux in your Crux configuration.
    EOS
  end
end
```

**참고:** 최신 트렌드는 `tic`을 실행하지 않고 미리 컴파일된 terminfo 파일을 복사하는 방식이다 (WezTerm PR #6538 참고).

---

## 4. TERM 환경변수 전략

### 4.1 옵션 비교

#### 옵션 A: `TERM=xterm-crux` (권장)

**장점:**
- "xterm" 문자열 포함 → 많은 앱이 자동으로 xterm capability를 인식
- Ghostty, Kitty, Rio 모두 이 패턴을 채택 (검증된 전략)
- 앱 호환성 극대화
- Crux만의 추가 capability 광고 가능

**단점:**
- 원격 서버에 terminfo가 없으면 "unknown terminal" 오류
- SSH 시 terminfo 전파 필요

#### 옵션 B: `TERM=crux`

**장점:**
- 깔끔한 이름
- 고유 식별

**단점:**
- 많은 앱이 "xterm"으로 시작하는 TERM만 인식 (Vim 9.0 등 하드코딩 문제)
- Ghostty가 이 접근법을 시도했다가 호환성 문제로 포기
- 앱 호환성 위험 높음

#### 옵션 C: `TERM=xterm-256color` (대안)

**장점:**
- 모든 시스템에 이미 설치됨
- 호환성 문제 없음

**단점:**
- Crux 고유 capability를 광고할 수 없음
- styled underlines, synchronized output 등 고급 기능 사용 불가
- 터미널 식별 불가

### 4.2 권장 전략: `xterm-crux` + 폴백

```
기본값: TERM=xterm-crux  (crux terminfo가 설치된 경우)
폴백:  TERM=xterm-256color  (crux terminfo가 없는 경우)
```

**구현 로직 (Rust 의사코드):**
```rust
fn determine_term() -> &'static str {
    // 1. crux terminfo가 사용 가능한지 확인
    if terminfo_available("xterm-crux") {
        "xterm-crux"
    } else {
        // 2. 폴백
        "xterm-256color"
    }
}

fn terminfo_available(term: &str) -> bool {
    // TERMINFO, ~/.terminfo, TERMINFO_DIRS, 시스템 경로에서 검색
    // 또는 setupterm() / tigetstr() 호출로 확인
    ...
}
```

### 4.3 `TERM` vs `TERM_PROGRAM` 구분

| 변수 | 용도 | 값 | 소비자 |
|------|------|-----|--------|
| `TERM` | terminfo 항목 이름 | `xterm-crux` | ncurses, 모든 TUI 앱 |
| `TERM_PROGRAM` | 실제 터미널 프로그램 이름 | `Crux` | shell 스크립트, 테마 |
| `TERM_PROGRAM_VERSION` | 버전 | `0.1.0` | 기능 감지 |
| `COLORTERM` | 트루컬러 지원 힌트 | `truecolor` | vim, nvim 등 |

**권장 환경변수 설정:**
```bash
TERM=xterm-crux
TERM_PROGRAM=Crux
TERM_PROGRAM_VERSION=0.1.0
COLORTERM=truecolor
```

### 4.4 SSH 전파 전략

원격 서버에 crux terminfo가 없을 때의 해결책:

**방법 1: 자동 전파 (Ghostty 방식)**
```bash
# shell integration에서 자동으로 실행
infocmp -x xterm-crux | ssh $HOST -- tic -x -
```

**방법 2: SSH config 폴백 (OpenSSH 8.7+)**
```
Host *
    SetEnv TERM=xterm-256color
```

**방법 3: 사용자 수동 설치**
```bash
# 로컬에서 원격으로 복사
infocmp -x xterm-crux | ssh user@host -- tic -x -
```

---

## 5. 현대적 Capability 목록

### 5.1 True Color (24비트 RGB)

| Capability | 유형 | 값/설명 |
|-----------|------|---------|
| `Tc` | boolean | tmux가 인식하는 true color 플래그 |
| `RGB` | boolean | ncurses의 직접 색상 지원 표시 |
| `setaf`/`setab` | string | 전경/배경색 설정 (RGB 지원 포함) |

**참고:** `Tc`는 tmux 전용 비공식 확장, `RGB`는 ncurses가 인식하는 공식 확장이다. 둘 다 설정하는 것이 좋다.

### 5.2 스타일 밑줄 (Styled Underlines)

| Capability | 유형 | 값 | 설명 |
|-----------|------|-----|------|
| `Su` | boolean | - | 스타일 밑줄 지원 표시 (감지용) |
| `Smulx` | string | `\E[4\:%p1%dm` | 밑줄 스타일 설정 |
| `Setulc` | string | (아래 참조) | 밑줄 색상 설정 |

**Smulx 매개변수 값:**
- `0`: 밑줄 없음
- `1`: 직선 밑줄
- `2`: 이중 밑줄
- `3`: 곡선(curly) 밑줄
- `4`: 점선 밑줄
- `5`: 파선 밑줄

**Setulc 값:**
```
Setulc=\E[58\:2\:\:%p1%{65536}%/%d\:%p1%{256}%/%{255}%&%d\:%p1%{255}%&%dm
```
→ `CSI 58:2::R:G:B m` (RGB 밑줄 색상)

밑줄 색상 리셋: `CSI 59m`

### 5.3 동기화 출력 (Synchronized Output)

| Capability | 유형 | 값 | 설명 |
|-----------|------|-----|------|
| `Sync` | string | `\E[?2026%?%p1%{1}%-%tl%eh%;` | 동기화 시작/종료 |

**이스케이프 시퀀스:**
- `CSI ? 2026 h` → 동기화 시작 (Begin Synchronized Update)
- `CSI ? 2026 l` → 동기화 종료 (End Synchronized Update)
- `CSI ? 2026 $ p` → 지원 여부 쿼리 (DECRQM)

### 5.4 SGR 마우스 모드

| Capability | 유형 | 값 | 설명 |
|-----------|------|-----|------|
| `kmous` | string | `\E[<` | SGR 마우스 이벤트 접두사 |
| `XM` | string | (아래 참조) | 마우스 모드 설정 |

**XM 값:**
```
XM=\E[?1006;1000%?%p1%{1}%=%th%el%;
```
→ SGR 마우스 (1006) + 기본 마우스 (1000) 활성화/비활성화

### 5.5 괄호 붙여넣기 (Bracketed Paste)

| Capability | 유형 | 값 | 설명 |
|-----------|------|-----|------|
| `BD` | string | `\E[?2004l` | Bracketed paste 비활성화 |
| `BE` | string | `\E[?2004h` | Bracketed paste 활성화 |
| `PE` | string | `\E[201~` | Paste 종료 표시 |
| `PS` | string | `\E[200~` | Paste 시작 표시 |

**참고:** `XT` boolean이 설정되면 `Dsbp`/`Enbp`가 자동으로 설정된다.

### 5.6 포커스 이벤트 (Focus Events)

| Capability | 유형 | 값 | 설명 |
|-----------|------|-----|------|
| `Dsfcs` | string | `\E[?1004l` | 포커스 보고 비활성화 |
| `Enfcs` | string | `\E[?1004h` | 포커스 보고 활성화 |

**참고:** `XT` boolean이 설정되면 자동으로 설정된다.

### 5.7 커서 스타일 (Cursor Style)

| Capability | 유형 | 값 | 설명 |
|-----------|------|-----|------|
| `Ss` | string | `\E[%p1%d q` | 커서 스타일 설정 (DECSCUSR) |
| `Se` | string | `\E[0 q` | 커서 스타일 기본값으로 리셋 |

**DECSCUSR 값:**
- `0`: 기본값으로 리셋
- `1`: 깜빡이는 블록
- `2`: 고정 블록
- `3`: 깜빡이는 밑줄
- `4`: 고정 밑줄
- `5`: 깜빡이는 바
- `6`: 고정 바

### 5.8 클립보드 (OSC 52)

| Capability | 유형 | 값 | 설명 |
|-----------|------|-----|------|
| `Ms` | string | `\E]52;%p1%s;%p2%s\007` | 클립보드 설정 |

**매개변수:** `%p1` = 저장소 (`c` clipboard, `p` primary), `%p2` = base64 인코딩된 내용

### 5.9 그래핌 클러스터 (Mode 2027)

현재 terminfo에 공식 capability 이름이 없다. DECRQM으로 쿼리:
```
CSI ? 2027 $ p  → 지원 여부 질의
CSI ? 2027 h    → 그래핌 클러스터 모드 활성화
CSI ? 2027 l    → 그래핌 클러스터 모드 비활성화
```

**상태:** 아직 표준화되지 않았으며, 터미널별로 구현 중. Ghostty, WezTerm 등에서 논의/구현 중.

### 5.10 Kitty 키보드 프로토콜

terminfo에 공식 capability가 없으며, 이스케이프 시퀀스로 직접 제어:
```
CSI > flags u    → 프로토콜 활성화 (flags: 비트 플래그)
CSI < u          → 프로토콜 비활성화 (스택 pop)
CSI ? u          → 현재 플래그 쿼리
```

**플래그 비트:**
- `1`: 모호한 키 구분
- `2`: 이벤트 유형 보고
- `4`: 대체 키 보고
- `8`: 모든 키를 이스케이프 코드로
- `16`: 릴리즈 이벤트 보고

### 5.11 상태 표시줄 (Status Line)

| Capability | 유형 | 값 | 설명 |
|-----------|------|-----|------|
| `hs` | boolean | - | 상태 표시줄 있음 |
| `tsl` | string | `\E]0;` | 상태 표시줄 시작 |
| `fsl` | string | `\007` | 상태 표시줄 종료 |
| `dsl` | string | `\E]0;\007` | 상태 표시줄 비활성화 |

---

## 6. crux.terminfo 초안

아래는 Crux 터미널 에뮬레이터를 위한 terminfo 소스 파일 초안이다.

파일: [`crux.terminfo`](../extra/crux.terminfo)

```terminfo
# Crux Terminal Emulator - Terminfo Source
#
# 설치 방법:
#   tic -x -e xterm-crux,crux crux.terminfo
#
# 확인:
#   infocmp -x xterm-crux
#   echo $TERM
#
# 이 파일은 두 가지 항목을 정의:
#   xterm-crux  - 기본값 (xterm 호환성을 위한 접두사)
#   crux        - 별명 (xterm-crux와 동일)
#
# 기반: xterm-256color + 현대적 확장
#
# 비공식 확장 capability (tmux, neovim 등에서 인식):
#   Tc    - true color 지원 (tmux)
#   Su    - styled underline 지원 (kitty/neovim)
#   Smulx - 밑줄 스타일 설정
#   Setulc - 밑줄 색상 설정
#   Ss/Se - 커서 스타일 설정/리셋
#   Ms    - 클립보드 설정 (OSC 52)
#   Sync  - 동기화 출력
#

# ── 공통 capability 조각 ──────────────────────────────────
crux+common|crux common capabilities,

# ── Boolean capabilities ──
# am     : 자동 우측 마진 (auto right margin)
# bce    : 배경색 지우기 사용 (back color erase)
# ccc    : 색상 재정의 가능 (can change color)
# km     : 메타 키 있음 (has meta key)
# mc5i   : 프린터가 화면에 반영되지 않음
# mir    : 삽입 모드에서 안전한 이동 (move in insert mode)
# msgr   : 강조 모드에서 안전한 이동 (move in standout mode)
# xenl   : 줄바꿈 무시 (eat newline glitch / xterm newline)
# AX     : aixterm 스타일 속성 리셋 (SGR 39/49)
# XT     : xterm 호환 (bracketed paste, focus 등 자동 설정)
# Tc     : true color 지원 (tmux용)
# Su     : styled underline 지원
	am, bce, ccc, km, mc5i, mir, msgr, xenl,
	AX, XT, Tc, Su,

# ── Numeric capabilities ──
# cols   : 기본 컬럼 수
# it     : 탭 간격
# lines  : 기본 줄 수
# colors : 지원 색상 수 (256)
# pairs  : 지원 색상 조합 수
	cols#80, it#8, lines#24,
	colors#0x100, pairs#0x7FFF,

# ── 기본 터미널 제어 ──
# bel  : 벨 (경고음)
# blink: 깜빡임 속성
# bold : 굵게 속성
# civis: 커서 숨기기
# clear: 화면 지우기
# cnorm: 커서 보이기 (정상)
# cr   : 캐리지 리턴
# csr  : 스크롤 영역 설정
# cub  : 커서 왼쪽으로 N칸
# cub1 : 커서 왼쪽으로 1칸
# cud  : 커서 아래로 N칸
# cud1 : 커서 아래로 1칸
# cuf  : 커서 오른쪽으로 N칸
# cuf1 : 커서 오른쪽으로 1칸
# cup  : 커서 위치 지정
# cuu  : 커서 위로 N칸
# cuu1 : 커서 위로 1칸
# cvvis: 커서 매우 밝게
# dch  : 문자 삭제
# dch1 : 문자 1개 삭제
# dim  : 흐리게 속성
# dl   : 줄 삭제
# dl1  : 줄 1개 삭제
# ech  : 문자 지우기
# ed   : 화면 끝까지 지우기
# el   : 줄 끝까지 지우기
# el1  : 줄 시작까지 지우기
# enacs: 대체 문자셋 활성화
# flash: 시각적 벨
# home : 커서를 홈으로
# hpa  : 수평 절대 위치
# ht   : 탭
# hts  : 탭 정지 설정
# ich  : 문자 삽입
# il   : 줄 삽입
# il1  : 줄 1개 삽입
# ind  : 스크롤 위로
# indn : N줄 스크롤 위로
# invis: 보이지 않는 속성
# is2  : 초기화 문자열
# kbs  : 백스페이스 키
# kcbt : 역탭 키
# kcub1: 왼쪽 화살표
# kcud1: 아래 화살표
# kcuf1: 오른쪽 화살표
# kcuu1: 위 화살표
# kdch1: Delete 키
# kend : End 키
# kf1~kf12: F1-F12 키
# khome: Home 키
# kich1: Insert 키
# kmous: 마우스 이벤트 접두사
# knp  : Page Down
# kpp  : Page Up
# nel  : 새줄
# op   : 기본 색상 쌍으로 리셋
# rc   : 커서 위치 복원
# rev  : 반전 속성
# ri   : 역방향 스크롤
# rin  : N줄 역방향 스크롤
# ritm : 이탤릭 종료
# rmacs: 대체 문자셋 종료
# rmam : 자동 마진 종료
# rmcup: 대체 화면 종료
# rmir : 삽입 모드 종료
# rmkx : 키패드 모드 종료
# rmso : 강조 모드 종료
# rmul : 밑줄 모드 종료
# rs1  : 터미널 리셋
# sc   : 커서 위치 저장
# setab: ANSI 배경색 설정
# setaf: ANSI 전경색 설정
# sgr  : 속성 설정 (파라미터화)
# sgr0 : 모든 속성 리셋
# sitm : 이탤릭 시작
# smacs: 대체 문자셋 시작
# smam : 자동 마진 시작
# smcup: 대체 화면 시작
# smir : 삽입 모드 시작
# smkx : 키패드 모드 시작
# smso : 강조 모드 시작
# smul : 밑줄 모드 시작
# tbc  : 모든 탭 정지 제거
# tsl  : 상태 표시줄 시작
# fsl  : 상태 표시줄 종료
# dsl  : 상태 표시줄 비활성화
# u6   : 커서 위치 보고 형식
# u7   : 커서 위치 요청
# u8   : 장치 속성 응답
# u9   : 장치 속성 요청
# vpa  : 수직 절대 위치
	bel=^G,
	blink=\E[5m, bold=\E[1m, dim=\E[2m, invis=\E[8m, rev=\E[7m,
	civis=\E[?25l, cnorm=\E[?12l\E[?25h, cvvis=\E[?12;25h,
	clear=\E[H\E[2J, ed=\E[J, el=\E[K, el1=\E[1K, ech=\E[%p1%dX,
	cr=\r, nel=\EE, ind=\n, ri=\EM,
	indn=\E[%p1%dS, rin=\E[%p1%dT,
	csr=\E[%i%p1%d;%p2%dr,
	cub=\E[%p1%dD, cub1=^H,
	cud=\E[%p1%dB, cud1=\n,
	cuf=\E[%p1%dC, cuf1=\E[C,
	cup=\E[%i%p1%d;%p2%dH,
	cuu=\E[%p1%dA, cuu1=\E[A,
	home=\E[H,
	hpa=\E[%i%p1%dG, vpa=\E[%i%p1%dd,
	dch=\E[%p1%dP, dch1=\E[P,
	dl=\E[%p1%dM, dl1=\E[M,
	ich=\E[%p1%d@,
	il=\E[%p1%dL, il1=\E[L,
	ht=\t, hts=\EH, tbc=\E[3g,
	enacs=\E(B\E)0,
	flash=\E[?5h$<100/>\E[?5l,
	is2=\E[!p\E[?3;4l\E[4l\E>,
	sc=\E7, rc=\E8,
	smacs=\E(0, rmacs=\E(B,
	smam=\E[?7h, rmam=\E[?7l,
	smcup=\E[?1049h\E[22;0;0t, rmcup=\E[?1049l\E[23;0;0t,
	smir=\E[4h, rmir=\E[4l,
	smkx=\E[?1h\E=, rmkx=\E[?1l\E>,
	smso=\E[7m, rmso=\E[27m,
	smul=\E[4m, rmul=\E[24m,
	sitm=\E[3m, ritm=\E[23m,
	sgr0=\E[m\E(B,
	sgr=\E[0%?%p6%t;1%;%?%p5%t;2%;%?%p2%t;4%;%?%p4%t;5%;%?%p7%t;8%;%?%p1%p3%|%t;7%;m%?%p9%t\E(0%e\E(B%;,

# ── 상태 표시줄 (for nvim 등) ──
	hs,
	tsl=\E]0;, fsl=\007, dsl=\E]0;\007,

# ── 색상 설정 ──
	op=\E[39;49m,
	setab=\E[%?%p1%{8}%<%t4%p1%d%e%p1%{16}%<%t10%p1%{8}%-%d%e48;5;%p1%d%;m,
	setaf=\E[%?%p1%{8}%<%t3%p1%d%e%p1%{16}%<%t9%p1%{8}%-%d%e38;5;%p1%d%;m,
	initc=\E]4;%p1%d;rgb\:%p2%{255}%*%{1000}%/%2.2X/%p3%{255}%*%{1000}%/%2.2X/%p4%{255}%*%{1000}%/%2.2X\E\\,
	oc=\E]104\007,
	rs1=\Ec\E]104\007,

# ── 커서 위치 보고 ──
	u6=\E[%i%d;%dR, u7=\E[6n, u8=\E[?%[;0123456789]c, u9=\E[c,

# ── 키 정의 ──
	kbs=\177, kcbt=\E[Z,
	kcub1=\EOD, kcud1=\EOB, kcuf1=\EOC, kcuu1=\EOA,
	kdch1=\E[3~, kend=\EOF, khome=\EOH,
	kich1=\E[2~, knp=\E[6~, kpp=\E[5~,
# F1-F12
	kf1=\EOP, kf2=\EOQ, kf3=\EOR, kf4=\EOS,
	kf5=\E[15~, kf6=\E[17~, kf7=\E[18~, kf8=\E[19~,
	kf9=\E[20~, kf10=\E[21~, kf11=\E[23~, kf12=\E[24~,
# F13-F24 (Shift+F1-F12)
	kf13=\E[1;2P, kf14=\E[1;2Q, kf15=\E[1;2R, kf16=\E[1;2S,
	kf17=\E[15;2~, kf18=\E[17;2~, kf19=\E[18;2~, kf20=\E[19;2~,
	kf21=\E[20;2~, kf22=\E[21;2~, kf23=\E[23;2~, kf24=\E[24;2~,
# F25-F36 (Ctrl+F1-F12)
	kf25=\E[1;5P, kf26=\E[1;5Q, kf27=\E[1;5R, kf28=\E[1;5S,
	kf29=\E[15;5~, kf30=\E[17;5~, kf31=\E[18;5~, kf32=\E[19;5~,
	kf33=\E[20;5~, kf34=\E[21;5~, kf35=\E[23;5~, kf36=\E[24;5~,
# F37-F48 (Ctrl+Shift+F1-F12)
	kf37=\E[1;6P, kf38=\E[1;6Q, kf39=\E[1;6R, kf40=\E[1;6S,
	kf41=\E[15;6~, kf42=\E[17;6~, kf43=\E[18;6~, kf44=\E[19;6~,
	kf45=\E[20;6~, kf46=\E[21;6~, kf47=\E[23;6~, kf48=\E[24;6~,
# F49-F63 (Meta variations)
	kf49=\E[1;3P, kf50=\E[1;3Q, kf51=\E[1;3R, kf52=\E[1;3S,
	kf53=\E[15;3~, kf54=\E[17;3~, kf55=\E[18;3~, kf56=\E[19;3~,
	kf57=\E[20;3~, kf58=\E[21;3~, kf59=\E[23;3~, kf60=\E[24;3~,
	kf61=\E[1;4P, kf62=\E[1;4Q, kf63=\E[1;4R,

# ── 마우스 지원 (SGR 모드) ──
	kmous=\E[<,
	XM=\E[?1006;1000%?%p1%{1}%=%th%el%;,
	xm=\E[<%i%p3%d;%p1%d;%p2%d;%?%p4%tM%em%;,

# ── 괄호 붙여넣기 (Bracketed Paste) ──
	BD=\E[?2004l, BE=\E[?2004h,
	PE=\E[201~, PS=\E[200~,

# ── 포커스 이벤트 ──
	Dsfcs=\E[?1004l, Enfcs=\E[?1004h,

# ── 현대적 확장 capability ──

# 커서 스타일 (DECSCUSR)
# Ss: 커서 스타일 설정 (0=기본, 1=깜빡블록, 2=고정블록, 3=깜빡밑줄, 4=고정밑줄, 5=깜빡바, 6=고정바)
# Se: 커서 스타일 기본값 리셋
	Ss=\E[%p1%d q, Se=\E[0 q,

# 스타일 밑줄 (Kitty 프로토콜)
# Smulx: 밑줄 스타일 (0=없음, 1=직선, 2=이중, 3=곡선, 4=점선, 5=파선)
	Smulx=\E[4\:%p1%dm,

# 밑줄 색상 (RGB)
# Setulc: 밑줄 색상 설정 (인수: R*65536 + G*256 + B)
	Setulc=\E[58\:2\:\:%p1%{65536}%/%d\:%p1%{256}%/%{255}%&%d\:%p1%{255}%&%dm,

# 클립보드 (OSC 52)
# Ms: 클립보드 설정 (p1=저장소, p2=base64 내용)
	Ms=\E]52;%p1%s;%p2%s\007,

# 동기화 출력 (Mode 2026)
# Sync: p1=1이면 시작(h), 아니면 종료(l)
	Sync=\E[?2026%?%p1%{1}%-%tl%eh%;,

# ── xterm-crux 항목 (기본값, 256색) ──────────────────────
xterm-crux|crux terminal emulator (256 colors),
	use=crux+common,

# ── crux 항목 (별명) ──────────────────────────────────────
crux|crux terminal emulator,
	use=xterm-crux,

# ── crux-direct 항목 (Direct/True Color 전용) ────────────
crux-direct|crux terminal emulator (direct color),
	use=crux+common,
	RGB,
	colors#0x1000000, pairs#0x7FFF,
	ccc@,
	initc@, oc@,
	op=\E[39;49m,
	setab=\E[%?%p1%{8}%<%t4%p1%d%e48\:2\:\:%p1%{65536}%/%d\:%p1%{256}%/%{255}%&%d\:%p1%{255}%&%d%;m,
	setaf=\E[%?%p1%{8}%<%t3%p1%d%e38\:2\:\:%p1%{65536}%/%d\:%p1%{256}%/%{255}%&%d\:%p1%{255}%&%d%;m,
```

### 6.1 설치 방법

```bash
# 소스에서 컴파일 및 설치 (사용자 디렉토리)
tic -x -e xterm-crux,crux,crux-direct crux.terminfo

# 시스템 전역 설치
sudo tic -x -e xterm-crux,crux,crux-direct crux.terminfo

# 확인
infocmp -x xterm-crux
```

### 6.2 설계 결정 사항

| 결정 | 선택 | 근거 |
|------|------|------|
| TERM 이름 | `xterm-crux` | xterm 접두사로 호환성 확보 (Ghostty/Kitty/Rio 선례) |
| 독립형 정의 | O | Alacritty처럼 xterm-256color 의존 없이 완전 자체 정의 |
| Fragment 패턴 | `crux+common` | 256색/direct color 변형 공유 |
| Direct color 변형 | `crux-direct` | Alacritty의 `alacritty-direct`와 동일 패턴 |
| True color | `Tc` + `RGB` | tmux(`Tc`)와 ncurses(`RGB`) 모두 지원 |
| Styled underlines | `Su` + `Smulx` + `Setulc` | nvim, tmux 등에서 활용 |
| Synchronized output | `Sync` | 화면 깜빡임 방지 |
| 클립보드 | `Ms` (OSC 52) | nvim, tmux 클립보드 연동 |
| 커서 스타일 | `Ss`/`Se` | vim/nvim 커서 형태 변경 |
| 상태 표시줄 | `hs`/`tsl`/`fsl`/`dsl` | nvim 등에서 창 제목 설정 |

### 6.3 향후 고려사항

1. **ncurses 공식 등록**: Ghostty처럼 ncurses terminfo 데이터베이스에 제출 추진
2. **XTGETTCAP 구현**: VT 이스케이프로 terminfo 쿼리 가능하게 (SSH 환경 대응)
3. **Shell integration**: SSH 접속 시 자동 terminfo 전파 기능
4. **Kitty 키보드 프로토콜**: terminfo에는 미반영이나, 앱에서 직접 감지/활성화
5. **Mode 2027 (그래핌 클러스터)**: 표준화 후 terminfo capability 추가

---

## 참고 자료

- [terminfo(5) 매뉴얼](https://www.man7.org/linux/man-pages/man5/terminfo.5.html)
- [Alacritty terminfo](https://github.com/alacritty/alacritty/blob/master/extra/alacritty.info)
- [WezTerm terminfo](https://github.com/wezterm/wezterm/blob/main/termwiz/data/wezterm.terminfo)
- [Rio terminfo](https://github.com/raphamorim/rio/blob/main/misc/rio.terminfo)
- [Ghostty terminfo 문서](https://ghostty.org/docs/help/terminfo)
- [Ghostty Devlog 004](https://mitchellh.com/writing/ghostty-devlog-004)
- [Kitty styled underlines](https://sw.kovidgoyal.net/kitty/underlines/)
- [Kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/)
- [Synchronized Output spec](https://gist.github.com/christianparpart/d8a62cc1ab659194337d73e399004036)
- [Grapheme Clusters in Terminals](https://mitchellh.com/writing/grapheme-clusters-in-terminals)
- [WezTerm TERM config](https://wezterm.org/config/lua/config/term.html)
