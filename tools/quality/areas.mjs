// The parts of the repository the dashboard groups reports by, in sidebar order. A report lives in
// reports/<area id>/; adding an area is a line here and a folder there.
export const AREAS = [
	{
		id: 'backend',
		title: 'Backend',
		description: 'The Rust workspace in backend/: daemon, CLI and desktop app.'
	},
	{
		id: 'web',
		title: 'Web',
		description: 'The SvelteKit dashboard in web/, built into the binary.'
	}
];
