import { describe, expect, it } from 'vitest';
import { PRIVATE_DEFAULTS, type ExportTarget } from '#lib/export/export.ts';
import {
	fetchCharts,
	fetchPreview,
	fileName,
	loadChart,
	loadDocument,
	svgDataUrl
} from '#lib/export/preview.ts';

const session: ExportTarget = { kind: 'session', id: 's1' };
const options = { ...PRIVATE_DEFAULTS };

type Answer = { body: string; status?: number; headers?: Record<string, string> };

/** A fetch that answers by URL and records what was asked. */
function daemon(answer: (url: string) => Answer) {
	const asked: string[] = [];
	const fetcher: typeof fetch = (input) => {
		const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
		asked.push(url);
		const { body, status = 200, headers } = answer(url);
		return Promise.resolve(new Response(body, { status, headers }));
	};
	return { fetcher, asked };
}

const charts = (list: unknown): Answer => ({ body: JSON.stringify(list) });
const svg = (chart: string): Answer => ({
	body: `<svg id="${chart}"/>`,
	headers: { 'content-type': 'image/svg+xml' }
});

describe('fileName', () => {
	it.each([
		['attachment; filename="gitprompt-session.html"', 'gitprompt-session.html'],
		['inline', 'gitprompt-export'],
		[null, 'gitprompt-export']
	])('%s -> %s', (header, name) => {
		const headers = header ? { 'content-disposition': header } : undefined;
		expect(fileName(new Response('', { headers }))).toBe(name);
	});
});

describe('fetchPreview', () => {
	it('reads the text, name and type of the file', async () => {
		const { fetcher } = daemon(() => ({
			body: '# Session',
			headers: {
				'content-type': 'text/markdown',
				'content-disposition': 'attachment; filename="s.md"'
			}
		}));
		expect(await fetchPreview('/x', fetcher)).toEqual({
			text: '# Session',
			name: 's.md',
			type: 'text/markdown'
		});
	});

	it('says what the daemon answered when it fails', async () => {
		const { fetcher } = daemon(() => ({ body: 'no such session', status: 404 }));
		await expect(fetchPreview('/x', fetcher)).rejects.toThrow('404: no such session');
	});
});

describe('fetchCharts', () => {
	it('fails on an error status instead of reading the error as charts', async () => {
		const { fetcher } = daemon(() => ({ body: '{"error":"locked"}', status: 500 }));
		await expect(fetchCharts('/c', fetcher)).rejects.toThrow('500: {"error":"locked"}');
	});

	it.each([{ error: 'x' }, [{ id: 1, title: 'x' }], [{ id: 'a' }], [null]])(
		'fails on an answer that is not a list of charts: %j',
		async (body) => {
			const { fetcher } = daemon(() => charts(body));
			await expect(fetchCharts('/c', fetcher)).rejects.toThrow('Unexpected answer');
		}
	);
});

describe('loadDocument', () => {
	it('asks for the format with the options', async () => {
		const { fetcher, asked } = daemon(() => ({ body: 'a,b' }));
		expect((await loadDocument(session, 'csv', options, fetcher)).text).toBe('a,b');
		expect(asked).toHaveLength(1);
		expect(asked[0]).toMatch(/^\/api\/export\/session\?.*format=csv/);
	});
});

describe('loadChart', () => {
	const list = [
		{ id: 'tools', title: 'Tools' },
		{ id: 'tokens', title: 'Tokens' }
	];
	const withCharts = (body: unknown) =>
		daemon((url) => {
			if (url.includes('/charts?')) return charts(body);
			return svg(new URL(url, 'http://x').searchParams.get('chart') ?? '');
		});

	it('keeps the wanted chart while the export has it', async () => {
		const { fetcher } = withCharts(list);
		const result = await loadChart(session, options, 'tokens', fetcher);
		expect(result.chart).toBe('tokens');
		expect(result.charts).toEqual(list);
		expect(result.svg).toMatchObject({ text: '<svg id="tokens"/>', type: 'image/svg+xml' });
	});

	it('falls back to the first chart when the wanted one is gone', async () => {
		const { fetcher } = withCharts(list);
		expect((await loadChart(session, options, 'gone', fetcher)).chart).toBe('tools');
	});

	it('draws nothing when the export has no chart', async () => {
		const { fetcher, asked } = withCharts([]);
		expect(await loadChart(session, options, 'tools', fetcher)).toEqual({
			charts: [],
			chart: '',
			svg: null
		});
		expect(asked).toHaveLength(1);
	});
});

describe('svgDataUrl', () => {
	it('encodes the SVG so any character survives', () => {
		const url = svgDataUrl('<svg><text>50% & #1</text></svg>');
		expect(url.startsWith('data:image/svg+xml;charset=utf-8,')).toBe(true);
		expect(decodeURIComponent(url.split(',')[1])).toBe('<svg><text>50% & #1</text></svg>');
	});
});
