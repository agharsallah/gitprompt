import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NOTIFICATION_MS, showNotification } from '#lib/shared/notifications.ts';
import { promptHref } from '#lib/shared/links.ts';
import { Toasts } from '#lib/ui/toast.svelte.ts';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('showNotification', () => {
	it('shows the title, the body and a link to the target, for longer than a confirmation', () => {
		const toasts = new Toasts();
		showNotification(toasts, {
			kind: 'long_turn',
			title: 'A long turn finished',
			body: 'fix the parser ran for 12 min',
			target: { page: 'prompt', id: 7 }
		});
		const shown = toasts.items.map(({ message, detail, href }) => ({ message, detail, href }));
		expect(shown).toEqual([
			{
				message: 'A long turn finished',
				detail: 'fix the parser ran for 12 min',
				href: promptHref(7)
			}
		]);
		vi.advanceTimersByTime(NOTIFICATION_MS - 1);
		expect(toasts.items).toHaveLength(1);
		vi.advanceTimersByTime(1);
		expect(toasts.items).toEqual([]);
	});
});
