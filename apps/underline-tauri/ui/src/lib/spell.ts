// FR-099, UC-005 A28 (ADR-030, OQ-207, OQ-208): the spell check's
// commands for the spell menu of both editing views.

import { commands } from './bindings';
import { messageOf } from './editor/sync';

/**
 * UC-005 A28 step 1: up to five corrections for `word` of `note`, the
 * likeliest first, in the language the note names (OQ-207), else in the
 * spell check language; the backend works them out off the session.
 *
 * Throws the backend's error.
 */
export async function spellSuggestions(word: string, note: string): Promise<string[]> {
  const answer = await commands.spellSuggest(word, note);
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

/**
 * OQ-208, UC-005 A28 step 2: adds `word` to the workspace dictionary, shared
 * through git; every note is checked again. The reason when it could not
 * be added, else `null`.
 */
export async function addToWorkspaceDictionary(word: string): Promise<string | null> {
  try {
    const answer = await commands.spellAddWorkspaceWord(word);
    return answer.status === 'error' ? answer.error.message : null;
  } catch (error) {
    return messageOf(error);
  }
}
