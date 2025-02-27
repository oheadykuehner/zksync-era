use std::{convert::TryInto, str::FromStr};

use bigdecimal::{BigDecimal, ToPrimitive};
use sqlx::{
    postgres::{PgArguments, Postgres},
    query::Query,
    types::chrono::{DateTime, NaiveDateTime, Utc},
};
use thiserror::Error;

use zksync_contracts::BaseSystemContractsHashes;
use zksync_types::{
    api,
    block::{L1BatchHeader, MiniblockHeader},
    commitment::{L1BatchMetaParameters, L1BatchMetadata},
    l2_to_l1_log::L2ToL1Log,
    Address, L1BatchNumber, MiniblockNumber, H2048, H256,
};

#[derive(Debug, Error)]
pub enum StorageL1BatchConvertError {
    #[error("Incomplete L1 batch")]
    Incomplete,
}

/// Projection of the `l1_batches` table corresponding to [`L1BatchHeader`].
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageL1BatchHeader {
    pub number: i64,
    pub timestamp: i64,
    pub is_finished: bool,
    pub l1_tx_count: i32,
    pub l2_tx_count: i32,
    pub fee_account_address: Vec<u8>,
    pub l2_to_l1_logs: Vec<Vec<u8>>,
    pub l2_to_l1_messages: Vec<Vec<u8>>,
    pub bloom: Vec<u8>,
    pub priority_ops_onchain_data: Vec<Vec<u8>>,
    pub used_contract_hashes: serde_json::Value,
    pub base_fee_per_gas: BigDecimal,
    pub l1_gas_price: i64,
    pub l2_fair_gas_price: i64,
    pub bootloader_code_hash: Option<Vec<u8>>,
    pub default_aa_code_hash: Option<Vec<u8>>,
    pub protocol_version: Option<i32>,
}

impl From<StorageL1BatchHeader> for L1BatchHeader {
    fn from(l1_batch: StorageL1BatchHeader) -> Self {
        let priority_ops_onchain_data: Vec<_> = l1_batch
            .priority_ops_onchain_data
            .into_iter()
            .map(|raw_data| raw_data.into())
            .collect();

        L1BatchHeader {
            number: L1BatchNumber(l1_batch.number as u32),
            is_finished: l1_batch.is_finished,
            timestamp: l1_batch.timestamp as u64,
            fee_account_address: Address::from_slice(&l1_batch.fee_account_address),
            priority_ops_onchain_data,
            l1_tx_count: l1_batch.l1_tx_count as u16,
            l2_tx_count: l1_batch.l2_tx_count as u16,
            l2_to_l1_logs: convert_l2_to_l1_logs(l1_batch.l2_to_l1_logs),
            l2_to_l1_messages: l1_batch.l2_to_l1_messages,

            bloom: H2048::from_slice(&l1_batch.bloom),
            used_contract_hashes: serde_json::from_value(l1_batch.used_contract_hashes)
                .expect("invalid value for used_contract_hashes in the DB"),
            base_fee_per_gas: l1_batch
                .base_fee_per_gas
                .to_u64()
                .expect("base_fee_per_gas should fit in u64"),
            base_system_contracts_hashes: convert_base_system_contracts_hashes(
                l1_batch.bootloader_code_hash,
                l1_batch.default_aa_code_hash,
            ),
            l1_gas_price: l1_batch.l1_gas_price as u64,
            l2_fair_gas_price: l1_batch.l2_fair_gas_price as u64,
            protocol_version: l1_batch
                .protocol_version
                .map(|v| (v as u16).try_into().unwrap()),
        }
    }
}

fn convert_l2_to_l1_logs(raw_logs: Vec<Vec<u8>>) -> Vec<L2ToL1Log> {
    raw_logs
        .into_iter()
        .map(|raw_log| L2ToL1Log::from_slice(&raw_log))
        .collect()
}

// TODO (SMA-1635): Make these fields non optional in database
fn convert_base_system_contracts_hashes(
    bootloader_code_hash: Option<Vec<u8>>,
    default_aa_code_hash: Option<Vec<u8>>,
) -> BaseSystemContractsHashes {
    BaseSystemContractsHashes {
        bootloader: bootloader_code_hash
            .map(|hash| H256::from_slice(&hash))
            .expect("should not be none"),
        default_aa: default_aa_code_hash
            .map(|hash| H256::from_slice(&hash))
            .expect("should not be none"),
    }
}

/// Projection of the `l1_batches` table corresponding to [`L1BatchHeader`] + [`L1BatchMetadata`].
// TODO(PLA-369): use `#[sqlx(flatten)]` once upgraded to newer `sqlx`
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageL1Batch {
    pub number: i64,
    pub timestamp: i64,
    pub is_finished: bool,
    pub l1_tx_count: i32,
    pub l2_tx_count: i32,
    pub fee_account_address: Vec<u8>,
    pub bloom: Vec<u8>,
    pub l2_to_l1_logs: Vec<Vec<u8>>,
    pub priority_ops_onchain_data: Vec<Vec<u8>>,

    pub parent_hash: Option<Vec<u8>>,
    pub hash: Option<Vec<u8>>,
    pub merkle_root_hash: Option<Vec<u8>>,
    pub commitment: Option<Vec<u8>>,
    pub meta_parameters_hash: Option<Vec<u8>>,
    pub pass_through_data_hash: Option<Vec<u8>>,
    pub aux_data_hash: Option<Vec<u8>>,

    pub rollup_last_leaf_index: Option<i64>,
    pub zkporter_is_available: Option<bool>,
    pub bootloader_code_hash: Option<Vec<u8>>,
    pub default_aa_code_hash: Option<Vec<u8>>,

    pub l2_to_l1_messages: Vec<Vec<u8>>,
    pub l2_l1_compressed_messages: Option<Vec<u8>>,
    pub l2_l1_merkle_root: Option<Vec<u8>>,
    pub compressed_initial_writes: Option<Vec<u8>>,
    pub compressed_repeated_writes: Option<Vec<u8>>,
    pub compressed_write_logs: Option<Vec<u8>>,
    pub compressed_contracts: Option<Vec<u8>>,

    pub eth_prove_tx_id: Option<i32>,
    pub eth_commit_tx_id: Option<i32>,
    pub eth_execute_tx_id: Option<i32>,

    pub used_contract_hashes: serde_json::Value,

    pub base_fee_per_gas: BigDecimal,
    pub l1_gas_price: i64,
    pub l2_fair_gas_price: i64,

    pub protocol_version: Option<i32>,
}

impl From<StorageL1Batch> for L1BatchHeader {
    fn from(l1_batch: StorageL1Batch) -> Self {
        let priority_ops_onchain_data: Vec<_> = l1_batch
            .priority_ops_onchain_data
            .into_iter()
            .map(Vec::into)
            .collect();

        L1BatchHeader {
            number: L1BatchNumber(l1_batch.number as u32),
            is_finished: l1_batch.is_finished,
            timestamp: l1_batch.timestamp as u64,
            fee_account_address: Address::from_slice(&l1_batch.fee_account_address),
            priority_ops_onchain_data,
            l1_tx_count: l1_batch.l1_tx_count as u16,
            l2_tx_count: l1_batch.l2_tx_count as u16,
            l2_to_l1_logs: convert_l2_to_l1_logs(l1_batch.l2_to_l1_logs),
            l2_to_l1_messages: l1_batch.l2_to_l1_messages,

            bloom: H2048::from_slice(&l1_batch.bloom),
            used_contract_hashes: serde_json::from_value(l1_batch.used_contract_hashes)
                .expect("invalid value for used_contract_hashes in the DB"),
            base_fee_per_gas: l1_batch
                .base_fee_per_gas
                .to_u64()
                .expect("base_fee_per_gas should fit in u64"),
            base_system_contracts_hashes: convert_base_system_contracts_hashes(
                l1_batch.bootloader_code_hash,
                l1_batch.default_aa_code_hash,
            ),
            l1_gas_price: l1_batch.l1_gas_price as u64,
            l2_fair_gas_price: l1_batch.l2_fair_gas_price as u64,
            protocol_version: l1_batch
                .protocol_version
                .map(|v| (v as u16).try_into().unwrap()),
        }
    }
}

impl TryInto<L1BatchMetadata> for StorageL1Batch {
    type Error = StorageL1BatchConvertError;

    fn try_into(self) -> Result<L1BatchMetadata, Self::Error> {
        Ok(L1BatchMetadata {
            root_hash: H256::from_slice(&self.hash.ok_or(StorageL1BatchConvertError::Incomplete)?),
            rollup_last_leaf_index: self
                .rollup_last_leaf_index
                .ok_or(StorageL1BatchConvertError::Incomplete)?
                as u64,
            merkle_root_hash: H256::from_slice(
                &self
                    .merkle_root_hash
                    .ok_or(StorageL1BatchConvertError::Incomplete)?,
            ),
            initial_writes_compressed: self
                .compressed_initial_writes
                .ok_or(StorageL1BatchConvertError::Incomplete)?,
            repeated_writes_compressed: self
                .compressed_repeated_writes
                .ok_or(StorageL1BatchConvertError::Incomplete)?,
            l2_l1_messages_compressed: self
                .l2_l1_compressed_messages
                .ok_or(StorageL1BatchConvertError::Incomplete)?,
            l2_l1_merkle_root: H256::from_slice(
                &self
                    .l2_l1_merkle_root
                    .ok_or(StorageL1BatchConvertError::Incomplete)?,
            ),
            aux_data_hash: H256::from_slice(
                &self
                    .aux_data_hash
                    .ok_or(StorageL1BatchConvertError::Incomplete)?,
            ),
            meta_parameters_hash: H256::from_slice(
                &self
                    .meta_parameters_hash
                    .ok_or(StorageL1BatchConvertError::Incomplete)?,
            ),
            pass_through_data_hash: H256::from_slice(
                &self
                    .pass_through_data_hash
                    .ok_or(StorageL1BatchConvertError::Incomplete)?,
            ),
            commitment: H256::from_slice(
                &self
                    .commitment
                    .ok_or(StorageL1BatchConvertError::Incomplete)?,
            ),
            block_meta_params: L1BatchMetaParameters {
                zkporter_is_available: self
                    .zkporter_is_available
                    .ok_or(StorageL1BatchConvertError::Incomplete)?,
                bootloader_code_hash: H256::from_slice(
                    &self
                        .bootloader_code_hash
                        .ok_or(StorageL1BatchConvertError::Incomplete)?,
                ),
                default_aa_code_hash: H256::from_slice(
                    &self
                        .default_aa_code_hash
                        .ok_or(StorageL1BatchConvertError::Incomplete)?,
                ),
            },
        })
    }
}

/// Returns block_number SQL statement
pub fn web3_block_number_to_sql(block_number: api::BlockNumber) -> String {
    match block_number {
        api::BlockNumber::Number(number) => number.to_string(),
        api::BlockNumber::Earliest => 0.to_string(),
        api::BlockNumber::Pending => {
            "(SELECT (MAX(number) + 1) as number FROM miniblocks)".to_string()
        }
        api::BlockNumber::Latest | api::BlockNumber::Committed => {
            "(SELECT MAX(number) as number FROM miniblocks)".to_string()
        }
        api::BlockNumber::Finalized => "
                (SELECT COALESCE(
                    (
                        SELECT MAX(number) FROM miniblocks
                        WHERE l1_batch_number = (
                            SELECT MAX(number) FROM l1_batches
                            JOIN eth_txs ON
                                l1_batches.eth_execute_tx_id = eth_txs.id
                            WHERE
                                eth_txs.confirmed_eth_tx_history_id IS NOT NULL
                        )
                    ),
                    0
                ) as number)
            "
        .to_string(),
    }
}

pub fn web3_block_where_sql(block_id: api::BlockId, arg_index: u8) -> String {
    match block_id {
        api::BlockId::Hash(_) => format!("miniblocks.hash = ${arg_index}"),
        api::BlockId::Number(api::BlockNumber::Number(_)) => {
            format!("miniblocks.number = ${arg_index}")
        }
        api::BlockId::Number(number) => {
            let block_sql = web3_block_number_to_sql(number);
            format!("miniblocks.number = {}", block_sql)
        }
    }
}

pub fn bind_block_where_sql_params<'q>(
    block_id: &'q api::BlockId,
    query: Query<'q, Postgres, PgArguments>,
) -> Query<'q, Postgres, PgArguments> {
    match block_id {
        // these block_id types result in `$1` in the query string, which we have to `bind`
        api::BlockId::Hash(block_hash) => query.bind(block_hash.as_bytes()),
        api::BlockId::Number(api::BlockNumber::Number(number)) => {
            query.bind(number.as_u64() as i64)
        }
        // others don't introduce `$1`, so we don't have to `bind` anything
        _ => query,
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageBlockDetails {
    pub number: i64,
    pub l1_batch_number: i64,
    pub timestamp: i64,
    pub l1_tx_count: i32,
    pub l2_tx_count: i32,
    pub root_hash: Option<Vec<u8>>,
    pub commit_tx_hash: Option<String>,
    pub committed_at: Option<NaiveDateTime>,
    pub prove_tx_hash: Option<String>,
    pub proven_at: Option<NaiveDateTime>,
    pub execute_tx_hash: Option<String>,
    pub executed_at: Option<NaiveDateTime>,
    // L1 gas price assumed in the corresponding batch
    pub l1_gas_price: i64,
    // L2 gas price assumed in the corresponding batch
    pub l2_fair_gas_price: i64,
    pub bootloader_code_hash: Option<Vec<u8>>,
    pub default_aa_code_hash: Option<Vec<u8>>,
    pub fee_account_address: Option<Vec<u8>>, // May be None if the block is not yet sealed
    pub protocol_version: Option<i32>,
}

impl StorageBlockDetails {
    pub(crate) fn into_block_details(self, current_operator_address: Address) -> api::BlockDetails {
        let status = if self.number == 0 || self.execute_tx_hash.is_some() {
            api::BlockStatus::Verified
        } else {
            api::BlockStatus::Sealed
        };

        let base = api::BlockDetailsBase {
            timestamp: self.timestamp as u64,
            l1_tx_count: self.l1_tx_count as usize,
            l2_tx_count: self.l2_tx_count as usize,
            status,
            root_hash: self.root_hash.as_deref().map(H256::from_slice),
            commit_tx_hash: self
                .commit_tx_hash
                .as_deref()
                .map(|hash| H256::from_str(hash).expect("Incorrect commit_tx hash")),
            committed_at: self
                .committed_at
                .map(|committed_at| DateTime::from_naive_utc_and_offset(committed_at, Utc)),
            prove_tx_hash: self
                .prove_tx_hash
                .as_deref()
                .map(|hash| H256::from_str(hash).expect("Incorrect prove_tx hash")),
            proven_at: self
                .proven_at
                .map(|proven_at| DateTime::<Utc>::from_naive_utc_and_offset(proven_at, Utc)),
            execute_tx_hash: self
                .execute_tx_hash
                .as_deref()
                .map(|hash| H256::from_str(hash).expect("Incorrect execute_tx hash")),
            executed_at: self
                .executed_at
                .map(|executed_at| DateTime::<Utc>::from_naive_utc_and_offset(executed_at, Utc)),
            l1_gas_price: self.l1_gas_price as u64,
            l2_fair_gas_price: self.l2_fair_gas_price as u64,
            base_system_contracts_hashes: convert_base_system_contracts_hashes(
                self.bootloader_code_hash,
                self.default_aa_code_hash,
            ),
        };
        api::BlockDetails {
            base,
            number: MiniblockNumber(self.number as u32),
            l1_batch_number: L1BatchNumber(self.l1_batch_number as u32),
            operator_address: self
                .fee_account_address
                .map(|fee_account_address| Address::from_slice(&fee_account_address))
                .unwrap_or(current_operator_address),
            protocol_version: self
                .protocol_version
                .map(|v| (v as u16).try_into().unwrap()),
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageL1BatchDetails {
    pub number: i64,
    pub timestamp: i64,
    pub l1_tx_count: i32,
    pub l2_tx_count: i32,
    pub root_hash: Option<Vec<u8>>,
    pub commit_tx_hash: Option<String>,
    pub committed_at: Option<NaiveDateTime>,
    pub prove_tx_hash: Option<String>,
    pub proven_at: Option<NaiveDateTime>,
    pub execute_tx_hash: Option<String>,
    pub executed_at: Option<NaiveDateTime>,
    pub l1_gas_price: i64,
    pub l2_fair_gas_price: i64,
    pub bootloader_code_hash: Option<Vec<u8>>,
    pub default_aa_code_hash: Option<Vec<u8>>,
}

impl From<StorageL1BatchDetails> for api::L1BatchDetails {
    fn from(details: StorageL1BatchDetails) -> Self {
        let status = if details.number == 0 || details.execute_tx_hash.is_some() {
            api::BlockStatus::Verified
        } else {
            api::BlockStatus::Sealed
        };

        let base = api::BlockDetailsBase {
            timestamp: details.timestamp as u64,
            l1_tx_count: details.l1_tx_count as usize,
            l2_tx_count: details.l2_tx_count as usize,
            status,
            root_hash: details.root_hash.as_deref().map(H256::from_slice),
            commit_tx_hash: details
                .commit_tx_hash
                .as_deref()
                .map(|hash| H256::from_str(hash).expect("Incorrect commit_tx hash")),
            committed_at: details
                .committed_at
                .map(|committed_at| DateTime::<Utc>::from_naive_utc_and_offset(committed_at, Utc)),
            prove_tx_hash: details
                .prove_tx_hash
                .as_deref()
                .map(|hash| H256::from_str(hash).expect("Incorrect prove_tx hash")),
            proven_at: details
                .proven_at
                .map(|proven_at| DateTime::<Utc>::from_naive_utc_and_offset(proven_at, Utc)),
            execute_tx_hash: details
                .execute_tx_hash
                .as_deref()
                .map(|hash| H256::from_str(hash).expect("Incorrect execute_tx hash")),
            executed_at: details
                .executed_at
                .map(|executed_at| DateTime::<Utc>::from_naive_utc_and_offset(executed_at, Utc)),
            l1_gas_price: details.l1_gas_price as u64,
            l2_fair_gas_price: details.l2_fair_gas_price as u64,
            base_system_contracts_hashes: convert_base_system_contracts_hashes(
                details.bootloader_code_hash,
                details.default_aa_code_hash,
            ),
        };
        api::L1BatchDetails {
            base,
            number: L1BatchNumber(details.number as u32),
        }
    }
}

pub struct StorageMiniblockHeader {
    pub number: i64,
    pub timestamp: i64,
    pub hash: Vec<u8>,
    pub l1_tx_count: i32,
    pub l2_tx_count: i32,
    pub base_fee_per_gas: BigDecimal,
    pub l1_gas_price: i64,
    // L1 gas price assumed in the corresponding batch
    pub l2_fair_gas_price: i64,
    // L2 gas price assumed in the corresponding batch
    pub bootloader_code_hash: Option<Vec<u8>>,
    pub default_aa_code_hash: Option<Vec<u8>>,
    pub protocol_version: Option<i32>,

    // The maximal number of virtual blocks that can be created with this miniblock.
    // If this value is greater than zero, then at least 1 will be created, but no more than
    // min(virtual_blocks, miniblock_number - virtual_block_number), i.e. making sure that virtual blocks
    // never go beyond the miniblock they are based on.
    pub virtual_blocks: i64,
}

impl From<StorageMiniblockHeader> for MiniblockHeader {
    fn from(row: StorageMiniblockHeader) -> Self {
        MiniblockHeader {
            number: MiniblockNumber(row.number as u32),
            timestamp: row.timestamp as u64,
            hash: H256::from_slice(&row.hash),
            l1_tx_count: row.l1_tx_count as u16,
            l2_tx_count: row.l2_tx_count as u16,
            base_fee_per_gas: row.base_fee_per_gas.to_u64().unwrap(),
            l1_gas_price: row.l1_gas_price as u64,
            l2_fair_gas_price: row.l2_fair_gas_price as u64,
            base_system_contracts_hashes: convert_base_system_contracts_hashes(
                row.bootloader_code_hash,
                row.default_aa_code_hash,
            ),
            protocol_version: row.protocol_version.map(|v| (v as u16).try_into().unwrap()),
            virtual_blocks: row.virtual_blocks as u32,
        }
    }
}

/// Information about L1 batch which a certain miniblock belongs to.
#[derive(Debug)]
pub struct ResolvedL1BatchForMiniblock {
    /// L1 batch which the miniblock belongs to. `None` if the miniblock is not explicitly attached
    /// (i.e., its L1 batch is not sealed).
    pub miniblock_l1_batch: Option<L1BatchNumber>,
    /// Pending (i.e., unsealed) L1 batch.
    pub pending_l1_batch: L1BatchNumber,
}

impl ResolvedL1BatchForMiniblock {
    /// Returns the L1 batch number that the miniblock has now or will have in the future (provided
    /// that the node will operate correctly).
    pub fn expected_l1_batch(&self) -> L1BatchNumber {
        self.miniblock_l1_batch.unwrap_or(self.pending_l1_batch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web3_block_number_to_sql_earliest() {
        let sql = web3_block_number_to_sql(api::BlockNumber::Earliest);
        assert_eq!(sql, 0.to_string());
    }

    #[test]
    fn test_web3_block_number_to_sql_pending() {
        let sql = web3_block_number_to_sql(api::BlockNumber::Pending);
        assert_eq!(
            sql,
            "(SELECT (MAX(number) + 1) as number FROM miniblocks)".to_string()
        );
    }

    #[test]
    fn test_web3_block_number_to_sql_latest() {
        let sql = web3_block_number_to_sql(api::BlockNumber::Latest);
        assert_eq!(
            sql,
            "(SELECT MAX(number) as number FROM miniblocks)".to_string()
        );
    }

    #[test]
    fn test_web3_block_number_to_sql_committed() {
        let sql = web3_block_number_to_sql(api::BlockNumber::Committed);
        assert_eq!(
            sql,
            "(SELECT MAX(number) as number FROM miniblocks)".to_string()
        );
    }

    #[test]
    fn test_web3_block_number_to_sql_finalized() {
        let sql = web3_block_number_to_sql(api::BlockNumber::Finalized);
        assert_eq!(
            sql,
            "
                (SELECT COALESCE(
                    (
                        SELECT MAX(number) FROM miniblocks
                        WHERE l1_batch_number = (
                            SELECT MAX(number) FROM l1_batches
                            JOIN eth_txs ON
                                l1_batches.eth_execute_tx_id = eth_txs.id
                            WHERE
                                eth_txs.confirmed_eth_tx_history_id IS NOT NULL
                        )
                    ),
                    0
                ) as number)
            "
            .to_string()
        );
    }
}
