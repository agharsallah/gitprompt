/**
 * A test environment for runes: Node, but modules compiled and resolved as for the browser, so
 * `$effect` in a `.svelte.ts` module runs (the default `node` environment uses Svelte's server
 * build, where effects never run). There is no DOM: tests stub the globals they need.
 *
 * Tests named `*.svelte.test.ts` run in it (vite.config.ts, project `runes`).
 */
import type { Environment } from 'vitest/runtime';

export default {
	name: 'svelte-client',
	viteEnvironment: 'client',
	setup: () => ({ teardown: () => {} })
} satisfies Environment;
