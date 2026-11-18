import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Toasts, TOAST_MS } from '#lib/ui/toast.svelte.ts';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('toasts', () => {
	it('shows a message and takes it away by itself', () => {
		const toasts = new Toasts();
		toasts.show('Copied · 5 characters');
		expect(toasts.items.map((t) => [t.message, t.tone])).toEqual([
			['Copied · 5 characters', 'info']
		]);
		vi.advanceTimersByTime(TOAST_MS);
		expect(toasts.items).toEqual([]);
	});

	it('keeps each for its own time and can be told the tone', () => {
		const toasts = new Toasts();
		toasts.show('first', { ms: 100 });
		toasts.show('second', { tone: 'error', ms: 500 });
		vi.advanceTimersByTime(100);
		expect(toasts.items.map((t) => t.message)).toEqual(['second']);
		expect(toasts.items[0].tone).toBe('error');
		toasts.dismiss(toasts.items[0].id);
		expect(toasts.items).toEqual([]);
	});

	it('carries a detail line and a link when given', () => {
		const toasts = new Toasts();
		toasts.show('A possible leak', { detail: 'OpenAI API key sk-…y6Hd', href: '/#leaks' });
		expect(toasts.items.map(({ message, detail, href }) => [message, detail, href])).toEqual([
			['A possible leak', 'OpenAI API key sk-…y6Hd', '/#leaks']
		]);
	});
});
