---
title: Terminal Window Resize, Text Reflow, and Cursor Tracking
description: Deep research into terminal resize/reflow implementations across major terminal emulators, the #1 bug pattern for new terminals
phase: 2
topics: [terminal-core, reflow, resize, cursor-tracking, vim-bug]
related: [terminal-emulation.md, terminal-architecture.md]
---

# Terminal Window Resize, Text Reflow, and Cursor Tracking

> **Why this matters:** Text reflow on resize is the #1 reported bug pattern for new terminal emulators. ALL major terminals have bugs in this area. The vim → resize → vim exit → broken shell bug is a combination of three complex interactions: alternate screen switching, saved cursor position (DECSC/DECRC), and text reflow.

## Executive Summary

Terminal window resizing with text reflow is one of the most complex and bug-prone areas in terminal emulation. The core problem: **there is no specification** for how saved cursor positions should behave when text reflows during a resize. Every terminal handles it differently, and many handle it incorrectly.

**The Classic Bug Pattern:**
1. User runs vim (enters alternate screen)
2. User resizes the terminal window
3. User exits vim (returns to normal screen)
4. Shell prompt and previous output are corrupted or missing

**Root Causes:**
- Saved cursor position (DECSC) not reflowed with text
- Alternate screen and normal screen tracked separately
- Pending wrap state not reset during reflow
- Wide characters (CJK) splitting at the wrong boundary

---

## 1. The Core Problem

### 1.1 What Happens During Reflow?

When a terminal window is resized:

1. **PTY notification**: The terminal calls `ioctl(TIOCSWINSZ)` to update the PTY size
2. **Signal sent**: The PTY sends `SIGWINCH` to the foreground process group
3. **Grid restructuring**: The terminal must restructure its internal grid:
   - **Shrinking**: Long lines must be split (soft-wrapped) across multiple rows
   - **Growing**: Previously soft-wrapped lines must be rejoined
4. **Cursor repositioning**: All cursors (current + saved) must be adjusted to their new positions
5. **Viewport adjustment**: The scrollback region may need to expand/contract

### 1.2 The DECSC/DECRC Problem

**DECSC (ESC 7)** saves cursor state:
- Cursor row and column (absolute screen coordinates)
- Character sets (G0, G1, G2, G3 designations)
- **Pending wrap state** (crucial for reflow!)
- SGR attributes (bold, color, etc.)
- Origin mode (DECOM - absolute vs scroll region relative)

**DECRC (ESC 8)** restores this state.

**The unspecified behavior:** VT specifications define what DECSC/DECRC save/restore, but they say **nothing** about what happens when the terminal is resized between save and restore.

### 1.3 The Vim Bug Explained

```
1. Shell running on normal screen
   - Cursor at column 42 of a soft-wrapped line
   - Shell saves cursor: ESC 7

2. Vim starts
   - Switches to alternate screen (ESC [ ? 1049 h)
   - Draws its UI
   - Normal screen frozen in background

3. User resizes terminal (narrow → wide)
   - Alternate screen reflows (vim's UI adapts)
   - Normal screen *should* reflow too
   - But: Does the saved cursor position reflow?

4. Vim exits
   - Switches back to normal screen (ESC [ ? 1049 l)
   - Shell restores cursor: ESC 8
   - **BUG**: Cursor restored to OLD grid coordinates
   - Shell prompt overwrites previous output at wrong position
```

**Key insight:** The alternate screen and normal screen have **separate cursor state**, but the normal screen needs to reflow even while the alternate screen is active.

---

## 2. How Each Terminal Handles It

### 2.1 Kitty (#8325)

**Issue:** [Cursor position incorrect after DECSC, resize with reflow, followed by DECRC](https://github.com/kovidgoyal/kitty/issues/8325)

**Original behavior:** Off-by-one error when restoring cursor after reflow on soft-wrapped lines.

**Resolution:** Fixed during multicell character rewrite. The reflow code was completely rewritten to handle wide characters correctly, which inadvertently fixed this bug.

**Current status:** ✅ Reflows saved cursor correctly (as of 2024)

**Implementation notes:**
- Kitty only special-cases the prompt the cursor is at for reflow, not ones in scrollback
- Shell integration helps: shell can tell Kitty to redraw prompt on resize
- Source: [kovidgoyal/kitty](https://github.com/kovidgoyal/kitty)

---

### 2.2 Ghostty (#5718)

**Issue:** [Terminal resize with reflow doesn't reflow the saved cursor (ESC 7)](https://github.com/ghostty-org/ghostty/issues/5718)

**Problem:** Ghostty correctly reflows the primary cursor but NOT the saved cursor.

**Cross-terminal comparison from the issue:**

| Terminal | Reflows Saved Cursor? | Notes |
|----------|----------------------|-------|
| Ghostty 1.1.0 | ❌ No | Fixed in PR #5720 |
| Gnome Console (libvte) | ✅ Yes | Reference implementation |
| foot | ✅ Yes | Correct behavior |
| Kitty | ⚠️ Off-by-one | Fixed later |
| iTerm2 | ❌ No | |
| WezTerm | ❌ No | Issue #6669 filed |
| Terminal.app | ❌ Nonsensical | Completely wrong |
| Alacritty | ❌ Nonsensical | Completely wrong |

**Resolution:** Fixed in PR #5720 (February 2025)

**Mitchell Hashimoto's approach:** Shell integration where the shell can communicate to terminal that it can redraw the prompt on resize. On resize, Ghostty clears the terminal line, preventing text reflow and resulting in "perfect reflow on resize."

---

### 2.3 Windows Terminal (#4200)

**Issue:** [Scenario: ResizeWithReflow and related issues](https://github.com/microsoft/terminal/issues/4200)

**Implemented in:** [PR #4741 - Add support for "reflow"ing the Terminal buffer](https://github.com/microsoft/terminal/pull/4741)

**Technical challenges:**

1. **Scrollback reflow**: Text in scrollback wasn't rewrapped when terminal resized
2. **Line wrapping artifacts**: Missing characters when combining linewrap with backspace
3. **ConPTY buffer handling**: PTY layer needed to emit wrapped lines without spurious carriage returns
4. **Cursor positioning bugs**: Cursor misalignment in Emacs, curses apps, WSL2 git rebase

**Remaining issues post-implementation:**
- Reflow with long input occasionally loses cursor visibility
- Alt+Enter rendering errors
- Alternate screen behavior during resize (deferred to v1.x)

**Related issues:**
- [#7466 - Text Doesn't Reflow Properly on Window Resize after Vim Opened and Closed](https://github.com/microsoft/terminal/issues/7466)
- [#14291 - Add an option to disable reflow on window resize](https://github.com/microsoft/terminal/issues/14291)

---

### 2.4 WezTerm (#6669)

**Issue:** [Cursor position incorrect after DECSC, resize with reflow, then DECRC](https://github.com/wezterm/wezterm/issues/6669)

**Status:** Filed February 2025, references Ghostty issue #5718

**Suggestion:** Should reset pending wrap state

**Related issues:**
- [#234 - Incorrect screen wrapping/reflow upon resizing terminal window](https://github.com/wezterm/wezterm/issues/234)
- [#2987 - Bad prompt on resize behavior](https://github.com/wezterm/wezterm/issues/2987)

---

### 2.5 tmux (#4366)

**Issue:** [Cursor position incorrect after DECSC, resize with reflow, then DECRC](https://github.com/tmux/tmux/issues/4366)

**Problem:** tmux has its own virtual terminal implementation and suffers from the same saved cursor reflow issue.

**Complexity:** tmux runs in the alternate screen buffer of the outer terminal, adding an extra layer where reflow can go wrong.

**Suggested behavior:** Follow libvte's principle of least surprise for end users - saved cursors should be reflowed.

**Historical context:** tmux was initially "pretty adamant about not supporting reflow" but eventually added it due to user demand.

**Related issues:**
- [#516 - Pane resizing breaks bash, zsh, csh prompts](https://github.com/tmux/tmux/issues/516)
- [#783 - Line reflow messed up](https://github.com/tmux/tmux/issues/783)
- [#1249 - Very slow reflow with large histories](https://github.com/tmux/tmux/issues/1249)
- [#3064 - OSC 133 (shell integration / semantic prompt) support](https://github.com/tmux/tmux/issues/3064)

---

### 2.6 iTerm2 (#12166)

**Issue:** [Cursor position incorrect after DECSC, resize with reflow, then DECRC](https://gitlab.com/gnachman/iterm2/-/issues/12166)

**Problem:** Two separate bugs:
1. Saved cursors not being reflowed
2. Pending wrap state not being reset

**Important note:** "It's easy to fix one aspect and not the other." The issue provides separate test scripts for each bug.

**Test approach:** Resize terminal to less than half screen width, run test script, resize to max size while paused.

---

### 2.7 xterm

**Behavior:** xterm traditionally **does NOT do reflow** at all.

**Rationale:** Following strict VT100 compatibility. VT100 hardware terminals couldn't reflow text.

**User experience:** When you resize xterm, lines get clipped (shrinking) or padded (growing) but not rewrapped.

**Related projects:**
- [xterm.js #622 - Support reflowing lines on resize](https://github.com/xtermjs/xterm.js/issues/622)
- [xterm.js #2121 - Screen resize deletes characters, no text reflow](https://github.com/xtermjs/xterm.js/issues/2121)

The JavaScript implementation (xterm.js) has been adding reflow support, but native xterm remains without it.

---

### 2.8 Alacritty

**Implementation:** Uses `alacritty_terminal` crate with robust reflow support.

**Source code:** [`alacritty_terminal/src/grid/resize.rs`](https://github.com/alacritty/alacritty/blob/master/alacritty_terminal/src/grid/resize.rs)

**Key data structures:**
- `Grid<Cell>` - 2D grid with rows and columns
- `Row` with `WRAPLINE` flag indicating line continues on next row
- Separate handling for growing vs shrinking columns

**Changelog (v0.5.0):**
- ✅ Fixed linewrap tracking when switching between primary and alternate screen buffer
- ✅ Fixed reflow of cursor during resize

**Issues:**
- [#2302 - Triple Click Should Select Wrapped Lines](https://github.com/alacritty/alacritty/issues/2302)
- [#3584 - Reflow not wrapping cursor correctly](https://github.com/alacritty/alacritty/issues/3584)
- [#4419 - Resize / Reflow Issues](https://github.com/alacritty/alacritty/issues/4419)
- [#2567 - Text reflow slow with large grids](https://github.com/alacritty/alacritty/issues/2567)

**Performance:** Reflow should complete "pretty much immediately" even with 100K lines filled.

**Recent fix:** [PR #7873 - Fix logic for reflowing cursor when growing columns, after shrinking columns](https://github.com/alacritty/alacritty/pull/7873)

**Problem:** Sequences of `shrink_columns()` and `grow_columns()` didn't properly reflow cursor back to original position due to saturation at boundary conditions.

---

### 2.9 Konsole (KDE)

**Feature addition:** Text reflow added in Konsole 21.04 (January 2021)

**Development time:** Over a dozen years from [Bug #196998 - Konsole should reflow the text when resizing](https://bugs.kde.org/show_bug.cgi?id=196998)

**Developers:** Carlos Alves and Tomaz Canabrava

**Merge requests:**
- [!181 - Reflow lines when Terminal resizes](https://invent.kde.org/utilities/konsole/-/merge_requests/181)
- [!321 - Reflow lines when Terminal resizes](https://invent.kde.org/utilities/konsole/-/merge_requests/321)

**Quote from developers:** "Text reflow itself was noted as being the easiest part, with the surrounding implementation details being much more complex."

**Configuration:** Feature enabled by default but can be disabled.

**Announcement:** ["This week in KDE: text reflow in Konsole!"](https://pointieststick.com/2021/01/15/this-week-in-kde-text-reflow-in-konsole/)

---

### 2.10 foot (Wayland)

**Project:** [dnkl/foot](https://codeberg.org/dnkl/foot) by Daniel Eklöf

**Status:** ✅ Correctly reflows saved cursor (confirmed in Ghostty comparison)

**Version:** Text reflow added in foot 1.2.0

**Performance optimization:** [#504 - Text reflow is too slow](https://codeberg.org/dnkl/foot/issues/504)
- Problem: Inefficient remapping of OSC-8 (hyperlink) start/endpoints during reflow
- Solution: Redefined `CELL_MULT_COL_SPACER` to be a base value + remaining spacer count

**Shell integration:** [#939 - Text reflow: estimate prompt position after resize](https://codeberg.org/dnkl/foot/issues/939)

**Implementation detail:** Uses a simple boolean in the row struct for prompt markers, which must be reflowed along with text.

---

### 2.11 libvte (GNOME Terminal, others)

**Status:** ✅ Reference implementation - reflows saved cursor correctly

**Used by:**
- GNOME Terminal
- GNOME Console
- Terminator
- Tilix
- Many others

**Design philosophy:** "Principle of least surprise for end users"

**Historical context:** [Ubuntu Bug #298385 - Reflow terminal contents when resizing the window](https://bugs.launchpad.net/ubuntu/+source/gnome-terminal/+bug/298385)

**No alternate screen disable:** Many libvte-based terminals don't allow users to disable alternate screen behavior.

**Implementation:** Located at [GNOME/vte GitLab](https://gitlab.gnome.org/GNOME/vte)

---

## 3. Soft Wrap vs Hard Wrap Tracking

### 3.1 The Distinction

**Hard wrap (newline):** Application explicitly printed `\n` or `\r\n`
- User pressed Enter
- Application output includes newline
- Should be preserved when copying text

**Soft wrap (terminal-wrapped):** Line exceeded terminal width, terminal automatically wrapped it
- No newline in the actual data
- Should be treated as single logical line when copying
- Should be rejoined when terminal width increases

### 3.2 How Terminals Store This

**Alacritty:** `WRAPLINE` flag in cell metadata
```rust
// Set wrap flag if next line still has cells
row.set_wrap(true);

// Remove wrap flag before appending additional cells
row.set_wrap(false);
```

**foot:** Boolean in row struct

**Windows Terminal:** ConPTY tracks wrapped lines to emit correct sequences

**Critical for:**
- Copy/paste (should newlines be included?)
- Triple-click selection (select whole logical line)
- Reflow (which lines can be rejoined?)

### 3.3 Copy/Paste Behavior

**Expected:** When user selects and copies wrapped text:
- Soft-wrapped lines → copied as single line (no newlines inserted)
- Hard-wrapped lines → copied with newlines preserved

**Common bugs:**
- [Windows Terminal #6901 - When Copying, Line Wrapped Text Is Inconsistently Broken Into Multiple Lines](https://github.com/microsoft/terminal/issues/6901)
- [Alacritty #4993 - Extra new lines inserted on copy (Windows)](https://github.com/alacritty/alacritty/issues/4993)

**Windows behavior:** "Enable line wrapping selection" checkbox in console settings determines if wrapped lines are included in rectangular selection.

---

## 4. Alternate Screen (1049) and Reflow

### 4.1 Alternate Screen Basics

**ESC [ ? 1049 h** - Enter alternate screen:
1. Save cursor position (DECSC)
2. Switch to alternate screen buffer
3. Clear alternate screen

**ESC [ ? 1049 l** - Exit alternate screen:
1. Switch back to normal screen buffer
2. Restore cursor position (DECRC)

**Separate state:**
- Each screen (normal + alternate) has its own grid
- Each screen has its own saved cursor state
- Cursor saved on primary screen is inaccessible from alternate screen

### 4.2 The Reflow Question

**When vim is running (alternate screen active), should the normal screen reflow?**

**Arguments for reflowing normal screen:**
- User expects shell output to reflow when they return
- Prevents the vim exit → broken shell bug
- Matches user mental model

**Arguments against:**
- Performance: reflowing invisible screen is wasted work
- Complexity: hard to test behavior user can't see
- May cause issues if normal screen cursor position becomes invalid

**What terminals do:**
- Most modern terminals reflow both screens
- But many don't reflow the **saved cursor** on the normal screen
- This causes the vim bug

### 4.3 Saved Cursor Across Screen Switch

**The failure mode:**
```
Normal screen: Shell saves cursor at (10, 42) on a soft-wrapped line
↓
Switch to alternate screen (vim starts)
↓
User resizes terminal (narrow → wide)
  - Alternate screen reflows ✓
  - Normal screen reflows ✓
  - Normal screen's saved cursor... maybe? ✗
↓
Switch back to normal screen (vim exits)
↓
Shell restores cursor: gets (10, 42) in OLD coordinate space
  → Wrong position, output corrupted
```

**The fix:** Saved cursor positions must be reflowed even when their screen is not active.

---

## 5. alacritty_terminal's Internal Reflow

### 5.1 Grid Structure

**Source:** [`alacritty_terminal/src/grid/`](https://github.com/alacritty/alacritty/blob/master/alacritty_terminal/src/grid/)

```rust
pub struct Grid<T> {
    raw: Storage<T>,           // Underlying storage
    cols: usize,               // Column count
    lines: usize,              // Visible line count
    display_offset: usize,     // Scrollback offset
    selection: Option<Selection>,
}

pub struct Row<T> {
    inner: Vec<T>,             // Cells in this row
    occupied_cells: usize,     // Non-empty cells
}
```

**Cell flags:**
```rust
pub struct Flags: u16 {
    const WRAPLINE        = 0b0000_0001;  // Line continues on next row
    const WIDE_CHAR       = 0b0000_0010;  // Cell is a wide char
    const WIDE_CHAR_SPACER = 0b0000_0100; // Cell is a spacer for wide char
    const LEADING_WIDE_CHAR_SPACER = 0b0000_1000;
    // ... other flags
}
```

### 5.2 Reflow Algorithm

**Key file:** [`alacritty_terminal/src/grid/resize.rs`](https://github.com/alacritty/alacritty/blob/master/alacritty_terminal/src/grid/resize.rs)

#### Growing Columns

**Algorithm:** Iterate backward through rows, combining lines with `WRAPLINE` flag set.

```
1. Start from bottom row, work upward
2. For each row with WRAPLINE flag:
   a. Pull cells from next row (below)
   b. Remove leading spacers for wide characters
   c. Handle wide-character boundary conditions
   d. Clear WRAPLINE if next row now empty
3. Adjust viewport offset based on rows deleted
4. Adjust cursor position based on cells pulled
```

**Cursor adjustment:**
```rust
// Calculate num_wrapped = cells pulled from next line
let num_wrapped = /* calculation */;

// Resize cursor's line and reflow if necessary
cursor.point.column = cursor.point.column.sub(num_wrapped, Boundary::Cursor);

// Handle wrap-to-next-line case
if cursor.point.column == 0 && /* no content */ {
    input_needs_wrap = true;
}
```

#### Shrinking Columns

**Algorithm:** Process rows forward, extracting cells that exceed new width.

```
1. Start from top row, work downward
2. For each row:
   a. Extract cells exceeding new width
   b. Buffer them for next row
   c. Set WRAPLINE flag on current row
   d. Manage wide-character spacers
3. Adjust viewport offset if lines added
4. Clamp cursor to new valid bounds
```

**Wide character handling:** If a wide character (e.g., CJK) would be split at the new column boundary, the entire character wraps to the next line.

### 5.3 Saved Cursor Handling

**From WebFetch analysis:**

> "The saved cursor position is clamped independently: 'Clamp saved cursor, since only primary cursor is scrolled into viewport.' During shrinking operations, it's constrained to valid bounds without being reflowed like the active cursor."

**Implication:** In Alacritty 0.25, the saved cursor is **NOT** fully reflowed like the primary cursor. It's just clamped to valid grid coordinates.

**This explains why Alacritty was in the "Nonsensical" category in Ghostty's comparison table.**

### 5.4 What Crux Needs to Do

**Option 1: Use alacritty_terminal as-is**
- Reflow is handled internally
- But saved cursor behavior may be incorrect (need to verify current version)
- May need to patch or contribute upstream fix

**Option 2: Implement custom reflow**
- Full control over saved cursor behavior
- More work, more potential for bugs
- Need to maintain as alacritty_terminal evolves

**Recommended:** Use alacritty_terminal but verify current behavior with tests. If saved cursor reflow is broken, contribute a fix upstream (benefits entire Rust terminal ecosystem).

---

## 6. Proposed Solutions and Best Practices

### 6.1 Content-Based Cursor Anchoring

**Problem with grid coordinates:** When grid restructures during reflow, coordinates become meaningless.

**Content-based approach:**
1. Before reflow, identify the cell content at cursor position
2. Perform reflow
3. After reflow, search for that same cell content
4. Place cursor there

**Issues:**
- What if content is ambiguous (multiple identical cells)?
- What if content was in a line that merged with another?
- Performance cost of searching

**Better approach: Logical line + offset**
1. Track which logical line (hard-wrap-delimited) the cursor is on
2. Track offset within that logical line
3. During reflow, logical lines don't change (only their grid representation)
4. Reposition cursor to same logical line + offset

### 6.2 Tracking Logical vs Grid Position

```rust
struct Cursor {
    // Grid position (what VT emulator uses)
    grid_row: usize,
    grid_col: usize,

    // Logical position (for reflow)
    logical_line: usize,      // Line number (hard-wrap delimited)
    logical_offset: usize,    // Character offset within logical line
}
```

**On text write:** Update both grid and logical positions.

**On reflow:** Recalculate grid position from logical position.

### 6.3 Handling Wide Characters

**Problem:** What if cursor is in the middle of a wide character after reflow?

**Example:**
- Before reflow: Cursor at column 79, next cell (80) has CJK character (width=2)
- Terminal shrinks to 80 columns
- CJK character wraps to next line
- Cursor stays at column 79? Or wraps too?

**Best practice:** If cursor is logically "on" a wide character, move it to the first cell of that character in the new grid.

**Wide character spacer cells:** Should never have cursor on them. Always snap cursor to the leading cell.

### 6.4 Pending Wrap State

**CRITICAL:** This is saved by DECSC and must be handled correctly.

**What is pending wrap?** Cursor is at the rightmost column and the next character should wrap to the next line, but the wrap hasn't happened yet.

**Why it matters:**
```
Terminal is 80 columns wide
User types 80 characters
Cursor is at column 79 (0-indexed)
Pending wrap is TRUE

If user types one more character:
  - Character goes to column 0 of next line
  - Cursor follows

If terminal resizes to 120 columns:
  - Pending wrap should become FALSE
  - Cursor stays at column 79
  - Next character goes to column 80 (same line)
```

**On reflow:** Reset pending wrap state if line no longer needs to wrap.

### 6.5 Performance Considerations

**Large scrollback:** Reflowing 100,000 lines can be slow.

**Optimization strategies:**

1. **Lazy reflow:** Only reflow visible viewport + small margin
   - Reflow scrollback on-demand when user scrolls
   - Track "needs reflow" flag for each region

2. **Incremental reflow:** Process in chunks, yield to event loop
   - Prevents UI freeze
   - Show progress indicator for large reflows

3. **Parallel reflow:** Reflow independent regions in parallel
   - Scrollback chunks are independent
   - Use Rayon for data parallelism

4. **Avoid reflow:** Shell integration to redraw prompt
   - Kitty/Ghostty approach: clear line, let shell redraw
   - Requires OSC 133 semantic prompts
   - Perfect reflow without computation cost

**Benchmark targets (from Alacritty):**
- 100K lines full reflow: < 100ms
- Viewport-only reflow: < 10ms

---

## 7. Recommendations for Crux

### 7.1 Implementation Strategy

**Phase 1: Basic reflow (Phase 2 of Crux)**
- Use `alacritty_terminal`'s built-in reflow
- Verify it handles primary cursor correctly
- Document any limitations

**Phase 2: Saved cursor fix (Phase 3 or 4)**
- Test saved cursor behavior with provided test scripts
- If broken, implement fix:
  - Track logical position for saved cursor
  - Reflow saved cursor during resize
  - Contribute fix upstream to alacritty_terminal

**Phase 3: Shell integration (Phase 5)**
- Implement OSC 133 semantic prompt markers
- Add shell integration scripts (bash, zsh, fish)
- Allow prompt redraw on resize (Ghostty approach)

### 7.2 Testing Strategy

**Critical test cases:**

1. **Basic reflow:**
   - Long line wraps when terminal narrows
   - Wrapped line rejoins when terminal widens
   - Cursor follows correctly

2. **Vim scenario:**
   - Run vim, resize, exit
   - Shell prompt should be intact

3. **DECSC/DECRC:**
   - Save cursor on wrapped line
   - Resize (both grow and shrink)
   - Restore cursor
   - Should restore to correct logical position

4. **Wide characters:**
   - CJK text wrapping
   - Cursor on wide character during resize
   - Should not split wide characters incorrectly

5. **Pending wrap state:**
   - Cursor at rightmost column
   - Resize before typing next character
   - Next character should go to correct position

**Use existing test scripts:**
- Ghostty #5718 has bash reproduction script
- iTerm2 #12166 has two separate test scripts
- tmux #4366 references test scripts

### 7.3 Debugging Tips

**Add logging:**
```rust
debug!("Reflow: cols {} -> {}", old_cols, new_cols);
debug!("Cursor before: {:?}", cursor);
debug!("Cursor after: {:?}", cursor);
debug!("Saved cursor before: {:?}", saved_cursor);
debug!("Saved cursor after: {:?}", saved_cursor);
```

**Visual debugging:**
- Highlight cells with WRAPLINE flag in different color
- Show grid coordinates on hover (dev mode)
- Animate reflow step-by-step (slow motion mode)

**Automated testing:**
- Capture terminal state before resize
- Resize
- Capture state after
- Compare with expected state
- Use `portable-pty` to automate input

### 7.4 Documentation

**Add to research/core/terminal-resize-reflow.md** (this document):
- Link to test cases
- Link to upstream issues
- Document Crux's design decisions

**Add to CLAUDE.md or AGENTS.md:**
- "Reflow is complex, see research/core/terminal-resize-reflow.md"
- "Don't assume DECSC/DECRC 'just work' across resize"

**Add comments in code:**
```rust
// CRITICAL: Saved cursor must be reflowed, not just clamped.
// See: research/core/terminal-resize-reflow.md
// Many terminals get this wrong (Ghostty #5718, Kitty #8325)
```

---

## 8. Open Questions for Crux

1. **Does alacritty_terminal 0.25 reflow saved cursors correctly?**
   - Need to test with Ghostty's reproduction script
   - If not, is this fixed in a newer version?
   - If still broken, we need to fix it

2. **How to handle reflow when alternate screen is active?**
   - Reflow both screens?
   - Only reflow active screen?
   - What about saved cursor on inactive screen?

3. **Performance target for large scrollback?**
   - Crux will use GPU rendering (fast)
   - But reflow is CPU-bound
   - Is lazy reflow needed, or is full reflow fast enough?

4. **Should Crux allow disabling reflow?**
   - Some users want old xterm behavior
   - Add config option?
   - Or always reflow (simpler code)?

5. **OSC 133 semantic prompts from day one?**
   - Enables perfect reflow with shell integration
   - But requires shell script installation
   - Should basic reflow work without shell integration?

---

## 9. References and Source Code

### 9.1 GitHub Issues (Terminal Implementations)

**Kitty:**
- [#8325 - Cursor position incorrect after DECSC, resize with reflow, followed by DECRC](https://github.com/kovidgoyal/kitty/issues/8325)
- [#3848 - [RFC] Shell integration](https://github.com/kovidgoyal/kitty/discussions/3848)
- [#5766 - kitty breaks soft-wrap on vertical cursor motion](https://github.com/kovidgoyal/kitty/issues/5766)

**Ghostty:**
- [#5718 - Terminal resize with reflow doesn't reflow the saved cursor (ESC 7)](https://github.com/ghostty-org/ghostty/issues/5718)
- [#5932 - OSC 133: Support semantic prompt regions](https://github.com/ghostty-org/ghostty/issues/5932)

**Windows Terminal:**
- [#4200 - Scenario: ResizeWithReflow and related issues](https://github.com/microsoft/terminal/issues/4200)
- [#4741 - Add support for "reflow"ing the Terminal buffer (PR)](https://github.com/microsoft/terminal/pull/4741)
- [#7466 - Text Doesn't Reflow Properly on Window Resize after Vim Opened and Closed](https://github.com/microsoft/terminal/issues/7466)
- [#6901 - When Copying, Line Wrapped Text Is Inconsistently Broken Into Multiple Lines](https://github.com/microsoft/terminal/issues/6901)
- [#14291 - Add an option to disable reflow on window resize](https://github.com/microsoft/terminal/issues/14291)

**WezTerm:**
- [#6669 - Cursor position incorrect after DECSC, resize with reflow, then DECRC](https://github.com/wezterm/wezterm/issues/6669)
- [#234 - Incorrect screen wrapping/reflow upon resizing terminal window](https://github.com/wezterm/wezterm/issues/234)
- [#2987 - Bad prompt on resize behavior](https://github.com/wezterm/wezterm/issues/2987)

**tmux:**
- [#4366 - Cursor position incorrect after DECSC, resize with reflow, then DECRC](https://github.com/tmux/tmux/issues/4366)
- [#516 - Pane resizing breaks bash, zsh, csh prompts](https://github.com/tmux/tmux/issues/516)
- [#783 - Line reflow messed up](https://github.com/tmux/tmux/issues/783)
- [#1249 - Very slow reflow with large histories](https://github.com/tmux/tmux/issues/1249)
- [#3064 - OSC 133 (shell integration / semantic prompt) support](https://github.com/tmux/tmux/issues/3064)

**iTerm2:**
- [#12166 - Cursor position incorrect after DECSC, resize with reflow, then DECRC](https://gitlab.com/gnachman/iterm2/-/issues/12166)

**Alacritty:**
- [#2302 - Triple Click Should Select Wrapped Lines](https://github.com/alacritty/alacritty/issues/2302)
- [#3584 - Reflow not wrapping cursor correctly](https://github.com/alacritty/alacritty/issues/3584)
- [#4419 - Resize / Reflow Issues](https://github.com/alacritty/alacritty/issues/4419)
- [#2567 - Text reflow slow with large grids](https://github.com/alacritty/alacritty/issues/2567)
- [#7873 - Fix logic for reflowing cursor when growing columns, after shrinking columns (PR)](https://github.com/alacritty/alacritty/pull/7873)
- [#4993 - Extra new lines inserted on copy (Windows)](https://github.com/alacritty/alacritty/issues/4993)

**xterm.js:**
- [#622 - Support reflowing lines on resize](https://github.com/xtermjs/xterm.js/issues/622)
- [#2121 - Screen resize deletes characters, no text reflow](https://github.com/xtermjs/xterm.js/issues/2121)

**Konsole:**
- [Bug #196998 - Konsole should reflow the text when resizing](https://bugs.kde.org/show_bug.cgi?id=196998)
- [!181 - Reflow lines when Terminal resizes](https://invent.kde.org/utilities/konsole/-/merge_requests/181)
- [!321 - Reflow lines when Terminal resizes](https://invent.kde.org/utilities/konsole/-/merge_requests/321)

**foot:**
- [#504 - Text reflow is too slow](https://codeberg.org/dnkl/foot/issues/504)
- [#939 - Text reflow: estimate prompt position after resize](https://codeberg.org/dnkl/foot/issues/939)
- [#1088 - Add support for OSC-133;A (prompt markers)](https://codeberg.org/dnkl/foot/pulls/1088)

**Vim/Neovim:**
- [#15511 - Terminal mode should soft-wrap lines, not hard-wrap](https://github.com/neovim/neovim/issues/15511)
- [#11717 - vim always resets alternate screen](https://github.com/vim/vim/issues/11717)
- [#4997 - :terminal text clipped after resize](https://github.com/neovim/neovim/issues/4997)

### 9.2 Source Code Repositories

**Alacritty:**
- [alacritty_terminal/src/grid/resize.rs](https://github.com/alacritty/alacritty/blob/master/alacritty_terminal/src/grid/resize.rs)
- [alacritty_terminal/src/grid/row.rs](https://github.com/alacritty/alacritty/blob/master/alacritty_terminal/src/grid/row.rs)
- [alacritty_terminal::grid - Rust docs](https://docs.rs/alacritty_terminal/latest/alacritty_terminal/grid/)

**Kitty:**
- [kovidgoyal/kitty](https://github.com/kovidgoyal/kitty)

**Ghostty:**
- [ghostty-org/ghostty](https://github.com/ghostty-org/ghostty)

**foot:**
- [dnkl/foot](https://codeberg.org/dnkl/foot)

**libvte:**
- [GNOME/vte](https://gitlab.gnome.org/GNOME/vte)

### 9.3 VT Specifications and Documentation

**DEC VT510:**
- [DECSC—Save Cursor](https://vt100.net/docs/vt510-rm/DECSC.html)
- [DECRC—Restore Cursor](https://vt100.net/docs/vt510-rm/DECRC.html)
- [DECSTBM—Set Top and Bottom Margins](https://vt100.net/docs/vt510-rm/DECSTBM.html)

**Ghostty VT Docs:**
- [Save Cursor (DECSC) - ESC](https://ghostty.org/docs/vt/esc/decsc)
- [Restore Cursor (DECRC) - ESC](https://ghostty.org/docs/vt/esc/decrc)
- [Set Top and Bottom Margins (DECSTBM) - CSI](https://ghostty.org/docs/vt/csi/decstbm)

**xterm Control Sequences:**
- [ctlseqs (contents)](https://invisible-island.net/xterm/ctlseqs/ctlseqs-contents.html)

### 9.4 Shell Integration and OSC 133

**WezTerm:**
- [Shell Integration](https://wezterm.org/shell-integration.html)
- [shell-integration.md source](https://github.com/wezterm/wezterm/blob/main/docs/shell-integration.md)

**Issues:**
- [WezTerm #7168 - OSC 133 move to previous/next prompt not working inside tmux](https://github.com/wezterm/wezterm/issues/7168)
- [tmux #3064 - OSC 133 (shell integration / semantic prompt) support](https://github.com/tmux/tmux/issues/3064)
- [Ghostty #5932 - OSC 133: Support semantic prompt regions](https://github.com/ghostty-org/ghostty/issues/5932)

**Specification:**
- [zsh: PATCH: terminal integration with semantic markers](https://www.zsh.org/mla/workers/2025/msg00106.html)
- [Neovim Terminal docs](https://neovim.io/doc/user/terminal.html)

### 9.5 Performance and Architecture

**Scrollback optimization:**
- [Alacritty: Scrollback lands](https://jwilm.io/blog/alacritty-lands-scrollback/)
- [WezTerm: Scrollback docs](https://wezterm.org/scrollback.html)
- [WezTerm #3356 - How does a terminal emulator know when it is safe to push a line into the scrollback?](https://github.com/wezterm/wezterm/discussions/3356)

**SIGWINCH and PTY resize:**
- [Playing with SIGWINCH](https://www.rkoucha.fr/tech_corner/sigwinch.html)
- [TIOCSWINSZ(2const) - Linux manual page](https://man7.org/linux/man-pages/man2/TIOCSWINSZ.2const.html)
- [The history of Unix's ioctl and signal about window sizes | Hacker News](https://news.ycombinator.com/item?id=42039401)

**Blog posts and articles:**
- [Mitchell Hashimoto: Ghostty](https://mitchellh.com/ghostty)
- [Mitchell Hashimoto: Ghostty Devlog 001](https://mitchellh.com/writing/ghostty-devlog-001)
- ["This week in KDE: text reflow in Konsole!"](https://pointieststick.com/2021/01/15/this-week-in-kde-text-reflow-in-konsole/)

### 9.6 CJK and Wide Characters

**Issues:**
- [Google Gemini #13537 - fix(ui): Correct mouse click cursor positioning for wide characters (PR)](https://github.com/google-gemini/gemini-cli/pull/13537)
- [Windows Terminal #370 - Ambiguous width character in CJK environment](https://github.com/microsoft/terminal/issues/370)
- [Kitty #6560 - incorrect handling of CJK ambiguous width characters](https://github.com/kovidgoyal/kitty/issues/6560)
- [Alacritty #2385 - Wrap line if double-width character is in last column](https://github.com/alacritty/alacritty/issues/2385)

**Tools and libraries:**
- [wcwidth · PyPI](https://pypi.org/project/wcwidth/)
- [Unicode: Proper Complex Script Support in Text Terminals (PDF)](https://www.unicode.org/L2/L2023/23107-terminal-suppt.pdf)

---

## 10. Summary: Key Takeaways for Crux

1. **Reflow is hard.** Every major terminal has bugs in this area. Don't feel bad if Crux's first implementation isn't perfect.

2. **Saved cursor is the gotcha.** Most terminals reflow the primary cursor correctly but forget about saved cursor (DECSC). This causes the vim bug.

3. **alacritty_terminal might have this bug.** Verify current behavior. If broken, fix it and contribute upstream.

4. **Track logical position, not just grid coordinates.** This makes reflow much easier to reason about.

5. **Pending wrap state matters.** Don't forget to reset it during reflow.

6. **Wide characters are tricky.** Never split a wide character across lines. Always move the whole character.

7. **Test with the vim scenario.** If this works, most other scenarios will work too.

8. **Shell integration is the future.** OSC 133 semantic prompts let the shell redraw on resize, avoiding reflow complexity entirely.

9. **Performance matters at scale.** 100K line scrollback is common. Optimize for this case.

10. **Document everything.** Future maintainers will thank you when debugging subtle reflow bugs.

---

**Last updated:** 2025-02-12
**Researched by:** oh-my-claudecode:researcher (Claude Opus 4.6)
**For:** Crux terminal emulator
