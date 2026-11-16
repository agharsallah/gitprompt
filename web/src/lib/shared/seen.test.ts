import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createSeen, forgetSeen, type Observe } from '#lib/shared/seen.ts';
import type { UsageAction } from '#lib/shared/usage-actions.ts';

/** An observer the test drives: `show()` and `hide()` stand for the element scrolling in and out. */
function fakeObserver() {
	let onChange: (visible: boolean) => void = () => {};
	const stopped = vi.fn();
	const observe: Observe = (_node, callback) => {
		onChange = callback;
		return stopped;
	};
	return { observe, show: () => onChange(true), hide: () => onChange(false), stopped };
}

const node = {} as Element;

function setup(dwellMs = 1000) {
	const observer = fakeObserver();
	const counted: UsageAction[] = [];
	const seen = createSeen({ dwellMs, observe: observer.observe, count: (a) => counted.push(a) });
	return { ...observer, counted, mount: (action?: UsageAction) => seen(node, action) };
}

beforeEach(() => {
	vi.useFakeTimers();
	forgetSeen();
});
afterEach(() => vi.useRealTimers());

describe('use:seen', () => {
	it('counts once after the element stays in view for the dwell time', () => {
		const { mount, show, counted, stopped } = setup(1000);
		mount('panel.seen.rhythm');
		show();
		vi.advanceTimersByTime(999);
		expect(counted).toEqual([]);
		vi.advanceTimersByTime(1);
		expect(counted).toEqual(['panel.seen.rhythm']);
		expect(stopped).toHaveBeenCalledTimes(1);
	});

	it('does not count an element that scrolls past before the dwell time', () => {
		const { mount, show, hide, counted } = setup(1000);
		mount('panel.seen.rhythm');
		show();
		vi.advanceTimersByTime(600);
		hide();
		vi.advanceTimersByTime(5000);
		expect(counted).toEqual([]);
		show();
		vi.advanceTimersByTime(1000);
		expect(counted).toEqual(['panel.seen.rhythm']);
	});

	it('restarts the dwell when the element comes back into view', () => {
		const { mount, show, hide, counted } = setup(1000);
		mount('panel.seen.rhythm');
		show();
		vi.advanceTimersByTime(600);
		hide();
		show();
		vi.advanceTimersByTime(600);
		expect(counted).toEqual([]);
		vi.advanceTimersByTime(400);
		expect(counted).toEqual(['panel.seen.rhythm']);
	});

	it('counts nothing without an action', () => {
		const { mount, show, counted } = setup(100);
		mount(undefined);
		show();
		vi.advanceTimersByTime(100);
		expect(counted).toEqual([]);
	});

	it('follows an action given later', () => {
		const { mount, show, counted } = setup(100);
		const handle = mount(undefined);
		handle.update('panel.seen.spend');
		show();
		vi.advanceTimersByTime(100);
		expect(counted).toEqual(['panel.seen.spend']);
	});

	it('counts an action once per visit, even when the element is rendered again', () => {
		const first = setup(100);
		first.mount('panel.seen.rhythm');
		first.show();
		vi.advanceTimersByTime(100);
		expect(first.counted).toEqual(['panel.seen.rhythm']);

		const again = setup(100);
		again.mount('panel.seen.rhythm');
		again.show();
		vi.advanceTimersByTime(100);
		expect(again.counted).toEqual([]);

		forgetSeen();
		const nextVisit = setup(100);
		nextVisit.mount('panel.seen.rhythm');
		nextVisit.show();
		vi.advanceTimersByTime(100);
		expect(nextVisit.counted).toEqual(['panel.seen.rhythm']);
	});

	it('stops observing and cancels a pending count when destroyed', () => {
		const { mount, show, counted, stopped } = setup(1000);
		const handle = mount('panel.seen.rhythm');
		show();
		handle.destroy();
		vi.advanceTimersByTime(5000);
		expect(counted).toEqual([]);
		expect(stopped).toHaveBeenCalled();
	});
});
