// FR-100, UC-010 BR-001 (ADR-030): the spell check languages Underline
// offers, by the code the settings file holds and the name the window
// shows; the same list as `SpellCheckLanguage` of underline-core.

/** Each language: its code and its name, off first. */
export const SPELL_LANGUAGES: readonly (readonly [string, string])[] = [
  ['off', 'Off'],
  ['en-US', 'English (US)'],
  ['en-GB', 'English (UK)'],
  ['fr-FR', 'French (France)'],
  ['es-ES', 'Spanish (Spain)'],
];
