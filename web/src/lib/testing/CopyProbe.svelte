<script lang="ts">
	import type { UsageAction } from '#lib/shared/usage-actions.ts';

	/** Stands in for CopyButton in a test: shows what a copy would put on the clipboard and count. */
	let { text, count }: { text: string; count: UsageAction } = $props();
</script>

<button type="button" data-copy={text} data-count={count}>Copy</button>
