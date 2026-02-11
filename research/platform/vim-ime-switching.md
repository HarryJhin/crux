---
title: "Vim IME 자동 전환 — 커서 모양 감지 기반"
description: "Vim Normal/Insert 모드 전환 시 한영 IME 자동 전환 구현 전략. DECSCUSR 시퀀스 감지, macOS TIS API, save/restore 패턴"
phase: [3, 5]
topics: [ime, vim, korean, cursor-shape, decscusr, input-source, tis-api, macos]
related:
  - platform/ime-clipboard.md
  - core/keymapping.md
  - core/terminal-emulation.md
  - core/config-system.md
---

# Vim IME 자동 전환 — 커서 모양 감지 기반 구현 전략

> 작성일: 2026-02-12
> 목적: Vim에서 Normal/Insert 모드 전환 시 한영 IME를 자동으로 전환하는 기능의 구현 전략 수립
> 핵심 가치: **한국어 개발자의 Vim 사용 경험을 근본적으로 개선하는 킬러 피처**

---

## 1. 문제 정의

### 1.1 현재 상황

한국어(또는 다른 CJK 언어)를 사용하는 Vim 사용자는 모드 전환 시마다 IME를 수동으로 전환해야 한다:

```
Insert 모드 (한글 입력) → Esc → Normal 모드 → 한영키 눌러서 영문 전환 → j/k/dd 등 명령
Normal 모드 (영문) → i → Insert 모드 → 한영키 눌러서 한글 전환 → 한글 입력
```

이 과정에서 발생하는 문제:
- **매번 수동 전환**: 하루에 수백~수천 번의 불필요한 한영 전환
- **실수로 인한 오입력**: Normal 모드에서 한글이 입력되면 `ㅓㅓㅓ` 같은 무의미한 문자 발생
- **흐름 단절**: 코딩 집중력이 IME 전환으로 인해 지속적으로 끊김
- **학습 장벽**: Vim 입문자에게 추가적인 인지 부하

### 1.2 이상적인 동작

```
Insert 모드 (한글 입력) → Esc → Normal 모드 [자동으로 ABC 전환] → 바로 명령 입력
Normal 모드 → i → Insert 모드 [자동으로 이전 한글 IME 복원] → 바로 한글 입력
```

터미널 에뮬레이터 레벨에서 이를 구현하면, **Vim 플러그인 없이** 모든 Vim/Neovim/vi-mode 셸에서 작동한다.

### 1.3 왜 터미널 레벨인가

기존 해결책(im-select, vim-macos-ime 등)은 Vim 플러그인으로 `InsertLeave`/`InsertEnter` 이벤트를 후킹한다. 그러나:

- Vim에서만 동작 (zsh vi-mode, fish vi-mode 등에서는 별도 설정 필요)
- 플러그인 설치/설정 필요
- 외부 CLI 도구(`im-select`, `macism`) 의존
- Neovim/Vim 버전별 호환성 이슈

**터미널 에뮬레이터가 DECSCUSR를 감지하면**, 어떤 프로그램이든 커서 모양을 변경하는 순간 IME가 자동 전환된다.

---

## 2. DECSCUSR 커서 모양 감지

### 2.1 DECSCUSR 시퀀스 사양

DECSCUSR(DEC Set Cursor Style)은 VT520에서 정의된 제어 시퀀스로, 커서 모양을 변경한다:

| 시퀀스 | 커서 모양 | 일반적 용도 |
|--------|-----------|-------------|
| `\e[0 q` | 기본값(보통 블링킹 블록) | 리셋 |
| `\e[1 q` | 블링킹 블록 | Normal 모드 기본값 |
| `\e[2 q` | 고정 블록 | Normal 모드 |
| `\e[3 q` | 블링킹 밑줄 | Replace 모드 (일부 설정) |
| `\e[4 q` | 고정 밑줄 | Replace 모드 |
| `\e[5 q` | 블링킹 바(bar) | Insert 모드 (일반적) |
| `\e[6 q` | 고정 바(bar) | Insert 모드 |

### 2.2 Vim/Neovim의 커서 모양 설정

대부분의 Vim 사용자는 `.vimrc` 또는 `init.vim`에 다음과 같은 설정을 사용한다:

```vim
" Vim
let &t_SI = "\e[5 q"   " Insert 모드: 블링킹 바
let &t_EI = "\e[2 q"   " Normal 모드: 고정 블록
let &t_SR = "\e[3 q"   " Replace 모드: 블링킹 밑줄
```

```lua
-- Neovim (guicursor 기본값이 이미 DECSCUSR 사용)
vim.opt.guicursor = "n-v-c:block,i-ci-ve:ver25,r-cr-o:hor20"
```

Neovim은 **기본적으로** 모드별 커서 모양 변경을 활성화한다.

### 2.3 VT 파서에서의 감지 위치

`alacritty_terminal`의 VT 파서는 CSI 시퀀스를 `Handler` 트레이트의 콜백으로 전달한다. DECSCUSR는 CSI 시퀀스의 일종이므로 기존 파서 인프라를 활용할 수 있다:

```
PTY 출력 → VT 파서 → CSI 핸들러 → DECSCUSR 감지 → CursorShape 이벤트 발생
```

`alacritty_terminal`에서 DECSCUSR는 `Handler::set_cursor_shape()` 또는 유사한 콜백을 통해 처리된다. Crux는 이 콜백에서 추가로 IME 전환 이벤트를 발생시키면 된다.

### 2.4 커서 모양 → 모드 매핑

기본 매핑 전략:

| 커서 모양 | 추정 모드 | IME 동작 |
|-----------|-----------|----------|
| 블록 (0, 1, 2) | Normal/Visual | → ABC(영문)로 전환 |
| 바 (5, 6) | Insert | → 이전 IME 복원 |
| 밑줄 (3, 4) | Replace | → 이전 IME 복원 |

이 매핑은 설정 파일에서 사용자가 커스터마이즈할 수 있어야 한다.

---

## 3. macOS Input Source 전환 API

### 3.1 Text Input Source Services (TIS) API

macOS Carbon 프레임워크의 Text Input Source Services가 핵심 API:

```c
// 현재 키보드 입력 소스 가져오기
extern TISInputSourceRef
TISCopyCurrentKeyboardInputSource(void);

// 입력 소스 선택(전환)
extern OSStatus
TISSelectInputSource(TISInputSourceRef inputSource);

// 사용 가능한 입력 소스 목록 가져오기
extern CFArrayRef
TISCreateInputSourceList(
    CFDictionaryRef properties,    // 필터 조건 (NULL이면 전체)
    Boolean includeAllInstalled);  // 설치만 된 것도 포함?

// 입력 소스 속성 가져오기
extern void*
TISGetInputSourceProperty(
    TISInputSourceRef inputSource,
    CFStringRef propertyKey);      // e.g. kTISPropertyInputSourceID
```

### 3.2 주요 속성 키

```c
kTISPropertyInputSourceID          // "com.apple.keylayout.ABC" 같은 고유 ID
kTISPropertyInputSourceCategory    // 카테고리 (키보드/입력기)
kTISPropertyInputSourceType        // 타입
kTISPropertyInputSourceIsASCIICapable  // ASCII 입력 가능 여부
kTISPropertyInputSourceIsEnabled   // 활성화 여부
kTISPropertyInputSourceIsSelected  // 현재 선택 여부
kTISPropertyLocalizedName          // 지역화된 이름 ("ABC", "한국어 - 2벌식")
kTISPropertyInputSourceLanguages   // 지원 언어 배열
```

### 3.3 한국어 관련 Input Source ID

| Input Source ID | 설명 |
|-----------------|------|
| `com.apple.keylayout.ABC` | 기본 영문 (추천) |
| `com.apple.keylayout.US` | US 키보드 |
| `com.apple.inputmethod.Korean.2SetKorean` | 2벌식 한글 |
| `com.apple.inputmethod.Korean.3SetKorean` | 3벌식 최종 |
| `com.apple.inputmethod.Korean.390Sebulshik` | 3벌식 390 |
| `com.apple.inputmethod.Korean.GongjinCheong` | 공진청 |
| `org.youknowone.inputmethod.Gureum.*` | 구름 입력기 (서드파티) |

### 3.4 TISSelectInputSource의 CJKV 버그

macOS에는 **악명 높은 버그**가 있다: `TISSelectInputSource()`로 CJKV 입력 소스를 전환하면 메뉴바 아이콘은 바뀌지만 **실제 입력 소스는 전환되지 않는** 경우가 있다.

**macism의 워크어라운드**:
`macism` 도구는 이 버그를 우회하기 위해 임시 윈도우를 생성하고 포커스를 전환하는 트릭을 사용한다:

1. `TISSelectInputSource()`로 입력 소스 전환 요청
2. 보이지 않는 임시 윈도우 생성
3. 해당 윈도우로 포커스 이동
4. 원래 윈도우로 포커스 복귀
5. 이 과정에서 macOS가 실제로 입력 소스를 전환

이 워크어라운드가 Crux에서도 필요한지 검증이 필요하다. 터미널 에뮬레이터 자체가 NSApplication의 키 윈도우이므로, 동일 앱 내에서의 전환은 더 안정적일 수 있다.

### 3.5 스레드 안전성

- `TISSelectInputSource()`는 **반드시 메인 스레드**에서 호출해야 한다
- GPUI의 이벤트 루프는 메인 스레드에서 실행되므로, VT 파서 이벤트를 메인 스레드 콜백으로 전달하는 패턴이 필요
- `alacritty_terminal`의 PTY 읽기는 별도 스레드에서 실행될 수 있으므로, 채널이나 이벤트 큐를 통해 메인 스레드로 전달

---

## 4. 구현 아키텍처

### 4.1 전체 흐름

```
[PTY 출력 스트림]
    │
    ▼
[VT 파서 (alacritty_terminal)]
    │  CSI Ps SP q (DECSCUSR) 감지
    ▼
[CursorShapeChanged 이벤트]
    │
    ▼
[IME 전환 컨트롤러]
    │  커서 모양 → 모드 매핑
    │  디바운싱 (rapid switching 방지)
    ▼
[macOS TIS API 호출] ← 반드시 메인 스레드
    │
    ▼
[입력 소스 전환 완료]
```

### 4.2 Save/Restore 패턴

Normal 모드 진입 시 현재 IME를 저장하고, Insert 모드 복귀 시 복원하는 패턴:

```rust
struct ImeSwitchState {
    /// Insert 모드에서 사용하던 입력 소스 ID
    saved_input_source: Option<String>,
    /// Normal 모드에서 사용할 영문 입력 소스 ID
    normal_mode_source: String,  // 기본: "com.apple.keylayout.ABC"
    /// 현재 감지된 커서 모양
    current_cursor_shape: CursorShape,
    /// 마지막 전환 시각 (디바운싱용)
    last_switch_time: Instant,
    /// 기능 활성화 여부
    enabled: bool,
}

impl ImeSwitchState {
    fn on_cursor_shape_changed(&mut self, shape: CursorShape) {
        if !self.enabled { return; }
        if shape == self.current_cursor_shape { return; }

        // 디바운싱: 50ms 이내 연속 전환 무시
        if self.last_switch_time.elapsed() < Duration::from_millis(50) {
            return;
        }

        let old_shape = self.current_cursor_shape;
        self.current_cursor_shape = shape;
        self.last_switch_time = Instant::now();

        match (Self::is_normal_mode(old_shape), Self::is_normal_mode(shape)) {
            // Insert → Normal: 현재 IME 저장 후 영문 전환
            (false, true) => {
                self.saved_input_source = Some(get_current_input_source_id());
                select_input_source(&self.normal_mode_source);
            }
            // Normal → Insert: 저장된 IME 복원
            (true, false) => {
                if let Some(ref source) = self.saved_input_source {
                    select_input_source(source);
                }
            }
            // 같은 카테고리 내 전환 (Normal→Normal, Insert→Insert): 무시
            _ => {}
        }
    }

    fn is_normal_mode(shape: CursorShape) -> bool {
        matches!(shape, CursorShape::Block | CursorShape::BlinkingBlock)
    }
}
```

### 4.3 디바운싱 전략

빠른 연속 전환(예: `i` 입력 직후 `Esc`)에서 IME가 불필요하게 왔다갔다 하는 것을 방지:

- **최소 간격**: 50ms (기본값, 설정 가능)
- **큐잉 없음**: 마지막 요청만 반영 (intermediate 전환 무시)
- **비동기 전환**: TIS API 호출이 메인 스레드를 블로킹하지 않도록 주의

### 4.4 Rust에서 TIS API 호출

`core-foundation` 크레이트와 직접 FFI를 조합한 접근:

```rust
use core_foundation::base::*;
use core_foundation::string::*;
use core_foundation::array::*;
use core_foundation::dictionary::*;
use std::ptr;

// Carbon TIS 함수 extern 선언
extern "C" {
    fn TISCopyCurrentKeyboardInputSource() -> *mut __CFData;  // TISInputSourceRef
    fn TISSelectInputSource(source: *mut __CFData) -> i32;    // OSStatus
    fn TISCreateInputSourceList(
        properties: *const __CFDictionary,
        include_all: bool,
    ) -> *mut __CFArray;
    fn TISGetInputSourceProperty(
        source: *mut __CFData,
        key: *const __CFString,
    ) -> *mut core_foundation::base::CFTypeRef;

    // 속성 키 상수
    static kTISPropertyInputSourceID: *const __CFString;
}

/// 현재 입력 소스 ID를 가져온다
fn get_current_input_source_id() -> String {
    unsafe {
        let source = TISCopyCurrentKeyboardInputSource();
        if source.is_null() { return String::new(); }

        let id_ptr = TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
        if id_ptr.is_null() {
            CFRelease(source as *const _);
            return String::new();
        }

        let cf_str = id_ptr as *const __CFString;
        let result = CFString::wrap_under_get_rule(cf_str).to_string();
        CFRelease(source as *const _);
        result
    }
}

/// 지정된 ID의 입력 소스로 전환한다
fn select_input_source(source_id: &str) {
    unsafe {
        let cf_id = CFString::new(source_id);
        let key = CFString::wrap_under_get_rule(kTISPropertyInputSourceID);

        let filter = CFDictionary::from_CFType_pairs(&[
            (key.as_CFType(), cf_id.as_CFType())
        ]);

        let sources = TISCreateInputSourceList(
            filter.as_concrete_TypeRef(),
            false,
        );

        if sources.is_null() { return; }

        let array = CFArray::wrap_under_create_rule(sources);
        if array.len() > 0 {
            let source = array.get(0);
            TISSelectInputSource(source as *mut _);
        }
    }
}
```

> **참고**: 실제 구현 시 `objc2` 크레이트 생태계의 `objc2-input-method-kit` 사용을 검토할 것.
> 또한 `core-foundation-sys` 크레이트의 타입을 사용하여 더 안전한 FFI 바인딩이 가능할 수 있다.

---

## 5. 엣지 케이스 및 주의사항

### 5.1 Vim 이외의 프로그램

DECSCUSR를 사용하는 프로그램은 Vim만이 아니다:

| 프로그램 | DECSCUSR 사용 | 커서 변경 의미 |
|----------|---------------|----------------|
| Vim/Neovim | O | Normal/Insert/Replace 모드 |
| zsh (vi-mode) | O | vi-cmd/vi-ins 모드 |
| fish (vi-mode) | O | default/insert 모드 |
| bash (set -o vi) | 일부 | vi-cmd/vi-ins |
| tmux | 패스스루 | 내부 프로그램의 커서 전달 |
| htop, less, man | X (일반적) | N/A |
| fzf | 일부 | 검색 모드 |

**전략**: 커서 모양이 변경되는 모든 경우에 IME 전환을 트리거하되, 사용자가 프로그램별로 비활성화할 수 있는 옵션 제공.

### 5.2 프로그램별 비활성화

셸 통합(OSC 133) 또는 `TERM_PROGRAM` 감지와 연동하여, 특정 프로그램에서만 IME 자동 전환을 활성화하는 옵션:

```toml
[ime.auto_switch]
enabled = true
# 특정 프로그램에서만 활성화 (빈 배열이면 모든 프로그램)
only_programs = ["vim", "nvim"]
# 또는 특정 프로그램에서 비활성화
exclude_programs = ["htop", "less"]
```

그러나 터미널 에뮬레이터가 현재 실행 중인 프로그램을 정확히 알기 어렵다는 한계가 있다. OSC 133 셸 통합이 활성화된 경우에만 프로그램 감지가 가능하다.

### 5.3 다중 한글 입력기

사용자가 2벌식, 3벌식, 구름 입력기 등 다양한 한글 입력기를 사용할 수 있다. Save/Restore 패턴에서 **입력 소스 ID를 그대로 저장**하므로, 어떤 입력기든 정확히 복원된다.

### 5.4 입력 소스 전환 실패

`TISSelectInputSource()`가 실패할 수 있는 경우:
- 해당 입력 소스가 비활성화됨
- 입력 소스가 제거됨 (구름 입력기 삭제 등)
- CJKV 버그 (§3.4 참고)
- 권한 문제 (일반적으로 없음, 터미널 앱은 접근성 권한 불필요)

**처리 전략**: 실패 시 로그 출력, 재시도 없음. 사용자에게 상태바로 현재 IME 표시.

### 5.5 tmux 내부에서의 동작

tmux는 DECSCUSR를 **패스스루**한다. 즉, tmux 내부의 Vim이 DECSCUSR를 출력하면, 외부 터미널(Crux)까지 전달된다. 따라서 tmux 사용 시에도 IME 자동 전환이 동작한다.

단, tmux의 `terminal-overrides` 설정에서 DECSCUSR가 활성화되어 있어야 한다:
```
set -ga terminal-overrides ',xterm-crux:Ss=\E[%p1%d q:Se=\E[2 q'
```

### 5.6 SSH 세션

SSH를 통해 원격 서버의 Vim을 사용할 때도 DECSCUSR가 터미널까지 전달되므로, IME 자동 전환이 정상 동작한다. 이것은 Vim 플러그인 방식 대비 큰 장점이다 (원격 서버에 플러그인 설치 불필요).

---

## 6. 다른 터미널의 접근 방식

### 6.1 iTerm2

iTerm2는 직접적인 IME 자동 전환 기능을 제공하지 않는다. 대신:
- 프로필(Profile) 기반 커서 모양 설정 제공
- DECSCUSR 시퀀스 지원하여 Vim 커서 모양 변경 반영
- IME 전환은 사용자가 `im-select` + Vim 플러그인으로 구현

### 6.2 Kitty

Kitty도 자체 IME 전환 기능은 없다:
- DECSCUSR 완전 지원
- Kitty keyboard protocol로 확장 키 이벤트 지원
- kitten(확장 프로그램) 시스템으로 IME 관련 확장 요청 있으나 미구현 (issue #469)
- CJKV 입력 소스 전환 관련 버그 리포트 다수 (issue #4219, #8131)

### 6.3 Ghostty

Ghostty의 접근:
- DECSCUSR 지원
- 한글 입력 관련 이슈 진행 중 (discussions #5312 — Backspace 문제)
- 자체 IME 자동 전환 기능 없음
- macOS 네이티브 IME 통합에 집중

### 6.4 Alacritty

Alacritty는 최소주의 접근:
- DECSCUSR 지원
- 자체 IME 전환 기능 없음
- 외부 도구 (`im-select`, `macism`) 의존
- 가장 많은 "IME 자동 전환" 요청이 있는 터미널 중 하나

### 6.5 Zed Editor (참고)

에디터이지만 참고할 만한 접근:
- Vim 모드에서 Normal/Visual 모드 진입 시 IME를 **자동으로 무시** (바이패스)
- Insert 모드에서만 IME 활성화
- 터미널이 아닌 에디터 레벨에서 구현하므로, Vim의 `InsertLeave`/`InsertEnter` 이벤트 직접 활용
- 최근 버그 수정(#41766)으로 이 기능이 일시적으로 작동하지 않아 사용자 불만 발생

### 6.6 im-select / macism (CLI 도구)

| 도구 | 언어 | 특징 |
|------|------|------|
| `im-select` | C++/ObjC | 간단한 get/set CLI. CJKV 전환 불안정 |
| `macism` | Swift | CJKV 전환 안정적 (워크어라운드 포함). 임시 윈도우 트릭 사용 |
| `vim-macos-ime` | VimL | Vim 플러그인. `macism` 호출 |
| `im-select.nvim` | Lua | Neovim 플러그인. `im-select` 또는 `macism` 호출 |

**Crux의 차별점**: 이들 CLI 도구를 **내장**하여 외부 의존성 없이 동작. DECSCUSR 감지를 통해 **Vim 플러그인 없이** 자동 전환.

---

## 7. 설정 스키마

### 7.1 TOML 설정

```toml
[ime]
# IME 자동 전환 기능 전체 활성화/비활성화
auto_switch = true

# Normal 모드에서 사용할 입력 소스
# 기본값: 시스템의 첫 번째 ASCII 입력 소스 (보통 "com.apple.keylayout.ABC")
normal_mode_source = "com.apple.keylayout.ABC"

# 커서 모양별 모드 매핑
# "normal" = 영문으로 전환, "insert" = 저장된 IME 복원, "ignore" = 무시
[ime.cursor_mapping]
block = "normal"           # \e[1 q, \e[2 q
bar = "insert"             # \e[5 q, \e[6 q
underline = "insert"       # \e[3 q, \e[4 q

# 디바운싱 간격 (밀리초)
debounce_ms = 50

# 상태바에 현재 IME 표시
show_indicator = true
```

### 7.2 런타임 토글

키바인딩으로 IME 자동 전환을 즉시 토글할 수 있어야 한다:

```toml
[keybinding]
"ctrl-shift-i" = "toggle_ime_auto_switch"
```

---

## 8. 구현 로드맵

### Phase 3 (Korean/CJK IME)

1. **기본 DECSCUSR 감지**: `alacritty_terminal`의 커서 모양 변경 이벤트를 Crux 이벤트 시스템으로 전달
2. **TIS API 바인딩**: Rust FFI로 `TISCopyCurrentKeyboardInputSource`, `TISSelectInputSource` 호출
3. **Save/Restore 로직**: `ImeSwitchState` 구현
4. **CJKV 버그 워크어라운드**: macism의 임시 윈도우 트릭이 필요한지 검증
5. **기본 설정**: `config.toml`에 `[ime]` 섹션 추가

### Phase 5 (Config & Polish)

6. **프로그램별 설정**: OSC 133 셸 통합과 연동하여 프로그램 감지
7. **상태바 인디케이터**: 현재 IME 상태를 터미널 UI에 표시
8. **키바인딩**: 런타임 토글 단축키
9. **진단 명령**: `crux ime-status` CLI로 현재 설정 및 상태 확인

### 테스트 전략

- **단위 테스트**: `ImeSwitchState`의 상태 전이 로직
- **통합 테스트**: PTY에 DECSCUSR 시퀀스를 쓰고, IME 전환 이벤트가 발생하는지 확인
- **수동 테스트 매트릭스**:
  - Vim + 2벌식 / 3벌식 / 구름
  - Neovim + 2벌식
  - zsh vi-mode + 2벌식
  - tmux 내 Vim + 2벌식
  - SSH 원격 Vim + 2벌식

---

## 9. 경쟁 우위 분석

| 기능 | Crux | iTerm2 | Kitty | Ghostty | Alacritty |
|------|------|--------|-------|---------|-----------|
| DECSCUSR 기반 IME 자동 전환 | **O (내장)** | X | X | X | X |
| Vim 플러그인 불필요 | **O** | X | X | X | X |
| SSH 원격에서도 동작 | **O** | X* | X* | X* | X* |
| CJKV 전환 안정성 | **내장 워크어라운드** | N/A | N/A | N/A | N/A |
| 프로그램별 설정 | **O** | N/A | N/A | N/A | N/A |

*\* im-select + Vim 플러그인 조합으로 부분적으로 가능하지만, 원격 서버에도 플러그인 설치 필요*

이 기능은 **한국어/CJK 개발자가 Crux를 선택해야 하는 강력한 이유**가 된다. 어떤 주류 터미널 에뮬레이터도 이 기능을 내장하고 있지 않다.

---

## 참고 자료

- [im-select](https://github.com/daipeihust/im-select) — CLI 입력 소스 전환 도구
- [macism](https://github.com/laishulu/macism) — CJKV 안정 전환 도구 (Swift)
- [vim-macos-ime](https://github.com/laishulu/vim-macos-ime) — Vim IME 자동 전환 플러그인
- [im-select.nvim](https://github.com/keaising/im-select.nvim) — Neovim IME 자동 전환 플러그인
- [kawa](https://github.com/hatashiro/kawa) — macOS 입력 소스 전환 유틸리티 (Swift, TIS API 참조 구현)
- [TextInputSources.h](https://github.com/phracker/MacOSX-SDKs/blob/master/MacOSX10.6.sdk/System/Library/Frameworks/Carbon.framework/Versions/A/Frameworks/HIToolbox.framework/Versions/A/Headers/TextInputSources.h) — macOS TIS API 헤더
- [DECSCUSR 사양](https://vim.fandom.com/wiki/Change_cursor_shape_in_different_modes) — 커서 모양 변경 이스케이프 시퀀스
- [Zed IME 토론](https://github.com/zed-industries/zed/discussions/42439) — Zed의 IME 자동 전환 논의
