/**
 * Whether the palette is open. One shared state: ⌘K (the layout), the sidebar's Find button and a
 * command all open it the same way. The dialog's code is fetched the first time it is opened
 * (`loaded`), so pages pay nothing until then.
 */
import { track } from '#lib/shared/usage.ts';

export class PaletteState {
	open = $state(false);
	/** Opened at least once: the dialog's code is wanted from now on. */
	loaded = $state(false);
	/** How many times it was opened: what makes the recent items be fetched again. */
	opened = $state(0);

	show() {
		if (this.open) return;
		this.loaded = true;
		this.open = true;
		this.opened += 1;
		track('palette.open');
	}

	hide() {
		this.open = false;
	}

	toggle() {
		if (this.open) this.hide();
		else this.show();
	}
}

export const palette = new PaletteState();
