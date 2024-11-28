use std::fmt::{Debug, Formatter};
use reth_consensus::{Consensus, ConsensusError};
use crate::apos::{AposError,APos,EXTRA_VANITY,NONCE_AUTH_VOTE,NONCE_DROP_VOTE,DIFF_IN_TURN,DIFF_NO_TURN};
use crate::snapshot_test::EXTRA_SEAL;
use alloy_genesis::ChainConfig;
use reth_primitives::{
    proofs, Block, BlockBody, BlockWithSenders, Header, SealedBlock, SealedHeader,
    TransactionSigned, Withdrawals,
};
// use alloy_eips::{eip1898::BlockHashOrNumber, eip7685::Requests};
use alloy_primitives::{U256, BlockHash, hex, U32, Bloom, BlockNumber, keccak256, B64, B256, Address, Bytes};
use reth_evm::provider::EvmEnvProvider;
use reth_storage_api::{BlockReader, HeaderProvider, SnapshotProvider, StateProviderFactory};


impl<Provider: HeaderProvider + StateProviderFactory + BlockReader + EvmEnvProvider + SnapshotProvider + Clone + Unpin + 'static> Consensus for APos<Provider>
{

    fn validate_header(&self,header: &SealedHeader) -> Result<(), ConsensusError>  {
        if header.number == 0 {
            return Err(ConsensusError::ParentUnknown {});
        }
        let number = header.number;
    
        // Don't waste time checking blocks from the future
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        if header.timestamp> current_time {
            return Err(AposError::InvalidTimestamp);
        }
    
        // Checkpoint blocks need to enforce zero beneficiary
        let checkpoint = (number % self.epoch) == 0;
        if checkpoint  {
            return Err(AposError::InvalidCheckpointBeneficiary);
        }
          // Nonces must be 0x00..0 or 0xff..f, zeroes enforced on checkpoints
        if header.nonce != NONCE_AUTH_VOTE && header.nonce != NONCE_DROP_VOTE {
            return Err(AposError::InvalidVote);
        }
        if checkpoint && header.nonce != NONCE_DROP_VOTE {
            return Err(AposError::InvalidCheckpointVote);
        }
    
         // Check that the extra-data contains both the vanity and signature
        if header.extra_data.len() < EXTRA_VANITY {
            return Err(AposError::MissingVanity);
        }
        if header.extra_data.len() < EXTRA_VANITY + EXTRA_SEAL {
            return Err(AposError::MissingSignature);
        }
         // Ensure that the extra-data contains a signer list on checkpoint, but none otherwise
        let signers_bytes = header.extra_data.len() - EXTRA_VANITY - EXTRA_SEAL;
        if !checkpoint && signers_bytes != 0 {
            return Err(AposError::ExtraSigners);
        }
        if checkpoint && signers_bytes % ADDRESS_LENGTH != 0 {
            return Err(AposError::InvalidCheckpointSigners);
        }
         // Ensure that the block's difficulty is meaningful (may not be correct at this point)
        if number > 0 {
            if header.difficulty.is_zero() || 
            (header.difficulty != DIFF_IN_TURN && header.difficulty != DIFF_NO_TURN) {
                return Err(AposError::InvalidDifficulty);
            }
        }
        //  // Verify that the gas limit is <= 2^63-1
        // if header.gas_limit > MAX_GAS_LIMIT {
        //     return Err(format!("Invalid gasLimit: have {}, max {}", header.gas_limit, MAX_GAS_LIMIT).into());
        // }
    
        Ok(())
            
        }
    
    
    fn validate_header_against_parent(&self,header: &SealedHeader,parent: &SealedHeader,) -> Result<(),ConsensusError>  {
                // The genesis block is the always valid dead-end
                let number = header.number;
                if number == 0 {
                    return Ok(());
                }
                  // Verify that the gasUsed is <= gasLimit
            if header.gas_used > header.gas_limit {
                return Err(ConsensusError::HeaderGasUsedExceedsGasLimit { gas_used: header.gas_used, gas_limit: header.gas_limit })
            }
    
            if header.is_timestamp_in_past(parent.timestamp) {
                return Err(ConsensusError::TimestampIsInPast {
                    parent_timestamp: parent.timestamp,
                    timestamp: header.timestamp,
                })
            }
      
            Ok(())
        }
    
    
    fn validate_header_with_total_difficulty(&self,header: &Header,total_difficulty:U256,) -> Result<(),ConsensusError>  {
            APos::calc_difficulty(&mut self, header.parent_hash);
            Ok(())
        }
    
    
    fn validate_block_pre_execution(&self,block: &SealedBlock) -> Result<(),ConsensusError>  {
        APos::snapshot(&mut self,block.number,block.ommers_hash,block.parent_hash);
        APos::prepare(&mut self, &mut block.header);
        
        Ok(())
        }
    
    
    fn validate_block_post_execution(&self,block: &BlockWithSenders,input:PostExecutionInput<'_> ,) -> Result<(),ConsensusError>  {
            APos::seal(&mut self,block);
            Ok(())
        }
        
       
        // fn validate_header_range(&self,headers: &[SealedHeader]) -> Result<(),HeaderConsensusError>{
        // if let Some((initial_header,remaining_headers)) = headers.split_first(){
        //     self.validate_header(initial_header).map_err(|e|HeaderConsensusError(e,initial_header.clone()))? ;
        //     let mut parent = initial_header;
        //     for child in remaining_headers {
        //         self.validate_header(child).map_err(|e|HeaderConsensusError(e,child.clone()))? ;
        //         self.validate_header_against_parent(child,parent).map_err(|e|HeaderConsensusError(e,child.clone()))? ;
        //         parent = child;
        //     }
        // }Ok(())
        // }
}
