import { afterEach, describe, expect, it, vi } from 'vitest';
import { clickAction, clickThrough } from '#lib/insights/counting.ts';

const tracked = vi.hoisted(() => [] as string[]);
vi.mock('#lib/shared/usage.ts', () => ({ track: (action: string) => tracked.push(action) }));

describe('clickAction', () => {
	it('counts a link to the prompts under the panel it is in', () => {
		expect(clickAction('/prompts?origin=human&range=7d', 'rhythm')).toBe('panel.click.rhythm');
		expect(clickAction('/prompts', 'key-figures')).toBe('panel.click.key-figures');
		expect(clickAction('http://127.0.0.1:7878/prompts/?starred=true', 'starred')).toBe(
			'panel.click.starred'
		);
	});

	it('counts nothing for a link to another page', () => {
		expect(clickAction('/sessions/abc', 'sessions')).toBeUndefined();
		expect(clickAction('/prompts-old', 'sessions')).toBeUndefined();
		expect(clickAction('/settings#data', 'spend')).toBeUndefined();
	});

	it('counts nothing outside a known panel', () => {
		expect(clickAction('/prompts?q=x', undefined)).toBeUndefined();
		expect(clickAction('/prompts?q=x', 'not-a-panel')).toBeUndefined();
	});

	it('follows the base path the app is served under', () => {
		expect(clickAction('/gp/prompts?q=x', 'rhythm', '/gp/prompts')).toBe('panel.click.rhythm');
		expect(clickAction('/prompts?q=x', 'rhythm', '/gp/prompts')).toBeUndefined();
	});
});

describe('clickThrough', () => {
	class FakeElement {
		constructor(
			private readonly found: Record<string, FakeElement | null>,
			readonly dataset: Record<string, string> = {},
			private readonly href: string | null = null
		) {}
		closest(selector: string) {
			return this.found[selector] ?? null;
		}
		getAttribute() {
			return this.href;
		}
	}

	/** Mounts the action on a node; returns what a click on `target` does and the removal. */
	function mount() {
		vi.stubGlobal('Element', FakeElement);
		let listener: (event: { target: unknown }) => void = () => {};
		const node = {
			addEventListener: (_: string, fn: typeof listener) => (listener = fn),
			removeEventListener: vi.fn()
		};
		const handle = clickThrough(node as unknown as HTMLElement);
		return { click: (target: unknown) => listener({ target }), node, handle };
	}

	afterEach(() => {
		vi.unstubAllGlobals();
		tracked.length = 0;
	});

	it('counts a click on a link to the prompts inside a panel', () => {
		const { click } = mount();
		const panel = new FakeElement({}, { panel: 'rhythm' });
		const link = new FakeElement({ '[data-panel]': panel }, {}, '/prompts?range=7d');
		click(new FakeElement({ 'a[href]': link }));
		expect(tracked).toEqual(['panel.click.rhythm']);
	});

	it('counts nothing for a click that is not on a link, or on another page', () => {
		const { click } = mount();
		click({});
		click(new FakeElement({}));
		const other = new FakeElement({}, {}, '/sessions/abc');
		click(new FakeElement({ 'a[href]': other }));
		expect(tracked).toEqual([]);
	});

	it('stops listening when it is destroyed', () => {
		const { node, handle } = mount();
		(handle as { destroy: () => void }).destroy();
		expect(node.removeEventListener).toHaveBeenCalledOnce();
	});
});
