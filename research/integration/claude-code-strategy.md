---
title: "Claude Code 통합 전략"
description: "Claude Code repo structure, PaneBackend interface reverse-engineering, Feature Request strategy, WezTerm issue #23574 analysis"
date: 2026-02-12
phase: [5]
topics: [claude-code, feature-request, pane-backend, strategy, community]
status: final
related:
  - ipc-protocol-design.md
  - ipc-external-patterns.md
---

# Claude Code PR 제출 요건 및 프로세스 조사 보고서

## 1. Claude Code 저장소 구조

### 기본 정보
- **저장소**: https://github.com/anthropics/claude-code
- **포크 수**: 5,084
- **오픈 이슈**: 6,626
- **라이선스**: 독점 소프트웨어 (© Anthropic PBC. All rights reserved.)
- **소스 코드**: `cli.js` (11.4MB) 단일 번들 파일로 배포, 소스 코드 비공개

### 저장소 디렉토리 구조
```
anthropics/claude-code/
├── .claude-plugin
├── .claude/
├── .github/
│   ├── ISSUE_TEMPLATE/
│   │   ├── bug_report.yml
│   │   ├── feature_request.yml
│   │   ├── documentation.yml
│   │   └── model_behavior.yml
│   └── workflows/
├── CHANGELOG.md
├── LICENSE.md          # "All rights reserved" - 오픈소스 아님
├── README.md
├── examples/
│   ├── hooks/
│   └── settings/
├── plugins/            # 외부 기여 주요 경로
│   ├── agent-sdk-dev/
│   ├── code-review/
│   ├── hookify/
│   ├── ralph-wiggum/
│   └── ... (14개 플러그인)
└── scripts/
```

### 핵심 발견: CONTRIBUTING.md 없음
- 공식 기여 가이드라인이 존재하지 않음
- PR 템플릿도 없음
- 기여 방법에 대한 공식 문서 부재

---

## 2. WezTerm 통합 이슈 #23574 분석

### 이슈 상세
- **제목**: `[FEATURE] Add WezTerm as a split-pane backend for agent teams (teammateMode)`
- **작성자**: mertkaradayi
- **상태**: OPEN
- **생성일**: 2026-02-06
- **라벨**: `enhancement`, `area:tui`, `area:core`

### 제안된 인터페이스 매핑

| 작업 | tmux 명령 | WezTerm CLI |
|------|-----------|-------------|
| 수평 분할 | `tmux split-window -h -- cmd` | `wezterm cli split-pane --right -- cmd` |
| 수직 분할 | `tmux split-window -v -- cmd` | `wezterm cli split-pane --bottom -- cmd` |
| 패인 목록 | `tmux list-panes` | `wezterm cli list` |
| 패인 포커스 | `tmux select-pane -t N` | `wezterm cli activate-pane --pane-id N` |
| 패인 정보 | `tmux display -p '#{pane_id}'` | `WEZTERM_PANE` 환경변수 (자동 설정) |

### 핵심 구현 세부사항
- `TERM_PROGRAM=WezTerm` 환경변수로 감지
- `wezterm cli split-pane`이 새 패인 ID를 반환
- 각 생성된 패인에 `WEZTERM_PANE` 환경변수 자동 설정
- **현재 커뮤니티 반응**: 댓글 2개, 좋아요 11+ (활발한 관심)

### 제안된 동작
- `teammateMode`가 `"auto"`일 때 `TERM_PROGRAM=WezTerm` 체크
- 또는 `"wezterm"`을 명시적 `teammateMode` 값으로 추가

---

## 3. Claude Code 터미널 백엔드 아키텍처 (소스 코드 분석)

### 핵심 아키텍처 개요

Claude Code의 터미널 백엔드는 다음 3개 계층으로 구성됨:

```
┌─────────────────────────────────────────┐
│         teammateMode 설정               │
│     ["auto", "tmux", "in-process"]      │
├─────────────────────────────────────────┤
│       BackendRegistry (Jt 함수)          │
│  환경 감지 → 백엔드 선택 → 캐싱          │
├────────────┬────────────┬───────────────┤
│ TmuxBackend│ITermBackend│InProcessBackend│
│  (UEA)     │  (iEA)     │   (ju4)       │
└────────────┴────────────┴───────────────┘
```

### 현재 teammateMode 유효값
```javascript
wl4 = ["auto", "tmux", "in-process"]
```
**주의**: `"iterm2"`는 별도의 teammateMode 값이 아님. `"auto"` 또는 `"tmux"` 내에서 자동 감지됨.

### BackendRegistry 감지 우선순위 (detectAndGetBackend / Jt 함수)

```
1. insideTmux ($TMUX 환경변수 존재)?
   → YES: TmuxBackend 선택 (네이티브 tmux 세션)

2. inITerm2 (TERM_PROGRAM=iTerm.app || ITERM_SESSION_ID 존재)?
   → YES:
     a. 사용자가 preferTmuxOverIterm2 설정?
        → YES: iTerm2 감지 스킵
     b. it2 CLI 사용 가능?
        → YES: ITermBackend 선택 (네이티브 iTerm2)
     c. tmux 사용 가능?
        → YES: TmuxBackend 선택 (iTerm2 내 tmux 폴백)
     d. → ERROR: "it2 CLI 미설치"

3. tmux 사용 가능?
   → YES: TmuxBackend 선택 (외부 세션 모드)

4. → ERROR: "패인 백엔드 없음" (tmux 설치 안내)
```

### 백엔드 인터페이스 (필수 구현 메서드)

소스 코드 분석으로 확인된 **PaneBackend 인터페이스**:

```typescript
interface PaneBackend {
  // 식별
  type: string;                    // "tmux" | "iterm2"
  displayName: string;             // 사용자에게 표시되는 이름
  supportsHideShow: boolean;       // 패인 숨기기/보이기 지원 여부

  // 감지
  isAvailable(): Promise<boolean>;
  isRunningInside(): Promise<boolean>;

  // 패인 생성 (핵심)
  createTeammatePaneInSwarmView(name: string, color: string): Promise<{
    paneId: string;
    isFirstTeammate: boolean;
  }>;

  // 패인 조작
  sendCommandToPane(paneId: string, command: string, external?: boolean): Promise<void>;
  setPaneBorderColor(paneId: string, color: string, external?: boolean): Promise<void>;
  setPaneTitle(paneId: string, title: string, color: string, external?: boolean): Promise<void>;
  enablePaneBorderStatus(windowTarget?: string, external?: boolean): Promise<void>;
  rebalancePanes(target: string, withLeader: boolean): Promise<void>;

  // 패인 생명주기
  killPane(paneId: string, external?: boolean): Promise<boolean>;
  hidePane(paneId: string, external?: boolean): Promise<boolean>;
  showPane(paneId: string, target: string, external?: boolean): Promise<boolean>;

  // 유틸리티
  getCurrentPaneId(): Promise<string | null>;
  getCurrentWindowTarget(): Promise<string | null>;
  getCurrentWindowPaneCount(target?: string, external?: boolean): Promise<number | null>;
}
```

### PaneBackendExecutor (Tu4) - 통합 래퍼

모든 패인 백엔드는 `PaneBackendExecutor`로 래핑되어 사용됨:

```typescript
class PaneBackendExecutor {
  // 팀원 생성
  spawn(config: {
    name: string;
    teamName: string;
    prompt: string;
    color: string;
    planModeRequired?: boolean;
    model?: string;
    cwd: string;
    parentSessionId?: string;
    // ...
  }): Promise<SpawnResult>;

  // 메시지 전송
  sendMessage(agentId: string, message: { text: string; from: string; color: string; timestamp?: string }): Promise<void>;

  // 종료
  terminate(agentId: string, reason: string): Promise<boolean>;
  kill(agentId: string): Promise<boolean>;
  isActive(agentId: string): Promise<boolean>;
}
```

### 팀원 스폰 명령 형식

`PaneBackendExecutor.spawn()`이 실행하는 명령:
```bash
cd <cwd> && CLAUDECODE=1 [CLAUDE_CONFIG_DIR=...] \
  claude \
  --agent-id <agentName@teamName> \
  --agent-name <name> \
  --team-name <teamName> \
  --agent-color <color> \
  --parent-session-id <sessionId> \
  [--plan-mode-required] \
  [--dangerously-skip-permissions | --permission-mode acceptEdits] \
  [--model <model>] \
  --teammate-mode <mode>
```

### 환경 감지 메커니즘

```javascript
// tmux 감지
insideTmux = !!process.env.TMUX
tmuxPaneId = process.env.TMUX_PANE

// iTerm2 감지
isInITerm2 = process.env.TERM_PROGRAM === "iTerm.app"
           || !!process.env.ITERM_SESSION_ID
           || terminal === "iTerm.app"

// tmux 사용 가능 여부
isTmuxAvailable = (await exec("tmux", ["-V"])).code === 0

// it2 CLI 사용 가능 여부
isIt2Available = (await exec("it2", ["--version"])).code === 0
```

### 색상 매핑 시스템

```javascript
const colorMap = {
  red: "red", blue: "blue", green: "green",
  yellow: "yellow", purple: "magenta",
  orange: "colour208", pink: "colour205", cyan: "cyan"
};
```

---

## 4. 기타 터미널 백엔드 요청 분석

### Ghostty 지원 - 이슈 #24189
- **상태**: OPEN
- **핵심 블로커**: 프로그래밍 가능한 API/IPC 메커니즘 미존재
  - macOS: AppleScript/App Intents 개발 중
  - Linux: D-Bus 통합 계획됨
- **TERM_PROGRAM**: `ghostty` (이미 감지됨, 하지만 트루컬러 지원 확인용)
- **결론**: CLI가 없어 현재 통합 불가

### Zellij 지원 - 이슈 #24122
- **상태**: OPEN
- **감지**: `$ZELLIJ` 환경변수
- **CLI**: `zellij action new-pane --direction right --name "agent" -- cmd`
- **고유 기능**: 플로팅 패인 (`--floating`)
- **커뮤니티 반응**: 활발 (좋아요 5+, 댓글 3개)

### Windows Terminal 지원 - 이슈 #24384
- **상태**: OPEN
- **플랫폼**: Windows 전용

### 패턴 분석

| 터미널 | CLI 존재 | 감지 환경변수 | 통합 가능성 |
|--------|----------|--------------|-------------|
| WezTerm | `wezterm cli` (완전) | `TERM_PROGRAM=WezTerm` | **높음** |
| Zellij | `zellij action` (완전) | `$ZELLIJ` | **높음** |
| Ghostty | 없음 (개발 중) | `TERM_PROGRAM=ghostty` | **낮음** (API 대기) |
| Windows Terminal | `wt` (제한적) | `$WT_SESSION` | 중간 |
| **Crux** | `crux` (설계 중) | `TERM_PROGRAM=crux` | **높음** (우리가 만듦) |

**핵심 인사이트**: CLI가 완전한 터미널만 통합 가능. Crux는 CLI를 우리가 직접 설계하므로 **Claude Code의 정확한 요구사항에 맞출 수 있는 유일한 터미널**.

---

## 5. PR 제출 전략

### 치명적 발견: 코드 기여 불가

```
LICENSE.md: "© Anthropic PBC. All rights reserved."
```

**Claude Code는 오픈소스가 아닙니다.**

- 소스 코드가 번들된 `cli.js`로만 배포됨
- 모든 머지된 PR은 Anthropic 직원 또는 승인된 외부 기여자
- 외부 기여는 **plugins/**, **docs/**, **examples/** 디렉토리에 한정
- **core 터미널 백엔드 코드 수정 PR은 외부에서 제출 불가**

### 현실적 전략: Feature Request → Anthropic 내부 구현

#### 1단계: GitHub Issue 제출 (Feature Request)

```markdown
제목: [FEATURE] Add Crux as a split-pane backend for agent teams (teammateMode)

라벨: enhancement, area:tui, area:core
```

**이슈 내용 구성**:
- Problem Statement: Crux 사용자가 split-pane 모드 사용 불가
- Proposed Solution: Crux CLI를 사용한 패인 관리
- CLI 명령 매핑 테이블 (tmux ↔ crux 매핑)
- 감지 방법: `TERM_PROGRAM=crux`
- 구현 가이드 (Anthropic 개발자가 참고할 수 있도록)

#### 2단계: Plugin으로 프로토타입 제공

plugins/ 디렉토리에 PR 제출 가능:
```
plugins/crux-terminal-backend/
├── README.md           # 사용법, 설치 안내
├── CLAUDE.md           # 플러그인 설명
├── hooks/
│   ├── setup.sh        # Crux 감지 및 환경 설정
│   └── teammate-spawn.sh  # 팀원 스폰 훅
└── marketplace.json    # 플러그인 메타데이터
```

#### 3단계: Anthropic 관계 구축

- Discord (Claude Developers) 참여
- GitHub 이슈에서 WezTerm/Zellij 이슈 참조하며 범용 백엔드 인터페이스 제안
- 가능하다면 Anthropic DevRel 팀과 직접 소통

#### 4단계: 범용 백엔드 인터페이스 제안

현재 백엔드가 하드코딩되어 있으므로, 플러그인 기반 백엔드 시스템 제안:
```
"범용 터미널 백엔드 인터페이스를 정의하면
WezTerm, Zellij, Ghostty, Crux 등 모든 터미널이
플러그인으로 백엔드를 추가할 수 있습니다"
```

이 제안은 #23574, #24122, #24189, #24384 모든 이슈를 한번에 해결하는 방안.

---

## 6. Crux CLI가 구현해야 할 기술 요구사항

### 환경 감지

```bash
# Crux가 설정해야 하는 환경변수
TERM_PROGRAM=crux              # 필수
TERM_PROGRAM_VERSION=0.1.0     # 권장
CRUX_PANE_ID=<pane-id>         # 각 패인에 자동 설정
CRUX_SESSION=<session-name>    # 선택
```

### CLI 명령 인터페이스 (Claude Code 요구사항에 맞춤)

```bash
# 패인 생성 (split)
crux cli split-pane --direction right -- <command>
crux cli split-pane --direction bottom -- <command>
# 반환: 새 패인 ID (stdout)

# 패인 목록
crux cli list-panes [--format json]
# JSON 출력: [{paneId, title, size, active}]

# 패인 포커스
crux cli focus-pane --pane-id <id>

# 패인 종료
crux cli kill-pane --pane-id <id>
# 반환: exit code 0 (성공) / 비-0 (실패)

# 명령 전송
crux cli send-keys --pane-id <id> <text> Enter

# 패인 외형
crux cli set-pane-title --pane-id <id> <title>
crux cli set-pane-border-color --pane-id <id> <color>

# 레이아웃
crux cli set-layout --target <window> tiled|main-vertical

# 패인 숨기기/보이기 (선택, TmuxBackend만 지원)
crux cli hide-pane --pane-id <id>
crux cli show-pane --pane-id <id> --target <window>
```

### JSON 출력 형식 요구사항

```json
// crux cli list-panes --format json
[
  {
    "paneId": "0",
    "windowId": "0",
    "tabId": "0",
    "title": "team-lead",
    "size": "120x40",
    "cwd": "/Users/jjh/project",
    "active": true
  }
]

// crux cli split-pane 출력
// stdout: "3" (새 패인 ID만)
// exit code: 0
```

### 오류 코드 및 처리

```
Exit Code 0: 성공
Exit Code 1: 일반 오류
Exit Code 2: 패인 없음 (kill, focus 시)
Exit Code 3: 창/탭 없음

stderr: 에러 메시지 (사람이 읽을 수 있는)
stdout: 성공 시 결과 데이터
```

### 패인 생명주기 관리

Claude Code가 기대하는 패인 생명주기:

```
1. spawn() 호출
   → createTeammatePaneInSwarmView(name, color)
   → paneId 반환

2. 명령 전송
   → sendCommandToPane(paneId, "claude --agent-id ... --teammate-mode ...", external)

3. 패인 설정
   → setPaneBorderColor(paneId, color)
   → setPaneTitle(paneId, name, color)
   → enablePaneBorderStatus()

4. 리밸런싱
   → rebalancePanes() (tiled 또는 main-vertical)

5. 정리
   → killPane(paneId) 또는 hidePane(paneId)
```

---

## 7. 리스크 및 블로커

### 치명적 리스크

#### R1: 코드 기여 불가 (Critical)
- **원인**: Claude Code는 독점 소프트웨어
- **영향**: 직접 터미널 백엔드 PR 제출 불가
- **완화**: Feature Request 이슈 제출 + Anthropic 관계 구축
- **대안**: 플러그인/훅 기반 우회 통합

#### R2: API 안정성 (High)
- **원인**: 내부 백엔드 인터페이스는 공개 API가 아님
- **영향**: 버전 업데이트 시 호환성 깨질 수 있음
- **완화**: 공식 백엔드 인터페이스 표준화 제안

#### R3: Anthropic의 우선순위 (Medium)
- **원인**: WezTerm 이슈가 먼저 제출됨 (2026-02-06)
- **영향**: WezTerm이 먼저 구현될 수 있음
- **완화**: 범용 인터페이스 제안으로 동시 해결 유도

### 기회

#### O1: 최초의 "Claude Code-aware" 터미널
- WezTerm, Ghostty, Zellij은 모두 범용 터미널
- Crux는 **Claude Code Agent Teams를 위해 설계**됨
- CLI를 Claude Code의 정확한 인터페이스에 맞출 수 있음
- 이것은 다른 터미널이 따라할 수 없는 차별점

#### O2: 범용 백엔드 인터페이스 표준 주도
- #23574, #24122, #24189, #24384 모든 이슈의 공통 해결책
- Crux가 이 표준을 제안하고 참조 구현을 제공하면 생태계 리더십 확보

#### O3: 플러그인 시스템 활용
- 코어 코드 변경 없이 hooks/plugins으로 프로토타입 가능
- 검증된 프로토타입으로 코어 통합 설득력 강화

---

## 8. 구체적 액션 플랜

### Phase 1: 기반 구축 (즉시)

1. **Crux CLI에 필수 명령 구현**
   - `crux cli split-pane --direction right|bottom -- <cmd>`
   - `crux cli list-panes [--format json]`
   - `crux cli kill-pane --pane-id <id>`
   - `crux cli send-keys --pane-id <id> <text> Enter`
   - `crux cli focus-pane --pane-id <id>`

2. **환경변수 설정**
   - 모든 Crux 창/패인에 `TERM_PROGRAM=crux` 설정
   - 각 패인에 `CRUX_PANE_ID` 자동 설정

### Phase 2: GitHub 이슈 제출

3. **Feature Request 이슈 작성**
   - WezTerm #23574와 동일한 구조로 작성
   - tmux ↔ Crux CLI 매핑 테이블 포함
   - 작동하는 CLI 데모 포함
   - #23574, #24122 참조하며 "범용 백엔드 인터페이스" 필요성 언급

### Phase 3: 플러그인 PR 제출

4. **plugins/crux-terminal-backend 플러그인 PR**
   - hooks를 사용한 프로토타입 구현
   - 설치/사용 가이드 포함
   - `marketplace.json` 메타데이터

### Phase 4: 커뮤니티 참여

5. **Claude Developers Discord 참여**
6. **WezTerm/Zellij 이슈에 코멘트** - 범용 인터페이스 제안
7. **Anthropic DevRel 접촉**

### Phase 5: 코어 통합 요청

8. **범용 Terminal Backend Interface RFC 제출**
   - 모든 터미널이 사용 가능한 표준 인터페이스 정의
   - 플러그인 기반 백엔드 등록 시스템 제안
   - Crux를 참조 구현으로 제공

---

## 부록: 참조 이슈 및 링크

| 이슈 번호 | 제목 | 상태 | 관련성 |
|-----------|------|------|--------|
| #23574 | WezTerm split-pane backend | OPEN | **직접 경쟁/참조** |
| #24122 | Zellij terminal multiplexer support | OPEN | **참조** |
| #24189 | Ghostty split-pane backend | OPEN | 참조 (CLI 없어 블로킹) |
| #24384 | Windows Terminal split-pane backend | OPEN | 참조 |
| #23572 | tmux/iTerm2 silent fallback bug | OPEN | 아키텍처 이해 |
| #23815 | iTerm2 split-pane mode fallback bug | OPEN | 아키텍처 이해 |
| #24771 | tmux split panes disconnected | OPEN | 메시징 시스템 이해 |
| #23950 | Configurable tmux split direction | OPEN | 확장성 참조 |
| #19555 | Built-in multiplexer vision | 참조됨 | 장기 비전 참조 |

### 소스 코드 분석 참조

분석된 클래스/함수:
- `TmuxBackend` (UEA): `/tmp/claude-code-inspect/package/cli.js` line ~2108-2116
- `ITermBackend` (iEA): 같은 파일
- `InProcessBackend` (ju4): 같은 파일
- `PaneBackendExecutor` (Tu4): 같은 파일
- `BackendRegistry` (Jt): 감지 및 선택 로직
- `detectAndGetBackend`: 환경 감지 우선순위

### 현재 백엔드 등록 방식

```javascript
// 하드코딩된 등록
registerTmuxBackend(TmuxBackend);      // pEA(UEA)
registerITermBackend(ITermBackend);     // nEA(iEA)

// 감지 우선순위
// 1. tmux (inside) → TmuxBackend
// 2. iTerm2 + it2 → ITermBackend
// 3. iTerm2 + tmux → TmuxBackend (fallback)
// 4. tmux (external) → TmuxBackend
// 5. 없음 → Error
```

새로운 백엔드를 추가하려면 이 등록 시스템에 `registerCruxBackend` 같은 함수를 추가해야 하지만, **이는 코어 코드 변경이므로 Anthropic만 할 수 있음**.
