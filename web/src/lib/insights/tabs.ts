/**
 * The Overview below its first screen: five tabs, each listing the panels it holds and the question
 * each panel answers. Data only: the page draws whatever this table holds, so a new panel is one
 * more row here (and its branch in overview/TabPanel.svelte). The tab lives in the URL (`?tab=`);
 * without one, the tab you used last (lib/settings/views.ts), else Activity. Pure, tested in
 * tabs.test.ts.
 */
import type { Capability } from '#lib/api/types.ts';
import { pageHref, withQuery } from '#lib/shared/links.ts';
import type { Availability } from '#lib/sources/capabilities.ts';
import { missingNotice } from '#lib/sources/missing.ts';
import type { Notice } from '#lib/ui/panelState.ts';
import type { OverviewTabName, PanelName } from '#lib/shared/usage-actions.ts';

export type TabPanelEntry = {
	panel: PanelName;
	/** The panel's title: the question it answers. */
	question: string;
	/** The question over the 24-hour range, when the panel counts hours instead of days. */
	hourly?: string;
};

export type OverviewTabEntry = {
	id: OverviewTabName;
	label: string;
	/** What the tab holds, in a few words under its name. */
	hint: string;
	/** The data the tab is about: when no tool in use reports it, the tab says so in place. */
	needs?: { capability: Capability; what: string };
	panels: readonly TabPanelEntry[];
};

export const OVERVIEW_TABS = [
	{
		id: 'activity',
		label: 'Activity',
		hint: 'week rhythm · sessions',
		panels: [
			{ panel: 'rhythm', question: 'When in the week do I prompt?' },
			{ panel: 'sessions', question: 'How long do my sessions last?' }
		]
	},
	{
		id: 'types',
		label: 'Prompt types',
		hint: 'who wrote them · commands · length · sent again',
		panels: [
			{ panel: 'origin', question: 'Who wrote these prompts?' },
			{ panel: 'commands', question: 'Which slash commands do I run?' },
			{ panel: 'writing', question: "How long are my prompts, and what's in them?" },
			{ panel: 'reuse', question: 'Which prompts do I send again?' }
		]
	},
	{
		id: 'cost',
		label: 'Models and cost',
		hint: 'where the money goes · which model',
		needs: { capability: 'cost', what: 'costs' },
		panels: [
			{ panel: 'spend', question: 'Where does the money go?' },
			{ panel: 'models', question: 'Which model do I use most?' },
			{
				panel: 'activity',
				question: 'What does each day cost?',
				hourly: 'What does each hour cost?'
			}
		]
	},
	{
		id: 'agentic',
		label: 'Agentic work',
		hint: 'what the agent does · subagents · speed · outcomes',
		needs: { capability: 'tools', what: 'tool calls' },
		panels: [
			{ panel: 'agent', question: 'What does the agent do?' },
			{ panel: 'delegation', question: 'How much goes to subagents and MCP servers?' },
			{ panel: 'performance', question: 'How fast and reliable are turns?' },
			{ panel: 'outcomes', question: 'What came of my prompts?' }
		]
	},
	{
		id: 'starred',
		label: 'Starred',
		hint: 'prompts you starred',
		panels: [{ panel: 'starred', question: 'Which prompts did I star?' }]
	}
] as const satisfies readonly OverviewTabEntry[];

export type OverviewTab = (typeof OVERVIEW_TABS)[number]['id'];

export const OVERVIEW_TAB_IDS: OverviewTab[] = OVERVIEW_TABS.map((t) => t.id);

export const DEFAULT_TAB: OverviewTab = 'activity';

export const isOverviewTab = (value: string | null | undefined): value is OverviewTab =>
	OVERVIEW_TAB_IDS.some((id) => id === value);

/** The tab to show: the URL's `?tab=`, else the one remembered, else Activity. Unknown ids fall back. */
export function resolveTab(inUrl: string | null, remembered: string | null): OverviewTab {
	if (isOverviewTab(inUrl)) return inUrl;
	return isOverviewTab(remembered) ? remembered : DEFAULT_TAB;
}

export const tabEntry = (id: OverviewTab): OverviewTabEntry =>
	OVERVIEW_TABS.find((t) => t.id === id) ?? OVERVIEW_TABS[0];

/** The question a panel answers, for the range shown (hours over 24 hours, else days). */
export const questionOf = (entry: TabPanelEntry, hourly: boolean): string =>
	(hourly && entry.hourly) || entry.question;

/** The id of a panel's place on the page, for the tab's "In this tab" jump links. */
export const panelAnchor = (panel: PanelName) => `overview-${panel}`;

/**
 * The Overview with `tab` open and the rest of `search` (`?range=7d&…`) kept: the filters. The default tab is
 * named too, so the URL always wins over the tab remembered.
 */
export function overviewTabHref(search: string, tab: OverviewTab): string {
	const params = new URLSearchParams(search);
	params.set('tab', tab);
	return withQuery(pageHref('/'), params);
}

/**
 * What a tab says in place of its takeaway when no tool in use reports the data it is about (its
 * panels then say the same, each in its place): the tab stays where it is, so nothing moves.
 */
export function tabNotice(
	entry: OverviewTabEntry,
	can: (capability: Capability) => Availability
): Notice | null {
	if (!entry.needs) return null;
	const reporters = can(entry.needs.capability);
	return reporters.available.length ? null : missingNotice(entry.needs.what, reporters);
}
