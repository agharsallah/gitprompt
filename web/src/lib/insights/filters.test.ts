import { describe, expect, it } from 'vitest';
import { promptsHref, readFilters, toQuery, without } from '#lib/insights/filters.ts';
import { ALL_ORIGINS } from '#lib/prompts/originFilter.svelte.ts';

const NOW = Date.parse('2026-11-07T12:00:00Z');
const read = (search: string) =>
	readFilters(new URLSearchParams(search), '7d', ['typed', 'slash_command']);

describe('readFilters', () => {
	it('falls back to the default range and the remembered origins', () => {
		expect(read('')).toEqual({
			range: expect.objectContaining({ id: '7d' }),
			origins: ['typed', 'slash_command']
		});
	});

	it('reads range, origins and dimensions from the URL', () => {
		const f = read(
			'range=30d&origin=task_notification&origin=bogus&repo=/r/app&model=opus&source='
		);
		expect(f.range.id).toBe('30d');
		expect(f.origins).toEqual(['task_notification']);
		expect(f.repo).toBe('/r/app');
		expect(f.model).toBe('opus');
		expect(f.source).toBeUndefined();
	});
});

describe('toQuery', () => {
	it('turns the range into an instant and the model into a list', () => {
		expect(toQuery(read('model=opus'), NOW)).toEqual({
			since: '2026-10-31T12:00:00.000Z',
			origin: ['typed', 'slash_command'],
			source: undefined,
			repo: undefined,
			model: ['opus']
		});
	});

	it('sends no origin and no start for every origin and all time', () => {
		const q = toQuery(read(`range=all&${ALL_ORIGINS.map((o) => `origin=${o}`).join('&')}`), NOW);
		expect(q.since).toBeUndefined();
		expect(q.origin).toEqual([]);
	});

	it('drops one filter to count the choices of that filter', () => {
		expect(without({ repo: '/r', model: ['x'] }, 'repo')).toEqual({ model: ['x'] });
	});
});

describe('promptsHref', () => {
	it('links to the prompts behind a number, with the same filters', () => {
		const href = promptsHref(
			'/prompts',
			{ since: 'S', origin: ['typed'], repo: '/r' },
			{ outcome: ['failed'] }
		);
		expect(new URLSearchParams(href.split('?')[1]).toString()).toBe(
			'since=S&origin=typed&repo=%2Fr&outcome=failed'
		);
	});

	it('names every origin when the view shows them all, and lets the extra filters win', () => {
		const params = new URLSearchParams(
			promptsHref('/prompts', { since: 'S' }, { since: 'D' }).split('?')[1]
		);
		expect(params.get('since')).toBe('D');
		expect(params.getAll('origin')).toEqual(ALL_ORIGINS);
	});
});
