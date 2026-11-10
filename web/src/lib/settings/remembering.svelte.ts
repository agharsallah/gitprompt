// Whether this browser remembers filters and the time range (lib/settings/remembered.ts), as
// reactive state for the Settings page: the General switch changes it, the "This browser" list
// re-reads the remembered values when it changes.
import { isRemembering, setRemembering } from '#lib/settings/remembered.ts';

class Remembering {
	on = $state(isRemembering());
	/** Bumped whenever remembered values are changed from Settings. */
	version = $state(0);

	set(on: boolean) {
		setRemembering(on);
		this.on = on;
		this.version++;
	}

	changed() {
		this.version++;
	}
}

export const remembering = new Remembering();
