use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sui_sdk::rpc_types::{
    SuiObjectDataOptions, SuiTransactionBlockEffectsAPI, SuiTransactionBlockResponseOptions,
    SuiExecutionStatus,
};
use sui_sdk::types::base_types::{ObjectID, SuiAddress};
use sui_sdk::types::crypto::{SuiKeyPair, EncodeDecodeBase64, Signature as SuiSdkSignature};
use sui_sdk::types::programmable_transaction_builder::ProgrammableTransactionBuilder;
use sui_sdk::types::transaction::{CallArg, TransactionData, ObjectArg, Transaction};
use sui_sdk::SuiClient;
use sui_sdk::SuiClientBuilder;
use move_core_types::identifier::Identifier;
use shared_crypto::intent::{Intent, IntentMessage};
use sui_types::object::Owner;

// Constants
const PACKAGE_ID_STR: &str = "0xe99f0a2f17480d0859a5eb3c565a9f6ea3cbe4a7dec819dbacdb37f5ee33f482";
const MODULE_NAME: &str = "price_oracle";
const CREATE_PRICE_OBJECT_FUNC_NAME: &str = "create_price_object";
const UPDATE_PRICE_FUNC_NAME: &str = "update_price";

const PUBLISHER_PRIVATE_KEY_B64: &str = "ALiJ7ig1JDkCMh4/TL914LABL4HVntuoSXtf414NmW9K";
const PUBLISHER_ADDRESS_STR: &str = "0x267eb37d0b256d86f5fea3a86c895de51b23aa4d6abf13fc144b850fed4b7167";

const KNOWN_OBJECTS_FILENAME: &str = "known_price_objects.json";
const SUI_TESTNET_RPC_URL: &str = "https://fullnode.testnet.sui.io:443";
const DECIMALS: u8 = 6;
const GAS_BUDGET: u64 = 100_000_000;
const DEFAULT_GAS_PRICE: u64 = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceInfo {
    pub symbol: String,
    pub price: f64,
    pub timestamp_ms: u64,
}

type KnownObjectsMap = HashMap<String, ObjectID>;

fn get_publisher_keypair() -> Result<SuiKeyPair> {
    SuiKeyPair::decode_base64(PUBLISHER_PRIVATE_KEY_B64)
        .map_err(|e| anyhow!("Failed to decode base64 private key: {}", e))
}

fn load_known_objects() -> Result<KnownObjectsMap> {
    let path = Path::new(KNOWN_OBJECTS_FILENAME);
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let file = File::open(path).context(format!("Failed to open {}", KNOWN_OBJECTS_FILENAME))?;
    let reader = BufReader::new(file);
    let objects: KnownObjectsMap = serde_json::from_reader(reader)
        .context(format!("Failed to parse JSON from {}", KNOWN_OBJECTS_FILENAME))?;
    Ok(objects)
}

fn save_known_objects(objects: &KnownObjectsMap) -> Result<()> {
    let path = Path::new(KNOWN_OBJECTS_FILENAME);
    let file = OpenOptions::new().write(true).create(true).truncate(true).open(path)
        .context(format!("Failed to open or create {} for writing", KNOWN_OBJECTS_FILENAME))?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, objects)
        .context(format!("Failed to write JSON to {}", KNOWN_OBJECTS_FILENAME))?;
    Ok(())
}

fn scale_price(price_f64: f64) -> u64 {
    (price_f64 * 10f64.powi(DECIMALS as i32)).round() as u64
}

async fn get_or_create_price_object_id(
    sui_client: &SuiClient,
    signer_address: SuiAddress,
    keypair: &SuiKeyPair,
    symbol: &str,
) -> Result<ObjectID> {
    let mut known_objects = load_known_objects()?;
    if let Some(object_id) = known_objects.get(symbol) {
        println!("Found existing ObjectID {} for symbol {}", object_id, symbol);
        return Ok(*object_id);
    }

    println!("No ObjectID found for symbol {}. Creating new PriceObject...", symbol);

    let package_id = ObjectID::from_str(PACKAGE_ID_STR)?;
    let module_ident = Identifier::from_str(MODULE_NAME).context("Invalid module name")?;
    let function_ident = Identifier::from_str(CREATE_PRICE_OBJECT_FUNC_NAME).context("Invalid function name")?;

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let symbol_bytes = symbol.as_bytes().to_vec();
        builder.move_call(
            package_id,
            module_ident.clone(),
            function_ident.clone(),
            vec![], 
            vec![
                CallArg::Pure(bcs::to_bytes(&symbol_bytes).context("BCS failed for symbol_bytes")?),
                CallArg::Pure(bcs::to_bytes(&0u64).context("BCS failed for initial_price")?),
                CallArg::Pure(bcs::to_bytes(&0u64).context("BCS failed for initial_timestamp_ms")?),
                CallArg::Pure(bcs::to_bytes(&DECIMALS).context("BCS failed for DECIMALS")?),
            ],
        ).context("Move call construction failed")?;
        builder.finish()
    };

    let gas_price = sui_client.governance_api().get_reference_gas_price().await.unwrap_or(DEFAULT_GAS_PRICE);

    let gas_coins_response = sui_client
        .coin_read_api()
        .get_coins(signer_address, None, None, Some(1))
        .await
        .context("Failed to fetch gas coins for create_price_object")?;

    let gas_object_ref = gas_coins_response
        .data
        .get(0)
        .ok_or_else(|| anyhow!("No gas coins found for address {} to create object", signer_address))?
        .object_ref();
    
    let tx_data = TransactionData::new_programmable(
        signer_address,
        vec![gas_object_ref],
        pt,
        GAS_BUDGET,
        gas_price,
    );
    
    let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data.clone());
    let fastcrypto_signature = SuiSdkSignature::new_secure(&intent_msg, keypair);

    let transaction_envelope = Transaction::from_generic_sig_data(tx_data.clone(), vec![fastcrypto_signature.clone().into()]);

    let response = sui_client
        .quorum_driver_api()
        .execute_transaction_block(
            transaction_envelope, 
            SuiTransactionBlockResponseOptions::new().with_effects().with_events(), 
            None
        )
        .await
        .context("Failed to execute create_price_object transaction")?;

    if response.effects.as_ref().map_or(true, |e| e.status() != &SuiExecutionStatus::Success) {
        return Err(anyhow!("create_price_object transaction failed: {:?}", response.effects.as_ref().map(|e| e.status())));
    }

    let effects: &sui_sdk::rpc_types::SuiTransactionBlockEffects = response.effects.as_ref().ok_or_else(|| anyhow!("Transaction effects are missing"))?;
    let mut new_object_id: Option<ObjectID> = None;
    let price_object_type_tag_str_pattern = format!("{}::{}::PriceObject", PACKAGE_ID_STR, MODULE_NAME);

    for created_obj_ref in effects.created() {
        let owner_address = match created_obj_ref.owner {
            Owner::AddressOwner(addr) => Some(addr),
            _ => None, // Other owner types are not relevant here
        };

        if owner_address == Some(signer_address) {
            let object_id_to_check = created_obj_ref.reference.object_id;
            let obj_response = sui_client
                .read_api()
                .get_object_with_options(object_id_to_check, SuiObjectDataOptions::new().with_type())
                .await
                .context(format!("Failed to fetch created object {} to verify type", object_id_to_check))?;

            if let Some(obj_data) = obj_response.data {
                if let Some(obj_type) = obj_data.type_ {
                    if obj_type.to_string().contains(&price_object_type_tag_str_pattern) {
                        new_object_id = Some(object_id_to_check);
                        println!("Found created PriceObject with ID: {}", new_object_id.unwrap());
                        break;
                    }
                }
            }
        }
    }
    
    let new_object_id = new_object_id.ok_or_else(|| {
        let events_str = response.events.map_or_else(
            || "No events".to_string(),
            |evts| format!("{:?}", evts.data.iter().map(|e| e.type_.to_string()).collect::<Vec<_>>()),
        );
        anyhow!(
            "Could not find created PriceObject ID in transaction effects. Effects: {:?}, Events: {}",
            effects, events_str
        )
    })?;

    known_objects.insert(symbol.to_string(), new_object_id);
    save_known_objects(&known_objects)?;
    println!("New PriceObject ID {} for symbol {} saved.", new_object_id, symbol);

    Ok(new_object_id)
}

pub async fn submit_price_update(price_info: PriceInfo) -> Result<String> {
    println!("Attempting to submit price update for: {:?}", price_info);

    let keypair = get_publisher_keypair().context("Failed to get publisher keypair")?;
    
    let public_key = keypair.public();
    let signer_address = SuiAddress::from(&public_key);
    
    let expected_signer_address = SuiAddress::from_str(PUBLISHER_ADDRESS_STR)?;
    if signer_address != expected_signer_address {
        return Err(anyhow!(
            "Derived signer address {} does not match expected address {}",
            signer_address,
            expected_signer_address
        ));
    }
    println!("Signer address: {}", signer_address);

    let sui_client = SuiClientBuilder::default()
        .request_timeout(Duration::from_secs(30))
        .build(SUI_TESTNET_RPC_URL)
        .await
        .context(format!("Failed to build Sui client for URL: {}", SUI_TESTNET_RPC_URL))?;
    
    println!("Sui client connected to: {}", SUI_TESTNET_RPC_URL);

    let price_object_id = get_or_create_price_object_id(
        &sui_client,
        signer_address,
        &keypair,
        &price_info.symbol,
    )
    .await
    .context(format!("Failed to get or create PriceObject ID for symbol {}", price_info.symbol))?;
    
    println!("Using PriceObject ID {} for symbol {}", price_object_id, price_info.symbol);
    
    let object_to_update_response = sui_client
        .read_api()
        .get_object_with_options(price_object_id, SuiObjectDataOptions::new().with_owner().with_previous_transaction())
        .await
        .context(format!("Failed to fetch PriceObject {} for update", price_object_id))?;
    
    let object_data = object_to_update_response.data
        .ok_or_else(|| anyhow!("PriceObject {} data not found for update", price_object_id))?;
    let object_to_update_ref = object_data.object_ref();

    let scaled_price_val = scale_price(price_info.price);
    println!("Scaled price for {}: {} (original: {}, decimals: {})", price_info.symbol, scaled_price_val, price_info.price, DECIMALS);

    let package_id = ObjectID::from_str(PACKAGE_ID_STR)?;
    let module_ident = Identifier::from_str(MODULE_NAME).context("Invalid module name for update")?;
    let function_ident = Identifier::from_str(UPDATE_PRICE_FUNC_NAME).context("Invalid function name for update")?;

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.move_call(
            package_id,
            module_ident.clone(),
            function_ident.clone(),
            vec![], 
            vec![
                CallArg::Object(ObjectArg::ImmOrOwnedObject(object_to_update_ref)),
                CallArg::Pure(bcs::to_bytes(&scaled_price_val).context("BCS failed for scaled_price_val")?),
                CallArg::Pure(bcs::to_bytes(&price_info.timestamp_ms).context("BCS failed for timestamp_ms")?),
            ],
        ).context("Move call construction failed for update")?;
        builder.finish()
    };
    
    let gas_price = sui_client.governance_api().get_reference_gas_price().await.unwrap_or(DEFAULT_GAS_PRICE);

    let gas_coins_response = sui_client
        .coin_read_api()
        .get_coins(signer_address, None, None, Some(1))
        .await
        .context("Failed to fetch gas coins for update_price")?;

    let gas_object_ref = gas_coins_response
        .data
        .get(0)
        .ok_or_else(|| anyhow!("No gas coins found for address {} to update price", signer_address))?
        .object_ref();
   
    let tx_data = TransactionData::new_programmable(
        signer_address,
        vec![gas_object_ref],
        pt,
        GAS_BUDGET,
        gas_price,
    );

    let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data.clone());
    let fastcrypto_signature = SuiSdkSignature::new_secure(&intent_msg, &keypair);

    let transaction_envelope = Transaction::from_generic_sig_data(tx_data, vec![fastcrypto_signature.clone().into()]);

    println!("Submitting update_price transaction for symbol {}...", price_info.symbol);
    let response = sui_client
        .quorum_driver_api()
        .execute_transaction_block(
            transaction_envelope, 
            SuiTransactionBlockResponseOptions::new().with_effects(), 
            None
        )
        .await
        .context("Failed to execute update_price transaction")?;

    if response.effects.as_ref().map_or(true, |e| e.status() != &SuiExecutionStatus::Success) {
         return Err(anyhow!("update_price transaction failed: {:?}. Digest: {}", 
            response.effects.as_ref().map(|e| e.status()),
            response.digest
        ));
    }
    
    println!(
        "Successfully submitted price update for {}. Transaction Digest: {}",
        price_info.symbol, response.digest
    );

    Ok(response.digest.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publish_flow() {
        let btc_price_info_1 = PriceInfo {
            symbol: "BTC/USD_TEST_RUST_FIX_V2".to_string(),
            price: 68000.10,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        
        println!("Test 1: Submitting first price for {}", btc_price_info_1.symbol);
        match submit_price_update(btc_price_info_1.clone()).await {
            Ok(digest) => println!("Test 1 Succeeded. Digest: {}", digest),
            Err(e) => {
                let mut known = load_known_objects().unwrap_or_default();
                if known.remove(&btc_price_info_1.symbol).is_some() {
                    save_known_objects(&known).expect("Failed to cleanup test symbol from known_objects after Test 1 fail");
                    println!("Cleaned up test symbol {} from {} after Test 1 fail", btc_price_info_1.symbol, KNOWN_OBJECTS_FILENAME);
                }
                panic!("Test 1 Failed: {:?}", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(12)).await;

        let btc_price_info_2 = PriceInfo {
            symbol: btc_price_info_1.symbol.clone(), 
            price: 68002.25,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + 1000, 
        };
        println!("\nTest 2: Submitting second price for {}", btc_price_info_2.symbol);
        match submit_price_update(btc_price_info_2).await {
            Ok(digest) => println!("Test 2 Succeeded. Digest: {}", digest),
            Err(e) => panic!("Test 2 Failed: {:?}", e),
        }
        
        let mut known = load_known_objects().unwrap_or_default();
        if known.remove(&btc_price_info_1.symbol).is_some() {
            save_known_objects(&known).expect("Failed to cleanup test symbol from known_objects");
            println!("Cleaned up test symbol {} from {}", btc_price_info_1.symbol, KNOWN_OBJECTS_FILENAME);
        }
    }
} 