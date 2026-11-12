import { describe, expect, it } from 'vitest';
import { Task } from '#lib/shared/task.svelte.ts';

describe('Task', () => {
	it('is busy while the step runs, and clear after a success', async () => {
		const task = new Task();
		let release = () => {};
		const running = task.run(() => new Promise<void>((resolve) => (release = resolve)));
		expect(task.busy).toBe(true);
		release();
		expect(await running).toBe(true);
		expect([task.busy, task.error]).toEqual([false, '']);
	});

	it('keeps the error message of a failure and never throws', async () => {
		const task = new Task();
		expect(await task.run(() => Promise.reject(new Error('daemon down')))).toBe(false);
		expect([task.busy, task.error]).toEqual([false, 'daemon down']);
		expect(await task.run(() => Promise.reject(new Error(String('plain'))))).toBe(false);
		expect(task.error).toBe('plain');
	});

	it('clears an old error when it runs again', async () => {
		const task = new Task();
		await task.run(() => Promise.reject(new Error('first')));
		await task.run(() => Promise.resolve());
		expect(task.error).toBe('');
	});
});
