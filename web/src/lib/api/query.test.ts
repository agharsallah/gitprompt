import { describe, expect, it } from 'vitest';
import { queryParams } from '#lib/api/query.ts';

describe('queryParams', () => {
	it('repeats list values and leaves out empty ones', () => {
		const params = queryParams({
			origin: ['typed', 'slash_command'],
			model: [],
			q: '',
			repo: '/r/app',
			group_by: 'day',
			since: undefined,
			limit: 10
		});
		expect(params.getAll('origin')).toEqual(['typed', 'slash_command']);
		expect(params.toString()).toBe(
			'origin=typed&origin=slash_command&repo=%2Fr%2Fapp&group_by=day&limit=10'
		);
	});

	it('is empty without filters (the daemon then counts every prompt)', () =>
		expect(queryParams({}).toString()).toBe(''));
});
