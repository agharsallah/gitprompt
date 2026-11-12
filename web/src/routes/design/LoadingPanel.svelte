<script lang="ts">
	import ErrorView from '#lib/errors/ErrorView.svelte';
	import ListSkeleton from '#lib/ui/ListSkeleton.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import PanelSkeleton from '#lib/ui/PanelSkeleton.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';

	/** While data loads, and when it fails: skeletons, then each kind of error as it is shown. */
</script>

<Panel
	title="Loading and errors"
	description="Data streams in: a skeleton shaped like what comes holds its place (lib/async/Deferred.svelte), then later refreshes keep the old content. Errors stay where they happened, said the same way everywhere (lib/errors/)."
>
	<div class="grid two">
		<div class="stack">
			<div class="row">
				<Skeleton width="10rem" />
				<Skeleton shape="circle" height="1.5rem" />
				<Skeleton width="6rem" height="2rem" shape="block" />
			</div>
			<Skeleton lines={3} />
			<PanelSkeleton body="rows" rows={4} />
			<ListSkeleton variant="rows" rows={3} />
		</div>
		<div class="stack">
			<ErrorView error={new TypeError('Failed to fetch')} what="the overview" onretry={() => {}} />
			<ErrorView
				error={new Error('Cannot read properties of undefined')}
				what="this panel"
				onretry={() => {}}
			/>
			<ErrorView
				error={new Error('500 /api/stats: database is locked')}
				status={500}
				what="the overview"
				note="What you see below was loaded earlier."
				onretry={() => {}}
			/>
			<ErrorView
				error={new TypeError('Failed to fetch dynamically imported module: /_app/x.js')}
				what="this panel"
			/>
		</div>
	</div>
</Panel>
