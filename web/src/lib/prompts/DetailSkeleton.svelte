<script lang="ts">
	import Skeleton from '#lib/ui/Skeleton.svelte';
</script>

<!-- The shape of a prompt's detail while it loads. -->
<div class="detail-skeleton">
	<div class="detail-head">
		<Skeleton width="5rem" height="1.25rem" shape="block" />
		<Skeleton width="7rem" height="0.8rem" />
	</div>
	<Skeleton lines={4} height="1rem" />
	<Skeleton shape="block" height="7rem" />
	<div class="detail-head">
		{#each ['5rem', '6rem', '4.5rem', '5.5rem'] as width, i (i)}
			<Skeleton {width} height="1.75rem" shape="block" />
		{/each}
	</div>
	<Skeleton lines={3} />
</div>

<style>
	.detail-skeleton {
		display: grid;
		gap: var(--space-5);
	}

	.detail-head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3);
	}
</style>
