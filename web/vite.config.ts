import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

const daemon = `http://127.0.0.1:${process.env.GITPROMPT_PORT || 7878}`;

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// The daemon serves the built dashboard from its package.
			adapter: adapter({ pages: '../src/gitprompt/dashboard' })
		})
	],
	server: { proxy: { '/api': daemon } },
	test: {
		include: ['src/**/*.test.ts'],
		environment: 'node',
		// Day grouping uses the local calendar: pin a zone with daylight saving to keep tests honest.
		env: { TZ: 'Europe/Berlin' }
	}
});
