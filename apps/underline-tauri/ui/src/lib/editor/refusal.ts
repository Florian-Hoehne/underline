// UC-024 A3 to A6: why a change of a note attribute was not written, and the
// field it is about, as the views answer it.

import type { AttributeFieldName } from '../bindings';

/** Why nothing was written, and what about (`null`: not known). */
export type Refusal = { problem: string; about: AttributeFieldName | null };
