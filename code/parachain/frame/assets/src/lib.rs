//! # Assets Pallet
//!
//! The Assets pallet provides implementation of common currency traits
//! (e.g. from [`orml`](https://docs.rs/orml-traits) or `frame_support`)
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
//!   and [substrate_assets](https://github.com/paritytech/substrate/tree/master/frame/assets)
//!
//! ### Implementations
//!
//! The Assets pallet provides implementations for the following traits:
//!
//! - [`Currency`](frame_support::traits::Currency):
//!  Functions for dealing with a fungible assets system.
//! - [`ReservableCurrency`](frame_support::traits::ReservableCurrency):
//!  Functions for dealing with assets that can be reserved from an account.
//! - [`MultiCurrency`](orml_traits::MultiCurrency):
//!  Abstraction over a fungible multi-currency system.
//! - [`MultiLockableCurrency`](orml_traits::MultiLockableCurrency):
//!  A fungible multi-currency system whose accounts can have liquidity restrictions.
//! - [`MultiReservableCurrency`](orml_traits::MultiReservableCurrency):
//!  A fungible multi-currency system where funds can be reserved from the user.
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
//! - `mint_initialize`
//! - `mint_initialize_with_governance`
//! - `mint_into`
//! - `burn_from`
//
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

mod orml;

#[cfg(test)]
mod mocks;

#[cfg(test)]
mod tests;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarking;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
	use crate::weights::WeightInfo;
	use composable_support::validation::Validate;
	use composable_traits::currency::{AssetIdLike, BalanceLike, CurrencyFactory, RangeId};
	use frame_support::{
		dispatch::DispatchResultWithPostInfo,
		pallet_prelude::*,
		sp_runtime::traits::StaticLookup,
		traits::{
			fungible::{
				Inspect as NativeInspect, Mutate as NativeMutate, Transfer as NativeTransfer,
			},
			fungibles::{Inspect, Mutate, Transfer},
			EnsureOrigin,
		},
	};
	use frame_system::{ensure_root, ensure_signed, pallet_prelude::OriginFor};
	use num_traits::Zero;
	use primitives::currency::ValidateCurrencyId;
	use sp_runtime::{DispatchError, FixedPointOperand};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// currency id
		type AssetId: AssetIdLike;
		type Balance: BalanceLike + FixedPointOperand;
		#[pallet::constant]
		type NativeAssetId: Get<Self::AssetId>;
		type GenerateCurrencyId: CurrencyFactory<AssetId = Self::AssetId, Balance = Self::Balance>;
		type NativeCurrency;
		type MultiCurrency;
		type WeightInfo: WeightInfo;
		/// origin of admin of this pallet
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type CurrencyValidator: Validate<Self::AssetId, ValidateCurrencyId>;
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
	impl<T: Config> Pallet<T>
	where
		<T as Config>::NativeCurrency: NativeTransfer<T::AccountId, Balance = T::Balance>
			+ NativeInspect<T::AccountId, Balance = T::Balance>
			+ NativeMutate<T::AccountId, Balance = T::Balance>,
		<T as Config>::MultiCurrency: Inspect<T::AccountId, Balance = T::Balance, AssetId = T::AssetId>
			+ Transfer<T::AccountId, Balance = T::Balance, AssetId = T::AssetId>
			+ Mutate<T::AccountId, Balance = T::Balance, AssetId = T::AssetId>,
	{
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
			#[pallet::compact] amount: T::Balance,
			keep_alive: bool,
		) -> DispatchResultWithPostInfo {
			let src = ensure_signed(origin)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as Transfer<T::AccountId>>::transfer(asset, &src, &dest, amount, keep_alive)?;
			Ok(().into())
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
			#[pallet::compact] value: T::Balance,
			keep_alive: bool,
		) -> DispatchResultWithPostInfo {
			let src = ensure_signed(origin)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as NativeTransfer<T::AccountId>>::transfer(&src, &dest, value, keep_alive)?;
			Ok(().into())
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
			#[pallet::compact] value: T::Balance,
			keep_alive: bool,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let source = T::Lookup::lookup(source)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as Transfer<_>>::transfer(asset, &source, &dest, value, keep_alive)?;
			Ok(().into())
		}

		/// Transfer `amount` of the the native asset from `origin` to `dest`. This requires root.
		///
		/// # Errors
		///  - When `origin` is not root.
		///  - If the account has insufficient free balance to make the transfer, or if `keep_alive`
		///    cannot be respected.
		///  - If the `dest` cannot be looked up.
		#[pallet::weight(T::WeightInfo::force_transfer_native())]
		#[pallet::call_index(3)]
		pub fn force_transfer_native(
			origin: OriginFor<T>,
			source: <T::Lookup as StaticLookup>::Source,
			dest: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] value: T::Balance,
			keep_alive: bool,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let source = T::Lookup::lookup(source)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as NativeTransfer<_>>::transfer(&source, &dest, value, keep_alive)?;
			Ok(().into())
		}

		/// Transfer all free balance of the `asset` from `origin` to `dest`.
		///
		/// # Errors
		///  - When `origin` is not signed.
		///  - If the `dest` cannot be looked up.
		#[pallet::weight(T::WeightInfo::transfer_all())]
		#[pallet::call_index(4)]
		pub fn transfer_all(
			origin: OriginFor<T>,
			asset: T::AssetId,
			dest: <T::Lookup as StaticLookup>::Source,
			keep_alive: bool,
		) -> DispatchResult {
			let transactor = ensure_signed(origin)?;
			let reducible_balance =
				<Self as Inspect<T::AccountId>>::reducible_balance(asset, &transactor, keep_alive);
			let dest = T::Lookup::lookup(dest)?;
			<Self as Transfer<T::AccountId>>::transfer(
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
		#[pallet::weight(T::WeightInfo::transfer_all_native())]
		#[pallet::call_index(5)]
		pub fn transfer_all_native(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			keep_alive: bool,
		) -> DispatchResult {
			let transactor = ensure_signed(origin)?;
			let reducible_balance =
				<Self as NativeInspect<T::AccountId>>::reducible_balance(&transactor, keep_alive);
			let dest = T::Lookup::lookup(dest)?;
			<Self as NativeTransfer<T::AccountId>>::transfer(
				&transactor,
				&dest,
				reducible_balance,
				keep_alive,
			)?;
			Ok(())
		}

		/// Creates a new asset, minting `amount` of funds into the `dest` account. Intended to be
		/// used for creating wrapped assets, not associated with any project.
		#[pallet::weight(T::WeightInfo::mint_initialize())]
		#[pallet::call_index(6)]
		pub fn mint_initialize(
			origin: OriginFor<T>,
			#[pallet::compact] amount: T::Balance,
			dest: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let id = T::GenerateCurrencyId::create(RangeId::TOKENS)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as Mutate<T::AccountId>>::mint_into(id, &dest, amount)?;
			Ok(().into())
		}

		/// Creates a new asset, minting `amount` of funds into the `dest` account. The `dest`
		/// account can use the democracy pallet to mint further assets, or if the governance_origin
		/// is set to an owned account, using signed transactions. In general the
		/// `governance_origin` should be generated from the pallet id.
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::mint_initialize())]
		pub fn mint_initialize_with_governance(
			origin: OriginFor<T>,
			#[pallet::compact] amount: T::Balance,
			_governance_origin: <T::Lookup as StaticLookup>::Source,
			dest: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResultWithPostInfo {
			T::AdminOrigin::ensure_origin(origin)?;
			let id = T::GenerateCurrencyId::create(RangeId::TOKENS)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as Mutate<T::AccountId>>::mint_into(id, &dest, amount)?;
			Ok(().into())
		}

		/// Mints `amount` of `asset_id` into the `dest` account.
		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::mint_into())]
		pub fn mint_into(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			dest: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			T::AdminOrigin::ensure_origin(origin)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as Mutate<T::AccountId>>::mint_into(asset_id, &dest, amount)?;
			Ok(().into())
		}

		/// Burns `amount` of `asset_id` into the `dest` account.
		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::burn_from())]
		pub fn burn_from(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			dest: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			T::AdminOrigin::ensure_origin(origin)?;
			let dest = T::Lookup::lookup(dest)?;
			<Self as Mutate<T::AccountId>>::burn_from(asset_id, &dest, amount)?;
			Ok(().into())
		}
	}

	pub(crate) fn valid_asset_id<T: Config>(asset_id: T::AssetId) -> Option<T::AssetId> {
		T::CurrencyValidator::validate(asset_id).ok()
	}

	mod currency {
		use super::*;
		use frame_support::traits::{
			BalanceStatus, Currency, ExistenceRequirement, ReservableCurrency, SignedImbalance,
			WithdrawReasons,
		};

		impl<T: Config> ReservableCurrency<T::AccountId> for Pallet<T>
		where
			<T as Config>::NativeCurrency: Currency<T::AccountId, Balance = T::Balance>,
			<T as Config>::NativeCurrency: ReservableCurrency<T::AccountId, Balance = T::Balance>,
		{
			fn can_reserve(who: &T::AccountId, value: Self::Balance) -> bool {
				<<T as Config>::NativeCurrency>::can_reserve(who, value)
			}

			fn slash_reserved(
				who: &T::AccountId,
				value: Self::Balance,
			) -> (Self::NegativeImbalance, Self::Balance) {
				<<T as Config>::NativeCurrency>::slash_reserved(who, value)
			}

			fn reserved_balance(who: &T::AccountId) -> Self::Balance {
				<<T as Config>::NativeCurrency>::reserved_balance(who)
			}

			fn reserve(who: &T::AccountId, value: Self::Balance) -> DispatchResult {
				<<T as Config>::NativeCurrency>::reserve(who, value)
			}

			fn unreserve(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
				<<T as Config>::NativeCurrency>::unreserve(who, value)
			}

			fn repatriate_reserved(
				slashed: &T::AccountId,
				beneficiary: &T::AccountId,
				value: Self::Balance,
				status: BalanceStatus,
			) -> Result<Self::Balance, DispatchError> {
				<<T as Config>::NativeCurrency>::repatriate_reserved(
					slashed,
					beneficiary,
					value,
					status,
				)
			}
		}

		impl<T: Config> Currency<T::AccountId> for Pallet<T>
		where
			<T as Config>::NativeCurrency: Currency<T::AccountId, Balance = T::Balance>,
		{
			type Balance = <<T as Config>::NativeCurrency as Currency<T::AccountId>>::Balance;
			type PositiveImbalance =
				<<T as Config>::NativeCurrency as Currency<T::AccountId>>::PositiveImbalance;
			type NegativeImbalance =
				<<T as Config>::NativeCurrency as Currency<T::AccountId>>::NegativeImbalance;

			fn total_balance(who: &T::AccountId) -> Self::Balance {
				<<T as Config>::NativeCurrency>::total_balance(who)
			}

			fn can_slash(who: &T::AccountId, value: Self::Balance) -> bool {
				<<T as Config>::NativeCurrency>::can_slash(who, value)
			}

			fn total_issuance() -> Self::Balance {
				<<T as Config>::NativeCurrency>::total_issuance()
			}

			fn minimum_balance() -> Self::Balance {
				<<T as Config>::NativeCurrency>::minimum_balance()
			}

			fn burn(amount: Self::Balance) -> Self::PositiveImbalance {
				<<T as Config>::NativeCurrency>::burn(amount)
			}

			fn issue(amount: Self::Balance) -> Self::NegativeImbalance {
				<<T as Config>::NativeCurrency>::issue(amount)
			}

			fn pair(amount: Self::Balance) -> (Self::PositiveImbalance, Self::NegativeImbalance) {
				<<T as Config>::NativeCurrency>::pair(amount)
			}

			fn free_balance(who: &T::AccountId) -> Self::Balance {
				<<T as Config>::NativeCurrency>::free_balance(who)
			}

			fn ensure_can_withdraw(
				who: &T::AccountId,
				amount: Self::Balance,
				reasons: WithdrawReasons,
				new_balance: Self::Balance,
			) -> DispatchResult {
				<<T as Config>::NativeCurrency>::ensure_can_withdraw(
					who,
					amount,
					reasons,
					new_balance,
				)
			}

			fn transfer(
				source: &T::AccountId,
				dest: &T::AccountId,
				value: Self::Balance,
				existence_requirement: ExistenceRequirement,
			) -> DispatchResult {
				<<T as Config>::NativeCurrency>::transfer(
					source,
					dest,
					value,
					existence_requirement,
				)
			}

			fn slash(
				who: &T::AccountId,
				value: Self::Balance,
			) -> (Self::NegativeImbalance, Self::Balance) {
				<<T as Config>::NativeCurrency>::slash(who, value)
			}

			fn deposit_into_existing(
				who: &T::AccountId,
				value: Self::Balance,
			) -> Result<Self::PositiveImbalance, DispatchError> {
				<<T as Config>::NativeCurrency>::deposit_into_existing(who, value)
			}

			fn resolve_into_existing(
				who: &T::AccountId,
				value: Self::NegativeImbalance,
			) -> Result<(), Self::NegativeImbalance> {
				<<T as Config>::NativeCurrency>::resolve_into_existing(who, value)
			}

			fn deposit_creating(
				who: &T::AccountId,
				value: Self::Balance,
			) -> Self::PositiveImbalance {
				<<T as Config>::NativeCurrency>::deposit_creating(who, value)
			}

			fn resolve_creating(who: &T::AccountId, value: Self::NegativeImbalance) {
				<<T as Config>::NativeCurrency>::resolve_creating(who, value)
			}

			fn withdraw(
				who: &T::AccountId,
				value: Self::Balance,
				reasons: WithdrawReasons,
				liveness: ExistenceRequirement,
			) -> Result<Self::NegativeImbalance, DispatchError> {
				<<T as Config>::NativeCurrency>::withdraw(who, value, reasons, liveness)
			}

			fn settle(
				who: &T::AccountId,
				value: Self::PositiveImbalance,
				reasons: WithdrawReasons,
				liveness: ExistenceRequirement,
			) -> Result<(), Self::PositiveImbalance> {
				<<T as Config>::NativeCurrency>::settle(who, value, reasons, liveness)
			}

			fn make_free_balance_be(
				who: &T::AccountId,
				balance: Self::Balance,
			) -> SignedImbalance<Self::Balance, Self::PositiveImbalance> {
				<<T as Config>::NativeCurrency>::make_free_balance_be(who, balance)
			}
		}
	}

	mod fungible {
		use super::*;

		use frame_support::traits::{
			tokens::{
				fungible::{Inspect, InspectHold, Mutate, MutateHold, Transfer, Unbalanced},
				DepositConsequence, WithdrawConsequence,
			},
			LockableCurrency, WithdrawReasons,
		};
		use orml_traits::LockIdentifier;

		impl<T: Config> MutateHold<T::AccountId> for Pallet<T>
		where
			<T as Config>::NativeCurrency: InspectHold<T::AccountId, Balance = T::Balance>,
			<T as Config>::NativeCurrency: Transfer<T::AccountId, Balance = T::Balance>,
			<T as Config>::NativeCurrency: MutateHold<T::AccountId, Balance = T::Balance>,
		{
			fn hold(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
				<<T as Config>::NativeCurrency>::hold(who, amount)
			}

			fn release(
				who: &T::AccountId,
				amount: Self::Balance,
				best_effort: bool,
			) -> Result<Self::Balance, DispatchError> {
				<<T as Config>::NativeCurrency>::release(who, amount, best_effort)
			}

			fn transfer_held(
				source: &T::AccountId,
				dest: &T::AccountId,
				amount: Self::Balance,
				best_effort: bool,
				on_held: bool,
			) -> Result<Self::Balance, DispatchError> {
				<<T as Config>::NativeCurrency>::transfer_held(
					source,
					dest,
					amount,
					best_effort,
					on_held,
				)
			}
		}

		impl<T: Config> Mutate<T::AccountId> for Pallet<T>
		where
			<T as Config>::NativeCurrency: Inspect<T::AccountId, Balance = T::Balance>,
			<T as Config>::NativeCurrency: Mutate<T::AccountId, Balance = T::Balance>,
		{
			fn mint_into(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
				<<T as Config>::NativeCurrency>::mint_into(who, amount)
			}
			fn burn_from(
				who: &T::AccountId,
				amount: Self::Balance,
			) -> Result<Self::Balance, DispatchError> {
				<<T as Config>::NativeCurrency>::burn_from(who, amount)
			}

			fn slash(
				who: &T::AccountId,
				amount: Self::Balance,
			) -> Result<Self::Balance, DispatchError> {
				<<T as Config>::NativeCurrency>::slash(who, amount)
			}
			fn teleport(
				source: &T::AccountId,
				dest: &T::AccountId,
				amount: Self::Balance,
			) -> Result<Self::Balance, DispatchError> {
				<<T as Config>::NativeCurrency>::teleport(source, dest, amount)
			}
		}

		impl<T: Config> Unbalanced<T::AccountId> for Pallet<T>
		where
			<T as Config>::NativeCurrency: Unbalanced<T::AccountId, Balance = T::Balance>,
		{
			fn set_balance(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
				<<T as Config>::NativeCurrency>::set_balance(who, amount)
			}

			fn set_total_issuance(amount: Self::Balance) {
				<<T as Config>::NativeCurrency>::set_total_issuance(amount)
			}

			fn decrease_balance(
				who: &T::AccountId,
				amount: Self::Balance,
			) -> Result<Self::Balance, DispatchError> {
				<<T as Config>::NativeCurrency>::decrease_balance(who, amount)
			}

			fn decrease_balance_at_most(
				who: &T::AccountId,
				amount: Self::Balance,
			) -> Self::Balance {
				<<T as Config>::NativeCurrency>::decrease_balance_at_most(who, amount)
			}

			fn increase_balance(
				who: &T::AccountId,
				amount: Self::Balance,
			) -> Result<Self::Balance, DispatchError> {
				<<T as Config>::NativeCurrency>::increase_balance(who, amount)
			}

			fn increase_balance_at_most(
				who: &T::AccountId,
				amount: Self::Balance,
			) -> Self::Balance {
				<<T as Config>::NativeCurrency>::increase_balance_at_most(who, amount)
			}
		}

		impl<T: Config> Transfer<T::AccountId> for Pallet<T>
		where
			<T as Config>::NativeCurrency: Transfer<T::AccountId, Balance = T::Balance>,
		{
			fn transfer(
				source: &T::AccountId,
				dest: &T::AccountId,
				amount: Self::Balance,
				keep_alive: bool,
			) -> Result<Self::Balance, DispatchError> {
				<<T as Config>::NativeCurrency>::transfer(source, dest, amount, keep_alive)
			}
		}

		impl<T: Config> LockableCurrency<T::AccountId> for Pallet<T>
		where
			<T as Config>::NativeCurrency: LockableCurrency<T::AccountId, Balance = T::Balance>,
		{
			type Moment = <T::NativeCurrency as LockableCurrency<T::AccountId>>::Moment;
			type MaxLocks = <T::NativeCurrency as LockableCurrency<T::AccountId>>::MaxLocks;

			fn set_lock(
				id: LockIdentifier,
				who: &T::AccountId,
				amount: T::Balance,
				reasons: WithdrawReasons,
			) {
				<T::NativeCurrency as LockableCurrency<T::AccountId>>::set_lock(
					id, who, amount, reasons,
				);
			}

			fn extend_lock(
				id: LockIdentifier,
				who: &T::AccountId,
				amount: T::Balance,
				reasons: WithdrawReasons,
			) {
				<T::NativeCurrency as LockableCurrency<T::AccountId>>::extend_lock(
					id, who, amount, reasons,
				);
			}

			fn remove_lock(id: LockIdentifier, who: &T::AccountId) {
				<T::NativeCurrency as LockableCurrency<T::AccountId>>::remove_lock(id, who);
			}
		}

		impl<T: Config> Inspect<T::AccountId> for Pallet<T>
		where
			<T as Config>::NativeCurrency: Inspect<T::AccountId, Balance = T::Balance>,
		{
			type Balance = T::Balance;

			fn total_issuance() -> Self::Balance {
				<<T as Config>::NativeCurrency>::total_issuance()
			}

			fn minimum_balance() -> Self::Balance {
				<<T as Config>::NativeCurrency>::minimum_balance()
			}

			fn balance(who: &T::AccountId) -> Self::Balance {
				<<T as Config>::NativeCurrency>::balance(who)
			}

			fn reducible_balance(who: &T::AccountId, keep_alive: bool) -> Self::Balance {
				<<T as Config>::NativeCurrency>::reducible_balance(who, keep_alive)
			}

			fn can_deposit(
				who: &T::AccountId,
				amount: Self::Balance,
				mint: bool,
			) -> DepositConsequence {
				<<T as Config>::NativeCurrency>::can_deposit(who, amount, mint)
			}

			fn can_withdraw(
				who: &T::AccountId,
				amount: Self::Balance,
			) -> WithdrawConsequence<Self::Balance> {
				<<T as Config>::NativeCurrency>::can_withdraw(who, amount)
			}
		}

		impl<T: Config> InspectHold<T::AccountId> for Pallet<T>
		where
			<T as Config>::NativeCurrency:
				Inspect<T::AccountId, Balance = T::Balance> + InspectHold<T::AccountId>,
		{
			fn balance_on_hold(who: &T::AccountId) -> Self::Balance {
				<<T as Config>::NativeCurrency>::balance_on_hold(who)
			}

			fn can_hold(who: &T::AccountId, amount: Self::Balance) -> bool {
				<<T as Config>::NativeCurrency>::can_hold(who, amount)
			}
		}
	}

	mod fungibles {
		use super::*;

		use frame_support::traits::tokens::{
			fungible::{
				Inspect as NativeInspect, InspectHold as NativeInspectHold, Mutate as NativeMutate,
				MutateHold as NativeMutateHold, Transfer as NativeTransfer,
				Unbalanced as NativeUnbalanced,
			},
			fungibles::{Inspect, InspectHold, Mutate, MutateHold, Transfer, Unbalanced},
			DepositConsequence, WithdrawConsequence,
		};

		impl<T: Config> Unbalanced<T::AccountId> for Pallet<T>
		where
			<T as Config>::NativeCurrency: NativeUnbalanced<T::AccountId, Balance = T::Balance>,
			<T as Config>::MultiCurrency:
				Unbalanced<T::AccountId, Balance = T::Balance, AssetId = T::AssetId>,
		{
			fn set_balance(
				asset: Self::AssetId,
				who: &T::AccountId,
				amount: Self::Balance,
			) -> DispatchResult {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::set_balance(who, amount)
				}
				let asset = valid_asset_id::<T>(asset).ok_or(Error::<T>::InvalidCurrency)?;
				<<T as Config>::MultiCurrency>::set_balance(asset, who, amount)
			}

			fn set_total_issuance(asset: Self::AssetId, amount: Self::Balance) {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::set_total_issuance(amount)
				}
				if let Some(asset) = valid_asset_id::<T>(asset) {
					<<T as Config>::MultiCurrency>::set_total_issuance(asset, amount)
				}
			}

			fn decrease_balance(
				asset: Self::AssetId,
				who: &T::AccountId,
				amount: Self::Balance,
			) -> Result<Self::Balance, DispatchError> {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::decrease_balance(who, amount)
				}
				let asset = valid_asset_id::<T>(asset).ok_or(Error::<T>::InvalidCurrency)?;
				<<T as Config>::MultiCurrency>::decrease_balance(asset, who, amount)
			}

			fn decrease_balance_at_most(
				asset: Self::AssetId,
				who: &T::AccountId,
				amount: Self::Balance,
			) -> Self::Balance {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::decrease_balance_at_most(who, amount)
				}
				if let Some(asset) = valid_asset_id::<T>(asset) {
					return <<T as Config>::MultiCurrency>::decrease_balance_at_most(
						asset, who, amount,
					)
				}
				T::Balance::zero()
			}

			fn increase_balance(
				asset: Self::AssetId,
				who: &T::AccountId,
				amount: Self::Balance,
			) -> Result<Self::Balance, DispatchError> {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::increase_balance(who, amount)
				}
				let asset = valid_asset_id::<T>(asset).ok_or(Error::<T>::InvalidCurrency)?;
				<<T as Config>::MultiCurrency>::increase_balance(asset, who, amount)
			}

			fn increase_balance_at_most(
				asset: Self::AssetId,
				who: &T::AccountId,
				amount: Self::Balance,
			) -> Self::Balance {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::increase_balance_at_most(who, amount)
				}
				if let Some(asset) = valid_asset_id::<T>(asset) {
					return <<T as Config>::MultiCurrency>::increase_balance_at_most(
						asset, who, amount,
					)
				}
				T::Balance::zero()
			}
		}

		impl<T: Config> Transfer<T::AccountId> for Pallet<T>
		where
			<T as Config>::NativeCurrency: NativeTransfer<T::AccountId, Balance = T::Balance>,
			<T as Config>::NativeCurrency: NativeInspect<T::AccountId, Balance = T::Balance>,
			<T as Config>::MultiCurrency:
				Transfer<T::AccountId, Balance = T::Balance, AssetId = T::AssetId>,
		{
			fn transfer(
				asset: Self::AssetId,
				source: &T::AccountId,
				dest: &T::AccountId,
				amount: Self::Balance,
				keep_alive: bool,
			) -> Result<Self::Balance, DispatchError> {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::transfer(
						source, dest, amount, keep_alive,
					)
				}
				let asset = valid_asset_id::<T>(asset).ok_or(Error::<T>::InvalidCurrency)?;
				<<T as Config>::MultiCurrency>::transfer(asset, source, dest, amount, keep_alive)
			}
		}

		impl<T: Config> MutateHold<T::AccountId> for Pallet<T>
		where
			<T as Config>::NativeCurrency: NativeInspectHold<T::AccountId, Balance = T::Balance>,
			<T as Config>::NativeCurrency: NativeTransfer<T::AccountId, Balance = T::Balance>,
			<T as Config>::NativeCurrency: NativeMutateHold<T::AccountId, Balance = T::Balance>,

			<T as Config>::MultiCurrency:
				InspectHold<T::AccountId, Balance = T::Balance, AssetId = T::AssetId>,
			<T as Config>::MultiCurrency:
				Transfer<T::AccountId, Balance = T::Balance, AssetId = T::AssetId>,
			<T as Config>::MultiCurrency:
				MutateHold<T::AccountId, Balance = T::Balance, AssetId = T::AssetId>,
		{
			fn hold(
				asset: Self::AssetId,
				who: &T::AccountId,
				amount: Self::Balance,
			) -> DispatchResult {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::hold(who, amount)
				}
				let asset = valid_asset_id::<T>(asset).ok_or(Error::<T>::InvalidCurrency)?;
				<<T as Config>::MultiCurrency>::hold(asset, who, amount)
			}

			fn release(
				asset: Self::AssetId,
				who: &T::AccountId,
				amount: Self::Balance,
				best_effort: bool,
			) -> Result<Self::Balance, DispatchError> {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::release(who, amount, best_effort)
				}
				let asset = valid_asset_id::<T>(asset).ok_or(Error::<T>::InvalidCurrency)?;
				<<T as Config>::MultiCurrency>::release(asset, who, amount, best_effort)
			}

			fn transfer_held(
				asset: Self::AssetId,
				source: &T::AccountId,
				dest: &T::AccountId,
				amount: Self::Balance,
				best_effort: bool,
				on_hold: bool,
			) -> Result<Self::Balance, DispatchError> {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::transfer_held(
						source,
						dest,
						amount,
						best_effort,
						on_hold,
					)
				}
				let asset = valid_asset_id::<T>(asset).ok_or(Error::<T>::InvalidCurrency)?;
				<<T as Config>::MultiCurrency>::transfer_held(
					asset,
					source,
					dest,
					amount,
					best_effort,
					on_hold,
				)
			}
		}

		impl<T: Config> Mutate<T::AccountId> for Pallet<T>
		where
			<T as Config>::MultiCurrency:
				Inspect<T::AccountId, Balance = T::Balance, AssetId = T::AssetId>,
			<T as Config>::MultiCurrency:
				Mutate<T::AccountId, Balance = T::Balance, AssetId = T::AssetId>,
			<T as Config>::NativeCurrency: NativeInspect<T::AccountId, Balance = T::Balance>,
			<T as Config>::NativeCurrency: NativeMutate<T::AccountId, Balance = T::Balance>,
		{
			fn mint_into(
				asset: Self::AssetId,
				who: &T::AccountId,
				amount: Self::Balance,
			) -> DispatchResult {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::mint_into(who, amount)
				}
				let asset = valid_asset_id::<T>(asset).ok_or(Error::<T>::InvalidCurrency)?;
				<<T as Config>::MultiCurrency>::mint_into(asset, who, amount)
			}
			fn burn_from(
				asset: Self::AssetId,
				who: &T::AccountId,
				amount: Self::Balance,
			) -> Result<Self::Balance, DispatchError> {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::burn_from(who, amount)
				}
				let asset = valid_asset_id::<T>(asset).ok_or(Error::<T>::InvalidCurrency)?;
				<<T as Config>::MultiCurrency>::burn_from(asset, who, amount)
			}

			fn slash(
				asset: Self::AssetId,
				who: &T::AccountId,
				amount: Self::Balance,
			) -> Result<Self::Balance, DispatchError> {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::slash(who, amount)
				}
				let asset = valid_asset_id::<T>(asset).ok_or(Error::<T>::InvalidCurrency)?;
				<<T as Config>::MultiCurrency>::slash(asset, who, amount)
			}
			fn teleport(
				asset: Self::AssetId,
				source: &T::AccountId,
				dest: &T::AccountId,
				amount: Self::Balance,
			) -> Result<Self::Balance, DispatchError> {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::teleport(source, dest, amount)
				}
				let asset = valid_asset_id::<T>(asset).ok_or(Error::<T>::InvalidCurrency)?;
				<<T as Config>::MultiCurrency>::teleport(asset, source, dest, amount)
			}
		}

		impl<T: Config> Inspect<T::AccountId> for Pallet<T>
		where
			<T as Config>::MultiCurrency:
				Inspect<T::AccountId, Balance = T::Balance, AssetId = T::AssetId>,

			<T as Config>::NativeCurrency: NativeInspect<T::AccountId, Balance = T::Balance>,
		{
			type AssetId = T::AssetId;
			type Balance = T::Balance;

			fn total_issuance(asset: Self::AssetId) -> Self::Balance {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::total_issuance()
				}
				if let Some(asset) = valid_asset_id::<T>(asset) {
					return <<T as Config>::MultiCurrency>::total_issuance(asset)
				}
				T::Balance::zero()
			}

			fn minimum_balance(asset: Self::AssetId) -> Self::Balance {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::minimum_balance()
				}
				if let Some(asset) = valid_asset_id::<T>(asset) {
					return <<T as Config>::MultiCurrency>::minimum_balance(asset)
				}
				T::Balance::zero()
			}

			fn balance(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::balance(who)
				}
				if let Some(asset) = valid_asset_id::<T>(asset) {
					return <<T as Config>::MultiCurrency>::balance(asset, who)
				}
				T::Balance::zero()
			}

			fn reducible_balance(
				asset: Self::AssetId,
				who: &T::AccountId,
				keep_alive: bool,
			) -> Self::Balance {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::reducible_balance(who, keep_alive)
				}
				if let Some(asset) = valid_asset_id::<T>(asset) {
					return <<T as Config>::MultiCurrency>::reducible_balance(asset, who, keep_alive)
				}
				T::Balance::zero()
			}

			fn can_deposit(
				asset: Self::AssetId,
				who: &T::AccountId,
				amount: Self::Balance,
				mint: bool,
			) -> DepositConsequence {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::can_deposit(who, amount, mint)
				}
				if let Some(asset) = valid_asset_id::<T>(asset) {
					return <<T as Config>::MultiCurrency>::can_deposit(asset, who, amount, mint)
				}
				DepositConsequence::UnknownAsset
			}

			fn can_withdraw(
				asset: Self::AssetId,
				who: &T::AccountId,
				amount: Self::Balance,
			) -> WithdrawConsequence<Self::Balance> {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::can_withdraw(who, amount)
				}
				if let Some(asset) = valid_asset_id::<T>(asset) {
					return <<T as Config>::MultiCurrency>::can_withdraw(asset, who, amount)
				}
				WithdrawConsequence::UnknownAsset
			}

			fn asset_exists(asset: Self::AssetId) -> bool {
				valid_asset_id::<T>(asset).is_some()
			}
		}

		impl<T: Config> InspectHold<T::AccountId> for Pallet<T>
		where
			<T as Config>::MultiCurrency: Inspect<T::AccountId, Balance = T::Balance, AssetId = T::AssetId>
				+ InspectHold<T::AccountId>,
			<T as Config>::NativeCurrency:
				NativeInspect<T::AccountId, Balance = T::Balance> + NativeInspectHold<T::AccountId>,
		{
			fn balance_on_hold(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::balance_on_hold(who)
				}
				if let Some(asset) = valid_asset_id::<T>(asset) {
					return <<T as Config>::MultiCurrency>::balance_on_hold(asset, who)
				}
				T::Balance::zero()
			}

			fn can_hold(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance) -> bool {
				if asset == T::NativeAssetId::get() {
					return <<T as Config>::NativeCurrency>::can_hold(who, amount)
				}
				if let Some(asset) = valid_asset_id::<T>(asset) {
					return <<T as Config>::MultiCurrency>::can_hold(asset, who, amount)
				}
				false
			}
		}
	}
}
