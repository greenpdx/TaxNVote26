// tests/setup.ts — boot the real WASM module once for every test file.
//
// happy-dom doesn't ship a working fetch() that can read local files,
// so we load the .wasm bytes off disk and feed them to the
// synchronous initSync() exported by wasm-pack.

import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'
import { initSync } from '../src/wasm/tnv_budget_tree.js'

const here = dirname(fileURLToPath(import.meta.url))
const wasmPath = resolve(here, '../src/wasm/tnv_budget_tree_bg.wasm')
initSync({ module: readFileSync(wasmPath) })
