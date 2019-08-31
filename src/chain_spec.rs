use hex_literal::hex;
use primitives::{crypto::UncheckedInto, ed25519, sr25519, Pair};
use substrate_kitties_runtime::{
    AccountId, BalancesConfig, ConsensusConfig, GenesisConfig, IndicesConfig, SudoConfig,
    TimestampConfig,
};
use substrate_service;
use substrate_telemetry::TelemetryEndpoints;

use ed25519::Public as AuthorityId;

// Note this is the URL for the telemetry server
//const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig>;

/// The chain specification option. This is expected to come in from the CLI and
/// is little more than one of a number of alternatives which can easily be converted
/// from a string (`--chain=...`) into a `ChainSpec`.
#[derive(Clone, Debug)]
pub enum Alternative {
    /// Whatever the current runtime is, with just Alice as an auth.
    Development,
    /// Whatever the current runtime is, with simple Alice/Bob auths.
    LocalTestnet,
    DemoTestnet,
    DemoTestnetLatest,
}

fn authority_key(s: &str) -> AuthorityId {
    ed25519::Pair::from_string(&format!("//{}", s), None)
        .expect("static values are valid; qed")
        .public()
}

fn account_key(s: &str) -> AccountId {
    sr25519::Pair::from_string(&format!("//{}", s), None)
        .expect("static values are valid; qed")
        .public()
}

impl Alternative {
    /// Get an actual chain config from one of the alternatives.
    pub(crate) fn load(self) -> Result<ChainSpec, String> {
        Ok(match self {
            Alternative::Development => ChainSpec::from_genesis(
                "Development",
                "dev",
                || {
                    testnet_genesis(
                        vec![authority_key("Alice")],
                        vec![
                            account_key("Alice"),
                            account_key("Bob"),
                            account_key("Charlie"),
                            account_key("Dave"),
                            account_key("Eve"),
                            account_key("Ferdie"),
                        ],
                        account_key("Alice"),
                    )
                },
                vec![],
                None,
                None,
                None,
                None,
            ),
            Alternative::LocalTestnet => ChainSpec::from_genesis(
                "Local Testnet",
                "local_testnet",
                || {
                    testnet_genesis(
                        vec![authority_key("Alice"), authority_key("Bob")],
                        vec![
                            account_key("Alice"),
                            account_key("Bob"),
                            account_key("Charlie"),
                            account_key("Dave"),
                            account_key("Eve"),
                            account_key("Ferdie"),
                        ],
                        account_key("Alice"),
                    )
                },
                vec![],
                None,
                None,
                None,
                None,
            ),
            Alternative::DemoTestnet => {
                ChainSpec::from_embedded(include_bytes!("../genesis/demo.json"))
                    .map_err(|e| format!("Error loading demo genesis {}", e))?
            }
            Alternative::DemoTestnetLatest => {
                ChainSpec::from_genesis(
                    "Substrate Kitty",
                    "sub_kitty",
                    || {
                        demonet_genesis(
                        vec![hex!["4dd27440e20325e8130d42f39d7224ba98a7ddb70e4179d759ff948f9f7909df"].unchecked_into()],
                        vec![
                            hex!["b09529548f342639c244d0ba3c2ad9a1a59484d51e850dcbe23b679cc710b703"].unchecked_into(),
                            hex!["d73ea23e15bbbd579fbcdeed65ad7d3c2242c83b75cf93ea50281c3cac7d5141"].unchecked_into(),
                            hex!["1556615d41e3cc6cf1f1d8a1204c1a653d8e2f549c22c5950a18506617d33d23"].unchecked_into(),
                            hex!["6f4dda8c20743474d9b3dadbf2a91a6696010aae01e4d1d3f1e21d0a19aa2623"].unchecked_into(),
                            hex!["f7e722d7ff5bbf122f72b728db39f9f9e02fac350a1c874363fd1234436e281e"].unchecked_into(),
                            hex!["c53308f6aa60663700587e4364da2d1e5ddcf360dfc1c9210362f506438ccb57"].unchecked_into(),
                        ],
                        hex!["e06b2b273fd42134ef5980d0feb6a0600728c54ecddb4de16114886fd41aa504"].unchecked_into(),
                    )
                    },
                    vec![],
                    Some(TelemetryEndpoints::new(vec![(
                        "wss://telemetry.polkadot.io/submit/".into(),
                        0,
                    )])),
                    Some("subkitty"),
                    None,
                    None,
                )
            }
        })
    }

    pub(crate) fn from(s: &str) -> Option<Self> {
        match s {
            "dev" => Some(Alternative::Development),
            "local" => Some(Alternative::LocalTestnet),
            "" | "demo" => Some(Alternative::DemoTestnet),
            "demo-latest" => Some(Alternative::DemoTestnetLatest),
            _ => None,
        }
    }
}

fn testnet_genesis(
    initial_authorities: Vec<AuthorityId>,
    endowed_accounts: Vec<AccountId>,
    root_key: AccountId,
) -> GenesisConfig {
    GenesisConfig {
		consensus: Some(ConsensusConfig {
			code: include_bytes!("../runtime/wasm/target/wasm32-unknown-unknown/release/substrate_kitties_runtime_wasm.compact.wasm").to_vec(),
			authorities: initial_authorities.clone(),
		}),
		system: None,
		timestamp: Some(TimestampConfig {
			minimum_period: 2, // 4 second block time.
		}),
		indices: Some(IndicesConfig {
			ids: endowed_accounts.clone(),
		}),
		balances: Some(BalancesConfig {
			transaction_base_fee: 1,
			transaction_byte_fee: 0,
			existential_deposit: 500,
			transfer_fee: 0,
			creation_fee: 0,
			balances: endowed_accounts.iter().cloned().map(|k|(k, 1 << 60)).collect(),
			vesting: vec![],
		}),
		sudo: Some(SudoConfig {
			key: root_key,
		}),
	}
}

fn demonet_genesis(
    initial_authorities: Vec<AuthorityId>,
    endowed_accounts: Vec<AccountId>,
    root_key: AccountId,
) -> GenesisConfig {
    GenesisConfig {
		consensus: Some(ConsensusConfig {
			code: include_bytes!("../runtime/wasm/target/wasm32-unknown-unknown/release/substrate_kitties_runtime_wasm.compact.wasm").to_vec(),
			authorities: initial_authorities.clone(),
		}),
		system: None,
		timestamp: Some(TimestampConfig {
			minimum_period: 6, // 12 second block time.
		}),
		indices: Some(IndicesConfig {
			ids: endowed_accounts.clone(),
		}),
		balances: Some(BalancesConfig {
			transaction_base_fee: 0,
			transaction_byte_fee: 0,
			existential_deposit: 0,
			transfer_fee: 0,
			creation_fee: 0,
			balances: endowed_accounts.iter().cloned().map(|k|(k, 10u128.pow(18+6))).collect(),
			vesting: vec![],
		}),
		sudo: Some(SudoConfig {
			key: root_key,
		}),
	}
}
