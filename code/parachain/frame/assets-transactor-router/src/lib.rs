//! # Assets Transactor Router Pallet
//!
//! The Transactor Router provides implementations of common currency traits
//! (e.g. from [`orml`](https://docs.rs/orml-traits) and `frame_support`)
//! and functionality for handling transfers and minting.
//!
//! - [`Config`]
//! - [`Call`]
//! - [`Pallet`]
//!
//! ## Overview
//!
//! The Assets pallet provides functions for:
//!
//! - Transferring balances of native and other assets between accounts.
//! - Minting and burn new assets by per asset governance.
//! - Crediting and debiting of created asset balances.
//! - By design similar to [orml_currencies](https://docs.rs/orml-currencies/latest/orml_currencies/)
//!   and [substrate_assets](https://github.com/paritytech/substrate/tree/master/frame/assets).
//!
//! ### Implementations
//!
//! The Assets pallet provides implementations for the following traits:
//!
//! - [`Currency`](frame_support::traits::Currency):
//! - [`LockableCurrency`](frame_support::traits::tokens::currency::LockableCurrency)
//! - [`ReservableCurrency`](frame_support::traits::ReservableCurrency):
//! - [`MultiCurrency`](orml_traits::MultiCurrency):
//! - [`MultiLockableCurrency`](orml_traits::MultiLockableCurrency):
//! - [`MultiReservableCurrency`](orml_traits::MultiReservableCurrency):
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `transfer`
//! - `transfer_native`
//! - `force_transfer`
//! - `force_transfer_native`
//! - `transfer_all`
//! - `transfer_all_native`
//! - `mint_into`
//! - `burn_from`

// we start lag behind useful traits:
// TODO: implement fungibles::Balanced like orml Tokens do
// TODO: implement tokens::NamedReservableCurrency like orml Tokens do

#![cfg_attr(
	not(test),
	warn(
		clippy::disallowed_methods,
		clippy::disallowed_types,
		clippy::indexing_slicing,
		clippy::todo,
		clippy::unwrap_used,
		clippy::panic
	)
)] // allow in tests
#![deny(clippy::unseparated_literal_suffix, unused_imports)]
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

mod currency;
mod fungible;
mod fungibles;
mod orml;

#[cfg(test)]
mod mocks;

#[cfg(test)]
mod tests;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarking;
pub mod weights;

macro_rules! route {
	(
		fn $fn:ident($asset:ident: $asset_ty:ty $(, $arg:ident: $ty:ty)* $(,)?) $(-> $ret:ty)?;
	) => {
		fn $fn($asset: $asset_ty, $($arg:$ty),*) $(-> $ret)? {
			if T::AssetId::from($asset.into()) == <T::NativeAssetId as frame_support::traits::Get<_>>::get() {
				<<T as Config>::NativeTransactor>::$fn($($arg),*)
			} else {
				crate::route_asset_type! { $fn($asset, $($arg),*) }
			}
		}
	};
}

// bad design, except it does db read on each balance/tokens operation (which is main operation
// in crypto) it also triple encodes foreign local relation (enum + prefix + raw data)
// reads foreign location when it no needed
// and prevents use permission less/unknown tokens (transfer first, document later)
// and has issues with well defined conventions for relay native nad native tokens
// this check must be deleted (it was merged without review)
// also non of parachain does that afaik
macro_rules! route_asset_type {
	(
		$fn:ident($asset:ident $(, $arg:ident)* $(,)?)
	) => {
		match <T::AssetsRegistry as composable_traits::assets::AssetTypeInspect>::inspect(&$asset) {
			composable_traits::assets::AssetType::Foreign => {
				<<T as Config>::ForeignTransactor>::$fn($asset, $($arg),*)
			}
			composable_traits::assets::AssetType::Local => {
				<<T as Config>::LocalTransactor>::$fn($asset, $($arg),*)
			}
		}
	};
}

pub(crate) use route;
pub(crate) use route_asset_type;

#[frame_support::pallet]
pub mod pallet {
	use crate::weights::WeightInfo;
	use codec::FullCodec;
	use composable_traits::{
		assets::{
			AssetInfo, AssetTypeInspect, CreateAsset, GenerateAssetId, InspectRegistryMetadata,
			MutateRegistryMetadata,
		},
		currency::{AssetIdLike, BalanceLike},
		xcm::assets::RemoteAssetRegistryMutate,
	};
	use frame_support::{
		dispatch::DispatchResult,
		pallet_prelude::*,
		sp_runtime::traits::StaticLookup,
		traits::{
			fungible, fungibles, Currency, EnsureOrigin, LockableCurrency, ReservableCurrency,
		},
	};
	use frame_system::{ensure_root, ensure_signed, pallet_prelude::OriginFor};
	use orml_traits::{MultiCurrency, MultiLockableCurrency, MultiReservableCurrency};
	use sp_runtime::{DispatchError, FixedPointOperand};
	use sp_std::{fmt::Debug, str};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type AssetId: AssetIdLike + From<u128> + Into<u128> + MaybeSerializeDeserialize;
		type AssetLocation: FullCodec
			+ Eq
			+ PartialEq
			+ MaybeSerializeDeserialize
			+ Debug
			+ Clone
			+ TypeInfo
			+ MaxEncodedLen;
		type Balance: BalanceLike + FixedPointOperand;

		#[pallet::constant]
		type NativeAssetId: Get<Self::AssetId>;

		type ForeignTransactor: fungibles::Inspect<Self::AccountId, Balance = Self::Balance, AssetId = Self::AssetId>
			+ fungibles::Transfer<Self::AccountId>
			+ fungibles::Mutate<Self::AccountId>
			+ fungibles::Unbalanced<Self::AccountId>
			+ fungibles::InspectHold<Self::AccountId>
			+ fungibles::MutateHold<Self::AccountId>
			+ MultiCurrency<Self::AccountId, Balance = Self::Balance, CurrencyId = Self::AssetId>
			+ MultiLockableCurrency<
				Self::AccountId,
				Balance = Self::Balance,
				CurrencyId = Self::AssetId,
			> + MultiReservableCurrency<
				Self::AccountId,
				Balance = Self::Balance,
				CurrencyId = Self::AssetId,
			>;

		type LocalTransactor: fungibles::Inspect<Self::AccountId, Balance = Self::Balance, AssetId = Self::AssetId>
			+ fungibles::Transfer<Self::AccountId>
			+ fungibles::Mutate<Self::AccountId>
			+ fungibles::Unbalanced<Self::AccountId>
			+ fungibles::InspectHold<Self::AccountId>
			+ fungibles::MutateHold<Self::AccountId>
			+ MultiCurrency<Self::AccountId, Balance = Self::Balance, CurrencyId = Self::AssetId>
			+ MultiLockableCurrency<
				Self::AccountId,
				Balance = Self::Balance,
				CurrencyId = Self::AssetId,
			> + MultiReservableCurrency<
				Self::AccountId,
				Balance = Self::Balance,
				CurrencyId = Self::AssetId,
			>;

		type NativeTransactor: fungible::Inspect<Self::AccountId, Balance = Self::Balance>
			+ fungible::Transfer<Self::AccountId>
			+ fungible::Mutate<Self::AccountId>
			+ fungible::Unbalanced<Self::AccountId>
			+ fungible::InspectHold<Self::AccountId>
			+ fungible::Transfer<Self::AccountId>
			+ fungible::MutateHold<Self::AccountId>
			+ Currency<Self::AccountId, Balance = Self::Balance>
			+ LockableCurrency<Self::AccountId, Balance = Self::Balance>
			+ ReservableCurrency<Self::AccountId, Balance = Self::Balance>;

		/// origin of admin of this pallet
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Assets registry
		/// Maintains general info about any given asset
		type AssetsRegistry: AssetTypeInspect<AssetId = Self::AssetId>
			+ RemoteAssetRegistryMutate<
				AssetId = Self::AssetId,
				AssetNativeLocation = Self::AssetLocation,
				Balance = Self::Balance,
			> + InspectRegistryMetadata<AssetId = Self::AssetId>
			+ MutateRegistryMetadata<AssetId = Self::AssetId>
			+ GenerateAssetId<AssetId = Self::AssetId>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::error]
	pub enum Error<T> {
		CannotSetNewCurrencyToRegistry,
		InvalidCurrency,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Transfer `amount` of `asset` from `origin` to `dest`.
		///
		/// # Errors
		///  - When `origin` is not signed.
		///  - If the account has insufficient free balance to make the transfer, or if `keep_alive`
		///    cannot be respected.
		///  - If the `dest` cannot be looked up.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::transfer())]
		pub fn transfer(
			origin: OriginFor<T>,
			asset: T::AssetId,
			dest: <T::Lookup as StaticLookup>::Source,
			amount: T::Balance,
			keep_alive: bool,
		) -> DispatchResult {
			let source = ensure_signed(origin)?;
			let dest = T::Lookup::lookup(dest)?;

			<Pallet<T> as fungibles::Transfer<_>>::transfer(
				asset, &source, &dest, amount, keep_alive,
			)?;
			Ok(())
		}

		/// Transfer `amount` of the native asset from `origin` to `dest`. This is slightly
		/// cheaper to call, as it avoids an asset lookup.
		///
		/// # Errors
		///  - When `origin` is not signed.
		///  - If the account has insufficient free balance to make the transfer, or if `keep_alive`
		///    cannot be respected.
		///  - If the `dest` cannot be looked up.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::transfer_native())]
		pub fn transfer_native(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			value: T::Balance,
			keep_alive: bool,
		) -> DispatchResult {
			let source = ensure_signed(origin)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as fungible::Transfer<_>>::transfer(&source, &dest, value, keep_alive)?;
			Ok(())
		}

		/// Transfer `amount` of the `asset` from `origin` to `dest`. This requires root.
		///
		/// # Errors
		///  - When `origin` is not root.
		///  - If the account has insufficient free balance to make the transfer, or if `keep_alive`
		///    cannot be respected.
		///  - If the `dest` cannot be looked up.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::force_transfer())]
		pub fn force_transfer(
			origin: OriginFor<T>,
			asset: T::AssetId,
			source: <T::Lookup as StaticLookup>::Source,
			dest: <T::Lookup as StaticLookup>::Source,
			value: T::Balance,
			keep_alive: bool,
		) -> DispatchResult {
			ensure_root(origin)?;
			let source = T::Lookup::lookup(source)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as fungibles::Transfer<_>>::transfer(asset, &source, &dest, value, keep_alive)?;
			Ok(())
		}

		/// Transfer `amount` of the the native asset from `origin` to `dest`. This requires root.
		///
		/// # Errors
		///  - When `origin` is not root.
		///  - If the account has insufficient free balance to make the transfer, or if `keep_alive`
		///    cannot be respected.
		///  - If the `dest` cannot be looked up.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::force_transfer_native())]
		pub fn force_transfer_native(
			origin: OriginFor<T>,
			source: <T::Lookup as StaticLookup>::Source,
			dest: <T::Lookup as StaticLookup>::Source,
			value: T::Balance,
			keep_alive: bool,
		) -> DispatchResult {
			ensure_root(origin)?;
			let source = T::Lookup::lookup(source)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as fungible::Transfer<_>>::transfer(&source, &dest, value, keep_alive)?;
			Ok(())
		}

		/// Transfer all free balance of the `asset` from `origin` to `dest`.
		///
		/// # Errors
		///  - When `origin` is not signed.
		///  - If the `dest` cannot be looked up.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::transfer_all())]
		pub fn transfer_all(
			origin: OriginFor<T>,
			asset: T::AssetId,
			dest: <T::Lookup as StaticLookup>::Source,
			keep_alive: bool,
		) -> DispatchResult {
			let transactor = ensure_signed(origin)?;
			let reducible_balance = <Self as fungibles::Inspect<T::AccountId>>::reducible_balance(
				asset,
				&transactor,
				keep_alive,
			);
			let dest = T::Lookup::lookup(dest)?;
			<Self as fungibles::Transfer<T::AccountId>>::transfer(
				asset,
				&transactor,
				&dest,
				reducible_balance,
				keep_alive,
			)?;
			Ok(())
		}

		/// Transfer all free balance of the native asset from `origin` to `dest`.
		///
		/// # Errors
		///  - When `origin` is not signed.
		///  - If the `dest` cannot be looked up.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::transfer_all_native())]
		pub fn transfer_all_native(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			keep_alive: bool,
		) -> DispatchResult {
			let transactor = ensure_signed(origin)?;
			let reducible_balance =
				<Self as fungible::Inspect<_>>::reducible_balance(&transactor, keep_alive);
			let dest = T::Lookup::lookup(dest)?;
			<Self as fungible::Transfer<_>>::transfer(
				&transactor,
				&dest,
				reducible_balance,
				keep_alive,
			)?;
			Ok(())
		}

		/// Mints `amount` of `asset_id` into the `dest` account.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::mint_into())]
		pub fn mint_into(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			dest: <T::Lookup as StaticLookup>::Source,
			amount: T::Balance,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as fungibles::Mutate<T::AccountId>>::mint_into(asset_id, &dest, amount)?;
			Ok(())
		}

		/// Burns `amount` of `asset_id` into the `dest` account.
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::burn_from())]
		pub fn burn_from(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			dest: <T::Lookup as StaticLookup>::Source,
			amount: T::Balance,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as fungibles::Mutate<T::AccountId>>::burn_from(asset_id, &dest, amount)?;
			Ok(())
		}
	}

	impl<T: Config> CreateAsset for Pallet<T> {
		type LocalAssetId = T::AssetId;
		type ForeignAssetId = T::AssetLocation;
		type Balance = T::Balance;

		fn create_local_asset(
			protocol_id: [u8; 4],
			nonce: u64,
			asset_info: AssetInfo<T::Balance>,
		) -> Result<Self::LocalAssetId, DispatchError> {
			let asset_id = T::AssetsRegistry::generate_asset_id(protocol_id, nonce);

			T::AssetsRegistry::register_asset(asset_id, None, asset_info)?;

			Ok(asset_id)
		}

		fn create_foreign_asset(
			protocol_id: [u8; 4],
			nonce: u64,
			asset_info: AssetInfo<T::Balance>,
			foreign_asset_id: Self::ForeignAssetId,
		) -> Result<Self::LocalAssetId, DispatchError> {
			let asset_id = T::AssetsRegistry::generate_asset_id(protocol_id, nonce);

			T::AssetsRegistry::register_asset(asset_id, Some(foreign_asset_id), asset_info)?;

			Ok(asset_id)
		}
	}
}
