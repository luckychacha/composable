use crate::{
	mock::{AccountId, Extrinsic, RuntimeCall, RuntimeEvent, *},
	AssetInfo, Error, PrePrice, Withdraw, *,
};
use codec::Decode;
use composable_traits::{
	defi::CurrencyPair,
	oracle::{self, Price},
};
use frame_support::{
	assert_noop, assert_ok,
	traits::{Currency as _, Hooks},
	BoundedVec,
};
use pallet_balances::Error as BalancesError;
use parking_lot::RwLock;
use sp_core::offchain::{testing, OffchainDbExt, OffchainWorkerExt, TransactionPoolExt};
use sp_io::TestExternalities;
use sp_keystore::{testing::KeyStore, KeystoreExt, SyncCryptoStore};
use sp_runtime::{
	traits::{BadOrigin, Zero},
	FixedPointNumber, FixedU128, Percent, RuntimeAppPublic,
};
use std::sync::Arc;

use crate::validation::ValidBlockInterval;
use composable_support::validation::Validated;
use composable_tests_helpers::{
	prop_assert_noop, prop_assert_ok,
	test::currency::{BTC, NORMALIZED, PICA},
};
use proptest::prelude::*;

use composable_tests_helpers::test::{block::process_and_progress_blocks, helper::RuntimeTrait};
use composable_traits::{oracle::RewardTracker, time::MS_PER_YEAR_NAIVE};
use sp_core::H256;

const UNIT: Balance = 1_000_000_000_000;

prop_compose! {
	fn asset_info()
		(
			answers in MinAnswerBound::get()..MaxAnswerBound::get(),
			answers_delta in 1..MaxAnswerBound::get() - MinAnswerBound::get(),
			block_interval in (StalePrice::get()+1)..(BlockNumber::MAX/16),
			threshold in 0..100_u8,
			reward in 0..u128::MAX,
			slash in 0..u128::MAX,
		) -> AssetInfo<Percent, BlockNumber, Balance> {
			let (min_answers, max_answers) = if answers == MaxAnswerBound::get() {
				(answers.saturating_sub(answers_delta).max(MinAnswerBound::get()), answers)
			} else {
				(answers, answers.saturating_add(answers_delta).min(MaxAnswerBound::get()))
			};
			let threshold: Percent = Percent::from_percent(threshold);

			AssetInfo {
				threshold,
				min_answers,
				max_answers,
				block_interval,
				reward_weight: reward,
				slash,
				emit_price_changes: false,
			}
		}
}

prop_compose! {
	fn asset_id()
		(x in 0..AssetId::MAX) -> AssetId {
			x
		}
}

prop_compose! {
	fn price_value()
		(x in 0..PriceValue::MAX) -> PriceValue {
		x
	}
}

prop_compose! {
	fn account_id()
		(x in 0..u64::MAX) -> AccountId {
			let h256 = H256::from_low_u64_be(x);
			AccountId::from_h256(h256)
		}
}

mod add_asset_and_info {
	use super::*;

	proptest! {
		#![proptest_config(ProptestConfig::with_cases(10_000))]

		#[test]
		fn normal_asset_and_info_assert(
			asset_id in asset_id(),
			asset_info in asset_info(),
		) {
			new_test_ext().execute_with(|| {
				let root_account = get_root_account();

				prop_assert_ok!(Oracle::add_asset_and_info(
					RuntimeOrigin::signed(root_account),
					asset_id,
					Validated::new(asset_info.threshold).unwrap(),
					Validated::new(asset_info.min_answers).unwrap(),
					Validated::new(asset_info.max_answers).unwrap(),
					Validated::new(asset_info.block_interval).unwrap(),
					asset_info.reward_weight,
					asset_info.slash,
					asset_info.emit_price_changes,
				));

				Ok(())
			})?;
		}

		#[test]
		fn asset_count_should_not_increase_when_updating_asset_info(
			asset_id in asset_id(),
			asset_info_1 in asset_info(),
			asset_info_2 in asset_info(),
		) {
			new_test_ext().execute_with(|| {
				let root_account = get_root_account();

				prop_assert_ok!(Oracle::add_asset_and_info(
					RuntimeOrigin::signed(root_account),
					asset_id,
					Validated::new(asset_info_1.threshold).unwrap(),
					Validated::new(asset_info_1.min_answers).unwrap(),
					Validated::new(asset_info_1.max_answers).unwrap(),
					Validated::new(asset_info_1.block_interval).unwrap(),
					asset_info_1.reward_weight.min(u128::MAX / 2),
					asset_info_1.slash,
					asset_info_1.emit_price_changes,
				));

				// does not increment asset_count because we have info for the same asset_id
				prop_assert_ok!(Oracle::add_asset_and_info(
					RuntimeOrigin::signed(root_account),
					asset_id,
					Validated::new(asset_info_2.threshold).unwrap(),
					Validated::new(asset_info_2.min_answers).unwrap(),
					Validated::new(asset_info_2.max_answers).unwrap(),
					Validated::new(asset_info_2.block_interval).unwrap(),
					asset_info_2.reward_weight.min(u128::MAX / 2),
					asset_info_2.slash,
					asset_info_2.emit_price_changes,
				));
				prop_assert_eq!(Oracle::assets_count(), 1);

				Ok(())
			})?;
		}

		#[test]
		fn fails_when_non_root_account(
			asset_id in asset_id(),
			asset_info in asset_info(),
			account_id in account_id(),
		) {
			// very small chance of happening, but for correctness' sake ;)
			prop_assume!(account_id != get_root_account());

			new_test_ext().execute_with(|| {
				prop_assert_noop!(
					Oracle::add_asset_and_info(
						RuntimeOrigin::signed(account_id),
						asset_id,
						Validated::new(asset_info.threshold).unwrap(),
						Validated::new(asset_info.min_answers).unwrap(),
						Validated::new(asset_info.max_answers).unwrap(),
						Validated::new(asset_info.block_interval).unwrap(),
						asset_info.reward_weight,
						asset_info.slash,
						asset_info.emit_price_changes,
					),
					BadOrigin
				);
				Ok(())
			})?;
		}


		#[test]
		fn can_have_multiple_assets(
			asset_id_1 in asset_id(),
			asset_id_2 in asset_id(),
			mut asset_info_1 in asset_info(),
			mut asset_info_2 in asset_info(),
		) {
			new_test_ext().execute_with(|| {
				let root_account = get_root_account();

			prop_assume!(asset_id_1 != asset_id_2);
				asset_info_1.reward_weight = asset_info_1.reward_weight.min(u128::MAX / 2);
				asset_info_2.reward_weight = asset_info_2.reward_weight.min(u128::MAX / 2);
				prop_assert_ok!(Oracle::add_asset_and_info(
					RuntimeOrigin::signed(root_account),
					asset_id_1,
					Validated::new(asset_info_1.threshold).unwrap(),
					Validated::new(asset_info_1.min_answers).unwrap(),
					Validated::new(asset_info_1.max_answers).unwrap(),
					Validated::new(asset_info_1.block_interval).unwrap(),
					asset_info_1.reward_weight,
					asset_info_1.slash,
					asset_info_1.emit_price_changes,
				));

				prop_assert_ok!(Oracle::add_asset_and_info(
					RuntimeOrigin::signed(root_account),
					asset_id_2,
					Validated::new(asset_info_2.threshold).unwrap(),
					Validated::new(asset_info_2.min_answers).unwrap(),
					Validated::new(asset_info_2.max_answers).unwrap(),
					Validated::new(asset_info_2.block_interval).unwrap(),
					asset_info_2.reward_weight,
					asset_info_2.slash,
					asset_info_2.emit_price_changes,
				));

				prop_assert_eq!(Oracle::asset_info(asset_id_1), Some(asset_info_1));
				prop_assert_eq!(Oracle::asset_info(asset_id_2), Some(asset_info_2));
				prop_assert_eq!(Oracle::assets_count(), 2);

				Ok(())
			})?;
		}



		#[test]
		fn max_answers_cannot_be_less_than_min_answers(
			asset_id in asset_id(),
			asset_info in asset_info(),
		) {
			let root_account = get_root_account();

			new_test_ext().execute_with(|| {
				prop_assert_noop!(
					Oracle::add_asset_and_info(
						RuntimeOrigin::signed(root_account),
						asset_id,
						Validated::new(asset_info.threshold).unwrap(),       // notice that max and min are reversed:
						Validated::new(asset_info.max_answers).unwrap(),     // MIN
						Validated::new(asset_info.min_answers - 1).unwrap(), // MAX
						Validated::new(asset_info.block_interval).unwrap(),
						asset_info.reward_weight,
						asset_info.slash,
						asset_info.emit_price_changes,
					),
					Error::<Test>::MaxAnswersLessThanMinAnswers
				);

				Ok(())
			})?;
		}

		#[test]
		fn cannot_exceed_max_assets_count(
			asset_id_1 in asset_id(),
			asset_id_2 in asset_id(),
			asset_id_3 in asset_id(),
			asset_id_4 in asset_id(),
			asset_id_5 in asset_id(),
			asset_id_6 in asset_id(),
			asset_id_7 in asset_id(),
			asset_id_8 in asset_id(),
			asset_id_9 in asset_id(),
			asset_id_10 in asset_id(),
			asset_id_11 in asset_id(),
			asset_id_12 in asset_id(),
			asset_id_13 in asset_id(),
			asset_id_14 in asset_id(),
			asset_id_15 in asset_id(),
			asset_id_16 in asset_id(),
			asset_info_1 in asset_info(),
			asset_info_2 in asset_info(),
			asset_info_3 in asset_info(),
			asset_info_4 in asset_info(),
			asset_info_5 in asset_info(),
			asset_info_6 in asset_info(),
			asset_info_7 in asset_info(),
			asset_info_8 in asset_info(),
			asset_info_9 in asset_info(),
			asset_info_10 in asset_info(),
			asset_info_11 in asset_info(),
			asset_info_12 in asset_info(),
			asset_info_13 in asset_info(),
			asset_info_14 in asset_info(),
			asset_info_15 in asset_info(),
			asset_info_16 in asset_info(),
		) {
			new_test_ext().execute_with(|| {
				let root_account = get_root_account();

				// If the following check fails, that means that the mock.rs was changed,
				// and therefore this test should also be changed.
				prop_assert_eq!(MaxAssetsCount::get(), 15_u32);

				let asset_ids = [
					asset_id_1,
					asset_id_2,
					asset_id_3,
					asset_id_4,
					asset_id_5,
					asset_id_6,
					asset_id_7,
					asset_id_8,
					asset_id_9,
					asset_id_10,
					asset_id_11,
					asset_id_12,
					asset_id_13,
					asset_id_14,
					asset_id_15,
					asset_id_16,
				];

				// Items of asset_ids must be unique.
				let mut tmp_asset_ids = asset_ids.clone();
				tmp_asset_ids.sort();
				let mut tmp_asset_ids = tmp_asset_ids.iter().collect::<Vec<_>>();
				tmp_asset_ids.dedup();
				prop_assume!(asset_ids.len() == tmp_asset_ids.len());

				let asset_infos = [
					asset_info_1,
					asset_info_2,
					asset_info_3,
					asset_info_4,
					asset_info_5,
					asset_info_6,
					asset_info_7,
					asset_info_8,
					asset_info_9,
					asset_info_10,
					asset_info_11,
					asset_info_12,
					asset_info_13,
					asset_info_14,
					asset_info_15,
					asset_info_16,
				];

				let weight = u128::MAX / u128::try_from(asset_infos.len()).unwrap();

				let asset_infos = asset_infos.map(|mut asset_info| {
					asset_info.reward_weight = asset_info.reward_weight.min(weight);
					asset_info
				});

				// First we create 15 assets, which is allowed because within mock.rs, we see:
				// pub const MaxAssetsCount: u32 = 15;
				// it would be nicer to do this in a loop up to MaxAssetsCount,
				// but AFAIK it is not possible to generate props within the proptest body.
				for (asset_id, asset_info) in asset_ids.iter().take(asset_ids.len() - 1).zip(asset_infos.iter()) {
					prop_assert_ok!(Oracle::add_asset_and_info(
						RuntimeOrigin::signed(root_account),
						*asset_id,
						Validated::new(asset_info.threshold).unwrap(),
						Validated::new(asset_info.min_answers).unwrap(),
						Validated::new(asset_info.max_answers).unwrap(),
						Validated::new(asset_info.block_interval).unwrap(),
						asset_info.reward_weight,
						asset_info.slash,
						asset_info.emit_price_changes,
					));

					prop_assert_eq!(Oracle::asset_info(asset_id), Some(asset_info.clone()));
				}

				prop_assert_eq!(Oracle::assets_count(), 15);

				let last_asset_id = &asset_ids[asset_ids.len() - 1];
				let last_asset_info = &asset_infos[asset_infos.len() - 1];
				prop_assert_noop!(Oracle::add_asset_and_info(
					RuntimeOrigin::signed(root_account),
					*last_asset_id,
					Validated::new(last_asset_info.threshold).unwrap(),
					Validated::new(last_asset_info.min_answers).unwrap(),
					Validated::new(last_asset_info.max_answers).unwrap(),
					Validated::new(last_asset_info.block_interval).unwrap(),
					last_asset_info.reward_weight,
					last_asset_info.slash,
					last_asset_info.emit_price_changes,
				),
				Error::<Test>::ExceedAssetsCount);

				Ok(())
			})?;
		}
	}
}

mod set_signer {
	use super::*;
	proptest! {
		#![proptest_config(ProptestConfig::with_cases(10_000))]

		#[test]
		fn root_can_be_controller_and_set_signer(
			_caller_account in account_id(),
			signer_account in account_id(),
		) {
			new_test_ext().execute_with(|| {
				let root_account = get_root_account();
				prop_assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), root_account, signer_account));
				prop_assert_eq!(Oracle::controller_to_signer(root_account), Some(signer_account));
				prop_assert_eq!(Oracle::signer_to_controller(signer_account), Some(root_account));

				Ok(())
			})?;
		}

		#[test]
		fn signer_can_also_become_controller(
			controller_account in account_id(),
			signer_account_1 in account_id(), // Will also become a controller.
			signer_account_2 in account_id(), // Will become the signer associated with the controller above.
			controller_balance in MinStake::get()..Balance::MAX,
			signer_1_balance in MinStake::get()..Balance::MAX,
		) {
			prop_assume!(signer_account_1 != signer_account_2);

			new_test_ext().execute_with(|| {
				let root_account = get_root_account();
				Balances::make_free_balance_be(&controller_account, controller_balance);

				prop_assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), controller_account, signer_account_1));
				prop_assert_eq!(Oracle::controller_to_signer(controller_account), Some(signer_account_1));
				prop_assert_eq!(Oracle::signer_to_controller(signer_account_1), Some(controller_account));

				Balances::make_free_balance_be(&signer_account_1, signer_1_balance);

				prop_assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), signer_account_1, signer_account_2));
				prop_assert_eq!(Oracle::controller_to_signer(signer_account_1), Some(signer_account_2));
				prop_assert_eq!(Oracle::signer_to_controller(signer_account_2), Some(signer_account_1));

				Ok(())
			})?;
		}

		#[test]
		fn need_min_stake_balance(
			signer_account in account_id(),
			controller_account in account_id(),
			controller_balance in 0..MinStake::get(),
		) {
			prop_assume!(signer_account != controller_account);

			new_test_ext().execute_with(|| {
				Balances::make_free_balance_be(&controller_account, controller_balance);

				let root_account = get_root_account();
				prop_assert_noop!(
					Oracle::set_signer(RuntimeOrigin::signed(root_account), controller_account, signer_account),
					BalancesError::<Test>::InsufficientBalance
				);

				Ok(())
			})?;
		}

		#[test]
		fn cannot_use_same_signer_for_two_controllers(
			signer_account in account_id(),
			controller_1_account in account_id(),
			controller_1_balance in MinStake::get()..Balance::MAX,
			controller_2_account in account_id(),
			controller_2_balance in MinStake::get()..Balance::MAX,
		) {
			prop_assume!(signer_account != controller_1_account);
			prop_assume!(signer_account != controller_2_account);
			prop_assume!(controller_1_account != controller_2_account);

			new_test_ext().execute_with(|| {
				let root_account = get_root_account();
				Balances::make_free_balance_be(&controller_1_account, controller_1_balance);
				Balances::make_free_balance_be(&controller_2_account, controller_2_balance);

				prop_assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), controller_1_account, signer_account));

				assert_noop!(
					Oracle::set_signer(RuntimeOrigin::signed(root_account), controller_2_account, signer_account),
					Error::<Test>::SignerUsed
				);

				Ok(())
			})?;
		}

		#[test]
		fn cannot_use_same_controller_for_two_signers(
			signer_1_account in account_id(),
			signer_2_account in account_id(),
			controller_account in account_id(),
			controller_balance in (MinStake::get() * 2)..Balance::MAX,
		) {
			prop_assume!(signer_1_account != signer_2_account);
			prop_assume!(signer_1_account != controller_account);
			prop_assume!(signer_2_account != controller_account);

			new_test_ext().execute_with(|| {
			let root_account = get_root_account();
				Balances::make_free_balance_be(&controller_account, controller_balance);

				prop_assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), controller_account, signer_1_account));

				assert_noop!(
					Oracle::set_signer(RuntimeOrigin::signed(root_account), controller_account, signer_2_account),
					Error::<Test>::ControllerUsed
				);

				Ok(())
			})?;
		}


	}
}

mod add_stake {
	use super::*;
	proptest! {
		#![proptest_config(ProptestConfig::with_cases(10_000))]

		#[test]
		fn cannot_add_stake_without_signer_account(
			controller_account in account_id(),
			stake in 0..Balance::MAX,
		) {
			new_test_ext().execute_with(|| {
				prop_assert_noop!(Oracle::add_stake(RuntimeOrigin::signed(controller_account), stake), Error::<Test>::UnsetSigner);
				Ok(())
			})?;
		}

		#[test]
		fn can_add_balance_to_stake(
			controller_account in account_id(),
			signer_account in account_id(),
			controller_balance in (MinStake::get() + 1)..(Balance::MAX/2), // +1 so that the controller lives after setting signer
			signer_balance in 0..(Balance::MAX/2),
			stake in 0..(Balance::MAX/2),
		) {
			prop_assume!(controller_account != signer_account);

			// stake = stake.min
			new_test_ext().execute_with(|| {
				Balances::make_free_balance_be(&controller_account, controller_balance);
				Balances::make_free_balance_be(&signer_account, signer_balance);

				let root_account = get_root_account();
				prop_assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), controller_account, signer_account));

				let new_controller_balance = controller_balance - MinStake::get();

				// Check if the pre-add-stake balances are correct
				prop_assert_eq!(Balances::free_balance(&controller_account), new_controller_balance);
				prop_assert_eq!(Balances::free_balance(&signer_account), signer_balance);

				// Add the stake
				let stake_to_add = stake.min(new_controller_balance - 1); // -1 so that the controller lives after adding stake
				prop_assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(controller_account), stake_to_add));

				// Check if the post-add-stake balances are correct
				prop_assert_eq!(Balances::free_balance(controller_account), new_controller_balance - stake_to_add);
				prop_assert_eq!(Balances::total_balance(&controller_account), new_controller_balance - stake_to_add);

				// Check if the signer's stake is updated correctly
				let amount_staked = Oracle::oracle_stake(signer_account).unwrap_or_else(|| 0_u32.into());
				prop_assert_eq!(amount_staked, stake_to_add + MinStake::get());

				// Check if the stake is not accidentally added to the controller
				let controller_stake = Oracle::oracle_stake(controller_account).unwrap_or_else(|| 0_u32.into());
				prop_assert_eq!(controller_stake, 0);

				// Check if the signer's total balance includes the amount staked
				prop_assert_eq!(Balances::total_balance(&signer_account), signer_balance + amount_staked);

				Ok(())
			})?;
		}

		#[test]
		fn account_must_live_after_adding_stake(
			controller_account in account_id(),
			signer_account in account_id(),
			controller_balance in (MinStake::get() + 1)..(Balance::MAX/2), // +1 so that the controller lives after setting signer
			signer_balance in 0..(Balance::MAX/2),
		) {
			prop_assume!(controller_account != signer_account);

			new_test_ext().execute_with(|| {
				Balances::make_free_balance_be(&controller_account, controller_balance);
				Balances::make_free_balance_be(&signer_account, signer_balance);

				let root_account = get_root_account();
				prop_assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), controller_account, signer_account));

				let new_controller_balance = controller_balance - MinStake::get();

				// Check if the pre-add-stake balances are correct
				prop_assert_eq!(Balances::free_balance(&controller_account), new_controller_balance);
				prop_assert_eq!(Balances::free_balance(&signer_account), signer_balance);

				// Try to stake the entire controller balance
				prop_assert_noop!(
					Oracle::add_stake(RuntimeOrigin::signed(controller_account), new_controller_balance),
					BalancesError::<Test>::KeepAlive
				);

				Ok(())
			})?;
		}

		// TODO: test ExceedStake
		// TODO: check if stakes are isolated
	}
}

mod reclaim_stake {
	use super::*;
	proptest! {
		#![proptest_config(ProptestConfig::with_cases(10_000))]

		#[test]
		fn cannot_reclaim_stake_when_no_signer_set(
			controller_account in account_id(),
		) {
			new_test_ext().execute_with(|| {
				prop_assert_noop!(
					Oracle::reclaim_stake(RuntimeOrigin::signed(controller_account)),
					Error::<Test>::UnsetSigner
				);

				Ok(())
			})?;
		}

		#[test]
		fn cannot_reclaim_stake_when_no_declared_withdraws(
			controller_account in account_id(),
			controller_balance in MinStake::get()..Balance::MAX,
			signer_account in account_id(),
		) {
			prop_assume!(controller_account != signer_account);

			new_test_ext().execute_with(|| {
				let root_account = get_root_account();
				Balances::make_free_balance_be(&controller_account, controller_balance);
				prop_assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), controller_account, signer_account));

				prop_assert_noop!(
					Oracle::reclaim_stake(RuntimeOrigin::signed(controller_account)),
					Error::<Test>::Unknown
				);

				Ok(())
			})?;
		}

		#[test]
		fn cannot_remove_stake_when_there_is_none(
			controller_account in account_id(),
			controller_balance in (MinStake::get()+1)..Balance::MAX, // +1 to keep alive
			signer_account in account_id(),
			start_block in 0..(BlockNumber::MAX / 2),
		) {
			prop_assume!(controller_account != signer_account);

			new_test_ext().execute_with(|| {
				let root_account = get_root_account();
				Balances::make_free_balance_be(&controller_account, controller_balance);
				prop_assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), controller_account, signer_account));

				System::set_block_number(start_block);
				// Remove the stake from setting the signer
				prop_assert_ok!(Oracle::remove_stake(RuntimeOrigin::signed(controller_account)));

				// Can't remove anymore because we did not stake anything else
				prop_assert_noop!(
					Oracle::remove_stake(RuntimeOrigin::signed(controller_account)),
					Error::<Test>::NoStake
				);

				Ok(())
			})?;
		}

		#[test]
		fn can_reclaim_stake_after_removing_stake(
			controller_account in account_id(),
			controller_balance in (MinStake::get()+1)..(Balance::MAX/4), // +1 to keep alive
			signer_account in account_id(),
			signer_balance in 0..(Balance::MAX/4),
			stake_to_add in 0..(Balance::MAX/4),
			start_block in 0..(BlockNumber::MAX / 4),
			wait_after_unlock in 0..(BlockNumber::MAX / 4),
		) {
			prop_assume!(controller_account != signer_account);

			new_test_ext().execute_with(|| {
				Balances::make_free_balance_be(&controller_account, controller_balance);
				Balances::make_free_balance_be(&signer_account, signer_balance);
				let root_account = get_root_account();
				prop_assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), controller_account, signer_account));

				let actual_stake_to_add = stake_to_add.min(controller_balance - MinStake::get() - 1);

				prop_assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(controller_account), actual_stake_to_add));

				// Assert that the stake is added
				prop_assert_eq!(
					Oracle::oracle_stake(signer_account),
					Some(actual_stake_to_add + MinStake::get())
				);

				// Remove the stake
				System::set_block_number(start_block);
				prop_assert_ok!(Oracle::remove_stake(RuntimeOrigin::signed(controller_account)));

				// Check if the withdrawal is correctly declared
				let withdrawal = Withdraw { stake: actual_stake_to_add + MinStake::get(), unlock_block: start_block + StakeLock::get() };
				prop_assert_eq!(Oracle::declared_withdraws(signer_account), Some(withdrawal.clone()));

				// ... and that the stake is removed
				prop_assert_eq!(Oracle::oracle_stake(signer_account), None);

				prop_assert_noop!(
					Oracle::remove_stake(RuntimeOrigin::signed(controller_account)),
					Error::<Test>::NoStake
				);

				// Check that stake cannot be claimed too early
				prop_assert_noop!(
					Oracle::reclaim_stake(RuntimeOrigin::signed(controller_account)),
					Error::<Test>::StakeLocked
				);

				System::set_block_number(withdrawal.unlock_block + wait_after_unlock);

				prop_assert_ok!(Oracle::reclaim_stake(RuntimeOrigin::signed(controller_account)));

				// Check if the controller's balance is correct
				prop_assert_eq!(Balances::free_balance(&controller_account), controller_balance);
				prop_assert_eq!(Balances::free_balance(&signer_account), signer_balance);

				// After reclaiming the stake, the controller <-> signer relationship is removed
				prop_assert_eq!(Oracle::controller_to_signer(controller_account), None);
				prop_assert_eq!(Oracle::signer_to_controller(signer_account), None);

				assert_noop!(Oracle::reclaim_stake(RuntimeOrigin::signed(controller_account)), Error::<Test>::UnsetSigner);
				assert_noop!(Oracle::reclaim_stake(RuntimeOrigin::signed(signer_account)), Error::<Test>::UnsetSigner);


				Ok(())
			})?;
		}
	}
}

#[test]
fn calculate_reward_per_block() {
	new_test_ext().execute_with(|| {
		let root = get_root_account();
		System::set_block_number(1);
		Timestamp::set_timestamp(1);
		Oracle::on_initialize(1);
		// 3 oracles
		let account_1_controller = get_account_1();
		let account_1_signer = get_account_3();
		Balances::make_free_balance_be(&account_1_controller, 1000);
		Balances::make_free_balance_be(&account_1_signer, 0);
		let account_2_controller = get_account_4();
		let account_2_signer = get_account_5();
		Balances::make_free_balance_be(&account_2_controller, 1000);
		Balances::make_free_balance_be(&account_2_signer, 0);
		let account_3_controller = get_account_6();
		let account_3_signer = get_account_7();
		Balances::make_free_balance_be(&account_3_controller, 1000);
		Balances::make_free_balance_be(&account_3_signer, 0);
		let treasury_account = get_treasury_account();
		Balances::make_free_balance_be(&treasury_account, 10000);
		let rewards_account = Oracle::account_id();
		Balances::make_free_balance_be(&rewards_account, 10000);

		// 2 assets
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(root),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(1).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			10,
			5,
			false,
		));

		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(root),
			1,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(1).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			30,
			5,
			false,
		));

		let root_account = get_root_account();
		// adding stake
		assert_ok!(Oracle::set_signer(
			RuntimeOrigin::signed(root_account),
			account_1_controller,
			account_1_signer
		));
		assert_ok!(Oracle::set_signer(
			RuntimeOrigin::signed(root_account),
			account_2_controller,
			account_2_signer
		));
		assert_ok!(Oracle::set_signer(
			RuntimeOrigin::signed(root_account),
			account_3_controller,
			account_3_signer
		));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_1_controller), 99));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_2_controller), 199));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_3_controller), 299));
		assert_eq!(Oracle::oracle_stake(account_1_signer), Some(100));
		assert_eq!(Oracle::oracle_stake(account_2_signer), Some(200));
		assert_eq!(Oracle::oracle_stake(account_3_signer), Some(300));

		// reward tracker
		let annual_cost_per_oracle: Balance = 262800000;
		let num_ideal_oracles: u8 = 1;
		assert_ok!(Oracle::adjust_rewards(
			RuntimeOrigin::root(),
			annual_cost_per_oracle,
			num_ideal_oracles
		));
		assert_eq!(
			RewardTrackerStore::<Test>::get(),
			Some(RewardTracker {
				period: MS_PER_YEAR_NAIVE,
				start: 1,
				total_already_rewarded: 0,
				current_block_reward: 100,
				total_reward_weight: 40,
			})
		);
		assert_eq!(AccumulatedRewardsPerAsset::<Test>::get(0_u128), None);
		assert_eq!(AccumulatedRewardsPerAsset::<Test>::get(1_u128), None);

		// add prices for asset 1 from 1 user
		System::set_block_number(6);
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_1_signer), 100_u128, 0_u128));
		let price = PrePrice { price: 100_u128, block: 6, who: account_1_signer };
		assert_eq!(Oracle::pre_prices(0), vec![price]);
		System::set_block_number(7);
		Timestamp::set_timestamp(7);
		assert_eq!(Balances::free_balance(account_1_controller), 900);
		assert_eq!(Balances::free_balance(rewards_account), 10000);
		Oracle::on_initialize(7);
		assert_eq!(Balances::free_balance(account_1_controller), 925);
		assert_eq!(Balances::free_balance(rewards_account), 9975);
		assert_eq!(AccumulatedRewardsPerAsset::<Test>::get(0_u128), Some(Zero::zero()));
		assert_eq!(AccumulatedRewardsPerAsset::<Test>::get(1_u128), Some(75_u128));

		// 2 users for another asset (it has rewards accumulated for 2 block -- 2 on_initialize)
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_2_signer), 200_u128, 1_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_3_signer), 200_u128, 1_u128));
		System::set_block_number(8);
		Timestamp::set_timestamp(8);
		assert_eq!(Balances::free_balance(account_2_controller), 800);
		assert_eq!(Balances::free_balance(account_3_controller), 700);
		assert_eq!(Balances::free_balance(rewards_account), 9975);
		Oracle::on_initialize(8);
		assert_eq!(Balances::free_balance(account_2_controller), 860);
		assert_eq!(Balances::free_balance(account_3_controller), 790);
		assert_eq!(Balances::free_balance(rewards_account), 9825);
		assert_eq!(AccumulatedRewardsPerAsset::<Test>::get(0_u128), Some(25_u128));
		assert_eq!(AccumulatedRewardsPerAsset::<Test>::get(1_u128), Some(Zero::zero()));

		// 3 users for both assets
		System::set_block_number(100);
		Timestamp::set_timestamp(100);
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_1_signer), 100_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_2_signer), 100_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_2_signer), 200_u128, 1_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_3_signer), 200_u128, 1_u128));
		System::set_block_number(101);
		Timestamp::set_timestamp(101);
		Oracle::on_initialize(101);
		assert_eq!(Balances::free_balance(account_1_controller), 941);
		assert_eq!(Balances::free_balance(account_2_controller), 923);
		assert_eq!(Balances::free_balance(account_3_controller), 835);
		assert_eq!(Balances::free_balance(rewards_account), 9701);
		assert_eq!(AccumulatedRewardsPerAsset::<Test>::get(0_u128), Some(Zero::zero()));
		assert_eq!(AccumulatedRewardsPerAsset::<Test>::get(1_u128), Some(Zero::zero()));

		// all 3 user for all 2 assets
		System::set_block_number(107);
		Timestamp::set_timestamp(107);
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_1_signer), 100_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_2_signer), 100_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_3_signer), 100_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_1_signer), 200_u128, 1_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_2_signer), 200_u128, 1_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_3_signer), 200_u128, 1_u128));
		System::set_block_number(108);
		Timestamp::set_timestamp(108);
		Oracle::on_initialize(108);
		assert_eq!(Balances::free_balance(account_1_controller), 957);
		assert_eq!(Balances::free_balance(account_2_controller), 956);
		assert_eq!(Balances::free_balance(account_3_controller), 884);
		assert_eq!(Balances::free_balance(rewards_account), 9603);
	});
}

#[test]
fn calculate_reward_per_block_min_aswers_eq_3() {
	new_test_ext().execute_with(|| {
		let root = get_root_account();
		System::set_block_number(1);
		Timestamp::set_timestamp(1);
		Oracle::on_initialize(1);
		// 3 oracles
		let account_1_controller = get_account_1();
		let account_1_signer = get_account_3();
		Balances::make_free_balance_be(&account_1_controller, 1000);
		Balances::make_free_balance_be(&account_1_signer, 0);
		let account_2_controller = get_account_4();
		let account_2_signer = get_account_5();
		Balances::make_free_balance_be(&account_2_controller, 1000);
		Balances::make_free_balance_be(&account_2_signer, 0);
		let account_3_controller = get_account_6();
		let account_3_signer = get_account_7();
		Balances::make_free_balance_be(&account_3_controller, 1000);
		Balances::make_free_balance_be(&account_3_signer, 0);
		let treasury_account = get_treasury_account();
		Balances::make_free_balance_be(&treasury_account, 10000);
		let rewards_account = Oracle::account_id();
		Balances::make_free_balance_be(&rewards_account, 10000);

		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(root),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			30,
			5,
			false,
		));

		// adding stake
		let root_account = get_root_account();
		assert_ok!(Oracle::set_signer(
			RuntimeOrigin::signed(root_account),
			account_1_controller,
			account_1_signer
		));
		assert_ok!(Oracle::set_signer(
			RuntimeOrigin::signed(root_account),
			account_2_controller,
			account_2_signer
		));
		assert_ok!(Oracle::set_signer(
			RuntimeOrigin::signed(root_account),
			account_3_controller,
			account_3_signer
		));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_1_controller), 99));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_2_controller), 199));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_3_controller), 299));
		assert_eq!(Oracle::oracle_stake(account_1_signer), Some(100));
		assert_eq!(Oracle::oracle_stake(account_2_signer), Some(200));
		assert_eq!(Oracle::oracle_stake(account_3_signer), Some(300));

		// reward tracker
		let annual_cost_per_oracle: Balance = 262800000;
		let num_ideal_oracles: u8 = 1;
		assert_ok!(Oracle::adjust_rewards(
			RuntimeOrigin::root(),
			annual_cost_per_oracle,
			num_ideal_oracles
		));
		assert_eq!(
			RewardTrackerStore::<Test>::get(),
			Some(RewardTracker {
				period: MS_PER_YEAR_NAIVE,
				start: 1,
				total_already_rewarded: 0,
				current_block_reward: 100,
				total_reward_weight: 30,
			})
		);

		System::set_block_number(6);
		Timestamp::set_timestamp(6);
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_1_signer), 200_u128, 0_u128));
		System::set_block_number(7);
		Timestamp::set_timestamp(7);
		Oracle::on_initialize(7);
		assert_eq!(Balances::free_balance(account_1_controller), 900);
		assert_eq!(Balances::free_balance(account_2_controller), 800);
		assert_eq!(Balances::free_balance(account_3_controller), 700);
		assert_eq!(Balances::free_balance(rewards_account), 10000);
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_2_signer), 200_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_3_signer), 200_u128, 0_u128));
		System::set_block_number(8);
		Timestamp::set_timestamp(8);
		Oracle::on_initialize(8);
		assert_eq!(Balances::free_balance(account_1_controller), 933);
		assert_eq!(Balances::free_balance(account_2_controller), 866);
		assert_eq!(Balances::free_balance(account_3_controller), 800);
		assert_eq!(Balances::free_balance(rewards_account), 9801);
	});
}

mod submit_price {
	use super::*;
	proptest! {
		#![proptest_config(ProptestConfig::with_cases(10_000))]

		#[test]
		fn cannot_submit_prices_when_not_requested(
			account_id in account_id(),
			asset_id in asset_id(),
			price_value in price_value(),
		) {
			new_test_ext().execute_with(|| {
				prop_assert_noop!(
					Oracle::submit_price(RuntimeOrigin::signed(account_id), asset_id, price_value),
					Error::<Test>::PriceNotRequested
				);
				Ok(())
			})?;
		}

		#[test]
		fn cannot_submit_price_when_stake_too_low(
			submitter_account in account_id(),
			asset_id in asset_id(),
			asset_info in asset_info(),
			price_value in price_value(),
			start_block in 0..(BlockNumber::MAX/8),
		) {
			new_test_ext().execute_with(|| {
				let root_account = get_root_account();

				System::set_block_number(start_block);

				prop_assert_ok!(Oracle::add_asset_and_info(
					RuntimeOrigin::signed(root_account),
					asset_id,
					Validated::new(asset_info.threshold).unwrap(),
					Validated::new(asset_info.min_answers).unwrap(),
					Validated::new(asset_info.max_answers).unwrap(),
					Validated::new(asset_info.block_interval).unwrap(),
					asset_info.reward_weight,
					asset_info.slash,
					asset_info.emit_price_changes,
				));

				let last_update = Oracle::prices(asset_id).block;

				System::set_block_number(last_update + asset_info.block_interval + 1);

				prop_assert_noop!(
					Oracle::submit_price(RuntimeOrigin::signed(submitter_account), price_value, asset_id),
					Error::<Test>::NotEnoughStake
				);

				Ok(())
			})?;
		}

	}

	fn submit_price_fails_stake_less_than_asset_slash() {
		new_test_ext().execute_with(|| {
			let account_1 = get_account_1();
			let account_2 = get_root_account();

			assert_ok!(Oracle::add_asset_and_info(
				RuntimeOrigin::signed(account_2),
				0,
				Validated::new(Percent::from_percent(80)).unwrap(),
				Validated::new(3).unwrap(),
				Validated::new(3).unwrap(),
				Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
				5,
				200,
				false,
			));

			System::set_block_number(6);
			let root_account = get_root_account();
			assert_ok!(Oracle::set_signer(
				RuntimeOrigin::signed(root_account),
				account_2,
				account_1
			));
			assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_2), 50));
			// fails as asset's slash is high compare to current stake of account_1
			assert_noop!(
				Oracle::submit_price(RuntimeOrigin::signed(account_1), 100_u128, 0_u128),
				Error::<Test>::NotEnoughStake
			);
		});
	}
}

#[test]
fn add_price() {
	new_test_ext().execute_with(|| {
		let account_1 = get_account_1();
		let account_2 = get_root_account();
		let account_4 = get_account_4();
		let account_5 = get_account_5();

		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(3).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			false,
		));

		System::set_block_number(6);
		// fails no stake
		assert_noop!(
			Oracle::submit_price(RuntimeOrigin::signed(account_1), 100_u128, 0_u128),
			Error::<Test>::NotEnoughStake
		);

		let root_account = get_root_account();
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_2, account_1));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_1, account_2));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_5, account_4));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_4, account_5));

		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_1), 50));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_2), 50));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_4), 50));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_5), 50));

		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_1), 100_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_2), 100_u128, 0_u128));
		assert_noop!(
			Oracle::submit_price(RuntimeOrigin::signed(account_2), 100_u128, 0_u128),
			Error::<Test>::AlreadySubmitted
		);
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_4), 100_u128, 0_u128));

		assert_eq!(Oracle::answer_in_transit(account_1), Some(5));
		assert_eq!(Oracle::answer_in_transit(account_2), Some(5));
		assert_eq!(Oracle::answer_in_transit(account_4), Some(5));

		assert_noop!(
			Oracle::submit_price(RuntimeOrigin::signed(account_5), 100_u128, 0_u128),
			Error::<Test>::MaxPrices
		);

		let price = PrePrice { price: 100_u128, block: 6, who: account_1 };

		let price2 = PrePrice { price: 100_u128, block: 6, who: account_2 };

		let price4 = PrePrice { price: 100_u128, block: 6, who: account_4 };

		assert_eq!(Oracle::pre_prices(0), vec![price, price2, price4]);
		System::set_block_number(2);
		Oracle::on_initialize(2);

		// fails price not requested
		assert_noop!(
			Oracle::submit_price(RuntimeOrigin::signed(account_1), 100_u128, 0_u128),
			Error::<Test>::PriceNotRequested
		);

		// non existent asset_id
		assert_noop!(
			Oracle::submit_price(RuntimeOrigin::signed(account_1), 100_u128, 10_u128),
			Error::<Test>::PriceNotRequested
		);
	});
}

#[test]
fn submit_price_fails_stake_less_than_asset_slash() {
	new_test_ext().execute_with(|| {
		let account_1 = get_account_1();
		let account_2 = get_root_account();

		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(3).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			200,
			false,
		));

		System::set_block_number(6);
		let root_account = get_root_account();
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_2, account_1));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_2), 50));
		// fails as asset's slash is high compare to current stake of account_1
		assert_noop!(
			Oracle::submit_price(RuntimeOrigin::signed(account_1), 100_u128, 0_u128),
			Error::<Test>::NotEnoughStake
		);
	});
}

fn halborn_test_price_manipulation() {
	new_test_ext().execute_with(|| {
		const ASSET_ID: u128 = 0;
		const MIN_ANSWERS: u32 = 3;
		const MAX_ANSWERS: u32 = 5;
		const THRESHOLD: Percent = Percent::from_percent(80);
		const BLOCK_INTERVAL: u64 = 5;
		const REWARD: u128 = 5;
		const SLASH: u128 = 5;
		const EMIT_PRICE_CHANGES: bool = false;

		let root_account = get_root_account();
		let account_1 = get_account_1();
		let account_3 = get_account_3();
		let account_4 = get_account_4();
		let account_5 = get_account_5();

		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(root_account),
			ASSET_ID,
			Validated::new(THRESHOLD).unwrap(),
			Validated::new(MIN_ANSWERS).unwrap(),
			Validated::new(MAX_ANSWERS).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(BLOCK_INTERVAL).unwrap(),
			REWARD,
			SLASH,
			EMIT_PRICE_CHANGES,
		));
		System::set_block_number(6);
		let root_account = get_root_account();
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_3, account_1));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_1, account_3));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_4, account_5));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_5, account_4));

		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_1), 50));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_3), 50));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_4), 50));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_5), 50));

		// Scenario 1: >50% of Oracles are malicious
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_1), 100_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_3), 690_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_4), 900_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_5), 900_u128, 0_u128));
		System::set_block_number(7);
		Oracle::on_initialize(7);
		System::set_block_number(13);
		// Scenario 2: 50% of Oracles are malicious
		// These prices prices will not be consider
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_1), 100_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_3), 100_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_4), 900_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_5), 900_u128, 0_u128));
		System::set_block_number(14);
		Oracle::on_initialize(14);
	});
}

#[test]
fn medianize_price() {
	new_test_ext().execute_with(|| {
		let account_1 = get_account_1();
		// should not panic
		Oracle::get_median_price(&Oracle::pre_prices(0));
		for i in 0..3 {
			let price = i as u128 + 100_u128;
			add_price_storage(price, 0, account_1, 0);
		}
		let price = Oracle::get_median_price(&Oracle::pre_prices(0));
		assert_eq!(price, Some(101));
	});
}

#[test]
#[should_panic = "No `keystore` associated for the current context!"]
fn check_request() {
	new_test_ext().execute_with(|| {
		let account_2 = get_root_account();
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			false,
		));
		System::set_block_number(6);
		Oracle::check_requests();
	});
}

#[test]
fn not_check_request() {
	new_test_ext().execute_with(|| {
		Oracle::check_requests();
	});
}

#[test]
fn is_requested() {
	new_test_ext().execute_with(|| {
		let account_2 = get_root_account();
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			false,
		));
		System::set_block_number(6);
		assert!(Oracle::is_requested(&0));

		let price = Price { price: 0, block: 6 };
		Prices::<Test>::insert(0, price);

		assert!(!Oracle::is_requested(&0));

		System::set_block_number(11);
		assert!(!Oracle::is_requested(&0));
	});
}

#[test]
fn test_payout_slash() {
	new_test_ext().execute_with(|| {
		let account_1 = get_account_1();
		let account_2 = get_root_account();
		let account_3 = get_account_3();
		let account_4 = get_account_4();
		let account_5 = get_account_5();
		let treasury_account = get_treasury_account();
		let mut reward_tracker = RewardTracker::default();
		reward_tracker.start = 1;
		reward_tracker.current_block_reward = 100;
		reward_tracker.total_reward_weight = 82;
		RewardTrackerStore::<Test>::set(Option::from(reward_tracker));
		Balances::make_free_balance_be(&Oracle::account_id(), 100);
		let root_account = get_root_account();
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_5, account_2));

		let one = PrePrice { price: 79, block: 0, who: account_1 };
		let two = PrePrice { price: 100, block: 0, who: account_2 };
		let three = PrePrice { price: 151, block: 0, who: account_3 };
		let four = PrePrice { price: 400, block: 0, who: account_4 };

		let five = PrePrice { price: 100, block: 0, who: account_5 };

		let asset_info = AssetInfo {
			threshold: Percent::from_percent(0),
			min_answers: 0,
			max_answers: 0,
			block_interval: 0,
			reward_weight: 0,
			slash: 0,
			emit_price_changes: false,
		};
		// doesn't panic when percent not set
		// need to set AccumulatedRewardsPerAsset because previously rewards were not accumlated but
		// but current_block_reward was distributed on every handle_payout call
		// here checking for initial value of AccumulatedRewarsPerAsset which is None
		assert_ok!(Oracle::handle_payout(&vec![one, two, three, four, five], 100, 0, &asset_info));
		assert_eq!(Balances::free_balance(account_1), 100);
		assert_eq!(Balances::free_balance(Oracle::account_id()), 100);

		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			18,
			5,
			false,
		));
		let reward_tracker = RewardTrackerStore::<Test>::get().unwrap();
		assert_eq!(reward_tracker.total_reward_weight, 100);

		add_price_storage(79, 0, account_1, 0);
		add_price_storage(100, 0, account_2, 0);
		let root_account = get_root_account();
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_2, account_1));
		assert_eq!(Oracle::oracle_stake(account_1), Some(1));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_1, account_4));
		assert_eq!(Oracle::oracle_stake(account_4), Some(1));

		assert_eq!(Oracle::answer_in_transit(account_1), Some(5));
		assert_eq!(Oracle::answer_in_transit(account_2), Some(5));
		assert_eq!(Balances::free_balance(treasury_account), 100);

		// reward per block is 100, weight of an asset is 18
		AccumulatedRewardsPerAsset::<Test>::mutate(0, |balance| *balance = Some(18_u128));
		assert_ok!(Oracle::handle_payout(
			&vec![one, two, three, four, five],
			100,
			0,
			&Oracle::asset_info(0).unwrap(),
		));

		assert_eq!(Oracle::answer_in_transit(account_1), Some(0));
		assert_eq!(Oracle::answer_in_transit(account_2), Some(0));
		// account 1 and 4 gets slashed 2 and 5 gets rewarded
		assert_eq!(Oracle::oracle_stake(account_1), Some(0));
		// 5 gets 2's reward and its own
		let reward_tracker = RewardTrackerStore::<Test>::get().unwrap();
		assert_eq!(reward_tracker.total_already_rewarded, 18);
		assert_eq!(Balances::free_balance(account_5), 117);
		assert_eq!(Balances::free_balance(account_2), 99);

		assert_eq!(Balances::free_balance(account_3), 100);
		assert_eq!(Balances::free_balance(account_4), 100);
		assert_eq!(Oracle::oracle_stake(account_4), Some(0));
		// treasury gets 1 from both account1 and account4's stake
		assert_eq!(Balances::free_balance(treasury_account), 102);
		// 18/100 of the reward goes to the oracle accounts as rewards
		assert_eq!(Balances::free_balance(Oracle::account_id()), 82);

		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(90)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			18,
			5,
			false,
		));
		let reward_tracker = RewardTrackerStore::<Test>::get().unwrap();
		assert_eq!(reward_tracker.total_reward_weight, 100);
		// reward per block is 100, weight of an asset is 18
		AccumulatedRewardsPerAsset::<Test>::mutate(0, |balance| *balance = Some(18_u128));
		assert_ok!(Oracle::handle_payout(
			&vec![one, two, three, four, five],
			100,
			0,
			&Oracle::asset_info(0).unwrap(),
		));

		// account 4 gets slashed 2 5 and 1 gets rewarded
		assert_eq!(Balances::free_balance(account_1), 99);
		// 5 gets 2's reward and its own
		assert_eq!(Balances::free_balance(account_5), 135);
		assert_eq!(Balances::free_balance(account_2), 99);

		assert_eq!(Balances::free_balance(account_3), 100);
		assert_eq!(Oracle::oracle_stake(account_4), Some(0));
		assert_eq!(Balances::free_balance(treasury_account), 102);
		assert_eq!(Balances::free_balance(account_4), 100);
		// 36/100 of the reward goes to the oracle accounts as rewards
		assert_eq!(Balances::free_balance(Oracle::account_id()), 64);
	});
}

#[test]
fn test_reset_reward_tracker_if_expired() {
	new_test_ext().execute_with(|| {
		let mut reward_tracker = RewardTracker::default();
		reward_tracker.period = 1000;
		reward_tracker.total_already_rewarded = 100000;
		reward_tracker.start = 1;
		reward_tracker.current_block_reward = 100;
		reward_tracker.total_reward_weight = 50000;
		RewardTrackerStore::<Test>::set(Option::from(reward_tracker));
		Timestamp::set_timestamp(1001);
		Oracle::reset_reward_tracker_if_expired();

		let reward_tracker = RewardTrackerStore::<Test>::get().unwrap();
		assert_eq!(reward_tracker.period, 1000);
		assert_eq!(reward_tracker.total_already_rewarded, 0);
		assert_eq!(reward_tracker.start, 1001);
		assert_eq!(reward_tracker.current_block_reward, 100);
		assert_eq!(reward_tracker.total_reward_weight, 50000);
	});
}

#[test]
fn test_adjust_rewards() {
	new_test_ext().execute_with(|| {
		let annual_cost_per_oracle: Balance = 100_000 * UNIT;
		let mut num_ideal_oracles: u8 = 10;
		const BLOCKS_PER_YEAR: u64 = MS_PER_YEAR_NAIVE / MILLISECS_PER_BLOCK;
		Timestamp::set_timestamp(1);

		// first time
		assert_ok!(Oracle::adjust_rewards(
			RuntimeOrigin::root(),
			annual_cost_per_oracle,
			num_ideal_oracles
		));
		assert_reward_tracker(
			annual_cost_per_oracle,
			num_ideal_oracles,
			BLOCKS_PER_YEAR as Balance,
		);

		// second time after quarter of the year has passed. Increase the ideal number of Oracles.
		// Rewards have not been issued yet so the .
		Timestamp::set_timestamp(MS_PER_YEAR_NAIVE / 4);
		num_ideal_oracles = 12;
		assert_ok!(Oracle::adjust_rewards(
			RuntimeOrigin::root(),
			annual_cost_per_oracle,
			num_ideal_oracles
		));
		let remaining_blocks_per_year = (BLOCKS_PER_YEAR * 3 / 4) as Balance;
		assert_reward_tracker(annual_cost_per_oracle, num_ideal_oracles, remaining_blocks_per_year);

		// third time after half of the year has passed. Decrease the ideal number of Oracles.
		// set the total already rewarded to be half of the annual reward.
		Timestamp::set_timestamp(MS_PER_YEAR_NAIVE / 2);
		num_ideal_oracles = 10;
		let mut reward_tracker = Oracle::reward_tracker_store().unwrap();
		reward_tracker.total_already_rewarded =
			annual_cost_per_oracle * (num_ideal_oracles as Balance) / 2;
		RewardTrackerStore::<Test>::set(Option::from(reward_tracker));
		assert_ok!(Oracle::adjust_rewards(
			RuntimeOrigin::root(),
			annual_cost_per_oracle,
			num_ideal_oracles
		));
		let remaining_blocks_per_year = (BLOCKS_PER_YEAR / 2) as Balance;
		// dividing the annual cost per oracle by 2 because the total already rewarded is half of
		// the annual cost per oracle.
		assert_reward_tracker(
			annual_cost_per_oracle / 2,
			num_ideal_oracles,
			remaining_blocks_per_year,
		);

		// fourth time after a year and a half has passed. Increase the ideal number of Oracles.
		// set the total already rewarded to be half the annual cost per oracle.
		Timestamp::set_timestamp(MS_PER_YEAR_NAIVE * 3 / 2);
		num_ideal_oracles = 20;
		let mut reward_tracker = Oracle::reward_tracker_store().unwrap();
		reward_tracker.total_already_rewarded =
			annual_cost_per_oracle * (num_ideal_oracles as Balance) / 2;
		RewardTrackerStore::<Test>::set(Option::from(reward_tracker));
		assert_ok!(Oracle::adjust_rewards(
			RuntimeOrigin::root(),
			annual_cost_per_oracle,
			num_ideal_oracles
		));
		let remaining_blocks_per_year = (BLOCKS_PER_YEAR / 2) as Balance;
		// dividing the annual cost per oracle by 2 because the total already rewarded is half of
		// the annual cost per oracle.
		assert_reward_tracker(
			annual_cost_per_oracle / 2,
			num_ideal_oracles,
			remaining_blocks_per_year,
		);
	});
}

fn assert_reward_tracker(
	annual_cost_per_oracle: Balance,
	num_ideal_oracles: u8,
	remaining_blocks_per_year: Balance,
) {
	let reward_tracker = Oracle::reward_tracker_store().unwrap();
	assert_eq!(
		reward_tracker.current_block_reward,
		annual_cost_per_oracle * (num_ideal_oracles as Balance) / remaining_blocks_per_year
	);
}

#[test]
fn halborn_test_bypass_slashing() {
	new_test_ext().execute_with(|| {
		const ASSET_ID: u128 = 0;
		const MIN_ANSWERS: u32 = 3;
		const MAX_ANSWERS: u32 = 5;
		const THRESHOLD: Percent = Percent::from_percent(80);
		const BLOCK_INTERVAL: u64 = 5;
		const REWARD: u128 = 5;
		const SLASH: u128 = 5;
		const EMIT_PRICE_CHANGES: bool = false;
		Timestamp::set_timestamp(10);
		let mut reward_tracker = RewardTracker::default();
		reward_tracker.start = 1;
		reward_tracker.current_block_reward = 100;
		reward_tracker.total_reward_weight = 100;
		RewardTrackerStore::<Test>::set(Option::from(reward_tracker));
		Balances::make_free_balance_be(&Oracle::account_id(), 100);

		//assert_ok!(Oracle::set_reward_rate(RuntimeOrigin::root(), REWARD_RATE));
		let account_1 = get_account_1();
		let account_2 = get_root_account();
		let account_4 = get_account_4();
		let account_5 = get_account_5();
		let treasury_account = get_treasury_account();

		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			ASSET_ID,
			Validated::new(THRESHOLD).unwrap(),
			Validated::new(MIN_ANSWERS).unwrap(),
			Validated::new(MAX_ANSWERS).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(BLOCK_INTERVAL).unwrap(),
			REWARD,
			SLASH,
			EMIT_PRICE_CHANGES,
		));

		let balance1 = Balances::free_balance(account_1);
		let balance2 = Balances::free_balance(account_2);
		let balance4 = Balances::free_balance(account_4);
		let balance5 = Balances::free_balance(account_5);

		println!("BALANCE before staking");
		println!("1: {}", balance1);
		println!("2: {}", balance2);
		println!("4: {}", balance4);
		println!("5: {}", balance5);

		System::set_block_number(6);
		let root_account = get_root_account();
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_2, account_1));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_1, account_2));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_5, account_4));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_4, account_5));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_1), 50));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_2), 50));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_4), 99));
		assert_ok!(Oracle::add_stake(RuntimeOrigin::signed(account_5), 50));

		let balance1 = Balances::free_balance(account_1);
		let balance2 = Balances::free_balance(account_2);
		let balance4 = Balances::free_balance(account_4);
		let balance5 = Balances::free_balance(account_5);
		println!("BALANCE before price submissions");
		println!("1: {}", balance1);
		println!("2: {}", balance2);
		println!("4: {}", balance4);
		println!("5: {}", balance5);

		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_1), 100_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_2), 100_u128, 0_u128));
		// Proposing price of 4000 would result in getting stake slashed of controller account_5
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_4), 4000_u128, 0_u128));

		System::set_block_number(7);
		Oracle::on_initialize(7);
		let res = <Oracle as oracle::Oracle>::get_price(0, 1).unwrap();
		println!("AFTER 1st SUBMIT PRICE - PRICE: {:?} | BLOCK: {:?}", res.price, res.block);
		let balance1 = Balances::free_balance(account_1);
		let balance2 = Balances::free_balance(account_2);
		let balance4 = Balances::free_balance(account_4);
		let balance5 = Balances::free_balance(account_5);
		println!("BALANCE after price submissions");
		println!("1: {}", balance1);
		println!("2: {}", balance2);
		println!("4: {}", balance4);
		println!("5: {}", balance5);

		assert_ok!(Oracle::remove_stake(RuntimeOrigin::signed(account_5)));
		System::set_block_number(44);
		assert_ok!(Oracle::reclaim_stake(RuntimeOrigin::signed(account_5)));
		let balance1 = Balances::free_balance(account_1);
		let balance2 = Balances::free_balance(account_2);
		let balance4 = Balances::free_balance(account_4);
		let balance5 = Balances::free_balance(account_5);
		let balance_treasury = Balances::free_balance(treasury_account);
		println!("BALANCE after account_4 removed stake:");
		println!("1: {}", balance1);
		println!("2: {}", balance2);
		println!("4: {}", balance4);
		println!("5: {}", balance5);
		println!("TreasuryAccount Balance: {}", balance_treasury);
		// account4 (signer) with controller account5 has reported skewed price.
		// So account5 's stake is slashed and slashed amount is transferred to treasury_account
		assert_eq!(balance5, 95_u128);
		assert_eq!(balance_treasury, 105_u128);
		assert_eq!(Balances::free_balance(account_1), 51);
		assert_eq!(Balances::free_balance(account_4), 0);
	});
}

#[test]
fn on_init() {
	new_test_ext().execute_with(|| {
		// no price fetch
		Oracle::on_initialize(1);
		let price = Price { price: 0, block: 0 };

		assert_eq!(Oracle::prices(0), price);

		// add and request oracle id
		let account_2 = get_root_account();
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			false,
		));
		// set prices into storage
		let account_1 = get_account_1();
		for i in 0..3 {
			let price = i as u128 + 100_u128;
			add_price_storage(price, 0, account_1, 2);
		}

		Oracle::on_initialize(2);
		let price = Price { price: 101, block: 2 };

		assert_eq!(Oracle::prices(0), price);
		// prunes state
		assert_eq!(Oracle::pre_prices(0), vec![]);

		// doesn't prune state if under min prices
		for i in 0..2 {
			let price = i as u128 + 100_u128;
			add_price_storage(price, 0, account_1, 3);
		}

		// does not fire under min answers
		Oracle::on_initialize(3);
		assert_eq!(Oracle::pre_prices(0).len(), 2);
		assert_eq!(Oracle::prices(0), price);
	});
}

#[test]
fn update_price() {
	new_test_ext().execute_with(|| {
		let account_2 = get_root_account();
		let account_1 = get_account_1();

		// Add KSM info.
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			4, // KSM
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			false, // do not emit PriceChange event
		));

		// Update price for KSM.
		do_price_update(4, 2);

		// `PriceChanged` Event should NOT be emitted.
		Test::assert_no_event(RuntimeEvent::Oracle(crate::Event::PriceChanged(0, 101)));

		// Add PICA info.
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			1, // PICA
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			true, // emit PriceChange event
		));

		// Update price for PICA.
		do_price_update(1, 3);

		// `PriceChanged` Event should be emitted.
		System::assert_has_event(RuntimeEvent::Oracle(crate::Event::PriceChanged(1, 101)));

		// Set series of EQUAL prices for PICA into storage.
		//				 -----
		for _ in 0..3 {
			let price = 100_u128;
			add_price_storage(price, 1, account_1, 2);
		}

		// Process next block
		process_and_progress_blocks::<Oracle, Test>(1);

		// `PriceChanged` event for last price (100) should NOT be emitted, as prices didn't
		// change
		Test::assert_no_event(RuntimeEvent::Oracle(crate::Event::PriceChanged(1, 100)));
	});
}

#[test]
fn historic_pricing() {
	new_test_ext().execute_with(|| {
		// add and request oracle id
		let account_2 = get_root_account();
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			false,
		));

		let mut price_history = vec![];

		do_price_update(0, 0);

		assert_eq!(Oracle::price_history(0).len(), 0);
		assert_eq!(Oracle::price_history(0), price_history);

		do_price_update(0, 5);

		let price_5 = Price { price: 101, block: 5 };
		price_history = vec![price_5.clone()];

		assert_eq!(Oracle::price_history(0), price_history);
		assert_eq!(Oracle::price_history(0).len(), 1);

		do_price_update(0, 10);
		let price_10 = Price { price: 101, block: 10 };
		price_history = vec![price_5.clone(), price_10.clone()];

		assert_eq!(Oracle::price_history(0), price_history);
		assert_eq!(Oracle::price_history(0).len(), 2);

		do_price_update(0, 15);
		let price_15 = Price { price: 101, block: 15 };
		price_history = vec![price_5, price_10.clone(), price_15.clone()];

		assert_eq!(Oracle::price_history(0), price_history);
		assert_eq!(Oracle::price_history(0).len(), 3);

		do_price_update(0, 20);
		let price_20 = Price { price: 101, block: 20 };
		price_history = vec![price_10, price_15, price_20];

		assert_eq!(Oracle::price_history(0), price_history);
		assert_eq!(Oracle::price_history(0).len(), 3);
	});
}

#[test]
fn price_of_amount() {
	new_test_ext().execute_with(|| {
		let value = NORMALIZED::units(50_000);
		let amount = BTC::units(5);

		Prices::<Test>::insert(BTC::ID, Price { price: value, block: System::block_number() });

		let total_price = <Oracle as oracle::Oracle>::get_price(BTC::ID, amount).unwrap();

		assert_eq!(total_price.price, value * amount / BTC::ONE)
	});
}

#[test]
fn ratio_human_case() {
	new_test_ext().execute_with(|| {
		let price = Price { price: 10000, block: System::block_number() };
		Prices::<Test>::insert(13, price);
		let price = Price { price: 100, block: System::block_number() };
		Prices::<Test>::insert(42, price);
		let mut pair = CurrencyPair::new(13, 42);

		let ratio = <Oracle as oracle::Oracle>::get_ratio(pair).unwrap();
		assert_eq!(ratio, FixedU128::saturating_from_integer(100));
		pair.reverse();
		let ratio = <Oracle as oracle::Oracle>::get_ratio(pair).unwrap();

		assert_eq!(ratio, FixedU128::saturating_from_rational(1_u32, 100_u32));
	})
}

#[test]
fn inverses() {
	new_test_ext().execute_with(|| {
		Prices::<Test>::insert(
			BTC::ID,
			Price { price: NORMALIZED::units(1), block: System::block_number() },
		);
		let inverse =
			<Oracle as oracle::Oracle>::get_price_inverse(BTC::ID, NORMALIZED::units(1)).unwrap();
		assert_eq!(inverse, BTC::units(1));

		Prices::<Test>::insert(
			BTC::ID,
			Price { price: NORMALIZED::units(1), block: System::block_number() },
		);
		let inverse =
			<Oracle as oracle::Oracle>::get_price_inverse(BTC::ID, NORMALIZED::units(2)).unwrap();
		assert_eq!(inverse, BTC::units(2));
	})
}

#[test]
fn ratio_base_is_way_less_smaller() {
	new_test_ext().execute_with(|| {
		let price = Price { price: 1, block: System::block_number() };
		Prices::<Test>::insert(BTC::ID, price);
		let price = Price { price: NORMALIZED::units(1), block: System::block_number() };
		Prices::<Test>::insert(PICA::ID, price);
		let pair = CurrencyPair::new(BTC::ID, PICA::ID);

		let ratio = <Oracle as oracle::Oracle>::get_ratio(pair).unwrap();

		assert_eq!(ratio, FixedU128::saturating_from_rational(1, 1000000000000_u64));
	})
}

#[test]
fn get_twap() {
	new_test_ext().execute_with(|| {
		// add and request oracle id
		let account_2 = get_root_account();
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			false,
		));

		let asset_id = 0;
		let block = 24;
		do_price_update(asset_id, block);

		let price_1 = Price { price: 100, block: 21 };
		let price_2 = Price { price: 100, block: 22 };
		let price_3 = Price { price: 120, block: 23 };
		let historic_prices = [price_1, price_2, price_3].to_vec();
		set_historic_prices(asset_id, historic_prices);

		let twap = Oracle::get_twap(0, 3);
		// twap should be ((1 * 100) + (1 * 120) + (1 * 101)) / (1 + 1 + 1)
		assert_eq!(twap, Ok(107));

		let err_2_twap = Oracle::get_twap(0, 100);
		assert_eq!(err_2_twap, Err(Error::<Test>::DepthTooLarge.into()));
	});
}

#[test]
fn get_twap_for_amount() {
	use composable_traits::oracle::Oracle as _;
	new_test_ext().execute_with(|| {
		// add and request oracle id
		let account_2 = get_root_account();
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			BTC::ID,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			false,
		));
		let block = 26;
		let account_1 = get_account_1();

		for _ in 0..3 {
			add_price_storage(NORMALIZED::units(120), BTC::ID, account_1, block);
		}

		System::set_block_number(block);
		Oracle::on_initialize(block);

		let price_1 = Price { price: NORMALIZED::units(100), block: 21 };
		let price_2 = Price { price: NORMALIZED::units(100), block: 22 };
		let price_3 = Price { price: NORMALIZED::units(100), block: 25 };
		let historic_prices = [price_1, price_2, price_3].to_vec();
		set_historic_prices(BTC::ID, historic_prices);

		let amount = 10_000;
		let twap = Oracle::get_twap_for_amount(BTC::ID, amount);
		// twap should be ((1 * 100) + (3 * 100) + (1 * 120)) / (1 + 3 + 1) * 10_000
		assert_eq!(twap, Ok(104 * amount));
	});
}

#[test]
fn on_init_prune_scenarios() {
	new_test_ext().execute_with(|| {
		// add and request oracle id
		let account_2 = get_root_account();
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			false
		));
		// set prices into storage
		let account_1 = get_account_1();
		for i in 0..3 {
			let price = i as u128 + 100_u128;
			add_price_storage(price, 0, account_1, 0);
		}
		// all pruned
		Oracle::on_initialize(3);
		let price = Price { price: 0, block: 0 };
		assert_eq!(Oracle::prices(0), price);
		assert_eq!(Oracle::pre_prices(0).len(), 0);

		for i in 0..5 {
			let price = i as u128 + 1_u128;
			add_price_storage(price, 0, account_1, 0);
		}

		for i in 0..3 {
			let price = i as u128 + 100_u128;
			add_price_storage(price, 0, account_1, 3);
		}

		// more than half pruned
		Oracle::on_initialize(3);
		let price = Price { price: 101, block: 3 };
		assert_eq!(Oracle::prices(0), price);

		for i in 0..5 {
			let price = i as u128 + 1_u128;
			add_price_storage(price, 0, account_1, 0);
		}

		for i in 0..2 {
			let price = i as u128 + 300_u128;
			add_price_storage(price, 0, account_1, 3);
		}

		// more than half pruned not enough for a price call, same as previous
		Oracle::on_initialize(5);
		let price = Price { price: 101, block: 3 };
		assert_eq!(Oracle::pre_prices(0).len(), 2);
		assert_eq!(Oracle::prices(0), price);
	});
}

#[test]
fn test_max_oracle_index() {
	new_test_ext().execute_with(|| {
		// add and request oracle id

		let root_account = get_root_account();
		let account_1 = get_account_1();
		let account_2 = get_root_account();
		let account_3 = get_account_3();
		let account_4 = get_account_4();
		let account_5 = get_account_5();
		let account_6 = get_account_6();
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(root_account),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(2).unwrap(),
			Validated::new(6).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			1,
			false,
		));
		//set signer
		Balances::make_free_balance_be(&account_1, 2 * MinStake::get());
		Balances::make_free_balance_be(&account_2, 2 * MinStake::get());
		Balances::make_free_balance_be(&account_3, 2 * MinStake::get());
		Balances::make_free_balance_be(&account_4, 2 * MinStake::get());
		Balances::make_free_balance_be(&account_5, 2 * MinStake::get());
		Balances::make_free_balance_be(&account_6, 2 * MinStake::get());
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_1, account_1));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_2, account_2));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_3, account_3));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_4, account_4));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_5, account_5));
		assert_ok!(Oracle::set_signer(RuntimeOrigin::signed(root_account), account_6, account_6));
		// set prices into storage
		System::set_block_number(6);
		Timestamp::set_timestamp(6);
		Oracle::on_initialize(6);
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_1), 100_u128, 0_u128));
		System::set_block_number(100);
		Timestamp::set_timestamp(100);
		Oracle::on_initialize(100);
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_2), 200_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_3), 300_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_4), 400_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_5), 500_u128, 0_u128));
		assert_ok!(Oracle::submit_price(RuntimeOrigin::signed(account_6), 600_u128, 0_u128));

		System::set_block_number(101);
		Timestamp::set_timestamp(101);
		Oracle::on_initialize(101);
		// price prunes all but last 2 answers, median went from 103
		let price = Price { price: 400, block: 101 };
		assert_eq!(Oracle::prices(0), price);
		assert_eq!(Oracle::pre_prices(0).len(), 0);
	});
}

#[test]
fn on_init_over_max_answers() {
	new_test_ext().execute_with(|| {
		// add and request oracle id
		let account_2 = get_root_account();
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(1).unwrap(),
			Validated::new(2).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			false,
		));
		// set prices into storage
		let account_1 = get_account_1();
		for i in 0..5 {
			let price = i as u128 + 100_u128;
			add_price_storage(price, 0, account_1, 0);
		}

		assert_eq!(Oracle::answer_in_transit(account_1), Some(25));

		// all pruned
		Oracle::on_initialize(0);
		// price prunes all but last 2 answers, median went from 103
		let price = Price { price: 103, block: 0 };
		assert_eq!(Oracle::prices(0), price);
		assert_eq!(Oracle::pre_prices(0).len(), 0);

		assert_eq!(Oracle::answer_in_transit(account_1), Some(0));
	});
}

#[test]
fn prune_old_pre_prices_edgecase() {
	new_test_ext().execute_with(|| {
		let asset_info = AssetInfo {
			threshold: Percent::from_percent(80),
			min_answers: 3,
			max_answers: 5,
			block_interval: 5,
			reward_weight: 5,
			slash: 5,
			emit_price_changes: false,
		};
		Oracle::prune_old_pre_prices(&asset_info, vec![], 0);
	});
}

#[test]
fn should_make_http_call_and_parse_result() {
	let (mut t, _, _) = offchain_worker_env(|state| price_oracle_response(state, "0"));

	t.execute_with(|| {
		// when
		let price = Oracle::fetch_price(&0).unwrap();
		// then
		assert_eq!(price, 15523);
	});
}

#[test]
fn knows_how_to_mock_several_http_calls() {
	let (mut t, _, _) = offchain_worker_env(|state| {
		state.expect_request(testing::PendingRequest {
			method: "GET".into(),
			uri: "http://localhost:3001/price/0".into(),
			response: Some(br#"{"0": 100}"#.to_vec()),
			sent: true,
			..Default::default()
		});

		state.expect_request(testing::PendingRequest {
			method: "GET".into(),
			uri: "http://localhost:3001/price/0".into(),
			response: Some(br#"{"0": 200}"#.to_vec()),
			sent: true,
			..Default::default()
		});

		state.expect_request(testing::PendingRequest {
			method: "GET".into(),
			uri: "http://localhost:3001/price/0".into(),
			response: Some(br#"{"0": 300}"#.to_vec()),
			sent: true,
			..Default::default()
		});
	});

	t.execute_with(|| {
		let price1 = Oracle::fetch_price(&0).unwrap();
		let price2 = Oracle::fetch_price(&0).unwrap();
		let price3 = Oracle::fetch_price(&0).unwrap();

		assert_eq!(price1, 100);
		assert_eq!(price2, 200);
		assert_eq!(price3, 300);
	})
}

#[test]
fn should_submit_signed_transaction_on_chain() {
	let (mut t, _, pool_state) = offchain_worker_env(|state| price_oracle_response(state, "0"));

	t.execute_with(|| {
		let account_2 = get_root_account();
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			false,
		));

		// when
		Oracle::fetch_price_and_send_signed(&0, Oracle::asset_info(0).unwrap()).unwrap();
		// then
		let tx = pool_state.write().transactions.pop().unwrap();
		assert!(pool_state.read().transactions.is_empty());
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		assert_eq!(tx.signature.unwrap().0, 0);
		assert_eq!(
			tx.call,
			RuntimeCall::Oracle(crate::Call::submit_price { price: 15523, asset_id: 0 })
		);
	});
}

#[test]
#[should_panic = "Tx already submitted"]
fn should_check_oracles_submitted_price() {
	let (mut t, oracle_account_id, _) =
		offchain_worker_env(|state| price_oracle_response(state, "0"));

	t.execute_with(|| {
		let account_2 = get_root_account();

		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(account_2),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			5,
			5,
			false,
		));

		add_price_storage(100_u128, 0, oracle_account_id, 0);
		// when
		Oracle::fetch_price_and_send_signed(&0, Oracle::asset_info(0).unwrap()).unwrap();
	});
}

#[test]
#[should_panic = "Max answers reached"]
fn should_check_oracles_max_answer() {
	let (mut t, _, _) = offchain_worker_env(|state| price_oracle_response(state, "0"));
	let asset_info = AssetInfo {
		threshold: Percent::from_percent(0),
		min_answers: 0,
		max_answers: 0,
		block_interval: 0,
		reward_weight: 0,
		slash: 0,
		emit_price_changes: false,
	};
	t.execute_with(|| {
		Oracle::fetch_price_and_send_signed(&0, asset_info).unwrap();
	});
}

#[test]
fn parse_price_works() {
	let test_data = vec![
		("{\"1\":6536.92}", Some(6536)),
		("{\"1\":650000000}", Some(650000000)),
		("{\"2\":6536}", None),
		("{\"0\":\"6432\"}", None),
	];

	for (json, expected) in test_data {
		assert_eq!(expected, Oracle::parse_price(json, "1"));
	}
}

fn add_price_storage(price: u128, asset_id: u128, who: AccountId, block: u64) {
	let price = PrePrice { price, block, who };
	PrePrices::<Test>::mutate(asset_id, |current_prices| current_prices.try_push(price).unwrap());
	AnswerInTransit::<Test>::mutate(who, |transit| {
		*transit = Some(transit.unwrap_or_else(Zero::zero) + 5)
	});
}

fn do_price_update(asset_id: u128, block: u64) {
	let account_1 = get_account_1();
	for i in 0..3 {
		let price = i as u128 + 100_u128;
		add_price_storage(price, asset_id, account_1, block);
	}

	System::set_block_number(block);
	Oracle::on_initialize(block);
	let price = Price { price: 101, block };
	assert_eq!(Oracle::prices(asset_id), price);
}

fn set_historic_prices(asset_id: u128, historic_prices: Vec<Price<u128, u64>>) {
	PriceHistory::<Test>::insert(asset_id, BoundedVec::try_from(historic_prices).unwrap());
}

fn price_oracle_response(state: &mut testing::OffchainState, price_id: &str) {
	let base: String = "http://localhost:3001/price/".to_owned();
	let url = base + price_id;

	state.expect_request(testing::PendingRequest {
		method: "GET".into(),
		uri: url,
		response: Some(br#"{"0": 15523}"#.to_vec()),
		sent: true,
		..Default::default()
	});
}

fn offchain_worker_env(
	state_updater: fn(&mut testing::OffchainState),
) -> (TestExternalities, AccountId, Arc<RwLock<testing::PoolState>>) {
	const PHRASE: &str =
		"news slush supreme milk chapter athlete soap sausage put clutch what kitten";

	let (offchain, offchain_state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();
	let keystore = KeyStore::new();
	let account_id = SyncCryptoStore::sr25519_generate_new(
		&keystore,
		crate::crypto::Public::ID,
		Some(&format!("{}/hunter1", PHRASE)),
	)
	.unwrap();

	let mut t = sp_io::TestExternalities::default();
	t.register_extension(OffchainDbExt::new(offchain.clone()));
	t.register_extension(OffchainWorkerExt::new(offchain));
	t.register_extension(TransactionPoolExt::new(pool));
	t.register_extension(KeystoreExt(Arc::new(keystore)));

	state_updater(&mut offchain_state.write());

	(t, account_id, pool_state)
}

#[cfg(test)]
mod test {
	use super::*;
	use composable_support::validation::Validate;
	use frame_support::assert_ok;
	use validation::{ValidBlockInterval, ValidMaxAnswer, ValidMinAnswers, ValidThreshold};

	#[test]
	fn test_threshold_valid_case() {
		assert_ok!(<ValidThreshold as Validate<Percent, ValidThreshold>>::validate(
			Percent::from_percent(99)
		));
	}

	#[test]
	fn test_threshold_invalid_case() {
		assert!(<ValidThreshold as Validate<Percent, ValidThreshold>>::validate(
			Percent::from_percent(100)
		)
		.is_err());

		assert!(<ValidThreshold as Validate<Percent, ValidThreshold>>::validate(
			Percent::from_percent(110)
		)
		.is_err());
	}

	#[test]
	fn test_threshold() {
		assert!(<ValidThreshold as Validate<Percent, ValidThreshold>>::validate(
			Percent::from_percent(100)
		)
		.is_err());

		assert!(<ValidThreshold as Validate<Percent, ValidThreshold>>::validate(
			Percent::from_percent(110)
		)
		.is_err());
	}

	#[test]
	fn test_max_answer_valid_case() {
		assert_ok!(<ValidMaxAnswer<MaxAnswerBound> as Validate<
			u32,
			ValidMaxAnswer<MaxAnswerBound>,
		>>::validate(2_u32));
	}

	#[test]
	fn test_max_answer_invalid_case() {
		assert!(<ValidMaxAnswer<MaxAnswerBound> as Validate<
			u32,
			ValidMaxAnswer<MaxAnswerBound>,
		>>::validate(16_u32)
		.is_err());
	}

	#[test]
	fn test_min_answer_valid_case() {
		assert!(<ValidMinAnswers<MinAnswerBound> as Validate<
			u32,
			ValidMinAnswers<MinAnswerBound>,
		>>::validate(0_u32)
		.is_err());
	}

	#[test]
	fn test_min_answer_invalid_case() {
		assert_ok!(<ValidMinAnswers<MinAnswerBound> as Validate<
			u32,
			ValidMinAnswers<MinAnswerBound>,
		>>::validate(4_u32));
	}

	#[test]
	fn test_block_interval_valid_case() {
		assert_ok!(<ValidBlockInterval<StalePrice> as Validate<
			BlockNumber,
			ValidBlockInterval<StalePrice>,
		>>::validate(100_u64));
	}

	#[test]
	fn test_block_interval_invalid_case() {
		assert!(<ValidBlockInterval<StalePrice> as Validate<
			BlockNumber,
			ValidBlockInterval<StalePrice>,
		>>::validate(2_u64)
		.is_err());
	}
}
#[test]
fn reward_rate_doesnt_start_on_asset_info() {
	new_test_ext().execute_with(|| {
		let root = get_root_account();
		assert_eq!(RewardTrackerStore::<Test>::get(), None);
		System::set_block_number(1);
		Timestamp::set_timestamp(1);
		Oracle::on_initialize(1);
		assert_eq!(RewardTrackerStore::<Test>::get(), None);
		let account_1_controller = get_account_1();
		let account_1_signer = get_account_3();
		Balances::make_free_balance_be(&account_1_controller, 1000);
		Balances::make_free_balance_be(&account_1_signer, 0);
		let rewards_account = Oracle::account_id();
		Balances::make_free_balance_be(&rewards_account, 10000);

		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(root),
			0,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			10,
			5,
			false,
		));

		assert_eq!(
			RewardTrackerStore::<Test>::get(),
			Some(RewardTracker {
				period: 0,
				start: 0,
				total_already_rewarded: 0,
				current_block_reward: 0,
				total_reward_weight: 10,
			})
		);
		System::set_block_number(2);
		Timestamp::set_timestamp(2);
		Oracle::on_initialize(2);
		assert_eq!(
			RewardTrackerStore::<Test>::get(),
			Some(RewardTracker {
				period: 0,
				start: 0,
				total_already_rewarded: 0,
				current_block_reward: 0,
				total_reward_weight: 10,
			})
		);
		let annual_cost_per_oracle: Balance = 100_000 * UNIT;
		let num_ideal_oracles: u8 = 10;
		assert_ok!(Oracle::adjust_rewards(
			RuntimeOrigin::root(),
			annual_cost_per_oracle,
			num_ideal_oracles
		));
		assert_eq!(
			RewardTrackerStore::<Test>::get(),
			Some(RewardTracker {
				period: MS_PER_YEAR_NAIVE,
				start: 2,
				total_already_rewarded: 0,
				current_block_reward: 380517503805,
				total_reward_weight: 10,
			})
		);
		System::set_block_number(100);
		Timestamp::set_timestamp(100);
		Oracle::on_initialize(100);
		assert_ok!(Oracle::add_asset_and_info(
			RuntimeOrigin::signed(root),
			1,
			Validated::new(Percent::from_percent(80)).unwrap(),
			Validated::new(3).unwrap(),
			Validated::new(5).unwrap(),
			Validated::<BlockNumber, ValidBlockInterval<StalePrice>>::new(5).unwrap(),
			10,
			5,
			false,
		));
		assert_eq!(
			RewardTrackerStore::<Test>::get(),
			Some(RewardTracker {
				period: MS_PER_YEAR_NAIVE,
				start: 2,
				total_already_rewarded: 0,
				current_block_reward: 380517503805,
				total_reward_weight: 20,
			})
		);
	});
}
