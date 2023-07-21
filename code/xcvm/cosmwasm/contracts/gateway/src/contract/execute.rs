extern crate alloc;

use crate::{
	assets, auth,
	error::{ContractError, ContractResult},
	events::make_event,
	exec, msg,
	prelude::*,
	state,
	topology::get_route,
};

use cosmwasm_std::{
	to_binary, wasm_execute, Addr, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
	Ibc3ChannelOpenResponse, IbcBasicResponse, IbcChannelCloseMsg, IbcChannelConnectMsg,
	IbcChannelOpenMsg, IbcChannelOpenResponse, IbcMsg, IbcOrder, IbcPacketAckMsg,
	IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, IbcTimeout, IbcTimeoutBlock,
	MessageInfo, Reply, Response, SubMsg, SubMsgResult, WasmMsg, ensure_eq,
};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;

use cw_utils::ensure_from_older_version;
use ibc_rs_scale::core::ics24_host::identifier::ChannelId;
use xc_core::{
	ibc::{to_cw_message, Ics20MessageHook, WasmMemo},
	proto::{decode_packet, Encodable},
	shared::{XcPacket, DefaultXCVMProgram},
	CallOrigin, Displayed, Funds, Picasso, XCVMAck, gateway::Asset,
};

use super::EXEC_PROGRAM_REPLY_ID;

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn execute(
	deps: DepsMut,
	env: Env,
	info: MessageInfo,
	msg: msg::ExecuteMsg,
) -> ContractResult<Response> {
	match msg {
		msg::ExecuteMsg::IbcSetNetworkChannel { from, to, channel_id, gateway } => {
			let auth = auth::Admin::authorise(deps.as_ref(), &info)?;
			handle_ibc_set_network_channel(auth, deps, to, channel_id)
		},

		msg::ExecuteMsg::ExecuteProgram { execute_program } =>
			exec::handle_execute_program(deps, env, info, execute_program),

		msg::ExecuteMsg::ExecuteProgramPrivileged { call_origin, execute_program } => {
			let auth = auth::Contract::authorise(&env, &info)?;
			exec::handle_execute_program_privilleged(auth, deps, env, call_origin, execute_program)
		},

		msg::ExecuteMsg::Bridge(msg) => {
			let auth =
				auth::Interpreter::authorise(deps.as_ref(), &info, msg.interpreter_origin.clone())?;
			handle_bridge_forward(auth, deps, info, msg)
		},

		msg::ExecuteMsg::RegisterAsset(msg) => {
			let auth = auth::Admin::authorise(deps.as_ref(), &info)?;
			assets::handle_register_asset(auth, deps, msg.id, msg.asset)
		},

		msg::ExecuteMsg::UnregisterAsset { asset_id } => {
			let auth = auth::Admin::authorise(deps.as_ref(), &info)?;
			assets::handle_unregister_asset(auth, deps, asset_id)
		},
		msg::ExecuteMsg::Wasm(msg) => {
			let auth = auth::WasmHook::authorise(deps.storage, &env, &info, msg.from_network_id)?;
			remote_wasm_execute(auth, msg, env)
		},
	}
}

fn remote_wasm_execute(
	_: auth::WasmHook,
	msg: Ics20MessageHook,
	env: Env,
) -> Result<Response, ContractError> {
	let packet: XcPacket = decode_packet(&msg.data).map_err(ContractError::Protobuf)?;
	let call_origin = CallOrigin::Remote {
		relayer: Addr::unchecked("no access"),
		user_origin: packet.user_origin,
	};
	let execute_program = msg::ExecuteProgramMsg {
		salt: packet.salt,
		program: packet.program,
		assets: packet.assets,
	};
	let msg = msg::ExecuteMsg::ExecuteProgramPrivileged { call_origin, execute_program };
	let msg = wasm_execute(env.contract.address, &msg, Default::default())?;
	Ok(Response::new().add_submessage(SubMsg::reply_always(msg, EXEC_PROGRAM_REPLY_ID)))
}

/// Handle a request gateway message.
/// The call must originate from an interpreter.
fn handle_bridge_forward(
	_: auth::Interpreter,
	deps: DepsMut,
	info: MessageInfo,
	msg: xc_core::gateway::BridgeMsg,
) -> ContractResult<Response> {
	let channel_id = state::IBC_NETWORK_CHANNEL
		.load(deps.storage, msg.network_id)
		.map_err(|_| ContractError::UnknownChannel)?;

	let packet = XcPacket {
		interpreter: String::from(info.sender).into_bytes(),
		user_origin: msg.interpreter_origin.user_origin,
		salt: msg.execute_program.salt,
		program: msg.execute_program.program,
		assets: msg.execute_program.assets,
	};
	
	ensure_eq!(packet.assets.0.len(), 1, "ICS20 limitation");
	
	let (local_asset, amount) = packet.assets.0.get(0).expect("verified at outer boundaries");
	let route = get_route(deps.storage, msg.network_id, *local_asset)?;
	let packket = XcPacket {
		assets : packet.assets.into_iter().map(|a|)
		..packet,
	};
	let mut event = make_event("bridge")
		.add_attribute("to_network_id", msg.network_id.to_string())
		.add_attribute(
			"assets",
			serde_json_wasm::to_string(&packet.assets)
				.map_err(|_| ContractError::FailedToSerialize)?,
		)
		.add_attribute(
			"program",
			serde_json_wasm::to_string(&packet.program)
				.map_err(|_| ContractError::FailedToSerialize)?,
		);
	if !packet.salt.is_empty() {
		let salt_attr = Binary::from(packet.salt.as_slice()).to_string();
		event = event.add_attribute("salt", salt_attr);
	}
		
	let coin = Coin::new(amount.0.into(), route.local_native_denom.clone());

	let memo = serde_json_wasm::to_string(&WasmMemo {
		contract: route.gateway_to_send_to.clone(),
		msg: to_binary(&Ics20MessageHook {
			from_network_id: route.from_network,
			data: Binary::from(packet.encode()),
		})?,
		ibc_callback: None,
	})?;

	let msg = to_cw_message(memo, coin, route)?;

	Ok(Response::default().add_event(event).add_message(msg))
}

fn handle_ibc_set_network_channel(
	_: auth::Admin,
	deps: DepsMut,
	network_id: xc_core::NetworkId,
	channel_id: ChannelId,
) -> ContractResult<Response> {
	state::IBC_CHANNEL_INFO
		.load(deps.storage, channel_id.to_string())
		.map_err(|_| ContractError::UnknownChannel)?;
	state::IBC_NETWORK_CHANNEL.save(deps.storage, network_id, &channel_id.to_string())?;
	Ok(Response::default().add_event(
		make_event("set_network_channel")
			.add_attribute("network_id", network_id.to_string())
			.add_attribute("channel_id", channel_id.to_string()),
	))
}
