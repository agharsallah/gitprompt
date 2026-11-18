import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Source, Stats } from '#lib/api/types.ts';
import { parseRange } from '#lib/insights/range.ts';
import { availability } from '#lib/sources/capabilities.ts';
import { bucket, redone, source, stats } from '#lib/testing/builders.ts';
import KeyFigures from './KeyFigures.svelte';

/** The tiles drawn for these stats, with the tools in `sources` connected. */
function draw(keyFigures: Stats, sources: Source[]): string {
	const can = {
		cost: availability(sources, 'cost'),
		turnEnd: availability(sources, 'turn_end'),
		tools: availability(sources, 'tools')
	};
	const props = {
		stats: keyFigures,
		repos: stats(),
		range: parseRange('7d'),
		slots: [],
		link: () => '/prompts',
		can
	};
	return render(KeyFigures, { props }).body;
}

/** Claude Code's hooks, connected: prompts, turn ends and tools, but no cost. */
const hooksOnly = [source({ connected: true })];

describe('KeyFigures', () => {
	it('says spend is not reported by a tool without cost, never $0', () => {
		const html = draw(stats({ totals: bucket({ prompts: 3 }) }), hooksOnly);
		expect(html).toContain('Not reported');
		expect(html).not.toContain('$0');
	});

	it('says "Nothing redone" for a period with none', () => {
		const html = draw(stats({ redone: redone() }), hooksOnly);
		expect(html).toContain('Nothing redone');
		expect(html).toContain('None found');
	});

	it('draws redone work amber, as cost', () => {
		const work = redone({ turns: 1, agent_time_s: 600, cost_usd: 2, failed: { turns: 1 } });
		const html = draw(stats({ redone: work }), hooksOnly);
		expect(html).toContain('$2.00 and 10m 00s redone');
		expect(html).toContain('tone-warning');
	});
});
