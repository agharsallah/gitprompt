/**
 * What GitPrompt counts about its own dashboard, on this machine only (`track`, usage.ts): the
 * closed list of names. A name is a dotted lowercase word the daemon accepts (letters, digits,
 * `.`, `_`, `-`; at most 48 characters), so a new action is one more entry in a list below.
 */

/** The Overview panels: seen (stayed in view), and clicked through to the prompts behind them. */
export const PANELS = [
	'key-figures',
	'activity',
	'rhythm',
	'sessions',
	'origin',
	'commands',
	'writing',
	'reuse',
	'models',
	'spend',
	'performance',
	'agent',
	'delegation',
	'outcomes',
	'starred'
] as const;
export type PanelName = (typeof PANELS)[number];

/** The tabs under a prompt in the reader. */
export const READER_TABS = [
	'activity',
	'usage',
	'git',
	'session',
	'versions',
	'events',
	'raw'
] as const;
export type ReaderTab = (typeof READER_TABS)[number];

/** The pages, by what they show (not by route id). */
export const PAGES = [
	'overview',
	'prompts',
	'sessions',
	'session',
	'projects',
	'project',
	'sources',
	'fields',
	'settings'
] as const;
export type PageName = (typeof PAGES)[number];

/** The kinds of filter: a change of one in the URL of the page you are on. */
export const FILTER_KINDS = [
	'range',
	'origin',
	'source',
	'surface',
	'repo',
	'model',
	'outcome',
	'search',
	'tag',
	'starred',
	'quick'
] as const;
export type FilterKind = (typeof FILTER_KINDS)[number];

export type UsageAction =
	| `panel.seen.${PanelName}`
	| `panel.click.${PanelName}`
	| `reader.tab.${ReaderTab}`
	| `page.opened.${PageName}`
	| `filter.${FilterKind}`
	| 'copy.prompt'
	| 'copy.resume'
	| 'copy.code'
	| 'export.open'
	| 'palette.open'
	| 'palette.copy'
	| 'tab.project.commits'
	| 'commit.open.prompt';

export const panelAction = <K extends 'seen' | 'click', P extends PanelName>(kind: K, panel: P) =>
	`panel.${kind}.${panel}` as const;

/** Every action, for a view of what is never used. */
export const USAGE_ACTIONS: readonly UsageAction[] = [
	...PANELS.map((p) => panelAction('seen', p)),
	...PANELS.map((p) => panelAction('click', p)),
	...READER_TABS.map((t) => `reader.tab.${t}` as const),
	...PAGES.map((p) => `page.opened.${p}` as const),
	...FILTER_KINDS.map((k) => `filter.${k}` as const),
	'copy.prompt',
	'copy.resume',
	'copy.code',
	'export.open',
	'palette.open',
	'palette.copy',
	'tab.project.commits',
	'commit.open.prompt'
];
