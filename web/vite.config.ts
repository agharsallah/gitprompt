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
		include: ['src/**/*.test.ts'],
		environment: 'node',
		// Day grouping uses the local calendar: pin a zone with daylight saving to keep tests honest.
		env: { TZ: 'Europe/Berlin' }
	}
});
