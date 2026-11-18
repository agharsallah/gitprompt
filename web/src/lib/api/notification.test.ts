import { describe, expect, it } from 'vitest';
import { parseNotification } from '#lib/api/notification.ts';

const data = (value: unknown) => JSON.stringify(value);

describe('parseNotification', () => {
	it('reads a notification as the daemon sends it', () => {
		const sent = {
			kind: 'long_turn',
			title: 'A long turn finished',
			body: 'fix the parser ran for 12 min',
			target: { page: 'prompt', id: 7 }
		};
		expect(parseNotification(data(sent))).toEqual(sent);
	});

	it('keeps each target it knows', () => {
		const targets = [
			{ page: 'leaks' },
			{ page: 'source', id: 'claude-code-otel' },
			{ page: 'session', id: 's1' },
			{ page: 'prompts', q: 'lint' }
		];
		for (const target of targets) {
			const parsed = parseNotification(
				data({ kind: 'possible_leak', title: 't', body: 'b', target })
			);
			expect(parsed?.target).toEqual(target);
		}
	});

	it('leads a target it does not know, or a malformed one, to the Overview', () => {
		for (const target of [{ page: 'later' }, { page: 'prompt', id: 'x' }, 'leaks', null]) {
			const parsed = parseNotification(
				data({ kind: 'quiet_connector', title: 't', body: 'b', target })
			);
			expect(parsed?.target).toEqual({ page: 'overview' });
		}
	});

	it('turns away what is not a notification', () => {
		const notifications = [
			'not json',
			data([1]),
			data({ prompts: 1, events: 0 }),
			data({ kind: 'later', title: 't', body: 'b', target: { page: 'leaks' } }),
			data({ kind: 'long_turn', title: 1, body: 'b', target: { page: 'leaks' } })
		];
		for (const text of notifications) expect(parseNotification(text)).toBeNull();
	});
});
