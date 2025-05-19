use anyhow::{Result, anyhow};
use std::fs;
use std::path::PathBuf;
use sui_sdk::SuiClientBuilder;
use sui_sdk::types::base_types::{SuiAddress, ObjectID};
use sui_sdk::types::crypto::{EncodeDecodeBase64, SuiKeyPair, get_key_pair_from_rng, Signature};
use sui_sdk::rpc_types::{
    ObjectChange, SuiObjectDataOptions, SuiTransactionBlockEffectsAPI,
    SuiTransactionBlockResponseOptions, // SuiTransactionBlockResponse removed as unused for now
};
use sui_sdk::types::transaction::{TransactionData, CallArg, Transaction};
use sui_types::{Intent, IntentMessage}; // Use sui-types directly for Intent, from root
use sui_sdk::types::programmable_transaction_builder::ProgrammableTransactionBuilder;
// use sui_sdk::json::SuiJsonValue; // Removed as unused
use std::str::FromStr;
use sui_sdk::types::object::Owner;
use move_core_types::identifier::Identifier; // Using this again

const SUI_KEYPAIR_FILE: &str = "./sui_config/sui.keystore";
const SUI_PACKAGE_ID_STR: &str = "0xe99f0a2f17480d0859a5eb3c565a9f6ea3cbe4a7dec819dbacdb37f5ee33f482";
const PRICE_ORACLE_MODULE_NAME: &str = "price_oracle";

pub struct OracleSuiClient {
    sui_client: sui_sdk::SuiClient,
    keypair: SuiKeyPair,
    active_address: SuiAddress,
    package_id: ObjectID,
}

impl OracleSuiClient {
    pub async fn new(rpc_url: Option<String>) -> Result<Self> {
        let rpc_to_use =
            rpc_url.unwrap_or_else(|| "https://fullnode.testnet.sui.io:443".to_string());
        log::info!("Connecting to Sui network using RPC: {}", rpc_to_use);
        let sui_client = SuiClientBuilder::default().build(rpc_to_use).await?;

        let keypair_path = PathBuf::from(SUI_KEYPAIR_FILE);
        log::info!(
            "Attempting to load keypair from: {:?}",
            keypair_path
                .canonicalize()
                .unwrap_or_else(|_| keypair_path.clone())
        );

        let keypair = if keypair_path.exists() {
            log::info!("Keypair file found. Attempting to load.");
            let content = fs::read_to_string(&keypair_path)?;
            let first_key_str = content
                .lines()
                .next()
                .ok_or_else(|| anyhow!("Keystore file is empty or invalid."))?;

            match SuiKeyPair::decode_base64(first_key_str) {
                Ok(kp) => {
                    log::info!("Successfully loaded keypair from file.");
                    kp
                }
                Err(e) => {
                    log::error!(
                        "Failed to decode keypair from file content \'{}\': {}. Falling back to generated keypair.",
                        first_key_str,
                        e
                    );
                    let (_scheme, kp) = get_key_pair_from_rng(&mut rand::rngs::OsRng);
                    SuiKeyPair::Ed25519(kp)
                }
            }
        } else {
            log::warn!(
                "Sui keypair file not found at: {}. Generating a new ed25519 keypair. THIS IS NOT SUITABLE FOR PRODUCTION.",
                SUI_KEYPAIR_FILE
            );
            let (_scheme, kp) = get_key_pair_from_rng(&mut rand::rngs::OsRng);
            SuiKeyPair::Ed25519(kp)
        };

        let active_address = SuiAddress::from(&keypair.public());
        let package_id = ObjectID::from_str(SUI_PACKAGE_ID_STR)?;

        log::info!("Sui client initialized. Active address: {}", active_address);
        log::info!("Using Package ID: {}", package_id);

        Ok(Self {
            sui_client,
            keypair,
            active_address,
            package_id,
        })
    }

    pub fn active_address(&self) -> SuiAddress {
        self.active_address
    }

    pub async fn get_sui_balance(&self) -> Result<u64> {
        let coins = self
            .sui_client
            .coin_read_api()
            .get_coins(self.active_address, None, None, None)
            .await?;
        Ok(coins.data.iter().map(|c| c.balance).sum())
    }

    pub async fn create_price_object_on_sui(
        &self,
        symbol_str: &str,
        initial_price: u64,
        initial_timestamp_ms: u64,
        decimals_val: u8,
    ) -> Result<ObjectID> {
        log::info!(
            "Attempting to create PriceObject for symbol: {}, price: {}, ts: {}, decimals: {}",
            symbol_str,
            initial_price,
            initial_timestamp_ms,
            decimals_val
        );

        let mut pt_builder = ProgrammableTransactionBuilder::new();
        let symbol_vec = symbol_str.as_bytes().to_vec();

        pt_builder.move_call(
            self.package_id,
            Identifier::from_str(PRICE_ORACLE_MODULE_NAME)?,
            Identifier::from_str("create_price_object")?,
            vec![], // type_arguments: none for this call
            vec![
                CallArg::Pure(bcs::to_bytes(&symbol_vec)?),
                CallArg::Pure(bcs::to_bytes(&initial_price)?),
                CallArg::Pure(bcs::to_bytes(&initial_timestamp_ms)?),
                CallArg::Pure(bcs::to_bytes(&decimals_val)?),
            ],
        )?;

        let gas_budget = 30_000_000;
        let gas_price = self.sui_client.read_api().get_reference_gas_price().await?;
        
        let gas_coins = self
            .sui_client
            .coin_read_api()
            .get_coins(self.active_address, None, None, Some(1))
            .await?;
        let gas_object_ref = gas_coins
            .data
            .get(0)
            .ok_or_else(|| anyhow!("No gas coins found for address {}", self.active_address))?
            .object_ref();

        let tx_data = TransactionData::new_programmable(
            self.active_address,
            vec![gas_object_ref],
            pt_builder.finish(),
            gas_budget,
            gas_price,
        );

        let signature = Signature::new_secure(
            &IntentMessage::new(Intent::sui_transaction(), tx_data.clone()),
             &self.keypair
        );
        let transaction = Transaction::new_with_signatures(tx_data, vec![signature]);

        let response_options = SuiTransactionBlockResponseOptions::new()
            .with_effects()
            .with_object_changes();

        let tx_response = self
            .sui_client
            .quorum_driver_api()
            .execute_transaction_block(transaction, response_options, None)
            .await?;

        log::info!(
            "Create PriceObject transaction digest: {:?}",
            tx_response.digest
        );

        if let Some(effects) = tx_response.effects {
            if effects.status().is_ok() {
                if let Some(obj_changes) = tx_response.object_changes {
                    for change in obj_changes {
                        if let ObjectChange::Created {
                            object_id,
                            owner: object_owner,
                            object_type, // We get object_type here
                            ..
                        } = change
                        {
                            if object_owner == Owner::AddressOwner(self.active_address) {
                                // Check if the created object is our PriceObject
                                if let Some(tag) = object_type {
                                    let package_id_str = self.package_id.to_string();
                                    if tag.module_utf8_lossy() == PRICE_ORACLE_MODULE_NAME
                                        && tag.name_utf8_lossy() == "PriceObject"
                                        && tag.address_str_lossy() == package_id_str
                                    {
                                        log::info!(
                                            "Successfully created PriceObject with ID: {}",
                                            object_id
                                        );
                                        return Ok(object_id);
                                    }
                                }
                            }
                        }
                    }
                    log::error!("Could not find created PriceObject ID in transaction object_changes: {:?}", obj_changes);
                    Err(anyhow!("Failed to find created PriceObject ID in transaction object_changes."))
                } else {
                    Err(anyhow!("Object changes not available in transaction effects for create_price_object call."))
                }
            } else {
                log::error!("Create PriceObject transaction failed: {:?}", effects.status());
                Err(anyhow!("Create PriceObject transaction failed: {:?}", effects.status()))
            }
        } else {
            Err(anyhow!(
                "Transaction effects not available for create_price_object call."
            ))
        }
    }

    pub async fn update_price_object_on_sui(
        &self,
        price_object_id: ObjectID,
        new_price: u64,
        new_timestamp_ms: u64,
    ) -> Result<()> {
        log::info!(
            "Attempting to update PriceObject {} with price: {}, ts: {}",
            price_object_id,
            new_price,
            new_timestamp_ms
        );

        let mut pt_builder = ProgrammableTransactionBuilder::new();

        let object_options = SuiObjectDataOptions::new().with_owner();
        let object_info = self
            .sui_client
            .read_api()
            .get_object_with_options(price_object_id, object_options)
            .await?
            .data
            .ok_or_else(|| {
                anyhow!(
                    "Failed to get object info for PriceObject {}",
                    price_object_id
                )
            })?;

        let owned_object_ref = object_info.object_ref();

        pt_builder.move_call(
            self.package_id,
            Identifier::from_str(PRICE_ORACLE_MODULE_NAME)?,
            Identifier::from_str("update_price")?,
            vec![], // type_arguments: none
            vec![
                CallArg::Object(sui_sdk::types::transaction::ObjectArg::ImmOrOwnedObject(
                    owned_object_ref,
                )),
                CallArg::Pure(bcs::to_bytes(&new_price)?),
                CallArg::Pure(bcs::to_bytes(&new_timestamp_ms)?),
            ],
        )?;

        let gas_budget = 30_000_000;
        let gas_price = self.sui_client.read_api().get_reference_gas_price().await?;
        
        let gas_coins = self
            .sui_client
            .coin_read_api()
            .get_coins(self.active_address, None, None, Some(1))
            .await?;
        let gas_object_ref = gas_coins
            .data
            .get(0)
            .ok_or_else(|| anyhow!("No gas coins found for address {}", self.active_address))?
            .object_ref();

        let tx_data = TransactionData::new_programmable(
            self.active_address,
            vec![gas_object_ref],
            pt_builder.finish(),
            gas_budget,
            gas_price,
        );

        let signature = Signature::new_secure(
            &IntentMessage::new(Intent::sui_transaction(), tx_data.clone()),
            &self.keypair
        );
        let transaction = Transaction::new_with_signatures(tx_data, vec![signature]);

        let response_options = SuiTransactionBlockResponseOptions::new().with_effects();

        let tx_response = self
            .sui_client
            .quorum_driver_api()
            .execute_transaction_block(transaction, response_options, None)
            .await?;

        log::info!(
            "Update PriceObject transaction digest: {:?}",
            tx_response.digest
        );

        if let Some(effects) = tx_response.effects {
            if effects.status().is_ok() {
                log::info!("Successfully updated PriceObject {}", price_object_id);
                Ok(())
            } else {
                log::error!(
                    "Failed to update PriceObject {}: {:?}",
                    price_object_id,
                    effects.status()
                );
                Err(anyhow!(
                    "Update PriceObject transaction failed: {:?}",
                    effects.status()
                ))
            }
        } else {
            Err(anyhow!(
                "Transaction effects not available for update_price call."
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use chrono::Utc; // Added for the test

    const TEST_KEYPAIR_FILE: &str = "./test_sui.keystore";
    const TEST_VALID_ED25519_KEYPAIR_BASE64: &str = "ANRj4Rx5FZRehqwrctiLgZDPrY/3tI5+uJLCdaXPCj6C";

    fn setup_temp_keypair_file(content: &str) -> Result<PathBuf> {
        let path = PathBuf::from(TEST_KEYPAIR_FILE);
        let mut file = fs::File::create(&path)?;
        writeln!(file, "{}", content)?;
        Ok(path)
    }

    fn cleanup_temp_keypair_file() {
        let path = PathBuf::from(TEST_KEYPAIR_FILE);
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }

    #[tokio::test]
    async fn test_sui_client_new_with_generated_keypair_if_file_not_found() {
        cleanup_temp_keypair_file();
        let _original_file_const = SUI_KEYPAIR_FILE;
        let client_result = OracleSuiClient::new(Some("http://127.0.0.1:1".to_string())).await;
        assert!(client_result.is_ok());
        let client = client_result.unwrap();
        assert_ne!(client.active_address(), SuiAddress::ZERO);
        log::info!(
            "Tested generated keypair. Address: {}",
            client.active_address()
        );
    }

    #[tokio::test]
    async fn test_sui_client_new_with_valid_keypair_file() -> Result<()> {
        setup_temp_keypair_file(TEST_VALID_ED25519_KEYPAIR_BASE64)?;

        let _ = fs::create_dir_all("./sui_config/");
        let original_sui_keypair_file_path = PathBuf::from(SUI_KEYPAIR_FILE);
        fs::copy(TEST_KEYPAIR_FILE, &original_sui_keypair_file_path)?;

        let client_result = OracleSuiClient::new(Some("http://127.0.0.1:1".to_string())).await;
        cleanup_temp_keypair_file();
        let _ = fs::remove_file(&original_sui_keypair_file_path);

        assert!(
            client_result.is_ok(),
            "Client creation failed: {:?}",
            client_result.err()
        );
        let client = client_result.unwrap();

        assert_ne!(client.active_address(), SuiAddress::ZERO);
        log::info!(
            "Tested loading valid keypair from file. Address: {}",
            client.active_address()
        );

        let expected_kp = SuiKeyPair::decode_base64(TEST_VALID_ED25519_KEYPAIR_BASE64).unwrap();
        assert_eq!(
            client.active_address(),
            SuiAddress::from(&expected_kp.public())
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_sui_client_init_and_get_balance() {
        let client_result = OracleSuiClient::new(None).await;
        assert!(client_result.is_ok());
        let client = client_result.unwrap();
        log::info!(
            "[Test] Active address for balance check: {}",
            client.active_address()
        );

        let balance_result = client.get_sui_balance().await;
        assert!(
            balance_result.is_ok(),
            "Failed to get balance: {:?}",
            balance_result.err()
        );
        log::info!("[Test] SUI balance: {}", balance_result.unwrap());
    }

    #[tokio::test]
    #[ignore] // This test requires a live Sui network (testnet) and a funded address.
                // Also, the keypair used here must match an address that has SUI tokens on testnet.
                // And the SUI_PACKAGE_ID must be deployed on that network.
                // For testing, you might need to pre-fund the address associated with SUI_KEYPAIR_FILE
                // or the one generated if the file doesn't exist.
    async fn test_create_and_update_price_object_on_sui() -> Result<()> {
        // Ensure SUI_KEYPAIR_FILE points to a keypair with funds on testnet
        // and SUI_PACKAGE_ID is deployed.
        // For testing, you might need to pre-fund the address associated with SUI_KEYPAIR_FILE
        // or the one generated if the file doesn't exist.

        // Use the default testnet RPC for this integration test
        let client = OracleSuiClient::new(None).await?;
        log::info!("[Test] Using address: {} for create/update test", client.active_address());

        // You'll need SUI in this account on testnet to pay for gas.
        let balance = client.get_sui_balance().await?;
        log::info!("[Test] Account balance: {} MIST", balance);
        assert!(balance > 50_000_000, "Insufficient balance to run create/update test. Please fund address {}. Current balance: {} MIST", client.active_address(), balance);

        let symbol = "BTC/USD_TEST_RUST"; // Changed slightly to avoid collision if you ran before
        let initial_price = 70000_00000000; // 70,000 with 8 decimals
        let initial_ts = Utc::now().timestamp_millis() as u64;
        let decimals = 8u8;

        let price_object_id = client
            .create_price_object_on_sui(symbol, initial_price, initial_ts, decimals)
            .await?;
        
        log::info!("[Test] Created PriceObject on Sui with ID: {}", price_object_id);
        assert_ne!(price_object_id, ObjectID::ZERO); // Check it's not a zero ID

        // Optional: Add a small delay to allow the network to process, though usually not needed for sequential owned object calls by same client
        // tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let updated_price = 71000_00000000; // 71,000
        let updated_ts = Utc::now().timestamp_millis() as u64;

        client
            .update_price_object_on_sui(price_object_id, updated_price, updated_ts)
            .await?;
        
        log::info!("[Test] Successfully updated PriceObject {} on Sui", price_object_id);
        
        // To verify further, you could add a function to OracleSuiClient to fetch and deserialize PriceObject data.
        // For now, a successful transaction for update is our primary check.

        Ok(())
    }
}
