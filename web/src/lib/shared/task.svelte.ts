/**
 * Reactive state for a user action that calls the daemon: `busy` while it runs, `error` if it
 * failed. One instance per component; `run` never throws.
 */
export class Task {
	busy = $state(false);
	error = $state('');

	async run(step: () => Promise<unknown>): Promise<boolean> {
		this.busy = true;
		this.error = '';
		try {
			await step();
			return true;
		} catch (e) {
			this.error = e instanceof Error ? e.message : String(e);
			return false;
		} finally {
			this.busy = false;
		}
	}
}
