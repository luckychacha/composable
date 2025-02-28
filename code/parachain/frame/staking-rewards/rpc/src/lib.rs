use codec::Codec;
use composable_support::rpc_helpers::SafeRpcWrapper;
use core::{fmt::Display, str::FromStr};
use jsonrpsee::{
	core::{Error as RpcError, RpcResult},
	proc_macros::rpc,
	types::{error::CallError, ErrorObject},
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use sp_std::{cmp::Ord, collections::btree_map::BTreeMap, sync::Arc};
use staking_rewards_runtime_api::{ClaimableAmountError, StakingRewardsRuntimeApi};

#[rpc(client, server)]
pub trait StakingRewardsApi<BlockHash, AssetId, FinancialNftInstanceId, Balance>
where
	AssetId: FromStr + Display + Ord,
	FinancialNftInstanceId: FromStr + Display,
	Balance: FromStr + Display,
{
	#[method(name = "stakingRewards_claimableAmount")]
	fn claimable_amount(
		&self,
		fnft_collection_id: SafeRpcWrapper<AssetId>,
		fnft_instance_id: SafeRpcWrapper<FinancialNftInstanceId>,
		at: Option<BlockHash>,
	) -> RpcResult<Result<BTreeMap<AssetId, Balance>, ClaimableAmountError>>;
}

pub struct StakingRewards<C, Block> {
	client: Arc<C>,
	_marker: sp_std::marker::PhantomData<Block>,
}

impl<C, M> StakingRewards<C, M> {
	pub fn new(client: Arc<C>) -> Self {
		Self { client, _marker: Default::default() }
	}
}

impl<C, Block, AssetId, FinancialNftInstanceId, Balance>
	StakingRewardsApiServer<<Block as BlockT>::Hash, AssetId, FinancialNftInstanceId, Balance>
	for StakingRewards<C, (Block, AssetId, FinancialNftInstanceId, Balance)>
where
	Block: BlockT,
	AssetId: Send + Sync + 'static + Codec + FromStr + Display + Ord,
	FinancialNftInstanceId: Send + Sync + 'static + Codec + FromStr + Display,
	Balance: Send + Sync + 'static + Codec + FromStr + Display,
	C: Send + Sync + 'static,
	C: ProvideRuntimeApi<Block>,
	C: HeaderBackend<Block>,
	C::Api: StakingRewardsRuntimeApi<Block, AssetId, FinancialNftInstanceId, Balance>,
{
	fn claimable_amount(
		&self,
		fnft_collection_id: SafeRpcWrapper<AssetId>,
		fnft_instance_id: SafeRpcWrapper<FinancialNftInstanceId>,
		at: Option<<Block as BlockT>::Hash>,
	) -> RpcResult<Result<BTreeMap<AssetId, Balance>, ClaimableAmountError>> {
		let api = self.client.runtime_api();

		let at = at.unwrap_or_else(|| self.client.info().best_hash);

		// calling ../../runtime-api
		let runtime_api_result = api.claimable_amount(at, fnft_collection_id, fnft_instance_id);
		runtime_api_result.map_err(|e| {
			RpcError::Call(CallError::Custom(ErrorObject::owned(
				9876,
				"Something wrong",
				Some(format!("{:?}", e)),
			)))
		})
	}
}
