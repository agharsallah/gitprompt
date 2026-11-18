import { describe, expect, it } from 'vitest';
import { clearedHref, filterChips, foldLabel } from '#lib/insights/filterChips.ts';
import {
	activeFilters,
	DIMENSIONS,
	filterParams,
	readFilters,
	toQuery,
	type InsightFilters
} from '#lib/insights/filters.ts';
import { bucket, stats } from '#lib/testing/builders.ts';

const NOW = Date.parse('2026-11-18T12:00:00Z');
const REMEMBERED = ['typed', 'slash_command'] as const;
const read = (search: string) => readFilters(new URLSearchParams(search), '7d', [...REMEMBERED]);
const here = (search: string) => ({ pathname: '/', search: search ? `?${search}` : '' });

/** Every filter URL the Overview has written so far: the range, origins and the four selects. */
const CANONICAL = [
	'range=24h&origin=typed',
	'range=7d&origin=typed&origin=slash_command',
	'range=30d&origin=task_notification',
	'range=all&origin=typed&source=claude-code',
	'range=7d&origin=typed&surface=desktop',
	'range=7d&origin=typed&repo=%2Fw%2Fapp',
	'range=7d&origin=typed&model=claude-opus-4',
	'range=30d&origin=typed&origin=slash_command&source=codex&surface=cli&repo=%2Fw%2Fapi&model=gpt-5'
];

/** URLs as people type or paste them: missing range or origin, unknown values, other pages' params. */
const LOOSE = [
	'',
	'repo=/w/app',
	'origin=bogus&model=opus',
	'range=nope&source=',
	'outcome=failed&has=code&command=review&tool=Bash&q=fix&repo=/w/app',
	'range=24h&origin=typed&q=hello&tag=x'
];

describe('the filter URL: parse and serialize agree', () => {
	it.each(CANONICAL)('%s reads back to the same URL', (url) => {
		expect(filterParams(read(url)).toString()).toBe(url);
	});

	it.each(CANONICAL)('%s gives the same state after a round trip', (url) => {
		const state = read(url);
		expect(read(filterParams(state).toString())).toEqual(state);
	});

	it.each(LOOSE)('%s asks the daemon the same after a round trip', (url) => {
		const state = read(url);
		const again = read(filterParams(state).toString());
		expect(toQuery(again, NOW)).toEqual(toQuery(state, NOW));
		for (const d of DIMENSIONS) expect(again[d]).toBe(state[d]);
	});

	it('keeps only the Overview filters: other pages’ parameters are not part of its state', () => {
		const state = read('outcome=failed&has=code&command=review&tool=Bash&q=fix');
		expect(Object.keys(state).sort()).toEqual(['origins', 'range']);
	});
});

describe('the folded bar: chips from the same state', () => {
	it.each([...CANONICAL, ...LOOSE])('%s: one chip per filter on, none lost', (url) => {
		const state = read(url);
		const chips = filterChips(state, {}, here(url));
		const on = DIMENSIONS.filter((d) => state[d]);
		expect(chips.map((c) => c.dimension)).toEqual(on);
		expect(chips).toHaveLength(activeFilters(state).length);
	});

	it('counts on the button only what is not range or origin', () => {
		const state = read('range=30d&origin=typed&repo=/w/app&model=opus');
		expect(foldLabel(filterChips(state, {}, here('')).length)).toBe('Filters · 2');
		expect(foldLabel(0)).toBe('Filters');
	});

	it('names a chip by the facet and the daemon’s name for the value', () => {
		const options = { repo: stats({ groups: [bucket({ key: '/w/app', detail: 'me/app' })] }) };
		const state: InsightFilters = { ...read(''), repo: '/w/app', model: 'claude-opus-4-1' };
		const labels = filterChips(state, options, here('')).map((c) => c.label);
		expect(labels[0]).toBe('Project: me/app');
		expect(labels[1]).toMatch(/^Model: /);
	});

	it('removes one filter and keeps the rest of the URL', () => {
		const url = 'range=30d&origin=typed&repo=%2Fw%2Fapp&model=opus';
		const [repo, model] = filterChips(read(url), {}, here(url));
		expect(repo.href).toBe('/?range=30d&origin=typed&model=opus');
		expect(model.href).toBe('/?range=30d&origin=typed&repo=%2Fw%2Fapp');
	});

	it('clears every chip and keeps range and origins as they were in the URL', () => {
		const url = 'origin=typed&source=codex&repo=%2Fw';
		expect(clearedHref(here(url), read(url))).toBe('/?origin=typed');
	});
});
