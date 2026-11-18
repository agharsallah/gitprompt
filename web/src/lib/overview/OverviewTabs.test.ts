import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Capability, Source } from '#lib/api/types.ts';
import type { Overview } from '#lib/insights/overview.ts';
import ActivityPanel from '#lib/insights/panels/ActivityPanel.svelte';
import AgentPanel from '#lib/insights/panels/AgentPanel.svelte';
import CommandsPanel from '#lib/insights/panels/CommandsPanel.svelte';
import DelegationPanel from '#lib/insights/panels/DelegationPanel.svelte';
import ModelsPanel from '#lib/insights/panels/ModelsPanel.svelte';
import OriginPanel from '#lib/insights/panels/OriginPanel.svelte';
import OutcomesPanel from '#lib/insights/panels/OutcomesPanel.svelte';
import PerformancePanel from '#lib/insights/panels/PerformancePanel.svelte';
import ReusePanel from '#lib/insights/panels/ReusePanel.svelte';
import RhythmPanel from '#lib/insights/panels/RhythmPanel.svelte';
import SessionsPanel from '#lib/insights/panels/SessionsPanel.svelte';
import SpendPanel from '#lib/insights/panels/SpendPanel.svelte';
import StarredPanel from '#lib/insights/panels/StarredPanel.svelte';
import WritingPanel from '#lib/insights/panels/WritingPanel.svelte';
import { OVERVIEW_TABS, panelAnchor, type OverviewTab } from '#lib/insights/tabs.ts';
import OverviewTabs from '#lib/overview/OverviewTabs.svelte';
import type { PanelName } from '#lib/shared/usage-actions.ts';
import { availability } from '#lib/sources/capabilities.ts';
import { bucket, overview, prompt, source, stats } from '#lib/testing/builders.ts';

const link = () => '/prompts';
const connected = [source({ connected: true })];
const canWith = (sources: Source[]) => (c: Capability) => availability(sources, c);

function draw(selected: OverviewTab, data: Overview = overview(), sources = connected): string {
	const props = {
		selected,
		onselect: () => {},
		data,
		link,
		slots: [],
		can: canWith(sources),
		weekStart: 'monday' as const
	};
	return render(OverviewTabs, { props }).body;
}

const anchorsIn = (html: string) =>
	OVERVIEW_TABS.flatMap((t) => t.panels.map((p) => p.panel)).filter((p) =>
		html.includes(`id="${panelAnchor(p)}"`)
	);

describe('OverviewTabs', () => {
	it('draws the five tabs, each saying what it holds', () => {
		const html = draw('activity');
		for (const tab of OVERVIEW_TABS) {
			expect(html).toContain(tab.label);
			expect(html).toContain(tab.hint);
		}
	});

	// No browser runs here, so "two screens tall at 1280×800" is checked by its cause: only the open
	// tab's panels are on the page, whatever tab that is.
	it('draws only the open tab’s panels', () => {
		for (const tab of OVERVIEW_TABS) {
			expect(anchorsIn(draw(tab.id))).toEqual(tab.panels.map((p) => p.panel));
		}
	});

	it('opens on the takeaway, then "In this tab" with a jump link per panel, then the panels', () => {
		const data = overview({ starred: [prompt(), prompt(), prompt()] });
		const html = draw('starred', data);
		const said = html.indexOf('3 prompts starred in this window.');
		const jump = html.indexOf('In this tab');
		const panel = html.indexOf(`id="${panelAnchor('starred')}"`);
		expect(said).toBeGreaterThan(-1);
		expect(said).toBeLessThan(jump);
		expect(jump).toBeLessThan(panel);
		expect(html).toContain(`href="#${panelAnchor('starred')}"`);
		expect(html).toContain('Which prompts did I star?');
	});

	it('lists each panel’s question in "In this tab", per hour over 24 hours', () => {
		const html = draw('cost', overview({ hourly: true }));
		expect(html).toContain('What does each hour cost?');
		expect(html).toContain('Where does the money go?');
	});

	it('says in place when no tool in use reports the tab’s data, and keeps its panels', () => {
		const html = draw('cost', overview(), [source({ connected: true, capabilities: ['prompt'] })]);
		expect(html).toContain('Costs not available');
		expect(anchorsIn(html)).toEqual(['spend', 'models', 'activity']);
	});

	it('says no takeaway when the tab’s numbers say nothing', () => {
		expect(draw('agentic')).not.toContain('class="takeaway');
	});
});

/** Each panel with the props the page gives it, over numbers that make it draw its content. */
const busy = overview({
	activity: stats({
		totals: bucket({ prompts: 3, sessions: 1, cost_usd: 1, ended: 1, tool_calls: 2 })
	}),
	week: stats({ totals: bucket({ prompts: 3 }) }),
	origins: stats({ totals: bucket({ prompts: 3 }) }),
	starred: [prompt()]
});
const can = canWith(connected);
/** Each panel drawn as the page draws it: its HTML. */
const DRAW: Record<PanelName, (() => string) | null> = {
	'key-figures': null,
	rhythm: () => render(RhythmPanel, { props: { data: busy, link, weekStart: 'monday' } }).body,
	sessions: () => render(SessionsPanel, { props: { data: busy } }).body,
	origin: () => render(OriginPanel, { props: { data: busy, link } }).body,
	commands: () =>
		render(CommandsPanel, {
			props: { data: busy, link, availability: can('session_metrics') }
		}).body,
	writing: () => render(WritingPanel, { props: { data: busy, link } }).body,
	reuse: () => render(ReusePanel, { props: { data: busy, link } }).body,
	spend: () => render(SpendPanel, { props: { data: busy, link, availability: can('cost') } }).body,
	models: () =>
		render(ModelsPanel, { props: { data: busy, link, availability: can('model') } }).body,
	activity: () =>
		render(ActivityPanel, {
			props: { data: busy, slots: [], link, availability: can('cost') }
		}).body,
	agent: () => render(AgentPanel, { props: { data: busy, link } }).body,
	delegation: () => render(DelegationPanel, { props: { data: busy, link } }).body,
	performance: () => render(PerformancePanel, { props: { data: busy, link } }).body,
	outcomes: () =>
		render(OutcomesPanel, {
			props: { data: busy, link, availability: can('session_metrics') }
		}).body,
	starred: () => render(StarredPanel, { props: { prompts: busy.starred, href: link() } }).body
};

describe('every panel', () => {
	it('is titled with its question from the tab table', () => {
		for (const tab of OVERVIEW_TABS) {
			for (const { panel, question } of tab.panels) {
				const html = DRAW[panel]?.() ?? '';
				expect(html, panel).toContain(question);
			}
		}
	});
});
