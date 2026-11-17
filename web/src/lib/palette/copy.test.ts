import { describe, expect, it } from 'vitest';
import { copiedMessage, copyText, type CopyDeps } from '#lib/palette/copy.ts';

function deps(write: CopyDeps['write'] = async () => {}) {
	const log: string[] = [];
	const d: CopyDeps = {
		write: async (text) => {
			log.push(`write ${(await text).length}`);
			await write(text);
		},
		say: (message, tone) => void log.push(`${tone}: ${message}`),
		count: () => void log.push('count')
	};
	return { d, log };
}

describe('copying from the palette', () => {
	it('says the size with a thousands separator', () => {
		expect(copiedMessage('x'.repeat(1240))).toBe('Copied · 1,240 characters');
		expect(copiedMessage('x')).toBe('Copied · 1 character');
	});

	it('counts characters, not UTF-16 units', () => {
		expect(copiedMessage('😀😀')).toBe('Copied · 2 characters');
	});

	it('writes, counts, then confirms', async () => {
		const { d, log } = deps();
		expect(await copyText('hello', d)).toBe(true);
		expect(log).toEqual(['write 5', 'count', 'info: Copied · 5 characters']);
	});

	it('starts the write before the text has arrived, so it stays inside the key press', async () => {
		const { d, log } = deps();
		let arrive: (text: string) => void = () => {};
		const text = new Promise<string>((resolve) => (arrive = resolve));
		const started: unknown[] = [];
		const copying = copyText(text, { ...d, write: (t) => (started.push(t), d.write(t)) });
		expect(started).toEqual([text]);
		arrive('hello');
		expect(await copying).toBe(true);
		expect(log).toEqual(['write 5', 'count', 'info: Copied · 5 characters']);
	});

	it('passes on a text that never arrives, and says nothing about the clipboard', async () => {
		const { d, log } = deps();
		await expect(copyText(Promise.reject(new Error('offline')), d)).rejects.toThrow('offline');
		expect(log).toEqual([]);
	});

	it('says a refusal, and does not count it', async () => {
		const { d, log } = deps(() => Promise.reject(new Error('denied')));
		expect(await copyText('hello', d)).toBe(false);
		expect(log).toEqual(['write 5', 'error: Could not copy: the browser refused clipboard access']);
	});
});
