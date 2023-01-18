
//! Autogenerated weights for `mosaic`
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

/// Weight functions for `mosaic`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> mosaic::WeightInfo for WeightInfo<T> {
	// Storage: Mosaic Relayer (r:0 w:1)
	fn set_relayer() -> Weight {
		Weight::from_ref_time(28_813_000 as u64)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Mosaic Relayer (r:1 w:1)
	fn rotate_relayer() -> Weight {
		Weight::from_ref_time(35_013_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Mosaic Relayer (r:1 w:0)
	// Storage: Mosaic NetworkInfos (r:0 w:1)
	fn set_network() -> Weight {
		Weight::from_ref_time(35_997_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Mosaic AssetsInfo (r:1 w:1)
	fn set_budget() -> Weight {
		Weight::from_ref_time(33_729_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Mosaic AssetsInfo (r:1 w:0)
	// Storage: Mosaic LocalToRemoteAsset (r:1 w:0)
	// Storage: Mosaic NetworkInfos (r:1 w:0)
	// Storage: System Account (r:2 w:2)
	// Storage: Mosaic TimeLockPeriod (r:1 w:0)
	// Storage: Mosaic OutgoingTransactions (r:1 w:1)
	// Storage: Mosaic Nonce (r:1 w:1)
	fn transfer_to() -> Weight {
		Weight::from_ref_time(110_139_000 as u64)
			.saturating_add(T::DbWeight::get().reads(8 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
	// Storage: Mosaic Relayer (r:1 w:0)
	// Storage: Mosaic RemoteToLocalAsset (r:1 w:0)
	// Storage: Mosaic OutgoingTransactions (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	fn accept_transfer() -> Weight {
		Weight::from_ref_time(86_441_000 as u64)
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Mosaic OutgoingTransactions (r:1 w:1)
	// Storage: System Account (r:2 w:2)
	fn claim_stale_to() -> Weight {
		Weight::from_ref_time(90_566_000 as u64)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Mosaic Relayer (r:1 w:0)
	// Storage: Mosaic RemoteToLocalAsset (r:1 w:0)
	// Storage: Mosaic AssetsInfo (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	// Storage: Mosaic IncomingTransactions (r:1 w:1)
	fn timelocked_mint() -> Weight {
		Weight::from_ref_time(92_125_000 as u64)
			.saturating_add(T::DbWeight::get().reads(5 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Mosaic TimeLockPeriod (r:0 w:1)
	fn set_timelock_duration() -> Weight {
		Weight::from_ref_time(11_164_000 as u64)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Mosaic Relayer (r:1 w:0)
	// Storage: Mosaic RemoteToLocalAsset (r:1 w:0)
	// Storage: Mosaic IncomingTransactions (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	fn rescind_timelocked_mint() -> Weight {
		Weight::from_ref_time(83_152_000 as u64)
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Mosaic IncomingTransactions (r:1 w:1)
	// Storage: System Account (r:2 w:2)
	fn claim_to() -> Weight {
		Weight::from_ref_time(90_477_000 as u64)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Mosaic NetworkInfos (r:1 w:0)
	// Storage: Mosaic LocalToRemoteAsset (r:1 w:1)
	// Storage: Mosaic RemoteToLocalAsset (r:0 w:1)
	fn update_asset_mapping() -> Weight {
		Weight::from_ref_time(43_274_000 as u64)
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Mosaic RemoteAmmWhitelist (r:1 w:1)
	fn add_remote_amm_id() -> Weight {
		Weight::from_ref_time(18_179_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Mosaic RemoteAmmWhitelist (r:1 w:1)
	fn remove_remote_amm_id() -> Weight {
		Weight::from_ref_time(19_703_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}
