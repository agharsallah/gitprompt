import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Overview } from '#lib/insights/overview.ts';
import { parseRange } from '#lib/insights/range.ts';
import WeekStrip from '#lib/overview/WeekStrip.svelte';
import { sessionsHref } from '#lib/shared/links.ts';
import { overview, splitDay, stats } from '#lib/testing/builders.ts';

const NOW = new Date(2026, 10, 18, 12).getTime();
const draw = (o: Overview) => render(WeekStrip, { props: { data: o } }).body;

/** The table view's rows, in order: [day link, total, parts]. */
function rows(html: string): string[][] {
	return [...html.matchAll(/<tr>([\s\S]*?)<\/tr>/g)].map(([, row]) =>
		[...row.matchAll(/<t[hd][^>]*>([\s\S]*?)<\/t[hd]>/g)].map(([, cell]) =>
			cell.replace(/<!--.*?-->/g, '').trim()
		)
	);
}

const week = (
	groups = [
		splitDay('2026-11-16', [
			['/w/app', 2],
			['/w/api', 1]
		]),
		splitDay('2026-11-17', [['/w/api', 2]]),
		splitDay('2026-11-18', [
			['/w/app', 3],
			['/w/api', 9]
		])
	]
) => overview({ strip: stats({ group_by: 'day', split_by: 'repo', groups }) }, NOW);

describe('WeekStrip', () => {
	it('lists each day with its prompts per project, stacked in the series order', () => {
		const table = rows(draw(week())).slice(-3);
		expect(table.map((r) => r.slice(1))).toEqual([
			['3 prompts', 'api 1 prompt, app 2 prompts'],
			['2 prompts', 'api 2 prompts'],
			['12 prompts', 'api 9 prompts, app 3 prompts']
		]);
	});

	it('makes every day a doorway to its sessions', () => {
		const html = draw(week());
		for (const day of ['2026-11-16', '2026-11-17', '2026-11-18'])
			expect(html).toContain(`href="${sessionsHref({ day })}"`);
	});

	it('keeps the chosen project on the way to the day', () => {
		const o = week();
		const html = draw({ ...o, filters: { ...o.filters, repo: '/w/app' } });
		expect(html).toContain(
			sessionsHref({ day: '2026-11-18', project: '/w/app' }).replaceAll('&', '&amp;')
		);
	});

	it('says the streaks, names the projects and the day that stands out', () => {
		const html = draw(week());
		expect(html).toContain('Each day, by project');
		expect(html).toMatch(/longest streak 3 days/);
		expect(html).toContain('aria-label="Projects"');
		expect(html).toContain('Today stands out: 12 prompts');
	});

	it('shows hours over 24 hours, without a day streak', () => {
		const o = overview({ filters: { range: parseRange('24h'), origins: [] }, strip: stats() }, NOW);
		const html = draw(o);
		expect(html).toContain('Each hour, by project');
		expect(html).not.toContain('streak');
	});
});
