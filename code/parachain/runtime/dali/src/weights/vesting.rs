
//! Autogenerated weights for `vesting`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-12-16, STEPS: `50`, REPEAT: 10, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `fde3d2d43403`, CPU: `Intel(R) Xeon(R) CPU @ 2.20GHz`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dali-dev"), DB CACHE: 1024

// Executed Command:
// /nix/store/y1z2mfgy9msqas77hhxszf78hqg6mx5y-composable/bin/composable
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

/// Weight functions for `vesting`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> vesting::WeightInfo for WeightInfo<T> {
	// Storage: Vesting VestingSchedules (r:1 w:1)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: Tokens Locks (r:1 w:1)
	// Storage: Tokens Accounts (r:1 w:1)
	/// The range of component `s` is `[1, 100]`.
	fn claim(s: u32, ) -> Weight {
		Weight::from_ref_time(108_329_000_u64)
			// Standard Error: 22_000
			.saturating_add(Weight::from_ref_time(4_983_000_u64).saturating_mul(s as u64))
			.saturating_add(T::DbWeight::get().reads(4_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	// Storage: Vesting VestingScheduleNonce (r:1 w:1)
	// Storage: Vesting VestingSchedules (r:1 w:1)
	// Storage: Tokens Accounts (r:2 w:2)
	// Storage: System Account (r:2 w:1)
	// Storage: Tokens Locks (r:1 w:1)
	fn vested_transfer() -> Weight {
		Weight::from_ref_time(174_740_000_u64)
			.saturating_add(T::DbWeight::get().reads(7_u64))
			.saturating_add(T::DbWeight::get().writes(6_u64))
	}
	// Storage: Tokens Locks (r:1 w:1)
	// Storage: Tokens Accounts (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	// Storage: Vesting VestingSchedules (r:0 w:1)
	// Storage: Vesting VestingScheduleNonce (r:1 w:1)
	/// The range of component `s` is `[0, 100]`.
	fn update_vesting_schedules(s: u32, ) -> Weight {
		Weight::from_ref_time(105_639_000_u64)
			// Standard Error: 34_000
			.saturating_add(Weight::from_ref_time(3_364_000_u64).saturating_mul(s as u64))
			.saturating_add(T::DbWeight::get().reads(4_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
	}
	// Storage: Vesting VestingSchedules (r:1 w:1)
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: Tokens Locks (r:1 w:1)
	// Storage: Tokens Accounts (r:1 w:1)
	/// The range of component `s` is `[1, 100]`.
	fn claim_for(s: u32, ) -> Weight {
		Weight::from_ref_time(102_377_000_u64)
			// Standard Error: 24_000
			.saturating_add(Weight::from_ref_time(4_789_000_u64).saturating_mul(s as u64))
			.saturating_add(T::DbWeight::get().reads(4_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
}
