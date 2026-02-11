---
title: "Homebrew / macOS 배포 파이프라인"
description: "Homebrew Formula/Cask, code signing, notarization, Universal Binary (x86_64 + aarch64), CI/CD GitHub Actions, Tap strategy"
date: 2026-02-12
phase: [6]
topics: [homebrew, distribution, code-signing, notarization, universal-binary, ci-cd, github-actions]
status: final
related:
  - ../integration/claude-code-strategy.md
---

# Homebrew / macOS 배포 파이프라인 완전 가이드

> Crux: Rust + GPUI (Metal 렌더링) 기반 macOS 터미널 에뮬레이터를 위한 배포 전략

---

## 목차

1. [Homebrew Formula/Cask 구조 분석](#1-homebrew-formulacask-구조-분석)
2. [homebrew-core / homebrew-cask 제출 요건](#2-homebrew-core--homebrew-cask-제출-요건)
3. [macOS 코드 서명 & 공증(Notarization)](#3-macos-코드-서명--공증notarization)
4. [Universal Binary 빌드](#4-universal-binary-빌드)
5. [CI/CD 파이프라인 (GitHub Actions)](#5-cicd-파이프라인-github-actions)
6. [릴리스 엔지니어링](#6-릴리스-엔지니어링)
7. [Homebrew Tap (커스텀 저장소)](#7-homebrew-tap-커스텀-저장소)
8. [Crux를 위한 권장 전략](#8-crux를-위한-권장-전략)

---

## 1. Homebrew Formula/Cask 구조 분석

### 1.1 Formula vs Cask: Crux에 적합한 방식

| 구분 | Formula (homebrew-core) | Cask (homebrew-cask) |
|------|------------------------|---------------------|
| **대상** | CLI 도구, 라이브러리 | GUI 앱 (.app 번들) |
| **설치 방식** | 소스에서 빌드 | 미리 빌드된 바이너리 다운로드 |
| **명령어** | `brew install foo` | `brew install --cask foo` |
| **코드 서명** | 필요 없음 (소스 빌드) | **Gatekeeper 통과 필수** (Apple Silicon) |
| **적합한 경우** | `rio-terminal` (CLI 중심) | `alacritty`, `wezterm` (GUI 앱) |

**Crux의 경우:**
- GPUI 기반 GUI 앱이므로 `.app` 번들로 배포할 경우 → **Cask**
- CLI로도 실행 가능한 바이너리라면 → **Formula** (소스 빌드)
- **권장: 초기에는 Formula(소스 빌드) 방식 + 커스텀 Tap으로 시작**

> **중요 사례:** Alacritty는 Cask로 배포했으나 코드 서명 미비로 2025년 10월 Homebrew에서 deprecated 처리됨 (2026-09-01 비활성화 예정). Rio는 Formula로 배포하여 이 문제를 회피함.

### 1.2 실제 Formula 예시: Rio Terminal (Rust + GPU 터미널)

Rio Terminal은 Crux와 가장 유사한 사례로, Rust 기반 GPU 가속 터미널이 homebrew-core Formula로 등록된 케이스이다.

```ruby
class RioTerminal < Formula
  desc "Hardware-accelerated GPU terminal emulator powered by WebGPU"
  homepage "https://rioterm.com/"
  url "https://github.com/raphamorim/rio/archive/refs/tags/v0.2.37.tar.gz"
  sha256 "f52bcd0fb3c669cae016c614a77a95547c2769e6a98a17d7bbc703b6e72af169"
  license "MIT"
  head "https://github.com/raphamorim/rio.git", branch: "main"

  livecheck do
    url :stable
    regex(/^v?(\d+(?:\.\d+)+)$/i)
  end

  bottle do
    sha256 cellar: :any_skip_relocation, arm64_tahoe:   "4ec1c734..."
    sha256 cellar: :any_skip_relocation, arm64_sequoia: "46d0d3d9..."
    sha256 cellar: :any_skip_relocation, arm64_sonoma:  "6f776eb5..."
    sha256 cellar: :any_skip_relocation, sonoma:        "2c89e840..."
  end

  depends_on "rust" => :build
  depends_on :macos  # macOS 전용

  def install
    system "cargo", "install", *std_cargo_args(path: "frontends/rioterm")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/rio --version")
    system bin/"rio", "--write-config", testpath/"rio.toml"
    assert_match "enable-log-file = false", (testpath/"rio.toml").read
  end
end
```

**핵심 패턴:**
- `depends_on "rust" => :build` — Rust를 빌드 의존성으로 선언
- `depends_on :macos` — macOS 전용 앱임을 명시
- `system "cargo", "install", *std_cargo_args(...)` — Homebrew 표준 cargo 빌드
- `bottle` 블록 — Homebrew 봇이 자동으로 미리 빌드한 바이너리 (bottle)
- `cellar: :any_skip_relocation` — 시스템 라이브러리에만 의존, 재배치 불필요

### 1.3 실제 Cask 예시: Alacritty

```ruby
cask "alacritty" do
  version "0.16.1"
  sha256 "..." # DMG 파일의 SHA256

  url "https://github.com/alacritty/alacritty/releases/download/v#{version}/Alacritty-v#{version}.dmg"
  name "Alacritty"
  desc "GPU-accelerated terminal emulator"
  homepage "https://github.com/alacritty/alacritty/"

  depends_on macos: ">= :big_sur"

  app "Alacritty.app"
  binary "#{appdir}/Alacritty.app/Contents/MacOS/alacritty"

  # 쉘 완성 파일
  zsh_completion "#{appdir}/Alacritty.app/Contents/Resources/completions/_alacritty"
  bash_completion "#{appdir}/Alacritty.app/Contents/Resources/completions/alacritty.bash"
  fish_completion "#{appdir}/Alacritty.app/Contents/Resources/completions/alacritty.fish"

  zap trash: [
    "~/.config/alacritty",
    "~/Library/Preferences/org.alacritty.plist",
    "~/Library/Saved Application State/org.alacritty.savedState",
  ]

  # 2025년 10월 Gatekeeper 실패로 deprecated
  disable! date: "2026-09-01", because: "does not pass the macOS Gatekeeper check"
end
```

### 1.4 실제 Cask 예시: WezTerm

```ruby
cask "wezterm" do
  version "20240203-110809,5046fc22"
  sha256 "..."

  url "https://github.com/wezterm/wezterm/releases/download/#{version.csv.first}/WezTerm-macos-#{version.csv.first}.zip",
      verified: "github.com/wezterm/wezterm/"
  name "WezTerm"
  desc "GPU-accelerated cross-platform terminal emulator and multiplexer"
  homepage "https://wezterm.org/"

  app "WezTerm.app"
  binary "#{appdir}/WezTerm.app/Contents/MacOS/wezterm"
  binary "#{appdir}/WezTerm.app/Contents/MacOS/wezterm-gui"
  binary "#{appdir}/WezTerm.app/Contents/MacOS/wezterm-mux-server"

  zsh_completion "#{appdir}/WezTerm.app/Contents/Resources/shell-completion/zsh/_wezterm"
  bash_completion "#{appdir}/WezTerm.app/Contents/Resources/shell-completion/bash/wezterm.bash"
  fish_completion "#{appdir}/WezTerm.app/Contents/Resources/shell-completion/fish/wezterm.fish"

  conflicts_with cask: "wezterm@nightly"

  zap trash: [
    "~/.config/wezterm",
    "~/Library/Saved Application State/com.github.wez.wezterm.savedState",
  ]
end
```

### 1.5 GPUI/Metal 의존성 처리

Crux가 GPUI(Metal 렌더링)를 사용하는 경우의 Formula 의존성 고려사항:

| 의존성 | 처리 방법 |
|--------|----------|
| Metal Framework | macOS 시스템 프레임워크 → 별도 선언 불필요 |
| Xcode CLT | Homebrew가 자동 설치 요구 |
| Rust | `depends_on "rust" => :build` |
| cmake (GPUI 빌드 시) | `depends_on "cmake" => :build` (필요시) |
| pkg-config | `depends_on "pkg-config" => :build` (필요시) |
| macOS 최소 버전 | `depends_on macos: ">= :ventura"` (macOS 13+) |

**주의:** GPUI는 Metal을 직접 사용하므로 macOS 전용이며, `depends_on :macos` 선언이 필수적이다.

---

## 2. homebrew-core / homebrew-cask 제출 요건

### 2.1 homebrew-core (Formula) 제출 요건

#### 필수 요건

| 요건 | 상세 |
|------|------|
| **라이선스** | DFSG(Debian Free Software Guidelines) 호환 오픈소스 라이선스 |
| **소스 빌드** | 소스코드에서 빌드 가능해야 함 |
| **안정 버전** | 업스트림에서 태깅된 안정 릴리스 필요 (beta/unstable 불가) |
| **타볼 배포** | Git checkout보다 tarball 선호 |
| **플랫폼 지원** | 최근 3개 macOS 버전(Apple Silicon + x86_64) 및 x86_64 Linux에서 빌드/테스트 통과 |
| **자체 업데이트 금지** | 자동 업데이트 기능은 비활성화 필수 |

#### 인지도/인기도 요건 (신규/마이너 프로젝트)

| 기준 | 최소값 |
|------|--------|
| GitHub Stars | **75개 이상** |
| GitHub Forks | **30개 이상** |
| GitHub Watchers | **30개 이상** |
| 외부 사용 증거 | 저자 외 사용자의 PR/이슈 필요 |
| 홈페이지 | 접근 가능한 프로젝트 홈페이지 |

> 위 3가지 인기도 기준 중 **하나 이상**을 충족하면 됨.

#### GUI 앱 관련 제한

- **homebrew-core에는 `.app` 번들 포함 불가**
- GUI는 선택적이어야 하며, CLI 도구/라이브러리 우선
- X11/XQuartz GUI 회피

> **Crux 시사점:** CLI로 실행 가능한 바이너리를 Formula로 등록하고, `.app` 번들은 Cask 또는 Tap으로 별도 배포하는 전략이 적합.

### 2.2 homebrew-cask 제출 요건

#### 필수 요건

| 요건 | 상세 |
|------|------|
| **Gatekeeper 통과** | Apple Silicon Mac에서 Gatekeeper 검증 통과 필수 → **코드 서명 + 공증 필수** |
| **SIP 호환** | System Integrity Protection 비활성화 요구 불가 |
| **최신 macOS 호환** | 최신 macOS 버전과 호환 |
| **GUI 앱** | CLI 전용 오픈소스는 homebrew-core로 이동 |

#### 인기도 요건

| 기준 | 최소값 |
|------|--------|
| GitHub Stars | 75개 이상 |
| GitHub Forks | 30개 이상 |
| GitHub Watchers | 30개 이상 |

#### 흔한 거부 사유

- Gatekeeper 실패 (코드 서명/공증 미비)
- 유지보수 중단된 소프트웨어
- 알려진 보안 취약점 미패치
- CLI 전용 오픈소스 (homebrew-core로 이동 권고)
- 정보 없는 비공개 앱

### 2.3 제출 프로세스

#### 테스트 및 감사

```bash
# Formula의 경우
brew uninstall --force crux
HOMEBREW_NO_INSTALL_FROM_API=1 brew install --build-from-source crux
brew test crux
brew audit --strict --new --online crux
brew style crux

# Cask의 경우
export HOMEBREW_NO_AUTO_UPDATE=1
export HOMEBREW_NO_INSTALL_FROM_API=1
brew install --cask crux
brew uninstall --cask crux
brew audit --new --cask crux
brew style --fix crux
```

#### PR 제출 순서

1. homebrew-core 또는 homebrew-cask 저장소 Fork
2. 기존 유사 Formula/Cask 참고하여 작성
3. 로컬 테스트 통과 확인
4. Feature branch 생성 후 PR 제출
5. BrewTestBot 자동 빌드/테스트 대기
6. 메인테이너 리뷰 및 피드백 대응

#### 리뷰 타임라인

- **일반적:** 1~4주
- BrewTestBot 자동 테스트 통과 후 메인테이너 수동 리뷰
- 피드백 반영 후 재리뷰 필요

---

## 3. macOS 코드 서명 & 공증(Notarization)

### 3.1 왜 필요한가?

| 상황 | 서명/공증 필요 여부 |
|------|-------------------|
| homebrew-core Formula (소스 빌드) | **불필요** — 사용자 로컬에서 빌드 |
| homebrew-cask (미리 빌드된 바이너리) | **필수** — Gatekeeper 검증 통과 필요 |
| GitHub Releases 직접 다운로드 | **강력 권장** — 없으면 사용자가 수동 우회 필요 |
| 커스텀 Tap (소스 빌드) | **불필요** — 사용자 로컬에서 빌드 |

### 3.2 Apple Developer Program

| 항목 | 상세 |
|------|------|
| **비용** | 연간 $99 USD |
| **가입 유형** | 개인 또는 조직 |
| **필요한 인증서** | "Developer ID Application" (Gatekeeper용) |
| **API 키** | App Store Connect API 키 (CI 자동화용) |

### 3.3 코드 서명 프로세스

```bash
# 1. 인증서 확인
security find-identity -v -p codesigning

# 2. 바이너리 서명
codesign --force --options runtime \
  --sign "Developer ID Application: Your Name (TEAM_ID)" \
  --timestamp \
  target/release/crux

# 3. .app 번들 서명 (Deep signing)
codesign --force --deep --options runtime \
  --sign "Developer ID Application: Your Name (TEAM_ID)" \
  --timestamp \
  Crux.app

# 4. 서명 검증
codesign --verify --deep --strict --verbose=2 Crux.app
spctl --assess --type exec --verbose=2 Crux.app
```

**주요 옵션:**
- `--options runtime` — Hardened Runtime 활성화 (공증 필수 조건)
- `--timestamp` — 인증서 만료 후에도 서명 유효 유지
- `--force --deep` — 번들 내 모든 실행 파일 재서명

### 3.4 공증(Notarization) 프로세스

```bash
# 1. ZIP 또는 DMG로 패키징
ditto -c -k --keepParent Crux.app Crux.zip
# 또는
hdiutil create -volname "Crux" -srcfolder Crux.app -ov -format UDZO Crux.dmg

# 2. notarytool로 제출 (API 키 방식 — CI 권장)
xcrun notarytool submit Crux.dmg \
  --key ~/.private_keys/AuthKey_XXXXX.p8 \
  --key-id XXXXX \
  --issuer "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" \
  --wait

# 3. 공증 결과 확인
xcrun notarytool log <submission-id> \
  --key ~/.private_keys/AuthKey_XXXXX.p8 \
  --key-id XXXXX \
  --issuer "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"

# 4. Staple (오프라인 검증용)
xcrun stapler staple Crux.dmg
# 또는
xcrun stapler staple Crux.app
```

### 3.5 Ad-hoc 서명 (대안)

**Apple Developer Program 없이 사용 가능한 방법:**

```bash
# Ad-hoc 서명 (무료, 로컬에서만 유효)
codesign --force --deep --sign - Crux.app
```

- Gatekeeper 통과 **불가** → homebrew-cask 등록 불가
- GitHub Releases 배포 시 사용자가 `xattr -rd com.apple.quarantine` 실행 필요
- **소스 빌드 Formula의 경우에는 문제 없음** (Homebrew가 로컬 빌드 후 ad-hoc 서명)

### 3.6 rcodesign (오픈소스 대안)

Apple의 `codesign`/`notarytool` 대신 순수 Rust 구현을 사용할 수 있다:

```bash
# rcodesign 설치
cargo install apple-codesign

# 서명
rcodesign sign --p12-file developer-id.p12 --p12-password-file pw.txt Crux.app

# 공증 제출
rcodesign notary-submit --api-key-path api-key.json Crux.dmg --wait
```

장점: macOS가 아닌 환경(Linux CI)에서도 서명/공증 가능.

### 3.7 참고: 다른 프로젝트의 서명 현황

| 프로젝트 | 코드 서명 | 공증 | Homebrew 상태 |
|----------|----------|------|--------------|
| **Alacritty** | Ad-hoc (`--sign -`) | 없음 | **Cask deprecated** (2025-10) |
| **WezTerm** | Developer ID | 있음 | Cask 정상 운영 |
| **Rio** | N/A (Formula 소스 빌드) | N/A | **Formula 정상 운영** |

---

## 4. Universal Binary 빌드

### 4.1 개요

macOS는 Apple Silicon(arm64)과 Intel(x86_64) 두 아키텍처를 지원한다. Universal Binary는 `lipo`로 두 아키텍처 바이너리를 하나로 합친 것이다.

### 4.2 빌드 프로세스

```bash
# 1. 타겟 추가
rustup target add aarch64-apple-darwin x86_64-apple-darwin

# 2. 각 아키텍처별 빌드
MACOSX_DEPLOYMENT_TARGET="13.0" cargo build --release --target=aarch64-apple-darwin
MACOSX_DEPLOYMENT_TARGET="13.0" cargo build --release --target=x86_64-apple-darwin

# 3. lipo로 Universal Binary 생성
lipo -create \
  target/aarch64-apple-darwin/release/crux \
  target/x86_64-apple-darwin/release/crux \
  -output target/release/crux-universal

# 4. 검증
file target/release/crux-universal
# 출력: crux-universal: Mach-O universal binary with 2 architectures:
#   [x86_64:Mach-O 64-bit executable x86_64]
#   [arm64:Mach-O 64-bit executable arm64]

lipo -info target/release/crux-universal
# 출력: Architectures in the fat file: crux-universal are: x86_64 arm64
```

### 4.3 Alacritty의 Universal Binary Makefile

Alacritty의 실제 Makefile에서 참고할 부분:

```makefile
# Universal Binary 빌드
binary-universal:
	MACOSX_DEPLOYMENT_TARGET="10.12" cargo build --release --target=x86_64-apple-darwin
	MACOSX_DEPLOYMENT_TARGET="10.12" cargo build --release --target=aarch64-apple-darwin
	@lipo target/x86_64-apple-darwin/release/alacritty \
		target/aarch64-apple-darwin/release/alacritty \
		-create -output target/release/alacritty

# Universal .app 번들 생성
app-universal: binary-universal
	# man page, terminfo, 리소스 복사 등...
	@codesign --force --deep --sign - $(APP_DIR)

# Universal DMG 생성
dmg-universal: app-universal
	@ln -sf /Applications $(DMG_DIR)/Applications
	@hdiutil create -volname "Alacritty" \
		-srcfolder $(DMG_DIR) \
		-ov -format UDZO \
		target/release/osx/Alacritty.dmg
```

### 4.4 GPUI/Metal과 Universal Binary

| 고려사항 | 상세 |
|---------|------|
| Metal 지원 | Metal은 x86_64와 arm64 모두에서 동작 (macOS 10.14+) |
| GPUI 크로스 컴파일 | macOS에서 두 타겟 모두 네이티브 컴파일 가능 (크로스 컴파일러 불필요) |
| `MACOSX_DEPLOYMENT_TARGET` | macOS 13+ → `"13.0"` 으로 설정 |
| GitHub Actions runner | `macos-latest` (Apple Silicon) 에서 x86_64 크로스 빌드 가능 |

### 4.5 Crux용 Makefile 템플릿

```makefile
APP_NAME = Crux
APP_DIR = target/release/osx/$(APP_NAME).app
DMG_DIR = target/release/osx

MACOS_MIN = 13.0

.PHONY: binary-universal app-universal dmg-universal

binary-universal:
	MACOSX_DEPLOYMENT_TARGET="$(MACOS_MIN)" cargo build --release --target=x86_64-apple-darwin
	MACOSX_DEPLOYMENT_TARGET="$(MACOS_MIN)" cargo build --release --target=aarch64-apple-darwin
	@lipo target/x86_64-apple-darwin/release/crux \
		target/aarch64-apple-darwin/release/crux \
		-create -output target/release/crux

app-universal: binary-universal
	@mkdir -p "$(APP_DIR)/Contents/MacOS"
	@mkdir -p "$(APP_DIR)/Contents/Resources"
	@cp target/release/crux "$(APP_DIR)/Contents/MacOS/crux"
	@cp resources/Info.plist "$(APP_DIR)/Contents/Info.plist"
	@cp resources/crux.icns "$(APP_DIR)/Contents/Resources/crux.icns"
	@codesign --force --deep --sign - "$(APP_DIR)"

dmg-universal: app-universal
	@mkdir -p "$(DMG_DIR)"
	@ln -sf /Applications "$(DMG_DIR)/Applications"
	@hdiutil create -volname "$(APP_NAME)" \
		-srcfolder "$(DMG_DIR)" \
		-ov -format UDZO \
		target/release/osx/$(APP_NAME).dmg
```

---

## 5. CI/CD 파이프라인 (GitHub Actions)

### 5.1 Alacritty의 실제 Release 워크플로우

```yaml
name: Release

on:
  push:
    tags: ["v[0-9]+.[0-9]+.[0-9]+*"]

env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  CARGO_TERM_COLOR: always

jobs:
  macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install dependencies
        run: brew install scdoc
      - name: Install ARM target
        run: rustup update && rustup target add aarch64-apple-darwin && rustup target add x86_64-apple-darwin
      - name: Test
        run: cargo test --release --target=x86_64-apple-darwin
      - name: Build ARM
        run: cargo build --release --target=aarch64-apple-darwin
      - name: Make DMG
        run: make dmg-universal
      - name: Upload Application
        run: |
          mv ./target/release/osx/Alacritty.dmg ./Alacritty-${GITHUB_REF##*/}.dmg
          ./.github/workflows/upload_asset.sh ./Alacritty-${GITHUB_REF##*/}.dmg $GITHUB_TOKEN
```

### 5.2 Crux를 위한 완전한 CI/CD 워크플로우

#### CI 워크플로우 (PR 검증)

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  check:
    name: Check & Lint
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Cache Rust dependencies
        uses: Swatinem/rust-cache@v2
        with:
          shared-key: "macos-check"

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Clippy lints
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Build
        run: cargo build --release

      - name: Run tests
        run: cargo test --release

  build-universal:
    name: Build Universal Binary
    runs-on: macos-latest
    needs: check
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin,x86_64-apple-darwin

      - name: Cache Rust dependencies
        uses: Swatinem/rust-cache@v2
        with:
          shared-key: "macos-universal"

      - name: Build arm64
        run: |
          MACOSX_DEPLOYMENT_TARGET="13.0" cargo build --release --target=aarch64-apple-darwin

      - name: Build x86_64
        run: |
          MACOSX_DEPLOYMENT_TARGET="13.0" cargo build --release --target=x86_64-apple-darwin

      - name: Create Universal Binary
        run: |
          lipo -create \
            target/aarch64-apple-darwin/release/crux \
            target/x86_64-apple-darwin/release/crux \
            -output target/release/crux-universal
          file target/release/crux-universal
```

#### Release 워크플로우 (태그 → 빌드 → 서명 → 공증 → 배포)

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ["v[0-9]+.[0-9]+.[0-9]+*"]

env:
  CARGO_TERM_COLOR: always

permissions:
  contents: write

jobs:
  build-macos:
    name: Build macOS Universal
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin,x86_64-apple-darwin

      - name: Cache Rust dependencies
        uses: Swatinem/rust-cache@v2

      - name: Build arm64
        run: |
          MACOSX_DEPLOYMENT_TARGET="13.0" cargo build --release --target=aarch64-apple-darwin

      - name: Build x86_64
        run: |
          MACOSX_DEPLOYMENT_TARGET="13.0" cargo build --release --target=x86_64-apple-darwin

      - name: Create Universal Binary
        run: |
          lipo -create \
            target/aarch64-apple-darwin/release/crux \
            target/x86_64-apple-darwin/release/crux \
            -output target/release/crux

      - name: Create .app bundle
        run: make app-universal

      - name: Import signing certificate
        env:
          CERTIFICATE_BASE64: ${{ secrets.APPLE_CERTIFICATE_BASE64 }}
          CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          KEYCHAIN_PASSWORD: ${{ secrets.KEYCHAIN_PASSWORD }}
        run: |
          # 임시 키체인 생성
          KEYCHAIN_PATH=$RUNNER_TEMP/app-signing.keychain-db
          security create-keychain -p "$KEYCHAIN_PASSWORD" $KEYCHAIN_PATH
          security set-keychain-settings -lut 21600 $KEYCHAIN_PATH
          security unlock-keychain -p "$KEYCHAIN_PASSWORD" $KEYCHAIN_PATH

          # 인증서 가져오기
          echo -n "$CERTIFICATE_BASE64" | base64 --decode -o $RUNNER_TEMP/certificate.p12
          security import $RUNNER_TEMP/certificate.p12 -P "$CERTIFICATE_PASSWORD" \
            -A -t cert -f pkcs12 -k $KEYCHAIN_PATH
          security list-keychain -d user -s $KEYCHAIN_PATH

      - name: Code sign application
        env:
          SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
        run: |
          codesign --force --deep --options runtime \
            --sign "$SIGNING_IDENTITY" \
            --timestamp \
            target/release/osx/Crux.app

          # 서명 검증
          codesign --verify --deep --strict --verbose=2 target/release/osx/Crux.app

      - name: Create DMG
        run: make dmg-universal

      - name: Notarize DMG
        env:
          APPLE_API_KEY: ${{ secrets.APPLE_API_KEY }}
          APPLE_API_KEY_ID: ${{ secrets.APPLE_API_KEY_ID }}
          APPLE_API_ISSUER: ${{ secrets.APPLE_API_ISSUER }}
        run: |
          # API 키 파일 생성
          mkdir -p ~/.private_keys
          echo -n "$APPLE_API_KEY" > ~/.private_keys/AuthKey_${APPLE_API_KEY_ID}.p8

          # 공증 제출 및 대기
          xcrun notarytool submit target/release/osx/Crux.dmg \
            --key ~/.private_keys/AuthKey_${APPLE_API_KEY_ID}.p8 \
            --key-id "$APPLE_API_KEY_ID" \
            --issuer "$APPLE_API_ISSUER" \
            --wait

          # Staple
          xcrun stapler staple target/release/osx/Crux.dmg

      - name: Get version from tag
        id: version
        run: echo "VERSION=${GITHUB_REF#refs/tags/}" >> $GITHUB_OUTPUT

      - name: Upload DMG artifact
        uses: actions/upload-artifact@v4
        with:
          name: Crux-${{ steps.version.outputs.VERSION }}.dmg
          path: target/release/osx/Crux.dmg

      - name: Upload binary artifact
        uses: actions/upload-artifact@v4
        with:
          name: crux-${{ steps.version.outputs.VERSION }}-universal-apple-darwin
          path: target/release/crux

  create-release:
    name: Create GitHub Release
    needs: build-macos
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Get version from tag
        id: version
        run: echo "VERSION=${GITHUB_REF#refs/tags/}" >> $GITHUB_OUTPUT

      - name: Generate changelog
        uses: orhun/git-cliff-action@v4
        id: changelog
        with:
          config: cliff.toml
          args: --latest --strip header

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: Prepare release assets
        run: |
          VERSION=${{ steps.version.outputs.VERSION }}
          # DMG 이름 변경
          mv artifacts/Crux-${VERSION}.dmg/Crux.dmg ./Crux-${VERSION}.dmg
          # 바이너리 압축
          cd artifacts/crux-${VERSION}-universal-apple-darwin
          tar czf ../../crux-${VERSION}-universal-apple-darwin.tar.gz crux
          cd ../..
          # SHA256 체크섬 생성
          shasum -a 256 Crux-${VERSION}.dmg > checksums.txt
          shasum -a 256 crux-${VERSION}-universal-apple-darwin.tar.gz >> checksums.txt

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          body: ${{ steps.changelog.outputs.content }}
          files: |
            Crux-${{ steps.version.outputs.VERSION }}.dmg
            crux-${{ steps.version.outputs.VERSION }}-universal-apple-darwin.tar.gz
            checksums.txt
          draft: false
          prerelease: ${{ contains(github.ref, '-') }}

  update-homebrew-tap:
    name: Update Homebrew Tap
    needs: create-release
    runs-on: ubuntu-latest
    steps:
      - name: Get version from tag
        id: version
        run: echo "VERSION=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT

      - name: Trigger tap update
        env:
          GH_TOKEN: ${{ secrets.HOMEBREW_TAP_TOKEN }}
        run: |
          gh workflow run update-formula.yml \
            -f version=${{ steps.version.outputs.VERSION }} \
            -R crux-terminal/homebrew-crux
```

### 5.3 캐싱 전략

```yaml
# 방법 1: Swatinem/rust-cache (권장)
- name: Cache Rust dependencies
  uses: Swatinem/rust-cache@v2
  with:
    # 작업별로 캐시 분리
    shared-key: "macos-release"
    # 캐시할 추가 디렉토리
    cache-directories: |
      ~/.cargo/registry
      ~/.cargo/git

# 방법 2: sccache (컴파일러 레벨 캐싱)
- name: Setup sccache
  uses: mozilla-actions/sccache-action@v0.0.9

- name: Configure sccache
  run: |
    echo "SCCACHE_GHA_ENABLED=true" >> $GITHUB_ENV
    echo "RUSTC_WRAPPER=sccache" >> $GITHUB_ENV

# 두 가지 병행 사용 가능 (가장 빠름)
```

| 캐싱 방법 | 캐시 대상 | 절감 효과 |
|-----------|----------|----------|
| Swatinem/rust-cache | cargo registry, target 디렉토리 | 의존성 재다운로드/재빌드 방지 |
| sccache | 컴파일 결과물 (rustc 출력) | 컴파일 시간 50-70% 감소 |
| 병행 사용 | 모든 레벨 | 최대 80% 빌드 시간 감소 |

### 5.4 필요한 GitHub Secrets

| Secret 이름 | 용도 | 설정 방법 |
|-------------|------|----------|
| `APPLE_CERTIFICATE_BASE64` | 코드 서명 인증서 (.p12) | `base64 < cert.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | 인증서 암호 | Apple Developer에서 내보내기 시 설정 |
| `KEYCHAIN_PASSWORD` | CI 임시 키체인 암호 | 임의 문자열 |
| `APPLE_SIGNING_IDENTITY` | 서명 ID | `"Developer ID Application: Name (TEAMID)"` |
| `APPLE_API_KEY` | App Store Connect API 키 내용 | API 키 .p8 파일 내용 |
| `APPLE_API_KEY_ID` | API 키 ID | Apple Developer 포털에서 확인 |
| `APPLE_API_ISSUER` | API 발급자 UUID | Apple Developer 포털에서 확인 |
| `HOMEBREW_TAP_TOKEN` | Tap 저장소 접근 토큰 | GitHub PAT (actions:write, contents:write) |

---

## 6. 릴리스 엔지니어링

### 6.1 버전 관리 (SemVer)

```
MAJOR.MINOR.PATCH
  |     |     |
  |     |     └── 버그 수정 (하위 호환)
  |     └──────── 기능 추가 (하위 호환)
  └────────────── 호환성 깨는 변경
```

**Crux 초기 단계 권장:**
- `0.x.y` — 초기 개발 (API 안정성 보장 안 함)
- `0.1.0` — 첫 공개 릴리스
- `1.0.0` — 안정 릴리스 (일상 사용 가능)

### 6.2 Conventional Commits

```
feat: 새로운 기능 추가
fix: 버그 수정
docs: 문서 변경
style: 코드 스타일 변경 (기능 변화 없음)
refactor: 코드 리팩토링
perf: 성능 개선
test: 테스트 추가/수정
build: 빌드 시스템 변경
ci: CI/CD 변경
chore: 기타 변경
```

예시:
```
feat(renderer): Metal 렌더링 파이프라인 최적화
fix(input): 한글 IME 입력 시 커서 위치 오류 수정
perf(gpu): 셰이더 컴파일 캐싱 추가
```

### 6.3 git-cliff 설정

```toml
# cliff.toml
[changelog]
header = """
# Changelog\n
All notable changes to this project will be documented in this file.\n
"""
body = """
{% if version %}\
    ## [{{ version | trim_start_matches(pat="v") }}] - {{ timestamp | date(format="%Y-%m-%d") }}
{% else %}\
    ## [unreleased]
{% endif %}\
{% for group, commits in commits | group_by(attribute="group") %}
    ### {{ group | striptags | trim | upper_first }}
    {% for commit in commits %}
        - {% if commit.scope %}*({{ commit.scope }})* {% endif %}\
            {% if commit.breaking %}[**breaking**] {% endif %}\
            {{ commit.message | upper_first }}\
            {% if commit.links %} ({{ commit.links | join(sep=", ") }}){% endif %}\
    {% endfor %}
{% endfor %}\n
"""
trim = true

[git]
conventional_commits = true
filter_unconventional = true
split_commits = false
commit_parsers = [
    { message = "^feat", group = "Features" },
    { message = "^fix", group = "Bug Fixes" },
    { message = "^doc", group = "Documentation" },
    { message = "^perf", group = "Performance" },
    { message = "^refactor", group = "Refactoring" },
    { message = "^style", group = "Styling" },
    { message = "^test", group = "Testing" },
    { message = "^build", group = "Build" },
    { message = "^ci", group = "CI/CD" },
    { message = "^chore", skip = true },
]
filter_commits = false
tag_pattern = "v[0-9].*"
```

### 6.4 GitHub Release Notes 형식

```markdown
## Crux v0.1.0

### Features
- **(renderer)** Metal 기반 GPU 가속 렌더링 엔진
- **(terminal)** 기본 VTE 호환 터미널 에뮬레이션
- **(config)** TOML 기반 설정 파일 지원

### Bug Fixes
- **(input)** 한글 IME 조합 중 커서 깜박임 수정

### Downloads

| 플랫폼 | 파일 |
|--------|------|
| macOS (Universal) DMG | [Crux-v0.1.0.dmg](link) |
| macOS (Universal) Binary | [crux-v0.1.0-universal-apple-darwin.tar.gz](link) |

### Checksums (SHA256)
```
abc123... Crux-v0.1.0.dmg
def456... crux-v0.1.0-universal-apple-darwin.tar.gz
```

### Installation

**Homebrew (권장):**
```bash
brew tap crux-terminal/crux
brew install crux
```
```

### 6.5 자동화 파이프라인 요약

```
[Conventional Commit] → [Push to main]
       ↓
[release-plz / manual tag]
       ↓
[v0.1.0 태그 생성]
       ↓
[GitHub Actions Release 워크플로우 트리거]
       ↓
┌──────────────────────────────┐
│ Build Universal Binary       │
│ → Code Sign                  │
│ → Create DMG                 │
│ → Notarize                   │
│ → Staple                     │
└──────────────────────────────┘
       ↓
[git-cliff로 CHANGELOG 생성]
       ↓
[GitHub Release 생성 + 에셋 업로드]
       ↓
[Homebrew Tap 포뮬러 자동 업데이트]
```

---

## 7. Homebrew Tap (커스텀 저장소)

### 7.1 왜 Tap으로 시작하는가?

| 이유 | 설명 |
|------|------|
| 즉시 배포 | homebrew-core 리뷰 대기 없이 즉시 사용자에게 배포 |
| 인기도 요건 없음 | Stars/Forks 제한 없음 |
| 완전한 통제 | 릴리스 주기, 포뮬러 구조 자유롭게 결정 |
| 빠른 업데이트 | homebrew-core PR 리뷰 없이 즉시 업데이트 |
| 나중에 이전 가능 | 프로젝트 성장 후 homebrew-core/cask로 이전 가능 |

### 7.2 Tap 저장소 생성

```bash
# 1. GitHub에 homebrew-crux 저장소 생성
# https://github.com/crux-terminal/homebrew-crux

# 2. 로컬에서 Tap 초기화
brew tap-new crux-terminal/crux

# 3. Tap 디렉토리로 이동
cd $(brew --repository crux-terminal/crux)

# 4. 원격 저장소 연결
git remote set-url origin https://github.com/crux-terminal/homebrew-crux
git push --set-upstream origin main
```

### 7.3 Tap용 Formula (소스 빌드)

```ruby
# Formula/crux.rb
class Crux < Formula
  desc "GPU-accelerated terminal emulator built with Rust and GPUI"
  homepage "https://github.com/crux-terminal/crux"
  url "https://github.com/crux-terminal/crux/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "여기에_실제_sha256_해시"
  license "MIT"
  head "https://github.com/crux-terminal/crux.git", branch: "main"

  livecheck do
    url :stable
    regex(/^v?(\d+(?:\.\d+)+)$/i)
  end

  depends_on "rust" => :build
  depends_on :macos

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/crux --version")
  end
end
```

### 7.4 Tap용 Cask (미리 빌드된 DMG)

```ruby
# Casks/crux.rb
cask "crux" do
  version "0.1.0"
  sha256 "여기에_실제_sha256_해시"

  url "https://github.com/crux-terminal/crux/releases/download/v#{version}/Crux-v#{version}.dmg"
  name "Crux"
  desc "GPU-accelerated terminal emulator built with Rust and GPUI"
  homepage "https://github.com/crux-terminal/crux"

  depends_on macos: ">= :ventura"

  app "Crux.app"
  binary "#{appdir}/Crux.app/Contents/MacOS/crux"

  zap trash: [
    "~/.config/crux",
    "~/Library/Preferences/com.crux-terminal.crux.plist",
    "~/Library/Saved Application State/com.crux-terminal.crux.savedState",
  ]
end
```

### 7.5 사용자 설치 명령어

```bash
# Formula (소스 빌드) 방식
brew tap crux-terminal/crux
brew install crux

# Cask (미리 빌드된 DMG) 방식
brew tap crux-terminal/crux
brew install --cask crux

# 또는 한 줄로
brew install crux-terminal/crux/crux
```

### 7.6 Tap 포뮬러 자동 업데이트

#### Tap 저장소의 업데이트 워크플로우

```yaml
# .github/workflows/update-formula.yml (homebrew-crux 저장소)
name: Update Formula

on:
  workflow_dispatch:
    inputs:
      version:
        description: "New version (without v prefix)"
        required: true
        type: string

jobs:
  update:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Update Formula
        run: |
          VERSION="${{ github.event.inputs.version }}"
          TARBALL_URL="https://github.com/crux-terminal/crux/archive/refs/tags/v${VERSION}.tar.gz"

          # SHA256 계산
          SHA256=$(curl -sL "$TARBALL_URL" | shasum -a 256 | cut -d' ' -f1)

          # Formula 업데이트
          cat > Formula/crux.rb << 'FORMULA_EOF'
          class Crux < Formula
            desc "GPU-accelerated terminal emulator built with Rust and GPUI"
            homepage "https://github.com/crux-terminal/crux"
            url "https://github.com/crux-terminal/crux/archive/refs/tags/v${VERSION}.tar.gz"
            sha256 "${SHA256}"
            license "MIT"
            head "https://github.com/crux-terminal/crux.git", branch: "main"

            livecheck do
              url :stable
              regex(/^v?(\d+(?:\.\d+)+)$/i)
            end

            depends_on "rust" => :build
            depends_on :macos

            def install
              system "cargo", "install", *std_cargo_args
            end

            test do
              assert_match version.to_s, shell_output("#{bin}/crux --version")
            end
          end
          FORMULA_EOF

          # 실제 값으로 치환
          sed -i "s|\${VERSION}|${VERSION}|g" Formula/crux.rb
          sed -i "s|\${SHA256}|${SHA256}|g" Formula/crux.rb

      - name: Commit and push
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git add Formula/crux.rb
          git commit -m "crux ${VERSION}"
          git push
```

#### 메인 저장소에서 Tap 업데이트 트리거

```yaml
# Release 워크플로우의 마지막 단계에 추가
- name: Update Homebrew Tap
  env:
    GH_TOKEN: ${{ secrets.HOMEBREW_TAP_TOKEN }}
  run: |
    VERSION=${GITHUB_REF#refs/tags/v}
    gh workflow run update-formula.yml \
      -f version=$VERSION \
      -R crux-terminal/homebrew-crux
```

### 7.7 homebrew-core 이전 체크리스트

Tap에서 충분히 안정화된 후 homebrew-core로 이전할 때의 체크리스트:

- [ ] GitHub Stars 75개 이상 달성
- [ ] 외부 사용자의 PR/Issue 존재
- [ ] 안정 태그 릴리스 존재
- [ ] `brew audit --strict --new --online crux` 통과
- [ ] `brew test crux` 통과
- [ ] 최근 3개 macOS 버전에서 빌드/테스트 통과 확인
- [ ] DFSG 호환 라이선스 (MIT, Apache 2.0 등)
- [ ] 자체 업데이트 기능 비활성화 확인
- [ ] README에 Homebrew 설치 방법 문서화

---

## 8. Crux를 위한 권장 전략

### 8.1 단계별 배포 로드맵

```
Phase 1 (MVP): GitHub Releases + 커스텀 Tap (Formula)
  ↓
Phase 2 (안정화): 코드 서명 + 공증 + Tap에 Cask 추가
  ↓
Phase 3 (성장): homebrew-core Formula 제출
  ↓
Phase 4 (성숙): homebrew-cask 제출 (선택적)
```

### Phase 1: MVP (즉시 시작 가능)

**비용: $0 | 난이도: 낮음**

1. GitHub Releases에 Universal Binary + 소스 tarball 업로드
2. `crux-terminal/homebrew-crux` Tap 생성
3. Formula(소스 빌드) 방식으로 배포
4. `brew tap crux-terminal/crux && brew install crux`

**이 단계에서 필요한 것:**
- GitHub Actions Release 워크플로우
- Tap 저장소 + Formula
- Makefile (Universal Binary 빌드)

### Phase 2: 안정화 ($99/년 필요)

**비용: $99/년 | 난이도: 중간**

1. Apple Developer Program 가입
2. CI/CD에 코드 서명 + 공증 추가
3. DMG 배포 (서명/공증 완료)
4. Tap에 Cask 추가 (미리 빌드된 DMG)

### Phase 3: 성장

**비용: $0 추가 | 난이도: 중간**

1. homebrew-core에 Formula PR 제출
2. 인기도 요건(75 stars) 충족 확인
3. 모든 macOS 버전 + 플랫폼 빌드 테스트 통과
4. 메인테이너 리뷰 대응

### Phase 4: 성숙 (선택적)

**비용: $0 추가 | 난이도: 높음**

1. homebrew-cask에 Cask PR 제출 (코드 서명/공증 필수)
2. Gatekeeper 통과 검증
3. homebrew-core Formula + homebrew-cask Cask 병행 운영

### 8.2 최종 권장 사항

| 결정 사항 | 권장 | 이유 |
|-----------|------|------|
| 초기 배포 | Formula (소스 빌드) | 코드 서명 불필요, Alacritty deprecation 교훈 |
| 저장소 | 커스텀 Tap | 즉시 배포, 완전한 통제 |
| Universal Binary | 필수 | Apple Silicon + Intel 지원 |
| CI/CD | GitHub Actions | 무료, Rust 생태계 통합 우수 |
| 캐싱 | Swatinem/rust-cache + sccache | 최대 80% 빌드 시간 감소 |
| 버전 관리 | SemVer + Conventional Commits | 자동 CHANGELOG 생성 |
| 코드 서명 | Phase 2에서 도입 | $99/년 비용이지만 장기적으로 필수 |
| CHANGELOG | git-cliff | Rust 생태계 표준, 높은 커스터마이징 |

---

## 참고 자료

- [Homebrew Acceptable Formulae](https://docs.brew.sh/Acceptable-Formulae)
- [Homebrew Acceptable Casks](https://docs.brew.sh/Acceptable-Casks)
- [Homebrew Adding Software](https://docs.brew.sh/Adding-Software-to-Homebrew)
- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Homebrew Taps](https://docs.brew.sh/Taps)
- [Rio Terminal Formula](https://formulae.brew.sh/formula/rio-terminal)
- [Alacritty Cask](https://formulae.brew.sh/cask/alacritty)
- [WezTerm Cask](https://formulae.brew.sh/cask/wezterm)
- [Alacritty Signing Issue](https://github.com/alacritty/alacritty/issues/8749)
- [Alacritty Release Workflow](https://github.com/alacritty/alacritty/actions/workflows/release.yml)
- [WezTerm Homebrew Tap](https://github.com/wezterm/homebrew-wezterm)
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)
- [git-cliff](https://git-cliff.org/)
- [Automated Rust Releases](https://blog.orhun.dev/automated-rust-releases/)
- [Homebrew Tap with Bottles](https://brew.sh/2020/11/18/homebrew-tap-with-bottles-uploaded-to-github-releases/)
- [Automate Homebrew Formula Updates](https://josh.fail/2023/automate-updating-custom-homebrew-formulae-with-github-actions/)
- [macOS Code Signing Guide](https://dennisbabkin.com/blog/?t=how-to-get-certificate-code-sign-notarize-macos-binaries-outside-apple-app-store)
- [rcodesign (Rust Code Signing)](https://gregoryszorc.com/blog/2022/08/08/achieving-a-completely-open-source-implementation-of-apple-code-signing-and-notarization/)
