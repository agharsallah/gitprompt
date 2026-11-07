/**
 * One shared "now" for every relative time on screen, ticking every 15 seconds while the tab is
 * visible. One timer for the whole app instead of one per timestamp.
 */
class Clock {
	now = $state(Date.now());

	constructor() {
		if (typeof window === 'undefined') return;
		setInterval(() => {
			if (document.visibilityState === 'visible') this.now = Date.now();
		}, 15_000);
		document.addEventListener('visibilitychange', () => (this.now = Date.now()));
	}
}

export const clock = new Clock();
