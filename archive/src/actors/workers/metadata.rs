// Copyright 2017-2019 Parity Technologies (UK) Ltd.
// This file is part of substrate-archive.

// substrate-archive is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// substrate-archive is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with substrate-archive.  If not, see <http://www.gnu.org/licenses/>.

use crate::{
    error::Error as ArchiveError,
    queries,
    rpc::Rpc,
    types::{BatchBlock, Block, Metadata as MetadataT, Substrate},
};
use itertools::Itertools;
use sp_runtime::traits::{Block as _, Header as _};
use xtra::prelude::*;

/// Actor to fetch metadata about a block/blocks from RPC
/// Accepts workers to decode blocks and a URL for the RPC
pub struct Metadata {
    url: String,
    pool: sqlx::PgPool,
}

impl Metadata {
    pub fn new(url: String, pool: sqlx::PgPool) -> Self {
        Self { url, pool }
    }
}

impl Actor for Metadata {}

#[async_trait::async_trait]
impl<T> Handler<Block<T>> for Metadata
where
    T: Substrate,
{
    async fn handle(&mut self, blk: Block<T>, _ctx: &mut Context<Self>) -> Result<(), ArchiveError> {
        let rpc = super::connect::<T>(self.url.as_str()).await;
        let hash = blk.inner.block.header().hash();
        meta_checker(blk.spec, Some(hash), &rpc, &self.pool).await?;
        // let v = sched.ask_next("transform", block)?.await;
        // log::debug!("{:?}", v);
        Ok(())
    }
}

#[async_trait::async_trait]
impl<T> Handler<BatchBlock<T>> for Metadata
where
    T: Substrate,
{
    async fn handle(&mut self, blks: BatchBlock<T>, _ctx: &mut Context<Self>) -> Result<(), ArchiveError> {
        let rpc = super::connect::<T>(self.url.as_str()).await;

        let versions = blks
            .inner()
            .iter()
            .unique_by(|b| b.spec)
            .collect::<Vec<&Block<T>>>();

        for b in versions.iter() {
            meta_checker(b.spec, Some(b.inner.block.hash()), &rpc, &self.pool).await?;
        }
        // let v = sched.ask_next("transform", batch_items)?.await;
        // log::debug!("{:?}", v);
        Ok(())
    }
}

// checks if the metadata exists in the database
// if it doesn't exist yet, fetch metadata and insert it
async fn meta_checker<T>(
    ver: u32,
    hash: Option<T::Hash>,
    rpc: &Rpc<T>,
    pool: &sqlx::PgPool,
) -> Result<(), ArchiveError>
where
    T: Substrate,
{
    if !queries::check_if_meta_exists(ver, pool).await? {
        let meta = rpc.metadata(hash).await?;
        let meta = MetadataT::new(ver, meta);
        // let v = sched.ask_next("transform", meta)?.await;
        // log::debug!("{:?}", v);
    }
    Ok(())
}
