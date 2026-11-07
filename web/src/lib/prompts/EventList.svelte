<script lang="ts">
	import type { Event } from '#lib/api/types.ts';
	import { eventTime, toolDetail } from '#lib/insights/timeline.ts';
	import { toolKind, KIND_INFO } from '#lib/insights/tools.ts';
	import { modelName } from '#lib/shared/format.ts';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import TextField from '#lib/ui/TextField.svelte';

	/** Every event of the turn, in the order it happened, each with its full payload. */
	let { events, start }: { events: Event[]; start: string } = $props();

	let filter = $state('');
	const sorted = $derived([...events].sort((a, b) => eventTime(a) - eventTime(b)));
	const shown = $derived(
		filter
			? sorted.filter((e) => `${e.event} ${detail(e)}`.toLowerCase().includes(filter.toLowerCase()))
			: sorted
	);
	const offset = (e: Event) => {
		const s = (eventTime(e) - Date.parse(start)) / 1000;
		return `${s < 0 ? '−' : '+'}${Math.abs(s).toFixed(1)}s`;
	};
	// Telemetry events are the OTel stream; the rest are hooks.
	const isTelemetry = (e: Event) => 'event.timestamp' in e.raw;
	function detail(e: Event): string {
		const tool = (e.raw.tool_name ?? e.fields.tool_name) as string | undefined;
		return [
			tool,
			tool && toolDetail(e.raw.tool_input),
			e.raw.model && modelName(String(e.raw.model)),
			e.raw.agent_type,
			e.raw.notification_type,
			e.raw.query_source
		]
			.filter((v) => typeof v === 'string' && v)
			.join('  ');
	}
	const color = (e: Event) => {
		const tool = (e.raw.tool_name ?? e.fields.tool_name) as string | undefined;
		if (tool) return KIND_INFO[toolKind(tool)].color;
		if (e.event === 'api_request') return 'var(--color-agent)';
		if (e.event?.includes('Failure') || e.event === 'api_error') return 'var(--color-danger-solid)';
		return 'var(--color-border-control)';
	};
</script>

<div class="events">
	<div class="tools">
		<TextField
			label="Filter events"
			hideLabel
			type="search"
			size="sm"
			bind:value={filter}
			placeholder="Filter by event, tool or input"
			clearable
		/>
		<span class="muted">{shown.length} of {events.length}</span>
	</div>
	<ol>
		{#each shown as e (e.id)}
			<li style:--dot={color(e)}>
				<details>
					<summary>
						<span class="at num">{offset(e)}</span>
						<span class="dot" aria-hidden="true"></span>
						<span class="name">{e.event ?? 'unknown'}</span>
						<span class="detail truncate">{detail(e)}</span>
						<span class="origin">{isTelemetry(e) ? 'telemetry' : 'hook'}</span>
					</summary>
					<CodeBlock
						code={JSON.stringify(e.raw, null, 2)}
						label="{e.event} payload"
						maxHeight="20rem"
					/>
				</details>
			</li>
		{/each}
	</ol>
</div>

<style>
	.events {
		display: grid;
		gap: var(--space-3);
	}

	.tools {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		font-size: var(--text-xs);
	}

	.tools > :global(.field) {
		flex: 0 1 20rem;
	}

	ol {
		display: grid;
	}

	summary {
		display: grid;
		grid-template-columns: 4.5rem 10px max-content minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-2);
		min-height: 2rem;
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		font-size: var(--text-sm);
		cursor: pointer;
		list-style: none;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	summary:hover {
		background: var(--color-surface-hover);
	}

	details[open] summary {
		background: var(--color-surface-sunken);
	}

	details[open] :global(.code-block) {
		margin: var(--space-2) 0 var(--space-3) calc(4.5rem + var(--space-6));
	}

	.at {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		text-align: right;
	}

	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--dot);
	}

	.name {
		font-weight: var(--weight-medium);
	}

	.detail {
		color: var(--color-text-secondary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.origin {
		color: var(--color-text-muted);
		font-size: var(--text-2xs);
	}

	@media (max-width: 40rem) {
		summary {
			grid-template-columns: 3.5rem 10px minmax(0, 1fr);
		}

		.detail,
		.origin {
			display: none;
		}

		details[open] :global(.code-block) {
			margin-left: 0;
		}
	}
</style>
