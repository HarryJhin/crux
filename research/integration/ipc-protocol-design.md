---
title: "Crux IPC 프로토콜 설계"
description: "Crux IPC architecture and protocol design — WezTerm/tmux CLI comparison, Claude Code Agent Teams PaneBackend interface, Crux Protocol JSON-RPC spec"
date: 2026-02-11
phase: [2, 5]
topics: [ipc, crux-protocol, pane-control, claude-code, agent-teams, json-rpc]
status: final
related:
  - ipc-external-patterns.md
  - claude-code-strategy.md
---

# IPC / CLI 페인 제어 및 Claude Code Agent Teams 통합 연구

> Crux 터미널 에뮬레이터를 위한 IPC 아키텍처 및 프로토콜 설계 연구 문서
> 작성일: 2026-02-11

---

## 목차

1. [WezTerm CLI 아키텍처](#1-wezterm-cli-아키텍처)
2. [tmux CLI 아키텍처 (참조)](#2-tmux-cli-아키텍처-참조)
3. [Claude Code Agent Teams](#3-claude-code-agent-teams)
4. [Crux IPC 설계](#4-crux-ipc-설계)
5. [Crux 프로토콜 설계](#5-crux-프로토콜-설계)
6. [참고 문헌](#6-참고-문헌)

---

## 1. WezTerm CLI 아키텍처

### 1.1 개요

WezTerm은 Rust 기반 GPU 가속 크로스플랫폼 터미널 에뮬레이터로, 내장 멀티플렉서와 풍부한 CLI 인터페이스를 제공한다. CLI 클라이언트는 Unix 도메인 소켓을 통해 GUI/서버 프로세스와 통신하며, `codec::Pdu` 프로토콜을 사용한다.

**핵심 아키텍처 특성:**
- 클라이언트-서버 모델 (GUI/mux-server가 서버, CLI가 클라이언트)
- Unix 도메인 소켓 기반 IPC
- `varbincode` (가변 길이 바이너리 인코딩) + `zstd` 압축
- 도메인 추상화 (Local, Client, RemoteSsh)

### 1.2 CLI 명령어 상세

#### `wezterm cli split-pane`

페인을 분할하고 새 페인의 ID를 stdout으로 출력한다.

```
wezterm cli split-pane [OPTIONS] [PROG]...
```

| 옵션 | 설명 |
|------|------|
| `--bottom` | 아래 방향 수직 분할 (기본값) |
| `--top` | 위 방향 수직 분할 |
| `--left` | 왼쪽 방향 수평 분할 |
| `--right` | 오른쪽 방향 수평 분할 |
| `--horizontal` | `--right`와 동일 |
| `--cells CELLS` | 새 분할 크기 (셀 단위) |
| `--percent PERCENT` | 새 분할 크기 (백분율) |
| `--pane-id PANE_ID` | 분할 대상 페인 지정 |
| `--move-pane-id MOVE_PANE_ID` | 기존 페인을 분할로 이동 |
| `--top-level` | 개별 페인이 아닌 전체 윈도우 분할 |
| `--cwd CWD` | 새 페인의 작업 디렉토리 |
| `[PROG]...` | 실행할 프로그램 (미지정시 기본 셸) |

**사용 예시:**
```bash
# 기본 하단 분할
$ wezterm cli split-pane
42  # 새 pane_id 반환

# 오른쪽 30% 크기로 분할하며 특정 명령 실행
$ wezterm cli split-pane --right --percent 30 -- claude --resume <id> --teammate

# 기존 페인을 분할로 이동
$ wezterm cli split-pane --right --move-pane-id 5
```

**Crux 설계 시사점:**
- 분할 시 새 pane_id를 반환하는 패턴은 매우 유용 → Crux도 동일하게 구현 필요
- `--top-level` 옵션으로 전체 윈도우 vs 개별 페인 분할 구분 → Crux에서도 고려
- `--move-pane-id`로 페인 재배치 가능 → 고급 기능으로 고려

#### `wezterm cli send-text`

텍스트를 특정 페인에 페이스트 방식으로 전송한다.

```
wezterm cli send-text [OPTIONS] [TEXT]
```

| 옵션 | 설명 |
|------|------|
| `--pane-id <PANE_ID>` | 대상 페인 (기본: `WEZTERM_PANE` 환경변수) |
| `--no-paste` | 브래킷 페이스트 모드 없이 직접 전송 |
| `[TEXT]` | 전송할 텍스트 (미지정시 stdin에서 읽음) |

**동작 방식:**
- 기본적으로 Bracketed Paste Mode 래핑 (`\e[200~...\e[201~`)
- `--no-paste` 플래그로 직접 키 입력처럼 전송 가능
- 텍스트 미지정 시 stdin에서 파이프로 읽기 가능

```bash
# 특정 페인에 명령 전송
$ wezterm cli send-text --pane-id 42 --no-paste "ls -la\n"

# stdin에서 읽어 전송
$ echo "hello" | wezterm cli send-text --pane-id 42
```

#### `wezterm cli get-text`

페인의 텍스트 콘텐츠를 캡처한다.

```
wezterm cli get-text [OPTIONS]
```

| 옵션 | 설명 |
|------|------|
| `--pane-id <PANE_ID>` | 대상 페인 |
| `--start-line <LINE>` | 시작 줄 (0=화면 첫줄, 음수=스크롤백) |
| `--end-line <LINE>` | 끝 줄 |
| `--escapes` | ANSI 이스케이프 시퀀스 포함 |

**Crux 설계 시사점:**
- 스크롤백 영역까지 접근 가능하게 음수 라인 번호 지원
- `--escapes` 플래그로 스타일 정보 포함/제외 선택

#### `wezterm cli list`

윈도우, 탭, 페인 목록을 출력한다.

```
wezterm cli list [--format <table|json>]
```

**테이블 출력 (기본):**
```
WINID TABID PANEID WORKSPACE SIZE    TITLE CWD
0     0     0      default   120x40 zsh   /Users/me
0     0     3      default   60x40  zsh   /Users/me
0     0     5      default   120x20 zsh   /Users/me
```

**JSON 출력:**
```json
{
  "window_id": 0,
  "tab_id": 0,
  "pane_id": 0,
  "workspace": "default",
  "size": { "rows": 24, "cols": 80 },
  "title": "zsh",
  "cwd": "file://hostname/home/user/",
  "cursor_x": 0,
  "cursor_y": 0,
  "cursor_shape": "Default",
  "cursor_visibility": "Visible",
  "is_active": true,
  "is_zoomed": false,
  "tty_name": "/dev/ttys001"
}
```

**Crux 설계 시사점:**
- JSON 출력은 프로그래밍 통합에 필수 → Crux도 반드시 지원
- `is_active`, `is_zoomed` 같은 상태 필드는 AI 에이전트 연동에 유용
- `tty_name` 필드로 PTY 식별 가능

### 1.3 멀티플렉서 서버/클라이언트 모델

#### 동작 모드

```
┌──────────────────────────────────────────────────────────┐
│                    WezTerm Architecture                    │
├──────────────────────────────────────────────────────────┤
│                                                          │
│  ┌─────────────┐     Unix Socket      ┌──────────────┐  │
│  │  CLI Client  │ ◄──────────────────► │  LocalListener│  │
│  │  (wezterm    │    codec::Pdu        │  (서버측)     │  │
│  │   cli ...)   │    varbincode+zstd   │              │  │
│  └─────────────┘                       └──────┬───────┘  │
│                                               │          │
│  ┌─────────────┐                       ┌──────▼───────┐  │
│  │  GUI Front   │ ◄─────────────────► │     Mux      │  │
│  │  (렌더링)    │    직접 호출          │  (멀티플렉서) │  │
│  └─────────────┘                       └──────┬───────┘  │
│                                               │          │
│                                  ┌────────────┼────────┐ │
│                                  │            │        │ │
│                           ┌──────▼──┐  ┌──────▼──┐     │ │
│                           │ Local   │  │ Client  │ ... │ │
│                           │ Domain  │  │ Domain  │     │ │
│                           │ (PTY)   │  │ (RPC)   │     │ │
│                           └─────────┘  └─────────┘     │ │
│                                                        │ │
└──────────────────────────────────────────────────────────┘
```

#### 소켓 위치 및 디스커버리

- GUI 모드: `WEZTERM_UNIX_SOCKET` 환경변수에 소켓 경로 저장
- Linux: `/run/user/$UID/wezterm/gui-sock-$PID`
- macOS: 유사한 런타임 디렉토리 경로
- CLI 클라이언트는 `WEZTERM_UNIX_SOCKET`을 읽어 서버를 찾음

#### 통신 흐름

1. **디스커버리**: CLI가 `WEZTERM_UNIX_SOCKET` 환경변수에서 소켓 경로 확인
2. **연결**: `wezterm_client::Client::new_unix_domain()`으로 소켓 연결
3. **RPC 교환**: `codec::Pdu` 메시지로 오퍼레이션 요청 (spawn, list, split-pane 등)
4. **응답**: 서버가 업데이트된 멀티플렉서 상태를 PDU로 인코딩하여 응답
5. **상태 동기화**: 원격 페인의 터미널 출력이 `MuxNotification::Alert`로 전달

#### 도메인 추상화

| 도메인 타입 | 역할 | 연결 방식 |
|------------|------|----------|
| `LocalDomain` | 로컬 프로세스 관리 | `LocalPane` → Terminal + PTY |
| `ClientDomain` | 원격 프록시 | `ClientPane` → RPC |
| `RemoteSshDomain` | SSH 멀티플렉싱 | SSH 터널 |

**Crux 설계 시사점:**
- WezTerm의 도메인 추상화는 좋은 참조 모델
- 단, Crux는 초기 버전에서 `LocalDomain`만 구현해도 충분
- 환경변수 기반 소켓 디스커버리 패턴은 CLI 통합에 필수

---

## 2. tmux CLI 아키텍처 (참조)

### 2.1 핵심 명령어

#### 페인 관리

```bash
# 수평 분할 (좌우)
tmux split-window -h [-p PERCENT] [-l SIZE] [-- COMMAND]

# 수직 분할 (상하)
tmux split-window -v [-p PERCENT] [-l SIZE] [-- COMMAND]

# 페인 목록
tmux list-panes [-F FORMAT]

# 페인 선택/포커스
tmux select-pane -t TARGET

# 텍스트 전송 (키 입력 시뮬레이션)
tmux send-keys -t TARGET "KEYS" Enter

# 페인 레이아웃 변경
tmux select-layout {even-horizontal,even-vertical,main-horizontal,main-vertical,tiled}
```

#### tmux vs WezTerm CLI 비교

| 오퍼레이션 | tmux | WezTerm CLI |
|-----------|------|-------------|
| 수평 분할 | `split-window -h` | `split-pane --right` |
| 수직 분할 | `split-window -v` | `split-pane --bottom` |
| 페인 목록 | `list-panes` | `list` |
| 페인 포커스 | `select-pane -t N` | `activate-pane --pane-id N` |
| 텍스트 전송 | `send-keys -t N "text"` | `send-text --pane-id N "text"` |
| 텍스트 읽기 | `capture-pane -t N -p` | `get-text --pane-id N` |
| 페인 ID 확인 | `display -p '#{pane_id}'` | `WEZTERM_PANE` 환경변수 |
| JSON 출력 | `list-panes -F '#{...}'` | `list --format json` |

### 2.2 클라이언트-서버 아키텍처

tmux는 유닉스 소켓 기반 클라이언트-서버 모델을 사용한다:

```
┌──────────────┐     Unix Socket     ┌──────────────┐
│ tmux client  │ ◄─────────────────► │ tmux server  │
│ (터미널 UI)  │   텍스트 프로토콜    │ (세션 관리)   │
└──────────────┘                     └──────┬───────┘
                                           │
                              ┌─────────────┼─────────────┐
                              │             │             │
                        ┌─────▼─────┐ ┌─────▼─────┐ ┌────▼────┐
                        │ Session 0 │ │ Session 1 │ │ ...     │
                        │  Window 0 │ │  Window 0 │ │         │
                        │   Pane 0  │ │   Pane 0  │ │         │
                        │   Pane 1  │ │   Pane 1  │ │         │
                        └───────────┘ └───────────┘ └─────────┘
```

**소켓 위치:** `/tmp/tmux-$UID/default` (기본값)

### 2.3 Control Mode 프로토콜 (`tmux -CC`)

#### 개요

Control Mode는 iTerm2의 George Nachman이 설계한 텍스트 기반 프로토콜로, 터미널 앱이 tmux와 프로그래밍적으로 인터페이스할 수 있게 한다.

#### 진입 방법

```bash
# 일반 컨트롤 모드 (echo 활성)
tmux -C new-session

# 애플리케이션용 컨트롤 모드 (iTerm2 등)
tmux -CC new-session
# → \033P1000p DCS 시퀀스 전송 (터미널이 감지 가능)
# → 종료시 %exit + \033\ (ST) 전송
```

#### 명령 / 응답 형식

모든 명령의 출력은 guard 라인으로 래핑된다:

```
# 성공
%begin TIMESTAMP CMD_NUM FLAGS
... 출력 내용 ...
%end TIMESTAMP CMD_NUM FLAGS

# 실패
%begin TIMESTAMP CMD_NUM FLAGS
... 에러 내용 ...
%error TIMESTAMP CMD_NUM FLAGS
```

#### 비동기 알림 (% 접두사)

| 알림 | 용도 |
|------|------|
| `%output %PANE TEXT` | 페인 출력 (일반 모드) |
| `%extended-output %PANE MS_BEHIND : TEXT` | 페인 출력 (플로우 컨트롤) |
| `%pane-mode-changed %PANE` | 페인 모드 변경 |
| `%window-pane-changed @WIN %PANE` | 윈도우 내 활성 페인 변경 |
| `%window-add @WIN` | 윈도우 추가 |
| `%window-close @WIN` | 윈도우 닫힘 |
| `%window-renamed @WIN NAME` | 윈도우 이름 변경 |
| `%session-changed $SESS NAME` | 세션 변경 |
| `%session-renamed $SESS NAME` | 세션 이름 변경 |
| `%sessions-changed` | 세션 생성/삭제 |
| `%pause %PANE` | 플로우 컨트롤로 일시정지 |
| `%continue %PANE` | 일시정지 해제 |
| `%subscription-changed NAME VALUE` | 구독 포맷 변경 |

#### 특수 명령

```bash
# 클라이언트 크기 설정
refresh-client -C WxH

# 플래그 설정
refresh-client -f no-output        # %output 알림 억제
refresh-client -f wait-exit        # 종료 대기
refresh-client -f pause-after=SEC  # 플로우 컨트롤

# 플로우 컨트롤 액션
refresh-client -A '%PANE:continue'
refresh-client -A '%PANE:pause'

# 포맷 구독
refresh-client -B 'name:type:format'
```

#### iTerm2 통합 방식

1. 사용자가 `tmux -CC`로 세션 시작
2. iTerm2가 DCS 시퀀스(`\033P1000p`)를 감지하여 컨트롤 모드 진입
3. tmux 윈도우/페인을 iTerm2 네이티브 탭/분할로 렌더링
4. `%output` 알림으로 페인 내용 실시간 수신
5. 사용자 입력을 tmux 명령으로 변환하여 전송
6. 연결 해제/재연결 시 세션 상태 완전 복원

**Crux 설계 시사점:**
- tmux Control Mode의 `%` 알림 패턴은 비동기 IPC의 좋은 참조
- 하지만 Crux는 텍스트 기반이 아닌 구조화된 JSON 프로토콜 사용 권장
- tmux의 플로우 컨트롤 메커니즘은 대량 출력 처리에 필수적

---

## 3. Claude Code Agent Teams

### 3.1 개요

Claude Code Agent Teams는 여러 Claude Code 인스턴스를 팀으로 조직하여 병렬로 작업할 수 있게 하는 실험적 기능이다. 현재 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` 환경변수로 활성화한다.

**아키텍처 구성요소:**

| 구성요소 | 역할 |
|---------|------|
| Team Lead | 메인 세션. 팀 생성, 태스크 분배, 결과 종합 |
| Teammates | 독립 Claude Code 인스턴스. 할당된 태스크 수행 |
| Task List | 공유 태스크 목록 (`~/.claude/tasks/{team-name}/`) |
| Mailbox | 에이전트 간 메시징 시스템 |

### 3.2 터미널 환경 감지

#### teammateMode 설정

```json
// ~/.claude/settings.json
{
  "teammateMode": "auto"  // "auto" | "tmux" | "in-process"
}
```

| 모드 | 동작 |
|------|------|
| `"auto"` (기본) | tmux 세션 내부면 split-pane, 아니면 in-process |
| `"tmux"` | split-pane 모드 강제, tmux/iTerm2 자동 감지 |
| `"in-process"` | 모든 팀메이트를 메인 터미널 내에서 실행 |

#### 터미널 감지 로직

현재 Claude Code의 터미널 백엔드 감지:

1. **tmux 감지**: `$TMUX` 환경변수 확인
2. **iTerm2 감지**: `it2` CLI 사용 가능 여부 + Python API 활성화 확인
3. **폴백**: 위 조건 불충족 시 in-process 모드

**알려진 문제:**
- iTerm2 감지가 캐시되거나 잘못된 항목을 확인하는 경우가 있음 ([#23572](https://github.com/anthropics/claude-code/issues/23572))
- pane-base-index가 0이 아닌 경우 send-keys 대상이 잘못됨 ([#23527](https://github.com/anthropics/claude-code/issues/23527))
- split-pane 모드에서 팀메이트가 초기 메일박스 메시지를 처리하지 못하는 버그 ([#24108](https://github.com/anthropics/claude-code/issues/24108))

### 3.3 tmux 백엔드에서의 CLI 호출

Claude Code가 tmux 모드에서 실제로 호출하는 명령 패턴:

```bash
# 1. 팀메이트 페인 생성
tmux split-window -h -- claude --resume <session-id> --teammate

# 2. 팀메이트에게 명령 전송
tmux send-keys -t <pane-index> "text" Enter

# 3. 페인 목록 확인
tmux list-panes -F '#{pane_id}:#{pane_title}:#{pane_active}'

# 4. 페인 포커스
tmux select-pane -t <pane-index>
```

### 3.4 새 터미널 백엔드 지원 요구사항

Claude Code가 새 터미널 백엔드를 지원하려면 다음 오퍼레이션이 필요하다:

| 오퍼레이션 | 용도 | 필수 여부 |
|-----------|------|----------|
| **Split Pane** | 팀메이트용 새 페인 생성 | 필수 |
| **Send Text/Keys** | 팀메이트에게 명령 전송 | 필수 |
| **List Panes** | 활성 페인 목록 조회 | 필수 |
| **Focus Pane** | 특정 페인 활성화 | 권장 |
| **Get Pane ID** | 현재 페인 식별 | 필수 |
| **Close Pane** | 팀메이트 종료 시 페인 정리 | 권장 |
| **환경 감지** | 터미널 종류 자동 인식 | 필수 |

### 3.5 GitHub 이슈 분석

#### Issue #23574: WezTerm 지원

- **상태**: OPEN (2026-02-06)
- **반응**: 13 👍, 4 🚀
- **요약**: WezTerm CLI가 tmux의 모든 필수 오퍼레이션에 대한 직접 대응 명령을 이미 제공
- **감지**: `TERM_PROGRAM=WezTerm` 환경변수
- **핵심 이점**: `split-pane`이 새 pane_id를 반환하므로 추적이 간단, `WEZTERM_PANE` 자동 설정

```
# WezTerm에서의 Claude Code 팀메이트 생성 예상 패턴
wezterm cli split-pane --right -- claude --resume <session-id> --teammate
# → 새 pane_id 반환 (예: 42)
wezterm cli send-text --pane-id 42 --no-paste "command\n"
wezterm cli list --format json
```

#### Issue #24189: Ghostty 지원

- **상태**: OPEN (2026-02-08)
- **차단 요인**: Ghostty에 안정적인 CLI/IPC 메커니즘이 아직 없음
- **진행 상황**: Ghostty 팀이 플랫폼별 IPC 개발 중 (macOS: AppleScript/App Intents, Linux: D-Bus)
- **추적**: [ghostty-org/ghostty#2353](https://github.com/ghostty-org/ghostty/discussions/2353)

#### 관련 이슈

| 이슈 | 내용 |
|------|------|
| #24122 | Zellij 멀티플렉서 지원 요청 |
| #23950 | tmux split 방향 설정 가능하게 |
| #24385 | iTerm2 페인이 팀메이트 종료 후 닫히지 않음 |
| #19555 | 동적 thought bubble 윈도우 (빌트인 멀티플렉서) |

### 3.6 Crux 통합 전략

Crux가 Claude Code Agent Teams를 지원하려면 두 가지 접근 방식이 가능하다:

#### 방식 A: CLI 호환 인터페이스 (단기)

WezTerm과 동일한 패턴으로 CLI 제공:

```bash
crux cli split-pane --right -- claude --teammate
crux cli send-text --pane-id <ID> "text"
crux cli list --format json
crux cli get-text --pane-id <ID>
crux cli activate-pane --pane-id <ID>
```

- `TERM_PROGRAM=Crux` 환경변수로 감지
- `CRUX_PANE` 환경변수로 현재 페인 ID 전달
- Claude Code에 PR을 보내 Crux 백엔드 추가 요청

#### 방식 B: tmux 호환 모드 (즉시)

tmux CLI와 호환되는 래퍼 제공:

```bash
# crux가 내부적으로 tmux 명령을 자체 IPC로 변환
crux --tmux-compat split-window -h -- command
crux --tmux-compat send-keys -t target "text" Enter
crux --tmux-compat list-panes
```

- 기존 tmux 백엔드를 바로 활용 가능
- 하지만 기능 제약이 있고, 유지보수 부담

**권장: 방식 A** (CLI 호환 인터페이스)를 구현하고 Claude Code에 기여

---

## 4. Crux IPC 설계

### 4.1 아키텍처 개요

```
┌─────────────────────────────────────────────────────────┐
│                   Crux IPC Architecture                   │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────┐                                       │
│  │ crux cli     │──┐                                    │
│  │ (CLI 클라이언트)│  │                                   │
│  └──────────────┘  │                                    │
│                    │   Unix Domain Socket                │
│  ┌──────────────┐  │   ($CRUX_SOCKET)                   │
│  │ Claude Code  │──┼──►┌────────────────┐               │
│  │ (에이전트)    │  │   │  IPC Server    │               │
│  └──────────────┘  │   │  (tokio task)  │               │
│                    │   └────────┬───────┘               │
│  ┌──────────────┐  │           │                        │
│  │ 외부 도구     │──┘    ┌──────▼──────┐                 │
│  │ (MCP 등)     │       │ Crux Core   │                 │
│  └──────────────┘       │ (메인 앱)    │                 │
│                         │             │                 │
│              ┌──────────┼──────────┐  │                 │
│              │          │          │  │                 │
│         ┌────▼───┐ ┌────▼───┐ ┌────▼──┐                │
│         │ Pane 0 │ │ Pane 1 │ │Pane 2 │                │
│         │ (PTY)  │ │ (PTY)  │ │(PTY)  │                │
│         └────────┘ └────────┘ └───────┘                │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 4.2 Unix 도메인 소켓 서버 (Rust + Tokio)

#### 소켓 경로 및 디스커버리

```rust
// 소켓 경로 결정
fn socket_path() -> PathBuf {
    // 우선순위:
    // 1. $CRUX_SOCKET 환경변수 (사용자 지정)
    // 2. $XDG_RUNTIME_DIR/crux/gui-sock-$PID
    // 3. /tmp/crux-$UID/gui-sock-$PID

    if let Ok(path) = std::env::var("CRUX_SOCKET") {
        return PathBuf::from(path);
    }

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/tmp/crux-{}", unsafe { libc::getuid() }));

    let dir = PathBuf::from(runtime_dir).join("crux");
    std::fs::create_dir_all(&dir).expect("Failed to create socket directory");

    dir.join(format!("gui-sock-{}", std::process::id()))
}
```

#### 서버 구현 (Tokio)

```rust
use tokio::net::UnixListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn start_ipc_server(
    socket_path: &Path,
    pane_manager: Arc<PaneManager>,
) -> Result<()> {
    // 기존 소켓 파일 정리
    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path)?;

    // 소켓 권한 설정 (소유자만 읽기/쓰기)
    std::fs::set_permissions(socket_path,
        std::fs::Permissions::from_mode(0o600))?;

    // 환경변수에 소켓 경로 기록
    std::env::set_var("CRUX_SOCKET", socket_path.to_str().unwrap());

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let pm = pane_manager.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, pm).await {
                        eprintln!("Client error: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Accept error: {}", e);
            }
        }
    }
}
```

### 4.3 프로토콜 선택: JSON-RPC 2.0

#### 선택 이유

| 옵션 | 장점 | 단점 |
|------|------|------|
| **JSON-RPC 2.0** ✅ | 표준화됨, 디버깅 용이, 도구 지원 풍부 | 바이너리 대비 오버헤드 |
| Custom Binary | 최고 성능, 최소 오버헤드 | 디버깅 어려움, 문서화 부담 |
| gRPC | 강력한 타입 시스템, 코드 생성 | 터미널 IPC에 과도, 빌드 복잡 |
| MessagePack-RPC | 바이너리 효율 + 구조화 | JSON-RPC만큼 보편적이지 않음 |

**JSON-RPC 2.0 선택 근거:**
- IPC 메시지 크기가 작아 JSON 오버헤드 무시 가능
- CLI 도구에서 `jq`로 디버깅 가능
- 표준 사양으로 외부 도구 통합 용이
- 알림(notification)과 요청(request)을 자연스럽게 구분

#### 메시지 프레이밍

소켓 스트림에서 JSON-RPC 메시지 경계를 구분하는 방식:

```
# 옵션 1: 길이 접두사 (권장)
<4바이트 빅엔디안 길이><JSON-RPC 메시지>

# 옵션 2: 개행 구분 (간단)
{"jsonrpc":"2.0",...}\n

# 옵션 3: Content-Length 헤더 (LSP 스타일)
Content-Length: 128\r\n
\r\n
{"jsonrpc":"2.0",...}
```

**권장: 옵션 1 (길이 접두사)**
- 바이너리 데이터 포함 가능
- 파싱이 가장 효율적
- 버퍼 관리가 명확

### 4.4 동시성 처리

```rust
/// 페인 매니저 - 동시 접근 안전
pub struct PaneManager {
    panes: Arc<RwLock<HashMap<PaneId, PaneState>>>,
    next_id: Arc<AtomicU64>,
    event_tx: broadcast::Sender<PaneEvent>,
}

impl PaneManager {
    /// 새 페인 생성 (분할)
    pub async fn split_pane(&self, request: SplitPaneRequest) -> Result<PaneId> {
        let new_id = PaneId(self.next_id.fetch_add(1, Ordering::Relaxed));

        // PTY 생성 및 프로세스 시작
        let pty = create_pty(&request)?;
        let pane_state = PaneState::new(new_id, pty, request.direction);

        // 페인 등록
        {
            let mut panes = self.panes.write().await;
            panes.insert(new_id, pane_state);
        }

        // 이벤트 브로드캐스트
        let _ = self.event_tx.send(PaneEvent::Created {
            pane_id: new_id,
            parent_id: request.target_pane,
        });

        Ok(new_id)
    }

    /// 특정 페인에 텍스트 전송
    pub async fn send_text(&self, pane_id: PaneId, text: &str, no_paste: bool) -> Result<()> {
        let panes = self.panes.read().await;
        let pane = panes.get(&pane_id)
            .ok_or(Error::PaneNotFound(pane_id))?;

        if no_paste {
            pane.pty_writer.write_all(text.as_bytes()).await?;
        } else {
            // Bracketed paste mode
            pane.pty_writer.write_all(b"\x1b[200~").await?;
            pane.pty_writer.write_all(text.as_bytes()).await?;
            pane.pty_writer.write_all(b"\x1b[201~").await?;
        }

        Ok(())
    }
}
```

### 4.5 보안

#### 소켓 권한

```rust
// Unix 소켓 생성 후 즉시 권한 설정
fn secure_socket(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    // 소유자만 읽기/쓰기 (0600)
    std::fs::set_permissions(path,
        std::fs::Permissions::from_mode(0o600))?;

    // 소켓 디렉토리도 소유자만 접근 (0700)
    if let Some(parent) = path.parent() {
        std::fs::set_permissions(parent,
            std::fs::Permissions::from_mode(0o700))?;
    }

    Ok(())
}
```

#### 인증 (선택적)

간단한 토큰 기반 인증:

```rust
/// 연결 시 인증 핸드셰이크
async fn authenticate_client(stream: &mut UnixStream) -> Result<bool> {
    // 소켓 권한으로 충분한 경우가 대부분
    // 추가 보안이 필요한 경우:

    // 1. 환경변수로 공유되는 세션 토큰
    let expected_token = std::env::var("CRUX_AUTH_TOKEN").ok();

    if let Some(token) = expected_token {
        let mut buf = [0u8; 256];
        let n = stream.read(&mut buf).await?;
        let client_token = std::str::from_utf8(&buf[..n])?;
        return Ok(client_token.trim() == token);
    }

    // 토큰 미설정 시 소켓 권한만으로 인증
    Ok(true)
}
```

#### peer credentials 검증

```rust
use std::os::unix::net::UCred;

// 연결된 클라이언트의 UID/GID 확인
fn verify_peer(stream: &UnixStream) -> Result<()> {
    let cred: UCred = stream.peer_cred()?;
    let my_uid = unsafe { libc::getuid() };

    if cred.uid() != my_uid {
        return Err(Error::Unauthorized(
            format!("UID mismatch: expected {}, got {}", my_uid, cred.uid())
        ));
    }

    Ok(())
}
```

### 4.6 CLI 클라이언트 디스커버리

```rust
/// CLI 클라이언트가 서버 소켓을 찾는 로직
fn discover_socket() -> Result<PathBuf> {
    // 1. 환경변수 (현재 페인의 소켓)
    if let Ok(path) = std::env::var("CRUX_SOCKET") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    // 2. 런타임 디렉토리에서 가장 최근 소켓 찾기
    let runtime_dir = runtime_directory();
    let mut sockets: Vec<_> = std::fs::read_dir(&runtime_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str()
            .map(|n| n.starts_with("gui-sock-"))
            .unwrap_or(false))
        .collect();

    // 수정 시간으로 정렬 (최신 우선)
    sockets.sort_by_key(|e| {
        std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok())
    });

    sockets.first()
        .map(|e| e.path())
        .ok_or(Error::NoServerFound)
}
```

---

## 5. Crux 프로토콜 설계

### 5.1 네임스페이스 체계

Crux 프로토콜은 계층적 네임스페이스를 사용한다:

```
crux:<domain>/<action>
```

#### 도메인 목록

| 도메인 | 설명 | 우선순위 |
|--------|------|---------|
| `crux:pane/*` | 페인 제어 (분할, 텍스트 전송/읽기) | P0 (필수) |
| `crux:window/*` | 윈도우/탭 관리 | P0 (필수) |
| `crux:clipboard/*` | 리치 클립보드 오퍼레이션 | P1 (중요) |
| `crux:ime/*` | IME 상태 제어 | P1 (중요) |
| `crux:render/*` | 인라인 렌더링 (마크다운, 이미지) | P2 (향후) |
| `crux:notify/*` | 알림 시스템 | P2 (향후) |
| `crux:theme/*` | 테마/외형 제어 | P3 (선택) |

### 5.2 `crux:pane/*` - 페인 제어

#### `crux:pane/split`

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "crux:pane/split",
  "params": {
    "target_pane_id": 0,         // 분할 대상 (null = 현재 활성 페인)
    "direction": "right",         // "right" | "left" | "top" | "bottom"
    "size": {
      "type": "percent",          // "percent" | "cells"
      "value": 50
    },
    "cwd": "/Users/jjh/project",  // 선택적
    "command": ["claude", "--resume", "abc", "--teammate"],  // 선택적
    "env": {                       // 추가 환경변수
      "CLAUDE_TEAM": "my-team"
    },
    "top_level": false             // 전체 윈도우 분할 여부
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "pane_id": 42,
    "window_id": 0,
    "tab_id": 0,
    "size": { "rows": 40, "cols": 60 },
    "tty": "/dev/ttys003"
  }
}
```

#### `crux:pane/send-text`

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "crux:pane/send-text",
  "params": {
    "pane_id": 42,
    "text": "ls -la\n",
    "bracketed_paste": false   // true면 브래킷 페이스트 래핑
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": { "bytes_written": 7 }
}
```

#### `crux:pane/get-text`

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "crux:pane/get-text",
  "params": {
    "pane_id": 42,
    "start_line": 0,       // 0 = 화면 첫줄, 음수 = 스크롤백
    "end_line": null,       // null = 화면 끝까지
    "include_escapes": false
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "lines": [
      "total 128",
      "drwxr-xr-x  15 jjh  staff   480  2 11 14:30 .",
      "-rw-r--r--   1 jjh  staff  1234  2 11 14:30 file.txt"
    ],
    "first_line": 0,
    "cursor_row": 3,
    "cursor_col": 0
  }
}
```

#### `crux:pane/list`

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "crux:pane/list",
  "params": {}  // 필터 옵션 추가 가능
}

// Response
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "panes": [
      {
        "pane_id": 0,
        "window_id": 0,
        "tab_id": 0,
        "size": { "rows": 40, "cols": 120 },
        "title": "zsh",
        "cwd": "file:///Users/jjh/",
        "is_active": true,
        "is_zoomed": false,
        "cursor": { "x": 5, "y": 0, "shape": "block", "visible": true },
        "tty": "/dev/ttys001",
        "pid": 12345
      },
      {
        "pane_id": 42,
        "window_id": 0,
        "tab_id": 0,
        "size": { "rows": 40, "cols": 60 },
        "title": "claude --teammate",
        "cwd": "file:///Users/jjh/project/",
        "is_active": false,
        "is_zoomed": false,
        "cursor": { "x": 0, "y": 0, "shape": "block", "visible": true },
        "tty": "/dev/ttys003",
        "pid": 12346
      }
    ]
  }
}
```

#### `crux:pane/activate`

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "crux:pane/activate",
  "params": { "pane_id": 42 }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": { "success": true }
}
```

#### `crux:pane/close`

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "crux:pane/close",
  "params": {
    "pane_id": 42,
    "force": false   // true면 프로세스 강제 종료
  }
}
```

#### `crux:pane/resize`

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "crux:pane/resize",
  "params": {
    "pane_id": 42,
    "direction": "right",  // 확장할 방향
    "amount": 10,           // 셀 단위
    "type": "cells"         // "cells" | "percent"
  }
}
```

#### `crux:pane/move`

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 8,
  "method": "crux:pane/move",
  "params": {
    "pane_id": 42,
    "target_pane_id": 0,
    "direction": "right"
  }
}
```

### 5.3 `crux:clipboard/*` - 리치 클립보드

#### `crux:clipboard/write`

```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "method": "crux:clipboard/write",
  "params": {
    "target": "system",     // "system" | "primary" | "internal"
    "content": [
      {
        "mime_type": "text/plain",
        "data": "Hello, World!"
      },
      {
        "mime_type": "text/html",
        "data": "<b>Hello</b>, World!"
      },
      {
        "mime_type": "image/png",
        "data_base64": "iVBORw0KGgo..."   // Base64 인코딩
      }
    ]
  }
}
```

#### `crux:clipboard/read`

```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "method": "crux:clipboard/read",
  "params": {
    "source": "system",
    "preferred_types": ["text/plain", "text/html"]
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 11,
  "result": {
    "content": [
      {
        "mime_type": "text/plain",
        "data": "Hello, World!"
      },
      {
        "mime_type": "text/html",
        "data": "<b>Hello</b>, World!"
      }
    ],
    "available_types": ["text/plain", "text/html", "image/png"]
  }
}
```

### 5.4 `crux:ime/*` - IME 상태 제어

#### `crux:ime/get-state`

```json
{
  "jsonrpc": "2.0",
  "id": 20,
  "method": "crux:ime/get-state",
  "params": { "pane_id": 0 }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 20,
  "result": {
    "active": true,
    "composing": true,
    "composition_text": "한글",
    "cursor_position": 2,
    "input_source": "com.apple.inputmethod.Korean.2SetKorean"
  }
}
```

#### `crux:ime/set-input-source`

```json
{
  "jsonrpc": "2.0",
  "id": 21,
  "method": "crux:ime/set-input-source",
  "params": {
    "pane_id": 0,
    "input_source": "com.apple.keylayout.ABC"
  }
}
```

### 5.5 `crux:render/*` - 인라인 렌더링

#### `crux:render/image`

```json
{
  "jsonrpc": "2.0",
  "id": 30,
  "method": "crux:render/image",
  "params": {
    "pane_id": 0,
    "image": {
      "format": "png",           // "png" | "jpeg" | "gif" | "svg"
      "data_base64": "iVBOR...", // 또는 file_path
      "file_path": null
    },
    "placement": {
      "width": { "type": "cells", "value": 40 },
      "height": { "type": "auto" },
      "position": "cursor"        // "cursor" | "absolute"
    }
  }
}
```

#### `crux:render/markdown`

```json
{
  "jsonrpc": "2.0",
  "id": 31,
  "method": "crux:render/markdown",
  "params": {
    "pane_id": 0,
    "markdown": "# Title\n\nSome **bold** text with `code`",
    "theme": "auto"  // "auto" | "dark" | "light"
  }
}
```

### 5.6 비동기 알림 (Notifications)

JSON-RPC 2.0의 알림(id 없는 메시지)을 활용한 이벤트 구독:

#### 구독 요청

```json
{
  "jsonrpc": "2.0",
  "id": 100,
  "method": "crux:events/subscribe",
  "params": {
    "events": [
      "pane.created",
      "pane.closed",
      "pane.output",
      "pane.title-changed",
      "pane.focus-changed"
    ],
    "pane_filter": null  // null = 모든 페인, [42] = 특정 페인만
  }
}
```

#### 알림 메시지

```json
// 페인 생성 알림
{
  "jsonrpc": "2.0",
  "method": "crux:event/pane.created",
  "params": {
    "pane_id": 42,
    "parent_pane_id": 0,
    "timestamp": "2026-02-11T14:30:00.000Z"
  }
}

// 페인 출력 알림
{
  "jsonrpc": "2.0",
  "method": "crux:event/pane.output",
  "params": {
    "pane_id": 42,
    "data": "$ ls\nfile1.txt  file2.txt\n",
    "timestamp": "2026-02-11T14:30:01.000Z"
  }
}

// 페인 포커스 변경 알림
{
  "jsonrpc": "2.0",
  "method": "crux:event/pane.focus-changed",
  "params": {
    "pane_id": 42,
    "focused": true,
    "previous_pane_id": 0,
    "timestamp": "2026-02-11T14:30:02.000Z"
  }
}
```

### 5.7 버전 관리 전략

#### 프로토콜 버전

```json
// 연결 시 핸드셰이크
{
  "jsonrpc": "2.0",
  "id": 0,
  "method": "crux:handshake",
  "params": {
    "client_name": "crux-cli",
    "client_version": "0.1.0",
    "protocol_version": "1.0",
    "capabilities": ["pane", "clipboard", "ime", "render"]
  }
}

// 응답
{
  "jsonrpc": "2.0",
  "id": 0,
  "result": {
    "server_name": "crux",
    "server_version": "0.1.0",
    "protocol_version": "1.0",
    "supported_capabilities": ["pane", "clipboard", "ime"],
    "session_token": "abc123..."  // 선택적 인증 토큰
  }
}
```

#### 버전 호환성 규칙

| 버전 변경 | 규칙 |
|-----------|------|
| Patch (1.0.x) | 버그 수정만, 하위 호환 보장 |
| Minor (1.x.0) | 새 메서드 추가 가능, 기존 메서드 변경 불가 |
| Major (x.0.0) | 호환 불가 변경, 마이그레이션 기간 제공 |

### 5.8 기존 터미널 프로토콜과의 관계

#### 이스케이프 시퀀스 기반 프로토콜

| 시퀀스 | 형식 | 용도 | 예시 |
|--------|------|------|------|
| **OSC** | `ESC ] Ps ; Pt ST` | 운영체제 명령 | 클립보드, 타이틀, 색상 |
| **DCS** | `ESC P ... ST` | 디바이스 제어 | tmux 컨트롤 모드, Sixel |
| **APC** | `ESC _ ... ST` | 애플리케이션 명령 | Kitty 그래픽스 프로토콜 |
| **CSI** | `ESC [ ... final` | 제어 시퀀스 | 커서 이동, 스크롤, SGR |

#### Crux의 이중 프로토콜 전략

Crux는 **두 가지 통신 채널**을 동시에 지원한다:

1. **IPC 채널 (Unix Socket + JSON-RPC)**: 외부 프로세스에서의 프로그래밍적 제어
   - CLI 클라이언트, Claude Code Agent Teams, MCP 도구 등
   - 구조화된 요청/응답, 타입 안전

2. **In-band 채널 (이스케이프 시퀀스)**: PTY를 통한 애플리케이션 내 통신
   - 셸 통합, 인라인 이미지 (Kitty/Sixel), OSC 52 클립보드
   - 기존 표준 호환, SSH 통과 가능

```
외부 프로세스 ──► Unix Socket ──► JSON-RPC ──► Crux Core
                                                  ↑
PTY 내부 앱 ──► PTY fd ──► 이스케이프 시퀀스 ──────┘
```

#### 커스텀 OSC 시퀀스 (In-band 확장)

PTY 내부 앱이 Crux 고유 기능을 사용하고 싶을 때:

```
# Crux 커스텀 OSC (번호 대역: 7700-7799)
ESC ] 7700 ; <json-payload> ST

# 예: 마크다운 인라인 렌더링
ESC ] 7700 ; {"action":"render_markdown","content":"# Hello"} ST

# 예: 리치 클립보드 쓰기
ESC ] 7701 ; {"mime":"text/html","data":"<b>bold</b>"} ST
```

**OSC 번호 선택 근거:**
- 0-119: xterm 표준
- 133: 셸 통합 (FinalTerm)
- 1337: iTerm2 확장
- 7700-7799: Crux 전용 (충돌 방지를 위해 높은 번호 대역)

### 5.9 전체 프로토콜 메서드 요약

| 네임스페이스 | 메서드 | 우선순위 |
|-------------|--------|---------|
| `crux:handshake` | 연결 초기화 | P0 |
| `crux:pane/split` | 페인 분할 | P0 |
| `crux:pane/send-text` | 텍스트 전송 | P0 |
| `crux:pane/get-text` | 텍스트 읽기 | P0 |
| `crux:pane/list` | 페인 목록 | P0 |
| `crux:pane/activate` | 페인 포커스 | P0 |
| `crux:pane/close` | 페인 닫기 | P0 |
| `crux:pane/resize` | 페인 크기 변경 | P1 |
| `crux:pane/move` | 페인 재배치 | P2 |
| `crux:window/create` | 새 윈도우 | P0 |
| `crux:window/list` | 윈도우 목록 | P0 |
| `crux:window/close` | 윈도우 닫기 | P1 |
| `crux:clipboard/write` | 클립보드 쓰기 | P1 |
| `crux:clipboard/read` | 클립보드 읽기 | P1 |
| `crux:ime/get-state` | IME 상태 조회 | P1 |
| `crux:ime/set-input-source` | 입력 소스 변경 | P1 |
| `crux:render/image` | 이미지 렌더링 | P2 |
| `crux:render/markdown` | 마크다운 렌더링 | P2 |
| `crux:events/subscribe` | 이벤트 구독 | P1 |
| `crux:events/unsubscribe` | 이벤트 구독 해제 | P1 |

---

## 6. 참고 문헌

### WezTerm

- [WezTerm CLI Reference](https://wezterm.org/cli/cli/index.html)
- [WezTerm split-pane](https://wezterm.org/cli/cli/split-pane.html)
- [WezTerm send-text](https://wezterm.org/cli/cli/send-text.html)
- [WezTerm get-text](https://wezterm.org/cli/cli/get-text.html)
- [WezTerm list](https://wezterm.org/cli/cli/list.html)
- [WezTerm Multiplexing](https://wezterm.org/multiplexing.html)
- [WezTerm Multiplexer Architecture (DeepWiki)](https://deepwiki.com/wezterm/wezterm/2.3-multiplexer-architecture)
- [WezTerm unix_domains config](https://wezterm.org/config/lua/config/unix_domains.html)

### tmux

- [tmux Control Mode Wiki](https://github.com/tmux/tmux/wiki/Control-Mode)
- [iTerm2 tmux Integration Documentation](https://iterm2.com/documentation-tmux-integration.html)
- [tmux Integration Best Practices (GitLab)](https://gitlab.com/gnachman/iterm2/-/wikis/tmux-Integration-Best-Practices)

### Claude Code Agent Teams

- [Claude Code Agent Teams Documentation](https://code.claude.com/docs/en/agent-teams)
- [Issue #23574: WezTerm split-pane backend](https://github.com/anthropics/claude-code/issues/23574)
- [Issue #24189: Ghostty split-pane backend](https://github.com/anthropics/claude-code/issues/24189)
- [Issue #23572: tmux/iTerm2 silent fallback bug](https://github.com/anthropics/claude-code/issues/23572)
- [Issue #23527: pane-base-index 문제](https://github.com/anthropics/claude-code/issues/23527)
- [Issue #24108: 메일박스 메시지 미처리 버그](https://github.com/anthropics/claude-code/issues/24108)
- [Issue #24122: Zellij 지원 요청](https://github.com/anthropics/claude-code/issues/24122)

### 터미널 프로토콜

- [Xterm Control Sequences](https://www.invisible-island.net/xterm/ctlseqs/ctlseqs.html)
- [iTerm2 Proprietary Escape Codes](https://iterm2.com/3.0/documentation-escape-codes.html)
- [Kitty Terminal Protocol Extensions](https://sw.kovidgoyal.net/kitty/protocol-extensions/)
- [Kitty Graphics Protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
- [Ghostty Control Sequences Concepts](https://ghostty.org/docs/vt/concepts/sequences)
- [Ghostty Scripting API Discussion #2353](https://github.com/ghostty-org/ghostty/discussions/2353)

### Rust IPC

- [tokio-unix-ipc crate](https://crates.io/crates/tokio-unix-ipc)
- [tokio UnixListener docs](https://docs.rs/tokio/latest/tokio/net/struct.UnixListener.html)
- [axum Unix Domain Socket example](https://github.com/tokio-rs/axum/blob/main/examples/unix-domain-socket/src/main.rs)
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- [JSON-RPC Transport: Sockets](https://www.simple-is-better.org/json-rpc/transport_sockets.html)

### 기타 참조

- [Kitty Remote Control (DeepWiki)](https://deepwiki.com/kovidgoyal/kitty/6.1-remote-control-system)
- [WezTerm Escape Sequences](https://wezterm.org/escape-sequences.html)
