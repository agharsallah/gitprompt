<script lang="ts">
	/** The rail as a logo: a session line, a branch, two prompts made and one still open. */
	let { size = 28 }: { size?: number } = $props();
</script>

<svg class="mark" width={size} height={size} viewBox="0 0 32 32" aria-hidden="true">
	<rect width="32" height="32" rx="8" />
	<path d="M12 6v20M12 19c0-5 9-3 9-8" />
	<circle class="node" cx="12" cy="9.5" r="3.6" />
	<circle class="node" cx="21" cy="10" r="3.6" />
	<circle class="open" cx="12" cy="23" r="3.2" />
</svg>

<style>
	rect {
		fill: var(--color-accent);
	}

	path {
		fill: none;
		stroke: var(--color-accent-soft);
		stroke-width: 2.4;
		stroke-linecap: round;
		opacity: 0.8;
	}

	.node {
		fill: var(--color-text-on-accent);
	}

	.open {
		fill: var(--color-accent);
		stroke: var(--color-text-on-accent);
		stroke-width: 2.2;
	}
</style>
