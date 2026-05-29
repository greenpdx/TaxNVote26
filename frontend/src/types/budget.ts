// src/types/budget.ts
// Mirrors the Rust types in node.rs / adjust.rs

export interface BudgetConfig {
  minFractionOfDefault: number
  precision: number
  enforceExactSum: boolean
}

export interface BudgetNode {
  idx: number
  id: string
  name: string
  value: number
  defaultValue: number
  templateValue: number
  locked: boolean
  parent: number // -1 for root
  children: number[]
  level: number
}

export interface Change {
  idx: number
  newVal: number
}

export type DisplayMode = 'full' | 'simple'

/** What the WASM module exposes (wasm.rs API) */
export interface IWasmBudgetTree {
  len(): number
  get_value(id: string): number
  all_values(): Float64Array
  all_nodes_json(): string
  adjust(id: string, newValue: number): Float64Array
  lock(id: string): boolean
  unlock(id: string): boolean
  reset_node(id: string): Float64Array
  reset_to_default(): Float64Array
  reset_to_template(): Float64Array
  apply_template(json: string): Float64Array
  validate(): string
}
