/**
 * `use:seen={action}`: counts `action` once when the element stays in view for a moment (a panel
 * scrolled past in a blink is not "seen"). Once per page visit: the same action is not counted
 * again when the element is rendered anew by a refresh; `forgetSeen()` starts a new visit.
 */
import { track } from '#lib/shared/usage.ts';
import type { UsageAction } from '#lib/shared/usage-actions.ts';

/** How long an element has to stay in view. */
export const DWELL_MS = 1000;

/** How much of the element has to be in view. */
const VISIBLE = 0.25;

/** Calls `onChange(visible)` as `node` comes into and leaves the view; returns how to stop. */
export type Observe = (node: Element, onChange: (visible: boolean) => void) => () => void;

export const observeIntersection: Observe = (node, onChange) => {
	if (typeof IntersectionObserver === 'undefined') return () => {};
	const observer = new IntersectionObserver(
		(entries) => {
			for (const entry of entries) onChange(entry.isIntersecting);
		},
		{ threshold: VISIBLE }
	);
	observer.observe(node);
	return () => observer.disconnect();
};

const counted = new Set<UsageAction>();

/** A new page visit: panels seen on the last one count again. */
export const forgetSeen = () => counted.clear();

type Options = { dwellMs?: number; observe?: Observe; count?: (action: UsageAction) => void };

type Seen = (
	node: Element,
	action: UsageAction | undefined
) => { update: (action: UsageAction | undefined) => void; destroy: () => void };

/** The action, with its dwell time, observer and counter set by a test; `seen` is the real one. */
export function createSeen({
	dwellMs = DWELL_MS,
	observe = observeIntersection,
	count = track
}: Options = {}): Seen {
	return (node, initial) => {
		let action = initial;
		let timer: ReturnType<typeof setTimeout> | undefined;

		const stopObserving = observe(node, (visible) => {
			clearTimeout(timer);
			if (!visible) return;
			timer = setTimeout(() => {
				if (!action || counted.has(action)) return;
				counted.add(action);
				count(action);
				stopObserving();
			}, dwellMs);
		});

		return {
			update: (next) => {
				action = next;
			},
			destroy: () => {
				clearTimeout(timer);
				stopObserving();
			}
		};
	};
}

export const seen = createSeen();
