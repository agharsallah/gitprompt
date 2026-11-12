import { describe, expect, it } from 'vitest';
import {
	chartFileName,
	chartsUrl,
	exportUrl,
	FORMATS,
	loosened,
	loosenedContent,
	offeredOptions,
	OPTIONS,
	PRIVATE_DEFAULTS,
	subject,
	type ExportKind,
	type ExportOptions
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

	it('list every option that differs from the most private choice', () => {
		const open = {
			prompt_text: true,
			relative_paths: false,
			drop_identity: false,
			drop_raw: false
		};
		expect(loosened(open)).toEqual(['prompt_text', 'relative_paths', 'drop_identity', 'drop_raw']);
		expect(loosened({ ...PRIVATE_DEFAULTS, drop_raw: false })).toEqual(['drop_raw']);
	});

	it('cannot be loosened by changing the defaults', () => {
		expect(Object.isFrozen(PRIVATE_DEFAULTS)).toBe(true);
	});

	it('offer raw payloads only where they exist', () => {
		const kinds = Object.fromEntries(OPTIONS.map((o) => [o.key, o.kinds]));
		expect(kinds).toEqual({
			prompt_text: undefined,
			relative_paths: undefined,
			drop_identity: undefined,
			drop_raw: ['prompts']
		});
	});

	const offered: [ExportKind, (keyof ExportOptions)[]][] = [
		['prompts', ['prompt_text', 'relative_paths', 'drop_identity', 'drop_raw']],
		['session', ['prompt_text', 'relative_paths', 'drop_identity']],
		['project', ['prompt_text', 'relative_paths', 'drop_identity']],
		['overview', ['prompt_text', 'relative_paths', 'drop_identity']]
	];
	it.each(offered)('offered for %s: %j', (kind, keys) =>
		expect(offeredOptions(kind).map((o) => o.key)).toEqual(keys)
	);

	it('say what each loosened option adds to the file', () => {
		expect(loosenedContent([])).toBe('');
		expect(loosenedContent(['prompt_text', 'relative_paths', 'drop_identity', 'drop_raw'])).toBe(
			'the text of your prompts, absolute paths, account identity, raw payloads'
		);
	});

	it('offer each document format once', () => {
		expect(FORMATS.map((f) => f.value)).toEqual(['html', 'md', 'csv', 'json']);
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

	it('send loosened options as they are, and no download or chart unless asked', () => {
		const options = { ...PRIVATE_DEFAULTS, prompt_text: true, drop_identity: false };
		const url = new URL(exportUrl({ kind: 'project', id: 'p' }, 'html', options), 'http://x');
		expect(url.pathname).toBe('/api/export/project');
		expect(url.searchParams.get('prompt_text')).toBe('true');
		expect(url.searchParams.get('drop_identity')).toBe('false');
		expect(url.searchParams.has('download')).toBe(false);
		expect(url.searchParams.has('chart')).toBe(false);
		const plain = new URL(
			exportUrl({ kind: 'project', id: 'p' }, 'html', options, { download: false, chart: '' }),
			'http://x'
		);
		expect(plain.searchParams.has('download')).toBe(false);
		expect(plain.searchParams.has('chart')).toBe(false);
	});

	it('cover every page of a selection: no limit either', () => {
		const target = { kind: 'overview' as const, query: { repo: '/r', limit: 50 } };
		const url = new URL(exportUrl(target, 'json', PRIVATE_DEFAULTS), 'http://x');
		expect(url.searchParams.has('limit')).toBe(false);
		const charts = new URL(chartsUrl(target, PRIVATE_DEFAULTS), 'http://x');
		expect(charts.pathname).toBe('/api/export/overview/charts');
		expect(charts.searchParams.get('repo')).toBe('/r');
		expect(charts.searchParams.has('limit')).toBe(false);
		expect(charts.searchParams.has('format')).toBe(false);
		expect(charts.searchParams.get('drop_raw')).toBe('true');
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

	it('make chart names safe and lower case', () => {
		expect(chartFileName('session', 'Tools / By Kind', 'svg')).toBe(
			'gitprompt-session-tools-by-kind.svg'
		);
	});
});

describe('subject', () => {
	it('names what is exported', () => {
		expect(subject('session')).toBe('this session');
		expect(subject('project')).toBe('this project');
		expect(subject('overview')).toBe('the Overview');
		expect(subject('prompts')).toBe('these prompts');
	});
});
