
//! Autogenerated weights for `oracle`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-01-18, STEPS: `50`, REPEAT: 10, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `7066db519b2a`, CPU: `Intel(R) Xeon(R) CPU @ 3.10GHz`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dali-dev"), DB CACHE: 1024

// Executed Command:
// /nix/store/a8zw7i97gjsaanq9c839pbaklnsjsqcf-composable/bin/composable
// benchmark
// pallet
// --chain=dali-dev
// --execution=wasm
// --wasm-execution=compiled
// --wasm-instantiation-strategy=legacy-instance-reuse
// --pallet=*
// --extrinsic=*
// --steps=50
// --repeat=10
// --output=code/parachain/runtime/dali/src/weights

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for `oracle`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> oracle::WeightInfo for WeightInfo<T> {
	// Storage: Oracle AssetsCount (r:1 w:1)
	// Storage: Oracle RewardTrackerStore (r:1 w:1)
	// Storage: Oracle AssetsInfo (r:1 w:1)
	fn add_asset_and_info() -> Weight {
		Weight::from_ref_time(40_647_000 as u64)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: Oracle RewardTrackerStore (r:1 w:1)
	fn adjust_rewards() -> Weight {
		Weight::from_ref_time(34_212_000 as u64)
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Oracle ControllerToSigner (r:1 w:1)
	// Storage: Oracle SignerToController (r:1 w:1)
	// Storage: Oracle OracleStake (r:1 w:1)
	fn set_signer() -> Weight {
		Weight::from_ref_time(103_654_000 as u64)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Oracle ControllerToSigner (r:1 w:0)
	// Storage: Oracle OracleStake (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	fn add_stake() -> Weight {
		Weight::from_ref_time(93_851_000 as u64)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Oracle ControllerToSigner (r:1 w:0)
	// Storage: Oracle OracleStake (r:1 w:1)
	// Storage: Oracle DeclaredWithdraws (r:0 w:1)
	fn remove_stake() -> Weight {
		Weight::from_ref_time(43_125_000 as u64)
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Oracle ControllerToSigner (r:1 w:1)
	// Storage: Oracle DeclaredWithdraws (r:1 w:1)
	// Storage: System Account (r:1 w:0)
	// Storage: Oracle SignerToController (r:0 w:1)
	fn reclaim_stake() -> Weight {
		Weight::from_ref_time(49_713_000 as u64)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Oracle OracleStake (r:1 w:0)
	// Storage: Oracle Prices (r:1 w:0)
	// Storage: Oracle AssetsInfo (r:1 w:0)
	// Storage: Oracle AnswerInTransit (r:1 w:1)
	// Storage: Oracle PrePrices (r:1 w:1)
	/// The range of component `p` is `[1, 25]`.
	fn submit_price(p: u32, ) -> Weight {
		Weight::from_ref_time(54_749_000 as u64)
			// Standard Error: 5_412
			.saturating_add(Weight::from_ref_time(323_123 as u64).saturating_mul(p as u64))
			.saturating_add(T::DbWeight::get().reads(5 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Oracle PrePrices (r:1 w:1)
	// Storage: Oracle AnswerInTransit (r:1 w:1)
	/// The range of component `p` is `[1, 25]`.
	fn update_pre_prices(p: u32, ) -> Weight {
		Weight::from_ref_time(15_397_000 as u64)
			// Standard Error: 1_590
			.saturating_add(Weight::from_ref_time(212_086 as u64).saturating_mul(p as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Oracle PriceHistory (r:1 w:1)
	// Storage: Oracle SignerToController (r:1 w:0)
	// Storage: Oracle AnswerInTransit (r:1 w:1)
	// Storage: Oracle RewardTrackerStore (r:1 w:0)
	// Storage: Oracle Prices (r:0 w:1)
	// Storage: Oracle PrePrices (r:0 w:1)
	/// The range of component `p` is `[1, 25]`.
	fn update_price(p: u32, ) -> Weight {
		Weight::from_ref_time(35_004_000 as u64)
			// Standard Error: 43_699
			.saturating_add(Weight::from_ref_time(3_794_178 as u64).saturating_mul(p as u64))
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
}
