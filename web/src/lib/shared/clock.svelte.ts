/**
 * One shared "now" for every relative time on screen, ticking every 15 seconds while the window
 * is visible (the root layout calls `setVisible` from `platform.onWindowVisible`). One timer for
 * the whole app instead of one per timestamp, and none while the window is hidden.
 */
export const TICK_MS = 15_000;

class Clock {
	now = $state(Date.now());
	#timer: ReturnType<typeof setInterval> | undefined;

	constructor() {
		if (typeof window !== 'undefined') this.#start();
	}

	#start() {
		this.#timer = setInterval(() => (this.now = Date.now()), TICK_MS);
	}

	/** Hidden: no tick. Shown: "now" is fresh at once, then ticks again. */
	setVisible(visible: boolean) {
		clearInterval(this.#timer);
		this.#timer = undefined;
		if (!visible) return;
		this.now = Date.now();
		this.#start();
	}
}

export const clock = new Clock();
