//! With Polkadot Bridge Hub bridge configuration.

use crate::{AccountId, Runtime, RuntimeEvent, RuntimeOrigin};

use bp_messages::{target_chain::ForbidInboundMessages, LaneId, MessageNonce};
use bp_parachains::SingleParaStoredHeaderDataBuilder;
use bp_runtime::{ChainId, UnderlyingChainProvider};
use bridge_runtime_common::{
	messages::{
		source::{FromThisChainMaximalOutboundPayloadSize, FromThisChainMessageVerifier, TargetHeaderChainAdapter},
		target::SourceHeaderChainAdapter,
		BridgedChainWithMessages, MessageBridge, ThisChainWithMessages,
	},
	messages_xcm_extension::XcmAsPlainPayload,
};
use frame_support::{RuntimeDebug, parameter_types};

/// Lane that we are using to send and receive messages.
pub const XCM_LANE: LaneId = LaneId([0, 0, 0, 0]);

// TODO: separate crate in bridges repo
pub mod bp_polkadot_bulletin_chain {
	pub use bp_polkadot_core::*;
	use bp_header_chain::ChainWithGrandpa;
	use bp_messages::MessageNonce;
	use bp_runtime::Chain;
	use frame_support::{weights::{DispatchClass, Weight}, RuntimeDebug};

	/// Name of the With-Polkadot Bulletin Chain GRANDPA pallet instance that is deployed at bridged chains.
	pub const WITH_POLKADOT_BULLETIN_CHAIN_GRANDPA_PALLET_NAME: &str = "BridgePolkadotBulletinChainGrandpa";
	/// Name of the with-Bulletin Chain messages pallet used at other chain runtimes.
	pub const WITH_POLKADOT_BULLETIN_CHAIN_MESSAGES_PALLET_NAME: &'static str = "WithPolkadotBulletinChainMessages";

	/// Maximal number of unrewarded relayer entries at inbound lane for Bulletin Chain.
	/// Note: this value is security-relevant, decreasing it should not be done without careful
	/// analysis (like the one above).
	pub const MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX: MessageNonce = 1024;

	/// Maximal number of unconfirmed messages at inbound lane for Bulletin Chain.
	/// Note: this value is security-relevant, decreasing it should not be done without careful
	/// analysis (like the one above).
	pub const MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX: MessageNonce = 4096;

	/// Polkadot Bulletin Chain declaration.
	#[derive(RuntimeDebug)]
	pub struct PolkadotBulletinChain;

	impl Chain for PolkadotBulletinChain {
		type BlockNumber = <PolkadotLike as Chain>::BlockNumber;
		type Hash = <PolkadotLike as Chain>::Hash;
		type Hasher = <PolkadotLike as Chain>::Hasher;
		type Header = <PolkadotLike as Chain>::Header;
	
		type AccountId = <PolkadotLike as Chain>::AccountId;
		type Balance = <PolkadotLike as Chain>::Balance;
		type Nonce = <PolkadotLike as Chain>::Nonce;
		type Signature = <PolkadotLike as Chain>::Signature;

		// TODO: when porting to parity-bridges-common, check if we can reuse polkadot weight/size limits

		fn max_extrinsic_size() -> u32 {
			*crate::BlockLength::get().max.get(DispatchClass::Normal)
		}

		fn max_extrinsic_weight() -> Weight {
			crate::BlockWeights::get()
				.get(DispatchClass::Normal)
				.max_extrinsic
				.unwrap_or(Weight::MAX)
		}
	}

	impl ChainWithGrandpa for PolkadotBulletinChain {
		const WITH_CHAIN_GRANDPA_PALLET_NAME: &'static str = WITH_POLKADOT_BULLETIN_CHAIN_GRANDPA_PALLET_NAME;
		const MAX_AUTHORITIES_COUNT: u32 = MAX_AUTHORITIES_COUNT;
		const REASONABLE_HEADERS_IN_JUSTIFICATON_ANCESTRY: u32 = REASONABLE_HEADERS_IN_JUSTIFICATON_ANCESTRY;
		const MAX_HEADER_SIZE: u32 = MAX_HEADER_SIZE;
		const AVERAGE_HEADER_SIZE_IN_JUSTIFICATION: u32 = AVERAGE_HEADER_SIZE_IN_JUSTIFICATION;
	}
}

parameter_types! {
	/// A number of Polkadot mandatory headers that are accepted for free at every
	/// **this chain** block.
	pub const MaxFreePolkadotHeadersPerBlock: u32 = 4;
	/// A number of Polkadot header digests that we keep in the storage.
	pub const PolkadotHeadersToKeep: u32 = 1024;
	/// A name of parachains pallet at Pokadot.
	pub const AtPolkadotParasPalletName: &'static str = bp_polkadot::PARAS_PALLET_NAME;

	/// Chain identifier of Polkadot Bridge Hub.
	pub const BridgeHubPolkadotChainId: ChainId = bp_runtime::BRIDGE_HUB_POLKADOT_CHAIN_ID;
	/// A number of Polkadot Bridge Hub head digests that we keep in the storage.
	pub const BridgeHubPolkadotHeadsToKeep: u32 = 1024;
	/// A maximal size of Polkadot Bridge Hub head digest.
	pub const MaxPolkadotBrdgeHubHeadSize: u32 = bp_polkadot::MAX_NESTED_PARACHAIN_HEAD_DATA_SIZE;

	/// All active outbound lanes.
	pub const ActiveOutboundLanes: &'static [LaneId] = &[XCM_LANE];
	/// Maximal number of unrewarded relayer entries.
	pub const MaxUnrewardedRelayerEntriesAtInboundLane: MessageNonce =
		bp_bridge_hub_polkadot::MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX;
	/// Maximal number of unconfirmed messages.
	pub const MaxUnconfirmedMessagesAtInboundLane: MessageNonce =
		bp_bridge_hub_polkadot::MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX;
}

/// An instance of `pallet_bridge_grandpa` used to bridge with Polkadot.
pub type WithPolkadotBridgeGrandpaInstance = ();
/// An instance of `pallet_bridge_parachains` used to bridge with Polkadot.
pub type WithPolkadotBridgeParachainsInstance = ();
/// An instance of `pallet_bridge_messages` used to bridge with Polkadot Bridge Hub.
pub type WithBridgeHubPolkadotMessagesInstance = ();

impl pallet_bridge_grandpa::Config<WithPolkadotBridgeGrandpaInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = (); // TODO

	type BridgedChain = bp_polkadot::Polkadot;
	type MaxFreeMandatoryHeadersPerBlock = MaxFreePolkadotHeadersPerBlock;
	type HeadersToKeep = PolkadotHeadersToKeep;
}

impl pallet_bridge_parachains::Config<WithPolkadotBridgeParachainsInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = (); // TODO

	type BridgesGrandpaPalletInstance = WithPolkadotBridgeGrandpaInstance;
	type ParasPalletName = AtPolkadotParasPalletName;
	type ParaStoredHeaderDataBuilder =
		SingleParaStoredHeaderDataBuilder<bp_bridge_hub_polkadot::BridgeHubPolkadot>;
	type HeadsToKeep = BridgeHubPolkadotHeadsToKeep;
	type MaxParaHeadDataSize = MaxPolkadotBrdgeHubHeadSize;
}

impl pallet_bridge_messages::Config<WithBridgeHubPolkadotMessagesInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = (); // TODO

	type BridgedChainId = BridgeHubPolkadotChainId;
	type ActiveOutboundLanes = ActiveOutboundLanes;
	type MaxUnrewardedRelayerEntriesAtInboundLane = MaxUnrewardedRelayerEntriesAtInboundLane;
	type MaxUnconfirmedMessagesAtInboundLane = MaxUnconfirmedMessagesAtInboundLane;

	type MaximalOutboundPayloadSize = FromThisChainMaximalOutboundPayloadSize<WithBridgeHubPolkadotMessageBridge>;
	type OutboundPayload = XcmAsPlainPayload;

	type InboundPayload = XcmAsPlainPayload;
	type InboundRelayer = AccountId;
	type DeliveryPayments = ();

	type TargetHeaderChain = TargetHeaderChainAdapter<WithBridgeHubPolkadotMessageBridge>;
	type LaneMessageVerifier = FromThisChainMessageVerifier<WithBridgeHubPolkadotMessageBridge>;
	type DeliveryConfirmationPayments = ();

	type SourceHeaderChain = SourceHeaderChainAdapter<WithBridgeHubPolkadotMessageBridge>;
	type MessageDispatch = ForbidInboundMessages<(), Self::InboundPayload>; // TODO: no XCM configuration
	type OnMessagesDelivered = ();
}

/// Message bridge with Polkadot Bridge Hub.
pub struct WithBridgeHubPolkadotMessageBridge;

impl MessageBridge for WithBridgeHubPolkadotMessageBridge {
	const BRIDGED_MESSAGES_PALLET_NAME: &'static str =
		bp_polkadot_bulletin_chain::WITH_POLKADOT_BULLETIN_CHAIN_MESSAGES_PALLET_NAME;
	type ThisChain = PolkadotBulletinChain;
	type BridgedChain = BridgeHubPolkadot;
	type BridgedHeaderChain = pallet_bridge_parachains::ParachainHeaders<
		Runtime,
		WithPolkadotBridgeParachainsInstance,
		bp_bridge_hub_polkadot::BridgeHubPolkadot,
	>;
}

/// BridgeHubPolkadot chain from message lane point of view.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct BridgeHubPolkadot;

impl UnderlyingChainProvider for BridgeHubPolkadot {
	type Chain = bp_bridge_hub_polkadot::BridgeHubPolkadot;
}

impl BridgedChainWithMessages for BridgeHubPolkadot {}

/// BridgeHubRococo chain from message lane point of view.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct PolkadotBulletinChain;

impl UnderlyingChainProvider for PolkadotBulletinChain {
	type Chain = bp_polkadot_bulletin_chain::PolkadotBulletinChain;
}

impl ThisChainWithMessages for PolkadotBulletinChain {
	type RuntimeOrigin = RuntimeOrigin;
}
