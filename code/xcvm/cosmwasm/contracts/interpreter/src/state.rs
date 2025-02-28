use cosmwasm_std::{Addr, SubMsgResponse};
use cw_storage_plus::{Item, Map};
use serde::{Deserialize, Serialize};
use xc_core::InterpreterOrigin;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "std", derive(schemars::JsonSchema))]
pub struct Config {
	pub gateway_address: xc_core::gateway::Gateway,
	pub interpreter_origin: InterpreterOrigin,
}

/// The interpreter configuration.
pub const CONFIG: Item<Config> = Item::new("config");

/// List of owners able to execute programs on our behalf. Be aware that only `trusted` address must
/// be added.
pub const OWNERS: Map<Addr, ()> = Map::new("owners");

/// This register hold the latest program instruction (index) executed.
pub const IP_REGISTER: Item<u16> = Item::new("ip_register");

/// This register contains the latest executed instruction result for the program.
/// It can be either a success `SubMsgResponse` or an error message (in this case changes of message
/// were not applied).
pub const RESULT_REGISTER: Item<Result<SubMsgResponse, String>> = Item::new("result_register");

pub const TIP_REGISTER: Item<Addr> = Item::new("tip_register");
