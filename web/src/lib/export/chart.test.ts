import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { png, save, svgSize } from '#lib/export/chart.ts';

describe('svgSize', () => {
	it('reads the size the root element declares', () => {
		expect(
			svgSize('<svg xmlns="x" width="800" height="412.5"><rect width="5" height="5"/></svg>')
		).toEqual({
			width: 800,
			height: 412.5
		});
	});

	it('ignores sizes of inner elements and lookalike attributes', () => {
		expect(
			svgSize('<svg stroke-width="3" data-height="9"><rect width="5" height="5"/></svg>')
		).toEqual({
			width: 640,
			height: 320
		});
	});

	it('falls back to the default chart size for relative or missing sizes', () => {
		expect(svgSize('<svg width="100%" height="auto"></svg>')).toEqual({ width: 640, height: 320 });
		expect(svgSize('not an svg')).toEqual({ width: 640, height: 320 });
	});
});

/** The browser pieces `save` and `png` use, recorded (the tests run without a DOM). */
class FakeImage {
	static made: FakeImage[] = [];
	static fails = false;
	src = '';
	constructor() {
		FakeImage.made.push(this);
	}
	decode() {
		return FakeImage.fails ? Promise.reject(new Error('bad svg')) : Promise.resolve();
	}
}

type FakeElement = Record<string, unknown> & { tag: string };

function fakeDocument(drawn: Blob | null) {
	const appended: FakeElement[] = [];
	const events: string[] = [];
	const drawImage = vi.fn();
	const createElement = (tag: string): FakeElement => {
		const element: FakeElement = {
			tag,
			click: () => events.push(`click ${String(element.download)}`),
			remove: () => events.push('remove'),
			getContext: (kind: string) => (kind === '2d' ? { drawImage } : null),
			toBlob: (done: (b: Blob | null) => void, type: string) => {
				events.push(`toBlob ${type}`);
				done(drawn);
			}
		};
		return element;
	};
	const body = { append: (e: FakeElement) => appended.push(e) };
	return { document: { createElement, body }, appended, events, drawImage };
}

let blobs: Blob[];
let revoked: string[];

beforeEach(() => {
	blobs = [];
	revoked = [];
	FakeImage.made = [];
	FakeImage.fails = false;
	vi.spyOn(URL, 'createObjectURL').mockImplementation((b) => {
		blobs.push(b as Blob);
		return `blob:${blobs.length}`;
	});
	vi.spyOn(URL, 'revokeObjectURL').mockImplementation((url) => revoked.push(url));
	vi.stubGlobal('Image', FakeImage);
});
afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe('save', () => {
	it('downloads the blob under its name, then frees it', () => {
		vi.useFakeTimers();
		const dom = fakeDocument(null);
		vi.stubGlobal('document', dom.document);
		const blob = new Blob(['<svg/>']);

		save(blob, 'chart.svg');
		expect(blobs).toEqual([blob]);
		expect(dom.appended).toMatchObject([{ tag: 'a', href: 'blob:1', download: 'chart.svg' }]);
		expect(dom.events).toEqual(['click chart.svg', 'remove']);
		// The browser reads the URL after the click returns: freed a moment later, not at once.
		expect(revoked).toEqual([]);
		vi.advanceTimersByTime(1000);
		expect(revoked).toEqual(['blob:1']);
	});
});

describe('png', () => {
	const svg = '<svg width="300.4" height="100.3"></svg>';

	it('draws the SVG at its declared size times the scale', async () => {
		const drawn = new Blob(['png']);
		const dom = fakeDocument(drawn);
		vi.stubGlobal('document', dom.document);

		await expect(png(svg, 3)).resolves.toBe(drawn);
		expect(blobs[0].type).toBe('image/svg+xml');
		await expect(blobs[0].text()).resolves.toBe(svg);
		expect(FakeImage.made.map((i) => i.src)).toEqual(['blob:1']);
		const [image, x, y, width, height] = dom.drawImage.mock.calls[0];
		expect(image).toBe(FakeImage.made[0]);
		expect([x, y, width, height]).toEqual([0, 0, 901, 301]);
		expect(dom.events).toEqual(['toBlob image/png']);
		expect(revoked).toEqual(['blob:1']);
	});

	it('draws at twice the size by default', async () => {
		const dom = fakeDocument(new Blob(['png']));
		vi.stubGlobal('document', dom.document);
		await png(svg);
		expect(dom.drawImage.mock.calls[0].slice(3)).toEqual([601, 201]);
	});

	it('fails when the canvas cannot be encoded, and frees the SVG', async () => {
		vi.stubGlobal('document', fakeDocument(null).document);
		await expect(png(svg)).rejects.toThrow('could not draw the chart');
		expect(revoked).toEqual(['blob:1']);
	});

	it('fails when the SVG cannot be decoded, and frees it', async () => {
		FakeImage.fails = true;
		const dom = fakeDocument(new Blob(['png']));
		vi.stubGlobal('document', dom.document);
		await expect(png(svg)).rejects.toThrow('bad svg');
		expect(dom.drawImage).not.toHaveBeenCalled();
		expect(revoked).toEqual(['blob:1']);
	});
});
