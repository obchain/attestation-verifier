use core::fmt;

/// Why a single proof was rejected. Any of these makes the SP1 program abort,
/// so no proof is ever produced over a forged or foreign credential.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// `claimData.context` is not valid JSON.
    MalformedContext,
    /// `claimData.parameters` is not valid JSON or has no `url`.
    MalformedParameters,
    BadAddress,
    BadSignature,
    NoSignatures,
    /// No signature recovered to an attestor in the policy's allowlist.
    UntrustedAttestor,
    /// The attested URL is not an allowed endpoint of a supported platform.
    UnknownPlatform,
    /// The request / extraction spec is not one the policy pins (provider hash).
    UnpinnedProvider,
    /// `contextAddress` is not the wallet the proof is being generated for.
    WalletMismatch,
    /// `contextMessage` lacks the app's domain-separation tag.
    WrongContextTag,
    /// Attestation is older than the policy allows (or from the future).
    Stale,
    /// A required extracted field (e.g. course title) is missing.
    MissingField(&'static str),
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::MalformedContext => "claim context is not valid JSON",
            Self::MalformedParameters => "claim parameters are not valid JSON or lack a url",
            Self::BadAddress => "malformed address",
            Self::BadSignature => "malformed or invalid signature",
            Self::NoSignatures => "proof carries no signatures",
            Self::UntrustedAttestor => "no signature from a trusted attestor",
            Self::UnknownPlatform => "attested URL is not an allowed platform endpoint",
            Self::UnpinnedProvider => "request or extraction spec is not a pinned provider",
            Self::WalletMismatch => "proof is bound to a different wallet",
            Self::WrongContextTag => "context message lacks the expected app tag",
            Self::Stale => "attestation is outside the allowed time window",
            Self::MissingField(k) => return write!(f, "required field `{k}` not found"),
        };
        f.write_str(s)
    }
}

impl std::error::Error for VerifyError {}
