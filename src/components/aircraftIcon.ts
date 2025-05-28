import { toValue } from 'vue';

export function silhouetteUrl(icaoTypeCode: string): string {
  const typeCode = toValue(icaoTypeCode);

  if (!icaoTypeCode || icaoTypeCode.length === 0) {
    return '';
  } else {
    return `/silhouettes/${typeCode.toLowerCase()}.png`;
  }
}
