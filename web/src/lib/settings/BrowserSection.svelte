<script lang="ts">
	import { RotateCcw } from '@lucide/svelte';
	import { BROWSER_PREFERENCES } from '#lib/settings/browser.ts';
	import { views } from '#lib/settings/views.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	// Plain localStorage keys aren't reactive: bump this after a reset to read them again.
	let version = $state(0);
	const rows = $derived.by(() => {
		void version;
		void views.search;
		return BROWSER_PREFERENCES.map((p) => ({ ...p, value: p.current() }));
	});
</script>

<Panel
	title="This browser"
	tone="sunken"
	description="Kept in this browser's local storage, not in preferences.json: another browser starts from the defaults."
>
	<table>
		<caption class="sr-only">Preferences kept in this browser</caption>
		<thead>
			<tr>
				<th scope="col">Preference</th>
				<th scope="col">Value</th>
				<th scope="col">Changed in</th>
				<th scope="col"><span class="sr-only">Reset</span></th>
			</tr>
		</thead>
		<tbody>
			{#each rows as p (p.key)}
				<tr>
					<td>{p.label} <span class="mono muted">{p.key}</span></td>
					<td>{p.value}</td>
					<td class="muted">{p.where}</td>
					<td class="reset">
						<Button
							size="sm"
							variant="ghost"
							icon={RotateCcw}
							onclick={() => {
								p.reset();
								version++;
							}}>Reset</Button
						>
					</td>
				</tr>
			{/each}
		</tbody>
	</table>
</Panel>

<style>
	table {
		width: 100%;
		font-size: var(--text-sm);
	}

	th {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
		text-align: left;
	}

	th,
	td {
		padding: var(--space-1_5) var(--space-2);
		vertical-align: middle;
	}

	.mono {
		display: block;
		font-size: var(--text-xs);
	}

	.reset {
		text-align: right;
	}
</style>
