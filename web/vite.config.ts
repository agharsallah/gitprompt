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
			// Built into web/build, which the Rust daemon embeds (backend/crates/gitprompt-dashboard) and
			// the desktop app ships. Story pages (/sessions/<id>/, /projects/<id>/) have no file of
			// their own and get the fallback page, which routes in the browser.
			adapter: adapter({ pages: 'build', fallback: '200.html' })
		})
	],
	server: { proxy: { '/api': daemon } },
	test: {
		// Day grouping uses the local calendar: pin a zone with daylight saving to keep tests honest.
		env: { TZ: 'Europe/Berlin' },
		projects: [
			{
				extends: true,
				test: {
					name: 'unit',
					include: ['src/**/*.test.ts'],
					exclude: ['src/**/*.svelte.test.ts'],
					environment: 'node'
				}
			},
			{
				// Tests of runes ($effect in a .svelte.ts module) need Svelte's browser build.
				extends: true,
				resolve: { conditions: ['browser'] },
				test: {
					name: 'runes',
					include: ['src/**/*.svelte.test.ts'],
					environment: './src/lib/testing/client-environment.ts'
				}
			}
		],
		coverage: {
			include: ['src/lib/**/*.ts'],
			// The display helpers are pure and every branch is a corner case someone hit: keep them whole.
			thresholds: { 'src/lib/shared/{format,model-name}.ts': { 100: true } }
		}
	}
});
