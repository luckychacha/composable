// Copyright 2021 Composable Developer.
// This file is part of Composable Finance.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use codec::{Decode, Encode};

use sp_runtime::{
	traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, One, Saturating, Zero},
	ArithmeticError, FixedPointNumber, FixedU128, RuntimeDebug,
};

/// The fixed point number from 0..to max.
/// Unlike `Ratio` it can be more than 1.
/// And unlike `NormalizedCollateralFactor`, it can be less than one.
pub type Rate = FixedU128;

/// The fixed point number of suggested by substrate precision
/// Must be (1.0.. because applied only to price normalized values
pub type NormalizedCollateralFactor = FixedU128;

/// Must be [0..1]
/// TODO: implement Ratio as wrapper over FixedU128
pub type Ratio = FixedU128;

/// seconds
pub type Timestamp = u64;

/// seconds
pub type Duration = u64;

/// Number like of higher bits, so that amount and balance calculations are done it it with higher
/// precision via fixed point.
/// While this is 128 bit, cannot support u128 because 18 bits are for of mantissa.
/// Can support u128 it lifter to use FixedU256
pub type LiftedFixedBalance = FixedU128;

/// little bit slower than maximizing performance by knowing constraints.
/// Example, you sum to negative numbers, can get underflow, so need to check on each add; but if you have positive number only, you cannot have underflow.
/// Same for other constrains, like non zero divisor.
pub trait SafeArithmetic: Sized {
	fn safe_add(&self, rhs: &Self) -> Result<Self, ArithmeticError>;
	fn safe_div(&self, rhs: &Self) -> Result<Self, ArithmeticError>;
	fn safe_mul(&self, rhs: &Self) -> Result<Self, ArithmeticError>;
	fn safe_sub(&self, rhs: &Self) -> Result<Self, ArithmeticError>;
}

impl SafeArithmetic for LiftedFixedBalance {
	#[inline(always)]
	fn safe_add(&self, rhs: &Self) -> Result<Self, ArithmeticError> {
		self.checked_add(rhs).ok_or(ArithmeticError::Overflow)
	}
	#[inline(always)]
	fn safe_div(&self, rhs: &Self) -> Result<Self, ArithmeticError> {
		if rhs.is_zero() {
			return Err(ArithmeticError::DivisionByZero);
		}

		self.checked_div(rhs).ok_or(ArithmeticError::Overflow)
	}

	#[inline(always)]
	fn safe_mul(&self, rhs: &Self) -> Result<Self, ArithmeticError> {
		self.checked_mul(rhs).ok_or(ArithmeticError::Overflow)
	}

	#[inline(always)]
	fn safe_sub(&self, rhs: &Self) -> Result<Self, ArithmeticError> {
		self.checked_sub(rhs).ok_or(ArithmeticError::Underflow)
	}
}

pub const SECONDS_PER_YEAR: Timestamp = 365 * 24 * 60 * 60;

/// utilization_ratio = total_borrows / (total_cash + total_borrows)
pub fn calc_utilization_ratio(
	cash: LiftedFixedBalance,
	borrows: LiftedFixedBalance,
) -> Result<Ratio, ArithmeticError> {
	if borrows.is_zero() {
		return Ok(Ratio::zero());
	}

	let total = cash.safe_add(&borrows)?;
	let util_ratio = borrows.checked_div(&total).expect("above checks prove it cannot error");
	assert!(util_ratio <= Ratio::one(), "because dividing summand by sum");
	Ok(util_ratio)
}

/// Parallel interest rate model
#[cfg_attr(feature = "std", derive(serde::Deserialize, serde::Serialize))]
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug)]
pub enum InterestRateModel {
	Jump(JumpModel),
	Curve(CurveModel),
}

impl Default for InterestRateModel {
	fn default() -> Self {
		Self::new_jump_model(
			Rate::saturating_from_rational(2, 100),
			Rate::saturating_from_rational(10, 100),
			Rate::saturating_from_rational(32, 100),
			Ratio::saturating_from_rational(80, 100),
		)
	}
}

impl InterestRateModel {
	pub fn new_jump_model(
		base_rate: Rate,
		jump_rate: Rate,
		full_rate: Rate,
		jump_utilization: Ratio,
	) -> Self {
		Self::Jump(JumpModel::new_model(base_rate, jump_rate, full_rate, jump_utilization))
	}

	pub fn new_curve_model(base_rate: Rate) -> Self {
		Self::Curve(CurveModel::new_model(base_rate))
	}

	pub fn check_model(&self) -> bool {
		match self {
			Self::Jump(jump) => jump.check_model(),
			Self::Curve(curve) => curve.check_model(),
		}
	}

	/// Calculates the current borrow interest rate
	pub fn get_borrow_rate(&self, utilization: Ratio) -> Option<Rate> {
		match self {
			Self::Jump(jump) => jump.get_borrow_rate(utilization),
			Self::Curve(curve) => curve.get_borrow_rate(utilization),
		}
	}

	/// Calculates the current supply interest rate
	pub fn get_supply_rate(borrow_rate: Rate, util: Ratio, reserve_factor: Ratio) -> Rate {
		// ((1 - reserve_factor) * borrow_rate) * utilization
		let one_minus_reserve_factor = Ratio::one().saturating_sub(reserve_factor);
		let rate_to_pool = borrow_rate.saturating_mul(one_minus_reserve_factor.into());

		rate_to_pool.saturating_mul(util.into())
	}
}

/// The jump interest rate model
#[cfg_attr(feature = "std", derive(serde::Deserialize, serde::Serialize))]
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, Default)]
pub struct JumpModel {
	/// The base interest rate when utilization rate is 0
	pub base_rate: Rate,
	/// The interest rate on jump utilization point
	pub jump_rate: Rate,
	/// The max interest rate when utilization rate is 100%
	pub full_rate: Rate,
	/// The utilization point at which the jump_rate is applied
	/// For jump_utilization, we should have used sp_runtime::Perquintil, but since Balance is
	/// based on u128 and Perquintil can't be created from u128.
	pub jump_utilization: Ratio,
}

impl JumpModel {
	pub const MAX_BASE_RATE: Ratio = Ratio::from_inner(100_000_000_000_000_000); // 10%
	pub const MAX_JUMP_RATE: Ratio = Ratio::from_inner(300_000_000_000_000_000); // 30%
	pub const MAX_FULL_RATE: Ratio = Ratio::from_inner(500_000_000_000_000_000); // 50%

	/// Create a new rate model
	pub fn new_model(
		base_rate: Ratio,
		jump_rate: Ratio,
		full_rate: Ratio,
		jump_utilization: Ratio,
	) -> JumpModel {
		if jump_utilization > Ratio::one() {
			Self { base_rate, jump_rate, full_rate, jump_utilization: Ratio::one() }
		} else {
			Self { base_rate, jump_rate, full_rate, jump_utilization }
		}
	}

	/// Check the jump model for sanity
	pub fn check_model(&self) -> bool {
		if self.base_rate > Self::MAX_BASE_RATE
			|| self.jump_rate > Self::MAX_JUMP_RATE
			|| self.full_rate > Self::MAX_FULL_RATE
		{
			return false;
		}
		if self.base_rate > self.jump_rate || self.jump_rate > self.full_rate {
			return false;
		}

		true
	}

	/// Calculates the borrow interest rate of jump model
	pub fn get_borrow_rate(&self, utilization: Ratio) -> Option<Rate> {
		if utilization <= self.jump_utilization {
			// utilization * (jump_rate - base_rate) / jump_utilization + base_rate
			let result = self
				.jump_rate
				.checked_sub(&self.base_rate)?
				.saturating_mul(utilization.into())
				.checked_div(&self.jump_utilization.into())?
				.checked_add(&self.base_rate)?;

			Some(result)
		} else {
			// (utilization - jump_utilization)*(full_rate - jump_rate) / ( 1 - jump_utilization) +
			// jump_rate
			let excess_util = utilization.saturating_sub(self.jump_utilization);
			let result = self
				.full_rate
				.checked_sub(&self.jump_rate)?
				.saturating_mul(excess_util.into())
				.checked_div(&(Ratio::one().saturating_sub(self.jump_utilization).into()))?
				.checked_add(&self.jump_rate)?;

			Some(result)
		}
	}
}

/// The curve interest rate model
#[cfg_attr(feature = "std", derive(serde::Deserialize, serde::Serialize))]
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, Default)]
pub struct CurveModel {
	base_rate: Rate,
}

impl CurveModel {
	pub const MAX_BASE_RATE: Rate = Rate::from_inner(Rate::DIV / 100 * 10); // 10%

	/// Create a new curve model
	pub fn new_model(base_rate: Rate) -> CurveModel {
		Self { base_rate }
	}

	/// Check the curve model for sanity
	pub fn check_model(&self) -> bool {
		self.base_rate <= Self::MAX_BASE_RATE
	}

	/// Calculates the borrow interest rate of curve model
	pub fn get_borrow_rate(&self, utilization: Ratio) -> Option<Rate> {
		const NINE: usize = 9;
		utilization.saturating_pow(NINE).checked_add(&self.base_rate)
	}
}

pub fn accrued_interest(borrow_rate: Rate, amount: u128, delta_time: Duration) -> Option<u128> {
	borrow_rate
		.checked_mul_int(amount)?
		.checked_mul(delta_time.into())?
		.checked_div(SECONDS_PER_YEAR.into())
}

pub fn increment_index(borrow_rate: Rate, index: Rate, delta_time: Duration) -> Option<Rate> {
	borrow_rate
		.checked_mul(&index)?
		.checked_mul(&FixedU128::saturating_from_integer(delta_time))?
		.checked_div(&FixedU128::saturating_from_integer(SECONDS_PER_YEAR))
}

pub fn increment_borrow_rate(borrow_rate: Rate, delta_time: Duration) -> Option<Rate> {
	borrow_rate
		.checked_mul(&FixedU128::saturating_from_integer(delta_time))?
		.checked_div(&FixedU128::saturating_from_integer(SECONDS_PER_YEAR))
}

#[cfg(test)]
mod tests {
	use super::*;
	use proptest::{prop_assert, strategy::Strategy, test_runner::TestRunner};
	use sp_runtime::FixedU128;

	// Test jump model
	#[test]
	fn init_jump_model_works() {
		let base_rate = Rate::saturating_from_rational(2, 100);
		let jump_rate = Rate::saturating_from_rational(10, 100);
		let full_rate = Rate::saturating_from_rational(32, 100);
		let jump_utilization = Ratio::saturating_from_rational(80, 100);

		assert_eq!(
			JumpModel::new_model(base_rate, jump_rate, full_rate, jump_utilization),
			JumpModel {
				base_rate: Rate::from_inner(20_000_000_000_000_000).into(),
				jump_rate: Rate::from_inner(100_000_000_000_000_000).into(),
				full_rate: Rate::from_inner(320_000_000_000_000_000).into(),
				jump_utilization: Ratio::saturating_from_rational(80, 100),
			}
		);
	}

	#[test]
	fn get_borrow_rate_works() {
		// init
		let base_rate = Rate::saturating_from_rational(2, 100);
		let jump_rate = Rate::saturating_from_rational(10, 100);
		let full_rate = Rate::saturating_from_rational(32, 100);
		let jump_utilization = Ratio::saturating_from_rational(80, 100);
		let jump_model = JumpModel::new_model(base_rate, jump_rate, full_rate, jump_utilization);
		assert!(jump_model.check_model());

		// normal rate
		let mut cash: u128 = 500;
		let borrows: u128 = 1000;
		let util = Ratio::saturating_from_rational(borrows, cash + borrows);
		let borrow_rate = jump_model.get_borrow_rate(util).unwrap();
		assert_eq!(
			borrow_rate,
			jump_model.jump_rate.saturating_mul(util.into()) + jump_model.base_rate,
		);

		// jump rate
		cash = 100;
		let util = Ratio::saturating_from_rational(borrows, cash + borrows);
		let borrow_rate = jump_model.get_borrow_rate(util).unwrap();
		let normal_rate =
			jump_model.jump_rate.saturating_mul(jump_utilization.into()) + jump_model.base_rate;
		let excess_util = util.saturating_sub(jump_utilization);
		assert_eq!(
			borrow_rate,
			(jump_model.full_rate - jump_model.jump_rate).saturating_mul(excess_util.into())
				/ FixedU128::saturating_from_rational(20, 100)
				+ normal_rate,
		);
	}

	// Test curve model
	// TODO: Add test cases for curve model

	#[test]
	fn get_supply_rate_works() {
		let borrow_rate = Rate::saturating_from_rational(2, 100);
		let util = Ratio::saturating_from_rational(50, 100);
		let reserve_factor = Ratio::zero();
		let supply_rate = InterestRateModel::get_supply_rate(borrow_rate, util, reserve_factor);
		assert_eq!(
			supply_rate,
			borrow_rate
				.saturating_mul(((Ratio::one().saturating_sub(reserve_factor)) * util).into()),
		);
	}

	#[test]
	fn curve_model_correctly_calculates_borrow_rate() {
		let model = CurveModel::new_model(Rate::saturating_from_rational(2, 100));
		assert_eq!(
			model.get_borrow_rate(Ratio::saturating_from_rational(80, 100)).unwrap(),
			Rate::from_inner(154217728000000000)
		);
	}

	#[derive(Debug, Clone)]
	struct JumpModelStrategy {
		pub base_rate: Ratio,
		pub jump_percentages: Ratio,
		pub full_percentages: Ratio,
		pub target_utilization_percentage: Ratio,
	}

	fn valid_jump_model() -> impl Strategy<Value = JumpModelStrategy> {
		(
			(1..=10u32).prop_map(|x| Ratio::saturating_from_rational(x, 100)),
			(11..=30u32).prop_map(|x| Ratio::saturating_from_rational(x, 100)),
			(31..=50).prop_map(|x| Ratio::saturating_from_rational(x, 100)),
			(0..=100).prop_map(|x| Ratio::saturating_from_rational(x, 100)),
		)
			.prop_filter("Jump rate model", |(base, jump, full, _)| {
				// tried high order strategy - failed as it tries to combine collections with not collection
				// alternative to define arbitrary and proptest attributes with filtering
				// overall cardinality is small, so should work well
				// here we have one liner, not sure why in code we have many lines....
				base <= jump
					&& jump <= full && base <= &JumpModel::MAX_BASE_RATE
					&& jump <= &JumpModel::MAX_JUMP_RATE
					&& full <= &JumpModel::MAX_FULL_RATE
			})
			.prop_map(
				|(base_rate, jump_percentages, full_percentages, target_utilization_percentage)| {
					JumpModelStrategy {
						base_rate,
						full_percentages,
						jump_percentages,
						target_utilization_percentage,
					}
				},
			)
	}

	#[test]
	fn proptest_jump_model() {
		let mut runner = TestRunner::default();
		runner
			.run(&(valid_jump_model(), 0..=u64::MAX, 0..=u64::MAX), |(strategy, cash, borrows)| {
				let base_rate = strategy.base_rate;
				let jump_rate = strategy.jump_percentages;
				let full_rate = strategy.full_percentages;
				let jump_utilization = strategy.target_utilization_percentage;
				let jump_model =
					JumpModel::new_model(base_rate, jump_rate, full_rate, jump_utilization);
				assert!(jump_model.check_model());

				let util = calc_utilization_ratio(
					FixedU128::checked_from_integer(cash as u128).unwrap(),
					FixedU128::checked_from_integer(borrows as u128).unwrap(),
				)
				.unwrap();
				let borrow_rate = jump_model.get_borrow_rate(util).unwrap();
				prop_assert!(borrow_rate > Rate::zero());
				Ok(())
			})
			.unwrap();
	}
}
