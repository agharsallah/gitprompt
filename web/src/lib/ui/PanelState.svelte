<script lang="ts">
	import type { Snippet } from 'svelte';
	import Loading from '#lib/async/Loading.svelte';
	import ErrorView from '#lib/errors/ErrorView.svelte';
	import { plural } from '#lib/shared/format.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import type { Notice, PanelState } from '#lib/ui/panelState.ts';
	import PanelSkeleton from '#lib/ui/PanelSkeleton.svelte';

	/**
	 * What a panel, list or story shows, one way everywhere: pass the state (panelState.ts) and the
	 * content as children. `ready` draws the children alone, with nothing added around them; `fresh`
	 * draws them under a live chip; every other state stands in for them. Use it inside a `Panel`
	 * (which takes the state itself) or on its own.
	 */
	let { state, children }: { state: PanelState; children?: Snippet } = $props();

	type Of<K extends PanelState['kind']> = Extract<PanelState, { kind: K }>;

	const freshLabel = (s: Of<'fresh'>) =>
		s.unit ? `${s.count} new ${s.count === 1 ? s.unit : `${s.unit}s`}` : `${s.count} new`;

	function sparseText(s: Of<'sparse'>) {
		const least = plural(s.minimum, s.unit ?? 'prompt');
		const soFar =
			s.have === undefined ? '' : `; there ${s.have === 1 ? 'is' : 'are'} ${s.have} so far`;
		return `This needs at least ${least} to say anything useful${soFar}.`;
	}
</script>

{#if state.kind === 'ready'}
	{@render children?.()}
{:else if state.kind === 'fresh'}
	<div class="fresh" role="status">
		<Badge tone="accent" dot size="sm">{freshLabel(state)}</Badge>
	</div>
	{@render children?.()}
{:else if state.kind === 'loading'}
	<Loading label={state.label ?? 'Loading'}>
		<PanelSkeleton
			frame={false}
			title={false}
			body={state.body}
			rows={state.rows}
			height={state.height}
		/>
	</Loading>
{:else if state.kind === 'empty' || state.kind === 'unsupported'}
	{@render notice(state)}
{:else if state.kind === 'error'}
	<ErrorView error={state.error} what={state.what} onretry={state.retry} />
{:else}
	<EmptyState compact title="Too little data yet">{sparseText(state)}</EmptyState>
{/if}

{#snippet notice(n: Notice)}
	{#snippet message()}{n.message}{/snippet}
	{#snippet next()}
		{#if n.next}<Button size="sm" href={n.next.href} title={n.next.title}>{n.next.label}</Button
			>{/if}
	{/snippet}
	<EmptyState
		compact
		title={n.title}
		children={n.message ? message : undefined}
		actions={n.next ? next : undefined}
	/>
{/snippet}

<style>
	.fresh {
		display: flex;
		justify-content: flex-end;
		margin-bottom: var(--space-2);
	}
</style>
