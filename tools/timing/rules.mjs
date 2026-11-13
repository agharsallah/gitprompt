// What counts as a time dependency: code whose result depends on when it runs or on how long
// something takes, so a run on a loaded machine (CI, a laptop on battery) can differ from one on an
// idle one. Each rule matches one line of source, in tests, in product code, or in both.
//
// Not every one is a fault, so each rule says where it blocks and where it only reports:
//   block   the usual cause of a flaky test or untestable code: a new one fails `just timing`, the
//           quality gate and CI, unless the ledger (ledger.json) accepts that line;
//   report  a design choice (a retention interval, a hook timeout, a log's duration): listed on the
//           quality page for review, never failing.
// A text match is a place to look, not proof; `just flaky` (flaky.mjs) is the safety net for what
// no pattern sees (races, reused ports).
//
// This file is policy, like the lint configs: changing what blocks is the user's decision.

/** Where a line is: `test` (test files, test helpers, `#[cfg(test)]` modules) or `code`. */
export const SCOPES = ['test', 'code'];

/**
 * The files that supply the time to everything else: the server's clock, the CLI's entry points
 * (each hands `now` to the code it calls), the page's shared clock, the desktop app's measurement
 * mode. A clock read there is the injection itself, so it is reported, never blocked. A path ending
 * in `/` is a folder.
 */
export const CLOCK_SEAMS = [
	'backend/crates/gitprompt-server/src/clock.rs',
	'backend/crates/gitprompt-cli/src/',
	'backend/apps/desktop/src/perf/',
	'web/src/lib/shared/clock.svelte.ts'
];

/**
 * id, title, scope (`test`, `code` or `any`), block (the scopes where a new one fails; the others
 * report), seams (files where it only reports), why (what can go wrong), fix (what to do
 * instead), rust / web: a RegExp for that language's lines (absent: the rule does not apply), and
 * rustExcept / webExcept: a global RegExp removed from the line before matching.
 */
export const RULES = [
	{
		id: 'sleep',
		title: 'Sleeps in a test',
		scope: 'test',
		block: ['test'],
		why: 'Waits a fixed time for something to happen: too short on a loaded machine (the test fails now and then), too long on an idle one (the suite is slow).',
		fix: 'Wait for the event itself: a signal or future the code under test gives (`spool_drained()`), a channel, a lock, or drive time (`ManualClock`, `tokio::time::pause`, fake timers).',
		rust: /\b(?:thread|time)::sleep\s*\(/,
		web: /\bsetTimeout\s*\(/
	},
	{
		id: 'elapsed-assert',
		title: 'Asserts how long something took',
		scope: 'test',
		block: ['test'],
		why: 'A test that fails when the machine is slow, not when the code is wrong.',
		fix: 'Assert the outcome; measure speed in a benchmark or a quality report (`just perf`, `just quality measure`), where a slow machine is a reading, not a failure.',
		rust: /assert[a-z_]*!\(.*\belapsed\(\)|\belapsed\(\)\s*[<>]/,
		web: /expect\(.*(?:performance\.now|Date\.now)\(\).*\)\.toBe(?:Less|Greater)Than/
	},
	{
		id: 'deadline',
		title: 'Times out in a test',
		scope: 'test',
		block: ['test'],
		why: 'A raw timeout decides the test: on a loaded machine the answer comes after it and the test fails, though the code was right.',
		fix: 'Wait for the condition with the one polling helper (`eventually`, an exact condition with a generous deadline as a failsafe), or on a signal.',
		rust: /\b(?:recv_|wait_)?timeout\s*\(|\bwait_exit\s*\(/,
		// …or a test or hook given a timeout as its last argument: `}, 120_000);` (0: no deadline).
		web: /\bwaitFor\s*\(|\btimeout\s*:\s*\d|AbortSignal\.timeout\s*\(|^\s*\}\s*,\s*[1-9][\d_]*\s*\);?\s*$|\b(?:it|test|beforeAll|beforeEach|afterAll|afterEach)\(.*,\s*[1-9][\d_]*\s*\);?\s*$/
	},
	{
		id: 'poll',
		title: 'Polls a condition in a test',
		scope: 'test',
		block: [],
		why: 'Sound while the condition is exact and the deadline only stops a hang; a signal is still better when the code can give one.',
		fix: 'Keep the condition exact; prefer a signal (`Daemon::spool_drained`, a lock, a channel) when there is one.',
		rust: /\beventually\s*\(/
	},
	{
		id: 'wall-clock',
		title: 'Reads the clock directly',
		scope: 'any',
		block: ['code'],
		seams: CLOCK_SEAMS,
		why: 'Not injectable: a test cannot control it, and a value derived from it cannot be asserted (or only between two readings).',
		fix: "Take now from the injected clock (the server's `Clock`, a `now` parameter, the page's shared `clock`); a default parameter (`now = Date.now()`) is fine. In a test, a fixed time.",
		rust: /\b(?:Timestamp|SystemTime|Utc|Local|Zoned)::now\s*\(/,
		web: /\bDate\.now\(\)|\bnew Date\(\)/,
		// A default parameter (`now = Date.now(),` or `now = new Date())`) is how the page injects
		// time: removed from the line before matching.
		webExcept: /[\w$]+\s*=\s*(?:Date\.now\(\)|new Date\(\))(?=\s*(?:[,)]|$))/g
	},
	{
		id: 'stopwatch',
		title: 'Measures elapsed time',
		scope: 'any',
		block: [],
		why: 'Behaviour or output that depends on how long something took (a log line is harmless; a branch on it is not).',
		fix: 'Keep durations to logs and measurements; never branch on one in code a test covers.',
		rust: /\bInstant::now\s*\(/,
		web: /\bperformance\.now\s*\(/
	},
	{
		id: 'timer',
		title: 'Times out or runs on a timer',
		scope: 'code',
		block: [],
		why: 'Product behaviour on a schedule or a timeout (retention every 6 h, a hook that gives up after 2 s): each needs a way for tests to drive it.',
		fix: 'Make the interval or timeout a parameter tests set (`DaemonOptions`), or let tests trigger the work directly.',
		rust: /\b(?:thread|time)::sleep\s*\(|\btime::interval\s*\(|\b(?:recv_|busy_)?timeout\s*\(/,
		web: /\bset(?:Timeout|Interval)\s*\(/
	}
];

/** The rules that apply to a line in `scope` of a `language` file. */
export function rulesFor(language, scope) {
	return RULES.filter((rule) => rule[language] && (rule.scope === 'any' || rule.scope === scope));
}

/** `block` or `report`: what a match of `rule` in `file`, in `scope`, does. */
export function policyOf(rule, file, scope) {
	const seam = rule.seams?.some((path) => (path.endsWith('/') ? file.startsWith(path) : file === path));
	return rule.block.includes(scope) && !seam ? 'block' : 'report';
}

/** Where a rule blocks, in words: "tests", "code, not the clock seams", "never (reported only)". */
export function blocksIn(rule) {
	if (!rule.block.length) return 'never (reported only)';
	const where = rule.block.map((scope) => (scope === 'test' ? 'tests' : 'code')).join(' and ');
	return rule.seams ? `${where}, not the clock seams` : where;
}
