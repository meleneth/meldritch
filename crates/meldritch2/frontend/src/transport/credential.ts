export interface CredentialLocation {
  hash: string;
}

/** Resolve a musician access link without sending its credential to the server. */
export function credentialFromLocation(location: CredentialLocation) {
  const fragment = location.hash.replace(/^#\??/, "");
  return new URLSearchParams(fragment).get("credential") || "";
}
