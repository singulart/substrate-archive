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

//! Common Sql queries on Archive Database abstracted into rust functions

use super::BlockModel;
use crate::error::Result;
use hashbrown::HashSet;
use serde::{de::DeserializeOwned, Deserialize};
use sp_runtime::traits::Block as BlockT;
use std::convert::{TryInto, TryFrom};
use sqlx::{prelude::*, PgConnection};

/// Return type of queries that `SELECT version`
struct Version {
    version: i32,
}

/// Return type of querys that `SELECT missing_num ... FROM ... GENERATE_SERIES(a, z)`
pub struct Series {
    missing_num: Option<i32>,
}

/// Return type of querys that `SELECT MAX(int)`
struct Max {
    max: Option<i32>,
}

/// Return type querys that `SELECT EXISTS`
struct DoesExist {
    exists: Option<bool>,
}

// Return type of querys that `SELECT block_num`
struct BlockNum {
    block_num: i32,
}

// Return type of querys that `SELECT data`
struct Bytes {
    data: Vec<u8>,
}

/// Get missing blocks from the relational database between numbers `min` and
/// MAX(block_num). LIMIT result to length `max_block_load`. The highest effective
/// value for `min` is i32::MAX.
pub(crate) async fn missing_blocks_min_max(
    conn: &mut PgConnection,
    min: u32,
    max_block_load: u32,
) -> Result<HashSet<u32>> {
    let min = i32::try_from(min).unwrap_or(i32::MAX);
    let max_block_load = i64::try_from(max_block_load).unwrap_or(i64::MAX);
    Ok(sqlx::query_as!(
        Series,
        "SELECT missing_num
        FROM (SELECT MAX(block_num) AS max_num FROM blocks) max,
            GENERATE_SERIES($1, max_num) AS missing_num
        WHERE
        NOT EXISTS (SELECT id FROM blocks WHERE block_num = missing_num)
        ORDER BY missing_num ASC
        LIMIT $2
        ",
        min,
        max_block_load
    )
    .fetch_all(conn)
    .await?
    .iter()
    .map(|t| t.missing_num.unwrap() as u32)
    .collect())
}

#[derive(FromRow)]
pub struct Extrinsics {
    pub block_num: i32,
    pub hash: Vec<u8>,
    pub spec: i32,
    pub ext: Vec<u8>,
}

///  Returns missing extrinsics from the database.
///  A missing extrinsic is categorized as an extrinsic that belongs to a block we have indexed in
///  the `blocks` table, but which is not present in the `extrinsics` table.
pub(crate) async fn missing_extrinsics(conn: &mut PgConnection) -> Result<Vec<Extrinsics>> {
    sqlx::query_as!(Extrinsics,
        "SELECT block_num, hash, spec, ext
           FROM blocks
           WHERE NOT EXISTS (SELECT block_num FROM extrinsics WHERE extrinsics.block_num = blocks.block_num)
           AND blocks.block_num != 0
           ORDER BY block_num"
        )
        .fetch_all(conn)
        .await
        .map_err(Into::into)
}

/// Get the maximium block number from the relational database
pub(crate) async fn max_block(conn: &mut PgConnection) -> Result<Option<u32>> {
    let max = sqlx::query_as!(Max, "SELECT MAX(block_num) FROM blocks")
        .fetch_one(conn)
        .await?;
    Ok(max.max.map(|v| v as u32))
}

/// Will get blocks such that they exist in the `blocks` table but they
/// do not exist in the `storage` table
/// blocks are ordered by spec version
///
/// # Returns full blocks
pub(crate) async fn blocks_storage_intersection(
    conn: &mut sqlx::PgConnection,
) -> Result<Vec<BlockModel>> {
    sqlx::query_as!(
        BlockModel,
        "SELECT *
        FROM blocks
        WHERE NOT EXISTS (SELECT * FROM storage WHERE storage.block_num = blocks.block_num)
        AND blocks.block_num != 0
        ORDER BY blocks.spec",
    )
    .fetch_all(conn)
    .await
    .map_err(Into::into)
}

/// Get a block by id from the relational database
pub(crate) async fn get_full_block_by_id(
    conn: &mut sqlx::PgConnection,
    id: i32,
) -> Result<BlockModel> {
    sqlx::query_as!(
        BlockModel,
        "
        SELECT id, parent_hash, hash, block_num, state_root, extrinsics_root, digest, ext, spec
        FROM blocks
        WHERE id = $1
        ",
        id
    )
    .fetch_one(conn)
    .await
    .map_err(Into::into)
}

/// Get a block by block number from the relational database
#[cfg(test)]
pub(crate) async fn get_full_block_by_num(
    conn: &mut sqlx::PgConnection,
    block_num: u32,
) -> Result<BlockModel> {
    let safe_block_num = i32::try_from(block_num).unwrap_or(i32::MAX);
    sqlx::query_as!(
        BlockModel,
        "
        SELECT id, parent_hash, hash, block_num, state_root, extrinsics_root, digest, ext, spec
        FROM blocks
        WHERE block_num = $1
        ",
        safe_block_num
    )
    .fetch_one(conn)
    .await
    .map_err(Into::into)
}

/// Check if the runtime version identified by `spec` exists in the relational database
pub(crate) async fn check_if_meta_exists(spec: u32, conn: &mut PgConnection) -> Result<bool> {
    let spec = match i32::try_from(spec) {
        Err(_) => return Ok(false),
        Ok(n) => n,
    };
    let does_exist = sqlx::query_as!(
        DoesExist,
        r#"SELECT EXISTS(SELECT version FROM metadata WHERE version = $1)"#,
        spec
    )
    .fetch_one(conn)
    .await?;
    Ok(does_exist.exists.unwrap_or(false))
}

/// Check if the block identified by `hash` exists in the relational database
pub(crate) async fn has_block<B: BlockT>(hash: B::Hash, conn: &mut PgConnection) -> Result<bool> {
    let hash = hash.as_ref();
    let does_exist = sqlx::query_as!(
        DoesExist,
        r#"SELECT EXISTS(SELECT 1 FROM blocks WHERE hash = $1)"#,
        hash,
    )
    .fetch_one(conn)
    .await?;
    Ok(does_exist.exists.unwrap_or(false))
}

/// Get a list of block_numbers, out of the passed-in blocknumbers, which exist in the relational
/// database
pub(crate) async fn has_blocks<B: BlockT>(
    nums: &[u32],
    conn: &mut PgConnection,
) -> Result<Vec<u32>> {
    let nums: Vec<i32> = nums.iter().filter_map(|n| i32::try_from(*n).ok()).collect();
    Ok(sqlx::query_as!(
        BlockNum,
        "SELECT block_num FROM blocks WHERE block_num = ANY ($1)",
        &nums,
    )
    .fetch_all(conn)
    .await?
    .into_iter()
    .map(|r| r.block_num as u32)
    .collect())
}

/// Get all the metadata versions stored in the relational database
pub(crate) async fn get_versions(conn: &mut PgConnection) -> Result<Vec<u32>> {
    Ok(sqlx::query_as!(Version, "SELECT version FROM metadata")
        .fetch_all(conn)
        .await?
        .into_iter()
        .map(|r| r.version as u32)
        .collect())
}

/// Get all the blocks queued for execution in the background task queue.
pub(crate) async fn get_all_blocks<B: BlockT + DeserializeOwned>(
    conn: &mut PgConnection,
) -> Result<impl Iterator<Item = Result<B>>> {
    let blocks = sqlx::query_as!(
        Bytes,
        "SELECT data FROM _background_tasks WHERE job_type = 'execute_block'",
    )
    .fetch_all(conn)
    .await?;

    // temporary struct to deserialize job
    #[derive(Deserialize)]
    struct JobIn<BL: BlockT> {
        block: BL,
    }
    Ok(blocks.into_iter().map(|r| {
        let b: JobIn<B> = rmp_serde::from_read(r.data.as_slice())?;
        Ok(b.block)
    }))
}

pub(crate) async fn get_version(conn: &mut PgConnection, block_num: u32) -> Result<u32> {
    let version = sqlx::query_as::<_, (i32, )>("SELECT spec FROM blocks WHERE block_num = $1")
        .bind(block_num as i64)
        .fetch_one(conn)
        .await?;
    Ok(version.0.try_into()?)
}

pub(crate) async fn get_metadata(conn: &mut PgConnection, spec: &u32) -> Result<Vec<u8>> {
    let meta = sqlx::query_as::<_, (Vec<u8>,)>("SELECT meta FROM metadata WHERE version = $1")
        .bind(spec)
        .fetch_one(conn)
        .await?;
    Ok(meta.0)
}

#[cfg(test)]
mod tests {
    //! Must be connected to a postgres database
    use super::*;
    // use diesel::test_transaction;
}
