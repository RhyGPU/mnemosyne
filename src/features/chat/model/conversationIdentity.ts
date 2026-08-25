import type { AssistantMessageVariant } from "../../../tauri";

export function selectedVariantIndex(variants: AssistantMessageVariant[]) {
  const index = variants.findIndex((variant) => variant.is_selected);
  return index >= 0 ? index : 0;
}

export function conversationIdForSoul(soulId: string) {
  return `local-mock-${soulId}`;
}

export function conversationIdForSettingAndSoul(settingId: string, soulId: string) {
  return `local-mock-${settingId}-${soulId}`;
}
