/** Panel states the Overview panels share (lib/ui/panelState.ts). */
import type { PanelState } from '#lib/ui/panelState.ts';

/** The empty state of a panel that counts prompts, under the page's range and filters. */
export const NO_PROMPTS: Extract<PanelState, { kind: 'empty' }> = {
	kind: 'empty',
	title: 'No prompts in this window'
};
