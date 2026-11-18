import { describe, expect, it } from 'vitest';
import {
	DEFAULT_TAB,
	OVERVIEW_TAB_IDS,
	OVERVIEW_TABS,
	overviewTabHref,
	panelAnchor,
	questionOf,
	resolveTab,
	tabEntry,
	tabNotice
} from '#lib/insights/tabs.ts';
import {
	keepOverviewTab,
	OVERVIEW_TAB_KEY,
	REMEMBER_KEY,
	savedOverviewTab
} from '#lib/settings/views.ts';
import { pageHref } from '#lib/shared/links.ts';
import { OVERVIEW_TABS as COUNTED_TABS, PANELS } from '#lib/shared/usage-actions.ts';
import { availability } from '#lib/sources/capabilities.ts';
import { source } from '#lib/testing/builders.ts';
import { blockedStorage, memoryStorage } from '#lib/testing/storage.ts';
import { tabKeyTarget } from '#lib/ui/tabs.ts';

/**
 * The 14 panels the Overview drew below its first screen before the tabs (routes/+page.svelte),
 * and the §08 checklist row "14 panels in 4 sections → 5 tabs": each lands in exactly one tab.
 * The key figures stay on the first screen.
 */
const OVERVIEW_PANELS = [
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
];

/** The questions of the A2 sketch's table (docs/redesign/index.html), per tab, for today's panels. */
const A2_QUESTIONS = {
	activity: ['When in the week do I prompt?', 'How long do my sessions last?'],
	types: [
		'Who wrote these prompts?',
		'Which slash commands do I run?',
		"How long are my prompts, and what's in them?",
		'Which prompts do I send again?'
	],
	cost: ['Where does the money go?', 'Which model do I use most?', 'What does each day cost?'],
	agentic: [
		'What does the agent do?',
		'How much goes to subagents and MCP servers?',
		'How fast and reliable are turns?',
		'What came of my prompts?'
	],
	starred: ['Which prompts did I star?']
};

const placed = OVERVIEW_TABS.flatMap((t) => t.panels.map((p) => p.panel));

describe('the tab table', () => {
	it('holds every Overview panel of the §08 checklist exactly once', () => {
		expect([...placed].sort()).toEqual([...OVERVIEW_PANELS].sort());
		expect(new Set(placed).size).toBe(placed.length);
	});

	it('holds every counted panel but the key figures, which stay on the first screen', () => {
		expect([...placed].sort()).toEqual(PANELS.filter((p) => p !== 'key-figures').sort());
	});

	it('is the five tabs of the sketch, in its order, each counted when opened', () => {
		expect(OVERVIEW_TABS.map((t) => t.label)).toEqual([
			'Activity',
			'Prompt types',
			'Models and cost',
			'Agentic work',
			'Starred'
		]);
		expect(OVERVIEW_TAB_IDS).toEqual([...COUNTED_TABS]);
	});

	it('titles each panel with the question of the A2 table', () => {
		for (const tab of OVERVIEW_TABS) {
			expect(tab.panels.map((p) => p.question)).toEqual(A2_QUESTIONS[tab.id]);
		}
	});

	it('asks per hour over 24 hours where a panel counts hours', () => {
		const daily = tabEntry('cost').panels.find((p) => p.panel === 'activity')!;
		expect(questionOf(daily, false)).toBe('What does each day cost?');
		expect(questionOf(daily, true)).toBe('What does each hour cost?');
		const rhythm = tabEntry('activity').panels[0];
		expect(questionOf(rhythm, true)).toBe(rhythm.question);
	});

	it('gives every panel its own anchor', () => {
		const anchors = placed.map(panelAnchor);
		expect(new Set(anchors).size).toBe(anchors.length);
		expect(panelAnchor('rhythm')).toBe('overview-rhythm');
	});
});

describe('which tab opens', () => {
	it('the URL wins over the tab remembered', () => {
		expect(resolveTab('agentic', 'cost')).toBe('agentic');
	});

	it('the tab remembered wins over the default', () => {
		expect(resolveTab(null, 'cost')).toBe('cost');
	});

	it('Activity without either', () => {
		expect(resolveTab(null, null)).toBe(DEFAULT_TAB);
		expect(DEFAULT_TAB).toBe('activity');
	});

	it('an unknown ?tab= falls back to the one remembered, then to the default', () => {
		expect(resolveTab('nonsense', 'starred')).toBe('starred');
		expect(resolveTab('toString', 'constructor')).toBe('activity');
		expect(resolveTab('', '')).toBe('activity');
	});

	it('remembers the tab used last in this browser, unless remembering is off', () => {
		const storage = memoryStorage();
		keepOverviewTab('types', storage);
		expect(resolveTab(null, savedOverviewTab(storage))).toBe('types');
		expect(storage.items.get(OVERVIEW_TAB_KEY)).toBe('types');

		const off = memoryStorage({ [REMEMBER_KEY]: 'off', [OVERVIEW_TAB_KEY]: 'cost' });
		keepOverviewTab('types', off);
		expect(off.items.get(OVERVIEW_TAB_KEY)).toBe('cost');
		expect(resolveTab(null, savedOverviewTab(off))).toBe('activity');
	});

	it('opens on the default when storage is blocked', () => {
		const storage = blockedStorage();
		expect(() => keepOverviewTab('cost', storage)).not.toThrow();
		expect(resolveTab(null, savedOverviewTab(storage))).toBe('activity');
	});

	it('links to a tab with the filters kept', () => {
		const href = overviewTabHref('?range=30d&tab=types&model=opus', 'starred');
		expect(href).toBe(`${pageHref('/')}?range=30d&tab=starred&model=opus`);
		expect(overviewTabHref('', 'activity')).toBe(`${pageHref('/')}?tab=activity`);
	});
});

describe('moving between tabs with the keyboard', () => {
	const tabs = OVERVIEW_TABS.map(() => ({}));
	const order = (key: string, from: number) => tabKeyTarget(key, from, tabs);

	it('→ goes to the next tab in the order shown, and from the last back to the first', () => {
		const visited = [0];
		for (let i = 0; i < OVERVIEW_TABS.length; i++)
			visited.push(order('ArrowRight', visited.at(-1)!)!);
		expect(visited.map((i) => OVERVIEW_TAB_IDS[i])).toEqual([
			'activity',
			'types',
			'cost',
			'agentic',
			'starred',
			'activity'
		]);
	});

	it('← goes back, and from the first round to the last', () => {
		expect(OVERVIEW_TAB_IDS[order('ArrowLeft', 0)!]).toBe('starred');
		expect(OVERVIEW_TAB_IDS[order('ArrowLeft', 2)!]).toBe('types');
	});
});

describe('a tab whose data no tool reports', () => {
	const can =
		(sources = [source({ connected: true })]) =>
		(c: Parameters<typeof availability>[1]) =>
			availability(sources, c);

	it('says so in place of its takeaway', () => {
		const notice = tabNotice(tabEntry('cost'), can());
		expect(notice?.title).toBe('Costs not available');
	});

	it('says nothing when a tool reports it, or when the tab needs nothing special', () => {
		expect(tabNotice(tabEntry('agentic'), can())).toBeNull();
		expect(tabNotice(tabEntry('activity'), can())).toBeNull();
	});
});
