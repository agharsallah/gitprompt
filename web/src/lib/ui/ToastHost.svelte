<script lang="ts">
	import { toasts } from '#lib/ui/toast.svelte.ts';
</script>

<!-- One live region for every toast: a message is announced when it is added. -->
<div class="host" role="status" aria-live="polite">
	{#each toasts.items as toast (toast.id)}
		{#if toast.href}
			<a
				class={['toast', 'link', toast.tone]}
				href={toast.href}
				onclick={() => toasts.dismiss(toast.id)}
			>
				<span class="message">{toast.message}</span>
				{#if toast.detail}<span class="detail">{toast.detail}</span>{/if}
			</a>
		{:else}
			<p class={['toast', toast.tone]}>
				{toast.message}
				{#if toast.detail}<span class="detail">{toast.detail}</span>{/if}
			</p>
		{/if}
	{/each}
</div>

<style>
	.host {
		position: fixed;
		right: var(--space-4);
		bottom: var(--space-4);
		z-index: var(--z-toast);
		display: grid;
		gap: var(--space-2);
		pointer-events: none;
	}

	.toast {
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		background: var(--color-surface-raised);
		box-shadow: var(--shadow-overlay);
		font-size: var(--text-sm);
		animation: appear var(--duration-base) var(--ease-out);
	}

	.link {
		display: grid;
		gap: var(--space-1);
		max-width: min(24rem, calc(100vw - 2 * var(--space-4)));
		color: inherit;
		text-decoration: none;
		pointer-events: auto;
	}

	.link:hover .message {
		text-decoration: underline;
	}

	.message {
		font-weight: var(--weight-semibold);
	}

	.detail {
		display: block;
		color: var(--color-text-secondary);
	}

	.error {
		color: var(--color-danger-text);
	}

	@keyframes appear {
		from {
			opacity: 0;
			transform: translateY(4px);
		}
	}
</style>
