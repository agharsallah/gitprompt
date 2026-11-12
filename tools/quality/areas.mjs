// The parts of the repository the dashboard groups reports by, in sidebar order. A report lives in
// reports/<area id>/; adding an area is a line here and a folder there. `inputs`: the files every
// report of the area depends on (a report adds its own); when none changed since a report's last
// successful run, `just quality` keeps that result instead of rebuilding it.
export const AREAS = [
	{
		id: 'backend',
		title: 'Backend',
		description: 'The Rust workspace in backend/: daemon, CLI and desktop app.',
		// The API types test (ts_types.rs) compares the Rust wire types with the dashboard's copy.
		inputs: ['backend', 'web/src/lib/api/generated.ts']
	},
	{
		id: 'web',
		title: 'Web',
		description: 'The SvelteKit dashboard in web/, built into the binary.',
		inputs: ['web']
	}
];
