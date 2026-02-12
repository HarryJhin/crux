<p align="center">
  <img src="extra/crux-logo.svg" width="160" alt="Crux">
</p>

<h1 align="center">Crux</h1>

<p align="center">
  Rust와 Metal로 만든 macOS용 GPU 가속 터미널 에뮬레이터.<br>
  AI 코딩 시대를 위해 설계 — 네이티브 MCP 서버와 한국어/CJK IME를 핵심에 두었습니다.
</p>

<p align="center">
  <a href="README.md">English</a> · <a href="README.ko.md"><strong>한국어</strong></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/상태-초기%20개발-orange" alt="상태: 초기 개발">
  <img src="https://img.shields.io/badge/플랫폼-macOS%2013%2B-blue" alt="macOS 13+">
  <img src="https://img.shields.io/badge/라이선스-MIT%20%2F%20Apache--2.0-green" alt="라이선스: MIT / Apache-2.0">
</p>

> **초기 개발 단계 (6단계 중 1단계)**
> 터미널 기본 렌더링이 동작합니다. 아래 대부분의 기능은 로드맵에 있습니다.
> 전체 구현 계획은 [PLAN.md](PLAN.md)를 참고하세요.

<!-- TODO: 스크린샷 추가 예정 -->

---

## 왜 Crux인가?

Claude Code 같은 AI 코딩 도구는 **터미널 패널을 프로그래밍 방식으로 제어**해야 합니다 — 화면 분할, 명령 전송, 출력 읽기. 지금은 AppleScript 래퍼를 덕지덕지 붙이거나 tmux를 거쳐야만 가능합니다. 이걸 네이티브로 지원하는 터미널은 없습니다.

한편, **macOS의 모든 터미널은 한국어 입력 버그가 있습니다.** Alacritty는 한글 조합 중 스페이스가 두 번 입력됩니다. Ghostty는 수정 키를 누르면 조합 중인 한글이 깨집니다. iTerm2는 한자 변환 창 위치가 어긋납니다. 이건 소수의 문제가 아닙니다 — 수백만 CJK 사용자가 매일 겪는 일입니다.

Crux는 이 두 문제를 근본부터 해결하기 위해 만들어졌습니다:

- **네이티브 MCP 서버** — AI 에이전트(Claude Desktop, Claude Code, Cursor)가 설정 없이 Crux를 직접 제어
- **한국어/CJK IME 최우선 지원** — 나중에 붙인 게 아니라, 첫날부터 핵심 설계 원칙
- **프로그래밍 가능한 패널 제어** — 분할, 명령 실행, 출력 모니터링을 위한 실제 API
- **GPU 가속** — [GPUI](https://gpui.rs)를 통한 Metal 렌더링, 120 FPS 목표

---

## 현재 구현된 기능

- GPUI를 통한 Metal GPU 렌더링 터미널 창
- [alacritty_terminal](https://github.com/alacritty/alacritty) 기반 VT100/xterm 에뮬레이션
- 트루컬러(24비트 RGB) + 256 색상 지원
- 수정 키 처리를 포함한 키보드 입력
- SGR 마우스 리포팅 및 괄호 붙여넣기(bracketed paste)
- 커스텀 terminfo 엔트리(`xterm-crux`), `xterm-256color` 폴백

---

## 로드맵

| 단계 | 주요 내용 | 상태 |
|------|-----------|------|
| **1. 기본 터미널** | 셸 렌더링, 키보드, VT 에뮬레이션, terminfo | **진행 중** |
| **2. 탭 & 패널** | 화면 분할, IPC 서버, CLI 클라이언트, 셸 통합 | 예정 |
| **3. 한국어/CJK IME** | NSTextInputClient, 한글 조합, 후보 창 | 예정 |
| **4. 리치 기능** | 마크다운 미리보기, 클릭 가능한 링크, 그래픽스 프로토콜 | 예정 |
| **5. AI 통합** | 네이티브 MCP 서버(30개 도구), tmux 호환, 설정 시스템 | 예정 |
| **6. 배포** | Homebrew, 코드 서명, 공증, 유니버설 바이너리 | 예정 |

자세한 체크리스트(200개 이상 항목)는 [PLAN.md](PLAN.md)를 참고하세요.

---

## 소스에서 빌드

**사전 요구 사항**: macOS 13+(Ventura), Rust 안정 툴체인, **Xcode.app** 전체 설치(Command Line Tools만으로는 불가 — Metal 셰이더 컴파일에 전체 IDE가 필요합니다).

```bash
# Metal 컴파일러 확인
xcrun -sdk macosx metal --version

# 클론, 빌드, 실행
git clone https://github.com/HarryJhin/crux.git
cd crux
cargo run -p crux-app
```

선택 사항: terminfo 엔트리를 컴파일하면 터미널 기능 협상이 완전해집니다.

```bash
tic -x -e xterm-crux,crux,crux-direct extra/crux.terminfo
```

---

## 프로젝트 구조

7개의 크레이트로 구성된 Cargo 워크스페이스:

```
crux-terminal       VT 에뮬레이션 (alacritty_terminal + portable-pty)
crux-terminal-view  GPU 렌더링 (GPUI 캔버스, 셀, 커서, 선택)
crux-app            애플리케이션 셸 (윈도우 관리, GPUI 부트스트랩)
crux-protocol       공유 타입 및 프로토콜 정의                    [스텁]
crux-ipc            유닉스 소켓 서버, JSON-RPC 2.0               [스텁]
crux-clipboard      리치 클립보드 및 드래그 앤 드롭              [스텁]
crux-mcp            네이티브 MCP 서버                          [예정]
```

아키텍처 결정과 기술 심층 분석은 [research/](research/) 디렉토리를 참고하세요.

---

## 기여하기

기여를 환영합니다 — 프로젝트가 초기 단계이므로 참여하기 좋은 시점입니다.

- **언어**: Rust (안정 툴체인)
- **스타일**: `cargo fmt` + `cargo clippy -- -D warnings`
- **커밋**: [Conventional Commits](https://www.conventionalcommits.org/) 및 크레이트명 스코프 (예: `feat(terminal): add sixel support`)
- **테스트**: 새 기능에는 테스트 필수

---

## 라이선스

[MIT](LICENSE-MIT)와 [Apache 2.0](LICENSE-APACHE) 이중 라이선스. 원하는 쪽을 선택하세요.

---

<p align="center">
  <strong>Crux</strong> — 라틴어로 "핵심", 그리고 남십자성(Southern Cross).<br>
  AI 코딩을 위한 터미널 UX의 핵심.
</p>
