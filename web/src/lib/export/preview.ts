/**
 * The export preview's requests: the file as the daemon renders it (text, file name, media type),
 * and for one chart the list of charts plus the chosen chart as SVG. The preview is the file, so the
 * dialog saves these bytes without asking again. Tested in preview.test.ts with an injected fetch.
 */
import {
	chartsUrl,
	exportUrl,
	type DocumentFormat,
	type ExportOptions,
	type ExportTarget
} from '#lib/export/export.ts';

/** What the dialog previews: a document format, or one chart. */
export type ExportMode = DocumentFormat | 'chart';
export type Preview = { text: string; name: string; type: string };
export type ChartInfo = { id: string; title: string };
/** The charts an export offers, the one shown (empty when there is none) and its SVG. */
export type ChartPreview = { charts: ChartInfo[]; chart: string; svg: Preview | null };

const FALLBACK_NAME = 'gitprompt-export';

/** The file name the daemon gives (Content-Disposition), so preview and download agree. */
export function fileName(response: Response, fallback = FALLBACK_NAME): string {
	const header = response.headers.get('content-disposition') ?? '';
	return /filename="([^"]+)"/.exec(header)?.[1] ?? fallback;
}

/** The response, or an error saying the status and what the daemon answered. */
async function ok(response: Response): Promise<Response> {
	if (!response.ok) throw new Error(`${response.status}: ${await response.text()}`);
	return response;
}

/** One rendered file, as text. */
export async function fetchPreview(url: string, fetcher: typeof fetch = fetch): Promise<Preview> {
	const response = await ok(await fetcher(url));
	const type = response.headers.get('content-type') ?? 'text/plain';
	return { text: await response.text(), name: fileName(response), type };
}

const isChartInfo = (value: unknown): value is ChartInfo =>
	typeof value === 'object' &&
	value !== null &&
	'id' in value &&
	typeof value.id === 'string' &&
	'title' in value &&
	typeof value.title === 'string';

/** The charts of an export; an answer that is not a list of charts is an error, not an empty list. */
export async function fetchCharts(
	url: string,
	fetcher: typeof fetch = fetch
): Promise<ChartInfo[]> {
	const list: unknown = await (await ok(await fetcher(url))).json();
	if (!Array.isArray(list) || !list.every(isChartInfo)) {
		throw new Error(`Unexpected answer for the list of charts: ${JSON.stringify(list)}`);
	}
	return list;
}

/** A document export rendered in `format`. */
export function loadDocument(
	target: ExportTarget,
	format: DocumentFormat,
	options: ExportOptions,
	fetcher: typeof fetch = fetch
): Promise<Preview> {
	return fetchPreview(exportUrl(target, format, options), fetcher);
}

/** The charts, and `wanted` as SVG when the export still has it, else its first chart. */
export async function loadChart(
	target: ExportTarget,
	options: ExportOptions,
	wanted: string,
	fetcher: typeof fetch = fetch
): Promise<ChartPreview> {
	const charts = await fetchCharts(chartsUrl(target, options), fetcher);
	const chart = charts.some((c) => c.id === wanted) ? wanted : (charts[0]?.id ?? '');
	const svg = chart
		? await fetchPreview(exportUrl(target, 'svg', options, { chart }), fetcher)
		: null;
	return { charts, chart, svg };
}

/** An SVG as an image URL: nothing to revoke, so a preview never leaks a blob. */
export function svgDataUrl(svg: string): string {
	return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}
