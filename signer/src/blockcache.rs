//! Minimal in-memory compact-block cache for `zcash_client_backend::sync::run`.
//!
//! `sync::run` requires a `BlockCache` (and its `BlockSource` supertrait). The
//! crate ships no production implementation, so this is the in-memory cache
//! from the `BlockCache` trait docs, with a real `with_blocks` so cached blocks
//! are actually fed to the scanner. `sync::run` fetches blocks into the cache
//! in batches, scans, then deletes them, so memory stays bounded.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use zcash_client_backend::data_api::chain::{error, BlockCache, BlockSource};
use zcash_client_backend::data_api::scanning::ScanRange;
use zcash_client_backend::proto::compact_formats::CompactBlock;
use zcash_protocol::consensus::BlockHeight;

#[derive(Clone, Default)]
pub struct MemBlockCache {
    blocks: Arc<Mutex<Vec<CompactBlock>>>,
}

impl MemBlockCache {
    pub fn new() -> Self {
        Self::default()
    }
}

impl BlockSource for MemBlockCache {
    type Error = Infallible;

    fn with_blocks<F, WalletErrT>(
        &self,
        from_height: Option<BlockHeight>,
        limit: Option<usize>,
        mut with_block: F,
    ) -> Result<(), error::Error<WalletErrT, Self::Error>>
    where
        F: FnMut(CompactBlock) -> Result<(), error::Error<WalletErrT, Self::Error>>,
    {
        let blocks = self.blocks.lock().unwrap();
        let mut sorted: Vec<CompactBlock> = blocks.clone();
        sorted.sort_by_key(|b| b.height);
        let mut count = 0usize;
        for block in sorted {
            let height = BlockHeight::from_u32(block.height as u32);
            if from_height.is_some_and(|from| height < from) {
                continue;
            }
            if limit.is_some_and(|lim| count >= lim) {
                break;
            }
            with_block(block)?;
            count += 1;
        }
        Ok(())
    }
}

#[async_trait]
impl BlockCache for MemBlockCache {
    fn get_tip_height(
        &self,
        range: Option<&ScanRange>,
    ) -> Result<Option<BlockHeight>, Self::Error> {
        let blocks = self.blocks.lock().unwrap();
        let highest = blocks
            .iter()
            .filter(|block| {
                let h = BlockHeight::from_u32(block.height as u32);
                range.is_none_or(|r| r.block_range().contains(&h))
            })
            .map(|block| block.height)
            .max();
        Ok(highest.map(|h| BlockHeight::from_u32(h as u32)))
    }

    async fn read(&self, range: &ScanRange) -> Result<Vec<CompactBlock>, Self::Error> {
        Ok(self
            .blocks
            .lock()
            .unwrap()
            .iter()
            .filter(|block| {
                let h = BlockHeight::from_u32(block.height as u32);
                range.block_range().contains(&h)
            })
            .cloned()
            .collect())
    }

    async fn insert(&self, mut compact_blocks: Vec<CompactBlock>) -> Result<(), Self::Error> {
        self.blocks.lock().unwrap().append(&mut compact_blocks);
        Ok(())
    }

    async fn delete(&self, range: ScanRange) -> Result<(), Self::Error> {
        self.blocks.lock().unwrap().retain(|block| {
            !range
                .block_range()
                .contains(&BlockHeight::from_u32(block.height as u32))
        });
        Ok(())
    }
}
