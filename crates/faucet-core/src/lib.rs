//! Shared types and validation for the Zcash testnet faucet.
//!
//! This crate is compiled both natively (the signer and the Worker logic) and
//! to `wasm32` (the Worker runtime and, via `faucet-addr-wasm`, the browser).
//! Keep its dependency surface small and wasm-friendly.
//!
//! The address validation here is the single source of truth for which
//! destination addresses the faucet accepts. It reuses the official
//! `zcash_address` parser so the browser, the Worker, and the signer all agree
//! byte-for-byte instead of reimplementing address parsing per layer.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use zcash_address::{ConversionError, TryFromAddress, ZcashAddress};
use zcash_protocol::{PoolType, consensus::NetworkType};

/// Zcash network the faucet operates on. This faucet is testnet only, but the
/// network is explicit so validation can reject mainnet addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Testnet,
    Mainnet,
}

impl From<Network> for NetworkType {
    fn from(n: Network) -> Self {
        match n {
            Network::Testnet => NetworkType::Test,
            Network::Mainnet => NetworkType::Main,
        }
    }
}

/// Value pool a destination address will receive into. Sapling is intentionally
/// excluded: the faucet neither builds nor accepts Sapling outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pool {
    Transparent,
    Orchard,
}

/// Why a destination address was rejected. Each variant maps to a user-facing
/// message via [`AddressRejection::message`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressRejection {
    /// Not a parseable Zcash address.
    Unparseable,
    /// Parsed, but for the wrong network (e.g. a mainnet address on testnet).
    WrongNetwork,
    /// A Sapling address, which this faucet does not support.
    SaplingUnsupported,
    /// A legacy Sprout address, which this faucet does not support.
    SproutUnsupported,
    /// A Unified Address exposing neither a transparent nor an Orchard receiver.
    NoSupportedReceiver,
}

impl AddressRejection {
    /// A short, user-facing explanation suitable for display in the frontend.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            Self::Unparseable => "Not a valid Zcash address.",
            Self::WrongNetwork => "This is not a testnet address.",
            Self::SaplingUnsupported => {
                "Sapling addresses are not supported. Use a transparent or \
                 Orchard (unified) address."
            }
            Self::SproutUnsupported => "Sprout addresses are not supported.",
            Self::NoSupportedReceiver => {
                "This unified address has no transparent or Orchard receiver."
            }
        }
    }
}

impl core::fmt::Display for AddressRejection {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.message())
    }
}

/// A destination address that passed validation, with the pool the faucet will
/// send into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidAddress {
    pub pool: Pool,
}

/// Internal classifier: maps a parsed Zcash address to the faucet pool, or to a
/// typed rejection reason carried through `ConversionError::User`.
struct Classified(Pool);

enum ClassifyErr {
    Sapling,
    Sprout,
    NoSupportedReceiver,
}

impl TryFromAddress for Classified {
    type Error = ClassifyErr;

    fn try_from_transparent_p2pkh(
        _net: NetworkType,
        _data: [u8; 20],
    ) -> Result<Self, ConversionError<Self::Error>> {
        Ok(Self(Pool::Transparent))
    }

    fn try_from_transparent_p2sh(
        _net: NetworkType,
        _data: [u8; 20],
    ) -> Result<Self, ConversionError<Self::Error>> {
        Ok(Self(Pool::Transparent))
    }

    fn try_from_tex(
        _net: NetworkType,
        _data: [u8; 20],
    ) -> Result<Self, ConversionError<Self::Error>> {
        // TEX is a transparent-source-restricted P2PKH; still a transparent
        // destination as far as the faucet is concerned.
        Ok(Self(Pool::Transparent))
    }

    fn try_from_sapling(
        _net: NetworkType,
        _data: [u8; 43],
    ) -> Result<Self, ConversionError<Self::Error>> {
        Err(ConversionError::User(ClassifyErr::Sapling))
    }

    fn try_from_sprout(
        _net: NetworkType,
        _data: [u8; 64],
    ) -> Result<Self, ConversionError<Self::Error>> {
        Err(ConversionError::User(ClassifyErr::Sprout))
    }

    fn try_from_unified(
        _net: NetworkType,
        data: zcash_address::unified::Address,
    ) -> Result<Self, ConversionError<Self::Error>> {
        // Prefer Orchard; fall back to a transparent receiver; reject a
        // unified address that only carries Sapling (or unknown) receivers.
        if data.has_receiver_of_type(PoolType::ORCHARD) {
            Ok(Self(Pool::Orchard))
        } else if data.has_receiver_of_type(PoolType::TRANSPARENT) {
            Ok(Self(Pool::Transparent))
        } else {
            Err(ConversionError::User(ClassifyErr::NoSupportedReceiver))
        }
    }
}

/// Validate a destination address for a faucet drip on the given network.
///
/// Accepts transparent (P2PKH, P2SH, TEX) addresses and unified addresses that
/// expose an Orchard or transparent receiver. Rejects Sapling, Sprout,
/// wrong-network, and unparseable addresses.
///
/// # Errors
///
/// Returns an [`AddressRejection`] describing why the address was rejected.
pub fn validate_destination(
    addr: &str,
    network: Network,
) -> Result<ValidAddress, AddressRejection> {
    let parsed: ZcashAddress = addr
        .trim()
        .parse()
        .map_err(|_| AddressRejection::Unparseable)?;

    match parsed.convert_if_network::<Classified>(network.into()) {
        Ok(Classified(pool)) => Ok(ValidAddress { pool }),
        Err(ConversionError::IncorrectNetwork { .. }) => Err(AddressRejection::WrongNetwork),
        Err(ConversionError::User(ClassifyErr::Sapling)) => {
            Err(AddressRejection::SaplingUnsupported)
        }
        Err(ConversionError::User(ClassifyErr::Sprout)) => Err(AddressRejection::SproutUnsupported),
        Err(ConversionError::User(ClassifyErr::NoSupportedReceiver)) => {
            Err(AddressRejection::NoSupportedReceiver)
        }
        // All address kinds are handled above, so Unsupported cannot occur;
        // treat it defensively as unparseable rather than panicking.
        Err(ConversionError::Unsupported(_)) => Err(AddressRejection::Unparseable),
    }
}

/// Request body the frontend sends to the Worker to ask for a drip. The amount
/// and pool are decided server-side; the client only supplies its address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DripRequest {
    pub address: String,
    /// Optional memo, attached to the shielded (Orchard) output. Ignored for
    /// transparent destinations (transparent outputs cannot carry a memo).
    #[serde(default)]
    pub memo: Option<String>,
}

/// Successful drip result returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DripResponse {
    pub txid: String,
    pub pool: Pool,
    pub amount_zat: u64,
}

/// Request the Worker sends to the signer service over the authenticated
/// channel. The signer decides nothing about policy; it only builds and sends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerSendRequest {
    pub address: String,
    pub amount_zat: u64,
    pub pool: Pool,
    /// Optional memo for the shielded output (max 512 bytes). Only valid when
    /// `pool` is `Orchard`.
    #[serde(default)]
    pub memo: Option<String>,
}

/// Maximum Zcash memo size, in bytes.
pub const MEMO_MAX_BYTES: usize = 512;

/// Response from the signer service after broadcasting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerSendResponse {
    pub txid: String,
}

/// The faucet's on-chain reserves, split by pool, plus its receiving address
/// and sync position. Returned by the signer `/balance` and surfaced to the
/// frontend so users can see the faucet's funds. Amounts are in zatoshis
/// (1 ZEC = 100_000_000 zat). Transparent funds (mining coinbase) are shielded
/// into Orchard before they can be sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaucetBalanceResponse {
    pub unified_address: String,
    pub chain_tip: u32,
    pub fully_scanned: u32,
    pub transparent_total_zat: u64,
    pub orchard_spendable_zat: u64,
    pub orchard_total_zat: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use zcash_address::unified;

    #[test]
    fn rejects_garbage() {
        assert_eq!(
            validate_destination("not-an-address", Network::Testnet),
            Err(AddressRejection::Unparseable)
        );
        assert_eq!(
            validate_destination("", Network::Testnet),
            Err(AddressRejection::Unparseable)
        );
    }

    #[test]
    fn rejects_mainnet_transparent_on_testnet() {
        // A mainnet transparent P2PKH address.
        let mainnet_t = "t1Hsc1LR8yKnbbe3twRp88p6vFfC5t7DLbs";
        assert_eq!(
            validate_destination(mainnet_t, Network::Testnet),
            Err(AddressRejection::WrongNetwork)
        );
    }

    // Real testnet vectors taken from the zcash_address crate test data.
    const TESTNET_T: &str = "tm9iMLAuYMzJ6jtFLcA7rzUmfreGuKvr7Ma";
    const TESTNET_UA: &str = "utest10c5kutapazdnf8ztl3pu43nkfsjx89fy3uuff8tsmxm6s86j37pe7uz94z5jhkl49pqe8yz75rlsaygexk6jpaxwx0esjr8wm5ut7d5s";
    const TESTNET_SAPLING: &str =
        "ztestsapling1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqfhgwqu";

    #[test]
    fn accepts_testnet_transparent() {
        assert_eq!(
            validate_destination(TESTNET_T, Network::Testnet),
            Ok(ValidAddress {
                pool: Pool::Transparent
            })
        );
    }

    #[test]
    fn trims_whitespace() {
        let padded = format!("  {TESTNET_T}  ");
        assert_eq!(
            validate_destination(&padded, Network::Testnet),
            Ok(ValidAddress {
                pool: Pool::Transparent
            })
        );
    }

    /// Build a testnet unified address string from a set of receivers, so the
    /// pool-selection logic can be tested for each receiver combination.
    fn testnet_ua(receivers: Vec<unified::Receiver>) -> String {
        use unified::Encoding;
        unified::Address::try_from_items(receivers)
            .expect("valid unified address")
            .encode(&NetworkType::Test)
    }

    #[test]
    fn unified_with_orchard_is_orchard() {
        let ua = testnet_ua(vec![unified::Receiver::Orchard([1u8; 43])]);
        assert_eq!(
            validate_destination(&ua, Network::Testnet),
            Ok(ValidAddress {
                pool: Pool::Orchard
            })
        );
    }

    #[test]
    fn unified_orchard_and_transparent_prefers_orchard() {
        let ua = testnet_ua(vec![
            unified::Receiver::Orchard([2u8; 43]),
            unified::Receiver::P2pkh([3u8; 20]),
        ]);
        assert_eq!(
            validate_destination(&ua, Network::Testnet),
            Ok(ValidAddress {
                pool: Pool::Orchard
            })
        );
    }

    #[test]
    fn unified_sapling_only_is_rejected() {
        // The bundled TESTNET_UA vector carries only a Sapling receiver.
        assert_eq!(
            validate_destination(TESTNET_UA, Network::Testnet),
            Err(AddressRejection::NoSupportedReceiver)
        );
    }

    #[test]
    fn rejects_testnet_sapling() {
        assert_eq!(
            validate_destination(TESTNET_SAPLING, Network::Testnet),
            Err(AddressRejection::SaplingUnsupported)
        );
    }
}
