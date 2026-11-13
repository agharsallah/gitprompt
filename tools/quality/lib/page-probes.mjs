// What the page-timing reports measure inside the dashboard: the probe script the desktop app also
// runs in its performance profile (backend/apps/desktop/src/perf/probes.js, one copy for both).
// Chrome runs it before the dashboard's own scripts and the reports call its probes.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { REPO } from './paths.mjs';

/** The script to preload into the page; it defines window.__gitpromptPerf. */
export const PROBES = readFileSync(join(REPO, 'backend', 'apps', 'desktop', 'src', 'perf', 'probes.js'), 'utf8');

/** An expression calling one probe: call('switchTo', '/prompts'). */
export const call = (name, ...args) => `window.__gitpromptPerf.${name}(${args.map((a) => JSON.stringify(a)).join(', ')})`;
