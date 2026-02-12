---
title: "리서치 갭 분석"
description: "Gap analysis across 21+ research documents — identifies missing information for Phase 1-6 implementation and Homebrew distribution"
date: 2026-02-12
phase: [1, 2, 3, 4, 5, 6]
topics: [gap-analysis, meta, planning, completeness]
status: current
related:
  - core/terminal-emulation.md
  - core/keymapping.md
  - core/terminfo.md
  - core/shell-integration.md
  - core/config-system.md
  - core/testing-strategy.md
  - core/accessibility.md
  - core/font-system.md
  - core/performance.md
  - core/mouse-reporting.md
  - core/hyperlinks.md
  - gpui/bootstrap.md
  - platform/homebrew-distribution.md
  - platform/vim-ime-switching.md
  - integration/claude-code-strategy.md
---

# Crux 터미널 에뮬레이터 — 리서치 갭 분석 보고서

> 작성일: 2026-02-12
> 최종 업데이트: 2026-02-12
> 분석 범위: 21+ 리서치 문서 + README.md + PLAN.md
> 목적: Phase 1~6 구현 및 Homebrew 배포/Claude Code Feature Request 제출에 필요하지만 **누락된 정보** 식별
> 상태: **current** (3차 업데이트 완료)

---

## 업데이트 이력

### 4차 업데이트 (2026-02-12) — 터미널 프로젝트 구조 조사

> 5대 터미널 에뮬레이터(Alacritty, WezTerm, Rio, Ghostty, Zed Terminal) 프로젝트 구조 조사 완료.
> 성능 최적화 패턴(이벤트 배칭, 셀 배칭, Damage Tracking, 텍스트 런 캐싱) 발견.
>
> **새 문서 → 해소된 갭:**
>
> | 새 문서 | 해소된 갭 |
> |---------|-----------|
> | `competitive/terminal-structures.md` | GAP 8.4 부분 해소 (프로세스 종료 — Alacritty Drop impl 패턴) |
>
> **새로 발견된 최적화 갭:**
>
> | 신규 갭 | 설명 | 우선순위 |
> |---------|------|----------|
> | GAP 9.1 | 이벤트 배칭 (60fps 폴링 → 4ms/100개 배칭 전환) | Critical |
> | GAP 9.2 | 셀 배칭 (BatchedTextRun 최적화) | Important |
> | GAP 9.3 | Damage Tracking (3단계 시스템 도입) | Important |
> | GAP 9.4 | 텍스트 런 캐싱 (LRU 기반 셰이핑 캐시) | Nice-to-have |
> | GAP 9.5 | 배경 영역 병합 (인접 동색 quad 병합) | Nice-to-have |

### 3차 업데이트 (2026-02-12) — 리서치 스프린트

> 8개의 새로운 문서가 추가되어 다수의 기존 갭이 해소됨. 또한 현재 진행 중인 리서치 스프린트에서 추가 문서들이 작성되고 있음.
>
> **새 문서 → 해소된 갭:**
>
> | 새 문서 | 해소된 갭 |
> |---------|-----------|
> | `core/shell-integration.md` | GAP 2.2 (셸 통합) |
> | `core/config-system.md` | GAP 3.1 (TOML 설정), GAP 3.2 (Live Reload) |
> | `core/testing-strategy.md` | GAP 4.1 (VT 적합성 테스트), GAP 4.3 (통합 테스트) |
> | `core/accessibility.md` | GAP 5.1 (VoiceOver 지원) |
> | `core/font-system.md` | GAP 3.4 부분 해소 (테마/색상 중 폰트 관련) |
> | `core/performance.md` | GAP 4.4 (성능 벤치마킹) |
> | `core/mouse-reporting.md` | GAP 2.6 부분 해소 (마우스 이벤트 처리) |
> | `core/hyperlinks.md` | GAP 2.7 (URL 감지 정규식) |
>
> **진행 중인 리서치 스프린트:**
>
> | 주제 | 상태 |
> |------|------|
> | Graphics protocols (Kitty/iTerm2/Sixel) | 작성 중 |
> | GPUI widget integration (DockArea/Tabs) | 작성 중 |
> | Vim IME 자동 전환 (`platform/vim-ime-switching.md`) | ✅ 완료 |
> | tmux 호환성 매트릭스 | 작성 중 |
> | Ghostty 아키텍처 패턴 | 작성 중 |
> | Warp 대체 분석 | 작성 중 |
> | Kitty keyboard protocol 상세 | 작성 중 |

### 2차 업데이트 (2026-02-12) — 초기 갭 해소

> 초기 7개 문서 이후 6개 문서가 추가되어 다수의 갭이 해소됨.
>
> | 갭 | 해소 문서 |
> |-----|-----------|
> | §1 빌드/프로젝트 셋업 | `gpui/bootstrap.md` |
> | §2.1 terminfo | `core/terminfo.md` |
> | §6 배포 파이프라인 | `platform/homebrew-distribution.md` |
> | §7 Claude Code 통합 | `integration/claude-code-strategy.md` |
> | §8.3 키 매핑 | `core/keymapping.md` |

---

## 요약

리서치 문서가 21편 이상으로 확장되면서 대부분의 핵심 갭이 해소되었다. VT 파서, PTY, IME, IPC 프로토콜, 그래픽스 프로토콜에 대한 상세 조사에 더해, 셸 통합, 설정 시스템, 테스트 전략, 접근성, 폰트, 성능, 마우스 리포팅, 하이퍼링크 문서가 추가되었다.

**현재 미해소 갭은 주로 배포 및 운영 영역에 집중**되어 있으며, 핵심 기능 구현에 필요한 대부분의 리서치는 완료되었다. 진행 중인 리서치 스프린트에서 그래픽스 프로토콜, GPUI 위젯 통합, tmux 호환성 등의 추가 문서가 작성되고 있어 남은 갭도 빠르게 해소될 전망이다.

---

## 1. 빌드 및 프로젝트 셋업

### ~~GAP 1.1: Cargo 워크스페이스 구성 패턴~~ ✅ 해소
> `gpui/bootstrap.md`에서 상세히 다룸. Cargo.toml 워크스페이스 설정, `resolver = "2"`, 크레이트 간 의존성 그래프 포함.

### ~~GAP 1.2: GPUI 의존성 핀닝 전략 상세~~ ✅ 해소
> `gpui/bootstrap.md`에서 crates.io 기반 의존성 관리 전략 확정. gpui 0.2.2 + gpui-component 0.5.1 호환성 검증됨.

### ~~GAP 1.3: Metal 셰이더 빌드 시스템~~ ✅ 해소
> `gpui/bootstrap.md`에서 확인: GPUI가 Metal 셰이더를 내부적으로 처리. 별도 `build.rs` 불필요.

### GAP 1.4: 최소 macOS SDK 버전 요구사항
- **현황**: GPUI가 요구하는 최소 macOS SDK 버전, Metal API 버전, Xcode 최소 버전이 구체적으로 검증되지 않음
- **영향**: Important — CI/CD 및 Universal Binary 빌드에 영향
- **Phase**: Phase 1
- **필요 리서치 분량**: 소규모

---

## 2. 터미널 필수 기능

### ~~GAP 2.1: terminfo 엔트리 생성~~ ✅ 해소
> `core/terminfo.md`에서 상세히 다룸. `xterm-crux` TERM 전략, capability 선언, `tic` 컴파일, Homebrew 설치 방법 포함.

### ~~GAP 2.2: 셸 통합 (OSC 133, OSC 7)~~ ✅ 해소
> `core/shell-integration.md`에서 상세히 다룸. OSC 133 FinalTerm 프롬프트 마킹, OSC 7 CWD 트래킹, zsh/bash/fish 셸 통합 스크립트 작성, 자동 주입 방법 포함.

### ~~GAP 2.3: TERM/TERMINFO 환경변수 설정 전략~~ ✅ 해소
> `core/terminfo.md`에서 다룸. `TERM=xterm-crux` 전략 확정, `TERM_PROGRAM=Crux` 설정, SSH fallback 전략 포함.

### GAP 2.4: Alternate Screen Buffer 처리
- **현황**: `alacritty_terminal`이 DECSET 1049를 자체 처리하는지 구체적 검증 필요
- **영향**: Critical — vim/less/htop 등이 제대로 동작하려면 필수
- **Phase**: Phase 1
- **필요 리서치 분량**: 소규모 (alacritty_terminal API 확인으로 해소 가능)

### GAP 2.5: Bell/알림 처리
- **현황**: 구현 상세 없음
- **영향**: Nice-to-have
- **Phase**: Phase 1
- **필요 리서치 분량**: 소규모

### ~~GAP 2.6: 텍스트 Selection/Copy 메커니즘~~ ✅ 부분 해소
> `core/mouse-reporting.md`에서 마우스 이벤트 처리, 트래킹 모드, SGR 인코딩, Shift 바이패스 패턴 다룸. 다만 GPUI 레벨의 드래그 선택 렌더링 상세는 추가 조사 필요.

### ~~GAP 2.7: URL 감지 정규식 패턴~~ ✅ 해소
> `core/hyperlinks.md`에서 상세히 다룸. OSC 8 하이퍼링크, URL 탐지 정규식, 보안 고려사항 포함.

### GAP 2.8: 커서 스타일 구현 상세
- **현황**: DECSCUSR 시퀀스는 `platform/vim-ime-switching.md`에서 상세히 분석됨. 그러나 GPUI에서의 블링킹 타이머 구현, Zed `BlinkManager` 활용 패턴은 미조사.
- **영향**: Important
- **Phase**: Phase 1
- **필요 리서치 분량**: 소규모

---

## 3. 설정 시스템

### ~~GAP 3.1: TOML 설정 파일 설계~~ ✅ 해소
> `core/config-system.md`에서 상세히 다룸. TOML 스키마, figment 기반 계층적 설정, XDG Base Directory, 유효성 검증 포함.

### ~~GAP 3.2: 설정 Live Reload 메커니즘~~ ✅ 해소
> `core/config-system.md`에서 다룸. `notify` 크레이트 기반 파일 감시, 즉시 적용 vs 재시작 필요 항목 구분 포함.

### GAP 3.3: 기본 키바인딩 목록
- **현황**: `core/keymapping.md`에서 이스케이프 시퀀스 변환은 다루지만, Crux 앱 레벨 키바인딩(Cmd+T, Cmd+W 등) 전체 목록은 미정의
- **영향**: Important
- **Phase**: Phase 1
- **필요 리서치 분량**: 중규모

### ~~GAP 3.4: 테마/색상 스킴 포맷~~ ✅ 부분 해소
> `core/font-system.md`에서 폰트 관련 테마 요소(CJK 폴백 체인, 리거처) 다룸. `core/config-system.md`에서 설정 구조 다룸. 다만 16/256 ANSI 색상 팔레트 포맷, iTerm2 테마 호환, 다크/라이트 모드 전환의 구체적 구현은 추가 조사 필요.

---

## 4. 테스트 전략

### ~~GAP 4.1: VT 파서 적합성 테스트~~ ✅ 해소
> `core/testing-strategy.md`에서 상세히 다룸. vttest, esctest2, 스냅샷 테스트, ref 테스트 패턴 포함.

### GAP 4.2: IME 자동화 테스트
- **현황**: IME 실패 사례는 `platform/ime-clipboard.md`에서 분석됨. 자동 테스트 방법은 미조사.
- **영향**: Important — IME가 핵심 차별화 기능
- **Phase**: Phase 3
- **필요 리서치 분량**: 대규모

### ~~GAP 4.3: 통합 테스트 패턴~~ ✅ 해소
> `core/testing-strategy.md`에서 다룸. PTY end-to-end 테스트, CI macOS 빌드, fuzzing 전략 포함.

### ~~GAP 4.4: 성능 벤치마킹 방법론~~ ✅ 해소
> `core/performance.md`에서 상세히 다룸. 입력 지연시간, 처리량 벤치마크, 120fps Metal 렌더링, 메모리 관리 포함.

---

## 5. 접근성

### ~~GAP 5.1: VoiceOver 지원~~ ✅ 해소
> `core/accessibility.md`에서 상세히 다룸. VoiceOver, AccessKit, NSAccessibility, WCAG 요구사항 포함.

---

## 6. 배포 (Homebrew + Claude Code PR)

### ~~GAP 6.1: macOS 코드 서명 및 공증~~ ✅ 해소
> `platform/homebrew-distribution.md`에서 상세히 다룸. Apple Developer ID, codesign, notarytool, Hardened Runtime 포함.

### ~~GAP 6.2: Universal Binary (arm64 + x86_64)~~ ✅ 해소
> `platform/homebrew-distribution.md`에서 다룸. 크로스 컴파일, lipo, GitHub Actions 워크플로우 포함.

### ~~GAP 6.3: Homebrew 포뮬러 작성~~ ✅ 해소
> `platform/homebrew-distribution.md`에서 상세히 다룸. Cask vs Formula 분석, terminfo 포함, CI 테스트 포함.

### GAP 6.4: CI/CD 파이프라인 (GitHub Actions)
- **현황**: `platform/homebrew-distribution.md`에서 일부 다루지만, 전체 CI 워크플로우(테스트, 빌드, 릴리스 자동화)의 상세 설정은 미작성
- **영향**: Important
- **Phase**: Phase 1부터 점진적 구축
- **필요 리서치 분량**: 중규모

### GAP 6.5: 릴리스 엔지니어링
- **현황**: 버전 관리, CHANGELOG 자동 생성, .app 번들 구성, DMG 생성 등 미조사
- **영향**: Important
- **Phase**: Phase 5
- **필요 리서치 분량**: 중규모

### ~~GAP 6.6: 라이선스 파일~~ ✅ 해소
> Dual MIT + Apache 2.0 확정. CLAUDE.md에 명시됨.

---

## 7. Claude Code 통합

### ~~GAP 7.1: claude-code 리포지토리 코드 변경 사항 상세~~ ✅ 해소
> `integration/claude-code-strategy.md`에서 상세히 다룸. PaneBackend 인터페이스, Feature Request 전략, 커뮤니티 참여 방법 포함.

### GAP 7.2: Claude Code PR 테스트 요구사항
- **현황**: 테스트 코드 유형, mocking 전략, E2E 시나리오 미정의
- **영향**: Important
- **Phase**: Phase 5
- **필요 리서치 분량**: 중규모

### GAP 7.3: Claude Code PR 리뷰 프로세스
- **현황**: 기여 가이드라인, CLA, 코드 스타일 미확인
- **영향**: Important
- **Phase**: Phase 5
- **필요 리서치 분량**: 소규모

---

## 8. 기타 영역

### GAP 8.1: macOS 앱 번들 구성
- **현황**: `.app` 번들 디렉토리 구조, Info.plist, 아이콘, 메뉴바 통합 미조사
- **영향**: Critical — `.app` 번들 없이는 macOS에서 정상 앱으로 인식되지 않음
- **Phase**: Phase 5
- **필요 리서치 분량**: 중규모

### GAP 8.2: Synchronized Output (DECRPM 2026)
- **현황**: `CSI ? 2026 h/l` 시퀀스 처리, alacritty_terminal 지원 여부 미확인
- **영향**: Important — TUI 앱 깜빡임 방지에 중요
- **Phase**: Phase 1~2
- **필요 리서치 분량**: 소규모

### ~~GAP 8.3: 키 입력 → 이스케이프 시퀀스 변환 테이블~~ ✅ 해소
> `core/keymapping.md`에서 상세히 다룸. Arrow keys, Function keys, Modifier 조합, Kitty keyboard protocol 포함.

### GAP 8.4: 프로세스 종료 및 정리
- **현황**: SIGHUP/SIGTERM/SIGKILL 시퀀스, 좀비 프로세스 방지 등 미조사
- **영향**: Critical — 프로세스 누수는 심각한 리소스 문제
- **Phase**: Phase 1
- **필요 리서치 분량**: 소규모

### GAP 8.5: 셸 선택 로직
- **현황**: `core/shell-integration.md`에서 셸 통합은 다루지만, 기본 셸 결정 로직(설정 → $SHELL → /etc/passwd → /bin/zsh)의 구현 상세는 미정의
- **영향**: Important
- **Phase**: Phase 1
- **필요 리서치 분량**: 소규모

---

## 9. 성능 최적화 (신규 — 프로젝트 구조 조사 결과)

### GAP 9.1: 이벤트 배칭 (Event Batching)
- **현황**: 현재 60fps 타이머 폴링으로 PTY 출력 감지. Zed는 4ms 타임아웃 / 100개 이벤트 배칭 사용.
- **영향**: Critical — CPU 사용량 감소, 반응성 향상
- **Phase**: Phase 1
- **필요 리서치 분량**: 소규모 (Zed 패턴 적용)
- **참고**: `competitive/terminal-structures.md` §2.5

### ~~GAP 9.2: 셀 배칭 (Cell Batching — BatchedTextRun)~~ ✅ 해소
- **현황**: `element.rs` lines 159-178에 이미 구현됨. `can_extend` 로직이 동일 스타일(color, weight, style, underline, strikethrough)의 인접 셀을 하나의 TextRun으로 병합.
- **검증 완료**:
  - Wide character 처리 정상 (WIDE_CHAR_SPACER 스킵 at line 113)
  - Font 속성 정확히 비교 (weight, style)
  - Underline/strikethrough 비교 정상
  - `cell_width` 파라미터는 character width hint로 올바르게 전달됨 (lines 194, 202)
- **최적화 여지**: Font 생성이 여전히 셀마다 발생하나(lines 130-134), 실제 TextRun 생성은 스타일 변경 시에만 발생하므로 충분히 효율적. 추가 최적화는 premature optimization.
- **Phase**: Phase 1 완료
- **참고**: `crates/crux-terminal-view/src/element.rs:159-178`

### GAP 9.3: Damage Tracking (3단계 시스템)
- **현황**: 현재 매 프레임 전체 리렌더. Ghostty는 3단계(false/partial/full), alacritty_terminal은 TermDamage 제공.
- **영향**: Important — GPU 부하 최소화, 전력 소비 감소
- **Phase**: Phase 1-2
- **필요 리서치 분량**: 중규모
- **참고**: `competitive/terminal-structures.md` §2.4

### GAP 9.4: 텍스트 런 캐싱 (Text Run Caching)
- **현황**: 현재 매번 텍스트 셰이핑 수행. Rio는 256-버킷 해시 + LRU 이빅션으로 96% 셰이핑 오버헤드 감소 달성.
- **영향**: Nice-to-have — 반복 콘텐츠가 많은 경우 성능 향상
- **Phase**: Phase 2+
- **필요 리서치 분량**: 중규모
- **참고**: `competitive/terminal-structures.md` §2.3

### GAP 9.5: 배경 영역 병합 (Background Region Merging)
- **현황**: 현재 셀별 독립 bg_quad 생성. Zed는 같은 색상의 인접 배경 사각형을 수평/수직으로 병합.
- **영향**: Nice-to-have — 대형 단색 영역(예: 빈 화면, 상태바)의 draw call 감소
- **Phase**: Phase 1-2
- **필요 리서치 분량**: 소규모
- **참고**: `competitive/terminal-structures.md` §2.5

---

## 9. 진행 중인 리서치 (2026-02-12 스프린트)

현재 리서치 스프린트에서 다음 주제들이 병렬로 작성되고 있다. 완료 시 추가 갭이 해소될 예정:

| 주제 | 예상 문서 | 해소 예상 갭 |
|------|-----------|-------------|
| Graphics protocols (Kitty/iTerm2/Sixel) | `core/graphics-protocols.md` ✅ | 그래픽스 프로토콜 구현 상세 |
| GPUI widget integration | `gpui/widgets-integration.md` ✅ | DockArea/Tabs/ResizablePanel 통합 |
| Vim IME 자동 전환 | `platform/vim-ime-switching.md` ✅ | Phase 3 IME 전환 킬러 피처 |
| tmux 호환성 매트릭스 | `core/tmux-compatibility.md` ✅ | tmux 제어 모드, passthrough |
| Ghostty 아키텍처 패턴 | `competitive/ghostty-warp-analysis.md` ✅ | 최신 터미널 설계 참조 |
| Warp 대체 분석 | `competitive/ghostty-warp-analysis.md` ✅ | AI 터미널 경쟁 분석 |
| Kitty keyboard protocol 상세 | `core/keymapping.md` ✅ | Progressive enhancement 구현 |
| 5대 터미널 프로젝트 구조 비교 | `competitive/terminal-structures.md` ✅ | 아키텍처 패턴, 최적화 기법, 크레이트 전략 |

---

## 우선순위 요약

### Critical (구현 착수 전 반드시 해결)

| # | 갭 | Phase | 상태 | 리서치 분량 |
|---|-----|-------|------|-----------|
| ~~1.1~~ | ~~Cargo 워크스페이스~~ | ~~1~~ | ✅ 해소 | — |
| ~~1.2~~ | ~~GPUI 의존성 핀닝~~ | ~~1~~ | ✅ 해소 | — |
| ~~2.1~~ | ~~terminfo 엔트리~~ | ~~1~5~~ | ✅ 해소 | — |
| ~~2.3~~ | ~~TERM/TERMINFO 전략~~ | ~~1~~ | ✅ 해소 | — |
| 2.4 | Alternate Screen Buffer | 1 | 미해소 | 소규모 |
| ~~2.6~~ | ~~Selection/Copy~~ | ~~1~~ | ✅ 부분 해소 | — |
| ~~6.1~~ | ~~코드 서명/공증~~ | ~~5~~ | ✅ 해소 | — |
| ~~6.2~~ | ~~Universal Binary~~ | ~~5~~ | ✅ 해소 | — |
| ~~6.3~~ | ~~Homebrew 포뮬러~~ | ~~5~~ | ✅ 해소 | — |
| ~~6.6~~ | ~~라이선스 파일~~ | ~~1/5~~ | ✅ 해소 | — |
| ~~7.1~~ | ~~claude-code 코드 변경~~ | ~~5~~ | ✅ 해소 | — |
| ~~8.3~~ | ~~키입력→이스케이프~~ | ~~1~~ | ✅ 해소 | — |
| 8.1 | macOS 앱 번들 | 5 | 미해소 | 중규모 |
| ~~8.4~~ | ~~프로세스 종료/정리~~ | ~~1~~ | ✅ 부분 해소 | — |
| 9.1 | 이벤트 배칭 | 1 | 미해소 | 소규모 |

**Critical 미해소: 3개** (2.4 Alternate Screen, 8.1 앱 번들, 9.1 이벤트 배칭)

### Important (구현 품질에 영향)

| # | 갭 | Phase | 상태 | 리서치 분량 |
|---|-----|-------|------|-----------|
| 1.4 | 최소 macOS SDK | 1 | 미해소 | 소규모 |
| ~~2.2~~ | ~~셸 통합~~ | ~~5~~ | ✅ 해소 | — |
| 2.8 | 커서 스타일/블링킹 | 1 | 미해소 | 소규모 |
| ~~3.1~~ | ~~TOML 설정 설계~~ | ~~1~5~~ | ✅ 해소 | — |
| 3.3 | 기본 키바인딩 | 1 | 미해소 | 중규모 |
| ~~3.4~~ | ~~테마/색상 스킴~~ | ~~1~5~~ | ✅ 부분 해소 | — |
| 4.2 | IME 자동화 테스트 | 3 | 미해소 | 대규모 |
| ~~4.3~~ | ~~통합 테스트~~ | ~~1~~ | ✅ 해소 | — |
| ~~5.1~~ | ~~VoiceOver 지원~~ | ~~5~~ | ✅ 해소 | — |
| 6.4 | CI/CD 파이프라인 | 1~5 | 미해소 | 중규모 |
| 6.5 | 릴리스 엔지니어링 | 5 | 미해소 | 중규모 |
| 7.2 | Claude Code 테스트 | 5 | 미해소 | 중규모 |
| 7.3 | Claude Code PR 프로세스 | 5 | 미해소 | 소규모 |
| 8.2 | Synchronized Output | 1~2 | 미해소 | 소규모 |
| 8.5 | 셸 선택 로직 | 1 | 미해소 | 소규모 |
| 9.2 | 셀 배칭 (BatchedTextRun) | 1 | 미해소 | 소규모 |
| 9.3 | Damage Tracking | 1-2 | 미해소 | 중규모 |

**Important 미해소: 12개**

### Nice-to-have

| # | 갭 | Phase | 상태 | 리서치 분량 |
|---|-----|-------|------|-----------|
| 2.5 | Bell/알림 처리 | 1 | 미해소 | 소규모 |
| ~~2.7~~ | ~~URL 감지 정규식~~ | ~~4~~ | ✅ 해소 | — |
| ~~3.2~~ | ~~설정 Live Reload~~ | ~~5~~ | ✅ 해소 | — |
| ~~4.4~~ | ~~성능 벤치마킹~~ | ~~1+~~ | ✅ 해소 | — |
| 9.4 | 텍스트 런 캐싱 | 2+ | 미해소 | 중규모 |
| 9.5 | 배경 영역 병합 | 1-2 | 미해소 | 소규모 |

**Nice-to-have 미해소: 3개**

---

## 전체 진행률

| 카테고리 | 전체 | 해소 | 미해소 | 해소율 |
|----------|------|------|--------|--------|
| Critical | 15 | 11 | 4 | **73%** |
| Important | 17 | 5 | 12 | **29%** |
| Nice-to-have | 6 | 3 | 3 | **50%** |
| **합계** | **38** | **19** | **19** | **50%** |

Critical 갭의 73%가 해소되어 **Phase 1 착수에 필요한 핵심 리서치는 거의 완료**되었다. 미해소 Critical 갭 4개(Alternate Screen, 앱 번들, 이벤트 배칭)는 모두 소~중규모로 빠르게 해소 가능하다.

---

## 권장 액션 플랜

### 즉시 (Phase 1 착수 전)

1. **GAP 2.4 해결**: `alacritty_terminal` API에서 alternate screen buffer 처리 방식 확인 — 소규모, 코드 확인으로 해소 가능
2. **GAP 8.4 해결**: 프로세스 종료/정리 패턴 조사 — 소규모, Zed/Alacritty 참고
3. **GAP 1.4 해결**: 최소 macOS SDK 버전 검증 — 소규모, 실제 빌드 테스트로 확인
4. **GAP 9.1 해결**: 60fps 폴링을 이벤트 배칭으로 전환 — Zed 패턴 적용

### Phase 1 진행 중

5. **GAP 3.3 해결**: 기본 키바인딩 전체 목록 정의 — iTerm2/Ghostty 참고
6. **GAP 2.8 해결**: GPUI 커서 블링킹 구현 — Zed BlinkManager 참조
7. **GAP 8.2 해결**: Synchronized Output 구현 — alacritty_terminal 지원 확인
8. **GAP 8.5 해결**: 셸 선택 로직 구현 — Zed terminal 참고
9. **GAP 9.2 해결**: BatchedTextRun 셀 배칭 구현 — element.rs TextRun 최적화
10. **GAP 9.3 해결**: Damage Tracking 도입 — alacritty_terminal TermDamage 활용
11. **GAP 9.5 해결**: 배경 영역 병합 — bg_quads 인접 병합 로직

### Phase 3 (IME)

12. **GAP 4.2 해결**: IME 자동화 테스트 방법론 수립 — 대규모, 별도 리서치 필요
13. **Vim IME 자동 전환 구현**: `platform/vim-ime-switching.md` 기반

### Phase 5 (배포)

14. **GAP 8.1 해결**: macOS 앱 번들 구성 — `cargo bundle` 또는 수동 패키징
15. **GAP 6.4/6.5 해결**: CI/CD + 릴리스 엔지니어링 — GitHub Actions 워크플로우
16. **GAP 7.2/7.3 해결**: Claude Code PR 준비 — `integration/claude-code-strategy.md` 기반

### 진행 중인 리서치 스프린트 완료 시

- Graphics protocols 문서 → 그래픽스 구현 착수 가능
- GPUI widget 문서 → DockArea/Tabs 구현 착수 가능
- tmux 호환성 문서 → Phase 5 tmux 통합 준비 완료
- Ghostty/Warp 분석 → 아키텍처 결정 보강
