use crate::{
	instrument::CostRules,
	pallet_hook::PalletHook,
	runtimes::{
		abstraction::CosmwasmAccount,
		vm::{CosmwasmVM, CosmwasmVMError},
	},
	types::*,
	*,
};
use common::cosmwasm::CosmwasmToSubstrateAccount;
use composable_traits::currency::{CurrencyFactory, RangeId};
use core::marker::PhantomData;

use cosmwasm_std::{
	ContractResult, Event as CosmwasmEvent, Ibc3ChannelOpenResponse, IbcMsg, IbcTimeout,
	QueryResponse, Response, SubMsg, WasmMsg,
};
use cosmwasm_vm::vm::{VMBase, VmErrorOf, VmGas};
use cosmwasm_vm_wasmi::OwnedWasmiVM;
use frame_support::{
	ord_parameter_types,
	pallet_prelude::ConstU32,
	parameter_types,
	traits::{ConstU64, EitherOfDiverse, Everything},
	PalletId,
};
use frame_system::{EnsureRoot, EnsureSigned, EnsureSignedBy};
use num_traits::Zero;
use orml_traits::parameter_type_with_key;
use primitives::currency::{CurrencyId, ForeignAssetId};
use sp_core::H256;
use sp_runtime::{
	generic,
	traits::{AccountIdConversion, BlakeTwo256, Convert, ConvertInto, IdentityLookup},
	AccountId32, DispatchError,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type Header = generic::Header<u32, BlakeTwo256>;
type Balance = u128;
type AccountId = AccountId32;
type Amount = i128;

#[allow(clippy::derivable_impls)]
impl Default for Test {
	fn default() -> Self {
		Self {}
	}
}

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Cosmwasm: crate,
		Balances: pallet_balances,
		AssetsRegistry: pallet_assets_registry,
		Assets: pallet_assets_transactor_router,
		Timestamp: pallet_timestamp,
		Tokens: orml_tokens,
	}
);

ord_parameter_types! {
	pub const RootAccount: AccountId = get_root_account();
}

parameter_types! {
	pub const BlockHashCount: u32 = 250;
	pub const SS58Prefix: u8 = 42;
	pub const ExistentialDeposit: u64 = 10000;
	pub const NativeAssetId: CurrencyId = CurrencyId(1);
}

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = u64;
	type BlockNumber = u32;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<2>;
}

parameter_types! {
	pub const MaxLocks: u32 = 256;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_a: CurrencyId| -> Balance {
		Zero::zero()
	};
}

pub struct CurrencyHooks;
impl orml_traits::currency::MutationHooks<AccountId, CurrencyId, Balance> for CurrencyHooks {
	type OnDust = ();
	type OnSlash = ();
	type PreDeposit = ();
	type PostDeposit = ();
	type PreTransfer = ();
	type PostTransfer = ();
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

type ReserveIdentifier = [u8; 8];
impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = MaxLocks;
	type ReserveIdentifier = ReserveIdentifier;
	type MaxReserves = frame_support::traits::ConstU32<2>;
	type DustRemovalWhitelist = Everything;
	type CurrencyHooks = CurrencyHooks;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ConstU32<2>;
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
}

pub struct CurrencyIdGenerator;

impl CurrencyFactory for CurrencyIdGenerator {
	type AssetId = CurrencyId;
	type Balance = Balance;

	fn create(_: RangeId) -> Result<Self::AssetId, sp_runtime::DispatchError> {
		Ok(CurrencyId(1))
	}

	fn protocol_asset_id_to_unique_asset_id(
		_protocol_asset_id: u32,
		_range_id: RangeId,
	) -> Result<Self::AssetId, DispatchError> {
		Ok(CurrencyId(1))
	}

	fn unique_asset_id_to_protocol_asset_id(_unique_asset_id: Self::AssetId) -> u32 {
		1
	}
}

parameter_types! {
	pub const PicassoNetworkId: u32 = 0;
}

impl pallet_assets_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type LocalAssetId = CurrencyId;
	type ForeignAssetId = ForeignAssetId;
	type UpdateAssetRegistryOrigin = EnsureRoot<AccountId>;
	type ParachainOrGovernanceOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
	type Balance = Balance;
	type Convert = ConvertInto;
	type NetworkId = PicassoNetworkId;
}

impl pallet_assets_transactor_router::Config for Test {
	type AssetId = CurrencyId;
	type Balance = Balance;
	type NativeAssetId = NativeAssetId;
	type NativeTransactor = Balances;
	type LocalTransactor = Tokens;
	type ForeignTransactor = Tokens;
	type WeightInfo = ();
	type AdminOrigin = EnsureRoot<AccountId>;
	type AssetLocation = ForeignAssetId;
	type AssetsRegistry = AssetsRegistry;
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ConstU64<5>;
	type WeightInfo = ();
}

/// Native <-> Cosmwasm asset mapping
pub struct AssetToDenom;

impl Convert<alloc::string::String, Result<CurrencyId, ()>> for AssetToDenom {
	fn convert(currency_id: alloc::string::String) -> Result<CurrencyId, ()> {
		core::str::FromStr::from_str(&currency_id).map_err(|_| ())
	}
}

impl Convert<CurrencyId, alloc::string::String> for AssetToDenom {
	fn convert(CurrencyId(currency_id): CurrencyId) -> alloc::string::String {
		currency_id.to_string()
	}
}

parameter_types! {
	pub const CosmwasmPalletId: PalletId = PalletId(*b"cosmwasm");
	pub IbcRelayerAccount: AccountId = PalletId(*b"centauri").into_account_truncating();
	pub const ChainId: &'static str = "composable-network-dali";
	pub const MaxFrames: u16 = 64;
	pub const MaxCodeSize: u32 = 512 * 1024;
	pub const MaxInstrumentedCodeSize: u32 = 1024 * 1024;
	pub const MaxMessageSize: u32 = 256 * 1024;
	pub const MaxContractLabelSize: u32 = 64;
	pub const MaxContractTrieIdSize: u32 = H256::len_bytes() as u32;
	pub const MaxInstantiateSaltSize: u32 = 128;
	pub const MaxFundsAssets: u32 = 32;
	pub const CodeTableSizeLimit: u32 = 4096;
	pub const CodeGlobalVariableLimit: u32 = 256;
	pub const CodeParameterLimit: u32 = 128;
	pub const CodeBranchTableSizeLimit: u32 = 256;
	pub const CodeStackLimit: u32 = u32::MAX;

	pub const CodeStorageByteDeposit: u32 = 1;
	pub const ContractStorageByteReadPrice: u32 = 1;
	pub const ContractStorageByteWritePrice: u32 = 1;
	pub WasmCostRules: CostRules<Test> = Default::default();
}

pub struct IbcLoopback<Config> {
	_marker: PhantomData<Config>,
}

impl<T: Config> ibc_primitives::IbcHandler<AccountIdOf<T>> for IbcLoopback<T> {
	fn handle_message(
		_msg: ibc_primitives::HandlerMessage<AccountIdOf<T>>,
	) -> Result<(), ibc_primitives::Error> {
		System::remark_with_event(
			frame_system::Origin::<Test>::Signed(MOCK_PALLET_IBC_CONTRACT_ADDRESS).into(),
			b"ibc->cw->ibc".to_vec(),
		)
		.unwrap();
		// here we should call the cw contract, so it goes to area of emulation of ibc pallet, may
		// be next pr
		Ok(())
	}

	fn latest_height_and_timestamp(
		_port_id: &::ibc::core::ics24_host::identifier::PortId,
		_channel_id: &::ibc::core::ics24_host::identifier::ChannelId,
	) -> Result<(::ibc::Height, ::ibc::timestamp::Timestamp), ibc_primitives::Error> {
		todo!("loopback")
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn create_client(
	) -> Result<::ibc::core::ics24_host::identifier::ClientId, ibc_primitives::Error> {
		todo!("loopback")
	}
	#[cfg(feature = "runtime-benchmarks")]
	fn create_connection(
		_client_id: ::ibc::core::ics24_host::identifier::ClientId,
		_connection_id: ::ibc::core::ics24_host::identifier::ConnectionId,
	) -> Result<(), ibc_primitives::Error> {
		todo!("loopback")
	}
}

pub struct MockHook;

pub const MOCK_PALLET_CONTRACT_ADDRESS_1: AccountIdOf<Test> = AccountId32::new([u8::MAX; 32]);
pub const MOCK_PALLET_CONTRACT_ADDRESS_2: AccountIdOf<Test> = AccountId32::new([120u8; 32]);

pub const MOCK_PALLET_IBC_CONTRACT_ADDRESS: AccountIdOf<Test> = AccountId32::new([42; 32]);
pub const MOCK_CONTRACT_IBC_EVENT_TYPE_1: &str = "ibc-magic";

pub const MOCK_CONTRACT_EVENT_TYPE_1: &str = "magic";
pub const MOCK_CONTRACT_EVENT_TYPE_2: &str = "magic but it is blue";
pub const MOCK_QUERY_JS: &str = "It's JavaScript, What Did You Expect";

pub const MOCK_PALLET_ACCOUNT_ID_1: AccountIdOf<Test> = AccountId32::new([1u8; 32]);
pub const MOCK_PALLET_ACCOUNT_ID_2: AccountIdOf<Test> = AccountId32::new([2u8; 32]);

impl PalletHook<Test> for MockHook {
	// This mocked hook shows two pallets with contract hooks that currently exhibit the same
	// behavior. The behavior does not need to be identical in practice.

	fn info(
		contract_address: &AccountIdOf<Test>,
	) -> Option<
		PalletContractCodeInfo<AccountIdOf<Test>, ContractLabelOf<Test>, ContractTrieIdOf<Test>>,
	> {
		match *contract_address {
			MOCK_PALLET_CONTRACT_ADDRESS_1 => Some(PalletContractCodeInfo::new(
				MOCK_PALLET_ACCOUNT_ID_1,
				false,
				"pallet-mock-1".as_bytes().to_vec().try_into().unwrap_or_default(),
			)),
			MOCK_PALLET_CONTRACT_ADDRESS_2 => Some(PalletContractCodeInfo::new(
				MOCK_PALLET_ACCOUNT_ID_2,
				false,
				"pallet-mock-2".as_bytes().to_vec().try_into().unwrap_or_default(),
			)),
			MOCK_PALLET_IBC_CONTRACT_ADDRESS => Some(PalletContractCodeInfo::new(
				MOCK_PALLET_IBC_CONTRACT_ADDRESS,
				true,
				"MOCK_PALLET_IBC_CONTRACT_ADDRESS"
					.as_bytes()
					.to_vec()
					.try_into()
					.unwrap_or_default(),
			)),
			_ => None,
		}
	}

	fn execute<'a>(
		vm: &mut OwnedWasmiVM<CosmwasmVM<'a, Test>>,
		entrypoint: EntryPoint,
		message: &[u8],
	) -> Result<
		ContractResult<Response<<OwnedWasmiVM<CosmwasmVM<'a, Test>> as VMBase>::MessageCustom>>,
		VmErrorOf<OwnedWasmiVM<CosmwasmVM<'a, Test>>>,
	> {
		match entrypoint {
			EntryPoint::IbcChannelConnect => match *vm.0.data().contract_address.as_ref() {
				MOCK_PALLET_IBC_CONTRACT_ADDRESS => Ok(ContractResult::Ok(Response::new())),
				_ => Err(CosmwasmVMError::Unsupported),
			},
			EntryPoint::IbcChannelClose => Err(CosmwasmVMError::Unsupported),
			EntryPoint::IbcPacketTimeout => Err(CosmwasmVMError::Unsupported),
			EntryPoint::IbcPacketAck => Err(CosmwasmVMError::Unsupported),
			EntryPoint::IbcPacketReceive => match *vm.0.data().contract_address.as_ref() {
				MOCK_PALLET_IBC_CONTRACT_ADDRESS => match message {
					&[1, 2, 3] => {
						System::remark_with_event(
							frame_system::Origin::<Test>::Signed(MOCK_PALLET_IBC_CONTRACT_ADDRESS)
								.into(),
							b"ibc->cw->ibc->cw".to_vec(),
						)
						.unwrap();
						Ok(ContractResult::Ok(
							Response::new().add_event(CosmwasmEvent::new("cw-ibc-cw-ibc-cw")),
						))
					},

					_ => {
						let response = Response::new()
							.add_event(CosmwasmEvent::new(MOCK_CONTRACT_IBC_EVENT_TYPE_1))
							.add_message(WasmMsg::Execute {
								contract_addr: CosmwasmToSubstrateAccount::convert(
									MOCK_PALLET_IBC_CONTRACT_ADDRESS,
								),
								msg: cosmwasm_std::Binary("42".as_bytes().to_vec()),
								funds: Default::default(),
							})
							.add_message(IbcMsg::SendPacket {
								channel_id: "channel-0".to_string(),
								data: [1, 2, 3].into(),
								timeout: IbcTimeout::with_timestamp(
									cosmwasm_std::Timestamp::from_nanos(0),
								),
							})
							.set_data(0x666_u32.to_le_bytes());

						Ok(ContractResult::Ok(response))
					},
				},
				_ => Err(CosmwasmVMError::Unsupported),
			},
			_ => match *vm.0.data().contract_address.as_ref() {
				MOCK_PALLET_IBC_CONTRACT_ADDRESS => {
					System::remark_with_event(
						frame_system::Origin::<Test>::Signed(MOCK_PALLET_IBC_CONTRACT_ADDRESS)
							.into(),
						b"ibc->cw".to_vec(),
					)
					.unwrap();
					Ok(ContractResult::Ok(Response::new()))
				},
				MOCK_PALLET_CONTRACT_ADDRESS_1 => {
					vm.charge(VmGas::Instrumentation { metered: 1 })?;
					let mut response = Response::new()
						.add_event(CosmwasmEvent::new(MOCK_CONTRACT_EVENT_TYPE_1))
						.set_data(0xDEADC0DE_u32.to_le_bytes());
					let depth = message.first().copied().unwrap_or(0);
					if depth > 0 {
						response = response.add_submessage(SubMsg::new(WasmMsg::Execute {
							contract_addr: CosmwasmToSubstrateAccount::convert(
								MOCK_PALLET_CONTRACT_ADDRESS_1,
							),
							msg: vec![depth - 1].into(),
							funds: Default::default(),
						}));
					}
					match vm
						.continue_query(
							CosmwasmAccount::new(MOCK_PALLET_CONTRACT_ADDRESS_1),
							Default::default(),
						)?
						.into()
					{
						ContractResult::Err(x) if x == MOCK_QUERY_JS =>
							Ok(ContractResult::Ok(response)),
						_ => Ok(ContractResult::Err("JavaScript must fail".into())),
					}
				},
				MOCK_PALLET_CONTRACT_ADDRESS_2 => {
					vm.charge(VmGas::Instrumentation { metered: 1 })?;
					let mut response = Response::new()
						.add_event(CosmwasmEvent::new(MOCK_CONTRACT_EVENT_TYPE_2))
						.set_data(0xDEADC0DE_u32.to_le_bytes());
					let depth = message.first().copied().unwrap_or(0);
					if depth > 0 {
						response = response.add_submessage(SubMsg::new(WasmMsg::Execute {
							contract_addr: CosmwasmToSubstrateAccount::convert(
								MOCK_PALLET_CONTRACT_ADDRESS_2,
							),
							msg: vec![depth - 1].into(),
							funds: Default::default(),
						}));
					}
					match vm
						.continue_query(
							CosmwasmAccount::new(MOCK_PALLET_CONTRACT_ADDRESS_2),
							Default::default(),
						)?
						.into()
					{
						ContractResult::Err(x) if x == MOCK_QUERY_JS =>
							Ok(ContractResult::Ok(response)),
						_ => Ok(ContractResult::Err("JavaScript must fail".into())),
					}
				},
				_ => Err(CosmwasmVMError::Unsupported), // Should be impossible
			},
		}
	}

	fn query<'a>(
		vm: &mut OwnedWasmiVM<CosmwasmVM<'a, Test>>,
		_message: &[u8],
	) -> Result<ContractResult<QueryResponse>, VmErrorOf<OwnedWasmiVM<CosmwasmVM<'a, Test>>>> {
		match *vm.0.data().contract_address.as_ref() {
			MOCK_PALLET_CONTRACT_ADDRESS_1 | MOCK_PALLET_CONTRACT_ADDRESS_2 =>
				Ok(ContractResult::Err(MOCK_QUERY_JS.into())),
			_ => Err(CosmwasmVMError::Unsupported), // Should be impossible
		}
	}

	fn run<'a>(
		vm: &mut OwnedWasmiVM<CosmwasmVM<'a, Test>>,
		_entrypoint: EntryPoint,
		_message: &[u8],
	) -> Result<Vec<u8>, VmErrorOf<OwnedWasmiVM<CosmwasmVM<'a, Test>>>> {
		match *vm.0.data().contract_address.as_ref() {
			MOCK_PALLET_IBC_CONTRACT_ADDRESS =>
				Ok(serde_json::to_vec(&ContractResult::Ok(Ibc3ChannelOpenResponse {
					version: "42".to_string(),
				}))
				.unwrap()),
			_ => Err(CosmwasmVMError::Unsupported),
		}
	}
}

impl Config for Test {
	const MAX_FRAMES: u8 = 64;
	type RuntimeEvent = RuntimeEvent;
	type AccountIdExtended = AccountId;
	type PalletId = CosmwasmPalletId;
	type MaxCodeSize = MaxCodeSize;
	type MaxInstrumentedCodeSize = MaxInstrumentedCodeSize;
	type MaxMessageSize = MaxMessageSize;
	type AccountToAddr = CosmwasmToSubstrateAccount;
	type AssetToDenom = AssetToDenom;
	type Balance = Balance;
	type AssetId = CurrencyId;
	type Assets = Assets;
	type NativeAsset = Balances;
	type ChainId = ChainId;
	type MaxContractLabelSize = MaxContractLabelSize;
	type MaxContractTrieIdSize = MaxContractTrieIdSize;
	type MaxInstantiateSaltSize = MaxInstantiateSaltSize;
	type MaxFundsAssets = MaxFundsAssets;
	type CodeTableSizeLimit = CodeTableSizeLimit;
	type CodeGlobalVariableLimit = CodeGlobalVariableLimit;
	type CodeParameterLimit = CodeParameterLimit;
	type CodeBranchTableSizeLimit = CodeBranchTableSizeLimit;
	type CodeStackLimit = CodeStackLimit;
	type CodeStorageByteDeposit = CodeStorageByteDeposit;
	type ContractStorageByteReadPrice = ContractStorageByteReadPrice;
	type ContractStorageByteWritePrice = ContractStorageByteWritePrice;
	type UnixTime = Timestamp;
	type WeightInfo = ();
	type WasmCostRules = WasmCostRules;
	type IbcRelayerAccount = IbcRelayerAccount;
	type IbcRelayer = IbcLoopback<Self>;
	type PalletHook = MockHook;
	type UploadWasmOrigin =
		EitherOfDiverse<EnsureSignedBy<RootAccount, AccountId>, EnsureRoot<AccountId>>;

	type ExecuteWasmOrigin = EnsureSigned<AccountId>;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let origin = frame_benchmarking::account("signer", 0, 0xCAFEBABE);
	let balances: Vec<(AccountId, Balance)> = vec![(origin, 1_000_000_000_000_000_000)];
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	let genesis = pallet_balances::GenesisConfig::<Test> { balances };
	genesis.assimilate_storage(&mut t).unwrap();
	t.into()
}

pub(crate) const fn get_root_account() -> AccountId {
	AccountId32::new([1u8; 32])
}
