// FR-099, UC-005 A28 (ADR-030): the spell check's commands for the spell
// menu of both editing views.

import { commands } from './bindings';
import { messageOf } from './editor/sync';

/**
 * UC-005 A28 step 1: up to five corrections for `word`, the likeliest
 * first; the backend works them out off the session.
 *
 * Throws the backend's error.
 */
export async function spellSuggestions(word: string): Promise<string[]> {
  const answer = await commands.spellSuggest(word);
  if (answer.status === 'error') {
    throw answer.error;
  }
  return answer.data;
}

/**
 * UC-005 A28 step 2: adds `word` to the user's dictionary; every note is
 * checked again, so it loses its mark everywhere. The reason when it could
 * not be added, else `null`.
 */
export async function addToDictionary(word: string): Promise<string | null> {
  try {
    const answer = await commands.spellAddWord(word);
    return answer.status === 'error' ? answer.error.message : null;
  } catch (error) {
    return messageOf(error);
  }
}
