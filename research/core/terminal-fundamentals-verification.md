---
title: "Terminal Fundamentals Verification Report"
description: "External-source verification of terminal emulator fundamentals. Gap analysis against existing Crux research docs, new findings, and practical implementation checklist."
date: 2026-02-12
phase: [1, 2, 3]
topics: [verification, fundamentals, compatibility, testing, security]
status: final
related:
  - ./terminal-emulation.md
  - ./terminal-architecture.md
  - ./keymapping.md
  - ./terminfo.md
  - ./performance.md
  - ./mouse-reporting.md
  - ./shell-integration.md
  - ../platform/ime-clipboard.md
---

# Terminal Fundamentals Verification Report

> **Purpose**: 외부 소스 기반으로 기존 Crux 리서치 문서의 정확성을 검증하고, 누락된 기본기를 식별한다.
> **Date**: 2026-02-12
> **Sources**: vttest, Thomas Dickey's xterm FAQ, Dan Luu's latency research, Ghostty/Alacritty/WezTerm/Kitty/Rio 이슈 트래커, Mitchell Hashimoto's grapheme research

---

## 1. 기존 문서 대비 갭 분석

### 1.1 문서별 커버리지 평가

| 기존 문서 | 상태 | 주요 갭 |
|-----------|------|---------|
| `terminal-emulation.md` | **양호** | vttest 카테고리별 커버리지 미언급, 리플로우 상세 없음 |
| `terminal-architecture.md` | **양호** | 좀비 프로세스 처리, SIGCHLD 핸들링 미언급 |
| `keymapping.md` | **양호** | macOS Option key left/right variant 분리 미상세 |
| `terminfo.md` | **양호** | SSH 환경 fallback 전략 추가 필요 |
| `performance.md` | **양호** | Dan Luu 레이턴시 기준 (< 20ms) 미언급 |
| `mouse-reporting.md` | **양호** | 선택(selection) 엣지 케이스 미포함 |
| `shell-integration.md` | **양호** | tmux 내부에서의 shell integration 실패 패턴 미언급 |
| `config-system.md` | **양호** | 파싱 에러 UX (사용자 혼란 패턴) 미언급 |
| `font-system.md` | **부분적** | Nerd Fonts 너비 문제, box-drawing 내장 렌더링 미상세 |
| `ime-clipboard.md` | **양호** | 2026-02-12 최신화 완료 |
| (없음) | **누락** | **보안**: 이스케이프 시퀀스 인젝션, OSC 52 남용 |
| (없음) | **누락** | **윈도우 리사이즈 리플로우**: 저장된 커서 + 리플로우 상호작용 |
| (없음) | **누락** | **macOS 특화 이슈**: Secure Keyboard Entry, Full Disk Access, 공증 |

### 1.2 심각도별 갭 분류

**Critical (구현 전 반드시 해결):**
1. 윈도우 리사이즈 + 저장된 커서 리플로우 (모든 터미널이 버그 보유)
2. 좀비 프로세스 reaping (SIGCHLD 핸들러)
3. 보안: 이스케이프 시퀀스 인젝션 방어
4. OSC 52 read 차단 (클립보드 읽기는 보안 위험)

**High (Phase 1-2에서 해결):**
5. macOS Option key Left/Right 독립 설정
6. SSH 환경 TERM fallback 전략
7. Nerd Fonts 너비 계산
8. Bracketed paste mode 보안 검증

**Medium (Phase 3+ 해결):**
9. Mode 2027 그래핌 클러스터
10. 리플로우 알고리즘 (soft wrap 추적)
11. 폰트 리가처 (opt-in)

---

## 2. 신규 발견사항

### 2.1 윈도우 리사이즈 + 리플로우: 모든 터미널이 틀림

**문제**: DECSC(ESC 7)로 커서를 저장한 후 윈도우 리사이즈로 텍스트가 리플로우되면, DECRC(ESC 8)로 복원한 커서 위치가 어긋남.

**영향받는 터미널**:
- [Kitty #8325](https://github.com/kovidgoyal/kitty/issues/8325) — 커서 위치 off-by-one
- [Ghostty #5718](https://github.com/ghostty-org/ghostty/issues/5718) — 저장된 커서 리플로우 안 됨
- [Windows Terminal #4200](https://github.com/microsoft/terminal/issues/4200) — 리플로우 시나리오 전체 추적 이슈
- [WezTerm #6669](https://github.com/wezterm/wezterm/issues/6669) — 동일 증상
- [tmux #4366](https://github.com/tmux/tmux/issues/4366) — tmux도 동일

**실사용 시나리오**: vim 진입 → 윈도우 리사이즈 → vim 종료 → 셸 텍스트 소실 (복원된 커서가 잘못된 위치에서 셸 프롬프트를 렌더링)

**Crux 대응**:
- 저장된 커서가 리플로우를 따라가도록 구현해야 함
- 단순 그리드 좌표가 아닌, 콘텐츠 기반 앵커링 필요
- 스펙이 미정의(unspecified)이므로 테스트 기반으로 올바른 동작 정의 필요
- **테스트**: vim 진입 → 리사이즈 → vim 종료 → 셸 텍스트 보존 확인

### 2.2 PTY: 좀비 프로세스와 시그널 처리

**기존 문서에 없는 내용:**

```
좀비 프로세스 방지:
  - SIGCHLD 핸들러에서 waitpid(..., WNOHANG)를 루프로 호출
  - 또는 SIGCHLD를 SIG_IGN으로 설정 (자동 수거)
  - Double-fork 패턴 (init이 상속)

시그널 전달 규칙:
  - SIGWINCH: 윈도우 크기 업데이트 → SIGWINCH 전달 → 자식이 새 크기 읽기 (순서 중요)
  - SIGTERM: 자식에게 전달 (PID 1 문제 — shell form은 SIGTERM 미전달)
  - SIGINT/SIGQUIT: PTY 디바이스가 직접 처리 (터미널이 전달할 필요 없음)

로그인 셸 vs 비로그인 셸:
  - 로그인: /etc/profile → ~/.bash_profile → ~/.bash_login → ~/.profile
  - 비로그인: /etc/bash.bashrc → ~/.bashrc
  - PATH, UID, GID, TERM은 로그인 셸이 설정
```

**참고**: [Zombie Process Prevention](https://www.baeldung.com/cs/process-lifecycle-zombie-state), [SIGWINCH Handling](https://www.rkoucha.fr/tech_corner/sigwinch.html), [Docker Signal Issues](https://petermalmgren.com/signal-handling-docker/)

### 2.3 보안: 이스케이프 시퀀스 인젝션

**기존 문서에 전혀 없는 내용.**

**위협 모델**:
1. **OSC 52 클립보드 인젝션**: 악성 프로그램이 클립보드에 명령 삽입
2. **Bracketed paste 우회**: `\e[200~`로 시작하는 악성 코드가 브래킷 조기 종료 (CVE-2021-31701, CVE-2021-37326)
3. **문자 증폭**: ANSI 코드로 수십억 문자 출력 → DoS
4. **타이틀 바 스푸핑**: 사용자를 오도하는 가짜 타이틀
5. **Ghostty 1.0 CVE**: 윈도우 타이틀 시퀀스 처리 결함 → 임의 코드 실행

**Crux 방어 전략**:
```
1. OSC 52 write-only (read 구현 금지 — 클립보드 도난 위험)
2. OSC 52 write에도 사용자 동의 프롬프트 (프로파일 기반)
3. 시퀀스 길이 제한 (OSC 52: 100,000 바이트)
4. 브래킷 paste 모드에서 내부 \e[200~ / \e[201~ 이스케이프 필터링
5. 외부 입력 출력 시 이스케이프 시퀀스 sanitization
```

**참고**: [CyberArk: ANSI Escape Abuse](https://www.cyberark.com/resources/threat-research-blog/dont-trust-this-title-abusing-terminal-emulators-with-ansi-escape-characters), [Packet Labs: Weaponizing ANSI](https://www.packetlabs.net/posts/weaponizing-ansi-escape-sequences/), [Ghostty CVE](https://dgl.cx/2024/12/ghostty-terminal-title)

### 2.4 macOS Option Key: Left/Right 독립 설정

**기존 `keymapping.md`에서 Option-as-Meta를 다루지만 Left/Right 분리가 미흡.**

**사용자 기대**:
- **Left Option → Meta** (Emacs, 셸 단축키: Alt+B, Alt+F)
- **Right Option → 특수문자** (€, ±, ≈ 등 macOS 기본 동작 유지)

**경쟁 터미널 구현**:
| 터미널 | Left/Right 분리 | 기본값 |
|--------|----------------|--------|
| iTerm2 | ✅ Yes | Normal |
| Ghostty | ✅ Yes (`macos-option-as-alt = left`) | None |
| Alacritty | ❌ No (전체) | None |
| WezTerm | ❌ No (전체) | None |

**Crux 설계**: Ghostty 방식 채택 권장 — `option_as_alt = "left"` / `"right"` / `"both"` / `"none"`

**참고**: [EmacsWiki: Meta Key Problems](https://www.emacswiki.org/emacs/MetaKeyProblems)

### 2.5 성능 기준: Dan Luu 레이턴시 측정

**기존 `performance.md`에 벤치마크가 있지만 구체적 기준치가 부족.**

| 범위 | 평가 | 참고 |
|------|------|------|
| < 1ms | 최상 (펜+종이 수준) | 비현실적 |
| **< 10ms** | **우수** | Terminal.app (~6ms) |
| **10-20ms** | **양호** | 인지 가능하지만 수용 |
| 25-50ms | 보통 | Alacritty, iTerm2 (25-44ms) |
| 50ms+ | 느림 | "래그" 체감 |

**핵심 인사이트** (Dan Luu):
> "터미널은 stdout 처리량을 벤치마크하지만, 처리량과 레이턴시의 관계는 비직관적이다. 사용자는 `cat` 할 때가 아니라 **타이핑할 때** 레이턴시를 체감한다."

**Crux 목표**: 입력 레이턴시 < 20ms (유휴 상태 기준)

**참고**: [Dan Luu: Terminal Latency](https://danluu.com/term-latency/)

### 2.6 신규 터미널의 Top 5 버그 패턴

Ghostty, Alacritty, Rio, Kitty, WezTerm의 첫해 이슈 분석 결과:

| 순위 | 버그 카테고리 | 빈도 | 심각도 | Crux 관련 |
|------|-------------|------|--------|-----------|
| **1** | **윈도우 리사이즈 + 커서 리플로우** | 매우 높음 | Critical | Phase 1 |
| **2** | **폰트 렌더링** (Nerd Fonts, Powerline, box-drawing) | 매우 높음 | Moderate | Phase 1-2 |
| **3** | **SSH + TERM 미인식** | 높음 | Critical | Phase 1 |
| **4** | **macOS Option key** | 높음 | Critical | Phase 1 |
| **5** | **셸 통합 실패** (oh-my-zsh, Powerlevel10k) | 높음 | Moderate | Phase 2 |

**추가 주요 패턴**: 키보드 입력/비US 레이아웃, 마우스 모드/선택, tmux 호환성, 설정 파싱 에러, 시작 크래시, 메모리 누수, 보안 취약점

### 2.7 SSH TERM Fallback 전략

**기존 `terminfo.md`에 SSH 시나리오가 부족.**

```
SSH 접속 시 TERM 해석 순서:
1. 서버에 xterm-crux terminfo가 있으면 → TERM=xterm-crux 그대로 사용
2. 없으면 → 터미널이 TERM=xterm-256color로 폴백해야 함
3. 폴백 전략:
   a. 사용자가 SSH 환경 감지 시 자동 폴백 (권장)
   b. shell alias: ssh() { TERM=xterm-256color command ssh "$@"; }
   c. ~/.ssh/config: SetEnv TERM=xterm-256color (per-host)

서버에 terminfo 설치 방법:
  infocmp -x xterm-crux | ssh remote "mkdir -p ~/.terminfo/x && tic -x -o ~/.terminfo -"
```

**참고**: [SSH Locale Forwarding](https://www.linuxbabe.com/linux-server/fix-ssh-locale-environment-variable-error)

---

## 3. alacritty_terminal이 처리하는 것 vs Crux가 처리해야 하는 것

### 3.1 alacritty_terminal이 처리

| 영역 | 커버리지 |
|------|---------|
| VT 파싱 (ANSI, CSI, OSC, DCS) | ✅ |
| 터미널 상태 머신 (모드, 속성) | ✅ |
| 그리드 저장 + 데미지 추적 | ✅ |
| 대체 화면 전환 (1049) | ✅ |
| SGR 속성 (밑줄 색상 58/59 포함) | ✅ |
| 마우스 모드 (1000, 1002, 1004-1007) | ✅ |
| 브래킷 붙여넣기 (2004) | ✅ |
| 포커스 이벤트 (1004) | ✅ |
| OSC 52 파싱 | ✅ (파싱만) |
| 선택 로직 | ✅ (렌더링 제외) |

### 3.2 Crux가 직접 처리해야 하는 것

| 영역 | 설명 | 우선순위 |
|------|------|---------|
| **PTY 관리** | 할당, SIGCHLD, SIGWINCH, 환경변수, 로그인 셸 | P0 |
| **렌더링** | GPUI Metal 셀 렌더링, 색상, 커서, 선택 하이라이트 | P0 |
| **입력 인코딩** | 키보드 → 이스케이프 시퀀스 변환 | P0 |
| **마우스 인코딩** | 마우스 이벤트 → SGR 1006 인코딩 | P0 |
| **Unicode 너비** | wcwidth 또는 그래핌 기반 너비 계산 | P0 |
| **스크롤백 저장** | 메모리 관리, 제한, 검색 | P0 |
| **클립보드** | NSPasteboard, OSC 52 write-only | P1 |
| **IME** | NSTextInputClient (Phase 3) | P0 (Phase 3) |
| **terminfo** | 설치, TERM 설정, SSH 폴백 | P0 |
| **보안** | 시퀀스 길이 제한, 브래킷 paste 검증 | P1 |
| **Option key** | Left/Right 독립 Meta 설정 | P1 |
| **리플로우** | 리사이즈 시 텍스트 리플로우 + 커서 추적 | P1 |
| **Nerd Fonts** | 더블 너비 글리프, box-drawing 내장 렌더링 | P2 |
| **Mode 2027** | 그래핌 클러스터 (선택적) | P2 |

---

## 4. 구현 전 테스트 체크리스트

### Phase 1 완료 전 필수 테스트

**셸 호환성**:
- [ ] zsh + oh-my-zsh + Powerlevel10k (프롬프트 렌더링, 리사이즈)
- [ ] fish shell (vi 모드, 자동 완성)
- [ ] bash + readline (PS1 이스케이프 시퀀스)

**핵심 앱**:
- [ ] vim/nvim (커서 모양 변경, 포커스 이벤트, 브래킷 붙여넣기)
- [ ] tmux (트루컬러, 마우스 모드, OSC 패스스루)
- [ ] htop / btm (CPU/메모리 사용률 표시)
- [ ] fzf (마우스 모드, 대체 화면)
- [ ] lazygit / lazydocker

**SSH**:
- [ ] terminfo 없는 서버 접속 시 폴백 동작
- [ ] 로캘 포워딩 (LANG, LC_*)
- [ ] SSH 내부에서 트루컬러 동작

**macOS**:
- [ ] Option key Left/Right 독립 동작
- [ ] Command key 단축키 (Cmd+C 복사 vs Ctrl+C 시그널)

**윈도우 리사이즈**:
- [ ] vim 진입 → 리사이즈 → vim 종료 → 셸 텍스트 보존
- [ ] Powerlevel10k 오른쪽 프롬프트 리사이즈 후 정렬
- [ ] 멀티라인 명령어 리사이즈 시 무결성

**유니코드**:
- [ ] CJK 전각 문자 2셀
- [ ] 이모지 (VS-16, ZWJ 시퀀스)
- [ ] 조합 문자

**성능**:
- [ ] 입력 레이턴시 < 20ms (Dan Luu 방법론)
- [ ] 시작 시간 < 500ms
- [ ] 10K 스크롤백 메모리 < 10MB

**보안**:
- [ ] OSC 52 write 동작 확인 (read는 차단)
- [ ] 브래킷 paste 내부의 가짜 브래킷 필터링
- [ ] 초대형 시퀀스 (100KB+) 처리 (크래시 안 됨)

---

## 5. 참고 자료

### 스펙 & 테스트 스위트
- [vttest - VT100/VT220/XTerm test utility](https://invisible-island.net/vttest/vttest.html)
- [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
- [XTerm FAQ - Thomas Dickey](https://invisible-island.net/xterm/xterm.faq.html)
- [Alacritty Escape Sequence Support](https://github.com/alacritty/alacritty/blob/master/docs/escape_support.md)

### 성능
- [Terminal Latency - Dan Luu](https://danluu.com/term-latency/)
- [Kitty Performance Benchmarks](https://sw.kovidgoyal.net/kitty/performance/)

### 유니코드 & 텍스트
- [Grapheme Clusters in Terminals - Mitchell Hashimoto](https://mitchellh.com/writing/grapheme-clusters-in-terminals)
- [True Color Support in Terminals](https://gist.github.com/XVilka/8346728)
- [Unicode East Asian Width (UAX #11)](https://www.unicode.org/reports/tr11/tr11-40.html)
- [FreeDesktop BiDi in Terminal Emulators](https://terminal-wg.pages.freedesktop.org/bidi/)

### 보안
- [CyberArk: ANSI Escape Abuse](https://www.cyberark.com/resources/threat-research-blog/dont-trust-this-title-abusing-terminal-emulators-with-ansi-escape-characters)
- [Packet Labs: Weaponizing ANSI](https://www.packetlabs.net/posts/weaponizing-ansi-escape-sequences/)
- [Ghostty 1.0 CVE - Terminal Title](https://dgl.cx/2024/12/ghostty-terminal-title)

### PTY & 시그널
- [Zombie Process Prevention](https://www.baeldung.com/cs/process-lifecycle-zombie-state)
- [SIGWINCH Handling](https://www.rkoucha.fr/tech_corner/sigwinch.html)
- [Docker Signal Handling](https://petermalmgren.com/signal-handling-docker/)
- [PTY: What powers docker attach](https://iximiuz.com/en/posts/linux-pty-what-powers-docker-attach-functionality/)

### 리사이즈 & 리플로우 버그
- [Kitty #8325: DECSC + reflow cursor](https://github.com/kovidgoyal/kitty/issues/8325)
- [Ghostty #5718: Resize doesn't reflow saved cursor](https://github.com/ghostty-org/ghostty/issues/5718)
- [Windows Terminal #4200: ResizeWithReflow](https://github.com/microsoft/terminal/issues/4200)
- [tmux #4366: Cursor position after resize](https://github.com/tmux/tmux/issues/4366)
- [WezTerm #6669: Same symptom](https://github.com/wezterm/wezterm/issues/6669)

### 셸 & 앱 호환성
- [Ghostty Shell Integration](https://ghostty.org/docs/features/shell-integration)
- [WezTerm Shell Integration](https://wezterm.org/shell-integration.html)
- [tmux FAQ](https://github.com/tmux/tmux/wiki/FAQ)
- [Neovim TUI Documentation](https://neovim.io/doc/user/tui.html)

### macOS
- [EmacsWiki: Meta Key Problems](https://www.emacswiki.org/emacs/MetaKeyProblems)
- [Apple: Safely open apps](https://support.apple.com/en-us/102445)
- [Lap Cat Software: Full Disk Access](https://lapcatsoftware.com/articles/FullDiskAccess.html)

### 폰트 렌더링
- [Nerd Fonts: Character width discussion](https://github.com/ryanoasis/nerd-fonts/discussions/969)
- [Alacritty: Builtin font for box-drawing](https://github.com/alacritty/alacritty/commit/f7177101eda589596ab08866892bd4629bd1ef44)
- [Powerline: Troubleshooting](https://powerline.readthedocs.io/en/master/troubleshooting.html)

### 클립보드 & 보안
- [Sunaku: OSC 52 with tmux and Vim](https://sunaku.github.io/tmux-yank-osc52.html)
- [jdhao: Bracketed Paste Mode](https://jdhao.github.io/2021/02/01/bracketed_paste_mode/)
