---
title: "IPC 설계 패턴 및 외부 통합 조사"
description: "WezTerm CLI internals (source-level), JSON-RPC 2.0 best practices, Unix socket IPC security, event subscription patterns"
date: 2026-02-11
phase: [2]
topics: [ipc, wezterm, json-rpc, unix-socket, security, event-subscription]
status: final
related:
  - ipc-protocol-design.md
  - claude-code-strategy.md
---

# IPC 설계 패턴 및 Claude Code Agent Teams 통합 조사 보고서

## 목차
1. [WezTerm CLI 아키텍처 분석](#1-wezterm-cli-아키텍처-분석)
2. [Claude Code Agent Teams 터미널 백엔드](#2-claude-code-agent-teams-터미널-백엔드)
3. [JSON-RPC 2.0 over Unix Socket 모범사례](#3-json-rpc-20-over-unix-socket-모범사례)
4. [터미널 IPC 보안 패턴](#4-터미널-ipc-보안-패턴)
5. [이벤트 구독 패턴](#5-이벤트-구독-패턴)
6. [Crux 터미널을 위한 설계 권장사항](#6-crux-터미널을-위한-설계-권장사항)

---

## 1. WezTerm CLI 아키텍처 분석

### 1.1 전체 아키텍처 개요

WezTerm은 **GUI 프로세스와 CLI 클라이언트 간 Unix Domain Socket 기반 IPC**를 사용하는 모듈러 아키텍처를 채택한다. 핵심 설계 원칙은 터미널 에뮬레이션 로직이 GUI 프론트엔드와 독립적으로 동작하며, Mux(멀티플렉서) 계층이 세션/윈도우/탭/패인을 관리하는 것이다.

```
┌─────────────────┐     Unix Socket     ┌──────────────────────┐
│  wezterm cli     │ ◄──────────────────► │  wezterm-gui /       │
│  (클라이언트)     │     codec::Pdu      │  wezterm-mux-server  │
│                  │     프로토콜          │  (서버)               │
└─────────────────┘                      └──────────────────────┘
                                                   │
                                          ┌────────┴────────┐
                                          │      Mux        │
                                          │  ┌──────────┐   │
                                          │  │ Domain    │   │
                                          │  │ Tab       │   │
                                          │  │ Pane      │   │
                                          │  │ Window    │   │
                                          │  └──────────┘   │
                                          └─────────────────┘
```

### 1.2 소켓 발견 메커니즘

WezTerm CLI는 다단계 소켓 발견 전략을 사용한다:

**소스: `wezterm-client/src/client.rs` - `compute_unix_domain()`**

```rust
fn compute_unix_domain(prefer_mux: bool, class_name: &str) -> anyhow::Result<config::UnixDomain> {
    match std::env::var_os("WEZTERM_UNIX_SOCKET") {
        // 1단계: WEZTERM_UNIX_SOCKET 환경변수가 설정되어 있으면 사용
        Some(path) if !path.is_empty() => Ok(config::UnixDomain {
            socket_path: Some(path.into()),
            ..Default::default()
        }),
        Some(_) | None => {
            if !prefer_mux {
                // 2단계: GUI 인스턴스의 소켓 경로를 심볼릭 링크로 찾기
                if let Ok(gui) = crate::discovery::resolve_gui_sock_path(class_name) {
                    return Ok(config::UnixDomain {
                        socket_path: Some(gui),
                        no_serve_automatically: true,
                        ..Default::default()
                    });
                }
            }
            // 3단계: 설정 파일의 unix_domains 항목 사용
            let config = configuration();
            Ok(config.unix_domains.first()?.clone())
        }
    }
}
```

**소켓 발견 순서:**
1. `WEZTERM_UNIX_SOCKET` 환경변수 (패인 내부에서 자동 설정됨)
2. 심볼릭 링크 기반 GUI 소켓 발견 (`discovery.rs`)
3. 설정 파일의 `unix_domains` 항목

**소스: `wezterm-client/src/discovery.rs` - Unix 소켓 발견**

macOS에서는 `RUNTIME_DIR` 내에 `default-{class_name}` 심볼릭 링크를 생성하여 실행 중인 GUI 인스턴스의 소켓 경로를 가리킨다:

```rust
// macOS
fn compute_name(class_name: &str) -> String {
    format!("default-{}", class_name)
}

// Linux (Wayland/X11 세션별 구분)
fn compute_name(class_name: &str) -> String {
    if let Ok(wayland) = std::env::var("WAYLAND_DISPLAY") {
        format!("wayland-{}-{}", wayland, class_name)
    } else {
        let x11 = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
        format!("x11-{}-{}", x11, class_name)
    }
}
```

추가로 `discover_gui_socks()`는 `gui-sock-*` 패턴의 파일을 탐색하여 라이브 소켓을 발견하고, 죽은 소켓은 자동 정리한다.

### 1.3 Codec/직렬화 (varbincode + zstd)

**소스: `codec/src/lib.rs`**

WezTerm은 자체 바이너리 프로토콜인 **codec::Pdu**를 사용한다:

**프레임 포맷:**
```
tagged_len: leb128  (u64 MSB가 설정되면 zstd 압축됨)
serial:     leb128  (요청-응답 매칭용 시리얼 번호)
ident:      leb128  (PDU 타입 식별자)
data:       bytes   (varbincode 직렬화된 페이로드)
```

**핵심 설계 결정:**
- **LEB128 가변 길이 정수 인코딩**: 작은 값은 적은 바이트로 인코딩하여 대역폭 절약
- **varbincode**: serde 기반 바이너리 직렬화 (가변 길이 정수 사용)
- **zstd 조건부 압축**: `COMPRESS_THRESH` 이상의 데이터만 압축 시도, 압축 후 크기가 더 작을 때만 사용
- **버전 관리**: `CODEC_VERSION: usize = 45` 상수로 프로토콜 호환성 관리
- **시리얼 번호**: 요청-응답 매칭을 위한 단조 증가 시리얼

```rust
fn serialize<T: serde::Serialize>(t: &T) -> anyhow::Result<(Vec<u8>, bool)> {
    let mut uncompressed = Vec::new();
    let mut encode = varbincode::Serializer::new(&mut uncompressed);
    t.serialize(&mut encode)?;

    if uncompressed.len() <= COMPRESS_THRESH {
        return Ok((uncompressed, false));
    }
    // zstd 압축 시도
    let mut compressed = Vec::new();
    let mut compress = zstd::Encoder::new(&mut compressed, zstd::DEFAULT_COMPRESSION_LEVEL)?;
    // ... 압축 후 크기 비교하여 작은 쪽 선택
}
```

### 1.4 PDU(Protocol Data Unit) 메시지 타입

WezTerm은 `pdu!` 매크로로 모든 메시지 타입을 정의하며, 각 타입에 고유한 정수 식별자를 부여한다:

```rust
pdu! {
    ErrorResponse: 0,
    Ping: 1,
    Pong: 2,
    ListPanes: 3,
    ListPanesResponse: 4,
    SpawnResponse: 8,
    WriteToPane: 9,
    SendKeyDown: 11,
    SendPaste: 13,
    Resize: 14,
    SetClipboard: 20,
    GetCodecVersion: 26,
    GetCodecVersionResponse: 27,
    SplitPane: 34,
    KillPane: 35,
    SpawnV2: 36,
    PaneRemoved: 37,
    NotifyAlert: 39,
    PaneFocused: 53,
    // ... 총 62개 PDU 타입
}
```

**핵심 PDU 구조체:**

```rust
pub struct SplitPane {
    pub pane_id: PaneId,
    pub split_request: SplitRequest,
    pub command: Option<CommandBuilder>,
    pub command_dir: Option<String>,
    pub domain: SpawnTabDomain,
    pub move_pane_id: Option<PaneId>,
}

pub struct SpawnV2 {
    pub domain: SpawnTabDomain,
    pub window_id: Option<WindowId>,
    pub command: Option<CommandBuilder>,
    pub command_dir: Option<String>,
    pub size: TerminalSize,
    pub workspace: String,
}

pub struct SpawnResponse {
    pub pane_id: PaneId,
    pub tab_id: TabId,
    pub window_id: WindowId,
    pub size: TerminalSize,
}
```

### 1.5 Mux 서버 아키텍처

**소스: `wezterm-mux-server-impl/src/local.rs`**

```rust
pub struct LocalListener {
    listener: UnixListener,
}

impl LocalListener {
    pub fn with_domain(unix_dom: &UnixDomain) -> anyhow::Result<Self> {
        let listener = safely_create_sock_path(unix_dom)?;
        Ok(Self::new(listener))
    }

    pub fn run(&mut self) {
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    spawn_into_main_thread(async move {
                        crate::dispatch::process(stream).await
                    }).detach();
                }
                Err(err) => { log::error!("accept failed: {}", err); return; }
            }
        }
    }
}
```

**소스: `wezterm-mux-server-impl/src/dispatch.rs`**

세션 처리는 이벤트 루프 기반이다:

```rust
pub async fn process_async<T>(mut stream: Async<T>) -> anyhow::Result<()> {
    let (item_tx, item_rx) = smol::channel::unbounded::<Item>();

    let pdu_sender = PduSender::new(move |pdu| {
        item_tx.try_send(Item::WritePdu(pdu))
    });
    let mut handler = SessionHandler::new(pdu_sender);

    // Mux 이벤트 구독
    let mux = Mux::get();
    mux.subscribe(move |n| tx.try_send(Item::Notif(n)).is_ok());

    loop {
        let rx_msg = item_rx.recv();
        let wait_for_read = stream.readable().map(|_| Ok(Item::Readable));

        match smol::future::or(rx_msg, wait_for_read).await {
            Ok(Item::Readable) => {
                // 클라이언트 PDU 디코딩 및 처리
                let decoded = Pdu::decode_async(&mut stream, None).await?;
                handler.process_one(decoded);
            }
            Ok(Item::WritePdu(decoded)) => {
                // 응답 PDU 인코딩 및 전송
                decoded.pdu.encode_async(&mut stream, decoded.serial).await?;
                stream.flush().await?;
            }
            Ok(Item::Notif(MuxNotification::PaneRemoved(pane_id))) => {
                // 서버 → 클라이언트 이벤트 푸시
                Pdu::PaneRemoved(codec::PaneRemoved { pane_id })
                    .encode_async(&mut stream, 0).await?;
            }
            // ... 기타 이벤트 처리
        }
    }
}
```

### 1.6 Mux 알림 시스템

**소스: `mux/src/lib.rs`**

```rust
pub enum MuxNotification {
    PaneOutput(PaneId),
    PaneAdded(PaneId),
    PaneRemoved(PaneId),
    WindowCreated(WindowId),
    WindowRemoved(WindowId),
    WindowInvalidated(WindowId),
    Alert { pane_id: PaneId, alert: Alert },
    PaneFocused(PaneId),
    TabAddedToWindow { tab_id: TabId, window_id: WindowId },
    TabResized(TabId),
    TabTitleChanged { tab_id: TabId, title: String },
    // ...
}

pub struct Mux {
    tabs: RwLock<HashMap<TabId, Arc<Tab>>>,
    panes: RwLock<HashMap<PaneId, Arc<dyn Pane>>>,
    windows: RwLock<HashMap<WindowId, Window>>,
    domains: RwLock<HashMap<DomainId, Arc<dyn Domain>>>,
    subscribers: RwLock<HashMap<usize, Box<dyn Fn(MuxNotification) -> bool + Send + Sync>>>,
    // ...
}
```

### 1.7 에러 처리 패턴

WezTerm은 `anyhow` 크레이트를 일관되게 사용하며, 다음 패턴을 적용한다:

- **ErrorResponse PDU**: 서버 처리 실패 시 구조화된 에러 응답
- **정상적 연결 해제 감지**: `UnexpectedEof`, `BrokenPipe` 에러는 조용히 처리
- **시리얼 번호 검증**: 비정상적으로 큰 시리얼은 `CorruptResponse`로 처리
- **컨텍스트 추가**: `.context("reading Pdu from client")` 패턴으로 에러 추적

---

## 2. Claude Code Agent Teams 터미널 백엔드

### 2.1 Agent Teams 아키텍처

Claude Code Agent Teams는 다음 컴포넌트로 구성된다:

| 컴포넌트 | 역할 |
|----------|------|
| **Team Lead** | 팀 생성, 동료 에이전트 스폰, 작업 조율 |
| **Teammates** | 각각 독립적인 Claude Code 인스턴스 |
| **Task List** | 에이전트 간 공유 작업 목록 |
| **Mailbox** | 에이전트 간 메시징 시스템 |

**저장 경로:**
- 팀 설정: `~/.claude/teams/{team-name}/config.json`
- 작업 목록: `~/.claude/tasks/{team-name}/`

### 2.2 디스플레이 모드와 백엔드 선택

Claude Code는 두 가지 디스플레이 모드를 지원한다:

1. **In-process**: 모든 동료 에이전트가 메인 터미널 내부에서 실행 (기본값)
2. **Split panes**: 각 동료 에이전트가 자체 패인을 가짐 (tmux 또는 iTerm2 필요)

**`teammateMode` 설정 값:**
- `"auto"` (기본): tmux 세션 안에 있으면 split-pane, 아니면 in-process
- `"tmux"`: split-pane 모드 강제 (tmux 또는 iTerm2 자동 감지)
- `"in-process"`: in-process 모드 강제

**현재 백엔드 감지 로직:**
```
if (inside tmux session?) → tmux backend
else if (in iTerm2 + it2 CLI available?) → iTerm2 backend
else if (tmux available in PATH?) → tmux backend
else → in-process fallback
```

### 2.3 백엔드가 구현해야 하는 오퍼레이션

WezTerm issue #23574의 분석에서 도출한 필수 오퍼레이션:

| 오퍼레이션 | tmux | iTerm2 | WezTerm CLI |
|-----------|------|--------|-------------|
| 수평 분할 | `tmux split-window -h -- cmd` | `it2 split` | `wezterm cli split-pane --right -- cmd` |
| 수직 분할 | `tmux split-window -v -- cmd` | `it2 split` | `wezterm cli split-pane --bottom -- cmd` |
| 패인 목록 | `tmux list-panes` | Python API | `wezterm cli list` |
| 패인 포커스 | `tmux select-pane -t N` | Python API | `wezterm cli activate-pane --pane-id N` |
| 패인 정보 | `$TMUX_PANE` | 세션 ID | `$WEZTERM_PANE` 환경변수 |
| 패인 종료 | `tmux kill-pane -t N` | Python API | `wezterm cli kill-pane --pane-id N` |

**터미널 감지 환경변수:**
- tmux: `$TMUX` 설정 여부
- iTerm2: `$TERM_PROGRAM=iTerm.app`
- WezTerm: `$TERM_PROGRAM=WezTerm`
- Ghostty: `$TERM_PROGRAM=ghostty`

### 2.4 새 백엔드 추가에 필요한 인터페이스

Claude Code issue #23574에서 제안하는 WezTerm 백엔드 스폰 패턴:

```bash
# 동료 에이전트를 네이티브 WezTerm 패인에 스폰
wezterm cli split-pane --right -- claude --resume <session-id> --teammate
wezterm cli split-pane --bottom -- claude --resume <session-id> --teammate

# 패인 ID 반환으로 추적 가능
$ wezterm cli split-pane --right -- echo "hello"
3    # 새 패인 ID 반환

# 패인 관리
$ wezterm cli list                           # 패인 목록
$ wezterm cli activate-pane --pane-id 3     # 패인 포커스
$ wezterm cli kill-pane --pane-id 3         # 패인 종료
```

**백엔드 타입 트래킹:** config.json에 `backendType`과 `tmuxPaneId` (또는 해당 백엔드의 패인 식별자) 저장.

### 2.5 Ghostty 비교: API 부재 문제

Ghostty는 내부적으로 split 액션을 지원하지만 **프로그래밍적 API가 아직 없다**:
- macOS: AppleScript / App Intents (개발 중)
- Linux: D-Bus 통합 (계획 중)
- 추적: ghostty-org/ghostty#2353

이는 **Crux 터미널이 첫날부터 프로그래밍적 IPC API를 제공해야 하는 이유**를 명확히 보여준다.

---

## 3. JSON-RPC 2.0 over Unix Socket 모범사례

### 3.1 프로토콜 선택: JSON-RPC 2.0 vs 커스텀 바이너리

| 기준 | JSON-RPC 2.0 | 커스텀 바이너리 (WezTerm 방식) |
|------|-------------|--------------------------|
| **디버깅** | JSON이므로 쉽게 읽을 수 있음 | 바이너리라 별도 도구 필요 |
| **성능** | 텍스트 직렬화 오버헤드 | 매우 낮은 오버헤드 |
| **호환성** | 표준 규격, 다양한 클라이언트 | WezTerm 전용 |
| **확장성** | 메서드 추가가 간단 | 버전 관리 필요 |
| **생태계** | Claude Code, LSP 등과 일관 | 독자적 |
| **Crux 권장** | **권장** | 성능 크리티컬 경로에만 |

**권장사항:** Claude Code 통합이 핵심 목표이므로 JSON-RPC 2.0을 기본 프로토콜로 채택하고, 터미널 출력 스트리밍 같은 고성능 경로에만 바이너리 프레이밍을 고려한다.

### 3.2 Rust 크레이트 추천

#### 옵션 1: jsonrpsee (권장)

```toml
[dependencies]
jsonrpsee = { version = "0.24", features = ["server", "client", "macros"] }
tokio = { version = "1", features = ["full"] }
```

**장점:**
- Parity Technologies 관리, 활발한 유지보수
- tokio 기반 async/await
- Tower 미들웨어 통합
- proc-macro로 RPC 인터페이스 정의
- 구독(subscription) 내장 지원

**단점:**
- Unix socket 트랜스포트가 기본 제공되지 않음 (커스텀 트랜스포트 필요)
- WebSocket/HTTP 중심 설계

#### 옵션 2: 수동 구현 (경량, 완전한 제어)

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full", "net"] }
tokio-util = { version = "0.7", features = ["codec"] }
```

**장점:**
- Unix socket에 최적화된 트랜스포트
- 불필요한 의존성 없음
- 완전한 제어 (프레이밍, 백프레셔 등)

**단점:**
- 직접 구현해야 할 부분이 많음

#### 권장: 하이브리드 접근

핵심 JSON-RPC 타입만 정의하고, tokio Unix socket 위에 직접 구현:

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,  // "2.0"
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,  // None = notification
}

#[derive(Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}
```

### 3.3 메시지 프레이밍 패턴

#### 옵션 A: 길이 접두사 (Length-Prefix)

```
[4바이트 리틀엔디안 길이][JSON 페이로드]
```

```rust
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

// tokio-util의 LengthDelimitedCodec 사용
let codec = LengthDelimitedCodec::builder()
    .length_field_length(4)
    .new_codec();
```

**장점:** 이진 안전, 정확한 메시지 경계, 큰 메시지 지원
**단점:** 디버깅 시 프레이밍 해석 필요

#### 옵션 B: 개행 구분 (Newline-Delimited)

```
{"jsonrpc":"2.0","method":"split_pane",...}\n
{"jsonrpc":"2.0","result":...}\n
```

```rust
use tokio_util::codec::LinesCodec;

let codec = LinesCodec::new_with_max_length(64 * 1024);
```

**장점:** 사람이 읽을 수 있음, netcat/socat으로 디버깅 가능, LSP와 유사
**단점:** JSON 내부에 개행 불가 (compact 직렬화 필수)

#### 옵션 C: Content-Length 헤더 (LSP 스타일)

```
Content-Length: 82\r\n
\r\n
{"jsonrpc":"2.0","method":"split_pane","params":{"direction":"right"},"id":1}
```

**장점:** LSP와 완전히 호환, JSON에 개행 가능
**단점:** 파싱이 약간 복잡

**Crux 권장:** **옵션 A (길이 접두사)** - tokio-util의 `LengthDelimitedCodec`이 바로 사용 가능하고, 바이너리 안전하며, 성능이 가장 좋다. 디버깅용으로는 별도의 CLI 도구를 제공하면 된다.

### 3.4 Async 서버 구조 (tokio)

```rust
use tokio::net::UnixListener;
use tokio_util::codec::Framed;
use futures::{SinkExt, StreamExt};

struct IpcServer {
    socket_path: PathBuf,
    mux: Arc<Mux>,
}

impl IpcServer {
    async fn run(&self) -> anyhow::Result<()> {
        let listener = UnixListener::bind(&self.socket_path)?;

        loop {
            let (stream, _addr) = listener.accept().await?;
            let mux = self.mux.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(stream, mux).await {
                    tracing::error!("Client error: {}", e);
                }
            });
        }
    }

    async fn handle_client(
        stream: tokio::net::UnixStream,
        mux: Arc<Mux>,
    ) -> anyhow::Result<()> {
        // 피어 자격증명 확인
        let cred = stream.peer_cred()?;
        verify_peer(&cred)?;

        let codec = LengthDelimitedCodec::builder()
            .length_field_length(4)
            .new_codec();
        let mut framed = Framed::new(stream, codec);

        // 이벤트 구독 채널
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(256);

        loop {
            tokio::select! {
                // 클라이언트 요청 처리
                Some(msg) = framed.next() => {
                    let msg = msg?;
                    let request: JsonRpcRequest = serde_json::from_slice(&msg)?;
                    let response = dispatch_request(&mux, &request, &event_tx).await;
                    let bytes = serde_json::to_vec(&response)?;
                    framed.send(bytes.into()).await?;
                }
                // 서버 → 클라이언트 이벤트 푸시
                Some(event) = event_rx.recv() => {
                    let notification = create_notification(event);
                    let bytes = serde_json::to_vec(&notification)?;
                    framed.send(bytes.into()).await?;
                }
                else => break,
            }
        }
        Ok(())
    }
}
```

### 3.5 에러 코드 표준

JSON-RPC 2.0 표준 에러 코드를 기반으로 터미널 전용 에러 코드를 확장:

```rust
// JSON-RPC 2.0 표준 에러
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;

// Crux 터미널 전용 에러 (-32000 ~ -32099)
const PANE_NOT_FOUND: i64 = -32001;
const TAB_NOT_FOUND: i64 = -32002;
const WINDOW_NOT_FOUND: i64 = -32003;
const SPLIT_FAILED: i64 = -32004;
const SPAWN_FAILED: i64 = -32005;
const PERMISSION_DENIED: i64 = -32006;
const VERSION_MISMATCH: i64 = -32007;
const SUBSCRIPTION_FAILED: i64 = -32008;
```

### 3.6 버전 협상 및 핸드셰이크

```rust
// 클라이언트 → 서버: 첫 연결 시
{
    "jsonrpc": "2.0",
    "method": "initialize",
    "params": {
        "protocol_version": "1.0",
        "client_name": "crux-cli",
        "client_version": "0.1.0",
        "capabilities": {
            "subscriptions": true,
            "binary_frames": false
        }
    },
    "id": 0
}

// 서버 → 클라이언트: 응답
{
    "jsonrpc": "2.0",
    "result": {
        "protocol_version": "1.0",
        "server_name": "crux-terminal",
        "server_version": "0.1.0",
        "capabilities": {
            "subscriptions": true,
            "binary_frames": true,
            "max_panes": 256
        }
    },
    "id": 0
}
```

---

## 4. 터미널 IPC 보안 패턴

### 4.1 Unix 소켓 권한 모델

#### WezTerm의 보안 패턴

**소스: `wezterm-mux-server-impl/src/local.rs` - `safely_create_sock_path()`**

```rust
fn safely_create_sock_path(unix_dom: &UnixDomain) -> anyhow::Result<UnixListener> {
    let sock_dir = sock_path.parent()?;

    // 1. 사용자 소유 디렉토리 생성
    create_user_owned_dirs(sock_dir)?;

    // 2. 디렉토리 권한 검증 (Unix)
    #[cfg(unix)]
    {
        if !running_under_wsl() && !unix_dom.skip_permissions_check {
            let meta = sock_dir.symlink_metadata()?;
            let permissions = meta.permissions();
            // 그룹/기타 사용자 쓰기 권한이 있으면 거부
            if (permissions.mode() & 0o22) != 0 {
                anyhow::bail!(
                    "The permissions for {} are insecure and currently \
                     allow other users to write to it",
                    sock_dir.display()
                );
            }
        }
    }

    // 3. 기존 소켓 제거 후 바인드
    std::fs::remove_file(sock_path).ok();
    let listener = UnixListener::bind(sock_path)?;

    // 4. sticky bit 설정
    config::set_sticky_bit(&sock_path);

    Ok(listener)
}
```

#### Docker의 보안 패턴

- 기본 소켓 파일: `/var/run/docker.sock`
- 권한: root 소유, docker 그룹, 모드 660
- docker 그룹 멤버십으로 접근 제어
- 원격 접근 시 TLS 클라이언트 인증서 필수

#### Crux 권장 보안 모델

```rust
const SOCKET_DIR_MODE: u32 = 0o700;  // 소유자만 접근
const SOCKET_FILE_MODE: u32 = 0o600; // 소유자만 읽기/쓰기

fn create_secure_socket(runtime_dir: &Path) -> anyhow::Result<UnixListener> {
    // 1. XDG_RUNTIME_DIR 사용 (이미 사용자별로 격리됨)
    let sock_dir = runtime_dir.join("crux");
    std::fs::create_dir_all(&sock_dir)?;

    // 2. 디렉토리 권한 설정
    std::fs::set_permissions(&sock_dir,
        std::fs::Permissions::from_mode(SOCKET_DIR_MODE))?;

    // 3. 디렉토리 소유권 검증
    let meta = sock_dir.symlink_metadata()?;
    let uid = std::os::unix::fs::MetadataExt::uid(&meta);
    if uid != nix::unistd::getuid().as_raw() {
        anyhow::bail!("Socket directory owned by different user");
    }

    // 4. 기존 소켓 정리 및 바인드
    let sock_path = sock_dir.join("ipc.sock");
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path)?;

    // 5. 소켓 파일 권한 설정
    std::fs::set_permissions(&sock_path,
        std::fs::Permissions::from_mode(SOCKET_FILE_MODE))?;

    Ok(listener)
}
```

### 4.2 피어 자격증명 검증 (UCred)

```rust
use tokio::net::UnixStream;

async fn verify_peer(stream: &UnixStream) -> anyhow::Result<()> {
    let cred = stream.peer_cred()?;

    // UID 검증: 동일 사용자만 허용
    let my_uid = nix::unistd::getuid();
    if cred.uid() != my_uid.as_raw() {
        anyhow::bail!(
            "Peer UID {} does not match server UID {}",
            cred.uid(), my_uid
        );
    }

    // PID 로깅 (디버깅/감사 용도)
    if let Some(pid) = cred.pid() {
        tracing::info!("Accepted connection from PID {}", pid);
    }

    Ok(())
}
```

**플랫폼별 구현:**
- **Linux**: `SO_PEERCRED` 소켓 옵션 (`getsockopt`)
- **macOS/BSD**: `getpeereid()` 함수
- **tokio**: `UnixStream::peer_cred()` (크로스플랫폼 추상화)

### 4.3 토큰 기반 추가 인증

파일 시스템 권한 + UCred로 대부분의 보안 요구사항을 충족하지만, 추가 보안이 필요한 경우:

```rust
use rand::Rng;
use std::fs;

struct TokenAuth {
    token_path: PathBuf,
}

impl TokenAuth {
    fn generate_token(runtime_dir: &Path) -> anyhow::Result<Self> {
        let token: [u8; 32] = rand::thread_rng().gen();
        let token_hex = hex::encode(token);
        let token_path = runtime_dir.join("crux").join("auth-token");

        fs::write(&token_path, &token_hex)?;
        fs::set_permissions(&token_path,
            fs::Permissions::from_mode(0o600))?;

        Ok(Self { token_path })
    }

    fn verify(&self, provided_token: &str) -> bool {
        if let Ok(stored) = fs::read_to_string(&self.token_path) {
            // 상수 시간 비교로 타이밍 공격 방지
            constant_time_eq(stored.as_bytes(), provided_token.as_bytes())
        } else {
            false
        }
    }
}
```

### 4.4 보안 계층 요약

```
┌──────────────────────────────────────────┐
│ 계층 1: 파일 시스템 권한                   │
│   - 디렉토리: 0o700 (소유자만)             │
│   - 소켓: 0o600 (소유자만 읽기/쓰기)       │
├──────────────────────────────────────────┤
│ 계층 2: 피어 자격증명 (UCred)              │
│   - UID 일치 확인                          │
│   - PID 로깅 (감사)                       │
├──────────────────────────────────────────┤
│ 계층 3: 핸드셰이크 검증                    │
│   - 프로토콜 버전 확인                     │
│   - 클라이언트 capabilities 교환           │
├──────────────────────────────────────────┤
│ 계층 4: 토큰 인증 (선택적)                 │
│   - 파일 기반 공유 비밀                    │
│   - 상수 시간 비교                        │
└──────────────────────────────────────────┘
```

---

## 5. 이벤트 구독 패턴

### 5.1 JSON-RPC 2.0 Pub/Sub 패턴

Ethereum의 JSON-RPC pub/sub 패턴을 터미널 IPC에 적용:

#### 구독 생성

```json
// 클라이언트 → 서버
{
    "jsonrpc": "2.0",
    "method": "subscribe",
    "params": {
        "events": ["pane_output", "pane_added", "pane_removed", "pane_focused"]
    },
    "id": 1
}

// 서버 → 클라이언트
{
    "jsonrpc": "2.0",
    "result": {
        "subscription_id": "sub_a1b2c3d4"
    },
    "id": 1
}
```

#### 이벤트 알림 (Notification)

```json
// 서버 → 클라이언트 (id 없음 = notification)
{
    "jsonrpc": "2.0",
    "method": "subscription",
    "params": {
        "subscription": "sub_a1b2c3d4",
        "event": "pane_added",
        "data": {
            "pane_id": 3,
            "tab_id": 0,
            "window_id": 0,
            "title": "zsh",
            "cwd": "/Users/me/project"
        }
    }
}
```

#### 구독 해제

```json
{
    "jsonrpc": "2.0",
    "method": "unsubscribe",
    "params": { "subscription_id": "sub_a1b2c3d4" },
    "id": 2
}
```

### 5.2 WezTerm 이벤트 처리 참조

WezTerm의 `dispatch.rs`는 Mux 구독을 통해 패인 생명주기 이벤트를 클라이언트에 푸시한다:

```rust
// 핵심 패턴: 서버가 Mux 이벤트를 구독하고, 연결된 클라이언트에 전달
mux.subscribe(move |notification| {
    tx.try_send(Item::Notif(notification)).is_ok()
    // false를 반환하면 구독 해제
});

// 이벤트 처리
match notification {
    MuxNotification::PaneRemoved(pane_id) => {
        // 서버 → 클라이언트 알림 전송
        Pdu::PaneRemoved(codec::PaneRemoved { pane_id })
            .encode_async(&mut stream, 0).await?;
    }
    MuxNotification::PaneOutput(pane_id) => {
        handler.schedule_pane_push(pane_id);
    }
    MuxNotification::PaneFocused(pane_id) => {
        Pdu::PaneFocused(codec::PaneFocused { pane_id })
            .encode_async(&mut stream, 0).await?;
    }
    // ...
}
```

### 5.3 고빈도 이벤트 백프레셔 처리

터미널 출력은 매우 고빈도 이벤트를 생성한다. WezTerm의 접근 방식을 참고한 백프레셔 전략:

#### 전략 1: 채널 용량 제한 + 드롭

```rust
// 바운드 채널로 백프레셔 적용
let (event_tx, event_rx) = tokio::sync::mpsc::channel::<Event>(256);

// 전송 실패 시 (채널 가득 참) → 이벤트 드롭
match event_tx.try_send(event) {
    Ok(()) => {}
    Err(TrySendError::Full(_)) => {
        tracing::warn!("Event channel full, dropping event");
        // 느린 클라이언트에 대해 이벤트 드롭은 안전
        // (클라이언트는 다음 폴링에서 최신 상태를 가져올 수 있음)
    }
    Err(TrySendError::Closed(_)) => break,
}
```

#### 전략 2: 코얼레싱 (병합)

```rust
// WezTerm의 mux_output_parser_coalesce_delay_ms 참조
struct CoalescingEventSender {
    pending: HashMap<PaneId, Instant>,
    delay: Duration,
}

impl CoalescingEventSender {
    async fn schedule_output_event(&mut self, pane_id: PaneId) {
        // 같은 패인의 출력 이벤트는 짧은 시간 내에 병합
        if self.pending.contains_key(&pane_id) {
            return; // 이미 스케줄됨
        }
        self.pending.insert(pane_id, Instant::now());

        tokio::time::sleep(self.delay).await;
        self.pending.remove(&pane_id);

        // 병합된 단일 이벤트 전송
        self.send_coalesced_event(pane_id).await;
    }
}
```

#### 전략 3: 이벤트 타입별 차등 처리

```rust
enum EventPriority {
    /// 즉시 전달 (구조적 변경)
    Immediate,  // PaneAdded, PaneRemoved, SplitChanged
    /// 코얼레싱 가능 (빈번한 업데이트)
    Coalesced,  // PaneOutput, CursorMoved, TitleChanged
    /// 요청 시에만 (폴링)
    OnDemand,   // RenderContent, ScrollbackData
}

fn classify_event(event: &MuxNotification) -> EventPriority {
    match event {
        MuxNotification::PaneAdded(_) |
        MuxNotification::PaneRemoved(_) |
        MuxNotification::WindowCreated(_) => EventPriority::Immediate,

        MuxNotification::PaneOutput(_) |
        MuxNotification::PaneFocused(_) |
        MuxNotification::TabTitleChanged { .. } => EventPriority::Coalesced,

        _ => EventPriority::OnDemand,
    }
}
```

### 5.4 구독 관리 구현

```rust
use std::collections::HashMap;
use tokio::sync::broadcast;

struct SubscriptionManager {
    subscriptions: HashMap<String, Subscription>,
    mux_events: broadcast::Receiver<MuxNotification>,
}

struct Subscription {
    id: String,
    client_tx: tokio::sync::mpsc::Sender<JsonRpcNotification>,
    event_filter: Vec<String>,
    created_at: Instant,
}

impl SubscriptionManager {
    fn subscribe(
        &mut self,
        events: Vec<String>,
        client_tx: tokio::sync::mpsc::Sender<JsonRpcNotification>,
    ) -> String {
        let id = format!("sub_{}", uuid::Uuid::new_v4().simple());
        self.subscriptions.insert(id.clone(), Subscription {
            id: id.clone(),
            client_tx,
            event_filter: events,
            created_at: Instant::now(),
        });
        id
    }

    fn unsubscribe(&mut self, subscription_id: &str) -> bool {
        self.subscriptions.remove(subscription_id).is_some()
    }

    async fn dispatch_event(&self, event: MuxNotification) {
        let event_name = event.name();
        for sub in self.subscriptions.values() {
            if sub.event_filter.contains(&event_name.to_string()) {
                let notification = JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "subscription".to_string(),
                    params: serde_json::json!({
                        "subscription": sub.id,
                        "event": event_name,
                        "data": event.to_json(),
                    }),
                };
                let _ = sub.client_tx.try_send(notification);
            }
        }
    }
}
```

---

## 6. Crux 터미널을 위한 설계 권장사항

### 6.1 IPC 아키텍처 제안

```
┌─────────────────┐       JSON-RPC 2.0       ┌──────────────────────┐
│  crux cli        │ ◄───────────────────────► │  crux (GUI)          │
│  (클라이언트)     │  Length-prefix framing   │                      │
│                  │  over Unix Socket        │  ┌──────────────┐    │
│  crux-cli        │                          │  │ IPC Server    │    │
│  split-pane      │                          │  │ (tokio)       │    │
│  list            │                          │  └──────┬───────┘    │
│  spawn           │                          │         │            │
│  send-text       │                          │  ┌──────┴───────┐    │
│  activate-pane   │                          │  │ Mux           │    │
│  kill-pane       │                          │  │ (패인/탭 관리)  │    │
│  subscribe       │                          │  └──────────────┘    │
└─────────────────┘                          └──────────────────────┘
     │
     │ Claude Code 통합
     │
     ▼
┌─────────────────┐
│ Claude Code      │
│ teammateMode:    │
│   "crux"         │
│                  │
│ TERM_PROGRAM=    │
│   crux           │
└─────────────────┘
```

### 6.2 소켓 경로 전략

```rust
// 소켓 경로 결정 순서
fn compute_socket_path() -> PathBuf {
    // 1. CRUX_SOCKET 환경변수 (패인 내부에서 자동 설정)
    if let Ok(path) = std::env::var("CRUX_SOCKET") {
        return PathBuf::from(path);
    }

    // 2. XDG_RUNTIME_DIR 기반 (권장)
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir)
            .join("crux")
            .join(format!("gui-{}.sock", std::process::id()));
    }

    // 3. macOS 폴백: ~/Library/Application Support/crux/
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            return home
                .join("Library")
                .join("Application Support")
                .join("crux")
                .join(format!("gui-{}.sock", std::process::id()));
        }
    }

    // 4. 최종 폴백: /tmp/crux-{uid}/
    PathBuf::from(format!("/tmp/crux-{}", nix::unistd::getuid()))
        .join(format!("gui-{}.sock", std::process::id()))
}
```

### 6.3 Claude Code 통합을 위한 환경변수

```rust
// 패인 스폰 시 환경변수 설정
fn spawn_pane_env(pane_id: PaneId, socket_path: &Path) -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("TERM_PROGRAM".to_string(), "crux".to_string());
    env.insert("TERM_PROGRAM_VERSION".to_string(), env!("CARGO_PKG_VERSION").to_string());
    env.insert("CRUX_SOCKET".to_string(), socket_path.to_string_lossy().to_string());
    env.insert("CRUX_PANE".to_string(), pane_id.to_string());
    env
}
```

### 6.4 CLI 명령어 설계

```
crux cli split-pane [--right|--bottom|--left|--top] [--pane-id N] [-- PROG...]
crux cli spawn [--window-id N] [-- PROG...]
crux cli list [--format json|table]
crux cli activate-pane --pane-id N
crux cli kill-pane --pane-id N
crux cli send-text --pane-id N [TEXT]
crux cli get-text --pane-id N [--start-line N] [--end-line N]
crux cli resize --pane-id N --rows R --cols C
crux cli subscribe [--events pane_added,pane_removed,...]
crux cli version
```

### 6.5 RPC 메서드 정의

```rust
// 핵심 메서드 정의
const METHODS: &[(&str, &str)] = &[
    // 연결 관리
    ("initialize",      "핸드셰이크 및 capabilities 교환"),
    ("ping",            "연결 상태 확인"),
    ("shutdown",        "정상적 연결 종료"),

    // 패인 관리
    ("split_pane",      "패인 분할"),
    ("spawn",           "새 패인 생성"),
    ("kill_pane",       "패인 종료"),
    ("list_panes",      "패인 목록 조회"),
    ("activate_pane",   "패인 포커스"),
    ("resize_pane",     "패인 크기 조정"),
    ("get_pane_info",   "패인 정보 조회"),

    // 입출력
    ("write_to_pane",   "패인에 텍스트 입력"),
    ("send_key",        "키 이벤트 전송"),
    ("get_text",        "패인 텍스트 읽기"),

    // 구독
    ("subscribe",       "이벤트 구독"),
    ("unsubscribe",     "이벤트 구독 해제"),

    // 윈도우/탭
    ("list_windows",    "윈도우 목록"),
    ("list_tabs",       "탭 목록"),
];
```

### 6.6 프로젝트 구조 제안

```
crates/
├── crux-ipc-protocol/      # 프로토콜 정의 (공유)
│   ├── src/
│   │   ├── lib.rs
│   │   ├── methods.rs      # RPC 메서드 정의
│   │   ├── types.rs        # 요청/응답 타입
│   │   ├── events.rs       # 이벤트 타입
│   │   ├── errors.rs       # 에러 코드
│   │   └── framing.rs      # 메시지 프레이밍
│   └── Cargo.toml
│
├── crux-ipc-server/        # IPC 서버 (GUI에 임베드)
│   ├── src/
│   │   ├── lib.rs
│   │   ├── server.rs       # tokio UnixListener
│   │   ├── session.rs      # 클라이언트 세션 관리
│   │   ├── dispatch.rs     # 요청 디스패칭
│   │   ├── subscriptions.rs # 이벤트 구독 관리
│   │   └── security.rs     # 소켓 보안/UCred
│   └── Cargo.toml
│
├── crux-ipc-client/        # IPC 클라이언트 라이브러리
│   ├── src/
│   │   ├── lib.rs
│   │   ├── client.rs       # 연결 및 요청
│   │   ├── discovery.rs    # 소켓 발견
│   │   └── reconnect.rs    # 재연결 로직
│   └── Cargo.toml
│
└── crux-cli/               # CLI 바이너리
    ├── src/
    │   ├── main.rs
    │   ├── commands/
    │   │   ├── split_pane.rs
    │   │   ├── spawn.rs
    │   │   ├── list.rs
    │   │   ├── activate.rs
    │   │   └── subscribe.rs
    │   └── output.rs       # JSON/테이블 출력 포맷
    └── Cargo.toml
```

### 6.7 의존성 추천

```toml
# crux-ipc-protocol/Cargo.toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"

# crux-ipc-server/Cargo.toml
[dependencies]
crux-ipc-protocol = { path = "../crux-ipc-protocol" }
tokio = { version = "1", features = ["net", "rt-multi-thread", "sync", "macros"] }
tokio-util = { version = "0.7", features = ["codec"] }
futures = "0.3"
tracing = "0.1"
nix = { version = "0.29", features = ["socket", "user"] }
uuid = { version = "1", features = ["v4"] }
bytes = "1"

# crux-ipc-client/Cargo.toml
[dependencies]
crux-ipc-protocol = { path = "../crux-ipc-protocol" }
tokio = { version = "1", features = ["net", "sync", "macros"] }
tokio-util = { version = "0.7", features = ["codec"] }
futures = "0.3"
dirs = "5"

# crux-cli/Cargo.toml
[dependencies]
crux-ipc-client = { path = "../crux-ipc-client" }
clap = { version = "4", features = ["derive"] }
serde_json = "1"
tabled = "0.17"  # 테이블 출력
tokio = { version = "1", features = ["rt", "macros"] }
```

---

## 참고 자료

### WezTerm 소스코드
- [codec/src/lib.rs](https://github.com/wezterm/wezterm/blob/main/codec/src/lib.rs) - PDU 프로토콜 정의
- [mux/src/lib.rs](https://github.com/wezterm/wezterm/blob/main/mux/src/lib.rs) - Mux 아키텍처 및 알림 시스템
- [wezterm-client/src/client.rs](https://github.com/wezterm/wezterm/blob/main/wezterm-client/src/client.rs) - 클라이언트 연결
- [wezterm-client/src/discovery.rs](https://github.com/wezterm/wezterm/blob/main/wezterm-client/src/discovery.rs) - 소켓 발견
- [wezterm-mux-server-impl/src/local.rs](https://github.com/wezterm/wezterm/blob/main/wezterm-mux-server-impl/src/local.rs) - LocalListener
- [wezterm-mux-server-impl/src/dispatch.rs](https://github.com/wezterm/wezterm/blob/main/wezterm-mux-server-impl/src/dispatch.rs) - 세션 디스패칭
- [wezterm-mux-server-impl/src/sessionhandler.rs](https://github.com/wezterm/wezterm/blob/main/wezterm-mux-server-impl/src/sessionhandler.rs) - PDU 핸들러

### Claude Code Agent Teams
- [Claude Code Agent Teams 문서](https://code.claude.com/docs/en/agent-teams)
- [WezTerm 백엔드 feature request #23574](https://github.com/anthropics/claude-code/issues/23574)
- [Ghostty 백엔드 feature request #24189](https://github.com/anthropics/claude-code/issues/24189)
- [tmux fallback bug #23572](https://github.com/anthropics/claude-code/issues/23572)

### JSON-RPC 2.0
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- [Geth Pub/Sub over JSON-RPC](https://geth.ethereum.org/docs/interacting-with-geth/rpc/pubsub)
- [jsonrpsee (Rust)](https://github.com/paritytech/jsonrpsee)
- [jsonrpc-pubsub (Rust)](https://docs.rs/jsonrpc-pubsub/)

### 보안
- [Docker Daemon Socket 보안](https://docs.docker.com/engine/security/protect-access/)
- [tokio UCred](https://docs.rs/tokio/latest/tokio/net/unix/struct.UCred.html)
- [Rust std UCred](https://doc.rust-lang.org/nightly/src/std/os/unix/net/ucred.rs.html)

### WezTerm 아키텍처
- [WezTerm DeepWiki - Multiplexer Architecture](https://deepwiki.com/wezterm/wezterm/2.3-multiplexer-architecture)
- [WezTerm Multiplexing 문서](https://wezterm.org/multiplexing.html)
- [varbincode 라이브러리](https://github.com/wez/varbincode)
