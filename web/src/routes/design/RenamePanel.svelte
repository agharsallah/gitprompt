<script lang="ts">
	import InlineRename from '#lib/ui/InlineRename.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	/** A text you rename in place: the button, the field, a save and a save that fails. */

	// Nothing is stored here: a save answers at once, and the second sample's always fails.
	const accept = () => Promise.resolve();
	const refuse = () => Promise.reject(new Error('the daemon is not answering'));
</script>

<Panel title="Rename in place">
	<div class="samples">
		<p>
			<InlineRename
				value="Tidy the parser"
				fallback="Tidy the parser"
				label="Session name"
				what="the sample name"
				onsave={accept}
			/>
		</p>
		<p class="muted">
			Click the text to edit it. Enter saves, Esc cancels, leaving the field saves. Emptying it goes
			back to the default.
		</p>
		<p>
			<InlineRename
				value="A name that cannot be saved"
				label="Sample name that fails"
				what="the sample name"
				onsave={refuse}
			/>
		</p>
		<p class="muted">
			The new text shows at once; when the save fails it goes back, and the reason is said.
		</p>
	</div>
</Panel>

<style>
	.samples {
		display: grid;
		gap: var(--space-2);
	}
</style>
