# TNV Visual Budget Tree — Plan & Report

**Date:** May 21, 2026
**Project:** Tax N Vote — Visual Budget Adjustment Interface
**Author:** Computado Rita / Shaun Savage

---

## 1. Architecture Overview

The visual tree is a Vue 3 (or React) single-page application that displays
the federal budget as an expandable hierarchy with interactive sliders.
All budget logic lives in a Rust/WASM module; the display layer is stateless
and receives only changesets.

```
┌─────────────────────────────────────────────────┐
│  Vue / React Display Layer                      │
│                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐  │
│  │ TreeView │  │ NodeRow  │  │ SliderControl│  │
│  │ (list)   │──│ (row)    │──│ (input)      │  │
│  └──────────┘  └──────────┘  └──────────────┘  │
│       │              │              │           │
│       ▼              ▼              ▼           │
│  ┌─────────────────────────────────────────┐    │
│  │  Pinia Store (reactive node array)      │    │
│  │  nodes[]: { idx, id, name, value, ... } │    │
│  └───────────────┬─────────────────────────┘    │
│                  │  changeset in/out             │
│  ┌───────────────▼─────────────────────────┐    │
│  │  WASM Bridge (thin JS wrapper)          │    │
│  │  adjust(id, val) → [(idx,val),...]      │    │
│  └───────────────┬─────────────────────────┘    │
└──────────────────┼──────────────────────────────┘
                   │
        ┌──────────▼──────────┐
        │  Rust/WASM Module   │
        │  BudgetTree (opaque)│
        │  35 passing tests   │
        └─────────────────────┘
```

## 2. Display Modes

### 2.1 Full Tree (Power User)

Expandable tree showing all levels: Agency → Bureau → Account.
Every node has an expand arrow, a value display, a slider (on select),
lock/unlock toggle, zero button, and reset button.

- Default: agencies collapsed, sorted by value descending
- Click agency → expands to bureaus
- Click bureau → expands to accounts (leaves)
- Click any node → shows slider inline
- Slider drag → calls WASM adjust() → changeset patches all affected nodes

### 2.2 Simple Tree (General Public)

Same tree, but optimized for "I know what I care about":

- Shows agencies as a flat list with percentage bars
- Tap any agency → expands to bureaus
- Tap bureau → expands to accounts
- User can find specific items (e.g. "TSA" under DHS) and fund/defund them
- Rest of the tree auto-adjusts
- "Submit My Tax Dollar" button at bottom

## 3. Component Hierarchy (Vue 3)

```
App.vue
├── HeaderBar.vue          ← title, mode toggle, reset buttons
├── BudgetSummary.vue      ← total, pie chart or bar summary
├── TreeView.vue           ← the scrollable tree container
│   └── NodeRow.vue        ← one row per visible node (virtual list)
│       ├── ExpandArrow    ← ▸/▾ toggle
│       ├── ValueDisplay   ← formatted dollars + percentage
│       ├── PercentBar     ← visual proportional bar
│       ├── SliderControl  ← range input (shown on select)
│       ├── LockToggle     ← 🔒/🔓
│       └── ActionButtons  ← zero, reset
├── SearchBar.vue          ← filter/find nodes by name
└── SubmitPanel.vue        ← "Submit My Tax Dollar" (future)
```

## 4. Data Flow

### 4.1 Init
1. WASM module loads, builds tree from embedded CSV
2. JS calls `all_nodes_json()` → gets metadata for every node
3. Pinia store populates `nodes[]` reactive array
4. TreeView renders top-level agencies

### 4.2 User Adjusts a Slider
1. `SliderControl` emits `@adjust(id, newValue)`
2. Store calls `wasmTree.adjust(id, newValue)` → packed f64 array
3. Store unpacks changeset: `for i in 0..len step 2: nodes[arr[i]].value = arr[i+1]`
4. Vue reactivity updates only affected NodeRow components
5. Total: < 16ms end-to-end

### 4.3 Lock/Unlock
1. User clicks 🔒 → store calls `wasmTree.lock(id)`
2. Node's `locked` flag updated in store
3. Locked nodes show dimmed slider, lock icon

### 4.4 Reset
- "Reset Node" → `wasmTree.reset_node(id)` → changeset
- "Reset to Template" → `wasmTree.reset_to_template()` → changeset
- "Reset to Default" → `wasmTree.reset_to_default()` → changeset

## 5. UI Specifications

### 5.1 NodeRow Layout (Desktop)
```
[▸] [$1,234,567]  [████████░░░░] 15.2%  Department of Defense    [🔒] [0] [↺]
                   ← percent bar →       ← name →                 actions
```

### 5.2 NodeRow Layout (Mobile)
```
[▸] Department of Defense          $1.2B  15.2%
    [████████████████░░░░░░░░░░░░░░░░░░░]
    [──────────●────────────────────] [🔒][↺]
```

### 5.3 Color Coding
- Level 0 (root): not shown
- Level 1 (agency): bold, larger text, colored bar
- Level 2 (bureau): normal text, lighter bar
- Level 3 (account): smaller text, subtle bar
- Locked: dimmed, lock icon
- Changed from default: highlight color

## 6. Technology Stack

- **Vue 3** + Composition API + `<script setup>`
- **Pinia** for state management
- **Vite** for build (with vite-plugin-rsw for WASM)
- **TypeScript** for type safety
- **Tailwind CSS** or scoped CSS for styling
- **Rust/WASM** via wasm-pack (already built, 35/35 tests passing)

## 7. File Structure (Vue Project)

```
tnv-vue/
├── src/
│   ├── App.vue
│   ├── main.ts
│   ├── stores/
│   │   └── budget.ts          ← Pinia store, WASM bridge
│   ├── components/
│   │   ├── HeaderBar.vue
│   │   ├── BudgetSummary.vue
│   │   ├── TreeView.vue
│   │   ├── NodeRow.vue
│   │   ├── SliderControl.vue
│   │   ├── LockToggle.vue
│   │   ├── SearchBar.vue
│   │   └── SubmitPanel.vue
│   ├── composables/
│   │   └── useWasm.ts         ← WASM init + wrapper
│   └── types/
│       └── budget.ts          ← TypeScript interfaces
├── public/
├── pkg/                       ← wasm-pack output (gitignored)
├── package.json
├── vite.config.ts
└── tsconfig.json
```

## 8. Implementation Phases

### Phase 1: Working Prototype (this session)
- React artifact with JS mock of WASM API
- Full tree display with expand/collapse
- Sliders with proportional redistribution
- Lock/unlock
- Both display modes

### Phase 2: Vue 3 Migration
- Port React prototype to Vue 3 + Pinia
- Integrate real WASM module
- Add TypeScript types matching Rust structs

### Phase 3: Polish
- Search/filter
- Keyboard navigation
- Mobile optimization
- Performance (virtual scrolling for 1,237 nodes)
- Accessibility

### Phase 4: Tax Dollar
- Submit panel
- Tax Dollar format export (CSV)
- Template loading
- Validation integration

## 9. Status

| Component         | Status    |
|-------------------|-----------|
| Rust core         | ✅ Done (35/35 tests) |
| WASM bindings     | ✅ Written (needs wasm-pack build) |
| Architecture      | ✅ Defined |
| Vue plan          | ✅ This document |
| React prototype   | 🔨 Building now |
| Vue 3 app         | ⬜ Phase 2 |
| Tax Dollar export | ⬜ Phase 4 |
| Template validator | ⬜ Future |
| Tax Dollar validator | ⬜ Future |
