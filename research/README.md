# Research Documentation Index

> Crux 터미널 에뮬레이터 개발을 위한 기술 조사 문서 모음 (총 29편, ~810KB)
>
> 모든 문서는 YAML frontmatter를 포함하며, `phase`, `topics`, `related` 필드로 탐색 가능합니다.

---

## Directory Structure

```
research/
├── README.md                 ← 이 파일 (마스터 인덱스)
├── gap-analysis.md           ← 메타: 누락 영역 식별 (current)
├── core/                     ← 터미널 코어 기술
│   ├── terminal-emulation.md     VT 파서, PTY, 그래픽스 프로토콜, tmux, Unicode
│   ├── terminal-architecture.md  Alacritty/WezTerm/Rio/Ghostty 아키텍처 분석
│   ├── keymapping.md             키 입력 → 이스케이프 시퀀스 + Kitty 키보드 프로토콜
│   ├── terminfo.md               Terminfo 소스, 컴파일, TERM 전략
│   ├── shell-integration.md      쉘 통합: OSC 7/133/1337, 자동 주입
│   ├── config-system.md          설정 시스템: TOML, 핫 리로드, 스키마
│   ├── testing-strategy.md       테스트 전략: vttest, esctest2, fuzzing, CI
│   ├── font-system.md            폰트: Core Text, CJK 폴백, 리거처
│   ├── accessibility.md          접근성: VoiceOver, AccessKit, WCAG
│   ├── performance.md            성능: 지연시간, 처리량, 120fps, 메모리
│   ├── hyperlinks.md             하이퍼링크: OSC 8, URL 탐지, 보안
│   ├── mouse-reporting.md        마우스: 트래킹 모드, SGR 인코딩
│   ├── graphics-protocols.md     ★ 그래픽스: Kitty/iTerm2/Sixel 프로토콜, Ghostty 구현
│   └── tmux-compatibility.md     ★ tmux: VT 호환 매트릭스, Control Mode, DECLRMM
├── gpui/                     ← GPUI 프레임워크
│   ├── framework.md              GPUI 렌더링 파이프라인, 컴포넌트, IME, 제한사항
│   ├── terminal-implementations.md  gpui-ghostty/Zed/gpui-terminal 소스 분석
│   ├── bootstrap.md              Cargo workspace 설정, 빌드 환경, 최소 앱
│   └── widgets-integration.md    ★ DockArea, Tabs, ResizablePanel, Markdown
├── integration/              ← IPC 및 외부 통합
│   ├── ipc-external-patterns.md  WezTerm 내부, JSON-RPC, 보안, 이벤트 구독
│   ├── ipc-protocol-design.md    Crux IPC/프로토콜 설계, PaneBackend 인터페이스
│   ├── claude-code-strategy.md   Claude Code 저장소 분석, Feature Request 전략
│   └── mcp-integration.md        ★ MCP 통합: rmcp SDK, 30개 도구 설계, 아키텍처
├── competitive/              ← ★ 경쟁 분석 (신규)
│   ├── ghostty-warp-analysis.md  Ghostty 아키텍처 + Warp 대체 분석 + 포지셔닝
│   ├── terminal-structures.md    ★ 5대 터미널 프로젝트 구조 비교 (Alacritty/WezTerm/Rio/Ghostty/Zed)
│   └── warp-settings-analysis.md ★ Warp 설정 기능 조사 및 Crux 채택 분석
├── testing/                  ← ★ 테스팅 인프라 (신규)
│   └── ai-agent-testing.md       AI 에이전트 테스팅: MCP 도구 7개, 셀프 테스팅, CI/CD
└── platform/                 ← macOS 플랫폼 네이티브
    ├── ime-clipboard.md          한국어 IME, NSPasteboard, objc2, 드래그&드롭
    ├── homebrew-distribution.md  Homebrew, 코드 서명, 공증, Universal Binary
    └── vim-ime-switching.md      ★ Vim 모드 IME 자동전환, TISSelectInputSource
```

> ★ = 이번 리서치 스프린트에서 신규 추가된 문서

---

## Documents by Phase

### Phase 1: Basic Terminal (MVP)
| Document | Key Topics |
|----------|------------|
| [core/terminal-emulation.md](core/terminal-emulation.md) | VT 파서 비교, PTY, 그래픽스, tmux, Unicode/CJK |
| [core/terminal-architecture.md](core/terminal-architecture.md) | 4대 터미널 아키텍처 패턴 |
| [core/keymapping.md](core/keymapping.md) | 이스케이프 시퀀스, Kitty 프로토콜, macOS Option |
| [core/terminfo.md](core/terminfo.md) | terminfo 생성, TERM 전략, 현대적 capability |
| [gpui/framework.md](gpui/framework.md) | GPUI 개요, 렌더링, IME, 제한사항 |
| [gpui/terminal-implementations.md](gpui/terminal-implementations.md) | 기존 GPUI 터미널 소스 분석 |
| [gpui/bootstrap.md](gpui/bootstrap.md) | Cargo workspace, 빌드 설정, 최소 앱 |
| [core/testing-strategy.md](core/testing-strategy.md) | vttest, esctest2, ref 테스트, fuzzing, CI 파이프라인 |
| [core/font-system.md](core/font-system.md) | Core Text, CJK 폴백 체인, 리거처, 박스 드로잉 |
| [core/performance.md](core/performance.md) | 입력 지연시간, 처리량, 120fps Metal, 메모리 관리 |
| [core/mouse-reporting.md](core/mouse-reporting.md) | 마우스 트래킹 모드, SGR 인코딩, Shift 바이패스 |

### Phase 2: Tabs, Panes, IPC
| Document | Key Topics |
|----------|------------|
| [integration/ipc-external-patterns.md](integration/ipc-external-patterns.md) | WezTerm CLI 소스 분석, JSON-RPC 패턴, 보안 |
| [integration/ipc-protocol-design.md](integration/ipc-protocol-design.md) | Crux 프로토콜 설계, CLI 인터페이스 |
| [integration/mcp-integration.md](integration/mcp-integration.md) | ★ MCP 프로토콜, rmcp SDK, 30개 도구 설계, 아키텍처 |
| [core/shell-integration.md](core/shell-integration.md) | OSC 7/133/1337, 쉘 자동 주입, 명령 경계 |
| [gpui/widgets-integration.md](gpui/widgets-integration.md) | ★ DockArea, Tabs, ResizablePanel 통합 패턴 |

### Phase 3: Korean/CJK IME, Rich Clipboard
| Document | Key Topics |
|----------|------------|
| [platform/ime-clipboard.md](platform/ime-clipboard.md) | NSTextInputClient, 한국어 IME 실패 분석, NSPasteboard |
| [platform/vim-ime-switching.md](platform/vim-ime-switching.md) | ★ Vim 모드 IME 자동전환, DECSCUSR, TIS API |

### Phase 4: Markdown Preview, Graphics, Kitty Protocol
| Document | Key Topics |
|----------|------------|
| [core/graphics-protocols.md](core/graphics-protocols.md) | ★ Kitty/iTerm2/Sixel 프로토콜, 구현 아키텍처 |
| [core/terminal-emulation.md](core/terminal-emulation.md) | Sixel/Kitty/iTerm2 그래픽스 프로토콜 (개요) |
| [core/keymapping.md](core/keymapping.md) | Kitty keyboard protocol (§16 상세) |
| [core/hyperlinks.md](core/hyperlinks.md) | OSC 8 하이퍼링크, URL 탐지, 보안 |

### Phase 5: tmux, Claude Code Integration
| Document | Key Topics |
|----------|------------|
| [core/tmux-compatibility.md](core/tmux-compatibility.md) | ★ VT 호환 매트릭스, Control Mode, DECLRMM |
| [integration/ipc-protocol-design.md](integration/ipc-protocol-design.md) | Claude Code Agent Teams PaneBackend |
| [integration/claude-code-strategy.md](integration/claude-code-strategy.md) | Feature Request 전략, 커뮤니티 참여 |
| [integration/mcp-integration.md](integration/mcp-integration.md) | ★ MCP 차별화 도구 10개, 보안, 구현 로드맵 |
| [core/config-system.md](core/config-system.md) | TOML 설정, 핫 리로드, figment, 스키마 검증 |

### Phase 6: Homebrew Distribution
| Document | Key Topics |
|----------|------------|
| [platform/homebrew-distribution.md](platform/homebrew-distribution.md) | Formula/Cask, CI/CD, 코드 서명, 공증 |

### Cross-Phase: Testing Infrastructure
| Document | Key Topics |
|----------|------------|
| [testing/ai-agent-testing.md](testing/ai-agent-testing.md) | ★ AI 에이전트 테스팅: 7개 MCP 도구, 셀프 테스팅, golden state, CI/CD |

### Cross-Phase: Competitive Analysis
| Document | Key Topics |
|----------|------------|
| [competitive/ghostty-warp-analysis.md](competitive/ghostty-warp-analysis.md) | ★ Ghostty 아키텍처, Warp 대체 분석, 포지셔닝 |
| [competitive/terminal-structures.md](competitive/terminal-structures.md) | ★ 프로젝트 구조 비교: 크레이트 분리, 렌더링, 최적화 패턴 |
| [competitive/warp-settings-analysis.md](competitive/warp-settings-analysis.md) | ★ Warp 설정 기능 조사: 채택 우선순위, TOML 확장 제안 |

### Future: Accessibility
| Document | Key Topics |
|----------|------------|
| [core/accessibility.md](core/accessibility.md) | VoiceOver, AccessKit, NSAccessibility, WCAG |

---

## Quick Navigation by Task

| 작업 | 시작 문서 | 참고 문서 |
|------|-----------|-----------|
| **GPUI 앱 부트스트랩** | [gpui/bootstrap.md](gpui/bootstrap.md) | [gpui/framework.md](gpui/framework.md) |
| **VT 파서 통합** | [core/terminal-emulation.md](core/terminal-emulation.md) § 1 | [core/terminal-architecture.md](core/terminal-architecture.md) |
| **키보드 입력 처리** | [core/keymapping.md](core/keymapping.md) | [core/terminfo.md](core/terminfo.md) |
| **Kitty 키보드 프로토콜** | [core/keymapping.md](core/keymapping.md) § 16 | [core/tmux-compatibility.md](core/tmux-compatibility.md) |
| **터미널 렌더링 (Element)** | [gpui/terminal-implementations.md](gpui/terminal-implementations.md) | [gpui/framework.md](gpui/framework.md) § 2 |
| **탭/분할 패널 UI** | [gpui/widgets-integration.md](gpui/widgets-integration.md) | [gpui/framework.md](gpui/framework.md) |
| **IPC 서버 구현** | [integration/ipc-protocol-design.md](integration/ipc-protocol-design.md) § 4-5 | [integration/ipc-external-patterns.md](integration/ipc-external-patterns.md) |
| **한국어 IME 구현** | [platform/ime-clipboard.md](platform/ime-clipboard.md) § 1 | [gpui/framework.md](gpui/framework.md) § 3 |
| **Vim IME 자동전환** | [platform/vim-ime-switching.md](platform/vim-ime-switching.md) | [platform/ime-clipboard.md](platform/ime-clipboard.md) |
| **클립보드/드래그&드롭** | [platform/ime-clipboard.md](platform/ime-clipboard.md) § 3 | — |
| **그래픽스 프로토콜** | [core/graphics-protocols.md](core/graphics-protocols.md) | [core/terminal-emulation.md](core/terminal-emulation.md) |
| **tmux 호환성** | [core/tmux-compatibility.md](core/tmux-compatibility.md) | [core/terminal-emulation.md](core/terminal-emulation.md) |
| **MCP 통합 (AI Agent)** | [integration/mcp-integration.md](integration/mcp-integration.md) | [integration/ipc-protocol-design.md](integration/ipc-protocol-design.md) |
| **Claude Code 통합** | [integration/claude-code-strategy.md](integration/claude-code-strategy.md) | [integration/ipc-protocol-design.md](integration/ipc-protocol-design.md) § 3 |
| **Homebrew 배포** | [platform/homebrew-distribution.md](platform/homebrew-distribution.md) | [integration/claude-code-strategy.md](integration/claude-code-strategy.md) |
| **쉘 통합 (OSC 133)** | [core/shell-integration.md](core/shell-integration.md) | [core/terminal-emulation.md](core/terminal-emulation.md) |
| **설정 시스템** | [core/config-system.md](core/config-system.md) | — |
| **테스트 전략** | [core/testing-strategy.md](core/testing-strategy.md) | [core/terminal-emulation.md](core/terminal-emulation.md) |
| **AI 에이전트 테스팅** | [testing/ai-agent-testing.md](testing/ai-agent-testing.md) | [integration/mcp-integration.md](integration/mcp-integration.md) |
| **폰트/CJK 폴백** | [core/font-system.md](core/font-system.md) | [platform/ime-clipboard.md](platform/ime-clipboard.md) |
| **접근성 (VoiceOver)** | [core/accessibility.md](core/accessibility.md) | [gpui/framework.md](gpui/framework.md) |
| **마우스 리포팅** | [core/mouse-reporting.md](core/mouse-reporting.md) | [core/keymapping.md](core/keymapping.md) |
| **성능 최적화** | [core/performance.md](core/performance.md) | [gpui/framework.md](gpui/framework.md) |
| **하이퍼링크 (OSC 8)** | [core/hyperlinks.md](core/hyperlinks.md) | [core/terminal-emulation.md](core/terminal-emulation.md) |
| **경쟁 분석/포지셔닝** | [competitive/ghostty-warp-analysis.md](competitive/ghostty-warp-analysis.md) | [competitive/terminal-structures.md](competitive/terminal-structures.md) |
| **설정 시스템 (Warp 참고)** | [competitive/warp-settings-analysis.md](competitive/warp-settings-analysis.md) | [core/config-system.md](core/config-system.md) |
| **누락 영역 확인** | [gap-analysis.md](gap-analysis.md) | 전체 |

---

## Document Relationships

```
core/terminal-emulation ◄──► core/terminal-architecture
        │                           │
        ├──► core/mouse-reporting   ├──► core/performance
        │                           │
        ├──► core/hyperlinks        ├──► core/config-system
        │                           │
        ├──► core/graphics-protocols ★   ├──► core/tmux-compatibility ★
        │                           │
        ▼                           ▼
core/keymapping ◄──► core/terminfo    gpui/framework
        │                               │
        ▼                           ┌───┴───┐───────────┐
platform/ime-clipboard          gpui/       gpui/       gpui/
        ▲                   terminal-impl  bootstrap  widgets-integ ★
        │
    ┌───┴───┐
core/       platform/
font-system vim-ime-switching ★

core/shell-integration ──► core/terminal-emulation
                       ──► integration/ipc-protocol-design

core/testing-strategy  ──► core/terminal-emulation
                       ──► core/terminal-architecture

core/accessibility     ──► core/terminal-architecture
                       ──► gpui/framework

integration/ipc-external-patterns ◄──► integration/ipc-protocol-design
                                              │
                                      ┌───────┴───────┐
                                      ▼               ▼
                           integration/        integration/
                           claude-code-strategy mcp-integration ★
                                      │
                                      ▼
                           platform/homebrew-distribution

competitive/ghostty-warp-analysis ★ ──► 전체 문서 참조

competitive/terminal-structures ★ ──► core/terminal-architecture
                                 ──► core/performance
                                 ──► gpui/terminal-implementations

competitive/warp-settings-analysis ★ ──► core/config-system

testing/ai-agent-testing ★ ──► integration/mcp-integration
                          ──► integration/claude-code-strategy
                          ──► core/terminal-emulation
                          ──► gpui/framework
```

---

## Notes

- **Integration 문서 4개의 계층**: `ipc-external-patterns.md`(외부 IPC 패턴 조사) → `ipc-protocol-design.md`(Crux IPC 설계) → `claude-code-strategy.md`(Claude Code 통합 전략) → `mcp-integration.md`(MCP 프로토콜 통합, AI 에이전트 도구 설계)
- **IPC 문서 2개의 범위 차이**: `ipc-external-patterns.md`는 외부 터미널의 IPC 패턴(WezTerm 소스 레벨, JSON-RPC, 보안)을 다루고, `ipc-protocol-design.md`는 Crux 자체 프로토콜 설계를 다룹니다.
- **GPUI 문서 4개의 계층**: `framework.md`(GPUI란 무엇인가) → `terminal-implementations.md`(다른 사람들은 어떻게 만들었나) → `bootstrap.md`(우리는 어떻게 시작하나) → `widgets-integration.md`(탭/패널을 어떻게 조합하나)
- **testing/ 디렉토리 신규**: AI 에이전트 테스팅 인프라 문서. 7개 테스팅 MCP 도구, 셀프 테스팅 아키텍처, golden state 비교, CI/CD 4계층 전략, esctest2/vtebench/insta 통합.
- **core/ 문서 확장**: 초기 4개 → 14개로 확장. 최근 그래픽스 프로토콜, tmux 호환성 문서 추가.
- **competitive/ 디렉토리 확장**: Ghostty/Warp 분석에 이어 5대 터미널(Alacritty, WezTerm, Rio, Ghostty, Zed) 프로젝트 구조 비교 문서 추가. 크레이트 분리 전략, 렌더링 파이프라인, 성능 최적화 패턴의 횡단 비교 제공.
- **competitive/ 디렉토리 3번째 문서**: Warp 설정 기능 조사. Crux config-system.md와 교차 비교하여 채택 가능 설정 항목 12개 식별, TOML 스키마 확장 제안.
- **gap-analysis.md**: 3차 업데이트 완료 (status: current). Critical 갭 해소율 79% (11/14).
- **keymapping.md 보강**: §16에 Kitty Keyboard Protocol 상세 (~307줄) 추가. CSI u 포맷, 5개 Flag, 스택 메커니즘 포함.
