<script lang="ts">
	import { api } from '#lib/api/client.ts';
	import { live } from '#lib/api/live.svelte.ts';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';

	/**
	 * Is capture working? The stream's state, and when anything was last captured (from the
	 * sources at load, then from live notices).
	 */
	let lastCapture = $state<string | null>(null);

	$effect(() => {
		api
			.sources()
			.then((sources) => {
				const latest = sources
					.map((s) => s.last_at)
					.filter((t): t is string => !!t)
					.sort()
					.at(-1);
				if (latest && (!lastCapture || latest > lastCapture)) lastCapture = latest;
			})
			.catch(() => {});
		return live.subscribe(() => (lastCapture = new Date().toISOString()));
	});

	const STATES = {
		open: { label: 'Listening', tone: 'on' },
		connecting: { label: 'Connecting', tone: 'wait' },
		reconnecting: { label: 'Reconnecting', tone: 'wait' },
		closed: { label: 'Not listening', tone: 'off' }
	} as const;
	const status = $derived(STATES[live.state]);
</script>

<div class="live" role="status">
	{#key live.lastNotice}
		<span class="dot {status.tone}" aria-hidden="true"></span>
	{/key}
	<span class="text">
		<span class="state">{status.label}</span>
		<span class="last">Last capture <RelativeTime iso={lastCapture} /></span>
	</span>
</div>

<style>
	.live {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		font-size: var(--text-xs);
	}

	.dot {
		flex: none;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-text-muted);
	}

	.dot.on {
		background: var(--color-success-solid);
		animation: ping 900ms var(--ease-out) 1;
	}

	.dot.wait {
		background: var(--color-warning-solid);
	}

	.text {
		display: grid;
		min-width: 0;
		line-height: var(--leading-snug);
	}

	.state {
		color: var(--color-text);
		font-weight: var(--weight-medium);
	}

	.last {
		overflow: hidden;
		color: var(--color-text-muted);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* A single ring when something new is captured. */
	@keyframes ping {
		from {
			box-shadow: 0 0 0 0 color-mix(in srgb, var(--color-success-solid) 60%, transparent);
		}
		to {
			box-shadow: 0 0 0 8px transparent;
		}
	}
</style>
