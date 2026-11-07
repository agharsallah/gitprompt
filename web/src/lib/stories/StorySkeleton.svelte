<script lang="ts">
	import ListSkeleton from '#lib/ui/ListSkeleton.svelte';
	import PanelSkeleton from '#lib/ui/PanelSkeleton.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';

	/** A session or project story while it loads: its header, key figures, then its chapters. */
</script>

<div class="story">
	<div class="header">
		<Skeleton width="min(28rem, 70%)" height="2rem" shape="block" />
		<Skeleton width="min(36rem, 90%)" height="1rem" />
	</div>
	<PanelSkeleton body="figures" rows={4} title={false} />
	<ListSkeleton rows={4} height="3rem" />
</div>

<style>
	.story {
		display: grid;
		gap: var(--space-6);
	}

	.header {
		display: grid;
		gap: var(--space-3);
	}
</style>
