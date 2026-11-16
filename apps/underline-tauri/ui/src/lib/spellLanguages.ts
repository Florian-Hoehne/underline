// FR-100, UC-010 BR-001 (ADR-030): the spell check languages Underline
// offers, by the code the settings file holds and the name the window
// shows; the same list as `SpellCheckLanguage` of underline-core.

/** Each language: its code and its name, off first, then by name. */
export const SPELL_LANGUAGES: readonly (readonly [string, string])[] = [
  ['off', 'Off'],
  ['en-GB', 'English (UK)'],
  ['en-US', 'English (US)'],
  ['fr-FR', 'French (France)'],
  ['de-DE', 'German (Germany)'],
  ['de-CH', 'German (Switzerland)'],
  ['it-IT', 'Italian (Italy)'],
  ['es-ES', 'Spanish (Spain)'],
];
