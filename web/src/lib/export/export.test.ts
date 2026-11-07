import { describe, expect, it } from 'vitest';
import {
	chartFileName,
	chartsUrl,
	exportUrl,
	loosened,
	OPTIONS,
	PRIVATE_DEFAULTS
} from '#lib/export/export.ts';

describe('export options', () => {
	it('start from the most private choice', () => {
		expect(PRIVATE_DEFAULTS).toEqual({
			prompt_text: false,
			relative_paths: true,
			drop_identity: true,
			drop_raw: true
		});
		expect(loosened(PRIVATE_DEFAULTS)).toEqual([]);
		expect(loosened({ ...PRIVATE_DEFAULTS, prompt_text: true })).toEqual(['prompt_text']);
	});

	it('describe every option and never offer to show secrets', () => {
		expect(OPTIONS.map((o) => o.key).sort()).toEqual(Object.keys(PRIVATE_DEFAULTS).sort());
		expect(OPTIONS.some((o) => /secret/i.test(o.label))).toBe(false);
	});
});

describe('export URLs', () => {
	it('name a story by id and send every option', () => {
		const url = new URL(
			exportUrl({ kind: 'session', id: 's 1' }, 'md', PRIVATE_DEFAULTS),
			'http://x'
		);
		expect(url.pathname).toBe('/api/export/session');
		expect(Object.fromEntries(url.searchParams)).toEqual({
			id: 's 1',
			format: 'md',
			prompt_text: 'false',
			relative_paths: 'true',
			drop_identity: 'true',
			drop_raw: 'true'
		});
	});

	it('carry the filters of a selection, without paging', () => {
		const target = {
			kind: 'prompts' as const,
			query: { origin: ['typed' as const, 'slash_command' as const], repo: '/r', before: '9' }
		};
		const url = new URL(
			exportUrl(target, 'svg', PRIVATE_DEFAULTS, { download: true, chart: 'outcomes' }),
			'http://x'
		);
		expect(url.searchParams.getAll('origin')).toEqual(['typed', 'slash_command']);
		expect(url.searchParams.get('repo')).toBe('/r');
		expect(url.searchParams.has('before')).toBe(false);
		expect(url.searchParams.get('download')).toBe('true');
		expect(url.searchParams.get('chart')).toBe('outcomes');
		expect(chartsUrl(target, PRIVATE_DEFAULTS)).toMatch(/^\/api\/export\/prompts\/charts\?/);
	});

	it('name chart files', () => {
		expect(chartFileName('overview', 'tool-kinds', 'png')).toBe(
			'gitprompt-overview-tool-kinds.png'
		);
	});
});
