import { afterEach, describe, expect, it, vi } from 'vitest';
import { copyCounted, writeClipboard } from '#lib/shared/clipboard.ts';
import type { UsageAction } from '#lib/shared/usage-actions.ts';

type Parts = Record<string, Promise<Blob>>;

/** A clipboard that remembers what it was handed, and a ClipboardItem that keeps its promise. */
function stubClipboard(options: { items?: boolean; refuse?: boolean } = {}) {
	const texts: string[] = [];
	const items: Parts[] = [];
	const refuse = () => Promise.reject(new Error('denied'));
	vi.stubGlobal('navigator', {
		clipboard: {
			writeText: (text: string) =>
				options.refuse ? refuse() : Promise.resolve(void texts.push(text)),
			write: (list: { parts: Parts }[]) => {
				items.push(...list.map((item) => item.parts));
				return options.refuse ? refuse() : Promise.resolve();
			}
		}
	});
	if (options.items === false) {
		vi.stubGlobal('ClipboardItem', undefined);
	} else {
		vi.stubGlobal(
			'ClipboardItem',
			class {
				constructor(readonly parts: Parts) {}
			}
		);
	}
	return { texts, items };
}

afterEach(() => vi.unstubAllGlobals());

describe('writeClipboard', () => {
	it('writes a text that is here as text', async () => {
		const { texts } = stubClipboard();
		await writeClipboard('hello');
		expect(texts).toEqual(['hello']);
	});

	it('starts at once on a promised text, handing the clipboard the promise itself', async () => {
		const { texts, items } = stubClipboard();
		let arrive: (text: string) => void = () => {};
		const writing = writeClipboard(new Promise<string>((resolve) => (arrive = resolve)));
		expect(items).toHaveLength(1);
		arrive('later');
		await writing;
		expect(await items[0]['text/plain'].then((blob) => blob.text())).toBe('later');
		expect(texts).toEqual([]);
	});

	it('waits for a promised text where there is no ClipboardItem', async () => {
		const { texts } = stubClipboard({ items: false });
		await writeClipboard(Promise.resolve('later'));
		expect(texts).toEqual(['later']);
	});
});

describe('copyCounted', () => {
	it('counts the copy once the text is on the clipboard', async () => {
		const { texts } = stubClipboard();
		const counted: UsageAction[] = [];
		expect(await copyCounted('hello', 'copy.prompt', (a) => void counted.push(a))).toBe(true);
		expect([texts, counted]).toEqual([['hello'], ['copy.prompt']]);
	});

	it('counts nothing when the clipboard refuses', async () => {
		stubClipboard({ refuse: true });
		const counted: UsageAction[] = [];
		expect(await copyCounted('hello', 'copy.prompt', (a) => void counted.push(a))).toBe(false);
		expect(counted).toEqual([]);
	});
});
