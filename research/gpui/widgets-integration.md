---
title: "GPUI 위젯 통합 연구 — DockArea, Tabs, ResizablePanel"
description: "gpui-component 라이브러리의 DockArea, TabBar, ResizablePanel 위젯을 활용한 탭/분할 패인 UI 구현 전략"
date: 2026-02-12
phase: [2]
topics: [gpui, gpui-component, dock-area, tabs, resizable-panel, split-panes, markdown, widget-composition]
status: final
related:
  - framework.md
  - terminal-implementations.md
  - bootstrap.md
  - ../integration/ipc-protocol-design.md
---

# GPUI 위젯 통합 연구 보고서

> Crux 터미널 에뮬레이터의 탭/분할 패인 UI 구현을 위한 gpui-component 위젯 분석
> 작성일: 2026-02-12

---

## 목차

1. [gpui-component 라이브러리 개요](#1-gpui-component-라이브러리-개요)
2. [DockArea 위젯 — 분할 패인 관리](#2-dockarea-위젯--분할-패인-관리)
3. [Tab / TabBar 위젯](#3-tab--tabbar-위젯)
4. [ResizablePanel 위젯](#4-resizablepanel-위젯)
5. [위젯 조합 아키텍처](#5-위젯-조합-아키텍처)
6. [Markdown 렌더링](#6-markdown-렌더링)
7. [Ghostty 비교 분석](#7-ghostty-비교-분석)
8. [Crux 구현 전략](#8-crux-구현-전략)
9. [참고 자료](#9-참고-자료)

---

## 1. gpui-component 라이브러리 개요

### 1.1 라이브러리 소개

[gpui-component](https://github.com/longbridge/gpui-component)는 Longbridge에서 개발한 **60+ 크로스 플랫폼 UI 위젯 라이브러리**로, GPUI 프레임워크 위에서 동작한다. Zed 에디터 자체 컴포넌트와는 별도의 독립 크레이트로, IDE급 레이아웃 시스템을 제공한다.

| 항목 | 상세 |
|------|------|
| 크레이트 | `gpui-component` |
| 현재 버전 | 0.5.1 |
| GPUI 호환 버전 | gpui 0.2.2 |
| 라이선스 | Apache-2.0 |
| 위젯 수 | 55+ (공식 컴포넌트 페이지 기준) |
| 문서화율 | ~53% |

### 1.2 설치 및 초기화

```toml
[dependencies]
gpui = "0.2.2"
gpui-component = "0.5.1"
gpui-component-assets = "0.5.1"  # 기본 아이콘 (선택)
```

**초기화 패턴** — `gpui_component::init(cx)`는 앱 시작 시 반드시 최상위에서 호출해야 한다:

```rust
use gpui::*;
use gpui_component::*;

fn main() {
    let app = Application::new().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);  // 테마 시스템 및 글로벌 기능 활성화

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|_| MyApp::new());
                cx.new(|cx| Root::new(view, window, cx))
            })?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
```

> **핵심**: `Root::new()`으로 감싸야 테마 시스템과 전역 컴포넌트 기능이 정상 동작한다.

### 1.3 위젯 카테고리

| 카테고리 | 위젯 |
|----------|-------|
| **기본** | Button, Checkbox, Radio, Switch, Input, Select, Slider, Label, Badge, Tag |
| **레이아웃** | DockArea, TabPanel, StackPanel, ResizablePanel, Sidebar, Sheet |
| **고급** | Table, Tree, List, VirtualList, Calendar, Chart, Menu |
| **입력** | DatePicker, ColorPicker, OtpInput, NumberInput, Editor (코드 에디터) |
| **피드백** | Dialog, Notification, Popover, Tooltip, Alert, Progress, Spinner |
| **유틸** | Icon, Kbd, Divider, Skeleton, Clipboard, Scrollable |

### 1.4 설계 철학

gpui-component는 **Stateless RenderOnce 엘리먼트**를 기본 빌딩 블록으로 사용한다. 상태 관리는 개별 위젯이 아닌 뷰 레벨에서 처리하며, 이는 GPUI의 Entity-View 패턴과 일치한다.

- **Stateless** (RenderOnce): Button, Tab, Badge 등 — 단순하고 예측 가능
- **Stateful** (Render + Entity): DockArea, Table, Editor 등 — 내부 상태를 Entity로 관리

---

## 2. DockArea 위젯 — 분할 패인 관리

### 2.1 개요

DockArea는 gpui-component의 **핵심 레이아웃 위젯**으로, IDE 스타일의 복잡한 패널 배치를 지원한다. Zed 에디터의 패널 시스템에서 영감을 받아 설계되었으며, 트리 구조 기반의 레이아웃 관리를 제공한다.

**주요 기능:**
- Center 영역 + Left/Right/Bottom 도크
- Split(수평/수직) + Tabs + Panel + Tiles 레이아웃
- 드래그 앤 드롭 탭 재배치
- 패널 줌 (전체 화면 확대)
- 레이아웃 잠금 (실수 방지)
- 직렬화/역직렬화 (상태 저장 및 복원)

### 2.2 핵심 타입 계층

```
DockArea
├── center: DockItem (트리 구조)
├── left_dock: Option<Dock>
├── right_dock: Option<Dock>
└── bottom_dock: Option<Dock>

DockItem (enum, 트리 노드)
├── Split { axis, items, sizes, stack_panel }
├── Tabs { items, active_index, tab_panel }
├── Panel { panel_view }
└── Tiles { items, tiles_view }
```

### 2.3 Panel 트레이트

커스텀 패널을 만들려면 `Panel` 트레이트를 구현해야 한다. 이 트레이트는 `EventEmitter<PanelEvent>`, `Render`, `Focusable`을 확장한다.

```rust
use gpui_component::dock::{Panel, PanelEvent, PanelView};
use gpui::*;

struct CruxTerminalPanel {
    title: SharedString,
    focus_handle: FocusHandle,
    // ... terminal state
}

impl Panel for CruxTerminalPanel {
    // [필수] 직렬화/역직렬화에 사용되는 식별자 (변경 불가)
    fn panel_name(&self) -> &'static str {
        "CruxTerminalPanel"
    }

    // [선택] 탭에 표시되는 이름
    fn title(&self, _cx: &App) -> AnyElement {
        self.title.clone().into_any_element()
    }

    // [선택] 패널 닫기 가능 여부 (기본: true)
    fn closable(&self, _cx: &App) -> bool {
        true
    }

    // [선택] 줌(전체화면) 가능 여부
    fn zoomable(&self, _cx: &App) -> bool {
        true
    }

    // [선택] 직렬화 (레이아웃 저장)
    fn dump(&self, _cx: &App) -> Option<PanelState> {
        // 패널 상태를 JSON으로 직렬화
        None
    }
}

impl EventEmitter<PanelEvent> for CruxTerminalPanel {}

impl Focusable for CruxTerminalPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CruxTerminalPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .size_full()
            .child("Terminal content here")
    }
}
```

**Panel 트레이트의 선택적 메서드:**

| 메서드 | 설명 | 기본값 |
|--------|------|--------|
| `panel_name()` | 직렬화 식별자 (필수) | — |
| `title()` | 탭 헤더 렌더링 | — |
| `title_style()` | 타이틀 커스텀 스타일 | 기본 스타일 |
| `title_suffix()` | 타이틀 뒤 추가 콘텐츠 | 없음 |
| `closable()` | 닫기 가능 여부 | `true` |
| `zoomable()` | 줌 가능 여부 | `false` |
| `visible()` | 표시 여부 | `true` |
| `toolbar_buttons()` | 타이틀 바 버튼 | 없음 |
| `dropdown_menu()` | 컨텍스트 메뉴 | 없음 |
| `set_active()` | 활성/비활성 전환 콜백 | — |
| `set_zoomed()` | 줌 상태 전환 콜백 | — |
| `on_added_to()` | TabPanel에 추가될 때 콜백 | — |
| `on_removed()` | 부모에서 제거될 때 콜백 | — |
| `dump()` | 상태 직렬화 | `None` |
| `inner_padding()` | 탭 레이아웃 내 패딩 | `true` |

### 2.4 DockArea 생성 및 레이아웃 구성

```rust
use gpui_component::dock::*;
use gpui::*;
use std::sync::Arc;

// DockArea 생성
let dock_area = cx.new(|cx| {
    let mut area = DockArea::new("main-dock", Some(1), window, cx);
    let weak_area = cx.entity().downgrade();

    // 터미널 패널 생성
    let term1 = cx.new(|cx| CruxTerminalPanel::new("zsh", window, cx));
    let term2 = cx.new(|cx| CruxTerminalPanel::new("bash", window, cx));
    let term3 = cx.new(|cx| CruxTerminalPanel::new("python", window, cx));

    // 레이아웃 구성: 수평 분할 + 탭
    area.set_center(
        DockItem::h_split(
            vec![
                // 왼쪽: 탭 2개
                DockItem::tabs(
                    vec![Arc::new(term1), Arc::new(term2)],
                    &weak_area, window, cx,
                ),
                // 오른쪽: 단일 패널
                DockItem::tab(
                    term3.clone(), &weak_area, window, cx,
                ).size(px(400.)),
            ],
            &weak_area, window, cx,
        ),
        window, cx,
    );

    area
});
```

### 2.5 DockItem 레이아웃 패턴

DockItem은 트리 구조로, 4가지 variant를 조합하여 복잡한 레이아웃을 구성한다:

```rust
// 1. 수평 분할 (좌우)
DockItem::h_split(items, &weak_area, window, cx)

// 2. 수직 분할 (상하)
DockItem::v_split(items, &weak_area, window, cx)

// 3. 크기 지정 분할
DockItem::split_with_sizes(axis, items, sizes, &weak_area, window, cx)

// 4. 탭 (여러 패널을 탭으로 그룹화)
DockItem::tabs(panels, &weak_area, window, cx)

// 5. 단일 패널
DockItem::panel(panel_view)

// 6. 타일 (자유 배치, 드래그/리사이즈)
DockItem::tiles(tile_items, &weak_area, window, cx)

// 크기 제약 체이닝
DockItem::tab(panel, &weak_area, window, cx).size(px(250.))
```

**IDE 스타일 레이아웃 예시:**

```rust
// Zed IDE와 유사한 레이아웃
//  ┌──────────┬──────────────────┬──────────┐
//  │ Explorer │  Editor (tabs)   │ Outline  │
//  │ (250px)  │                  │ (300px)  │
//  │          ├──────────────────┤          │
//  │          │  Terminal (200px)│          │
//  └──────────┴──────────────────┴──────────┘

area.set_center(
    DockItem::h_split(
        vec![
            DockItem::tab(explorer, &w, window, cx).size(px(250.)),
            DockItem::v_split(
                vec![
                    DockItem::tabs(vec![Arc::new(editor1), Arc::new(editor2)], &w, window, cx),
                    DockItem::tab(terminal, &w, window, cx).size(px(200.)),
                ],
                &w, window, cx,
            ),
        ],
        &w, window, cx,
    ),
    window, cx,
);

// 오른쪽 도크 설정
area.set_right_dock(
    DockItem::tab(outline, &w, window, cx),
    Some(px(300.)),  // 초기 너비
    true,            // 기본 열림 상태
    window, cx,
);
```

### 2.6 프로그래매틱 패널 조작 (IPC 연동)

DockArea는 프로그래매틱 조작 API를 제공하여, IPC를 통한 외부 제어에 적합하다:

```rust
// 패널 추가
dock_area.update(cx, |area, cx| {
    let new_panel = cx.new(|cx| CruxTerminalPanel::new("new-shell", window, cx));
    area.add_panel(Arc::new(new_panel), DockPlacement::Center, None, window, cx);
});

// 도크 토글 (열기/닫기)
dock_area.update(cx, |area, cx| {
    area.toggle_dock(DockPlacement::Right, window, cx);
});

// 도크 상태 확인
let is_open = dock_area.read(cx).is_dock_open(DockPlacement::Left, cx);

// 레이아웃 잠금 (분할/이동 방지, 리사이즈만 허용)
dock_area.update(cx, |area, cx| {
    area.set_locked(true, window, cx);
});
```

### 2.7 레이아웃 직렬화/역직렬화

DockArea의 레이아웃 상태를 JSON으로 저장하고 복원할 수 있다. 이는 세션 복원과 IPC 상태 공유에 필수적이다:

```rust
// 저장
let state: DockAreaState = dock_area.read(cx).dump(cx);
let json = serde_json::to_string(&state).unwrap();
std::fs::write("~/.config/crux/layout.json", &json).unwrap();

// 복원
let json = std::fs::read_to_string("~/.config/crux/layout.json").unwrap();
let state: DockAreaState = serde_json::from_str(&json).unwrap();
dock_area.update(cx, |area, cx| {
    area.load(state, window, cx).ok();
});
```

> **주의**: `PanelRegistry`에 패널 타입을 등록해야 역직렬화가 동작한다. `register_panel()` 함수로 패널 이름과 생성자를 전역 레지스트리에 등록한다.

### 2.8 DockArea 이벤트

```rust
enum DockEvent {
    // 레이아웃 변경 시 발생
    // 패널 추가/제거, 탭 이동, 분할 변경 등
}
```

DockArea는 `EventEmitter<DockEvent>`를 구현하므로, 레이아웃 변경을 구독하여 자동 저장 등의 반응을 구현할 수 있다.

---

## 3. Tab / TabBar 위젯

### 3.1 개요

gpui-component의 Tab/TabBar는 **독립 위젯**과 **DockArea 내장 TabPanel** 두 가지 형태로 사용할 수 있다.

- **독립 TabBar**: 간단한 탭 인터페이스에 적합 (커스텀 레이아웃)
- **DockArea TabPanel**: 드래그 앤 드롭, 패널 직렬화 등 고급 기능 포함

### 3.2 기본 TabBar 사용

```rust
use gpui_component::tab::{Tab, TabBar};

TabBar::new("terminal-tabs")
    .selected_index(0)
    .on_click(|selected_index, _, _| {
        println!("Tab {} selected", selected_index);
    })
    .child(Tab::new().label("zsh"))
    .child(Tab::new().label("bash"))
    .child(Tab::new().label("python"))
```

### 3.3 상태 관리가 포함된 탭

```rust
struct TerminalTabsView {
    active_tab: usize,
    terminals: Vec<Entity<CruxTerminalPanel>>,
}

impl Render for TerminalTabsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .child(
                TabBar::new("term-tabs")
                    .selected_index(self.active_tab)
                    .on_click(cx.listener(|view, index, _, cx| {
                        view.active_tab = *index;
                        cx.notify();
                    }))
                    .children(
                        self.terminals.iter().map(|term| {
                            Tab::new().label(term.read(cx).title.clone())
                        })
                    )
            )
            .child(
                // 활성 탭의 터미널 뷰 렌더링
                div()
                    .flex_1()
                    .child(self.terminals[self.active_tab].clone())
            )
    }
}
```

### 3.4 닫기 버튼이 있는 탭

```rust
struct CloseableTabsView {
    tabs: Vec<String>,
    active_tab: usize,
}

impl CloseableTabsView {
    fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 {
            self.tabs.remove(index);
            if self.active_tab >= index && self.active_tab > 0 {
                self.active_tab -= 1;
            }
            cx.notify();
        }
    }
}

impl Render for CloseableTabsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        TabBar::new("closeable-tabs")
            .selected_index(self.active_tab)
            .on_click(cx.listener(|view, index, _, cx| {
                view.active_tab = *index;
                cx.notify();
            }))
            .children(
                self.tabs.iter().enumerate().map(|(index, tab_name)| {
                    Tab::new().label(tab_name.clone())
                        .suffix(
                            Button::new(format!("close-{}", index))
                                .icon(IconName::X)
                                .ghost()
                                .xsmall()
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.close_tab(index, cx);
                                }))
                        )
                })
            )
    }
}
```

### 3.5 TabBar 주요 동작

| 기능 | 설명 |
|------|------|
| `selected_index()` | 활성 탭 인덱스 설정 |
| `on_click()` | 탭 클릭 핸들러 (TabBar에 설정하면 개별 Tab의 on_click은 무시됨) |
| `with_menu()` | 드롭다운 메뉴 추가 (탭 수가 많을 때 유용) |
| 자동 스크롤 | 탭이 컨테이너 너비를 초과하면 자동으로 스크롤 활성화 |
| `TabVariant` | 탭 스타일 변형 (기본, 필 등) |

> **참고**: 고급 닫기 기능 (탭 드래그 앤 드롭, 패널 간 이동 등)이 필요하면 독립 TabBar 대신 **DockArea의 TabPanel 시스템**을 사용하는 것이 권장된다.

### 3.6 Crux에서의 키보드 단축키 구현 전략

TabBar 자체에는 키보드 단축키가 내장되어 있지 않으므로, GPUI의 액션 시스템을 활용해야 한다:

```rust
// 액션 정의
actions!(terminal_tabs, [NewTab, CloseTab, NextTab, PrevTab,
    SelectTab1, SelectTab2, SelectTab3, /* ... SelectTab9 */]);

// 키 바인딩 등록
cx.bind_keys([
    KeyBinding::new("cmd-t", NewTab, None),
    KeyBinding::new("cmd-w", CloseTab, None),
    KeyBinding::new("ctrl-tab", NextTab, None),
    KeyBinding::new("ctrl-shift-tab", PrevTab, None),
    KeyBinding::new("cmd-1", SelectTab1, None),
    KeyBinding::new("cmd-2", SelectTab2, None),
    // ... cmd-9
]);
```

---

## 4. ResizablePanel 위젯

### 4.1 개요

ResizablePanel은 **수평/수직 분할 레이아웃**을 구현하며, 드래그 핸들을 통한 패널 크기 조정을 지원한다.

**핵심 타입:**
- `ResizablePanelGroup`: 리사이즈 가능한 패널들의 컨테이너
- `ResizablePanel`: 개별 리사이즈 가능 패널
- `ResizableState`: 패널 상태 (크기 정보 등)
- `ResizablePanelEvent`: 리사이즈 이벤트

### 4.2 기본 사용법

```rust
use gpui_component::resizable::*;

// 수평 분할 (좌우)
h_resizable("h-split", window, cx)
    .child(
        resizable_panel()
            .size(px(300.))
            .child("Left Panel")
    )
    .child(
        resizable_panel()
            .child("Right Panel")
    )

// 수직 분할 (상하)
v_resizable("v-split", window, cx)
    .child(
        resizable_panel()
            .size(px(200.))
            .child("Top Panel")
    )
    .child(
        resizable_panel()
            .child("Bottom Panel")
    )
```

### 4.3 크기 제약 조건

```rust
// 최소/최대 크기 설정
resizable_panel()
    .size(px(200.))                    // 초기 크기
    .size_range(px(150.)..px(400.))    // 최소 150px, 최대 400px
    .child("Constrained Panel")

// 최소 크기만 설정
resizable_panel()
    .size_range(px(100.)..Pixels::MAX)
    .child("Flexible Panel")

// 고정 크기 (리사이즈 불가)
resizable_panel()
    .size(px(300.))
    .size_range(px(300.)..px(300.))
    .child("Fixed Panel")
```

### 4.4 중첩 분할

터미널 에뮬레이터에서 필수적인 **분할 안의 분할** 패턴:

```rust
// 복잡한 중첩 레이아웃
// ┌──────────┬──────────┐
// │          │ Top Right│
// │  Left    ├──────────┤
// │          │ Bot Right│
// ├──────────┴──────────┤
// │     Bottom          │
// └─────────────────────┘
v_resizable("main-layout", window, cx)
    .child(
        resizable_panel()
            .size(px(400.))
            .child(
                h_resizable("top-split", window, cx)
                    .child(
                        resizable_panel()
                            .size(px(300.))
                            .child("Left")
                    )
                    .child(
                        resizable_panel()
                            .child(
                                v_resizable("right-split", window, cx)
                                    .child(
                                        resizable_panel()
                                            .size(px(200.))
                                            .child("Top Right")
                                    )
                                    .child(
                                        resizable_panel()
                                            .child("Bottom Right")
                                    )
                            )
                    )
            )
    )
    .child(
        resizable_panel()
            .child("Bottom Panel")
    )
```

### 4.5 리사이즈 이벤트 구독

```rust
struct SplitTerminalView {
    resize_state: Entity<ResizableState>,
}

impl SplitTerminalView {
    fn new(cx: &mut Context<Self>) -> Self {
        let resize_state = ResizableState::new(cx);

        // 리사이즈 이벤트 구독
        cx.subscribe(&resize_state, |this, _, event: &ResizablePanelEvent, cx| {
            match event {
                ResizablePanelEvent::Resized => {
                    let sizes = this.resize_state.read(cx).sizes();
                    // IPC로 레이아웃 변경 알림 전송
                    println!("Panel sizes changed: {:?}", sizes);
                }
            }
        });

        Self { resize_state }
    }
}

impl Render for SplitTerminalView {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        h_resizable("terminals", self.resize_state.clone())
            .child(
                resizable_panel()
                    .size(px(400.))
                    .size_range(px(200.)..Pixels::MAX)
                    .child("Terminal 1")
            )
            .child(
                resizable_panel()
                    .size_range(px(200.)..Pixels::MAX)
                    .child("Terminal 2")
            )
    }
}
```

### 4.6 DockArea vs 독립 ResizablePanel

| 기준 | DockArea (StackPanel) | 독립 ResizablePanel |
|------|----------------------|-------------------|
| 드래그 앤 드롭 | O (탭 간 이동) | X |
| 직렬화 | O (자동) | 수동 구현 필요 |
| 탭 통합 | O (TabPanel 내장) | X (별도 구현) |
| 줌 기능 | O (패널 줌) | X |
| 복잡도 | 높음 | 낮음 |
| 사용 사례 | IDE 스타일 전체 레이아웃 | 단순 분할 뷰 |

> **Crux 권장**: 메인 레이아웃에는 DockArea를 사용하고, DockArea 내부에서 분할이 필요한 경우에는 DockItem::Split (내부적으로 StackPanel/ResizablePanel 활용)을 사용한다.

---

## 5. 위젯 조합 아키텍처

### 5.1 전체 계층 구조

Crux 터미널 에뮬레이터의 위젯 조합 아키텍처:

```
Window
└── Root (gpui_component::Root)
    └── CruxApp (메인 뷰)
        ├── TitleBar (커스텀 타이틀 바)
        └── DockArea ("main-dock")
            ├── center: DockItem
            │   └── Split (h_split 또는 v_split)
            │       ├── Tabs [term1, term2, term3]
            │       │   └── TabPanel
            │       │       ├── Tab → CruxTerminalPanel
            │       │       ├── Tab → CruxTerminalPanel
            │       │       └── Tab → CruxTerminalPanel
            │       └── Split (v_split)
            │           ├── Tabs [term4]
            │           └── Tabs [term5]
            ├── left_dock: None (또는 파일 탐색기)
            ├── right_dock: None
            └── bottom_dock: None
```

### 5.2 포커스 관리

GPUI의 `FocusHandle`은 위젯 계층을 통해 전파된다. 터미널 에뮬레이터에서 정확한 키보드 입력 라우팅이 핵심이다.

```rust
struct CruxTerminalPanel {
    focus_handle: FocusHandle,
    terminal: Entity<CruxTerminal>,  // alacritty_terminal 래퍼
}

impl Focusable for CruxTerminalPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CruxTerminalPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)  // 포커스 추적
            .on_focus(cx.listener(|this, _, window, cx| {
                // 포커스 획득 시: 커서 깜빡임 시작, IME 활성화
            }))
            .on_blur(cx.listener(|this, _, window, cx| {
                // 포커스 상실 시: 커서 정지, IME 비활성화
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                // 키보드 이벤트를 PTY에 전달
                this.terminal.update(cx, |term, cx| {
                    term.handle_key_event(event, window, cx);
                });
            }))
            .size_full()
            .child(CruxTerminalView::new(self.terminal.clone()))
    }
}
```

**포커스 전파 흐름:**

```
Window → DockArea → TabPanel → CruxTerminalPanel → FocusHandle
                                    ↓
                               on_key_down → PTY write
```

DockArea의 TabPanel은 활성 탭의 패널로 포커스를 자동 전달한다. `set_active()` 콜백에서 포커스 전환 로직을 구현할 수 있다.

### 5.3 이벤트 라우팅

키보드 이벤트가 올바른 터미널 패인에 도달하는 경로:

1. **Window**: 키 이벤트 수신
2. **GPUI**: 포커스가 있는 뷰로 이벤트 전파
3. **DockArea**: TabPanel을 통해 활성 패널로 전달
4. **CruxTerminalPanel**: `on_key_down`에서 이벤트 처리
5. **CruxTerminal**: alacritty_terminal에 키 입력 전달
6. **PTY**: 실제 쉘에 바이트 쓰기

### 5.4 활성 패인 추적

```rust
struct CruxApp {
    dock_area: Entity<DockArea>,
    active_terminal: Option<Entity<CruxTerminalPanel>>,
}

impl CruxApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dock_area = cx.new(|cx| {
            let area = DockArea::new("main", Some(1), window, cx);
            // ... 레이아웃 설정
            area
        });

        // DockArea 이벤트 구독하여 활성 패널 추적
        cx.subscribe(&dock_area, |this, _, event: &DockEvent, cx| {
            // 활성 패널 변경 감지 및 업데이트
        });

        Self {
            dock_area,
            active_terminal: None,
        }
    }
}
```

---

## 6. Markdown 렌더링

### 6.1 현재 상황

gpui-component v0.5.1의 공식 컴포넌트 목록에는 **독립 Markdown 위젯이 포함되어 있지 않다**. 그러나 다음과 같은 관련 기능이 존재한다:

- **Editor 컴포넌트**: Tree-sitter 기반 구문 강조 지원 (200K줄까지 고성능)
- **Highlighter 모듈**: 코드 구문 강조 엔진
- 라이브러리 README에서 "Markdown and simple HTML content rendering" 언급

### 6.2 대안 전략

Crux의 Markdown 렌더링 (PLAN.md Phase 4)에는 다음 접근법을 고려한다:

**방법 1: gpui-component Editor 활용**
```rust
// 읽기 전용 코드 블록 렌더링에 Editor 활용
let state = cx.new(|cx|
    InputState::new(window, cx)
        .code_editor("markdown")  // markdown 구문 강조
        .line_number(false)
);
```

**방법 2: 커스텀 Markdown Element 구현**
GPUI의 Element 트레이트를 직접 구현하여 Markdown AST를 GPU 가속 렌더링으로 변환한다. `pulldown-cmark`로 파싱 후 GPUI 엘리먼트 트리로 변환하는 방식이다.

**방법 3: Zed의 Markdown 렌더러 참조**
Zed 에디터 소스에 포함된 Markdown 렌더링 로직을 참조하여 커스텀 구현한다.

> **Crux Phase 4 권장**: 방법 2를 기본으로 하되, 코드 블록에는 gpui-component의 Highlighter를 활용하여 Tree-sitter 기반 구문 강조를 적용한다.

---

## 7. Ghostty 비교 분석

### 7.1 Ghostty의 탭/분할 구현

Ghostty는 macOS에서 **네이티브 AppKit 컴포넌트**를 사용하며, GPUI와는 근본적으로 다른 접근법을 취한다.

| 항목 | Ghostty (AppKit) | Crux (GPUI/gpui-component) |
|------|-----------------|---------------------------|
| 언어 | Swift + C (libghostty) | Rust |
| 탭 구현 | NSWindow 탭 그룹 (네이티브) | DockArea TabPanel (커스텀) |
| 분할 패인 | SplitTree + NSView (커스텀) | DockItem::Split (커스텀) |
| 드래그 앤 드롭 | 제한적 (네이티브 탭 제약) | DockArea 내장 지원 |
| 렌더링 | Metal (자체 구현) | Metal (GPUI 추상화) |
| 키보드 단축키 | NSMenuItem + responder chain | GPUI action system |
| 상태 복원 | NSWindow restoration API | DockAreaState JSON 직렬화 |

### 7.2 Ghostty의 컨트롤러 계층

```
AppDelegate
└── TerminalController (NSWindowController)
    └── BaseTerminalController
        ├── SplitTree<SurfaceView>  (분할 패인 트리)
        ├── focusedSurface           (현재 포커스된 터미널)
        └── NSWindow tab group       (네이티브 탭)
```

**핵심 설계 결정:**
- **네이티브 탭**: `NSWindow.tabbedWindows`를 활용하여 macOS 표준 탭 UX 제공
- **커스텀 분할**: `SplitTree` 데이터 구조로 재귀적 분할 관리
- **탭 라벨링**: `relabelTabs()`로 Cmd+1~9 단축키 자동 할당
- **탭 순서 감지**: `NSView.frameDidChangeNotification`으로 수동 탭 재배치 감지 (macOS가 직접 알림을 제공하지 않으므로)
- **Undo 시스템**: `ExpiringUndoManager`로 탭/패인 닫기 취소 지원

### 7.3 장단점 비교

**Ghostty (네이티브) 장점:**
- macOS 표준 UX와 완벽 통합 (시스템 설정의 탭 동작 존중)
- 메모리/CPU 오버헤드 최소 (OS가 탭 관리)
- 접근성(VoiceOver) 자동 지원
- 네이티브 탭 병합/분리 제스처

**Ghostty (네이티브) 단점:**
- macOS 전용 (크로스 플랫폼 불가)
- 탭 커스터마이징 제한 (네이티브 UI 제약)
- 탭 재배치 감지를 위한 해킹 필요
- 탭 간 드래그 앤 드롭 제한

**Crux (GPUI/gpui-component) 장점:**
- 완전한 UI 커스터마이징 자유
- DockArea의 풍부한 레이아웃 기능 (직렬화, 드래그 앤 드롭, 줌, 잠금)
- IPC를 통한 프로그래매틱 제어에 최적화
- 타일 레이아웃 등 고급 배치 지원

**Crux (GPUI/gpui-component) 단점:**
- macOS 네이티브 탭 UX와 다름 (학습 곡선)
- 접근성 직접 구현 필요
- 위젯 라이브러리 의존 (pre-1.0 불안정성)

### 7.4 Ghostty의 교훈

1. **네이티브 vs 커스텀은 트레이드오프**: Ghostty는 네이티브를 선택해 macOS 통합에서 이점을 얻었지만, 크로스 플랫폼과 커스터마이징에서 대가를 치렀다.
2. **Undo/Redo는 사용자 경험에 중요**: 탭/패인 닫기 취소 기능은 사용자 만족도에 큰 영향을 미친다. DockArea의 직렬화를 활용하면 구현 가능하다.
3. **상태 복원은 필수**: Ghostty의 `window-save-state` 기능처럼, Crux도 DockAreaState 직렬화로 세션 복원을 구현해야 한다.
4. **탭 번호는 Cmd+1~9로**: 표준이 된 터미널 탭 단축키이다.

---

## 8. Crux 구현 전략

### 8.1 Phase 2 구현 로드맵

PLAN.md Phase 2에서 구현할 탭/분할 패인 UI의 권장 아키텍처:

```
crates/crux-app/src/
├── app.rs           # CruxApp — DockArea 초기화 및 관리
├── dock/
│   ├── mod.rs       # dock 모듈
│   ├── terminal_panel.rs  # CruxTerminalPanel (Panel 트레이트 구현)
│   └── panel_registry.rs  # 패널 등록 및 직렬화
├── actions.rs       # 키보드 단축키 액션 정의
└── keybindings.rs   # 키 바인딩 등록
```

### 8.2 구현 순서

1. **CruxTerminalPanel**: Panel 트레이트 구현 (기존 CruxTerminalView 래핑)
2. **DockArea 초기화**: 기본 레이아웃으로 단일 탭 설정
3. **탭 관리**: 새 탭 (Cmd+T), 탭 닫기 (Cmd+W), 탭 전환 (Cmd+1~9)
4. **분할 패인**: 수평 분할 (Cmd+D), 수직 분할 (Cmd+Shift+D)
5. **포커스 관리**: 패인 간 포커스 이동 (Cmd+Option+Arrow)
6. **레이아웃 직렬화**: 세션 저장/복원
7. **IPC 연동**: 프로그래매틱 패인 제어 API

### 8.3 IPC 통합 설계

DockArea의 프로그래매틱 API를 IPC JSON-RPC 2.0과 매핑:

| IPC 메서드 | DockArea API |
|-----------|-------------|
| `crux:pane/split` | `set_center(DockItem::h_split(...))` |
| `crux:pane/close` | `remove_panel(...)` |
| `crux:tab/new` | `add_panel(...)` |
| `crux:tab/focus` | TabPanel active_index 변경 |
| `crux:dock/toggle` | `toggle_dock(placement)` |
| `crux:layout/save` | `dump(cx)` → JSON |
| `crux:layout/restore` | `load(state, ...)` |
| `crux:layout/lock` | `set_locked(true, ...)` |

### 8.4 핵심 고려사항

1. **DockArea vs 독립 위젯**: 메인 레이아웃에 DockArea를 사용하여 드래그 앤 드롭, 직렬화, 줌 기능을 무료로 얻는다.
2. **Panel 트레이트**: CruxTerminalPanel이 Panel 트레이트를 구현하여 DockArea에 자연스럽게 통합한다.
3. **PanelRegistry**: 패널 타입을 전역 등록하여 역직렬화 시 올바른 패널 인스턴스를 생성한다.
4. **포커스 핸들**: 각 터미널 패널이 독립적인 FocusHandle을 가지며, DockArea가 활성 패널로 포커스를 관리한다.
5. **IME 통합**: 포커스된 패널에서만 IME를 활성화하고, preedit 텍스트는 해당 패널의 오버레이로 렌더링한다 (framework.md 참조).
6. **성능**: DockArea의 레이아웃 변경은 전체 리렌더링을 유발할 수 있으므로, 터미널 셀 렌더링은 Element 레벨에서 damage tracking을 유지해야 한다.

---

## 9. 참고 자료

### gpui-component
- [GitHub 저장소](https://github.com/longbridge/gpui-component)
- [공식 문서](https://longbridge.github.io/gpui-component/)
- [docs.rs API 문서](https://docs.rs/gpui-component/latest/gpui_component/)
- [crates.io](https://crates.io/crates/gpui-component)
- [DeepWiki 분석](https://deepwiki.com/longbridge/gpui-component)

### GPUI
- [GPUI 공식 사이트](https://www.gpui.rs/)
- [GPUI crates.io](https://crates.io/crates/gpui)

### Ghostty
- [Ghostty 공식 사이트](https://ghostty.org/)
- [Ghostty macOS 윈도우/탭 관리 DeepWiki](https://deepwiki.com/ghostty-org/ghostty/6.3-macos-window-and-tab-management)
- [Ghostty Features](https://ghostty.org/docs/features)
- [Ghostty 키바인드 레퍼런스](https://ghostty.org/docs/config/keybind/reference)

### 내부 문서
- [GPUI 프레임워크 연구](./framework.md) — GPUI 코어 분석
- [터미널 구현체 연구](./terminal-implementations.md) — Zed 터미널 분석
- [부트스트랩 연구](./bootstrap.md) — GPUI 앱 초기화 패턴
- [IPC 프로토콜 설계](../integration/ipc-protocol-design.md) — JSON-RPC 2.0 프로토콜
