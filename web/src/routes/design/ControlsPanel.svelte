<script lang="ts">
	import { Plug, Search, Star, Tag, Trash2 } from '@lucide/svelte';
	import Button from '#lib/ui/Button.svelte';
	import Chip from '#lib/ui/Chip.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import ToggleChip from '#lib/ui/ToggleChip.svelte';

	/** Every way to act: buttons in each variant and state, toggles, chips and fields, all live. */

	let segment = $state<'day' | 'week' | 'month'>('week');
	let text = $state('');
	let toggled = $state(true);
	let pressed = $state(false);

	const PERIODS: { value: typeof segment; label: string }[] = [
		{ value: 'day', label: 'Day' },
		{ value: 'week', label: 'Week' },
		{ value: 'month', label: 'Month' }
	];
</script>

<Panel title="Buttons and toggles">
	<div class="row">
		<Button variant="primary" icon={Plug}>Connect</Button>
		<Button>Secondary</Button>
		<Button variant="ghost">Ghost</Button>
		<Button variant="danger" icon={Trash2}>Disconnect</Button>
		<Button loading>Saving</Button>
		<Button disabled>Disabled</Button>
		<Button icon={Star} label="Star" {pressed} onclick={() => (pressed = !pressed)} />
		<Button size="sm" variant="primary">Small</Button>
		<Button size="sm" icon={Search} label="Search" />
	</div>
	<div class="row">
		<SegmentedControl label="Period" bind:value={segment} options={PERIODS} />
		<ToggleChip bind:checked={toggled} icon={Star} count={12}>Starred</ToggleChip>
		<Chip icon={Tag}>#refactor</Chip>
		<Chip onremove={() => {}} removeLabel="Remove filter">repo: gitprompt</Chip>
	</div>
	<div class="row fields">
		<TextField
			label="Search prompts"
			icon={Search}
			bind:value={text}
			placeholder="Search prompt text"
			clearable
		/>
		<TextField label="Tag" size="sm" placeholder="Small field" hint="Hints sit under the field." />
	</div>
</Panel>

<style>
	.fields > :global(*) {
		flex: 1 1 14rem;
	}
</style>
