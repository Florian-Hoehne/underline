// The title of a fake note, as `NoteId::title` gives it: the page name with a
// dash shown as a space (UC-003 BR-001); a page name that is exactly a date,
// `2026-11-11`, keeps its dashes (OQ-272, UC-025 BR-001).

/** The title of the note `note` (its place without extension). */
export function mockTitle(note: string): string {
  const page = note.split('/').at(-1) ?? note;
  return /^\d{4}-\d{2}-\d{2}$/.test(page) ? page : page.replace(/-/g, ' ');
}
