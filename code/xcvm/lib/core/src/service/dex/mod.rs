use crate::{prelude::*, NetworkId};

use self::osmosis_std::types::osmosis::poolmanager::v1beta1::SwapAmountInRoute;

pub mod osmosis_std;

pub type ExchangeId = crate::shared::Displayed<u128>;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "std", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ExchangeType {
	OsmosisCrossChainSwap(vec::Vec<SwapAmountInRoute>),
}

/// allows to execute Exchange instruction
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "std", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub struct ExchangeItem {
	pub exchange_id: ExchangeId,
	pub network_id: NetworkId,
	pub exchange: ExchangeType,
}
