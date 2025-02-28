use cosmwasm_std::Addr;
use cw_storage_plus::Item;
use xc_core::NetworkId;

pub const GATEWAY: Item<Addr> = Item::new("gateway");
pub const NETWORK: Item<NetworkId> = Item::new("network");
