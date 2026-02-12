---
title: "Crux MCP 통합 전략"
description: "Model Context Protocol 아키텍처, Rust SDK(rmcp), 터미널 MCP 도구 설계, 경쟁 분석, 구현 로드맵"
date: 2026-02-12
phase: [2, 5]
topics: [mcp, ai-agent, rmcp, json-rpc, claude-code, agent-teams, unix-socket]
status: final
related:
  - ipc-protocol-design.md
  - ipc-external-patterns.md
  - claude-code-strategy.md
---

# MCP(Model Context Protocol) 통합 연구

> Crux 터미널 에뮬레이터의 MCP 서버/클라이언트 통합 전략 연구
> 작성일: 2026-02-12

---

## 목차

1. [MCP 프로토콜 개요](#1-mcp-프로토콜-개요)
2. [시장 현황: 터미널 × MCP 생태계](#2-시장-현황-터미널--mcp-생태계)
3. [Crux MCP 도구 설계](#3-crux-mcp-도구-설계)
4. [Rust SDK 기술 평가](#4-rust-sdk-기술-평가)
5. [아키텍처 설계](#5-아키텍처-설계)
6. [기존 IPC와의 관계](#6-기존-ipc와의-관계)
7. [보안 고려사항](#7-보안-고려사항)
8. [구현 로드맵](#8-구현-로드맵)
9. [참고 문헌](#9-참고-문헌)

---

## 1. MCP 프로토콜 개요

### 1.1 배경

Model Context Protocol(MCP)은 AI 애플리케이션이 외부 도구 및 데이터 소스와 상호작용하기 위한 표준 프로토콜이다. Anthropic이 2024년 11월 발표하였으며, 2025년 12월 Linux Foundation 산하 Agentic AI Foundation에 기증되었다.

**핵심 가치**: MCP 서버를 한 번 구현하면, MCP 클라이언트를 내장한 **모든 AI 도구**(Claude Desktop, Claude Code, Cursor, Windsurf, Copilot 등)에서 즉시 사용 가능하다.

### 1.2 3계층 아키텍처

```
┌─────────────────────────────────────┐
│   Host (AI 애플리케이션)              │
│   Claude Desktop, Cursor, IDE 등     │
│   - 다수의 Client 인스턴스 관리       │
│   - 보안 정책 및 사용자 동의 강제     │
└────────────┬────────────────────────┘
             │
     ┌───────┼───────┐
     ▼       ▼       ▼
┌────────┐┌────────┐┌────────┐
│Client 1││Client 2││Client 3│  각 Client는 하나의 Server와 1:1 연결
└───┬────┘└───┬────┘└───┬────┘
    ▼         ▼         ▼
┌────────┐┌────────┐┌────────┐
│Server 1││Server 2││Server 3│  각 Server는 독립적, 서로 격리됨
│(Files) ││ (DB)   ││(Crux)  │
└────────┘└────────┘└────────┘
```

**설계 원칙:**

1. **Host**: 컨테이너 프로세스. 다수의 Client를 생성/관리하고, 보안 정책 강제
2. **Client**: 하나의 Server와 상태 유지(stateful) 연결. 메시지 라우팅
3. **Server**: 특화된 컨텍스트와 기능 제공. 다른 Server나 전체 대화를 볼 수 없음
4. **격리**: Server 간 상호 참조 불가. 보안 경계가 명확

### 1.3 프로토콜 계층

| 계층 | 기반 | 역할 |
|------|------|------|
| **데이터 계층** | JSON-RPC 2.0 | 수명주기 관리, 도구/리소스/프롬프트, 알림 |
| **트랜스포트 계층** | stdio / HTTP+SSE | 연결 수립, 메시지 프레이밍, 인증 |

### 1.4 트랜스포트 유형

#### stdio (로컬 통합)

- Client가 Server를 자식 프로세스로 실행
- stdin/stdout으로 양방향 통신
- **주의**: `println!()`, `print!()`로 stdout에 출력하면 JSON-RPC 메시지가 오염됨
- 로깅은 반드시 stderr로

```
Client ──spawn──> Server process
         stdin ──> JSON-RPC request
         stdout <── JSON-RPC response
```

#### Streamable HTTP (원격 연결, 현대적 표준)

- HTTP POST: 클라이언트→서버 메시지
- HTTP GET + SSE: 서버→클라이언트 알림/스트리밍
- `Mcp-Session-Id` 헤더로 세션 관리
- `Last-Event-ID`로 재연결 시 재개 지원

```
Client ──POST /mcp──> Server
         GET /mcp ──> SSE stream
```

#### SSE (레거시, deprecated)

2025-03-26부로 deprecated. Streamable HTTP로 마이그레이션 권장.

### 1.5 3대 프리미티브

#### Tools (도구)

LLM이 호출할 수 있는 **함수**. 사용자 승인 필요.

```json
{
  "name": "crux_create_pane",
  "description": "Create a new terminal pane by splitting",
  "inputSchema": {
    "type": "object",
    "properties": {
      "direction": { "type": "string", "enum": ["horizontal", "vertical"] }
    }
  }
}
```

#### Resources (리소스)

파일 같은 **읽기 전용 데이터**. AI 모델에 컨텍스트 제공.

```json
{
  "uri": "crux://pane/1/scrollback",
  "name": "Pane 1 Scrollback Buffer",
  "mimeType": "text/plain"
}
```

#### Prompts (프롬프트)

사전 정의된 **템플릿**. 복잡한 상호작용 구조화.

```json
{
  "name": "debug-session",
  "description": "Set up multi-pane debugging layout",
  "arguments": [{ "name": "project_dir", "required": true }]
}
```

### 1.6 수명주기

```
Client                              Server
  │                                    │
  │  initialize (capabilities)         │
  │───────────────────────────────────>│
  │                                    │
  │  InitializeResult (capabilities)   │
  │<───────────────────────────────────│
  │                                    │
  │  initialized (notification)        │
  │───────────────────────────────────>│
  │                                    │
  │  tools/list, resources/list ...    │
  │<──────────── 메시지 교환 ──────────>│
  │                                    │
  │  shutdown / close                  │
  │───────────────────────────────────>│
```

Capability Negotiation: 초기화 시 서버와 클라이언트가 각자 지원하는 기능을 교환한다.

**서버 Capabilities**: `tools`, `resources`, `prompts`, `logging`
**클라이언트 Capabilities**: `sampling`, `roots`, `elicitation`

---

## 2. 시장 현황: 터미널 × MCP 생태계

### 2.1 현재 터미널의 AI 통합 상황

| 터미널 | AI 통합 방식 | MCP 지원 | 제한사항 |
|--------|-------------|----------|----------|
| **Warp** | 자체 에이전트 플랫폼 (Oz) | 없음 (독자 프로토콜) | 클라우드 의존, 폐쇄적, 로그인 필수 |
| **Ghostty** | AppleScript/App Intents 개발 중 | 외부 커뮤니티 MCP | AppleScript 느림, macOS 전용 |
| **WezTerm** | Lua 스크립팅 | 외부 커뮤니티 MCP | Lua API 문서 부족 |
| **iTerm2** | AppleScript 자동화 | 외부 MCP 다수 | 100-300ms/명령, 느림 |
| **Kitty** | Remote control 프로토콜 | 외부 커뮤니티 MCP | Python 의존 |
| **Alacritty** | 없음 | 없음 | 미니멀 철학 |

**핵심 발견: 네이티브로 MCP 서버를 내장한 터미널은 아직 없다.** 모든 현존 구현은 AppleScript, tmux 명령어, 소켓을 통한 외부 래퍼이다.

### 2.2 기존 터미널 관련 MCP 서버

| 서버 | 접근 방식 | 도구 수 | 특징 |
|------|----------|---------|------|
| [terminal-mcp](https://github.com/elleryfamilia/terminal-mcp) | node-pty + xterm 헤드리스 | 6 | 세션 녹화, REPL 디버깅 |
| [tmux-mcp](https://github.com/jonrad/tmux-mcp) | libtmux 래핑 | ~10 | 패널 분할, 세션 관리 |
| [iterm-mcp](https://github.com/ferrislucas/iterm-mcp) | AppleScript | ~5 | macOS 전용, 느림 |
| [ghostty-mcp](https://lobehub.com/mcp/yourusername-ghostty-mcp) | CLI 래핑 | ~5 | 개발 중 |
| [wezterm-mcp](https://glama.ai/mcp/servers/@vaporif/wezterm-mcp) | CLI 래핑 | ~5 | Rust 구현 |
| [kitty-mcp](https://glama.ai/mcp/servers/@den-tanui/kitty-mcp) | Remote control | ~5 | 스크롤백 캡처 |
| [conductor-mcp](https://github.com/GGPrompts/conductor-mcp) | tmux 오케스트레이션 | 33 | Claude Code 워커 조율 |

### 2.3 Claude Code의 현재 터미널 상호작용

Claude Code Agent Teams는 현재 3가지 백엔드를 지원:

1. **tmux 백엔드**: `tmux split-window`, `tmux send-keys`, `tmux capture-pane`
2. **iTerm2 백엔드**: AppleScript (macOS 전용, 느림)
3. **in-process 백엔드**: 단일 터미널, 시각적 분리 없음

**Pain Points:**
- Ghostty 미지원 (Feature Request #24189 — 높은 수요)
- AppleScript 오버헤드로 iTerm2 통합이 느림
- 제한적 상태 검사 (working directory, running process 쿼리 불가)
- ANSI 코드 수동 파싱 필요

### 2.4 Warp의 선도적 접근 (벤치마크)

Warp는 2026년 1월 Agents 3.0을 출시하며 Terminal-bench 1위(52%)를 달성:

- **Full terminal control**: 디버거, DB 셸, TUI 앱과 직접 상호작용
- **Interactive prompt handling**: 대화형 프롬프트 자동 응답
- **External integrations**: Slack/Linear/GitHub 터미널 내 통합
- **Multi-agent**: Claude Code, Codex, Gemini CLI 동시 실행

그러나 **클라우드 의존적**, **폐쇄 소스**, **로그인 필수**라는 제한이 있다.

### 2.5 전략적 포지셔닝

```
                    클라우드 의존
                        ↑
                   Warp (독자 플랫폼)
                        │
    로컬 우선 ←──────────┼──────────→ AI 네이티브
                        │
              iTerm2    │    ★ Crux (네이티브 MCP)
              WezTerm   │
              Ghostty   │
                        ↓
                    전통적 터미널
```

Crux의 차별점: **오픈소스 + 로컬 우선 + 표준 프로토콜(MCP) 기반 AI 네이티브 터미널**

---

## 3. Crux MCP 도구 설계

### 3.1 Phase 1: 핵심 도구 (MVP, 20개)

AI 에이전트가 터미널을 기본적으로 제어하는 데 필요한 최소 도구셋.

#### 3.1.1 패널/탭 관리 (5개)

| 도구 | 입력 | 출력 | 설명 |
|------|------|------|------|
| `crux_create_pane` | `direction: "horizontal" \| "vertical"`, `cwd?: string` | `PaneInfo { id, pid, cwd }` | 패널 분할 생성 |
| `crux_close_pane` | `pane_id: string` | `{ success: bool }` | 패널 닫기 |
| `crux_focus_pane` | `pane_id: string` | `{ success: bool }` | 포커스 이동 |
| `crux_list_panes` | (없음) | `Vec<PaneInfo>` | 전체 패널 목록 + 메타데이터 |
| `crux_resize_pane` | `pane_id: string`, `cols: u16`, `rows: u16` | `{ success: bool }` | 패널 크기 조절 |

#### 3.1.2 명령 실행 & 출력 캡처 (5개)

| 도구 | 입력 | 출력 | 설명 |
|------|------|------|------|
| `crux_execute_command` | `pane_id: string`, `command: string`, `timeout_ms?: u64` | `{ exit_code: i32, stdout: string }` | 명령 실행 후 결과 반환 |
| `crux_send_keys` | `pane_id: string`, `keys: string` | `{ success: bool }` | 원시 키 시퀀스 (Ctrl+C, Enter 등) |
| `crux_send_text` | `pane_id: string`, `text: string` | `{ success: bool }` | 텍스트 입력 |
| `crux_get_output` | `pane_id: string`, `lines?: u32` | `{ text: string, line_count: u32 }` | 최근 N줄 출력 |
| `crux_wait_for_output` | `pane_id: string`, `pattern: string`, `timeout_ms: u64` | `{ matched: bool, text: string }` | 패턴 매칭까지 대기 |

#### 3.1.3 터미널 상태 조회 (5개)

| 도구 | 입력 | 출력 | 설명 |
|------|------|------|------|
| `crux_get_current_directory` | `pane_id: string` | `{ cwd: string }` | 셸 작업 디렉토리 |
| `crux_get_running_process` | `pane_id: string` | `{ name: string, pid: u32 }` | 활성 프로세스 |
| `crux_get_pane_state` | `pane_id: string` | `PaneState { cols, rows, cursor, scroll_pos }` | 전체 상태 스냅샷 |
| `crux_get_selection` | `pane_id: string` | `{ text: string }` | 선택된 텍스트 |
| `crux_get_scrollback` | `pane_id: string`, `offset?: u32`, `limit?: u32` | `{ text: string, total_lines: u32 }` | 스크롤백 버퍼 |

#### 3.1.4 콘텐츠 캡처 & 세션 (5개)

| 도구 | 입력 | 출력 | 설명 |
|------|------|------|------|
| `crux_screenshot_pane` | `pane_id: string` | `{ image: base64_png }` | 패널 시각적 캡처 (GPUI 렌더) |
| `crux_get_raw_text` | `pane_id: string` | `{ text: string }` | ANSI 코드 제거된 순수 텍스트 |
| `crux_get_formatted_output` | `pane_id: string`, `lines?: u32` | `{ text: string }` | ANSI 코드 포함 출력 |
| `crux_save_session` | `name?: string` | `{ session_id: string }` | 전체 세션 직렬화 |
| `crux_restore_session` | `session_id: string` | `{ success: bool }` | 세션 복원 |

### 3.2 Phase 2: Crux 고유 차별화 도구 (10개)

Crux만의 강점(GPUI 렌더링, 한국어 IME, 리치 클립보드, DockArea)을 활용하는 도구들.

#### 3.2.1 구조화된 출력 파싱

```
crux_parse_output_structured
```

GPUI 렌더링 엔진이 이미 시각적 구조(테이블 경계, 컬럼, 헤더)를 이해하고 있으므로, 터미널 출력을 구조화된 JSON으로 변환할 수 있다.

**예시**: `docker ps` 실행 → ASCII 테이블 대신 JSON 배열 반환

```json
{
  "type": "table",
  "headers": ["CONTAINER ID", "IMAGE", "STATUS", "PORTS"],
  "rows": [
    ["abc123", "nginx:latest", "Up 2 hours", "80/tcp"],
    ["def456", "postgres:16", "Up 5 hours", "5432/tcp"]
  ]
}
```

#### 3.2.2 시각적 Diff

```
crux_visual_diff
```

명령 실행 전/후 스크린샷을 비교하여 픽셀 diff + 의미적 diff를 반환한다. GPUI의 GPU 가속 렌더링으로 스크린샷 캡처가 저비용이기 때문에 가능한 기능.

**활용**: TUI 앱 상태 확인, 빌드 전후 변경사항 검증

#### 3.2.3 IME 인식 입력

```
crux_type_with_ime
```

한국어/일본어/중국어 IME 입력을 시뮬레이션한다. 조합(composition) 완료까지 대기하고, preedit 텍스트가 PTY에 전송되지 않도록 보장한다.

**왜 Crux만 가능한가**: 다른 MCP 서버는 ASCII만 전송 가능. Crux는 IME 파이프라인을 직접 제어하므로 CJK 자동화가 가능하다.

#### 3.2.4 클립보드 인텔리전스

```
crux_clipboard_context
```

클립보드 히스토리를 소스 패널, 타임스탬프와 함께 추적한다. 리치 포맷(이미지, RTF, 파일 목록)도 지원.

```
crux_paste_smart
```

클립보드 내용을 컨텍스트에 맞게 변환하여 붙여넣기. JSON → pretty-print, URL → 하이퍼링크 등.

#### 3.2.5 Agent Workspace 레이아웃

```
crux_load_workspace
```

Claude Code Agent Teams용 사전 정의 멀티패널 레이아웃을 로드한다. DockArea가 프로그래밍적 레이아웃 생성을 지원하므로, tmux 스크립팅 없이 선언적으로 워크스페이스를 구성할 수 있다.

**프리셋 예시:**

| 이름 | 레이아웃 | 용도 |
|------|---------|------|
| `debug-session` | 코드 \| 로그 \| 디버거 | 디버깅 |
| `full-stack` | 프론트 \| 백엔드 \| DB \| 테스트 | 풀스택 개발 |
| `agent-team-3` | 리더 \| 워커1 \| 워커2 | 3-에이전트 팀 |
| `monitoring` | 프로세스 \| 로그 \| 메트릭 | 모니터링 |

#### 3.2.6 실시간 출력 스트리밍

```
crux_stream_output
```

SSE(Server-Sent Events)로 터미널 출력을 실시간 스트리밍한다. 요청/응답이 아닌 이벤트 기반으로, 에이전트가 빌드 출력을 모니터링하다가 첫 에러에서 즉시 반응할 수 있다.

```
event: output
data: {"pane_id": "1", "text": "error[E0308]: mismatched types\n"}

event: exit
data: {"pane_id": "1", "exit_code": 1}
```

#### 3.2.7 다중 패널 조율

```
crux_coordinate_panes
```

다중 패널에서 순차적 조건부 실행을 단일 호출로 선언한다.

```json
{
  "steps": [
    { "pane": "backend", "command": "cargo run", "wait_for": "Listening on 0.0.0.0:8080" },
    { "pane": "frontend", "command": "npm run dev", "wait_for": "ready in" },
    { "pane": "test", "command": "cargo test --test e2e" }
  ]
}
```

**킬러 기능**: 현재 Agent Teams에서 가장 어려운 작업인 "서비스 시작 순서 보장"을 단일 도구 호출로 해결한다.

#### 3.2.8 셸 컨텍스트 주입

```
crux_inject_context
```

셸을 재시작하지 않고 환경변수, 별칭, 함수를 동적으로 주입/해제한다.

```json
{
  "pane_id": "1",
  "env": { "RUST_BACKTRACE": "1", "LOG_LEVEL": "debug" },
  "expires_after_commands": 5
}
```

#### 3.2.9 터미널 스냅샷

```
crux_create_snapshot / crux_restore_snapshot
```

전체 터미널 상태(모든 패널, 프로세스, 스크롤백, 환경변수)를 직렬화/복원한다. 재현 가능한 디버깅 세션에 필수적.

#### 3.2.10 의도 감지

```
crux_detect_intent
```

셸 명령어 + 출력을 분석하여 의도를 분류한다:

```json
{
  "intent": "test_failure",
  "confidence": 0.95,
  "context": {
    "failed_tests": ["test_pane_creation", "test_resize"],
    "error_summary": "2 tests failed, 15 passed"
  },
  "suggested_actions": ["cargo test test_pane_creation -- --nocapture"]
}
```

### 3.3 Phase 3: MCP 클라이언트 기능

Crux가 외부 MCP 서버를 **소비**하는 기능. 우선순위 낮음.

| 기능 | 설명 |
|------|------|
| MCP 서버 연결 관리 | Git, DB, Slack 등 외부 MCP 서버 연결/해제 |
| GPUI 오버레이 UI | MCP 도구 선택/결과를 터미널 위에 오버레이 렌더링 |
| 키보드 단축키 | `Ctrl+Shift+M` → MCP 도구 팔레트 |
| 자동 컨텍스트 제공 | 현재 패널 상태를 Resource로 자동 노출 |

---

## 4. Rust SDK 기술 평가

### 4.1 공식 SDK: rmcp

| 항목 | 상세 |
|------|------|
| **크레이트** | [`rmcp`](https://crates.io/crates/rmcp) |
| **버전** | 0.15.0 (2026-02-10 릴리스, 총 54회 릴리스) |
| **저장소** | [`modelcontextprotocol/rust-sdk`](https://github.com/modelcontextprotocol/rust-sdk) |
| **프로토콜** | MCP 2025-11-25 완전 지원 |
| **런타임** | Tokio async |
| **Rust Edition** | 2024 (nightly 필요 — 릴리스 시점에 stable 확인 필요) |

### 4.2 핵심 매크로

```rust
#[tool_router]   // 도구 라우팅 자동 생성
#[tool]          // 메서드를 MCP 도구로 표시
#[tool_handler]  // ServerHandler 트레이트 구현
#[task_handler]  // 비동기 태스크 수명주기
```

### 4.3 최소 서버 예시

```rust
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::router::tool::ToolRouter,
    model::*,
    tool, tool_handler, tool_router,
};

pub struct CruxMcpServer {
    tool_router: ToolRouter<CruxMcpServer>,
    pane_tx: tokio::sync::mpsc::Sender<PaneCommand>,
}

#[tool_router]
impl CruxMcpServer {
    fn new(pane_tx: tokio::sync::mpsc::Sender<PaneCommand>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            pane_tx,
        }
    }

    #[tool(description = "Create a new terminal pane by splitting")]
    async fn crux_create_pane(&self, direction: String) -> String {
        let (respond_tx, respond_rx) = tokio::sync::oneshot::channel();
        self.pane_tx.send(PaneCommand::Create {
            direction,
            respond: respond_tx,
        }).await.unwrap();
        respond_rx.await.unwrap()
    }

    #[tool(description = "Get recent output from a pane")]
    async fn crux_get_output(&self, pane_id: String, lines: Option<u32>) -> String {
        let (respond_tx, respond_rx) = tokio::sync::oneshot::channel();
        self.pane_tx.send(PaneCommand::GetOutput {
            pane_id,
            lines: lines.unwrap_or(50),
            respond: respond_tx,
        }).await.unwrap();
        respond_rx.await.unwrap()
    }
}

#[tool_handler]
impl ServerHandler for CruxMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("Crux terminal emulator MCP server".into()),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            ..Default::default()
        }
    }
}
```

### 4.4 커뮤니티 대안 비교

| 크레이트 | 상태 | 장점 | 단점 |
|---------|------|------|------|
| **rmcp** (공식) | 활발 | 공식 지원, 완전한 기능, 매크로 | nightly 필요 (Edition 2024) |
| `rust-mcp-sdk` | 활발 | 하위 호환성, 풀 구현 | 비공식 |
| `mcp-sdk-rs` | 활발 | WebSocket 지원 | 포크 기반 |
| `rust-mcp-server` | 활발 | 인체공학적 API | 서버 전용 |

**권장**: 공식 `rmcp` 사용. 최고 수준의 유지보수, 문서, 기능 완성도.

### 4.5 TypeScript/Python SDK와의 기능 비교

| 기능 | TypeScript | Python | rmcp (Rust) |
|------|-----------|--------|-------------|
| Resources | O | O | O |
| Tools | O | O | O (`#[tool]` 매크로) |
| Prompts | O | O | O |
| Sampling | O | O | O |
| stdio 트랜스포트 | O | O | O |
| HTTP/SSE 트랜스포트 | O | O | O (Axum 기반) |
| OAuth2 | O | O | O (`AuthorizationManager`) |
| 세션 관리 | O | O | O (`Mcp-Session-Id`) |
| 프로시져럴 매크로 | X | X | O (Rust 고유) |

---

## 5. 아키텍처 설계

### 5.1 추천 아키텍처: 내장 MCP 서버

```
┌──────────────────────────────────────────────────┐
│  Crux Terminal (GPUI, 메인 스레드)                 │
│                                                    │
│  ┌──────────────────────────────────────────────┐ │
│  │  터미널 상태                                    │ │
│  │  - DockArea (패널 관리)                         │ │
│  │  - PTY 핸들                                    │ │
│  │  - alacritty_terminal Grid                     │ │
│  │  - 스크롤백 버퍼                                │ │
│  └──────────────┬───────────────────────────────┘ │
│                 │ mpsc 채널                        │
│  ┌──────────────▼───────────────────────────────┐ │
│  │  MCP Server (rmcp + Axum)                    │ │
│  │  - 별도 Tokio 런타임 스레드                     │ │
│  │  - Unix socket: ~/.crux/mcp.sock             │ │
│  │  - HTTP fallback: 127.0.0.1:{port}           │ │
│  │  - 도구 핸들러 20+ (Phase 1)                   │ │
│  └──────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────┘
         ↕ Unix socket / HTTP
┌──────────────────────────────────────────────────┐
│  crux-mcp-bridge (stdio ↔ Unix socket)           │
│  Claude Desktop가 stdio로 실행하는 경량 바이너리    │
└──────────────────────────────────────────────────┘
         ↕ stdio
┌──────────────────────────────────────────────────┐
│  Claude Desktop / Claude Code / Cursor / ...     │
│  (MCP 클라이언트)                                  │
└──────────────────────────────────────────────────┘
```

### 5.2 트랜스포트 선택

| 트랜스포트 | 장점 | 단점 | 용도 |
|-----------|------|------|------|
| **Unix socket** (권장) | 낮은 오버헤드, 포트 불필요, 보안 | macOS/Linux 전용 | Crux ↔ MCP 브릿지 |
| **HTTP localhost** | 디버깅 용이, 유연함 | 포트 관리 필요 | 개발/테스트, 원격 접근 |
| **stdio** | Claude Desktop 기본 | GUI 앱에 부적합 | 브릿지 바이너리 전용 |

**결론**: Crux 본체는 Unix socket으로 MCP 서버를 노출하고, Claude Desktop 호환을 위해 `crux-mcp-bridge` 바이너리가 stdio ↔ Unix socket을 중계한다.

### 5.3 스레딩 모델

```
메인 스레드 (GPUI)         MCP 스레드 (Tokio)
─────────────────          ──────────────────
Window 렌더링               MCP 요청 수신
이벤트 처리                  도구 핸들러 실행
DockArea 조작         ←──── PaneCommand (mpsc)
PTY 입출력            ────→ PaneResult (oneshot)
```

- GPUI는 macOS 요구사항으로 메인 스레드에서 실행
- MCP 서버는 별도 스레드에서 Tokio 런타임으로 실행
- `tokio::sync::mpsc`로 MCP → GPUI 명령 전송
- `tokio::sync::oneshot`으로 GPUI → MCP 결과 반환

### 5.4 크레이트 구조

```
crux-mcp/              (새 크레이트)
├── Cargo.toml
└── src/
    ├── lib.rs         # MCP 서버 코어
    ├── tools/         # 도구 핸들러
    │   ├── mod.rs
    │   ├── pane.rs    # 패널 관리 도구
    │   ├── execute.rs # 명령 실행 도구
    │   ├── state.rs   # 상태 조회 도구
    │   └── content.rs # 콘텐츠 캡처 도구
    ├── resources.rs   # MCP 리소스 (스크롤백, 환경변수 등)
    ├── prompts.rs     # MCP 프롬프트 템플릿
    └── transport.rs   # Unix socket / HTTP 설정

crux-mcp-bridge/       (새 바이너리 크레이트)
├── Cargo.toml
└── src/
    └── main.rs        # stdio ↔ Unix socket 브릿지
```

### 5.5 워크스페이스 의존성 그래프 (갱신)

```
crux-protocol  (shared types, no internal deps)
    ↓
crux-terminal  (VT emulation)
    ↓
crux-terminal-view  (GPUI Element)
    ↓
crux-app  (main: window management, GPUI bootstrap, DockArea)
    ↑
crux-mcp  ────→ crux-protocol  (MCP 도구 ↔ IPC 명령 매핑)
    ↑
crux-mcp-bridge  (stdio ↔ Unix socket)

crux-ipc        (Unix socket server — depends on crux-protocol)
crux-clipboard  (NSPasteboard — depends on crux-protocol)
```

---

## 6. 기존 IPC와의 관계

Crux는 이미 `crux-ipc` 크레이트에서 JSON-RPC 2.0 기반 Unix socket IPC를 설계하고 있다 (`ipc-protocol-design.md` 참조). MCP와 IPC는 상호 보완적이며 계층이 다르다.

### 6.1 이중 프로토콜 전략

| 계층 | 프로토콜 | 네임스페이스 | 대상 | 예시 |
|------|---------|-------------|------|------|
| **내부 IPC** | `crux:<domain>/<action>` (JSON-RPC) | `crux:pane/split` | crux-cli, 내부 컴포넌트 | `crux pane split --direction horizontal` |
| **외부 MCP** | MCP (JSON-RPC) | `crux_create_pane` | 모든 AI 에이전트 | Claude Desktop → MCP → Crux |

### 6.2 호출 체인

```
AI Agent (Claude Code)
  ↓ MCP (tools/call)
crux-mcp (MCP 서버)
  ↓ mpsc 채널 또는 IPC 클라이언트
crux-app (GPUI 메인)
  ↓ DockArea API
터미널 패널 조작
```

MCP 도구는 내부적으로 IPC 프로토콜의 동일한 명령을 호출한다. 즉 **MCP는 IPC 위의 AI 친화적 래퍼**이다.

### 6.3 PaneBackend 인터페이스 매핑

`ipc-protocol-design.md`에서 정의한 PaneBackend 13개 메서드와 MCP 도구의 매핑:

| PaneBackend 메서드 | MCP 도구 | 비고 |
|-------------------|----------|------|
| `spawn_pane()` | `crux_create_pane` | direction 파라미터 추가 |
| `close_pane()` | `crux_close_pane` | 동일 |
| `focus_pane()` | `crux_focus_pane` | 동일 |
| `list_panes()` | `crux_list_panes` | 메타데이터 확장 |
| `resize_pane()` | `crux_resize_pane` | 동일 |
| `send_keys()` | `crux_send_keys` | 동일 |
| `send_text()` | `crux_send_text` | 동일 |
| `get_output()` | `crux_get_output` | lines 파라미터 추가 |
| `get_cwd()` | `crux_get_current_directory` | 동일 |
| `get_process()` | `crux_get_running_process` | 동일 |
| `screenshot()` | `crux_screenshot_pane` | base64 PNG |
| `save_session()` | `crux_save_session` | 동일 |
| `restore_session()` | `crux_restore_session` | 동일 |

MCP는 PaneBackend를 완전히 포함하면서, 추가적인 AI 전용 도구(구조화 파싱, 시각적 diff, IME 등)를 제공한다.

---

## 7. 보안 고려사항

### 7.1 MCP 보안 원칙 (프로토콜 스펙)

1. **사용자 동의 및 제어**: 모든 데이터 접근/조작에 명시적 동의 필요
2. **데이터 프라이버시**: 사용자 데이터를 동의 없이 전송 금지
3. **도구 안전성**: 도구는 임의 코드 실행을 의미 — 반드시 사용자 승인 후 호출
4. **LLM Sampling 제어**: 서버의 프롬프트 접근을 의도적으로 제한

### 7.2 터미널 특화 보안 위협

| 위협 | 설명 | 대응 |
|------|------|------|
| **명령 주입** | 악의적 MCP 클라이언트가 `rm -rf /` 전송 | 명령 화이트리스트, 위험 명령 사용자 확인 |
| **파일시스템 탈출** | cwd 변경 후 민감 파일 접근 | 작업 디렉토리 제한 옵션 |
| **프로세스 신호** | SIGKILL 등 임의 시그널 | 시그널 종류 제한 |
| **ANSI 이스케이프 주입** | 출력에 악의적 이스케이프 시퀀스 삽입 | 출력 반환 시 이스케이프 정규화 |
| **셸 이스케이프** | `$(command)`, `` `command` `` 등 | 입력 시 셸 메타문자 이스케이프 |

### 7.3 보안 설계

```toml
# crux.toml 보안 설정 예시
[mcp.security]
# 승인 없이 허용되는 명령 패턴
allowed_commands = ["ls", "cat", "git *", "cargo *", "npm *"]

# 항상 사용자 확인이 필요한 명령
dangerous_patterns = ["rm", "sudo", "chmod", "kill"]

# 작업 디렉토리 제한 (비어있으면 제한 없음)
allowed_directories = []

# 타임아웃 (ms)
command_timeout_ms = 30000

# 최대 동시 패널 수
max_panes = 20
```

---

## 8. 구현 로드맵

### Phase 2.5: MCP 서버 MVP

PLAN.md Phase 2 (Tabs, Panes, IPC) 이후, Phase 3 이전에 삽입.

| # | 작업 | 예상 크기 | 의존성 |
|---|------|----------|--------|
| 1 | `crux-mcp` 크레이트 생성 + rmcp 연동 | S | Phase 2 IPC 완료 |
| 2 | Unix socket 트랜스포트 설정 (Axum) | S | #1 |
| 3 | 패널 관리 5개 도구 구현 | M | #2, DockArea 완료 |
| 4 | 명령 실행 5개 도구 구현 | M | #3, PTY 완료 |
| 5 | 상태 조회 5개 도구 구현 | M | #4 |
| 6 | 콘텐츠 캡처 5개 도구 구현 | M | #5 |
| 7 | `crux-mcp-bridge` stdio 브릿지 | S | #2 |
| 8 | Claude Desktop 통합 테스트 | S | #7, 전체 도구 완료 |

### Phase 5.5: 차별화 도구

PLAN.md Phase 5 (tmux, Claude Code) 이후.

| # | 작업 | 예상 크기 | 의존성 |
|---|------|----------|--------|
| 9 | 구조화 출력 파싱 | L | Phase 2.5 완료 |
| 10 | 시각적 diff | M | GPUI 스크린샷 |
| 11 | IME 인식 입력 | M | Phase 3 IME 완료 |
| 12 | 클립보드 인텔리전스 | M | Phase 3 클립보드 완료 |
| 13 | Agent Workspace 레이아웃 | M | DockArea 프리셋 |
| 14 | 실시간 스트리밍 (SSE) | M | HTTP 트랜스포트 |
| 15 | 다중 패널 조율 | L | 전체 패널 도구 |
| 16 | 셸 컨텍스트 주입 | S | 쉘 통합 |
| 17 | 터미널 스냅샷 | M | 세션 직렬화 |
| 18 | 의도 감지 | L | 출력 파싱 |

### Claude Desktop 설정 예시

```json
{
  "mcpServers": {
    "crux-terminal": {
      "command": "crux-mcp-bridge",
      "args": ["--socket", "~/.crux/mcp.sock"]
    }
  }
}
```

---

## 9. 참고 문헌

### MCP 프로토콜 & 생태계

- [Model Context Protocol Specification (2025-11-25)](https://modelcontextprotocol.io/specification/2025-11-25)
- [MCP Architecture Documentation](https://modelcontextprotocol.io/specification/2025-11-25/architecture)
- [Build an MCP Server Guide](https://modelcontextprotocol.io/docs/develop/build-server)
- [Official MCP Servers Repository](https://github.com/modelcontextprotocol/servers)
- [MCP Best Practices 2026](https://www.philschmid.de/mcp-best-practices)

### SDK & 구현

- [Official Rust SDK (rmcp)](https://github.com/modelcontextprotocol/rust-sdk) — v0.15.0
- [rmcp crate documentation](https://docs.rs/rmcp)
- [TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk)
- [Python SDK](https://github.com/modelcontextprotocol/python-sdk)
- [Building stdio MCP Server in Rust](https://www.shuttle.dev/blog/2025/07/18/how-to-build-a-stdio-mcp-server-in-rust)

### 터미널 MCP 구현

- [terminal-mcp](https://github.com/elleryfamilia/terminal-mcp) — 범용 터미널 MCP
- [tmux-mcp](https://github.com/jonrad/tmux-mcp) — tmux 세션 관리
- [iterm-mcp](https://github.com/ferrislucas/iterm-mcp) — iTerm2 AppleScript
- [conductor-mcp](https://github.com/GGPrompts/conductor-mcp) — 33개 도구 Claude Code 오케스트레이션
- [Console Automation MCP](https://github.com/ooples/mcp-console-automation) — 콘솔 자동화

### AI 터미널 동향

- [Warp Agents 3.0](https://www.warp.dev/blog/agents-3-full-terminal-use-plan-code-review-integration)
- [Terminal-Bench](https://www.tbench.ai/) — AI 에이전트 벤치마크
- [Ghostty Backend Feature Request #24189](https://github.com/anthropics/claude-code/issues/24189)
- [ANSI Escape Codes in MCP (보안)](https://blog.trailofbits.com/2025/04/29/deceiving-users-with-ansi-terminal-codes-in-mcp/)

### Crux 내부 문서

- [ipc-protocol-design.md](ipc-protocol-design.md) — Crux IPC/프로토콜 설계, PaneBackend
- [ipc-external-patterns.md](ipc-external-patterns.md) — WezTerm/tmux IPC 패턴
- [claude-code-strategy.md](claude-code-strategy.md) — Claude Code Feature Request 전략
