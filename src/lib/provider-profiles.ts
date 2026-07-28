import type { ProviderProfile } from "../model";

export interface ProviderDraft { model: string; effort: string; }
export function providerModelOptions(models: string[], profiles: ProviderProfile[], hasAuthoritativeCatalog: boolean) { const catalogue = new Set(models); const savedModels = profiles.flatMap((profile) => profile.model ? [profile.model] : []); const unavailableSavedModels = hasAuthoritativeCatalog ? savedModels.filter((model) => !catalogue.has(model)) : []; return Array.from(new Set([...models, ...(!hasAuthoritativeCatalog ? savedModels : unavailableSavedModels)])).map((value) => ({ value, unavailable: hasAuthoritativeCatalog && !catalogue.has(value), label: hasAuthoritativeCatalog && !catalogue.has(value) ? `${value} (Unavailable: not in the current catalogue)` : value })); }
export function connectionTestDraft(draft: ProviderDraft): ProviderDraft { return { model: draft.model.trim(), effort: draft.effort }; }
