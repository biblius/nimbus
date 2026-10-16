/**
 * WebAuthn helpers matching the kanidm webauthn-rs tutorial client:
 * https://github.com/kanidm/webauthn-rs/blob/master/tutorial/server/axum/assets/js/auth.js
 */

export type CreationChallengeResponse = {
  publicKey: PublicKeyCredentialCreationOptionsJSON;
};

export type RequestChallengeResponse = {
  publicKey: PublicKeyCredentialRequestOptionsJSON;
};

export type PublicKeyCredentialCreationOptionsJSON = {
  challenge: string;
  rp: PublicKeyCredentialRpEntity;
  user: {
    id: string;
    name: string;
    displayName: string;
  };
  pubKeyCredParams: PublicKeyCredentialParameters[];
  timeout?: number;
  excludeCredentials?: {
    id: string;
    type: PublicKeyCredentialType;
    transports?: AuthenticatorTransport[];
  }[];
  authenticatorSelection?: AuthenticatorSelectionCriteria;
  attestation?: AttestationConveyancePreference;
  extensions?: AuthenticationExtensionsClientInputs;
};

export type PublicKeyCredentialRequestOptionsJSON = {
  challenge: string;
  timeout?: number;
  rpId?: string;
  allowCredentials?: {
    id: string;
    type: PublicKeyCredentialType;
    transports?: AuthenticatorTransport[];
  }[];
  userVerification?: UserVerificationRequirement;
  extensions?: AuthenticationExtensionsClientInputs;
};

export type RegisterPublicKeyCredentialJSON = {
  id: string;
  rawId: string;
  type: string;
  response: {
    attestationObject: string;
    clientDataJSON: string;
  };
};

export type AuthenticatePublicKeyCredentialJSON = {
  id: string;
  rawId: string;
  type: string;
  response: {
    authenticatorData: string;
    clientDataJSON: string;
    signature: string;
    userHandle: string | null;
  };
};

/** Decode a base64url string into bytes (WebAuthn wire format). */
function toUint8Array(value: string): Uint8Array {
  // Webauthn uses B64URL, we make it B64
  const padded = value.replace(/-/g, "+").replace(/_/g, "/");

  const remainder = padded.length % 4;

  let pad = "";

  if (remainder > 0) {
    pad = "=".repeat(4 - remainder);
  }

  const binary = atob(padded + pad);

  const bytes = new Uint8Array(binary.length);

  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }

  return bytes;
}

/** Encode bytes as base64url (no padding). */
function fromUint8Array(bytes: Uint8Array): string {
  let binary = "";

  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]!);
  }

  // B64 URL encode it, webauthn-rs expects it like this
  return btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

/**
 * Run `navigator.credentials.create` for a server-issued creation challenge.
 * Returns the credential JSON body expected by `POST /auth/passkey/setup/confirm`.
 */
export async function createPasskey(
  creationChallenge: CreationChallengeResponse,
): Promise<RegisterPublicKeyCredentialJSON> {
  const pubKey = creationChallenge.publicKey;

  // Browser wants ArrayBuffers, but server sends base64url strings
  // for binary fields, convert.
  const publicKey: PublicKeyCredentialCreationOptions = {
    challenge: toUint8Array(pubKey.challenge).buffer as ArrayBuffer,
    rp: pubKey.rp,
    user: {
      id: toUint8Array(pubKey.user.id).buffer as ArrayBuffer,
      name: pubKey.user.name,
      displayName: pubKey.user.displayName,
    },
    pubKeyCredParams: pubKey.pubKeyCredParams,
    timeout: pubKey.timeout,
    authenticatorSelection: pubKey.authenticatorSelection,
    attestation: pubKey.attestation,
    extensions: pubKey.extensions,
    excludeCredentials: pubKey.excludeCredentials?.map((cred) => ({
      id: toUint8Array(cred.id).buffer as ArrayBuffer,
      type: cred.type,
      transports: cred.transports,
    })),
  };

  const credential = (await navigator.credentials.create({
    publicKey,
  })) as PublicKeyCredential | null;

  if (
    !credential ||
    !(credential.response instanceof AuthenticatorAttestationResponse)
  ) {
    throw new Error("Passkey creation was cancelled or produced no credential");
  }

  // Browser gave ArrayBuffers; server expects base64url strings again.
  return {
    id: credential.id,
    rawId: fromUint8Array(new Uint8Array(credential.rawId)),
    type: credential.type,
    response: {
      attestationObject: fromUint8Array(
        new Uint8Array(credential.response.attestationObject),
      ),
      clientDataJSON: fromUint8Array(
        new Uint8Array(credential.response.clientDataJSON),
      ),
    },
  };
}

/**
 * Run `navigator.credentials.get` for a server-issued
 * authentication challenge.
 *
 * Returns the assertion JSON body expected by
 * `POST /auth/login/passkey/confirm`.
 */
export async function getPasskey(
  requestChallenge: RequestChallengeResponse,
): Promise<AuthenticatePublicKeyCredentialJSON> {
  const pubKey = requestChallenge.publicKey;

  const publicKey: PublicKeyCredentialRequestOptions = {
    challenge: toUint8Array(pubKey.challenge).buffer as ArrayBuffer,
    timeout: pubKey.timeout,
    rpId: pubKey.rpId,
    userVerification: pubKey.userVerification,
    extensions: pubKey.extensions,
    allowCredentials: pubKey.allowCredentials?.map((cred) => ({
      id: toUint8Array(cred.id).buffer as ArrayBuffer,
      type: cred.type,
      transports: cred.transports,
    })),
  };

  const credential = await navigator.credentials.get({ publicKey });
  const assertion = credential as PublicKeyCredential | null;

  if (
    !assertion ||
    !(assertion.response instanceof AuthenticatorAssertionResponse)
  ) {
    throw new Error(
      "Passkey authentication was cancelled or produced no credential",
    );
  }

  const userHandle = assertion.response.userHandle;

  return {
    id: assertion.id,
    rawId: fromUint8Array(new Uint8Array(assertion.rawId)),
    type: assertion.type,
    response: {
      authenticatorData: fromUint8Array(
        new Uint8Array(assertion.response.authenticatorData),
      ),
      clientDataJSON: fromUint8Array(
        new Uint8Array(assertion.response.clientDataJSON),
      ),
      signature: fromUint8Array(new Uint8Array(assertion.response.signature)),
      userHandle: userHandle
        ? fromUint8Array(new Uint8Array(userHandle))
        : null,
    },
  };
}
