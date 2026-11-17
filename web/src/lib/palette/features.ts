/**
 * Loads every feature's commands: each `<what>.commands.ts` under lib/ registers its own when it
 * is imported, so the layout imports this once and a new feature's commands appear by adding a
 * file, never by editing a list.
 */
export const featureCommands = import.meta.glob('/src/lib/**/*.commands.ts', { eager: true });
