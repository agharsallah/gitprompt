// Time dependencies in the dashboard code and its tests (tools/timing): sleeps, deadlines and timing
// assertions in tests, clock reads, stopwatches and timers in code, against the ledger of accepted
// ones. The same scan as `just timing`, the quality gate and CI.
import { TIMING_INPUTS, timingPage } from '../../lib/timing.mjs';

export default {
	title: 'Time dependencies',
	description: 'Code whose result depends on when it runs or how long something takes: sleeps, deadlines, clock reads, timers. New ones fail the check.',
	inputs: TIMING_INPUTS,
	run: (ctx) => timingPage(ctx, 'web/')
};
