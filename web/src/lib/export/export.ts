/**
 * Exports (plan E5, ADR-0018): what can be exported, the options and their most private defaults,
 * and the daemon URLs that render them (GET /api/export/{kind}). The preview and the downloaded file
 * are the same URL, so what the preview shows is exactly what the file holds.
 */
import { queryParams, type StatsQuery } from '#lib/api/query.ts';

export type ExportKind = 'session' | 'project' | 'overview' | 'prompts';
export type DocumentFormat = 'html' | 'md' | 'csv' | 'json';

/** A session or project by id; the Overview or a prompt selection by its filters. */
export type ExportTarget =
	{ kind: 'session' | 'project'; id: string } | { kind: 'overview' | 'prompts'; query: StatsQuery };

/** export/redact.py Options. Secrets are always hidden: that is not an option. */
export type ExportOptions = {
	prompt_text: boolean;
	relative_paths: boolean;
	drop_identity: boolean;
	drop_raw: boolean;
};

/** The most private choice of each option: what every export starts from. */
export const PRIVATE_DEFAULTS: Readonly<ExportOptions> = Object.freeze({
	prompt_text: false,
	relative_paths: true,
	drop_identity: true,
	drop_raw: true
});

export const FORMATS: { value: DocumentFormat; label: string; hint: string }[] = [
	{ value: 'html', label: 'HTML', hint: 'One file with its charts; opens offline in any browser.' },
	{ value: 'md', label: 'Markdown', hint: 'A report for a README, an issue or a note.' },
	{ value: 'csv', label: 'CSV', hint: 'The rows under the report, for a spreadsheet.' },
	{ value: 'json', label: 'JSON', hint: 'The same rows, with the options that made them.' }
];

export type OptionText = {
	key: keyof ExportOptions;
	label: string;
	description: string;
	/** Only offered for these kinds (raw payloads exist in a prompt selection only). */
	kinds?: ExportKind[];
};

export const OPTIONS: OptionText[] = [
	{
		key: 'prompt_text',
		label: 'Include prompt text',
		description:
			'What you typed, and system text quoted as such. Off: figures, outcomes, files and commits only.'
	},
	{
		key: 'relative_paths',
		label: 'Repository-relative paths',
		description: 'Paths inside a project start at its folder; your home folder becomes ~.'
	},
	{
		key: 'drop_identity',
		label: 'Leave out account identity',
		description:
			'Account email, user and organization ids from telemetry, commit authors and any email address.'
	},
	{
		key: 'drop_raw',
		label: 'Leave out raw payloads',
		description: 'The hook payload stored with each prompt, as received.',
		kinds: ['prompts']
	}
];

export const SECRETS_NOTE =
	'Keys, tokens, private keys and password assignments are replaced by “[hidden …]” in every format. This is not an option: a file you send can be forwarded, and a leaked key cannot be taken back.';

/** The options that differ from the most private choice (what a reviewer should look at). */
export function loosened(options: ExportOptions): (keyof ExportOptions)[] {
	return (Object.keys(PRIVATE_DEFAULTS) as (keyof ExportOptions)[]).filter(
		(k) => options[k] !== PRIVATE_DEFAULTS[k]
	);
}

/** What is being exported, as a noun phrase ("this session"). */
export function subject(kind: ExportKind): string {
	return {
		session: 'this session',
		project: 'this project',
		overview: 'the Overview',
		prompts: 'these prompts'
	}[kind];
}

function params(
	target: ExportTarget,
	options: ExportOptions,
	extra: Record<string, string>
): URLSearchParams {
	const p = 'id' in target ? new URLSearchParams({ id: target.id }) : queryParams(target.query);
	p.delete('before'); // an export covers every page of a prompt selection
	p.delete('limit');
	for (const [k, v] of Object.entries(options)) p.set(k, String(v));
	for (const [k, v] of Object.entries(extra)) p.set(k, v);
	return p;
}

/** The URL of one export: preview it, or add `download` to save it as a file. */
export function exportUrl(
	target: ExportTarget,
	format: DocumentFormat | 'svg',
	options: ExportOptions,
	extra: { download?: boolean; chart?: string } = {}
): string {
	const more: Record<string, string> = { format };
	if (extra.download) more.download = 'true';
	if (extra.chart) more.chart = extra.chart;
	return `/api/export/${target.kind}?${params(target, options, more)}`;
}

/** The charts of an export, each downloadable alone. */
export function chartsUrl(target: ExportTarget, options: ExportOptions): string {
	return `/api/export/${target.kind}/charts?${params(target, options, {})}`;
}

/** A file name for a chart: gitprompt-<kind>-<chart>.<ext>. */
export function chartFileName(kind: ExportKind, chart: string, extension: 'svg' | 'png'): string {
	return `gitprompt-${kind}-${chart.replace(/[^a-z0-9-]+/gi, '-').toLowerCase()}.${extension}`;
}
